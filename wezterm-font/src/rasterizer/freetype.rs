use crate::parser::ParsedFont;
use crate::rasterizer::{FontRasterizer, FAKE_ITALIC_SKEW};
use crate::units::*;
use crate::{ftwrap, RasterizedGlyph};
use ::freetype::{
    FT_Bool, FT_Err_Invalid_SVG_Document, FT_Err_Ok, FT_Error, FT_GlyphSlot, FT_GlyphSlotRec_,
    FT_Matrix, FT_Pointer, FT_Pos, FT_SVG_Document, FT_SVG_DocumentRec_, SVG_RendererHooks,
};
use anyhow::{bail, Context};
use config::{DisplayPixelGeometry, FreeTypeLoadFlags, FreeTypeLoadTarget};
use lfucache::LfuCache;
use resvg::tiny_skia::{Pixmap, PixmapMut, PixmapPaint, Transform};
use std::cell::RefCell;
use std::{mem, slice};
use wezterm_color_types::linear_u8_to_srgb8;

pub static SVG_HOOKS: SVG_RendererHooks = SVG_RendererHooks {
    init_svg: Some(init_svg_library),
    free_svg: Some(free_svg_library),
    render_svg: Some(svg_render),
    preset_slot: Some(svg_preset_slot),
};

pub struct FreeTypeRasterizer {
    has_color: bool,
    face: RefCell<ftwrap::Face>,
    _lib: ftwrap::Library,
    synthesize_bold: bool,
    freetype_load_target: Option<FreeTypeLoadTarget>,
    freetype_render_target: Option<FreeTypeLoadTarget>,
    freetype_load_flags: Option<FreeTypeLoadFlags>,
    display_pixel_geometry: DisplayPixelGeometry,
    scale: f64,
}

impl FontRasterizer for FreeTypeRasterizer {
    fn rasterize_glyph(
        &self,
        glyph_pos: u32,
        size: f64,
        dpi: u32,
    ) -> anyhow::Result<RasterizedGlyph> {
        self.face
            .borrow_mut()
            .set_font_size(size * self.scale, dpi)?;

        let (load_flags, render_mode) = ftwrap::compute_load_flags_from_config(
            self.freetype_load_flags,
            self.freetype_load_target,
            self.freetype_render_target,
        );

        let mut face = self.face.borrow_mut();
        let ft_glyph =
            face.load_and_render_glyph(glyph_pos, load_flags, render_mode, self.synthesize_bold)?;

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
            ftwrap::FT_Pixel_Mode::FT_PIXEL_MODE_BGRA => self.rasterize_bgra(pitch, ft_glyph, data),
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
                let linear_gray = data[src_offset + x];
                let gray = linear_u8_to_srgb8(linear_gray);

                // Texture is SRGBA, which in OpenGL means
                // that the RGB values are gamma adjusted
                // non-linear values, but the A value is
                // linear!

                rgba[dest_offset + (x * 4)] = gray;
                rgba[dest_offset + (x * 4) + 1] = gray;
                rgba[dest_offset + (x * 4) + 2] = gray;
                rgba[dest_offset + (x * 4) + 3] = linear_gray;
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
                let red = data[src_offset + (x * 3)];
                let green = data[src_offset + (x * 3) + 1];
                let blue = data[src_offset + (x * 3) + 2];

                let linear_alpha = red.max(green).max(blue);

                // Texture is SRGBA, which in OpenGL means
                // that the RGB values are gamma adjusted
                // non-linear values, but the A value is
                // linear!

                let red = linear_u8_to_srgb8(red);
                let green = linear_u8_to_srgb8(green);
                let blue = linear_u8_to_srgb8(blue);

                let (red, blue) = match self.display_pixel_geometry {
                    DisplayPixelGeometry::RGB => (red, blue),
                    DisplayPixelGeometry::BGR => (blue, red),
                };

                rgba[dest_offset + (x * 4)] = red;
                rgba[dest_offset + (x * 4) + 1] = green;
                rgba[dest_offset + (x * 4) + 2] = blue;
                rgba[dest_offset + (x * 4) + 3] = linear_alpha;
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
        _pitch: usize,
        ft_glyph: &FT_GlyphSlotRec_,
        data: &'static [u8],
    ) -> RasterizedGlyph {
        let width = ft_glyph.bitmap.width as usize;
        let height = ft_glyph.bitmap.rows as usize;

        let mut source_image = image::ImageBuffer::<image::Rgba<u8>, &[u8]>::from_raw(
            width as u32,
            height as u32,
            data,
        )
        .expect("image data to be valid");

        // emoji glyphs don't always fill the bitmap size, so we compute
        // the non-transparent bounds

        let mut cropped = crate::rasterizer::crop_to_non_transparent(&mut source_image).to_image();
        crate::rasterizer::swap_red_and_blue(&mut cropped);

        let dest_width = cropped.width() as usize;
        let dest_height = cropped.height() as usize;

        RasterizedGlyph {
            data: cropped.into_vec(),
            height: dest_height,
            width: dest_width,
            bearing_x: PixelLength::new(
                f64::from(ft_glyph.bitmap_left) * (dest_width as f64 / width as f64),
            ),
            bearing_y: PixelLength::new(
                f64::from(ft_glyph.bitmap_top) * (dest_height as f64 / height as f64),
            ),
            has_color: self.has_color,
        }
    }

