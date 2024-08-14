use std::time::Duration;

use actix::{Actor, Addr, Context, Handler, MailboxError, Message};
use actix::clock::sleep;
use actix::fut::result;

use actix_async_handler::async_handler;

#[derive(Message, Clone, Copy)]
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
    ponger: Addr<Ponger>
}

impl Actor for Pinger {
    type Context = Context<Self>;
}

#[async_handler]
impl Handler<Ping> for Pinger {

    type Result = u64;

    async fn handle(&mut self, msg: Ping, ctx: &mut Self::Context) -> Self::Result {

        if msg.0 > 1 {
            println!("test code before await");
            self.ponger.send(msg).await
        }

        if msg.0 > 2 {
            self.ponger.send(msg).await
        } else if msg.0 > 3 {
            // This seems useless, but it is here to test the else having an await
            self.ponger.send(msg).await
        }

        let result: Result<u64, MailboxError> = if msg.0 > 0 {
            let part = self.ponger.send(msg).await;
            self.ponger.send(Ping(part.unwrap())).await
        } else {
            Ok(42u64)
        };

        let mut final_result: Result<u64, MailboxError> = Err(MailboxError::Closed);
        final_result = if result.unwrap() > 1 {
            self.ponger.send(Ping(result.unwrap())).await
        } else {
            result
        };

        if let Ok(v) = final_result {
            self.ponger.send(Ping(v)).await
        }

        final_result.unwrap() + 1

    }

}

#[actix_rt::main]
async fn main() {

    let ponger = Ponger {}.start();
    let pinger = Pinger { ponger }.start();

    println!("Result {}", pinger.send(Ping(2)).await.unwrap());
    println!("Result {}", pinger.send(Ping(3)).await.unwrap());
}