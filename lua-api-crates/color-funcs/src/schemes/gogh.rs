use config::{ColorSchemeFile, ColorSchemeMetaData, Palette, RgbaColor};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct GoghTheme {
    name: String,
    black: String,
    red: String,
    green: String,
    yellow: String,
    blue: String,
    purple: String,
    cyan: String,
    white: String,
    brightBlack: String,
    brightRed: String,
    brightGreen: String,
    brightYellow: String,
    brightBlue: String,
    brightPurple: String,
    brightCyan: String,
    brightWhite: String,
    foreground: String,
    background: String,
    cursorColor: String,
}

impl GoghTheme {
    pub fn load_all(slice: &[u8]) -> anyhow::Result<Vec<ColorSchemeFile>> {
        #[derive(Deserialize, Debug)]
        struct Themes {
            themes: Vec<GoghTheme>,
        }

        let data: Themes = serde_json::from_slice(slice)?;
        let mut schemes = vec![];
        for s in data.themes {
            let cursor = RgbaColor::try_from(s.cursorColor)?;
            let name = s.name;

            schemes.push(ColorSchemeFile {
                colors: Palette {
                    foreground: Some(RgbaColor::try_from(s.foreground)?),
                    background: Some(RgbaColor::try_from(s.background)?),
                    cursor_fg: Some(cursor),
                    cursor_bg: Some(cursor),
                    cursor_border: Some(cursor),
                    ansi: Some([
                        RgbaColor::try_from(s.black)?,
                        RgbaColor::try_from(s.red)?,
                        RgbaColor::try_from(s.green)?,
                        RgbaColor::try_from(s.yellow)?,
                        RgbaColor::try_from(s.blue)?,
                        RgbaColor::try_from(s.purple)?,
                        RgbaColor::try_from(s.cyan)?,
                        RgbaColor::try_from(s.white)?,
                    ]),
                    brights: Some([
                        RgbaColor::try_from(s.brightBlack)?,
                        RgbaColor::try_from(s.brightRed)?,
                        RgbaColor::try_from(s.brightGreen)?,
                        RgbaColor::try_from(s.brightYellow)?,
                        RgbaColor::try_from(s.brightBlue)?,
                        RgbaColor::try_from(s.brightPurple)?,
                        RgbaColor::try_from(s.brightCyan)?,
                        RgbaColor::try_from(s.brightWhite)?,
                    ]),
                    ..Default::default()
                },
                metadata: ColorSchemeMetaData {
                    name: Some(name.clone()),
                    author: None,
                    origin_url: None,
                    wezterm_version: None,
                    aliases: vec![],
                },
            })
        }
        Ok(schemes)
    }
}
