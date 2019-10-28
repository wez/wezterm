use clipboard::{ClipboardContext, ClipboardProvider};
use failure::{format_err, Fallible};
use std::sync::Mutex;
use term::terminal::Clipboard;

pub struct SystemClipboard {
    inner: Mutex<Inner>,
}

struct Inner {
    /// macOS gets unhappy if we set up the clipboard too early,
    /// so we use an Option to defer it until we use it
    clipboard: Option<ClipboardContext>,
}

impl Inner {
    fn new() -> Self {
        Self { clipboard: None }
    }

    fn clipboard(&mut self) -> Fallible<&mut ClipboardContext> {
        if self.clipboard.is_none() {
            self.clipboard = Some(ClipboardContext::new().map_err(|e| format_err!("{}", e))?);
        }
        Ok(self.clipboard.as_mut().unwrap())
    }
}

impl SystemClipboard {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner::new()),
        }
    }
}

impl Clipboard for SystemClipboard {
    fn get_contents(&self) -> Fallible<String> {
        let mut inner = self.inner.lock().unwrap();
        inner
            .clipboard()?
            .get_contents()
            .map_err(|e| format_err!("{}", e))
    }

    fn set_contents(&self, data: Option<String>) -> Fallible<()> {
        let mut inner = self.inner.lock().unwrap();
        let clip = inner.clipboard()?;
        clip.set_contents(data.unwrap_or_else(|| "".into()))
            .map_err(|e| format_err!("{}", e))?;
        // Request the clipboard contents we just set; on some systems
        // if we copy and paste in wezterm, the clipboard isn't visible
        // to us again until the second call to get_clipboard.
        clip.get_contents()
            .map(|_| ())
            .map_err(|e| format_err!("{}", e))
    }
}