    pub fn from_locator(
        parsed: &ParsedFont,
        display_pixel_geometry: DisplayPixelGeometry,
    ) -> anyhow::Result<Self> {
        log::trace!("Rasterizier wants {:?}", parsed);
        let lib = ftwrap::Library::new()?;
        let mut face = lib.face_from_locator(&parsed.handle)?;
        let has_color = unsafe {
            (((*face.face).face_flags as u32) & (ftwrap::FT_FACE_FLAG_COLOR as u32)) != 0
        };

        if parsed.synthesize_italic {
            face.set_transform(Some(FT_Matrix {
                xx: 1 * 65536,                         // scale x
                yy: 1 * 65536,                         // scale y
                xy: (FAKE_ITALIC_SKEW * 65536.0) as _, // skew x
                yx: 0 * 65536,                         // skew y
            }));
        }

        Ok(Self {
            _lib: lib,
            face: RefCell::new(face),
            has_color,
            synthesize_bold: parsed.synthesize_bold,
            freetype_load_flags: parsed.freetype_load_flags,
            freetype_load_target: parsed.freetype_load_target,
            freetype_render_target: parsed.freetype_render_target,
            display_pixel_geometry,
            scale: parsed.scale.unwrap_or(1.),
        })
    }
}

struct SvgState {
    pixmap: Pixmap,
    doc_cache: LfuCache<DocCacheKey, resvg::usvg::Tree>,
}

impl SvgState {
    fn new() -> Self {
        Self {
            pixmap: Pixmap::new(1, 1).expect("to allocate empty pixmap"),
            doc_cache: LfuCache::new(
                "font.svg.parser.hit",
                "font.svg.parser.miss",
                |_| 64,
                &config::configuration(),
            ),
        }
    }
}

// The SVG rendering implementation was produced by roughly following
// <https://gitlab.freedesktop.org/freetype/freetype-demos/-/blob/master/src/rsvg-port.c>
// and adapting for resvg

unsafe extern "C" fn init_svg_library(data_pointer: *mut FT_Pointer) -> FT_Error {
    let state = Box::new(SvgState::new());
    *data_pointer = Box::into_raw(state) as *mut std::os::raw::c_void;
    FT_Err_Ok as FT_Error
}

unsafe extern "C" fn free_svg_library(data_pointer: *mut FT_Pointer) {
    let state: Box<SvgState> = Box::from_raw((*data_pointer) as *mut SvgState);
    drop(state);
    *data_pointer = std::ptr::null_mut();
}

