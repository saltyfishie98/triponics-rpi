mod helper;
mod mqtt;

use std::time::Duration;

use bevy_app::{prelude::*, ScheduleRunnerPlugin};
use bevy_ecs::{
    event::{Event, EventReader},
    system::ResMut,
};
use bevy_internal::MinimalPlugins;
use bevy_tokio_tasks::{TokioTasksPlugin, TokioTasksRuntime};
use futures::StreamExt;
#[allow(unused_imports)]
use tracing as log;

fn main() -> anyhow::Result<()> {
    helper::init_logging();

    App::new()
        .add_plugins((
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f32(
                1.0 / 60.0,
            ))),
            TokioTasksPlugin::default(),
        ))
        .add_plugins((mqtt::MqttPlugin {
            subscriptions: Some(&[("data/#", mqtt::Qos::_0)]),
            ..Default::default()
        },))
        .add_event::<RestartEvent>()
        .add_systems(Startup, test_task)
        .add_systems(Startup, exit_task)
        .run();

    log::info!("bye!");

    Ok(())
}

#[derive(Event)]
struct RestartEvent;

fn test_task(rt: ResMut<TokioTasksRuntime>) {
    rt.spawn_background_task(|mut ctx| async move {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            ctx.run_on_main_thread(move |ctx| ctx.world.send_event(RestartEvent))
                .await;
        }
    });
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

fn restart(mut ev_reader: EventReader<RestartEvent>) {
    while let Some(_) = ev_reader.read().next() {
        log::info!("restart");
    }
}
