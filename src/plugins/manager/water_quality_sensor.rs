use std::time::Duration;

use bevy_app::{Startup, Update};
use bevy_ecs::system::{Commands, ResMut, Resource};
use bevy_internal::time::common_conditions::on_timer;
use bevy_tokio_tasks::TokioTasksRuntime;
use tokio_modbus::prelude::*;

use crate::{helper::ToBytes, log, mqtt};

pub struct Plugin;
impl bevy_app::Plugin for Plugin {
    fn build(&self, app: &mut bevy_app::App) {
        use mqtt::add_on::action_message::StatusMessage;

        app.init_resource::<Manager>()
            .add_plugins((
                StatusMessage::<Manager, action::Database>::publish_condition(
                    on_timer(Duration::from_secs(60)), //
                ),
                StatusMessage::<Manager, action::MqttStatus>::publish_condition(
                    on_timer(Duration::from_secs(1)), //
                ),
            ))
            .add_systems(Startup, (Manager::start, Manager::register_home_assistant))
            .add_systems(Update, Manager::update);
    }
}

#[derive(Debug, Resource)]
pub struct Manager {
    data_sender: Option<tokio::sync::watch::Sender<SensorData>>,
    sensor_data_rx: tokio::sync::watch::Receiver<SensorData>,
    latest_data: SensorData,
}
impl Default for Manager {
    fn default() -> Self {
        let (tx, sensor_data_rx) = tokio::sync::watch::channel(SensorData::default());

        Self {
            data_sender: Some(tx),
            sensor_data_rx,
            latest_data: Default::default(),
        }
    }
}
impl Manager {
    const SENSOR_ADDR: u8 = 0x1;
    const SENSOR_DATA_ADDR: u16 = 0x0;
    const SENSOR_DATA_LEN: u16 = 3;

    pub fn get_data(&self) -> SensorData {
        self.latest_data
    }

    fn start(rt: ResMut<TokioTasksRuntime>, mut manager: ResMut<Manager>) {
        let tx = manager.data_sender.take().unwrap();

        rt.spawn_background_task(move |_| async move {
            let mut modbus_ctx = rtu::attach_slave(
                tokio_serial::SerialStream::open(
                    &tokio_serial::new("/dev/serial0", 9600), //
                )
                .unwrap(),
                Slave(Manager::SENSOR_ADDR),
            );

            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;

                let modbus_result = match modbus_ctx
                    .read_holding_registers(Manager::SENSOR_DATA_ADDR, Manager::SENSOR_DATA_LEN)
                    .await
                {
                    Ok(o) => o,
                    Err(e) => {
                        log::warn!("{e}");
                        continue;
                    }
                };

                match modbus_result {
                    Ok(data) => {
                        let new_data = SensorData {
                            ph: SensorData::ph_from_raw(data[0]),
                            ec: SensorData::ec_from_raw(data[1]),
                            temp: SensorData::temp_from_raw(data[2]),
                        };
                        log::debug!("new sensor data: {new_data:?}");
                        tx.send(new_data).map_err(|e| log::error!("{e}")).unwrap();
                    }
                    Err(e) => {
                        log::warn!("{e}");
                        continue;
                    }
                }
            }
        });
    }

    fn register_home_assistant(mut cmd: Commands) {
        use mqtt::add_on::home_assistant::Device;

        #[derive(serde::Serialize)]
        struct Config {
            name: &'static str,
            icon: &'static str,
            state_topic: &'static str,
            value_template: &'static str,
            unit_of_measurement: Option<&'static str>,
            device: Device,
        }

        cmd.spawn(mqtt::message::Message {
            topic: "homeassistant/sensor/time/water_quality_sensor/config".into(),
            payload: {
                serde_json::to_value(Config {
                    name: "Sampled Time",
                    icon: "mdi:clock",
                    state_topic: "status/triponics/water_quality_sensor/0",
                    value_template: "{{ as_datetime(value_json.timestamp) | as_local }}",
                    unit_of_measurement: None,
                    device: Device {
                        identifiers: &["water_quality_sensor"],
                        name: "Water Quality Sensor",
                    },
                })
                .unwrap()
                .to_bytes()
            },
            qos: mqtt::Qos::_1,
            retained: true,
        });

        cmd.spawn(mqtt::message::Message {
            topic: "homeassistant/sensor/ph/water_quality_sensor/config".into(),
            payload: {
                serde_json::to_value(Config {
                    name: "Water pH",
                    icon: "mdi:flask-round-bottom",
                    state_topic: "status/triponics/water_quality_sensor/0",
                    value_template: "{{ value_json.ph }}",
                    unit_of_measurement: Some("pH"),
                    device: Device {
                        identifiers: &["water_quality_sensor"],
                        name: "Water Quality Sensor",
                    },
                })
                .unwrap()
                .to_bytes()
            },
            qos: mqtt::Qos::_1,
            retained: true,
        });

        cmd.spawn(mqtt::message::Message {
            topic: "homeassistant/sensor/ec/water_quality_sensor/config".into(),
            payload: {
                serde_json::to_value(Config {
                    name: "Water EC",
                    icon: "mdi:lightning-bolt-outline",
                    state_topic: "status/triponics/water_quality_sensor/0",
                    value_template: "{{ value_json.ec }}",
                    unit_of_measurement: Some("mS/cm"),
                    device: Device {
                        identifiers: &["water_quality_sensor"],
                        name: "Water Quality Sensor",
                    },
                })
                .unwrap()
                .to_bytes()
            },
            qos: mqtt::Qos::_1,
            retained: true,
        });

        cmd.spawn(mqtt::message::Message {
            topic: "homeassistant/sensor/temperature/water_quality_sensor/config".into(),
            payload: {
                serde_json::to_value(Config {
                    name: "Water Temperature",
                    icon: "mdi:water-thermometer",
                    state_topic: "status/triponics/water_quality_sensor/0",
                    value_template: "{{ value_json.temp }}",
                    unit_of_measurement: Some("°C"),
                    device: Device {
                        identifiers: &["water_quality_sensor"],
                        name: "Water Quality Sensor",
                    },
                })
                .unwrap()
                .to_bytes()
            },
            qos: mqtt::Qos::_1,
            retained: true,
        });
    }

    fn update(mut manager: ResMut<Manager>) {
        let data = *manager.sensor_data_rx.borrow_and_update();
        manager.latest_data = data;
    }
}
impl mqtt::add_on::action_message::PublishStatus<action::Database> for Manager {
    fn get_status(&self) -> action::Database {
        let out = action::Database(self.get_data().into());
        log::info!("new water quality entry: {out:?}");
        out
    }
}
impl mqtt::add_on::action_message::PublishStatus<action::MqttStatus> for Manager {
    fn get_status(&self) -> action::MqttStatus {
        action::MqttStatus(self.get_data().into())
    }
}

