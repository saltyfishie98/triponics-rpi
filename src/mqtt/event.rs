use bevy_ecs::event::Event;

use super::MqttMessage;

#[derive(Debug, Event)]
pub struct RestartClient(pub &'static str);

#[derive(Debug, Event)]
pub struct IncomingMessages(pub(super) paho_mqtt::Message);
impl IncomingMessages {
    pub fn read<'de, T: MqttMessage<'de>>(&self) -> Option<serde_json::Result<T>> {
        let msg = self.0.clone();
        if msg.topic() == T::TOPIC {
            Some(serde_json::from_slice(msg.payload()))
        } else {
            None
        }
    }
}
