use std::time::Duration;

use bevy_app::{Startup, Update};
use bevy_ecs::system::{Commands, Res, Resource};
use bevy_internal::prelude::DetectChanges;
use bevy_tokio_tasks::TokioTasksRuntime;

use crate::{helper::ToBytes, log, mqtt, plugins};

pub struct Plugin;
impl bevy_app::Plugin for Plugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_resource::<plugins::manager::RelayManager>()
            .init_resource::<Manager>()
            .add_plugins(mqtt::add_on::action_message::RequestMessage::<Manager>::new())
            .add_systems(Startup, (Manager::register_home_assistant,))
            .add_systems(Update, (Manager::update_ph_down, Manager::update_ph_up));
    }
}

#[derive(Debug, Resource, serde::Deserialize, serde::Serialize)]
pub struct Manager {
    ph_down_state: bool,
    ph_up_state: bool,
    #[serde(skip)]
    unit_time: Duration,
}
impl Default for Manager {
    fn default() -> Self {
        Self {
            ph_down_state: Default::default(),
            ph_up_state: Default::default(),
            unit_time: Duration::from_secs(3),
        }
    }
}
impl Manager {
    fn register_home_assistant(mut cmd: Commands) {
        #[derive(serde::Serialize)]
        struct Config {
            name: &'static str,
            command_topic: &'static str,
            command_template: &'static str,
            payload_press: bool,
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

        let dur = this.unit_time;

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

        let dur = this.unit_time;

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
            })
            .await;
        });
    }
}
impl mqtt::add_on::action_message::RequestHandler for Manager {
    type Request = action::Update;
    type Response = action::Response;

    fn update_state(request: Self::Request, state: &mut Self) -> Option<Self::Response> {
        state.update_state(request);
        Some(action::Response(Ok("updated ph dosing state".into())))
    }
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
