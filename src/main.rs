mod actor;

mod app;
use app::*;

#[actix::main]
async fn main() {
    let app = App::new()
        .with_actor(actor::Mqtt::new())
        .with_actor(actor::Database::new());

    app.signal().await;
    println!("bye!");
}
