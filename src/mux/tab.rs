use crate::mux::domain::DomainId;
use crate::mux::renderable::Renderable;
use failure::Error;
use portable_pty::PtySize;
use std::cell::RefMut;
use term::color::ColorPalette;
use term::{KeyCode, KeyModifiers, MouseEvent, TerminalHost};

static TAB_ID: ::std::sync::atomic::AtomicUsize = ::std::sync::atomic::AtomicUsize::new(0);
pub type TabId = usize;

pub fn alloc_tab_id() -> TabId {
    TAB_ID.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed)
}

pub trait Tab {
    fn tab_id(&self) -> TabId;
    fn renderer(&self) -> RefMut<dyn Renderable>;
    fn get_title(&self) -> String;
    fn send_paste(&self, text: &str) -> Result<(), Error>;
    fn reader(&self) -> Result<Box<dyn std::io::Read + Send>, Error>;
    fn writer(&self) -> RefMut<dyn std::io::Write>;
    fn resize(&self, size: PtySize) -> Result<(), Error>;
    fn key_down(&self, key: KeyCode, mods: KeyModifiers) -> Result<(), Error>;
    fn mouse_event(&self, event: MouseEvent, host: &mut dyn TerminalHost) -> Result<(), Error>;
    fn advance_bytes(&self, buf: &[u8], host: &mut dyn TerminalHost);
    fn is_dead(&self) -> bool;
    fn palette(&self) -> ColorPalette;
    fn domain_id(&self) -> DomainId;
}
