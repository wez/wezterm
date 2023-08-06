use crate::ScreenRect;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Screens {
    pub main: ScreenInfo,
    pub active: ScreenInfo,
    pub by_name: HashMap<String, ScreenInfo>,
    pub virtual_rect: ScreenRect,
}

#[derive(Debug, Clone)]
pub struct ScreenInfo {
    pub name: String,
    pub rect: ScreenRect,
    pub scale: f64,
    pub max_fps: Option<usize>,
    pub effective_dpi: Option<f64>,
}
