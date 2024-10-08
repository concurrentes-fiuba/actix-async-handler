use proc_macro2::{Span, TokenStream, TokenTree};
use quote::{quote, ToTokens};
use rust_format::Error::BadSourceCode;
use rust_format::Formatter;
use syn::{Block, Error, Expr, ExprAssign, ExprAwait, ExprBlock, ExprForLoop, ExprIf, Ident, ImplItem, ImplItemFn, ImplItemType, ItemImpl, Local, LocalInit, Macro, Pat, Result, Stmt};
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
    let mut item_fn = syn::parse2::<ItemImpl>(input.clone())?;

    let is_handler = item_fn.trait_.as_ref()
        .and_then(|trait_| trait_.1.segments.first().map(|i| "Handler" == i.ident.to_string()))
        .unwrap_or(false);

    if !is_handler {
        return Err(Error::new(input.span(), "#[async_handler] can only be applied to an actor Handler impl"))
    }

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

    let future_chain = build_future_chain(awaits, true, None);

    let result_type = result_type_ident(is_atomic, body.span());

    let block = quote!({
        use actix::ActorFutureExt;
        actix::#result_type::new(Box::pin(actix::fut::wrap_future::<_, Self>(actix::fut::ready(()))
            #future_chain
       ))
    });

    match syn::parse2::<Block>(block.clone()) {
        Ok(block) => {
            body.block = block;
            Ok(())
        }
        Err(e) => {
            // Errors from parse2 are very short, let's try to use the compiler via RustFmt instead
            match rust_format::RustFmt::default().format_tokens(quote!( fn handler() { #block })) {
                Err(BadSourceCode(e)) => Err(Error::new(body.span(), e)),
                _ => Err(e) // we couldn't get more detail from the formatter, just throw whatever we had,
            }
        }
    }
}

fn build_future_chain(awaits: Vec<TokenStream>, enclose_first: bool, mut return_unit: Option<TokenStream>) -> TokenStream {
    awaits.iter().rfold(None, |acc, await_block|
        match acc {
            Some((count, inner))
                if count == (awaits.len()-1) && !enclose_first => Some((count + 1, quote! {
                    { #await_block #inner }
            })),
            Some((count, inner)) => Some((count + 1, quote! {
                .then(move |__res, __self, __ctx| {
                    #await_block #inner
                })
            })),
            None if return_unit.is_some() => {
                let return_value = return_unit.take().unwrap();
                Some((1, quote! {
                    .map(move |__res, __self, __ctx| {
                        #await_block;
                        #return_value
                    })
                }))
            },
            None if !await_block.is_empty() => Some((1, quote! {
                .map(move |__res, __self, __ctx| {
                    #await_block
                })
            })),
            None => Some((1, quote!()))
        }
    ).unwrap_or((0, quote!())).1
}

fn split_awaits(block: &Block) -> Vec<TokenStream> {
    let mut parts = vec!(TokenStream::new());
    for stmt in &block.stmts {
        if !match stmt {
            Stmt::Expr(Expr::Await(expr), _) => {
                expr_await(&mut parts, expr);
                true
            }
            Stmt::Expr(Expr::Assign(ExprAssign { left, right: expr, .. }), ..) =>
                match &**expr {
                    Expr::Await(inner) => {
                        expr_await(&mut parts, inner);
                        quote!(
                            #left = __res;
                        ).to_tokens(parts.last_mut().unwrap());
                        true
                    }
                    Expr::If(expr ) => {
                        if expr_if(&mut parts, expr, false) {
                            quote!(
                                #left = __res;
                            ).to_tokens(parts.last_mut().unwrap());
                            true
                        } else {
                            false
                        }
                    }
                    Expr::ForLoop(ExprForLoop { pat, expr, body, .. } ) => {
                        if expr_for_loop(&mut parts, pat, expr, body, Some(left.to_token_stream())) {
                            quote!(
                                #left = __res;
                            ).to_tokens(parts.last_mut().unwrap());
                            true
                        } else {
                            false
                        }
                    }
                    _ => false
                }
            Stmt::Local(Local { pat, init: Some(LocalInit { expr, .. }), .. } ) =>
                match &**expr {
                    Expr::Await(inner) => {
                        expr_await(&mut parts, inner);
                        quote! (
                            let #pat = __res;
                        ).to_tokens(parts.last_mut().unwrap());
                        true
                    }
                    Expr::If(expr ) => {
                        if expr_if(&mut parts, expr, false) {
                            quote!(
                                let #pat = __res;
                            ).to_tokens(parts.last_mut().unwrap());
                            true
                        } else {
                            false
                        }
                    }
                    Expr::ForLoop(ExprForLoop { pat: for_pat, expr, body, .. } ) => {
                        if expr_for_loop(&mut parts, for_pat, expr, body, Some(pat.to_token_stream())) {
                            quote!(
                                let #pat = __res;
                            ).to_tokens(parts.last_mut().unwrap());
                            true
                        } else {
                            false
                        }
                    }
                    _ => false
                }
            Stmt::Expr(Expr::If(expr ), ..) => {
                expr_if(&mut parts, expr, true)
            }
            Stmt::Expr(Expr::ForLoop(ExprForLoop { pat, expr, body, .. } ), ..) => {
                expr_for_loop(&mut parts, pat, expr, body, None)
            }
            _ => false
        } {
            stmt.to_tokens(parts.last_mut().unwrap());
        }
    }
    parts
}

fn expr_if(parts: &mut Vec<TokenStream>, expr: &ExprIf, return_unit: bool) -> bool {
    let result = expr_if_inner(expr, return_unit);
    if result.is_empty() {
        false
    } else {
        result.to_tokens(parts.last_mut().unwrap());
        parts.push(TokenStream::new());
        true
    }
}
fn expr_if_inner(expr: &ExprIf, return_unit: bool) -> TokenStream {
    let ExprIf { cond, then_branch, else_branch, .. } = expr;
    let then_parts = split_awaits(then_branch);

    let mut token_stream = TokenStream::new();

    let ret = if return_unit { Some(quote! { () }) } else { None };

    if then_parts.len() > 1 {
        let then_chain = build_future_chain(then_parts, false, ret.clone());
        quote!(
            if #cond {
                Box::pin(#then_chain) as std::pin::Pin<Box<dyn actix::fut::future::ActorFuture<Self, Output=_>>>
            }
        ).to_tokens(&mut token_stream);
        if else_branch.is_none() {
            quote!(
                else {
                    Box::pin(actix::fut::ready(()))
                }
            ).to_tokens(&mut token_stream);
        } else {
            let else_expr = else_branch.as_ref().unwrap().1.as_ref();
            let awaited = match else_expr {
                Expr::Block(ExprBlock { block, .. }) => {
                    let else_parts = split_awaits(block);
                    if else_parts.len() > 1 {
                        let else_chain = build_future_chain(else_parts, false, ret);
                        quote!(
                            else {
                                Box::pin(#else_chain)
                            }
                        ).to_tokens(&mut token_stream);
                        true
                    } else {
                        false
                    }
                },
                Expr::If(if_expr) => {
                    let else_parts = expr_if_inner(if_expr, return_unit);
                    if !else_parts.is_empty() {
                        // chained else if(s) have awaits
                        quote!(
                            else #else_parts
                        ).to_tokens(&mut token_stream);
                        true
                    } else {
                        false
                    }
                }
                _ => false
            };
            if !awaited {
                if return_unit {
                    quote!(
                        else {
                            Box::pin(actix::fut::ready({ #else_expr; }))
                        }
                    )
                } else {
                    quote!(
                        else {
                            Box::pin(actix::fut::ready(#else_expr))
                        }
                    )
                }.to_tokens(&mut token_stream);
            }
        }
    } else if else_branch.is_some() {
        match else_branch.as_ref().unwrap().1.as_ref() {
            Expr::Block(ExprBlock { block, .. }) => {
                let else_parts = split_awaits(block);
                if else_parts.len() > 1 {
                    let else_chain = build_future_chain(else_parts, false, ret);
                    non_awaited_if_expr_for_else(return_unit, cond, then_branch, &mut token_stream);
                    quote!(
                        else {
                            Box::pin(#else_chain) as std::pin::Pin<Box<dyn actix::fut::future::ActorFuture<Self, Output=_>>>
                        }
                    ).to_tokens(&mut token_stream);
                }
            }
            Expr::If(if_expr) => {
                let else_parts = expr_if_inner(if_expr, return_unit);
                if !else_parts.is_empty() {
                    non_awaited_if_expr_for_else(return_unit, cond, then_branch, &mut token_stream);
                    // chained else if(s) have awaits
                    quote!(
                        else #else_parts
                    ).to_tokens(&mut token_stream);
                }
            }
            _ => ()
        }
    }
    token_stream
}

fn non_awaited_if_expr_for_else(return_unit: bool, cond: &Box<Expr>, then_branch: &Block, mut token_stream: &mut TokenStream) {
    if return_unit {
        quote!(
            if #cond {
                Box::pin(actix::fut::ready({ #then_branch; }))
            }
        ).to_tokens(&mut token_stream);
    } else {
        quote!(
            if #cond {
                Box::pin(actix::fut::ready(#then_branch))
            }
        ).to_tokens(&mut token_stream);
    }
}

fn expr_await(parts: &mut Vec<TokenStream>, expr: &ExprAwait) {
    let base = &*expr.base;
    quote!(
        actix::fut::wrap_future::<_, Self>(#base)
    ).to_tokens(parts.last_mut().unwrap());
    parts.push(TokenStream::new());
}

fn expr_for_loop(parts: &mut Vec<TokenStream>, pat: &Box<Pat>, expr: &Box<Expr>, body: &Block, acc: Option<TokenStream>) -> bool {
    let body_parts = split_awaits(body);
    if body_parts.len() > 1 {

        let unpack_acc_prefix = match acc.clone() {
            Some(a) => quote! { let mut #a = __acc; },
            _ => quote! {}
        };

        let acc = acc.or(Some(quote! { () }));
        let body = build_future_chain(body_parts, false, acc.clone());

        quote! {
            use actix::ActorStreamExt;
            actix::fut::wrap_stream(futures::stream::iter(IntoIterator::into_iter(#expr)))
                .fold(#acc, move |__acc, #pat, __self, __ctx| {
                    #unpack_acc_prefix;
                    Box::pin(#body) as std::pin::Pin<Box<dyn actix::fut::future::ActorFuture<Self, Output=_>>>
                })
        }.to_tokens(parts.last_mut().unwrap());
        parts.push(TokenStream::new());
        true
    } else {
        false
    }
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
    fn test_splits_awaits() {
        let block = parse_quote!({
            println!("Before 1");
            println!("Before 2");
            sleep(Duration::from_secs(1)).await;
            println!("After first 1");
            let result = sleep(Duration::from_secs(1)).await;
            result = sleep(Duration::from_secs(1)).await;
            println!("Final 1");
            println!("Final 2");
        });

        let split = split_awaits(&block);
        assert_eq!(split.len(), 4);
    }
}