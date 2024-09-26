use bevy_app::Update;
use bevy_ecs::system::{Local, Res, ResMut, Resource};
use bevy_internal::{prelude::DetectChanges, time::common_conditions::on_timer};

use super::relay_module;
use crate::{
    helper::ErrorLogFormat,
    log,
    plugins::{
        manager,
        mqtt::{self, add_on::action_message::StatusMessage},
        state_file,
    },
};

pub struct Plugin {
    pub config: Config,
}
impl bevy_app::Plugin for Plugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_resource::<manager::RelayManager>()
            .init_resource::<Manager>()
            .insert_resource(self.config.clone())
            .add_plugins((
                StatusMessage::<Manager>::publish_condition(on_timer(
                    std::time::Duration::from_secs(1),
                )),
                state_file::StateFile::<Manager>::new(),
            ))
            .add_systems(Update, (Manager::watcher, Manager::update_switch));
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
pub struct Manager {
    sprayer_state: bool,
    #[serde(
        serialize_with = "crate::helper::time::serialize_offset_datetime_as_local",
        deserialize_with = "crate::helper::time::deserialize_offset_datetime_as_local"
    )]
    next_spray_time: time::OffsetDateTime,
}
impl Manager {
    pub fn update_state(&mut self, new_state: bool) {
        self.sprayer_state = new_state;
        log::trace!("[aeroponic_spray] state updated -> {:?}", new_state);
    }

    fn update_switch(mut relay_manager: ResMut<relay_module::Manager>, this: Res<Self>) {
        if this.is_changed() {
            if let Err(e) = relay_manager.update_state(
                relay_module::action::Update {
                    switch_2: Some(this.sprayer_state),
                    ..Default::default()
                }, //
            ) {
                log::warn!(
                    "[aeroponic_spray] failed to update relay manager, reason:\n{}",
                    e.fmt_error()
                )
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
                this.update_state(false);
                *maybe_end_time = None;
                this.next_spray_time = now + config.spray_interval;

                let next_spray_local_time = {
                    let o = this.next_spray_time;
                    o.to_offset(*crate::timezone_offset())
                        .format(&crate::time_log_fmt())
                        .unwrap()
                };

                log::info!(
                    "[aeroponic_spray] <APP> set -> OFF (next spray time: {})",
                    next_spray_local_time
                )
            }
            return;
        }

        if this.next_spray_time <= now && maybe_end_time.is_none() {
            let end_time = now + config.spray_duration;

            this.update_state(true);
            *maybe_end_time = Some(end_time);

            log::info!(
                "[aeroponic_spray] <APP> set -> ON (spray until: {})",
                end_time
                    .to_offset(*crate::timezone_offset())
                    .format(&crate::time_log_fmt())
                    .unwrap()
            );
        }
    }
}
impl Default for Manager {
    fn default() -> Self {
        Self {
            sprayer_state: Default::default(),
            next_spray_time: time::OffsetDateTime::now_utc().to_offset(*crate::timezone_offset()),
        }
    }
}
impl state_file::SaveState for Manager {
    type State<'de> = Self;

    const FILENAME: &str = "aeroponic_spray_manager";

    fn build(state: Self::State<'_>) -> Self {
        state
    }

    fn save<'de>(&self) -> Self::State<'de> {
        let mut state = self.clone();
        state.sprayer_state = false;
        state.next_spray_time.to_offset(*crate::timezone_offset());
        state
    }
}
impl mqtt::add_on::action_message::PublishStatus for Manager {
    type Status = action::AeroponicSprayerStatus;

    fn get_status(&self) -> Self::Status {
        let next_spray_time = if !self.sprayer_state {
            self.next_spray_time
                .to_offset(*crate::timezone_offset())
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
    use crate::{constants, plugins::mqtt, AtomicFixedString};

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct AeroponicSprayerStatus {
        pub sprayer_state: bool,
        pub next_spray_time: AtomicFixedString,
    }
    impl mqtt::add_on::action_message::MessageImpl for AeroponicSprayerStatus {
        const PREFIX: &'static str = constants::mqtt_prefix::STATUS;
        const PROJECT: &'static str = constants::project::NAME;
        const GROUP: &'static str = "aeroponics";
        const DEVICE: &'static str = constants::project::DEVICE;
        const QOS: mqtt::Qos = mqtt::Qos::_1;
    }
}
