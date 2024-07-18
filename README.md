Actix Async Handler
===================

A attribute macro to support writing `async` message handlers for Actix actors

Using this macro you can convert [this example](https://docs.rs/actix/latest/actix/struct.AtomicResponse.html#examples) 

```rust
fn handle(&mut self, _: Msg, _: &mut Context<Self>) -> Self::Result {
    AtomicResponse::new(Box::pin(
        async {}
            .into_actor(self)
            .map(|_, this, _| {
                this.0 = 30;
            })
            .then(|_, this, _| {
                sleep(Duration::from_secs(3)).into_actor(this)
            })
            .map(|_, this, _| {
                this.0 -= 1;
                this.0
            }),
    ))
}

```

into a much more readable async handler

```rust
async fn handle(&mut self, _msg: Msg, _ctx: &mut Context<Self>) -> Self::Result {
    self.0 = 30;
    sleep(Duration::from_secs(3)).await;
    self.0 -= 1;
    self.0
}
```