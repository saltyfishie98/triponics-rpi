pub mod alarm;
pub mod signal;

#[allow(unused_imports)]
use crate::log;

pub struct App {
    actor_addr_vec: Vec<Box<dyn ActorProxy>>,
}
impl App {
    pub fn new() -> Self {
        Self {
            actor_addr_vec: Vec::new(),
        }
    }

    pub fn with_actor<T>(self, actor: T) -> Self
    where
        T: actix::Actor<Context = actix::Context<T>>
            + actix::Handler<signal::Stop>
            + actix::Handler<signal::Terminate>
            + std::fmt::Debug,
    {
        let Self { mut actor_addr_vec } = self;
        actor_addr_vec.push(Box::new(actor.start()));
        Self { actor_addr_vec }
    }

    pub async fn run(self) {
        tokio::signal::ctrl_c().await.unwrap();
        futures::future::join_all(
            self.actor_addr_vec
                .iter()
                .map(|addr| addr.clean())
                .collect::<Vec<_>>(),
        )
        .await;
    }
}

pub trait ActorProxy {
    fn clean(&self) -> futures::future::BoxFuture<Result<(), actix::MailboxError>>;
}
impl<T> ActorProxy for actix::Addr<T>
where
    T: actix::Actor<Context = actix::Context<T>>
        + actix::Handler<signal::Stop>
        + actix::Handler<signal::Terminate>,
{
    fn clean(&self) -> futures::future::BoxFuture<Result<(), actix::MailboxError>> {
        Box::pin(async {
            if !self.connected() {
                return Ok(());
            }

            if self.send(signal::Stop).await.is_err() {
                self.send(signal::Terminate).await.unwrap();
            }

            log::info!("cleaned actor '{}'", core::any::type_name::<T>());
            Ok(())
        })
    }
}
