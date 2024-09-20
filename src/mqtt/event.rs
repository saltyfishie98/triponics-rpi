use bevy_ecs::event::Event;

use super::MqttMessage;
use tracing as log;

#[derive(Debug, Event)]
pub struct RestartClient(pub &'static str);

#[derive(Debug, Event)]
pub struct IncomingMessage(pub(super) paho_mqtt::Message);
impl IncomingMessage {
    pub fn get<T: MqttMessage>(&self) -> Option<T> {
        let msg = self.0.clone();
        if let Some(req_topic) = T::request_topic() {
            if msg.topic() == req_topic.as_ref() {
                match serde_json::from_slice(msg.payload()) {
                    Ok(out) => Some(out),
                    Err(e) => {
                        log::warn!("error reading incoming mqtt message, reason: {e}");
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        }
    }
}
