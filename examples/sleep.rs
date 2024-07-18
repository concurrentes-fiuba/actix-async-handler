use std::time::Duration;

use actix::{Actor, ActorContext, Addr, Context, Handler, Message};
use actix::clock::sleep;

use actix_async_handler::async_handler;

#[derive(Message)]
#[rtype(result = "u64")]
struct Ping(u64);

struct Ponger {}

impl Actor for Ponger {
    type Context = Context<Self>;
}

#[async_handler]
impl Handler<Ping> for Ponger {
    type Result = u64;

    async fn handle(&mut self, msg: Ping, _ctx: &mut Self::Context) -> Self::Result {
        println!("[Ponger] sleeping for {} secs", msg.0);
        sleep(Duration::from_secs(msg.0)).await;
        println!("[Ponger] woke up.");
        msg.0
    }
}

struct Pinger {
    count: u64,
    ponger: Addr<Ponger>
}

impl Actor for Pinger {
    type Context = Context<Self>;
}

#[async_handler]
impl Handler<Ping> for Pinger {

    type Result = u64;

    async fn handle(&mut self, msg: Ping, ctx: &mut Self::Context) -> Self::Result {
        println!("[Pinger] sleeping for {} secs", msg.0);
        sleep(Duration::from_secs(msg.0)).await;
        println!("[Pinger] woke up");
        println!("[Pinger] alive {}", ctx.state().alive());
        // let result = self.ponger.send(msg).await;
        // self.count += result.unwrap();
        self.count += 1;
        self.count
    }

}

#[actix_rt::main]
async fn main() {

    let ponger = Ponger {}.start();
    let pinger = Pinger { count: 0, ponger }.start();

    println!("Count {}", pinger.send(Ping(2)).await.unwrap());
}