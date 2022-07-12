use super::*;

pub fn sync_iterm2() -> anyhow::Result<Vec<Scheme>> {
    let built_in = scheme::load_schemes("assets/colors")?;
    let mut scheme_map: BTreeMap<_, _> = built_in
        .iter()
        .map(|scheme| (&scheme.name, scheme))
        .collect();

    let it2: Vec<_> = scheme::load_schemes("../github/iTerm2-Color-Schemes/wezterm")?
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
