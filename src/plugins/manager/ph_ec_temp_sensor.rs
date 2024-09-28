use std::time::Duration;

use bevy_app::{Startup, Update};
use bevy_ecs::system::{ResMut, Resource};
use bevy_internal::time::common_conditions::on_timer;
use bevy_tokio_tasks::TokioTasksRuntime;
use tokio_modbus::prelude::*;

use crate::{log, mqtt};

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

    fn update(mut manager: ResMut<Manager>) {
        let data = *manager.sensor_data_rx.borrow_and_update();
        manager.latest_data = data;
    }
}
impl mqtt::add_on::action_message::PublishStatus for Manager {
    type Status = action::Status;

    fn get_status(&self) -> Self::Status {
        self.get_data().into()
    }
}

pub struct Plugin;
impl bevy_app::Plugin for Plugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_resource::<Manager>()
            .add_plugins((
                mqtt::add_on::action_message::StatusMessage::<Manager>::publish_condition(
                    on_timer(Duration::from_secs(1)),
                ),
            ))
            .add_systems(Startup, Manager::start)
            .add_systems(Update, Manager::update);
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

    pub const GROUP: &str = "ph_ec_temp_sensor";
    pub const QOS: mqtt::Qos = mqtt::Qos::_1;

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct Status {
        timestamp: i64,
        ph: f32,
        ec: f32,
        temp: f32,
    }
    impl mqtt::add_on::action_message::MessageImpl for Status {
        const PREFIX: &'static str = constants::mqtt_prefix::STATUS;
        const PROJECT: &'static str = constants::project::NAME;
        const GROUP: &'static str = GROUP;
        const DEVICE: &'static str = constants::project::DEVICE;
        const QOS: mqtt::Qos = QOS;
    }
    impl From<super::SensorData> for Status {
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
}
