use std::sync::Arc;

use bevy_ecs::component::Component;

use crate::helper::AtomicFixedString;

use super::Qos;

#[derive(Component)]
pub struct NewSubscriptions(pub &'static str, pub Qos);

#[derive(Component, serde::Serialize, serde::Deserialize, Clone)]
pub struct PublishMsg {
    pub(super) topic: AtomicFixedString,
    #[serde(
        serialize_with = "crate::helper::serialize_arc_bytes",
        deserialize_with = "crate::helper::deserialize_arc_bytes"
    )]
    pub(super) payload: Arc<[u8]>,
    pub(super) qos: Qos,
}
impl PublishMsg {
    pub fn new(topic: &'static str, payload: impl AsRef<[u8]>, qos: Qos) -> Self {
        Self {
            topic: topic.into(),
            payload: payload.as_ref().into(),
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
                &String::from_utf8(self.payload.as_ref().into()).unwrap_or("INVALID UTF-8".into()),
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
