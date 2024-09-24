mod config;
mod constants;
mod helper;
mod manager;
mod mqtt;

use std::{sync::LazyLock, time::Duration};

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
    let args = Args::parse();

    helper::init_logging(args.stdout);

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
            },
            manager::switch::Plugin,
            manager::growlight::Plugin,
            manager::aeroponic_spray::Plugin {
                config: Default::default(),
            },
            manager::state_file::Plugin::default(),
        ))
        .add_systems(
            Startup,
            (
                local::exit_task, //
                local::Counter::subscribe,
            ),
        )
        .add_systems(Update, (local::Counter::log_msg,))
        .run();

    log::info!("bye!\n");

    Ok(())
}

fn data_directory() -> &'static std::path::Path {
    static DATA_DIRECTORY: LazyLock<std::path::PathBuf> = LazyLock::new(|| {
        let mut cwd = std::env::current_dir().unwrap();
        cwd.push("data");
        cwd
    });
    DATA_DIRECTORY.as_path()
}

fn timezone_offset() -> &'static time::UtcOffset {
    static TIMEZONE_OFFSET: LazyLock<time::UtcOffset> = LazyLock::new(|| time::macros::offset!(+8));

    &TIMEZONE_OFFSET
}

mod local {
    use super::*;

    pub fn exit_task(rt: ResMut<TokioTasksRuntime>) {
        rt.spawn_background_task(|mut ctx| async move {
            let _ = tokio::signal::ctrl_c().await;
            ctx.run_on_main_thread(move |ctx| {
                ctx.world.send_event(AppExit::Success);
            })
            .await;
        });
    }

    #[derive(bevy_ecs::system::Resource, Clone, serde::Serialize, serde::Deserialize, Debug)]
    pub struct Counter {
        data: u32,
        datetime: String,
    }
    impl mqtt::message::MessageInfo for Counter {
        fn topic() -> helper::AtomicFixedString {
            "test".into()
        }

        fn qos() -> mqtt::Qos {
            mqtt::Qos::_1
        }
    }
    impl Counter {
        pub fn subscribe(mut cmd: Commands) {
            cmd.insert_resource(Counter {
                data: 0,
                datetime: local::local_time_now_str(),
            });
            cmd.spawn(
                mqtt::message::Subscriptions::new()
                    .with_msg::<Counter>()
                    .finalize(),
            );
        }

        pub fn log_msg(mut ev_reader: EventReader<mqtt::event::IncomingMessage>) {
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
