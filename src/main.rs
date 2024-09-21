mod config;
mod helper;
mod mqtt;
mod msg;

use std::time::Duration;

use bevy_app::{prelude::*, ScheduleRunnerPlugin};
use bevy_ecs::{
    event::EventReader,
    system::{Commands, ResMut},
};
use bevy_internal::MinimalPlugins;
use bevy_tokio_tasks::{TokioTasksPlugin, TokioTasksRuntime};

use time::macros::offset;
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
        .add_plugins(mqtt::MqttPlugin {
            client_create_options,
            client_connect_options,
            initial_subscriptions: mqtt::Subscriptions::new()
                .with_action_msg::<msg::relay::growlight::Message>()
                .with_action_msg::<msg::relay::switch_1::Message>()
                .with_action_msg::<msg::relay::switch_2::Message>()
                .with_action_msg::<msg::relay::switch_3::Message>()
                .finalize(),
        })
        .add_systems(
            Startup,
            (
                exit_task, //
                Counter::subscribe,
            ),
        )
        .add_systems(Update, (Counter::log_msg,))
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

#[derive(bevy_ecs::system::Resource, Clone, serde::Serialize, serde::Deserialize, Debug)]
struct Counter {
    data: u32,
    datetime: String,
}
impl mqtt::MqttMessage for Counter {
    fn topic() -> helper::AtomicFixedString {
        "test".into()
    }

    fn qos() -> mqtt::Qos {
        mqtt::Qos::_1
    }
}
impl Counter {
    fn subscribe(mut cmd: Commands) {
        cmd.insert_resource(Counter {
            data: 0,
            datetime: m_::local_time_now_str(),
        });
        cmd.spawn(
            mqtt::Subscriptions::new()
                .with_msg::<Counter>()
                .finalize()
                .0,
        );
    }

    fn log_msg(mut ev_reader: EventReader<mqtt::event::IncomingMessage>) {
        while let Some(incoming_msg) = ev_reader.read().next() {
            if let Some(msg) = incoming_msg.get::<Counter>() {
                log::debug!("receive mqtt msg: {:?}", msg)
            }
        }
    }
}

mod m_ {
    use super::*;

    pub fn local_time_now_str() -> String {
        time::OffsetDateTime::now_utc()
            .to_offset(offset!(+8))
            .format(
                &time::format_description::parse("[year]-[month]-[day] [hour]:[minute]:[second]")
                    .unwrap(),
            )
            .unwrap()
    }
}
