use bevy_ecs::{
    schedule::SystemConfigs,
    system::{Commands, Res, Resource},
};

use super::{component, MqttMessage, Qos};
use crate::log;

pub trait ActionPrefix {
    const STATUS_PREFIX: &'static str = "data";
    const REQUEST_PREFIX: &'static str = "request";
    const RESPONSE_PREFIX: &'static str = "response";
}
impl<T: ActionMessage> ActionPrefix for T {}

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

pub trait ActionMessageHandler
where
    Self: MqttMessage,
{
    type State;
    type Request: ActionMessage;
    type Status: ActionMessage;
    type Response: ActionMessage;

    fn on_request() -> Option<SystemConfigs> {
        None
    }

    fn status_publish() -> Option<SystemConfigs> {
        None
    }

    fn publish_status(mut cmd: Commands, maybe_this: Option<Res<Self>>)
    where
        Self: Resource + ActionMessage<Type = action_type::Status>,
    {
        if let Some(this) = maybe_this {
            log::trace!("publishing {this:?}");
            cmd.spawn(this.status_msg());
        }
    }

    fn status_msg(&self) -> component::PublishMsg
    where
        Self: ActionMessage<Type = action_type::Status>,
    {
        component::PublishMsg {
            topic: Self::topic(),
            payload: self.to_payload(),
            qos: Self::qos(),
        }
    }
}

pub trait ActionMessage
where
    Self: MqttMessage + ActionPrefix,
{
    type Type: ActionType;
    const PROJECT: &'static str;
    const GROUP: &'static str;
    const DEVICE: &'static str;
    const QOS: Qos;
}
impl<T: ActionMessage> MqttMessage for T {
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

    fn qos() -> super::Qos {
        T::QOS
    }
}
