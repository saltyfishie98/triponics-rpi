use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
    time::Duration,
};

use bevy_app::{Plugin, Startup, Update};
use bevy_ecs::{
    event::EventReader,
    system::{Query, Res, Resource},
};
use bevy_tokio_tasks::TokioTasksRuntime;
use futures::StreamExt;
use tokio::sync::Mutex;

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
            .insert_resource(MqttCacheManager::new(
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
                    system::publish_message,
                ),
            )
            .world_mut()
            .send_event(event::RestartClient);
    }
}

#[derive(Debug, Resource)]
struct MqttCacheManager {
    connection: &'static Mutex<rusqlite::Connection>,
}
impl MqttCacheManager {
    fn new(client_id: &'static str, path: &Path) -> Self {
        let mut path = PathBuf::from(path);
        path.push("cache");
        path.push(format!("{client_id}.db3"));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();

        let conn = rusqlite::Connection::open(path).unwrap();
        conn.execute(include_str!("sql/create_table.sql"), ())
            .unwrap();

        static DB_CONNECTION: OnceLock<Mutex<rusqlite::Connection>> = OnceLock::new();
        let connection = DB_CONNECTION.get_or_init(|| Mutex::new(conn));

        Self { connection }
    }

    async fn add(
        conn: &'static Mutex<rusqlite::Connection>,
        msg: &component::PublishMsg,
    ) -> anyhow::Result<()> {
        let conn = conn.lock().await;

        let msg_1 = msg.clone();
        let out = Ok(conn
            .execute(
                include_str!("sql/add_data.sql"),
                (
                    time::OffsetDateTime::now_utc().unix_timestamp(),
                    postcard::to_allocvec(msg)?,
                ),
            )
            .map(|_| ())?);

        log::trace!("cached {msg_1:?}");
        out
    }

    async fn read(
        conn: &'static Mutex<rusqlite::Connection>,
        count: u32,
    ) -> anyhow::Result<Vec<component::PublishMsg>> {
        let conn = conn.lock().await;

        let mut stmt = conn.prepare(include_str!("sql/read_data.sql"))?;
        let rows = stmt.query_map([count], |row| row.get::<usize, Vec<u8>>(0))?;

        let out = rows
            .map(|data| {
                Ok::<_, anyhow::Error>(postcard::from_bytes::<component::PublishMsg>(&data?)?)
            })
            .collect::<Result<Vec<_>, _>>();

        if let Err(e) = conn.execute(include_str!("sql/delete_data.sql"), [count]) {
            log::warn!("failed to delete cached data, reason {e}");
        }

        out
    }

    // async fn read(&mut self) -> anyhow::Result<()> {
    //     let conn = conn.lock().await;

    //     todo!()
    // }
}

mod system {
    use bevy_ecs::entity::Entity;
    use bevy_ecs::event::EventWriter;
    use bevy_ecs::query::With;
    use bevy_ecs::system::{Commands, ResMut};

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
                log::trace!("started recv task!");

