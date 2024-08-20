mod actor;

mod app;
use app::*;

#[actix::main]
async fn main() {
    App::new()
        .with_actor(actor::Mqtt::new())
        .with_actor(actor::CtrlLogic::new())
        .with_actor(actor::InputController::new())
        .with_actor(actor::OutputController::new())
        .run()
        .await;

    println!("bye!");
}
