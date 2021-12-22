use anyhow::Context;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

pub struct Downloader {}

/// Simple heuristics to try to avoid obvious trickery with
/// the name provided by the remote system
fn neuter_name(name: &str) -> Option<&str> {
    let name = match name.rsplit_once(|c| c == '/' || c == '\\') {
        Some((_, base)) => base,
        None => name,
    };

    if name == "." || name == ".." {
        return None;
    }

    if name.contains(':') {
        return None;
    }

    Some(name)
}

impl Downloader {
    pub fn new() -> Self {
        Self {}
    }

    /// Given a suggested name, make a few attempts to derive a local name
    /// in the user's download folder that doesn't conflict with any other
    /// files in that folder.
    /// Returns the selected name and the opened File on success.
    fn resolve_file_name(&self, name: Option<&str>) -> anyhow::Result<(PathBuf, File)> {
        let name = name
            .and_then(neuter_name)
            .unwrap_or("downloaded-via-wezterm");

        let download_dir = dirs_next::download_dir()
            .ok_or_else(|| anyhow::anyhow!("unable to locate download directory"))?;

        for n in 0..20 {
            let candidate = if n == 0 {
                download_dir.join(name)
            } else {
                download_dir.join(&format!("{}.{}", name, n))
            };

            if let Ok(file) = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&candidate)
            {
                return Ok((candidate, file));
            }
        }

        anyhow::bail!(
            "Unable to find non-conflicting download name for {} in {}",
            name,
            download_dir.display()
        );
    }

    fn save_to_downloads_impl(
        &self,
        orig_name: Option<String>,
        data: Vec<u8>,
    ) -> anyhow::Result<()> {
        let (name, mut file) = self.resolve_file_name(orig_name.as_deref())?;
        file.write_all(&data)
            .with_context(|| format!("writing {} of data to {}", data.len(), name.display()))?;

        let url = format!("file://{}", name.display());
        wezterm_toast_notification::persistent_toast_notification_with_click_to_open_url(
            "Download completed",
            &format!("Downloaded {}", name.display()),
            &url,
        );

        log::info!("Downloaded {}", name.display());

        Ok(())
    }
}

impl wezterm_term::DownloadHandler for Downloader {
    fn save_to_downloads(&self, name: Option<String>, data: Vec<u8>) {
        if !config::configuration().allow_download_protocols {
            log::error!(
                "Ignoring download request for {:?}, as allow_download_protocols=false",
                name
            );
            return;
        }
        if let Err(err) = self.save_to_downloads_impl(name, data) {
            log::error!("failed to download: {:#}", err);
        }
    }
}
