use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};

use bevy_app::{Plugin, Startup, Update};
use bevy_ecs::{
    event::EventReader,
    schedule::IntoSystemConfigs,
    system::{Query, Res, Resource},
};
use bevy_tokio_tasks::TokioTasksRuntime;
use futures::StreamExt;

use crate::helper::AppExtensions;
#[allow(unused_imports)]
use tracing as log;

#[derive(Default)]
pub struct MqttPlugin {
    pub client_create_options: MqttCreateOptions,
    pub initial_subscriptions: &'static [(&'static str, Qos)],
}
impl Plugin for MqttPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        let (mqtt_message_tx, receiver) = std::sync::mpsc::channel::<event::MqttMessage>();

        let Self {
            client_create_options,
            initial_subscriptions: subscriptions,
        } = self;

        app.insert_resource(client_create_options.clone())
            .insert_resource(Subscriptions(subscriptions.to_vec()))
            .insert_resource(MqttMessageSender(mqtt_message_tx))
            .insert_resource(MqttFileManager::new(
                client_create_options.client_id,
                client_create_options.offline_storage_path.as_path(),
            ))
            .add_event::<event::RestartClient>()
            .add_event_channel(receiver)
            .add_systems(Startup, system::setup_offline_path)
            .add_systems(
                Update,
                (
                    system::restart_client,
                    system::publish_offline_cache,
                    system::publish_message.after(system::publish_offline_cache),
                ),
            )
            .world_mut()
            .send_event(event::RestartClient);
    }
}

mod system {
    use bevy_ecs::event::EventWriter;

    use super::component;
    use super::*;

    pub fn setup_offline_path() {}

    pub fn restart_client(
        rt: Res<TokioTasksRuntime>,
        client_create_options: Res<MqttCreateOptions>,
        mqtt_msg_tx: Res<MqttMessageSender>,
        mut ev: EventReader<event::RestartClient>,
    ) {
        if ev.is_empty() {
            return;
        } else {
            ev.clear();
        }

        static mut RESTARTING: bool = false;
        unsafe {
            if RESTARTING {
                log::info!("mqtt client is already restarting");
                return;
            }

            RESTARTING = true;
        }

        log::info!("restarting mqtt client!");

        let client_create_options = client_create_options.clone();
        let (start_fence_tx, start_fence_rx) = tokio::sync::oneshot::channel::<()>();
        let mqtt_msg_tx = mqtt_msg_tx.0.clone();

        rt.spawn_background_task(|mut ctx| async move {
            async fn mqtt_recv_task(
                mqtt_msg_tx: std::sync::mpsc::Sender<event::MqttMessage>,
                start_fence_rx: tokio::sync::oneshot::Receiver<()>,
                mut stream: paho_mqtt::AsyncReceiver<Option<paho_mqtt::Message>>,
            ) {
                let _ = start_fence_rx.await;

                while let Some(Some(msg)) = stream.next().await {
                    log::trace!("polled mqtt msg -> {msg}");
                    if let Err(e) = mqtt_msg_tx.send(event::MqttMessage(msg)) {
                        log::warn!("{e}");
                    }
                }
            }

            let mut inner_client =
                paho_mqtt::AsyncClient::new(paho_mqtt::CreateOptions::from(&client_create_options))
                    .unwrap_or_else(|err| {
                        println!("Error creating the client: {}", err);
                        std::process::exit(1);
                    });

            let conn_opts = paho_mqtt::ConnectOptionsBuilder::with_mqtt_version(
                paho_mqtt::MQTT_VERSION_5,
            )
            .clean_start(false)
            .properties(
                paho_mqtt::properties![paho_mqtt::PropertyCode::SessionExpiryInterval => 3600],
            )
            .finalize();

            match inner_client.connect(conn_opts).await {
                Err(e) => {
                    log::warn!("{e}");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    ctx.run_on_main_thread(move |ctx| ctx.world.send_event(event::RestartClient))
                        .await;
                }
                Ok(_) => {
                    let recv_task = tokio::spawn(mqtt_recv_task(
                        mqtt_msg_tx,
                        start_fence_rx,
                        inner_client.get_stream(client_create_options.request_channel_capacity),
                    ));

                    let client = MqttClient {
                        inner_client,
                        recv_task,
                    };

                    let (topics, qos_s) = ctx
                        .run_on_main_thread(move |ctx| {
                            let world = ctx.world;
                            if let Some(current_client) = world.remove_resource::<MqttClient>() {
                                current_client.recv_task.abort();
                            }

                            let Subscriptions(subs) =
                                world.get_resource::<Subscriptions>().unwrap();

                            subs.iter().fold(
                                (Vec::new(), Vec::new()),
                                |(mut topics, mut qos_s), (topic, qos)| {
                                    topics.push(*topic);
                                    qos_s.push(*qos as i32);
                                    (topics, qos_s)
                                },
                            )
                        })
                        .await;

                    log::debug!("subscribing to topic: {:?}", topics);
                    if let Err(e) = client.inner_client.subscribe_many(&topics, &qos_s).await {
                        log::warn!("error on restart subscription, reason: {}", e);
                    }
                    let _ = start_fence_tx.send(());

                    ctx.run_on_main_thread(move |ctx| {
                        ctx.world.insert_resource(client);
                    })
                    .await;
                }
            }

            log::info!("mqtt client restarted!");

            unsafe {
                RESTARTING = false;
            }
        });
    }

