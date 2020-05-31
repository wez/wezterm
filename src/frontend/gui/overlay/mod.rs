use crate::frontend::gui::termwindow::TermWindow;
use crate::mux::tab::{Tab, TabId};
use crate::termwiztermtab::{allocate, TermWizTerminal};
use std::pin::Pin;
use std::rc::Rc;

mod copy;
mod launcher;
mod search;
mod tabnavigator;

pub use copy::CopyOverlay;
pub use launcher::launcher;
pub use search::SearchOverlay;
pub use tabnavigator::tab_navigator;

pub fn start_overlay<T, F>(
    term_window: &TermWindow,
    tab: &Rc<dyn Tab>,
    func: F,
) -> (
    Rc<dyn Tab>,
    Pin<Box<dyn std::future::Future<Output = Option<anyhow::Result<T>>>>>,
)
where
    T: Send + 'static,
    F: Send + 'static + FnOnce(TabId, TermWizTerminal) -> anyhow::Result<T>,
{
    let tab_id = tab.tab_id();
    let dims = tab.renderer().get_dimensions();
    let (tw_term, tw_tab) = allocate(dims.cols, dims.viewport_rows);

    let window = term_window.window.clone().unwrap();

    let future = promise::spawn::spawn_into_new_thread(move || {
        let res = func(tab_id, tw_term);
        TermWindow::schedule_cancel_overlay(window, tab_id);
        res
    });

    (Rc::new(tw_tab), Box::pin(future))
}
