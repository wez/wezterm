use std::sync::{Arc, Mutex};
use wezterm_term::ClipboardSelection;
use window::{Clipboard, Window, WindowOps};

/// ClipboardHelper bridges between the window crate clipboard
/// manipulation and the term crate clipboard interface
#[derive(Clone)]
pub struct ClipboardHelper {
    pub window: Window,
    pub clipboard_contents: Arc<Mutex<Option<String>>>,
}

impl wezterm_term::Clipboard for ClipboardHelper {
    fn get_contents(&self, _selection: ClipboardSelection) -> anyhow::Result<String> {
        // Even though we could request the clipboard contents using a call
        // like `self.window.get_clipboard().wait()` here, that requires
        // that the event loop be processed to do its work.
        // Since we are typically called in a blocking fashion on the
        // event loop, we have to manually arrange to populate the
        // clipboard_contents cache prior to calling the code that
        // might call us.
        Ok(self
            .clipboard_contents
            .lock()
            .unwrap()
            .as_ref()
            .cloned()
            .unwrap_or_else(String::new))
    }

    fn set_contents(
        &self,
        selection: ClipboardSelection,
        data: Option<String>,
    ) -> anyhow::Result<()> {
        self.window.set_clipboard(
            match selection {
                ClipboardSelection::Clipboard => Clipboard::Clipboard,
                ClipboardSelection::PrimarySelection => Clipboard::PrimarySelection,
            },
            data.unwrap_or_else(String::new),
        );
        Ok(())
    }
}
