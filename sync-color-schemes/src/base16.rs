use super::*;
use serde::Deserialize;
use std::sync::Arc;
use tar::Archive;
use tempfile::NamedTempFile;
use tokio::sync::Semaphore;

async fn fetch_base16_list() -> anyhow::Result<Vec<String>> {
    let data = fetch_url_as_str(
        "https://raw.githubusercontent.com/chriskempson/base16-schemes-source/main/list.yaml",
    )
    .await?;

    let mapping: HashMap<String, String> = serde_yaml::from_str(&data)?;
    Ok(mapping.into_values().collect())
}

async fn fetch_repo_tarball(repo_url: &str, branch: &str) -> anyhow::Result<Vec<u8>> {
    let tarball_url = format!("{repo_url}/tarball/{branch}");
    fetch_url(&tarball_url).await
}

#[derive(Deserialize, Debug)]
#[allow(non_snake_case, dead_code)]
struct Base16Scheme {
    pub scheme: String,
    pub author: String,
    pub base00: String,
    pub base01: String,
    pub base02: String,
    pub base03: String,
    pub base04: String,
    pub base05: String,
    pub base06: String,
    pub base07: String,
    pub base08: String,
    pub base09: String,
    pub base0A: String,
    pub base0B: String,
    pub base0C: String,
    pub base0D: String,
    pub base0E: String,
    pub base0F: String,
}

async fn extract_scheme_yamls(url: &str, tar_data: &[u8]) -> anyhow::Result<Vec<Scheme>> {
    let decoder = libflate::gzip::Decoder::new(tar_data)?;
    let mut tar = Archive::new(decoder);
    let mut schemes = vec![];

    for entry in tar.entries()? {
        let mut entry = entry?;

        if entry
            .path()?
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s == "yaml" || s == "yml")
            .unwrap_or(false)
        {
            let dest_file = NamedTempFile::new()?;
            entry.unpack(dest_file.path())?;

            if let Ok(mut scheme) =
                color_funcs::schemes::base16::Base16Scheme::load_file(dest_file.path())
            {
                let name = format!("{} (base16)", scheme.metadata.name.unwrap());
                scheme.metadata.name = Some(name.clone());
                scheme.metadata.origin_url = Some(url.to_string());
                apply_nightly_version(&mut scheme.metadata);

                schemes.push(Scheme {
                    name: name,
                    file_name: None,
                    data: scheme,
                });
            }
        }
    }

    Ok(schemes)
}

pub async fn sync() -> anyhow::Result<Vec<Scheme>> {
    let repos = fetch_base16_list().await?;
    let mut futures = vec![];
    let semaphore = Arc::new(Semaphore::new(10));
    for repo in repos {
        let repo = repo.clone();
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        futures.push(tokio::spawn(async move {
            let topic = CACHE
                .topic("default-branch")
                .context("creating main branch topic")?;

            if let Ok(Some(hit)) = topic.get(&repo) {
                let branch = String::from_utf8_lossy(&hit.data).to_string();
                let tb = fetch_repo_tarball(&repo, &branch).await?;
                return extract_scheme_yamls(&repo, &tb).await;
            }

            for branch in ["main", "master"] {
                if let Ok(tb) = fetch_repo_tarball(&repo, branch).await {
                    topic.set(&repo, branch.as_bytes(), Duration::from_secs(86400))?;
                    return extract_scheme_yamls(&repo, &tb).await;
                }
            }
            drop(permit);
            anyhow::bail!("couldn't figure out branch for {repo}");
        }));
    }

    let mut schemes = vec![];
    for item in futures::future::join_all(futures).await {
        if let Ok(Ok(mut items)) = item {
            schemes.append(&mut items);
        }
    }

    Ok(schemes)
}
