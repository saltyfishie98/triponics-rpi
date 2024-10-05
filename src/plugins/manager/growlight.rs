use std::time::Duration;

use bevy_app::{Startup, Update};
use bevy_ecs::{
    schedule::IntoSystemConfigs,
    system::{Commands, IntoSystem, Local, Res, ResMut, Resource},
};
use bevy_internal::{prelude::DetectChanges, time::common_conditions::on_timer};

use crate::{
    config::ConfigFile,
    helper::ToBytes,
    log,
    plugins::{manager, mqtt},
};

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Copy)]
pub struct Config {
    #[serde(
        serialize_with = "crate::helper::serde_time::serialize_time",
        deserialize_with = "crate::helper::serde_time::deserialize_time"
    )]
    start_time: time::Time,
    #[serde(
        serialize_with = "crate::helper::serde_time::serialize_duration_formatted",
        deserialize_with = "crate::helper::serde_time::deserialize_duration_formatted"
    )]
    on_duration: Duration,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            start_time: time::macros::time!(7:00 am),
            on_duration: Duration::from_secs(12 * 60 * 60),
        }
    }
}

pub struct Plugin {
    pub config: Config,
}
impl bevy_app::Plugin for Plugin {
    fn build(&self, app: &mut bevy_app::App) {
        use mqtt::add_on::action_message::{RequestMessage, StatusMessage};

        app.init_resource::<manager::RelayManager>()
            .init_resource::<Manager>()
            .insert_resource(StartTime(local::datetime_today(&self.config.start_time)))
            .insert_resource(EndTime(local::datetime_today(
                &(self.config.start_time + self.config.on_duration),
            )))
            .add_plugins((
                RequestMessage::<Manager>::new(),
                StatusMessage::<Manager, action::StatusMqtt>::publish_condition(
                    on_timer(std::time::Duration::from_secs(1)), //
                ),
            ))
            .add_systems(Startup, Manager::setup)
            .add_systems(
                Update,
                (
                    Manager::update,
                    Manager::trigger_on,
                    Manager::trigger_off,
                    Manager::update_datetime.run_if(
                        |start_time: Res<StartTime>| -> bool {
                            time::OffsetDateTime::now_utc()
                                .to_offset(*crate::timezone_offset())
                                .date()
                                > start_time.0.date()
                        }, // new day
                    ),
                ),
            );
    }
}

#[derive(Debug, Default, Resource, serde::Serialize, serde::Deserialize)]
pub struct Manager {
    pub state: bool,
}
impl Manager {
    pub fn turn_on(&mut self) {
        self.state = true;
    }

    pub fn turn_off(&mut self) {
        self.state = false;
    }

