use anyhow::Context;
use config::{ColorSchemeFile, ColorSchemeMetaData, Palette, RgbaColor};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Deserialize, Debug)]
#[allow(non_snake_case, dead_code)]
pub struct Base16Scheme {
    scheme: String,
    author: String,
    base00: String,
    base01: String,
    base02: String,
    base03: String,
    base04: String,
    base05: String,
    base06: String,
    base07: String,
    base08: String,
    base09: String,
    base0A: String,
    base0B: String,
    base0C: String,
    base0D: String,
    base0E: String,
    base0F: String,
}

impl Base16Scheme {
    pub fn load_file<P: AsRef<Path>>(path: P) -> anyhow::Result<ColorSchemeFile>
    where
        P: std::fmt::Debug,
    {
        let data = std::fs::read_to_string(&path).context(format!("read file {path:?}"))?;

        let scheme: Self = serde_yaml::from_str(&data)?;

        let base_0 = RgbaColor::try_from(scheme.base00)?;
        let base_1 = RgbaColor::try_from(scheme.base01)?;
        let base_2 = RgbaColor::try_from(scheme.base02)?;
        let base_3 = RgbaColor::try_from(scheme.base03)?;
        let base_4 = RgbaColor::try_from(scheme.base04)?;
        let base_5 = RgbaColor::try_from(scheme.base05)?;
        let base_6 = RgbaColor::try_from(scheme.base06)?;
        let base_7 = RgbaColor::try_from(scheme.base07)?;
        let base_8 = RgbaColor::try_from(scheme.base08)?;
        let base_9 = RgbaColor::try_from(scheme.base09)?;
        let base_a = RgbaColor::try_from(scheme.base0A)?;
        let base_b = RgbaColor::try_from(scheme.base0B)?;
        let base_c = RgbaColor::try_from(scheme.base0C)?;
        let base_d = RgbaColor::try_from(scheme.base0D)?;
        let base_e = RgbaColor::try_from(scheme.base0E)?;
        let base_f = RgbaColor::try_from(scheme.base0F)?;

        let mut indexed = HashMap::new();
        indexed.insert(16, base_9);
        indexed.insert(17, base_f);
        indexed.insert(18, base_1);
        indexed.insert(19, base_2);
        indexed.insert(20, base_4);
        indexed.insert(21, base_6);

        Ok(ColorSchemeFile {
            colors: Palette {
                foreground: Some(base_5),
                background: Some(base_0),
                cursor_fg: Some(base_0),
                cursor_bg: Some(base_5),
                cursor_border: Some(base_5),
                selection_bg: Some(base_5),
                selection_fg: Some(base_0),
                ansi: Some([
                    base_0, base_8, base_b, base_a, base_d, base_e, base_c, base_5,
                ]),
                brights: Some([
                    base_3, base_8, base_b, base_a, base_d, base_e, base_c, base_7,
                ]),
                indexed,
                ..Default::default()
            },
            metadata: ColorSchemeMetaData {
                name: Some(scheme.scheme),
                author: Some(scheme.author),
                origin_url: None,
                wezterm_version: None,
                aliases: vec![],
            },
        })
    }
}
