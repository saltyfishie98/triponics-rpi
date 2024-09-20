pub fn init_logging() {
    use tracing_subscriber::{layer::SubscriberExt, Layer};

    let subscriber = tracing_subscriber::Registry::default()
        .with(tracing_subscriber::EnvFilter::try_from_env("LOGGING").unwrap_or_default());

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

fn deserialize_arc_str<'de, D>(deserializer: D) -> Result<Arc<str>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(String::deserialize(deserializer)?.into())
}

fn serialize_arc_str<S>(v: &Arc<str>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(v)
}

fn deserialize_arc_bytes<'de, D>(deserializer: D) -> Result<Arc<[u8]>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Vec::deserialize(deserializer)?.into())
}

fn serialize_arc_bytes<S>(v: &Arc<[u8]>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_bytes(v.as_ref())
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct AtomicFixedBytes(
    #[serde(
        serialize_with = "serialize_arc_bytes",
        deserialize_with = "deserialize_arc_bytes"
    )]
    Arc<[u8]>,
);
impl From<&'static [u8]> for AtomicFixedBytes {
    fn from(value: &'static [u8]) -> Self {
        Self(value.into())
    }
}
impl From<Arc<[u8]>> for AtomicFixedBytes {
    fn from(value: Arc<[u8]>) -> Self {
        Self(value)
    }
}
impl From<Vec<u8>> for AtomicFixedBytes {
    fn from(value: Vec<u8>) -> Self {
        Self(Arc::<[u8]>::from(value))
    }
}
impl AsRef<[u8]> for AtomicFixedBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct AtomicFixedString(
    #[serde(
        serialize_with = "serialize_arc_str",
        deserialize_with = "deserialize_arc_str"
    )]
    Arc<str>,
);
impl From<&'static str> for AtomicFixedString {
    fn from(value: &'static str) -> Self {
        Self(value.into())
    }
}
impl From<String> for AtomicFixedString {
    fn from(value: String) -> Self {
        Self(value.into())
    }
}
impl From<AtomicFixedString> for Arc<str> {
    fn from(value: AtomicFixedString) -> Self {
        value.0
    }
}
impl AsRef<str> for AtomicFixedString {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

// This introduces event channels, on one side of which is mpsc::Sender<T>, and on another
// side is bevy's EventReader<T>, and it automatically bridges between the two.
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use bevy_app::{App, PreUpdate};
use bevy_ecs::event::EventWriter;
use bevy_ecs::system::Resource;
use bevy_ecs::{event::Event, system::Res};
use bevy_internal::prelude::{Deref, DerefMut};
use serde::Deserialize;

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
