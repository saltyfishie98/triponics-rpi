use std::time::Duration;

use bevy_app::{Startup, Update};
use bevy_ecs::system::{Commands, Res, Resource};
use bevy_internal::prelude::DetectChanges;
use bevy_tokio_tasks::TokioTasksRuntime;

use crate::{config::ConfigFile, constants, helper::ToBytes, log, mqtt, plugins};

pub struct Plugin {
    pub config: Config,
}
impl bevy_app::Plugin for Plugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_resource::<plugins::manager::RelayManager>()
            .insert_resource(Manager::new(self.config))
            .add_plugins((
                mqtt::add_on::action_message::RequestMessage::<Manager>::new(),
                mqtt::add_on::action_message::ConfigMessage::<Manager, Config>::new(),
            ))
            .add_systems(Startup, (Manager::register_home_assistant,))
            .add_systems(Update, (Manager::update_ph_down, Manager::update_ph_up));
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Copy)]
pub struct Config {
    #[serde(
        serialize_with = "crate::helper::serde_time::serialize_duration_formatted",
        deserialize_with = "crate::helper::serde_time::deserialize_duration_formatted"
    )]
    pub unit_time_user: Duration,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            unit_time_user: Duration::from_secs(3),
        }
    }
}
impl mqtt::add_on::action_message::MessageImpl for Config {
    const PREFIX: &'static str = constants::mqtt_prefix::CONFIG;
    const PROJECT: &'static str = constants::project::NAME;
    const GROUP: &'static str = action::GROUP;
    const DEVICE: &'static str = constants::project::DEVICE;
    const QOS: mqtt::Qos = mqtt::Qos::_1;
}

#[derive(Debug, Resource)]
pub struct Manager {
    ph_down_state: bool,
    ph_up_state: bool,
    config: Config,
}
impl Manager {
    fn new(config: Config) -> Self {
        Self {
            config,
            ph_down_state: false,
            ph_up_state: false,
        }
    }

    fn register_home_assistant(mut cmd: Commands) {
        #[derive(serde::Serialize)]
        struct Config {
            name: &'static str,
            command_topic: &'static str,
            command_template: &'static str,
            payload_press: bool,
            device: mqtt::add_on::home_assistant::Device,
        }

        #[derive(serde::Serialize)]
        struct State {
            name: &'static str,
            state_topic: &'static str,
            value_template: &'static str,
            icon: &'static str,
            device: mqtt::add_on::home_assistant::Device,
        }

        cmd.spawn(mqtt::message::Message {
            topic: "homeassistant/button/down/ph_dosing/config".into(),
            payload: {
                serde_json::to_value(Config {
                    name: "Dose pH Down",
                    command_topic: "request/triponics/ph_dosing/0",
                    command_template: "{ \"ph_down\" : {{value | lower}} }",
                    payload_press: true,
                    device: mqtt::add_on::home_assistant::Device {
                        identifiers: &["triponics-ph-dosing"],
                        name: "Dosing Pumps",
                    },
                })
                .unwrap()
                .to_bytes()
            },
            qos: mqtt::Qos::_1,
            retained: true,
        });

        cmd.spawn(mqtt::message::Message {
            topic: "homeassistant/button/up/ph_dosing/config".into(),
            payload: {
                serde_json::to_value(Config {
                    name: "Dose pH Up",
                    command_topic: "request/triponics/ph_dosing/0",
                    command_template: "{ \"ph_up\" : {{value | lower}} }",
                    payload_press: true,
                    device: mqtt::add_on::home_assistant::Device {
                        identifiers: &["triponics-ph-dosing"],
                        name: "Dosing Pumps",
                    },
                })
                .unwrap()
                .to_bytes()
            },
            qos: mqtt::Qos::_1,
            retained: true,
        });

        cmd.spawn(mqtt::message::Message {
            topic: "homeassistant/sensor/ph_down_pump/ph_dosing/config".into(),
            payload: {
                serde_json::to_value(State {
                    name: "Pump pH Down",
                    state_topic: "status/triponics/relay_module/0",
                    value_template: "{{ \"ON\" if value_json.relay_6 else \"OFF\"}}",
                    device: mqtt::add_on::home_assistant::Device {
                        identifiers: &["triponics-ph-dosing"],
                        name: "Dosing Pumps",
                    },
                    icon: "mdi:arrow-down-bold-circle",
                })
                .unwrap()
                .to_bytes()
            },
            qos: mqtt::Qos::_1,
            retained: true,
        });

        cmd.spawn(mqtt::message::Message {
            topic: "homeassistant/sensor/ph_up_pump/ph_dosing/config".into(),
            payload: {
                serde_json::to_value(State {
                    name: "Pump pH Up",
                    state_topic: "status/triponics/relay_module/0",
                    value_template: "{{ \"ON\" if value_json.relay_7 else \"OFF\"}}",
                    device: mqtt::add_on::home_assistant::Device {
                        identifiers: &["triponics-ph-dosing"],
                        name: "Dosing Pumps",
                    },
                    icon: "mdi:arrow-up-bold-circle",
                })
                .unwrap()
                .to_bytes()
            },
            qos: mqtt::Qos::_1,
            retained: true,
        });
    }

