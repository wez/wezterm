use crate::{attr, bound};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    parse_quote, Data, DataEnum, DataStruct, DeriveInput, Error, Fields, FieldsNamed, Result,
};

pub fn derive(input: DeriveInput) -> Result<TokenStream> {
    match &input.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => derive_struct(&input, fields),
        Data::Enum(enumeration) => derive_enum(&input, enumeration),
        Data::Struct(_) => Err(Error::new(
            Span::call_site(),
            "currently only structs with named fields are supported",
        )),
        Data::Union(_) => Err(Error::new(
            Span::call_site(),
            "currently only structs and enums are supported by this derive",
        )),
    }
}

fn derive_struct(input: &DeriveInput, fields: &FieldsNamed) -> Result<TokenStream> {
    let info = attr::container_info(&input.attrs)?;
    let ident = &input.ident;
    let literal = ident.to_string();
    let (impl_generics, ty_generics, _where_clause) = input.generics.split_for_impl();

    let placements = fields
        .named
        .iter()
        .map(attr::field_info)
        .collect::<Result<Vec<_>>>()?;
    let needs_default = placements.iter().any(|f| f.skip);
    let field_names = placements
        .iter()
        .filter_map(|f| {
            if f.skip || f.flatten {
                None
            } else {
                Some(f.name.to_string())
            }
        })
        .collect::<Vec<_>>();

    // If any of the fields are flattened, then we don't have enough
    // structure in the FromDynamic interface to know precisely which
    // fields were legitimately used by any recursively flattened item,
    // or, in the recursive item, to know which of the fields were used
    // by the parent.
    // We need to disable warning or raising errors for unknown fields
    // in that case to avoid false positives.
    let adjust_options = if placements.iter().any(|f| f.flatten) {
        quote!(let options = options.flatten();)
    } else {
        quote!()
    };

    let field_names = quote!(
        &[ #( #field_names, )* ]
    );

    let placements = placements
        .into_iter()
        .map(|f| f.from_dynamic(&literal))
        .collect::<Vec<_>>();

    let bound = parse_quote!(wezterm_dynamic::FromDynamic);
    let bounded_where_clause = bound::where_clause_with_bound(&input.generics, bound);

    let obj = if needs_default {
        quote!(
            Ok(Self {
                #(
                    #placements
                )*
                .. Self::default()
            })
        )
    } else {
        quote!(
            Ok(Self {
                #(
                    #placements
                )*
            })
        )
    };

    let from_dynamic = match info.try_from {
        Some(try_from) => {
            quote!(
                use std::convert::TryFrom;
                let target = <#try_from>::from_dynamic(value, options)?;
                <#ident>::try_from(target).map_err(|e| wezterm_dynamic::Error::Message(format!("{:#}", e)))
            )
        }
        None => {
            quote!(
                match value {
                    Value::Object(obj) => {
                        wezterm_dynamic::Error::raise_unknown_fields(options, #literal, &obj, Self::possible_field_names())?;
                        #obj
                    }
                    other => Err(wezterm_dynamic::Error::NoConversion {
                        source_type: other.variant_name().to_string(),
                        dest_type: #literal
                    }),
                }
            )
        }
    };

    let tokens = quote! {
        impl #impl_generics wezterm_dynamic::FromDynamic for #ident #ty_generics #bounded_where_clause {
            fn from_dynamic(value: &wezterm_dynamic::Value, options: wezterm_dynamic::FromDynamicOptions) -> std::result::Result<Self, wezterm_dynamic::Error> {
                use wezterm_dynamic::{Value, BorrowedKey, ObjectKeyTrait};
                #adjust_options
                #from_dynamic
            }

        }
        impl #impl_generics #ident #ty_generics #bounded_where_clause {
            pub const fn possible_field_names() -> &'static [&'static str] {
                #field_names
            }
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
    let info = attr::container_info(&input.attrs)?;

    let ident = &input.ident;
    let literal = ident.to_string();

    let variant_names = enumeration
        .variants
        .iter()
        .map(|variant| variant.ident.to_string())
        .collect::<Vec<_>>();

    let from_dynamic = match info.try_from {
        Some(try_from) => {
            quote!(
                use std::convert::TryFrom;
                let target = <#try_from>::from_dynamic(value, options)?;
                <#ident>::try_from(target).map_err(|e| wezterm_dynamic::Error::Message(format!("{:#}", e)))
            )
        }
        None => {
            let units = enumeration
                .variants
                .iter()
                .filter_map(|variant| match &variant.fields {
                    Fields::Unit => {
                        let ident = &variant.ident;
                        let literal = ident.to_string();
                        Some(quote!(
                        #literal => {
                            return Ok(Self::#ident);
                        }
                        ))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();

            let variants = enumeration.variants.iter().map(|variant| {
                let ident = &variant.ident;
                let literal = ident.to_string();

                match &variant.fields {
                    Fields::Unit => {
                        // Already handled separately
                        quote!()
                    }
                    Fields::Named(fields) => {
                        let var_fields = fields
                            .named
                            .iter()
                            .map(|f| {
                                let info = attr::field_info(f).unwrap();
                                info.from_dynamic(&literal)
                            })
                        .collect::<Vec<_>>();

                        quote!(
                            #literal => {
                                match value {
                                    Value::Object(obj) => {
                                        Ok(Self::#ident {
                                            #( #var_fields )*
                                        })
                                    }
                                    other => return Err(wezterm_dynamic::Error::NoConversion {
                                        source_type: other.variant_name().to_string(),
                                        dest_type: "Object",
                                    }),
                                }
                            }
                            )
                    }
                    Fields::Unnamed(fields) => {
                        if fields.unnamed.len() == 1 {
                            let ty = fields.unnamed.iter().map(|f| &f.ty).next().unwrap();
                            quote!(
                                #literal => {
                                    Ok(Self::#ident(<#ty>::from_dynamic(value, options)?))
                                }
                                )
                        } else {
                            let var_fields = fields
                                .unnamed
                                .iter()
                                .enumerate()
                                .map(|(idx, f)| {
                                    let ty = &f.ty;
                                    quote!(
                                        <#ty>::from_dynamic(
                                            arr.get(#idx)
                                            .ok_or_else(|| wezterm_dynamic::Error::Message(
                                                format!("missing idx {} of enum struct {}", #idx, #literal)))?,
                                            options
                                            )?,
                                    )
                                })
                            .collect::<Vec<_>>();
                            quote!(
                                #literal => {
                                    match value {
                                        Value::Array(arr) => {
                                            Ok(Self::#ident (
                                                #( #var_fields )*
                                            ))
                                        }
                                        other => return Err(wezterm_dynamic::Error::NoConversion {
                                            source_type: other.variant_name().to_string(),
                                            dest_type: "Array",
                                        }),
                                    }
                                }
                                )
                        }
                    }
                }
            }).collect::<Vec<_>>();

            quote!(
                    match value {
                        Value::String(s) => {
                            match s.as_str() {
                                #( #units )*
                                _ => Err(wezterm_dynamic::Error::InvalidVariantForType {
                                    variant_name: s.clone(),
                                    type_name: #literal,
                                    possible: #ident::variants(),
                                })
                            }
                        }
                        Value::Object(place) => {
                            if place.len() == 1 {
                                let (name, value) : (&Value, &Value) = place.iter().next().unwrap();

                                match name {
                                    Value::String(name) => {
                                        match name.as_str() {
                                            #( #variants )*
                                            _ => Err(wezterm_dynamic::Error::InvalidVariantForType {
                                                variant_name: name.to_string(),
                                                type_name: #literal,
                                                possible: #ident::variants(),
                                            })
                                        }
                                    }
                                    _ => Err(wezterm_dynamic::Error::InvalidVariantForType {
                                        variant_name: name.variant_name().to_string(),
                                        type_name: #literal,
                                        possible: #ident::variants(),
                                    })
                                }
                            } else {
                                Err(wezterm_dynamic::Error::IncorrectNumberOfEnumKeys {
                                    type_name: #literal,
                                    num_keys: place.len(),
                                })
                            }
                        }
                        other => Err(wezterm_dynamic::Error::NoConversion {
                            source_type: other.variant_name().to_string(),
                            dest_type: #literal
                        }),
                    }
            )
        }
    };

    let tokens = quote! {
        impl wezterm_dynamic::FromDynamic for #ident {
            fn from_dynamic(value: &wezterm_dynamic::Value, options: wezterm_dynamic::FromDynamicOptions) -> std::result::Result<Self, wezterm_dynamic::Error> {
                use wezterm_dynamic::{Value, BorrowedKey, ObjectKeyTrait};
                #from_dynamic
            }
        }

        impl #ident {
            pub fn variants() -> &'static [&'static str] {
                &[
                    #( #variant_names, )*
                ]
            }
        }
    };

    if info.debug {
        eprintln!("{}", tokens);
    }
    Ok(tokens)
}
