pub mod add_on;
pub mod event;

mod options;
pub use options::*;

mod wrapper;
pub use wrapper::*;

use std::time::Duration;

use bevy_app::Update;
use bevy_ecs::{
    entity::Entity,
    event::{EventReader, EventWriter},
    prelude::on_event,
    query::With,
    schedule::IntoSystemConfigs,
    system::{Commands, Local, Query, Res, ResMut},
    world::World,
};
use bevy_internal::time::{Time, Timer};
use bevy_tokio_tasks::TokioTasksRuntime;
use futures::StreamExt;
use tokio::sync::Mutex;

use crate::{helper::AsyncEventExt, log, AtomicFixedString};

pub struct Plugin {
    pub client_create_options: ClientCreateOptions,
    pub client_connect_options: ClientConnectOptions,
}
impl bevy_app::Plugin for Plugin {
    fn build(&self, app: &mut bevy_app::App) {
        let (mqtt_incoming_msg_queue, mqtt_incoming_msg_rx) =
            std::sync::mpsc::channel::<event::IncomingMessage>();

        let Self {
            client_create_options,
            client_connect_options,
        } = self;

        app.insert_resource(client_create_options.clone())
            .insert_resource(client_connect_options.clone())
            .insert_resource(local::MqttSubscriptions(Vec::new()))
            .insert_resource(local::MqttIncommingMsgTx(mqtt_incoming_msg_queue))
            .insert_resource(local::MqttCacheManager::new(
                client_create_options.client_id.clone(),
                client_create_options.cache_dir_path.as_ref().unwrap(),
            ))
            .add_event::<event::RestartClient>()
            .add_async_event_receiver(mqtt_incoming_msg_rx)
            .add_systems(
                Update,
                (
                    Self::update_subscriptions,
                    Self::restart_client //
                        .run_if(on_event::<event::RestartClient>()),
                    Self::publish_offline_cache //
                        .after(Self::restart_client),
                    Self::publish_message
                        .after(Self::restart_client)
                        .after(Self::publish_offline_cache),
                ),
            )
            .world_mut()
            .send_event(event::RestartClient("initial restart"));
    }
}
impl Plugin {
    fn restart_client(
        mut cmd: Commands,
        mut ev: EventReader<event::RestartClient>,
        mut restart_timer: Local<Option<Timer>>,
        time: Res<Time>,
        client: Option<Res<local::MqttClient>>,
        create_opts: Res<ClientCreateOptions>,
    ) {
        async fn mqtt_recv_task(
            mqtt_msg_tx: std::sync::mpsc::Sender<event::IncomingMessage>,
            mut stream: paho_mqtt::AsyncReceiver<Option<paho_mqtt::Message>>,
        ) {
            log::trace!("[mqtt] started receive task!");

            while let Some(msg) = stream.next().await {
                match msg {
                    Some(msg) => {
                        log::trace!(
                            "[mqtt] received msg -> {}: {}",
                            msg.topic(),
                            msg.payload_str()
                        );
                        if let Err(e) = mqtt_msg_tx.send(event::IncomingMessage(msg)) {
                            log::warn!("[mqtt] failed to forward received message, reason: {e}");
                        }
                    }
                    None => {
                        log::trace!("[mqtt] client disconnected");
                    }
                }
            }
        }

        async fn mqtt_ping_task(client: paho_mqtt::AsyncClient, duration: Duration) {
            let ping_interval = Duration::from_secs_f32(duration.as_secs_f32() * 0.7);
            log::debug!("[mqtt] ping interval: {}", ping_interval.as_secs_f32());

            loop {
                log::trace!("[mqtt] ping!");
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
            sub_info: Option<(Vec<AtomicFixedString>, Vec<i32>)>,
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

            client.set_connected_callback(|_| log::trace!("[mqtt] client connected (callback)"));
            client.set_disconnected_callback(|_, _, _| {
                log::trace!("[mqtt] client disconnected (callback)")
            });

            let strm = client.get_stream(stream_size);
            client.connect(conn_opts).await?;

            if let Some((topics, qos)) = sub_info {
                client.subscribe_many(&topics, &qos).await?;
                log::debug!("[mqtt] subscribed to topics: {topics:?}");
            }

            Ok((client, strm))
        }

        if ev.is_empty() {
            return;
        }

        let reason = Box::new(ev.read().next().unwrap().0);
        ev.clear();

        let timer = restart_timer.clone();

        if let Some(r_timer) = restart_timer.as_mut() {
            r_timer.tick(time.delta());
        } else if client.is_some() {
            restart_timer.replace(Timer::new(
                create_opts.restart_interval.unwrap(),
                bevy_internal::time::TimerMode::Repeating,
            ));
        }

        cmd.add(|world: &mut World| {
            if let Some(old_client) = world.remove_resource::<local::MqttClient>() {
                log::trace!("[mqtt] removed old client");
                let local::MqttClient {
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

            match world.get_resource::<local::RestartTaskHandle>() {
                None => {
                    if let Some(timer) = timer {
                        if !timer.just_finished() {
                            return;
                        }
                    }

                    let (tx, rx) = crossbeam_channel::bounded::<local::NewMqttClient>(1);
                    world.insert_resource(local::NewMqttClientRecv(rx));

                    let handle = {
                        let rt = world.get_resource::<TokioTasksRuntime>().unwrap();
                        let create_opts =
                            world.get_resource::<ClientCreateOptions>().unwrap().clone();
                        let local::MqttSubscriptions(subscriptions) =
                            world.get_resource::<local::MqttSubscriptions>().unwrap();

                        let paho_create_opts = paho_mqtt::CreateOptions::from(&create_opts);
                        let paho_conn_opts = paho_mqtt::ConnectOptions::from(&connect_opts);
                        let paho_subs = if !subscriptions.is_empty() {
                            Some(
                                subscriptions
                                    .iter()
                                    .map(|(t, q)| (t.clone(), *q as i32))
                                    .unzip(),
                            )
                        } else {
                            None
                        };

                        rt.spawn_background_task(move |_| async move {
                            log::info!("[mqtt] client restart triggered, reason: {reason}");
                            tx.send(local::NewMqttClient(
                                make_client(
                                    paho_create_opts,
                                    paho_conn_opts,
                                    create_opts.incoming_msg_buffer_size.unwrap(),
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

                    world.insert_resource(local::RestartTaskHandle(handle));
                }
                Some(_) => {
                    let local::NewMqttClientRecv(rx) =
                        world.get_resource::<local::NewMqttClientRecv>().unwrap();

                    match rx.try_recv() {
                        Err(crossbeam_channel::TryRecvError::Empty) => {
                            log::trace!("[mqtt] client restarting...");
                            return;
                        }
                        Err(crossbeam_channel::TryRecvError::Disconnected) => {
                            log::error!("[mqtt] client factory channel disconnected");
                        }
                        Ok(local::NewMqttClient(Ok((client, stream)))) => {
                            let rt = world.get_resource::<TokioTasksRuntime>().unwrap();
                            let local::MqttIncommingMsgTx(tx) = world
                                .get_resource::<local::MqttIncommingMsgTx>()
                                .unwrap()
                                .clone();

                            let recv_task = rt.spawn_background_task(|_| async move {
                                mqtt_recv_task(tx, stream).await;
                            });

                            let ping_task = {
                                let ping_client = client.clone();
                                rt.spawn_background_task(move |_| async move {
                                    if let Some(duration) = connect_opts.keep_alive_interval {
                                        mqtt_ping_task(ping_client, duration).await;
                                    }
                                })
                            };

                            log::info!("[mqtt] client restarted");
                            world.insert_resource(local::MqttClient {
                                inner_client: client,
                                recv_task,
                                ping_task,
                            })
                        }
                        Ok(local::NewMqttClient(Err(e))) => {
                            log::warn!("[mqtt] failed to restart client, reason: {e}");
                            world.send_event(event::RestartClient("failed to connect"));
                        }
                    }

                    if let Some(local::RestartTaskHandle(handle)) =
                        world.remove_resource::<local::RestartTaskHandle>()
                    {
                        handle.abort();
                    }
                }
            }
        });
    }

    fn publish_offline_cache(
        mut cmd: Commands,
        mut restarter: EventWriter<event::RestartClient>,
        rt: Res<TokioTasksRuntime>,
        cache: Res<local::MqttCacheManager>,
        client: Option<ResMut<local::MqttClient>>,
    ) {
        if client.is_none() {
            log::trace!("[mqtt] publish cache -> blocked by unavailable mqtt client");
            restarter.send(event::RestartClient("client not present"));
            return;
        }

        let client = client.unwrap();

        if !client.inner_client.is_connected() {
            log::debug!("[mqtt] publish cache -> blocked by mqtt client not connected");
            restarter.send(event::RestartClient("client not connected"));
            return;
        }

        let conn = cache.connection;

        let res = rt
            .runtime()
            .block_on(async { local::MqttCacheManager::read(conn, 10).await });

        match res {
            Ok(msg_vec) => msg_vec.into_iter().for_each(|msg| {
                cmd.spawn(msg);
            }),
            Err(e) => log::warn!("[mqtt] failed to read from cache, reason: {e}"),
        }
    }

    fn publish_message(
        mut cmd: Commands,
        mut query: Query<&mut message::Message>,
        rt: Res<TokioTasksRuntime>,
        client: Option<Res<local::MqttClient>>,
        cache_manager: Res<local::MqttCacheManager>,
        entt: Query<Entity, With<message::Message>>,
    ) {
        fn process_msg(
            (rt, msg, maybe_client, cache): (
                &Res<TokioTasksRuntime>,
                message::Message,
                Option<paho_mqtt::AsyncClient>,
                &'static Mutex<rusqlite::Connection>,
            ),
        ) {
            rt.spawn_background_task(move |_| async move {
                match maybe_client {
                    Some(client) => match client.try_publish(msg.clone().into()) {
                        Ok(o) => {
                            log::debug!("[mqtt] published msg -> {}", o.message());
                        }
                        Err(e) => {
                            log::warn!("[mqtt] failed to publish message, reason {e}");
                            local::MqttCacheManager::add(cache, &msg).await.unwrap();
                        }
                    },
                    None => {
                        local::MqttCacheManager::add(cache, &msg).await.unwrap();
                    }
                }
            });
        }

        match client {
            Some(client) => {
                if !client.inner_client.is_connected() {
                    log::trace!("[mqtt] client not connected to broker");
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
                log::trace!("[mqtt] client object unavailable");
                query
                    .iter_mut()
                    .map(|msg| (&rt, msg.clone(), None, cache_manager.connection))
                    .for_each(process_msg);
            }
        }

        entt.iter().for_each(|entt| {
            cmd.entity(entt).remove::<message::Message>();
        });
    }

    fn update_subscriptions(
        mut cmd: Commands,
        entt: Query<Entity, With<message::Subscriptions>>,
        client: Option<Res<local::MqttClient>>,
    ) {
        if client.is_none() {
            return;
        }

        entt.iter().for_each(|entt| {
            cmd.add(move |world: &mut World| {
                if let Some(new_sub) = world.entity_mut(entt).take::<message::Subscriptions>() {
                    if let Some(client) = world.get_resource::<local::MqttClient>() {
                        let message::Subscriptions(subs) = new_sub;
                        if !subs.is_empty() {
                            let rt = world.get_resource::<TokioTasksRuntime>().unwrap();

                            let new_sub = subs.clone();

                            let (topics, qos): (Vec<_>, Vec<_>) =
                                subs.iter().map(|(t, q)| (t.clone(), *q as i32)).unzip();

                            let handle = client.inner_client.subscribe_many(&topics, &qos);

                            rt.spawn_background_task(move |mut ctx| async move {
                                if let Err(e) = handle.await {
                                    log::warn!(
                                        "[mqtt] failed subscribing to {topics:?}, reason: {e}"
                                    );
                                } else {
                                    ctx.run_on_main_thread(move |ctx| {
                                        let mut subs = ctx
                                            .world
                                            .get_resource_mut::<local::MqttSubscriptions>()
                                            .unwrap();
                                        subs.0.extend_from_slice(&new_sub);
                                        log::info!("[mqtt] subscribed to topics {topics:?}");
                                    })
                                    .await;
                                }
                            });
                        }
                    }
                }
            });
        });
    }
}

pub mod message {
    use std::sync::Arc;

    use bevy_ecs::component::Component;
    use serde::de::DeserializeOwned;

    use crate::{AtomicFixedBytes, AtomicFixedString};

    use super::Qos;

    pub trait MessageInfo
    where
        Self: serde::Serialize + DeserializeOwned + core::fmt::Debug,
    {
        fn topic() -> AtomicFixedString;
        fn qos() -> Qos;

        fn to_payload(&self) -> AtomicFixedBytes {
            let mut out = Vec::new();
            serde_json::to_writer(&mut out, self).unwrap();
            out.into()
        }

        fn make(self) -> Message {
            Message {
                topic: Self::topic(),
                payload: self.to_payload(),
                qos: Self::qos(),
            }
        }
    }

    #[derive(Default)]
    pub struct SubscriptionsBuilder {
        subs: Vec<(AtomicFixedString, Qos)>,
    }
    impl SubscriptionsBuilder {
        pub fn with_msg<T: MessageInfo>(mut self) -> Self {
            self.subs.push((T::topic(), T::qos()));
            self
        }

        pub fn finalize(self) -> Subscriptions {
            Subscriptions(self.subs.into())
        }
    }

    #[derive(Component, Debug, Clone)]
    pub struct Subscriptions(pub(super) Arc<[(AtomicFixedString, Qos)]>);
    impl Subscriptions {
        #[allow(clippy::new_ret_no_self)]
        pub fn new() -> SubscriptionsBuilder {
            Default::default()
        }
    }

    #[derive(Component, serde::Serialize, serde::Deserialize, Clone)]
    pub struct Message {
        pub(super) topic: AtomicFixedString,
        pub(super) payload: AtomicFixedBytes,
        pub(super) qos: Qos,
    }
    impl std::fmt::Debug for Message {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("PublishMsg")
                .field("topic", &self.topic)
                .field(
                    "payload",
                    &String::from_utf8(self.payload.as_ref().to_vec())
                        .unwrap_or("INVALID UTF-8".into()),
                )
                .field("qos", &self.qos)
                .finish()
        }
    }
    impl From<Message> for paho_mqtt::Message {
        fn from(value: Message) -> Self {
            let Message {
                topic,
                payload,
                qos,
            } = value;
            Self::new(topic.as_ref(), payload.as_ref(), qos as i32)
        }
    }
}

mod local {
    use std::{
        path::{Path, PathBuf},
        sync::OnceLock,
    };

    use bevy_ecs::system::Resource;
    use tokio::sync::Mutex;

    use crate::AtomicFixedString;

    use super::{event, log, message, Qos};

    #[derive(Debug, Resource)]
    pub struct MqttCacheManager {
        pub connection: &'static Mutex<rusqlite::Connection>,
    }
    impl MqttCacheManager {
        pub fn new(client_id: AtomicFixedString, path: &Path) -> Self {
            let mut path = PathBuf::from(path);
            path.push(format!("{}.db3", client_id.as_ref()));
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();

            let conn = rusqlite::Connection::open(path).unwrap();
            conn.execute(include_str!("../../sql/create_table.sql"), ())
                .unwrap();

            static DB_CONNECTION: OnceLock<Mutex<rusqlite::Connection>> = OnceLock::new();
            let connection = DB_CONNECTION.get_or_init(|| Mutex::new(conn));

            Self { connection }
        }

        pub async fn add(
            conn: &'static Mutex<rusqlite::Connection>,
            msg: &message::Message,
        ) -> anyhow::Result<()> {
            let conn = conn.lock().await;

            let msg_1 = msg.clone();
            let out = Ok(conn
                .execute(
                    include_str!("../../sql/add_data.sql"),
                    (
                        time::OffsetDateTime::now_utc().unix_timestamp(),
                        postcard::to_allocvec(msg)?,
                    ),
                )
                .map(|_| ())?);

            log::trace!("[mqtt] cached -> {msg_1:?}");
            out
        }

        pub async fn read(
            conn: &'static Mutex<rusqlite::Connection>,
            count: u32,
        ) -> anyhow::Result<Vec<message::Message>> {
            let conn = conn.lock().await;

            let mut stmt = conn.prepare(include_str!("../../sql/read_data.sql"))?;
            let rows = stmt.query_map([count], |row| row.get::<usize, Vec<u8>>(0))?;

            let out = rows
                .map(|data| {
                    Ok::<_, anyhow::Error>(postcard::from_bytes::<message::Message>(&data?)?)
                })
                .collect::<Result<Vec<_>, _>>();

            if let Err(e) = conn.execute(include_str!("../../sql/delete_data.sql"), [count]) {
                log::warn!("[mqtt] failed to delete cached data, reason {e}");
            }

            out
        }
    }

    #[derive(Resource)]
    pub struct MqttClient {
        pub inner_client: paho_mqtt::AsyncClient,
        pub recv_task: tokio::task::JoinHandle<()>,
        pub ping_task: tokio::task::JoinHandle<()>,
    }

    #[derive(Debug, Resource, Clone)]
    pub struct MqttIncommingMsgTx(pub std::sync::mpsc::Sender<event::IncomingMessage>);

    #[allow(unused)]
    #[derive(Debug, Resource)]
    pub struct MqttSubscriptions(pub Vec<(AtomicFixedString, Qos)>);

    #[derive(Debug, Resource)]
    pub struct RestartTaskHandle(pub tokio::task::JoinHandle<()>);

    #[derive(Resource)]
    pub struct NewMqttClient(
        pub  Result<
            (
                paho_mqtt::AsyncClient,
                paho_mqtt::AsyncReceiver<Option<paho_mqtt::Message>>,
            ),
            paho_mqtt::Error,
        >,
    );

    #[derive(Debug, Resource)]
    pub struct NewMqttClientRecv(pub crossbeam_channel::Receiver<NewMqttClient>);
}
