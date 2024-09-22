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

#[derive(Debug, Event)]
pub struct StatusUpdate<T: State>(T::Status);

pub struct ActionMessage<T>
where
    T: State,
{
    _p: PhantomData<T>,
    status_publish_duration: Option<std::time::Duration>,
}
impl<T> ActionMessage<T>
where
    T: State,
    T::Status: Send + Sync + 'static,
{
    pub fn new(status_publish_duration: Option<std::time::Duration>) -> Self {
        Self {
            _p: PhantomData::<T>,
            status_publish_duration,
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
impl<T> Plugin for ActionMessage<T>
where
    T: State,
    T::Status: Send + Sync + 'static,
{
    fn build(&self, app: &mut bevy_app::App) {
        app.init_resource::<T>()
            .add_event::<StatusUpdate<T>>()
            .add_systems(Startup, Self::subscribe_request)
            .add_systems(Update, Self::state_update);

        if let Some(duration) = self.status_publish_duration {
            app.add_systems(Update, Self::publish_status.run_if(on_timer(duration)));
        }
    }
}

pub trait State
where
    Self: Resource + Default + Sized + Send + Sync + 'static,
{
    type Request: MessageImpl<Type = action_type::Request>;
    type Status: MessageImpl<Type = action_type::Status>;
    type Response: MessageImpl<Type = action_type::Response>;

    fn get_status(&self) -> Self::Status;

    fn update_state(request: Self::Request, state: &mut Self) -> Option<Self::Response> {
        let _ = request;
        let _ = state;

        log::debug!("ignored received request -> {request:?}");

        None
    }
}

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

pub trait ActionType {
    const PREFIX: &str;
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
