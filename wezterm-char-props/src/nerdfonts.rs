#[cfg(feature = "std")]
use std::collections::HashMap;
#[cfg(feature = "std")]
use std::sync::LazyLock;

#[cfg(feature = "std")]
pub static NERD_FONTS: LazyLock<HashMap<&'static str, char>> = LazyLock::new(build_map);

pub use crate::nerdfonts_data::NERD_FONT_GLYPHS;

#[cfg(feature = "std")]
fn build_map() -> HashMap<&'static str, char> {
    crate::nerdfonts_data::NERD_FONT_GLYPHS
        .iter()
        .map(|tuple| *tuple)
        .collect()
}
