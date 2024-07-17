extern crate proc_macro;
#[macro_use]
extern crate syn;

use proc_macro::TokenStream;

use crate::r#impl::async_handler_impl;

mod r#impl;

#[proc_macro_attribute]
pub fn async_handler(attribute: TokenStream, input: TokenStream) -> TokenStream {

    let attribute = parse_macro_input!(attribute);
    let input = parse_macro_input!(input);
    async_handler_impl(attribute, input).into()

}