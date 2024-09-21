use std::marker::PhantomData;

use bevy_app::{Plugin, Startup, Update};
use bevy_ecs::{
    event::{Event, EventReader},
    schedule::IntoSystemConfigs,
    system::{Commands, Res, ResMut, Resource},
};
use bevy_internal::time::common_conditions::on_timer;

use super::super::{component, MqttMessage, Qos};
use crate::mqtt;

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
            mqtt::Subscriptions::new()
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
                    cmd.spawn(res.into_message());
                }
            }
        }
    }

    fn publish_status(mut cmd: Commands, maybe_state: Option<Res<T>>) {
        if let Some(state) = maybe_state {
            cmd.spawn(component::PublishMsg {
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
        app.add_event::<StatusUpdate<T>>()
            .add_systems(Startup, Self::subscribe_request)
            .add_systems(Update, Self::state_update);

        if let Some(duration) = self.status_publish_duration {
            app.add_systems(Update, Self::publish_status.run_if(on_timer(duration)));
        }
    }
}

pub trait State
where
    Self: Resource + Sized + Send + Sync + 'static,
{
    type Request: Impl<Type = action_type::Request>;
    type Status: Impl<Type = action_type::Status>;
    type Response: Impl<Type = action_type::Response>;

    fn get_status(&self) -> Self::Status;

    fn update_state(request: Self::Request, state: &mut Self) -> Option<Self::Response> {
        let _ = request;
        let _ = state;

        log::debug!("ignored received request -> {request:?}");

        None
    }
}

pub trait Impl
where
    Self: MqttMessage + ActionPrefix,
{
    type Type: ActionType; // needed for MqttMessage blanket impl
    const PROJECT: &'static str;
    const GROUP: &'static str;
    const DEVICE: &'static str;
    const QOS: Qos;
}
impl<T: Impl> MqttMessage for T {
    fn topic() -> crate::helper::AtomicFixedString {
        format!(
            "{}/{}/{}/{}",
            T::Type::prefix::<T>(),
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

pub trait ActionPrefix {
    const STATUS_PREFIX: &'static str = "data";
    const REQUEST_PREFIX: &'static str = "request";
    const RESPONSE_PREFIX: &'static str = "response";
}
impl<T: Impl> ActionPrefix for T {}

pub trait ActionType {
    fn prefix<T: ActionPrefix>() -> &'static str;
}

pub mod action_type {
    pub struct Status;
    impl super::ActionType for Status {
        fn prefix<T: super::ActionPrefix>() -> &'static str {
            T::STATUS_PREFIX
        }
    }

    pub struct Request;
    impl super::ActionType for Request {
        fn prefix<T: super::ActionPrefix>() -> &'static str {
            T::REQUEST_PREFIX
        }
    }

    pub struct Response;
    impl super::ActionType for Response {
        fn prefix<T: super::ActionPrefix>() -> &'static str {
            T::RESPONSE_PREFIX
        }
    }
}
