use crate::r#impl::async_handler_inner;
use quote::quote;
use rust_format::Formatter;

#[test]
fn test_splits_awaits_integration() {
    let result = async_handler_inner(true, quote! {
        impl Handler<T> for AnActor {
            async fn handle(&mut self, msg: Ping, ctx: &mut Self::Context) -> Self::Result {
                println!("Before 1");
                println!("Before 2");
                sleep(Duration::from_secs(1)).await;
                println!("After first 1");
                sleep(Duration::from_secs(1)).await;
                sleep(Duration::from_secs(1)).await;
                println!("Final 1");
                println!("Final 2");
            }
        }
    });

    let expected =
r#"impl Handler<T> for AnActor {
    fn handle(&mut self, msg: Ping, ctx: &mut Self::Context) -> Self::Result {
        use actix::ActorFutureExt;
        actix::AtomicResponse::new(Box::pin(
            actix::fut::wrap_future::<_, Self>(actix::fut::ready(())).then(
                move |__res, __self, __ctx| {
                    println!("Before 1");
                    println!("Before 2");
                    actix::fut::wrap_future::<_, Self>(sleep(Duration::from_secs(1))).then(
                        move |__res, __self, __ctx| {
                            println!("After first 1");
                            actix::fut::wrap_future::<_, Self>(sleep(Duration::from_secs(1))).then(
                                move |__res, __self, __ctx| {
                                    actix::fut::wrap_future::<_, Self>(sleep(Duration::from_secs(
                                        1,
                                    )))
                                    .map(
                                        move |__res, __self, __ctx| {
                                            println!("Final 1");
                                            println!("Final 2");
                                        },
                                    )
                                },
                            )
                        },
                    )
                },
            ),
        ))
    }
}
"#;

    let actual = rust_format::RustFmt::default().format_tokens(result.expect("")).expect("");
    assert_eq!(expected, actual);
}

#[test]
fn test_splits_awaits_no_awaits_integration() {

    let result = async_handler_inner(true, quote! {
        impl Handler<T> for AnActor {

            type Result = String;

            async fn handle(&mut self, msg: Ping, ctx: &mut Self::Context) -> Self::Result {
                println!("Before 1");
                println!("Before 2");
            }
        }
    });

    let expected =
r#"impl Handler<T> for AnActor {
    type Result = actix::AtomicResponse<Self, String>;
    fn handle(&mut self, msg: Ping, ctx: &mut Self::Context) -> Self::Result {
        use actix::ActorFutureExt;
        actix::AtomicResponse::new(Box::pin(
            actix::fut::wrap_future::<_, Self>(actix::fut::ready(())).map(
                move |__res, __self, __ctx| {
                    println!("Before 1");
                    println!("Before 2");
                },
            ),
        ))
    }
}
"#;

    let actual = rust_format::RustFmt::default().format_tokens(result.expect("")).expect("");
    assert_eq!(expected, actual)
}

#[test]
fn test_variables_thru_chain() {
    let result = async_handler_inner(true, quote! {
        impl Handler<GetVariables> for Variables {
            type Result = u64;
            async fn handle(&mut self, msg: GetVariables, ctx: &mut Self::Context) -> Self::Result {
                let mut var0 = msg.0;
                sleep(Duration::from_secs(1)).await;
                var0 += msg.0;
                let mut var1 = var0;
                sleep(Duration::from_secs(1)).await;
                var0 += var1;
                var1 += var0;
                let var2 = msg.0;
                sleep(Duration::from_secs(1)).await;
                msg.0 + var0 + var1 + var2
            }
        }
    });

    let expected =
r#"impl Handler<GetVariables> for Variables {
    type Result = actix::AtomicResponse<Self, u64>;
    fn handle(&mut self, msg: GetVariables, ctx: &mut Self::Context) -> Self::Result {
        use actix::ActorFutureExt;
        actix::AtomicResponse::new(Box::pin(
            actix::fut::wrap_future::<_, Self>(actix::fut::ready(())).then(
                move |__res, __self, __ctx| {
                    let mut var0 = msg.0;
                    actix::fut::wrap_future::<_, Self>(sleep(Duration::from_secs(1))).then(
                        move |__res, __self, __ctx| {
                            var0 += msg.0;
                            let mut var1 = var0;
                            actix::fut::wrap_future::<_, Self>(sleep(Duration::from_secs(1))).then(
                                move |__res, __self, __ctx| {
                                    var0 += var1;
                                    var1 += var0;
                                    let var2 = msg.0;
                                    actix::fut::wrap_future::<_, Self>(sleep(Duration::from_secs(
                                        1,
                                    )))
                                    .map(move |__res, __self, __ctx| msg.0 + var0 + var1 + var2)
                                },
                            )
                        },
                    )
                },
            ),
        ))
    }
}
"#;

    let actual = rust_format::RustFmt::default().format_tokens(result.clone().expect("")).expect("");
    assert_eq!(expected, actual)
}

#[test]
fn test_await_return_value_assignment() {
    let result = async_handler_inner(true, quote! {
        impl Handler<GetVariables> for ResultAssignment {
            type Result = u64;
            async fn handle(&mut self, msg: GetVariables, ctx: &mut Self::Context) -> Self::Result {
                let result2;
                let result1 = self.other_actor.send(0).await;
                result2 = self.other_actor.send(result1).await;
                result1 + result2
            }
        }
    });

    let expected =
        r#"impl Handler<GetVariables> for ResultAssignment {
    type Result = actix::AtomicResponse<Self, u64>;
    fn handle(&mut self, msg: GetVariables, ctx: &mut Self::Context) -> Self::Result {
        use actix::ActorFutureExt;
        actix::AtomicResponse::new(Box::pin(
            actix::fut::wrap_future::<_, Self>(actix::fut::ready(())).then(
                move |__res, __self, __ctx| {
                    let result2;
                    actix::fut::wrap_future::<_, Self>(__self.other_actor.send(0)).then(
                        move |__res, __self, __ctx| {
                            let result1 = __res;
                            actix::fut::wrap_future::<_, Self>(__self.other_actor.send(result1))
                                .map(move |__res, __self, __ctx| {
                                    result2 = __res;
                                    result1 + result2
                                })
                        },
                    )
                },
            ),
        ))
    }
}
"#;

    let actual = rust_format::RustFmt::default().format_tokens(result.clone().expect("")).expect("");
    assert_eq!(expected, actual)
}