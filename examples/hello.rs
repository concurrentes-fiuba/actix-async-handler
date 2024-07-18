use actix::prelude::*;
use actix::clock::sleep;
use std::time::Duration;
use actix_async_handler::async_handler;

/// This is the example from the [actix::AtomicResponse] doc translated

#[derive(Message)]
#[rtype(usize)]
struct Msg;

struct MyActor(usize);

impl Actor for MyActor {
    type Context = Context<Self>;
}

#[async_handler]
impl Handler<Msg> for MyActor {
    type Result = usize;

    async fn handle(&mut self, _msg: Msg, _ctx: &mut Context<Self>) -> Self::Result {
        self.0 = 30;
        sleep(Duration::from_secs(3)).await;
        self.0 -= 1;
        self.0
    }
}


#[actix_rt::main]
async fn main() {

    let actor = MyActor(0).start();
    println!("Result {}", actor.send(Msg).await.unwrap());
}