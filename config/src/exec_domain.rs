use crate::config::validate_domain_name;
use luahelper::impl_lua_conversion_dynamic;
use wezterm_dynamic::{FromDynamic, ToDynamic, Value};

#[derive(Debug, Clone, FromDynamic, ToDynamic)]
pub enum ValueOrFunc {
    Value(Value),
    Func(String),
}
impl_lua_conversion_dynamic!(ValueOrFunc);

#[derive(Debug, Clone, FromDynamic, ToDynamic)]
pub struct ExecDomain {
    #[dynamic(validate = "validate_domain_name")]
    pub name: String,
    pub fixup_command: String,
    pub label: Option<ValueOrFunc>,
}
impl_lua_conversion_dynamic!(ExecDomain);
