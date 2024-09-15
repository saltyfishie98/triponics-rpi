mod config;
mod helper;
mod mqtt;

use std::time::Duration;

use bevy_app::{prelude::*, ScheduleRunnerPlugin};
use bevy_ecs::{
    event::EventReader,
    schedule::IntoSystemConfigs,
    system::{Commands, Local, ResMut},
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
            initial_subscriptions: mqtt::Subscriptions::new().with::<LightRelay>().finalize(),
        })
        .insert_resource(Counter {
            data: 0,
            datetime: local_time_now_str(),
        })
        .add_systems(Startup, (exit_task, Counter::subscribe))
        .add_systems(
            Update,
            (
                Counter::update,
                Counter::publish.run_if(on_timer(Duration::from_secs(1))),
                Counter::log_msg,
                LightRelay::update,
                LightRelay::publish.run_if(on_timer(Duration::from_secs(1))),
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
impl mqtt::MqttMessage<'_> for Counter {
    const TOPIC: &'static str = "data/triponics/counter/0";
    const QOS: mqtt::Qos = mqtt::Qos::_1;
}
impl Counter {
    fn subscribe(mut cmd: Commands) {
        cmd.spawn(mqtt::Subscriptions::new().with::<Counter>().finalize());
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

    fn update(mut counter: ResMut<Counter>) {
        log::trace!("update control");
        counter.data = rand::thread_rng().gen_range(0..100);
        counter.datetime = local_time_now_str();
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, bevy_ecs::system::Resource)]
struct LightRelay {
    light_on: bool,
}
impl mqtt::MqttMessage<'_> for LightRelay {
    const TOPIC: &'static str = "data/triponics/grow_light/0";
    const QOS: mqtt::Qos = mqtt::Qos::_1;
}
impl LightRelay {
    fn update(
        mut cmd: Commands,
        mut ev_reader: EventReader<mqtt::event::IncomingMessage>,
        mut pin: Local<Option<rppal::gpio::OutputPin>>,
    ) {
        if pin.is_none() {
            log::debug!("init light gpio");
            *pin = Some({
                let mut pin = rppal::gpio::Gpio::new()
                    .unwrap()
                    .get(23)
                    .unwrap()
                    .into_output();

                pin.set_high();
                pin
            });
            cmd.insert_resource(Self { light_on: false })
        }

        while let Some(incoming_msg) = ev_reader.read().next() {
            if let Some(msg) = incoming_msg.get::<LightRelay>() {
                let pin = pin.as_mut().unwrap();

                if msg.light_on {
                    pin.set_low();
                    cmd.insert_resource(Self { light_on: true })
                } else {
                    pin.set_high();
                    cmd.insert_resource(Self { light_on: false })
                }
            }
        }
    }
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
