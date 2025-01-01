use crate::uniforms::{UniformBuilder, UniformStruct};
use config::EasingFunction;
use std::time::{Duration, Instant};

#[derive(Debug, Copy, Clone, PartialEq)]
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

    pub fn intensity(&mut self, is_one_shot: bool) -> Option<(f32, Instant)> {
        if is_one_shot {
            self.intensity_one_shot()
        } else {
            Some(self.intensity_continuous())
        }
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
                let fps = if self.in_function == EasingFunction::Constant
                    && self.out_function == EasingFunction::Constant
                {
                    1
                } else {
                    config::configuration().animation_fps as u64
                };
                let next = match fps {
                    1 if elapsed < self.in_duration => {
                        start + Duration::from_secs_f32(self.in_duration)
                    }
                    1 => start + Duration::from_secs_f32(self.in_duration + self.out_duration),
                    _ => {
                        let frame_interval = 1000 / fps as u64;
                        let elapsed = (elapsed * 1000.).ceil() as u64;
                        let remain = elapsed % frame_interval;
                        if remain != 0
                            && self.last_render.elapsed() >= Duration::from_millis(frame_interval)
                        {
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

pub struct ColorEaseUniform {
    pub in_function: [f32; 4],
    pub out_function: [f32; 4],
    pub in_duration_ms: u32,
    pub out_duration_ms: u32,
}

impl From<ColorEase> for ColorEaseUniform {
    fn from(ease: ColorEase) -> ColorEaseUniform {
        Self {
            in_duration_ms: (ease.in_duration * 1000.).ceil() as u32,
            out_duration_ms: (ease.out_duration * 1000.).ceil() as u32,
            in_function: ease.in_function.as_bezier_array(),
            out_function: ease.out_function.as_bezier_array(),
        }
    }
}

impl<'a> UniformStruct<'a> for ColorEaseUniform {
    fn add_fields(&'a self, struct_name: &str, builder: &mut UniformBuilder<'a>) {
        builder.add_struct_field(struct_name, "in_function", &self.in_function);
        builder.add_struct_field(struct_name, "out_function", &self.out_function);
        builder.add_struct_field(struct_name, "in_duration_ms", &self.in_duration_ms);
        builder.add_struct_field(struct_name, "out_duration_ms", &self.out_duration_ms);
    }
}
