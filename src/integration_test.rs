use crate::r#impl::async_handler_inner;
use quote::quote;
use rust_format::Formatter;
use syn::ExprIf;

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

#[test]
fn test_if_single_branch() {
    let result = async_handler_inner(true, quote! {
        impl Handler<Conditional> for ResultAssignment {
            type Result = u64;
            async fn handle(&mut self, msg: Conditional, ctx: &mut Self::Context) -> Self::Result {

                if msg.0 > 0 {
                    self.other_actor.send(0).await;
                }

                self.other_actor.send(msg).await
            }
        }
    });

    let expected =
        r#"impl Handler<Conditional> for ResultAssignment {
    type Result = actix::AtomicResponse<Self, u64>;
    fn handle(&mut self, msg: Conditional, ctx: &mut Self::Context) -> Self::Result {
        use actix::ActorFutureExt;
        actix::AtomicResponse::new(Box::pin(
            actix::fut::wrap_future::<_, Self>(actix::fut::ready(())).then(
                move |__res, __self, __ctx| {
                    if msg.0 > 0 {
                        Box::pin({
                            actix::fut::wrap_future::<_, Self>(__self.other_actor.send(0)).map(
                                move |__res, __self, __ctx| {
                                    ()
                                },
                            )
                        })
                            as std::pin::Pin<
                                Box<dyn actix::fut::future::ActorFuture<Self, Output = _>>,
                            >
                    } else {
                        Box::pin(actix::fut::ready(()))
                    }
                    .then(move |__res, __self, __ctx| {
                        actix::fut::wrap_future::<_, Self>(__self.other_actor.send(msg))
                    })
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
fn test_if_branch_awaits() {
    let result = async_handler_inner(true, quote! {
        impl Handler<Conditional> for ResultAssignment {
            type Result = u64;
            async fn handle(&mut self, msg: Conditional, ctx: &mut Self::Context) -> Self::Result {
                if msg.0 > 0 {
                    let part = self.other_actor.send(0).await;
                    self.other_actor.send(part).await
                } else {
                    call_boring_non_awaitable_stuff();
                    42
                }

                self.other_actor.send(0).await
            }
        }
    });

    let expected =
        r#"impl Handler<Conditional> for ResultAssignment {
    type Result = actix::AtomicResponse<Self, u64>;
    fn handle(&mut self, msg: Conditional, ctx: &mut Self::Context) -> Self::Result {
        use actix::ActorFutureExt;
        actix::AtomicResponse::new(Box::pin(
            actix::fut::wrap_future::<_, Self>(actix::fut::ready(())).then(
                move |__res, __self, __ctx| {
                    if msg.0 > 0 {
                        Box::pin({
                            actix::fut::wrap_future::<_, Self>(__self.other_actor.send(0)).then(
                                move |__res, __self, __ctx| {
                                    let part = __res;
                                    actix::fut::wrap_future::<_, Self>(
                                        __self.other_actor.send(part),
                                    )
                                    .map(
                                        move |__res, __self, __ctx| {
                                            ()
                                        },
                                    )
                                },
                            )
                        })
                            as std::pin::Pin<
                                Box<dyn actix::fut::future::ActorFuture<Self, Output = _>>,
                            >
                    } else {
                        Box::pin(actix::fut::ready({
                            {
                                call_boring_non_awaitable_stuff();
                                42
                            };
                        }))
                    }
                    .then(move |__res, __self, __ctx| {
                        actix::fut::wrap_future::<_, Self>(__self.other_actor.send(0))
                    })
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
fn test_if_branch_awaits_return_value() {
    let result = async_handler_inner(true, quote! {
        impl Handler<Conditional> for ResultAssignment {
            type Result = u64;
            async fn handle(&mut self, msg: Conditional, ctx: &mut Self::Context) -> Self::Result {
                let result = if msg.0 > 0 {
                    let part = self.other_actor.send(0).await;
                    self.other_actor.send(part).await
                } else {
                    call_boring_non_awaitable_stuff();
                    42
                };

                self.other_actor.send(result).await
            }
        }
    });

    let expected =
        r#"impl Handler<Conditional> for ResultAssignment {
    type Result = actix::AtomicResponse<Self, u64>;
    fn handle(&mut self, msg: Conditional, ctx: &mut Self::Context) -> Self::Result {
        use actix::ActorFutureExt;
        actix::AtomicResponse::new(Box::pin(
            actix::fut::wrap_future::<_, Self>(actix::fut::ready(())).then(
                move |__res, __self, __ctx| {
                    if msg.0 > 0 {
                        Box::pin({
                            actix::fut::wrap_future::<_, Self>(__self.other_actor.send(0)).then(
                                move |__res, __self, __ctx| {
                                    let part = __res;
                                    actix::fut::wrap_future::<_, Self>(
                                        __self.other_actor.send(part),
                                    )
                                },
                            )
                        })
                            as std::pin::Pin<
                                Box<dyn actix::fut::future::ActorFuture<Self, Output = _>>,
                            >
                    } else {
                        Box::pin(actix::fut::ready({
                            call_boring_non_awaitable_stuff();
                            42
                        }))
                    }
                    .then(move |__res, __self, __ctx| {
                        let result = __res;
                        actix::fut::wrap_future::<_, Self>(__self.other_actor.send(result))
                    })
                },
            ),
        ))
    }
}
"#;

    let actual = rust_format::RustFmt::default().format_tokens(result.clone().expect("")).expect("");
    assert_eq!(expected, actual);
}

#[test]
fn test_if_both_branches_await() {
    let result = async_handler_inner(true, quote! {
        impl Handler<Conditional> for ResultAssignment {
            type Result = u64;
            async fn handle(&mut self, msg: Conditional, ctx: &mut Self::Context) -> Self::Result {
                if msg.0 > 0 {
                    self.other_actor.send(0).await;
                } else {
                    self.negative_actor.send(42).await;
                };
            }
        }
    });

    let expected =
        r#"impl Handler<Conditional> for ResultAssignment {
    type Result = actix::AtomicResponse<Self, u64>;
    fn handle(&mut self, msg: Conditional, ctx: &mut Self::Context) -> Self::Result {
        use actix::ActorFutureExt;
        actix::AtomicResponse::new(Box::pin(
            actix::fut::wrap_future::<_, Self>(actix::fut::ready(())).then(
                move |__res, __self, __ctx| {
                    if msg.0 > 0 {
                        Box::pin({
                            actix::fut::wrap_future::<_, Self>(__self.other_actor.send(0)).map(
                                move |__res, __self, __ctx| {
                                    ()
                                },
                            )
                        })
                            as std::pin::Pin<
                                Box<dyn actix::fut::future::ActorFuture<Self, Output = _>>,
                            >
                    } else {
                        Box::pin({
                            actix::fut::wrap_future::<_, Self>(__self.negative_actor.send(42)).map(
                                move |__res, __self, __ctx| {
                                    ()
                                },
                            )
                        })
                    }
                },
            ),
        ))
    }
}
"#;

    let actual = rust_format::RustFmt::default().format_tokens(result.clone().expect("")).expect("");
    assert_eq!(expected, actual);
}

#[test]
fn test_if_else_awaits() {
    let result = async_handler_inner(true, quote! {
        impl Handler<Conditional> for ResultAssignment {
            type Result = u64;
            async fn handle(&mut self, msg: Conditional, ctx: &mut Self::Context) -> Self::Result {
                let result = if msg.0 > 0 {
                    call_boring_non_awaitable_stuff()
                } else {
                    self.negative_actor.send(42).await
                };
            }
        }
    });

    let expected =
        r#"impl Handler<Conditional> for ResultAssignment {
    type Result = actix::AtomicResponse<Self, u64>;
    fn handle(&mut self, msg: Conditional, ctx: &mut Self::Context) -> Self::Result {
        use actix::ActorFutureExt;
        actix::AtomicResponse::new(Box::pin(
            actix::fut::wrap_future::<_, Self>(actix::fut::ready(())).then(
                move |__res, __self, __ctx| {
                    if msg.0 > 0 {
                        Box::pin(actix::fut::ready({ call_boring_non_awaitable_stuff() }))
                    } else {
                        Box::pin({
                            actix::fut::wrap_future::<_, Self>(__self.negative_actor.send(42))
                        })
                            as std::pin::Pin<
                                Box<dyn actix::fut::future::ActorFuture<Self, Output = _>>,
                            >
                    }
                    .map(move |__res, __self, __ctx| {
                        let result = __res;
                    })
                },
            ),
        ))
    }
}
"#;

    let actual = rust_format::RustFmt::default().format_tokens(result.clone().expect("")).expect("");
    assert_eq!(expected, actual);
}

#[test]
fn test_if_else_chain_awaits() {
    let result = async_handler_inner(true, quote! {
        impl Handler<Conditional> for ResultAssignment {
            type Result = u64;
            async fn handle(&mut self, msg: Conditional, ctx: &mut Self::Context) -> Self::Result {
                let result = if msg.0 > 0 {
                    call_boring_non_awaitable_stuff()
                } else if other_cond {
                    other_boring_stuff()
                } else if nice_cond {
                    self.fun_actor.send(12).await
                } else {
                    self.negative_actor.send(42).await
                };
            }
        }
    });

    let expected =
        r#"impl Handler<Conditional> for ResultAssignment {
    type Result = actix::AtomicResponse<Self, u64>;
    fn handle(&mut self, msg: Conditional, ctx: &mut Self::Context) -> Self::Result {
        use actix::ActorFutureExt;
        actix::AtomicResponse::new(Box::pin(
            actix::fut::wrap_future::<_, Self>(actix::fut::ready(())).then(
                move |__res, __self, __ctx| {
                    if msg.0 > 0 {
                        Box::pin(actix::fut::ready({ call_boring_non_awaitable_stuff() }))
                    } else if other_cond {
                        Box::pin(actix::fut::ready({ other_boring_stuff() }))
                    } else if nice_cond {
                        Box::pin({ actix::fut::wrap_future::<_, Self>(__self.fun_actor.send(12)) })
                            as std::pin::Pin<
                                Box<dyn actix::fut::future::ActorFuture<Self, Output = _>>,
                            >
                    } else {
                        Box::pin({
                            actix::fut::wrap_future::<_, Self>(__self.negative_actor.send(42))
                        })
                    }
                    .map(move |__res, __self, __ctx| {
                        let result = __res;
                    })
                },
            ),
        ))
    }
}
"#;

    let actual = rust_format::RustFmt::default().format_tokens(result.clone().expect("")).expect("");
    assert_eq!(expected, actual);
}

#[test]
fn test_if_assigns() {
    let result = async_handler_inner(true, quote! {
        impl Handler<Conditional> for ResultAssignment {
            type Result = u64;
            async fn handle(&mut self, msg: Conditional, ctx: &mut Self::Context) -> Self::Result {
                let result;
                result = if msg.0 > 2 {
                    self.other_actor.send(part).await
                } else {
                    15
                };

                self.other_actor.send(result).await
            }
        }
    });

    let expected =
        r#"impl Handler<Conditional> for ResultAssignment {
    type Result = actix::AtomicResponse<Self, u64>;
    fn handle(&mut self, msg: Conditional, ctx: &mut Self::Context) -> Self::Result {
        use actix::ActorFutureExt;
        actix::AtomicResponse::new(Box::pin(
            actix::fut::wrap_future::<_, Self>(actix::fut::ready(())).then(
                move |__res, __self, __ctx| {
                    let result;
                    if msg.0 > 2 {
                        Box::pin({
                            actix::fut::wrap_future::<_, Self>(__self.other_actor.send(part))
                        })
                            as std::pin::Pin<
                                Box<dyn actix::fut::future::ActorFuture<Self, Output = _>>,
                            >
                    } else {
                        Box::pin(actix::fut::ready({ 15 }))
                    }
                    .then(move |__res, __self, __ctx| {
                        result = __res;
                        actix::fut::wrap_future::<_, Self>(__self.other_actor.send(result))
                    })
                },
            ),
        ))
    }
}
"#;

    let actual = rust_format::RustFmt::default().format_tokens(result.clone().expect("")).expect("");
    assert_eq!(expected, actual);
}

#[test]
fn test_for_loop() {
    let result = async_handler_inner(true, quote! {
        impl Handler<Conditional> for ResultAssignment {
            type Result = u64;
            async fn handle(&mut self, msg: Conditional, ctx: &mut Self::Context) -> Self::Result {

                for ponger in self.pongers {
                    println!("pre loop");
                    ponger.send(msg).await;
                    println!("middle loop");
                    ponger.send(Ping(msg.0 + 1)).await;
                    println!("end loop");
                }

                self.pongers[0].send(msg).await
            }
        }
    });


    let actual = rust_format::RustFmt::default().format_tokens(result.clone().expect("")).expect("");
    println!("{}", actual);
}