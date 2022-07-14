use super::*;

fn load_sexy_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Scheme>
where
    P: std::fmt::Debug,
{
    let mut scheme = color_funcs::schemes::sexy::Sexy::load_file(&path)?;
    let name = format!("{} (terminal.sexy)", scheme.metadata.name.unwrap());
    scheme.metadata.name = Some(name.clone());
    scheme
        .metadata
        .origin_url
        .replace("https://github.com/stayradiated/terminal.sexy".to_string());

    Ok(Scheme {
        name: name.clone(),
        file_name: None,
        data: scheme,
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
