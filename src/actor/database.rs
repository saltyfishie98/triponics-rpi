use actix::ActorContext;
use actix_broker::BrokerSubscribe;

use crate::app;

pub struct Database {
    task_handles: Option<Vec<tokio::task::JoinHandle<()>>>,
}
impl Database {
    pub fn new() -> Self {
        Self {
            task_handles: Some(Vec::new()),
        }
    }
}
impl actix::Actor for Database {
    type Context = actix::Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.subscribe_system_sync::<event::Insert>(ctx)
    }
}
impl actix::Handler<app::signal::StopSignal> for Database {
    type Result = actix::Response<app::signal::StopResult>;

    fn handle(&mut self, _msg: app::signal::StopSignal, ctx: &mut Self::Context) -> Self::Result {
        ctx.stop();

        let task_handles = self.task_handles.take().unwrap();
        actix::Response::fut(async {
            let handle_cnt = task_handles.len();
            futures::future::join_all(task_handles).await;
            println!("\ntask count: {handle_cnt}");
            Ok(())
        })
    }
}
impl actix::Handler<app::signal::TerminateSignal> for Database {
    type Result = actix::Response<()>;

    fn handle(
        &mut self,
        _msg: app::signal::TerminateSignal,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        ctx.terminate();
        actix::Response::reply(())
    }
}

pub mod event {
    use super::Database;

    #[derive(Debug, actix::Message, Clone)]
    #[rtype(result = "()")]
    pub struct Insert {
        #[allow(dead_code)]
        pub interval: u32,
    }

    impl actix::Handler<Insert> for Database {
        type Result = ();

        fn handle(&mut self, msg: Insert, _ctx: &mut Self::Context) -> Self::Result {
            let handle = tokio::task::spawn_local(async move {
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                println!("received: {:?}", msg);
            });
            if let Some(task_handles) = &mut self.task_handles {
                task_handles.push(handle)
            }
        }
    }
}
