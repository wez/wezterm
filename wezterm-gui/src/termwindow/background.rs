use crate::color::LinearRgba;
use crate::termwindow::RenderState;
use crate::Dimensions;
use anyhow::Context;
use config::{
    BackgroundLayer, BackgroundRepeat, BackgroundSize, BackgroundSource, ConfigHandle,
    DimensionContext, GradientOrientation,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use termwiz::image::{ImageData, ImageDataType};
use wezterm_term::StableRowIndex;

lazy_static::lazy_static! {
    static ref IMAGE_CACHE: Mutex<HashMap<String, CachedImage>> = Mutex::new(HashMap::new());
}

struct CachedImage {
    modified: SystemTime,
    image: Arc<ImageData>,
    marked: bool,
    speed: f32,
}

impl CachedImage {
    fn load(path: &str, speed: f32) -> anyhow::Result<Arc<ImageData>> {
        let modified = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .with_context(|| format!("getting metadata for {}", path))?;
        let mut cache = IMAGE_CACHE.lock().unwrap();
        if let Some(cached) = cache.get_mut(path) {
            if cached.modified == modified && cached.speed == speed {
                cached.marked = false;
                return Ok(Arc::clone(&cached.image));
            }
        }

        let data = std::fs::read(path)
            .with_context(|| format!("Failed to load window_background_image {}", path))?;
        log::trace!("loaded {}", path);
        let mut data = ImageDataType::EncodedFile(data).decode();
        data.adjust_speed(speed);
        let image = Arc::new(ImageData::with_data(data));

        cache.insert(
            path.to_string(),
            Self {
                modified,
                image: Arc::clone(&image),
                marked: false,
                speed,
            },
        );

        Ok(image)
    }

    fn mark() {
        let mut cache = IMAGE_CACHE.lock().unwrap();
        for entry in cache.values_mut() {
            entry.marked = true;
        }
    }

    fn sweep() {
        let mut cache = IMAGE_CACHE.lock().unwrap();
        cache.retain(|k, entry| {
            if entry.marked {
                log::trace!("Unloading {} from cache", k);
            }
            !entry.marked
        });
    }
}

pub struct LoadedBackgroundLayer {
    pub source: Arc<ImageData>,
    pub def: BackgroundLayer,
}

fn load_background_layer(
    layer: &BackgroundLayer,
    dimensions: &Dimensions,
) -> anyhow::Result<LoadedBackgroundLayer> {
    let data = match &layer.source {
        BackgroundSource::Gradient(g) => {
            let grad = g
                .build()
                .with_context(|| format!("building gradient {:?}", g))?;

            let mut width = match layer.width {
                BackgroundSize::Percent(p) => (p as u32 * dimensions.pixel_width as u32) / 100,
                BackgroundSize::Length(u) => u as u32,
                unsup => anyhow::bail!("{:?} not yet implemented", unsup),
            };
            let mut height = match layer.height {
                BackgroundSize::Percent(p) => (p as u32 * dimensions.pixel_height as u32) / 100,
                BackgroundSize::Length(u) => u as u32,
                unsup => anyhow::bail!("{:?} not yet implemented", unsup),
            };

            if matches!(g.orientation, GradientOrientation::Radial { .. }) {
                // To simplify the math, we compute a perfect circle
                // for the radial gradient, and let the texture sampler
                // perturb it to fill the window
                width = width.min(height);
                height = height.min(width);
            }

            let mut imgbuf = image::RgbaImage::new(width, height);
            let fw = width as f64;
            let fh = height as f64;

            fn to_pixel(c: colorgrad::Color) -> image::Rgba<u8> {
                let (r, g, b, a) = c.rgba_u8();
                image::Rgba([r, g, b, a])
            }

            // Map t which is in range [a, b] to range [c, d]
            fn remap(t: f64, a: f64, b: f64, c: f64, d: f64) -> f64 {
                (t - a) * ((d - c) / (b - a)) + c
            }

            let (dmin, dmax) = grad.domain();

            let rng = fastrand::Rng::new();

            // We add some randomness to the position that we use to
            // index into the color gradient, so that we can avoid
            // visible color banding.  The default 64 was selected
            // because it it was the smallest value on my mac where
            // the banding wasn't obvious.
            let noise_amount = g.noise.unwrap_or_else(|| {
                if matches!(g.orientation, GradientOrientation::Radial { .. }) {
                    16
                } else {
                    64
                }
            });

            fn noise(rng: &fastrand::Rng, noise_amount: usize) -> f64 {
                if noise_amount == 0 {
                    0.
                } else {
                    rng.usize(0..noise_amount) as f64 * -1.
                }
            }

            match g.orientation {
                GradientOrientation::Horizontal => {
                    for (x, _, pixel) in imgbuf.enumerate_pixels_mut() {
                        *pixel = to_pixel(grad.at(remap(
                            x as f64 + noise(&rng, noise_amount),
                            0.0,
                            fw,
                            dmin,
                            dmax,
                        )));
                    }
                }
                GradientOrientation::Vertical => {
                    for (_, y, pixel) in imgbuf.enumerate_pixels_mut() {
                        *pixel = to_pixel(grad.at(remap(
                            y as f64 + noise(&rng, noise_amount),
                            0.0,
                            fh,
                            dmin,
                            dmax,
                        )));
                    }
                }
                GradientOrientation::Linear { angle } => {
                    let angle = angle.unwrap_or(0.0).to_radians();
                    for (x, y, pixel) in imgbuf.enumerate_pixels_mut() {
                        let (x, y) = (x as f64, y as f64);
                        let (x, y) = (x - fw / 2., y - fh / 2.);
                        let t = x * f64::cos(angle) - y * f64::sin(angle);
                        *pixel = to_pixel(grad.at(remap(
                            t + noise(&rng, noise_amount),
                            -fw / 2.,
                            fw / 2.,
                            dmin,
                            dmax,
                        )));
                    }
                }
                GradientOrientation::Radial { radius, cx, cy } => {
                    let radius = fw * radius.unwrap_or(0.5);
                    let cx = fw * cx.unwrap_or(0.5);
                    let cy = fh * cy.unwrap_or(0.5);

                    for (x, y, pixel) in imgbuf.enumerate_pixels_mut() {
                        let nx = noise(&rng, noise_amount);
                        let ny = noise(&rng, noise_amount);

                        let t = (nx + (x as f64 - cx).powi(2) + (ny + y as f64 - cy).powi(2))
                            .sqrt()
                            / radius;
                        *pixel = to_pixel(grad.at(t));
                    }
                }
            }

            let data = imgbuf.into_vec();
            Arc::new(ImageData::with_data(ImageDataType::new_single_frame(
                width, height, data,
            )))
        }
        BackgroundSource::Color(color) => {
            // In theory we could just make a 1x1 texture and allow
            // the shader to stretch it, but if we do that, it'll blend
            // around the edges and look weird.
            // So we make a square texture in the ballpark of the window
            // surface.
            // It's not ideal.
            let width = match layer.width {
                BackgroundSize::Percent(p) => (p as u32 * dimensions.pixel_width as u32) / 100,
                BackgroundSize::Length(u) => u as u32,
                unsup => anyhow::bail!("{:?} not yet implemented", unsup),
            };
            let height = match layer.height {
                BackgroundSize::Percent(p) => (p as u32 * dimensions.pixel_height as u32) / 100,
                BackgroundSize::Length(u) => u as u32,
                unsup => anyhow::bail!("{:?} not yet implemented", unsup),
            };

            let size = width.min(height);

            let mut imgbuf = image::RgbaImage::new(size, size);
            let src_pixel = {
                let (r, g, b, a) = color.to_srgb_u8();
                image::Rgba([r, g, b, a])
            };
            for (_x, _y, pixel) in imgbuf.enumerate_pixels_mut() {
                *pixel = src_pixel;
            }
            let data = imgbuf.into_vec();
            Arc::new(ImageData::with_data(ImageDataType::new_single_frame(
                size, size, data,
            )))
        }
        BackgroundSource::File(source) => CachedImage::load(&source.path, source.speed)?,
    };

    Ok(LoadedBackgroundLayer {
        source: data,
        def: layer.clone(),
    })
}

pub fn load_background_image(
    config: &ConfigHandle,
    dimensions: &Dimensions,
) -> Vec<LoadedBackgroundLayer> {
    let mut layers = vec![];
    for layer in &config.background {
        let load_start = std::time::Instant::now();
        match load_background_layer(layer, dimensions) {
            Ok(layer) => {
                log::trace!("loaded layer in {:?}", load_start.elapsed());
                layers.push(layer);
            }
            Err(err) => {
                log::error!("Failed to load background: {:#}", err);
            }
        }
    }
    layers
}

pub fn reload_background_image(
    config: &ConfigHandle,
    existing: &[LoadedBackgroundLayer],
    dimensions: &Dimensions,
) -> Vec<LoadedBackgroundLayer> {
    // We want to reuse the existing version of the image where possible
    // so that the textures we may have cached can be re-used and so that
    // animation state can be preserved across the reload.
    let map: HashMap<_, _> = existing
        .iter()
        .map(|layer| (layer.source.hash(), &layer.source))
        .collect();

    CachedImage::mark();

    let result = load_background_image(config, dimensions)
        .into_iter()
        .map(|mut layer| {
            let hash = layer.source.hash();

            if let Some(existing) = map.get(&hash) {
                layer.source = Arc::clone(existing);
            }

            layer
        })
        .collect();

    CachedImage::sweep();

    result
}

impl crate::TermWindow {
    pub fn render_backgrounds(
        &self,
        bg_color: LinearRgba,
        top: StableRowIndex,
    ) -> anyhow::Result<()> {
        let gl_state = self.render_state.as_ref().unwrap();
        for (idx, layer) in self.window_background.iter().enumerate() {
            self.render_background(gl_state, bg_color, layer, idx, top)?;
        }
        Ok(())
    }

    fn render_background(
        &self,
        gl_state: &RenderState,
        bg_color: LinearRgba,
        layer: &LoadedBackgroundLayer,
        layer_index: usize,
        top: StableRowIndex,
    ) -> anyhow::Result<()> {
        let render_layer = gl_state.layer_for_zindex(-127 + layer_index as i8)?;
        let vbs = render_layer.vb.borrow();
        let mut vb_mut0 = vbs[0].current_vb_mut();
        let mut layer0 = vbs[0].map(&mut vb_mut0);

        let color = bg_color.mul_alpha(layer.def.opacity);

        let (sprite, next_due) = gl_state
            .glyph_cache
            .borrow_mut()
            .cached_image(&layer.source, None)?;
        self.update_next_frame_time(next_due);

        let pixel_width = self.dimensions.pixel_width as f32;
        let pixel_height = self.dimensions.pixel_height as f32;

        let tex_width = sprite.coords.width() as f32;
        let tex_height = sprite.coords.height() as f32;
        let aspect = tex_width as f32 / tex_height as f32;

        let h_context = DimensionContext {
            dpi: self.dimensions.dpi as f32,
            pixel_max: pixel_width,
            pixel_cell: self.render_metrics.cell_size.width as f32,
        };
        let v_context = DimensionContext {
            dpi: self.dimensions.dpi as f32,
            pixel_max: pixel_height,
            pixel_cell: self.render_metrics.cell_size.height as f32,
        };

        // log::info!("tex {tex_width}x{tex_height} aspect={aspect}");

        // Compute the largest aspect-preserved size that will fill the space
        let (max_aspect_width, max_aspect_height) = if aspect >= 1.0 {
            // Width is the longest side
            let target_height = pixel_width / aspect;
            if target_height > pixel_height {
                (pixel_width * pixel_height / target_height, pixel_height)
            } else {
                (pixel_width, target_height)
            }
        } else {
            // Height is the longest side
            let target_width = pixel_height / aspect;
            if target_width > pixel_width {
                (pixel_width, pixel_height * pixel_width / target_width)
            } else {
                (target_width, pixel_height)
            }
        };

        // Compute the smallest aspect-preserved size that will fit the space
        let (min_aspect_width, min_aspect_height) = if aspect >= 1.0 {
            // Width is the longest side
            if tex_height > pixel_height {
                (tex_width * pixel_height / tex_height, pixel_height)
            } else {
                (tex_width, tex_height)
            }
        } else {
            // Height is the longest side
            if tex_width > pixel_width {
                (pixel_width, tex_height * pixel_width / tex_width)
            } else {
                (tex_width, tex_height)
            }
        };

        let width = match layer.def.width {
            BackgroundSize::Contain => max_aspect_width as f32,
            BackgroundSize::Cover => min_aspect_width as f32,
            BackgroundSize::Length(n) => n as f32,
            BackgroundSize::Percent(p) => (pixel_width * p as f32) / 100.,
        };

        let height = match layer.def.height {
            BackgroundSize::Contain => max_aspect_height as f32,
            BackgroundSize::Cover => min_aspect_height as f32,
            BackgroundSize::Length(n) => n as f32,
            BackgroundSize::Percent(p) => (pixel_height * p as f32) / 100.,
        };
        let origin_x = pixel_width / -2.;
        let top_pixel = pixel_height / -2.;
        let mut origin_y = top_pixel;

        let repeat_x = layer
            .def
            .repeat_x_size
            .map(|size| size.evaluate_as_pixels(h_context))
            .unwrap_or(width);
        let repeat_y = layer
            .def
            .repeat_y_size
            .map(|size| size.evaluate_as_pixels(v_context))
            .unwrap_or(height);

        // log::info!("computed {width}x{height}");
        if let Some(factor) = layer.def.attachment.scroll_factor() {
            let distance = top as f32 * self.render_metrics.cell_size.height as f32 * factor;
            let num_tiles = distance / repeat_y;
            origin_y -= num_tiles.fract() * repeat_y;
        }

        let limit_y = top_pixel + pixel_height;

        for y_step in 0.. {
            let offset_y = y_step as f32 * repeat_y;
            let origin_y = origin_y + offset_y;
            if origin_y >= limit_y
                || (y_step > 0 && layer.def.repeat_y == BackgroundRepeat::NoRepeat)
            {
                break;
            }

            for x_step in 0.. {
                let offset_x = x_step as f32 * repeat_x;
                if offset_x >= pixel_width
                    || (x_step > 0 && layer.def.repeat_x == BackgroundRepeat::NoRepeat)
                {
                    break;
                }
                let origin_x = origin_x + offset_x;
                let mut quad = layer0.allocate()?;
                // log::info!("quad {origin_x},{origin_y} {width}x{height}");
                quad.set_position(origin_x, origin_y, origin_x + width, origin_y + height);
                quad.set_texture(sprite.texture_coords());
                quad.set_is_background_image();
                quad.set_hsv(Some(layer.def.hsb));
                quad.set_fg_color(color);
            }
        }

        Ok(())
    }
}
