pub fn init_logging() {
    use tracing_subscriber::{layer::SubscriberExt, Layer};

    let subscriber = tracing_subscriber::Registry::default().with(
        tracing_subscriber::EnvFilter::builder()
            .with_default_directive(tracing::level_filters::LevelFilter::TRACE.into())
            .from_env_lossy(),
    );

    let fmt = {
        let time_offset = time::UtcOffset::current_local_offset()
            .unwrap_or(time::UtcOffset::from_hms(8, 0, 0).unwrap());

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
