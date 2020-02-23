use crate::mux::tab::TabId;
use crate::server::client::Client;
use crate::server::codec::*;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use term::{MouseButton, MouseEvent, MouseEventKind};

pub struct MouseState {
    pending: AtomicBool,
    queue: VecDeque<MouseEvent>,
    client: Client,
    remote_tab_id: TabId,
}

impl MouseState {
    pub fn new(remote_tab_id: TabId, client: Client) -> Self {
        Self {
            remote_tab_id,
            client,
            pending: AtomicBool::new(false),
            queue: VecDeque::new(),
        }
    }

    pub fn append(&mut self, event: MouseEvent) {
        if let Some(last) = self.queue.back_mut() {
            if last.modifiers == event.modifiers {
                if last.kind == MouseEventKind::Move
                    && event.kind == MouseEventKind::Move
                    && last.button == event.button
                {
                    // Collapse any interim moves and just buffer up
                    // the last of them
                    *last = event;
                    return;
                }

                // Similarly, for repeated wheel scrolls, add up the deltas
                // rather than swamping the queue
                match (&last.button, &event.button) {
                    (MouseButton::WheelUp(a), MouseButton::WheelUp(b)) => {
                        last.button = MouseButton::WheelUp(a + b);
                        return;
                    }
                    (MouseButton::WheelDown(a), MouseButton::WheelDown(b)) => {
                        last.button = MouseButton::WheelDown(a + b);
                        return;
                    }
                    _ => {}
                }
            }
        }
        self.queue.push_back(event);
        log::trace!("MouseEvent {}: queued", self.queue.len());
    }

    fn pop(&mut self) -> Option<MouseEvent> {
        if !self.pending.load(Ordering::SeqCst) {
            self.queue.pop_front()
        } else {
            None
        }
    }

    pub fn next(state: Rc<RefCell<Self>>) -> bool {
        let mut mouse = state.borrow_mut();
        if let Some(event) = mouse.pop() {
            let client = mouse.client.clone();

            let state = Rc::clone(&state);
            mouse.pending.store(true, Ordering::SeqCst);
            let remote_tab_id = mouse.remote_tab_id;

            promise::spawn::spawn(async move {
                client
                    .mouse_event(SendMouseEvent {
                        tab_id: remote_tab_id,
                        event,
                    })
                    .await
                    .ok();

                let mouse = state.borrow_mut();
                mouse.pending.store(false, Ordering::SeqCst);
                drop(mouse);

                Self::next(Rc::clone(&state));
                Ok::<(), anyhow::Error>(())
            });
            true
        } else {
            false
        }
    }
}
