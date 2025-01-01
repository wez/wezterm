use wezterm_dynamic::Value;

/// Trait for returning metadata about config options
pub trait ConfigMeta {
    fn get_config_options(&self) -> &'static [ConfigOption];
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConfigContainer {
    None,
    Option,
    Vec,
    Map,
}

/// Metadata about a config option
pub struct ConfigOption {
    /// The field name
    pub name: &'static str,
    /// Brief documentation
    pub doc: &'static str,
    /// TODO: tags to categorize the option
    pub tags: &'static [&'static str],
    pub container: ConfigContainer,
    /// The type of the field
    pub type_name: &'static str,
    /// call this to get the default value
    pub default_value: Option<fn() -> Value>,
    /// TODO: For enum types, the set of possible values
    pub possible_values: &'static [&'static Value],
    /// TODO: For struct types, the fields in the child struct
    pub fields: &'static [ConfigOption],
}
