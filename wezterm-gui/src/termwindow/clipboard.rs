use crate::TermWindow;
use config::keyassignment::{ClipboardCopyDestination, ClipboardPasteSource};
use mux::pane::Pane;
use mux::window::WindowId as MuxWindowId;
use mux::Mux;
use std::rc::Rc;
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

impl TermWindow {
    pub fn setup_clipboard(
        window: &Window,
        mux_window_id: MuxWindowId,
        clipboard_contents: Arc<Mutex<Option<String>>>,
    ) {
        let clipboard: Arc<dyn wezterm_term::Clipboard> = Arc::new(ClipboardHelper {
            window: window.clone(),
            clipboard_contents,
        });
        let mux = Mux::get().unwrap();

        let mut mux_window = mux.get_window_mut(mux_window_id).unwrap();

        mux_window.set_clipboard(&clipboard);
        for tab in mux_window.iter() {
            for pos in tab.iter_panes() {
                pos.pane.set_clipboard(&clipboard);
            }
        }
    }

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

    pub async fn paste_from_clipboard(
        &mut self,
        pane: &Rc<dyn Pane>,
        clipboard: ClipboardPasteSource,
    ) {
        let pane_id = pane.pane_id();
        let window = self.window.as_ref().unwrap().clone();
        let clipboard = match clipboard {
            ClipboardPasteSource::Clipboard => Clipboard::Clipboard,
            ClipboardPasteSource::PrimarySelection => Clipboard::PrimarySelection,
        };
        let future = window.get_clipboard(clipboard);

        if let Ok(clip) = future.await {
            if let Some(pane) = self.pane_state(pane_id).overlay.clone().or_else(|| {
                let mux = Mux::get().unwrap();
                mux.get_pane(pane_id)
            }) {
                pane.trickle_paste(clip).ok();
            }
        }
    }
}
