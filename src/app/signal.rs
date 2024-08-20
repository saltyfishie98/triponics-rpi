#[derive(Debug, thiserror::Error)]
#[error("actore stop error!")]
pub struct StopError;

pub type StopResult = Result<(), StopError>;

#[derive(Debug, actix::Message)]
#[rtype(result = "StopResult")]
pub struct StopSignal;

#[derive(Debug, actix::Message)]
#[rtype(result = "()")]
pub struct TerminateSignal;
