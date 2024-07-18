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

    let awaits = split_awaits(&self_renamed);

    let future_chain = awaits.iter().rfold(None, |acc, await_block|
        match acc {
            Some(inner) => Some(quote! {
                .then(move |__res, __self, __ctx| {
                    #await_block #inner
                })
            }),
            None => Some(quote! {
                .map(move |__res, __self, __ctx| {
                    #await_block
                })
            })
        }
    ).unwrap_or(quote!());

    let result_type = result_type_ident(is_atomic, body.span());

    body.block = parse_quote!({
        use actix::ActorFutureExt;
        actix::#result_type::new(Box::pin(actix::fut::wrap_future::<_, Self>(actix::fut::ready(()))
            #future_chain
       ))
    });

    Ok(())
}

fn split_awaits(block: &Block) -> Vec<TokenStream> {
    let mut parts = Vec::new();
    let mut current_part = TokenStream::new();
    for stmt in &block.stmts {
        match stmt {
            Stmt::Expr(Expr::Await(expr), _) => {
                let base = &*expr.base;
                quote!(
                    actix::fut::wrap_future::<_, Self>(#base)
                ).to_tokens(&mut current_part);
                parts.push(current_part);
                current_part = TokenStream::new();
            }
            stmt => {
                stmt.to_tokens(&mut current_part);
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
        assert_eq!(split.len(), 4);
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

}