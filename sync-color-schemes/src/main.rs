use config::{ColorSchemeFile, ColorSchemeMetaData, Palette, RgbaColor};
use http_req::request::{HttpVersion, Request};
use http_req::uri::Uri;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;
use wezterm_dynamic::{ToDynamic, Value};

fn dynamic_to_toml(value: Value) -> anyhow::Result<toml::Value> {
    Ok(match value {
        Value::Null => anyhow::bail!("cannot map Null to toml"),
        Value::Bool(b) => toml::Value::Boolean(b),
        Value::String(s) => toml::Value::String(s),
        Value::Array(a) => {
            let mut arr = vec![];
            for v in a {
                arr.push(dynamic_to_toml(v)?);
            }
            toml::Value::Array(arr)
        }
        Value::Object(o) => {
            let mut map = toml::value::Map::new();
            for (k, v) in o {
                let k = match k {
                    Value::String(s) => s,
                    _ => anyhow::bail!("toml keys must be strings {k:?}"),
                };
                let v = match v {
                    Value::Null => continue,
                    other => dynamic_to_toml(other)?,
                };
                map.insert(k, v);
            }
            toml::Value::Table(map)
        }
        Value::U64(i) => toml::Value::Integer(i.try_into()?),
        Value::I64(i) => toml::Value::Integer(i.try_into()?),
        Value::F64(f) => toml::Value::Float(*f),
    })
}

#[derive(Debug, PartialEq)]
struct Scheme {
    pub name: String,
    pub file_name: Option<String>,
    pub data: ColorSchemeFile,
}

fn make_prefix(s: &str) -> (char, String) {
    let fields: Vec<_> = s.splitn(2, ':').collect();
    let key = fields.last().unwrap();
    for c in key.chars() {
        match c {
            '0'..='9' | 'a'..='z' => return (c, key.to_ascii_lowercase()),
            'A'..='Z' => return (c.to_ascii_lowercase(), key.to_ascii_lowercase()),
            _ => continue,
        }
    }
    panic!("no good prefix");
}

impl Scheme {
    fn to_toml_value(&self) -> anyhow::Result<toml::Value> {
        let value = self.data.to_dynamic();
        Ok(dynamic_to_toml(value)?)
    }

    fn to_toml(&self) -> anyhow::Result<String> {
        let value = self.to_toml_value()?;
        Ok(toml::ser::to_string_pretty(&value)?)
    }

    fn to_json(&self) -> anyhow::Result<String> {
        let mut value = self.to_toml_value()?;
        let (prefix, _) = make_prefix(&self.name);
        match &mut value {
            toml::Value::Table(map) => {
                let meta = map.get_mut("metadata").unwrap();
                match meta {
                    toml::Value::Table(meta) => {
                        meta.insert(
                            "prefix".to_string(),
                            toml::Value::String(prefix.to_string()),
                        );
                    }
                    _ => unreachable!(),
                }
            }
            _ => unreachable!(),
        }

        Ok(serde_json::to_string_pretty(&value)?)
    }

    fn to_json_value(&self) -> anyhow::Result<serde_json::Value> {
        let json = self.to_json()?;
        Ok(serde_json::from_str(&json)?)
    }
}

fn load_schemes<P: AsRef<Path>>(scheme_dir: P) -> anyhow::Result<Vec<Scheme>> {
    let scheme_dir_path = scheme_dir.as_ref();
    let dir = std::fs::read_dir(scheme_dir_path)?;

    let mut schemes = vec![];

    for entry in dir {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_str().unwrap();

        if name.ends_with(".toml") {
            let len = name.len();
            let scheme_name = &name[..len - 5];
            let data = std::fs::read_to_string(entry.path())?;
            let scheme = ColorSchemeFile::from_toml_str(&data)?;
            let name = match &scheme.metadata.name {
                Some(n) => n.to_string(),
                None => scheme_name.to_string(),
            };
            schemes.push(Scheme {
                name: name.clone(),
                file_name: Some(format!("{}/{name}", scheme_dir_path.display())),
                data: scheme,
            });
        }
    }

    schemes.sort_by_key(|scheme| scheme.name.clone());

    Ok(schemes)
}

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

