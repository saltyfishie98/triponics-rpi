mod helper;
mod mqtt;
mod publish_state;

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
impl publish_state::PublishState for Counter {
    fn to_publish(&self) -> mqtt::component::PublishMsg {
        let mut payload = Vec::new();
        serde_json::to_writer(&mut payload, self).unwrap();

        mqtt::component::PublishMsg::new("saltyfishie/counter", &payload, mqtt::Qos::_1)
    }
}

fn main() -> anyhow::Result<()> {
    helper::init_logging();

    let mut path = std::env::current_dir().unwrap();
    path.push("temp");
    path.push("paho");

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
                    server_uri: "mqtt://test.mosquitto.org",
                    client_id: "triponics-test-1",
                    incoming_msg_buffer_size: 100,
                    max_buffered_messages: Some(5000),
                    persistence_type: Some(mqtt::PersistenceType::FilePath(path)),
                    ..Default::default()
                },
                client_connect_options: mqtt::ClientConnectOptions {
                    clean_start: Some(false),
                    keep_alive_interval: Some(Duration::from_secs(1)),
                    ..Default::default()
                },
                ..Default::default()
            },
            publish_state::StatePublishPlugin {
                publish_interval: Duration::from_secs(1),
            },
        ))
        .insert_resource(Counter::new(0))
        .add_systems(Startup, exit_task)
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

fn log_mqtt_msg(mut ev_reader: EventReader<mqtt::event::MqttMessage>) {
    while let Some(mqtt::event::MqttMessage(msg)) = ev_reader.read().next() {
        log::debug!("mqtt msg: {}", msg);
    }
}

fn control(mut cmd: Commands, mut counter: ResMut<Counter>) {
    log::trace!("update control");
    cmd.spawn(publish_state::UpdateState::new(counter.clone()));
    counter.data += 1;
}
