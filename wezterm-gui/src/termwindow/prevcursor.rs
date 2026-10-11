use mux::renderable::StableCursorPosition;
use std::time::Instant;

#[derive(Clone)]
pub struct PrevCursorPos {
    pos: StableCursorPosition,
    when: Instant,
}

impl PrevCursorPos {
    pub fn new() -> Self {
        PrevCursorPos {
            pos: StableCursorPosition::default(),
            when: Instant::now(),
        }
    }

    /// Make the cursor look like it moved
    pub fn bump(&mut self) {
        self.when = Instant::now();
    }

    /// Update the cursor position if its different
    pub fn update(&mut self, newpos: &StableCursorPosition) {
        if &self.pos != newpos {
            self.pos = *newpos;
            self.when = Instant::now();
        }
    }

    /// When did the cursor last move?
    pub fn last_cursor_movement(&self) -> Instant {
        self.when
    }
}

/// Per-pane cursor render state for the post-process shader uniforms.
#[derive(Clone, Debug, Default)]
pub struct CursorRenderState {
    /// Cursor rect `{x, y, w, h}` in full-window pixel coords, or `None` if
    /// the cursor is hidden.
    pub current_cursor: Option<[f32; 4]>,
    /// Cursor color as RGBA normalized to `[0, 1]`.
    pub current_cursor_color: [f32; 4],
    pub previous_cursor: Option<[f32; 4]>,
    pub previous_cursor_color: [f32; 4],
    /// The `iTime` value at the last cursor change (position or color).
    pub cursor_change_time: f32,
}

impl CursorRenderState {
    /// Update if the rect or color changed, shifting current to previous.
    pub fn update(&mut self, rect: Option<[f32; 4]>, color: [f32; 4], time: f32) {
        if rect != self.current_cursor || color != self.current_cursor_color {
            self.previous_cursor = self.current_cursor;
            self.previous_cursor_color = self.current_cursor_color;
            self.current_cursor = rect;
            self.current_cursor_color = color;
            self.cursor_change_time = time;
        }
    }
}
