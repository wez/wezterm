use crate::config::{Config, FontAttributes, TextStyle};
use failure::{format_err, Error};
use font_loader::system_fonts;

pub fn load_system_fonts(
    _config: &Config,
    style: &TextStyle,
) -> Result<Vec<((Vec<u8>, i32), FontAttributes)>, Error> {
    let mut fonts = Vec::new();
    for font_attr in style.font_with_fallback() {
        let mut font_props = system_fonts::FontPropertyBuilder::new()
            .family(&font_attr.family)
            .monospace();
        font_props = if font_attr.bold {
            font_props.bold()
        } else {
            font_props
        };
        font_props = if font_attr.italic {
            font_props.italic()
        } else {
            font_props
        };
        let font_props = font_props.build();

        fonts.push((
            system_fonts::get(&font_props)
                .ok_or_else(|| format_err!("no font matching {:?}", font_attr))?,
            font_attr,
        ));
    }
    Ok(fonts)
}
