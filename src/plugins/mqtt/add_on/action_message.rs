use std::{marker::PhantomData, sync::RwLock};

use bevy_app::{Plugin, Startup, Update};
use bevy_ecs::{
    event::{Event, EventReader},
    schedule::{Condition, IntoSystemConfigs, SystemConfigs},
    system::{Commands, IntoSystem, Res, ResMut, Resource},
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

pub trait PublishStatus
where
    Self: Resource + Sized + Send + Sync + 'static,
{
    type Status: MessageImpl;
    fn get_status(&self) -> Self::Status;
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

pub struct StatusMessage<T>
where
    T: PublishStatus,
{
    _p: PhantomData<T>,
    system_configs: RwLock<Option<SystemConfigs>>,
}
impl<T> StatusMessage<T>
where
    T: PublishStatus,
    T::Status: Send + Sync + 'static,
{
    pub fn publish_condition<M>(condition: impl Condition<M>) -> Self {
        Self {
            _p: PhantomData::<T>,
            system_configs: RwLock::new(Some(
                IntoSystem::into_system(Self::publish_status)
                    .run_if(condition)
                    .into_configs(),
            )),
        }
    }

    fn publish_status(mut cmd: Commands, maybe_state: Option<Res<T>>) {
        if let Some(state) = maybe_state {
            cmd.spawn(message::Message {
                topic: T::Status::topic(),
                payload: state.get_status().to_payload(),
                qos: T::Status::qos(),
            });
        }
    }
}
impl<T> Plugin for StatusMessage<T>
where
    T: PublishStatus,
    T::Status: Send + Sync + 'static,
{
    fn build(&self, app: &mut bevy_app::App) {
        let system = self.system_configs.write().unwrap().take().unwrap();

        app.add_event::<local::StatusUpdate<T>>()
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
                        cmd.spawn(res.make());
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
    pub struct StatusUpdate<T: PublishStatus>(T::Status);
}
