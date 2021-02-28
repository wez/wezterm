use crate::termwindow::TermWindow;
use mux::pane::{Pane, PaneId};
use mux::tab::{Tab, TabId};
use mux::termwiztermtab::{allocate, TermWizTerminal};
use portable_pty::PtySize;
use std::pin::Pin;
use std::rc::Rc;

mod confirm_close_pane;
mod copy;
mod launcher;
mod search;
mod tabnavigator;

pub use confirm_close_pane::confirm_close_pane;
pub use confirm_close_pane::confirm_close_tab;
pub use confirm_close_pane::confirm_close_window;
pub use confirm_close_pane::confirm_quit_program;
pub use copy::CopyOverlay;
pub use launcher::launcher;
pub use search::SearchOverlay;
pub use tabnavigator::tab_navigator;

pub fn start_overlay<T, F>(
    term_window: &TermWindow,
    tab: &Rc<Tab>,
    func: F,
) -> (
    Rc<dyn Pane>,
    Pin<Box<dyn std::future::Future<Output = anyhow::Result<T>>>>,
)
where
    T: Send + 'static,
    F: Send + 'static + FnOnce(TabId, TermWizTerminal) -> anyhow::Result<T>,
{
    let tab_id = tab.tab_id();
    let tab_size = tab.get_size();
    let (tw_term, tw_tab) = allocate(tab_size);

    let window = term_window.window.clone().unwrap();

    let overlay_pane_id = tw_tab.pane_id();

    let future = promise::spawn::spawn_into_new_thread(move || {
        let res = func(tab_id, tw_term);
        TermWindow::schedule_cancel_overlay(window, tab_id, Some(overlay_pane_id));
        res
    });

    (tw_tab, Box::pin(future))
}

pub fn start_overlay_pane<T, F>(
    term_window: &TermWindow,
    pane: &Rc<dyn Pane>,
    func: F,
) -> (
    Rc<dyn Pane>,
    Pin<Box<dyn std::future::Future<Output = anyhow::Result<T>>>>,
)
where
    T: Send + 'static,
    F: Send + 'static + FnOnce(PaneId, TermWizTerminal) -> anyhow::Result<T>,
{
    let pane_id = pane.pane_id();
    let dims = pane.get_dimensions();
    let size = PtySize {
        cols: dims.cols as u16,
        rows: dims.viewport_rows as u16,
        pixel_width: term_window.render_metrics.cell_size.width as u16 * dims.cols as u16,
        pixel_height: term_window.render_metrics.cell_size.height as u16
            * dims.viewport_rows as u16,
    };
    let (tw_term, tw_tab) = allocate(size);

    let window = term_window.window.clone().unwrap();

    let future = promise::spawn::spawn_into_new_thread(move || {
        let res = func(pane_id, tw_term);
        TermWindow::schedule_cancel_overlay_for_pane(window, pane_id);
        res
    });

    (tw_tab, Box::pin(future))
}
