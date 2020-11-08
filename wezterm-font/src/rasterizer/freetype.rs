use crate::locator::FontDataHandle;
use crate::rasterizer::FontRasterizer;
use crate::units::*;
use crate::{ftwrap, RasterizedGlyph};
use ::freetype::FT_GlyphSlotRec_;
use anyhow::bail;
use std::cell::RefCell;
use std::mem;
use std::slice;

pub struct FreeTypeRasterizer {
    has_color: bool,
    face: RefCell<ftwrap::Face>,
    _lib: ftwrap::Library,
}

impl FontRasterizer for FreeTypeRasterizer {
    fn rasterize_glyph(
        &self,
        glyph_pos: u32,
        size: f64,
        dpi: u32,
    ) -> anyhow::Result<RasterizedGlyph> {
        self.face.borrow_mut().set_font_size(size, dpi)?;

        let (load_flags, render_mode) = ftwrap::compute_load_flags_from_config();

        let mut face = self.face.borrow_mut();
        let descender = unsafe { (*(*face.face).size).metrics.descender as f64 / 64.0 };
        let ft_glyph = face.load_and_render_glyph(glyph_pos, load_flags, render_mode)?;

        let mode: ftwrap::FT_Pixel_Mode =
            unsafe { mem::transmute(u32::from(ft_glyph.bitmap.pixel_mode)) };

        // pitch is the number of bytes per source row
        let pitch = ft_glyph.bitmap.pitch.abs() as usize;
        let data = unsafe {
            slice::from_raw_parts_mut(
                ft_glyph.bitmap.buffer,
                ft_glyph.bitmap.rows as usize * pitch,
            )
        };

        let glyph = match mode {
            ftwrap::FT_Pixel_Mode::FT_PIXEL_MODE_LCD => self.rasterize_lcd(pitch, ft_glyph, data),
            ftwrap::FT_Pixel_Mode::FT_PIXEL_MODE_BGRA => {
                self.rasterize_bgra(pitch, descender, ft_glyph, data)
            }
            ftwrap::FT_Pixel_Mode::FT_PIXEL_MODE_GRAY => self.rasterize_gray(pitch, ft_glyph, data),
            ftwrap::FT_Pixel_Mode::FT_PIXEL_MODE_MONO => self.rasterize_mono(pitch, ft_glyph, data),
            mode => bail!("unhandled pixel mode: {:?}", mode),
        };
        Ok(glyph)
    }
}

impl FreeTypeRasterizer {
    fn rasterize_mono(
        &self,
        pitch: usize,
        ft_glyph: &FT_GlyphSlotRec_,
        data: &[u8],
    ) -> RasterizedGlyph {
        let width = ft_glyph.bitmap.width as usize;
        let height = ft_glyph.bitmap.rows as usize;
        let size = (width * height * 4) as usize;
        let mut rgba = vec![0u8; size];
        for y in 0..height {
            let src_offset = y * pitch;
            let dest_offset = y * width * 4;
            let mut x = 0;
            for i in 0..pitch {
                if x >= width {
                    break;
                }
                let mut b = data[src_offset + i];
                for _ in 0..8 {
                    if x >= width {
                        break;
                    }
                    if b & 0x80 == 0x80 {
                        for j in 0..4 {
                            rgba[dest_offset + (x * 4) + j] = 0xff;
                        }
                    }
                    b <<= 1;
                    x += 1;
                }
            }
        }
        RasterizedGlyph {
            data: rgba,
            height,
            width,
            bearing_x: PixelLength::new(ft_glyph.bitmap_left as f64),
            bearing_y: PixelLength::new(ft_glyph.bitmap_top as f64),
            has_color: false,
        }
    }

    fn rasterize_gray(
        &self,
        pitch: usize,
        ft_glyph: &FT_GlyphSlotRec_,
        data: &[u8],
    ) -> RasterizedGlyph {
        let width = ft_glyph.bitmap.width as usize;
        let height = ft_glyph.bitmap.rows as usize;
        let size = (width * height * 4) as usize;
        let mut rgba = vec![0u8; size];
        for y in 0..height {
            let src_offset = y * pitch;
            let dest_offset = y * width * 4;
            for x in 0..width {
                let gray = data[src_offset + x];

                rgba[dest_offset + (x * 4)] = gray;
                rgba[dest_offset + (x * 4) + 1] = gray;
                rgba[dest_offset + (x * 4) + 2] = gray;
                rgba[dest_offset + (x * 4) + 3] = gray;
            }
        }
        RasterizedGlyph {
            data: rgba,
            height,
            width,
            bearing_x: PixelLength::new(ft_glyph.bitmap_left as f64),
            bearing_y: PixelLength::new(ft_glyph.bitmap_top as f64),
            has_color: false,
        }
    }

