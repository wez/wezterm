use anyhow::Context;
use config::{ColorSchemeFile, ColorSchemeMetaData, Palette, RgbaColor, SrgbaTuple};
use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize, Debug)]
struct Color {
    #[serde(rename = "Red Component")]
    red: f32,
    #[serde(rename = "Green Component")]
    green: f32,
    #[serde(rename = "Blue Component")]
    blue: f32,
}

impl From<Color> for RgbaColor {
    fn from(val: Color) -> Self {
        // For compatibility with `iterm2xrdb`, we round these
        // values off :-/
        fn compat(v: f32) -> f32 {
            (v * 255.).round() / 255.
        }
        SrgbaTuple(compat(val.red), compat(val.green), compat(val.blue), 1.0).into()
    }
}

#[derive(Deserialize, Debug)]
pub struct ITerm2 {
    #[serde(rename = "Ansi 0 Color")]
    ansi_0: Color,
    #[serde(rename = "Ansi 1 Color")]
    ansi_1: Color,
    #[serde(rename = "Ansi 2 Color")]
    ansi_2: Color,
    #[serde(rename = "Ansi 3 Color")]
    ansi_3: Color,
    #[serde(rename = "Ansi 4 Color")]
    ansi_4: Color,
    #[serde(rename = "Ansi 5 Color")]
    ansi_5: Color,
    #[serde(rename = "Ansi 6 Color")]
    ansi_6: Color,
    #[serde(rename = "Ansi 7 Color")]
    ansi_7: Color,
    #[serde(rename = "Ansi 8 Color")]
    ansi_8: Color,
    #[serde(rename = "Ansi 9 Color")]
    ansi_9: Color,
    #[serde(rename = "Ansi 10 Color")]
    ansi_10: Color,
    #[serde(rename = "Ansi 11 Color")]
    ansi_11: Color,
    #[serde(rename = "Ansi 12 Color")]
    ansi_12: Color,
    #[serde(rename = "Ansi 13 Color")]
    ansi_13: Color,
    #[serde(rename = "Ansi 14 Color")]
    ansi_14: Color,
    #[serde(rename = "Ansi 15 Color")]
    ansi_15: Color,
    #[serde(rename = "Background Color")]
    background: Color,
    #[serde(rename = "Bold Color")]
    #[allow(dead_code)]
    bold: Color,
    #[serde(rename = "Cursor Color")]
    cursor: Color,
    #[serde(rename = "Cursor Text Color")]
    cursor_text: Color,
    #[serde(rename = "Foreground Color")]
    foreground: Color,
    #[serde(rename = "Selected Text Color")]
    selected_text: Color,
    #[serde(rename = "Selection Color")]
    selection: Color,
}

impl ITerm2 {
    pub fn parse_str(s: &str) -> anyhow::Result<ColorSchemeFile> {
        let mut name = None;
        let mut author = None;
        let mut origin_url = None;

        // Look for metadata encoded in comments(!)
        for line in s.lines() {
            let fields = line.splitn(2, ":").collect::<Vec<_>>();
            if fields.len() == 2 {
                let k = fields[0].trim().to_ascii_lowercase();
                let v = fields[1].trim();
                if k == "name" {
                    name.replace(v.to_string());
                } else if k == "url" {
                    origin_url.replace(v.to_string());
                } else if k == "author" {
                    author.replace(v.to_string());
                }
            }
        }

        let scheme: Self = plist::from_bytes(s.as_bytes())?;

        let cursor = scheme.cursor.into();

        Ok(ColorSchemeFile {
            colors: Palette {
                foreground: Some(scheme.foreground.into()),
                background: Some(scheme.background.into()),
                cursor_fg: Some(scheme.cursor_text.into()),
                cursor_bg: Some(cursor),
                cursor_border: Some(cursor),
                selection_fg: Some(scheme.selected_text.into()),
                selection_bg: Some(scheme.selection.into()),
                ansi: Some([
                    scheme.ansi_0.into(),
                    scheme.ansi_1.into(),
                    scheme.ansi_2.into(),
                    scheme.ansi_3.into(),
                    scheme.ansi_4.into(),
                    scheme.ansi_5.into(),
                    scheme.ansi_6.into(),
                    scheme.ansi_7.into(),
                ]),
                brights: Some([
                    scheme.ansi_8.into(),
                    scheme.ansi_9.into(),
                    scheme.ansi_10.into(),
                    scheme.ansi_11.into(),
                    scheme.ansi_12.into(),
                    scheme.ansi_13.into(),
                    scheme.ansi_14.into(),
                    scheme.ansi_15.into(),
                ]),
                ..Default::default()
            },
            metadata: ColorSchemeMetaData {
                name,
                author,
                origin_url,
                wezterm_version: None,
                aliases: vec![],
            },
        })
    }

    pub fn load_file<P: AsRef<Path>>(path: P) -> anyhow::Result<ColorSchemeFile>
    where
        P: std::fmt::Debug,
    {
        let data = std::fs::read_to_string(&path).context(format!("read file {path:?}"))?;

        Self::parse_str(&data)
    }
}