    pub fn publish_message(
        rt: Res<TokioTasksRuntime>,
        mut query: Query<&mut component::PublishMsg>,
    ) {
        let process_msg = |msg: component::PublishMsgInner| {
            rt.spawn_background_task(|mut ctx| async move {
                ctx.run_on_main_thread(move |ctx| {
                    let offline_data = match ctx.world.get_resource::<MqttClient>() {
                        Some(client) => {
                            if client.inner_client.is_connected() {
                                client.inner_client.publish(msg.into());
                                None
                            } else {
                                ctx.world.send_event(event::RestartClient);
                                Some(msg)
                            }
                        }
                        None => Some(msg),
                    };

                    if let Some(to_file) = offline_data {
                        let mut cache = ctx.world.get_resource_mut::<MqttFileManager>().unwrap();
                        log::info!("cached msg: {:?}", to_file);
                        cache.add(&to_file).unwrap();
                    }
                })
                .await;
            });
        };

        query
            .iter_mut()
            .flat_map(|mut data| data.msg.take())
            .for_each(process_msg);
    }

    pub fn publish_offline_cache(
        client: Option<Res<MqttClient>>,
        mut restarter: EventWriter<event::RestartClient>,
    ) {
        if client.is_none() {
            log::trace!("publish cache -> blocked by unavailable mqtt client");
            return;
        }

        let client = client.unwrap();

        if !client.inner_client.is_connected() {
            log::trace!("publish cache -> blocked by mqtt client not connected");
            restarter.send(event::RestartClient);
            return;
        }

        todo!()
    }
}

pub mod component {
    use bevy_ecs::component::Component;

    use super::Qos;

    #[derive(serde::Serialize, serde::Deserialize)]
    pub(super) struct PublishMsgInner {
        topic: String,
        payload: Vec<u8>,
        qos: Qos,
    }
    impl std::fmt::Debug for PublishMsgInner {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("PublishMsgInner")
                .field("topic", &self.topic)
                .field("payload", &std::str::from_utf8(&self.payload).unwrap())
                .field("qos", &self.qos)
                .finish()
        }
    }
    impl From<PublishMsgInner> for paho_mqtt::Message {
        fn from(value: PublishMsgInner) -> Self {
            let PublishMsgInner {
                topic,
                payload,
                qos,
            } = value;
            Self::new(topic, payload, qos as i32)
        }
    }

    #[derive(Debug, Component)]
    pub struct PublishMsg {
        pub(super) msg: Option<PublishMsgInner>,
    }

    impl PublishMsg {
        pub fn new(topic: impl Into<String>, payload: impl Into<Vec<u8>>, qos: Qos) -> Self {
            Self {
                msg: Some(PublishMsgInner {
                    topic: topic.into(),
                    payload: payload.into(),
                    qos,
                }),
            }
        }
    }
}

pub mod event {
    use bevy_ecs::event::Event;

    #[derive(Debug, Event)]
    pub struct RestartClient;

