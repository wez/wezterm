use crate::config::{configuration, Config, TextStyle};
use crate::font::ftfont::FreeTypeFontImpl;
use crate::font::ftwrap;
use crate::font::hbwrap as harfbuzz;
use crate::font::system::*;
use failure::{Error, Fallible};
use font_kit::error::SelectionError;
use font_kit::family_handle::FamilyHandle;
use font_kit::family_name::FamilyName;
use font_kit::handle::Handle;
use font_kit::properties::Properties;
use font_kit::source::SystemSource;
use font_kit::sources::mem::MemSource;
use font_kit::sources::multi::MultiSource;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

pub type FontSystemImpl = FontKitFontSystem;
pub struct FontKitFontSystem {
    use_fontkit_loader: bool,
}

struct FileSystemDirectorySource {
    mem_source: MemSource,
}

impl FileSystemDirectorySource {
    pub fn new(paths: &[PathBuf]) -> Self {
        let mut fonts = vec![];

        for path in paths {
            for entry in walkdir::WalkDir::new(path).into_iter() {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(_) => continue,
                };

                let path = entry.path();
                let mut file = match std::fs::File::open(path) {
                    Err(_) => continue,
                    Ok(file) => file,
                };

                use font_kit::file_type::FileType;
                match font_kit::font::Font::analyze_file(&mut file) {
                    Err(_) => continue,
                    Ok(FileType::Single) => fonts.push(Handle::from_path(path.to_owned(), 0)),
                    Ok(FileType::Collection(font_count)) => {
                        for font_index in 0..font_count {
                            fonts.push(Handle::from_path(path.to_owned(), font_index))
                        }
                    }
                }
            }
        }

        Self {
            mem_source: MemSource::from_fonts(fonts.into_iter()).unwrap(),
        }
    }
}

impl font_kit::source::Source for FileSystemDirectorySource {
    fn all_fonts(&self) -> Result<Vec<Handle>, SelectionError> {
        self.mem_source.all_fonts()
    }

    fn all_families(&self) -> Result<Vec<String>, SelectionError> {
        self.mem_source.all_families()
    }

    fn select_family_by_name(&self, family_name: &str) -> Result<FamilyHandle, SelectionError> {
        self.mem_source.select_family_by_name(family_name)
    }

    fn select_by_postscript_name(&self, postscript_name: &str) -> Result<Handle, SelectionError> {
        self.mem_source.select_by_postscript_name(postscript_name)
    }
}

impl FontKitFontSystem {
    pub fn new(use_fontkit_loader: bool) -> Self {
        if use_fontkit_loader {
            log::error!(
                "DANGER: the FontKit fontsystem currently has \
                 issues.  Use FontKitAndFreeType instead"
            );
        }
        Self { use_fontkit_loader }
    }

    fn load_font_using_fontkit_render(
        &self,
        config: &Config,
        style: &TextStyle,
        font_scale: f64,
    ) -> Result<Box<dyn NamedFont>, Error> {
        let mut fonts = vec![];

        let point_size = (font_scale * config.font_size) as f32;

        for handle in self.locate_matches(config, style) {
            if let Ok(font) = load_handle_using_system_loader(handle, point_size) {
                fonts.push(font);
            }
        }

        failure::ensure!(!fonts.is_empty(), "unable to resolve any fonts via fontkit");

        Ok(Box::new(FontKitNamedFont { fonts }))
    }

    fn locate_matches(&self, config: &Config, style: &TextStyle) -> Vec<Handle> {
        let dir_source = Box::new(FileSystemDirectorySource::new(&config.font_dirs));
        let system_source = Box::new(SystemSource::new());
        let sources = MultiSource::from_sources(vec![dir_source, system_source]);
        let mut handles = vec![];

        for font in &style.font {
            let mut props = Properties::new();
            if font.bold {
                props.weight(font_kit::properties::Weight::BOLD);
            }
            if font.italic {
                props.style(font_kit::properties::Style::Italic);
            }
            let family = FamilyName::Title(font.family.clone());
            if let Ok(handle) = sources.select_best_match(&[family.clone()], &props) {
                handles.push(handle);
            }
        }

        // Supplement the strict CSS name matching with direct full name
        // matching in case their config file lists eg: "Operator mono SSm Lig Medium"
        // explicitly
        if handles.is_empty() {
            let mut map = HashMap::new();

            if let Ok(all) = sources.all_fonts() {
                for handle in all {
                    if let Ok(loaded) = handle.load() {
                        map.insert(loaded.full_name(), handle);
                    }
                }
            }

            for font in &style.font {
                if let Some(handle) = map.remove(&font.family) {
                    handles.push(handle);
                }
            }
        }

        handles
    }

