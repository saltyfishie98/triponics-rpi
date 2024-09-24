use bevy_app::Update;
use bevy_ecs::system::{Local, Res, ResMut, Resource};
use bevy_internal::{prelude::DetectChanges, time::common_conditions::on_timer};

use super::{state_file, switch};
use crate::{
    helper::ErrorLogFormat,
    log,
    mqtt::{self, add_on::action_message::StatusMessage},
    timezone_offset,
};

pub struct Plugin {
    pub config: Config,
}
impl bevy_app::Plugin for Plugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_resource::<AeroponicSprayManager>()
            .insert_resource(self.config.clone())
            .add_plugins((
                StatusMessage::<AeroponicSprayManager>::publish_condition(on_timer(
                    std::time::Duration::from_secs(1),
                )),
                state_file::StateFile::<AeroponicSprayManager>::new(),
            ))
            .add_systems(
                Update,
                (
                    AeroponicSprayManager::watcher,
                    AeroponicSprayManager::update_switch,
                ),
            );
    }
}

#[derive(Debug, Clone, Resource)]
pub struct Config {
    pub spray_duration: std::time::Duration,
    pub spray_interval: std::time::Duration,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            spray_duration: std::time::Duration::from_secs(3),
            spray_interval: std::time::Duration::from_secs(5 * 60),
        }
    }
}

#[derive(Debug, Resource, serde::Serialize, serde::Deserialize, Clone)]
pub struct AeroponicSprayManager {
    sprayer_state: bool,
    next_spray_time: time::OffsetDateTime,
}
impl AeroponicSprayManager {
    pub fn set_state(&mut self, new_state: bool) {
        self.sprayer_state = new_state;
    }

    fn update_switch(mut switch_manager: ResMut<switch::SwitchManager>, this: Res<Self>) {
        if this.is_changed() {
            if let Err(e) = switch_manager.update_state(switch::action::Update {
                switch_1: None,
                switch_2: Some(this.sprayer_state),
                switch_3: None,
            }) {
                log::warn!("\n{}", e.fmt_error())
            }
        }
    }

    fn watcher(
        mut this: ResMut<Self>,
        mut maybe_end_time: Local<Option<time::OffsetDateTime>>,
        config: Res<Config>,
    ) {
        let now = time::OffsetDateTime::now_utc();

        if let Some(end_time) = *maybe_end_time {
            if end_time <= now {
                this.set_state(false);
                *maybe_end_time = None;
                this.next_spray_time = now + config.spray_interval;
            }
            return;
        }

        if this.next_spray_time <= now && maybe_end_time.is_none() {
            this.set_state(true);
            *maybe_end_time = Some(now + config.spray_duration);
        }
    }
}
impl Default for AeroponicSprayManager {
    fn default() -> Self {
        Self {
            sprayer_state: Default::default(),
            next_spray_time: time::OffsetDateTime::now_utc(),
        }
    }
}
impl state_file::SaveState for AeroponicSprayManager {
    type State<'de> = Self;

    const FILENAME: &str = "aeroponic_spray_manager";

    fn build(state: Self::State<'_>) -> Self {
        state
    }

    fn save<'de>(&self) -> Self::State<'de> {
        let mut state = self.clone();
        state.sprayer_state = false;
        state
    }
}
impl mqtt::add_on::action_message::PublishStatus for AeroponicSprayManager {
    type Status = action::Status;

    fn get_status(&self) -> Self::Status {
        let next_spray_time = if !self.sprayer_state {
            self.next_spray_time
                .to_offset(*timezone_offset())
                .to_string()
                .into()
        } else {
            "".into()
        };

        Self::Status {
            sprayer_state: self.sprayer_state,
            next_spray_time,
        }
    }
}

pub mod action {
    use crate::{constants, helper::AtomicFixedString, mqtt};

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct Status {
        pub sprayer_state: bool,
        pub next_spray_time: AtomicFixedString,
    }
    impl mqtt::add_on::action_message::MessageImpl for Status {
        type Type = mqtt::add_on::action_message::action_type::Status;
        const PROJECT: &'static str = constants::project::NAME;
        const GROUP: &'static str = "aeroponics";
        const DEVICE: &'static str = constants::project::DEVICE;
        const QOS: mqtt::Qos = mqtt::Qos::_1;
    }
}
