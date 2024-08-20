use actix::{ActorContext, AsyncContext};
use actix_broker::BrokerSubscribe;

use crate::app;

#[derive(Debug, app::signal::Terminate)]
pub struct Mqtt {
    task_handle: Option<tokio::task::JoinHandle<()>>,
}
impl Mqtt {
    pub fn new() -> Self {
        Self { task_handle: None }
    }
}
impl actix::Actor for Mqtt {
    type Context = actix::Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.subscribe_system_sync::<event::incoming_payload::Data>(ctx);

        let self_addr = ctx.address();
        let task_handle = tokio::task::spawn_local(async move {
            loop {
                let event = event::incoming_payload::Data;
                println!("emitted: {event:?}");

                if let Err(e) = self_addr.send(event).await {
                    println!("{e}");
                }
            }
        });
        self.task_handle = Some(task_handle);
    }
}
impl actix::Handler<app::signal::Stop> for Mqtt {
    type Result = app::signal::StopResult;

    fn handle(&mut self, _msg: app::signal::Stop, ctx: &mut Self::Context) -> Self::Result {
        if let Some(task_handle) = &self.task_handle {
            task_handle.abort()
        }
        ctx.stop();
        Ok(())
    }
}

pub mod event {
    use super::Mqtt;

    pub mod incoming_payload {
        use super::*;

        #[derive(Debug, actix::Message, Clone)]
        #[rtype(result = "()")]
        pub struct Data;

        impl actix::Handler<Data> for Mqtt {
            type Result = ();

            fn handle(&mut self, _msg: Data, _ctx: &mut Self::Context) -> Self::Result {
                // TODO: this is user manual ctrl (do whatever with it)
                todo!()
            }
        }
    }
}