    fn setup(mut cmd: Commands) {
        #[derive(serde::Serialize)]
        struct Config {
            name: &'static str,
            icon: &'static str,
            state_topic: &'static str,
            value_template: &'static str,
            device: mqtt::add_on::home_assistant::Device,
        }

        cmd.spawn(mqtt::message::Message {
            topic: "homeassistant/sensor/on_time/growlight/config".into(),
            payload: {
                serde_json::to_value(Config {
                    name: "On Time",
                    icon: "mdi:clock",
                    state_topic: "status/triponics/growlight/0",
                    value_template:
                        "{{ (as_datetime(value_json.start_time) | as_local | string)[:19] }}",
                    device: mqtt::add_on::home_assistant::Device {
                        identifiers: &["growlight"],
                        name: "Growlight",
                    },
                })
                .unwrap()
                .to_bytes()
            },
            qos: mqtt::Qos::_1,
            retained: true,
        });

        cmd.spawn(mqtt::message::Message {
            topic: "homeassistant/sensor/off_time/growlight/config".into(),
            payload: {
                serde_json::to_value(Config {
                    name: "Off Time",
                    icon: "mdi:clock-outline",
                    state_topic: "status/triponics/growlight/0",
                    value_template:
                        "{{ (as_datetime(value_json.stop_time) | as_local | string)[:19] }}",
                    device: mqtt::add_on::home_assistant::Device {
                        identifiers: &["growlight"],
                        name: "Growlight",
                    },
                })
                .unwrap()
                .to_bytes()
            },
            qos: mqtt::Qos::_1,
            retained: true,
        });

        cmd.spawn(mqtt::message::Message {
            topic: "homeassistant/sensor/auto_state/growlight/config".into(),
            payload: {
                serde_json::to_value(Config {
                    name: "Auto State",
                    icon: "mdi:lightbulb-auto",
                    state_topic: "status/triponics/growlight/0",
                    value_template: "{{ \"ON\" if value_json.state else \"OFF\"}}",
                    device: mqtt::add_on::home_assistant::Device {
                        identifiers: &["growlight"],
                        name: "Growlight",
                    },
                })
                .unwrap()
                .to_bytes()
            },
            qos: mqtt::Qos::_1,
            retained: true,
        });
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

    fn trigger_on(
        mut this: ResMut<Manager>,
        start_time: Res<StartTime>,
        end_time: Res<EndTime>,
        mut pending_update: Local<bool>,
    ) {
        if start_time.is_changed() {
            *pending_update = true;
        }

        if time::OffsetDateTime::now_utc().to_offset(*crate::timezone_offset()) >= start_time.0
            && time::OffsetDateTime::now_utc().to_offset(*crate::timezone_offset()) < end_time.0
            && *pending_update
        {
            this.turn_on();
            *pending_update = false;
        }
    }

    fn trigger_off(
        mut this: ResMut<Manager>,
        end_time: Res<EndTime>,
        mut pending_update: Local<bool>,
    ) {
        if end_time.is_changed() {
            *pending_update = true;
        }

        if time::OffsetDateTime::now_utc().to_offset(*crate::timezone_offset()) >= end_time.0
            && *pending_update
        {
            this.turn_off();
            *pending_update = false;
        }
    }

    fn update_datetime(mut start_time: ResMut<StartTime>, mut end_time: ResMut<EndTime>) {
        *start_time = StartTime(local::datetime_today(&start_time.0.time()));
        *end_time = EndTime(local::datetime_today(&end_time.0.time()));
    }
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
impl mqtt::add_on::action_message::PublishStatus<action::StatusMqtt> for Manager {
    fn query_state() -> impl bevy_internal::prelude::System<In = (), Out = action::StatusMqtt> {
        fn func(
            this: Res<Manager>,
            start_time: Res<StartTime>,
            end_time: Res<EndTime>,
        ) -> action::StatusMqtt {
            action::StatusMqtt {
                state: this.state,
                start_time: start_time.0.unix_timestamp(),
                stop_time: end_time.0.unix_timestamp(),
            }
        }

        IntoSystem::into_system(func)
    }
}
impl ConfigFile for Manager {
    const FILENAME: &'static str = "growlight";
    type Config = Config;
}

#[derive(Debug, Resource)]
struct StartTime(time::OffsetDateTime);

#[derive(Debug, Resource)]
struct EndTime(time::OffsetDateTime);

pub mod action {
    use crate::{constants, plugins::mqtt, AtomicFixedString};

    pub(super) const GROUP: &str = "growlight";
    pub(super) const QOS: mqtt::Qos = mqtt::Qos::_1;

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct StatusMqtt {
        pub state: bool,
        pub start_time: i64,
        pub stop_time: i64,
    }
    impl mqtt::add_on::action_message::MessageImpl for StatusMqtt {
        const PREFIX: &'static str = constants::mqtt_prefix::STATUS;
        const PROJECT: &'static str = constants::project::NAME;
        const GROUP: &'static str = GROUP;
        const DEVICE: &'static str = constants::project::DEVICE;
        const QOS: mqtt::Qos = QOS;
    }

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

mod local {
    pub fn datetime_today(time: &time::Time) -> time::OffsetDateTime {
        let datetime = time::OffsetDateTime::now_utc().to_offset(*crate::timezone_offset());
        datetime.replace_time(*time)
    }
}
