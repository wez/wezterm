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
                Ok(Some(image)) => match save_clipboard_image(&paste_image_dir, &image) {
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

    let mut path = dir.join(format!("{base}.png"));
    let mut counter = 1;
    while path.exists() {
        path = dir.join(format!("{base}-{counter}.png"));
        counter += 1;
    }

    decoded.save_with_format(&path, image::ImageFormat::Png)?;
    Ok(path)
}
