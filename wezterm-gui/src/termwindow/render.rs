use super::box_model::*;
use crate::colorease::{ColorEase, ColorEaseUniform};
use crate::customglyph::{BlockKey, *};
use crate::glyphcache::{CachedGlyph, GlyphCache};
use crate::quad::{
    HeapQuadAllocator, QuadAllocator, QuadImpl, QuadTrait, TripleLayerQuadAllocator,
    TripleLayerQuadAllocatorTrait,
};
use crate::renderstate::BorrowedLayers;
use crate::selection::SelectionRange;
use crate::shapecache::*;
use crate::tabbar::{TabBarItem, TabEntry};
use crate::termwindow::{
    BorrowedShapeCacheKey, RenderState, ScrollHit, ShapedInfo, TermWindowNotif, UIItem, UIItemType,
};
use crate::uniforms::UniformBuilder;
use crate::utilsprites::RenderMetrics;
use ::window::bitmaps::atlas::OutOfTextureSpace;
use ::window::bitmaps::{TextureCoord, TextureRect, TextureSize};
use ::window::glium::uniforms::{
    MagnifySamplerFilter, MinifySamplerFilter, Sampler, SamplerWrapFunction,
};
use ::window::glium::{BlendingFunction, LinearBlendingFactor, Surface};
use ::window::{glium, DeadKeyStatus, PointF, RectF, SizeF, ULength, WindowOps};
use anyhow::anyhow;
use config::{
    ConfigHandle, Dimension, DimensionContext, FreeTypeLoadTarget, HsbTransform, TabBarColors,
    TextStyle, VisualBellTarget,
};
use euclid::num::Zero;
use mux::pane::{Pane, PaneId, WithPaneLines};
use mux::renderable::{RenderableDimensions, StableCursorPosition};
use mux::tab::{PositionedPane, PositionedSplit, SplitDirection};
use ordered_float::NotNan;
use smol::Timer;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use termwiz::cell::{unicode_column_width, Blink};
use termwiz::cellcluster::CellCluster;
use termwiz::hyperlink::Hyperlink;
use termwiz::surface::{CursorShape, CursorVisibility, SequenceNo};
use wezterm_bidi::Direction;
use wezterm_dynamic::Value;
use wezterm_font::shaper::PresentationWidth;
use wezterm_font::units::{IntPixelLength, PixelLength};
use wezterm_font::{ClearShapeCache, GlyphInfo, LoadedFont};
use wezterm_term::color::{ColorAttribute, ColorPalette, RgbColor};
use wezterm_term::{CellAttributes, Line, StableRowIndex};
use window::color::LinearRgba;

