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

use mqtt::MqttMessage;
use rand::Rng;
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
        .add_plugins((
            mqtt::MqttPlugin {
                client_create_options,
                client_connect_options,
                initial_subscriptions: mqtt::Subscriptions::new().finalize(),
            },
            mqtt::add_on::PublishStatePlugin {
                publish_interval: Duration::from_secs(1),
            },
        ))
        .insert_resource(Counter {
            data: 0,
            datetime: local_time_now_str(),
        })
        .add_systems(Startup, (exit_task, Counter::subscribe))
        .add_systems(Update, (control, Counter::log_msg))
        .run();

    log::info!("bye!");

    Ok(())
}

#[derive(bevy_ecs::system::Resource, Clone, serde::Serialize, serde::Deserialize, Debug)]
struct Counter {
    data: u32,
    datetime: String,
}
impl Counter {
    fn subscribe(mut cmd: Commands) {
        cmd.spawn(mqtt::Subscriptions::new().with::<Counter>().finalize());
    }

    fn log_msg(mut ev_reader: EventReader<mqtt::event::IncomingMessages>) {
        while let Some(all_msg) = ev_reader.read().next() {
            match all_msg.read::<Counter>() {
                Some(Ok(msg)) => log::debug!("receive mqtt msg: {:?}", msg),
                Some(Err(e)) => {
                    log::warn!("error while reading mqtt incoming mqtt msg, reason: {}", e)
                }
                None => log::debug!("msg payload not 'Counter'"),
            }
        }
    }
}
impl mqtt::MqttMessage<'_> for Counter {
    const TOPIC: &'static str = "data/triponics/counter/0";
    const QOS: mqtt::Qos = mqtt::Qos::_1;
}
impl mqtt::add_on::publish_state::StatePublisher for Counter {
    fn update_publish_state(&self) -> mqtt::component::PublishMsg {
        self.publish()
    }
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

fn control(mut cmd: Commands, mut counter: ResMut<Counter>) {
    log::trace!("update control");
    cmd.spawn(mqtt::add_on::publish_state::UpdateState::new(
        counter.clone(),
    ));
    counter.data = rand::thread_rng().gen_range(0..100);
    counter.datetime = local_time_now_str();
}

fn local_time_now_str() -> String {
    time::OffsetDateTime::now_utc()
        .to_offset(offset!(+8))
        .format(
            &time::format_description::parse("[year]-[month]-[day] [hour]:[minute]:[second]")
                .unwrap(),
        )
        .unwrap()
}
