mod actor;

mod app;
use app::*;

use tracing as log;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};

#[actix::main]
async fn main() {
    init_logging();

    App::new()
        .with_actor(actor::Mqtt::new())
        .with_actor(actor::CtrlLogic::new())
        .with_actor(actor::OutputController::new())
        .with_actor(actor::InputController::new(
            actor::input_controller::Config {
                update_interval: tokio::time::Duration::from_secs_f32(1.0 / 5.0),
            },
        ))
        .run()
        .await;

    log::info!("bye!");
}

fn init_logging() {
    let fmt = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_file(true)
        .with_line_number(true);

    let filter = tracing_subscriber::EnvFilter::from_default_env()
        .with_filter(tracing::level_filters::LevelFilter::TRACE);

    tracing_subscriber::registry().with(fmt).with(filter).init();
}
