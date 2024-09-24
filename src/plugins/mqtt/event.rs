use bevy_ecs::event::Event;

use super::message::MessageInfo;
use tracing as log;

#[derive(Debug, Event)]
pub struct RestartClient(pub &'static str);

#[derive(Debug, Event)]
pub struct IncomingMessage(pub(super) paho_mqtt::Message);
impl IncomingMessage {
    pub fn get<T: MessageInfo>(&self) -> Option<T> {
        let msg = self.0.clone();

        if msg.topic() == T::topic().as_ref() {
            match serde_json::from_slice(msg.payload()) {
                Ok(out) => Some(out),
                Err(e) => {
                    log::warn!("[mqtt] error reading incoming message, reason: {e}");
                    None
                }
            }
        } else {
            None
        }
    }
}
