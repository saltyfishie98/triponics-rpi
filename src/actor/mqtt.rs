use actix::ActorContext;
use actix_broker::BrokerSubscribe;

use crate::{app, log};

use super::input_controller;

#[derive(Debug, app::signal::Terminate)]
pub struct Mqtt {
    task_resrc: Option<(tokio::sync::mpsc::Receiver<serde_json::Value>,)>,
    task_tx: tokio::sync::mpsc::Sender<serde_json::Value>,
}
impl Mqtt {
    pub fn new() -> Self {
        let (task_tx, rx) = tokio::sync::mpsc::channel(10);
        Self {
            task_resrc: Some((rx,)),
            task_tx,
        }
    }

    async fn task(mut rx: tokio::sync::mpsc::Receiver<serde_json::Value>) {
        while let Some(payload) = rx.recv().await {
            log::info!("input: {payload}");
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }
    }
}
impl actix::Actor for Mqtt {
    type Context = actix::Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.subscribe_system_sync::<input_controller::broadcast::InputData>(ctx);

        let (rx,) = self.task_resrc.take().unwrap();
        actix::Arbiter::current().spawn(Self::task(rx));
    }
}
impl actix::Handler<app::signal::Stop> for Mqtt {
    type Result = app::signal::StopResult;

    fn handle(&mut self, _msg: app::signal::Stop, ctx: &mut Self::Context) -> Self::Result {
        ctx.stop();
        Ok(())
    }
}
impl<T> actix::Handler<T> for Mqtt
where
    T: serde::Serialize + actix::Message<Result = ()> + Send + 'static,
{
    type Result = ();

    fn handle(&mut self, msg: T, _ctx: &mut Self::Context) -> Self::Result {
        let tx = self.task_tx.clone();

        actix::Arbiter::current().spawn(async move {
            match serde_json::to_value(msg) {
                Ok(value) => {
                    let _ = tx.send(value).await;
                }
                Err(e) => {
                    log::warn!(
                        "failed to enqueue mqtt payload '{}', reason: {e}",
                        core::any::type_name::<T>()
                    )
                }
            }
        });
    }
}
