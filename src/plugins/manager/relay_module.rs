use bevy_ecs::system::Resource;
use bevy_internal::time::common_conditions::on_timer;
use relay::Relay;

use crate::{helper::ErrorLogFormat, log, plugins::mqtt};

pub struct Plugin;
impl bevy_app::Plugin for Plugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_resource::<Manager>().add_plugins((
            mqtt::add_on::action_message::RequestMessage::<Manager>::new(),
            mqtt::add_on::action_message::StatusMessage::<Manager>::publish_condition(on_timer(
                std::time::Duration::from_secs(1),
            )),
        ));
    }
}

mod gpio {
    pub const SWITCH_1: u8 = 22;
    pub const SWITCH_2: u8 = 23;
    pub const SWITCH_3: u8 = 24;
    pub const PH_UP_PUMP: u8 = 25;
    pub const PH_DOWN_PUMP: u8 = 26;
    pub const GROWLIGHT: u8 = 27;
}

#[derive(Debug, Resource)]
pub struct Manager {
    gpio_switch_1: relay::NO<rppal::gpio::OutputPin>,
    gpio_switch_2: relay::NO<rppal::gpio::OutputPin>,
    gpio_switch_3: relay::NC<rppal::gpio::OutputPin>,
    gpio_ph_down: relay::NO<rppal::gpio::OutputPin>,
    gpio_ph_up: relay::NO<rppal::gpio::OutputPin>,
    gpio_growlight: relay::NO<rppal::gpio::OutputPin>,
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
            gpio_switch_1: setup_gpio(&gpio, gpio::SWITCH_1)
                .map_err(|e| {
                    log::error!("[relay_module] failed to setup gpio pin for switch 1 relay, reason: {e}")
                })
                .unwrap(),
            gpio_switch_2: setup_gpio(&gpio, gpio::SWITCH_2)
                .map_err(|e| {
                    log::error!("[relay_module] failed to setup gpio pin for switch 2 relay, reason: {e}")
                })
                .unwrap(),
            gpio_switch_3: setup_gpio(&gpio, gpio::SWITCH_3)
                .map_err(|e| {
                    log::error!("[relay_module] failed to setup gpio pin for switch 3 relay, reason: {e}")
                })
                .unwrap(),
            gpio_ph_down: setup_gpio(&gpio, gpio::PH_DOWN_PUMP)
                .map_err(|e| {
                    log::error!(
                        "[relay_module] failed to setup gpio pin for pH down pump relay, reason: {e}"
                    )
                })
                .unwrap(),
            gpio_ph_up: setup_gpio(&gpio, gpio::PH_UP_PUMP)
                .map_err(|e| {
                    log::error!(
                        "[relay_module] failed to setup gpio pin for pH up pump relay, reason: {e}"
                    )
                })
                .unwrap(),
            gpio_growlight: setup_gpio(&gpio, gpio::GROWLIGHT)
                .map_err(|e| {
                    log::error!("[relay_module] failed to setup gpio pin for growlight relay, reason: {e}")
                })
                .unwrap(),
        }
    }
}
impl Manager {
    pub fn update_state(&mut self, request: action::Update) -> ResultStack<()> {
        fn update<T: relay::Relay<rppal::gpio::OutputPin>>(this: &mut T, new_state: Option<bool>) {
            if let Some(state) = new_state {
                this.set_state(state.into());
            }
        }

        update(&mut self.gpio_switch_1, request.switch_1);
        update(&mut self.gpio_switch_2, request.switch_2);
        update(&mut self.gpio_switch_3, request.switch_3);
        update(&mut self.gpio_ph_down, request.ph_down_pump);
        update(&mut self.gpio_ph_up, request.ph_up_pump);
        update(&mut self.gpio_growlight, request.growlight);

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
            switch_1: self.gpio_switch_1.get_state().into(),
            switch_2: self.gpio_switch_2.get_state().into(),
            switch_3: self.gpio_switch_3.get_state().into(),
            ph_down_pump: self.gpio_ph_down.get_state().into(),
            ph_up_pump: self.gpio_ph_up.get_state().into(),
            ph_growlight: self.gpio_growlight.get_state().into(),
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
        pub switch_1: Option<bool>,
        pub switch_2: Option<bool>,
        pub switch_3: Option<bool>,
        pub ph_down_pump: Option<bool>,
        pub ph_up_pump: Option<bool>,
        pub growlight: Option<bool>,
    }
    impl mqtt::add_on::action_message::MessageImpl for Update {
        type Type = mqtt::add_on::action_message::action_type::Request;
        const PROJECT: &'static str = constants::project::NAME;
        const GROUP: &'static str = GROUP;
        const DEVICE: &'static str = constants::project::DEVICE;
        const QOS: mqtt::Qos = QOS;
    }
    impl Default for Update {
        fn default() -> Self {
            Self {
                switch_1: Some(relay::State::Open.into()),
                switch_2: Some(relay::State::Open.into()),
                switch_3: Some(relay::State::Close.into()),
                ph_down_pump: Some(relay::State::Open.into()),
                ph_up_pump: Some(relay::State::Open.into()),
                growlight: Some(relay::State::Open.into()),
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

            if let Some(sw) = self.switch_1 {
                disp.entry(&"switch_1", &show_state(sw));
            }

            if let Some(sw) = self.switch_2 {
                disp.entry(&"switch_2", &show_state(sw));
            }

            if let Some(sw) = self.switch_3 {
                disp.entry(&"switch_3", &show_state(sw));
            }

            if let Some(sw) = self.ph_down_pump {
                disp.entry(&"ph_down_pump", &show_state(sw));
            }

            if let Some(sw) = self.ph_up_pump {
                disp.entry(&"ph_up_pump", &show_state(sw));
            }

            if let Some(sw) = self.growlight {
                disp.entry(&"growlight", &show_state(sw));
            }

            disp.finish()
        }
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct RelayStatus {
        pub switch_1: bool,
        pub switch_2: bool,
        pub switch_3: bool,
        pub ph_down_pump: bool,
        pub ph_up_pump: bool,
        pub ph_growlight: bool,
    }
    impl mqtt::add_on::action_message::MessageImpl for RelayStatus {
        type Type = mqtt::add_on::action_message::action_type::Status;
        const PROJECT: &'static str = constants::project::NAME;
        const GROUP: &'static str = GROUP;
        const DEVICE: &'static str = constants::project::DEVICE;
        const QOS: mqtt::Qos = QOS;
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct MqttResponse(pub Result<AtomicFixedString, AtomicFixedString>);
    impl mqtt::add_on::action_message::MessageImpl for MqttResponse {
        type Type = mqtt::add_on::action_message::action_type::Response;
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
