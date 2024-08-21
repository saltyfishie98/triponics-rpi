use actix_broker::BrokerSubscribe;

use crate::app;
#[allow(unused_imports)]
use crate::log;

use super::input_controller;

#[derive(Debug, app::signal::Stop, app::signal::Terminate)]
pub struct CtrlLogic;
impl actix::Actor for CtrlLogic {
    type Context = actix::Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.subscribe_system_sync::<input_controller::broadcast::InputData>(ctx)
    }
}
impl CtrlLogic {
    pub fn new() -> Self {
        Self
    }
}
impl actix::Handler<input_controller::broadcast::InputData> for CtrlLogic {
    type Result = ();

    fn handle(
        &mut self,
        msg: input_controller::broadcast::InputData,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        log::info!("CtrlLogic: data -> {:?}", msg);
    }
}
