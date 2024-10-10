use std::{marker::PhantomData, sync::RwLock};

use bevy_app::{Plugin, Startup, Update};
use bevy_ecs::{
    event::{Event, EventReader},
    schedule::{Condition, IntoSystemConfigs, SystemConfigs},
    system::{Commands, In, IntoSystem, ResMut, Resource, RunSystemOnce, System},
    world::World,
};

#[allow(unused_imports)]
use crate::log;
use crate::{
    plugins::mqtt::{
        self,
        message::{self, MessageInfo},
        Qos,
    },
    AtomicFixedString,
};

pub trait MessageImpl
where
    Self: MessageInfo,
{
    const PREFIX: &'static str;
    const PROJECT: &'static str;
    const GROUP: &'static str;
    const DEVICE: &'static str;
    const QOS: Qos;
}
impl<M> MessageInfo for M
where
    M: MessageImpl,
{
    fn topic() -> AtomicFixedString {
        format!("{}/{}/{}/{}", M::PREFIX, M::PROJECT, M::GROUP, M::DEVICE).into()
    }

    fn qos() -> Qos {
        M::QOS
    }
}

pub trait PublishStatus<S: MessageImpl>
where
    Self: Resource + Sized + Send + Sync + 'static,
{
    // fn get_status(&self) -> S;
    fn query_state() -> impl System<In = (), Out = S>;
}

pub trait RequestHandler
where
    Self: Resource + Sized + Send + Sync + 'static,
{
    type Request: MessageImpl;
    type Response: MessageImpl;

    fn update_state(request: Self::Request, state: &mut Self) -> Option<Self::Response> {
        let _ = request;
        let _ = state;

        log::warn!("[action_msg] ignored received request -> {request:?}");

        None
    }
}

pub struct ConfigMessage<T, Cfg>
where
    T: crate::config::ConfigFile + Send + Sync + 'static,
    Cfg: MessageImpl + Send + Sync + 'static,
{
    _t: PhantomData<T>,
    _c: PhantomData<Cfg>,
}
impl<T, Cfg> ConfigMessage<T, Cfg>
where
    T: crate::config::ConfigFile<Config = Cfg> + Send + Sync + 'static,
    Cfg: MessageImpl + std::fmt::Debug + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            _t: PhantomData::<T>,
            _c: PhantomData::<Cfg>,
        }
    }

    fn setup(mut cmd: Commands) {
        cmd.spawn(
            mqtt::message::Subscriptions::new()
                .with_msg::<local::LoadCfgMsg<Cfg>>()
                .with_msg::<local::SaveCfgMsg<Cfg>>()
                .finalize(),
        );
    }

    fn on_load_request(mut cmd: Commands, mut ev: EventReader<mqtt::event::IncomingMessage>) {
        while let Some(incoming) = ev.read().next() {
            if incoming.get::<local::LoadCfgMsg<Cfg>>().is_some() {
                if let Ok(cfg) = T::load_config() {
                    log::debug!("mqtt send config: {cfg:?}");
                    cmd.spawn(cfg.make_mqtt_msg());
                }
                break;
            }
        }

        ev.clear();
    }

    fn on_save_request(mut ev: EventReader<mqtt::event::IncomingMessage>) {
        while let Some(incoming) = ev.read().next() {
            if let Some(local::SaveCfgMsg(cfg)) = incoming.get::<local::SaveCfgMsg<Cfg>>() {
                log::debug!("received save config mqtt message");
                if let Err(e) = T::save_config(cfg) {
                    log::warn!("failed to save new config, reason: {e}");
                }
                break;
            }
        }

        ev.clear();
    }
}

impl<T, Cfg> Default for ConfigMessage<T, Cfg>
where
    T: crate::config::ConfigFile<Config = Cfg> + Send + Sync + 'static,
    Cfg: MessageImpl + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}
impl<T, Cfg> Plugin for ConfigMessage<T, Cfg>
where
    T: crate::config::ConfigFile<Config = Cfg> + Send + Sync + 'static,
    Cfg: MessageImpl + Send + Sync + 'static,
{
    fn build(&self, app: &mut bevy_app::App) {
        app.add_systems(Startup, (ConfigMessage::<T, Cfg>::setup,))
            .add_systems(
                Update,
                (
                    ConfigMessage::<T, Cfg>::on_load_request,
                    ConfigMessage::<T, Cfg>::on_save_request,
                ),
            );
    }
}

