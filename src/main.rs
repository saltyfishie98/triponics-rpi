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
use bevy_ecs::{
    event::EventReader,
    system::{Commands, ResMut, Resource},
};
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
    local::try_init();

    let mqtt_config = mqtt::Plugin::load_config().unwrap();
    let aeroponic_config = manager::AeroponicSprayManager::load_config().unwrap();
    let ph_dosing_config = manager::PhDosingManager::load_config().unwrap();
    let growlight_config = manager::GrowlightManager::load_config().unwrap();

    let configs = std::collections::HashMap::from([
        (
            mqtt::Plugin::config_filepath(),
            serde_json::to_string_pretty(&mqtt_config).unwrap(),
        ),
        (
            manager::AeroponicSprayManager::config_filepath(),
            serde_json::to_string_pretty(&aeroponic_config).unwrap(),
        ),
        (
            manager::PhDosingManager::config_filepath(),
            serde_json::to_string_pretty(&ph_dosing_config).unwrap(),
        ),
        (
            manager::GrowlightManager::config_filepath(),
            serde_json::to_string_pretty(&growlight_config).unwrap(),
        ),
    ]);

    configs.into_iter().for_each(|(path, config)| {
        log::info!(
            "loaded config file:\npath: {}\nconfig: {config}\n",
            path.to_str().unwrap()
        )
    });

    App::new()
        .add_plugins((
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f32(
                1.0 / 60.0,
            ))),
            TokioTasksPlugin::default(),
        ))
        .add_plugins((
            ExitHandler,
            state_file::Plugin::default(),
            mqtt::Plugin {
                config: mqtt_config,
            },
        ))
        .add_plugins((
            manager::relay_module::Plugin,
            manager::water_quality_sensor::Plugin,
            manager::growlight::Plugin {
                config: growlight_config,
            },
            manager::ph_dosing::Plugin {
                config: ph_dosing_config,
            },
            manager::aeroponic_spray::Plugin {
                config: aeroponic_config,
            },
        ))
        .run();

    log::info!("bye!");

    Ok(())
}

mod local {
    use bevy_ecs::{event::EventWriter, world::World};
    use helper::ErrorLogFormat;
    use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter, Registry};

    use super::*;

    #[derive(Resource)]
    struct ExitTx(tokio::sync::oneshot::Sender<()>);
    #[derive(Resource)]
    struct ExitRx(tokio::sync::oneshot::Receiver<()>);

    #[derive(Debug, serde::Serialize)]
    pub struct ExitMsg;
    impl<'de> serde::Deserialize<'de> for ExitMsg {
        fn deserialize<D>(_: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            Ok(ExitMsg)
        }
    }
    impl mqtt::add_on::action_message::MessageImpl for ExitMsg {
        const PREFIX: &'static str = crate::constants::mqtt_prefix::REQUEST;
        const PROJECT: &'static str = crate::constants::project::NAME;
        const GROUP: &'static str = "exit";
        const DEVICE: &'static str = crate::constants::project::DEVICE;
        const QOS: mqtt::Qos = mqtt::Qos::_1;
    }
    impl ExitMsg {
        pub fn setup(mut cmd: Commands) {
            let (tx, rx) = tokio::sync::oneshot::channel();
            cmd.insert_resource(ExitTx(tx));
            cmd.insert_resource(ExitRx(rx));
            cmd.spawn(
                mqtt::message::Subscriptions::new()
                    .with_msg::<ExitMsg>()
                    .finalize(),
            );
        }

        pub fn listen_mqtt(
            mut mqtt_ev: EventReader<mqtt::event::IncomingMessage>,
            mut exit: EventWriter<event::Exit>,
        ) {
            while let Some(msg) = mqtt_ev.read().next() {
                if msg.get::<Self>().is_some() {
                    exit.send(event::Exit);
                }
            }
        }

        pub fn listen(mut cmd: Commands, ev: EventReader<event::Exit>) {
            if !ev.is_empty() {
                cmd.add(|world: &mut World| {
                    let ExitTx(tx) = world.remove_resource::<ExitTx>().unwrap();
                    tx.send(()).unwrap();
                });
            }
        }
    }

    pub fn try_init() {
        let args = Args::parse();
        init_logging(args.stdout);
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
            let ExitRx(rx) = ctx
                .run_on_main_thread(move |ctx| ctx.world.remove_resource::<ExitRx>().unwrap())
                .await;

            let mut sigint =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt()).unwrap();

            let mut sigterm =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();

            tokio::select! {
                _ = rx => {}
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
}

pub mod event {
    use bevy_ecs::event::Event;

    #[derive(Event)]
    pub struct Exit;
}

struct ExitHandler;
impl Plugin for ExitHandler {
    fn build(&self, app: &mut App) {
        app.add_event::<event::Exit>()
            .add_systems(
                Startup,
                (
                    local::ExitMsg::setup, //
                    local::exit_task,
                ),
            )
            .add_systems(
                Update,
                (local::ExitMsg::listen_mqtt, local::ExitMsg::listen),
            );
    }
}
