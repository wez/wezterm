use std::time::{Duration, Instant};

use super::VisibleRowIndex;

bitflags! {
    #[derive(Default)]
    pub struct KeyModifiers :u8{
        const CTRL = 1;
        const ALT = 2;
        const META = 4;
        const SUPER = 8;
        const SHIFT = 16;
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum KeyCode {
    Char(char),
    Unknown,
    Control,
    Alt,
    Meta,
    Super,
    Hyper,
    Shift,
    Left,
    Up,
    Right,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    Insert,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    WheelUp,
    WheelDown,
    None,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MouseEventKind {
    Press,
    Release,
    Move,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct MouseEvent {
    pub kind: MouseEventKind,
    pub x: usize,
    pub y: VisibleRowIndex,
    pub button: MouseButton,
    pub modifiers: KeyModifiers,
}

/// This is a little helper that keeps track of the "click streak",
/// which is the number of successive clicks of the same mouse button
/// within the CLICK_INTERVAL.  The streak is reset to 1 each time
/// the mouse button differs from the last click, or when the elapsed
/// time exceeds CLICK_INTERVAL.
#[derive(Debug)]
pub struct LastMouseClick {
    button: MouseButton,
    time: Instant,
    pub streak: usize,
}

/// The multi-click interval, measured in milliseconds
const CLICK_INTERVAL: u64 = 500;

impl LastMouseClick {
    pub fn new(button: MouseButton) -> Self {
        Self {
            button,
            time: Instant::now(),
            streak: 1,
        }
    }

    pub fn add(&self, button: MouseButton) -> Self {
        let now = Instant::now();
        let streak = if button == self.button
            && now.duration_since(self.time) <= Duration::from_millis(CLICK_INTERVAL)
        {
            self.streak + 1
        } else {
            1
        };
        Self {
            button,
            time: now,
            streak,
        }
    }
}