#[derive(Debug, Default, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct SensorData {
    ph: f32,
    ec: f32,
    temp: f32,
}
impl SensorData {
    fn ph_from_raw(raw_data: u16) -> f32 {
        raw_data as f32 / 100.0
    }

    fn ec_from_raw(raw_data: u16) -> f32 {
        raw_data as f32 / 1000.0
    }

    fn temp_from_raw(raw_data: u16) -> f32 {
        raw_data as f32 / 10.0
    }
}

mod action {
    use crate::{constants, mqtt};

    pub const GROUP: &str = "water_quality_sensor";
    pub const QOS: mqtt::Qos = mqtt::Qos::_1;

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct State {
        timestamp: i64,
        ph: f32,
        ec: f32,
        temp: f32,
    }
    impl From<super::SensorData> for State {
        fn from(value: super::SensorData) -> Self {
            let super::SensorData { ph, ec, temp } = value;

            Self {
                timestamp: time::OffsetDateTime::now_utc().unix_timestamp(),
                ph,
                ec,
                temp,
            }
        }
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct Database(pub State);
    impl mqtt::add_on::action_message::MessageImpl for Database {
        const PREFIX: &'static str = constants::mqtt_prefix::DATABASE;
        const PROJECT: &'static str = constants::project::NAME;
        const GROUP: &'static str = GROUP;
        const DEVICE: &'static str = constants::project::DEVICE;
        const QOS: mqtt::Qos = QOS;
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct MqttStatus(pub State);
    impl mqtt::add_on::action_message::MessageImpl for MqttStatus {
        const PREFIX: &'static str = constants::mqtt_prefix::STATUS;
        const PROJECT: &'static str = constants::project::NAME;
        const GROUP: &'static str = GROUP;
        const DEVICE: &'static str = constants::project::DEVICE;
        const QOS: mqtt::Qos = QOS;
    }
}
