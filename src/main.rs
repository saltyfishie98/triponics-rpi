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

#[derive(bevy_ecs::system::Resource)]
struct Counter(u32);

fn main() -> anyhow::Result<()> {
    helper::init_logging();

    App::new()
        .insert_resource(Counter(0))
        .add_plugins((
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f32(
                1.0 / 60.0,
            ))),
            TokioTasksPlugin::default(),
        ))
        .add_plugins((mqtt::MqttPlugin {
            initial_subscriptions: &[("data/#", mqtt::Qos::_0)],
            client_create_options: mqtt::MqttCreateOptions {
                server_uri: "mqtt://test.mosquitto.org",
                client_id: "triponics-test",
                ..Default::default()
            },
        },))
        .add_systems(Startup, exit_task)
        .add_systems(Update, (log_mqtt_msg, publish))
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
        log::info!("mqtt msg: {}", msg);
    }
}

fn publish(mut cmd: Commands, mut counter: ResMut<Counter>) {
    // if counter.0 % 200 != 0 {
    //     return;
    // }

    let payload = format!("hello {}", counter.0);

    // log::info!("{payload}");

    cmd.spawn(mqtt::component::PublishMsg::new(
        "saltyfishie",
        payload,
        mqtt::Qos::_1,
    ));

    counter.0 += 1;
}