/*
 * This hook is called at two different locations.  Firstly, it is called
 * when presetting the glyphslot when `FT_Load_Glyph` is called.
 * Secondly, it is called right before the render hook is called.  When
 * `cache` is false, it is the former, when `cache` is true, it is the
 * latter.
 *
 * The job of this function is to preset the slot setting the width,
 * height, pitch, `bitmap.left`, and `bitmap.top`.  These are all
 * necessary for appropriate memory allocation, as well as ultimately
 * compositing the glyph later on by client applications.
 */
unsafe extern "C" fn svg_preset_slot(
    slot: FT_GlyphSlot,
    cache: FT_Bool,
    state: *mut FT_Pointer,
) -> FT_Error {
    let state: &mut SvgState = &mut *((*state) as *mut SvgState);
    let slot: &mut FT_GlyphSlotRec_ = &mut *slot;
    let document: &mut FT_SVG_DocumentRec_ = &mut *(slot.other as FT_SVG_Document);
    let cache = cache != 0;

    match svg_preset_slot_impl(slot, document, cache, state) {
        Ok(_) => FT_Err_Ok as FT_Error,
        Err(err) => {
            log::error!("svg_preset_slot: {err:#}");
            FT_Err_Invalid_SVG_Document as FT_Error
        }
    }
}

#[derive(Hash, Eq, PartialEq, Clone, Copy, Debug)]
struct DocCacheKey {
    doc_ptr: usize,
    start_glyph_id: u16,
    end_glyph_id: u16,
}

fn svg_preset_slot_impl(
    slot: &mut FT_GlyphSlotRec_,
    document: &mut FT_SVG_DocumentRec_,
    cache: bool,
    state: &mut SvgState,
) -> anyhow::Result<()> {
    use resvg::usvg::TreeParsing;

    let svg_data = unsafe {
        std::slice::from_raw_parts(document.svg_document, document.svg_document_length as usize)
    };

    let document_hash = DocCacheKey {
        doc_ptr: svg_data.as_ptr() as usize,
        start_glyph_id: document.start_glyph_id,
        end_glyph_id: document.end_glyph_id,
    };

    let utree = match state.doc_cache.get(&document_hash) {
        Some(t) => t,
        None => {
            let options = resvg::usvg::Options {
                image_href_resolver: resvg::usvg::ImageHrefResolver {
                    resolve_string: Box::new(|_href, _opts| None),
                    ..Default::default()
                },
                ..Default::default()
            };

            let utree =
                resvg::usvg::Tree::from_data(svg_data, &options).context("Tree::from_data")?;
            state.doc_cache.put(document_hash, utree);
            state
                .doc_cache
                .get(&document_hash)
                .expect("just inserted it")
        }
    };

    let tree_node;
    let rtree = if document.start_glyph_id < document.end_glyph_id {
        tree_node = utree
            .node_by_id(&format!("glyph{}", slot.glyph_index))
            .context("missing glyph")?;
        resvg::Tree::from_usvg_node(&tree_node).context("from_usvg_node")?
    } else {
        resvg::Tree::from_usvg(&utree)
    };

    let pixmap_size = rtree.size.to_int_size();

    let x_svg_to_out = document.metrics.x_ppem as f64 / pixmap_size.width() as f64;
    let y_svg_to_out = document.metrics.y_ppem as f64 / pixmap_size.height() as f64;

    let xx = (document.transform.xx as f64) / (1 << 16) as f64;
    let xy = -(document.transform.xy as f64) / (1 << 16) as f64;

    let yx = -(document.transform.yx as f64) / (1 << 16) as f64;
    let yy = (document.transform.yy as f64) / (1 << 16) as f64;

    let x0 =
        document.delta.x as f64 / 64. * pixmap_size.width() as f64 / document.metrics.x_ppem as f64;
    let y0 = -document.delta.y as f64 / 64. * pixmap_size.height() as f64
        / document.metrics.y_ppem as f64;

    let transform = resvg::tiny_skia::Transform::from_row(
        xx as f32, xy as f32, yx as f32, yy as f32, x0 as f32, y0 as f32,
    )
    .post_scale(x_svg_to_out as f32, y_svg_to_out as f32);

    let dimension_x = pixmap_size.width() as f64 * x_svg_to_out;
    let dimension_y = pixmap_size.height() as f64 * y_svg_to_out;

    slot.bitmap_left = 0;
    slot.bitmap_top = dimension_y as i32; // This sets the y-bearing. It is incorrect,
                                          // but better than using 0
    slot.bitmap.rows = dimension_y as _;
    slot.bitmap.width = dimension_x as _;
    slot.bitmap.pitch = (dimension_x as i32) * 4;
    slot.bitmap.pixel_mode = ftwrap::FT_Pixel_Mode::FT_PIXEL_MODE_BGRA as _;
    slot.metrics.width = (dimension_x * 64.0) as FT_Pos;
    slot.metrics.height = (dimension_y * 64.0) as FT_Pos;
    slot.metrics.horiBearingX = 0;
    slot.metrics.horiBearingY = 0;
    slot.metrics.vertBearingX = 0;
    slot.metrics.vertBearingY = 0;

    if slot.metrics.vertAdvance == 0 {
        slot.metrics.vertAdvance = (dimension_y * 1.2 * 64.) as FT_Pos;
    }

    if cache {
        let mut pixmap =
            Pixmap::new(dimension_x as u32, dimension_y as u32).context("Pixmap::new")?;
        rtree.render(transform, &mut pixmap.as_mut());
        state.pixmap = pixmap;
    }

    Ok(())
}

