use crate::termwindow::TermWindowNotif;
use crate::TermWindow;
use config::keyassignment::{ClipboardCopyDestination, ClipboardPasteSource};
use mux::pane::Pane;
use mux::Mux;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use window::{Clipboard, ClipboardImage, WindowOps};

impl TermWindow {
    pub fn copy_to_clipboard(&self, clipboard: ClipboardCopyDestination, text: String) {
        let clipboard = match clipboard {
            ClipboardCopyDestination::Clipboard => [Some(Clipboard::Clipboard), None],
            ClipboardCopyDestination::PrimarySelection => [Some(Clipboard::PrimarySelection), None],
            ClipboardCopyDestination::ClipboardAndPrimarySelection => [
                Some(Clipboard::Clipboard),
                Some(Clipboard::PrimarySelection),
            ],
        };
        for &c in &clipboard {
            if let Some(c) = c {
                self.window.as_ref().unwrap().set_clipboard(c, text.clone());
            }
        }
    }

    pub fn paste_from_clipboard(&mut self, pane: &Arc<dyn Pane>, clipboard: ClipboardPasteSource) {
        let pane_id = pane.pane_id();
        log::trace!(
            "paste_from_clipboard in pane {} {:?}",
            pane.pane_id(),
            clipboard
        );
        let window = self.window.as_ref().unwrap().clone();
        let clipboard = match clipboard {
            ClipboardPasteSource::Clipboard => Clipboard::Clipboard,
            ClipboardPasteSource::PrimarySelection => Clipboard::PrimarySelection,
        };
        let paste_image_dir = self.config.paste_image_dir.clone();
        let quoting = self.config.quote_dropped_files;

        promise::spawn::spawn(async move {
            // Prefer an image when the clipboard holds one, so that tools
            // that accept a path to an image can receive one.
            let image_path = match window.get_clipboard_image(clipboard).await {
                Ok(Some(image)) => match promise::spawn::spawn_into_new_thread(move || {
                    // Decoding, PNG encoding and disk I/O must not block the GUI.
                    save_clipboard_image(&paste_image_dir, &image)
                })
                .await
                {
                    Ok(path) => Some(path),
                    Err(err) => {
                        log::error!("failed to save clipboard image: {err:#}");
                        None
                    }
                },
                Ok(None) => None,
                Err(err) => {
                    log::debug!("clipboard has no image: {err:#}");
                    None
                }
            };

            let text = if let Some(path) = image_path {
                Some(quoting.escape(&path.to_string_lossy()))
            } else {
                match window.get_clipboard(clipboard).await {
                    Ok(text) => Some(text),
                    Err(err) => {
                        log::error!("failed to get clipboard: {err:#}");
                        None
                    }
                }
            };

            if let Some(text) = text {
                window.notify(TermWindowNotif::Apply(Box::new(move |myself| {
                    if let Some(pane) = myself
                        .pane_state(pane_id)
                        .overlay
                        .as_ref()
                        .map(|overlay| overlay.pane.clone())
                        .or_else(|| {
                            let mux = Mux::get();
                            mux.get_pane(pane_id)
                        })
                    {
                        pane.send_paste(&text).ok();
                    }
                })));
            }
        })
        .detach();
        self.maybe_scroll_to_bottom_for_input(&pane);
    }
}

/// Turn a filename suggested by the clipboard source into something that
/// is safe to use as a local file name, without any directory components.
fn sanitize_suggested_name(name: &str) -> Option<String> {
    let name = Path::new(name).file_name()?.to_str()?;
    let stem = Path::new(name).file_stem()?.to_str()?;
    let mut out = String::new();
    for c in stem.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
            out.push(c);
        } else if c.is_whitespace() {
            out.push('_');
        }
    }
    let out = out
        .trim_matches(|c| c == '.' || c == '_' || c == '-')
        .to_string();
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Save an image that was read from the clipboard into `dir`, re-encoding it
/// as PNG, and return the path of the saved file.
fn save_clipboard_image(dir: &Path, image: &ClipboardImage) -> anyhow::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;

    let decoded = image::load_from_memory(&image.data)
        .map_err(|err| anyhow::anyhow!("decoding clipboard image: {err}"))?;

    let base = image
        .suggested_name
        .as_deref()
        .and_then(sanitize_suggested_name)
        .unwrap_or_else(|| {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            image.data.hash(&mut hasher);
            format!(
                "paste-{}-{:08x}",
                chrono::Local::now().format("%Y%m%d-%H%M%S"),
                hasher.finish() as u32
            )
        });

    // Encode before publishing the final path, and never overwrite a file
    // (including a dangling symlink) if another paste chose the same name.
    let mut file = tempfile::NamedTempFile::new_in(dir)?;
    decoded.write_to(file.as_file_mut(), image::ImageFormat::Png)?;
    for counter in 0u64.. {
        let name = if counter == 0 {
            format!("{base}.png")
        } else {
            format!("{base}-{counter}.png")
        };
        let path = dir.join(name);
        match file.persist_noclobber(&path) {
            Ok(_) => return Ok(path),
            Err(err) if err.error.kind() == std::io::ErrorKind::AlreadyExists => {
                file = err.file;
            }
            Err(err) => return Err(err.error.into()),
        }
    }
    anyhow::bail!("exhausted clipboard image filenames")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clipboard_image() -> ClipboardImage {
        let mut data = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(2, 2)
            .write_to(&mut data, image::ImageFormat::Png)
            .unwrap();
        ClipboardImage {
            data: data.into_inner(),
            suggested_name: Some("screenshot.png".into()),
        }
    }

    #[test]
    fn saved_images_do_not_overwrite_existing_files() {
        let dir = tempfile::tempdir().unwrap();
        let existing = dir.path().join("screenshot.png");
        std::fs::write(&existing, b"keep me").unwrap();
        let path = save_clipboard_image(dir.path(), &clipboard_image()).unwrap();
        assert_ne!(path, existing);
        assert_eq!(std::fs::read(existing).unwrap(), b"keep me");
        assert_eq!(image::open(path).unwrap().width(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn saved_images_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = save_clipboard_image(dir.path(), &clipboard_image()).unwrap();
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o077,
            0
        );
    }

    #[cfg(unix)]
    #[test]
    fn saved_images_do_not_follow_dangling_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target.png");
        let link = dir.path().join("screenshot.png");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let path = save_clipboard_image(dir.path(), &clipboard_image()).unwrap();
        assert_ne!(path, link);
        assert!(!target.exists());
        assert_eq!(std::fs::read_link(link).unwrap(), target);
        assert_eq!(image::open(path).unwrap().width(), 2);
    }

    #[test]
    fn concurrent_pastes_save_distinct_images() {
        let dir = tempfile::tempdir().unwrap();
        let barrier = Arc::new(std::sync::Barrier::new(8));
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let dir = dir.path().to_path_buf();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    save_clipboard_image(&dir, &clipboard_image()).unwrap()
                })
            })
            .collect();
        let paths: std::collections::HashSet<_> = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect();
        assert_eq!(paths.len(), 8);
        for path in paths {
            assert_eq!(image::open(path).unwrap().width(), 2);
        }
    }
}
