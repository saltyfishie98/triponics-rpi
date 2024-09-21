use bevy_ecs::system::Resource;

use crate::{
    constants,
    helper::{self, ErrorLogFormat},
    log, mqtt,
};

#[derive(Debug, Resource)]
pub struct SwitchManager {
    gpio_switch_1: rppal::gpio::OutputPin,
    gpio_switch_2: rppal::gpio::OutputPin,
    gpio_switch_3: rppal::gpio::OutputPin,
}
impl SwitchManager {
    pub fn new() -> Result<Self> {
        fn init_gpio(pin: u8) -> Result<rppal::gpio::OutputPin> {
            Ok(rppal::gpio::Gpio::new()
                .map_err(|e| {
                    error_stack::report!(Error::Setup).attach_printable(format!("reason: '{e}'"))
                })?
                .get(pin)
                .map_err(|e| {
                    error_stack::report!(Error::Setup).attach_printable(format!("reason: '{e}'"))
                })?
                .into_output())
        }

        let mut out = Self {
            gpio_switch_1: init_gpio(constants::gpio::SWITCH_1)?,
            gpio_switch_2: init_gpio(constants::gpio::SWITCH_2)?,
            gpio_switch_3: init_gpio(constants::gpio::SWITCH_3)?,
        };

        helper::relay::set_state(&mut out.gpio_switch_1, false);
        helper::relay::set_state(&mut out.gpio_switch_2, false);
        helper::relay::set_state(&mut out.gpio_switch_3, false);

        Ok(out)
    }
}
impl mqtt::add_on::action_message::State for SwitchManager {
    type Status = action::Status;
    type Request = action::Request;
    type Response = action::Response;

    fn init() -> Self {
        Self::new()
            .map_err(|e| log::error!("\n{}", e.fmt_error()))
            .unwrap()
    }

    fn get_status(&self) -> Self::Status {
        Self::Status {
            switch_1: helper::relay::get_state(&self.gpio_switch_1),
            switch_2: helper::relay::get_state(&self.gpio_switch_2),
            switch_3: !helper::relay::get_state(&self.gpio_switch_3),
        }
    }

    fn update_state(request: Self::Request, state: &mut Self) -> Option<Self::Response> {
        fn update(pin: &mut rppal::gpio::OutputPin, new_state: Option<bool>) {
            if let Some(state) = new_state {
                helper::relay::set_state(pin, state);
            }
        }

        update(&mut state.gpio_switch_1, request.switch_1);
        update(&mut state.gpio_switch_2, request.switch_2);
        update(&mut state.gpio_switch_3, request.switch_3);

        Some(action::Response(Ok("changed state!".into())))
    }
}

mod action {
    use bevy_ecs::system::Resource;

    use crate::{constants, helper::AtomicFixedString, mqtt};

    const GROUP: &str = "switch";
    const QOS: mqtt::Qos = mqtt::Qos::_1;

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct Request {
        pub switch_1: Option<bool>,
        pub switch_2: Option<bool>,
        pub switch_3: Option<bool>,
    }
    impl mqtt::add_on::action_message::Impl for Request {
        type Type = mqtt::add_on::action_message::action_type::Request;
        const PROJECT: &'static str = constants::project::NAME;
        const GROUP: &'static str = GROUP;
        const DEVICE: &'static str = constants::project::DEVICE;
        const QOS: mqtt::Qos = QOS;
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Resource)]
    pub struct Status {
        pub switch_1: bool,
        pub switch_2: bool,
        pub switch_3: bool,
    }
    impl mqtt::add_on::action_message::Impl for Status {
        type Type = mqtt::add_on::action_message::action_type::Status;
        const PROJECT: &'static str = constants::project::NAME;
        const GROUP: &'static str = GROUP;
        const DEVICE: &'static str = constants::project::DEVICE;
        const QOS: mqtt::Qos = QOS;
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct Response(pub Result<AtomicFixedString, AtomicFixedString>);
    impl mqtt::add_on::action_message::Impl for Response {
        type Type = mqtt::add_on::action_message::action_type::Response;
        const PROJECT: &'static str = constants::project::NAME;
        const GROUP: &'static str = GROUP;
        const DEVICE: &'static str = constants::project::DEVICE;
        const QOS: mqtt::Qos = QOS;
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("error setting up switch manager")]
    Setup,
}

type Result<T> = error_stack::Result<T, Error>;