pub const TOP_LEFT_ROUNDED_CORNER: &[Poly] = &[Poly {
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

pub const BOTTOM_LEFT_ROUNDED_CORNER: &[Poly] = &[Poly {
    path: &[
        PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Zero),
        PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
        PolyCommand::QuadTo {
            control: (BlockCoord::Zero, BlockCoord::One),
            to: (BlockCoord::Zero, BlockCoord::Zero),
        },
        PolyCommand::Close,
    ],
    intensity: BlockAlpha::Full,
    style: PolyStyle::Fill,
}];

pub const TOP_RIGHT_ROUNDED_CORNER: &[Poly] = &[Poly {
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

pub const BOTTOM_RIGHT_ROUNDED_CORNER: &[Poly] = &[Poly {
    path: &[
        PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
        PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
        PolyCommand::QuadTo {
            control: (BlockCoord::One, BlockCoord::One),
            to: (BlockCoord::One, BlockCoord::Zero),
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

/// The data that we associate with a line; we use this to cache it shape hash
#[derive(Debug)]
pub struct CachedLineState {
    pub id: u64,
    pub seqno: SequenceNo,
    pub shape_hash: [u8; 16],
}

#[derive(Debug, Hash, Clone, PartialEq, Eq)]
pub struct LineQuadCacheKey {
    pub config_generation: usize,
    pub shape_generation: usize,
    pub quad_generation: usize,
    /// Only set if cursor.y == stable_row
    pub composing: Option<String>,
    pub selection: Range<usize>,
    pub shape_hash: [u8; 16],
    pub top_pixel_y: NotNan<f32>,
    pub left_pixel_x: NotNan<f32>,
    pub phys_line_idx: usize,
    pub pane_id: PaneId,
    pub pane_is_active: bool,
    /// A cursor position with the y value fixed at 0.
    /// Only is_some() if the y value matches this row.
    pub cursor: Option<CursorProperties>,
    pub reverse_video: bool,
    pub password_input: bool,
}

pub struct LineQuadCacheValue {
    /// For resolving hash collisions
    pub line: Line,
    pub expires: Option<Instant>,
    pub layers: HeapQuadAllocator,
    // Only set if the line contains any hyperlinks, so
    // that we can invalidate when it changes
    pub current_highlight: Option<Arc<Hyperlink>>,
    pub invalidate_on_hover_change: bool,
}

pub struct LineToElementParams<'a> {
    pub line: &'a Line,
    pub config: &'a ConfigHandle,
    pub palette: &'a ColorPalette,
    pub stable_line_idx: StableRowIndex,
    pub window_is_transparent: bool,
    pub cursor: &'a StableCursorPosition,
    pub reverse_video: bool,
    pub shape_key: &'a Option<LineToEleShapeCacheKey>,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct LineToEleShapeCacheKey {
    pub shape_hash: [u8; 16],
    pub composing: Option<(usize, String)>,
    pub shape_generation: usize,
}

pub struct LineToElementShapeItem {
    pub expires: Option<Instant>,
    pub shaped: Rc<Vec<LineToElementShape>>,
    // Only set if the line contains any hyperlinks, so
    // that we can invalidate when it changes
    pub current_highlight: Option<Arc<Hyperlink>>,
    pub invalidate_on_hover_change: bool,
}

pub struct LineToElementShape {
    pub attrs: CellAttributes,
    pub style: TextStyle,
    pub underline_tex_rect: TextureRect,
    pub fg_color: LinearRgba,
    pub bg_color: LinearRgba,
    pub underline_color: LinearRgba,
    pub x_pos: f32,
    pub pixel_width: f32,
    pub glyph_info: Rc<Vec<ShapedInfo>>,
    pub cluster: CellCluster,
}

pub struct RenderScreenLineOpenGLResult {
    pub invalidate_on_hover_change: bool,
}

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
    pub shape_key: Option<LineToEleShapeCacheKey>,
    pub password_input: bool,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct CursorProperties {
    pub position: StableCursorPosition,
    pub dead_key_or_leader: bool,
    pub cursor_is_default_color: bool,
    pub cursor_fg: LinearRgba,
    pub cursor_bg: LinearRgba,
    pub cursor_border_color: LinearRgba,
}

pub struct ComputeCellFgBgParams<'a> {
    pub selected: bool,
    pub cursor: Option<&'a StableCursorPosition>,
    pub fg_color: LinearRgba,
    pub bg_color: LinearRgba,
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
    pub fg_color_alt: LinearRgba,
    pub bg_color: LinearRgba,
    pub bg_color_alt: LinearRgba,
    pub fg_color_mix: f32,
    pub bg_color_mix: f32,
    pub cursor_border_color: LinearRgba,
    pub cursor_border_color_alt: LinearRgba,
    pub cursor_border_mix: f32,
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

impl super::TermWindow {
    pub fn paint_impl(&mut self, frame: &mut glium::Frame) {
        self.num_frames += 1;
        // If nothing on screen needs animating, then we can avoid
        // invalidating as frequently
        *self.has_animation.borrow_mut() = None;
        // Start with the assumption that we should allow images to render
        self.allow_images = true;

        let start = Instant::now();

        {
            let diff = start.duration_since(self.last_fps_check_time);
            if diff > Duration::from_secs(1) {
                let seconds = diff.as_secs_f32();
                self.fps = self.num_frames as f32 / seconds;
                self.num_frames = 0;
                self.last_fps_check_time = start;
            }
        }

        frame.clear_color(0., 0., 0., 0.);

        'pass: for pass in 0.. {
            match self.paint_opengl_pass() {
                Ok(_) => match self.render_state.as_mut().unwrap().allocated_more_quads() {
                    Ok(allocated) => {
                        if !allocated {
                            break 'pass;
                        }
                        self.invalidate_fancy_tab_bar();
                        self.invalidate_modal();
                    }
                    Err(err) => {
                        log::error!("{:#}", err);
                        break 'pass;
                    }
                },
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
                        self.invalidate_modal();

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
                        self.invalidate_modal();
                        self.shape_generation += 1;
                        self.shape_cache.borrow_mut().clear();
                        self.line_to_ele_shape_cache.borrow_mut().clear();
                    } else {
                        log::error!("paint_opengl_pass failed: {:#}", err);
                        break 'pass;
                    }
                }
            }
        }
        log::debug!("paint_impl before call_draw elapsed={:?}", start.elapsed());

        self.call_draw(frame).ok();
        self.last_frame_duration = start.elapsed();
        log::debug!(
            "paint_impl elapsed={:?}, fps={}",
            self.last_frame_duration,
            self.fps
        );
        metrics::histogram!("gui.paint.opengl", self.last_frame_duration);
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
        if next_due.is_some() {
            update_next_frame_time(&mut *self.has_animation.borrow_mut(), next_due);
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
        layers: &'a mut TripleLayerQuadAllocator,
        layer_num: usize,
        rect: RectF,
        color: LinearRgba,
    ) -> anyhow::Result<QuadImpl<'a>> {
        let mut quad = layers.allocate(layer_num)?;
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
        layers: &'a mut TripleLayerQuadAllocator,
        layer_num: usize,
        point: PointF,
        polys: &'static [Poly],
        underline_height: IntPixelLength,
        cell_size: SizeF,
        color: LinearRgba,
    ) -> anyhow::Result<QuadImpl<'a>> {
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

        let mut quad = layers.allocate(layer_num)?;

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

        let mut left_status = vec![];
        let mut left_eles = vec![];
        let mut right_eles = vec![];
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

        let item_to_elem = |item: &TabEntry| -> Element {
            let element = Element::with_line(&font, &item.title, palette);

            let bg_color = item
                .title
                .get_cell(0)
                .and_then(|c| match c.attrs().background() {
                    ColorAttribute::Default => None,
                    col => Some(palette.resolve_bg(col)),
                });
            let fg_color = item
                .title
                .get_cell(0)
                .and_then(|c| match c.attrs().foreground() {
                    ColorAttribute::Default => None,
                    col => Some(palette.resolve_fg(col)),
                });

            let new_tab = colors.new_tab();
            let new_tab_hover = colors.new_tab_hover();
            let active_tab = colors.active_tab();

            match item.item {
                TabBarItem::RightStatus | TabBarItem::LeftStatus | TabBarItem::None => element
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
                    .colors(bar_colors.clone()),
                TabBarItem::NewTabButton => Element::new(
                    &font,
                    ElementContent::Poly {
                        line_width: metrics.underline_height.max(2),
                        poly: SizedPoly {
                            poly: PLUS_BUTTON,
                            width: Dimension::Pixels(metrics.cell_size.height as f32 / 2.),
                            height: Dimension::Pixels(metrics.cell_size.height as f32 / 2.),
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
                    bg: new_tab.bg_color.to_linear().into(),
                    text: new_tab.fg_color.to_linear().into(),
                })
                .hover_colors(Some(ElementColors {
                    border: BorderColor::default(),
                    bg: new_tab_hover.bg_color.to_linear().into(),
                    text: new_tab_hover.fg_color.to_linear().into(),
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
                                .unwrap_or_else(|| active_tab.bg_color.into())
                                .to_linear(),
                        ),
                        bg: bg_color
                            .unwrap_or_else(|| active_tab.bg_color.into())
                            .to_linear()
                            .into(),
                        text: fg_color
                            .unwrap_or_else(|| active_tab.fg_color.into())
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
                        let inactive_tab = colors.inactive_tab();
                        let bg = bg_color
                            .unwrap_or_else(|| inactive_tab.bg_color.into())
                            .to_linear();
                        let edge = colors.inactive_tab_edge().to_linear();
                        ElementColors {
                            border: BorderColor {
                                left: bg,
                                right: edge,
                                top: bg,
                                bottom: bg,
                            },
                            bg: bg.into(),
                            text: fg_color
                                .unwrap_or_else(|| inactive_tab.fg_color.into())
                                .to_linear()
                                .into(),
                        }
                    })
                    .hover_colors({
                        let inactive_tab_hover = colors.inactive_tab_hover();
                        Some(ElementColors {
                            border: BorderColor::new(
                                bg_color
                                    .unwrap_or_else(|| inactive_tab_hover.bg_color.into())
                                    .to_linear(),
                            ),
                            bg: bg_color
                                .unwrap_or_else(|| inactive_tab_hover.bg_color.into())
                                .to_linear()
                                .into(),
                            text: fg_color
                                .unwrap_or_else(|| inactive_tab_hover.fg_color.into())
                                .to_linear()
                                .into(),
                        })
                    }),
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
                TabBarItem::LeftStatus => left_status.push(item_to_elem(item)),
                TabBarItem::None | TabBarItem::RightStatus => right_eles.push(item_to_elem(item)),
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
                                        width: Dimension::Pixels(
                                            metrics.cell_size.height as f32 / 2.,
                                        ),
                                        height: Dimension::Pixels(
                                            metrics.cell_size.height as f32 / 2.,
                                        ),
                                    },
                                },
                            )
                            // Ensure that we draw our background over the
                            // top of the rest of the tab contents
                            .zindex(1)
                            .vertical_align(VerticalAlign::Middle)
                            .float(Float::Right)
                            .item_type(UIItemType::CloseTab(tab_idx))
                            .hover_colors({
                                let inactive_tab_hover = colors.inactive_tab_hover();
                                let active_tab = colors.active_tab();

                                Some(ElementColors {
                                    border: BorderColor::default(),
                                    bg: (if active {
                                        inactive_tab_hover.bg_color
                                    } else {
                                        active_tab.bg_color
                                    })
                                    .to_linear()
                                    .into(),
                                    text: (if active {
                                        inactive_tab_hover.fg_color
                                    } else {
                                        active_tab.fg_color
                                    })
                                    .to_linear()
                                    .into(),
                                })
                            })
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

        let mut children = vec![];

        if !left_status.is_empty() {
            children.push(
                Element::new(&font, ElementContent::Children(left_status))
                    .colors(bar_colors.clone()),
            );
        }

        children.push(
            Element::new(&font, ElementContent::Children(left_eles))
                .vertical_align(VerticalAlign::Bottom)
                .colors(bar_colors.clone())
                .padding(BoxDimension {
                    left: Dimension::Cells(0.5),
                    right: Dimension::Cells(0.),
                    top: Dimension::Cells(0.),
                    bottom: Dimension::Cells(0.),
                })
                .zindex(1),
        );
        children.push(
            Element::new(&font, ElementContent::Children(right_eles))
                .colors(bar_colors.clone())
                .float(Float::Right),
        );

        let content = ElementContent::Children(children);

        let tabs = Element::new(&font, content)
            .display(DisplayType::Block)
            .item_type(UIItemType::TabBar(TabBarItem::None))
            .min_width(Some(Dimension::Pixels(self.dimensions.pixel_width as f32)))
            .min_height(Some(Dimension::Pixels(tab_bar_height)))
            .vertical_align(VerticalAlign::Bottom)
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
                zindex: 10,
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

    fn paint_modal(&mut self) -> anyhow::Result<()> {
        if let Some(modal) = self.get_modal() {
            for computed in modal.computed_element(self)?.iter() {
                let mut ui_items = computed.ui_items();

                let gl_state = self.render_state.as_ref().unwrap();
                self.render_element(&computed, gl_state, None)?;

                self.ui_items.append(&mut ui_items);
            }
        }

        Ok(())
    }

    fn paint_fancy_tab_bar(&self) -> anyhow::Result<Vec<UIItem>> {
        let computed = self.fancy_tab_bar.as_ref().ok_or_else(|| {
            anyhow::anyhow!("paint_fancy_tab_bar called but fancy_tab_bar is None")
        })?;
        let ui_items = computed.ui_items();

        let gl_state = self.render_state.as_ref().unwrap();
        self.render_element(&computed, gl_state, None)?;

        Ok(ui_items)
    }

    pub fn get_os_border(&self) -> window::parameters::Border {
        let mut border = self
            .os_parameters
            .as_ref()
            .and_then(|p| p.border_dimensions.clone())
            .unwrap_or_default();

        border.left += ULength::new(
            self.config
                .window_frame
                .border_left_width
                .evaluate_as_pixels(DimensionContext {
                    dpi: self.dimensions.dpi as f32,
                    pixel_max: self.dimensions.pixel_width as f32,
                    pixel_cell: self.render_metrics.cell_size.width as f32,
                })
                .ceil() as usize,
        );
        border.right += ULength::new(
            self.config
                .window_frame
                .border_right_width
                .evaluate_as_pixels(DimensionContext {
                    dpi: self.dimensions.dpi as f32,
                    pixel_max: self.dimensions.pixel_width as f32,
                    pixel_cell: self.render_metrics.cell_size.width as f32,
                })
                .ceil() as usize,
        );
        border.top += ULength::new(
            self.config
                .window_frame
                .border_top_height
                .evaluate_as_pixels(DimensionContext {
                    dpi: self.dimensions.dpi as f32,
                    pixel_max: self.dimensions.pixel_height as f32,
                    pixel_cell: self.render_metrics.cell_size.height as f32,
                })
                .ceil() as usize,
        );
        border.bottom += ULength::new(
            self.config
                .window_frame
                .border_bottom_height
                .evaluate_as_pixels(DimensionContext {
                    dpi: self.dimensions.dpi as f32,
                    pixel_max: self.dimensions.pixel_height as f32,
                    pixel_cell: self.render_metrics.cell_size.height as f32,
                })
                .ceil() as usize,
        );

        border
    }

    fn paint_tab_bar(&mut self, layers: &mut TripleLayerQuadAllocator) -> anyhow::Result<()> {
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
            !self.window_background.is_empty() || self.config.window_background_opacity != 1.0;
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
                    dpi: self.terminal_size.dpi,
                    pixel_height: self.render_metrics.cell_size.height as usize,
                    pixel_width: self.terminal_size.pixel_width,
                    reverse_video: false,
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
                shape_key: None,
                password_input: false,
            },
            layers,
        )?;

        Ok(())
    }

    fn paint_window_borders(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
    ) -> anyhow::Result<()> {
        let border_dimensions = self.get_os_border();

        if border_dimensions.top.get() > 0
            || border_dimensions.bottom.get() > 0
            || border_dimensions.left.get() > 0
            || border_dimensions.right.get() > 0
        {
            let height = self.dimensions.pixel_height as f32;
            let width = self.dimensions.pixel_width as f32;

            let border_top = border_dimensions.top.get() as f32;
            if border_top > 0.0 {
                self.filled_rectangle(
                    layers,
                    1,
                    euclid::rect(0.0, 0.0, width, border_top),
                    self.config
                        .window_frame
                        .border_top_color
                        .map(|c| c.to_linear())
                        .unwrap_or(border_dimensions.color),
                )?;
            }

            let border_left = border_dimensions.left.get() as f32;
            if border_left > 0.0 {
                self.filled_rectangle(
                    layers,
                    1,
                    euclid::rect(0.0, 0.0, border_left, height),
                    self.config
                        .window_frame
                        .border_left_color
                        .map(|c| c.to_linear())
                        .unwrap_or(border_dimensions.color),
                )?;
            }

            let border_bottom = border_dimensions.bottom.get() as f32;
            if border_bottom > 0.0 {
                self.filled_rectangle(
                    layers,
                    1,
                    euclid::rect(0.0, height - border_bottom, width, height),
                    self.config
                        .window_frame
                        .border_bottom_color
                        .map(|c| c.to_linear())
                        .unwrap_or(border_dimensions.color),
                )?;
            }

            let border_right = border_dimensions.right.get() as f32;
            if border_right > 0.0 {
                self.filled_rectangle(
                    layers,
                    1,
                    euclid::rect(width - border_right, 0.0, border_right, height),
                    self.config
                        .window_frame
                        .border_right_color
                        .map(|c| c.to_linear())
                        .unwrap_or(border_dimensions.color),
                )?;
            }
        }

        Ok(())
    }

    pub fn min_scroll_bar_height(&self) -> f32 {
        self.config
            .min_scroll_bar_height
            .evaluate_as_pixels(DimensionContext {
                dpi: self.dimensions.dpi as f32,
                pixel_max: self.terminal_size.pixel_height as f32,
                pixel_cell: self.render_metrics.cell_size.height as f32,
            })
    }

    pub fn build_pane(
        &mut self,
        pos: &PositionedPane,
        num_panes: usize,
    ) -> anyhow::Result<ComputedElement> {
        // First compute the bounds for the pane background

        let cell_width = self.render_metrics.cell_size.width as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;
        let (padding_left, padding_top) = self.padding_left_top();
        let tab_bar_height = if self.show_tab_bar {
            self.tab_bar_pixel_height()?
        } else {
            0.
        };
        let (top_bar_height, _bottom_bar_height) = if self.config.tab_bar_at_bottom {
            (0.0, tab_bar_height)
        } else {
            (tab_bar_height, 0.0)
        };

        let border = self.get_os_border();
        let top_pixel_y = top_bar_height + padding_top + border.top.get() as f32;

        // We want to fill out to the edges of the splits
        let (x, width_delta) = if pos.left == 0 {
            (
                0.,
                padding_left + border.left.get() as f32 + (cell_width / 2.0),
            )
        } else {
            (
                padding_left + border.left.get() as f32 - (cell_width / 2.0)
                    + (pos.left as f32 * cell_width),
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

        let background_rect = euclid::rect(
            x,
            y,
            // Go all the way to the right edge if we're right-most
            if pos.left + pos.width >= self.terminal_size.cols as usize {
                self.dimensions.pixel_width as f32 - x
            } else {
                (pos.width as f32 * cell_width) + width_delta
            },
            // Go all the way to the bottom if we're bottom-most
            if pos.top + pos.height >= self.terminal_size.rows as usize {
                self.dimensions.pixel_height as f32 - y
            } else {
                (pos.height as f32 * cell_height) + height_delta as f32
            },
        );

        // Bounds for the terminal cells
        let content_rect = euclid::rect(
            padding_left + border.left.get() as f32 - (cell_width / 2.0)
                + (pos.left as f32 * cell_width),
            top_pixel_y + (pos.top as f32 * cell_height) - (cell_height / 2.0),
            pos.width as f32 * cell_width,
            pos.height as f32 * cell_height,
        );

        let palette = pos.pane.palette();

        // TODO: visual bell background layer
        // TODO: scrollbar

        Ok(ComputedElement {
            item_type: None,
            zindex: 0,
            bounds: background_rect,
            border: PixelDimension::default(),
            border_rect: background_rect,
            border_corners: None,
            colors: ElementColors {
                border: BorderColor::default(),
                bg: if num_panes > 1 && self.window_background.is_empty() {
                    palette
                        .background
                        .to_linear()
                        .mul_alpha(self.config.window_background_opacity)
                        .into()
                } else {
                    InheritableColor::Inherited
                },
                text: InheritableColor::Inherited,
            },
            hover_colors: None,
            padding: background_rect,
            content_rect,
            baseline: 1.0,
            content: ComputedElementContent::Children(vec![]),
        })
    }

    fn paint_pane_opengl_new(
        &mut self,
        pos: &PositionedPane,
        num_panes: usize,
    ) -> anyhow::Result<()> {
        let computed = self.build_pane(pos, num_panes)?;
        let mut ui_items = computed.ui_items();
        self.ui_items.append(&mut ui_items);
        let gl_state = self.render_state.as_ref().unwrap();
        self.render_element(&computed, gl_state, None)
    }

    pub fn paint_pane_opengl(
        &mut self,
        pos: &PositionedPane,
        num_panes: usize,
        layers: &mut TripleLayerQuadAllocator,
    ) -> anyhow::Result<()> {
        if self.config.use_box_model_render {
            return self.paint_pane_opengl_new(pos, num_panes);
        }

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

        let global_cursor_fg = self.palette().cursor_fg;
        let global_cursor_bg = self.palette().cursor_bg;
        let config = self.config.clone();
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

        let pane_id = pos.pane.pane_id();
        let current_viewport = self.get_viewport(pane_id);
        let dims = pos.pane.get_dimensions();

        let gl_state = self.render_state.as_ref().unwrap();

        let cursor_border_color = palette.cursor_border.to_linear();
        let foreground = palette.foreground.to_linear();
        let white_space = gl_state.util_sprites.white_space.texture_coords();
        let filled_box = gl_state.util_sprites.filled_box.texture_coords();

        let window_is_transparent =
            !self.window_background.is_empty() || config.window_background_opacity != 1.0;

        let default_bg = palette
            .resolve_bg(ColorAttribute::Default)
            .to_linear()
            .mul_alpha(if window_is_transparent {
                0.
            } else {
                config.text_background_opacity
            });

        let cell_width = self.render_metrics.cell_size.width as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;
        let background_rect = {
            // We want to fill out to the edges of the splits
            let (x, width_delta) = if pos.left == 0 {
                (
                    0.,
                    padding_left + border.left.get() as f32 + (cell_width / 2.0),
                )
            } else {
                (
                    padding_left + border.left.get() as f32 - (cell_width / 2.0)
                        + (pos.left as f32 * cell_width),
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
            euclid::rect(
                x,
                y,
                // Go all the way to the right edge if we're right-most
                if pos.left + pos.width >= self.terminal_size.cols as usize {
                    self.dimensions.pixel_width as f32 - x
                } else {
                    (pos.width as f32 * cell_width) + width_delta
                },
                // Go all the way to the bottom if we're bottom-most
                if pos.top + pos.height >= self.terminal_size.rows as usize {
                    self.dimensions.pixel_height as f32 - y
                } else {
                    (pos.height as f32 * cell_height) + height_delta as f32
                },
            )
        };

        if num_panes > 1 && self.window_background.is_empty() {
            // Per-pane, palette-specified background

            let mut quad = self.filled_rectangle(
                layers,
                0,
                background_rect,
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
                &config,
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

                let mut quad = self.filled_rectangle(layers, 0, background_rect, background)?;

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

            let min_height = self.min_scroll_bar_height();

            let info = ScrollHit::thumb(
                &*pos.pane,
                current_viewport,
                self.dimensions.pixel_height.saturating_sub(
                    thumb_y_offset + border.bottom.get() + bottom_bar_height as usize,
                ),
                min_height as usize,
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
                layers,
                2,
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

        {
            let stable_range = match current_viewport {
                Some(top) => top..top + dims.viewport_rows as StableRowIndex,
                None => dims.physical_top..dims.physical_top + dims.viewport_rows as StableRowIndex,
            };

            pos.pane
                .apply_hyperlinks(stable_range.clone(), &self.config.hyperlink_rules);

            struct LineRender<'a, 'b> {
                term_window: &'a mut super::TermWindow,
                selrange: Option<SelectionRange>,
                rectangular: bool,
                dims: RenderableDimensions,
                top_pixel_y: f32,
                left_pixel_x: f32,
                pos: &'a PositionedPane,
                pane_id: PaneId,
                cursor: &'a StableCursorPosition,
                palette: &'a ColorPalette,
                default_bg: LinearRgba,
                cursor_border_color: LinearRgba,
                selection_fg: LinearRgba,
                selection_bg: LinearRgba,
                cursor_fg: LinearRgba,
                cursor_bg: LinearRgba,
                foreground: LinearRgba,
                cursor_is_default_color: bool,
                white_space: TextureRect,
                filled_box: TextureRect,
                window_is_transparent: bool,
                layers: &'a mut TripleLayerQuadAllocator<'b>,
                error: Option<anyhow::Error>,
            }

            let left_pixel_x = padding_left
                + border.left.get() as f32
                + (pos.left as f32 * self.render_metrics.cell_size.width as f32);

            let mut render = LineRender {
                term_window: self,
                selrange,
                rectangular,
                dims,
                top_pixel_y,
                left_pixel_x,
                pos,
                pane_id,
                cursor: &cursor,
                palette: &palette,
                cursor_border_color,
                selection_fg,
                selection_bg,
                cursor_fg,
                default_bg,
                cursor_bg,
                foreground,
                cursor_is_default_color,
                white_space,
                filled_box,
                window_is_transparent,
                layers,
                error: None,
            };

            impl<'a, 'b> LineRender<'a, 'b> {
                fn render_line(
                    &mut self,
                    stable_top: StableRowIndex,
                    line_idx: usize,
                    line: &&mut Line,
                ) -> anyhow::Result<()> {
                    let stable_row = stable_top + line_idx as StableRowIndex;
                    let selrange = self
                        .selrange
                        .map_or(0..0, |sel| sel.cols_for_row(stable_row, self.rectangular));
                    // Constrain to the pane width!
                    let selrange = selrange.start..selrange.end.min(self.dims.cols);

                    let (cursor, composing, password_input) = if self.cursor.y == stable_row {
                        (
                            Some(CursorProperties {
                                position: StableCursorPosition {
                                    y: 0,
                                    ..*self.cursor
                                },
                                dead_key_or_leader: self.term_window.dead_key_status
                                    != DeadKeyStatus::None
                                    || self.term_window.leader_is_active(),
                                cursor_fg: self.cursor_fg,
                                cursor_bg: self.cursor_bg,
                                cursor_border_color: self.cursor_border_color,
                                cursor_is_default_color: self.cursor_is_default_color,
                            }),
                            if let DeadKeyStatus::Composing(composing) =
                                &self.term_window.dead_key_status
                            {
                                Some(composing.to_string())
                            } else {
                                None
                            },
                            if self.term_window.config.detect_password_input {
                                match self.pos.pane.get_metadata() {
                                    Value::Object(obj) => {
                                        match obj.get(&Value::String("password_input".to_string()))
                                        {
                                            Some(Value::Bool(b)) => *b,
                                            _ => false,
                                        }
                                    }
                                    _ => false,
                                }
                            } else {
                                false
                            },
                        )
                    } else {
                        (None, None, false)
                    };

                    let shape_hash = self.term_window.shape_hash_for_line(line);

                    let quad_key = LineQuadCacheKey {
                        pane_id: self.pane_id,
                        password_input,
                        pane_is_active: self.pos.is_active,
                        config_generation: self.term_window.config.generation(),
                        shape_generation: self.term_window.shape_generation,
                        quad_generation: self.term_window.quad_generation,
                        composing: composing.clone(),
                        selection: selrange.clone(),
                        cursor,
                        shape_hash,
                        top_pixel_y: NotNan::new(self.top_pixel_y).unwrap()
                            + (line_idx + self.pos.top) as f32
                                * self.term_window.render_metrics.cell_size.height as f32,
                        left_pixel_x: NotNan::new(self.left_pixel_x).unwrap(),
                        phys_line_idx: line_idx,
                        reverse_video: self.dims.reverse_video,
                    };

                    if let Some(cached_quad) =
                        self.term_window.line_quad_cache.borrow_mut().get(&quad_key)
                    {
                        let expired = cached_quad
                            .expires
                            .map(|i| Instant::now() >= i)
                            .unwrap_or(false);
                        let hover_changed = if cached_quad.invalidate_on_hover_change {
                            !same_hyperlink(
                                cached_quad.current_highlight.as_ref(),
                                self.term_window.current_highlight.as_ref(),
                            )
                        } else {
                            false
                        };
                        if !expired && !hover_changed {
                            cached_quad.layers.apply_to(self.layers)?;
                            self.term_window.update_next_frame_time(cached_quad.expires);
                            return Ok(());
                        }
                    }

                    let mut buf = HeapQuadAllocator::default();
                    let next_due = self.term_window.has_animation.borrow_mut().take();

                    let shape_key = LineToEleShapeCacheKey {
                        shape_hash,
                        shape_generation: quad_key.shape_generation,
                        composing: if self.cursor.y == stable_row {
                            if let DeadKeyStatus::Composing(composing) =
                                &self.term_window.dead_key_status
                            {
                                Some((self.cursor.x, composing.to_string()))
                            } else {
                                None
                            }
                        } else {
                            None
                        },
                    };

                    let render_result = self.term_window.render_screen_line_opengl(
                        RenderScreenLineOpenGLParams {
                            top_pixel_y: *quad_key.top_pixel_y,
                            left_pixel_x: self.left_pixel_x,
                            pixel_width: self.dims.cols as f32
                                * self.term_window.render_metrics.cell_size.width as f32,
                            stable_line_idx: Some(stable_row),
                            line: &line,
                            selection: selrange.clone(),
                            cursor: &self.cursor,
                            palette: &self.palette,
                            dims: &self.dims,
                            config: &self.term_window.config,
                            cursor_border_color: self.cursor_border_color,
                            foreground: self.foreground,
                            is_active: self.pos.is_active,
                            pane: Some(&self.pos.pane),
                            selection_fg: self.selection_fg,
                            selection_bg: self.selection_bg,
                            cursor_fg: self.cursor_fg,
                            cursor_bg: self.cursor_bg,
                            cursor_is_default_color: self.cursor_is_default_color,
                            white_space: self.white_space,
                            filled_box: self.filled_box,
                            window_is_transparent: self.window_is_transparent,
                            default_bg: self.default_bg,
                            font: None,
                            style: None,
                            use_pixel_positioning: self
                                .term_window
                                .config
                                .experimental_pixel_positioning,
                            render_metrics: self.term_window.render_metrics,
                            shape_key: Some(shape_key),
                            password_input,
                        },
                        &mut TripleLayerQuadAllocator::Heap(&mut buf),
                    )?;

                    let expires = self.term_window.has_animation.borrow().as_ref().cloned();
                    self.term_window.update_next_frame_time(next_due);

                    buf.apply_to(self.layers)?;

                    let quad_value = LineQuadCacheValue {
                        layers: buf,
                        expires,
                        line: (*line).clone(),
                        invalidate_on_hover_change: render_result.invalidate_on_hover_change,
                        current_highlight: if render_result.invalidate_on_hover_change {
                            self.term_window.current_highlight.clone()
                        } else {
                            None
                        },
                    };

                    self.term_window
                        .line_quad_cache
                        .borrow_mut()
                        .put(quad_key, quad_value);

                    Ok(())
                }
            }

            impl<'a, 'b> WithPaneLines for LineRender<'a, 'b> {
                fn with_lines_mut(&mut self, stable_top: StableRowIndex, lines: &mut [&mut Line]) {
                    for (line_idx, line) in lines.iter().enumerate() {
                        if let Err(err) = self.render_line(stable_top, line_idx, line) {
                            self.error.replace(err);
                            return;
                        }
                    }
                }
            }

            pos.pane.with_lines_mut(stable_range.clone(), &mut render);
            if let Some(error) = render.error.take() {
                return Err(error);
            }
        }

        /*
        if let Some(zone) = zone {
            // TODO: render a thingy to jump to prior prompt
        }
        */
        metrics::histogram!("paint_pane_opengl.lines", start.elapsed());
        log::trace!("lines elapsed {:?}", start.elapsed());

        Ok(())
    }

    fn call_draw(&mut self, frame: &mut glium::Frame) -> anyhow::Result<()> {
        use crate::glium::texture::SrgbTexture2d;
        let gl_state = self.render_state.as_ref().unwrap();
        let tex = gl_state.glyph_cache.borrow().atlas.texture();
        let tex = tex.downcast_ref::<SrgbTexture2d>().unwrap();

        let projection = euclid::Transform3D::<f32, f32, f32>::ortho(
            -(self.dimensions.pixel_width as f32) / 2.0,
            self.dimensions.pixel_width as f32 / 2.0,
            self.dimensions.pixel_height as f32 / 2.0,
            -(self.dimensions.pixel_height as f32) / 2.0,
            -1.0,
            1.0,
        )
        .to_arrays_transposed();

        let use_subpixel = match self
            .config
            .freetype_render_target
            .unwrap_or(self.config.freetype_load_target)
        {
            FreeTypeLoadTarget::HorizontalLcd | FreeTypeLoadTarget::VerticalLcd => true,
            _ => false,
        };

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

        let milliseconds = self.created.elapsed().as_millis() as u32;

        let cursor_blink: ColorEaseUniform = (*self.cursor_blink_state.borrow()).into();
        let blink: ColorEaseUniform = (*self.blink_state.borrow()).into();
        let rapid_blink: ColorEaseUniform = (*self.rapid_blink_state.borrow()).into();

        for layer in gl_state.layers.borrow().iter() {
            for idx in 0..3 {
                let vb = &layer.vb.borrow()[idx];
                let (vertex_count, index_count) = vb.vertex_index_count();
                if vertex_count > 0 {
                    let vertices = vb.current_vb();
                    let subpixel_aa = use_subpixel && idx == 1;

                    let mut uniforms = UniformBuilder::default();

                    uniforms.add("projection", &projection);
                    uniforms.add("atlas_nearest_sampler", &atlas_nearest_sampler);
                    uniforms.add("atlas_linear_sampler", &atlas_linear_sampler);
                    uniforms.add("foreground_text_hsb", &foreground_text_hsb);
                    uniforms.add("subpixel_aa", &subpixel_aa);
                    uniforms.add("milliseconds", &milliseconds);
                    uniforms.add_struct("cursor_blink", &cursor_blink);
                    uniforms.add_struct("blink", &blink);
                    uniforms.add_struct("rapid_blink", &rapid_blink);

                    frame.draw(
                        vertices.slice(0..vertex_count).unwrap(),
                        vb.indices.slice(0..index_count).unwrap(),
                        &gl_state.glyph_prog,
                        &uniforms,
                        if subpixel_aa {
                            &dual_source_blending
                        } else {
                            &alpha_blending
                        },
                    )?;
                }

                vb.next_index();
            }
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
        layers: &mut TripleLayerQuadAllocator,
        split: &PositionedSplit,
        pane: &Rc<dyn Pane>,
    ) -> anyhow::Result<()> {
        let palette = pane.palette();
        let foreground = palette.split.to_linear();
        let cell_width = self.render_metrics.cell_size.width as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;

        let border = self.get_os_border();
        let first_row_offset = if self.show_tab_bar && !self.config.tab_bar_at_bottom {
            self.tab_bar_pixel_height()?
        } else {
            0.
        } + border.top.get() as f32;

        let (padding_left, padding_top) = self.padding_left_top();

        let pos_y = split.top as f32 * cell_height + first_row_offset + padding_top;
        let pos_x = split.left as f32 * cell_width + padding_left + border.left.get() as f32;

        if split.direction == SplitDirection::Horizontal {
            self.filled_rectangle(
                layers,
                2,
                euclid::rect(
                    pos_x + (cell_width / 2.0),
                    pos_y - (cell_height / 2.0),
                    self.render_metrics.underline_height as f32,
                    (1. + split.size as f32) * cell_height,
                ),
                foreground,
            )?;
            self.ui_items.push(UIItem {
                x: border.left.get() as usize
                    + padding_left as usize
                    + (split.left * cell_width as usize),
                width: cell_width as usize,
                y: padding_top as usize
                    + first_row_offset as usize
                    + split.top * cell_height as usize,
                height: split.size * cell_height as usize,
                item_type: UIItemType::Split(split.clone()),
            });
        } else {
            self.filled_rectangle(
                layers,
                2,
                euclid::rect(
                    pos_x - (cell_width / 2.0),
                    pos_y + (cell_height / 2.0),
                    (1.0 + split.size as f32) * cell_width,
                    self.render_metrics.underline_height as f32,
                ),
                foreground,
            )?;
            self.ui_items.push(UIItem {
                x: border.left.get() as usize
                    + padding_left as usize
                    + (split.left * cell_width as usize),
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
            for layer in gl_state.layers.borrow().iter() {
                layer.clear_quad_allocation();
            }
        }

        // Clear out UI item positions; we'll rebuild these as we render
        self.ui_items.clear();

        let panes = self.get_panes_to_render();
        let num_panes = panes.len();
        let focused = self.focused.is_some();
        let window_is_transparent =
            !self.window_background.is_empty() || self.config.window_background_opacity != 1.0;

        let start = Instant::now();
        let gl_state = self.render_state.as_ref().unwrap();
        let layer = gl_state.layer_for_zindex(0)?;
        let vbs = layer.vb.borrow();
        let vb = [&vbs[0], &vbs[1], &vbs[2]];
        let mut vb_mut0 = vb[0].current_vb_mut();
        let mut vb_mut1 = vb[1].current_vb_mut();
        let mut vb_mut2 = vb[2].current_vb_mut();
        let mut layers = TripleLayerQuadAllocator::Gpu(BorrowedLayers([
            vb[0].map(&mut vb_mut0),
            vb[1].map(&mut vb_mut1),
            vb[2].map(&mut vb_mut2),
        ]));
        log::trace!("quad map elapsed {:?}", start.elapsed());
        metrics::histogram!("quad.map", start.elapsed());

        // Render the full window background
        match (self.window_background.is_empty(), self.allow_images) {
            (false, true) => {
                let bg_color = self.palette().background.to_linear();

                let top = panes
                    .iter()
                    .find(|p| p.is_active)
                    .map(|p| match self.get_viewport(p.pane.pane_id()) {
                        Some(top) => top,
                        None => p.pane.get_dimensions().physical_top,
                    })
                    .unwrap_or(0);

                self.render_backgrounds(bg_color, top)?;
            }
            _ if window_is_transparent && panes.len() > 1 => {
                // Avoid doubling up the background color: the panes
                // will render out through the padding so there
                // should be no gaps that need filling in
            }
            _ => {
                // Regular window background color
                let background = if panes.len() == 1 {
                    // If we're the only pane, use the pane's palette
                    // to draw the padding background
                    panes[0].pane.palette().background
                } else {
                    self.palette().background
                }
                .to_linear()
                .mul_alpha(self.config.window_background_opacity);

                self.filled_rectangle(
                    &mut layers,
                    0,
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

        for pos in panes {
            if pos.is_active {
                self.update_text_cursor(&pos);
                if focused {
                    pos.pane.advise_focus();
                    mux::Mux::get()
                        .expect("called on mux thread")
                        .record_focus_for_current_identity(pos.pane.pane_id());
                }
            }
            self.paint_pane_opengl(&pos, num_panes, &mut layers)?;
        }

        if let Some(pane) = self.get_active_pane_or_overlay() {
            let splits = self.get_splits();
            for split in &splits {
                self.paint_split_opengl(&mut layers, split, &pane)?;
            }
        }

        if self.show_tab_bar {
            self.paint_tab_bar(&mut layers)?;
        }

        self.paint_window_borders(&mut layers)?;
        drop(layers);
        self.paint_modal()?;

        Ok(())
    }

    fn build_line_element_shape(
        &self,
        params: LineToElementParams,
    ) -> anyhow::Result<(Rc<Vec<LineToElementShape>>, bool)> {
        let (bidi_enabled, bidi_direction) = params.line.bidi_info();
        let bidi_hint = if bidi_enabled {
            Some(bidi_direction)
        } else {
            None
        };
        let cell_clusters = if let Some((cursor_x, composing)) =
            params.shape_key.as_ref().and_then(|k| k.composing.as_ref())
        {
            // Create an updated line with the composition overlaid
            let mut line = params.line.clone();
            let seqno = line.current_seqno();
            line.overlay_text_with_attribute(*cursor_x, &composing, CellAttributes::blank(), seqno);
            line.cluster(bidi_hint)
        } else {
            params.line.cluster(bidi_hint)
        };

        let gl_state = self.render_state.as_ref().unwrap();
        let mut shaped = vec![];
        let mut last_style = None;
        let mut x_pos = 0.;
        let mut expires = None;
        let mut invalidate_on_hover_change = false;

        for cluster in &cell_clusters {
            if !matches!(last_style.as_ref(), Some(ClusterStyleCache{attrs,..}) if *attrs == &cluster.attrs)
            {
                let attrs = &cluster.attrs;
                let style = self.fonts.match_style(params.config, attrs);
                let hyperlink = attrs.hyperlink();
                let is_highlited_hyperlink =
                    same_hyperlink(hyperlink, self.current_highlight.as_ref());
                if hyperlink.is_some() {
                    invalidate_on_hover_change = true;
                }
                // underline and strikethrough
                let underline_tex_rect = gl_state
                    .glyph_cache
                    .borrow_mut()
                    .cached_line_sprite(
                        is_highlited_hyperlink,
                        attrs.strikethrough(),
                        attrs.underline(),
                        attrs.overline(),
                        &self.render_metrics,
                    )?
                    .texture_coords();
                let bg_is_default = attrs.background() == ColorAttribute::Default;
                let bg_color = params.palette.resolve_bg(attrs.background()).to_linear();

                let fg_color = resolve_fg_color_attr(
                    &attrs,
                    attrs.foreground(),
                    &params.palette,
                    &params.config,
                    style,
                );
                let (fg_color, bg_color, bg_is_default) = {
                    let mut fg = fg_color;
                    let mut bg = bg_color;
                    let mut bg_default = bg_is_default;

                    // Check the line reverse_video flag and flip.
                    if attrs.reverse() == !params.reverse_video {
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

                            update_next_frame_time(&mut expires, Some(next));
                            self.update_next_frame_time(Some(next));
                        }
                    }

                    (fg, bg, bg_default)
                };

                let glyph_color = fg_color;
                let underline_color = match attrs.underline_color() {
                    ColorAttribute::Default => fg_color,
                    c => resolve_fg_color_attr(&attrs, c, &params.palette, &params.config, style),
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
                    style,
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
                None,
                &self.render_metrics,
            )?;
            let pixel_width = glyph_info
                .iter()
                .map(|info| info.glyph.x_advance.get() as f32)
                .sum();

            shaped.push(LineToElementShape {
                attrs: style_params.attrs.clone(),
                style: style_params.style.clone(),
                underline_tex_rect: style_params.underline_tex_rect,
                bg_color: style_params.bg_color,
                fg_color: style_params.fg_color,
                underline_color: style_params.underline_color,
                pixel_width,
                cluster: cluster.clone(),
                glyph_info,
                x_pos,
            });

            x_pos += pixel_width;
        }

        let shaped = Rc::new(shaped);

        if let Some(shape_key) = params.shape_key {
            self.line_to_ele_shape_cache.borrow_mut().put(
                shape_key.clone(),
                LineToElementShapeItem {
                    expires,
                    shaped: Rc::clone(&shaped),
                    invalidate_on_hover_change,
                    current_highlight: if invalidate_on_hover_change {
                        self.current_highlight.clone()
                    } else {
                        None
                    },
                },
            );
        }

        Ok((shaped, invalidate_on_hover_change))
    }

    /// "Render" a line of the terminal screen into the vertex buffer.
    /// This is nominally a matter of setting the fg/bg color and the
    /// texture coordinates for a given glyph.  There's a little bit
    /// of extra complexity to deal with multi-cell glyphs.
    fn render_screen_line_opengl(
        &self,
        params: RenderScreenLineOpenGLParams,
        layers: &mut TripleLayerQuadAllocator,
    ) -> anyhow::Result<RenderScreenLineOpenGLResult> {
        if params.line.is_double_height_bottom() {
            // The top and bottom lines are required to have the same content.
            // For the sake of simplicity, we render both of them as part of
            // rendering the top row, so we have nothing more to do here.
            return Ok(RenderScreenLineOpenGLResult {
                invalidate_on_hover_change: false,
            });
        }

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

        let (_bidi_enabled, bidi_direction) = params.line.bidi_info();
        let direction = bidi_direction.direction();

        // Do we need to shape immediately, or can we use the pre-shaped data?
        if let Some(composing) = composing {
            composition_width = unicode_column_width(composing, None);
        }

        let cursor_cell = if params.stable_line_idx == Some(params.cursor.y) {
            params.line.get_cell(params.cursor.x)
        } else {
            None
        };

        let cursor_range = if composition_width > 0 {
            params.cursor.x..params.cursor.x + composition_width
        } else if params.stable_line_idx == Some(params.cursor.y) {
            params.cursor.x..params.cursor.x + cursor_cell.as_ref().map(|c| c.width()).unwrap_or(1)
        } else {
            0..0
        };

        let cursor_range_pixels = params.left_pixel_x + cursor_range.start as f32 * cell_width
            ..params.left_pixel_x + cursor_range.end as f32 * cell_width;

        let mut shaped = None;
        let mut invalidate_on_hover_change = false;

        if let Some(shape_key) = &params.shape_key {
            let mut cache = self.line_to_ele_shape_cache.borrow_mut();
            if let Some(entry) = cache.get(shape_key) {
                let expired = entry.expires.map(|i| Instant::now() >= i).unwrap_or(false);
                let hover_changed = if entry.invalidate_on_hover_change {
                    !same_hyperlink(
                        entry.current_highlight.as_ref(),
                        self.current_highlight.as_ref(),
                    )
                } else {
                    false
                };

                if !expired && !hover_changed {
                    self.update_next_frame_time(entry.expires);
                    shaped.replace(Rc::clone(&entry.shaped));
                }

                invalidate_on_hover_change = entry.invalidate_on_hover_change;
            }
        }

        let shaped = if let Some(shaped) = shaped {
            shaped
        } else {
            let params = LineToElementParams {
                config: params.config,
                line: params.line,
                cursor: params.cursor,
                palette: params.palette,
                stable_line_idx: params.stable_line_idx.unwrap_or(0),
                window_is_transparent: params.window_is_transparent,
                reverse_video: params.dims.reverse_video,
                shape_key: &params.shape_key,
            };

            let (shaped, invalidate_on_hover) = self.build_line_element_shape(params)?;
            invalidate_on_hover_change = invalidate_on_hover;
            shaped
        };

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

        if params.dims.reverse_video {
            let mut quad = self.filled_rectangle(
                layers,
                0,
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

        // Assume that we are drawing retro tab bar if there is no
        // stable_line_idx set.
        let is_tab_bar = params.stable_line_idx.is_none();

        // Make a pass to compute background colors.
        // Need to consider:
        // * background when it is not the default color
        // * Reverse video attribute
        for item in shaped.iter() {
            let cluster = &item.cluster;
            let attrs = &cluster.attrs;
            let cluster_width = cluster.width;

            let bg_is_default = attrs.background() == ColorAttribute::Default;
            let bg_color = params.palette.resolve_bg(attrs.background()).to_linear();

            let fg_color = resolve_fg_color_attr(
                &attrs,
                attrs.foreground(),
                &params.palette,
                &params.config,
                &Default::default(),
            );

            let (bg_color, bg_is_default) = {
                let mut fg = fg_color;
                let mut bg = bg_color;
                let mut bg_default = bg_is_default;

                // Check the line reverse_video flag and flip.
                if attrs.reverse() == !params.dims.reverse_video {
                    std::mem::swap(&mut fg, &mut bg);
                    bg_default = false;
                }

                (
                    bg.mul_alpha(self.config.text_background_opacity),
                    bg_default,
                )
            };

            if !bg_is_default {
                let x = params.left_pixel_x
                    + if params.use_pixel_positioning {
                        item.x_pos
                    } else {
                        phys(cluster.first_cell_idx, num_cols, direction) as f32 * cell_width
                    };

                let mut width = if params.use_pixel_positioning {
                    item.pixel_width
                } else {
                    cluster_width as f32 * cell_width
                };

                // If the tab bar is falling just short of the full width of the
                // window, extend it to fit.
                // <https://github.com/wez/wezterm/issues/2210>
                if is_tab_bar && (x + width + cell_width) > params.pixel_width {
                    width += cell_width;
                }

                let rect = euclid::rect(x, params.top_pixel_y, width, cell_height);
                if let Some(rect) = rect.intersection(&bounding_rect) {
                    let mut quad = self.filled_rectangle(layers, 0, rect, bg_color)?;
                    quad.set_hsv(hsv);
                }
            }

            // Underlines
            if item.underline_tex_rect != params.white_space {
                // Draw one per cell, otherwise curly underlines
                // stretch across the whole span
                for i in 0..cluster_width {
                    let mut quad = layers.allocate(0)?;
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
                    quad.set_texture(item.underline_tex_rect);
                    quad.set_fg_color(item.underline_color);
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
                layers,
                0,
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
            let (fg_color, bg_color) = if let Some(c) = &cursor_cell {
                let attrs = c.attrs();

                let bg_color = params.palette.resolve_bg(attrs.background()).to_linear();

                let fg_color = resolve_fg_color_attr(
                    &attrs,
                    attrs.foreground(),
                    &params.palette,
                    &params.config,
                    &Default::default(),
                );

                (fg_color, bg_color)
            } else {
                (params.foreground, params.default_bg)
            };

            let ComputeCellFgBgResult {
                cursor_shape,
                cursor_border_color,
                cursor_border_color_alt,
                cursor_border_mix,
                ..
            } = self.compute_cell_fg_bg(ComputeCellFgBgParams {
                cursor: Some(params.cursor),
                selected: false,
                fg_color,
                bg_color,
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
                let mut quad = layers.allocate(0)?;
                quad.set_hsv(hsv);
                quad.set_has_color(false);

                let mut draw_basic = true;

                if params.password_input {
                    let attrs = cursor_cell
                        .as_ref()
                        .map(|cell| cell.attrs().clone())
                        .unwrap_or_else(|| CellAttributes::blank());

                    let glyph = self.resolve_lock_glyph(
                        &TextStyle::default(),
                        &attrs,
                        params.font.as_ref(),
                        gl_state,
                        &params.render_metrics,
                    )?;

                    if let Some(sprite) = &glyph.texture {
                        let width = sprite.coords.size.width as f32 * glyph.scale as f32;
                        let height =
                            sprite.coords.size.height as f32 * glyph.scale as f32 * height_scale;

                        let pos_y = pos_y
                            + cell_height
                            + (params.render_metrics.descender.get() as f32
                                - (glyph.y_offset + glyph.bearing_y).get() as f32)
                                * height_scale;

                        let pos_x = pos_x + (glyph.x_offset + glyph.bearing_x).get() as f32;
                        quad.set_position(pos_x, pos_y, pos_x + width, pos_y + height);
                        quad.set_texture(sprite.texture_coords());
                        draw_basic = false;
                    }
                }

                if draw_basic {
                    quad.set_position(
                        pos_x,
                        pos_y,
                        pos_x + (cursor_range.end - cursor_range.start) as f32 * cell_width,
                        pos_y + cell_height,
                    );
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
                }

                quad.set_fg_color(cursor_border_color);
                quad.set_alt_color_and_mix_value(cursor_border_color_alt, cursor_border_mix);
            }
        }

        let mut overlay_images = vec![];

        // Number of cells we've rendered, starting from the edge of the line
        let mut visual_cell_idx = 0;

        let mut cluster_x_pos = match direction {
            Direction::LeftToRight => 0.,
            Direction::RightToLeft => params.pixel_width,
        };

        for item in shaped.iter() {
            let cluster = &item.cluster;
            let glyph_info = &item.glyph_info;
            let images = cluster.attrs.images().unwrap_or_else(|| vec![]);
            let valign_adjust = match cluster.attrs.vertical_align() {
                termwiz::cell::VerticalAlign::BaseLine => 0.,
                termwiz::cell::VerticalAlign::SuperScript => {
                    params.render_metrics.cell_size.height as f32 * -0.25
                }
                termwiz::cell::VerticalAlign::SubScript => {
                    params.render_metrics.cell_size.height as f32 * 0.25
                }
            };

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
                                layers,
                                0,
                                visual_cell_idx + glyph_idx,
                                &params,
                                hsv,
                                item.fg_color,
                            )?;
                        }
                    }
                }

                {
                    // First, resolve this glyph to a texture
                    let mut texture = glyph.texture.as_ref().cloned();

                    let mut top = cell_height
                        + (params.render_metrics.descender.get() as f32 + valign_adjust
                            - (glyph.y_offset + glyph.bearing_y).get() as f32)
                            * height_scale;

                    if self.config.custom_block_glyphs {
                        if let Some(block) = &info.block_key {
                            texture.replace(
                                gl_state
                                    .glyph_cache
                                    .borrow_mut()
                                    .cached_block(*block, &params.render_metrics)?,
                            );
                            // Custom glyphs don't have the same offsets as computed
                            // by the shaper, and are rendered relative to the cell
                            // top left, rather than the baseline.
                            top = 0.;
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
                            log::trace!("breaking on overflow {} > {}", pos_x, params.pixel_width);
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
                                fg_color_alt,
                                fg_color_mix,
                                ..
                            } = self.compute_cell_fg_bg(ComputeCellFgBgParams {
                                cursor: if is_cursor { Some(params.cursor) } else { None },
                                selected,
                                fg_color: item.fg_color,
                                bg_color: item.bg_color,
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

                            if glyph_color == bg_color || cluster.attrs.invisible() {
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

                            let mut quad = layers.allocate(1)?;
                            quad.set_position(
                                gl_x + range.start,
                                pos_y + top,
                                gl_x + range.end,
                                pos_y + top + texture.coords.size.height as f32 * height_scale,
                            );
                            quad.set_fg_color(glyph_color);
                            quad.set_alt_color_and_mix_value(fg_color_alt, fg_color_mix);
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
                                item.fg_color,
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
                layers,
                2,
                phys(cell_idx, num_cols, direction),
                &params,
                hsv,
                glyph_color,
            )?;
        }

        metrics::histogram!("render_screen_line_opengl", start.elapsed());

        Ok(RenderScreenLineOpenGLResult {
            invalidate_on_hover_change,
        })
    }

    fn resolve_lock_glyph(
        &self,
        style: &TextStyle,
        attrs: &CellAttributes,
        font: Option<&Rc<LoadedFont>>,
        gl_state: &RenderState,
        metrics: &RenderMetrics,
    ) -> anyhow::Result<Rc<CachedGlyph>> {
        let fa_lock = "\u{f023}";
        let line = Line::from_text(fa_lock, attrs, 0, None);
        let cluster = line.cluster(None);
        let shape_info = self.cached_cluster_shape(style, &cluster[0], gl_state, font, metrics)?;
        Ok(Rc::clone(&shape_info[0].glyph))
    }

    pub fn populate_block_quad(
        &self,
        block: BlockKey,
        gl_state: &RenderState,
        quads: &mut dyn QuadAllocator,
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
        layers: &mut TripleLayerQuadAllocator,
        layer_num: usize,
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

        let mut quad = layers.allocate(layer_num)?;
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
            if let Some(bg_color_mix) = self.get_intensity_if_bell_target_ringing(
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
                let bg_color_alt = params
                    .config
                    .resolved_palette
                    .visual_bell
                    .map(|c| c.to_linear())
                    .unwrap_or(fg_color);

                return ComputeCellFgBgResult {
                    fg_color,
                    fg_color_alt: fg_color,
                    fg_color_mix: 0.,
                    bg_color,
                    bg_color_alt,
                    bg_color_mix,
                    cursor_shape: Some(CursorShape::Default),
                    cursor_border_color: bg_color,
                    cursor_border_color_alt: bg_color_alt,
                    cursor_border_mix: bg_color_mix,
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
                    fg_color_alt: fg_color,
                    fg_color_mix: 0.,
                    bg_color,
                    bg_color_alt: bg_color,
                    bg_color_mix: 0.,
                    cursor_shape: Some(CursorShape::Default),
                    cursor_border_color: color,
                    cursor_border_color_alt: color,
                    cursor_border_mix: 0.,
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

        let (fg_color, bg_color, cursor_bg) = match (
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
                    (
                        params.cursor_fg.when_fully_transparent(params.fg_color),
                        params.cursor_bg,
                        params.cursor_bg,
                    )
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

        let mut fg_color_alt = fg_color;
        let bg_color_alt = bg_color;
        let mut fg_color_mix = 0.;
        let bg_color_mix = 0.;
        let mut cursor_border_color_alt = cursor_bg;
        let mut cursor_border_mix = 0.;

        if blinking {
            let mut color_ease = self.cursor_blink_state.borrow_mut();
            color_ease.update_start(self.prev_cursor.last_cursor_movement());
            let (intensity, next) = color_ease.intensity_continuous();

            cursor_border_mix = intensity;
            cursor_border_color_alt = params.bg_color;

            if matches!(
                cursor_shape,
                CursorShape::BlinkingBlock | CursorShape::SteadyBlock,
            ) {
                fg_color_alt = params.fg_color;
                fg_color_mix = intensity;
            }

            self.update_next_frame_time(Some(next));
        }

        ComputeCellFgBgResult {
            fg_color,
            fg_color_alt,
            bg_color,
            bg_color_alt,
            fg_color_mix,
            bg_color_mix,
            cursor_border_color: cursor_bg,
            cursor_border_color_alt,
            cursor_border_mix,
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
        style: &TextStyle,
        glyph_cache: &mut GlyphCache,
        infos: &[GlyphInfo],
        font: &Rc<LoadedFont>,
        metrics: &RenderMetrics,
    ) -> anyhow::Result<Vec<Rc<CachedGlyph>>> {
        let mut glyphs = Vec::with_capacity(infos.len());
        let mut iter = infos.iter().peekable();
        while let Some(info) = iter.next() {
            if self.config.custom_block_glyphs {
                if info.only_char.and_then(BlockKey::from_char).is_some() {
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

            let followed_by_space = match iter.peek() {
                Some(next_info) => next_info.is_space,
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
        font: Option<&Rc<LoadedFont>>,
        metrics: &RenderMetrics,
    ) -> anyhow::Result<Rc<Vec<ShapedInfo>>> {
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
    ) -> Option<anyhow::Result<Rc<Vec<ShapedInfo>>>> {
        match self.shape_cache.borrow_mut().get(key) {
            Some(Ok(info)) => Some(Ok(Rc::clone(info))),
            Some(Err(err)) => Some(Err(anyhow!("cached shaper error: {}", err))),
            None => None,
        }
    }

    pub fn recreate_texture_atlas(&mut self, size: Option<usize>) -> anyhow::Result<()> {
        self.shape_generation += 1;
        self.shape_cache.borrow_mut().clear();
        self.line_to_ele_shape_cache.borrow_mut().clear();
        if let Some(render_state) = self.render_state.as_mut() {
            render_state.recreate_texture_atlas(&self.fonts, &self.render_metrics, size)?;
        }
        Ok(())
    }

    fn shape_hash_for_line(&mut self, line: &Line) -> [u8; 16] {
        let seqno = line.current_seqno();
        let mut id = None;
        if let Some(cached_arc) = line.get_appdata() {
            if let Some(line_state) = cached_arc.downcast_ref::<CachedLineState>() {
                if line_state.seqno == seqno {
                    // Touch the LRU
                    self.line_state_cache.borrow_mut().get(&line_state.id);
                    return line_state.shape_hash;
                }
                id.replace(line_state.id);
            }
        }

        let id = id.unwrap_or_else(|| {
            let id = self.next_line_state_id;
            self.next_line_state_id += 1;
            id
        });

        let shape_hash = line.compute_shape_hash();

        let state = Arc::new(CachedLineState {
            id,
            seqno,
            shape_hash,
        });

        line.set_appdata(Arc::clone(&state));

        self.line_state_cache.borrow_mut().put(id, state);
        shape_hash
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
    palette: &ColorPalette,
    config: &ConfigHandle,
    style: &config::TextStyle,
) -> LinearRgba {
    match fg {
        wezterm_term::color::ColorAttribute::Default => {
            if let Some(fg) = style.foreground {
                fg.into()
            } else {
                palette.resolve_fg(attrs.foreground())
            }
        }
        wezterm_term::color::ColorAttribute::PaletteIndex(idx)
            if idx < 8 && config.bold_brightens_ansi_colors =>
        {
            // For compatibility purposes, switch to a brighter version
            // of one of the standard ANSI colors when Bold is enabled.
            // This lifts black to dark grey.
            let idx = if attrs.intensity() == wezterm_term::Intensity::Bold {
                idx + 8
            } else {
                idx
            };

            palette.resolve_fg(wezterm_term::color::ColorAttribute::PaletteIndex(idx))
        }
        _ => palette.resolve_fg(fg),
    }
    .to_linear()
}

fn update_next_frame_time(storage: &mut Option<Instant>, next_due: Option<Instant>) {
    if let Some(next_due) = next_due {
        match storage.take() {
            None => {
                storage.replace(next_due);
            }
            Some(t) if next_due < t => {
                storage.replace(next_due);
            }
            Some(t) => {
                storage.replace(t);
            }
        }
    }
}

fn same_hyperlink(a: Option<&Arc<Hyperlink>>, b: Option<&Arc<Hyperlink>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        _ => false,
    }
}
