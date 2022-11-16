#![allow(dead_code)]
use crate::color::LinearRgba;
use crate::customglyph::{BlockKey, Poly};
use crate::glyphcache::CachedGlyph;
use crate::quad::{QuadImpl, QuadTrait, TripleLayerQuadAllocator, TripleLayerQuadAllocatorTrait};
use crate::termwindow::{
    BorrowedLayers, ColorEase, MouseCapture, RenderState, TermWindowNotif, UIItem, UIItemType,
};
use crate::utilsprites::RenderMetrics;
use ::window::{RectF, WindowOps};
use anyhow::anyhow;
use config::{Dimension, DimensionContext};
use finl_unicode::grapheme_clusters::Graphemes;
use std::cell::RefCell;
use std::rc::Rc;
use termwiz::cell::{grapheme_column_width, Presentation};
use termwiz::surface::Line;
use wezterm_font::units::PixelUnit;
use wezterm_font::LoadedFont;
use wezterm_term::color::{ColorAttribute, ColorPalette};
use window::bitmaps::atlas::Sprite;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalAlign {
    Top,
    Bottom,
    Middle,
}

impl Default for VerticalAlign {
    fn default() -> VerticalAlign {
        VerticalAlign::Top
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayType {
    Block,
    Inline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Float {
    None,
    Right,
}

impl Default for Float {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PixelDimension {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PixelSizedPoly {
    pub poly: &'static [Poly],
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SizedPoly {
    pub poly: &'static [Poly],
    pub width: Dimension,
    pub height: Dimension,
}

impl SizedPoly {
    pub fn to_pixels(&self, context: &LayoutContext) -> PixelSizedPoly {
        PixelSizedPoly {
            poly: self.poly,
            width: self.width.evaluate_as_pixels(context.width),
            height: self.height.evaluate_as_pixels(context.height),
        }
    }

    pub fn none() -> Self {
        Self {
            poly: &[],
            width: Dimension::default(),
            height: Dimension::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PixelCorners {
    pub top_left: PixelSizedPoly,
    pub top_right: PixelSizedPoly,
    pub bottom_left: PixelSizedPoly,
    pub bottom_right: PixelSizedPoly,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Corners {
    pub top_left: SizedPoly,
    pub top_right: SizedPoly,
    pub bottom_left: SizedPoly,
    pub bottom_right: SizedPoly,
}

impl Corners {
    pub fn to_pixels(&self, context: &LayoutContext) -> PixelCorners {
        PixelCorners {
            top_left: self.top_left.to_pixels(context),
            top_right: self.top_right.to_pixels(context),
            bottom_left: self.bottom_left.to_pixels(context),
            bottom_right: self.bottom_right.to_pixels(context),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BoxDimension {
    pub left: Dimension,
    pub top: Dimension,
    pub right: Dimension,
    pub bottom: Dimension,
}

impl BoxDimension {
    pub const fn new(dim: Dimension) -> Self {
        Self {
            left: dim,
            top: dim,
            right: dim,
            bottom: dim,
        }
    }

    pub fn to_pixels(&self, context: &LayoutContext) -> PixelDimension {
        PixelDimension {
            left: self.left.evaluate_as_pixels(context.width),
            top: self.top.evaluate_as_pixels(context.height),
            right: self.right.evaluate_as_pixels(context.width),
            bottom: self.bottom.evaluate_as_pixels(context.height),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum InheritableColor {
    Inherited,
    Color(LinearRgba),
    Animated {
        color: LinearRgba,
        alt_color: LinearRgba,
        ease: Rc<RefCell<ColorEase>>,
        one_shot: bool,
    },
}

impl Default for InheritableColor {
    fn default() -> Self {
        Self::Inherited
    }
}

impl From<LinearRgba> for InheritableColor {
    fn from(color: LinearRgba) -> InheritableColor {
        InheritableColor::Color(color)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BorderColor {
    pub left: LinearRgba,
    pub top: LinearRgba,
    pub right: LinearRgba,
    pub bottom: LinearRgba,
}

impl BorderColor {
    pub const fn new(color: LinearRgba) -> Self {
        Self {
            left: color,
            top: color,
            right: color,
            bottom: color,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ElementColors {
    pub border: BorderColor,
    pub bg: InheritableColor,
    pub text: InheritableColor,
}

struct ResolvedColor {
    color: LinearRgba,
    alt_color: LinearRgba,
    mix_value: f32,
}

impl ResolvedColor {
    fn apply(&self, quad: &mut QuadImpl) {
        quad.set_fg_color(self.color);
        quad.set_alt_color_and_mix_value(self.alt_color, self.mix_value);
    }
}

impl From<LinearRgba> for ResolvedColor {
    fn from(color: LinearRgba) -> Self {
        Self {
            color,
            alt_color: color,
            mix_value: 0.,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Element {
    pub item_type: Option<UIItemType>,
    pub vertical_align: VerticalAlign,
    pub zindex: i8,
    pub display: DisplayType,
    pub float: Float,
    pub padding: BoxDimension,
    pub margin: BoxDimension,
    pub border: BoxDimension,
    pub border_corners: Option<Corners>,
    pub colors: ElementColors,
    pub hover_colors: Option<ElementColors>,
    pub font: Rc<LoadedFont>,
    pub content: ElementContent,
    pub presentation: Option<Presentation>,
    pub line_height: Option<f64>,
    pub max_width: Option<Dimension>,
    pub min_width: Option<Dimension>,
    pub min_height: Option<Dimension>,
}

impl Element {
    pub fn new(font: &Rc<LoadedFont>, content: ElementContent) -> Self {
        Self {
            item_type: None,
            zindex: 0,
            display: DisplayType::Inline,
            float: Float::None,
            padding: BoxDimension::default(),
            margin: BoxDimension::default(),
            border: BoxDimension::default(),
            border_corners: None,
            vertical_align: VerticalAlign::default(),
            colors: ElementColors::default(),
            hover_colors: None,
            font: Rc::clone(font),
            content,
            presentation: None,
            line_height: None,
            max_width: None,
            min_width: None,
            min_height: None,
        }
    }

    pub fn with_line(font: &Rc<LoadedFont>, line: &Line, palette: &ColorPalette) -> Self {
        let mut content: Vec<Element> = vec![];
        let mut prior_attr = None;

        for cluster in line.cluster(None) {
            // Clustering may introduce cluster boundaries when the text hasn't actually
            // changed style. Undo that here.
            // There's still an issue where the style does actually change and we
            // subsequently don't clip the element.
            // <https://github.com/wez/wezterm/issues/2560>
            if let Some(prior) = content.last_mut() {
                let (fg, bg) = prior_attr.as_ref().unwrap();
                if cluster.attrs.background() == *bg && cluster.attrs.foreground() == *fg {
                    if let ElementContent::Text(t) = &mut prior.content {
                        t.push_str(&cluster.text);
                        continue;
                    }
                }
            }

            let child =
                Element::new(font, ElementContent::Text(cluster.text)).colors(ElementColors {
                    border: BorderColor::default(),
                    bg: if cluster.attrs.background() == ColorAttribute::Default {
                        InheritableColor::Inherited
                    } else {
                        palette
                            .resolve_bg(cluster.attrs.background())
                            .to_linear()
                            .into()
                    },
                    text: if cluster.attrs.foreground() == ColorAttribute::Default {
                        InheritableColor::Inherited
                    } else {
                        palette
                            .resolve_fg(cluster.attrs.foreground())
                            .to_linear()
                            .into()
                    },
                });

            content.push(child);
            prior_attr.replace((cluster.attrs.foreground(), cluster.attrs.background()));
        }

        Self::new(font, ElementContent::Children(content))
    }

    pub fn vertical_align(mut self, align: VerticalAlign) -> Self {
        self.vertical_align = align;
        self
    }

    pub fn item_type(mut self, item_type: UIItemType) -> Self {
        self.item_type.replace(item_type);
        self
    }

    pub fn display(mut self, display: DisplayType) -> Self {
        self.display = display;
        self
    }

    pub fn float(mut self, float: Float) -> Self {
        self.float = float;
        self
    }

    pub fn colors(mut self, colors: ElementColors) -> Self {
        self.colors = colors;
        self
    }

    pub fn hover_colors(mut self, colors: Option<ElementColors>) -> Self {
        self.hover_colors = colors;
        self
    }

    pub fn line_height(mut self, line_height: Option<f64>) -> Self {
        self.line_height = line_height;
        self
    }

    pub fn zindex(mut self, zindex: i8) -> Self {
        self.zindex = zindex;
        self
    }

    pub fn padding(mut self, padding: BoxDimension) -> Self {
        self.padding = padding;
        self
    }

    pub fn border(mut self, border: BoxDimension) -> Self {
        self.border = border;
        self
    }

    pub fn border_corners(mut self, corners: Option<Corners>) -> Self {
        self.border_corners = corners;
        self
    }

    pub fn margin(mut self, margin: BoxDimension) -> Self {
        self.margin = margin;
        self
    }

    pub fn max_width(mut self, width: Option<Dimension>) -> Self {
        self.max_width = width;
        self
    }

    pub fn min_width(mut self, width: Option<Dimension>) -> Self {
        self.min_width = width;
        self
    }

    pub fn min_height(mut self, height: Option<Dimension>) -> Self {
        self.min_height = height;
        self
    }
}

#[derive(Debug, Clone)]
pub enum ElementContent {
    Text(String),
    Children(Vec<Element>),
    Poly { line_width: isize, poly: SizedPoly },
}

pub struct LayoutContext<'a> {
    pub width: DimensionContext,
    pub height: DimensionContext,
    pub bounds: RectF,
    pub metrics: &'a RenderMetrics,
    pub gl_state: &'a RenderState,
    pub zindex: i8,
}

#[derive(Debug, Clone)]
pub struct ComputedElement {
    pub item_type: Option<UIItemType>,
    pub zindex: i8,
    /// The outer bounds of the element box (its margin)
    pub bounds: RectF,
    /// The outer bounds of the area enclosed by its border
    pub border_rect: RectF,
    pub border: PixelDimension,
    pub border_corners: Option<PixelCorners>,
    pub colors: ElementColors,
    pub hover_colors: Option<ElementColors>,
    /// The outer bounds of the area enclosed by the padding
    pub padding: RectF,
    /// The outer bounds of the content
    pub content_rect: RectF,
    pub baseline: f32,

    pub content: ComputedElementContent,
}

impl ComputedElement {
    pub fn translate(&mut self, delta: euclid::Vector2D<f32, PixelUnit>) {
        self.bounds = self.bounds.translate(delta);
        self.border_rect = self.border_rect.translate(delta);
        self.padding = self.padding.translate(delta);
        self.content_rect = self.content_rect.translate(delta);

        match &mut self.content {
            ComputedElementContent::Children(kids) => {
                for kid in kids {
                    kid.translate(delta)
                }
            }
            ComputedElementContent::Text(_) => {}
            ComputedElementContent::Poly { .. } => {}
        }
    }

    pub fn ui_items(&self) -> Vec<UIItem> {
        let mut items = vec![];
        self.ui_item_impl(&mut items);
        items
    }

    fn ui_item_impl(&self, items: &mut Vec<UIItem>) {
        if let Some(item_type) = &self.item_type {
            items.push(UIItem {
                x: self.bounds.min_x().max(0.) as usize,
                y: self.bounds.min_y().max(0.) as usize,
                width: self.bounds.width().max(0.) as usize,
                height: self.bounds.height().max(0.) as usize,
                item_type: item_type.clone(),
            });
        }

        match &self.content {
            ComputedElementContent::Text(_) => {}
            ComputedElementContent::Children(kids) => {
                for kid in kids {
                    kid.ui_item_impl(items);
                }
            }
            ComputedElementContent::Poly { .. } => {}
        }
    }
}

#[derive(Debug, Clone)]
pub enum ComputedElementContent {
    Text(Vec<ElementCell>),
    Children(Vec<ComputedElement>),
    Poly {
        line_width: isize,
        poly: PixelSizedPoly,
    },
}

#[derive(Debug, Clone)]
pub enum ElementCell {
    Sprite(Sprite),
    Glyph(Rc<CachedGlyph>),
}

struct Rects {
    padding: RectF,
    border_rect: RectF,
    bounds: RectF,
    content_rect: RectF,
    translate: euclid::Vector2D<f32, PixelUnit>,
}

impl Element {
    fn compute_rects(&self, context: &LayoutContext, content_rect: RectF) -> Rects {
        let padding = self.padding.to_pixels(context);
        let margin = self.margin.to_pixels(context);
        let border = self.border.to_pixels(context);

        let padding = euclid::rect(
            content_rect.min_x() - padding.left,
            content_rect.min_y() - padding.top,
            content_rect.width() + padding.left + padding.right,
            content_rect.height() + padding.top + padding.bottom,
        );

        let border_rect = euclid::rect(
            padding.min_x() - border.left,
            padding.min_y() - border.top,
            padding.width() + border.left + border.right,
            padding.height() + border.top + border.bottom,
        );

        let bounds = euclid::rect(
            border_rect.min_x() - margin.left,
            border_rect.min_y() - margin.top,
            border_rect.width() + margin.left + margin.right,
            border_rect.height() + margin.top + margin.bottom,
        );
        let translate = euclid::vec2(
            context.bounds.min_x() - bounds.min_x(),
            context.bounds.min_y() - bounds.min_y(),
        );
        Rects {
            padding: padding.translate(translate),
            border_rect: border_rect.translate(translate),
            bounds: bounds.translate(translate),
            content_rect: content_rect.translate(translate),
            translate,
        }
    }
}

impl super::TermWindow {
    pub fn compute_element<'a>(
        &self,
        context: &LayoutContext,
        element: &Element,
    ) -> anyhow::Result<ComputedElement> {
        let local_metrics;
        let local_context;
        let context = if let Some(line_height) = element.line_height {
            local_metrics = context.metrics.scale_line_height(line_height);
            local_context = LayoutContext {
                height: DimensionContext {
                    dpi: context.height.dpi,
                    pixel_max: context.height.pixel_max,
                    pixel_cell: context.height.pixel_cell * line_height as f32,
                },
                width: context.width,
                bounds: context.bounds,
                gl_state: context.gl_state,
                metrics: &local_metrics,
                zindex: context.zindex,
            };
            &local_context
        } else {
            context
        };
        let border_corners = element
            .border_corners
            .as_ref()
            .map(|c| c.to_pixels(context));
        let style = element.font.style();
        let border = element.border.to_pixels(context);
        let baseline = context.height.pixel_cell + context.metrics.descender.get() as f32;
        let min_width = match element.min_width {
            Some(w) => w.evaluate_as_pixels(context.width),
            None => 0.0,
        };
        let min_height = match element.min_height {
            Some(h) => h.evaluate_as_pixels(context.height),
            None => 0.0,
        };

        match &element.content {
            ElementContent::Text(s) => {
                let window = self.window.as_ref().unwrap().clone();
                let direction = wezterm_bidi::Direction::LeftToRight;
                let infos = element.font.shape(
                    &s,
                    move || window.notify(TermWindowNotif::InvalidateShapeCache),
                    BlockKey::filter_out_synthetic,
                    element.presentation,
                    direction,
                    None,
                    None,
                )?;
                let mut computed_cells = vec![];
                let mut glyph_cache = context.gl_state.glyph_cache.borrow_mut();
                let mut pixel_width = 0.0;
                let mut min_y = 0.0f32;

                for info in infos {
                    let cell_start = &s[info.cluster as usize..];
                    let mut iter = Graphemes::new(cell_start).peekable();
                    let grapheme = iter
                        .next()
                        .ok_or_else(|| anyhow!("info.cluster didn't map into string"))?;
                    if let Some(key) = BlockKey::from_str(grapheme) {
                        pixel_width += context.width.pixel_cell;
                        let sprite = glyph_cache.cached_block(key, context.metrics)?;
                        computed_cells.push(ElementCell::Sprite(sprite));
                    } else {
                        let next_grapheme: Option<&str> = iter.peek().map(|s| *s);
                        let followed_by_space = next_grapheme == Some(" ");
                        let num_cells = grapheme_column_width(grapheme, None);
                        let glyph = glyph_cache.cached_glyph(
                            &info,
                            style,
                            followed_by_space,
                            &element.font,
                            context.metrics,
                            num_cells as u8,
                        )?;

                        min_y =
                            min_y.min(baseline - (glyph.y_offset + glyph.bearing_y).get() as f32);

                        pixel_width += glyph.x_advance.get() as f32;
                        computed_cells.push(ElementCell::Glyph(glyph));
                    }
                }

                let pixel_width = match element.max_width {
                    Some(w) => w.evaluate_as_pixels(context.width).min(pixel_width),
                    None => pixel_width,
                }
                .max(min_width)
                .min(context.width.pixel_max);

                let content_rect = euclid::rect(
                    0.,
                    0.,
                    pixel_width,
                    context.height.pixel_cell.max(min_height),
                );

                let rects = element.compute_rects(context, content_rect);

                Ok(ComputedElement {
                    item_type: element.item_type.clone(),
                    zindex: element.zindex + context.zindex,
                    baseline,
                    border,
                    border_corners,
                    colors: element.colors.clone(),
                    hover_colors: element.hover_colors.clone(),
                    bounds: rects.bounds,
                    border_rect: rects.border_rect,
                    padding: rects.padding,
                    content_rect: rects.content_rect,
                    content: ComputedElementContent::Text(computed_cells),
                })
            }
            ElementContent::Children(kids) => {
                let mut block_pixel_width: f32 = 0.;
                let mut block_pixel_height: f32 = 0.;
                let mut computed_kids = vec![];
                let mut max_x: f32 = 0.;
                let mut float_width: f32 = 0.;
                let mut y_coord: f32 = 0.;

                let max_width = match element.max_width {
                    Some(w) => w
                        .evaluate_as_pixels(context.width)
                        .min(context.bounds.width()),
                    None => context.bounds.width(),
                }
                .min(context.width.pixel_max);

                for child in kids {
                    if child.display == DisplayType::Block {
                        y_coord += block_pixel_height;
                        block_pixel_height = 0.;
                        block_pixel_width = 0.;
                    }

                    let kid = self.compute_element(
                        &LayoutContext {
                            bounds: match child.float {
                                Float::None => euclid::rect(
                                    block_pixel_width,
                                    y_coord,
                                    context.bounds.max_x()
                                        - (context.bounds.min_x() + block_pixel_width),
                                    context.bounds.max_y() - (context.bounds.min_y() + y_coord),
                                ),
                                Float::Right => euclid::rect(
                                    0.,
                                    y_coord,
                                    context.bounds.width(),
                                    context.bounds.max_y() - (context.bounds.min_y() + y_coord),
                                ),
                            },
                            gl_state: context.gl_state,
                            height: context.height,
                            metrics: context.metrics,
                            width: DimensionContext {
                                dpi: context.width.dpi,
                                pixel_cell: context.width.pixel_cell,
                                pixel_max: max_width,
                            },
                            zindex: context.zindex + element.zindex,
                        },
                        child,
                    )?;
                    match child.float {
                        Float::Right => {
                            float_width += float_width.max(kid.bounds.width());
                        }
                        Float::None => {
                            block_pixel_width += kid.bounds.width();
                            max_x = max_x.max(block_pixel_width);
                        }
                    }
                    block_pixel_height = block_pixel_height.max(kid.bounds.height());

                    computed_kids.push(kid);
                }

                // Respect min-width
                max_x = max_x.max(min_width);

                let mut float_max_x = (max_x + float_width).min(max_width);

                let pixel_height = (y_coord + block_pixel_height).max(min_height);

                for (kid, child) in computed_kids.iter_mut().zip(kids.iter()) {
                    match child.float {
                        Float::Right => {
                            max_x = max_x.max(float_max_x);
                            let x = float_max_x - kid.bounds.width();
                            float_max_x -= kid.bounds.width();
                            kid.translate(euclid::vec2(x, 0.));
                        }
                        _ => {}
                    }
                    match child.vertical_align {
                        VerticalAlign::Bottom => {
                            kid.translate(euclid::vec2(0., pixel_height - kid.bounds.height()));
                        }
                        VerticalAlign::Middle => {
                            kid.translate(euclid::vec2(
                                0.,
                                (pixel_height - kid.bounds.height()) / 2.0,
                            ));
                        }
                        VerticalAlign::Top => {}
                    }
                }

                computed_kids.sort_by(|a, b| a.zindex.cmp(&b.zindex));

                let content_rect = euclid::rect(0., 0., max_x.min(max_width), pixel_height);
                let rects = element.compute_rects(context, content_rect);

                for kid in &mut computed_kids {
                    kid.translate(rects.translate);
                }

                Ok(ComputedElement {
                    item_type: element.item_type.clone(),
                    zindex: element.zindex + context.zindex,
                    baseline,
                    border,
                    border_corners,
                    colors: element.colors.clone(),
                    hover_colors: element.hover_colors.clone(),
                    bounds: rects.bounds,
                    border_rect: rects.border_rect,
                    padding: rects.padding,
                    content_rect: rects.content_rect,
                    content: ComputedElementContent::Children(computed_kids),
                })
            }
            ElementContent::Poly { poly, line_width } => {
                let poly = poly.to_pixels(context);
                let content_rect = euclid::rect(0., 0., poly.width, poly.height.max(min_height));
                let rects = element.compute_rects(context, content_rect);

                Ok(ComputedElement {
                    item_type: element.item_type.clone(),
                    zindex: element.zindex + context.zindex,
                    baseline,
                    border,
                    border_corners,
                    colors: element.colors.clone(),
                    hover_colors: element.hover_colors.clone(),
                    bounds: rects.bounds,
                    border_rect: rects.border_rect,
                    padding: rects.padding,
                    content_rect: rects.content_rect,
                    content: ComputedElementContent::Poly {
                        poly,
                        line_width: *line_width,
                    },
                })
            }
        }
    }

    pub fn render_element<'a>(
        &self,
        element: &ComputedElement,
        gl_state: &RenderState,
        inherited_colors: Option<&ElementColors>,
    ) -> anyhow::Result<()> {
        let layer = gl_state.layer_for_zindex(element.zindex)?;
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

        let colors = match &element.hover_colors {
            Some(hc) => {
                let hovering =
                    match &self.current_mouse_event {
                        Some(event) => {
                            let mouse_x = event.coords.x as f32;
                            let mouse_y = event.coords.y as f32;
                            mouse_x >= element.bounds.min_x()
                                && mouse_x <= element.bounds.max_x()
                                && mouse_y >= element.bounds.min_y()
                                && mouse_y <= element.bounds.max_y()
                        }
                        None => false,
                    } && matches!(self.current_mouse_capture, None | Some(MouseCapture::UI));
                if hovering {
                    hc
                } else {
                    &element.colors
                }
            }
            None => &element.colors,
        };

        self.render_element_background(element, colors, &mut layers, inherited_colors)?;
        let left = self.dimensions.pixel_width as f32 / -2.0;
        let top = self.dimensions.pixel_height as f32 / -2.0;
        match &element.content {
            ComputedElementContent::Text(cells) => {
                let mut pos_x = element.content_rect.min_x();
                for cell in cells {
                    if pos_x >= element.content_rect.max_x() {
                        break;
                    }
                    match cell {
                        ElementCell::Sprite(sprite) => {
                            let width = sprite.coords.width();
                            let height = sprite.coords.height();
                            let pos_y = top + element.content_rect.min_y();

                            if pos_x + width as f32 > element.content_rect.max_x() {
                                break;
                            }

                            let mut quad = layers.allocate(2)?;
                            quad.set_position(
                                pos_x + left,
                                pos_y,
                                pos_x + left + width as f32,
                                pos_y + height as f32,
                            );
                            self.resolve_text(colors, inherited_colors).apply(&mut quad);
                            quad.set_texture(sprite.texture_coords());
                            quad.set_hsv(None);
                            pos_x += width as f32;
                        }
                        ElementCell::Glyph(glyph) => {
                            if let Some(texture) = glyph.texture.as_ref() {
                                let pos_y = element.content_rect.min_y() as f32 + top
                                    - (glyph.y_offset + glyph.bearing_y).get() as f32
                                    + element.baseline;

                                if pos_x + glyph.x_advance.get() as f32
                                    > element.content_rect.max_x()
                                {
                                    break;
                                }
                                let pos_x = pos_x + (glyph.x_offset + glyph.bearing_x).get() as f32;
                                let width = texture.coords.size.width as f32 * glyph.scale as f32;
                                let height = texture.coords.size.height as f32 * glyph.scale as f32;

                                let mut quad = layers.allocate(1)?;
                                quad.set_position(
                                    pos_x + left,
                                    pos_y,
                                    pos_x + left + width,
                                    pos_y + height,
                                );
                                self.resolve_text(colors, inherited_colors).apply(&mut quad);
                                quad.set_texture(texture.texture_coords());
                                quad.set_has_color(glyph.has_color);
                                quad.set_hsv(None);
                            }
                            pos_x += glyph.x_advance.get() as f32;
                        }
                    }
                }
            }
            ComputedElementContent::Children(kids) => {
                drop(layers);
                drop(vb_mut0);
                drop(vb_mut1);
                drop(vb_mut2);

                for kid in kids {
                    self.render_element(kid, gl_state, Some(colors))?;
                }
            }
            ComputedElementContent::Poly { poly, line_width } => {
                if element.content_rect.width() >= poly.width {
                    let mut quad = self.poly_quad(
                        &mut layers,
                        1,
                        element.content_rect.origin,
                        poly.poly,
                        *line_width,
                        euclid::size2(poly.width, poly.height),
                        LinearRgba::TRANSPARENT,
                    )?;
                    self.resolve_text(colors, inherited_colors).apply(&mut quad);
                }
            }
        }

        Ok(())
    }

    fn resolve_text(
        &self,
        colors: &ElementColors,
        inherited_colors: Option<&ElementColors>,
    ) -> ResolvedColor {
        match &colors.text {
            InheritableColor::Inherited => match inherited_colors {
                Some(colors) => self.resolve_text(colors, None),
                None => LinearRgba::TRANSPARENT.into(),
            },
            InheritableColor::Color(color) => (*color).into(),
            InheritableColor::Animated {
                color,
                alt_color,
                ease,
                one_shot,
            } => {
                if let Some((mix_value, next)) = ease.borrow_mut().intensity(*one_shot) {
                    self.update_next_frame_time(Some(next));
                    ResolvedColor {
                        color: *color,
                        alt_color: *alt_color,
                        mix_value,
                    }
                } else {
                    (*color).into()
                }
            }
        }
    }

    fn resolve_bg(
        &self,
        colors: &ElementColors,
        inherited_colors: Option<&ElementColors>,
    ) -> ResolvedColor {
        match &colors.bg {
            InheritableColor::Inherited => match inherited_colors {
                Some(colors) => self.resolve_bg(colors, None),
                None => LinearRgba::TRANSPARENT.into(),
            },
            InheritableColor::Color(color) => (*color).into(),
            InheritableColor::Animated {
                color,
                alt_color,
                ease,
                one_shot,
            } => {
                if let Some((mix_value, next)) = ease.borrow_mut().intensity(*one_shot) {
                    self.update_next_frame_time(Some(next));
                    ResolvedColor {
                        color: *color,
                        alt_color: *alt_color,
                        mix_value,
                    }
                } else {
                    (*color).into()
                }
            }
        }
    }

    fn render_element_background<'a>(
        &self,
        element: &ComputedElement,
        colors: &ElementColors,
        layers: &mut TripleLayerQuadAllocator,
        inherited_colors: Option<&ElementColors>,
    ) -> anyhow::Result<()> {
        let mut top_left_width = 0.;
        let mut top_left_height = 0.;
        let mut top_right_width = 0.;
        let mut top_right_height = 0.;

        let mut bottom_left_width = 0.;
        let mut bottom_left_height = 0.;
        let mut bottom_right_width = 0.;
        let mut bottom_right_height = 0.;

        if let Some(c) = &element.border_corners {
            top_left_width = c.top_left.width;
            top_left_height = c.top_left.height;
            top_right_width = c.top_right.width;
            top_right_height = c.top_right.height;

            bottom_left_width = c.bottom_left.width;
            bottom_left_height = c.bottom_left.height;
            bottom_right_width = c.bottom_right.width;
            bottom_right_height = c.bottom_right.height;

            if top_left_width > 0. && top_left_height > 0. {
                self.poly_quad(
                    layers,
                    0,
                    element.border_rect.origin,
                    c.top_left.poly,
                    element.border.top as isize,
                    euclid::size2(top_left_width, top_left_height),
                    colors.border.top,
                )?
                .set_grayscale();
            }
            if top_right_width > 0. && top_right_height > 0. {
                self.poly_quad(
                    layers,
                    0,
                    euclid::point2(
                        element.border_rect.max_x() - top_right_width,
                        element.border_rect.min_y(),
                    ),
                    c.top_right.poly,
                    element.border.top as isize,
                    euclid::size2(top_right_width, top_right_height),
                    colors.border.top,
                )?
                .set_grayscale();
            }
            if bottom_left_width > 0. && bottom_left_height > 0. {
                self.poly_quad(
                    layers,
                    0,
                    euclid::point2(
                        element.border_rect.min_x(),
                        element.border_rect.max_y() - bottom_left_height,
                    ),
                    c.bottom_left.poly,
                    element.border.bottom as isize,
                    euclid::size2(bottom_left_width, bottom_left_height),
                    colors.border.bottom,
                )?
                .set_grayscale();
            }
            if bottom_right_width > 0. && bottom_right_height > 0. {
                self.poly_quad(
                    layers,
                    0,
                    euclid::point2(
                        element.border_rect.max_x() - bottom_right_width,
                        element.border_rect.max_y() - bottom_right_height,
                    ),
                    c.bottom_right.poly,
                    element.border.bottom as isize,
                    euclid::size2(bottom_right_width, bottom_right_height),
                    colors.border.bottom,
                )?
                .set_grayscale();
            }

            // Filling the background is more complex because we can't
            // simply fill the padding rect--we'd clobber the corner
            // graphics.
            // Instead, we consider the element as consisting of:
            //
            //   TL T TR
            //   L  C  R
            //   BL B BR
            //
            // We already rendered the corner pieces, so now we need
            // to do the rest

            // The `T` piece
            let mut quad = self.filled_rectangle(
                layers,
                0,
                euclid::rect(
                    element.border_rect.min_x() + top_left_width,
                    element.border_rect.min_y(),
                    element.border_rect.width() - (top_left_width + top_right_width) as f32,
                    top_left_height.max(top_right_height),
                ),
                LinearRgba::TRANSPARENT,
            )?;
            self.resolve_bg(colors, inherited_colors).apply(&mut quad);

            // The `B` piece
            let mut quad = self.filled_rectangle(
                layers,
                0,
                euclid::rect(
                    element.border_rect.min_x() + bottom_left_width,
                    element.border_rect.max_y() - bottom_left_height.max(bottom_right_height),
                    element.border_rect.width() - (bottom_left_width + bottom_right_width),
                    bottom_left_height.max(bottom_right_height),
                ),
                LinearRgba::TRANSPARENT,
            )?;
            self.resolve_bg(colors, inherited_colors).apply(&mut quad);

            // The `L` piece
            let mut quad = self.filled_rectangle(
                layers,
                0,
                euclid::rect(
                    element.border_rect.min_x(),
                    element.border_rect.min_y() + top_left_height,
                    top_left_width.max(bottom_left_width),
                    element.border_rect.height() - (top_left_height + bottom_left_height),
                ),
                LinearRgba::TRANSPARENT,
            )?;
            self.resolve_bg(colors, inherited_colors).apply(&mut quad);

            // The `R` piece
            let mut quad = self.filled_rectangle(
                layers,
                0,
                euclid::rect(
                    element.border_rect.max_x() - top_right_width,
                    element.border_rect.min_y() + top_right_height,
                    top_right_width.max(bottom_right_width),
                    element.border_rect.height() - (top_right_height + bottom_right_height),
                ),
                LinearRgba::TRANSPARENT,
            )?;
            self.resolve_bg(colors, inherited_colors).apply(&mut quad);

            // The `C` piece
            let mut quad = self.filled_rectangle(
                layers,
                0,
                euclid::rect(
                    element.border_rect.min_x() + top_left_width,
                    element.border_rect.min_y() + top_right_height.min(top_left_height),
                    element.border_rect.width() - (top_left_width + top_right_width),
                    element.border_rect.height()
                        - (top_right_height.min(top_left_height)
                            + bottom_right_height.min(bottom_left_height)),
                ),
                LinearRgba::TRANSPARENT,
            )?;
            self.resolve_bg(colors, inherited_colors).apply(&mut quad);
        } else if colors.bg != InheritableColor::Color(LinearRgba::TRANSPARENT) {
            let mut quad =
                self.filled_rectangle(layers, 0, element.padding, LinearRgba::TRANSPARENT)?;
            self.resolve_bg(colors, inherited_colors).apply(&mut quad);
        }

        if element.border_rect == element.padding {
            // There's no border to be drawn
            return Ok(());
        }

        if element.border.top > 0. && colors.border.top != LinearRgba::TRANSPARENT {
            self.filled_rectangle(
                layers,
                0,
                euclid::rect(
                    element.border_rect.min_x() + top_left_width as f32,
                    element.border_rect.min_y(),
                    element.border_rect.width() - (top_left_width + top_right_width) as f32,
                    element.border.top,
                ),
                colors.border.top,
            )?;
        }
        if element.border.bottom > 0. && colors.border.bottom != LinearRgba::TRANSPARENT {
            self.filled_rectangle(
                layers,
                0,
                euclid::rect(
                    element.border_rect.min_x() + bottom_left_width as f32,
                    element.border_rect.max_y() - element.border.bottom,
                    element.border_rect.width() - (bottom_left_width + bottom_right_width) as f32,
                    element.border.bottom,
                ),
                colors.border.bottom,
            )?;
        }
        if element.border.left > 0. && colors.border.left != LinearRgba::TRANSPARENT {
            self.filled_rectangle(
                layers,
                0,
                euclid::rect(
                    element.border_rect.min_x(),
                    element.border_rect.min_y() + top_left_height as f32,
                    element.border.left,
                    element.border_rect.height() - (top_left_height + bottom_left_height) as f32,
                ),
                colors.border.left,
            )?;
        }
        if element.border.right > 0. && colors.border.right != LinearRgba::TRANSPARENT {
            self.filled_rectangle(
                layers,
                0,
                euclid::rect(
                    element.border_rect.max_x() - element.border.right,
                    element.border_rect.min_y() + top_right_height as f32,
                    element.border.left,
                    element.border_rect.height() - (top_right_height + bottom_right_height) as f32,
                ),
                colors.border.right,
            )?;
        }

        Ok(())
    }
}
