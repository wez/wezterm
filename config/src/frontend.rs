use wezterm_dynamic::{FromDynamic, ToDynamic};

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromDynamic, ToDynamic)]
pub enum FrontEndSelection {
    OpenGL,
    WebGpu,
    Software,
}

impl Default for FrontEndSelection {
    fn default() -> Self {
        FrontEndSelection::OpenGL
    }
}
