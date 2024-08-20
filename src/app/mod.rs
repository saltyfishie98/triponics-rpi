pub mod alarm;
pub mod signal;

pub struct App {
    actor_addr: Vec<Box<dyn ActorProxy>>,
}
impl App {
    pub fn new() -> Self {
        Self {
            actor_addr: Vec::new(),
        }
    }

    pub fn with_actor(self, addr_proxy: impl ActorProxy + 'static) -> Self {
        let Self { mut actor_addr } = self;

        actor_addr.push(Box::new(addr_proxy));
        Self { actor_addr }
    }

    pub async fn signal(self) {
        tokio::signal::ctrl_c().await.unwrap();
        for addr in self.actor_addr.into_iter() {
            addr.clean().await.unwrap();
        }
    }
}

pub trait ActorProxy {
    fn clean(&self) -> futures::future::BoxFuture<Result<(), actix::MailboxError>>;
}
impl<T> ActorProxy for actix::Addr<T>
where
    T: actix::Actor<Context = actix::Context<T>>
        + actix::Handler<signal::StopSignal>
        + actix::Handler<signal::TerminateSignal>,
{
    fn clean(&self) -> futures::future::BoxFuture<Result<(), actix::MailboxError>> {
        Box::pin(async {
            if self.send(signal::StopSignal).await.is_err() {
                return self.send(signal::TerminateSignal).await;
            }
            Ok(())
        })
    }
}
