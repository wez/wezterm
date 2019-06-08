use super::window::TerminalWindow;
use crate::font::{FontConfiguration, FontSystemSelection};
use crate::frontend::{front_end, gui_executor};
use crate::mux::tab::{Tab, TabId};
use crate::mux::Mux;
use clipboard::{ClipboardContext, ClipboardProvider};
use failure::Fallible;
use failure::{format_err, Error};
use portable_pty::PtySize;
use promise::Future;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use term::{KeyCode, KeyModifiers};
use termwiz::hyperlink::Hyperlink;

#[derive(Debug, Clone)]
pub enum KeyAssignment {
    SpawnTab,
    SpawnWindow,
    ToggleFullScreen,
    Copy,
    Paste,
    ActivateTabRelative(isize),
    IncreaseFontSize,
    DecreaseFontSize,
    ResetFontSize,
    ActivateTab(usize),
    SendString(String),
    Nop,
    Hide,
    Show,
}

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
    keys: KeyMap,
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

type KeyMap = HashMap<(KeyCode, KeyModifiers), KeyAssignment>;

fn key_bindings() -> KeyMap {
    let mux = Mux::get().unwrap();
    let mut map = mux
        .config()
        .key_bindings()
        .expect("keys section of config to be valid");

    macro_rules! m {
        ($([$mod:expr, $code:expr, $action:expr]),* $(,)?) => {
            $(
            map.entry(($code, $mod)).or_insert($action);
            )*
        };
    };

    use KeyAssignment::*;

    // Apply the default bindings; if the user has already mapped
    // a given entry then that will take precedence.
    m!(
        // Clipboard
        [KeyModifiers::SUPER, KeyCode::Char('c'), Copy],
        [KeyModifiers::SUPER, KeyCode::Char('v'), Paste],
        [KeyModifiers::SHIFT, KeyCode::Insert, Paste],
        // Window management
        [KeyModifiers::SUPER, KeyCode::Char('m'), Hide],
        [KeyModifiers::SUPER, KeyCode::Char('n'), SpawnWindow],
        [KeyModifiers::ALT, KeyCode::Char('\n'), ToggleFullScreen],
        [KeyModifiers::ALT, KeyCode::Char('\r'), ToggleFullScreen],
        [KeyModifiers::ALT, KeyCode::Enter, ToggleFullScreen],
        // Font size manipulation
        [KeyModifiers::SUPER, KeyCode::Char('-'), DecreaseFontSize],
        [KeyModifiers::CTRL, KeyCode::Char('-'), DecreaseFontSize],
        [KeyModifiers::SUPER, KeyCode::Char('='), IncreaseFontSize],
        [KeyModifiers::CTRL, KeyCode::Char('='), IncreaseFontSize],
        [KeyModifiers::SUPER, KeyCode::Char('0'), ResetFontSize],
        [KeyModifiers::CTRL, KeyCode::Char('0'), ResetFontSize],
        // Tab navigation and management
        [KeyModifiers::SUPER, KeyCode::Char('t'), SpawnTab],
        [KeyModifiers::SUPER, KeyCode::Char('1'), ActivateTab(0)],
        [KeyModifiers::SUPER, KeyCode::Char('2'), ActivateTab(1)],
        [KeyModifiers::SUPER, KeyCode::Char('3'), ActivateTab(2)],
        [KeyModifiers::SUPER, KeyCode::Char('4'), ActivateTab(3)],
        [KeyModifiers::SUPER, KeyCode::Char('5'), ActivateTab(4)],
        [KeyModifiers::SUPER, KeyCode::Char('6'), ActivateTab(5)],
        [KeyModifiers::SUPER, KeyCode::Char('7'), ActivateTab(6)],
        [KeyModifiers::SUPER, KeyCode::Char('8'), ActivateTab(7)],
        [KeyModifiers::SUPER, KeyCode::Char('9'), ActivateTab(8)],
        [
            KeyModifiers::SUPER | KeyModifiers::SHIFT,
            KeyCode::Char('['),
            ActivateTabRelative(-1)
        ],
        [
            KeyModifiers::SUPER | KeyModifiers::SHIFT,
            KeyCode::Char('{'),
            ActivateTabRelative(-1)
        ],
        [
            KeyModifiers::SUPER | KeyModifiers::SHIFT,
            KeyCode::Char(']'),
            ActivateTabRelative(1)
        ],
        [
            KeyModifiers::SUPER | KeyModifiers::SHIFT,
            KeyCode::Char('}'),
            ActivateTabRelative(1)
        ],
    );

    map
}

impl<H: HostHelper> HostImpl<H> {
    pub fn new(helper: H) -> Self {
        Self {
            helper,
            clipboard: None,
            keys: key_bindings(),
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

    pub fn spawn_new_window(&mut self) {
        Future::with_executor(gui_executor().unwrap(), move || {
            let mux = Mux::get().unwrap();
            let fonts = Rc::new(FontConfiguration::new(
                Arc::clone(mux.config()),
                FontSystemSelection::get_default(),
            ));
            let tab = mux.default_domain().spawn(PtySize::default(), None)?;
            let front_end = front_end().expect("to be called on gui thread");
            front_end.spawn_new_window(mux.config(), &fonts, &tab)?;
            Ok(())
        });
    }

    pub fn perform_key_assignment(
        &mut self,
        tab: &Tab,
        assignment: &KeyAssignment,
    ) -> Fallible<()> {
        use KeyAssignment::*;
        match assignment {
            SpawnTab => self.with_window(|win| win.spawn_tab().map(|_| ())),
            SpawnWindow => self.spawn_new_window(),
            ToggleFullScreen => self.toggle_full_screen(),
            Copy => {
                // Nominally copy, but that is implicit, so NOP
            }
            Paste => {
                let text = self.get_clipboard()?;
                if text.len() <= PASTE_CHUNK_SIZE {
                    // Send it all now
                    tab.send_paste(&text)?;
                } else {
                    // It's pretty heavy, so we trickle it into the pty
                    tab.send_paste(&text[0..PASTE_CHUNK_SIZE])?;
                    trickle_paste(tab.tab_id(), text);
                }
            }
            ActivateTabRelative(n) => self.activate_tab_relative(*n),
            DecreaseFontSize => self.decrease_font_size(),
            IncreaseFontSize => self.increase_font_size(),
            ResetFontSize => self.reset_font_size(),
            ActivateTab(n) => self.activate_tab(*n),
            SendString(s) => tab.writer().write_all(s.as_bytes())?,
            Hide => self.hide_window(),
            Show => self.show_window(),
            Nop => {}
        }
        Ok(())
    }

    pub fn process_gui_shortcuts(
        &mut self,
        tab: &Tab,
        mods: KeyModifiers,
        key: KeyCode,
    ) -> Result<bool, Error> {
        if let Some(assignment) = self.keys.get(&(key, mods)).cloned() {
            self.perform_key_assignment(tab, &assignment)?;
            Ok(true)
        } else {
            Ok(false)
        }
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

    pub fn hide_window(&mut self) {
        self.with_window(move |win| {
            win.hide_window();
            Ok(())
        });
    }

    pub fn show_window(&mut self) {
        self.with_window(move |win| {
            win.show_window();
            Ok(())
        });
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
