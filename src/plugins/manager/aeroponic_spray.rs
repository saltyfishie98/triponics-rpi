use bevy_app::{Startup, Update};
use bevy_ecs::system::{Commands, IntoSystem, Local, Res, ResMut, Resource};
use bevy_internal::{prelude::DetectChanges, time::common_conditions::on_timer};

use super::relay_module;
use crate::{
    config::ConfigFile,
    constants,
    helper::ErrorLogFormat,
    log,
    mqtt::add_on::action_message::ConfigMessage,
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
            .insert_resource(Manager::new(self.config))
            .add_plugins((
                StatusMessage::<Manager, action::AeroponicSprayerStatus>::publish_condition(
                    on_timer(std::time::Duration::from_secs(1)),
                ),
                ConfigMessage::<Manager, Config>::new(),
                state_file::StateFile::<Manager>::new(),
            ))
            .add_systems(Startup, (Manager::setup,))
            .add_systems(Update, (Manager::watcher, Manager::update));
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Config {
    #[serde(
        serialize_with = "crate::helper::serde_time::serialize_duration_formatted",
        deserialize_with = "crate::helper::serde_time::deserialize_duration_formatted"
    )]
    pub spray_duration: std::time::Duration,
    #[serde(
        serialize_with = "crate::helper::serde_time::serialize_duration_formatted",
        deserialize_with = "crate::helper::serde_time::deserialize_duration_formatted"
    )]
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
impl mqtt::add_on::action_message::MessageImpl for Config {
    const PREFIX: &'static str = constants::mqtt_prefix::CONFIG;
    const PROJECT: &'static str = constants::project::NAME;
    const GROUP: &'static str = "aeroponics";
    const DEVICE: &'static str = constants::project::DEVICE;
    const QOS: mqtt::Qos = mqtt::Qos::_1;
}

#[derive(Debug, Resource, serde::Serialize, serde::Deserialize)]
pub struct Manager {
    sprayer_state: bool,
    #[serde(
        serialize_with = "crate::helper::serde_time::serialize_offset_datetime_as_local",
        deserialize_with = "crate::helper::serde_time::deserialize_offset_datetime_as_local"
    )]
    next_spray_time: time::OffsetDateTime,
    #[serde(skip)]
    spray_duration: std::time::Duration,
    #[serde(skip)]
    spray_interval: std::time::Duration,
}
impl Manager {
    pub fn turn_on(&mut self) {
        self.sprayer_state = true;
    }

    pub fn turn_off(&mut self) {
        self.sprayer_state = false;
    }

    fn new(config: Config) -> Self {
        Self {
            sprayer_state: false,
            next_spray_time: time::OffsetDateTime::now_utc().to_offset(*crate::timezone_offset()),
            spray_duration: config.spray_duration,
            spray_interval: config.spray_interval,
        }
    }

    fn setup(mut cmd: Commands) {
        use crate::helper::ToBytes;
        use mqtt::add_on::home_assistant::Device;

        #[derive(serde::Serialize)]
        struct Config {
            name: &'static str,
            icon: &'static str,
            state_topic: &'static str,
            value_template: &'static str,
            device: Device,
        }

        cmd.spawn(mqtt::message::Message {
            topic: "homeassistant/sensor/state/aeroponic_spray/config".into(),
            payload: {
                serde_json::to_value(Config {
                    name: "Sprayer Controller Command",
                    icon: "mdi:car-cruise-control",
                    state_topic: "status/triponics/aeroponics/0",
                    value_template: "{{ \"ON\" if value_json.sprayer_state else \"OFF\" }}",
                    device: Device {
                        identifiers: &["aeroponics"],
                        name: "Aeroponics",
                    },
                })
                .unwrap()
                .to_bytes()
            },
            qos: mqtt::Qos::_1,
            retained: true,
        });

        cmd.spawn(mqtt::message::Message {
            topic: "homeassistant/sensor/next_spray_time/aeroponic_spray/config".into(),
            payload: {
                serde_json::to_value(Config {
                    name: "Next Scheduled Spray",
                    icon: "mdi:clock",
                    state_topic: "status/triponics/aeroponics/0",
                    value_template:
                        "{{ (as_datetime(value_json.next_spray_time) | as_local | string )[:19] }}",
                    device: Device {
                        identifiers: &["aeroponics"],
                        name: "Aeroponics",
                    },
                })
                .unwrap()
                .to_bytes()
            },
            qos: mqtt::Qos::_1,
            retained: true,
        });
    }

    fn update(mut relay_manager: ResMut<relay_module::Manager>, this: Res<Self>) {
        if this.is_changed() {
            if let Err(e) = relay_manager.update_state(
                relay_module::action::Update {
                    relay_2: Some(this.sprayer_state),
                    ..relay_module::action::Update::empty()
                }, //
            ) {
                log::warn!(
                    "[aeroponic_spray] failed to update relay manager, reason:\n{}",
                    e.fmt_error()
                )
            }
        }
    }

    fn watcher(mut this: ResMut<Self>, mut maybe_end_time: Local<Option<time::OffsetDateTime>>) {
        let now = time::OffsetDateTime::now_utc();

        if let Some(end_time) = *maybe_end_time {
            if end_time <= now {
                this.turn_off();
                *maybe_end_time = None;
                this.next_spray_time = now + this.spray_interval;

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
            let end_time = now + this.spray_duration;

            this.turn_on();
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
impl ConfigFile for Manager {
    const FILENAME: &'static str = "aeroponic_spray";
    type Config = Config;
}
impl state_file::SaveState for Manager {
    type State<'de> = Self;

    const FILENAME: &str = "aeroponic_spray_manager";

    fn build(state: Self::State<'_>, this: Option<Self>) -> Self {
        if let Some(this) = this {
            Self {
                sprayer_state: state.sprayer_state,
                next_spray_time: state.next_spray_time,
                spray_duration: this.spray_duration,
                spray_interval: this.spray_interval,
            }
        } else {
            state
        }
    }

    fn save<'de>(&self) -> Self::State<'de> {
        let Self {
            sprayer_state,
            next_spray_time,
            spray_duration,
            spray_interval,
        } = *self;

        Self {
            sprayer_state,
            next_spray_time,
            spray_duration,
            spray_interval,
        }
    }
}
impl mqtt::add_on::action_message::PublishStatus<action::AeroponicSprayerStatus> for Manager {
    fn query_state(
    ) -> impl bevy_internal::prelude::System<In = (), Out = action::AeroponicSprayerStatus> {
        fn func(this: Res<Manager>) -> action::AeroponicSprayerStatus {
            action::AeroponicSprayerStatus {
                sprayer_state: this.sprayer_state,
                next_spray_time: this.next_spray_time.unix_timestamp().to_string().into(),
            }
        }

        IntoSystem::into_system(func)
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
