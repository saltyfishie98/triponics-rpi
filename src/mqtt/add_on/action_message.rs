use std::marker::PhantomData;

use bevy_app::{Plugin, Startup, Update};
use bevy_ecs::{
    event::{Event, EventReader},
    schedule::IntoSystemConfigs,
    system::{Commands, Res, ResMut, Resource},
};
use bevy_internal::time::common_conditions::on_timer;

use super::super::Qos;
use crate::mqtt::{
    self,
    message::{self, MessageInfo},
};

#[allow(unused_imports)]
use crate::log;

pub trait MessageImpl
where
    Self: MessageInfo,
{
    type Type: ActionType; // needed for MqttMessage blanket impl
    const PROJECT: &'static str;
    const GROUP: &'static str;
    const DEVICE: &'static str;
    const QOS: Qos;
}
impl<T: MessageImpl> MessageInfo for T {
    fn topic() -> crate::helper::AtomicFixedString {
        format!(
            "{}/{}/{}/{}",
            T::Type::PREFIX,
            T::PROJECT,
            T::GROUP,
            T::DEVICE
        )
        .into()
    }

    fn qos() -> Qos {
        T::QOS
    }
}

pub mod action_type {
    pub struct Status;
    impl super::ActionType for Status {
        const PREFIX: &str = "data";
    }

    pub struct Request;
    impl super::ActionType for Request {
        const PREFIX: &str = "request";
    }

    pub struct Response;
    impl super::ActionType for Response {
        const PREFIX: &str = "response";
    }
}

pub trait PublishStatus
where
    Self: Resource + Default + Sized + Send + Sync + 'static,
{
    type Status: MessageImpl<Type = action_type::Status>;
    fn get_status(&self) -> Self::Status;
}

pub trait RequestHandler
where
    Self: Resource + Default + Sized + Send + Sync + 'static,
{
    type Request: MessageImpl<Type = action_type::Request>;
    type Response: MessageImpl<Type = action_type::Response>;

    fn update_state(request: Self::Request, state: &mut Self) -> Option<Self::Response> {
        let _ = request;
        let _ = state;

        log::debug!("ignored received request -> {request:?}");

        None
    }
}

pub struct StatusMessage<T>
where
    T: PublishStatus,
{
    _p: PhantomData<T>,
    status_publish_duration: Option<std::time::Duration>,
}
impl<T> StatusMessage<T>
where
    T: PublishStatus,
    T::Status: Send + Sync + 'static,
{
    pub fn publish_interval(status_publish_duration: Option<std::time::Duration>) -> Self {
        Self {
            _p: PhantomData::<T>,
            status_publish_duration,
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
        app.init_resource::<T>()
            .add_event::<local::StatusUpdate<T>>();

        if let Some(duration) = self.status_publish_duration {
            app.add_systems(Update, Self::publish_status.run_if(on_timer(duration)));
        }
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
        mut state: ResMut<T>,
    ) {
        while let Some(incoming_msg) = ev_reader.read().next() {
            if let Some(request) = incoming_msg.get::<T::Request>() {
                if let Some(res) = T::update_state(request, &mut state) {
                    cmd.spawn(res.make());
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
        app.init_resource::<T>()
            .add_systems(Startup, Self::subscribe_request)
            .add_systems(Update, Self::state_update);
    }
}

mod local {
    use super::*;

    #[derive(Debug, Event)]
    pub struct StatusUpdate<T: PublishStatus>(T::Status);
}

pub trait ActionType {
    const PREFIX: &str;
}
