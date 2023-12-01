use wezterm_dynamic::{ToDynamic, FromDynamic};


#[derive(Debug, Clone, ToDynamic, PartialEq, Eq, FromDynamic)]
pub enum WindowLevel {
    AlwaysOnBottom = -1,
    Normal = 0,
    AlwaysOnTop = 3,
}
