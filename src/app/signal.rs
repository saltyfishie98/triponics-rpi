pub use macros::AppStopSignal as Stop;
pub use macros::AppTerminateSignal as Terminate;

#[derive(Debug, actix::Message)]
#[rtype(result = "StopResult")]
pub struct Stop;

#[derive(Debug, actix::Message)]
#[rtype(result = "()")]
pub struct Terminate;

#[derive(Debug, thiserror::Error)]
#[error("actore stop error!")]
pub struct StopError;

pub type StopResult = Result<(), StopError>;