    fn rasterize_lcd(
        &self,
        pitch: usize,
        ft_glyph: &FT_GlyphSlotRec_,
        data: &[u8],
    ) -> RasterizedGlyph {
        let width = ft_glyph.bitmap.width as usize / 3;
        let height = ft_glyph.bitmap.rows as usize;
        let size = (width * height * 4) as usize;
        let mut rgba = vec![0u8; size];
        for y in 0..height {
            let src_offset = y * pitch as usize;
            let dest_offset = y * width * 4;
            for x in 0..width {
                // Note: it is unclear whether the LCD data format
                // is BGR or RGB.  I'm using RGB here because the
                // antialiasing in other apps seems to do this.
                let red = data[src_offset + (x * 3)];
                let green = data[src_offset + (x * 3) + 1];
                let blue = data[src_offset + (x * 3) + 2];
                let alpha = red | green | blue;
                rgba[dest_offset + (x * 4)] = red;
                rgba[dest_offset + (x * 4) + 1] = green;
                rgba[dest_offset + (x * 4) + 2] = blue;
                rgba[dest_offset + (x * 4) + 3] = alpha;
            }
        }

        RasterizedGlyph {
            data: rgba,
            height,
            width,
            bearing_x: PixelLength::new(ft_glyph.bitmap_left as f64),
            bearing_y: PixelLength::new(ft_glyph.bitmap_top as f64),
            has_color: self.has_color,
        }
    }

    fn rasterize_bgra(
        &self,
        pitch: usize,
        descender: f64,
        ft_glyph: &FT_GlyphSlotRec_,
        data: &[u8],
    ) -> RasterizedGlyph {
        let width = ft_glyph.bitmap.width as usize;
        let height = ft_glyph.bitmap.rows as usize;

        // emoji glyphs don't always fill the bitmap size, so we compute
        // the non-transparent bounds here with this simplistic code.
        // This can likely be improved!

        let mut first_line = None;
        let mut first_col = None;
        let mut last_col = None;
        let mut last_line = None;

        for y in 0..height {
            let src_offset = y * pitch as usize;

            for x in 0..width {
                let alpha = data[src_offset + (x * 4) + 3];
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
        for y in (0..height).rev() {
            let src_offset = y * pitch as usize;

            for x in (0..width).rev() {
                let alpha = data[src_offset + (x * 4) + 3];
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

        let first_line = first_line.unwrap_or(0);
        let last_line = last_line.unwrap_or(0);
        let first_col = first_col.unwrap_or(0);
        let last_col = last_col.unwrap_or(0);

        let dest_width = 1 + last_col - first_col;
        let dest_height = 1 + last_line - first_line;

        let size = (dest_width * dest_height * 4) as usize;
        let mut rgba = vec![0u8; size];

        for y in first_line..=last_line {
            let src_offset = y * pitch as usize;
            let dest_offset = (y - first_line) * dest_width * 4;
            for x in first_col..=last_col {
                let blue = data[src_offset + (x * 4)];
                let green = data[src_offset + (x * 4) + 1];
                let red = data[src_offset + (x * 4) + 2];
                let alpha = data[src_offset + (x * 4) + 3];

                let dest_x = x - first_col;

                rgba[dest_offset + (dest_x * 4)] = red;
                rgba[dest_offset + (dest_x * 4) + 1] = green;
                rgba[dest_offset + (dest_x * 4) + 2] = blue;
                rgba[dest_offset + (dest_x * 4) + 3] = alpha;
            }
        }
        RasterizedGlyph {
            data: rgba,
            height: dest_height,
            width: dest_width,
            bearing_x: PixelLength::new(
                f64::from(ft_glyph.bitmap_left) * (dest_width as f64 / width as f64),
            ),

            // Fudge alert: this is font specific: I've found
            // that the emoji font on macOS doesn't account for the
            // descender in its metrics, so we're adding that offset
            // here to avoid rendering the glyph too high
            bearing_y: PixelLength::new(
                if cfg!(target_os = "macos") {
                    descender
                } else {
                    0.
                } + (f64::from(ft_glyph.bitmap_top) * (dest_height as f64 / height as f64)),
            ),

            has_color: self.has_color,
        }
    }

    pub fn from_locator(handle: &FontDataHandle) -> anyhow::Result<Self> {
        log::trace!("Rasterizier wants {:?}", handle);
        let lib = ftwrap::Library::new()?;
        let face = lib.face_from_locator(handle)?;
        let has_color = unsafe {
            (((*face.face).face_flags as u32) & (ftwrap::FT_FACE_FLAG_COLOR as u32)) != 0
        };
        Ok(Self {
            _lib: lib,
            face: RefCell::new(face),
            has_color,
        })
    }
}
