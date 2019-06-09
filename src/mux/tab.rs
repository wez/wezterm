use crate::mux::domain::DomainId;
use crate::mux::renderable::Renderable;
use downcast_rs::{impl_downcast, Downcast};
use failure::Fallible;
use portable_pty::PtySize;
use std::cell::RefMut;
use term::color::ColorPalette;
use term::{KeyCode, KeyModifiers, MouseEvent, TerminalHost};

static TAB_ID: ::std::sync::atomic::AtomicUsize = ::std::sync::atomic::AtomicUsize::new(0);
pub type TabId = usize;

pub fn alloc_tab_id() -> TabId {
    TAB_ID.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed)
}

pub trait Tab: Downcast {
    fn tab_id(&self) -> TabId;
    fn renderer(&self) -> RefMut<dyn Renderable>;
    fn get_title(&self) -> String;
    fn send_paste(&self, text: &str) -> Fallible<()>;
    fn reader(&self) -> Fallible<Box<dyn std::io::Read + Send>>;
    fn writer(&self) -> RefMut<dyn std::io::Write>;
    fn resize(&self, size: PtySize) -> Fallible<()>;
    fn key_down(&self, key: KeyCode, mods: KeyModifiers) -> Fallible<()>;
    fn mouse_event(&self, event: MouseEvent, host: &mut dyn TerminalHost) -> Fallible<()>;
    fn advance_bytes(&self, buf: &[u8], host: &mut dyn TerminalHost);
    fn is_dead(&self) -> bool;
    fn palette(&self) -> ColorPalette;
    fn domain_id(&self) -> DomainId;
}
impl_downcast!(Tab);
