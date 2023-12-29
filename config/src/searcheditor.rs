use std::collections::HashMap;
use wezterm_dynamic::{FromDynamic, ToDynamic};

#[derive(Debug, Copy, Clone, FromDynamic, ToDynamic, PartialEq, Default)]
pub enum SearchEditorLocation {
    #[default]
    NewTab,
    NewWindow,
}

#[derive(Default, Debug, Clone, PartialEq, FromDynamic, ToDynamic)]
pub struct SearchEditor {
    pub editor: Vec<String>,
    pub location: SearchEditorLocation,
    pub environment: HashMap<String, String>,
}
