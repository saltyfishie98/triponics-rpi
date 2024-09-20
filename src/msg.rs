use bevy_ecs::{
    event::EventReader,
    system::{Commands, IntoSystem, Local},
};
use tracing as log;

use crate::mqtt;

pub mod relay {
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

        fn update_system() -> impl bevy_ecs::system::System<In = (), Out = ()> {
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

            IntoSystem::into_system(update)
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

        fn update_system() -> impl bevy_ecs::system::System<In = (), Out = ()> {
            IntoSystem::into_system(Self::update)
        }
    }
    impl Switch01 {
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
                cmd.insert_resource(Self { state: false })
            }

            while let Some(incoming_msg) = ev_reader.read().next() {
                if let Some(msg) = incoming_msg.get::<Switch01>() {
                    let pin = pin.as_mut().unwrap();

                    if msg.state {
                        pin.set_low();
                        cmd.insert_resource(Self { state: true })
                    } else {
                        pin.set_high();
                        cmd.insert_resource(Self { state: false })
                    }
                }
            }
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

        fn update_system() -> impl bevy_ecs::system::System<In = (), Out = ()> {
            IntoSystem::into_system(Self::update)
        }
    }
    impl Switch02 {
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
                cmd.insert_resource(Self { state: false })
            }

            while let Some(incoming_msg) = ev_reader.read().next() {
                if let Some(msg) = incoming_msg.get::<Switch02>() {
                    let pin = pin.as_mut().unwrap();

                    if msg.state {
                        pin.set_low();
                        cmd.insert_resource(Self { state: true })
                    } else {
                        pin.set_high();
                        cmd.insert_resource(Self { state: false })
                    }
                }
            }
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

        fn update_system() -> impl bevy_ecs::system::System<In = (), Out = ()> {
            IntoSystem::into_system(Self::update)
        }
    }
    impl Switch03 {
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
                cmd.insert_resource(Self { state: true })
            }

            while let Some(incoming_msg) = ev_reader.read().next() {
                if let Some(msg) = incoming_msg.get::<Switch03>() {
                    let pin = pin.as_mut().unwrap();

                    if msg.state {
                        pin.set_high();
                        cmd.insert_resource(Self { state: true })
                    } else {
                        pin.set_low();
                        cmd.insert_resource(Self { state: false })
                    }
                }
            }
        }
    }
}