    fn load_font_using_ft_render(
        &self,
        config: &Config,
        style: &TextStyle,
        font_scale: f64,
    ) -> Result<Box<dyn NamedFont>, Error> {
        let mut lib = ftwrap::Library::new()?;
        // Some systems don't support this mode, so if it fails, we don't
        // care to abort the rest of what we're doing
        match lib.set_lcd_filter(ftwrap::FT_LcdFilter::FT_LCD_FILTER_DEFAULT) {
            Ok(_) => (),
            Err(err) => log::warn!("Ignoring: FT_LcdFilter failed: {:?}", err),
        };

        let mut fonts = vec![];

        for handle in self.locate_matches(config, style) {
            if let Ok(face) = lib.load_font_kit_handle(&handle) {
                fonts.push(FreeTypeFontImpl::with_face_size_and_dpi(
                    face,
                    config.font_size * font_scale,
                    config.dpi as u32,
                )?);
            }
        }

        if fonts.is_empty() {}

        failure::ensure!(!fonts.is_empty(), "unable to resolve any fonts via fontkit");

        Ok(Box::new(
            crate::font::fontloader_and_freetype::NamedFontImpl::new(lib, fonts),
        ))
    }
}

impl FontSystem for FontKitFontSystem {
    fn load_font(
        &self,
        config: &Config,
        style: &TextStyle,
        font_scale: f64,
    ) -> Result<Box<dyn NamedFont>, Error> {
        if self.use_fontkit_loader {
            self.load_font_using_fontkit_render(config, style, font_scale)
        } else {
            self.load_font_using_ft_render(config, style, font_scale)
        }
    }
}

pub struct FontKitFont {
    font: font_kit::font::Font,
    hb: RefCell<harfbuzz::Font>,
    point_size: f32,
}

pub struct FontKitNamedFont {
    fonts: Vec<FontKitFont>,
}

fn load_handle_using_system_loader(handle: Handle, point_size: f32) -> Fallible<FontKitFont> {
    let font = handle.load()?;
    log::error!(
        "loaded font ps={:?} full={}",
        font.postscript_name(),
        font.full_name()
    );

    let hb = RefCell::new(harfbuzz::Font::new_directwrite(
        &font.native_font().dwrite_font_face,
    ));
    Ok(FontKitFont {
        font,
        point_size,
        hb,
    })
}

impl NamedFont for FontKitNamedFont {
    fn get_fallback(&mut self, idx: FallbackIdx) -> Result<&dyn Font, Error> {
        self.fonts
            .get(idx)
            .map(|f| {
                let f: &dyn Font = f;
                f
            })
            .ok_or_else(|| failure::format_err!("no fallback fonts available (idx={})", idx))
    }

    fn shape(&mut self, s: &str) -> Result<Vec<GlyphInfo>, Error> {
        crate::font::shape_with_harfbuzz(self, 0, s)
    }
}

impl Font for FontKitFont {
    fn has_color(&self) -> bool {
        true
    }

    fn harfbuzz_shape(
        &self,
        buf: &mut harfbuzz::Buffer,
        features: Option<&[harfbuzz::hb_feature_t]>,
    ) {
        log::error!("shaping");
        self.hb.borrow_mut().shape(buf, features);
        log::error!("shaped");
    }

    fn metrics(&self) -> FontMetrics {
        let glyph = self
            .font
            .glyph_for_char('h')
            .expect("font to have a W char to get its metrics");
        let origin = self.font.origin(glyph).expect("to get origin for W glyph");
        let bounds = self
            .font
            .raster_bounds(
                glyph,
                self.point_size,
                &font_kit::loader::FontTransform::identity(),
                &origin,
                font_kit::hinting::HintingOptions::Full(self.point_size),
                font_kit::canvas::RasterizationOptions::SubpixelAa,
            )
            .expect("to be able to compute bounds for W")
            .to_f64();

        let metrics = self.font.metrics();

        let scale =
            (self.point_size * configuration().dpi as f32 / 72.) / metrics.units_per_em as f32;
        log::error!(
            "origin={}\nbounds: {:#?}\nmetrics: {:#?}\nscale={}",
            origin,
            bounds,
            metrics,
            scale
        );
        let fm = FontMetrics {
            cell_width: bounds.max_x() - bounds.min_x(),
            cell_height: bounds.max_y() - bounds.min_y(),
            descender: (metrics.descent * scale) as f64,
            underline_thickness: (metrics.underline_thickness * scale) as f64,
            underline_position: (metrics.underline_position * scale) as f64,
        };

        log::error!("fm: {:#?}", fm);

        fm
    }

    fn rasterize_glyph(&self, glyph_pos: u32) -> Result<RasterizedGlyph, Error> {
        let origin = self.font.origin(glyph_pos)?;

        let transform = font_kit::loader::FontTransform::identity();
        let hinting = font_kit::hinting::HintingOptions::Full(self.point_size);
        let raster = font_kit::canvas::RasterizationOptions::SubpixelAa;

        let bounds = self
            .font
            .raster_bounds(
                glyph_pos,
                self.point_size,
                &transform,
                &origin,
                hinting,
                raster,
            )?
            .to_u32();

        let mut canvas =
            font_kit::canvas::Canvas::new(&bounds.size, font_kit::canvas::Format::Rgba32);

        self.font.rasterize_glyph(
            &mut canvas,
            glyph_pos,
            self.point_size,
            &transform,
            &origin,
            hinting,
            raster,
        )?;

        Ok(RasterizedGlyph {
            data: canvas.pixels,
            height: canvas.size.height as usize,
            width: canvas.size.width as usize,
            bearing_x: 0.,
            bearing_y: 0.,
        })
    }
}