pub struct StatusMessage<T, Msg = T>
where
    T: PublishStatus<Msg>,
    Msg: MessageImpl,
{
    _p: PhantomData<T>,
    _m: PhantomData<Msg>,
    system_configs: RwLock<Option<SystemConfigs>>,
}
impl<T, Msg> StatusMessage<T, Msg>
where
    T: PublishStatus<Msg>,
    Msg: MessageImpl + Send + Sync + 'static,
{
    pub fn publish_condition<M>(condition: impl Condition<M>) -> Self {
        Self {
            _p: PhantomData::<T>,
            _m: PhantomData::<Msg>,
            system_configs: RwLock::new(Some(
                IntoSystem::into_system(Self::publish_status)
                    .run_if(condition)
                    .into_configs(),
            )),
        }
    }

    fn publish_status(mut cmd: Commands) {
        cmd.add(|world: &mut World| {
            fn spawn_msg<M: MessageImpl>(msg: In<M>, mut cmd: Commands) {
                cmd.spawn(message::Message {
                    topic: M::topic(),
                    payload: msg.to_payload(),
                    qos: M::qos(),
                    retained: false,
                });
            }

            world.run_system_once(T::query_state().pipe(spawn_msg::<Msg>));
        });
    }
}
impl<T, Msg> Plugin for StatusMessage<T, Msg>
where
    T: PublishStatus<Msg>,
    Msg: MessageImpl + Send + Sync + 'static,
{
    fn build(&self, app: &mut bevy_app::App) {
        let system = self.system_configs.write().unwrap().take().unwrap();

        app.add_event::<local::StatusUpdate<Msg>>()
            .add_systems(Update, system);
    }
}

pub struct RequestMessage<T>
where
    T: RequestHandler,
{
    _p: PhantomData<T>,
}
impl<T> RequestMessage<T>
where
    T: RequestHandler,
{
    pub fn new() -> Self {
        Self {
            _p: PhantomData::<T>,
        }
    }

    fn subscribe_request(mut cmd: Commands) {
        cmd.spawn(
            mqtt::message::Subscriptions::new()
                .with_msg::<T::Request>()
                .finalize(),
        );
    }

    fn state_update(
        mut cmd: Commands,
        mut ev_reader: EventReader<mqtt::event::IncomingMessage>,
        mut maybe_state: Option<ResMut<T>>,
    ) {
        while let Some(incoming_msg) = ev_reader.read().next() {
            if let Some(request) = incoming_msg.get::<T::Request>() {
                if let Some(ref mut state) = maybe_state {
                    if let Some(res) = T::update_state(request, state) {
                        cmd.spawn(res.make_mqtt_msg());
                    }
                }
            }
        }
    }
}
impl<T> Plugin for RequestMessage<T>
where
    T: RequestHandler,
{
    fn build(&self, app: &mut bevy_app::App) {
        app.add_systems(Startup, Self::subscribe_request)
            .add_systems(Update, Self::state_update);
    }
}

mod local {
    use super::*;

    #[derive(Debug, Event)]
    pub struct StatusUpdate<Msg: MessageImpl>(Msg);

    #[derive(Debug)]
    pub struct SaveCfgMsg<Cfg>(pub Cfg)
    where
        Cfg: MessageImpl + Send + Sync + 'static;
    impl<Cfg> serde::Serialize for SaveCfgMsg<Cfg>
    where
        Cfg: MessageImpl + Send + Sync + 'static,
    {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            self.0.serialize(serializer)
        }
    }
    impl<'de, Cfg> serde::Deserialize<'de> for SaveCfgMsg<Cfg>
    where
        Cfg: MessageImpl + Send + Sync + 'static,
    {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            Ok(Self(Cfg::deserialize(deserializer)?))
        }
    }
    impl<Cfg> MessageInfo for SaveCfgMsg<Cfg>
    where
        Cfg: MessageImpl + Send + Sync + 'static,
    {
        fn topic() -> AtomicFixedString {
            format!("save_{}", Cfg::topic()).into()
        }

        fn qos() -> Qos {
            Cfg::QOS
        }
    }

    #[derive(Debug)]
    pub struct LoadCfgMsg<Cfg>(pub Option<Cfg>)
    where
        Cfg: MessageImpl + Send + Sync + 'static;
    impl<Cfg> serde::Serialize for LoadCfgMsg<Cfg>
    where
        Cfg: MessageImpl + Send + Sync + 'static,
    {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            self.0.serialize(serializer)
        }
    }
    impl<'de, Cfg> serde::Deserialize<'de> for LoadCfgMsg<Cfg>
    where
        Cfg: MessageImpl + Send + Sync + 'static,
    {
        fn deserialize<D>(_: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            Ok(Self(None))
        }
    }
    impl<Cfg> MessageInfo for LoadCfgMsg<Cfg>
    where
        Cfg: MessageImpl + Send + Sync + 'static,
    {
        fn topic() -> AtomicFixedString {
            format!("load_{}", Cfg::topic()).into()
        }

        fn qos() -> Qos {
            Cfg::QOS
        }
    }
}
