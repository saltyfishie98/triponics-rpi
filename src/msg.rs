use bevy_ecs::{
    event::EventReader,
    system::{Commands, IntoSystem, Local},
};
use tracing as log;

use crate::mqtt;

pub mod relay {
    use std::time::Duration;

    use bevy_ecs::schedule::IntoSystemConfigs;
    use bevy_internal::time::common_conditions::on_timer;

    use super::*;

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, bevy_ecs::system::Resource)]
    pub struct GrowLight {
        state: bool,
    }
    impl mqtt::MqttMessage for GrowLight {
        const PROJECT: &'static str = "triponics";
        const GROUP: &'static str = "growlight";
        const DEVICE: &'static str = "0";

        const STATUS_QOS: mqtt::Qos = mqtt::Qos::_1;
        const ACTION_QOS: Option<mqtt::Qos> = Some(mqtt::Qos::_1);
    }
    impl mqtt::SystemStateMsgHandler for GrowLight {
        fn update() -> bevy_ecs::schedule::SystemConfigs {
            fn update(
                mut cmd: Commands,
                mut ev_reader: EventReader<mqtt::event::IncomingMessage>,
                mut pin: Local<Option<rppal::gpio::OutputPin>>,
            ) {
                if pin.is_none() {
                    log::debug!("init light gpio");
                    *pin = Some({
                        let mut pin = rppal::gpio::Gpio::new()
                            .unwrap()
                            .get(27)
                            .unwrap()
                            .into_output();

                        pin.set_high();
                        pin
                    });
                    cmd.insert_resource(GrowLight { state: false })
                }

                while let Some(incoming_msg) = ev_reader.read().next() {
                    if let Some(msg) = incoming_msg.get::<GrowLight>() {
                        let pin = pin.as_mut().unwrap();

                        if msg.state {
                            pin.set_low();
                            cmd.insert_resource(GrowLight { state: true })
                        } else {
                            pin.set_high();
                            cmd.insert_resource(GrowLight { state: false })
                        }
                    }
                }
            }

            IntoSystem::into_system(update).into_configs()
        }

        fn status() -> Option<bevy_ecs::schedule::SystemConfigs> {
            Some(
                IntoSystem::into_system(Self::publish_status)
                    .into_configs()
                    .run_if(on_timer(Duration::from_secs(1))),
            )
        }
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, bevy_ecs::system::Resource)]
    pub struct Switch01 {
        state: bool,
    }
    impl mqtt::MqttMessage for Switch01 {
        const PROJECT: &'static str = "triponics";
        const GROUP: &'static str = "switch_1";
        const DEVICE: &'static str = "0";

        const STATUS_QOS: mqtt::Qos = mqtt::Qos::_1;
        const ACTION_QOS: Option<mqtt::Qos> = Some(mqtt::Qos::_1);
    }
    impl mqtt::SystemStateMsgHandler for Switch01 {
        fn update() -> bevy_ecs::schedule::SystemConfigs {
            pub fn update(
                mut cmd: Commands,
                mut ev_reader: EventReader<mqtt::event::IncomingMessage>,
                mut pin: Local<Option<rppal::gpio::OutputPin>>,
            ) {
                if pin.is_none() {
                    log::debug!("init light gpio");
                    *pin = Some({
                        let mut pin = rppal::gpio::Gpio::new()
                            .unwrap()
                            .get(22)
                            .unwrap()
                            .into_output();

                        pin.set_high();
                        pin
                    });
                    cmd.insert_resource(Switch01 { state: false })
                }

                while let Some(incoming_msg) = ev_reader.read().next() {
                    if let Some(msg) = incoming_msg.get::<Switch01>() {
                        let pin = pin.as_mut().unwrap();

                        if msg.state {
                            pin.set_low();
                            cmd.insert_resource(Switch01 { state: true })
                        } else {
                            pin.set_high();
                            cmd.insert_resource(Switch01 { state: false })
                        }
                    }
                }
            }

            IntoSystem::into_system(update).into_configs()
        }

        fn status() -> Option<bevy_ecs::schedule::SystemConfigs> {
            Some(
                IntoSystem::into_system(Self::publish_status)
                    .into_configs()
                    .run_if(on_timer(Duration::from_secs(1))),
            )
        }
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, bevy_ecs::system::Resource)]
    pub struct Switch02 {
        state: bool,
    }
    impl mqtt::MqttMessage for Switch02 {
        const PROJECT: &'static str = "triponics";
        const GROUP: &'static str = "switch_2";
        const DEVICE: &'static str = "0";

        const STATUS_QOS: mqtt::Qos = mqtt::Qos::_1;
        const ACTION_QOS: Option<mqtt::Qos> = Some(mqtt::Qos::_1);
    }
    impl mqtt::SystemStateMsgHandler for Switch02 {
        fn update() -> bevy_ecs::schedule::SystemConfigs {
            pub fn update(
                mut cmd: Commands,
                mut ev_reader: EventReader<mqtt::event::IncomingMessage>,
                mut pin: Local<Option<rppal::gpio::OutputPin>>,
            ) {
                if pin.is_none() {
                    log::debug!("init light gpio");
                    *pin = Some({
                        let mut pin = rppal::gpio::Gpio::new()
                            .unwrap()
                            .get(23)
                            .unwrap()
                            .into_output();

                        pin.set_high();
                        pin
                    });
                    cmd.insert_resource(Switch02 { state: false })
                }

                while let Some(incoming_msg) = ev_reader.read().next() {
                    if let Some(msg) = incoming_msg.get::<Switch02>() {
                        let pin = pin.as_mut().unwrap();

                        if msg.state {
                            pin.set_low();
                            cmd.insert_resource(Switch02 { state: true })
                        } else {
                            pin.set_high();
                            cmd.insert_resource(Switch02 { state: false })
                        }
                    }
                }
            }

            IntoSystem::into_system(update).into_configs()
        }

        fn status() -> Option<bevy_ecs::schedule::SystemConfigs> {
            Some(
                IntoSystem::into_system(Self::publish_status)
                    .into_configs()
                    .run_if(on_timer(Duration::from_secs(1))),
            )
        }
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, bevy_ecs::system::Resource)]
    pub struct Switch03 {
        state: bool,
    }
    impl mqtt::MqttMessage for Switch03 {
        const PROJECT: &'static str = "triponics";
        const GROUP: &'static str = "switch_3";
        const DEVICE: &'static str = "0";

        const STATUS_QOS: mqtt::Qos = mqtt::Qos::_1;
        const ACTION_QOS: Option<mqtt::Qos> = Some(mqtt::Qos::_1);
    }
    impl mqtt::SystemStateMsgHandler for Switch03 {
        fn update() -> bevy_ecs::schedule::SystemConfigs {
            pub fn update(
                mut cmd: Commands,
                mut ev_reader: EventReader<mqtt::event::IncomingMessage>,
                mut pin: Local<Option<rppal::gpio::OutputPin>>,
            ) {
                if pin.is_none() {
                    log::debug!("init light gpio");
                    *pin = Some({
                        let mut pin = rppal::gpio::Gpio::new()
                            .unwrap()
                            .get(24)
                            .unwrap()
                            .into_output();

                        pin.set_high();
                        pin
                    });
                    cmd.insert_resource(Switch03 { state: true })
                }

                while let Some(incoming_msg) = ev_reader.read().next() {
                    if let Some(msg) = incoming_msg.get::<Switch03>() {
                        let pin = pin.as_mut().unwrap();

                        if msg.state {
                            pin.set_high();
                            cmd.insert_resource(Switch03 { state: true })
                        } else {
                            pin.set_low();
                            cmd.insert_resource(Switch03 { state: false })
                        }
                    }
                }
            }

            IntoSystem::into_system(update).into_configs()
        }

        fn status() -> Option<bevy_ecs::schedule::SystemConfigs> {
            Some(
                IntoSystem::into_system(Self::publish_status)
                    .into_configs()
                    .run_if(on_timer(Duration::from_secs(1))),
            )
        }
    }
}
