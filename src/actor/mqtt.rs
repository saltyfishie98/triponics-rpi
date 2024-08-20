use actix::{ActorContext, AsyncContext};
use actix_broker::BrokerSubscribe;

use crate::{app, log};

#[derive(Debug, app::signal::Terminate)]
pub struct Mqtt {
    task_handle: Option<tokio::task::JoinHandle<()>>,
}
impl Mqtt {
    pub fn new() -> Self {
        Self { task_handle: None }
    }

    async fn task(self_addr: actix::Addr<Self>) {
        while self_addr.connected() {
            let event = event::incoming_payload::Data;
            log::trace!("emitted: {event:?}");

            if self_addr.send(event).await.is_err() {
                break;
            }
        }

        if let Ok(Err(e)) = self_addr.send(app::signal::Stop).await {
            log::warn!("{e:#}")
        }

        log::error!("actor '{}' crashed!", core::any::type_name::<Self>())
    }
}
impl actix::Actor for Mqtt {
    type Context = actix::Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.subscribe_system_sync::<event::incoming_payload::Data>(ctx);

        self.task_handle = Some(tokio::task::spawn_local(Self::task(ctx.address())));
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
