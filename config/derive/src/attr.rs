use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    Attribute, Error, Field, GenericArgument, Ident, Lit, Meta, NestedMeta, Path, PathArguments,
    Result, Type,
};

#[allow(unused)]
pub struct ContainerInfo {
    pub into: Option<Path>,
    pub try_from: Option<Path>,
    pub debug: bool,
}

pub fn container_info(attrs: &[Attribute]) -> Result<ContainerInfo> {
    let mut into = None;
    let mut try_from = None;
    let mut debug = false;

    for attr in attrs {
        if !attr.path.is_ident("dynamic") {
            continue;
        }

        let list = match attr.parse_meta()? {
            Meta::List(list) => list,
            other => return Err(Error::new_spanned(other, "unsupported attribute")),
        };

        for meta in &list.nested {
            match meta {
                NestedMeta::Meta(Meta::Path(path)) => {
                    if path.is_ident("debug") {
                        debug = true;
                        continue;
                    }
                }
                NestedMeta::Meta(Meta::NameValue(value)) => {
                    if value.path.is_ident("into") {
                        if let Lit::Str(s) = &value.lit {
                            into = Some(s.parse()?);
                            continue;
                        }
                    }
                    if value.path.is_ident("try_from") {
                        if let Lit::Str(s) = &value.lit {
                            try_from = Some(s.parse()?);
                            continue;
                        }
                    }
                }
                _ => {}
            }
            return Err(Error::new_spanned(meta, "unsupported attribute"));
        }
    }

    Ok(ContainerInfo {
        into,
        try_from,
        debug,
    })
}

pub enum DefValue {
    None,
    Default,
    Path(Path),
}

#[allow(unused)]
pub struct FieldInfo<'a> {
    pub field: &'a Field,
    pub type_name: String,
    pub name: String,
    pub skip: bool,
    pub flatten: bool,
    pub allow_default: DefValue,
    pub into: Option<Path>,
    pub try_from: Option<Path>,
    pub deprecated: Option<String>,
    pub validate: Option<Path>,
    pub doc: String,
    pub container_type: ContainerType,
}

#[derive(Debug)]
pub enum ContainerType {
    None,
    Option,
    Vec,
    Map,
}

impl<'a> FieldInfo<'a> {
    pub fn to_option(&self) -> TokenStream {
        let name = &self.name;
        let doc = &self.doc;
        let type_name = &self.type_name;
        let container_type = Ident::new(&format!("{:?}", self.container_type), Span::call_site());
        let get_default = match self.compute_default() {
            Some(def) => quote!(Some(|| #def.to_dynamic())),
            None => quote!(None),
        };
        quote!(
            crate::meta::ConfigOption {
                name: #name,
                doc: #doc,
                tags: &[],
                container: crate::meta::ConfigContainer::#container_type,
                type_name: #type_name,
                default_value: #get_default,
                possible_values: &[],
                fields: &[],
            }
        )
    }

    fn compute_default(&self) -> Option<TokenStream> {
        let ty = &self.field.ty;
        match &self.allow_default {
            DefValue::Default => Some(quote!(
                <#ty>::default()
            )),
            DefValue::Path(default) => Some(quote!(
                #default()
            )),
            DefValue::None => None,
        }
    }
}

pub fn field_info(field: &Field) -> Result<FieldInfo> {
    let mut name = field.ident.as_ref().unwrap().to_string();
    let mut skip = false;
    let mut flatten = false;
    let mut allow_default = DefValue::None;
    let mut try_from = None;
    let mut validate = None;
    let mut into = None;
    let mut deprecated = None;
    let mut doc = String::new();
    let mut container_type = ContainerType::None;

    let type_name = match &field.ty {
        Type::Path(p) => {
            let last_seg = p.path.segments.last().unwrap();
            match &last_seg.arguments {
                PathArguments::None => last_seg.ident.to_string(),
                PathArguments::AngleBracketed(args) if args.args.len() == 1 => {
                    let arg = args.args.first().unwrap();
                    match arg {
                        GenericArgument::Type(Type::Path(t)) => {
                            container_type = match last_seg.ident.to_string().as_str() {
                                "Option" => ContainerType::Option,
                                "Vec" => ContainerType::Vec,
                                _ => panic!("unhandled type for {name}: {:#?}", field.ty),
                            };
                            t.path.segments.last().unwrap().ident.to_string()
                        }
                        _ => panic!("unhandled type for {name}: {:#?}", field.ty),
                    }
                }
                PathArguments::AngleBracketed(args) if args.args.len() == 2 => {
                    let arg = args.args.last().unwrap();
                    match arg {
                        GenericArgument::Type(Type::Path(t)) => {
                            container_type = match last_seg.ident.to_string().as_str() {
                                "HashMap" => ContainerType::Map,
                                _ => panic!("unhandled type for {name}: {:#?}", field.ty),
                            };
                            t.path.segments.last().unwrap().ident.to_string()
                        }
                        _ => panic!("unhandled type for {name}: {:#?}", field.ty),
                    }
                }
                _ => panic!("unhandled type for {name}: {:#?}", field.ty),
            }
        }
        _ => panic!("unhandled type for {name}: {:#?}", field.ty),
    };

    for attr in &field.attrs {
        if !attr.path.is_ident("dynamic") && !attr.path.is_ident("doc") {
            continue;
        }

        let list = match attr.parse_meta()? {
            Meta::List(list) => list,
            Meta::NameValue(value) if value.path.is_ident("doc") => {
                if let Lit::Str(s) = &value.lit {
                    if !doc.is_empty() {
                        doc.push('\n');
                    }
                    doc.push_str(&s.value());
                }
                continue;
            }
            other => {
                return Err(Error::new_spanned(
                    other.clone(),
                    format!("unsupported attribute {other:?}"),
                ))
            }
        };

        for meta in &list.nested {
            match meta {
                NestedMeta::Meta(Meta::NameValue(value)) => {
                    if value.path.is_ident("rename") {
                        if let Lit::Str(s) = &value.lit {
                            name = s.value();
                            continue;
                        }
                    }
                    if value.path.is_ident("default") {
                        if let Lit::Str(s) = &value.lit {
                            allow_default = DefValue::Path(s.parse()?);
                            continue;
                        }
                    }
                    if value.path.is_ident("deprecated") {
                        if let Lit::Str(s) = &value.lit {
                            deprecated.replace(s.value());
                            continue;
                        }
                    }
                    if value.path.is_ident("into") {
                        if let Lit::Str(s) = &value.lit {
                            into = Some(s.parse()?);
                            continue;
                        }
                    }
                    if value.path.is_ident("try_from") {
                        if let Lit::Str(s) = &value.lit {
                            try_from = Some(s.parse()?);
                            continue;
                        }
                    }
                    if value.path.is_ident("validate") {
                        if let Lit::Str(s) = &value.lit {
                            validate = Some(s.parse()?);
                            continue;
                        }
                    }
                }
                NestedMeta::Meta(Meta::Path(path)) => {
                    if path.is_ident("skip") {
                        skip = true;
                        continue;
                    }
                    if path.is_ident("flatten") {
                        flatten = true;
                        continue;
                    }
                    if path.is_ident("default") {
                        allow_default = DefValue::Default;
                        continue;
                    }
                }
                _ => {}
            }
            return Err(Error::new_spanned(meta, "unsupported attribute"));
        }
    }

    Ok(FieldInfo {
        type_name,
        field,
        name,
        skip,
        flatten,
        allow_default,
        try_from,
        into,
        deprecated,
        validate,
        doc,
        container_type,
    })
}
