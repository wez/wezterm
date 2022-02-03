use std::collections::HashMap;

lazy_static::lazy_static! {
    pub static ref NERD_FONTS: HashMap<&'static str, char> = build_map();
}

pub use crate::nerdfonts_data::NERD_FONT_GLYPHS;

fn build_map() -> HashMap<&'static str, char> {
    crate::nerdfonts_data::NERD_FONT_GLYPHS
        .iter()
        .map(|tuple| *tuple)
        .collect()
}
