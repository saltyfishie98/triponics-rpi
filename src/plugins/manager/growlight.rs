use bevy_app::Update;
use bevy_ecs::system::{Res, ResMut, Resource};
use bevy_internal::{prelude::DetectChanges, time::common_conditions::on_timer};

use crate::{
    constants, log,
    plugins::{manager, mqtt, state_file},
};

pub struct Plugin;
impl bevy_app::Plugin for Plugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_resource::<manager::RelayManager>()
            .init_resource::<Manager>()
            .add_plugins((
                state_file::StateFile::<Manager>::new(),
                mqtt::add_on::action_message::RequestMessage::<Manager>::new(),
                mqtt::add_on::action_message::StatusMessage::<Manager, Manager>::publish_condition(
                    on_timer(std::time::Duration::from_secs(1)),
                ),
            ))
            .add_systems(Update, Manager::update);
    }
}

#[derive(Debug, Resource, Default, serde::Serialize, serde::Deserialize)]
pub struct Manager {
    state: bool,
}
impl Manager {
    pub fn turn_on(&mut self) {
        self.state = true;
    }

    pub fn turn_off(&mut self) {
        self.state = false;
    }

    fn update(this: Res<Manager>, mut relay_manager: ResMut<manager::RelayManager>) {
        if this.is_changed() {
            if let Err(e) = relay_manager.update_state(
                manager::relay_module::action::Update {
                    relay_8: Some(this.state),
                    ..Default::default()
                }, //
            ) {
                log::warn!("[growlight] failed to update relay manager, reason:\n{e:#?}\n");
            }
        }
    }
}
impl mqtt::add_on::action_message::MessageImpl for Manager {
    const PREFIX: &'static str = constants::mqtt_prefix::DATABASE;
    const PROJECT: &'static str = constants::project::NAME;
    const GROUP: &'static str = action::GROUP;
    const DEVICE: &'static str = constants::project::DEVICE;
    const QOS: mqtt::Qos = action::QOS;
}
impl mqtt::add_on::action_message::RequestHandler for Manager {
    type Request = action::Update;
    type Response = action::MqttResponse;

    fn update_state(request: Self::Request, this: &mut Self) -> Option<Self::Response> {
        log::info!("[growlight] <USER> set -> {}", request);

        match request.state {
            true => {
                this.turn_on();
                Some(Ok("growlight turned on").into())
            }
            false => {
                this.turn_off();
                Some(Ok("growlight turned off").into())
            }
        }
    }
}
impl mqtt::add_on::action_message::PublishStatus for Manager {
    fn get_status(&self) -> impl mqtt::add_on::action_message::MessageImpl {
        Self { state: self.state }
    }
}
impl state_file::SaveState for Manager {
    const FILENAME: &str = "growlight_manager";
    type State<'de> = Self;

    fn build(state: Self::State<'_>) -> Self {
        state
    }

    fn save<'de>(&self) -> Self::State<'de> {
        Self { state: self.state }
    }
}

pub mod action {
    use crate::{constants, plugins::mqtt, AtomicFixedString};

    pub(super) const GROUP: &str = "growlight";
    pub(super) const QOS: mqtt::Qos = mqtt::Qos::_1;

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
        const PREFIX: &'static str = constants::mqtt_prefix::REQUEST;
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
    impl From<Result<&'static str, &'static str>> for MqttResponse {
        fn from(value: Result<&'static str, &'static str>) -> Self {
            Self(value.map(|o| o.into()).map_err(|e| e.into()))
        }
    }
}
