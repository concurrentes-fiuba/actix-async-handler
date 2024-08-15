use actix::{Addr, Handler};

use actix::{Actor, Context, Message};

use actix_async_handler::async_handler;

struct Counter {}

impl Actor for Counter {
    type Context = Context<Self>;
}

#[derive(Message, Clone, Copy)]
#[rtype(result = "u64")]
struct Count(u64);

impl Handler<Count> for Counter {
    type Result = u64;

    fn handle(&mut self, msg: Count, ctx: &mut Self::Context) -> Self::Result {
        msg.0 + 1
    }
}

#[actix_rt::test]
async fn test_awaits_and_variables() {

    struct AnActor {
        delegate: Addr<Counter>
    }

    impl Actor for AnActor {
        type Context = Context<Self>;
    }

    #[async_handler]
    impl Handler<Count> for AnActor {
        type Result = u64;

        async fn handle(&mut self, msg: Count, ctx: &mut Self::Context) -> Self::Result {
            let mut count = msg.0;
            let result = self.delegate.send(Count(count)).await;
            count += result.unwrap();
            let mut count1 = count;
            let mut result = self.delegate.send(Count(count)).await;
            count += result.unwrap();
            count1 += count;
            result = self.delegate.send(Count(count)).await;
            count + count1 + result.unwrap()
        }
    }

    let delegate = Counter {}.start();
    let addr = AnActor { delegate }.start();
    let result = addr.send(Count(1)).await.unwrap();
    assert_eq!(25, result);
}

#[actix_rt::test]
async fn test_no_awaits() {

    struct AnActor {
        delegate: Addr<Counter>
    }

    impl Actor for AnActor {
        type Context = Context<Self>;
    }

    #[async_handler]
    impl Handler<Count> for AnActor {
        type Result = u64;

        async fn handle(&mut self, msg: Count, ctx: &mut Self::Context) -> Self::Result {
            msg.0 + 1
        }
    }

    let delegate = Counter {}.start();
    let addr = AnActor { delegate }.start();
    let result = addr.send(Count(1)).await.unwrap();
    assert_eq!(2, result);
}

// splitting to avoid too many awaits in the handler as they end up really slowing up the compiler
#[actix_rt::test]
async fn test_ifs_part1() {

    struct AnActor {
        flags: u64,
        delegate: Addr<Counter>
    }

    impl Actor for AnActor {
        type Context = Context<Self>;
    }

    #[async_handler]
    impl Handler<Count> for AnActor {
        type Result = u64;

        async fn handle(&mut self, msg: Count, ctx: &mut Self::Context) -> Self::Result {

            self.flags = 0;

            // No awaits, no else
            if msg.0 == 0 {
                self.flags |= 1;
            }

            // No awaits, else
            if msg.0 == 1 {
                self.flags |= 2;
            } else {
                self.flags |= 4;
            }

            // No awaits, else if
            if msg.0 == 3 {
                self.flags |= 8;
            } else if msg.0 == 4 {
                self.flags |= 16;
            } else {
                self.flags |= 32;
            }

            // if awaits, no else
            if msg.0 == 6 {
                let r = self.delegate.send(Count(msg.0 - 1)).await;
                self.flags |= 1 << r.unwrap();
            }
            
            // if awaits, else does not
            if msg.0 == 7 {
                let r = self.delegate.send(Count(msg.0 - 1)).await;
                self.flags |= 1 << r.unwrap();
            } else {
                self.flags |= 1 << 8;
            }
            
            // if awaits, else if does not
            if msg.0 == 9 {
                let r = self.delegate.send(Count(msg.0 - 1)).await;
                self.flags |= 1 << r.unwrap();
            } else if msg.0 == 10 {
                self.flags |= 1 << 10;
            } else {
                self.flags |= 1 << 11;
            }
            
            // if awaits, else awaits
            if msg.0 == 12 {
                let r = self.delegate.send(Count(msg.0 - 1)).await;
                self.flags |= 1 << r.unwrap();
            } else {
                let r = self.delegate.send(Count(12)).await;
                self.flags |= 1 << r.unwrap();
            }
            
            // if awaits, else if awaits, else does not
            if msg.0 == 14 {
                let r = self.delegate.send(Count(msg.0 - 1)).await;
                self.flags |= 1 << r.unwrap();
            } else if msg.0 == 15 {
                let r = self.delegate.send(Count(msg.0 - 1)).await;
                self.flags |= 1 << r.unwrap();
            } else {
                self.flags |= 1 << 16;
            }
            
            // all await
            if msg.0 == 17 {
                let r = self.delegate.send(Count(msg.0 - 1)).await;
                self.flags |= 1 << r.unwrap();
            } else if msg.0 == 18 {
                let r = self.delegate.send(Count(msg.0 - 1)).await;
                self.flags |= 1 << r.unwrap();
            } else {
                let r = self.delegate.send(Count(18)).await;
                self.flags |= 1 << r.unwrap();
            }

            self.flags
        }
    }

    let delegate = Counter {}.start();
    let addr = AnActor { delegate, flags: 0 }.start();
    for i in 0..20 {
        let result = addr.send(Count(i)).await.unwrap();
        assert_eq!(((1 << i) & result) > 0, true, "case {} value {}", i, result);
    }

}

