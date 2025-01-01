use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod attr;
mod bound;
mod fromdynamic;
mod todynamic;

#[proc_macro_derive(ToDynamic, attributes(dynamic))]
pub fn derive_todynamic(input: TokenStream) -> TokenStream {
    todynamic::derive(parse_macro_input!(input as DeriveInput))
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

#[proc_macro_derive(FromDynamic, attributes(dynamic))]
pub fn derive_fromdynamic(input: TokenStream) -> TokenStream {
    fromdynamic::derive(parse_macro_input!(input as DeriveInput))
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}
