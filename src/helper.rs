// This introduces event channels, on one side of which is mpsc::Sender<T>, and on another
// side is bevy's EventReader<T>, and it automatically bridges between the two.
use std::sync::mpsc::Receiver;
use std::sync::Mutex;

use bevy_app::{App, PreUpdate};
use bevy_ecs::event::EventWriter;
use bevy_ecs::system::Resource;
use bevy_ecs::{event::Event, system::Res};
use bevy_internal::prelude::{Deref, DerefMut};

use crate::AtomicFixedString;

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

pub mod time {
    use serde::Deserialize;

    fn time_fmt() -> impl time::formatting::Formattable + time::parsing::Parsable {
        time::macros::format_description!(
            "[year]-[month padding:zero]-[day padding:zero] [hour]:[minute]:[second] [offset_hour sign:mandatory]:[offset_minute]:[offset_second]"
        )
    }

    pub fn serialize_offset_datetime_as_local<S>(
        offset_datetime: &time::OffsetDateTime,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let out = (*offset_datetime)
            .to_offset(*crate::timezone_offset())
            .format(&time_fmt())
            .unwrap();

        serializer.serialize_str(&out)
    }

    pub fn deserialize_offset_datetime_as_local<'de, D>(
        deserializer: D,
    ) -> Result<time::OffsetDateTime, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data = String::deserialize(deserializer)?;
        println!("{data}");
        Ok(time::OffsetDateTime::parse(&data, &time_fmt()).unwrap())
    }
}

pub trait ErrorLogFormat {
    fn fmt_error(&self) -> AtomicFixedString;
}
impl<E: std::error::Error> ErrorLogFormat for error_stack::Report<E> {
    fn fmt_error(&self) -> AtomicFixedString {
        format!("\n{self:?}\n").into()
    }
}
