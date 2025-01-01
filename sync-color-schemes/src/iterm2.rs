use super::*;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Semaphore;

pub async fn sync_iterm2() -> anyhow::Result<Vec<Scheme>> {
    let trees = fetch_url_as_str(
        "https://api.github.com/repos/mbadolato/iTerm2-Color-Schemes/git/trees/master?recursive=1",
    )
    .await?;

    #[derive(Deserialize, Debug)]
    struct GHTree {
        tree: Vec<Tree>,
    }

    #[derive(Deserialize, Debug)]
    struct Tree {
        path: String,
    }

    let tree_info: GHTree = serde_json::from_str(&trees)?;
    let mut schemes = vec![];

    let mut futures = vec![];

    let semaphore = Arc::new(Semaphore::new(10));

    for tree in tree_info.tree {
        if tree.path.ends_with(".itermcolors") {
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            futures.push(tokio::spawn(async move {
                let url = format!(
                    "https://raw.githubusercontent.com/mbadolato/iTerm2-Color-Schemes/master/{}",
                    tree.path
                );
                let source = fetch_url_as_str(&url).await?;
                drop(permit);

                let mut scheme = color_funcs::schemes::iterm2::ITerm2::parse_str(&source)?;

                // Always derive the name from the filename for schemes
                // coming from this repo; even if they had metadata inside
                // for the name, we had previously used the filename-derived
                // name and we'd like to preserve that for compatibility reasons!
                let name = tree
                    .path
                    .strip_prefix("schemes/")
                    .unwrap()
                    .strip_suffix(".itermcolors")
                    .unwrap()
                    .to_string();
                scheme.metadata.name.replace(name.clone());
                apply_nightly_version(&mut scheme.metadata);

                if scheme.metadata.origin_url.is_none() {
                    scheme
                        .metadata
                        .origin_url
                        .replace("https://github.com/mbadolato/iTerm2-Color-Schemes".to_string());
                }

                anyhow::Ok(Scheme {
                    name,
                    file_name: None,
                    data: scheme,
                })
            }));
        }
    }

    for item in futures::future::join_all(futures).await {
        let item = item??;
        schemes.push(item);
    }

    Ok(schemes)
}
