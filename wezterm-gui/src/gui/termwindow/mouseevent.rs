use crate::gui::tabbar::TabBarItem;
use crate::gui::termwindow::keyevent::window_mods_to_termwiz_mods;
use crate::gui::termwindow::{ScrollHit, TMB};
use ::window::{
    Modifiers, MouseButtons as WMB, MouseCursor, MouseEvent, MouseEventKind as WMEK, MousePress,
    WindowOps,
};
use config::keyassignment::{MouseEventTrigger, SpawnTabDomain};
use mux::pane::Pane;
use mux::tab::SplitDirection;
use mux::Mux;
use std::convert::TryInto;
use std::ops::Sub;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use wezterm_term::input::MouseEventKind as TMEK;
use wezterm_term::{LastMouseClick, StableRowIndex};

impl super::TermWindow {
    pub fn mouse_event_impl(&mut self, event: &MouseEvent, context: &dyn WindowOps) {
        let pane = match self.get_active_pane_or_overlay() {
            Some(pane) => pane,
            None => return,
        };

        let config = &self.config;
        // Round the x coordinate so that we're a bit more forgiving of
        // the horizontal position when selecting cells
        let x = ((event
            .coords
            .x
            .sub(config.window_padding.left as isize)
            .max(0) as f32)
            / self.render_metrics.cell_size.width as f32)
            .round()
            .trunc() as usize;
        // But don't round the y coordinate as that is more annoying
        let y = (event
            .coords
            .y
            .sub(config.window_padding.top as isize)
            .max(0)
            / self.render_metrics.cell_size.height) as i64;

        let first_line_offset = if self.show_tab_bar { 1 } else { 0 };
        self.last_mouse_coords = (x, y);

        let in_tab_bar = self.show_tab_bar && y == 0 && event.coords.y >= 0;
        let in_scroll_bar = self.show_scroll_bar && x >= self.terminal_size.cols as usize;
        // y position relative to top of viewport (not including tab bar)
        let term_y = y.saturating_sub(first_line_offset);

        match event.kind {
            WMEK::Release(ref press) => {
                self.current_mouse_button = None;
                if press == &MousePress::Left && self.scroll_drag_start.take().is_some() {
                    // Completed a scrollbar drag
                    return;
                }
                if press == &MousePress::Left && self.split_drag_start.take().is_some() {
                    // Completed a split drag
                    return;
                }
            }

            WMEK::Press(ref press) => {
                if let Some(focused) = self.focused.as_ref() {
                    if focused.elapsed() <= Duration::from_millis(200) {
                        log::trace!("discard mouse click because it focused the window");
                        return;
                    }
                }

                // Perform click counting
                let button = mouse_press_to_tmb(press);

                let click = match self.last_mouse_click.take() {
                    None => LastMouseClick::new(button),
                    Some(click) => click.add(button),
                };
                self.last_mouse_click = Some(click);
                self.current_mouse_button = Some(press.clone());
            }

            WMEK::VertWheel(amount) if !pane.is_mouse_grabbed() && !pane.is_alt_screen_active() => {
                // adjust viewport
                let dims = pane.get_dimensions();
                let position = self
                    .get_viewport(pane.pane_id())
                    .unwrap_or(dims.physical_top)
                    .saturating_sub(amount.into());
                self.set_viewport(pane.pane_id(), Some(position), dims);
                context.invalidate();
                return;
            }

            WMEK::Move => {
                let current_viewport = self.get_viewport(pane.pane_id());
                if let Some(from_top) = self.scroll_drag_start.as_ref() {
                    // Dragging the scroll bar
                    let pane = match self.get_active_pane_or_overlay() {
                        Some(pane) => pane,
                        None => return,
                    };

                    let dims = pane.get_dimensions();

                    let effective_thumb_top =
                        event.coords.y.saturating_sub(*from_top).max(0) as usize;

                    // Convert thumb top into a row index by reversing the math
                    // in ScrollHit::thumb
                    let row = ScrollHit::thumb_top_to_scroll_top(
                        effective_thumb_top,
                        &*pane,
                        current_viewport,
                        self.terminal_size,
                        &self.dimensions,
                    );
                    self.set_viewport(pane.pane_id(), Some(row), dims);
                    context.invalidate();
                    return;
                }

                if let Some(split) = self.split_drag_start.take() {
                    let mux = Mux::get().unwrap();
                    let tab = match mux.get_active_tab_for_window(self.mux_window_id) {
                        Some(tab) => tab,
                        None => return,
                    };
                    let delta = match split.direction {
                        SplitDirection::Horizontal => {
                            (x as isize).saturating_sub(split.left as isize)
                        }
                        SplitDirection::Vertical => {
                            (term_y as isize).saturating_sub(split.top as isize)
                        }
                    };

                    if delta != 0 {
                        tab.resize_split_by(split.index, delta);

                        self.split_drag_start = tab.iter_splits().into_iter().nth(split.index);
                        context.invalidate();
                    } else {
                        self.split_drag_start.replace(split);
                    }

                    return;
                }
            }
            _ => {}
        }

        if in_tab_bar {
            self.mouse_event_tab_bar(x, event, context);
        } else if in_scroll_bar {
            self.mouse_event_scroll_bar(pane, event, context);
        } else {
            self.mouse_event_terminal(pane, x, term_y, event, context);
        }
    }

