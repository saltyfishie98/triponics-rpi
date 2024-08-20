use crate::app;

#[derive(Debug, app::signal::Stop, app::signal::Terminate)]
pub struct CtrlLogic;
impl actix::Actor for CtrlLogic {
    type Context = actix::Context<Self>;
}
impl CtrlLogic {
    pub fn new() -> Self {
        Self
    }
}
