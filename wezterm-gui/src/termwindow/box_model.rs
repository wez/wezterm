#![allow(dead_code)]
use crate::color::LinearRgba;
use crate::customglyph::{BlockKey, Poly};
use crate::glyphcache::CachedGlyph;
use crate::termwindow::render::rgbcolor_to_window_color;
use crate::termwindow::{
    MappedQuads, RenderState, SrgbTexture2d, TermWindowNotif, UIItem, UIItemType,
};
use crate::utilsprites::RenderMetrics;
use ::window::{RectF, WindowOps};
use anyhow::anyhow;
use config::{Dimension, DimensionContext};
use std::ops::Sub;
use std::rc::Rc;
use termwiz::cell::{grapheme_column_width, Presentation};
use termwiz::surface::Line;
use unicode_segmentation::UnicodeSegmentation;
use wezterm_font::units::PixelUnit;
use wezterm_font::LoadedFont;
use wezterm_term::color::{ColorAttribute, ColorPalette};
use window::bitmaps::atlas::Sprite;

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

#[derive(Debug, Clone, Copy, PartialEq)]
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
            bottom_right: self.bottom_left.to_pixels(context),
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InheritableColor {
    Inherited,
    Color(LinearRgba),
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

#[derive(Debug, Clone, Copy, Default)]
pub struct ElementColors {
    pub border: BorderColor,
    pub bg: InheritableColor,
    pub text: InheritableColor,
}

impl ElementColors {
    pub fn resolve_bg(&self, inherited_colors: Option<&ElementColors>) -> LinearRgba {
        match self.bg {
            InheritableColor::Inherited => match inherited_colors {
                Some(colors) => colors.resolve_bg(None),
                None => LinearRgba::TRANSPARENT,
            },
            InheritableColor::Color(color) => color,
        }
    }

    pub fn resolve_text(&self, inherited_colors: Option<&ElementColors>) -> LinearRgba {
        match self.text {
            InheritableColor::Inherited => match inherited_colors {
                Some(colors) => colors.resolve_text(None),
                None => LinearRgba::TRANSPARENT,
            },
            InheritableColor::Color(color) => color,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Element {
    pub item_type: Option<UIItemType>,
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
            colors: ElementColors::default(),
            hover_colors: None,
            font: Rc::clone(font),
            content,
            presentation: None,
            line_height: None,
        }
    }

    pub fn with_line(font: &Rc<LoadedFont>, line: &Line, palette: &ColorPalette) -> Self {
        let mut content = vec![];

        for cluster in line.cluster() {
            let child =
                Element::new(font, ElementContent::Text(cluster.text)).colors(ElementColors {
                    border: BorderColor::default(),
                    bg: if cluster.attrs.background() == ColorAttribute::Default {
                        InheritableColor::Inherited
                    } else {
                        rgbcolor_to_window_color(palette.resolve_bg(cluster.attrs.background()))
                            .into()
                    },
                    text: if cluster.attrs.foreground() == ColorAttribute::Default {
                        InheritableColor::Inherited
                    } else {
                        rgbcolor_to_window_color(palette.resolve_fg(cluster.attrs.foreground()))
                            .into()
                    },
                });

            content.push(child);
        }

        Self::new(font, ElementContent::Children(content))
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
}

#[derive(Debug, Clone)]
pub enum ElementContent {
    Text(String),
    Children(Vec<Element>),
}

pub struct LayoutContext<'a> {
    pub width: DimensionContext,
    pub height: DimensionContext,
    pub bounds: RectF,
    pub metrics: &'a RenderMetrics,
    pub gl_state: &'a RenderState,
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
    fn translate(&mut self, delta: euclid::Vector2D<f32, PixelUnit>) {
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
        }
    }
}

#[derive(Debug, Clone)]
pub enum ComputedElementContent {
    Text(Vec<ElementCell>),
    Children(Vec<ComputedElement>),
}

#[derive(Debug, Clone)]
pub enum ElementCell {
    Sprite(Sprite<SrgbTexture2d>),
    Glyph(Rc<CachedGlyph<SrgbTexture2d>>),
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
            };
            &local_context
        } else {
            context
        };
        let padding = element.padding.to_pixels(context);
        let margin = element.margin.to_pixels(context);
        let border = element.border.to_pixels(context);
        let border_corners = element
            .border_corners
            .as_ref()
            .map(|c| c.to_pixels(context));
        let style = element.font.style();
        let baseline = context.height.pixel_cell + context.metrics.descender.get() as f32;

        match &element.content {
            ElementContent::Text(s) => {
                let window = self.window.as_ref().unwrap().clone();
                let infos = element.font.shape(
                    &s,
                    move || window.notify(TermWindowNotif::InvalidateShapeCache),
                    BlockKey::filter_out_synthetic,
                    element.presentation,
                )?;
                let mut computed_cells = vec![];
                let mut glyph_cache = context.gl_state.glyph_cache.borrow_mut();
                let mut pixel_width = 0.0;
                let mut min_y = 0.0f32;

                for info in infos {
                    let cell_start = &s[info.cluster as usize..];
                    let mut iter = cell_start.graphemes(true).peekable();
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

                let content_rect = euclid::rect(0., 0., pixel_width, context.height.pixel_cell);

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

                Ok(ComputedElement {
                    item_type: element.item_type.clone(),
                    zindex: element.zindex,
                    baseline,
                    border,
                    border_corners,
                    colors: element.colors,
                    hover_colors: element.hover_colors,
                    bounds: bounds.translate(translate),
                    border_rect: border_rect.translate(translate),
                    padding: padding.translate(translate),
                    content_rect: content_rect.translate(translate),
                    content: ComputedElementContent::Text(computed_cells),
                })
            }
            ElementContent::Children(kids) => {
                let mut pixel_width: f32 = 0.;
                let mut pixel_height: f32 = 0.;
                let mut computed_kids = vec![];
                let mut max_x: f32 = 0.;

                for child in kids {
                    let mut kid = self.compute_element(
                        &LayoutContext {
                            bounds: match child.float {
                                Float::None => euclid::rect(
                                    pixel_width,
                                    context.bounds.min_y(),
                                    context.bounds.max_x() - (context.bounds.min_x() + pixel_width),
                                    context.bounds.height(),
                                ),
                                Float::Right => euclid::rect(
                                    0.,
                                    context.bounds.min_y(),
                                    context.bounds.width(),
                                    context.bounds.height(),
                                ),
                            },
                            gl_state: context.gl_state,
                            height: context.height,
                            metrics: context.metrics,
                            width: context.width,
                        },
                        child,
                    )?;
                    match child.float {
                        Float::Right => {
                            let padded_max_x = context
                                .bounds
                                .max_x()
                                .sub(padding.left)
                                .sub(padding.right)
                                .sub(margin.left)
                                .sub(margin.right)
                                .sub(border.left)
                                .sub(border.right)
                                .max(0.);

                            kid.translate(euclid::vec2(padded_max_x - kid.bounds.width(), 0.));
                            max_x = max_x.max(padded_max_x);
                        }
                        Float::None => {
                            pixel_width += kid.bounds.width();
                            max_x = max_x.max(pixel_width);
                        }
                    }
                    pixel_height = pixel_height.max(kid.bounds.height());

                    computed_kids.push(kid);
                }

                computed_kids.sort_by(|a, b| a.zindex.cmp(&b.zindex));

                let content_rect = euclid::rect(0., 0., max_x, pixel_height);

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

                for kid in &mut computed_kids {
                    kid.translate(translate);
                }

                Ok(ComputedElement {
                    item_type: element.item_type.clone(),
                    zindex: element.zindex,
                    baseline,
                    border,
                    border_corners,
                    colors: element.colors,
                    hover_colors: element.hover_colors,
                    bounds: bounds.translate(translate),
                    border_rect: border_rect.translate(translate),
                    padding: padding.translate(translate),
                    content_rect: content_rect.translate(translate),
                    content: ComputedElementContent::Children(computed_kids),
                })
            }
        }
    }

    pub fn render_element<'a>(
        &self,
        element: &ComputedElement,
        layer: &'a mut MappedQuads,
        inherited_colors: Option<&ElementColors>,
    ) -> anyhow::Result<()> {
        let colors = match &element.hover_colors {
            Some(hc) => {
                let hovering = match &self.current_mouse_event {
                    Some(event) => {
                        let mouse_x = event.coords.x as f32;
                        let mouse_y = event.coords.y as f32;
                        mouse_x >= element.bounds.min_x()
                            && mouse_x <= element.bounds.max_x()
                            && mouse_y >= element.bounds.min_y()
                            && mouse_y <= element.bounds.max_y()
                    }
                    None => false,
                };
                if hovering {
                    hc
                } else {
                    &element.colors
                }
            }
            None => &element.colors,
        };

        self.render_element_background(element, colors, layer, inherited_colors)?;
        let left = self.dimensions.pixel_width as f32 / -2.0;
        let top = self.dimensions.pixel_height as f32 / -2.0;
        match &element.content {
            ComputedElementContent::Text(cells) => {
                let mut pos_x = element.content_rect.min_x() as f32;
                for cell in cells {
                    match cell {
                        ElementCell::Sprite(sprite) => {
                            let mut quad = layer.allocate()?;
                            let width = sprite.coords.width();
                            let height = sprite.coords.height();
                            let pos_y = top + element.content_rect.min_y() as f32;
                            quad.set_position(
                                pos_x + left,
                                pos_y,
                                pos_x + left + width as f32,
                                pos_y + height as f32,
                            );
                            quad.set_fg_color(colors.resolve_text(inherited_colors));
                            quad.set_texture(sprite.texture_coords());
                            quad.set_hsv(None);
                            pos_x += width as f32;
                        }
                        ElementCell::Glyph(glyph) => {
                            if let Some(texture) = glyph.texture.as_ref() {
                                let mut quad = layer.allocate()?;
                                let pos_y = element.content_rect.min_y() as f32 + top
                                    - (glyph.y_offset + glyph.bearing_y).get() as f32
                                    + element.baseline;

                                let width = texture.coords.size.width as f32 * glyph.scale as f32;
                                let height = texture.coords.size.height as f32 * glyph.scale as f32;

                                quad.set_position(
                                    pos_x + left,
                                    pos_y,
                                    pos_x + left + width,
                                    pos_y + height,
                                );
                                quad.set_fg_color(colors.resolve_text(inherited_colors));
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
                for kid in kids {
                    self.render_element(kid, layer, Some(colors))?;
                }
            }
        }

        Ok(())
    }

    fn render_element_background<'a>(
        &self,
        element: &ComputedElement,
        colors: &ElementColors,
        layer: &'a mut MappedQuads,
        inherited_colors: Option<&ElementColors>,
    ) -> anyhow::Result<()> {
        let mut top_left_width = 0;
        let mut top_left_height = 0;
        let mut top_right_width = 0;
        let mut top_right_height = 0;

        let mut bottom_left_width = 0;
        let mut bottom_left_height = 0;
        let mut bottom_right_width = 0;
        let mut bottom_right_height = 0;

        if let Some(c) = &element.border_corners {
            top_left_width = c.top_left.width as isize;
            top_left_height = c.top_left.height as isize;
            top_right_width = c.top_right.width as isize;
            top_right_height = c.top_right.height as isize;

            bottom_left_width = c.bottom_left.width as isize;
            bottom_left_height = c.bottom_left.height as isize;
            bottom_right_width = c.bottom_right.width as isize;
            bottom_right_height = c.bottom_right.height as isize;

            let underline_height = 1;
            if top_left_width > 0 && top_left_height > 0 {
                self.poly_quad(
                    layer,
                    element.border_rect.origin,
                    c.top_left.poly,
                    underline_height,
                    euclid::size2(top_left_width, top_left_height),
                    colors.border.top,
                )?;
            }
            if top_right_width > 0 && top_right_height > 0 {
                self.poly_quad(
                    layer,
                    euclid::point2(
                        element.border_rect.max_x() - top_right_width as f32,
                        element.border_rect.min_y(),
                    ),
                    c.top_right.poly,
                    underline_height,
                    euclid::size2(top_right_width, top_right_height),
                    colors.border.top,
                )?;
            }
            if bottom_left_width > 0 && bottom_left_height > 0 {
                self.poly_quad(
                    layer,
                    euclid::point2(
                        element.border_rect.min_x(),
                        element.border_rect.max_y() - bottom_left_height as f32,
                    ),
                    c.bottom_left.poly,
                    underline_height,
                    euclid::size2(bottom_left_width, bottom_left_height),
                    colors.border.bottom,
                )?;
            }
            if bottom_right_width > 0 && bottom_right_height > 0 {
                self.poly_quad(
                    layer,
                    euclid::point2(
                        element.border_rect.max_x() - bottom_right_width as f32,
                        element.border_rect.max_y() - bottom_right_height as f32,
                    ),
                    c.bottom_right.poly,
                    underline_height,
                    euclid::size2(bottom_right_width, bottom_right_height),
                    colors.border.bottom,
                )?;
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
            self.filled_rectangle(
                layer,
                euclid::rect(
                    element.border_rect.min_x() + top_left_width as f32,
                    element.border_rect.min_y(),
                    element.border_rect.width() - (top_left_width + top_right_width) as f32,
                    top_left_height.max(top_right_height) as f32,
                ),
                colors.resolve_bg(inherited_colors),
            )?;

            // The `B` piece
            self.filled_rectangle(
                layer,
                euclid::rect(
                    element.border_rect.min_x() + bottom_left_width as f32,
                    element.border_rect.max_y()
                        - bottom_left_height.max(bottom_right_height) as f32,
                    element.border_rect.width() - (bottom_left_width + bottom_right_width) as f32,
                    bottom_left_height.max(bottom_right_height) as f32,
                ),
                colors.resolve_bg(inherited_colors),
            )?;

            // The `L` piece
            self.filled_rectangle(
                layer,
                euclid::rect(
                    element.border_rect.min_x(),
                    element.border_rect.min_y() + top_left_height as f32,
                    top_left_width.max(bottom_left_width) as f32,
                    element.border_rect.height() - (top_left_height + bottom_left_height) as f32,
                ),
                colors.resolve_bg(inherited_colors),
            )?;

            // The `R` piece
            self.filled_rectangle(
                layer,
                euclid::rect(
                    element.border_rect.max_x() - top_right_width as f32,
                    element.border_rect.min_y() + top_right_height as f32,
                    top_right_width.max(bottom_right_width) as f32,
                    element.border_rect.height() - (top_right_height + bottom_right_height) as f32,
                ),
                colors.resolve_bg(inherited_colors),
            )?;

            // The `C` piece
            self.filled_rectangle(
                layer,
                euclid::rect(
                    element.border_rect.min_x() + top_left_width as f32,
                    element.border_rect.min_y() + top_right_height.min(top_left_height) as f32,
                    element.border_rect.width() - (top_left_width + top_right_width) as f32,
                    element.border_rect.height()
                        - (top_right_height.min(top_left_height)
                            + bottom_right_height.min(bottom_left_height))
                            as f32,
                ),
                colors.resolve_bg(inherited_colors),
            )?;
        } else if colors.bg != InheritableColor::Color(LinearRgba::TRANSPARENT) {
            self.filled_rectangle(layer, element.padding, colors.resolve_bg(inherited_colors))?;
        }

        if element.border_rect == element.padding {
            // There's no border to be drawn
            return Ok(());
        }

        if element.border.top > 0. && colors.border.top != LinearRgba::TRANSPARENT {
            self.filled_rectangle(
                layer,
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
                layer,
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
                layer,
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
                layer,
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
