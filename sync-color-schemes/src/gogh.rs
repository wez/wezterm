use super::*;
use color_funcs::schemes::gogh::GoghTheme;

pub async fn sync_gogh() -> anyhow::Result<Vec<Scheme>> {
    let latest =
        fetch_url("https://raw.githubusercontent.com/Gogh-Co/Gogh/master/data/themes.json").await?;

    Ok(GoghTheme::load_all(&latest)?
        .into_iter()
        .map(|mut scheme| {
            let name = format!("{} (Gogh)", scheme.metadata.name.unwrap());
            scheme.metadata.name = Some(name.clone());
            apply_nightly_version(&mut scheme.metadata);
            scheme
                .metadata
                .origin_url
                .replace("https://github.com/Gogh-Co/Gogh".to_string());

            Scheme {
                name,
                file_name: None,
                data: scheme,
            }
        })
        .collect())
}
