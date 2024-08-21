pub mod alarm;
pub mod signal;

use std::{any::TypeId, collections::HashMap, sync::OnceLock};

#[allow(unused_imports)]
use crate::log;

type AddrRegistry = HashMap<TypeId, Box<dyn AddrProxy + Send + Sync>>;

static ADDR_REGISTRY: OnceLock<HashMap<TypeId, Box<dyn AddrProxy + Send + Sync>>> = OnceLock::new();

pub struct AppBuilder {
    proto_addr_registry: AddrRegistry,
}
impl AppBuilder {
    pub fn with_actor<T>(self, actor: T) -> Result<Self, anyhow::Error>
    where
        T: actix::Actor<Context = actix::Context<T>>
            + actix::Handler<signal::Stop>
            + actix::Handler<signal::Terminate>,
    {
        let Self {
            mut proto_addr_registry,
        } = self;

        let id = TypeId::of::<T>();

        if let std::collections::hash_map::Entry::Vacant(e) = proto_addr_registry.entry(id) {
            e.insert(Box::new(actor.start()));
            log::info!("actor '{}' running!", std::any::type_name::<T>());

            Ok(Self {
                proto_addr_registry,
            })
        } else {
            Err(anyhow::anyhow!("only can have 1 instance!"))
        }
    }

    pub fn build(self) -> App {
        App {
            addr_registry: ADDR_REGISTRY.get_or_init(|| self.proto_addr_registry),
        }
    }
}

pub struct App {
    addr_registry: &'static AddrRegistry,
}
impl App {
    pub fn builder() -> AppBuilder {
        AppBuilder {
            proto_addr_registry: HashMap::new(),
        }
    }

    pub fn addr_of<T>() -> Option<actix::Addr<T>>
    where
        T: actix::Actor<Context = actix::Context<T>>
            + actix::Handler<signal::Stop>
            + actix::Handler<signal::Terminate>,
    {
        let reg = ADDR_REGISTRY.get()?;
        reg.get(&TypeId::of::<T>())?
            .as_any()
            .downcast_ref::<actix::Addr<T>>()
            .cloned()
    }

    pub async fn run(self) {
        tokio::signal::ctrl_c().await.unwrap();
        // actix::Arbiter::current().stop();
        futures::future::join_all(
            self.addr_registry
                .values()
                .map(|addr| addr.clean())
                .collect::<Vec<_>>(),
        )
        .await;
    }
}

pub trait AddrProxy {
    fn clean(&self) -> futures::future::BoxFuture<Result<(), actix::MailboxError>>;
    fn as_any(&self) -> &dyn std::any::Any;
}
impl<T> AddrProxy for actix::Addr<T>
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

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
