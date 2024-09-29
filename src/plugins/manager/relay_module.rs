use bevy_app::Startup;
use bevy_ecs::system::{Commands, Resource};
use bevy_internal::time::common_conditions::on_timer;
use relay::Relay;

use crate::{
    helper::{ErrorLogFormat, ToBytes},
    log,
    plugins::mqtt,
};

pub struct Plugin;
impl bevy_app::Plugin for Plugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_resource::<Manager>()
            .add_plugins((
                mqtt::add_on::action_message::RequestMessage::<Manager>::new(),
                mqtt::add_on::action_message::StatusMessage::<Manager>::publish_condition(
                    on_timer(std::time::Duration::from_secs(1)),
                ),
            ))
            .add_systems(Startup, Manager::start);
    }
}

mod gpio {
    pub const SWITCH_1: u8 = 22;
    pub const SWITCH_2: u8 = 23;
    pub const SWITCH_3: u8 = 24;
    pub const PH_DOWN_PUMP: u8 = 25;
    pub const PH_UP_PUMP: u8 = 26;
    pub const GROWLIGHT: u8 = 27;
}

#[derive(Debug, Resource)]
pub struct Manager {
    relay_1: relay::NO<rppal::gpio::OutputPin>,
    relay_2: relay::NO<rppal::gpio::OutputPin>,
    relay_3: relay::NC<rppal::gpio::OutputPin>,
    relay_6: relay::NO<rppal::gpio::OutputPin>,
    relay_7: relay::NO<rppal::gpio::OutputPin>,
    relay_8: relay::NO<rppal::gpio::OutputPin>,
}
impl Default for Manager {
    fn default() -> Self {
        fn setup_gpio<T: relay::Relay<rppal::gpio::OutputPin>>(
            gpio: &rppal::gpio::Gpio,
            pin: u8,
        ) -> rppal::gpio::Result<T> {
            Ok(T::new(gpio.get(pin)?.into_output()))
        }

        let gpio = rppal::gpio::Gpio::new()
            .map_err(|e| {
                log::error!("{e}");
            })
            .unwrap();

        Self {
            relay_1: setup_gpio(&gpio, gpio::SWITCH_1)
                .map_err(|e| {
                    log::error!("[relay_module] failed to setup gpio pin for switch 1 relay, reason: {e}")
                })
                .unwrap(),
            relay_2: setup_gpio(&gpio, gpio::SWITCH_2)
                .map_err(|e| {
                    log::error!("[relay_module] failed to setup gpio pin for switch 2 relay, reason: {e}")
                })
                .unwrap(),
            relay_3: setup_gpio(&gpio, gpio::SWITCH_3)
                .map_err(|e| {
                    log::error!("[relay_module] failed to setup gpio pin for switch 3 relay, reason: {e}")
                })
                .unwrap(),
            relay_6: setup_gpio(&gpio, gpio::PH_DOWN_PUMP)
                .map_err(|e| {
                    log::error!(
                        "[relay_module] failed to setup gpio pin for pH down pump relay, reason: {e}"
                    )
                })
                .unwrap(),
            relay_7: setup_gpio(&gpio, gpio::PH_UP_PUMP)
                .map_err(|e| {
                    log::error!(
                        "[relay_module] failed to setup gpio pin for pH up pump relay, reason: {e}"
                    )
                })
                .unwrap(),
            relay_8: setup_gpio(&gpio, gpio::GROWLIGHT)
                .map_err(|e| {
                    log::error!("[relay_module] failed to setup gpio pin for growlight relay, reason: {e}")
                })
                .unwrap(),
        }
    }
}
impl Manager {
    pub fn start(mut cmd: Commands) {
        use mqtt::add_on::home_assistant::Device;

        #[derive(serde::Serialize)]
        struct HAConfig {
            name: &'static str,
            unique_id: &'static str,
            command_topic: &'static str,
            command_template: &'static str,
            payload_on: bool,
            payload_off: bool,
            state_topic: &'static str,
            value_template: &'static str,
            state_on: bool,
            state_off: bool,
            device: Device,
        }

        cmd.spawn(mqtt::message::Message {
            topic: "homeassistant/switch/relay_1/relay_module/config".into(),
            payload: {
                serde_json::to_value(HAConfig {
                    name: "Relay 1 (Switch 1)",
                    unique_id: "triponics-relay-module_1",
                    command_topic: "request/triponics/relay_module/0",
                    command_template: "{ \"relay_1\" : {{value | lower}} }",
                    payload_on: true,
                    payload_off: false,
                    state_topic: "data/triponics/relay_module/0",
                    value_template: "{{ value_json.relay_1 }}",
                    state_on: true,
                    state_off: false,
                    device: Device {
                        identifiers: &["triponics-relay-module"],
                        name: "Relay Module",
                    },
                })
                .unwrap()
                .to_bytes()
            },
            qos: mqtt::Qos::_1,
            retained: true,
        });

        cmd.spawn(mqtt::message::Message {
            topic: "homeassistant/switch/relay_2/relay_module/config".into(),
            payload: {
                serde_json::to_value(HAConfig {
                    name: "Relay 2 (Switch 2)",
                    unique_id: "triponics-relay-module_2",
                    command_topic: "request/triponics/relay_module/0",
                    command_template: "{ \"relay_2\" : {{value | lower}} }",
                    payload_on: true,
                    payload_off: false,
                    state_topic: "data/triponics/relay_module/0",
                    value_template: "{{ value_json.relay_2 }}",
                    state_on: true,
                    state_off: false,
                    device: Device {
                        identifiers: &["triponics-relay-module"],
                        name: "Relay Module",
                    },
                })
                .unwrap()
                .to_bytes()
            },
            qos: mqtt::Qos::_1,
            retained: true,
        });

        cmd.spawn(mqtt::message::Message {
            topic: "homeassistant/switch/relay_3/relay_module/config".into(),
            payload: {
                serde_json::to_value(HAConfig {
                    name: "Relay 3 (Switch 3)",
                    unique_id: "triponics-relay-module_3",
                    command_topic: "request/triponics/relay_module/0",
                    command_template: "{ \"relay_3\" : {{value | lower}} }",
                    payload_on: true,
                    payload_off: false,
                    state_topic: "data/triponics/relay_module/0",
                    value_template: "{{ value_json.relay_3 }}",
                    state_on: true,
                    state_off: false,
                    device: Device {
                        identifiers: &["triponics-relay-module"],
                        name: "Relay Module",
                    },
                })
                .unwrap()
                .to_bytes()
            },
            qos: mqtt::Qos::_1,
            retained: true,
        });

        cmd.spawn(mqtt::message::Message {
            topic: "homeassistant/switch/relay_4/relay_module/config".into(),
            payload: {
                serde_json::to_value(HAConfig {
                    name: "Relay 6 (Pump pH Down)",
                    unique_id: "triponics-relay-module_6",
                    command_topic: "request/triponics/relay_module/0",
                    command_template: "{ \"relay_6\" : {{value | lower}} }",
                    payload_on: true,
                    payload_off: false,
                    state_topic: "data/triponics/relay_module/0",
                    value_template: "{{ value_json.relay_6 }}",
                    state_on: true,
                    state_off: false,
                    device: Device {
                        identifiers: &["triponics-relay-module"],
                        name: "Relay Module",
                    },
                })
                .unwrap()
                .to_bytes()
            },
            qos: mqtt::Qos::_1,
            retained: true,
        });

        cmd.spawn(mqtt::message::Message {
            topic: "homeassistant/switch/relay_5/relay_module/config".into(),
            payload: {
                serde_json::to_value(HAConfig {
                    name: "Relay 7 (Pump pH Up)",
                    unique_id: "triponics-relay-module_7",
                    command_topic: "request/triponics/relay_module/0",
                    command_template: "{ \"relay_7\" : {{value | lower}} }",
                    payload_on: true,
                    payload_off: false,
                    state_topic: "data/triponics/relay_module/0",
                    value_template: "{{ value_json.relay_7 }}",
                    state_on: true,
                    state_off: false,
                    device: Device {
                        identifiers: &["triponics-relay-module"],
                        name: "Relay Module",
                    },
                })
                .unwrap()
                .to_bytes()
            },
            qos: mqtt::Qos::_1,
            retained: true,
        });

        cmd.spawn(mqtt::message::Message {
            topic: "homeassistant/switch/relay_8/relay_module/config".into(),
            payload: {
                serde_json::to_value(HAConfig {
                    name: "Relay 8 (Growlight)",
                    unique_id: "triponics-relay-module_8",
                    command_topic: "request/triponics/relay_module/0",
                    command_template: "{ \"relay_8\" : {{value | lower}} }",
                    payload_on: true,
                    payload_off: false,
                    state_topic: "data/triponics/relay_module/0",
                    value_template: "{{ value_json.relay_8 }}",
                    state_on: true,
                    state_off: false,
                    device: Device {
                        identifiers: &["triponics-relay-module"],
                        name: "Relay Module",
                    },
                })
                .unwrap()
                .to_bytes()
            },
            qos: mqtt::Qos::_1,
            retained: true,
        });
    }

