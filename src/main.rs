mod helper;
mod mqtt;

use std::time::Duration;

use bevy_app::{prelude::*, ScheduleRunnerPlugin};
use bevy_ecs::{
    event::EventReader,
    system::{Commands, ResMut},
};
use bevy_internal::MinimalPlugins;
use bevy_tokio_tasks::{TokioTasksPlugin, TokioTasksRuntime};

#[allow(unused_imports)]
use tracing as log;

#[derive(bevy_ecs::system::Resource, Clone, serde::Serialize)]
struct Counter {
    data: u32,
}
impl Counter {
    fn new(data: u32) -> Self {
        Self { data }
    }
}
impl mqtt::add_on::publish_state::StatePublisher for Counter {
    fn to_publish(&self) -> mqtt::component::PublishMsg {
        let mut payload = Vec::new();
        serde_json::to_writer(&mut payload, self).unwrap();

        mqtt::component::PublishMsg::new("saltyfishie/counter", &payload, mqtt::Qos::_1)
    }
}

fn main() -> anyhow::Result<()> {
    helper::init_logging();

    let mut cache_dir_path = std::env::current_dir().unwrap();
    cache_dir_path.push("temp");

    let mut persist_path = cache_dir_path.clone();
    persist_path.push("paho");

    App::new()
        .add_plugins((
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f32(
                1.0 / 60.0,
            ))),
            TokioTasksPlugin::default(),
        ))
        .add_plugins((
            mqtt::MqttPlugin {
                client_create_options: mqtt::ClientCreateOptions {
                    restart_interval: Duration::from_secs(5),
                    server_uri: "10.42.0.1:1883",
                    client_id: "triponics-test-1",
                    incoming_msg_buffer_size: 100,
                    max_buffered_messages: Some(5000),
                    persistence_type: Some(mqtt::PersistenceType::FilePath(persist_path)),
                    cache_dir_path,
                    ..Default::default()
                },
                client_connect_options: mqtt::ClientConnectOptions {
                    clean_start: Some(false),
                    keep_alive_interval: Some(Duration::from_secs(1)),
                    ..Default::default()
                },
                ..Default::default()
            },
            mqtt::add_on::PublishStatePlugin {
                publish_interval: Duration::from_secs(1),
            },
        ))
        .insert_resource(Counter::new(0))
        .add_systems(Startup, (exit_task, test_subscription))
        .add_systems(Update, (control, log_mqtt_msg))
        .run();

    log::info!("bye!");

    Ok(())
}

fn exit_task(rt: ResMut<TokioTasksRuntime>) {
    rt.spawn_background_task(|mut ctx| async move {
        let _ = tokio::signal::ctrl_c().await;
        ctx.run_on_main_thread(move |ctx| {
            ctx.world.send_event(AppExit::Success);
        })
        .await;
    });
}

fn test_subscription(mut cmd: Commands) {
    cmd.spawn(mqtt::component::NewSubscriptions(
        "testing/#",
        mqtt::Qos::_1,
    ));
}

fn log_mqtt_msg(mut ev_reader: EventReader<mqtt::event::MqttSubsMessage>) {
    while let Some(mqtt::event::MqttSubsMessage(msg)) = ev_reader.read().next() {
        log::debug!("mqtt msg: {}", msg);
    }
}

fn control(mut cmd: Commands, mut counter: ResMut<Counter>) {
    log::trace!("update control");
    cmd.spawn(mqtt::add_on::publish_state::UpdateState::new(
        counter.clone(),
    ));
    counter.data += 1;
}
