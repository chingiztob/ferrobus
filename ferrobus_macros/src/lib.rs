//! This macro is used to generate Python stubs for the main library
//! It is kinda a hack, but it will work while `PyO3` does not support
//! automatically generating stubs for all the functions

use proc_macro::TokenStream;
use quote::quote;
use syn::{Item, parse_macro_input};

#[proc_macro_attribute]
pub fn stubgen(_attrs: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as Item);

    // Wrap the item with #[cfg_attr(feature = "stubgen", ...)]
    let output = match input {
        Item::Struct(s) => {
            quote! {
                #[cfg_attr(feature = "stubgen", pyo3_stub_gen::derive::gen_stub_pyclass)]
                #s
            }
        }
        Item::Impl(i) => {
            quote! {
                #[cfg_attr(feature = "stubgen", pyo3_stub_gen::derive::gen_stub_pymethods)]
                #i
            }
        }
        Item::Fn(f) => {
            quote! {
                #[cfg_attr(feature = "stubgen", pyo3_stub_gen::derive::gen_stub_pyfunction)]
                #f
            }
        }
        _ => {
            quote! {
                #input
            }
        }
    };

    output.into()
}