                while let Some(msg) = stream.next().await {
                    match msg {
                        Some(msg) => {
                            log::trace!("polled mqtt msg -> {msg}");
                            if let Err(e) = mqtt_msg_tx.send(event::MqttMessage(msg)) {
                                log::warn!("{e}");
                            }
                        }
                        None => {
                            log::trace!("disconnected");
                        }
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

            log::trace!("setup mqtt configs!");

            match inner_client.connect(conn_opts).await {
                Err(e) => {
                    log::warn!("failed to connect to mqtt broker, reason: {e}");
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

                    if !topics.is_empty() {
                        log::debug!("subscribing to topic: {:?}", topics);
                        if let Err(e) = client.inner_client.subscribe_many(&topics, &qos_s).await {
                            log::warn!("error on restart subscription, reason: {}", e);
                        }
                    }

                    let _ = start_fence_tx.send(());

                    ctx.run_on_main_thread(move |ctx| {
                        ctx.world.insert_resource(client);
                        log::trace!("connected to mqtt broker!");
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
        mut cmd: Commands,
        mut query: Query<&mut component::PublishMsg>,
        rt: Res<TokioTasksRuntime>,
        client: Option<Res<MqttClient>>,
        cache_manager: Res<MqttCacheManager>,
        entt: Query<Entity, With<component::PublishMsg>>,
    ) {
        let process_msg = move |(msg, maybe_client, cache): (
            component::PublishMsg,
            Option<paho_mqtt::AsyncClient>,
            &'static Mutex<rusqlite::Connection>,
        )| {
            rt.spawn_background_task(move |_| async move {
                match maybe_client {
                    Some(client) => {
                        let res = client.publish(msg.clone().into()).await;

                        if let Err(e) = res {
                            log::warn!("failed to publish mqtt msg, reason: {e}");
                            MqttCacheManager::add(cache, &msg).await.unwrap();
                        } else {
                            log::trace!("published -> {msg:?}");
                        }
                    }
                    None => {
                        MqttCacheManager::add(cache, &msg).await.unwrap();
                    }
                }
            });
        };

        match client {
            Some(client) => {
                query
                    .iter_mut()
                    .map(|msg| {
                        (
                            msg.clone(),
                            Some(client.inner_client.clone()),
                            cache_manager.connection,
                        )
                    })
                    .for_each(process_msg);
            }
            None => {
                query
                    .iter_mut()
                    .map(|msg| (msg.clone(), None, cache_manager.connection))
                    .for_each(process_msg);
            }
        }

        entt.iter().for_each(|entt| {
            cmd.entity(entt).remove::<component::PublishMsg>();
        });
    }

    pub fn publish_offline_cache(
        mut cmd: Commands,
        mut restarter: EventWriter<event::RestartClient>,
        rt: Res<TokioTasksRuntime>,
        cache: Res<MqttCacheManager>,
        client: Option<ResMut<MqttClient>>,
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

        let conn = cache.connection;

        let res = rt
            .runtime()
            .block_on(async { MqttCacheManager::read(conn, 100).await });

        match res {
            Ok(msg_vec) => msg_vec.into_iter().for_each(|msg| {
                cmd.spawn(msg);
            }),
            Err(e) => log::warn!("failed to read from cache, reason: {e}"),
        }
    }
}

pub mod component {
    use std::sync::Arc;

    use bevy_ecs::component::Component;

    use super::Qos;

    #[derive(Component, serde::Serialize, serde::Deserialize, Clone)]
    pub struct PublishMsg {
        #[serde(
            serialize_with = "crate::helper::serialize_arc_str",
            deserialize_with = "crate::helper::deserialize_arc_str"
        )]
        pub(super) topic: Arc<str>,
        #[serde(
            serialize_with = "crate::helper::serialize_arc_bytes",
            deserialize_with = "crate::helper::deserialize_arc_bytes"
        )]
        pub(super) payload: Arc<[u8]>,
        pub(super) qos: Qos,
    }
    impl PublishMsg {
        pub fn new(topic: impl AsRef<str>, payload: impl AsRef<[u8]>, qos: Qos) -> Self {
            Self {
                topic: topic.as_ref().into(),
                payload: payload.as_ref().into(),
                qos,
            }
        }
    }
    impl std::fmt::Debug for PublishMsg {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("PublishMsg")
                .field("topic", &self.topic)
                .field(
                    "payload",
                    &String::from_utf8(self.payload.as_ref().into()).unwrap(),
                )
                .field("qos", &self.qos)
                .finish()
        }
    }
    impl From<PublishMsg> for paho_mqtt::Message {
        fn from(value: PublishMsg) -> Self {
            let PublishMsg {
                topic,
                payload,
                qos,
            } = value;
            Self::new(topic.as_ref(), payload.as_ref(), qos as i32)
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
