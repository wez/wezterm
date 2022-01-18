use super::box_model::*;
use crate::customglyph::{BlockKey, *};
use crate::glium::texture::SrgbTexture2d;
use crate::glyphcache::{CachedGlyph, GlyphCache};
use crate::quad::Quad;
use crate::shapecache::*;
use crate::tabbar::{TabBarItem, TabEntry};
use crate::termwindow::{
    BorrowedShapeCacheKey, MappedQuads, RenderState, ScrollHit, ShapedInfo, TermWindowNotif,
    UIItem, UIItemType,
};
use crate::utilsprites::RenderMetrics;
use ::window::bitmaps::atlas::OutOfTextureSpace;
use ::window::bitmaps::{TextureCoord, TextureRect, TextureSize};
use ::window::glium::uniforms::{
    MagnifySamplerFilter, MinifySamplerFilter, Sampler, SamplerWrapFunction,
};
use ::window::glium::{uniform, BlendingFunction, LinearBlendingFactor, Surface};
use ::window::{glium, DeadKeyStatus, PointF, RectF, SizeF, WindowOps};
use anyhow::anyhow;
use config::{
    ConfigHandle, Dimension, DimensionContext, HsbTransform, TabBarColors, TextStyle,
    VisualBellTarget,
};
use euclid::num::Zero;
use mux::pane::Pane;
use mux::renderable::{RenderableDimensions, StableCursorPosition};
use mux::tab::{PositionedPane, PositionedSplit, SplitDirection};
use smol::Timer;
use std::ops::Range;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use termwiz::cell::{unicode_column_width, Blink};
use termwiz::cellcluster::CellCluster;
use termwiz::surface::{CursorShape, CursorVisibility};
use wezterm_font::units::{IntPixelLength, PixelLength};
use wezterm_font::{ClearShapeCache, GlyphInfo, LoadedFont};
use wezterm_term::color::{ColorAttribute, ColorPalette, RgbColor};
use wezterm_term::{CellAttributes, Line, StableRowIndex};
use window::bitmaps::atlas::SpriteSlice;
use window::bitmaps::Texture2d;
use window::color::LinearRgba;

const TOP_LEFT_ROUNDED_CORNER: &[Poly] = &[Poly {
    path: &[
        PolyCommand::MoveTo(BlockCoord::One, BlockCoord::One),
        PolyCommand::LineTo(BlockCoord::One, BlockCoord::Zero),
        PolyCommand::QuadTo {
            control: (BlockCoord::Zero, BlockCoord::Zero),
            to: (BlockCoord::Zero, BlockCoord::One),
        },
        PolyCommand::Close,
    ],
    intensity: BlockAlpha::Full,
    style: PolyStyle::Fill,
}];

const TOP_RIGHT_ROUNDED_CORNER: &[Poly] = &[Poly {
    path: &[
        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::One),
        PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::Zero),
        PolyCommand::QuadTo {
            control: (BlockCoord::One, BlockCoord::Zero),
            to: (BlockCoord::One, BlockCoord::One),
        },
        PolyCommand::Close,
    ],
    intensity: BlockAlpha::Full,
    style: PolyStyle::Fill,
}];

const X_BUTTON: &[Poly] = &[
    Poly {
        path: &[
            PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Zero),
            PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
        ],
        intensity: BlockAlpha::Full,
        style: PolyStyle::Outline,
    },
    Poly {
        path: &[
            PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
            PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
        ],
        intensity: BlockAlpha::Full,
        style: PolyStyle::Outline,
    },
];

const PLUS_BUTTON: &[Poly] = &[
    Poly {
        path: &[
            PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
            PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
        ],
        intensity: BlockAlpha::Full,
        style: PolyStyle::Outline,
    },
    Poly {
        path: &[
            PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
            PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
        ],
        intensity: BlockAlpha::Full,
        style: PolyStyle::Outline,
    },
];

pub struct RenderScreenLineOpenGLParams<'a> {
    /// zero-based offset from top of the window viewport to the line that
    /// needs to be rendered, measured in pixels
    pub top_pixel_y: f32,
    /// zero-based offset from left of the window viewport to the line that
    /// needs to be rendered, measured in pixels
    pub left_pixel_x: f32,
    pub pixel_width: f32,
    pub stable_line_idx: Option<StableRowIndex>,
    pub line: &'a Line,
    pub selection: Range<usize>,
    pub cursor: &'a StableCursorPosition,
    pub palette: &'a ColorPalette,
    pub dims: &'a RenderableDimensions,
    pub config: &'a ConfigHandle,
    pub pane: Option<&'a Rc<dyn Pane>>,

    pub white_space: TextureRect,
    pub filled_box: TextureRect,

    pub cursor_border_color: LinearRgba,
    pub foreground: LinearRgba,
    pub is_active: bool,

    pub selection_fg: LinearRgba,
    pub selection_bg: LinearRgba,
    pub cursor_fg: LinearRgba,
    pub cursor_bg: LinearRgba,

    pub window_is_transparent: bool,
    pub default_bg: LinearRgba,

    /// Override font resolution; useful together with
    /// the resolved title font
    pub font: Option<Rc<LoadedFont>>,
    pub style: Option<&'a TextStyle>,

    /// If true, use the shaper-determined pixel positions,
    /// rather than using monospace cell based positions.
    pub use_pixel_positioning: bool,
    pub pre_shaped: Option<&'a Vec<ShapedCluster<'a>>>,

    pub render_metrics: RenderMetrics,
}

pub struct ComputeCellFgBgParams<'a> {
    pub stable_line_idx: Option<StableRowIndex>,
    pub cell_idx: usize,
    pub cursor: &'a StableCursorPosition,
    pub selection: &'a Range<usize>,
    pub fg_color: LinearRgba,
    pub bg_color: LinearRgba,
    pub palette: &'a ColorPalette,
    pub is_active_pane: bool,
    pub config: &'a ConfigHandle,
    pub selection_fg: LinearRgba,
    pub selection_bg: LinearRgba,
    pub cursor_fg: LinearRgba,
    pub cursor_bg: LinearRgba,
    pub cursor_border_color: LinearRgba,
    pub in_composition: bool,
    pub pane: Option<&'a Rc<dyn Pane>>,
}

#[derive(Debug)]
pub struct ComputeCellFgBgResult {
    pub fg_color: LinearRgba,
    pub bg_color: LinearRgba,
    pub cursor_border_color: LinearRgba,
    pub cursor_shape: Option<CursorShape>,
}

/// Basic cache of computed data from prior cluster to avoid doing the same
/// work for space separated clusters with the same style
#[derive(Clone, Debug)]
pub struct ClusterStyleCache<'a> {
    attrs: &'a CellAttributes,
    style: &'a TextStyle,
    underline_tex_rect: TextureRect,
    fg_color: LinearRgba,
    bg_color: LinearRgba,
    underline_color: LinearRgba,
}

#[derive(Clone, Debug)]
pub struct ShapedCluster<'a> {
    style: ClusterStyleCache<'a>,
    x_pos: f32,
    pixel_width: f32,
    cluster: &'a CellCluster,
    glyph_info: Rc<Vec<ShapedInfo<SrgbTexture2d>>>,
}

impl super::TermWindow {
    pub fn paint_impl(&mut self, frame: &mut glium::Frame) {
        // If nothing on screen needs animating, then we can avoid
        // invalidating as frequently
        *self.has_animation.borrow_mut() = None;
        // Start with the assumption that we should allow images to render
        self.allow_images = true;

        let start = Instant::now();

        frame.clear_color(0., 0., 0., 0.);

        'pass: for pass in 0.. {
            match self.paint_opengl_pass() {
                Ok(_) => {
                    let mut allocated = false;
                    for vb_idx in 0..3 {
                        if let Some(need_quads) =
                            self.render_state.as_mut().unwrap().vb[vb_idx].need_more_quads()
                        {
                            self.invalidate_fancy_tab_bar();

                            // Round up to next multiple of 1024 that is >=
                            // the number of needed quads for this frame
                            let num_quads = (need_quads + 1023) & !1023;
                            if let Err(err) = self
                                .render_state
                                .as_mut()
                                .unwrap()
                                .reallocate_quads(vb_idx, num_quads)
                            {
                                log::error!(
                                    "Failed to allocate {} quads (needed {}): {:#}",
                                    num_quads,
                                    need_quads,
                                    err
                                );
                                break 'pass;
                            }
                            log::trace!("Allocated {} quads (needed {})", num_quads, need_quads);
                            allocated = true;
                        }
                    }
                    if !allocated {
                        break 'pass;
                    }
                }
                Err(err) => {
                    if let Some(&OutOfTextureSpace {
                        size: Some(size),
                        current_size,
                    }) = err.root_cause().downcast_ref::<OutOfTextureSpace>()
                    {
                        let result = if pass == 0 {
                            // Let's try clearing out the atlas and trying again
                            // self.clear_texture_atlas()
                            log::trace!("recreate_texture_atlas");
                            self.recreate_texture_atlas(Some(current_size))
                        } else {
                            log::trace!("grow texture atlas to {}", size);
                            self.recreate_texture_atlas(Some(size))
                        };
                        self.invalidate_fancy_tab_bar();

                        if let Err(err) = result {
                            if self.allow_images {
                                self.allow_images = false;
                                log::info!(
                                    "Not enough texture space ({:#}); \
                                     will retry render with images disabled",
                                    err
                                );
                            } else {
                                log::error!(
                                    "Failed to {} texture: {}",
                                    if pass == 0 { "clear" } else { "resize" },
                                    err
                                );
                                break 'pass;
                            }
                        }
                    } else if err.root_cause().downcast_ref::<ClearShapeCache>().is_some() {
                        self.invalidate_fancy_tab_bar();
                        self.shape_cache.borrow_mut().clear();
                    } else {
                        log::error!("paint_opengl_pass failed: {:#}", err);
                        break 'pass;
                    }
                }
            }
        }
        log::debug!("paint_impl before call_draw elapsed={:?}", start.elapsed());

        self.call_draw(frame).ok();
        log::debug!("paint_impl elapsed={:?}", start.elapsed());
        metrics::histogram!("gui.paint.opengl", start.elapsed());
        metrics::histogram!("gui.paint.opengl.rate", 1.);
        self.update_title_post_status();

