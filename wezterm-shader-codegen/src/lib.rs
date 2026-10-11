use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Ident};

/// Reflects struct fields as `const UNIFORM_FIELDS: &[UniformField]`.
///
/// Each field needs `#[uniform_type(Vec2|Float|UInt|Vec4)]` or `#[uniform_ignore]`.
#[proc_macro_derive(UniformBuffer, attributes(uniform_type, uniform_ignore))]
pub fn derive_uniform_buffer(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    match expand(&ast) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn expand(ast: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let fields = match &ast.data {
        Data::Struct(s) => &s.fields,
        _ => {
            return Err(syn::Error::new_spanned(
                ast,
                "UniformBuffer only supports structs",
            ))
        }
    };

    let mut reflected: Vec<proc_macro2::TokenStream> = Vec::new();

    for field in fields.iter() {
        let field_name = field
            .ident
            .as_ref()
            .ok_or_else(|| syn::Error::new_spanned(field, "UniformBuffer requires named fields"))?;

        if field
            .attrs
            .iter()
            .any(|a| a.path.is_ident("uniform_ignore"))
        {
            continue;
        }

        let uniform_type = field
            .attrs
            .iter()
            .find(|a| a.path.is_ident("uniform_type"))
            .ok_or_else(|| {
                syn::Error::new_spanned(
                    field,
                    "field must have #[uniform_type(...)] or #[uniform_ignore]",
                )
            })?;
        let variant: Ident = uniform_type.parse_args()?;

        reflected.push(quote! {
            wezterm_shader_types::UniformField {
                name: stringify!(#field_name),
                ty: wezterm_shader_types::UniformType::#variant,
            }
        });
    }

    Ok(quote! {
        pub const UNIFORM_FIELDS: &[wezterm_shader_types::UniformField] = &[
            #(#reflected),*
        ];
    })
}
