mod actor;

mod app;
use app::*;

use tracing as log;
use tracing_subscriber::{layer::SubscriberExt, Layer};

#[actix::main]
async fn main() -> anyhow::Result<()> {
    // std::panic::set_hook(Box::new(|info| {
    //     println!("Got panic, info: {}", info);
    //     std::process::abort();
    // }));

    init_logging();

    App::builder()
        .with_actor(actor::Mqtt::new().await)?
        .with_actor(actor::CtrlLogic::new())?
        .with_actor(actor::OutputController::new())?
        .with_actor(actor::InputController::new(
            actor::input_controller::Config {
                update_interval: tokio::time::Duration::from_secs_f32(1.0),
            },
        ))?
        .build()
        .run()
        .await;

    log::info!("bye!");
    Ok(())
}

fn init_logging() {
    let subscriber = tracing_subscriber::Registry::default().with(
        tracing_subscriber::EnvFilter::builder()
            .with_default_directive(tracing::level_filters::LevelFilter::DEBUG.into())
            .from_env_lossy(),
    );

    let fmt = {
        let time_offset = time::UtcOffset::current_local_offset().unwrap();

        tracing_subscriber::fmt::Layer::default()
            .with_target(false)
            .with_file(true)
            .with_line_number(true)
            .with_timer(tracing_subscriber::fmt::time::OffsetTime::new(
                time_offset,
                time::macros::format_description!(
                    "[year]-[month padding:zero]-[day padding:zero] [hour]:[minute]:[second]"
                ),
            ))
            .with_filter(tracing_subscriber::filter::LevelFilter::TRACE)
    };

    tracing::subscriber::set_global_default(subscriber.with(fmt)).unwrap();
}