    pub fn mouse_event_tab_bar(&mut self, x: usize, event: &MouseEvent, context: &dyn WindowOps) {
        match event.kind {
            WMEK::Press(MousePress::Left) => match self.tab_bar.hit_test(x) {
                TabBarItem::Tab(tab_idx) => {
                    self.activate_tab(tab_idx as isize).ok();
                }
                TabBarItem::NewTabButton => {
                    self.spawn_tab(&SpawnTabDomain::CurrentPaneDomain);
                }
                TabBarItem::None => {}
            },
            WMEK::Press(MousePress::Middle) => match self.tab_bar.hit_test(x) {
                TabBarItem::Tab(tab_idx) => {
                    self.close_tab_idx(tab_idx).ok();
                }
                TabBarItem::NewTabButton | TabBarItem::None => {}
            },
            WMEK::Press(MousePress::Right) => match self.tab_bar.hit_test(x) {
                TabBarItem::Tab(_) => {
                    self.show_tab_navigator();
                }
                TabBarItem::NewTabButton => {
                    self.show_launcher();
                }
                TabBarItem::None => {}
            },
            _ => {}
        }
        self.update_title();
        context.set_cursor(Some(MouseCursor::Arrow));
    }

    pub fn mouse_event_scroll_bar(
        &mut self,
        pane: Rc<dyn Pane>,
        event: &MouseEvent,
        context: &dyn WindowOps,
    ) {
        if let WMEK::Press(MousePress::Left) = event.kind {
            let dims = pane.get_dimensions();
            let current_viewport = self.get_viewport(pane.pane_id());

            let hit_result = ScrollHit::test(
                event.coords.y,
                &*pane,
                current_viewport,
                self.terminal_size,
                &self.dimensions,
            );

            match hit_result {
                ScrollHit::Above => {
                    // Page up
                    self.set_viewport(
                        pane.pane_id(),
                        Some(
                            current_viewport
                                .unwrap_or(dims.physical_top)
                                .saturating_sub(self.terminal_size.rows.try_into().unwrap()),
                        ),
                        dims,
                    );
                    context.invalidate();
                }
                ScrollHit::Below => {
                    // Page down
                    self.set_viewport(
                        pane.pane_id(),
                        Some(
                            current_viewport
                                .unwrap_or(dims.physical_top)
                                .saturating_add(self.terminal_size.rows.try_into().unwrap()),
                        ),
                        dims,
                    );
                    context.invalidate();
                }
                ScrollHit::OnThumb(from_top) => {
                    // Start a scroll drag
                    self.scroll_drag_start = Some(from_top);
                }
            };
        }
        context.set_cursor(Some(MouseCursor::Arrow));
    }

