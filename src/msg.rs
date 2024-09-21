use bevy_ecs::{
    event::EventReader,
    system::{Commands, IntoSystem, Local},
};
use tracing as log;

use crate::mqtt;

pub mod relay {
    use std::time::Duration;

    use bevy_ecs::schedule::IntoSystemConfigs;
    use bevy_internal::time::common_conditions::on_timer;
    use mqtt::add_on::action_message::{ActionMessage, ActionMessageHandler};

    use super::*;

    pub mod growlight {
        use super::*;

        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        pub struct Request {
            state: bool,
        }
        impl ActionMessage for Request {
            type Type = mqtt::add_on::action_message::action_type::Request;
            const PROJECT: &'static str = "triponics";
            const GROUP: &'static str = "growlight";
            const DEVICE: &'static str = "0";
            const QOS: mqtt::Qos = mqtt::Qos::_1;
        }

        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        pub struct Response;
        impl ActionMessage for Response {
            type Type = mqtt::add_on::action_message::action_type::Response;
            const PROJECT: &'static str = "triponics";
            const GROUP: &'static str = "growlight";
            const DEVICE: &'static str = "0";
            const QOS: mqtt::Qos = mqtt::Qos::_1;
        }

        #[derive(
            Debug, Clone, serde::Serialize, serde::Deserialize, bevy_ecs::system::Resource,
        )]
        pub struct Message {
            state: bool,
        }
        impl ActionMessage for Message {
            type Type = mqtt::add_on::action_message::action_type::Status;
            const PROJECT: &'static str = "triponics";
            const GROUP: &'static str = "growlight";
            const DEVICE: &'static str = "0";
            const QOS: mqtt::Qos = mqtt::Qos::_1;
        }
        impl ActionMessageHandler for Message {
            type Status = Self;
            type Request = Request;
            type Response = Response;

            fn on_request() -> Option<bevy_ecs::schedule::SystemConfigs> {
                fn update(
                    mut cmd: Commands,
                    mut ev_reader: EventReader<mqtt::event::IncomingMessage>,
                    mut pin: Local<Option<rppal::gpio::OutputPin>>,
                ) {
                    if pin.is_none() {
                        log::debug!("init light gpio");
                        *pin = Some({
                            let mut pin = rppal::gpio::Gpio::new()
                                .unwrap()
                                .get(27)
                                .unwrap()
                                .into_output();

                            pin.set_high();
                            pin
                        });
                        cmd.insert_resource(Message { state: false })
                    }

                    while let Some(incoming_msg) = ev_reader.read().next() {
                        if let Some(msg) = incoming_msg
                            .get_action_msg::<<Message as ActionMessageHandler>::Request>()
                        {
                            let pin = pin.as_mut().unwrap();

                            if msg.state {
                                pin.set_low();
                                cmd.insert_resource(Message { state: true })
                            } else {
                                pin.set_high();
                                cmd.insert_resource(Message { state: false })
                            }
                        }
                    }
                }

                Some(IntoSystem::into_system(update).into_configs())
            }

            fn status_publish() -> Option<bevy_ecs::schedule::SystemConfigs> {
                Some(
                    IntoSystem::into_system(Self::publish_status)
                        .into_configs()
                        .run_if(on_timer(Duration::from_secs(1))),
                )
            }
        }
    }

    pub mod switch_1 {
        use super::*;

        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        pub struct Request {
            state: bool,
        }
        impl ActionMessage for Request {
            type Type = mqtt::add_on::action_message::action_type::Request;
            const PROJECT: &'static str = "triponics";
            const GROUP: &'static str = "switch_1";
            const DEVICE: &'static str = "0";
            const QOS: mqtt::Qos = mqtt::Qos::_1;
        }

        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        pub struct Response;
        impl ActionMessage for Response {
            type Type = mqtt::add_on::action_message::action_type::Response;
            const PROJECT: &'static str = "triponics";
            const GROUP: &'static str = "switch_1";
            const DEVICE: &'static str = "0";
            const QOS: mqtt::Qos = mqtt::Qos::_1;
        }

        #[derive(
            Debug, Clone, serde::Serialize, serde::Deserialize, bevy_ecs::system::Resource,
        )]
        pub struct Message {
            state: bool,
        }
        impl ActionMessage for Message {
            type Type = mqtt::add_on::action_message::action_type::Status;
            const PROJECT: &'static str = "triponics";
            const GROUP: &'static str = "switch_1";
            const DEVICE: &'static str = "0";
            const QOS: mqtt::Qos = mqtt::Qos::_1;
        }
        impl ActionMessageHandler for Message {
            type Status = Self;
            type Request = Request;
            type Response = Response;

            fn on_request() -> Option<bevy_ecs::schedule::SystemConfigs> {
                pub fn update(
                    mut cmd: Commands,
                    mut ev_reader: EventReader<mqtt::event::IncomingMessage>,
                    mut pin: Local<Option<rppal::gpio::OutputPin>>,
                ) {
                    if pin.is_none() {
                        log::debug!("init light gpio");
                        *pin = Some({
                            let mut pin = rppal::gpio::Gpio::new()
                                .unwrap()
                                .get(22)
                                .unwrap()
                                .into_output();

                            pin.set_high();
                            pin
                        });
                        cmd.insert_resource(Message { state: false })
                    }

                    while let Some(incoming_msg) = ev_reader.read().next() {
                        if let Some(msg) = incoming_msg
                            .get_action_msg::<<Message as ActionMessageHandler>::Request>()
                        {
                            let pin = pin.as_mut().unwrap();

                            if msg.state {
                                pin.set_low();
                                cmd.insert_resource(Message { state: true })
                            } else {
                                pin.set_high();
                                cmd.insert_resource(Message { state: false })
                            }
                        }
                    }
                }

                Some(IntoSystem::into_system(update).into_configs())
            }

            fn status_publish() -> Option<bevy_ecs::schedule::SystemConfigs> {
                Some(
                    IntoSystem::into_system(Self::publish_status)
                        .into_configs()
                        .run_if(on_timer(Duration::from_secs(1))),
                )
            }
        }
    }

    pub mod switch_2 {
        use super::*;

        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        pub struct Request {
            state: bool,
        }
        impl ActionMessage for Request {
            type Type = mqtt::add_on::action_message::action_type::Request;
            const PROJECT: &'static str = "triponics";
            const GROUP: &'static str = "switch_2";
            const DEVICE: &'static str = "0";
            const QOS: mqtt::Qos = mqtt::Qos::_1;
        }

        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        pub struct Response;
        impl ActionMessage for Response {
            type Type = mqtt::add_on::action_message::action_type::Response;
            const PROJECT: &'static str = "triponics";
            const GROUP: &'static str = "switch_2";
            const DEVICE: &'static str = "0";
            const QOS: mqtt::Qos = mqtt::Qos::_1;
        }

        #[derive(
            Debug, Clone, serde::Serialize, serde::Deserialize, bevy_ecs::system::Resource,
        )]
        pub struct Message {
            state: bool,
        }
        impl ActionMessage for Message {
            type Type = mqtt::add_on::action_message::action_type::Status;
            const PROJECT: &'static str = "triponics";
            const GROUP: &'static str = "switch_2";
            const DEVICE: &'static str = "0";
            const QOS: mqtt::Qos = mqtt::Qos::_1;
        }
        impl ActionMessageHandler for Message {
            type Status = Self;
            type Request = Request;
            type Response = Response;

            fn on_request() -> Option<bevy_ecs::schedule::SystemConfigs> {
                pub fn update(
                    mut cmd: Commands,
                    mut ev_reader: EventReader<mqtt::event::IncomingMessage>,
                    mut pin: Local<Option<rppal::gpio::OutputPin>>,
                ) {
                    if pin.is_none() {
                        log::debug!("init light gpio");
                        *pin = Some({
                            let mut pin = rppal::gpio::Gpio::new()
                                .unwrap()
                                .get(23)
                                .unwrap()
                                .into_output();

                            pin.set_high();
                            pin
                        });
                        cmd.insert_resource(Message { state: false })
                    }

                    while let Some(incoming_msg) = ev_reader.read().next() {
                        if let Some(msg) = incoming_msg
                            .get_action_msg::<<Message as ActionMessageHandler>::Request>()
                        {
                            let pin = pin.as_mut().unwrap();

                            if msg.state {
                                pin.set_low();
                                cmd.insert_resource(Message { state: true })
                            } else {
                                pin.set_high();
                                cmd.insert_resource(Message { state: false })
                            }
                        }
                    }
                }

                Some(IntoSystem::into_system(update).into_configs())
            }

            fn status_publish() -> Option<bevy_ecs::schedule::SystemConfigs> {
                Some(
                    IntoSystem::into_system(Self::publish_status)
                        .into_configs()
                        .run_if(on_timer(Duration::from_secs(1))),
                )
            }
        }
    }

    pub mod switch_3 {
        use super::*;

        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        pub struct Request {
            state: bool,
        }
        impl ActionMessage for Request {
            type Type = mqtt::add_on::action_message::action_type::Request;
            const PROJECT: &'static str = "triponics";
            const GROUP: &'static str = "switch_2";
            const DEVICE: &'static str = "0";
            const QOS: mqtt::Qos = mqtt::Qos::_1;
        }

        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        pub struct Response;
        impl ActionMessage for Response {
            type Type = mqtt::add_on::action_message::action_type::Response;
            const PROJECT: &'static str = "triponics";
            const GROUP: &'static str = "switch_2";
            const DEVICE: &'static str = "0";
            const QOS: mqtt::Qos = mqtt::Qos::_1;
        }

        #[derive(
            Debug, Clone, serde::Serialize, serde::Deserialize, bevy_ecs::system::Resource,
        )]
        pub struct Message {
            state: bool,
        }
        impl ActionMessage for Message {
            type Type = mqtt::add_on::action_message::action_type::Status;
            const PROJECT: &'static str = "triponics";
            const GROUP: &'static str = "switch_3";
            const DEVICE: &'static str = "0";
            const QOS: mqtt::Qos = mqtt::Qos::_1;
        }
        impl ActionMessageHandler for Message {
            type Status = Self;
            type Request = Request;
            type Response = Response;

            fn on_request() -> Option<bevy_ecs::schedule::SystemConfigs> {
                pub fn update(
                    mut cmd: Commands,
                    mut ev_reader: EventReader<mqtt::event::IncomingMessage>,
                    mut pin: Local<Option<rppal::gpio::OutputPin>>,
                ) {
                    if pin.is_none() {
                        log::debug!("init light gpio");
                        *pin = Some({
                            let mut pin = rppal::gpio::Gpio::new()
                                .unwrap()
                                .get(24)
                                .unwrap()
                                .into_output();

                            pin.set_high();
                            pin
                        });
                        cmd.insert_resource(Message { state: true })
                    }

                    while let Some(incoming_msg) = ev_reader.read().next() {
                        if let Some(msg) = incoming_msg
                            .get_action_msg::<<Message as ActionMessageHandler>::Request>()
                        {
                            let pin = pin.as_mut().unwrap();

                            if msg.state {
                                pin.set_high();
                                cmd.insert_resource(Message { state: true })
                            } else {
                                pin.set_low();
                                cmd.insert_resource(Message { state: false })
                            }
                        }
                    }
                }

                Some(IntoSystem::into_system(update).into_configs())
            }

            fn status_publish() -> Option<bevy_ecs::schedule::SystemConfigs> {
                Some(
                    IntoSystem::into_system(Self::publish_status)
                        .into_configs()
                        .run_if(on_timer(Duration::from_secs(1))),
                )
            }
        }
    }
}
