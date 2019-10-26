#![cfg(feature = "enable-winit")]
use super::window::TerminalWindow;
use crate::font::{FontConfiguration, FontSystemSelection};
use crate::frontend::guicommon::clipboard::SystemClipboard;
use crate::frontend::{front_end, gui_executor};
use crate::keyassignment::{KeyAssignment, KeyMap};
use crate::mux::tab::Tab;
use crate::mux::Mux;
use failure::Error;
use failure::Fallible;
use log::error;
use portable_pty::PtySize;
use promise::Future;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::sync::Arc;
use term::terminal::Clipboard;
use term::{KeyCode, KeyModifiers};
use termwiz::hyperlink::Hyperlink;

pub trait HostHelper {
    fn with_window<F: Send + 'static + Fn(&mut dyn TerminalWindow) -> Result<(), Error>>(
        &self,
        func: F,
    );
    fn toggle_full_screen(&mut self);
}

pub struct HostImpl<H: HostHelper> {
    helper: H,
    clipboard: Arc<dyn Clipboard>,
    keys: KeyMap,
}

impl<H: HostHelper> HostImpl<H> {
    pub fn new(helper: H) -> Self {
        Self {
            helper,
            clipboard: Arc::new(SystemClipboard::new()),
            keys: KeyMap::new(),
        }
    }

    pub fn get_clipboard(&mut self) -> Fallible<Arc<dyn Clipboard>> {
        Ok(Arc::clone(&self.clipboard))
    }

    pub fn spawn_new_window(&mut self) {
        Future::with_executor(gui_executor().unwrap(), move || {
            let mux = Mux::get().unwrap();
            let fonts = Rc::new(FontConfiguration::new(
                Arc::clone(mux.config()),
                FontSystemSelection::get_default(),
            ));
            let window_id = mux.new_empty_window();
            let tab = mux
                .default_domain()
                .spawn(PtySize::default(), None, window_id)?;
            let front_end = front_end().expect("to be called on gui thread");
            front_end.spawn_new_window(mux.config(), &fonts, &tab, window_id)?;
            Ok(())
        });
    }

    pub fn perform_key_assignment(
        &mut self,
        tab: &dyn Tab,
        assignment: &KeyAssignment,
    ) -> Fallible<()> {
        use KeyAssignment::*;
        match assignment {
            SpawnTab(spawn_where) => {
                let spawn_where = spawn_where.clone();
                self.with_window(move |win| win.spawn_tab(&spawn_where).map(|_| ()))
            }
            SpawnWindow => self.spawn_new_window(),
            ToggleFullScreen => self.toggle_full_screen(),
            Copy => {
                // Nominally copy, but that is implicit, so NOP
            }
            Paste => {
                tab.trickle_paste(self.get_clipboard()?.get_contents()?)?;
            }
            ActivateTabRelative(n) => self.activate_tab_relative(*n),
            DecreaseFontSize => self.decrease_font_size(),
            IncreaseFontSize => self.increase_font_size(),
            ResetFontSize => self.reset_font_size(),
            ActivateTab(n) => self.activate_tab(*n),
            SendString(s) => tab.writer().write_all(s.as_bytes())?,
            Hide => self.hide_window(),
            Show => self.show_window(),
            CloseCurrentTab => self.close_current_tab(),
            Nop => {}
        }
        Ok(())
    }

    pub fn process_gui_shortcuts(
        &mut self,
        tab: &dyn Tab,
        mods: KeyModifiers,
        key: KeyCode,
    ) -> Result<bool, Error> {
        if let Some(assignment) = self.keys.lookup(key, mods) {
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

    pub fn close_current_tab(&mut self) {
        self.with_window(move |win| {
            let mux = Mux::get().unwrap();
            let tab = match mux.get_active_tab_for_window(win.get_mux_window_id()) {
                Some(tab) => tab,
                None => return Ok(()),
            };
            mux.remove_tab(tab.tab_id());
            if let Some(mut win) = mux.get_window_mut(win.get_mux_window_id()) {
                win.remove_by_id(tab.tab_id());
            }
            win.activate_tab_relative(0)
        });
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
    writer: &'a mut dyn std::io::Write,
    host: &'a mut HostImpl<H>,
}

impl<'a, H: HostHelper> TabHost<'a, H> {
    pub fn new(writer: &'a mut dyn std::io::Write, host: &'a mut HostImpl<H>) -> Self {
        Self { writer, host }
    }
}

impl<'a, H: HostHelper> term::TerminalHost for TabHost<'a, H> {
    fn writer(&mut self) -> &mut dyn std::io::Write {
        &mut self.writer
    }

    fn click_link(&mut self, link: &Arc<Hyperlink>) {
        match open::that(link.uri()) {
            Ok(_) => {}
            Err(err) => error!("failed to open {}: {:?}", link.uri(), err),
        }
    }

    fn get_clipboard(&mut self) -> Fallible<Arc<dyn Clipboard>> {
        self.host.get_clipboard()
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