    pub fn update_state(&mut self, request: action::Update) -> ResultStack<()> {
        fn update<T: relay::Relay<rppal::gpio::OutputPin>>(this: &mut T, new_state: Option<bool>) {
            if let Some(state) = new_state {
                this.set_state(state.into());
            }
        }

        update(&mut self.relay_1, request.relay_1);
        update(&mut self.relay_2, request.relay_2);
        update(&mut self.relay_3, request.relay_3);
        update(&mut self.relay_6, request.relay_6);
        update(&mut self.relay_7, request.relay_7);
        update(&mut self.relay_8, request.relay_8);

        log::trace!("[relay_module] state updated -> {:?}", request);

        Ok(())
    }
}
impl relay::RelayCtrl for rppal::gpio::OutputPin {
    fn energize(&mut self) {
        self.set_low()
    }

    fn de_energize(&mut self) {
        self.set_high();
    }

    fn is_energize(&self) -> bool {
        self.is_set_low()
    }
}
impl mqtt::add_on::action_message::RequestHandler for Manager {
    type Request = action::Update;
    type Response = action::MqttResponse;

    fn update_state(request: Self::Request, state: &mut Self) -> Option<Self::Response> {
        log::info!("[relay_module] <USER> set -> {request}");

        Some(action::MqttResponse(
            state
                .update_state(request)
                .map(|_| "stated updated!".into())
                .map_err(|e| {
                    log::warn!("\n{}", e.fmt_error());
                    "unknowned error!".into()
                }),
        ))
    }
}
impl mqtt::add_on::action_message::PublishStatus for Manager {
    type Status = action::RelayStatus;

