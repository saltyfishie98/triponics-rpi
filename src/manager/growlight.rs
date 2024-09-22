use bevy_ecs::system::Resource;
use bevy_internal::time::common_conditions::on_timer;

use crate::{
    constants,
    helper::{self, ErrorLogFormat},
    log, mqtt,
};

pub struct Plugin;
impl bevy_app::Plugin for Plugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_plugins((
            mqtt::add_on::action_message::RequestMessage::<GrowlightManager>::new(),
            mqtt::add_on::action_message::StatusMessage::<GrowlightManager>::publish_condition(
                on_timer(std::time::Duration::from_secs(1)),
            ),
        ));
    }
}

#[derive(Debug, Resource)]
pub struct GrowlightManager {
    gpio: rppal::gpio::OutputPin,
}
impl GrowlightManager {
    fn init() -> Result<Self> {
        let mut gpio = rppal::gpio::Gpio::new()
            .map_err(|e| {
                error_stack::report!(Error::Setup).attach_printable(format!("reason: '{e}'"))
            })?
            .get(constants::gpio::GROWLIGHT)
            .map_err(|e| {
                error_stack::report!(Error::Setup).attach_printable(format!("reason: '{e}'"))
            })?
            .into_output();

        helper::relay::set_state(&mut gpio, helper::relay::State::Open);

        Ok(Self { gpio })
    }

    pub fn update_state(&mut self, request: action::Update) -> Result<()> {
        let state = match request.state {
            true => helper::relay::State::Close,
            false => helper::relay::State::Open,
        };

        helper::relay::set_state(&mut self.gpio, state);
        Ok(())
    }
}
impl Default for GrowlightManager {
    fn default() -> Self {
        Self::init()
            .map_err(|e| log::error!("\n{}", e.fmt_error()))
            .unwrap()
    }
}
impl mqtt::add_on::action_message::RequestHandler for GrowlightManager {
    type Request = action::Update;
    type Response = action::MqttResponse;

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
impl mqtt::add_on::action_message::PublishStatus for GrowlightManager {
    type Status = action::MqttStatus;

    fn get_status(&self) -> Self::Status {
        Self::Status {
            state: helper::relay::get_state(&self.gpio),
        }
    }
}

pub mod action {
    use crate::{constants, helper::AtomicFixedString, mqtt};

    const GROUP: &str = "growlight";
    const QOS: mqtt::Qos = mqtt::Qos::_1;

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct Update {
        pub state: bool,
    }
    impl mqtt::add_on::action_message::MessageImpl for Update {
        type Type = mqtt::add_on::action_message::action_type::Request;
        const PROJECT: &'static str = constants::project::NAME;
        const GROUP: &'static str = GROUP;
        const DEVICE: &'static str = constants::project::DEVICE;
        const QOS: mqtt::Qos = QOS;
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct MqttStatus {
        pub state: bool,
    }
    impl mqtt::add_on::action_message::MessageImpl for MqttStatus {
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
    #[error("error setting up growlight manager")]
    Setup,
}

type Result<T> = error_stack::Result<T, Error>;
