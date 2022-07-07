use luahelper::impl_lua_conversion_dynamic;
use wezterm_dynamic::{FromDynamic, ToDynamic};

#[derive(Default, Debug, Clone, FromDynamic, ToDynamic)]
pub struct ExecDomain {
    pub name: String,
    pub event_name: String,
}
impl_lua_conversion_dynamic!(ExecDomain);
