use super::*;
use serde::Deserialize;
use tar::Archive;
use tempfile::NamedTempFile;

async fn fetch_base16_list() -> anyhow::Result<Vec<String>> {
    let data = fetch_url_as_str(
        "https://raw.githubusercontent.com/chriskempson/base16-schemes-source/main/list.yaml",
    )
    .await?;

    let mut result = vec![];
    for doc in yaml_rust::YamlLoader::load_from_str(&data)? {
        for value in doc
            .into_hash()
            .ok_or_else(|| anyhow::anyhow!("list.yaml isn't a hash"))?
            .values()
        {
            result.push(
                value
                    .clone()
                    .into_string()
                    .ok_or_else(|| anyhow::anyhow!("item {value:?} is not a string"))?,
            );
        }
    }

    Ok(result)
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
    println!("decoding tarball from {url}");
    let decoder = libflate::gzip::Decoder::new(tar_data)?;
    let mut tar = Archive::new(decoder);
    let mut schemes = vec![];

    for entry in tar.entries()? {
        let mut entry = entry?;
        if entry.path()?.extension() == Some(std::ffi::OsStr::new("yaml")) {
            let dest_file = NamedTempFile::new()?;
            entry.unpack(dest_file.path())?;

            let mut scheme =
                color_funcs::schemes::base16::Base16Scheme::load_file(dest_file.path())?;

            let name = format!("{} (base16)", scheme.metadata.name.unwrap());
            scheme.metadata.name = Some(name.clone());
            scheme.metadata.origin_url = Some(url.to_string());

            schemes.push(Scheme {
                name: name,
                file_name: None,
                data: scheme,
            });
        }
    }

    Ok(schemes)
}

pub async fn sync() -> anyhow::Result<Vec<Scheme>> {
    let repos = fetch_base16_list().await?;
    let mut futures = vec![];
    for repo in repos {
        for branch in ["master", "main"] {
            let repo = repo.clone();
            futures.push(tokio::spawn(async move {
                let tb = fetch_repo_tarball(&repo, branch).await?;
                println!("Got {} bytes of data for {repo}", tb.len());
                extract_scheme_yamls(&repo, &tb).await
            }));
        }
    }

    let mut schemes = vec![];
    for item in futures::future::join_all(futures).await {
        if let Ok(Ok(mut items)) = item {
            schemes.append(&mut items);
        }
    }

    Ok(schemes)
}