    fn get_status(&self) -> Self::Status {
        Self::Status {
            relay_1: self.relay_1.get_state().into(),
            relay_2: self.relay_2.get_state().into(),
            relay_3: self.relay_3.get_state().into(),
            relay_6: self.relay_6.get_state().into(),
            relay_7: self.relay_7.get_state().into(),
            relay_8: self.relay_8.get_state().into(),
        }
    }
}

pub mod action {
    use crate::{constants, plugins::mqtt, AtomicFixedString};

    use super::relay;

    const GROUP: &str = "relay_module";
    const QOS: mqtt::Qos = mqtt::Qos::_1;

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct Update {
        pub relay_1: Option<bool>,
        pub relay_2: Option<bool>,
        pub relay_3: Option<bool>,
        pub relay_6: Option<bool>,
        pub relay_7: Option<bool>,
        pub relay_8: Option<bool>,
    }
    impl Update {
        pub fn empty() -> Self {
            Self {
                relay_1: None,
                relay_2: None,
                relay_3: None,
                relay_6: None,
                relay_7: None,
                relay_8: None,
            }
        }
    }
    impl mqtt::add_on::action_message::MessageImpl for Update {
        const PREFIX: &'static str = constants::mqtt_prefix::REQUEST;
        const PROJECT: &'static str = constants::project::NAME;
        const GROUP: &'static str = GROUP;
        const DEVICE: &'static str = constants::project::DEVICE;
        const QOS: mqtt::Qos = QOS;
    }
    impl Default for Update {
        fn default() -> Self {
            Self {
                relay_1: Some(relay::State::Open.into()),
                relay_2: Some(relay::State::Open.into()),
                relay_3: Some(relay::State::Close.into()),
                relay_6: Some(relay::State::Open.into()),
                relay_7: Some(relay::State::Open.into()),
                relay_8: Some(relay::State::Open.into()),
            }
        }
    }
    impl std::fmt::Display for Update {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            fn show_state(b: bool) -> &'static str {
                if b {
                    "ON"
                } else {
                    "OFF"
                }
            }

