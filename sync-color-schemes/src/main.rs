use crate::scheme::Scheme;
use anyhow::Context;
use config::{ColorSchemeFile, ColorSchemeMetaData};
use serde::Deserialize;
use sqlite_cache::Cache;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;
use tar::Archive;
use tempfile::NamedTempFile;

mod base16;
mod gogh;
mod iterm2;
mod scheme;
mod sexy;

lazy_static::lazy_static! {
    static ref CACHE: Cache = make_cache();
}

fn apply_nightly_version(metadata: &mut ColorSchemeMetaData) {
    metadata
        .wezterm_version
        .replace("nightly builds only".to_string());
}

fn make_cache() -> Cache {
    let file_name = "/tmp/wezterm-sync-color-schemes.sqlite";
    let connection = sqlite_cache::rusqlite::Connection::open(&file_name).unwrap();
    Cache::new(sqlite_cache::CacheConfig::default(), connection).unwrap()
}

pub async fn fetch_url_as_str(url: &str) -> anyhow::Result<String> {
    let data = fetch_url(url)
        .await
        .with_context(|| format!("fetching {url}"))?;
    String::from_utf8(data).with_context(|| format!("converting data from {url} to string"))
}

pub async fn fetch_url(url: &str) -> anyhow::Result<Vec<u8>> {
    let topic = CACHE.topic("data-by-url").context("creating cache topic")?;

    let (updater, item) = topic
        .get_for_update(url)
        .await
        .context("lookup url in cache")?;
    if let Some(item) = item {
        return Ok(item.data);
    }

    println!("Going to request {url}");
    let client = reqwest::Client::builder()
        .user_agent("wezterm-sync-color-schemes/1.0")
        .build()?;

    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("fetching {url}"))?;
    let mut ttl = Duration::from_secs(86400);
    if let Some(value) = response.headers().get(reqwest::header::CACHE_CONTROL) {
        if let Ok(value) = value.to_str() {
            let fields = value.splitn(2, "=").collect::<Vec<_>>();
            if fields.len() == 2 && fields[0] == "max-age" {
                if let Ok(secs) = fields[1].parse::<u64>() {
                    ttl = Duration::from_secs(secs);
                }
            }
        }
    }

    let status = response.status();

    let data = response.bytes().await?.to_vec();

    if status != reqwest::StatusCode::OK {
        anyhow::bail!("{}", String::from_utf8_lossy(&data));
    }

    updater.write(&data, ttl).context("assigning to cache")?;
    Ok(data)
}

fn make_ident(key: &str) -> String {
    let key = key.to_ascii_lowercase();
    let fields: Vec<&str> = key
        .split(|c: char| !c.is_alphanumeric())
        .filter(|c| !c.is_empty())
        .collect();
    fields.join("-")
}

fn make_prefix(key: &str) -> (char, String) {
    for c in key.chars() {
        match c {
            '0'..='9' | 'a'..='z' => return (c, key.to_ascii_lowercase()),
            'A'..='Z' => return (c.to_ascii_lowercase(), key.to_ascii_lowercase()),
            _ => continue,
        }
    }
    panic!("no good prefix");
}

const KNOWN_NAMESPACES: &[&str] = &[" (base16)", " (terminal.sexy)", " (Gogh)"];

fn known_namespace(name: &str) -> bool {
    for ns in KNOWN_NAMESPACES {
        if name.ends_with(ns) {
            return true;
        }
    }
    false
}

fn suffixed_alias_matches_name(alias: &str, name: &str) -> bool {
    for ns in KNOWN_NAMESPACES {
        if let Some(stripped) = alias.strip_suffix(ns) {
            return stripped == name;
        }
    }
    false
}

