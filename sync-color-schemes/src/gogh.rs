use super::*;
use color_funcs::schemes::gogh::GoghTheme;

pub async fn load_gogh() -> anyhow::Result<Vec<Scheme>> {
    let latest =
        fetch_url("https://raw.githubusercontent.com/Gogh-Co/Gogh/master/data/themes.json").await?;

    Ok(GoghTheme::load_all(&latest)?
        .into_iter()
        .map(|mut scheme| {
            let name = format!("{} (Gogh)", scheme.metadata.name.unwrap());
            scheme.metadata.name = Some(name.clone());
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

pub async fn sync_gogh() -> anyhow::Result<Vec<Scheme>> {
    let built_in = scheme::load_schemes("assets/colors/gogh")?;
    let mut scheme_map: BTreeMap<_, _> = built_in
        .iter()
        .map(|scheme| (&scheme.name, scheme))
        .collect();
    let gogh = load_gogh().await?;

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
