use super::*;
use tar::Archive;
use tempfile::NamedTempFile;

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
    apply_nightly_version(&mut scheme.metadata);

    Ok(Scheme {
        name,
        file_name: None,
        data: scheme,
    })
}

pub async fn sync_sexy() -> anyhow::Result<Vec<Scheme>> {
    let tar_data =
        fetch_url("https://github.com/stayradiated/terminal.sexy/tarball/master").await?;

    let decoder = libflate::gzip::Decoder::new(tar_data.as_slice())?;
    let mut tar = Archive::new(decoder);

    let mut schemes = vec![];

    for entry in tar.entries()? {
        let mut entry = entry?;
        if entry.path()?.extension() == Some(std::ffi::OsStr::new("json")) {
            let dest_file = NamedTempFile::new()?;
            entry.unpack(dest_file.path())?;

            if let Ok(scheme) = load_sexy_file(dest_file.path()) {
                schemes.push(scheme);
            }
        }
    }

    Ok(schemes)
}
