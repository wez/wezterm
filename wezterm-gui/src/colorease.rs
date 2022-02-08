use config::EasingFunction;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct ColorEase {
    in_duration: f32,
    in_function: EasingFunction,
    out_duration: f32,
    out_function: EasingFunction,
    start: Option<Instant>,
    last_render: Instant,
}

impl ColorEase {
    pub fn new(
        in_duration_ms: u64,
        in_function: EasingFunction,
        out_duration_ms: u64,
        out_function: EasingFunction,
        start: Option<Instant>,
    ) -> Self {
        Self {
            in_duration: Duration::from_millis(in_duration_ms).as_secs_f32(),
            in_function,
            out_duration: Duration::from_millis(out_duration_ms).as_secs_f32(),
            out_function,
            start,
            last_render: Instant::now(),
        }
    }

    pub fn update_start(&mut self, start: Instant) {
        let start = match self.start.take() {
            Some(prior) if prior >= start => prior,
            _ => start,
        };
        self.start.replace(start);
    }

    pub fn intensity_continuous(&mut self) -> (f32, Instant) {
        match self.intensity_one_shot() {
            Some(intensity) => intensity,
            None => {
                // Start a new cycle
                self.start.replace(Instant::now());
                self.intensity_one_shot().expect("just started")
            }
        }
    }

    pub fn intensity_one_shot(&mut self) -> Option<(f32, Instant)> {
        let start = self.start?;
        let elapsed = start.elapsed().as_secs_f32();

        let intensity = if elapsed < self.in_duration {
            Some(
                self.in_function
                    .evaluate_at_position(elapsed / self.in_duration),
            )
        } else {
            let completion = (elapsed - self.in_duration) / self.out_duration;
            if completion >= 1.0 {
                None
            } else {
                Some(1.0 - self.out_function.evaluate_at_position(completion))
            }
        };

        match intensity {
            Some(i) => {
                let now = Instant::now();
                let fps = config::configuration().animation_fps as u64;
                let next = match fps {
                    1 if elapsed < self.in_duration => {
                        start + Duration::from_secs_f32(self.in_duration)
                    }
                    1 => {
                        start
                            + Duration::from_secs_f32(self.in_duration)
                            + Duration::from_secs_f32(self.out_duration)
                    }
                    _ => {
                        let frame_interval = 1000 / fps as u64;
                        let elapsed = (elapsed * 1000.).ceil() as u64;
                        let remain = elapsed % frame_interval;
                        if remain != 0 {
                            now + Duration::from_millis(remain)
                        } else {
                            now + Duration::from_millis(frame_interval)
                        }
                    }
                };
                self.last_render = now;
                Some((i, next))
            }
            None => {
                self.start.take();
                None
            }
        }
    }
}
