use actix::{ActorContext, AsyncContext};
use actix_broker::BrokerSubscribe;

use crate::app;

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
        self.subscribe_system_sync::<event::Payload>(ctx);

        let self_addr = ctx.address();
        let task_handle = tokio::task::spawn_local(async move {
            let mut interval = 0;
            loop {
                let event = event::Payload { interval };
                println!("emitted: {event:?}");

                if let Err(e) = self_addr.send(event).await {
                    println!("{e}");
                }

                interval += 1;
            }
        });
        self.task_handle = Some(task_handle);
    }
}
impl actix::Handler<app::signal::StopSignal> for Mqtt {
    type Result = app::signal::StopResult;

    fn handle(&mut self, _msg: app::signal::StopSignal, ctx: &mut Self::Context) -> Self::Result {
        if let Some(task_handle) = &self.task_handle {
            task_handle.abort()
        }
        ctx.stop();
        Ok(())
    }
}
impl actix::Handler<app::signal::TerminateSignal> for Mqtt {
    type Result = ();

    fn handle(
        &mut self,
        _msg: app::signal::TerminateSignal,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        ctx.terminate();
    }
}

pub mod event {
    use actix_broker::BrokerIssue;

    use crate::actor;

    use super::Mqtt;

    #[derive(Debug, actix::Message, Clone)]
    #[rtype(result = "()")]
    pub struct Payload {
        pub interval: u32,
    }

    impl actix::Handler<Payload> for Mqtt {
        type Result = ();

        fn handle(
            &mut self,
            Payload { interval }: Payload,
            _ctx: &mut Self::Context,
        ) -> Self::Result {
            self.issue_system_async(actor::database::event::Insert { interval })
        }
    }
}
