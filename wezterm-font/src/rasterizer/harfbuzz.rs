
use crate::hbwrap::{
    hb_blob_get_data, hb_blob_t, hb_bool_t, hb_glyph_extents_t, hb_paint_funcs_t, hb_tag_t,
    hb_tag_to_string, Face, Font, FontFuncs,
};
use crate::rasterizer::FAKE_ITALIC_SKEW;
use crate::units::PixelLength;
use crate::{FontRasterizer, ParsedFont, RasterizedGlyph};
use anyhow::Context;
use image::DynamicImage::{ImageLuma8, ImageLumaA8};

pub struct HarfbuzzRasterizer {
    face: Face,
    font: Font,
    funcs: FontFuncs,
}

impl HarfbuzzRasterizer {
    pub fn from_locator(parsed: &ParsedFont) -> anyhow::Result<Self> {
        let mut font = Font::from_locator(&parsed.handle)?;
        font.set_ot_funcs();
        let face = font.get_face();

        if parsed.synthesize_italic {
            font.set_synthetic_slant(FAKE_ITALIC_SKEW as f32);
        }
        if parsed.synthesize_bold {
            font.set_synthetic_bold(0.02, 0.02, false);
        }

        let mut funcs = FontFuncs::new()?;
        funcs.set_push_transform_func(Some(PaintData::push_transform_trampoline));
        funcs.set_pop_transform_func(Some(PaintData::pop_transform_trampoline));
        funcs.set_image_func(Some(PaintData::image_trampoline));

        Ok(Self { face, font, funcs })
    }
}

impl FontRasterizer for HarfbuzzRasterizer {
    fn rasterize_glyph(
        &self,
        glyph_pos: u32,
        size: f64,
        dpi: u32,
    ) -> anyhow::Result<RasterizedGlyph> {
        let mut data = PaintData {
            rasterizer: self,
            glyph_pos,
            size,
            dpi,
            result: RasterizedGlyph {
                data: vec![],
                height: 0,
                width: 0,
                bearing_x: PixelLength::new(0.),
                bearing_y: PixelLength::new(0.),
                has_color: false,
            },
        };

        let pixel_size = (size * dpi as f64 / 72.) as u32;
        let upem = self.face.get_upem();

        let scale = size as i32 * 64;
        let ppem = pixel_size;
        log::info!("computed scale={scale}, ppem={ppem}, upem={upem}");
        self.font.set_ppem(ppem, ppem);
        self.font.set_ptem(size as f32);
        self.font.set_font_scale(scale, scale);
        self.font.paint_glyph(
            glyph_pos,
            &self.funcs,
            &mut data as *mut _ as _,
            0,          // palette index 0
            0xffffffff, // 100% white
        );

        Ok(data.result)
    }
}

struct PaintData<'a> {
    rasterizer: &'a HarfbuzzRasterizer,
    glyph_pos: u32,
    size: f64,
    dpi: u32,
    result: RasterizedGlyph,
}

impl<'a> PaintData<'a> {
    extern "C" fn push_transform_trampoline(
        _funcs: *mut hb_paint_funcs_t,
        paint_data: *mut ::std::os::raw::c_void,
        xx: f32,
        yx: f32,
        xy: f32,
        yy: f32,
        dx: f32,
        dy: f32,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let this: &mut Self = unsafe { &mut *(paint_data as *mut Self) };
        this.push_transform(xx, yx, xy, yy, dx, dy);
    }

    extern "C" fn pop_transform_trampoline(
        _funcs: *mut hb_paint_funcs_t,
        paint_data: *mut ::std::os::raw::c_void,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let this: &mut Self = unsafe { &mut *(paint_data as *mut Self) };
        this.pop_transform();
    }

    extern "C" fn image_trampoline(
        _funcs: *mut hb_paint_funcs_t,
        paint_data: *mut ::std::os::raw::c_void,
        image: *mut hb_blob_t,
        width: ::std::os::raw::c_uint,
        height: ::std::os::raw::c_uint,
        format: hb_tag_t,
        slant: f32,
        extents: *mut hb_glyph_extents_t,
        _user_data: *mut ::std::os::raw::c_void,
    ) -> hb_bool_t {
        let this: &mut Self = unsafe { &mut *(paint_data as *mut Self) };

        let mut image_len = 0;
        let mut image_ptr = unsafe { hb_blob_get_data(image, &mut image_len) };
        let image =
            unsafe { std::slice::from_raw_parts(image_ptr as *const u8, image_len as usize) };

        let result = this.image(image, width, height, format, slant, unsafe {
            if extents.is_null() {
                None
            } else {
                Some(&*extents)
            }
        });
        match result {
            Ok(()) => 1,
            Err(err) => {
                log::error!("image: {err:#}");
                0
            }
        }
    }

