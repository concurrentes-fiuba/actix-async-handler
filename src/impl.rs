use proc_macro2::{Span, TokenStream, TokenTree};
use quote::{quote, ToTokens};
use syn::{Block, Error, Expr, Ident, ImplItem, ImplItemFn, ImplItemType, ItemImpl, Macro, Pat, Result, Stmt};
use syn::FnArg::Typed;
use syn::fold::Fold;
use syn::spanned::Spanned;

// inspiration https://github.com/alexcrichton/futures-await/blob/0cd2c3f10d5b0b978836e843a272a590ba574434/futures-await-async-macro/src/lib.rs#L401

pub fn async_handler_impl(attribute: TokenStream, input: TokenStream) -> TokenStream {

    let is_atomic = match &attribute.to_string() as &str {
        "non_atomic" => false,
        "" => true,
        _ => panic!("the #[async_handler] attribute currently only takes `non_atomic` as an arg"),
    };

    async_handler_inner(is_atomic, input)
        .unwrap_or_else(|e| e.to_compile_error())
}

pub fn async_handler_inner(is_atomic: bool, input: TokenStream) -> Result<TokenStream> {
    let input_item = syn::parse2::<ItemImpl>(input.clone());

    let is_handler = input_item.clone().ok()
        .and_then(|item| item.trait_)
        .and_then(|trait_| trait_.1.segments.first().map(|i| "Handler" == i.ident.to_string()))
        .unwrap_or(false);

    if !is_handler {
        return Err(Error::new(input.span(), "#[async_handler] can only be applied to an actor Handler impl"))
    }

    let mut item_fn = input_item.unwrap();

    for item in &mut item_fn.items {
        match item {
            ImplItem::Type(ref mut body) if "Result" == body.ident.to_string() => {
                process_result_type(is_atomic, body)?;
            }
            ImplItem::Fn(ref mut body) if "handle" == body.sig.ident.to_string() => {
                process_handler_fn(is_atomic, body)?;
            }
            _ => {}
        }
    }

    return Ok(quote! { #item_fn })
}

fn process_result_type(is_atomic: bool, body: &mut ImplItemType) -> Result<()> {
    let result_type = result_type_ident(is_atomic, body.span());
    let item_ty = &body.ty;
    body.ty = parse_quote! { actix::#result_type <Self, #item_ty > };
    Ok(())
}

fn result_type_ident(is_atomic: bool, span: Span) -> Ident {
    if is_atomic {
        Ident::new("AtomicResponse", span)
    } else {
        Ident::new("ResponseActFuture", span)
    }
}

fn process_handler_fn(is_atomic: bool, body: &mut ImplItemFn) -> Result<()> {
    body.sig.asyncness = None;

    body.sig.output = parse_quote! { -> Self::Result };

    let ctx_ident = if let Some(Typed(t)) = body.sig.inputs.last() {
        if let Pat::Ident(ident) = &*t.pat {
            Some(ident.ident.to_string())
        } else {
            None
        }
    } else {
        None
    }.ok_or(Error::new(body.span(), "#[async_handler] invalid argument types for Handler impl"))?;

    let self_renamed = RenameParams(ctx_ident).fold_block(body.clone().block);

    // TODO: support awaits inside ifs
    // TODO: support awaits inside fors
    // TODO: awaits in assignments and renaming in the following code to __res
    // TODO: state machine for internal vars passing thru future chain; including incoming message

    let awaits = split_awaits(&self_renamed);

    let mut future_chain = Vec::new();

    for await_block in &awaits[0..awaits.len() - 1] {
        future_chain.push(quote! {
            .then(move |__res, __self, __ctx| {
                #(#await_block)*
            })
        })
    }

    let last = awaits.last().unwrap();
    future_chain.push(quote! {
        .map(move |__res, __self, __ctx| {
            #(#last)*
        })
    });

    let result_type = result_type_ident(is_atomic, body.span());

    body.block = parse_quote!({
        use crate::actix::ActorFutureExt;
        actix::#result_type::new(Box::pin(actix::fut::wrap_future::<_, Self>(actix::fut::ready(()))
            #(#future_chain)*
       ))
    });

    Ok(())
}

fn split_awaits(block: &Block) -> Vec<Vec<Stmt>> {
    let mut parts = Vec::new();
    let mut current_part = Vec::new();
    for stmt in &block.stmts {
        match stmt {
            Stmt::Expr(Expr::Await(expr), _) => {
                let base = &*expr.base;
                current_part.push(parse_quote!(
                    return actix::fut::wrap_future::<_, Self>(#base);
                ));
                parts.push(current_part);
                current_part = Vec::new();
            }
            stmt => {
                current_part.push(stmt.clone());
            }
        }
    }
    parts.push(current_part);
    parts
}

struct RenameParams(String);

impl Fold for RenameParams {
    fn fold_ident(&mut self, i: Ident) -> Ident {
        if i == "self" {
            Ident::new("__self", i.span())
        } else if i == self.0 {
            Ident::new("__ctx", i.span())
        } else {
            i
        }
    }

    fn fold_macro(&mut self, i: Macro) -> Macro {
        let mut output = i.clone();
        output.tokens = TokenStream::new();
        for token in i.tokens {
            match token {
                TokenTree::Ident(ident) => {
                    TokenTree::Ident(self.fold_ident(ident)).to_tokens(&mut output.tokens);
                }
                other => {
                    other.to_tokens(&mut output.tokens)
                }
            }
        }
        output
    }

}

#[cfg(test)]
mod tests {
    use rust_format::Formatter;
    use super::*;

    #[test]
    fn test_requires_handler_impl_fails() {
        assert!(async_handler_inner(false, quote! {
            struct AnActor {}
        }).is_err());
    }

    #[test]
    fn test_requires_handler_impl() {
        assert!(async_handler_inner(false, quote! {
            impl Handler<T> for AnActor {}
        }).is_ok());
    }

    #[test]
    fn test_signature_removes_async() {
        todo!()
    }

    #[test]
    fn test_signature_updates_result_type() {
        todo!()
    }

    #[test]
    fn test_renames_self_top_level_references() {
        todo!()
    }

    #[test]
    fn test_renames_self_parameter_references() {
        todo!()
    }

    #[test]
    fn test_renames_self_macro_parameter_references() {
        todo!()
    }

    #[test]
    fn test_renames_ctx_top_level_references() {
        todo!()
    }

    #[test]
    fn test_renames_ctx_param_references() {
        todo!()
    }

    #[test]
    fn test_renames_ctx_macro_parameter_references() {
        todo!()
    }

    #[test]
    fn test_splits_awaits() {
        let block = parse_quote!({
            println!("Before 1");
            println!("Before 2");
            sleep(Duration::from_secs(1)).await;
            println!("After first 1");
            sleep(Duration::from_secs(1)).await;
            sleep(Duration::from_secs(1)).await;
            println!("Final 1");
            println!("Final 2");
        });

        let split = split_awaits(&block);
        dbg!(&split[0]);
        assert_eq!(split.len(), 4);
        assert_eq!(split[0].len(), 3);
        assert_eq!(split[1].len(), 2);
        assert_eq!(split[2].len(), 1);
        assert_eq!(split[3].len(), 2);
    }

    #[test]
    fn test_returns_last_await_result() {
        todo!()
    }

    #[test]
    fn test_returns_last_statement() {
        todo!()
    }

    #[test]
    fn test_uses_message_between_awaits() {
        todo!()
    }

    #[test]
    fn test_supports_variables_between_awaits() {
        todo!()
    }

    #[test]
    fn test_supports_conditional_awaits() {
        todo!()
    }

    #[test]
    fn test_supports_await_loops() {
        todo!()
    }

    #[test]
    fn test_non_atomic_response() {
        todo!()
    }

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
        use crate::actix::ActorFutureExt;
        actix::AtomicResponse::new(Box::pin(
            actix::fut::wrap_future::<_, Self>(actix::fut::ready(()))
                .then(move |__res, __self, __ctx| {
                    println!("Before 1");
                    println!("Before 2");
                    return actix::fut::wrap_future::<_, Self>(sleep(Duration::from_secs(1)));
                })
                .then(move |__res, __self, __ctx| {
                    println!("After first 1");
                    return actix::fut::wrap_future::<_, Self>(sleep(Duration::from_secs(1)));
                })
                .then(move |__res, __self, __ctx| {
                    return actix::fut::wrap_future::<_, Self>(sleep(Duration::from_secs(1)));
                })
                .map(move |__res, __self, __ctx| {
                    println!("Final 1");
                    println!("Final 2");
                }),
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
        use crate::actix::ActorFutureExt;
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

}