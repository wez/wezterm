use super::window::TerminalWindow;
use crate::frontend::gui_executor;
use crate::mux::tab::{Tab, TabId};
use crate::mux::Mux;
use clipboard::{ClipboardContext, ClipboardProvider};
use failure::{format_err, Error};
use promise::Future;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};
use term::{KeyCode, KeyModifiers};
use termwiz::hyperlink::Hyperlink;

pub trait HostHelper {
    fn with_window<F: Send + 'static + Fn(&mut TerminalWindow) -> Result<(), Error>>(
        &self,
        func: F,
    );
    fn toggle_full_screen(&mut self);
}

pub struct HostImpl<H: HostHelper> {
    helper: H,
    /// macOS gets unhappy if we set up the clipboard too early,
    /// so we use an Option to defer it until we use it
    clipboard: Option<ClipboardContext>,
}

const PASTE_CHUNK_SIZE: usize = 1024;

struct Paste {
    tab_id: TabId,
    text: String,
    offset: usize,
}

fn schedule_next_paste(paste: &Arc<Mutex<Paste>>) {
    let paste = Arc::clone(paste);
    Future::with_executor(gui_executor().unwrap(), move || {
        let mut locked = paste.lock().unwrap();
        let mux = Mux::get().unwrap();
        let tab = mux.get_tab(locked.tab_id).unwrap();

        let remain = locked.text.len() - locked.offset;
        let chunk = remain.min(PASTE_CHUNK_SIZE);
        let text_slice = &locked.text[locked.offset..locked.offset + chunk];
        tab.send_paste(text_slice).unwrap();

        if chunk < remain {
            // There is more to send
            locked.offset += chunk;
            schedule_next_paste(&paste);
        }

        Ok(())
    });
}

fn trickle_paste(tab_id: TabId, text: String) {
    let paste = Arc::new(Mutex::new(Paste {
        tab_id,
        text,
        offset: PASTE_CHUNK_SIZE,
    }));
    schedule_next_paste(&paste);
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

    pub fn process_gui_shortcuts(
        &mut self,
        tab: &Tab,
        mods: KeyModifiers,
        key: KeyCode,
    ) -> Result<bool, Error> {
        if mods == KeyModifiers::SUPER && key == KeyCode::Char('t') {
            self.with_window(|win| win.spawn_tab().map(|_| ()));
            return Ok(true);
        }

        if mods == KeyModifiers::ALT
            && (key == KeyCode::Char('\r') || key == KeyCode::Char('\n') || key == KeyCode::Enter)
        {
            self.toggle_full_screen();
            return Ok(true);
        }

        if cfg!(target_os = "macos") && mods == KeyModifiers::SUPER && key == KeyCode::Char('c') {
            // Nominally copy, but that is implicit, so NOP
            return Ok(true);
        }
        if (cfg!(target_os = "macos") && mods == KeyModifiers::SUPER && key == KeyCode::Char('v'))
            || (mods == KeyModifiers::SHIFT && key == KeyCode::Insert)
        {
            let text = self.get_clipboard()?;
            if text.len() <= PASTE_CHUNK_SIZE {
                // Send it all now
                tab.send_paste(&text)?;
                return Ok(true);
            }
            // It's pretty heavy, so we trickle it into the pty
            tab.send_paste(&text[0..PASTE_CHUNK_SIZE])?;
            trickle_paste(tab.tab_id(), text);
            return Ok(true);
        }
        if mods == (KeyModifiers::SUPER | KeyModifiers::SHIFT)
            && (key == KeyCode::Char('[') || key == KeyCode::Char('{'))
        {
            self.activate_tab_relative(-1);
            return Ok(true);
        }
        if mods == (KeyModifiers::SUPER | KeyModifiers::SHIFT)
            && (key == KeyCode::Char(']') || key == KeyCode::Char('}'))
        {
            self.activate_tab_relative(1);
            return Ok(true);
        }

        if (mods == KeyModifiers::SUPER || mods == KeyModifiers::CTRL) && key == KeyCode::Char('-')
        {
            self.decrease_font_size();
            return Ok(true);
        }
        if (mods == KeyModifiers::SUPER || mods == KeyModifiers::CTRL) && key == KeyCode::Char('=')
        {
            self.increase_font_size();
            return Ok(true);
        }
        if (mods == KeyModifiers::SUPER || mods == KeyModifiers::CTRL) && key == KeyCode::Char('0')
        {
            self.reset_font_size();
            return Ok(true);
        }

        if mods == KeyModifiers::SUPER {
            if let KeyCode::Char(c) = key {
                if c >= '0' && c <= '9' {
                    let tab_number = c as u32 - 0x30;
                    // Treat 0 as 10 as that is physically right of 9 on
                    // a keyboard
                    let tab_number = if tab_number == 0 { 10 } else { tab_number - 1 };
                    self.activate_tab(tab_number as usize);
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    pub fn activate_tab(&mut self, tab: usize) {
        self.with_window(move |win| win.activate_tab(tab))
    }

    pub fn activate_tab_relative(&mut self, tab: isize) {
        self.with_window(move |win| win.activate_tab_relative(tab))
    }

    pub fn increase_font_size(&mut self) {
        self.with_window(move |win| {
            let scale = win.fonts().get_font_scale();
            let dims = win.get_dimensions();
            win.scaling_changed(Some(scale * 1.1), None, dims.width, dims.height)
        })
    }

    pub fn decrease_font_size(&mut self) {
        self.with_window(move |win| {
            let scale = win.fonts().get_font_scale();
            let dims = win.get_dimensions();
            win.scaling_changed(Some(scale * 0.9), None, dims.width, dims.height)
        })
    }

    pub fn reset_font_size(&mut self) {
        self.with_window(move |win| {
            let dims = win.get_dimensions();
            win.scaling_changed(Some(1.0), None, dims.width, dims.height)
        })
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
    writer: &'a mut std::io::Write,
    host: &'a mut HostImpl<H>,
}

impl<'a, H: HostHelper> TabHost<'a, H> {
    pub fn new(writer: &'a mut std::io::Write, host: &'a mut HostImpl<H>) -> Self {
        Self { writer, host }
    }
}

impl<'a, H: HostHelper> term::TerminalHost for TabHost<'a, H> {
    fn writer(&mut self) -> &mut std::io::Write {
        &mut self.writer
    }

    fn click_link(&mut self, link: &Arc<Hyperlink>) {
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

    fn activate_tab(&mut self, tab: usize) {
        self.host.activate_tab(tab)
    }

    fn activate_tab_relative(&mut self, tab: isize) {
        self.host.activate_tab_relative(tab)
    }

    fn increase_font_size(&mut self) {
        self.host.increase_font_size()
    }

    fn decrease_font_size(&mut self) {
        self.host.decrease_font_size()
    }

    fn reset_font_size(&mut self) {
        self.host.reset_font_size()
    }
}
