pub mod add_on;
pub mod component;
pub mod event;

mod options;
use component::NewSubscriptions;
pub use options::*;

mod wrapper;
pub use wrapper::*;

use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
    time::Duration,
};

use bevy_app::{Plugin, Update};
use bevy_ecs::{
    entity::Entity,
    event::{EventReader, EventWriter},
    prelude::on_event,
    query::With,
    schedule::IntoSystemConfigs,
    system::{Commands, Local, Query, Res, ResMut, Resource},
    world::World,
};
use bevy_internal::time::{Time, Timer};
use bevy_tokio_tasks::TokioTasksRuntime;
use futures::StreamExt;
use tokio::sync::Mutex;

use crate::helper::{AsyncEventExt, AtomicFixedString};
#[allow(unused_imports)]
use tracing as log;

// #[derive(Default)]
pub struct MqttPlugin {
    pub client_create_options: ClientCreateOptions,
    pub client_connect_options: ClientConnectOptions,
    pub initial_subscriptions: Vec<NewSubscriptions>,
}
impl Plugin for MqttPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        let (mqtt_incoming_msg_queue, mqtt_incoming_msg_rx) =
            std::sync::mpsc::channel::<event::MqttSubsMessage>();

        let Self {
            client_create_options,
            client_connect_options,
            initial_subscriptions: subs,
        } = self;

        app.insert_resource(client_create_options.clone())
            .insert_resource(client_connect_options.clone())
            .insert_resource(MqttSubscriptions(subs.to_vec()))
            .insert_resource(MqttIncommingMsgTx(mqtt_incoming_msg_queue))
            .insert_resource(MqttCacheManager::new(
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
impl MqttPlugin {
    fn restart_client(
        mut cmd: Commands,
        mut ev: EventReader<event::RestartClient>,
        mut restart_timer: Local<Option<Timer>>,
        time: Res<Time>,
        client: Option<Res<MqttClient>>,
        create_opts: Res<ClientCreateOptions>,
    ) {
        async fn mqtt_recv_task(
            mqtt_msg_tx: std::sync::mpsc::Sender<event::MqttSubsMessage>,
            mut stream: paho_mqtt::AsyncReceiver<Option<paho_mqtt::Message>>,
        ) {
            log::debug!("started recv task!");

            while let Some(msg) = stream.next().await {
                match msg {
                    Some(msg) => {
                        log::trace!("polled mqtt msg");
                        if let Err(e) = mqtt_msg_tx.send(event::MqttSubsMessage(msg)) {
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
            sub_info: Option<(Vec<&str>, Vec<i32>)>,
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

            if let Some((topics, qos)) = sub_info {
                client.subscribe_many(&topics, &qos).await?;
                log::info!("subscribed to mqtt topics: {topics:?}");
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
                    if let Some(timer) = timer {
                        if !timer.just_finished() {
                            return;
                        }
                    }

                    let (tx, rx) = crossbeam_channel::bounded::<NewMqttClient>(1);
                    world.insert_resource(NewMqttClientRecv(rx));

                    let handle = {
                        let rt = world.get_resource::<TokioTasksRuntime>().unwrap();
                        let create_opts =
                            world.get_resource::<ClientCreateOptions>().unwrap().clone();
                        let MqttSubscriptions(subscriptions) =
                            world.get_resource::<MqttSubscriptions>().unwrap();

                        let paho_create_opts = paho_mqtt::CreateOptions::from(&create_opts);
                        let paho_conn_opts = paho_mqtt::ConnectOptions::from(&connect_opts);
                        let paho_subs = if !subscriptions.is_empty() {
                            Some(
                                subscriptions
                                    .iter()
                                    .map(|NewSubscriptions(t, q)| (*t, *q as i32))
                                    .unzip(),
                            )
                        } else {
                            None
                        };

                        rt.spawn_background_task(move |_| async move {
                            log::debug!("mqtt client restart triggered, reason: {reason}");
                            tx.send(NewMqttClient(
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

                    world.insert_resource(RestartTaskHandle(handle));
                }
                Some(_) => {
                    let NewMqttClientRecv(rx) = world.get_resource::<NewMqttClientRecv>().unwrap();

                    match rx.try_recv() {
                        Err(crossbeam_channel::TryRecvError::Empty) => {
                            log::trace!("mqtt client restarting...");
                            return;
                        }
                        Err(crossbeam_channel::TryRecvError::Disconnected) => {
                            log::error!("can't received new mqtt client");
                        }
                        Ok(NewMqttClient(Ok((client, stream)))) => {
                            let rt = world.get_resource::<TokioTasksRuntime>().unwrap();
                            let MqttIncommingMsgTx(tx) =
                                world.get_resource::<MqttIncommingMsgTx>().unwrap().clone();

                            let recv_task = rt.spawn_background_task(|_| async move {
                                mqtt_recv_task(tx, stream).await;
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

    fn publish_offline_cache(
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

    fn publish_message(
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
                log::debug!("staged -> {msg:?}");
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

    fn update_subscriptions(
        mut cmd: Commands,
        entt: Query<Entity, With<component::NewSubscriptions>>,
        client: Option<Res<MqttClient>>,
    ) {
        if client.is_none() {
            return;
        }

        entt.iter().for_each(|entt| {
            cmd.add(move |world: &mut World| {
                if let Some(new_sub) = world.entity_mut(entt).take::<component::NewSubscriptions>()
                {
                    if let Some(client) = world.get_resource::<MqttClient>() {
                        let NewSubscriptions(topic, qos) = new_sub;
                        let rt = world.get_resource::<TokioTasksRuntime>().unwrap();
                        let handle = client.inner_client.subscribe(topic, qos as i32);

                        rt.spawn_background_task(move |mut ctx| async move {
                            if let Err(e) = handle.await {
                                log::warn!("failed to subscribe to '{topic}', reason: {e}");
                            } else {
                                ctx.run_on_main_thread(move |ctx| {
                                    let mut subs =
                                        ctx.world.get_resource_mut::<MqttSubscriptions>().unwrap();
                                    subs.0.push(new_sub);
                                    log::info!("subscribed to topic '{topic}'");
                                })
                                .await;
                            }
                        });
                    }
                }
            });
        });
    }
}

#[derive(Debug, Resource)]
struct MqttCacheManager {
    connection: &'static Mutex<rusqlite::Connection>,
}
impl MqttCacheManager {
    fn new(client_id: AtomicFixedString, path: &Path) -> Self {
        let mut path = PathBuf::from(path);
        path.push(format!("{}.db3", client_id.as_ref()));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();

        let conn = rusqlite::Connection::open(path).unwrap();
        conn.execute(include_str!("../sql/create_table.sql"), ())
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
                include_str!("../sql/add_data.sql"),
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

        let mut stmt = conn.prepare(include_str!("../sql/read_data.sql"))?;
        let rows = stmt.query_map([count], |row| row.get::<usize, Vec<u8>>(0))?;

        let out = rows
            .map(|data| {
                Ok::<_, anyhow::Error>(postcard::from_bytes::<component::PublishMsg>(&data?)?)
            })
            .collect::<Result<Vec<_>, _>>();

        if let Err(e) = conn.execute(include_str!("../sql/delete_data.sql"), [count]) {
            log::warn!("failed to delete cached data, reason {e}");
        }

        out
    }
}

#[derive(Resource)]
struct MqttClient {
    inner_client: paho_mqtt::AsyncClient,
    recv_task: tokio::task::JoinHandle<()>,
    ping_task: tokio::task::JoinHandle<()>,
}

#[derive(Debug, Resource, Clone)]
struct MqttIncommingMsgTx(std::sync::mpsc::Sender<event::MqttSubsMessage>);

#[allow(unused)]
#[derive(Debug, Resource)]
struct MqttSubscriptions(Vec<NewSubscriptions>);

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
