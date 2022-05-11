use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, Error, Field, Lit, Meta, NestedMeta, Path, Result};

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

pub struct FieldInfo<'a> {
    pub field: &'a Field,
    pub name: String,
    pub skip: bool,
    pub flatten: bool,
    pub allow_default: DefValue,
    pub into: Option<Path>,
    pub try_from: Option<Path>,
}

impl<'a> FieldInfo<'a> {
    pub fn to_dynamic(&self) -> TokenStream {
        let name = &self.name;
        let ident = &self.field.ident;
        if self.skip {
            quote!()
        } else if self.flatten {
            quote!(
                self.#ident.place_dynamic(place);
            )
        } else if let Some(into) = &self.into {
            quote!(
                let target : #into = (&self.#ident).into();
                place.insert(#name.to_dynamic(), target.to_dynamic());
            )
        } else {
            quote!(
                place.insert(#name.to_dynamic(), self.#ident.to_dynamic());
            )
        }
    }

    pub fn from_dynamic(&self, struct_name: &str) -> TokenStream {
        let name = &self.name;
        let ident = &self.field.ident;
        let ty = &self.field.ty;
        if self.skip {
            quote!()
        } else if self.flatten {
            quote!(
                #ident:
                    <#ty>::from_dynamic(value, options)
                            .map_err(|source| wezterm_dynamic::Error::ErrorInField {
                                type_name: #struct_name,
                                field_name: #name,
                                error: source.to_string(),
                            })?,
            )
        } else if let Some(try_from) = &self.try_from {
            match &self.allow_default {
                DefValue::Default => {
                    quote!(
                        #ident: match obj.get_by_str(#name) {
                            Some(v) => {
                                use std::convert::TryFrom;
                                let target = <#try_from>::from_dynamic(v, options)
                                    .map_err(|source| wezterm_dynamic::Error::ErrorInField {
                                        type_name: #struct_name,
                                        field_name: #name,
                                        error: source.to_string(),
                                    })?;
                                <#ty>::try_from(target)
                                    .map_err(|source| wezterm_dynamic::Error::ErrorInField {
                                        type_name: #struct_name,
                                        field_name: #name,
                                        error: source.to_string(),
                                    })?
                            }
                            None => {
                                <#ty>::default()
                            }
                        },
                    )
                }
                DefValue::Path(default) => {
                    quote!(
                        #ident: match obj.get_by_str(&#name) {
                            Some(v) => {
                                use std::convert::TryFrom;
                                let target = <#try_from>::from_dynamic(v, options)
                                    .map_err(|source| wezterm_dynamic::Error::ErrorInField {
                                        type_name: #struct_name,
                                        field_name: #name,
                                        error: source.to_string(),
                                    })?;
                                <#ty>::try_from(target)
                                    .map_err(|source| wezterm_dynamic::Error::ErrorInField {
                                        type_name: #struct_name,
                                        field_name: #name,
                                        error: source.to_string(),
                                    })?
                            }
                            None => {
                                #default()
                            }
                        },
                    )
                }
                DefValue::None => {
                    quote!(
                        #ident: {
                            use std::convert::TryFrom;
                            let target = <#try_from>::from_dynamic(obj.get_by_str(#name).unwrap_or(&Value::Null), options)
                                    .map_err(|source| wezterm_dynamic::Error::ErrorInField {
                                        type_name: #struct_name,
                                        field_name: #name,
                                        error: source.to_string(),
                                    })?;
                            <#ty>::try_from(target)
                                    .map_err(|source| wezterm_dynamic::Error::ErrorInField {
                                        type_name: #struct_name,
                                        field_name: #name,
                                        error: source.to_string(),
                                    })?
                        },
                    )
                }
            }
        } else {
            match &self.allow_default {
                DefValue::Default => {
                    quote!(
                        #ident: match obj.get_by_str(#name) {
                            Some(v) => {
                                <#ty>::from_dynamic(v, options)
                                    .map_err(|source| wezterm_dynamic::Error::ErrorInField {
                                        type_name: #struct_name,
                                        field_name: #name,
                                        error: source.to_string(),
                                    })?
                            }
                            None => {
                                <#ty>::default()
                            }
                        },
                    )
                }
                DefValue::Path(default) => {
                    quote!(
                        #ident: match obj.get_by_str(#name) {
                            Some(v) => {
                                <#ty>::from_dynamic(v, options)
                                    .map_err(|source| wezterm_dynamic::Error::ErrorInField {
                                        type_name: #struct_name,
                                        field_name: #name,
                                        error: source.to_string(),
                                    })?
                            }
                            None => {
                                #default()
                            }
                        },
                    )
                }
                DefValue::None => {
                    quote!(
                        #ident:
                            <#ty>::from_dynamic(
                                    obj.get_by_str(#name).unwrap_or(&Value::Null),
                                    options
                                )
                                    .map_err(|source| wezterm_dynamic::Error::ErrorInField {
                                        type_name: #struct_name,
                                        field_name: #name,
                                        error: source.to_string(),
                                    })?,
                    )
                }
            }
        }
    }
}

pub fn field_info(field: &Field) -> Result<FieldInfo> {
    let mut name = field.ident.as_ref().unwrap().to_string();
    let mut skip = false;
    let mut flatten = false;
    let mut allow_default = DefValue::None;
    let mut try_from = None;
    let mut into = None;

    for attr in &field.attrs {
        if !attr.path.is_ident("dynamic") {
            continue;
        }

        let list = match attr.parse_meta()? {
            Meta::List(list) => list,
            other => return Err(Error::new_spanned(other, "unsupported attribute")),
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
        field,
        name,
        skip,
        flatten,
        allow_default,
        try_from,
        into,
    })
}
