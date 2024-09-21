use bevy_ecs::event::Event;

use super::{add_on, MqttMessage};
use tracing as log;

#[derive(Debug, Event)]
pub struct RestartClient(pub &'static str);

#[derive(Debug, Event)]
pub struct IncomingMessage(pub(super) paho_mqtt::Message);
impl IncomingMessage {
    pub fn get<T: MqttMessage>(&self) -> Option<T> {
        let msg = self.0.clone();

        if msg.topic() == T::topic().as_ref() {
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
    }
}
