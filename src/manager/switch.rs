use bevy_ecs::system::Resource;

use crate::{
    constants,
    helper::{self, ErrorLogFormat},
    log, mqtt,
};

pub struct Plugin;
impl bevy_app::Plugin for Plugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_plugins(mqtt::add_on::ActionMessage::<SwitchManager>::new(
            Some(std::time::Duration::from_secs(1)), //
        ));
    }
}

#[derive(Debug, Resource)]
pub struct SwitchManager {
    gpio_switch_1: rppal::gpio::OutputPin,
    gpio_switch_2: rppal::gpio::OutputPin,
    gpio_switch_3: rppal::gpio::OutputPin,
}
impl SwitchManager {
    pub fn init() -> Result<Self> {
        fn init_gpio(pin: u8) -> Result<rppal::gpio::OutputPin> {
            let mut out = rppal::gpio::Gpio::new()
                .map_err(|e| {
                    error_stack::report!(Error::Setup).attach_printable(format!("reason: '{e}'"))
                })?
                .get(pin)
                .map_err(|e| {
                    error_stack::report!(Error::Setup).attach_printable(format!("reason: '{e}'"))
                })?
                .into_output();

            helper::relay::set_state(&mut out, helper::relay::State::Open);
            Ok(out)
        }

        Ok(Self {
            gpio_switch_1: init_gpio(constants::gpio::SWITCH_1)?,
            gpio_switch_2: init_gpio(constants::gpio::SWITCH_2)?,
            gpio_switch_3: init_gpio(constants::gpio::SWITCH_3)?,
        })
    }

    pub fn update_state(&mut self, request: action::Update) -> Result<()> {
        fn update(pin: &mut rppal::gpio::OutputPin, new_state: Option<bool>, flip: bool) {
            if let Some(request_state) = new_state {
                let state = if !flip {
                    match request_state {
                        true => helper::relay::State::Close,
                        false => helper::relay::State::Open,
                    }
                } else {
                    match request_state {
                        true => helper::relay::State::Open,
                        false => helper::relay::State::Close,
                    }
                };

                helper::relay::set_state(pin, state);
            }
        }

        update(&mut self.gpio_switch_1, request.switch_1, false);
        update(&mut self.gpio_switch_2, request.switch_2, false);
        update(&mut self.gpio_switch_3, request.switch_3, true);

        Ok(())
    }
}
impl Default for SwitchManager {
    fn default() -> Self {
        Self::init()
            .map_err(|e| log::error!("\n{}", e.fmt_error()))
            .unwrap()
    }
}
impl mqtt::add_on::action_message::State for SwitchManager {
    type Status = action::MqttStatus;
    type Request = action::Update;
    type Response = action::MqttResponse;

    fn get_status(&self) -> Self::Status {
        Self::Status {
            switch_1: helper::relay::get_state(&self.gpio_switch_1),
            switch_2: helper::relay::get_state(&self.gpio_switch_2),
            switch_3: !helper::relay::get_state(&self.gpio_switch_3),
        }
    }

    fn update_state(request: Self::Request, state: &mut Self) -> Option<Self::Response> {
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

mod action {
    use crate::{constants, helper::AtomicFixedString, mqtt};

    const GROUP: &str = "switch";
    const QOS: mqtt::Qos = mqtt::Qos::_1;

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct Update {
        pub switch_1: Option<bool>,
        pub switch_2: Option<bool>,
        pub switch_3: Option<bool>,
    }
    impl mqtt::add_on::action_message::Impl for Update {
        type Type = mqtt::add_on::action_message::action_type::Request;
        const PROJECT: &'static str = constants::project::NAME;
        const GROUP: &'static str = GROUP;
        const DEVICE: &'static str = constants::project::DEVICE;
        const QOS: mqtt::Qos = QOS;
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct MqttStatus {
        pub switch_1: bool,
        pub switch_2: bool,
        pub switch_3: bool,
    }
    impl mqtt::add_on::action_message::Impl for MqttStatus {
        type Type = mqtt::add_on::action_message::action_type::Status;
        const PROJECT: &'static str = constants::project::NAME;
        const GROUP: &'static str = GROUP;
        const DEVICE: &'static str = constants::project::DEVICE;
        const QOS: mqtt::Qos = QOS;
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct MqttResponse(pub Result<AtomicFixedString, AtomicFixedString>);
    impl mqtt::add_on::action_message::Impl for MqttResponse {
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
