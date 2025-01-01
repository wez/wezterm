use anyhow::Context;
use config::{ColorSchemeFile, ColorSchemeMetaData, Palette, RgbaColor};
use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize, Debug)]
pub struct Sexy {
    name: String,
    author: String,
    color: [String; 16],
    foreground: String,
    background: String,
}

impl Sexy {
    pub fn load_file<P: AsRef<Path>>(path: P) -> anyhow::Result<ColorSchemeFile>
    where
        P: std::fmt::Debug,
    {
        let data = std::fs::read(&path).context(format!("read file {path:?}"))?;
        let sexy: Self = serde_json::from_slice(&data)?;

        Ok(ColorSchemeFile {
            colors: Palette {
                foreground: Some(RgbaColor::try_from(sexy.foreground)?),
                background: Some(RgbaColor::try_from(sexy.background)?),
                ansi: Some([
                    RgbaColor::try_from(sexy.color[0].clone())?,
                    RgbaColor::try_from(sexy.color[1].clone())?,
                    RgbaColor::try_from(sexy.color[2].clone())?,
                    RgbaColor::try_from(sexy.color[3].clone())?,
                    RgbaColor::try_from(sexy.color[4].clone())?,
                    RgbaColor::try_from(sexy.color[5].clone())?,
                    RgbaColor::try_from(sexy.color[6].clone())?,
                    RgbaColor::try_from(sexy.color[7].clone())?,
                ]),
                brights: Some([
                    RgbaColor::try_from(sexy.color[8].clone())?,
                    RgbaColor::try_from(sexy.color[9].clone())?,
                    RgbaColor::try_from(sexy.color[10].clone())?,
                    RgbaColor::try_from(sexy.color[11].clone())?,
                    RgbaColor::try_from(sexy.color[12].clone())?,
                    RgbaColor::try_from(sexy.color[13].clone())?,
                    RgbaColor::try_from(sexy.color[14].clone())?,
                    RgbaColor::try_from(sexy.color[15].clone())?,
                ]),
                ..Default::default()
            },
            metadata: ColorSchemeMetaData {
                name: Some(sexy.name),
                author: Some(sexy.author),
                origin_url: None,
                wezterm_version: None,
                aliases: vec![],
            },
        })
    }
}
