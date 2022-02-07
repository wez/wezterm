use config::EasingFunction;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct ColorEase {
    in_duration: f32,
    in_function: EasingFunction,
    out_duration: f32,
    out_function: EasingFunction,
    start: Option<Instant>,
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
        }
    }

    pub fn intensity_continuous(&mut self) -> f32 {
        match self.intensity_one_shot() {
            Some(intensity) => intensity,
            None => {
                // Start a new cycle
                self.start.replace(Instant::now());
                self.intensity_one_shot().expect("just started")
            }
        }
    }

    pub fn intensity_one_shot(&mut self) -> Option<f32> {
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

        if intensity.is_none() {
            self.start.take();
        }
        intensity
    }
}
