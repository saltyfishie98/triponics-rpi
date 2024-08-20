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
            let mut data = 0;
            loop {
                actix_broker::Broker::<actix_broker::SystemBroker>::issue_async(
                    broadcast::InputData { data },
                );
                log::info!("sent: {data}");
                data += 1;
                tokio::time::sleep(interval).await;
            }
        });
        self.task_handle = Some(task);
    }
}

pub mod broadcast {
    #[derive(Debug, actix::Message, Clone, serde::Serialize)]
    #[rtype(result = "()")]
    pub struct InputData {
        pub data: u32,
    }
}
