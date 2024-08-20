use crate::app;

#[derive(Debug, app::signal::Stop, app::signal::Terminate)]
pub struct InputController;
impl InputController {
    pub fn new() -> Self {
        Self
    }
}
impl actix::Actor for InputController {
    type Context = actix::Context<Self>;
}
