use crate::{attr, bound};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    parse_quote, Data, DataEnum, DataStruct, DeriveInput, Error, Fields, FieldsNamed, Ident, Result,
};

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
        Data::Enum(enumeration) => derive_enum(&input, enumeration),
        Data::Union(_) => Err(Error::new(
            Span::call_site(),
            "currently only structs and enums are supported by this derive",
        )),
    }
}

fn derive_struct(input: &DeriveInput, fields: &FieldsNamed) -> Result<TokenStream> {
    let ident = &input.ident;
    let info = attr::container_info(&input.attrs)?;
    let (impl_generics, ty_generics, _where_clause) = input.generics.split_for_impl();

    let placements = fields
        .named
        .iter()
        .map(attr::field_info)
        .collect::<Result<Vec<_>>>()?;
    let placements = placements
        .into_iter()
        .map(|f| f.to_dynamic())
        .collect::<Vec<_>>();

    let bound = parse_quote!(wezterm_dynamic::PlaceDynamic);
    let bounded_where_clause = bound::where_clause_with_bound(&input.generics, bound);

    let tokens = match info.into {
        Some(into) => {
            quote!(
            impl #impl_generics wezterm_dynamic::ToDynamic for #ident #ty_generics #bounded_where_clause {
                fn to_dynamic(&self) -> wezterm_dynamic::Value {
                    let target: #into = self.into();
                    target.to_dynamic()
                }
            }
            )
        }
        None => {
            quote!(
            impl #impl_generics wezterm_dynamic::PlaceDynamic for #ident #ty_generics #bounded_where_clause {
                fn place_dynamic(&self, place: &mut wezterm_dynamic::Object) {
                    #(
                        #placements
                    )*
                }
            }

            impl #impl_generics wezterm_dynamic::ToDynamic for #ident #ty_generics #bounded_where_clause {
                fn to_dynamic(&self) -> wezterm_dynamic::Value {
                use wezterm_dynamic::PlaceDynamic;

                let mut object = wezterm_dynamic::Object::default();
                self.place_dynamic(&mut object);
                wezterm_dynamic::Value::Object(object)
                }
            }
            )
        }
    };

    if info.debug {
        eprintln!("{}", tokens);
    }
    Ok(tokens)
}

fn derive_enum(input: &DeriveInput, enumeration: &DataEnum) -> Result<TokenStream> {
    if input.generics.lt_token.is_some() || input.generics.where_clause.is_some() {
        return Err(Error::new(
            Span::call_site(),
            "Enums with generics are not supported",
        ));
    }

    let ident = &input.ident;
    let info = attr::container_info(&input.attrs)?;

    let tokens = match info.into {
        Some(into) => {
            quote! {
                impl wezterm_dynamic::ToDynamic for #ident {
                    fn to_dynamic(&self) -> wezterm_dynamic::Value {
                        let target : #into = self.into();
                        target.to_dynamic()
                    }
                }
            }
        }
        None => {
            let variants = enumeration.variants
            .iter()
            .map(|variant| {
                let ident = &variant.ident;
                let literal = ident.to_string();
                match &variant.fields {
                    Fields::Unit => Ok(quote!(
                        Self::#ident => Value::String(#literal.to_string()),
                    )),
                    Fields::Named(fields) => {
                        let var_fields = fields
                            .named
                            .iter()
                            .map(|f| f.ident.as_ref().unwrap())
                            .collect::<Vec<_>>();
                        let placements = fields
                            .named
                            .iter()
                            .map(|f| {
                                let ident = f.ident.as_ref().unwrap();
                                let name = ident.to_string();
                                quote!(
                                    place.insert(#name.to_dynamic(), #ident.to_dynamic());
                                )
                            })
                            .collect::<Vec<_>>();

                        Ok(quote!(
                            Self::#ident { #( #var_fields, )* } => {
                                let mut place = wezterm_dynamic::Object::default();

                                #( #placements )*

                                let mut obj = wezterm_dynamic::Object::default();
                                obj.insert(#literal.to_dynamic(), Value::Object(place));
                                Value::Object(obj)
                            }
                        ))
                    }
                    Fields::Unnamed(fields) => {
                        let var_fields = fields
                            .unnamed
                            .iter()
                            .enumerate()
                            .map(|(idx, _f)| Ident::new(&format!("f{}", idx), Span::call_site()))
                            .collect::<Vec<_>>();

                        let hint = var_fields.len();

                        if hint == 1 {
                            Ok(quote!(
                                Self::#ident(f) => {
                                    let mut obj = wezterm_dynamic::Object::default();
                                    obj.insert(#literal.to_dynamic(), f.to_dynamic());
                                    Value::Object(obj)
                                }
                            ))
                        } else {
                            let placements = fields
                                .unnamed
                                .iter()
                                .zip(var_fields.iter())
                                .map(|(_f, ident)| {
                                    quote!(
                                        place.push(#ident.to_dynamic());
                                    )
                                })
                                .collect::<Vec<_>>();

                            Ok(quote!(
                                Self::#ident ( #( #var_fields, )* ) => {
                                    let mut place = Vec::with_capacity(#hint);

                                    #( #placements )*

                                    let mut obj = wezterm_dynamic::Object::default();
                                    obj.insert(#literal.to_dynamic(), Value::Array(place.into()));
                                    Value::Object(obj)
                                }
                            ))
                        }
                    }
                }
            })
            .collect::<Result<Vec<_>>>()?;

            quote! {
                impl wezterm_dynamic::ToDynamic for #ident {
                    fn to_dynamic(&self) -> wezterm_dynamic::Value {
                        use wezterm_dynamic::Value;
                        match self {
                            #(
                                #variants
                            )*
                        }
                    }
                }
            }
        }
    };

    if info.debug {
        eprintln!("{}", tokens);
    }
    Ok(tokens)
}
