use actix::{ActorContext, AsyncContext};
use actix_broker::BrokerSubscribe;

use crate::app;
#[allow(unused_imports)]
use crate::log;

use super::input_controller;

#[derive(app::signal::Terminate)]
pub struct Mqtt {
    mqtt_client: paho_mqtt::AsyncClient,
    task_handle: Option<tokio::task::JoinHandle<()>>,
}
impl Mqtt {
    pub async fn new() -> Self {
        let mqtt_client = tokio::task::spawn_local(async {
            let client = paho_mqtt::AsyncClient::new("test.mosquitto.org").unwrap_or_else(|err| {
                println!("Error creating the client: {}", err);
                std::process::exit(1);
            });

            client.connect(None);

            while !client.is_connected() {
                log::info!("waiting for mqtt client to connect to broker...");
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }

            client
        })
        .await
        .unwrap();

        Self {
            task_handle: None,
            mqtt_client,
        }
    }

    async fn task(_self_addr: actix::Addr<Self>) {
        loop {
            tokio::task::yield_now().await;
        }
    }
}
impl actix::Actor for Mqtt {
    type Context = actix::Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.subscribe_system_sync::<input_controller::broadcast::InputData>(ctx);
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
impl<T> actix::Handler<T> for Mqtt
where
    T: serde::Serialize + actix::Message<Result = ()> + 'static,
{
    type Result = ();

    fn handle(&mut self, msg: T, _ctx: &mut Self::Context) -> Self::Result {
        let client = self.mqtt_client.clone();
        tokio::task::spawn_local(async move {
            let mut bytes: Vec<u8> = Vec::new();
            serde_json::to_writer(&mut bytes, &msg).unwrap();

            if let Err(e) = client
                .publish(paho_mqtt::Message::new(
                    "data/test/test",
                    bytes,
                    paho_mqtt::QOS_1,
                ))
                .await
            {
                log::warn!("mqtt publish error, reason: {e}");
            } else {
                log::info!("new mqtt published!");
            }
        });
    }
}
