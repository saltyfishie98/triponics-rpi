use bevy_ecs::event::Event;

#[derive(Debug, Event)]
pub struct RestartClient(pub &'static str);

#[derive(Debug, Event)]
pub struct MqttSubsMessage(pub paho_mqtt::Message);
