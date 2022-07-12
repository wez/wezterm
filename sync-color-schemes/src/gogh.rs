use super::*;

#[derive(Deserialize, Debug)]
#[allow(non_snake_case)]
struct GoghTheme {
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

fn fetch_gogh_json() -> anyhow::Result<Vec<GoghTheme>> {
    let cache = "/tmp/wezterm-gogh.json";
    let need_fetch = match std::fs::metadata(cache) {
        Ok(m) => match m.modified() {
            Ok(t) => match t.elapsed() {
                Ok(d) => d > Duration::from_secs(86400),
                Err(_) => false,
            },
            Err(_) => true,
        },
        Err(_) => true,
    };

    let mut latest = Vec::new();
    if need_fetch {
        eprintln!("Updating gogh cache");
        let uri = Uri::try_from(
            "https://raw.githubusercontent.com/Gogh-Co/Gogh/master/data/themes.json",
        )?;
        Request::new(&uri)
            .version(HttpVersion::Http10)
            .send(&mut latest)?;
        std::fs::write(cache, &latest)?;
    } else {
        latest = std::fs::read(cache)?;
    }

    #[derive(Deserialize, Debug)]
    struct Themes {
        themes: Vec<GoghTheme>,
    }

    let data: Themes = serde_json::from_slice(&latest)?;

    Ok(data.themes)
}

pub fn load_gogh() -> anyhow::Result<Vec<Scheme>> {
    let mut schemes = vec![];
    for s in fetch_gogh_json()? {
        let cursor = RgbaColor::try_from(s.cursorColor)?;
        let name = format!("{} (Gogh)", s.name);

        schemes.push(Scheme {
            name: name.clone(),
            file_name: None,
            data: ColorSchemeFile {
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
                    origin_url: Some("https://github.com/Gogh-Co/Gogh".to_string()),
                },
            },
        })
    }
    Ok(schemes)
}

pub fn sync_gogh() -> anyhow::Result<Vec<Scheme>> {
    let built_in = scheme::load_schemes("assets/colors/gogh")?;
    let mut scheme_map: BTreeMap<_, _> = built_in
        .iter()
        .map(|scheme| (&scheme.name, scheme))
        .collect();
    let gogh = load_gogh()?;

    for scheme in &gogh {
        let toml = scheme.to_toml()?;

        let update = match scheme_map.get(&scheme.name) {
            None => true,
            Some(existing) => existing.to_toml()? != toml,
        };

        if update {
            let safe_name = safe_file_name(&scheme.name);
            let file_name = format!("assets/colors/gogh/{safe_name}.toml");
            eprintln!("Updating {file_name}");
            std::fs::write(file_name, toml)?;
        }

        scheme_map.remove(&scheme.name);
    }

    eprintln!(
        "Gogh Schemes to remove: {:?}",
        scheme_map.keys().collect::<Vec<_>>()
    );
    Ok(gogh)
}