            let mut disp = f.debug_map();

            if let Some(sw) = self.relay_1 {
                disp.entry(&"relay_1", &show_state(sw));
            }

            if let Some(sw) = self.relay_2 {
                disp.entry(&"relay_2", &show_state(sw));
            }

            if let Some(sw) = self.relay_3 {
                disp.entry(&"relay_3", &show_state(sw));
            }

            if let Some(sw) = self.relay_6 {
                disp.entry(&"relay_6", &show_state(sw));
            }

            if let Some(sw) = self.relay_7 {
                disp.entry(&"relay_7", &show_state(sw));
            }

            if let Some(sw) = self.relay_8 {
                disp.entry(&"relay_8", &show_state(sw));
            }

            disp.finish()
        }
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct RelayStatus {
        pub relay_1: bool,
        pub relay_2: bool,
        pub relay_3: bool,
        pub relay_6: bool,
        pub relay_7: bool,
        pub relay_8: bool,
    }
    impl mqtt::add_on::action_message::MessageImpl for RelayStatus {
        const PREFIX: &'static str = constants::mqtt_prefix::STATUS;
        const PROJECT: &'static str = constants::project::NAME;
        const GROUP: &'static str = GROUP;
        const DEVICE: &'static str = constants::project::DEVICE;
        const QOS: mqtt::Qos = QOS;
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct MqttResponse(pub Result<AtomicFixedString, AtomicFixedString>);
    impl mqtt::add_on::action_message::MessageImpl for MqttResponse {
        const PREFIX: &'static str = constants::mqtt_prefix::RESPONSE;
        const PROJECT: &'static str = constants::project::NAME;
        const GROUP: &'static str = GROUP;
        const DEVICE: &'static str = constants::project::DEVICE;
        const QOS: mqtt::Qos = QOS;
    }
}

mod relay {
    pub trait RelayCtrl {
        fn energize(&mut self);
        fn de_energize(&mut self);
        fn is_energize(&self) -> bool;
    }

    pub trait Relay<T: RelayCtrl> {
        fn new(inner: T) -> Self;
        fn set_state(&mut self, state: State);
        fn get_state(&self) -> State;
    }

    #[derive(Debug)]
    pub struct NO<T: RelayCtrl>(T);
    impl<T: RelayCtrl> Relay<T> for NO<T> {
        fn new(inner: T) -> Self {
            Self(inner)
        }

        fn set_state(&mut self, state: State) {
            match state {
                State::Open => self.0.de_energize(),
                State::Close => self.0.energize(),
            }
        }

        fn get_state(&self) -> State {
            if self.0.is_energize() {
                State::Close
            } else {
                State::Open
            }
        }
    }

    #[derive(Debug)]
    pub struct NC<T: RelayCtrl>(pub T);
    impl<T: RelayCtrl> Relay<T> for NC<T> {
        fn new(inner: T) -> Self {
            Self(inner)
        }

        fn set_state(&mut self, state: State) {
            match state {
                State::Open => self.0.energize(),
                State::Close => self.0.de_energize(),
            }
        }

        fn get_state(&self) -> State {
            if self.0.is_energize() {
                State::Open
            } else {
                State::Close
            }
        }
    }

    #[derive(Debug, Copy, Clone)]
    pub enum State {
        Open,
        Close,
    }
    impl From<bool> for State {
        fn from(value: bool) -> Self {
            match value {
                true => Self::Close,
                false => Self::Open,
            }
        }
    }
    impl From<State> for bool {
        fn from(value: State) -> Self {
            match value {
                State::Open => false,
                State::Close => true,
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {}

type ResultStack<T> = error_stack::Result<T, Error>;
