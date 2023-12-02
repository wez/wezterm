use wezterm_dynamic::{ToDynamic, FromDynamic};


#[derive(Debug, Clone, ToDynamic, PartialEq, Eq, FromDynamic)]
pub enum WindowLevel {
    AlwaysOnBottom = -1,
    Normal = 0,
    AlwaysOnTop = 3,
}

impl Default for WindowLevel {
    fn default() -> Self {
        WindowLevel::Normal
    }
}

impl From<i64> for WindowLevel {
    fn from(level: i64) -> Self {
        match level {
            -1 => WindowLevel::AlwaysOnBottom,
            0 => WindowLevel::Normal,
            3 => WindowLevel::AlwaysOnTop,
            _ => panic!("Invalid window level: {}", level),
        }
    }
}
