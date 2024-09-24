// This introduces event channels, on one side of which is mpsc::Sender<T>, and on another
// side is bevy's EventReader<T>, and it automatically bridges between the two.
use std::sync::mpsc::Receiver;
use std::sync::Mutex;

use bevy_app::{App, PreUpdate};
use bevy_ecs::event::EventWriter;
use bevy_ecs::system::Resource;
use bevy_ecs::{event::Event, system::Res};
use bevy_internal::prelude::{Deref, DerefMut};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{fmt, EnvFilter, Registry};

use crate::AtomicFixedString;

pub fn init_logging(to_stdout: bool) {
    let mut data_path = crate::data_directory().to_path_buf();
    data_path.push("app.log");

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
                time::macros::format_description!(
                    "[year]-[month padding:zero]-[day padding:zero] [hour]:[minute]:[second]"
                ),
            ));

        tracing::subscriber::set_global_default(subscriber.with(layer)).expect(expected);
    }

    #[cfg(not(debug_assertions))]
    {
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
            let file = std::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(data_path)
                .unwrap();

            let layer = layer.with_writer(file).with_ansi(false);

            tracing::subscriber::set_global_default(subscriber.with(layer)).expect(expected);
        } else {
            tracing::subscriber::set_global_default(subscriber.with(layer)).expect(expected);
        }
    }
}

#[derive(Resource, Deref, DerefMut)]
struct ChannelReceiver<T>(Mutex<Receiver<T>>);

pub trait AsyncEventExt {
    // Allows you to create bevy events using mpsc Sender
    fn add_async_event_receiver<T: Event>(&mut self, receiver: Receiver<T>) -> &mut Self;
}

impl AsyncEventExt for App {
    fn add_async_event_receiver<T: Event>(&mut self, receiver: Receiver<T>) -> &mut Self {
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

pub trait ErrorLogFormat {
    fn fmt_error(&self) -> AtomicFixedString;
}
impl<E: std::error::Error> ErrorLogFormat for error_stack::Report<E> {
    fn fmt_error(&self) -> AtomicFixedString {
        format!("\n{self:?}\n").into()
    }
}

pub mod relay {
    pub enum State {
        Open,
        Close,
    }

    pub fn get_state(pin: &rppal::gpio::OutputPin) -> bool {
        pin.is_set_low()
    }

    pub fn set_state(pin: &mut rppal::gpio::OutputPin, new_state: State) {
        match new_state {
            State::Open => pin.set_high(),
            State::Close => pin.set_low(),
        }
    }
}
