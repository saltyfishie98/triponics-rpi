use bevy_ecs::component::Component;

use crate::helper::{AtomicFixedBytes, AtomicFixedString};

use super::Qos;

pub trait MqttMsg<'de>: serde::Serialize + serde::Deserialize<'de> + Clone {
    const TOPIC: &'static str;
    const QOS: Qos;

    fn payload(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }

    fn subscribe_info() -> NewSubscriptions {
        NewSubscriptions(Self::TOPIC, Self::QOS)
    }

    fn publish(&self) -> PublishMsg {
        self.clone().into()
    }
}
impl<'de, T: MqttMsg<'de>> From<T> for PublishMsg {
    fn from(value: T) -> Self {
        PublishMsg {
            topic: T::TOPIC.into(),
            payload: value.payload().into(),
            qos: T::QOS,
        }
    }
}

#[derive(Component, Debug, Clone)]
pub struct NewSubscriptions(pub(super) &'static str, pub(super) Qos);

#[derive(Component, serde::Serialize, serde::Deserialize, Clone)]
pub struct PublishMsg {
    pub(super) topic: AtomicFixedString,
    pub(super) payload: AtomicFixedBytes,
    pub(super) qos: Qos,
}
impl std::fmt::Debug for PublishMsg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PublishMsg")
            .field("topic", &self.topic)
            .field(
                "payload",
                &String::from_utf8(self.payload.as_ref().to_vec())
                    .unwrap_or("INVALID UTF-8".into()),
            )
            .field("qos", &self.qos)
            .finish()
    }
}
impl From<PublishMsg> for paho_mqtt::Message {
    fn from(value: PublishMsg) -> Self {
        let PublishMsg {
            topic,
            payload,
            qos,
        } = value;
        Self::new(topic.as_ref(), payload.as_ref(), qos as i32)
    }
}
