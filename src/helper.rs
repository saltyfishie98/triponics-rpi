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

// This introduces event channels, on one side of which is mpsc::Sender<T>, and on another
// side is bevy's EventReader<T>, and it automatically bridges between the two.
use std::sync::mpsc::Receiver;
use std::sync::Mutex;

use bevy_app::{App, PreUpdate};
use bevy_ecs::event::EventWriter;
use bevy_ecs::system::Resource;
use bevy_ecs::{event::Event, system::Res};
use bevy_internal::prelude::{Deref, DerefMut};

#[derive(Resource, Deref, DerefMut)]
struct ChannelReceiver<T>(Mutex<Receiver<T>>);

pub trait AppExtensions {
    // Allows you to create bevy events using mpsc Sender
    fn add_event_channel<T: Event>(&mut self, receiver: Receiver<T>) -> &mut Self;
}

impl AppExtensions for App {
    fn add_event_channel<T: Event>(&mut self, receiver: Receiver<T>) -> &mut Self {
        assert!(
            !self.world().contains_resource::<ChannelReceiver<T>>(),
            "this event channel is already initialized",
        );

        self.add_event::<T>();
        self.add_systems(PreUpdate, channel_to_event::<T>);
        self.insert_resource(ChannelReceiver(Mutex::new(receiver)));
        self
    }
}

fn channel_to_event<T: 'static + Send + Sync + Event>(
    receiver: Res<ChannelReceiver<T>>,
    mut writer: EventWriter<T>,
) {
    // this should be the only system working with the receiver,
    // thus we always expect to get this lock
    let events = receiver.lock().expect("unable to acquire mutex lock");
    writer.send_batch(events.try_iter());
}
