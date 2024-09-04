mod config;
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

use mqtt::component::MqttMsg;
#[allow(unused_imports)]
use tracing as log;

fn main() -> anyhow::Result<()> {
    helper::init_logging();

    let config = config::AppConfig::load();
    log::debug!("config:\n{config:#?}");

    let config::AppConfig {
        mqtt:
            config::app::mqtt::Config {
                topic_source: _,
                create_options: client_create_options,
                connect_options: client_connect_options,
            },
    } = config;

    App::new()
        .add_plugins((
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f32(
                1.0 / 60.0,
            ))),
            TokioTasksPlugin::default(),
        ))
        .add_plugins((
            mqtt::MqttPlugin {
                client_create_options,
                client_connect_options,
                initial_subscriptions: vec![Counter::subscribe_info()],
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

#[derive(bevy_ecs::system::Resource, Clone, serde::Serialize, serde::Deserialize)]
struct Counter {
    data: u32,
}
impl Counter {
    fn new(data: u32) -> Self {
        Self { data }
    }
}
impl mqtt::component::MqttMsg<'_> for Counter {
    const TOPIC: &'static str = "saltyfishie/counter";
    const QOS: mqtt::Qos = mqtt::Qos::_1;
}
impl mqtt::add_on::publish_state::StatePublisher for Counter {
    fn update_publish_state(&self) -> mqtt::component::PublishMsg {
        let mut payload = Vec::new();
        serde_json::to_writer(&mut payload, self).unwrap();
        self.publish()
    }
}

fn test_subscription(mut cmd: Commands) {
    cmd.spawn(Counter::subscribe_info());
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
