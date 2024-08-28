use std::{
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
    time::Duration,
};

use bevy_app::{Plugin, Update};
use bevy_ecs::{
    event::EventReader,
    schedule::IntoSystemConfigs,
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
    pub client_create_options: ClientCreateOptions,
    pub client_connect_options: ClientConnectOptions,
    pub initial_subscriptions: &'static [(&'static str, Qos)],
}
impl Plugin for MqttPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        let (mqtt_incoming_msg_queue, mqtt_incoming_msg_rx) =
            std::sync::mpsc::channel::<event::MqttMessage>();

        let Self {
            client_create_options,
            client_connect_options,
            initial_subscriptions: subscriptions,
        } = self;

        app.insert_resource(client_create_options.clone())
            .insert_resource(client_connect_options.clone())
            .insert_resource(Subscriptions(subscriptions.to_vec()))
            .insert_resource(MqttIncommingMsgTx(mqtt_incoming_msg_queue))
            .insert_resource(MqttCacheManager::new(
                client_create_options.client_id,
                client_create_options.cache_dir_path.as_path(),
            ))
            .add_event::<event::RestartClient>()
            .add_event_channel(mqtt_incoming_msg_rx)
            .add_systems(
                Update,
                (
                    system::restart_client,
                    system::publish_offline_cache.after(system::restart_client),
                    system::publish_message
                        .after(system::restart_client)
                        .after(system::publish_offline_cache),
                ),
            )
            .world_mut()
            .send_event(event::RestartClient("initial restart"));
    }
}

mod system {
    use bevy_ecs::entity::Entity;
    use bevy_ecs::event::EventWriter;
    use bevy_ecs::query::With;
    use bevy_ecs::system::{Commands, ResMut};
    use bevy_ecs::world::World;

    use super::component;
    use super::*;