fn bake_for_config(mut schemeses: Vec<Scheme>) -> anyhow::Result<()> {
    let json_file_name = "docs/colorschemes/data.json";

    let mut version_by_name = BTreeMap::new();
    if let Ok(data) = std::fs::read_to_string(&json_file_name) {
        #[derive(Deserialize)]
        struct MetaOnly {
            metadata: MetaData,
        }
        #[derive(Deserialize)]
        struct MetaData {
            name: String,
            wezterm_version: Option<String>,
        }

        let existing: Vec<MetaOnly> = serde_json::from_str(&data)?;
        for item in &existing {
            if let Some(version) = &item.metadata.wezterm_version {
                version_by_name.insert(item.metadata.name.to_string(), version.to_string());
            }
        }

        if version_by_name.is_empty() {
            // We're bootstrapping the version info
            for item in &existing {
                let name = &item.metadata.name;
                if known_namespace(name) {
                    continue;
                }
                version_by_name.insert(name.to_string(), "Always".to_string());
            }
        }

        for scheme in &mut schemeses {
            if let Some(version) = version_by_name.get(&scheme.name) {
                scheme
                    .data
                    .metadata
                    .wezterm_version
                    .replace(version.to_string());
            }
        }
    }

    let mut all = vec![];
    for s in &schemeses {
        // Only interested in aliases with different-enough names
        let mut aliases = s.data.metadata.aliases.clone();
        aliases.retain(|name| !suffixed_alias_matches_name(&name, &s.name));

        // Normalize the list of aliases so that the canonical
        // name is included in the list at the start.
        let mut s = s.clone();
        s.data.metadata.aliases = aliases.clone();
        if !aliases.is_empty() {
            s.data.metadata.aliases.insert(0, s.name.clone());
        }

        all.push(s.clone());

        // Only instantiate aliases in our scheme data if
        // we had previously shipped a version with that
        // name in use
        for name in aliases {
            if version_by_name.get(&name).is_some() {
                let mut alias = Scheme {
                    name: name.clone(),
                    ..s.clone()
                };
                alias.data.metadata.name.replace(name.clone());
                all.push(alias);
            }
        }
    }
    all.sort_by_key(|s| make_prefix(&s.name));

    let count = all.len();
    let mut code = String::new();
    code.push_str(&format!(
        "//! This file was generated by sync-color-schemes\n
pub const SCHEMES: [(&'static str, &'static str); {count}] = [\n
    // Start here
",
    ));

    for s in &all {
        let name = s.name.escape_default();
        let toml = s.to_toml()?;
        let toml = toml.escape_default();
        code.push_str(&format!("(\"{name}\", \"{toml}\"),\n",));
    }
    code.push_str("];\n");

    {
        let file_name = "config/src/scheme_data.rs";
        let update = match std::fs::read_to_string(file_name) {
            Ok(existing) => existing != code,
            Err(_) => true,
        };

        if update {
            println!("Updating {file_name}");
            std::fs::write(file_name, code)?;
        }
    }

    // Summarize new schemes for the changelog
    let mut new_items = vec![];
    for s in &all {
        if s.data.metadata.wezterm_version.as_deref() == Some("nightly builds only") {
            let (prefix, _) = make_prefix(&s.name);
            let ident = make_ident(&s.name);
            new_items.push(format!(
                "[{}](colorschemes/{}/index.md#{})",
                s.name, prefix, ident
            ));
        }
    }
    if !new_items.is_empty() {
        println!("* Color schemes: {}", new_items.join(", "));
    }

    // And the data for the docs

    let mut doc_data = vec![];
    for s in all {
        doc_data.push(s.to_json_value()?);
    }

    let json_file_name = "docs/colorschemes/data.json";
    let json = serde_json::to_string_pretty(&doc_data)?;
    let update = match std::fs::read_to_string(json_file_name) {
        Ok(existing) => existing != json,
        Err(_) => true,
    };

    if update {
        println!("Updating {json_file_name}");
        std::fs::write(json_file_name, json)?;
    }

    Ok(())
}

fn push_or_alias(schemeses: &mut Vec<Scheme>, candidate: Scheme) -> bool {
    let mut aliased = false;
    for existing in schemeses.iter_mut() {
        if candidate.data.colors.ansi == existing.data.colors.ansi
            && candidate.data.colors.brights == existing.data.colors.brights
            && candidate.data.colors.foreground == existing.data.colors.foreground
            && candidate.data.colors.background == existing.data.colors.background
        {
            log::info!("Adding {} as alias of {}", candidate.name, existing.name);
            existing.data.metadata.aliases.push(candidate.name.clone());
            aliased = true;
        }
    }
    if !aliased {
        log::info!("Adding {}", candidate.name);
        schemeses.push(candidate);
    }
    aliased
}

fn accumulate(schemeses: &mut Vec<Scheme>, to_add: Vec<Scheme>) {
    // Only accumulate if the scheme looks different enough
    for candidate in to_add {
        push_or_alias(schemeses, candidate);
    }
}

async fn sync_toml(
    repo_url: &str,
    branch: &str,
    suffix: &str,
    schemeses: &mut Vec<Scheme>,
) -> anyhow::Result<()> {
    let tarball_url = format!("{repo_url}/tarball/{branch}");
    let tar_data = fetch_url(&tarball_url).await?;
    let decoder = libflate::gzip::Decoder::new(tar_data.as_slice())?;
    let mut tar = Archive::new(decoder);
    for entry in tar.entries()? {
        let mut entry = entry?;

        if entry
            .path()?
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s == "toml")
            .unwrap_or(false)
        {
            let dest_file = NamedTempFile::new()?;
            entry.unpack(dest_file.path())?;

            let data = std::fs::read_to_string(dest_file.path())?;

            match ColorSchemeFile::from_toml_str(&data) {
                Ok(mut scheme) => {
                    let name = match scheme.metadata.name {
                        Some(name) => name,
                        None => entry
                            .path()?
                            .file_stem()
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .to_string(),
                    };
                    let name = format!("{name}{suffix}");
                    scheme.metadata.name = Some(name.clone());
                    scheme.metadata.origin_url = Some(repo_url.to_string());
                    apply_nightly_version(&mut scheme.metadata);

                    let scheme = Scheme {
                        name: name.clone(),
                        file_name: None,
                        data: scheme,
                    };

                    push_or_alias(schemeses, scheme);
                }
                Err(err) => {
                    log::error!("{tarball_url}/{}: {err:#}", entry.path().unwrap().display());
                }
            }
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    // They color us! my precious!
    let mut schemeses = vec![];
    sync_toml(
        "https://github.com/catppuccin/wezterm",
        "main",
        "",
        &mut schemeses,
    )
    .await?;
    sync_toml(
        "https://github.com/EdenEast/nightfox.nvim",
        "main",
        "",
        &mut schemeses,
    )
    .await?;
    sync_toml(
        "https://github.com/Hiroya-W/wezterm-sequoia-theme",
        "main",
        "",
        &mut schemeses,
    )
    .await?;
    sync_toml(
        "https://github.com/dracula/wezterm",
        "main",
        "",
        &mut schemeses,
    )
    .await?;
    sync_toml(
        "https://github.com/olivercederborg/poimandres.nvim",
        "main",
        "",
        &mut schemeses,
    )
    .await?;
    accumulate(
        &mut schemeses,
        iterm2::sync_iterm2().await.context("sync iterm2")?,
    );
    accumulate(&mut schemeses, base16::sync().await.context("sync base16")?);
    accumulate(
        &mut schemeses,
        gogh::sync_gogh().await.context("sync gogh")?,
    );
    accumulate(
        &mut schemeses,
        sexy::sync_sexy().await.context("sync sexy")?,
    );
    bake_for_config(schemeses)?;

    Ok(())
}
