use bevy_ecs::component::Component;

use crate::helper::{AtomicFixedBytes, AtomicFixedString};

use super::{MqttMessage, Qos};

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
impl<'de, T: MqttMessage<'de>> From<T> for PublishMsg {
    fn from(value: T) -> Self {
        PublishMsg {
            topic: T::TOPIC.into(),
            payload: value.payload().into(),
            qos: T::QOS,
        }
    }
}
