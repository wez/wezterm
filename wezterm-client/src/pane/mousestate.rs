use crate::client::Client;
use codec::*;
use mux::tab::TabId;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use wezterm_term::{MouseButton, MouseEvent, MouseEventKind};

pub struct MouseState {
    pending: AtomicBool,
    queue: VecDeque<MouseEvent>,
    client: Client,
    remote_pane_id: TabId,
}

impl MouseState {
    pub fn new(remote_pane_id: TabId, client: Client) -> Self {
        Self {
            remote_pane_id,
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
                    (MouseButton::WheelLeft(a), MouseButton::WheelLeft(b)) => {
                        last.button = MouseButton::WheelLeft(a + b);
                        return;
                    }
                    (MouseButton::WheelRight(a), MouseButton::WheelRight(b)) => {
                        last.button = MouseButton::WheelRight(a + b);
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

    pub fn next(state: Arc<Mutex<Self>>) -> bool {
        let mut mouse = state.lock();
        if let Some(event) = mouse.pop() {
            let client = mouse.client.clone();

            let state = Arc::clone(&state);
            mouse.pending.store(true, Ordering::SeqCst);
            let remote_pane_id = mouse.remote_pane_id;

            promise::spawn::spawn(async move {
                client
                    .mouse_event(SendMouseEvent {
                        pane_id: remote_pane_id,
                        event,
                    })
                    .await
                    .ok();

                let mouse = state.lock();
                mouse.pending.store(false, Ordering::SeqCst);
                drop(mouse);

                Self::next(Arc::clone(&state));
                Ok::<(), anyhow::Error>(())
            })
            .detach();
            true
        } else {
            false
        }
    }
}
