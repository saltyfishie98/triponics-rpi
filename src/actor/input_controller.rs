use crate::app;

#[allow(unused_imports)]
use crate::log;

pub struct Config {
    pub update_interval: tokio::time::Duration,
}

#[derive(Debug, app::signal::Stop, app::signal::Terminate)]
pub struct InputController {
    update_interval: tokio::time::Duration,
    task_handle: Option<tokio::task::JoinHandle<()>>,
}
impl InputController {
    pub fn new(config: Config) -> Self {
        let Config { update_interval } = config;

        Self {
            update_interval,
            task_handle: None,
        }
    }
}
impl actix::Actor for InputController {
    type Context = actix::Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        let interval = self.update_interval;

        let task = tokio::task::spawn_local(async move {
            loop {
                log::info!("input!");
                tokio::time::sleep(interval).await;
            }
        });
        self.task_handle = Some(task);
    }
}
