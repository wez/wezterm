use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod attr;
mod bound;
mod configmeta;

#[proc_macro_derive(ConfigMeta, attributes(config))]
pub fn derive_config(input: TokenStream) -> TokenStream {
    configmeta::derive(parse_macro_input!(input as DeriveInput))
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}