        // If self.has_animation is some, then the last render detected
        // image attachments with multiple frames, so we also need to
        // invalidate the viewport when the next frame is due
        if self.focused.is_some() {
            if let Some(next_due) = *self.has_animation.borrow() {
                if Some(next_due) != *self.scheduled_animation.borrow() {
                    self.scheduled_animation.borrow_mut().replace(next_due);
                    let window = self.window.clone().take().unwrap();
                    promise::spawn::spawn(async move {
                        Timer::at(next_due).await;
                        let win = window.clone();
                        window.notify(TermWindowNotif::Apply(Box::new(move |tw| {
                            tw.scheduled_animation.borrow_mut().take();
                            win.invalidate();
                        })));
                    })
                    .detach();
                }
            }
        }
    }

    pub fn update_next_frame_time(&self, next_due: Option<Instant>) {
        if let Some(next_due) = next_due {
            let mut has_anim = self.has_animation.borrow_mut();
            match *has_anim {
                None => {
                    has_anim.replace(next_due);
                }
                Some(t) if next_due < t => {
                    has_anim.replace(next_due);
                }
                _ => {}
            }
        }
    }

    fn get_intensity_if_bell_target_ringing(
        &self,
        pane: &Rc<dyn Pane>,
        config: &ConfigHandle,
        target: VisualBellTarget,
    ) -> Option<f32> {
        let mut per_pane = self.pane_state(pane.pane_id());
        if let Some(ringing) = per_pane.bell_start {
            if config.visual_bell.target == target {
                let elapsed = ringing.elapsed().as_secs_f32();

                let in_duration =
                    Duration::from_millis(config.visual_bell.fade_in_duration_ms).as_secs_f32();
                let out_duration =
                    Duration::from_millis(config.visual_bell.fade_out_duration_ms).as_secs_f32();

                let intensity = if elapsed < in_duration {
                    Some(
                        config
                            .visual_bell
                            .fade_in_function
                            .evaluate_at_position(elapsed / in_duration),
                    )
                } else {
                    let completion = (elapsed - in_duration) / out_duration;
                    if completion >= 1.0 {
                        None
                    } else {
                        Some(
                            1.0 - config
                                .visual_bell
                                .fade_out_function
                                .evaluate_at_position(completion),
                        )
                    }
                };

                match intensity {
                    None => {
                        per_pane.bell_start.take();
                    }
                    Some(intensity) => {
                        self.update_next_frame_time(Some(
                            Instant::now() + Duration::from_millis(1000 / config.max_fps as u64),
                        ));
                        return Some(intensity);
                    }
                }
            }
        }
        None
    }

    pub fn filled_rectangle<'a>(
        &self,
        layer: &'a mut MappedQuads,
        rect: RectF,
        color: LinearRgba,
    ) -> anyhow::Result<Quad<'a>> {
        let mut quad = layer.allocate()?;
        let left_offset = self.dimensions.pixel_width as f32 / 2.;
        let top_offset = self.dimensions.pixel_height as f32 / 2.;
        let gl_state = self.render_state.as_ref().unwrap();
        quad.set_position(
            rect.min_x() as f32 - left_offset,
            rect.min_y() as f32 - top_offset,
            rect.max_x() as f32 - left_offset,
            rect.max_y() as f32 - top_offset,
        );
        quad.set_texture(gl_state.util_sprites.filled_box.texture_coords());
        quad.set_is_background();
        quad.set_fg_color(color);
        quad.set_hsv(None);
        Ok(quad)
    }

    pub fn poly_quad<'a>(
        &self,
        layer: &'a mut MappedQuads,
        point: PointF,
        polys: &'static [Poly],
        underline_height: IntPixelLength,
        cell_size: SizeF,
        color: LinearRgba,
    ) -> anyhow::Result<Quad<'a>> {
        let left_offset = self.dimensions.pixel_width as f32 / 2.;
        let top_offset = self.dimensions.pixel_height as f32 / 2.;
        let gl_state = self.render_state.as_ref().unwrap();
        let sprite = gl_state
            .glyph_cache
            .borrow_mut()
            .cached_block(
                BlockKey::PolyWithCustomMetrics {
                    polys,
                    underline_height,
                    cell_size: euclid::size2(cell_size.width as isize, cell_size.height as isize),
                },
                &self.render_metrics,
            )?
            .texture_coords();

        let mut quad = layer.allocate()?;

        quad.set_position(
            point.x - left_offset,
            point.y - top_offset,
            (point.x + cell_size.width as f32) - left_offset,
            (point.y + cell_size.height as f32) - top_offset,
        );
        quad.set_texture(sprite);
        quad.set_fg_color(color);
        quad.set_hsv(None);
        quad.set_has_color(false);
        Ok(quad)
    }

    pub fn tab_bar_pixel_height_impl(
        config: &ConfigHandle,
        fontconfig: &wezterm_font::FontConfiguration,
        render_metrics: &RenderMetrics,
    ) -> anyhow::Result<f32> {
        if config.use_fancy_tab_bar {
            let font = fontconfig.title_font()?;
            Ok((font.metrics().cell_height.get() as f32 * 2.).ceil())
        } else {
            Ok(render_metrics.cell_size.height as f32)
        }
    }

    pub fn tab_bar_pixel_height(&self) -> anyhow::Result<f32> {
        Self::tab_bar_pixel_height_impl(&self.config, &self.fonts, &self.render_metrics)
    }

    pub fn invalidate_fancy_tab_bar(&mut self) {
        self.fancy_tab_bar.take();
    }

    pub fn build_fancy_tab_bar(&self, palette: &ColorPalette) -> anyhow::Result<ComputedElement> {
        let font = self.fonts.title_font()?;
        let metrics = RenderMetrics::with_font_metrics(&font.metrics());
        let items = self.tab_bar.items();
        let colors = self
            .config
            .colors
            .as_ref()
            .and_then(|c| c.tab_bar.as_ref())
            .cloned()
            .unwrap_or_else(TabBarColors::default);

        let mut left_eles = vec![];
        let mut right_eles = vec![];

        let item_to_elem = |item: &TabEntry| -> Element {
            let element = Element::with_line(&font, &item.title, palette);

            let bg_color = item
                .title
                .cells()
                .get(0)
                .and_then(|c| match c.attrs().background() {
                    ColorAttribute::Default => None,
                    col => Some(palette.resolve_bg(col)),
                });
            let fg_color = item
                .title
                .cells()
                .get(0)
                .and_then(|c| match c.attrs().foreground() {
                    ColorAttribute::Default => None,
                    col => Some(palette.resolve_fg(col)),
                });

            match item.item {
                TabBarItem::None => element
                    .item_type(UIItemType::TabBar(TabBarItem::None))
                    .line_height(Some(1.75))
                    .margin(BoxDimension {
                        left: Dimension::Cells(0.),
                        right: Dimension::Cells(0.),
                        top: Dimension::Cells(0.0),
                        bottom: Dimension::Cells(0.),
                    })
                    .padding(BoxDimension {
                        left: Dimension::Cells(0.5),
                        right: Dimension::Cells(0.),
                        top: Dimension::Cells(0.),
                        bottom: Dimension::Cells(0.),
                    })
                    .border(BoxDimension::new(Dimension::Pixels(0.)))
                    .colors(ElementColors {
                        border: BorderColor::default(),
                        bg: rgbcolor_to_window_color(colors.inactive_tab.bg_color).into(),
                        text: rgbcolor_to_window_color(colors.inactive_tab.fg_color).into(),
                    }),
                TabBarItem::NewTabButton => Element::new(
                    &font,
                    ElementContent::Poly {
                        line_width: metrics.underline_height.max(2),
                        poly: SizedPoly {
                            poly: PLUS_BUTTON,
                            width: Dimension::Pixels(metrics.cell_size.width as f32 * 0.75),
                            height: Dimension::Pixels(metrics.cell_size.width as f32 * 0.75),
                        },
                    },
                )
                .vertical_align(VerticalAlign::Middle)
                .item_type(UIItemType::TabBar(item.item.clone()))
                .margin(BoxDimension {
                    left: Dimension::Cells(0.5),
                    right: Dimension::Cells(0.),
                    top: Dimension::Cells(0.2),
                    bottom: Dimension::Cells(0.),
                })
                .padding(BoxDimension {
                    left: Dimension::Cells(0.5),
                    right: Dimension::Cells(0.5),
                    top: Dimension::Cells(0.2),
                    bottom: Dimension::Cells(0.25),
                })
                .border(BoxDimension::new(Dimension::Pixels(1.)))
                .colors(ElementColors {
                    border: BorderColor::default(),
                    bg: rgbcolor_to_window_color(colors.new_tab.bg_color).into(),
                    text: rgbcolor_to_window_color(colors.new_tab.fg_color).into(),
                })
                .hover_colors(Some(ElementColors {
                    border: BorderColor::default(),
                    bg: rgbcolor_to_window_color(colors.new_tab_hover.bg_color).into(),
                    text: rgbcolor_to_window_color(colors.new_tab_hover.fg_color).into(),
                })),
                TabBarItem::Tab { active, .. } if active => element
                    .item_type(UIItemType::TabBar(item.item.clone()))
                    .margin(BoxDimension {
                        left: Dimension::Cells(0.),
                        right: Dimension::Cells(0.),
                        top: Dimension::Cells(0.2),
                        bottom: Dimension::Cells(0.),
                    })
                    .padding(BoxDimension {
                        left: Dimension::Cells(0.5),
                        right: Dimension::Cells(0.5),
                        top: Dimension::Cells(0.2),
                        bottom: Dimension::Cells(0.25),
                    })
                    .border(BoxDimension::new(Dimension::Pixels(1.)))
                    .border_corners(Some(Corners {
                        top_left: SizedPoly {
                            width: Dimension::Cells(0.5),
                            height: Dimension::Cells(0.5),
                            poly: TOP_LEFT_ROUNDED_CORNER,
                        },
                        top_right: SizedPoly {
                            width: Dimension::Cells(0.5),
                            height: Dimension::Cells(0.5),
                            poly: TOP_RIGHT_ROUNDED_CORNER,
                        },
                        bottom_left: SizedPoly::none(),
                        bottom_right: SizedPoly::none(),
                    }))
                    .colors(ElementColors {
                        border: BorderColor::new(rgbcolor_to_window_color(
                            bg_color.unwrap_or(colors.active_tab.bg_color),
                        )),
                        bg: rgbcolor_to_window_color(
                            bg_color.unwrap_or(colors.active_tab.bg_color),
                        )
                        .into(),
                        text: rgbcolor_to_window_color(
                            fg_color.unwrap_or(colors.active_tab.fg_color),
                        )
                        .into(),
                    }),
                TabBarItem::Tab { .. } => element
                    .item_type(UIItemType::TabBar(item.item.clone()))
                    .margin(BoxDimension {
                        left: Dimension::Cells(0.),
                        right: Dimension::Cells(0.),
                        top: Dimension::Cells(0.2),
                        bottom: Dimension::Cells(0.),
                    })
                    .padding(BoxDimension {
                        left: Dimension::Cells(0.5),
                        right: Dimension::Cells(0.5),
                        top: Dimension::Cells(0.2),
                        bottom: Dimension::Cells(0.25),
                    })
                    .border(BoxDimension::new(Dimension::Pixels(1.)))
                    .border_corners(Some(Corners {
                        top_left: SizedPoly {
                            width: Dimension::Cells(0.5),
                            height: Dimension::Cells(0.5),
                            poly: TOP_LEFT_ROUNDED_CORNER,
                        },
                        top_right: SizedPoly {
                            width: Dimension::Cells(0.5),
                            height: Dimension::Cells(0.5),
                            poly: TOP_RIGHT_ROUNDED_CORNER,
                        },
                        bottom_left: SizedPoly {
                            width: Dimension::Cells(0.),
                            height: Dimension::Cells(0.33),
                            poly: &[],
                        },
                        bottom_right: SizedPoly {
                            width: Dimension::Cells(0.),
                            height: Dimension::Cells(0.33),
                            poly: &[],
                        },
                    }))
                    .colors({
                        let bg = rgbcolor_to_window_color(
                            bg_color.unwrap_or(colors.inactive_tab.bg_color),
                        );
                        let edge = rgbcolor_to_window_color(colors.inactive_tab_edge);
                        ElementColors {
                            border: BorderColor {
                                left: bg,
                                right: edge,
                                top: bg,
                                bottom: bg,
                            },
                            bg: bg.into(),
                            text: rgbcolor_to_window_color(
                                fg_color.unwrap_or(colors.inactive_tab.fg_color),
                            )
                            .into(),
                        }
                    })
                    .hover_colors(Some(ElementColors {
                        border: BorderColor::new(rgbcolor_to_window_color(
                            bg_color.unwrap_or(colors.inactive_tab_hover.bg_color),
                        )),
                        bg: rgbcolor_to_window_color(
                            bg_color.unwrap_or(colors.inactive_tab_hover.bg_color),
                        )
                        .into(),
                        text: rgbcolor_to_window_color(
                            fg_color.unwrap_or(colors.inactive_tab_hover.fg_color),
                        )
                        .into(),
                    })),
            }
        };

        let num_tabs: f32 = items
            .iter()
            .map(|item| match item.item {
                TabBarItem::NewTabButton | TabBarItem::Tab { .. } => 1.,
                _ => 0.,
            })
            .sum();
        let max_tab_width = ((self.dimensions.pixel_width as f32 / num_tabs)
            - (1.5 * metrics.cell_size.width as f32))
            .max(0.);

        for item in items {
            match item.item {
                TabBarItem::None => right_eles.push(item_to_elem(item)),
                TabBarItem::Tab { tab_idx, active } => {
                    let mut elem = item_to_elem(item);
                    elem.max_width = Some(Dimension::Pixels(max_tab_width));
                    elem.content = match elem.content {
                        ElementContent::Text(_) => unreachable!(),
                        ElementContent::Poly { .. } => unreachable!(),
                        ElementContent::Children(mut kids) => {
                            let x_button = Element::new(
                                &font,
                                ElementContent::Poly {
                                    line_width: metrics.underline_height.max(2),
                                    poly: SizedPoly {
                                        poly: X_BUTTON,
                                        width: Dimension::Cells(0.5),
                                        height: Dimension::Cells(0.5),
                                    },
                                },
                            )
                            .vertical_align(VerticalAlign::Middle)
                            .float(Float::Right)
                            .item_type(UIItemType::CloseTab(tab_idx))
                            .hover_colors(Some(ElementColors {
                                border: BorderColor::default(),
                                bg: rgbcolor_to_window_color(if active {
                                    colors.inactive_tab_hover.bg_color
                                } else {
                                    colors.active_tab.bg_color
                                })
                                .into(),
                                text: rgbcolor_to_window_color(if active {
                                    colors.inactive_tab_hover.fg_color
                                } else {
                                    colors.active_tab.fg_color
                                })
                                .into(),
                            }))
                            .padding(BoxDimension {
                                left: Dimension::Cells(0.25),
                                right: Dimension::Cells(0.25),
                                top: Dimension::Cells(0.25),
                                bottom: Dimension::Cells(0.25),
                            })
                            .margin(BoxDimension {
                                left: Dimension::Cells(0.5),
                                right: Dimension::Cells(0.),
                                top: Dimension::Cells(0.),
                                bottom: Dimension::Cells(0.),
                            });

                            kids.push(x_button);
                            ElementContent::Children(kids)
                        }
                    };
                    left_eles.push(elem);
                }
                _ => left_eles.push(item_to_elem(item)),
            }
        }

        let bar_colors = ElementColors {
            border: BorderColor::default(),
            bg: rgbcolor_to_window_color(if self.focused.is_some() {
                self.config.window_frame.active_titlebar_bg
            } else {
                self.config.window_frame.inactive_titlebar_bg
            })
            .into(),
            text: rgbcolor_to_window_color(if self.focused.is_some() {
                self.config.window_frame.active_titlebar_fg
            } else {
                self.config.window_frame.inactive_titlebar_fg
            })
            .into(),
        };

        let left_ele = Element::new(&font, ElementContent::Children(left_eles))
            .vertical_align(VerticalAlign::Bottom)
            .colors(bar_colors.clone())
            .padding(BoxDimension {
                left: Dimension::Cells(0.5),
                right: Dimension::Cells(0.),
                top: Dimension::Cells(0.),
                bottom: Dimension::Cells(0.),
            });
        let right_ele = Element::new(&font, ElementContent::Children(right_eles))
            .colors(bar_colors.clone())
            .float(Float::Right)
            .zindex(-1);

        let content = ElementContent::Children(vec![left_ele, right_ele]);

        let tabs = Element::new(&font, content)
            .display(DisplayType::Block)
            .item_type(UIItemType::TabBar(TabBarItem::None))
            .min_width(Some(Dimension::Pixels(self.dimensions.pixel_width as f32)))
            .colors(bar_colors);

        let mut computed = self.compute_element(
            &LayoutContext {
                height: DimensionContext {
                    dpi: self.dimensions.dpi as f32,
                    pixel_max: self.dimensions.pixel_height as f32,
                    pixel_cell: metrics.cell_size.height as f32,
                },
                width: DimensionContext {
                    dpi: self.dimensions.dpi as f32,
                    pixel_max: self.dimensions.pixel_width as f32,
                    pixel_cell: metrics.cell_size.width as f32,
                },
                bounds: euclid::rect(
                    0.,
                    0.,
                    self.dimensions.pixel_width as f32,
                    self.dimensions.pixel_height as f32,
                ),
                metrics: &metrics,
                gl_state: self.render_state.as_ref().unwrap(),
            },
            &tabs,
        )?;

        if self.config.tab_bar_at_bottom {
            computed.translate(euclid::vec2(
                0.,
                self.dimensions.pixel_height as f32 - computed.bounds.height(),
            ));
        }

        Ok(computed)
    }

    fn paint_fancy_tab_bar(&self) -> anyhow::Result<Vec<UIItem>> {
        let computed = self
            .fancy_tab_bar
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("paint_tab_bar called but fancy_tab_bar is None"))?;
        let ui_items = computed.ui_items();

        let gl_state = self.render_state.as_ref().unwrap();
        let vb = &gl_state.vb[1];
        let mut vb_mut = vb.current_vb_mut();
        let mut layer1 = vb.map(&mut vb_mut);
        self.render_element(&computed, &mut layer1, None)?;

        Ok(ui_items)
    }

    fn paint_tab_bar(&mut self) -> anyhow::Result<()> {
        if self.config.use_fancy_tab_bar {
            if self.fancy_tab_bar.is_none() {
                let palette = self.palette().clone();
                self.fancy_tab_bar
                    .replace(self.build_fancy_tab_bar(&palette)?);
            }

            self.ui_items.append(&mut self.paint_fancy_tab_bar()?);
            return Ok(());
        }

        let palette = self.palette().clone();
        let tab_bar_height = self.tab_bar_pixel_height()?;
        let tab_bar_y = if self.config.tab_bar_at_bottom {
            ((self.dimensions.pixel_height as f32) - tab_bar_height).max(0.)
        } else {
            0.
        };

        // Register the tab bar location
        self.ui_items.append(&mut self.tab_bar.compute_ui_items(
            tab_bar_y as usize,
            self.render_metrics.cell_size.height as usize,
            self.render_metrics.cell_size.width as usize,
        ));

        let window_is_transparent =
            self.window_background.is_some() || self.config.window_background_opacity != 1.0;
        let gl_state = self.render_state.as_ref().unwrap();
        let white_space = gl_state.util_sprites.white_space.texture_coords();
        let filled_box = gl_state.util_sprites.filled_box.texture_coords();
        let default_bg = rgbcolor_alpha_to_window_color(
            palette.resolve_bg(ColorAttribute::Default),
            if window_is_transparent {
                0.
            } else {
                self.config.text_background_opacity
            },
        );

        let vb = [&gl_state.vb[0], &gl_state.vb[1], &gl_state.vb[2]];
        let mut vb_mut0 = vb[0].current_vb_mut();
        let mut vb_mut1 = vb[1].current_vb_mut();
        let mut vb_mut2 = vb[2].current_vb_mut();
        let mut layers = [
            vb[0].map(&mut vb_mut0),
            vb[1].map(&mut vb_mut1),
            vb[2].map(&mut vb_mut2),
        ];
        self.render_screen_line_opengl(
            RenderScreenLineOpenGLParams {
                top_pixel_y: tab_bar_y,
                left_pixel_x: 0.,
                pixel_width: self.dimensions.pixel_width as f32,
                stable_line_idx: None,
                line: self.tab_bar.line(),
                selection: 0..0,
                cursor: &Default::default(),
                palette: &palette,
                dims: &RenderableDimensions {
                    cols: self.dimensions.pixel_width
                        / self.render_metrics.cell_size.width as usize,
                    physical_top: 0,
                    scrollback_rows: 0,
                    scrollback_top: 0,
                    viewport_rows: 1,
                },
                config: &self.config,
                cursor_border_color: LinearRgba::default(),
                foreground: rgbcolor_to_window_color(palette.foreground),
                pane: None,
                is_active: true,
                selection_fg: LinearRgba::default(),
                selection_bg: LinearRgba::default(),
                cursor_fg: LinearRgba::default(),
                cursor_bg: LinearRgba::default(),
                white_space,
                filled_box,
                window_is_transparent,
                default_bg,
                style: None,
                font: None,
                use_pixel_positioning: self.config.experimental_pixel_positioning,
                pre_shaped: None,
                render_metrics: self.render_metrics,
            },
            &mut layers,
        )?;

        Ok(())
    }

    pub fn paint_pane_opengl(
        &mut self,
        pos: &PositionedPane,
        num_panes: usize,
    ) -> anyhow::Result<()> {
        self.check_for_dirty_lines_and_invalidate_selection(&pos.pane);
        /*
        let zone = {
            let dims = pos.pane.get_dimensions();
            let position = self
                .get_viewport(pos.pane.pane_id())
                .unwrap_or(dims.physical_top);

            let zones = self.get_semantic_zones(&pos.pane);
            let idx = match zones.binary_search_by(|zone| zone.start_y.cmp(&position)) {
                Ok(idx) | Err(idx) => idx,
            };
            let idx = ((idx as isize) - 1).max(0) as usize;
            zones.get(idx).cloned()
        };
        */

        let global_bg_color = self.palette().background;
        let config = &self.config;
        let palette = pos.pane.palette();

        let (padding_left, padding_top) = self.padding_left_top();

        let tab_bar_height = if self.show_tab_bar && !self.config.tab_bar_at_bottom {
            self.tab_bar_pixel_height()?
        } else {
            0.
        };
        let top_pixel_y = tab_bar_height + padding_top;

        let cursor = pos.pane.get_cursor_position();
        if pos.is_active {
            self.prev_cursor.update(&cursor);
        }

        let current_viewport = self.get_viewport(pos.pane.pane_id());
        let (stable_top, lines);
        let dims = pos.pane.get_dimensions();

        {
            let stable_range = match current_viewport {
                Some(top) => top..top + dims.viewport_rows as StableRowIndex,
                None => dims.physical_top..dims.physical_top + dims.viewport_rows as StableRowIndex,
            };

            let start = Instant::now();
            let (top, vp_lines) = pos
                .pane
                .get_lines_with_hyperlinks_applied(stable_range, &self.config.hyperlink_rules);
            metrics::histogram!("get_lines_with_hyperlinks_applied.latency", start.elapsed());
            log::trace!(
                "get_lines_with_hyperlinks_applied took {:?}",
                start.elapsed()
            );
            stable_top = top;
            lines = vp_lines;
        }

        let gl_state = self.render_state.as_ref().unwrap();
        let vb = [&gl_state.vb[0], &gl_state.vb[1], &gl_state.vb[2]];

        let start = Instant::now();
        let mut vb_mut0 = vb[0].current_vb_mut();
        let mut vb_mut1 = vb[1].current_vb_mut();
        let mut vb_mut2 = vb[2].current_vb_mut();
        let mut layers = [
            vb[0].map(&mut vb_mut0),
            vb[1].map(&mut vb_mut1),
            vb[2].map(&mut vb_mut2),
        ];
        log::trace!("quad map elapsed {:?}", start.elapsed());
        metrics::histogram!("quad.map", start.elapsed());

        let cursor_border_color = rgbcolor_to_window_color(palette.cursor_border);
        let foreground = rgbcolor_to_window_color(palette.foreground);
        let white_space = gl_state.util_sprites.white_space.texture_coords();
        let filled_box = gl_state.util_sprites.filled_box.texture_coords();

        let window_is_transparent =
            self.window_background.is_some() || config.window_background_opacity != 1.0;

        let default_bg = rgbcolor_alpha_to_window_color(
            palette.resolve_bg(ColorAttribute::Default),
            if window_is_transparent {
                0.
            } else {
                config.text_background_opacity
            },
        );

        // Render the full window background
        if pos.index == 0 {
            match (self.window_background.as_ref(), self.allow_images) {
                (Some(im), true) => {
                    // Render the window background image
                    let color = rgbcolor_alpha_to_window_color(
                        palette.background,
                        config.window_background_opacity,
                    );

                    let (sprite, next_due) =
                        gl_state.glyph_cache.borrow_mut().cached_image(im, None)?;
                    self.update_next_frame_time(next_due);
                    let mut quad = layers[0].allocate()?;
                    quad.set_position(
                        self.dimensions.pixel_width as f32 / -2.,
                        self.dimensions.pixel_height as f32 / -2.,
                        self.dimensions.pixel_width as f32 / 2.,
                        self.dimensions.pixel_height as f32 / 2.,
                    );
                    quad.set_texture(sprite.texture_coords());
                    quad.set_is_background_image();
                    quad.set_hsv(config.window_background_image_hsb);
                    quad.set_fg_color(color);
                }
                _ if window_is_transparent && num_panes > 1 => {
                    // Avoid doubling up the background color: the panes
                    // will render out through the padding so there
                    // should be no gaps that need filling in
                }
                _ => {
                    // Regular window background color
                    let background = rgbcolor_alpha_to_window_color(
                        if num_panes == 1 {
                            // If we're the only pane, use the pane's palette
                            // to draw the padding background
                            palette.background
                        } else {
                            global_bg_color
                        },
                        config.window_background_opacity,
                    );
                    self.filled_rectangle(
                        &mut layers[0],
                        euclid::rect(
                            0.,
                            0.,
                            self.dimensions.pixel_width as f32,
                            self.dimensions.pixel_height as f32,
                        ),
                        background,
                    )?;
                }
            }
        }

        if num_panes > 1 && self.window_background.is_none() {
            // Per-pane, palette-specified background
            let cell_width = self.render_metrics.cell_size.width as f32;
            let cell_height = self.render_metrics.cell_size.height as f32;

            // We want to fill out to the edges of the splits
            let (x, width_delta) = if pos.left == 0 {
                (0., padding_left + (cell_width / 2.0))
            } else {
                (
                    padding_left - (cell_width / 2.0) + (pos.left as f32 * cell_width),
                    cell_width,
                )
            };

            let (y, height_delta) = if pos.top == 0 {
                (
                    (top_pixel_y - padding_top),
                    padding_top + (cell_height / 2.0),
                )
            } else {
                (
                    top_pixel_y + (pos.top as f32 * cell_height) - (cell_height / 2.0),
                    cell_height,
                )
            };

            let mut quad = self.filled_rectangle(
                &mut layers[0],
                euclid::rect(
                    x,
                    y,
                    (pos.width as f32 * cell_width)
                        + width_delta
                        + if pos.left + pos.width >= self.terminal_size.cols as usize {
                            // And all the way to the right edge if we're right-most
                            crate::termwindow::resize::effective_right_padding(
                                &self.config,
                                DimensionContext {
                                    dpi: self.dimensions.dpi as f32,
                                    pixel_max: self.terminal_size.pixel_width as f32,
                                    pixel_cell: cell_width,
                                },
                            ) as f32
                        } else {
                            0.
                        },
                    (pos.height as f32 * cell_height)
                        + height_delta as f32
                        + if pos.top + pos.height >= self.terminal_size.rows as usize {
                            // And all the way to the bottom if we're bottom-most
                            self.config
                                .window_padding
                                .bottom
                                .evaluate_as_pixels(DimensionContext {
                                    dpi: self.dimensions.dpi as f32,
                                    pixel_max: self.terminal_size.pixel_height as f32,
                                    pixel_cell: cell_height,
                                })
                        } else {
                            0.
                        },
                ),
                rgbcolor_alpha_to_window_color(
                    palette.background,
                    config.window_background_opacity,
                ),
            )?;
            quad.set_hsv(if pos.is_active {
                None
            } else {
                Some(config.inactive_pane_hsb)
            });
        }

        {
            // If the bell is ringing, we draw another background layer over the
            // top of this in the configured bell color
            if let Some(intensity) = self.get_intensity_if_bell_target_ringing(
                &pos.pane,
                config,
                VisualBellTarget::BackgroundColor,
            ) {
                // target background color
                let (r, g, b, _) = config
                    .resolved_palette
                    .visual_bell
                    .unwrap_or(palette.foreground)
                    .to_linear_tuple_rgba();

                let background = if window_is_transparent {
                    // for transparent windows, we fade in the target color
                    // by adjusting its alpha
                    LinearRgba::with_components(r, g, b, intensity)
                } else {
                    // otherwise We'll interpolate between the background color
                    // and the the target color
                    let (r1, g1, b1, a) = rgbcolor_alpha_to_window_color(
                        palette.background,
                        config.window_background_opacity,
                    )
                    .tuple();
                    LinearRgba::with_components(
                        r1 + (r - r1) * intensity,
                        g1 + (g - g1) * intensity,
                        b1 + (b - b1) * intensity,
                        a,
                    )
                };
                log::trace!("bell color is {:?}", background);

                let cell_width = self.render_metrics.cell_size.width as f32;
                let cell_height = self.render_metrics.cell_size.height as f32;

                let mut quad = self.filled_rectangle(
                    &mut layers[0],
                    euclid::rect(
                        (pos.left as f32 * cell_width) + padding_left,
                        top_pixel_y + (pos.top as f32 * cell_height) + padding_top,
                        pos.width as f32 * cell_width,
                        pos.height as f32 * cell_height,
                    ),
                    background,
                )?;

                quad.set_hsv(if pos.is_active {
                    None
                } else {
                    Some(config.inactive_pane_hsb)
                });
            }
        }

        // TODO: we only have a single scrollbar in a single position.
        // We only update it for the active pane, but we should probably
        // do a per-pane scrollbar.  That will require more extensive
        // changes to ScrollHit, mouse positioning, PositionedPane
        // and tab size calculation.
        if pos.is_active && self.show_scroll_bar {
            let info = ScrollHit::thumb(
                &*pos.pane,
                current_viewport,
                &self.dimensions,
                tab_bar_height,
                config.tab_bar_at_bottom,
            );
            let thumb_top = info.top as f32;
            let thumb_size = info.height as f32;
            let color = rgbcolor_to_window_color(palette.scrollbar_thumb);

            // Adjust the scrollbar thumb position
            let config = &self.config;
            let padding = self.effective_right_padding(&config) as f32;

            // Register the scroll bar location
            self.ui_items.push(UIItem {
                x: self.dimensions.pixel_width - padding as usize,
                width: padding as usize,
                y: tab_bar_height as usize,
                height: thumb_top as usize,
                item_type: UIItemType::AboveScrollThumb,
            });
            self.ui_items.push(UIItem {
                x: self.dimensions.pixel_width - padding as usize,
                width: padding as usize,
                y: thumb_top as usize,
                height: thumb_size as usize,
                item_type: UIItemType::ScrollThumb,
            });
            self.ui_items.push(UIItem {
                x: self.dimensions.pixel_width - padding as usize,
                width: padding as usize,
                y: (thumb_top + thumb_size) as usize,
                height: self
                    .dimensions
                    .pixel_height
                    .saturating_sub((thumb_top + thumb_size) as usize),
                item_type: UIItemType::BelowScrollThumb,
            });

            self.filled_rectangle(
                &mut layers[2],
                euclid::rect(
                    self.dimensions.pixel_width as f32 - padding,
                    thumb_top,
                    padding,
                    thumb_size,
                ),
                color,
            )?;
        }

        let selrange = self.selection(pos.pane.pane_id()).range.clone();

        let start = Instant::now();
        let selection_fg = rgbcolor_to_window_color(palette.selection_fg);
        let selection_bg = rgbcolor_to_window_color(palette.selection_bg);
        let cursor_fg = rgbcolor_to_window_color(palette.cursor_fg);
        let cursor_bg = rgbcolor_to_window_color(palette.cursor_bg);
        for (line_idx, line) in lines.iter().enumerate() {
            let stable_row = stable_top + line_idx as StableRowIndex;

            let selrange = selrange.map_or(0..0, |sel| sel.cols_for_row(stable_row));
            // Constrain to the pane width!
            let selrange = selrange.start..selrange.end.min(dims.cols);

            self.render_screen_line_opengl(
                RenderScreenLineOpenGLParams {
                    top_pixel_y: top_pixel_y
                        + (line_idx + pos.top) as f32 * self.render_metrics.cell_size.height as f32,
                    left_pixel_x: padding_left
                        + (pos.left as f32 * self.render_metrics.cell_size.width as f32),
                    pixel_width: dims.cols as f32 * self.render_metrics.cell_size.width as f32,
                    stable_line_idx: Some(stable_row),
                    line: &line,
                    selection: selrange,
                    cursor: &cursor,
                    palette: &palette,
                    dims: &dims,
                    config: &config,
                    cursor_border_color,
                    foreground,
                    is_active: pos.is_active,
                    pane: Some(&pos.pane),
                    selection_fg,
                    selection_bg,
                    cursor_fg,
                    cursor_bg,
                    white_space,
                    filled_box,
                    window_is_transparent,
                    default_bg,
                    font: None,
                    style: None,
                    use_pixel_positioning: self.config.experimental_pixel_positioning,
                    pre_shaped: None,
                    render_metrics: self.render_metrics,
                },
                &mut layers,
            )?;
        }
        /*
        if let Some(zone) = zone {
            // TODO: render a thingy to jump to prior prompt
        }
        */
        metrics::histogram!("paint_pane_opengl.lines", start.elapsed());
        log::trace!("lines elapsed {:?}", start.elapsed());

        let start = Instant::now();
        drop(layers);
        metrics::histogram!("paint_pane_opengl.drop.quads", start.elapsed());
        log::trace!("quad drop elapsed {:?}", start.elapsed());

        Ok(())
    }

    pub fn call_draw(&mut self, frame: &mut glium::Frame) -> anyhow::Result<()> {
        let gl_state = self.render_state.as_ref().unwrap();
        let tex = gl_state.glyph_cache.borrow().atlas.texture();
        let projection = euclid::Transform3D::<f32, f32, f32>::ortho(
            -(self.dimensions.pixel_width as f32) / 2.0,
            self.dimensions.pixel_width as f32 / 2.0,
            self.dimensions.pixel_height as f32 / 2.0,
            -(self.dimensions.pixel_height as f32) / 2.0,
            -1.0,
            1.0,
        )
        .to_arrays_transposed();

        let dual_source_blending = glium::DrawParameters {
            blend: glium::Blend {
                color: BlendingFunction::Addition {
                    source: LinearBlendingFactor::SourceOneColor,
                    destination: LinearBlendingFactor::OneMinusSourceOneColor,
                },
                alpha: BlendingFunction::Addition {
                    source: LinearBlendingFactor::SourceOneColor,
                    destination: LinearBlendingFactor::OneMinusSourceOneColor,
                },
                constant_value: (0.0, 0.0, 0.0, 0.0),
            },

            ..Default::default()
        };

        let alpha_blending = glium::DrawParameters {
            blend: glium::Blend {
                color: BlendingFunction::Addition {
                    source: LinearBlendingFactor::SourceAlpha,
                    destination: LinearBlendingFactor::OneMinusSourceAlpha,
                },
                alpha: BlendingFunction::Addition {
                    source: LinearBlendingFactor::One,
                    destination: LinearBlendingFactor::OneMinusSourceAlpha,
                },
                constant_value: (0.0, 0.0, 0.0, 0.0),
            },
            ..Default::default()
        };

        // Clamp and use the nearest texel rather than interpolate.
        // This prevents things like the box cursor outlines from
        // being randomly doubled in width or height
        let atlas_nearest_sampler = Sampler::new(&*tex)
            .wrap_function(SamplerWrapFunction::Clamp)
            .magnify_filter(MagnifySamplerFilter::Nearest)
            .minify_filter(MinifySamplerFilter::Nearest);

        let atlas_linear_sampler = Sampler::new(&*tex)
            .wrap_function(SamplerWrapFunction::Clamp)
            .magnify_filter(MagnifySamplerFilter::Linear)
            .minify_filter(MinifySamplerFilter::Linear);

        let foreground_text_hsb = self.config.foreground_text_hsb;
        let foreground_text_hsb = (
            foreground_text_hsb.hue,
            foreground_text_hsb.saturation,
            foreground_text_hsb.brightness,
        );

        for idx in 0..3 {
            let vb = &gl_state.vb[idx];
            let (vertex_count, index_count) = vb.vertex_index_count();
            if vertex_count > 0 {
                let vertices = vb.current_vb();
                let subpixel_aa = idx == 1;

                frame.draw(
                    vertices.slice(0..vertex_count).unwrap(),
                    vb.indices.slice(0..index_count).unwrap(),
                    &gl_state.glyph_prog,
                    &uniform! {
                        projection: projection,
                        atlas_nearest_sampler:  atlas_nearest_sampler,
                        atlas_linear_sampler:  atlas_linear_sampler,
                        foreground_text_hsb: foreground_text_hsb,
                        subpixel_aa: subpixel_aa,
                    },
                    if subpixel_aa {
                        &dual_source_blending
                    } else {
                        &alpha_blending
                    },
                )?;
            }

            vb.next_index();
        }

        Ok(())
    }

    pub fn padding_left_top(&self) -> (f32, f32) {
        let h_context = DimensionContext {
            dpi: self.dimensions.dpi as f32,
            pixel_max: self.terminal_size.pixel_width as f32,
            pixel_cell: self.render_metrics.cell_size.width as f32,
        };
        let v_context = DimensionContext {
            dpi: self.dimensions.dpi as f32,
            pixel_max: self.terminal_size.pixel_height as f32,
            pixel_cell: self.render_metrics.cell_size.height as f32,
        };

        let padding_left = self
            .config
            .window_padding
            .left
            .evaluate_as_pixels(h_context);
        let padding_top = self.config.window_padding.top.evaluate_as_pixels(v_context);

        (padding_left, padding_top)
    }

    pub fn paint_split_opengl(
        &mut self,
        split: &PositionedSplit,
        pane: &Rc<dyn Pane>,
    ) -> anyhow::Result<()> {
        let gl_state = self.render_state.as_ref().unwrap();
        let vb = &gl_state.vb[2];
        let mut vb_mut = vb.current_vb_mut();
        let mut quads = vb.map(&mut vb_mut);
        let palette = pane.palette();
        let foreground = rgbcolor_to_window_color(palette.split);
        let cell_width = self.render_metrics.cell_size.width as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;

        let first_row_offset = if self.show_tab_bar && !self.config.tab_bar_at_bottom {
            self.tab_bar_pixel_height()?
        } else {
            0.
        };

        let (padding_left, padding_top) = self.padding_left_top();

        let pos_y = split.top as f32 * cell_height + first_row_offset + padding_top;
        let pos_x = split.left as f32 * cell_width + padding_left;

        if split.direction == SplitDirection::Horizontal {
            self.filled_rectangle(
                &mut quads,
                euclid::rect(
                    pos_x + (cell_width / 2.0),
                    pos_y - (cell_height / 2.0),
                    self.render_metrics.underline_height as f32,
                    (1. + split.size as f32) * cell_height,
                ),
                foreground,
            )?;
            self.ui_items.push(UIItem {
                x: padding_left as usize + (split.left * cell_width as usize),
                width: cell_width as usize,
                y: padding_top as usize
                    + first_row_offset as usize
                    + split.top * cell_height as usize,
                height: split.size * cell_height as usize,
                item_type: UIItemType::Split(split.clone()),
            });
        } else {
            self.filled_rectangle(
                &mut quads,
                euclid::rect(
                    pos_x - (cell_width / 2.0),
                    pos_y + (cell_height / 2.0),
                    (1.0 + split.size as f32) * cell_width,
                    self.render_metrics.underline_height as f32,
                ),
                foreground,
            )?;
            self.ui_items.push(UIItem {
                x: padding_left as usize + (split.left * cell_width as usize),
                width: split.size * cell_width as usize,
                y: padding_top as usize
                    + first_row_offset as usize
                    + split.top * cell_height as usize,
                height: cell_height as usize,
                item_type: UIItemType::Split(split.clone()),
            });
        }

        Ok(())
    }

    pub fn paint_opengl_pass(&mut self) -> anyhow::Result<()> {
        {
            let gl_state = self.render_state.as_ref().unwrap();
            for vb in &gl_state.vb {
                vb.clear_quad_allocation();
            }
        }

        // Clear out UI item positions; we'll rebuild these as we render
        self.ui_items.clear();

        let panes = self.get_panes_to_render();
        let num_panes = panes.len();

        for pos in panes {
            if pos.is_active {
                self.update_text_cursor(&pos.pane);
            }
            self.paint_pane_opengl(&pos, num_panes)?;
        }

        if let Some(pane) = self.get_active_pane_or_overlay() {
            let splits = self.get_splits();
            for split in &splits {
                self.paint_split_opengl(split, &pane)?;
            }
        }

        if self.show_tab_bar {
            self.paint_tab_bar()?;
        }

        Ok(())
    }

    fn cluster_and_shape<'a>(
        &self,
        cell_clusters: &'a [CellCluster],
        params: &'a RenderScreenLineOpenGLParams,
    ) -> anyhow::Result<Vec<ShapedCluster<'a>>> {
        let gl_state = self.render_state.as_ref().unwrap();
        let mut shaped = vec![];
        let mut last_style = None;
        let mut x_pos = 0.;

        for cluster in cell_clusters {
            if !matches!(last_style.as_ref(), Some(ClusterStyleCache{attrs,..}) if *attrs == &cluster.attrs)
            {
                let attrs = &cluster.attrs;
                let style = self.fonts.match_style(params.config, attrs);
                let is_highlited_hyperlink = match (attrs.hyperlink(), &self.current_highlight) {
                    (Some(ref this), &Some(ref highlight)) => **this == *highlight,
                    _ => false,
                };
                // underline and strikethrough
                let underline_tex_rect = gl_state
                    .glyph_cache
                    .borrow_mut()
                    .cached_line_sprite(
                        is_highlited_hyperlink,
                        attrs.strikethrough(),
                        attrs.underline(),
                        attrs.overline(),
                        &params.render_metrics,
                    )?
                    .texture_coords();
                let bg_is_default = attrs.background() == ColorAttribute::Default;
                let bg_color = params.palette.resolve_bg(attrs.background());

                let fg_color = resolve_fg_color_attr(&attrs, attrs.foreground(), &params, style);

                let (fg_color, bg_color, bg_is_default) = {
                    let mut fg = fg_color;
                    let mut bg = bg_color;
                    let mut bg_default = bg_is_default;

                    // Check the line reverse_video flag and flip.
                    if attrs.reverse() == !params.line.is_reverse() {
                        std::mem::swap(&mut fg, &mut bg);
                        bg_default = false;
                    }

                    // Check for blink, and if this is the "not-visible"
                    // part of blinking then set fg = bg.  This is a cheap
                    // means of getting it done without impacting other
                    // features.
                    let blink_rate = match attrs.blink() {
                        Blink::None => None,
                        Blink::Slow => Some((
                            params.config.text_blink_rate,
                            self.last_text_blink_paint.borrow_mut(),
                        )),
                        Blink::Rapid => Some((
                            params.config.text_blink_rate_rapid,
                            self.last_text_blink_paint_rapid.borrow_mut(),
                        )),
                    };
                    if let Some((blink_rate, mut last_time)) = blink_rate {
                        if blink_rate != 0 {
                            let milli_uptime = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_millis();

                            let ticks = milli_uptime / blink_rate as u128;
                            if (ticks & 1) == 0 {
                                fg = bg;
                            }

                            let interval = Duration::from_millis(blink_rate);
                            if last_time.elapsed() >= interval {
                                *last_time = Instant::now();
                            }
                            let due = *last_time + interval;

                            self.update_next_frame_time(Some(due));
                        }
                    }

                    (fg, bg, bg_default)
                };

                let glyph_color = rgbcolor_to_window_color(fg_color);
                let underline_color = match attrs.underline_color() {
                    ColorAttribute::Default => fg_color,
                    c => resolve_fg_color_attr(&attrs, c, &params, style),
                };
                let underline_color = rgbcolor_to_window_color(underline_color);

                let bg_color = rgbcolor_alpha_to_window_color(
                    bg_color,
                    if params.window_is_transparent && bg_is_default {
                        0.0
                    } else {
                        params.config.text_background_opacity
                    },
                );

                last_style.replace(ClusterStyleCache {
                    attrs,
                    style: params.style.unwrap_or(style),
                    underline_tex_rect: underline_tex_rect.clone(),
                    bg_color,
                    fg_color: glyph_color,
                    underline_color,
                });
            }

            let style_params = last_style.as_ref().expect("we just set it up").clone();

            let glyph_info = self.cached_cluster_shape(
                style_params.style,
                &cluster,
                &gl_state,
                params.line,
                params.font.as_ref(),
                &params.render_metrics,
            )?;
            let pixel_width = glyph_info
                .iter()
                .map(|info| info.glyph.x_advance.get() as f32)
                .sum();

            shaped.push(ShapedCluster {
                style: style_params,
                pixel_width,
                cluster,
                glyph_info,
                x_pos,
            });

            x_pos += pixel_width;
        }

        Ok(shaped)
    }

    /// "Render" a line of the terminal screen into the vertex buffer.
    /// This is nominally a matter of setting the fg/bg color and the
    /// texture coordinates for a given glyph.  There's a little bit
    /// of extra complexity to deal with multi-cell glyphs.
    pub fn render_screen_line_opengl(
        &self,
        params: RenderScreenLineOpenGLParams,
        layers: &mut [MappedQuads; 3],
    ) -> anyhow::Result<()> {
        let gl_state = self.render_state.as_ref().unwrap();

        let num_cols = params.dims.cols;

        let hsv = if params.is_active {
            None
        } else {
            Some(params.config.inactive_pane_hsb)
        };

        let cell_width = params.render_metrics.cell_size.width as f32;
        let cell_height = params.render_metrics.cell_size.height as f32;
        let pos_y = (self.dimensions.pixel_height as f32 / -2.) + params.top_pixel_y;

        let start = Instant::now();

        let mut last_cell_idx = 0;

        let local_shaped;
        let cell_clusters;

        let cursor_idx = if params.pane.is_some()
            && params.is_active
            && params.stable_line_idx == Some(params.cursor.y)
        {
            Some(params.cursor.x)
        } else {
            None
        };

        // Referencing the text being composed, but only if it belongs to this pane
        let composing = if cursor_idx.is_some() {
            if let DeadKeyStatus::Composing(composing) = &self.dead_key_status {
                Some(composing)
            } else {
                None
            }
        } else {
            None
        };

        let mut composition_width = 0;

        // Do we need to shape immediately, or can we use the pre-shaped data?
        let to_shape = if let Some(composing) = composing {
            // Create an updated line with the composition overlaid
            let mut line = params.line.clone();
            line.overlay_text_with_attribute(
                params.cursor.x,
                composing,
                CellAttributes::blank(),
                termwiz::surface::SEQ_ZERO,
            );
            cell_clusters = line.cluster(cursor_idx);
            composition_width = unicode_column_width(composing, None);
            Some(&cell_clusters)
        } else if params.pre_shaped.is_none() {
            cell_clusters = params.line.cluster(cursor_idx);
            Some(&cell_clusters)
        } else {
            None
        };

        let shaped = if let Some(cell_clusters) = to_shape {
            local_shaped = self.cluster_and_shape(&cell_clusters, &params)?;
            &local_shaped
        } else {
            params.pre_shaped.unwrap()
        };

        let bounding_rect = euclid::rect(
            params.left_pixel_x,
            params.top_pixel_y,
            params.pixel_width,
            cell_height,
        );

        // Make a pass to compute background colors.
        // Need to consider:
        // * background when it is not the default color
        // * Reverse video attribute
        for item in shaped {
            let cluster = &item.cluster;
            let attrs = &cluster.attrs;
            let cluster_width = cluster.width;

            let bg_is_default = attrs.background() == ColorAttribute::Default;
            let bg_color = params.palette.resolve_bg(attrs.background());

            let fg_color =
                resolve_fg_color_attr(&attrs, attrs.foreground(), &params, &Default::default());

            let (bg_color, bg_is_default) = {
                let mut fg = fg_color;
                let mut bg = bg_color;
                let mut bg_default = bg_is_default;

                // Check the line reverse_video flag and flip.
                if attrs.reverse() == !params.line.is_reverse() {
                    std::mem::swap(&mut fg, &mut bg);
                    bg_default = false;
                }

                (
                    rgbcolor_alpha_to_window_color(bg, self.config.text_background_opacity),
                    bg_default,
                )
            };

            if !bg_is_default {
                let rect = euclid::rect(
                    params.left_pixel_x
                        + if params.use_pixel_positioning {
                            item.x_pos
                        } else {
                            cluster.first_cell_idx as f32 * cell_width
                        },
                    params.top_pixel_y,
                    if params.use_pixel_positioning {
                        item.pixel_width
                    } else {
                        cluster_width as f32 * cell_width
                    },
                    cell_height,
                );
                if let Some(rect) = rect.intersection(&bounding_rect) {
                    let mut quad = self.filled_rectangle(&mut layers[0], rect, bg_color)?;
                    quad.set_hsv(hsv);
                }
            }
        }

        // Render the selection background color
        if !params.selection.is_empty() {
            let mut quad = self.filled_rectangle(
                &mut layers[0],
                euclid::rect(
                    params.left_pixel_x + (params.selection.start as f32 * cell_width),
                    params.top_pixel_y,
                    (params.selection.end - params.selection.start) as f32 * cell_width,
                    cell_height,
                ),
                params.selection_bg,
            )?;

            quad.set_hsv(hsv);
        }

        let mut overlay_images = vec![];

        for item in shaped {
            let style_params = &item.style;
            let cluster = &item.cluster;
            let glyph_info = &item.glyph_info;

            let mut current_idx = cluster.first_cell_idx;
            let mut cluster_x_pos = item.x_pos;

            for info in glyph_info.iter() {
                let glyph = &info.glyph;

                if params.use_pixel_positioning
                    && params.left_pixel_x + cluster_x_pos + glyph.x_advance.get() as f32
                        >= params.left_pixel_x + params.pixel_width
                {
                    break;
                }

                let top = cell_height + params.render_metrics.descender.get() as f32
                    - (glyph.y_offset + glyph.bearing_y).get() as f32;

                // We use this to remember the `left` offset value to use for glyph_idx > 0
                let mut slice_left = 0.;

                // Iterate each cell that comprises this glyph.  There is usually
                // a single cell per glyph but combining characters, ligatures
                // and emoji can be 2 or more cells wide.
                for glyph_idx in 0..info.pos.num_cells as usize {
                    let cell_idx = current_idx + glyph_idx;

                    if cell_idx >= num_cols {
                        // terminal line data is wider than the window.
                        // This happens for example while live resizing the window
                        // smaller than the terminal.
                        break;
                    }

                    last_cell_idx = current_idx;

                    let in_composition = composition_width > 0
                        && cell_idx >= params.cursor.x
                        && cell_idx <= params.cursor.x + composition_width;

                    let ComputeCellFgBgResult {
                        fg_color: glyph_color,
                        bg_color,
                        cursor_shape,
                        cursor_border_color,
                    } = self.compute_cell_fg_bg(ComputeCellFgBgParams {
                        stable_line_idx: params.stable_line_idx,
                        // We pass the current_idx instead of the cell_idx when
                        // computing the cursor/background color because we may
                        // have a series of ligatured glyphs that compose over the
                        // top of each other to form a double-wide grapheme cell.
                        // If we use cell_idx here we could render half of that
                        // in the cursor colors (good) and the other half in
                        // the text colors, which is bad because we get a half
                        // reversed, half not glyph and that is hard to read
                        // against the cursor background.
                        // When we cluster, we guarantee that the ligatures are
                        // broken around the cursor boundary, and clustering
                        // guarantees that different colors are broken out as
                        // well, so this assumption is probably good in all
                        // cases!
                        // <https://github.com/wez/wezterm/issues/478>
                        cell_idx: current_idx,
                        cursor: params.cursor,
                        selection: &params.selection,
                        fg_color: style_params.fg_color,
                        bg_color: style_params.bg_color,
                        palette: params.palette,
                        is_active_pane: params.is_active,
                        config: params.config,
                        selection_fg: params.selection_fg,
                        selection_bg: params.selection_bg,
                        cursor_fg: params.cursor_fg,
                        cursor_bg: params.cursor_bg,
                        cursor_border_color: params.cursor_border_color,
                        pane: params.pane,
                        in_composition,
                    });

                    let pos_x = (self.dimensions.pixel_width as f32 / -2.)
                        + params.left_pixel_x
                        + if params.use_pixel_positioning {
                            cluster_x_pos + (glyph.x_offset + glyph.bearing_x).get() as f32
                        } else {
                            cell_idx as f32 * cell_width
                        };

                    if pos_x > params.left_pixel_x + params.pixel_width {
                        log::info!(
                            "breaking on overflow {} > {} + {}",
                            pos_x,
                            params.left_pixel_x,
                            params.pixel_width
                        );
                        break;
                    }

                    let pixel_width = glyph.x_advance.get() as f32;

                    // Note: in use_pixel_positioning mode, we draw backgrounds
                    // for glyph_idx == 0 based on the whole glyph advance, rather than
                    // for each of the cells.

                    if cursor_shape.is_some() {
                        if glyph_idx == 0 {
                            // We'd like to render the cursor with the cell width
                            // so that double-wide cells look more reasonable.
                            // If we have a cursor shape, compute the intended cursor
                            // width.  We only use that if we're the first cell that
                            // comprises this glyph; if for some reason the cursor position
                            // is in the middle of a glyph we just use a single cell.
                            let cursor_width =
                                cursor_shape.map(|_| info.pos.num_cells).unwrap_or(1);

                            let mut quad = layers[0].allocate()?;
                            quad.set_position(
                                pos_x,
                                pos_y,
                                pos_x
                                    + if params.use_pixel_positioning {
                                        pixel_width
                                    } else {
                                        cursor_width as f32 * cell_width
                                    },
                                pos_y + cell_height,
                            );
                            quad.set_hsv(hsv);
                            quad.set_has_color(false);

                            quad.set_texture(
                                gl_state
                                    .glyph_cache
                                    .borrow_mut()
                                    .cursor_sprite(
                                        cursor_shape,
                                        &params.render_metrics,
                                        cursor_width,
                                    )?
                                    .texture_coords(),
                            );

                            quad.set_fg_color(cursor_border_color);
                        }
                    }

                    let images = cluster.attrs.images().unwrap_or_else(|| vec![]);

                    for img in &images {
                        if img.z_index() < 0 {
                            self.populate_image_quad(
                                &img,
                                gl_state,
                                &mut layers[0],
                                cell_idx,
                                &params,
                                hsv,
                                glyph_color,
                            )?;
                        }
                    }

                    // Underlines
                    if style_params.underline_tex_rect != params.white_space {
                        if !params.use_pixel_positioning || glyph_idx == 0 {
                            let mut quad = layers[0].allocate()?;
                            quad.set_position(
                                pos_x,
                                pos_y,
                                pos_x
                                    + if params.use_pixel_positioning {
                                        pixel_width
                                    } else {
                                        cell_width
                                    },
                                pos_y + cell_height,
                            );
                            quad.set_hsv(hsv);
                            quad.set_has_color(false);

                            quad.set_texture(style_params.underline_tex_rect);
                            quad.set_fg_color(style_params.underline_color);
                        }
                    }

                    let mut did_custom = false;

                    if self.config.custom_block_glyphs && glyph_idx == 0 {
                        if let Some(cell) = params.line.cells().get(cell_idx) {
                            if let Some(block) = BlockKey::from_cell(cell) {
                                if glyph_color != bg_color {
                                    self.populate_block_quad(
                                        block,
                                        gl_state,
                                        &mut layers[0],
                                        pos_x,
                                        &params,
                                        hsv,
                                        glyph_color,
                                    )?;
                                }
                                did_custom = true;
                            }
                        }
                    }

                    if !did_custom {
                        if let Some(texture) = glyph.texture.as_ref() {
                            if glyph_color != bg_color || glyph.has_color {
                                if params.use_pixel_positioning {
                                    // When use_pixel_positioning is in effect, we simply
                                    // draw the entire glyph at once.
                                    if glyph_idx == 0 {
                                        let mut quad = layers[1].allocate()?;
                                        let pos_y = params.top_pixel_y
                                            + self.dimensions.pixel_height as f32 / -2.0
                                            + top;
                                        quad.set_position(
                                            pos_x,
                                            pos_y,
                                            pos_x + texture.coords.size.width as f32,
                                            pos_y + texture.coords.size.height as f32,
                                        );
                                        quad.set_fg_color(glyph_color);
                                        quad.set_texture(texture.texture_coords());
                                        quad.set_hsv(if glyph.brightness_adjust != 1.0 {
                                            let hsv =
                                                hsv.unwrap_or_else(|| HsbTransform::default());
                                            Some(HsbTransform {
                                                brightness: hsv.brightness
                                                    * glyph.brightness_adjust,
                                                ..hsv
                                            })
                                        } else {
                                            hsv
                                        });
                                        quad.set_has_color(glyph.has_color);
                                    }
                                } else {
                                    let left = info.pos.x_offset.get() as f32 + info.pos.bearing_x;
                                    let slice = SpriteSlice {
                                        cell_idx: glyph_idx,
                                        num_cells: info.pos.num_cells as usize,
                                        cell_width: params.render_metrics.cell_size.width as usize,
                                        scale: glyph.scale as f32,
                                        left_offset: left,
                                    };

                                    let pixel_rect = slice.pixel_rect(texture);
                                    let texture_rect =
                                        texture.texture.to_texture_coords(pixel_rect);

                                    let left = if glyph_idx == 0 { left } else { slice_left };
                                    let bottom =
                                        (pixel_rect.size.height as f32 * glyph.scale as f32) + top
                                            - params.render_metrics.cell_size.height as f32;
                                    let right = pixel_rect.size.width as f32 + left
                                        - params.render_metrics.cell_size.width as f32;

                                    // Save the `right` position; we'll use it for the `left` adjust for
                                    // the next slice that comprises this glyph.
                                    // This is important because some glyphs (eg: 현재 브랜치) can have
                                    // fractional advance/offset positions that leave one half slightly
                                    // out of alignment with the other if we were to simply force the
                                    // `left` value to be 0 when glyph_idx > 0.
                                    slice_left = right;

                                    let mut quad = layers[1].allocate()?;
                                    quad.set_position(
                                        pos_x + left,
                                        pos_y + top,
                                        pos_x + cell_width + right,
                                        pos_y + cell_height + bottom,
                                    );
                                    quad.set_fg_color(glyph_color);
                                    quad.set_texture(texture_rect);
                                    quad.set_hsv(if glyph.brightness_adjust != 1.0 {
                                        let hsv = hsv.unwrap_or_else(|| HsbTransform::default());
                                        Some(HsbTransform {
                                            brightness: hsv.brightness * glyph.brightness_adjust,
                                            ..hsv
                                        })
                                    } else {
                                        hsv
                                    });
                                    quad.set_has_color(glyph.has_color);
                                }
                            }
                        }
                    }

                    for img in images {
                        if img.z_index() >= 0 {
                            overlay_images.push((cell_idx, img, glyph_color));
                        }
                    }
                }
                current_idx += info.pos.num_cells as usize;
                cluster_x_pos += glyph.x_advance.get() as f32;
            }
        }

        for (cell_idx, img, glyph_color) in overlay_images {
            self.populate_image_quad(
                &img,
                gl_state,
                &mut layers[2],
                cell_idx,
                &params,
                hsv,
                glyph_color,
            )?;
        }

        // If the clusters don't extend to the full physical width of the display,
        // we have a little bit more work to do to ensure that we correctly paint:
        // * Selection
        // * Cursor
        let right_fill_start = Instant::now();
        if last_cell_idx < num_cols {
            if params.line.is_reverse() {
                let mut quad = self.filled_rectangle(
                    &mut layers[0],
                    euclid::rect(
                        params.left_pixel_x + (last_cell_idx as f32 * cell_width),
                        params.top_pixel_y,
                        (num_cols - last_cell_idx) as f32 * cell_width,
                        cell_height,
                    ),
                    params.foreground,
                )?;
                quad.set_hsv(hsv);
            }

            if params.stable_line_idx == Some(params.cursor.y)
                && ((params.cursor.x > last_cell_idx) || shaped.is_empty())
            {
                // Compute the cursor fg/bg
                let ComputeCellFgBgResult {
                    fg_color: _glyph_color,
                    bg_color,
                    cursor_shape,
                    cursor_border_color,
                } = self.compute_cell_fg_bg(ComputeCellFgBgParams {
                    stable_line_idx: params.stable_line_idx,
                    cell_idx: params.cursor.x,
                    cursor: params.cursor,
                    selection: &params.selection,
                    fg_color: params.foreground,
                    bg_color: params.default_bg,
                    palette: params.palette,
                    is_active_pane: params.is_active,
                    config: params.config,
                    selection_fg: params.selection_fg,
                    selection_bg: params.selection_bg,
                    cursor_fg: params.cursor_fg,
                    cursor_bg: params.cursor_bg,
                    cursor_border_color: params.cursor_border_color,
                    pane: params.pane,
                    in_composition: false,
                });

                let pos_x = (self.dimensions.pixel_width as f32 / -2.)
                    + params.left_pixel_x
                    + (params.cursor.x as f32 * cell_width);

                let overflow = pos_x > params.left_pixel_x + params.pixel_width;

                if overflow {
                    log::info!(
                        "breaking on overflow {} > {} + {}",
                        pos_x,
                        params.left_pixel_x,
                        params.pixel_width
                    );
                } else {
                    if bg_color != LinearRgba::TRANSPARENT {
                        // Avoid poking a transparent hole underneath the cursor
                        let mut quad = self.filled_rectangle(
                            &mut layers[2],
                            euclid::rect(
                                params.left_pixel_x + (params.cursor.x as f32 * cell_width),
                                params.top_pixel_y,
                                cell_width,
                                cell_height,
                            ),
                            bg_color,
                        )?;
                        quad.set_hsv(hsv);
                    }
                    {
                        let mut quad = layers[2].allocate()?;
                        quad.set_position(pos_x, pos_y, pos_x + cell_width, pos_y + cell_height);

                        quad.set_has_color(false);
                        quad.set_hsv(hsv);

                        quad.set_texture(
                            gl_state
                                .glyph_cache
                                .borrow_mut()
                                .cursor_sprite(cursor_shape, &params.render_metrics, 1)?
                                .texture_coords(),
                        );
                        quad.set_fg_color(cursor_border_color);
                    }
                }
            }
        }
        metrics::histogram!(
            "render_screen_line_opengl.right_fill",
            right_fill_start.elapsed()
        );
        metrics::histogram!("render_screen_line_opengl", start.elapsed());
        log::trace!(
            "right fill {} -> elapsed {:?}",
            num_cols.saturating_sub(last_cell_idx),
            right_fill_start.elapsed()
        );

        Ok(())
    }

    pub fn populate_block_quad(
        &self,
        block: BlockKey,
        gl_state: &RenderState,
        quads: &mut MappedQuads,
        pos_x: f32,
        params: &RenderScreenLineOpenGLParams,
        hsv: Option<config::HsbTransform>,
        glyph_color: LinearRgba,
    ) -> anyhow::Result<()> {
        let sprite = gl_state
            .glyph_cache
            .borrow_mut()
            .cached_block(block, &params.render_metrics)?
            .texture_coords();

        let mut quad = quads.allocate()?;
        let cell_width = params.render_metrics.cell_size.width as f32;
        let cell_height = params.render_metrics.cell_size.height as f32;
        let pos_y = (self.dimensions.pixel_height as f32 / -2.) + params.top_pixel_y;
        quad.set_position(pos_x, pos_y, pos_x + cell_width, pos_y + cell_height);
        quad.set_hsv(hsv);
        quad.set_fg_color(glyph_color);
        quad.set_texture(sprite);
        quad.set_has_color(false);
        Ok(())
    }

    /// Render iTerm2 style image attributes
    pub fn populate_image_quad(
        &self,
        image: &termwiz::image::ImageCell,
        gl_state: &RenderState,
        quads: &mut MappedQuads,
        cell_idx: usize,
        params: &RenderScreenLineOpenGLParams,
        hsv: Option<config::HsbTransform>,
        glyph_color: LinearRgba,
    ) -> anyhow::Result<()> {
        if !self.allow_images {
            return Ok(());
        }

        let padding = self
            .render_metrics
            .cell_size
            .height
            .max(params.render_metrics.cell_size.width) as usize;
        let padding = if padding.is_power_of_two() {
            padding
        } else {
            padding.next_power_of_two()
        };

        let (sprite, next_due) = gl_state
            .glyph_cache
            .borrow_mut()
            .cached_image(image.image_data(), Some(padding))?;
        self.update_next_frame_time(next_due);
        let width = sprite.coords.size.width;
        let height = sprite.coords.size.height;

        let top_left = image.top_left();
        let bottom_right = image.bottom_right();

        // We *could* call sprite.texture.to_texture_coords() here,
        // but since that takes integer pixel coordinates, we'd
        // lose precision and end up with visual artifacts.
        // Instead, we compute the texture coords here in floating point.

        let texture_width = sprite.texture.width() as f32;
        let texture_height = sprite.texture.height() as f32;
        let origin = TextureCoord::new(
            (sprite.coords.origin.x as f32 + (*top_left.x * width as f32)) / texture_width,
            (sprite.coords.origin.y as f32 + (*top_left.y * height as f32)) / texture_height,
        );

        let size = TextureSize::new(
            (*bottom_right.x - *top_left.x) * width as f32 / texture_width,
            (*bottom_right.y - *top_left.y) * height as f32 / texture_height,
        );

        let texture_rect = TextureRect::new(origin, size);

        let mut quad = quads.allocate()?;
        let cell_width = params.render_metrics.cell_size.width as f32;
        let cell_height = params.render_metrics.cell_size.height as f32;
        let pos_y = (self.dimensions.pixel_height as f32 / -2.) + params.top_pixel_y;

        let pos_x = (self.dimensions.pixel_width as f32 / -2.)
            + params.left_pixel_x
            + (cell_idx as f32 * cell_width);

        let (padding_left, padding_top, padding_right, padding_bottom) = image.padding();

        quad.set_position(
            pos_x + padding_left as f32,
            pos_y + padding_top as f32,
            pos_x + cell_width + padding_left as f32 - padding_right as f32,
            pos_y + cell_height + padding_top as f32 - padding_bottom as f32,
        );
        quad.set_hsv(hsv);
        quad.set_fg_color(glyph_color);
        quad.set_texture(texture_rect);
        quad.set_has_color(true);

        Ok(())
    }

    pub fn compute_cell_fg_bg(&self, params: ComputeCellFgBgParams) -> ComputeCellFgBgResult {
        let selected = params.selection.contains(&params.cell_idx);
        let is_cursor = params.in_composition
            || params.pane.is_some()
                && params.stable_line_idx == Some(params.cursor.y)
                && params.cursor.x == params.cell_idx;

        if is_cursor {
            if let Some(intensity) = self.get_intensity_if_bell_target_ringing(
                params.pane.expect("is_cursor only true is pane present"),
                params.config,
                VisualBellTarget::CursorColor,
            ) {
                let (fg_color, bg_color) = if self.config.force_reverse_video_cursor {
                    (params.bg_color, params.fg_color)
                } else {
                    (params.cursor_fg, params.cursor_bg)
                };

                // interpolate between the background color
                // and the the target color
                let (r1, g1, b1, a) = bg_color.tuple();
                let (r, g, b, _) = params
                    .config
                    .resolved_palette
                    .visual_bell
                    .map(|c| c.to_linear_tuple_rgba())
                    .unwrap_or_else(|| fg_color.tuple());

                let bg_color = LinearRgba::with_components(
                    r1 + (r - r1) * intensity,
                    g1 + (g - g1) * intensity,
                    b1 + (b - b1) * intensity,
                    a,
                );

                return ComputeCellFgBgResult {
                    fg_color,
                    bg_color,
                    cursor_shape: Some(CursorShape::Default),
                    cursor_border_color: bg_color,
                };
            }

            let dead_key_or_leader =
                self.dead_key_status != DeadKeyStatus::None || self.leader_is_active();

            if dead_key_or_leader {
                let (fg_color, bg_color) = if self.config.force_reverse_video_cursor {
                    (params.bg_color, params.fg_color)
                } else {
                    (params.cursor_fg, params.cursor_bg)
                };

                let color = params
                    .config
                    .resolved_palette
                    .compose_cursor
                    .map(rgbcolor_to_window_color)
                    .unwrap_or(bg_color);

                return ComputeCellFgBgResult {
                    fg_color,
                    bg_color,
                    cursor_shape: Some(CursorShape::Default),
                    cursor_border_color: color,
                };
            }
        }

        let (cursor_shape, visibility) =
            if is_cursor && params.cursor.visibility == CursorVisibility::Visible {
                // This logic figures out whether the cursor is visible or not.
                // If the cursor is explicitly hidden then it is obviously not
                // visible.
                // If the cursor is set to a blinking mode then we are visible
                // depending on the current time.
                let shape = params
                    .config
                    .default_cursor_style
                    .effective_shape(params.cursor.shape);
                // Work out the blinking shape if its a blinking cursor and it hasn't been disabled
                // and the window is focused.
                let blinking = params.is_active_pane
                    && shape.is_blinking()
                    && params.config.cursor_blink_rate != 0
                    && self.focused.is_some();
                if blinking {
                    let now = std::time::Instant::now();

                    // schedule an invalidation so that we can paint the next
                    // cycle at the right time.
                    if let Some(window) = self.window.clone() {
                        let interval = Duration::from_millis(params.config.cursor_blink_rate);
                        let next = *self.next_blink_paint.borrow();
                        if next < now {
                            let target = next + interval;
                            let target = if target <= now {
                                now + interval
                            } else {
                                target
                            };

                            *self.next_blink_paint.borrow_mut() = target;
                            promise::spawn::spawn(async move {
                                Timer::at(target).await;
                                window.invalidate();
                            })
                            .detach();
                        }
                    }

                    // Divide the time since we last moved by the blink rate.
                    // If the result is even then the cursor is "on", else it
                    // is "off"

                    let milli_uptime = now
                        .duration_since(self.prev_cursor.last_cursor_movement())
                        .as_millis();
                    let ticks = milli_uptime / params.config.cursor_blink_rate as u128;
                    (
                        shape,
                        if (ticks & 1) == 0 {
                            CursorVisibility::Visible
                        } else {
                            CursorVisibility::Hidden
                        },
                    )
                } else {
                    (shape, CursorVisibility::Visible)
                }
            } else {
                (params.cursor.shape, CursorVisibility::Hidden)
            };

        let focused_and_active = self.focused.is_some() && params.is_active_pane;

        let (fg_color, bg_color, cursor_bg) =
            match (selected, focused_and_active, cursor_shape, visibility) {
                // Selected text overrides colors
                (true, _, _, CursorVisibility::Hidden) => {
                    (params.selection_fg, params.selection_bg, params.cursor_bg)
                }
                // block Cursor cell overrides colors
                (
                    _,
                    true,
                    CursorShape::BlinkingBlock | CursorShape::SteadyBlock,
                    CursorVisibility::Visible,
                ) => {
                    if self.config.force_reverse_video_cursor {
                        (params.bg_color, params.fg_color, params.fg_color)
                    } else {
                        (params.cursor_fg, params.cursor_bg, params.cursor_bg)
                    }
                }
                (
                    _,
                    true,
                    CursorShape::BlinkingUnderline
                    | CursorShape::SteadyUnderline
                    | CursorShape::BlinkingBar
                    | CursorShape::SteadyBar,
                    CursorVisibility::Visible,
                ) => {
                    if self.config.force_reverse_video_cursor {
                        (params.fg_color, params.bg_color, params.fg_color)
                    } else {
                        (params.fg_color, params.bg_color, params.cursor_bg)
                    }
                }
                // Normally, render the cell as configured (or if the window is unfocused)
                _ => (params.fg_color, params.bg_color, params.cursor_border_color),
            };

        ComputeCellFgBgResult {
            fg_color,
            bg_color,
            cursor_border_color: cursor_bg,
            cursor_shape: if visibility == CursorVisibility::Visible {
                match cursor_shape {
                    CursorShape::BlinkingBlock | CursorShape::SteadyBlock if focused_and_active => {
                        Some(CursorShape::Default)
                    }
                    // When not focused, convert bar to block to make it more visually
                    // distinct from the focused bar in another pane
                    _shape if !focused_and_active => Some(CursorShape::SteadyBlock),
                    shape => Some(shape),
                }
            } else {
                None
            },
        }
    }

    fn glyph_infos_to_glyphs(
        &self,
        cluster: &CellCluster,
        line: &Line,
        style: &TextStyle,
        glyph_cache: &mut GlyphCache<SrgbTexture2d>,
        infos: &[GlyphInfo],
        font: &Rc<LoadedFont>,
        metrics: &RenderMetrics,
    ) -> anyhow::Result<Vec<Rc<CachedGlyph<SrgbTexture2d>>>> {
        let mut glyphs = Vec::with_capacity(infos.len());
        for info in infos {
            let cell_idx = cluster.byte_to_cell_idx(info.cluster as usize);
            let num_cells = cluster.byte_to_cell_width(info.cluster as usize);

            if self.config.custom_block_glyphs {
                if let Some(cell) = line.cells().get(cell_idx) {
                    if BlockKey::from_cell(cell).is_some() {
                        // Don't bother rendering the glyph from the font, as it can
                        // have incorrect advance metrics.
                        // Instead, just use our pixel-perfect cell metrics
                        glyphs.push(Rc::new(CachedGlyph {
                            brightness_adjust: 1.0,
                            has_color: false,
                            texture: None,
                            x_advance: PixelLength::new(metrics.cell_size.width as f64),
                            x_offset: PixelLength::zero(),
                            y_offset: PixelLength::zero(),
                            bearing_x: PixelLength::zero(),
                            bearing_y: PixelLength::zero(),
                            scale: 1.0,
                        }));
                        continue;
                    }
                }
            }

            let followed_by_space = match line.cells().get(cell_idx + 1) {
                Some(cell) => cell.str() == " ",
                None => false,
            };

            glyphs.push(glyph_cache.cached_glyph(
                info,
                &style,
                followed_by_space,
                font,
                metrics,
                num_cells,
            )?);
        }
        Ok(glyphs)
    }

    /// Shape the printable text from a cluster
    fn cached_cluster_shape(
        &self,
        style: &TextStyle,
        cluster: &CellCluster,
        gl_state: &RenderState,
        line: &Line,
        font: Option<&Rc<LoadedFont>>,
        metrics: &RenderMetrics,
    ) -> anyhow::Result<Rc<Vec<ShapedInfo<SrgbTexture2d>>>> {
        let shape_resolve_start = Instant::now();
        let key = BorrowedShapeCacheKey {
            style,
            text: &cluster.text,
        };
        let glyph_info = match self.lookup_cached_shape(&key) {
            Some(Ok(info)) => info,
            Some(Err(err)) => return Err(err),
            None => {
                let font = match font {
                    Some(f) => Rc::clone(f),
                    None => self.fonts.resolve_font(style)?,
                };
                let window = self.window.as_ref().unwrap().clone();
                match font.shape(
                    &cluster.text,
                    move || window.notify(TermWindowNotif::InvalidateShapeCache),
                    BlockKey::filter_out_synthetic,
                    Some(cluster.presentation),
                ) {
                    Ok(info) => {
                        let glyphs = self.glyph_infos_to_glyphs(
                            cluster,
                            line,
                            &style,
                            &mut gl_state.glyph_cache.borrow_mut(),
                            &info,
                            &font,
                            metrics,
                        )?;
                        let shaped = Rc::new(ShapedInfo::process(metrics, cluster, &info, &glyphs));

                        self.shape_cache
                            .borrow_mut()
                            .put(key.to_owned(), Ok(Rc::clone(&shaped)));
                        shaped
                    }
                    Err(err) => {
                        if err.root_cause().downcast_ref::<ClearShapeCache>().is_some() {
                            return Err(err);
                        }

                        let res = anyhow!("shaper error: {}", err);
                        self.shape_cache.borrow_mut().put(key.to_owned(), Err(err));
                        return Err(res);
                    }
                }
            }
        };
        metrics::histogram!("cached_cluster_shape", shape_resolve_start.elapsed());
        log::trace!(
            "shape_resolve for cluster len {} -> elapsed {:?}",
            cluster.text.len(),
            shape_resolve_start.elapsed()
        );
        Ok(glyph_info)
    }

    fn lookup_cached_shape(
        &self,
        key: &dyn ShapeCacheKeyTrait,
    ) -> Option<anyhow::Result<Rc<Vec<ShapedInfo<SrgbTexture2d>>>>> {
        match self.shape_cache.borrow_mut().get(key) {
            Some(Ok(info)) => Some(Ok(Rc::clone(info))),
            Some(Err(err)) => Some(Err(anyhow!("cached shaper error: {}", err))),
            None => None,
        }
    }

    pub fn recreate_texture_atlas(&mut self, size: Option<usize>) -> anyhow::Result<()> {
        self.shape_cache.borrow_mut().clear();
        if let Some(render_state) = self.render_state.as_mut() {
            render_state.recreate_texture_atlas(&self.fonts, &self.render_metrics, size)?;
        }
        Ok(())
    }
}

