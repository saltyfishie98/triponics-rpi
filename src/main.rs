mod config;
mod helper;
mod mqtt;
mod msg;

use std::time::Duration;

use bevy_app::{prelude::*, ScheduleRunnerPlugin};
use bevy_ecs::{
    event::EventReader,
    schedule::{IntoSystemConfigs, IntoSystemSet},
    system::{Commands, IntoSystem, Local, ResMut, System, SystemBuilder, SystemParam},
    world::World,
};
use bevy_internal::{time::common_conditions::on_timer, MinimalPlugins};
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
        .add_plugins(mqtt::MqttPlugin {
            client_create_options,
            client_connect_options,
            initial_subscriptions: mqtt::Subscriptions::new()
                .with::<msg::relay::GrowLight>()
                .with::<msg::relay::Switch01>()
                .with::<msg::relay::Switch02>()
                .with::<msg::relay::Switch03>()
                .finalize(),
        })
        .add_systems(
            Startup,
            (
                exit_task, //
                Counter::subscribe,
            ),
        )
        .add_systems(
            Update,
            (
                Counter::log_msg,
                Counter::publish_status.run_if(on_timer(Duration::from_secs(1))),
                // msg::relay::GrowLight::update,
                // msg::relay::GrowLight::publish_status.run_if(on_timer(Duration::from_secs(1))),
                // msg::relay::Switch01::update,
                // msg::relay::Switch01::publish_status.run_if(on_timer(Duration::from_secs(1))),
                // msg::relay::Switch02::update,
                // msg::relay::Switch02::publish_status.run_if(on_timer(Duration::from_secs(1))),
                // msg::relay::Switch03::update,
                // msg::relay::Switch03::publish_status.run_if(on_timer(Duration::from_secs(1))),
            ),
        )
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
    const PROJECT: &'static str = "triponics";
    const GROUP: &'static str = "counter";
    const DEVICE: &'static str = "0";

    const STATUS_QOS: mqtt::Qos = mqtt::Qos::_1;
    const ACTION_QOS: Option<mqtt::Qos> = Some(mqtt::Qos::_1);

    fn update_system() -> impl bevy_ecs::system::System<In = (), Out = ()> {
        fn update(mut counter: ResMut<Counter>) {
            log::trace!("update control");
            counter.data = rand::thread_rng().gen_range(0..100);
            counter.datetime = m_::local_time_now_str();
        }

        IntoSystem::into_system(update)
    }
}
impl Counter {
    fn subscribe(mut cmd: Commands) {
        cmd.insert_resource(Counter {
            data: 0,
            datetime: m_::local_time_now_str(),
        });
        cmd.spawn(mqtt::Subscriptions::new().with::<Counter>().finalize().0);
    }

    fn log_msg(mut ev_reader: EventReader<mqtt::event::IncomingMessage>) {
        while let Some(incoming_msg) = ev_reader.read().next() {
            if let Some(msg) = incoming_msg.get::<Counter>() {
                log::debug!("receive mqtt msg: {:?}", msg)
            } else {
                log::debug!("msg payload not 'Counter'")
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
