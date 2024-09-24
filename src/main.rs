mod config;
mod constants;
mod helper;
mod plugins;

mod newtype;
use newtype::*;

mod globals;
use globals::*;

use std::time::Duration;

use bevy_app::{prelude::*, ScheduleRunnerPlugin};
use bevy_ecs::{
    event::EventReader,
    system::{Commands, ResMut},
};
use bevy_internal::MinimalPlugins;
use bevy_tokio_tasks::{TokioTasksPlugin, TokioTasksRuntime};
use clap::Parser;
use time::macros::offset;
use tracing as log;

#[derive(clap::Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// log to stdout
    #[arg(long)]
    pub stdout: bool,
}

fn main() -> anyhow::Result<()> {
    let (config::AppConfig {
        mqtt:
            config::app::mqtt::Config {
                create_options: client_create_options,
                connect_options: client_connect_options,
            },
    },) = local::try_init();

    App::new()
        .add_plugins((
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f32(
                1.0 / 60.0,
            ))),
            TokioTasksPlugin::default(),
        ))
        .add_plugins((
            plugins::state_file::Plugin::default(),
            plugins::mqtt::Plugin {
                client_create_options,
                client_connect_options,
            },
        ))
        .add_plugins((
            plugins::manager::switch::Plugin,
            plugins::manager::growlight::Plugin,
            plugins::manager::aeroponic_spray::Plugin {
                config: Default::default(),
            },
        ))
        .add_systems(Startup, (local::exit_task, local::Counter::subscribe))
        .add_systems(Update, (local::Counter::log_msg,))
        .run();

    log::info!("bye!\n");

    Ok(())
}

mod local {
    use helper::ErrorLogFormat;

    use super::*;

    pub fn try_init() -> (config::AppConfig,) {
        let args = Args::parse();
        helper::init_logging(args.stdout);

        let config = config::AppConfig::load();
        log::debug!("config:\n{config:#?}");

        (config,)
    }

    pub fn exit_task(rt: ResMut<TokioTasksRuntime>) {
        rt.spawn_background_task(|mut ctx| async move {
            let _ = tokio::signal::ctrl_c().await;
            ctx.run_on_main_thread(move |ctx| {
                let world = ctx.world;

                plugins::state_file::Plugin::disable(world);

                if let Some(mut switch_manager) =
                    world.remove_resource::<plugins::manager::SwitchManager>()
                {
                    if let Err(e) = switch_manager
                        .update_state(plugins::manager::switch::action::Update::default())
                    {
                        log::error!("failed to reset switch states, reason:\n{}", e.fmt_error());
                    }
                }

                if let Some(mut growlight_manager) =
                    world.remove_resource::<plugins::manager::GrowlightManager>()
                {
                    if let Err(e) = growlight_manager
                        .update_state(plugins::manager::growlight::action::Update::default())
                    {
                        log::error!(
                            "failed to reset growlight states, reason:\n{}",
                            e.fmt_error()
                        );
                    }
                }

                world.send_event(AppExit::Success);
            })
            .await;
        });
    }

    #[derive(bevy_ecs::system::Resource, Clone, serde::Serialize, serde::Deserialize, Debug)]
    pub struct Counter {
        data: u32,
        datetime: String,
    }
    impl plugins::mqtt::message::MessageInfo for Counter {
        fn topic() -> AtomicFixedString {
            "test".into()
        }

        fn qos() -> plugins::mqtt::Qos {
            plugins::mqtt::Qos::_1
        }
    }
    impl Counter {
        pub fn subscribe(mut cmd: Commands) {
            cmd.insert_resource(Counter {
                data: 0,
                datetime: local::local_time_now_str(),
            });
            cmd.spawn(
                plugins::mqtt::message::Subscriptions::new()
                    .with_msg::<Counter>()
                    .finalize(),
            );
        }

        pub fn log_msg(mut ev_reader: EventReader<plugins::mqtt::event::IncomingMessage>) {
            while let Some(incoming_msg) = ev_reader.read().next() {
                if let Some(msg) = incoming_msg.get::<Counter>() {
                    log::debug!("receive mqtt msg: {:?}", msg)
                }
            }
        }
    }

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
