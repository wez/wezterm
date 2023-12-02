use wezterm_dynamic::{ToDynamic, FromDynamic};


#[derive(Debug, Default, Clone, ToDynamic, PartialEq, Eq, FromDynamic)]
pub enum WindowLevel {
    AlwaysOnBottom = -1,
    #[default]
    Normal = 0,
    AlwaysOnTop = 3,
}

