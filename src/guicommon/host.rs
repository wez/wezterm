use super::window::TerminalWindow;
use crate::MasterPty;
use clipboard::{ClipboardContext, ClipboardProvider};
use failure::Error;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use termwiz::hyperlink::Hyperlink;

pub trait HostHelper {
    fn with_window<F: 'static + Fn(&mut TerminalWindow) -> Result<(), Error>>(&self, func: F);
    fn new_tab(&mut self);
    fn new_window(&mut self);
    fn toggle_full_screen(&mut self);
}

pub struct HostImpl<H: HostHelper> {
    helper: H,
    /// macOS gets unhappy if we set up the clipboard too early,
    /// so we use an Option to defer it until we use it
    clipboard: Option<ClipboardContext>,
}

impl<H: HostHelper> HostImpl<H> {
    pub fn new(helper: H) -> Self {
        Self {
            helper,
            clipboard: None,
        }
    }

    fn clipboard(&mut self) -> Result<&mut ClipboardContext, Error> {
        if self.clipboard.is_none() {
            self.clipboard = Some(ClipboardContext::new().map_err(|e| format_err!("{}", e))?);
        }
        Ok(self.clipboard.as_mut().unwrap())
    }

    pub fn get_clipboard(&mut self) -> Result<String, Error> {
        self.clipboard()?
            .get_contents()
            .map_err(|e| format_err!("{}", e))
    }

    pub fn set_clipboard(&mut self, clip: Option<String>) -> Result<(), Error> {
        self.clipboard()?
            .set_contents(clip.unwrap_or_else(|| "".into()))
            .map_err(|e| format_err!("{}", e))?;
        // Request the clipboard contents we just set; on some systems
        // if we copy and paste in wezterm, the clipboard isn't visible
        // to us again until the second call to get_clipboard.
        self.get_clipboard().map(|_| ())
    }
}

impl<H: HostHelper> Deref for HostImpl<H> {
    type Target = H;
    fn deref(&self) -> &H {
        &self.helper
    }
}
impl<H: HostHelper> DerefMut for HostImpl<H> {
    fn deref_mut(&mut self) -> &mut H {
        &mut self.helper
    }
}

/// Implements `TerminalHost` for a Tab.
/// `TabHost` instances are short lived and borrow references to
/// other state.
pub struct TabHost<'a, H: HostHelper> {
    pty: &'a mut MasterPty,
    host: &'a mut HostImpl<H>,
}

impl<'a, H: HostHelper> TabHost<'a, H> {
    pub fn new(pty: &'a mut MasterPty, host: &'a mut HostImpl<H>) -> Self {
        Self { pty, host }
    }
}

impl<'a, H: HostHelper> term::TerminalHost for TabHost<'a, H> {
    fn writer(&mut self) -> &mut std::io::Write {
        &mut self.pty
    }

    fn click_link(&mut self, link: &Rc<Hyperlink>) {
        match open::that(link.uri()) {
            Ok(_) => {}
            Err(err) => eprintln!("failed to open {}: {:?}", link.uri(), err),
        }
    }

    fn get_clipboard(&mut self) -> Result<String, Error> {
        self.host.get_clipboard()
    }

    fn set_clipboard(&mut self, clip: Option<String>) -> Result<(), Error> {
        self.host.set_clipboard(clip)
    }

    fn set_title(&mut self, _title: &str) {
        self.host.with_window(move |win| {
            win.update_title();
            Ok(())
        })
    }

    fn new_window(&mut self) {
        self.host.new_window();
    }
    fn new_tab(&mut self) {
        self.host.new_tab();
    }

    fn activate_tab(&mut self, tab: usize) {
        self.host.with_window(move |win| win.activate_tab(tab))
    }

    fn activate_tab_relative(&mut self, tab: isize) {
        self.host
            .with_window(move |win| win.activate_tab_relative(tab))
    }

    fn increase_font_size(&mut self) {
        self.host.with_window(move |win| {
            let scale = win.fonts().get_font_scale();
            let dims = win.get_dimensions();
            win.scaling_changed(Some(scale * 1.1), None, dims.width, dims.height)
        })
    }

    fn decrease_font_size(&mut self) {
        self.host.with_window(move |win| {
            let scale = win.fonts().get_font_scale();
            let dims = win.get_dimensions();
            win.scaling_changed(Some(scale * 0.9), None, dims.width, dims.height)
        })
    }

    fn reset_font_size(&mut self) {
        self.host.with_window(move |win| {
            let dims = win.get_dimensions();
            win.scaling_changed(Some(1.0), None, dims.width, dims.height)
        })
    }
}
