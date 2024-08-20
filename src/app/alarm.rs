pub struct Alarm;
impl actix::Actor for Alarm {
    type Context = actix::Context<Self>;
}
impl<T, E: std::error::Error> actix::Handler<event::Alarm<T, E>> for Alarm {
    type Result = ();

    fn handle(&mut self, _msg: event::Alarm<T, E>, _ctx: &mut Self::Context) -> Self::Result {
        todo!()
    }
}

pub mod event {
    #[derive(Debug, actix::Message)]
    #[rtype(result = "()")]
    pub struct Alarm<T, E: std::error::Error>(Result<T, E>);
}
