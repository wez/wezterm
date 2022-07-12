use super::*;

fn load_sexy_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Scheme>
where
    P: std::fmt::Debug,
{
    #[derive(Deserialize, Debug)]
    struct Sexy {
        name: String,
        author: String,
        color: [String; 16],
        foreground: String,
        background: String,
    }

    let data = std::fs::read(&path).context(format!("read file {path:?}"))?;
    let sexy: Sexy = serde_json::from_slice(&data)?;

    let name = format!("{} (terminal.sexy)", sexy.name);

    Ok(Scheme {
        name: name.clone(),
        file_name: None,
        data: ColorSchemeFile {
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
                name: Some(name.clone()),
                author: Some(sexy.author.clone()),
                origin_url: Some("https://github.com/stayradiated/terminal.sexy".to_string()),
            },
        },
    })
}

fn sync_sexy_dir<P: AsRef<Path>>(path: P, schemes: &mut Vec<Scheme>) -> anyhow::Result<()>
where
    P: std::fmt::Debug,
{
    let dir = std::fs::read_dir(&path).context(format!("reading dir {path:?}"))?;

    for entry in dir {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_str().unwrap();

        if name.ends_with(".json") {
            schemes.push(load_sexy_file(path.as_ref().join(name))?);
        }
    }

    Ok(())
}

pub fn sync_sexy() -> anyhow::Result<Vec<Scheme>> {
    let mut schemes = vec![];

    for path in [
        "../github/terminal.sexy/dist/schemes/base16",
        "../github/terminal.sexy/dist/schemes/collection",
        "../github/terminal.sexy/dist/schemes/xcolors.net",
    ] {
        sync_sexy_dir(path, &mut schemes)?;
    }
    Ok(schemes)
}
