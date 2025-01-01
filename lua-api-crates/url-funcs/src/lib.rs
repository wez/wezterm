use crate::mlua::UserDataFields;
use config::lua::get_or_create_sub_module;
use config::lua::mlua::{self, Lua, MetaMethod, UserData, UserDataMethods};
use percent_encoding::percent_decode;

pub fn register(lua: &Lua) -> anyhow::Result<()> {
    let url_mod = get_or_create_sub_module(lua, "url")?;

    url_mod.set(
        "parse",
        lua.create_function(|_, s: String| {
            let url = url::Url::parse(&s).map_err(|err| {
                mlua::Error::external(format!("{err:#} while parsing {s} as URL"))
            })?;
            Ok(Url { url })
        })?,
    )?;

    Ok(())
}

#[derive(Clone, Debug)]
pub struct Url {
    pub url: url::Url,
}

impl std::ops::Deref for Url {
    type Target = url::Url;
    fn deref(&self) -> &url::Url {
        &self.url
    }
}

impl std::ops::DerefMut for Url {
    fn deref_mut(&mut self) -> &mut url::Url {
        &mut self.url
    }
}

impl UserData for Url {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::ToString, |_, this, _: ()| {
            Ok(this.url.as_str().to_string())
        });
    }

    fn add_fields<'lua, F: UserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_field_method_get("scheme", |_, this| Ok(this.scheme().to_string()));
        fields.add_field_method_get("username", |_, this| Ok(this.username().to_string()));
        fields.add_field_method_get("password", |_, this| {
            Ok(this.password().map(|s| s.to_string()))
        });
        fields.add_field_method_get("host", |_, this| Ok(this.host_str().map(|s| s.to_string())));
        fields.add_field_method_get("port", |_, this| Ok(this.port()));
        fields.add_field_method_get("query", |_, this| Ok(this.query().map(|s| s.to_string())));
        fields.add_field_method_get("fragment", |_, this| {
            Ok(this.fragment().map(|s| s.to_string()))
        });
        fields.add_field_method_get("path", |_, this| Ok(this.path().to_string()));
        fields.add_field_method_get("file_path", |lua, this| {
            if let Some(segments) = this.path_segments() {
                let mut bytes = vec![];
                for segment in segments {
                    bytes.push(b'/');
                    bytes.extend(percent_decode(segment.as_bytes()));
                }

                // A windows drive letter must end with a slash.
                if bytes.len() > 2
                    && bytes[bytes.len() - 2].is_ascii_alphabetic()
                    && matches!(bytes[bytes.len() - 1], b':' | b'|')
                {
                    bytes.push(b'/');
                }

                let s = lua.create_string(bytes)?;
                Ok(Some(s))
            } else {
                Ok(None)
            }
        });
    }
}