/// The render hook.  The job of this hook is to simply render the glyph in
/// the buffer that has been allocated on the FreeType side.
unsafe extern "C" fn svg_render(slot: FT_GlyphSlot, data_pointer: *mut FT_Pointer) -> FT_Error {
    let state: &mut SvgState = &mut *((*data_pointer) as *mut SvgState);
    let slot: &mut FT_GlyphSlotRec_ = &mut *slot;

    match svg_render_impl(slot, state) {
        Ok(_) => FT_Err_Ok as FT_Error,
        Err(err) => {
            log::error!("svg_render: {err:#}");
            FT_Err_Invalid_SVG_Document as FT_Error
        }
    }
}

fn svg_render_impl(slot: &mut FT_GlyphSlotRec_, state: &mut SvgState) -> anyhow::Result<()> {
    let bitmap = unsafe {
        std::slice::from_raw_parts_mut(
            slot.bitmap.buffer,
            slot.bitmap.width as usize * slot.bitmap.rows as usize * 4,
        )
    };
    let mut pixmap =
        PixmapMut::from_bytes(bitmap, slot.bitmap.width, slot.bitmap.rows).context("PixmapMut")?;
    pixmap.draw_pixmap(
        0,
        0,
        state.pixmap.as_ref(),
        &PixmapPaint::default(),
        Transform::default(),
        None,
    );

    // Post-process: freetype wants BGRA but tiny-skia is RGBA
    for pixel in pixmap.pixels_mut() {
        let (r, g, b, a) = (pixel.red(), pixel.green(), pixel.blue(), pixel.alpha());
        *pixel =
            resvg::tiny_skia::PremultipliedColorU8::from_rgba(b, g, r, a).expect("swap to succeed");
    }

    slot.bitmap.pixel_mode = ftwrap::FT_Pixel_Mode::FT_PIXEL_MODE_BGRA as _;
    slot.bitmap.num_grays = 256;
    slot.format = crate::ftwrap::FT_Glyph_Format_::FT_GLYPH_FORMAT_BITMAP;

    Ok(())
}
