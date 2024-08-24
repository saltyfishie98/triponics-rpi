mod helper;
mod mqtt;

use std::time::Duration;

use bevy_app::{prelude::*, ScheduleRunnerPlugin};
use bevy_ecs::{
    event::{Event, EventReader},
    system::ResMut,
};
use bevy_internal::MinimalPlugins;
use bevy_tokio_tasks::{TokioTasksPlugin, TokioTasksRuntime};

#[allow(unused_imports)]
use tracing as log;

fn main() -> anyhow::Result<()> {
    helper::init_logging();

    App::new()
        .add_plugins((
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f32(
                1.0 / 60.0,
            ))),
            TokioTasksPlugin::default(),
        ))
        .add_plugins((mqtt::MqttPlugin {
            subscriptions: &[("data/#", mqtt::Qos::_0)],
            ..Default::default()
        },))
        .add_event::<RestartEvent>()
        .add_systems(Startup, exit_task)
        .add_systems(Update, log_mqtt_msg)
        .run();

    log::info!("bye!");

    // use paho_mqtt as mqtt;

    // use futures::StreamExt;
    // const TOPICS: &[&str] = &["data/#"];
    // const QOS: &[i32] = &[1];

    // // Create the client. Use an ID for a persistent session.
    // // A real system should try harder to use a unique ID.
    // let create_opts = mqtt::CreateOptionsBuilder::new()
    //     .server_uri("mqtt://test.mosquitto.org")
    //     .client_id("rust_async_sub_v5")
    //     .finalize();

    // // Create the client connection
    // let mut cli = mqtt::AsyncClient::new(create_opts).unwrap_or_else(|e| {
    //     println!("Error creating the client: {:?}", e);
    //     std::process::exit(1);
    // });

    // if let Err(err) = futures::executor::block_on(async {
    //     // Get message stream before connecting.
    //     let mut strm = cli.get_stream(25);

    //     // Define the set of options for the connection
    //     let lwt = mqtt::Message::new(
    //         "test/lwt",
    //         "[LWT] Async subscriber v5 lost connection",
    //         mqtt::QOS_1,
    //     );

    //     // Connect with MQTT v5 and a persistent server session (no clean start).
    //     // For a persistent v5 session, we must set the Session Expiry Interval
    //     // on the server. Here we set that requests will persist for an hour
    //     // (3600sec) if the service disconnects or restarts.
    //     let conn_opts = mqtt::ConnectOptionsBuilder::with_mqtt_version(mqtt::MQTT_VERSION_5)
    //         .clean_start(false)
    //         .properties(mqtt::properties![mqtt::PropertyCode::SessionExpiryInterval => 3600])
    //         .will_message(lwt)
    //         .finalize();

    //     // Make the connection to the broker
    //     cli.connect(conn_opts).await?;

    //     println!("Subscribing to topics: {:?}", TOPICS);
    //     let sub_opts = vec![mqtt::SubscribeOptions::with_retain_as_published(); TOPICS.len()];
    //     cli.subscribe_many_with_options(TOPICS, QOS, &sub_opts, None)
    //         .await?;

    //     // Just loop on incoming messages.
    //     println!("Waiting for messages...");

    //     // Note that we're not providing a way to cleanly shut down and
    //     // disconnect. Therefore, when you kill this app (with a ^C or
    //     // whatever) the server will get an unexpected drop and then
    //     // should emit the LWT message.

    //     while let Some(msg_opt) = strm.next().await {
    //         if let Some(msg) = msg_opt {
    //             if msg.retained() {
    //                 print!("(R) ");
    //             }
    //             println!("{}", msg);
    //         } else {
    //             // A "None" means we were disconnected. Try to reconnect...
    //             println!("Lost connection. Attempting reconnect.");
    //             while let Err(err) = cli.reconnect().await {
    //                 println!("Error reconnecting: {}", err);
    //                 // For tokio use: tokio::time::delay_for()
    //                 tokio::time::sleep(Duration::from_millis(1000)).await;
    //             }
    //         }
    //     }

    //     // Explicit return type for the async block
    //     Ok::<(), mqtt::Error>(())
    // }) {
    //     eprintln!("{}", err);
    // }

    Ok(())
}

#[derive(Event)]
struct RestartEvent;

fn exit_task(rt: ResMut<TokioTasksRuntime>) {
    rt.spawn_background_task(|mut ctx| async move {
        let _ = tokio::signal::ctrl_c().await;
        ctx.run_on_main_thread(move |ctx| {
            ctx.world.send_event(AppExit::Success);
        })
        .await;
    });
}

fn log_mqtt_msg(mut ev_reader: EventReader<mqtt::MqttMessage>) {
    while let Some(mqtt::MqttMessage(msg)) = ev_reader.read().next() {
        log::info!("mqtt msg: {}", msg);
    }
}
