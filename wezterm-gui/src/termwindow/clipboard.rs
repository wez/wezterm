use crate::termwindow::TermWindowNotif;
use crate::TermWindow;
use config::keyassignment::{ClipboardCopyDestination, ClipboardPasteSource};
use mux::pane::Pane;
use mux::window::WindowId as MuxWindowId;
use mux::Mux;
use std::rc::Rc;
use std::sync::Arc;
use wezterm_term::ClipboardSelection;
use window::{Clipboard, Window, WindowOps};

/// ClipboardHelper bridges between the window crate clipboard
/// manipulation and the term crate clipboard interface
#[derive(Clone)]
pub struct ClipboardHelper {
    pub window: Window,
}

impl wezterm_term::Clipboard for ClipboardHelper {
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
    pub fn setup_clipboard(window: &Window, mux_window_id: MuxWindowId) {
        let clipboard: Arc<dyn wezterm_term::Clipboard> = Arc::new(ClipboardHelper {
            window: window.clone(),
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

    pub fn paste_from_clipboard(&mut self, pane: &Rc<dyn Pane>, clipboard: ClipboardPasteSource) {
        let pane_id = pane.pane_id();
        let window = self.window.as_ref().unwrap().clone();
        let clipboard = match clipboard {
            ClipboardPasteSource::Clipboard => Clipboard::Clipboard,
            ClipboardPasteSource::PrimarySelection => Clipboard::PrimarySelection,
        };
        let future = window.get_clipboard(clipboard);
        promise::spawn::spawn(async move {
            if let Ok(clip) = future.await {
                window.notify(TermWindowNotif::Apply(Box::new(move |myself| {
                    if let Some(pane) = myself.pane_state(pane_id).overlay.clone().or_else(|| {
                        let mux = Mux::get().unwrap();
                        mux.get_pane(pane_id)
                    }) {
                        pane.trickle_paste(clip).ok();
                    }
                })));
            }
        })
        .detach();
        self.maybe_scroll_to_bottom_for_input(&pane);
    }
}
