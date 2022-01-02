// clippy hates bitflags
#![cfg_attr(
    feature = "cargo-clippy",
    allow(clippy::suspicious_arithmetic_impl, clippy::redundant_field_names)
)]

use super::VisibleRowIndex;
#[cfg(feature = "use_serde")]
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

pub use termwiz::input::KeyCode;
pub use termwiz::input::Modifiers as KeyModifiers;

#[cfg_attr(feature = "use_serde", derive(Deserialize, Serialize))]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    WheelUp(usize),
    WheelDown(usize),
    None,
}

#[cfg_attr(feature = "use_serde", derive(Deserialize, Serialize))]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MouseEventKind {
    Press,
    Release,
    Move,
}

#[cfg_attr(feature = "use_serde", derive(Deserialize, Serialize))]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct MouseEvent {
    pub kind: MouseEventKind,
    pub x: usize,
    pub y: VisibleRowIndex,
    pub x_pixel_offset: usize,
    pub y_pixel_offset: usize,
    pub button: MouseButton,
    pub modifiers: KeyModifiers,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ClickPosition {
    pub column: usize,
    pub row: i64,
    pub x_pixel_offset: usize,
    pub y_pixel_offset: usize,
}

/// This is a little helper that keeps track of the "click streak",
/// which is the number of successive clicks of the same mouse button
/// within the `CLICK_INTERVAL`.  The streak is reset to 1 each time
/// the mouse button differs from the last click, or when the elapsed
/// time exceeds `CLICK_INTERVAL`, or when the cursor position
/// changes to a different character cell.
#[derive(Debug, Clone)]
pub struct LastMouseClick {
    pub button: MouseButton,
    pub position: ClickPosition,
    time: Instant,
    pub streak: usize,
}

/// The multi-click interval, measured in milliseconds
const CLICK_INTERVAL: u64 = 500;

impl LastMouseClick {
    pub fn new(button: MouseButton, position: ClickPosition) -> Self {
        Self {
            button,
            position,
            time: Instant::now(),
            streak: 1,
        }
    }

    pub fn add(&self, button: MouseButton, position: ClickPosition) -> Self {
        let now = Instant::now();
        let streak = if button == self.button
            && position == self.position
            && now.duration_since(self.time) <= Duration::from_millis(CLICK_INTERVAL)
        {
            self.streak + 1
        } else {
            1
        };
        Self {
            button,
            position,
            time: now,
            streak,
        }
    }
}
