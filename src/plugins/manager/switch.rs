use bevy_ecs::system::Resource;
use bevy_internal::time::common_conditions::on_timer;

use crate::{
    constants,
    helper::{self, ErrorLogFormat},
    log,
    plugins::mqtt,
};

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

#[derive(Debug, Resource)]
pub struct Manager {
    gpio_switch_1: rppal::gpio::OutputPin,
    gpio_switch_2: rppal::gpio::OutputPin,
    gpio_switch_3: rppal::gpio::OutputPin,
}
impl Manager {
    pub fn init() -> ResultStack<Self> {
        fn init_gpio(pin: u8) -> ResultStack<rppal::gpio::OutputPin> {
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

    pub fn update_state(&mut self, request: action::Update) -> ResultStack<()> {
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
        log::trace!("[switch] state updated -> {:?}", request);

        Ok(())
    }
}
impl Default for Manager {
    fn default() -> Self {
        Self::init()
            .map_err(|e| log::error!("\n{}", e.fmt_error()))
            .unwrap()
    }
}
impl mqtt::add_on::action_message::RequestHandler for Manager {
    type Request = action::Update;
    type Response = action::MqttResponse;

    fn update_state(request: Self::Request, state: &mut Self) -> Option<Self::Response> {
        log::info!("[switch] <USER> set -> {request}");

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
    type Status = action::SwitchStatus;

    fn get_status(&self) -> Self::Status {
        Self::Status {
            switch_1: helper::relay::get_state(&self.gpio_switch_1),
            switch_2: helper::relay::get_state(&self.gpio_switch_2),
            switch_3: !helper::relay::get_state(&self.gpio_switch_3),
        }
    }
}

pub mod action {
    use crate::{constants, plugins::mqtt, AtomicFixedString};

    const GROUP: &str = "switch";
    const QOS: mqtt::Qos = mqtt::Qos::_1;

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct Update {
        pub switch_1: Option<bool>,
        pub switch_2: Option<bool>,
        pub switch_3: Option<bool>,
    }
    impl Default for Update {
        fn default() -> Self {
            Self {
                switch_1: Some(false),
                switch_2: Some(false),
                switch_3: Some(true),
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

            disp.finish()
        }
    }
    impl mqtt::add_on::action_message::MessageImpl for Update {
        type Type = mqtt::add_on::action_message::action_type::Request;
        const PROJECT: &'static str = constants::project::NAME;
        const GROUP: &'static str = GROUP;
        const DEVICE: &'static str = constants::project::DEVICE;
        const QOS: mqtt::Qos = QOS;
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct SwitchStatus {
        pub switch_1: bool,
        pub switch_2: bool,
        pub switch_3: bool,
    }
    impl mqtt::add_on::action_message::MessageImpl for SwitchStatus {
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

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("error setting up switch manager")]
    Setup,
}

type ResultStack<T> = error_stack::Result<T, Error>;
