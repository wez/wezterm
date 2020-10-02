use crate::frontend::gui::termwindow::TermWindow;
use crate::mux::pane::{Pane, PaneId};
use crate::mux::tab::{Tab, TabId};
use crate::termwiztermtab::{allocate, TermWizTerminal};
use std::pin::Pin;
use std::rc::Rc;

mod confirm_close_pane;
mod copy;
mod launcher;
mod search;
mod tabnavigator;

pub use confirm_close_pane::confirm_close_pane;
pub use confirm_close_pane::confirm_close_tab;
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
    Pin<Box<dyn std::future::Future<Output = Option<anyhow::Result<T>>>>>,
)
where
    T: Send + 'static,
    F: Send + 'static + FnOnce(TabId, TermWizTerminal) -> anyhow::Result<T>,
{
    let tab_id = tab.tab_id();
    let tab_size = tab.get_size();
    let (tw_term, tw_tab) = allocate(tab_size.cols.into(), tab_size.rows.into());

    let window = term_window.window.clone().unwrap();

    let future = promise::spawn::spawn_into_new_thread(move || {
        let res = func(tab_id, tw_term);
        TermWindow::schedule_cancel_overlay(window, tab_id);
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
    Pin<Box<dyn std::future::Future<Output = Option<anyhow::Result<T>>>>>,
)
where
    T: Send + 'static,
    F: Send + 'static + FnOnce(PaneId, TermWizTerminal) -> anyhow::Result<T>,
{
    let pane_id = pane.pane_id();
    let dims = pane.renderer().get_dimensions();
    let (tw_term, tw_tab) = allocate(dims.cols.into(), dims.viewport_rows.into());

    let window = term_window.window.clone().unwrap();

    let future = promise::spawn::spawn_into_new_thread(move || {
        let res = func(pane_id, tw_term);
        TermWindow::schedule_cancel_overlay_for_pane(window, pane_id);
        res
    });

    (tw_tab, Box::pin(future))
}
