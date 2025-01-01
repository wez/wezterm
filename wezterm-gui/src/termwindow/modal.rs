use crate::termwindow::box_model::ComputedElement;
use crate::TermWindow;
use config::keyassignment::KeyAssignment;
use downcast_rs::{impl_downcast, Downcast};
use std::cell::Ref;
use wezterm_term::{KeyCode, KeyModifiers, MouseEvent};

pub trait Modal: Downcast {
    fn perform_assignment(
        &self,
        _assignment: &KeyAssignment,
        _term_window: &mut TermWindow,
    ) -> bool {
        false
    }
    fn mouse_event(&self, event: MouseEvent, term_window: &mut TermWindow) -> anyhow::Result<()>;
    fn key_down(
        &self,
        key: KeyCode,
        mods: KeyModifiers,
        term_window: &mut TermWindow,
    ) -> anyhow::Result<bool>;
    fn computed_element(
        &self,
        term_window: &mut TermWindow,
    ) -> anyhow::Result<Ref<[ComputedElement]>>;
    fn reconfigure(&self, term_window: &mut TermWindow);
}
impl_downcast!(Modal);
