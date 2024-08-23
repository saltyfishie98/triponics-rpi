use bevy_app::{Plugin, Update};
use bevy_ecs::{
    event::{Event, EventReader},
    system::{Res, Resource},
};
use bevy_tokio_tasks::TokioTasksRuntime;

use futures::StreamExt;
#[allow(unused_imports)]
use tracing as log;

#[derive(Debug, Clone, Copy)]
#[repr(i32)]
pub enum Qos {
    _0 = paho_mqtt::QOS_0,
    _1 = paho_mqtt::QOS_1,
    _2 = paho_mqtt::QOS_2,
}

#[derive(Debug, Clone)]
pub enum PersistenceType {
    /// Messages are persisted to files in a local directory (default).
    File,
    /// Messages are persisted to files under the specified directory.
    FilePath(std::path::PathBuf),
    /// No persistence is used.
    None,
}
impl From<&PersistenceType> for paho_mqtt::PersistenceType {
    fn from(value: &PersistenceType) -> Self {
        match value {
            PersistenceType::File => paho_mqtt::PersistenceType::File,
            PersistenceType::FilePath(p) => paho_mqtt::PersistenceType::FilePath(p.clone()),
            PersistenceType::None => paho_mqtt::PersistenceType::None,
        }
    }
}

#[derive(Debug, Clone, Resource)]
pub struct MqttCreateOptions {
    server_uri: &'static str,
    client_id: Option<&'static str>,
    stream_buffer_size: usize,
    max_buffered_messages: Option<i32>,
    persistence_type: Option<PersistenceType>,
    send_while_disconnected: Option<bool>,
    allow_disconnected_send_at_anytime: Option<bool>,
    delete_oldest_messages: Option<bool>,
    restore_messages: Option<bool>,
    persist_qos0: Option<bool>,
}
impl Default for MqttCreateOptions {
    fn default() -> Self {
        Self {
            server_uri: "mqtt://test.mosquitto.org",
            client_id: Default::default(),
            stream_buffer_size: 10,
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
            stream_buffer_size: _,
        } = value;

        let builder = paho_mqtt::CreateOptionsBuilder::new().server_uri(*server_uri);

        let builder = if let Some(client_id) = *client_id {
            builder.client_id(client_id)
        } else {
            builder
        };

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

#[derive(Debug, Resource)]
struct Subscriptions(Vec<(&'static str, Qos)>);

#[derive(Resource)]
pub struct MqttClient {
    inner_client: paho_mqtt::AsyncClient,
    recv_task: tokio::task::JoinHandle<()>,
}

#[derive(Debug, Event)]
pub struct RestartClient;

#[derive(Debug, Default)]
pub struct MqttPlugin {
    pub client_create_options: MqttCreateOptions,
    pub subscriptions: Option<&'static [(&'static str, Qos)]>,
}
impl Plugin for MqttPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        let Self {
            client_create_options,
            subscriptions,
        } = self;

        let subs = if let Some(subs) = *subscriptions {
            subs.to_vec()
        } else {
            Vec::new()
        };

        app.add_event::<RestartClient>()
            .insert_resource(client_create_options.clone())
            .insert_resource(Subscriptions(subs))
            .add_systems(Update, restart_client)
            .world_mut()
            .send_event(RestartClient);
    }
}

fn restart_client(
    rt: Res<TokioTasksRuntime>,
    mut ev: EventReader<RestartClient>,
    client_create_options: Res<MqttCreateOptions>,
) {
    if ev.is_empty() {
        return;
    } else {
        ev.clear();
    }

    log::info!("restarting mqtt client!");
    let client_create_options = client_create_options.clone();

    rt.spawn_background_task(|mut ctx| async move {
        let client = {
            let mut inner_client =
                paho_mqtt::AsyncClient::new(paho_mqtt::CreateOptions::from(&client_create_options))
                    .unwrap_or_else(|err| {
                        println!("Error creating the client: {}", err);
                        std::process::exit(1);
                    });

            // let mut inner_client = paho_mqtt::AsyncClient::new(
            //     paho_mqtt::CreateOptionsBuilder::new()
            //         .server_uri("mqtt://test.mosquitto.org")
            //         .client_id("rust_async_sub_v5")
            //         .finalize(),
            // )
            // .unwrap_or_else(|err| {
            //     println!("Error creating the client: {}", err);
            //     std::process::exit(1);
            // });

            let conn_opts = paho_mqtt::ConnectOptionsBuilder::with_mqtt_version(
                paho_mqtt::MQTT_VERSION_5,
            )
            .clean_start(false)
            .properties(
                paho_mqtt::properties![paho_mqtt::PropertyCode::SessionExpiryInterval => 3600],
            )
            .finalize();

            let stream = inner_client.get_stream(25);
            inner_client.connect(conn_opts);
            let recv_task = tokio::task::spawn(mqtt_recv_task(stream));

            MqttClient {
                inner_client,
                recv_task,
            }
        };

        ctx.run_on_main_thread(move |ctx| {
            let world = ctx.world;
            world.remove_resource::<MqttClient>();

            let Subscriptions(subs) = world.get_resource::<Subscriptions>().unwrap();

            let (topics, qos_s) = subs.iter().fold(
                (Vec::new(), Vec::new()),
                |(mut topics, mut qos_s), (topic, qos)| {
                    topics.push(*topic);
                    qos_s.push(*qos as i32);
                    (topics, qos_s)
                },
            );

            log::debug!("subscribing to topic: {:?}", topics);
            client.inner_client.subscribe_many(&topics, &qos_s);

            world.insert_resource(client);
        })
        .await;

        log::info!("mqtt client restarted!");
    });
}

async fn mqtt_recv_task(mut stream: paho_mqtt::AsyncReceiver<Option<paho_mqtt::Message>>) {
    log::trace!("polling mqtt stream");
    while let Some(Some(msg)) = stream.next().await {
        log::info!("{}", msg);
    }
}
