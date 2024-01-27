use config::{ColorSchemeFile, ColorSchemeMetaData, Palette, RgbaColor};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct GoghTheme {
    name: String,
    #[serde(rename = "color_01")]
    black: String,
    #[serde(rename = "color_02")]
    red: String,
    #[serde(rename = "color_03")]
    green: String,
    #[serde(rename = "color_04")]
    yellow: String,
    #[serde(rename = "color_05")]
    blue: String,
    #[serde(rename = "color_06")]
    purple: String,
    #[serde(rename = "color_07")]
    cyan: String,
    #[serde(rename = "color_08")]
    white: String,
    #[serde(rename = "color_09")]
    brightBlack: String,
    #[serde(rename = "color_10")]
    brightRed: String,
    #[serde(rename = "color_11")]
    brightGreen: String,
    #[serde(rename = "color_12")]
    brightYellow: String,
    #[serde(rename = "color_13")]
    brightBlue: String,
    #[serde(rename = "color_14")]
    brightPurple: String,
    #[serde(rename = "color_15")]
    brightCyan: String,
    #[serde(rename = "color_16")]
    brightWhite: String,
    foreground: String,
    background: String,
    #[serde(rename = "cursor")]
    cursorColor: String,
}

impl GoghTheme {
    pub fn load_all(slice: &[u8]) -> anyhow::Result<Vec<ColorSchemeFile>> {
        let data: Vec<GoghTheme> = serde_json::from_slice(slice)?;
        let mut schemes = vec![];
        for s in data {
            let cursor = RgbaColor::try_from(s.cursorColor)?;
            let name = s.name;
            let background = RgbaColor::try_from(s.background)?;

            schemes.push(ColorSchemeFile {
                colors: Palette {
                    foreground: Some(RgbaColor::try_from(s.foreground)?),
                    background: Some(background),
                    cursor_fg: Some(background),
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
