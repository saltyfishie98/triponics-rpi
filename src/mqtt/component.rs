use bevy_ecs::component::Component;

use crate::helper::{AtomicFixedBytes, AtomicFixedString};

use super::Qos;

#[derive(Component)]
pub struct NewSubscriptions(pub &'static str, pub Qos);

#[derive(Component, serde::Serialize, serde::Deserialize, Clone)]
pub struct PublishMsg {
    pub(super) topic: AtomicFixedString,
    pub(super) payload: AtomicFixedBytes,
    pub(super) qos: Qos,
}
impl PublishMsg {
    pub fn new(topic: &'static str, payload: Vec<u8>, qos: Qos) -> Self {
        use std::sync::Arc;
        let payload: Arc<[u8]> = payload.into();

        Self {
            topic: topic.into(),
            payload: payload.into(),
            qos,
        }
    }
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