    fn update_state(&mut self, state: action::Update) {
        log::trace!("{state:?}");

        let action::Update { ph_down, ph_up } = state;

        if let Some(down_state) = ph_down {
            self.ph_down_state = down_state;
        }

        if let Some(up_state) = ph_up {
            self.ph_up_state = up_state;
        }
    }

    fn update_ph_down(rt: Res<TokioTasksRuntime>, this: Res<Self>) {
        if !this.is_changed() || this.is_added() {
            return;
        }

        if !this.ph_down_state {
            return;
        }

        let dur = this.config.unit_time_user;

        rt.spawn_background_task(move |mut ctx| async move {
            ctx.run_on_main_thread(|ctx| {
                let mut relay_manager = ctx
                    .world
                    .get_resource_mut::<plugins::manager::RelayManager>()
                    .unwrap();

                relay_manager
                    .update_state(plugins::manager::relay_module::action::Update {
                        relay_6: Some(true),
                        ..plugins::manager::relay_module::action::Update::empty()
                    })
                    .unwrap();
            })
            .await;

            tokio::time::sleep(dur).await;

            ctx.run_on_main_thread(|ctx| {
                let world = ctx.world;

                let mut relay_manager = world
                    .get_resource_mut::<plugins::manager::RelayManager>()
                    .unwrap();

                relay_manager
                    .update_state(plugins::manager::relay_module::action::Update {
                        relay_6: Some(false),
                        ..plugins::manager::relay_module::action::Update::empty()
                    })
                    .unwrap();

                let mut this = world.get_resource_mut::<Self>().unwrap();
                this.ph_down_state = false;

                let request = action::Update {
                    ph_down: Some(this.ph_down_state),
                    ph_up: None,
                };
                log::info!("[ph_dosing] <APP> set -> {request}");
            })
            .await;
        });
    }

    fn update_ph_up(rt: Res<TokioTasksRuntime>, this: Res<Self>) {
        if !this.is_changed() || this.is_added() {
            return;
        }

        if !this.ph_up_state {
            return;
        }

        let dur = this.config.unit_time_user;

        rt.spawn_background_task(move |mut ctx| async move {
            ctx.run_on_main_thread(|ctx| {
                let mut relay_manager = ctx
                    .world
                    .get_resource_mut::<plugins::manager::RelayManager>()
                    .unwrap();

                relay_manager
                    .update_state(plugins::manager::relay_module::action::Update {
                        relay_7: Some(true),
                        ..plugins::manager::relay_module::action::Update::empty()
                    })
                    .unwrap();
            })
            .await;

            tokio::time::sleep(dur).await;

            ctx.run_on_main_thread(|ctx| {
                let world = ctx.world;

                let mut relay_manager = world
                    .get_resource_mut::<plugins::manager::RelayManager>()
                    .unwrap();

                relay_manager
                    .update_state(plugins::manager::relay_module::action::Update {
                        relay_7: Some(false),
                        ..plugins::manager::relay_module::action::Update::empty()
                    })
                    .unwrap();

                let mut this = world.get_resource_mut::<Self>().unwrap();
                this.ph_up_state = false;

                let request = action::Update {
                    ph_down: None,
                    ph_up: Some(this.ph_up_state),
                };
                log::info!("[ph_dosing] <APP> set -> {request}");
            })
            .await;
        });
    }
}
impl mqtt::add_on::action_message::RequestHandler for Manager {
    type Request = action::Update;
    type Response = action::Response;

    fn update_state(request: Self::Request, state: &mut Self) -> Option<Self::Response> {
        log::info!("[ph_dosing] <USER> set -> {request}");

        state.update_state(request);
        Some(action::Response(Ok("updated ph dosing state".into())))
    }
}
impl ConfigFile for Manager {
    const FILENAME: &'static str = "ph_dosing";
    type Config = Config;
}

mod action {
    use crate::{constants, mqtt, AtomicFixedString};

    pub const GROUP: &str = "ph_dosing";

    #[derive(Debug, serde::Deserialize, serde::Serialize)]
    pub struct Update {
        pub ph_down: Option<bool>,
        pub ph_up: Option<bool>,
    }
    impl mqtt::add_on::action_message::MessageImpl for Update {
        const PREFIX: &'static str = constants::mqtt_prefix::REQUEST;
        const PROJECT: &'static str = constants::project::NAME;
        const GROUP: &'static str = GROUP;
        const DEVICE: &'static str = constants::project::DEVICE;
        const QOS: mqtt::Qos = mqtt::Qos::_1;
    }
    impl std::fmt::Display for Update {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let mut disp = f.debug_map();

            if let Some(up) = self.ph_up {
                disp.entry(&"ph_up", &up);
            }

            if let Some(down) = self.ph_down {
                disp.entry(&"ph_down", &down);
            };

            disp.finish()
        }
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct Response(pub Result<AtomicFixedString, AtomicFixedString>);
    impl mqtt::add_on::action_message::MessageImpl for Response {
        const PREFIX: &'static str = constants::mqtt_prefix::RESPONSE;
        const PROJECT: &'static str = constants::project::NAME;
        const GROUP: &'static str = GROUP;
        const DEVICE: &'static str = constants::project::DEVICE;
        const QOS: mqtt::Qos = mqtt::Qos::_1;
    }
}