    pub fn mouse_event_terminal(
        &mut self,
        mut pane: Rc<dyn Pane>,
        mut x: usize,
        mut y: i64,
        event: &MouseEvent,
        context: &dyn WindowOps,
    ) {
        let mut on_split = None;
        if y >= 0 {
            let y = y as usize;

            for split in self.get_splits() {
                on_split = match split.direction {
                    SplitDirection::Horizontal => {
                        if x == split.left && y >= split.top && y <= split.top + split.size {
                            Some(SplitDirection::Horizontal)
                        } else {
                            None
                        }
                    }
                    SplitDirection::Vertical => {
                        if y == split.top && x >= split.left && x <= split.left + split.size {
                            Some(SplitDirection::Vertical)
                        } else {
                            None
                        }
                    }
                };

                if on_split.is_some() {
                    if event.kind == WMEK::Press(MousePress::Left) {
                        context.set_cursor(on_split.map(|d| match d {
                            SplitDirection::Horizontal => MouseCursor::SizeLeftRight,
                            SplitDirection::Vertical => MouseCursor::SizeUpDown,
                        }));
                        self.split_drag_start.replace(split);
                        return;
                    }
                    break;
                }
            }
        }

        for pos in self.get_panes_to_render() {
            if y >= pos.top as i64
                && y <= (pos.top + pos.height) as i64
                && x >= pos.left
                && x <= pos.left + pos.width
            {
                if pane.pane_id() != pos.pane.pane_id() {
                    // We're over a pane that isn't active
                    match &event.kind {
                        WMEK::Press(_) => {
                            let mux = Mux::get().unwrap();
                            mux.get_active_tab_for_window(self.mux_window_id)
                                .map(|tab| tab.set_active_idx(pos.index));

                            pane = Rc::clone(&pos.pane);
                        }
                        WMEK::Move => {}
                        WMEK::Release(_) => {}
                        WMEK::VertWheel(_) => {}
                        WMEK::HorzWheel(_) => {}
                    }
                }
                x = x.saturating_sub(pos.left);
                y = y.saturating_sub(pos.top as i64);
                break;
            }
        }

        let dims = pane.get_dimensions();
        let stable_row = self
            .get_viewport(pane.pane_id())
            .unwrap_or(dims.physical_top)
            + y as StableRowIndex;

        self.last_mouse_terminal_coords = (x, stable_row); // FIXME: per-pane

        let (top, mut lines) = pane.get_lines(stable_row..stable_row + 1);
        let new_highlight = if top == stable_row {
            if let Some(line) = lines.get_mut(0) {
                if let Some(cell) = line.cells().get(x) {
                    cell.attrs().hyperlink().cloned()
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        match (self.current_highlight.as_ref(), new_highlight) {
            (Some(old_link), Some(new_link)) if Arc::ptr_eq(&old_link, &new_link) => {
                // Unchanged
            }
            (None, None) => {
                // Unchanged
            }
            (_, rhs) => {
                // We're hovering over a different URL, so invalidate and repaint
                // so that we render the underline correctly
                self.current_highlight = rhs;
                context.invalidate();
            }
        };

        context.set_cursor(Some(match on_split {
            Some(SplitDirection::Horizontal) => MouseCursor::SizeLeftRight,
            Some(SplitDirection::Vertical) => MouseCursor::SizeUpDown,
            None => {
                if self.current_highlight.is_some() {
                    // When hovering over a hyperlink, show an appropriate
                    // mouse cursor to give the cue that it is clickable
                    MouseCursor::Hand
                } else {
                    MouseCursor::Text
                }
            }
        }));

        let event_trigger_type = match &event.kind {
            WMEK::Press(press) => {
                let press = mouse_press_to_tmb(press);
                match self.last_mouse_click.as_ref() {
                    Some(LastMouseClick { streak, button, .. }) if *button == press => {
                        Some(MouseEventTrigger::Down {
                            streak: *streak,
                            button: press,
                        })
                    }
                    _ => None,
                }
            }
            WMEK::Release(press) => {
                let press = mouse_press_to_tmb(press);
                match self.last_mouse_click.as_ref() {
                    Some(LastMouseClick { streak, button, .. }) if *button == press => {
                        Some(MouseEventTrigger::Up {
                            streak: *streak,
                            button: press,
                        })
                    }
                    _ => None,
                }
            }
            WMEK::Move => {
                if let Some(LastMouseClick { streak, button, .. }) = self.last_mouse_click.as_ref()
                {
                    if Some(*button) == self.current_mouse_button.as_ref().map(mouse_press_to_tmb) {
                        Some(MouseEventTrigger::Drag {
                            streak: *streak,
                            button: *button,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            WMEK::VertWheel(_) | WMEK::HorzWheel(_) => None,
        };

        let ignore_grab_modifier = Modifiers::SHIFT;

        if !pane.is_mouse_grabbed() || event.modifiers.contains(ignore_grab_modifier) {
            if let Some(event_trigger_type) = event_trigger_type {
                let mut modifiers = event.modifiers;

                // Since we use shift to force assessing the mouse bindings, pretend
                // that shift is not one of the mods when the mouse is grabbed.
                if pane.is_mouse_grabbed() {
                    modifiers -= ignore_grab_modifier;
                }

                if let Some(action) = self
                    .input_map
                    .lookup_mouse(event_trigger_type.clone(), modifiers)
                {
                    self.perform_key_assignment(&pane, &action).ok();
                    return;
                }
            }
        }

        let mouse_event = wezterm_term::MouseEvent {
            kind: match event.kind {
                WMEK::Move => TMEK::Move,
                WMEK::VertWheel(_) | WMEK::HorzWheel(_) | WMEK::Press(_) => TMEK::Press,
                WMEK::Release(_) => TMEK::Release,
            },
            button: match event.kind {
                WMEK::Release(ref press) | WMEK::Press(ref press) => mouse_press_to_tmb(press),
                WMEK::Move => {
                    if event.mouse_buttons == WMB::LEFT {
                        TMB::Left
                    } else if event.mouse_buttons == WMB::RIGHT {
                        TMB::Right
                    } else if event.mouse_buttons == WMB::MIDDLE {
                        TMB::Middle
                    } else {
                        TMB::None
                    }
                }
                WMEK::VertWheel(amount) => {
                    if amount > 0 {
                        TMB::WheelUp(amount as usize)
                    } else {
                        TMB::WheelDown((-amount) as usize)
                    }
                }
                WMEK::HorzWheel(_) => TMB::None,
            },
            x,
            y,
            modifiers: window_mods_to_termwiz_mods(event.modifiers),
        };

        pane.mouse_event(mouse_event).ok();

        match event.kind {
            WMEK::Move => {}
            _ => {
                context.invalidate();
            }
        }
    }
}

fn mouse_press_to_tmb(press: &MousePress) -> TMB {
    match press {
        MousePress::Left => TMB::Left,
        MousePress::Right => TMB::Right,
        MousePress::Middle => TMB::Middle,
    }
}
