use std::time::Instant;

#[derive(Clone, Debug)]
pub struct CursorTrailState {
    /// 4 animated corners of the single continuous trail quad (matching Kitty's architecture):
    /// 0: top-right, 1: bottom-right, 2: bottom-left, 3: top-left
    pub corner_x: [f32; 4],
    pub corner_y: [f32; 4],

    /// Current target cursor position
    pub target_x: f32,
    pub target_y: f32,

    pub opacity: f32,
    pub is_animating: bool,

    last_update: Instant,
    initialized: bool,
}

impl Default for CursorTrailState {
    fn default() -> Self {
        Self::new()
    }
}

impl CursorTrailState {
    pub fn new() -> Self {
        Self {
            corner_x: [0.0; 4],
            corner_y: [0.0; 4],
            target_x: 0.0,
            target_y: 0.0,
            opacity: 0.0,
            is_animating: false,
            last_update: Instant::now(),
            initialized: false,
        }
    }

    pub fn set_target(
        &mut self,
        target_x: f32,
        target_y: f32,
        cell_w: f32,
        cell_h: f32,
        max_snap_distance: f32,
    ) {
        let target_r = target_x + cell_w;
        let target_b = target_y + cell_h;

        if !self.initialized {
            self.target_x = target_x;
            self.target_y = target_y;
            self.corner_x = [target_r, target_r, target_x, target_x];
            self.corner_y = [target_y, target_b, target_b, target_y];
            self.opacity = 0.0;
            self.is_animating = false;
            self.initialized = true;
            self.last_update = Instant::now();
            return;
        }

        let dist_x = (self.target_x - target_x).abs();
        let dist_y = (self.target_y - target_y).abs();

        if dist_x > max_snap_distance || dist_y > max_snap_distance {
            self.target_x = target_x;
            self.target_y = target_y;
            self.corner_x = [target_r, target_r, target_x, target_x];
            self.corner_y = [target_y, target_b, target_b, target_y];
            self.opacity = 0.0;
            self.is_animating = false;
            self.last_update = Instant::now();
            return;
        }

        if dist_x > 0.1 || dist_y > 0.1 {
            self.target_x = target_x;
            self.target_y = target_y;
            self.is_animating = true;
        }
    }

    /// Advance physics: the 4 corners of the single continuous quad move toward cursor targets.
    /// Leading corners move with decay_fast; trailing corners move with decay_slow (matching Kitty).
    pub fn tick(&mut self, now: Instant, decay_secs: f32, cell_w: f32, cell_h: f32) -> bool {
        if !self.initialized {
            return false;
        }

        let dt = now
            .duration_since(self.last_update)
            .as_secs_f32()
            .clamp(0.001, 0.05);
        self.last_update = now;

        let target_r = self.target_x + cell_w;
        let target_b = self.target_y + cell_h;
        let targets = [
            (target_r, self.target_y),      // 0: top-right
            (target_r, target_b),           // 1: bottom-right
            (self.target_x, target_b),      // 2: bottom-left
            (self.target_x, self.target_y), // 3: top-left
        ];

        let cursor_center_x = self.target_x + cell_w * 0.5;
        let cursor_center_y = self.target_y + cell_h * 0.5;
        let cursor_diag_2 = (cell_w * cell_w + cell_h * cell_h).sqrt() * 0.5;

        let mut dx = [0.0f32; 4];
        let mut dy = [0.0f32; 4];
        let mut dot = [0.0f32; 4];
        let mut min_dot = f32::MAX;
        let mut max_dot = f32::MIN;

        for i in 0..4 {
            dx[i] = targets[i].0 - self.corner_x[i];
            dy[i] = targets[i].1 - self.corner_y[i];
            let d_norm = (dx[i] * dx[i] + dy[i] * dy[i]).sqrt();
            if d_norm < 1e-4 {
                dot[i] = 0.0;
                continue;
            }
            let to_corner_x = targets[i].0 - cursor_center_x;
            let to_corner_y = targets[i].1 - cursor_center_y;
            let d = (dx[i] * to_corner_x + dy[i] * to_corner_y) / (cursor_diag_2 * d_norm);
            dot[i] = d;
            min_dot = min_dot.min(d);
            max_dot = max_dot.max(d);
        }

        // Match Kitty's default physics (decay_fast = 0.10, decay_slow = 0.40)
        // while respecting user's cursor_trail_decay config.
        let decay_slow = if decay_secs > 0.01 { decay_secs } else { 0.40 };
        let decay_fast = (decay_slow * 0.25).clamp(0.02, 0.20);

        let mut max_diff = 0.0f32;

        for i in 0..4 {
            let d_norm = (dx[i] * dx[i] + dy[i] * dy[i]).sqrt();
            max_diff = max_diff.max(d_norm);

            if d_norm < 1e-4 || min_dot == f32::MAX {
                continue;
            }
            let decay = if (max_dot - min_dot).abs() < 1e-5 {
                decay_slow
            } else {
                decay_slow + (decay_fast - decay_slow) * ((dot[i] - min_dot) / (max_dot - min_dot))
            };
            // Exponential ease matching Kitty's 1.0 - exp2(-10.0 * dt / decay)
            let step = 1.0 - (-10.0 * dt / decay).exp2();
            self.corner_x[i] += dx[i] * step;
            self.corner_y[i] += dy[i] * step;
        }

        if max_diff > 0.25 {
            self.opacity = 1.0;
            self.is_animating = true;
            true
        } else {
            // Smoothly reached target
            for i in 0..4 {
                self.corner_x[i] = targets[i].0;
                self.corner_y[i] = targets[i].1;
            }
            self.opacity = 0.0;
            self.is_animating = false;
            false
        }
    }
}