    fn push_transform(&mut self, xx: f32, yx: f32, xy: f32, yy: f32, dx: f32, dy: f32) {
        log::info!("push_transform: xx={xx} yx={yx} xy={xy} yy={yy} dx={dx} dy={dy}");
    }
    fn pop_transform(&mut self) {
        log::info!("pop_transform");
    }
    fn image(
        &mut self,
        image: &[u8],
        width: ::std::os::raw::c_uint,
        height: ::std::os::raw::c_uint,
        format: hb_tag_t,
        slant: f32,
        extents: Option<&hb_glyph_extents_t>,
    ) -> anyhow::Result<()> {
        let format = hb_tag_to_string(format);
        log::info!("image {width}x{height} format={format} slant={slant} {extents:?}");

        let decoded = image::io::Reader::new(std::io::Cursor::new(image))
            .with_guessed_format()?
            .decode()?;

        match &decoded {
            ImageLuma8(_) | ImageLumaA8(_) => self.result.has_color = false,
            _ => self.result.has_color = true,
        }

        let mut decoded = decoded.into_rgba8();

        // Convert to premultiplied form
        fn multiply_alpha(alpha: u8, color: u8) -> u8 {
            let temp: u32 = alpha as u32 * (color as u32 + 0x80);

            ((temp + (temp >> 8)) >> 8) as u8
        }

        for (_x, _y, pixel) in decoded.enumerate_pixels_mut() {
            let alpha = pixel[3];
            if alpha == 0 {
                pixel[0] = 0;
                pixel[1] = 0;
                pixel[2] = 0;
            } else {
                if alpha != 0xff {
                    for n in 0..3 {
                        pixel[n] = multiply_alpha(alpha, pixel[n]);
                    }
                }
            }
        }

        // Crop to the non-transparent portions of the image
        let mut first_line = None;
        let mut first_col = None;
        let mut last_col = None;
        let mut last_line = None;

        for (y, row) in decoded.rows().enumerate() {
            for (x, pixel) in row.enumerate() {
                let alpha = pixel[3];
                if alpha != 0 {
                    if first_line.is_none() {
                        first_line = Some(y);
                    }
                    first_col = match first_col.take() {
                        Some(other) if x < other => Some(x),
                        Some(other) => Some(other),
                        None => Some(x),
                    };
                }
            }
        }
        for (y, row) in decoded.rows().enumerate().rev() {
            for (x, pixel) in row.enumerate().rev() {
                let alpha = pixel[3];
                if alpha != 0 {
                    if last_line.is_none() {
                        last_line = Some(y);
                    }
                    last_col = match last_col.take() {
                        Some(other) if x > other => Some(x),
                        Some(other) => Some(other),
                        None => Some(x),
                    };
                }
            }
        }

        let first_col = first_col.unwrap_or(0) as u32;
        let first_line = first_line.unwrap_or(0) as u32;
        let last_col = last_col.unwrap_or(width as usize) as u32;
        let last_line = last_line.unwrap_or(height as usize) as u32;

        let cropped = image::imageops::crop(
            &mut decoded,
            first_col,
            first_line,
            last_col - first_col,
            last_line - first_line,
        )
        .to_image();
        self.result.height = cropped.height() as usize;
        self.result.width = cropped.width() as usize;

        log::info!("cropped -> {}x{}", self.result.width, self.result.height);

        self.result.data = cropped.into_vec();

        let (bearing_x, bearing_y) = extents
            .map(|ext| (ext.x_bearing as f64 / 64., ext.y_bearing as f64 / 64.))
            .unwrap_or((0., 0.));
        self.result.bearing_x = PixelLength::new(bearing_x);
        self.result.bearing_y = PixelLength::new(bearing_y);

        Ok(())
    }
}
