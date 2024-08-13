Actix Async Handler
===================

An attribute macro to support writing `async` message handlers for Actix actors

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

## Usage

Add actix_async_handler as dev dependency.

```
cargo add --dev actix_async_handler
```

When implementing an async handler, annotate it with the `#[async_handler]` attribute like 

```rust

#[async_handler]
impl Handler<Msg> for MyActor {
    type Result = u64; // or whatever your message handler returns, no enclosing ResponseActFuture or AtomicFuture needed 
    async fn handle(&mut self, _msg: Msg, _ctx: &mut Context<Self>) -> Self::Result {
        // your handler code, for example
        self.other_actor_addr.send(OtherMsg()).await // yay! we can use await
    }
```
that's it! Enjoy.

By default, the returned future will be an `AtomicFuture`, so your actor won't handle any other incoming messages until
fully resolves any awaited calls. This is the behavior that mostly respects the Hewitt's original model, letting you 
abstract the await-ness in your code and use it exactly like a sync version would do. If you rather let your actor 
process messages in between awaits, you can change it to be a `ResponseActFuture` by annotating your handler with 
`#[async_handler(non_atomic)]` instead. 


## Known Limitations

Known list of language features that won't be correctly translated, and hopefully workarounds that may exist. 

### Chained operations on await results

The following code is not translated well (yet)

```rust
let result = self.delegate_actor_addr.send(MyMsg).await.or_else(0) + 3
```

Isolate the awaitable call to its own expression instead

```rust
let await_result = self.delegate_actor_addr.send(MyMsg).await;
let result = await_result.or_else(0) + 3
```


### If expressions

#### Mutating variables inside if expressions

The following code won't work as expected

```rust
let mut result = None;

if some_condition {
    let returned_value = self.delegate_actor.send(message).await;
    result = returned_value.ok();
}

println!("{}", result); // Always prints None regardless of some_condition and returned_value
```

The `async_handler` macro translates your async code to a "pyramid of doom" in order to correctly
move the latest value of your variables. 

For example, a code like this

```rust

let a = call_a().await;
let b = call_b(a).await;
let c = call_c(b).await;
println!("{}, {}, {}", a, b, c)
```

becomes (simplified)

```rust
wrap_future(call_a())
    .then(move |__res, __self, __ctx| {
        let a = __res;
        wrap_future(call_b(a))    
            .then(move |__res, __self, __ctx| {
                let b = __res;
                wrap_future(call_c(b))
                    .then(move |__res, __self, __ctx| {
                        let c = __res;
                        println!("{}, {}, {}", a, b, c)
                    })
            })
    })
```

This way the latest lines are the innermost in the `then` chain, and as such are moving the correct values for the scope variables.

The problem arises when you are using an if condition. Here as we have different branches, `then` is applied externally.

For the first example, the translated code would look like (again simplified)

```rust
    let mut result = None;

    (if some_condition {
        wrap_future(self.delegate_actor.send(message))
            .then(move |__res, __self, __ctx| {
                let returned_value = __res;
                result = returned_value.ok(); // updates the local copy of result, useless
            } 
    } else {
        wrap_future(fut::ready(())) // both if branches need to return a future. 
    }).then(move |__res, __self, __ctx| {
        println!("{}", result);
    })
```

The `then` for the lines after the if is put outside the conditional chain, and as such captures the original variable 
value. Hence, the value stays the original from the point of view of the print.  

To overcome this issue, you should make your condition always return what you need to be updated.

In the code above, you should do instead

```rust

let mut result = None;

result = if some_condition {
    let returned_value = self.delegate_actor.send(message).await;
    returned_value.ok()
}

println!("{}", result);
```

If you have multiple variables you wish to update, you could pack them in a tuple

```rust

let mut a = 0, mut b = 0, mut c = 0;

(a, b, c) = if some_condition {
    a = call_a().await;
    b = call_b(b).await;
    c = call_c(c).await;
    (a, b, c)
} else {
    (a, b, c) // return the defaults. It is mandatory to have an else
}

```

#### Need for explicitly setting a return type for if expressions

This doesn't compile

```rust
let result = if some_condition {
    let a = call_a().await
    a.ok()
} else {
    None
}
```

As the translation code is not smart enough to figure the returned type of `a.ok()`

instead you should hit the compiler on the type like:

```rust
let result: Option<CallAResultType> = if some_condition {
    let a = call_a().await // image return type to be Result<CallAResultType, Err>
    a.ok()
} else {
    None
}
```

#### Early returning inside if expressions

This code wouldn't do what you expect

```rust

if some_early_exit_condition {
    call_a().await;
    return;
}

call_b(a).await;
...
```

As the `then` chain is external to the closure containing the `if`, it won't avoid the code after the await to be executed.

Write an else block containing the rest instead

```rust
if some_early_exit_condition {
    call_a().await;
} else {
    call_b(a).await;
    ... // rest of the code
}
```

### Previous declaration of result variable

This fails to compile with ``Cannot assign to `a` as it is not declared mutable``

```rust
let a;

if condition {
    a = call_a().await;
}
```

Given you cannot really use a for anything outside the then block, simply declare it local. If you 
want to "return" the result of the await call, refer to [Mutating variables inside if expressions](#mutating-variables-inside-if-expressions)

### match expressions

`await`s inside `match` expressions are not currently supported. Replace them with chained `if let` expressions instead like

```rust
match action {
    Move(x, y) => call_move_async(x, y).await,
    Talk(msg) => say_async(msg).await,
    _ => println!("unknown action");
}
```

becomes

```rust
if let Move(x, y) = action {
    call_move_async(x, y).await
} else if let Talk(msg) = action {
    say_async(msg).await
} else {
    println!("unknown action");
}
```