fn load_gogh() -> anyhow::Result<Vec<Scheme>> {
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

fn safe_file_name(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            c @ 'a'..='z' => c,
            c @ 'A'..='Z' => c,
            c @ '0'..='9' => c,
            c @ '('..=')' => c,
            ' ' => ' ',
            _ => '-',
        })
        .collect()
}

fn sync_iterm2() -> anyhow::Result<Vec<Scheme>> {
    let built_in = load_schemes("assets/colors")?;
    let mut scheme_map: BTreeMap<_, _> = built_in
        .iter()
        .map(|scheme| (&scheme.name, scheme))
        .collect();

    let it2: Vec<_> = load_schemes("../github/iTerm2-Color-Schemes/wezterm")?
        .into_iter()
        .map(|mut scheme| {
            scheme
                .data
                .metadata
                .origin_url
                .replace("https://github.com/mbadolato/iTerm2-Color-Schemes".to_string());
            scheme
        })
        .collect();

    for scheme in &it2 {
        let toml = scheme.to_toml()?;

        let update = match scheme_map.get(&scheme.name) {
            None => true,
            Some(existing) => existing.to_toml()? != toml,
        };

        if update {
            let file_name = format!("assets/colors/{}.toml", safe_file_name(&scheme.name));
            std::fs::write(file_name, toml)?;
        }

        scheme_map.remove(&scheme.name);
    }

    eprintln!(
        "Schemes to remove: {:?}",
        scheme_map.keys().collect::<Vec<_>>()
    );
    Ok(it2)
}

fn sync_gogh() -> anyhow::Result<Vec<Scheme>> {
    let built_in = load_schemes("assets/colors/gogh")?;
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

fn bake_for_config(schemeses: Vec<Scheme>) -> anyhow::Result<()> {
    let mut all = vec![];

    let count = schemeses.len();
    let mut code = String::new();
    code.push_str(&format!(
        "//! This file was generated by sync-color-schemes\n
pub const SCHEMES: [(&'static str, &'static str); {count}] = [",
    ));
    for s in &schemeses {
        let name = s.name.escape_default();
        let toml = s.to_toml()?;
        let toml = toml.escape_default();
        code.push_str(&format!("(\"{name}\", \"{toml}\"),\n",));

        all.push(s);
    }
    code.push_str("];\n");

    let file_name = "config/src/scheme_data.rs";
    let update = match std::fs::read_to_string(file_name) {
        Ok(existing) => existing != code,
        Err(_) => true,
    };

    if update {
        eprintln!("Updating {file_name}");
        std::fs::write(file_name, code)?;
    }

    // And the data for the docs

    all.sort_by_key(|s| make_prefix(&s.name));
    let mut doc_data = vec![];
    for s in all {
        doc_data.push(s.to_json_value()?);
    }

    let file_name = "docs/colorschemes/data.json";
    let json = serde_json::to_string_pretty(&doc_data)?;
    let update = match std::fs::read_to_string(file_name) {
        Ok(existing) => existing != json,
        Err(_) => true,
    };

    if update {
        eprintln!("Updating {file_name}");
        std::fs::write(file_name, json)?;
    }

    Ok(())
}

fn accumulate(schemeses: &mut Vec<Scheme>, to_add: Vec<Scheme>) {
    // Only accumulate if the scheme looks different enough
    'skip_candidate: for candidate in to_add {
        for existing in schemeses.iter() {
            if candidate.data.colors.ansi == existing.data.colors.ansi
                && candidate.data.colors.brights == existing.data.colors.brights
                && candidate.data.colors.foreground == existing.data.colors.foreground
                && candidate.data.colors.background == existing.data.colors.background
            {
                println!("{} is same as {}", candidate.name, existing.name);
                continue 'skip_candidate;
            }
        }

        schemeses.push(candidate);
    }
}

fn main() -> anyhow::Result<()> {
    // They color us! my precious!
    let mut schemeses = sync_iterm2()?;
    accumulate(&mut schemeses, sync_gogh()?);
    bake_for_config(schemeses)?;

    Ok(())
}
