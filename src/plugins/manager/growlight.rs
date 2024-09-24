use bevy_ecs::system::Resource;
use bevy_internal::time::common_conditions::on_timer;

use crate::{
    constants,
    helper::{self, ErrorLogFormat},
    log,
    plugins::{
        mqtt::{self, add_on::action_message::PublishStatus},
        state_file,
    },
};

pub struct Plugin;
impl bevy_app::Plugin for Plugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_resource::<Manager>().add_plugins((
            state_file::StateFile::<Manager>::new(),
            mqtt::add_on::action_message::RequestMessage::<Manager>::new(),
            mqtt::add_on::action_message::StatusMessage::<Manager>::publish_condition(on_timer(
                std::time::Duration::from_secs(1),
            )),
        ));
    }
}

#[derive(Debug, Resource)]
pub struct Manager {
    gpio: rppal::gpio::OutputPin,
}
impl Manager {
    fn init() -> ResultStack<Self> {
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

    pub fn update_state(&mut self, request: action::Update) -> ResultStack<()> {
        let state = match request.state {
            true => helper::relay::State::Close,
            false => helper::relay::State::Open,
        };

        helper::relay::set_state(&mut self.gpio, state);
        log::trace!("[growlight] state updated -> {:?}", request);
        Ok(())
    }
}
impl Default for Manager {
    fn default() -> Self {
        Self::init().unwrap()
    }
}
impl state_file::SaveState for Manager {
    const FILENAME: &str = "growlight_manager";
    type State<'de> = action::Update;

    fn build(state: Self::State<'_>) -> Self {
        let mut this = Self::init().unwrap();
        this.update_state(state).unwrap();
        Self { gpio: this.gpio }
    }

    fn save<'de>(&self) -> Self::State<'de> {
        Self::State {
            state: self.get_status().state,
        }
    }
}
impl mqtt::add_on::action_message::RequestHandler for Manager {
    type Request = action::Update;
    type Response = action::MqttResponse;

    fn update_state(request: Self::Request, state: &mut Self) -> Option<Self::Response> {
        log::info!("[growlight] <ACT_MSG> set -> {}", request);

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
    type Status = action::MqttStatus;

    fn get_status(&self) -> Self::Status {
        Self::Status {
            state: helper::relay::get_state(&self.gpio),
        }
    }
}

pub mod action {
    use crate::{constants, plugins::mqtt, AtomicFixedString};

    const GROUP: &str = "growlight";
    const QOS: mqtt::Qos = mqtt::Qos::_1;

    #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
    pub struct Update {
        pub state: bool,
    }
    impl std::fmt::Display for Update {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            if self.state {
                write!(f, "ON")
            } else {
                write!(f, "OFF")
            }
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

type ResultStack<T> = error_stack::Result<T, Error>;
