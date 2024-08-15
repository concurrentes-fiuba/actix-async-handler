use std::time::Duration;

use actix::{Actor, ActorStreamExt, Addr, Context, Handler, Message};
use actix::fut::result;
use actix_rt::time::sleep;

use actix_async_handler::async_handler;

#[derive(Message, Clone, Copy)]
#[rtype(result = "u64")]
struct Ping(u64);

struct Ponger {
    id: i32,
}

impl Actor for Ponger {
    type Context = Context<Self>;
}

#[async_handler]
impl Handler<Ping> for Ponger {
    type Result = u64;

    async fn handle(&mut self, msg: Ping, _ctx: &mut Self::Context) -> Self::Result {
        println!("[Ponger {}] sleeping for {} secs", self.id, msg.0);
        sleep(Duration::from_secs(msg.0)).await;
        println!("[Ponger {}] woke up.", self.id);
        msg.0
    }
}

struct Pinger {
    pongers: Vec<Addr<Ponger>>
}

impl Actor for Pinger {
    type Context = Context<Self>;
}

#[async_handler]
impl Handler<Ping> for Pinger {

    type Result = u64;

    async fn handle(&mut self, msg: Ping, ctx: &mut Self::Context) -> Self::Result {

        // just looping
        for ponger in self.pongers.clone() {
            println!("pre loop");
            ponger.send(msg).await;
            println!("middle loop");
            ponger.send(Ping(msg.0 + 1)).await;
            println!("end loop");
        };

        let mut i = 0;

        // mutating a var internally and getting the final result
        i = for ponger in self.pongers.clone() {
            i += 1;
            println!("pre loop");
            ponger.send(msg).await;
            println!("middle loop");
            ponger.send(Ping(msg.0 + i)).await;
            println!("end loop");
        };

        println!("{}", i);

        42
    }

}

#[actix_rt::main]
async fn main() {

    let mut pongers = vec![];
    for id in 0..5 {
        pongers.push(Ponger { id }.start());
    };
    let pinger = Pinger { pongers }.start();

    println!("Result {}", pinger.send(Ping(1)).await.unwrap());
}