pub fn rgbcolor_to_window_color(color: RgbColor) -> LinearRgba {
    rgbcolor_alpha_to_window_color(color, 1.0)
}

pub fn rgbcolor_alpha_to_window_color(color: RgbColor, alpha: f32) -> LinearRgba {
    let (red, green, blue, _) = color.to_linear_tuple_rgba();
    LinearRgba::with_components(red, green, blue, alpha)
}

fn resolve_fg_color_attr(
    attrs: &CellAttributes,
    fg: ColorAttribute,
    params: &RenderScreenLineOpenGLParams,
    style: &config::TextStyle,
) -> RgbColor {
    match fg {
        wezterm_term::color::ColorAttribute::Default => {
            if let Some(fg) = style.foreground {
                fg
            } else {
                params.palette.resolve_fg(attrs.foreground())
            }
        }
        wezterm_term::color::ColorAttribute::PaletteIndex(idx)
            if idx < 8 && params.config.bold_brightens_ansi_colors =>
        {
            // For compatibility purposes, switch to a brighter version
            // of one of the standard ANSI colors when Bold is enabled.
            // This lifts black to dark grey.
            let idx = if attrs.intensity() == wezterm_term::Intensity::Bold {
                idx + 8
            } else {
                idx
            };
            params
                .palette
                .resolve_fg(wezterm_term::color::ColorAttribute::PaletteIndex(idx))
        }
        _ => params.palette.resolve_fg(fg),
    }
}
