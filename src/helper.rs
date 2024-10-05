// This introduces event channels, on one side of which is mpsc::Sender<T>, and on another
// side is bevy's EventReader<T>, and it automatically bridges between the two.
use std::sync::mpsc::Receiver;
use std::sync::Mutex;

use bevy_app::{App, PreUpdate};
use bevy_ecs::event::EventWriter;
use bevy_ecs::system::Resource;
use bevy_ecs::{event::Event, system::Res};
use bevy_internal::prelude::{Deref, DerefMut};

use crate::{log, AtomicFixedBytes, AtomicFixedString};

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

pub mod serde_time {
    use serde::Deserialize;

    use super::{log, ToDuration};

    pub fn serialize_offset_datetime_as_local<S>(
        offset_datetime: &time::OffsetDateTime,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let out = (*offset_datetime)
            .to_offset(*crate::timezone_offset())
            .format(&crate::time_log_fmt())
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
        Ok(time::OffsetDateTime::parse(&data, &crate::time_log_fmt())
            .map_err(|e| {
                log::error!(
                    "error deserializing datetime, reason: {e}; expected format \"hh:mm:ss.sss\""
                )
            })
            .unwrap())
    }

    const TIME_FORMAT: &[time::format_description::BorrowedFormatItem<'_>] =
        time::macros::format_description!("[hour]:[minute]:[second].[subsecond digits:3]");

    pub fn serialize_time<S>(time: &time::Time, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&time.format(TIME_FORMAT).unwrap())
    }

    pub fn deserialize_time<'de, D>(deserializer: D) -> Result<time::Time, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data = String::deserialize(deserializer)?;
        Ok(time::Time::parse(&data, TIME_FORMAT)
            .map_err(|e| {
                log::error!(
                    "error deserializing time, reason: {e}; expected format \"hh:mm:ss.sss\""
                )
            })
            .unwrap())
    }

    pub fn serialize_duration_formatted<S>(
        duration: &std::time::Duration,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let start = time::macros::time!(00:00:00.000);
        let dur = start + *duration;
        serializer.serialize_str(&dur.format(TIME_FORMAT).unwrap())
    }

    pub fn deserialize_duration_formatted<'de, D>(
        deserializer: D,
    ) -> Result<std::time::Duration, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data = String::deserialize(deserializer)?;
        Ok(time::Time::parse(&data, TIME_FORMAT)
            .map_err(|e| {
                log::error!(
                    "error deserializing duration, reason: {e}; expected format \"hh:mm:ss.sss\""
                )
            })
            .unwrap()
            .to_duration())
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

pub trait ToBytes {
    fn to_bytes(&self) -> AtomicFixedBytes;
}
impl ToBytes for serde_json::Value {
    fn to_bytes(&self) -> AtomicFixedBytes {
        let mut bytes: Vec<u8> = Vec::new();
        serde_json::to_writer(&mut bytes, self).unwrap();
        bytes.into()
    }
}

pub trait ToDuration {
    fn to_duration(&self) -> std::time::Duration;
}
impl ToDuration for time::Time {
    fn to_duration(&self) -> std::time::Duration {
        let (h, m, s, ms) = self.as_hms_milli();
        std::time::Duration::from_secs_f32(
            (h as f32 * 60.0 * 60.0) + (m as f32 * 60.0) + (s as f32 + (ms as f32 / 1000.0)),
        )
    }
}
