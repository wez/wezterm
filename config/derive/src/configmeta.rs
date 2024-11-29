use crate::{attr, bound};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{parse_quote, Data, DataStruct, DeriveInput, Error, Fields, FieldsNamed, Result};

pub fn derive(input: DeriveInput) -> Result<TokenStream> {
    match &input.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => derive_struct(&input, fields),
        Data::Struct(_) => Err(Error::new(
            Span::call_site(),
            "currently only structs with named fields are supported",
        )),
        _ => Err(Error::new(
            Span::call_site(),
            "currently only structs and enums are supported by this derive",
        )),
    }
}

fn derive_struct(input: &DeriveInput, fields: &FieldsNamed) -> Result<TokenStream> {
    let info = attr::container_info(&input.attrs)?;
    let ident = &input.ident;
    let (impl_generics, ty_generics, _where_clause) = input.generics.split_for_impl();

    let options = fields
        .named
        .iter()
        .map(attr::field_info)
        .collect::<Result<Vec<_>>>()?;

    let options = options
        .into_iter()
        .filter_map(|f| if f.skip { None } else { Some(f.to_option()) })
        .collect::<Vec<_>>();

    let bound = parse_quote!(crate::ConfigMeta);
    let bounded_where_clause = bound::where_clause_with_bound(&input.generics, bound);

    let tokens = quote! {
        impl #impl_generics crate::meta::ConfigMeta for #ident #ty_generics #bounded_where_clause {
            fn get_config_options(&self) -> &'static [crate::meta::ConfigOption]
            {
                &[
                    #( #options, )*
                ]
            }
        }
    };

    if info.debug {
        eprintln!("{}", tokens);
    }
    Ok(tokens)
}
