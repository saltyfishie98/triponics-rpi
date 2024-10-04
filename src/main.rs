mod config;
mod constants;
mod helper;

mod plugins;
use config::ConfigFile;
use plugins::*;

mod newtype;
use newtype::*;

mod globals;
use globals::*;

use std::time::Duration;

use bevy_app::{prelude::*, ScheduleRunnerPlugin};
use bevy_ecs::system::ResMut;
use bevy_internal::MinimalPlugins;
use bevy_tokio_tasks::{TokioTasksPlugin, TokioTasksRuntime};
use clap::Parser;
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

    let aeroponic_config = manager::AeroponicSprayManager::load_config().unwrap();
    let ph_dosing_config = manager::PhDosingManager::load_config().unwrap();

    App::new()
        .add_plugins((
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f32(
                1.0 / 60.0,
            ))),
            TokioTasksPlugin::default(),
        ))
        .add_plugins((
            state_file::Plugin::default(),
            mqtt::Plugin {
                client_create_options,
                client_connect_options,
            },
        ))
        .add_plugins((
            manager::relay_module::Plugin,
            manager::growlight::Plugin,
            manager::water_quality_sensor::Plugin,
            manager::ph_dosing::Plugin {
                config: ph_dosing_config,
            },
            manager::aeroponic_spray::Plugin {
                config: aeroponic_config,
            },
        ))
        .add_systems(Startup, (local::exit_task,))
        .run();

    log::info!("bye!");

    Ok(())
}

mod local {
    use helper::ErrorLogFormat;
    use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter, Registry};

    use super::*;

    pub fn try_init() -> (config::AppConfig,) {
        let args = Args::parse();
        init_logging(args.stdout);

        let config = config::AppConfig::load();
        log::debug!("config:\n{config:#?}");

        (config,)
    }

    pub fn init_logging(to_stdout: bool) {
        let expected = "Failed to set subscriber";
        let subscriber = Registry::default().with(
            #[cfg(debug_assertions)]
            {
                EnvFilter::try_from_env("LOGGING").unwrap_or(EnvFilter::new("info"))
            },
            #[cfg(not(debug_assertions))]
            {
                EnvFilter::try_from_env("LOGGING").unwrap_or(EnvFilter::new("info"))
            },
        );

        #[cfg(debug_assertions)]
        {
            let _ = to_stdout;
            let layer = fmt::Layer::default()
                .with_thread_ids(true)
                .with_file(true)
                .with_target(false)
                .with_line_number(true)
                .with_timer(fmt::time::OffsetTime::new(
                    *crate::timezone_offset(),
                    time_log_fmt(),
                ));

            tracing::subscriber::set_global_default(subscriber.with(layer)).expect(expected);
        }

        #[cfg(not(debug_assertions))]
        {
            use std::io::Write;

            let layer = fmt::Layer::default()
                .with_file(true)
                .with_target(false)
                .with_line_number(true)
                .with_timer(fmt::time::OffsetTime::new(
                    *crate::timezone_offset(),
                    time::macros::format_description!(
                        "[year]-[month padding:zero]-[day padding:zero] [hour]:[minute]:[second]"
                    ),
                ));

            if !to_stdout {
                let mut data_path = crate::data_directory().to_path_buf();
                data_path.push("app.log");
                let log_path = data_path;

                let mut file = std::fs::OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(log_path)
                    .unwrap();

                file.write_all(b"\n=================================================\n\n")
                    .unwrap();
                let layer = layer.with_writer(file).with_ansi(false);

                tracing::subscriber::set_global_default(subscriber.with(layer)).expect(expected);
            } else {
                tracing::subscriber::set_global_default(subscriber.with(layer)).expect(expected);
            }
        }
    }

    pub fn exit_task(rt: ResMut<TokioTasksRuntime>) {
        rt.spawn_background_task(|mut ctx| async move {
            let mut sigint =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt()).unwrap();

            let mut sigterm =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();

            tokio::select! {
                _ = sigint.recv() => {}
                _ = sigterm.recv() => {}
            }

            ctx.run_on_main_thread(move |ctx| {
                let world = ctx.world;

                plugins::state_file::Plugin::disable(world);

                if let Some(mut switch_manager) =
                    world.remove_resource::<plugins::manager::RelayManager>()
                {
                    if let Err(e) = switch_manager
                        .update_state(plugins::manager::relay_module::action::Update::default())
                    {
                        log::error!("failed to reset switch states, reason:\n{}", e.fmt_error());
                    }
                }

                if let Some(mut growlight_manager) =
                    world.remove_resource::<plugins::manager::GrowlightManager>()
                {
                    growlight_manager.turn_off();
                }

                world.send_event(AppExit::Success);
            })
            .await;
        });
    }

    // #[derive(bevy_ecs::system::Resource, Clone, serde::Serialize, serde::Deserialize, Debug)]
    // pub struct Counter {
    //     data: u32,
    //     datetime: String,
    // }
    // impl plugins::mqtt::message::MessageInfo for Counter {
    //     fn topic() -> AtomicFixedString {
    //         "test".into()
    //     }

    //     fn qos() -> plugins::mqtt::Qos {
    //         plugins::mqtt::Qos::_1
    //     }
    // }
    // impl Counter {
    //     pub fn subscribe(mut cmd: Commands) {
    //         cmd.insert_resource(Counter {
    //             data: 0,
    //             datetime: local::local_time_now_str(),
    //         });
    //         cmd.spawn(
    //             plugins::mqtt::message::Subscriptions::new()
    //                 .with_msg::<Counter>()
    //                 .finalize(),
    //         );
    //     }

    //     pub fn log_msg(mut ev_reader: EventReader<plugins::mqtt::event::IncomingMessage>) {
    //         while let Some(incoming_msg) = ev_reader.read().next() {
    //             if let Some(msg) = incoming_msg.get::<Counter>() {
    //                 log::debug!("receive mqtt msg: {:?}", msg)
    //             }
    //         }
    //     }
    // }

    // pub fn local_time_now_str() -> String {
    //     time::OffsetDateTime::now_utc()
    //         .to_offset(offset!(+8))
    //         .format(
    //             &time::format_description::parse("[year]-[month]-[day] [hour]:[minute]:[second]")
    //                 .unwrap(),
    //         )
    //         .unwrap()
    // }
}
