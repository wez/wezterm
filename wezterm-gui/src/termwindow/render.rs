use super::box_model::*;
use crate::colorease::ColorEase;
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
use std::time::Instant;
use termwiz::cell::{unicode_column_width, Blink};
use termwiz::cellcluster::CellCluster;
use termwiz::surface::{CursorShape, CursorVisibility};
use wezterm_bidi::Direction;
use wezterm_font::shaper::PresentationWidth;
use wezterm_font::units::{IntPixelLength, PixelLength};
use wezterm_font::{ClearShapeCache, GlyphInfo, LoadedFont};
use wezterm_term::color::{ColorAttribute, ColorPalette, RgbColor};
use wezterm_term::{CellAttributes, Line, StableRowIndex};
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
    pub cursor_is_default_color: bool,

    pub window_is_transparent: bool,
    pub default_bg: LinearRgba,

    /// Override font resolution; useful together with
    /// the resolved title font
    pub font: Option<Rc<LoadedFont>>,
    pub style: Option<&'a TextStyle>,

    /// If true, use the shaper-determined pixel positions,
    /// rather than using monospace cell based positions.
    pub use_pixel_positioning: bool,

    pub render_metrics: RenderMetrics,
}

pub struct ComputeCellFgBgParams<'a> {
    pub selected: bool,
    pub cursor: Option<&'a StableCursorPosition>,
    pub fg_color: LinearRgba,
    pub bg_color: LinearRgba,
    pub palette: &'a ColorPalette,
    pub is_active_pane: bool,
    pub config: &'a ConfigHandle,
    pub selection_fg: LinearRgba,
    pub selection_bg: LinearRgba,
    pub cursor_fg: LinearRgba,
    pub cursor_bg: LinearRgba,
    pub cursor_is_default_color: bool,
    pub cursor_border_color: LinearRgba,
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
                let prior = self.scheduled_animation.borrow_mut().take();
                match prior {
                    Some(prior) if prior <= next_due => {
                        // Already due before that time
                    }
                    _ => {
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
                let mut color_ease = ColorEase::new(
                    config.visual_bell.fade_in_duration_ms,
                    config.visual_bell.fade_in_function,
                    config.visual_bell.fade_out_duration_ms,
                    config.visual_bell.fade_out_function,
                    Some(ringing),
                );

                let intensity = color_ease.intensity_one_shot();

                match intensity {
                    None => {
                        per_pane.bell_start.take();
                    }
                    Some((intensity, next)) => {
                        self.update_next_frame_time(Some(next));
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
            Ok((font.metrics().cell_height.get() as f32 * 1.75).ceil())
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
        let tab_bar_height = self.tab_bar_pixel_height()?;
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
                        border: BorderColor::new(
                            bg_color
                                .unwrap_or_else(|| colors.active_tab.bg_color.into())
                                .to_linear(),
                        ),
                        bg: bg_color
                            .unwrap_or_else(|| colors.active_tab.bg_color.into())
                            .to_linear()
                            .into(),
                        text: fg_color
                            .unwrap_or_else(|| colors.active_tab.fg_color.into())
                            .to_linear()
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
                        let bg = bg_color
                            .unwrap_or_else(|| colors.inactive_tab.bg_color.into())
                            .to_linear();
                        let edge = colors.inactive_tab_edge.to_linear();
                        ElementColors {
                            border: BorderColor {
                                left: bg,
                                right: edge,
                                top: bg,
                                bottom: bg,
                            },
                            bg: bg.into(),
                            text: fg_color
                                .unwrap_or_else(|| colors.inactive_tab.fg_color.into())
                                .to_linear()
                                .into(),
                        }
                    })
                    .hover_colors(Some(ElementColors {
                        border: BorderColor::new(
                            bg_color
                                .unwrap_or_else(|| colors.inactive_tab_hover.bg_color.into())
                                .to_linear(),
                        ),
                        bg: bg_color
                            .unwrap_or_else(|| colors.inactive_tab_hover.bg_color.into())
                            .to_linear()
                            .into(),
                        text: fg_color
                            .unwrap_or_else(|| colors.inactive_tab_hover.fg_color.into())
                            .to_linear()
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

        let border = self.get_os_border();

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
                    border.left.get() as f32,
                    0.,
                    self.dimensions.pixel_width as f32 - (border.left + border.right).get() as f32,
                    tab_bar_height,
                ),
                metrics: &metrics,
                gl_state: self.render_state.as_ref().unwrap(),
            },
            &tabs,
        )?;

        computed.translate(euclid::vec2(
            0.,
            if self.config.tab_bar_at_bottom {
                self.dimensions.pixel_height as f32
                    - (computed.bounds.height() + border.bottom.get() as f32)
            } else {
                border.top.get() as f32
            },
        ));

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

    pub fn get_os_border(&self) -> window::parameters::Border {
        self.os_parameters
            .as_ref()
            .and_then(|p| p.border_dimensions.clone())
            .unwrap_or_default()
    }

    fn paint_tab_bar(&mut self) -> anyhow::Result<()> {
        if self.config.use_fancy_tab_bar {
            if self.fancy_tab_bar.is_none() {
                let palette = self.palette().clone();
                let tab_bar = self.build_fancy_tab_bar(&palette)?;
                self.fancy_tab_bar.replace(tab_bar);
            }

            self.ui_items.append(&mut self.paint_fancy_tab_bar()?);
            return Ok(());
        }

        let border = self.get_os_border();

        let palette = self.palette().clone();
        let tab_bar_height = self.tab_bar_pixel_height()?;
        let tab_bar_y = if self.config.tab_bar_at_bottom {
            ((self.dimensions.pixel_height as f32) - (tab_bar_height + border.bottom.get() as f32))
                .max(0.)
        } else {
            border.top.get() as f32
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
        let default_bg = palette
            .resolve_bg(ColorAttribute::Default)
            .to_linear()
            .mul_alpha(if window_is_transparent {
                0.
            } else {
                self.config.text_background_opacity
            });

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
                foreground: palette.foreground.to_linear(),
                pane: None,
                is_active: true,
                selection_fg: LinearRgba::default(),
                selection_bg: LinearRgba::default(),
                cursor_fg: LinearRgba::default(),
                cursor_bg: LinearRgba::default(),
                cursor_is_default_color: true,
                white_space,
                filled_box,
                window_is_transparent,
                default_bg,
                style: None,
                font: None,
                use_pixel_positioning: self.config.experimental_pixel_positioning,
                render_metrics: self.render_metrics,
            },
            &mut layers,
        )?;

        Ok(())
    }

    fn paint_window_borders(&mut self) -> anyhow::Result<()> {
        if let Some(ref os_params) = self.os_parameters {
            if let Some(ref border_dimensions) = os_params.border_dimensions {
                let gl_state = self.render_state.as_ref().unwrap();
                let vb = &gl_state.vb[1];
                let mut vb_mut = vb.current_vb_mut();
                let mut layer1 = vb.map(&mut vb_mut);

                let height = self.dimensions.pixel_height as f32;
                let width = self.dimensions.pixel_width as f32;

                let border_top = border_dimensions.top.get() as f32;
                if border_top > 0.0 {
                    self.filled_rectangle(
                        &mut layer1,
                        euclid::rect(0.0, 0.0, width, border_top),
                        border_dimensions.color,
                    )?;
                }

                let border_left = border_dimensions.left.get() as f32;
                if border_left > 0.0 {
                    self.filled_rectangle(
                        &mut layer1,
                        euclid::rect(0.0, 0.0, border_left, height),
                        border_dimensions.color,
                    )?;
                }

                let border_bottom = border_dimensions.bottom.get() as f32;
                if border_bottom > 0.0 {
                    self.filled_rectangle(
                        &mut layer1,
                        euclid::rect(0.0, height - border_bottom, width, height),
                        border_dimensions.color,
                    )?;
                }

                let border_right = border_dimensions.right.get() as f32;
                if border_right > 0.0 {
                    self.filled_rectangle(
                        &mut layer1,
                        euclid::rect(width - border_right, 0.0, width, height),
                        border_dimensions.color,
                    )?;
                }
            }
        }

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
        let global_cursor_fg = self.palette().cursor_fg;
        let global_cursor_bg = self.palette().cursor_bg;
        let config = &self.config;
        let palette = pos.pane.palette();

        let (padding_left, padding_top) = self.padding_left_top();

        let tab_bar_height = if self.show_tab_bar {
            self.tab_bar_pixel_height()?
        } else {
            0.
        };
        let (top_bar_height, bottom_bar_height) = if self.config.tab_bar_at_bottom {
            (0.0, tab_bar_height)
        } else {
            (tab_bar_height, 0.0)
        };

        let border = self.get_os_border();
        let top_pixel_y = top_bar_height + padding_top + border.top.get() as f32;

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

        let cursor_border_color = palette.cursor_border.to_linear();
        let foreground = palette.foreground.to_linear();
        let white_space = gl_state.util_sprites.white_space.texture_coords();
        let filled_box = gl_state.util_sprites.filled_box.texture_coords();

        let window_is_transparent =
            self.window_background.is_some() || config.window_background_opacity != 1.0;

        let default_bg = palette
            .resolve_bg(ColorAttribute::Default)
            .to_linear()
            .mul_alpha(if window_is_transparent {
                0.
            } else {
                config.text_background_opacity
            });

        // Render the full window background
        if pos.index == 0 {
            match (self.window_background.as_ref(), self.allow_images) {
                (Some(im), true) => {
                    // Render the window background image
                    let color = palette
                        .background
                        .to_linear()
                        .mul_alpha(config.window_background_opacity);

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
                    let background = if num_panes == 1 {
                        // If we're the only pane, use the pane's palette
                        // to draw the padding background
                        palette.background
                    } else {
                        global_bg_color
                    }
                    .to_linear()
                    .mul_alpha(config.window_background_opacity);
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
                palette
                    .background
                    .to_linear()
                    .mul_alpha(config.window_background_opacity),
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
                let LinearRgba(r, g, b, _) = config
                    .resolved_palette
                    .visual_bell
                    .as_deref()
                    .unwrap_or(&palette.foreground)
                    .to_linear();

                let background = if window_is_transparent {
                    // for transparent windows, we fade in the target color
                    // by adjusting its alpha
                    LinearRgba::with_components(r, g, b, intensity)
                } else {
                    // otherwise We'll interpolate between the background color
                    // and the the target color
                    let (r1, g1, b1, a) = palette
                        .background
                        .to_linear()
                        .mul_alpha(config.window_background_opacity)
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
            let thumb_y_offset = top_bar_height as usize + border.top.get();

            let info = ScrollHit::thumb(
                &*pos.pane,
                current_viewport,
                self.dimensions.pixel_height.saturating_sub(
                    thumb_y_offset + border.bottom.get() + bottom_bar_height as usize,
                ),
                (self.render_metrics.cell_size.height as f32 / 2.0) as usize,
            );
            let abs_thumb_top = thumb_y_offset + info.top;
            let thumb_size = info.height;
            let color = palette.scrollbar_thumb.to_linear();

            // Adjust the scrollbar thumb position
            let config = &self.config;
            let padding = self.effective_right_padding(&config) as f32;

            let thumb_x = self.dimensions.pixel_width - padding as usize - border.right.get();

            // Register the scroll bar location
            self.ui_items.push(UIItem {
                x: thumb_x,
                width: padding as usize,
                y: thumb_y_offset,
                height: info.top,
                item_type: UIItemType::AboveScrollThumb,
            });
            self.ui_items.push(UIItem {
                x: thumb_x,
                width: padding as usize,
                y: abs_thumb_top,
                height: thumb_size,
                item_type: UIItemType::ScrollThumb,
            });
            self.ui_items.push(UIItem {
                x: thumb_x,
                width: padding as usize,
                y: abs_thumb_top + thumb_size,
                height: self
                    .dimensions
                    .pixel_height
                    .saturating_sub(abs_thumb_top + thumb_size),
                item_type: UIItemType::BelowScrollThumb,
            });

            self.filled_rectangle(
                &mut layers[2],
                euclid::rect(
                    thumb_x as f32,
                    abs_thumb_top as f32,
                    padding,
                    thumb_size as f32,
                ),
                color,
            )?;
        }

        let (selrange, rectangular) = {
            let sel = self.selection(pos.pane.pane_id());
            (sel.range.clone(), sel.rectangular)
        };

        let start = Instant::now();
        let selection_fg = palette.selection_fg.to_linear();
        let selection_bg = palette.selection_bg.to_linear();
        let cursor_fg = palette.cursor_fg.to_linear();
        let cursor_bg = palette.cursor_bg.to_linear();
        let cursor_is_default_color =
            palette.cursor_fg == global_cursor_fg && palette.cursor_bg == global_cursor_bg;

        for (line_idx, line) in lines.iter().enumerate() {
            let stable_row = stable_top + line_idx as StableRowIndex;

            let selrange = selrange.map_or(0..0, |sel| sel.cols_for_row(stable_row, rectangular));
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
                    cursor_is_default_color,
                    white_space,
                    filled_box,
                    window_is_transparent,
                    default_bg,
                    font: None,
                    style: None,
                    use_pixel_positioning: self.config.experimental_pixel_positioning,
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
        let foreground = palette.split.to_linear();
        let cell_width = self.render_metrics.cell_size.width as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;

        let first_row_offset = if self.show_tab_bar && !self.config.tab_bar_at_bottom {
            self.tab_bar_pixel_height()?
        } else {
            0.
        } + self.get_os_border().top.get() as f32;

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
        let focused = self.focused.is_some();

        for pos in panes {
            if pos.is_active {
                self.update_text_cursor(&pos.pane);
                if focused {
                    pos.pane.advise_focus();
                    mux::Mux::get()
                        .expect("called on mux thread")
                        .record_focus_for_current_identity(pos.pane.pane_id());
                }
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

        self.paint_window_borders()?;

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
                let bg_color = params.palette.resolve_bg(attrs.background()).to_linear();

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
                        Blink::Slow => {
                            Some((params.config.text_blink_rate, self.blink_state.borrow_mut()))
                        }
                        Blink::Rapid => Some((
                            params.config.text_blink_rate_rapid,
                            self.rapid_blink_state.borrow_mut(),
                        )),
                    };
                    if let Some((blink_rate, mut colorease)) = blink_rate {
                        if blink_rate != 0 {
                            let (intensity, next) = colorease.intensity_continuous();

                            let (r1, g1, b1, a) = bg.tuple();
                            let (r, g, b, _a) = fg.tuple();
                            fg = LinearRgba::with_components(
                                r1 + (r - r1) * intensity,
                                g1 + (g - g1) * intensity,
                                b1 + (b - b1) * intensity,
                                a,
                            );

                            self.update_next_frame_time(Some(next));
                        }
                    }

                    (fg, bg, bg_default)
                };

                let glyph_color = fg_color;
                let underline_color = match attrs.underline_color() {
                    ColorAttribute::Default => fg_color,
                    c => resolve_fg_color_attr(&attrs, c, &params, style),
                };

                let (bg_r, bg_g, bg_b, _) = bg_color.tuple();
                let bg_color = LinearRgba::with_components(
                    bg_r,
                    bg_g,
                    bg_b,
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

        let width_scale = if !params.line.is_single_width() {
            2.0
        } else {
            1.0
        };

        if params.line.is_double_height_bottom() {
            // The top and bottom lines are required to have the same content.
            // For the sake of simplicity, we render both of them as part of
            // rendering the top row, so we have nothing more to do here.
            return Ok(());
        }

        let height_scale = if params.line.is_double_height_top() {
            2.0
        } else {
            1.0
        };

        let cell_width = params.render_metrics.cell_size.width as f32 * width_scale;
        let cell_height = params.render_metrics.cell_size.height as f32 * height_scale;
        let pos_y = (self.dimensions.pixel_height as f32 / -2.) + params.top_pixel_y;
        let gl_x = self.dimensions.pixel_width as f32 / -2.;

        let start = Instant::now();

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

        let (bidi_enabled, bidi_direction) = params.line.bidi_info();
        let bidi_hint = if bidi_enabled {
            Some(bidi_direction)
        } else {
            None
        };
        let direction = bidi_direction.direction();

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
            cell_clusters = line.cluster(bidi_hint);
            composition_width = unicode_column_width(composing, None);
            &cell_clusters
        } else {
            cell_clusters = params.line.cluster(bidi_hint);
            &cell_clusters
        };

        let cursor_range = if composition_width > 0 {
            params.cursor.x..params.cursor.x + composition_width
        } else if params.stable_line_idx == Some(params.cursor.y) {
            params.cursor.x
                ..params.cursor.x
                    + params
                        .line
                        .cells()
                        .get(params.cursor.x)
                        .map(|c| c.width())
                        .unwrap_or(1)
        } else {
            0..0
        };

        let cursor_range_pixels = params.left_pixel_x + cursor_range.start as f32 * cell_width
            ..params.left_pixel_x + cursor_range.end as f32 * cell_width;

        let shaped = self.cluster_and_shape(&to_shape, &params)?;

        let bounding_rect = euclid::rect(
            params.left_pixel_x,
            params.top_pixel_y,
            params.pixel_width,
            cell_height,
        );

        fn phys(x: usize, num_cols: usize, direction: Direction) -> usize {
            match direction {
                Direction::LeftToRight => x,
                Direction::RightToLeft => num_cols - x,
            }
        }

        if params.line.is_reverse() {
            let mut quad = self.filled_rectangle(
                &mut layers[0],
                euclid::rect(
                    params.left_pixel_x,
                    params.top_pixel_y,
                    params.pixel_width,
                    cell_height,
                ),
                params.foreground,
            )?;
            quad.set_hsv(hsv);
        }

        // Make a pass to compute background colors.
        // Need to consider:
        // * background when it is not the default color
        // * Reverse video attribute
        for item in &shaped {
            let cluster = &item.cluster;
            let attrs = &cluster.attrs;
            let cluster_width = cluster.width;

            let bg_is_default = attrs.background() == ColorAttribute::Default;
            let bg_color = params.palette.resolve_bg(attrs.background()).to_linear();

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
                    bg.mul_alpha(self.config.text_background_opacity),
                    bg_default,
                )
            };

            if !bg_is_default {
                let rect = euclid::rect(
                    params.left_pixel_x
                        + if params.use_pixel_positioning {
                            item.x_pos
                        } else {
                            phys(cluster.first_cell_idx, num_cols, direction) as f32 * cell_width
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

            // Underlines
            if item.style.underline_tex_rect != params.white_space {
                // Draw one per cell, otherwise curly underlines
                // stretch across the whole span
                for i in 0..cluster_width {
                    let mut quad = layers[0].allocate()?;
                    let x = gl_x
                        + params.left_pixel_x
                        + if params.use_pixel_positioning {
                            item.x_pos
                        } else {
                            phys(cluster.first_cell_idx + i, num_cols, direction) as f32
                                * cell_width
                        };

                    quad.set_position(x, pos_y, x + cell_width, pos_y + cell_height);
                    quad.set_hsv(hsv);
                    quad.set_has_color(false);
                    quad.set_texture(item.style.underline_tex_rect);
                    quad.set_fg_color(item.style.underline_color);
                }
            }
        }

        // Render the selection background color.
        // This always uses a physical x position, regardles of the line
        // direction.
        let selection_pixel_range = if !params.selection.is_empty() {
            let start = params.left_pixel_x + (params.selection.start as f32 * cell_width);
            let width = (params.selection.end - params.selection.start) as f32 * cell_width;
            let mut quad = self.filled_rectangle(
                &mut layers[0],
                euclid::rect(start, params.top_pixel_y, width, cell_height),
                params.selection_bg,
            )?;

            quad.set_hsv(hsv);

            start..start + width
        } else {
            0.0..0.0
        };

        // Consider cursor
        if !cursor_range.is_empty() {
            let (fg_color, bg_color) = if let Some(c) = params.line.cells().get(cursor_range.start)
            {
                let attrs = c.attrs();
                let bg_color = params.palette.resolve_bg(attrs.background()).to_linear();

                let fg_color =
                    resolve_fg_color_attr(&attrs, attrs.foreground(), &params, &Default::default());

                (fg_color, bg_color)
            } else {
                (params.foreground, params.default_bg)
            };

            let ComputeCellFgBgResult {
                fg_color: _glyph_color,
                bg_color: _bg_color,
                cursor_shape,
                cursor_border_color,
            } = self.compute_cell_fg_bg(ComputeCellFgBgParams {
                cursor: Some(params.cursor),
                selected: false,
                fg_color,
                bg_color,
                palette: params.palette,
                is_active_pane: params.is_active,
                config: params.config,
                selection_fg: params.selection_fg,
                selection_bg: params.selection_bg,
                cursor_fg: params.cursor_fg,
                cursor_bg: params.cursor_bg,
                cursor_is_default_color: params.cursor_is_default_color,
                cursor_border_color: params.cursor_border_color,
                pane: params.pane,
            });
            let pos_x = (self.dimensions.pixel_width as f32 / -2.)
                + params.left_pixel_x
                + (phys(params.cursor.x, num_cols, direction) as f32 * cell_width);

            if cursor_shape.is_some() {
                let mut quad = layers[0].allocate()?;
                quad.set_position(
                    pos_x,
                    pos_y,
                    pos_x + (cursor_range.end - cursor_range.start) as f32 * cell_width,
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
                            (cursor_range.end - cursor_range.start) as u8,
                        )?
                        .texture_coords(),
                );

                quad.set_fg_color(cursor_border_color);
            }
        }

        let mut overlay_images = vec![];

        // Number of cells we've rendered, starting from the edge of the line
        let mut visual_cell_idx = 0;

        let mut cluster_x_pos = match direction {
            Direction::LeftToRight => 0.,
            Direction::RightToLeft => params.pixel_width,
        };

        for item in &shaped {
            let style_params = &item.style;
            let cluster = &item.cluster;
            let glyph_info = &item.glyph_info;
            let images = cluster.attrs.images().unwrap_or_else(|| vec![]);

            // TODO: remember logical/visual mapping for selection
            #[allow(unused_variables)]
            let mut phys_cell_idx = cluster.first_cell_idx;

            // Pre-decrement by the cluster width when doing RTL,
            // so that we can render it right-justified
            if direction == Direction::RightToLeft {
                cluster_x_pos -= if params.use_pixel_positioning {
                    item.pixel_width
                } else {
                    cluster.width as f32 * cell_width
                };
            }

            for info in glyph_info.iter() {
                let glyph = &info.glyph;

                if params.use_pixel_positioning
                    && params.left_pixel_x + cluster_x_pos + glyph.x_advance.get() as f32
                        >= params.left_pixel_x + params.pixel_width
                {
                    break;
                }

                for glyph_idx in 0..info.pos.num_cells as usize {
                    for img in &images {
                        if img.z_index() < 0 {
                            self.populate_image_quad(
                                &img,
                                gl_state,
                                &mut layers[0],
                                visual_cell_idx + glyph_idx,
                                &params,
                                hsv,
                                style_params.fg_color,
                            )?;
                        }
                    }
                }

                {
                    // First, resolve this glyph to a texture
                    let mut texture = glyph.texture.as_ref().cloned();
                    let mut top = cell_height
                        + (params.render_metrics.descender.get() as f32
                            - (glyph.y_offset + glyph.bearing_y).get() as f32)
                            * height_scale;

                    if self.config.custom_block_glyphs {
                        if let Some(cell) = params.line.cells().get(visual_cell_idx) {
                            if let Some(block) = BlockKey::from_cell(cell) {
                                texture.replace(
                                    gl_state
                                        .glyph_cache
                                        .borrow_mut()
                                        .cached_block(block, &params.render_metrics)?,
                                );
                                // Custom glyphs don't have the same offsets as computed
                                // by the shaper, and are rendered relative to the cell
                                // top left, rather than the baseline.
                                top = 0.;
                            }
                        }
                    }

                    if let Some(texture) = texture {
                        // TODO: clipping, but we can do that based on pixels

                        let pos_x = cluster_x_pos
                            + if params.use_pixel_positioning {
                                (glyph.x_offset + glyph.bearing_x).get() as f32
                            } else {
                                0.
                            };

                        if pos_x > params.pixel_width {
                            log::info!("breaking on overflow {} > {}", pos_x, params.pixel_width);
                            break;
                        }
                        let pos_x = pos_x + params.left_pixel_x;

                        // We need to conceptually slice this texture into
                        // up into strips that consider the cursor and selection
                        // background ranges. For ligatures that span cells, we'll
                        // need to explicitly render each strip independently so that
                        // we can set its foreground color to the appropriate color
                        // for the cursor/selection/regular background upon which
                        // it will be drawn.

                        /// Computes the intersection between r1 and r2.
                        /// It may be empty.
                        fn intersection(r1: &Range<f32>, r2: &Range<f32>) -> Range<f32> {
                            let start = r1.start.max(r2.start);
                            let end = r1.end.min(r2.end);
                            if end > start {
                                start..end
                            } else {
                                // Empty
                                start..start
                            }
                        }

                        /// Assess range `r` relative to `within`. If `r` intersects
                        /// `within` then return the 3 ranges that are subsets of `r`
                        /// which are to the left of `within`, intersecting `within`
                        /// and to the right of `within`.
                        /// If `r` and `within` do not intersect, returns `r` and
                        /// two empty ranges.
                        /// If `r` is itself an empty range, all returned ranges
                        /// will be empty.
                        fn range3(
                            r: &Range<f32>,
                            within: &Range<f32>,
                        ) -> (Range<f32>, Range<f32>, Range<f32>) {
                            if r.is_empty() {
                                return (r.clone(), r.clone(), r.clone());
                            }
                            let i = intersection(r, within);
                            if i.is_empty() {
                                return (r.clone(), i.clone(), i.clone());
                            }

                            let left = if i.start > r.start {
                                r.start..i.start
                            } else {
                                r.start..r.start
                            };

                            let right = if i.end < r.end {
                                i.end..r.end
                            } else {
                                r.end..r.end
                            };

                            (left, i, right)
                        }

                        let adjust = (glyph.x_offset + glyph.bearing_x).get() as f32;
                        let texture_range = pos_x + adjust
                            ..pos_x + adjust + (texture.coords.size.width as f32 * width_scale);

                        // First bucket the ranges according to cursor position
                        let (left, mid, right) = range3(&texture_range, &cursor_range_pixels);
                        // Then sub-divide the non-cursor ranges according to selection
                        let (la, lb, lc) = range3(&left, &selection_pixel_range);
                        let (ra, rb, rc) = range3(&right, &selection_pixel_range);

                        // and render each of these strips
                        for range in [la, lb, lc, mid, ra, rb, rc] {
                            if range.is_empty() {
                                continue;
                            }

                            let is_cursor = cursor_range_pixels.contains(&range.start);
                            let selected =
                                !is_cursor && selection_pixel_range.contains(&range.start);

                            let ComputeCellFgBgResult {
                                fg_color: glyph_color,
                                bg_color,
                                cursor_shape: _,
                                cursor_border_color: _,
                            } = self.compute_cell_fg_bg(ComputeCellFgBgParams {
                                cursor: if is_cursor { Some(params.cursor) } else { None },
                                selected,
                                fg_color: style_params.fg_color,
                                bg_color: style_params.bg_color,
                                palette: params.palette,
                                is_active_pane: params.is_active,
                                config: params.config,
                                selection_fg: params.selection_fg,
                                selection_bg: params.selection_bg,
                                cursor_fg: params.cursor_fg,
                                cursor_bg: params.cursor_bg,
                                cursor_is_default_color: params.cursor_is_default_color,
                                cursor_border_color: params.cursor_border_color,
                                pane: params.pane,
                            });

                            if glyph_color == bg_color {
                                // Essentially invisible: don't render it, as anti-aliasing
                                // can cause a ghostly outline of the invisible glyph to appear.
                                continue;
                            }

                            let pixel_rect = euclid::rect(
                                texture.coords.origin.x + (range.start - (pos_x + adjust)) as isize,
                                texture.coords.origin.y,
                                ((range.end - range.start) / width_scale) as isize,
                                texture.coords.size.height,
                            );

                            let texture_rect = texture.texture.to_texture_coords(pixel_rect);

                            let mut quad = layers[1].allocate()?;
                            quad.set_position(
                                gl_x + range.start,
                                pos_y + top,
                                gl_x + range.end,
                                pos_y + top + texture.coords.size.height as f32 * height_scale,
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

                for glyph_idx in 0..info.pos.num_cells as usize {
                    for img in &images {
                        if img.z_index() >= 0 {
                            overlay_images.push((
                                visual_cell_idx + glyph_idx,
                                img.clone(),
                                style_params.fg_color,
                            ));
                        }
                    }
                }
                phys_cell_idx += info.pos.num_cells as usize;
                visual_cell_idx += info.pos.num_cells as usize;
                cluster_x_pos += if params.use_pixel_positioning {
                    glyph.x_advance.get() as f32 * width_scale
                } else {
                    info.pos.num_cells as f32 * cell_width
                };
            }

            match direction {
                Direction::RightToLeft => {
                    // And decrement it again
                    cluster_x_pos -= if params.use_pixel_positioning {
                        item.pixel_width * width_scale
                    } else {
                        cluster.width as f32 * cell_width
                    };
                }
                Direction::LeftToRight => {}
            }
        }

        for (cell_idx, img, glyph_color) in overlay_images {
            self.populate_image_quad(
                &img,
                gl_state,
                &mut layers[2],
                phys(cell_idx, num_cols, direction),
                &params,
                hsv,
                glyph_color,
            )?;
        }

        metrics::histogram!("render_screen_line_opengl", start.elapsed());

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
        if params.cursor.is_some() {
            if let Some(intensity) = self.get_intensity_if_bell_target_ringing(
                params.pane.expect("cursor only set if pane present"),
                params.config,
                VisualBellTarget::CursorColor,
            ) {
                let (fg_color, bg_color) =
                    if self.config.force_reverse_video_cursor && params.cursor_is_default_color {
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
                    .map(|c| c.to_linear().tuple())
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
                let (fg_color, bg_color) =
                    if self.config.force_reverse_video_cursor && params.cursor_is_default_color {
                        (params.bg_color, params.fg_color)
                    } else {
                        (params.cursor_fg, params.cursor_bg)
                    };

                let color = params
                    .config
                    .resolved_palette
                    .compose_cursor
                    .map(|c| c.to_linear())
                    .unwrap_or(bg_color);

                return ComputeCellFgBgResult {
                    fg_color,
                    bg_color,
                    cursor_shape: Some(CursorShape::Default),
                    cursor_border_color: color,
                };
            }
        }

        let (cursor_shape, visibility) = match params.cursor {
            Some(cursor) => (
                params
                    .config
                    .default_cursor_style
                    .effective_shape(cursor.shape),
                cursor.visibility,
            ),
            _ => (CursorShape::default(), CursorVisibility::Hidden),
        };

        let focused_and_active = self.focused.is_some() && params.is_active_pane;

        let (mut fg_color, bg_color, mut cursor_bg) = match (
            params.selected,
            focused_and_active,
            cursor_shape,
            visibility,
        ) {
            // Selected text overrides colors
            (true, _, _, CursorVisibility::Hidden) => (
                params.selection_fg.when_fully_transparent(params.fg_color),
                params.selection_bg,
                params.cursor_bg,
            ),
            // block Cursor cell overrides colors
            (
                _,
                true,
                CursorShape::BlinkingBlock | CursorShape::SteadyBlock,
                CursorVisibility::Visible,
            ) => {
                if self.config.force_reverse_video_cursor && params.cursor_is_default_color {
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
                if self.config.force_reverse_video_cursor && params.cursor_is_default_color {
                    (params.fg_color, params.bg_color, params.fg_color)
                } else {
                    (params.fg_color, params.bg_color, params.cursor_bg)
                }
            }
            // Normally, render the cell as configured (or if the window is unfocused)
            _ => (params.fg_color, params.bg_color, params.cursor_border_color),
        };

        let blinking = params.cursor.is_some()
            && params.is_active_pane
            && cursor_shape.is_blinking()
            && params.config.cursor_blink_rate != 0
            && self.focused.is_some();

        if blinking {
            let mut color_ease = self.cursor_blink_state.borrow_mut();
            color_ease.update_start(self.prev_cursor.last_cursor_movement());
            let (intensity, next) = color_ease.intensity_continuous();

            // Invert the intensity: we want to start with a visible
            // cursor whenever the cursor moves, then fade out, then back.
            let bg_intensity = 1.0 - intensity;

            let (r1, g1, b1, a) = params.bg_color.tuple();
            let (r, g, b, _a) = cursor_bg.tuple();
            cursor_bg = LinearRgba::with_components(
                r1 + (r - r1) * bg_intensity,
                g1 + (g - g1) * bg_intensity,
                b1 + (b - b1) * bg_intensity,
                a,
            );

            if matches!(
                cursor_shape,
                CursorShape::BlinkingBlock | CursorShape::SteadyBlock,
            ) {
                let (r1, g1, b1, a) = fg_color.tuple();
                let (r, g, b, _a) = params.fg_color.tuple();
                fg_color = LinearRgba::with_components(
                    r1 + (r - r1) * intensity,
                    g1 + (g - g1) * intensity,
                    b1 + (b - b1) * intensity,
                    a,
                );
            }

            self.update_next_frame_time(Some(next));
        }

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
                info.num_cells,
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

                let presentation_width = PresentationWidth::with_cluster(&cluster);

                match font.shape(
                    &cluster.text,
                    move || window.notify(TermWindowNotif::InvalidateShapeCache),
                    BlockKey::filter_out_synthetic,
                    Some(cluster.presentation),
                    cluster.direction,
                    None, // FIXME: need more paragraph context
                    Some(&presentation_width),
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
                        let shaped = Rc::new(ShapedInfo::process(&info, &glyphs));

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
    let (red, green, blue, _) = color.to_linear_tuple_rgba().tuple();
    LinearRgba::with_components(red, green, blue, alpha)
}

fn resolve_fg_color_attr(
    attrs: &CellAttributes,
    fg: ColorAttribute,
    params: &RenderScreenLineOpenGLParams,
    style: &config::TextStyle,
) -> LinearRgba {
    match fg {
        wezterm_term::color::ColorAttribute::Default => {
            if let Some(fg) = style.foreground {
                fg.into()
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
    .to_linear()
}