    pub fn restart_client(
        mut cmd: Commands,
        mut ev: EventReader<event::RestartClient>,
        client: Option<Res<MqttClient>>,
    ) {
        async fn mqtt_recv_task(
            mqtt_msg_tx: std::sync::mpsc::Sender<event::MqttMessage>,
            mut stream: paho_mqtt::AsyncReceiver<Option<paho_mqtt::Message>>,
        ) {
            log::debug!("started recv task!");

            while let Some(msg) = stream.next().await {
                match msg {
                    Some(msg) => {
                        log::trace!("polled mqtt msg");
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

        async fn ping_task(client: paho_mqtt::AsyncClient, duration: Duration) {
            let ping_interval = Duration::from_secs_f32(duration.as_secs_f32() * 0.7);
            log::debug!("ping interval: {}", ping_interval.as_secs_f32());

            loop {
                log::trace!("ping mqtt");
                client.publish(paho_mqtt::Message::new(
                    format!("{}/ping", client.client_id()),
                    [0u8],
                    paho_mqtt::QOS_0,
                ));
                tokio::time::sleep(ping_interval).await;
            }
        }

        async fn make_client(
            create_opts: paho_mqtt::CreateOptions,
            conn_opts: paho_mqtt::ConnectOptions,
            stream_size: usize,
            (topics, qos): (Vec<&str>, Vec<i32>),
        ) -> Result<
            (
                paho_mqtt::AsyncClient,
                paho_mqtt::AsyncReceiver<Option<paho_mqtt::Message>>,
            ),
            paho_mqtt::Error,
        > {
            // Create the client connection
            let mut client = paho_mqtt::AsyncClient::new(create_opts).unwrap_or_else(|e| {
                println!("Error creating the client: {:?}", e);
                std::process::exit(1);
            });

            client.set_connected_callback(|_| log::trace!("CALLBACK: mqtt client connected"));
            client.set_disconnected_callback(|_, _, _| {
                log::trace!("CALLBACK: mqtt client disconnected")
            });

            let strm = client.get_stream(stream_size);
            client.connect(conn_opts).await?;
            client.subscribe_many(&topics, &qos).await?;

            Ok((client, strm))
        }

        if ev.is_empty() {
            return;
        }

        let reason = Box::new(ev.read().next().unwrap().0);
        ev.clear();

        if let Some(client) = client {
            if client.inner_client.is_connected() {
                log::debug!("mqtt client already connected");
                return;
            }
        }

        cmd.add(|world: &mut World| {
            if let Some(old_client) = world.remove_resource::<MqttClient>() {
                log::debug!("removed old client");
                let MqttClient {
                    inner_client,
                    recv_task,
                    ping_task,
                } = old_client;

                inner_client.disconnect(None);
                recv_task.abort();
                ping_task.abort();
            }

            let connect_opts = world
                .get_resource::<ClientConnectOptions>()
                .unwrap()
                .clone();

            match world.get_resource::<RestartTaskHandle>() {
                None => {
                    let (tx, rx) = crossbeam_channel::bounded::<NewMqttClient>(1);
                    world.insert_resource(NewMqttClientRecv(rx));

                    let handle = {
                        let rt = world.get_resource::<TokioTasksRuntime>().unwrap();
                        let create_opts =
                            world.get_resource::<ClientCreateOptions>().unwrap().clone();
                        let Subscriptions(subscriptions) =
                            world.get_resource::<Subscriptions>().unwrap();

                        let paho_subs = subscriptions.iter().map(|(t, q)| (*t, *q as i32)).unzip();
                        let paho_create_opts = paho_mqtt::CreateOptions::from(&create_opts);
                        let paho_conn_opts = paho_mqtt::ConnectOptions::from(&connect_opts);

                        rt.spawn_background_task(move |_| async move {
                            log::trace!("restart mqtt client, reason: {reason}");
                            tx.send(NewMqttClient(
                                make_client(
                                    paho_create_opts,
                                    paho_conn_opts,
                                    create_opts.incoming_msg_buffer_size,
                                    paho_subs,
                                )
                                .await,
                            ))
                            .unwrap();
                            loop {
                                tokio::task::yield_now().await;
                            }
                        })
                    };

                    world.insert_resource(RestartTaskHandle(handle));
                }
                Some(_) => {
                    let NewMqttClientRecv(rx) = world.get_resource::<NewMqttClientRecv>().unwrap();

                    match rx.try_recv() {
                        Err(crossbeam_channel::TryRecvError::Disconnected) => panic!(),
                        Err(crossbeam_channel::TryRecvError::Empty) => {
                            log::trace!("mqtt client restarting...");
                            return;
                        }
                        Ok(NewMqttClient(Ok((client, stream)))) => {
                            let rt = world.get_resource::<TokioTasksRuntime>().unwrap();
                            let MqttIncommingMsgTx(tx) =
                                world.get_resource::<MqttIncommingMsgTx>().unwrap();

                            let mqtt_incoming_msg_queue = tx.clone();

                            let recv_task = rt.spawn_background_task(|_| async move {
                                mqtt_recv_task(mqtt_incoming_msg_queue, stream).await;
                            });

                            let ping_task = {
                                let ping_client = client.clone();
                                rt.spawn_background_task(move |_| async move {
                                    if let Some(duration) = connect_opts.keep_alive_interval {
                                        ping_task(ping_client, duration).await;
                                    }
                                })
                            };

                            log::info!("mqtt client restarted");
                            world.insert_resource(MqttClient {
                                inner_client: client,
                                recv_task,
                                ping_task,
                            })
                        }
                        Ok(NewMqttClient(Err(e))) => {
                            log::warn!("failed to restart mqtt client, reason: {e}");
                            world.send_event(event::RestartClient("failed to connect"));
                        }
                    }

                    if let Some(RestartTaskHandle(handle)) =
                        world.remove_resource::<RestartTaskHandle>()
                    {
                        handle.abort();
                    }
                }
            }
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
            restarter.send(event::RestartClient("client not present"));
            return;
        }

        let client = client.unwrap();

        if !client.inner_client.is_connected() {
            log::trace!("publish cache -> blocked by mqtt client not connected");
            restarter.send(event::RestartClient("client not connected"));
            return;
        }

        let conn = cache.connection;

        let res = rt
            .runtime()
            .block_on(async { MqttCacheManager::read(conn, 10).await });

        match res {
            Ok(msg_vec) => msg_vec.into_iter().for_each(|msg| {
                cmd.spawn(msg);
            }),
            Err(e) => log::warn!("failed to read from cache, reason: {e}"),
        }
    }

    pub fn publish_message(
        mut cmd: Commands,
        mut query: Query<&mut component::PublishMsg>,
        rt: Res<TokioTasksRuntime>,
        client: Option<Res<MqttClient>>,
        cache_manager: Res<MqttCacheManager>,
        entt: Query<Entity, With<component::PublishMsg>>,
    ) {
        fn process_msg(
            (rt, msg, maybe_client, cache): (
                &Res<TokioTasksRuntime>,
                component::PublishMsg,
                Option<paho_mqtt::AsyncClient>,
                &'static Mutex<rusqlite::Connection>,
            ),
        ) {
            rt.spawn_background_task(move |_| async move {
                log::debug!("received -> {msg:?}");
                match maybe_client {
                    Some(client) => match client.try_publish(msg.clone().into()) {
                        Ok(o) => {
                            log::debug!("published msg -> {}", o.message());
                        }
                        Err(e) => {
                            log::warn!("failed to publish message, reason {e}");
                            MqttCacheManager::add(cache, &msg).await.unwrap();
                        }
                    },
                    None => {
                        MqttCacheManager::add(cache, &msg).await.unwrap();
                    }
                }
            });
        }

        match client {
            Some(client) => {
                if !client.inner_client.is_connected() {
                    log::trace!("not connected");
                    return;
                }

                query
                    .iter_mut()
                    .map(|msg| {
                        (
                            &rt,
                            msg.clone(),
                            Some(client.inner_client.clone()),
                            cache_manager.connection,
                        )
                    })
                    .for_each(process_msg);
            }
            None => {
                log::trace!("no client");
                query
                    .iter_mut()
                    .map(|msg| (&rt, msg.clone(), None, cache_manager.connection))
                    .for_each(process_msg);
            }
        }

        entt.iter().for_each(|entt| {
            cmd.entity(entt).remove::<component::PublishMsg>();
        });
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
    pub struct RestartClient(pub &'static str);

    #[derive(Debug, Event)]
    pub struct MqttMessage(pub paho_mqtt::Message);
}

#[derive(Debug, Resource)]
struct MqttCacheManager {
    connection: &'static Mutex<rusqlite::Connection>,
}
impl MqttCacheManager {
    fn new(client_id: &'static str, path: &Path) -> Self {
        let mut path = PathBuf::from(path);
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

        log::debug!("cached -> {msg_1:?}");
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
pub struct ClientCreateOptions {
    pub server_uri: &'static str,
    pub client_id: &'static str,
    pub cache_dir_path: PathBuf,
    pub incoming_msg_buffer_size: usize,
    pub max_buffered_messages: Option<i32>,
    pub persistence_type: Option<PersistenceType>,
    pub send_while_disconnected: Option<bool>,
    pub allow_disconnected_send_at_anytime: Option<bool>,
    pub delete_oldest_messages: Option<bool>,
    pub restore_messages: Option<bool>,
    pub persist_qos0: Option<bool>,
}
impl Default for ClientCreateOptions {
    fn default() -> Self {
        let mut cache_dir_path = std::env::current_dir().unwrap();
        cache_dir_path.push("cache");

        let mut persist_path = cache_dir_path.clone();
        persist_path.push("paho");

        Self {
            server_uri: "mqtt://test.mosquitto.org",
            client_id: Default::default(),
            incoming_msg_buffer_size: 25,
            cache_dir_path,
            persistence_type: Some(PersistenceType::FilePath(persist_path)),

            max_buffered_messages: Default::default(),
            send_while_disconnected: Default::default(),
            allow_disconnected_send_at_anytime: Default::default(),
            delete_oldest_messages: Default::default(),
            restore_messages: Default::default(),
            persist_qos0: Default::default(),
        }
    }
}
impl From<&'static str> for ClientCreateOptions {
    fn from(server_uri: &'static str) -> Self {
        Self {
            server_uri,
            ..Default::default()
        }
    }
}
impl From<&ClientCreateOptions> for paho_mqtt::CreateOptions {
    fn from(value: &ClientCreateOptions) -> Self {
        let ClientCreateOptions {
            server_uri,
            client_id,
            max_buffered_messages,
            persistence_type,
            send_while_disconnected,
            allow_disconnected_send_at_anytime,
            delete_oldest_messages,
            restore_messages,
            persist_qos0,
            incoming_msg_buffer_size: _,
            cache_dir_path: _,
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

#[derive(Clone, Resource, Default)]
pub struct ClientConnectOptions {
    pub clean_start: Option<bool>,
    pub connect_timeout: Option<Duration>,
    pub keep_alive_interval: Option<Duration>,
    pub max_inflight: Option<i32>,
    pub will_message: Option<(&'static str, Arc<[u8]>, Qos)>,
}
impl From<&ClientConnectOptions> for paho_mqtt::ConnectOptions {
    fn from(value: &ClientConnectOptions) -> Self {
        let ClientConnectOptions {
            clean_start,
            connect_timeout,
            keep_alive_interval,
            max_inflight,
            will_message,
        } = value;

        let mut builder = paho_mqtt::ConnectOptionsBuilder::new();

        if let Some(clean) = clean_start {
            builder.clean_start(*clean);
        }

        if let Some(timeout) = connect_timeout {
            builder.connect_timeout(*timeout);
        }

        if let Some(keep_alive_interval) = keep_alive_interval {
            builder.keep_alive_interval(*keep_alive_interval);
        }

        if let Some(max_inflight) = max_inflight {
            builder.max_inflight(*max_inflight);
        }

        if let Some((topic, payload, qos)) = will_message {
            let will = paho_mqtt::Message::new(*topic, payload.as_ref(), *qos as i32);
            builder.will_message(will);
        }

        builder.finalize()
    }
}

#[derive(Resource)]
pub struct MqttClient {
    inner_client: paho_mqtt::AsyncClient,
    recv_task: tokio::task::JoinHandle<()>,
    ping_task: tokio::task::JoinHandle<()>,
}

#[derive(Debug, Resource)]
struct MqttIncommingMsgTx(std::sync::mpsc::Sender<event::MqttMessage>);

#[allow(unused)]
#[derive(Debug, Resource)]
struct Subscriptions(Vec<(&'static str, Qos)>);

#[derive(Debug, Resource)]
struct RestartTaskHandle(tokio::task::JoinHandle<()>);

#[derive(Resource)]
struct NewMqttClient(
    Result<
        (
            paho_mqtt::AsyncClient,
            paho_mqtt::AsyncReceiver<Option<paho_mqtt::Message>>,
        ),
        paho_mqtt::Error,
    >,
);

#[derive(Debug, Resource)]
struct NewMqttClientRecv(crossbeam_channel::Receiver<NewMqttClient>);
