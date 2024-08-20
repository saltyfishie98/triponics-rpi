use crate::app;

#[derive(Debug, app::signal::Stop, app::signal::Terminate)]
pub struct OutputController;
impl OutputController {
    pub fn new() -> Self {
        Self
    }
}
impl actix::Actor for OutputController {
    type Context = actix::Context<Self>;
}
