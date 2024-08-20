mod actor;

mod app;
use app::*;

#[actix::main]
async fn main() {
    use actix::Actor;

    println!("hello!");
    let app = App::new()
        .with_actor(actor::Mqtt::new().start())
        .with_actor(actor::Database::new().start());

    app.signal().await;
    println!("bye!");
}