    #[derive(Debug, Event)]
    pub struct MqttMessage(pub paho_mqtt::Message);
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[repr(i32)]
pub enum Qos {
    _0 = paho_mqtt::QOS_0,
    _1 = paho_mqtt::QOS_1,
    _2 = paho_mqtt::QOS_2,
}

#[derive(Clone)]
pub enum PersistenceType {
    /// Messages are persisted to files in a local directory (default).
    File,
    /// Messages are persisted to files under the specified directory.
    FilePath(std::path::PathBuf),
    /// No persistence is used.
    None,
    User(fn() -> Box<dyn paho_mqtt::ClientPersistence + Send>),
}
impl From<&PersistenceType> for paho_mqtt::PersistenceType {
    fn from(value: &PersistenceType) -> Self {
        match value {
            PersistenceType::File => paho_mqtt::PersistenceType::File,
            PersistenceType::FilePath(p) => paho_mqtt::PersistenceType::FilePath(p.clone()),
            PersistenceType::None => paho_mqtt::PersistenceType::None,
            PersistenceType::User(make_user_persist) => {
                paho_mqtt::PersistenceType::User(Box::new(make_user_persist()))
            }
        }
    }
}

#[derive(Clone, Resource)]
pub struct MqttCreateOptions {
    pub server_uri: &'static str,
    pub client_id: &'static str,
    pub offline_storage_path: PathBuf,
    pub request_channel_capacity: usize,
    pub max_buffered_messages: Option<i32>,
    pub persistence_type: Option<PersistenceType>,
    pub send_while_disconnected: Option<bool>,
    pub allow_disconnected_send_at_anytime: Option<bool>,
    pub delete_oldest_messages: Option<bool>,
    pub restore_messages: Option<bool>,
    pub persist_qos0: Option<bool>,
}
impl Default for MqttCreateOptions {
    fn default() -> Self {
        Self {
            server_uri: "mqtt://test.mosquitto.org",
            client_id: Default::default(),
            request_channel_capacity: 10,
            offline_storage_path: std::env::current_dir().unwrap(),

            max_buffered_messages: Default::default(),
            persistence_type: Default::default(),
            send_while_disconnected: Default::default(),
            allow_disconnected_send_at_anytime: Default::default(),
            delete_oldest_messages: Default::default(),
            restore_messages: Default::default(),
            persist_qos0: Default::default(),
        }
    }
}
impl From<&'static str> for MqttCreateOptions {
    fn from(server_uri: &'static str) -> Self {
        Self {
            server_uri,
            ..Default::default()
        }
    }
}
impl From<&MqttCreateOptions> for paho_mqtt::CreateOptions {
    fn from(value: &MqttCreateOptions) -> Self {
        let MqttCreateOptions {
            server_uri,
            client_id,
            max_buffered_messages,
            persistence_type,
            send_while_disconnected,
            allow_disconnected_send_at_anytime,
            delete_oldest_messages,
            restore_messages,
            persist_qos0,
            request_channel_capacity: _,
            offline_storage_path: _,
        } = value;

        let builder = paho_mqtt::CreateOptionsBuilder::new()
            .server_uri(*server_uri)
            .client_id(*client_id);

        let builder = if let Some(n) = *max_buffered_messages {
            builder.max_buffered_messages(n)
        } else {
            builder
        };

        let builder = if let Some(persist) = persistence_type {
            builder.persistence(persist)
        } else {
            builder
        };

        let builder = if let Some(on) = *send_while_disconnected {
            builder.send_while_disconnected(on)
        } else {
            builder
        };

        let builder = if let Some(allow) = *allow_disconnected_send_at_anytime {
            builder.allow_disconnected_send_at_anytime(allow)
        } else {
            builder
        };

        let builder = if let Some(delete_oldest) = *delete_oldest_messages {
            builder.delete_oldest_messages(delete_oldest)
        } else {
            builder
        };

        let builder = if let Some(restore) = *restore_messages {
            builder.restore_messages(restore)
        } else {
            builder
        };

        let builder = if let Some(persist) = *persist_qos0 {
            builder.persist_qos0(persist)
        } else {
            builder
        };

        builder.finalize()
    }
}

#[derive(Resource)]
pub struct MqttClient {
    inner_client: paho_mqtt::AsyncClient,
    recv_task: tokio::task::JoinHandle<()>,
}

#[derive(Debug, Resource)]
struct MqttMessageSender(std::sync::mpsc::Sender<event::MqttMessage>);

#[derive(Debug, Resource)]
struct Subscriptions(Vec<(&'static str, Qos)>);

#[derive(Debug, Resource)]
struct MqttFileManager {
    pub file: File,
}
impl MqttFileManager {
    fn new(client_id: &'static str, path: &Path) -> Self {
        let mut path = PathBuf::from(path);
        path.push("cache");
        path.push(format!("{client_id}.mqtt"));

        std::fs::create_dir_all(path.parent().unwrap()).unwrap();

        Self {
            file: std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .read(true)
                .open(path)
                .unwrap(),
        }
    }

    fn add(&mut self, msg: &component::PublishMsgInner) -> std::io::Result<()> {
        let mut to_file = postcard::to_stdvec(msg).unwrap();
        to_file.push(b'\n');
        self.file.write_all(&to_file)
    }

    fn read(&mut self) -> Vec<component::PublishMsgInner> {}
}