#[actix_rt::test]
async fn test_ifs_part2() {

    struct AnActor {
        flags: u64,
        delegate: Addr<Counter>
    }

    impl Actor for AnActor {
        type Context = Context<Self>;
    }

    #[async_handler]
    impl Handler<Count> for AnActor {
        type Result = u64;

        async fn handle(&mut self, msg: Count, ctx: &mut Self::Context) -> Self::Result {

            self.flags = 0;

            // all await but else if
            if msg.0 == 20 {
                let r = self.delegate.send(Count(msg.0 - 1)).await;
                self.flags |= 1 << r.unwrap();
            } else if msg.0 == 21 {
                self.flags |= 1 << 21;
            } else {
                let r = self.delegate.send(Count(21)).await;
                self.flags |= 1 << r.unwrap();
            }

            // only else awaits
            if msg.0 == 23 {
                self.flags |= 1 << 23;
            } else {
                let r = self.delegate.send(Count(23)).await;
                self.flags |= 1 << r.unwrap();
            }

            // only else awaits, with else if
            if msg.0 == 25 {
                self.flags |= 1 << 25;
            } else if msg.0 == 26 {
                self.flags |= 1 << 26;
            } else {
                let r = self.delegate.send(Count(26)).await;
                self.flags |= 1 << r.unwrap();
            }

            // only else if awaits, no else
            if msg.0 == 28 {
                self.flags |= 1 << 28;
            } else if msg.0 == 29 {
                let r = self.delegate.send(Count(msg.0 - 1)).await;
                self.flags |= 1 << r.unwrap();
            }

            // only else if awaits, with else
            if msg.0 == 30 {
                self.flags |= 1 << 30;
            } else if msg.0 == 31 {
                let r = self.delegate.send(Count(msg.0 - 1)).await;
                self.flags |= 1 << r.unwrap();
            } else {
                self.flags |= 1 << 32;
            }

            // both elses await
            if msg.0 == 33 {
                self.flags |= 1 << 33;
            } else if msg.0 == 34 {
                let r = self.delegate.send(Count(msg.0 - 1)).await;
                self.flags |= 1 << r.unwrap();
            } else {
                let r = self.delegate.send(Count(34)).await;
                self.flags |= 1 << r.unwrap();
            }

            // if awaits, else if awaits, no else
            if msg.0 == 36 {
                let r = self.delegate.send(Count(msg.0 - 1)).await;
                self.flags |= 1 << r.unwrap();
            } else if msg.0 == 37 {
                let r = self.delegate.send(Count(msg.0 - 1)).await;
                self.flags |= 1 << r.unwrap();
            }

            self.flags
        }
    }

    let delegate = Counter {}.start();
    let addr = AnActor { delegate, flags: 0 }.start();
    for i in 20..38 {
        let result = addr.send(Count(i)).await.unwrap();
        assert_eq!(((1 << i) & result) > 0, true, "case {} value {}", i, result);
    }

}

#[actix_rt::test]
async fn test_ifs_return_values() {

    struct AnActor {
        delegate: Addr<Counter>
    }

    impl Actor for AnActor {
        type Context = Context<Self>;
    }

    #[async_handler]
    impl Handler<Count> for AnActor {
        type Result = u64;

        async fn handle(&mut self, msg: Count, ctx: &mut Self::Context) -> Self::Result {

            let mut ret1 = 0;

            ret1 = if msg.0 == 0 {
                let r = self.delegate.send(Count(0)).await;
                let r2 = self.delegate.send(Count(r.unwrap())).await;
                r2.unwrap()
            } else {
                1
            };

            let ret2 = if msg.0 == 1 {
                3
            } else {
                let r = self.delegate.send(Count(1)).await;
                let r2 = self.delegate.send(Count(r.unwrap())).await;
                r2.unwrap()
            };

            ret1 + ret2
        }
    }

    let delegate = Counter {}.start();
    let addr = AnActor { delegate }.start();
    let result = addr.send(Count(0)).await.unwrap();
    assert_eq!(result, 5);

    let result = addr.send(Count(1)).await.unwrap();
    assert_eq!(result, 4);
}

#[actix_rt::test]
async fn test_for_loop() {

    struct AnActor {
        acc: u64,
        delegates: Vec<Addr<Counter>>
    }

    impl Actor for AnActor {
        type Context = Context<Self>;
    }

    #[async_handler]
    impl Handler<Count> for AnActor {
        type Result = u64;

        async fn handle(&mut self, msg: Count, ctx: &mut Self::Context) -> Self::Result {

            let mut i = 0;
            i = for delegate in self.delegates.clone() {
                let r = delegate.send(Count(msg.0)).await;
                let r2 = delegate.send(Count(msg.0 + 1)).await;
                i += r.unwrap() + r2.unwrap()
            };

            for delegate in self.delegates.clone() {
                let r = delegate.send(Count(msg.0)).await;
                self.acc += r.unwrap();
            };

            i + self.acc
        }
    }

    let mut delegates = vec![];
    for id in 0..5 {
        delegates.push(Counter {}.start());
    };

    let addr = AnActor { delegates, acc: 0 }.start();
    let result = addr.send(Count(0)).await.unwrap();
    assert_eq!(result, 20);

}