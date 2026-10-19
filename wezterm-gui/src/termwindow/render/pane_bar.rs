use crate::customglyph::*;
use crate::tabbar::compute_pane_title;
use crate::termwindow::box_model::*;
use crate::termwindow::render::{RenderScreenLineParams, TripleLayerQuadAllocator};
use crate::termwindow::resize;
use crate::termwindow::{PaneInformation, TabInformation, UIItem, UIItemType};
use crate::utilsprites::RenderMetrics;
use config::{Dimension, DimensionContext, PaneBorderStatus, TabBarColors};
use mux::renderable::RenderableDimensions;
use mux::tab::PositionedPane;
use window::color::LinearRgba;

/// The X drawn in each pane close button — identical to the one used by the
/// fancy tab bar in `fancy_tab_bar.rs`.
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

impl crate::TermWindow {
    /// Paint a one-row title bar for a single split pane.
    ///
    /// The bar is drawn at the top or bottom of the pane's allocated cell
    /// space (controlled by `config.pane_border_status`).  `paint_pane`
    /// reserves this row by shifting terminal content down (Top) or
    /// truncating the last visible row (Bottom), so there is no overlap.
    ///
    /// The text content comes from the `format-pane-title` Lua event (see
    /// `compute_pane_title`).  When no callback is registered the pane's
    /// window title is used as a fallback.
    ///
    /// Returns `UIItem` records for any clickable regions (e.g. close button).
    pub fn paint_pane_title_bar(
        &mut self,
        pos: &PositionedPane,
        pane_info: &[PaneInformation],
        tab_info: &[TabInformation],
        layers: &mut TripleLayerQuadAllocator,
    ) -> anyhow::Result<Vec<UIItem>> {
        let config = self.config.clone();

        let cell_width = self.render_metrics.cell_size.width as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;
        let (padding_left, padding_top) = self.padding_left_top();

        // Compute right padding so the close button doesn't spill into the OS
        // border or window padding on the right side.
        let h_context = DimensionContext {
            dpi: self.dimensions.dpi as f32,
            pixel_max: self.terminal_size.pixel_width as f32,
            pixel_cell: cell_width,
        };
        let padding_right = resize::effective_right_padding(&config, h_context) as f32;

        let tab_bar_height = if self.show_tab_bar {
            self.tab_bar_pixel_height()?
        } else {
            0.
        };
        let (top_bar_height, _bottom_bar_height) = if config.tab_bar_at_bottom {
            (0.0, tab_bar_height)
        } else {
            (tab_bar_height, 0.0)
        };

        let border = self.get_os_border();
        let top_pixel_y = top_bar_height + padding_top + border.top.get() as f32;

        // Pixel y-coordinate of the title bar row.
        //
        // pos.height has already been reduced by one by get_pos_panes_for_tab
        // so that it equals the number of terminal rows.  The title bar
        // therefore sits at pos.top (Top) or pos.top + pos.height (Bottom),
        // which is the row immediately adjacent to the terminal content.
        let title_y = match config.pane_border_status {
            PaneBorderStatus::Top => top_pixel_y + pos.top as f32 * cell_height,
            PaneBorderStatus::Bottom => top_pixel_y + (pos.top + pos.height) as f32 * cell_height,
            PaneBorderStatus::Off => return Ok(vec![]),
        };

        // Left edge of this pane in pixels.
        let title_x = padding_left + border.left.get() as f32 + pos.left as f32 * cell_width;
        let pixel_width = pos.width as f32 * cell_width;

        // Look up the PaneInformation so we can call the Lua callback.
        let pane_info_item = match pane_info.iter().find(|p| p.pane_id == pos.pane.pane_id()) {
            Some(p) => p,
            None => return Ok(vec![]),
        };

        let title_line = compute_pane_title(pane_info_item, pane_info, tab_info, &config);

        let palette = pos.pane.palette();
        let window_is_transparent =
            !self.window_background.is_empty() || config.window_background_opacity != 1.0;

        // Use the tab bar's active/inactive colors so the pane title bar is
        // visually distinct from the terminal content and consistent with the
        // tab bar styling.
        let tab_colors = config
            .resolved_palette
            .tab_bar
            .as_ref()
            .cloned()
            .unwrap_or_else(TabBarColors::default);
        let bar_color = if pos.is_active {
            tab_colors.active_tab()
        } else {
            tab_colors.inactive_tab()
        };
        let foreground = bar_color.fg_color.to_linear();
        let default_bg = bar_color
            .bg_color
            .to_linear()
            .mul_alpha(if window_is_transparent { 0. } else { 1.0 });

        // Draw a solid background strip behind the title text so it is
        // legible even when a window background image is configured.
        self.filled_rectangle(
            layers,
            0,
            euclid::rect(title_x, title_y, pixel_width, cell_height),
            default_bg,
        )?;

        let gl_state = self.render_state.as_ref().unwrap();
        let white_space = gl_state.util_sprites.white_space.texture_coords();
        let filled_box = gl_state.util_sprites.filled_box.texture_coords();

        let dims = pos.pane.get_dimensions();

        // The close button uses the same Poly-based X as the fancy tab bar.
        // We know the exact right-edge pixel position, so we place it
        // absolutely rather than relying on Float::Right.  The container
        // element is fully transparent so it never overwrites the title text
        // rendered by render_screen_line below.
        let show_close = config.show_close_pane_button_in_pane_bar && pos.width >= 2;

        let mut ui_items = vec![];

        if show_close {
            let font = self.fonts.title_font()?;
            let metrics = RenderMetrics::with_font_metrics(&font.metrics());

            let inactive_tab_hover = tab_colors.inactive_tab_hover();
            let active_tab_color = tab_colors.active_tab();

            // Button width: 0.5 left margin + 0.25 left pad + poly + 0.25 right pad + 0.25 right margin
            let close_btn_width = cell_width * 1.75;
            // Clamp the button's right edge so it never overflows into the OS
            // border or window padding on the right side of the screen.
            let window_right = self.dimensions.pixel_width as f32
                - border.right.get() as f32
                - padding_right;
            let close_x = (title_x + pixel_width - close_btn_width).min(window_right - close_btn_width);

            let x_btn = Element::new(
                &font,
                ElementContent::Poly {
                    line_width: metrics.underline_height.max(2),
                    poly: SizedPoly {
                        poly: X_BUTTON,
                        width: Dimension::Pixels(metrics.cell_size.height as f32 / 2.),
                        height: Dimension::Pixels(metrics.cell_size.height as f32 / 2.),
                    },
                },
            )
            .zindex(1)
            .vertical_align(VerticalAlign::Middle)
            .item_type(UIItemType::ClosePane(pos.pane.pane_id()))
            .hover_colors(Some(ElementColors {
                border: BorderColor::default(),
                bg: (if pos.is_active {
                    inactive_tab_hover.bg_color
                } else {
                    active_tab_color.bg_color
                })
                .to_linear()
                .into(),
                text: (if pos.is_active {
                    inactive_tab_hover.fg_color
                } else {
                    active_tab_color.fg_color
                })
                .to_linear()
                .into(),
            }))
            .colors(ElementColors {
                border: BorderColor::default(),
                bg: default_bg.into(),
                text: foreground.into(),
            })
            .padding(BoxDimension {
                left: Dimension::Cells(0.25),
                right: Dimension::Cells(0.25),
                top: Dimension::Cells(0.25),
                bottom: Dimension::Cells(0.25),
            })
            .margin(BoxDimension {
                left: Dimension::Cells(0.5),
                right: Dimension::Cells(0.25),
                top: Dimension::Cells(0.),
                bottom: Dimension::Cells(0.),
            });

            // Wrap in a transparent container positioned at the right edge.
            // The container has no background so it never covers the title
            // text rendered by render_screen_line.
            let container = Element::new(&font, ElementContent::Children(vec![x_btn]))
                .colors(ElementColors {
                    border: BorderColor::default(),
                    bg: LinearRgba(0., 0., 0., 0.).into(),
                    text: foreground.into(),
                })
                .min_width(Some(Dimension::Pixels(close_btn_width)))
                .min_height(Some(Dimension::Pixels(cell_height)));

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
                    bounds: euclid::rect(close_x, 0., close_btn_width, cell_height),
                    metrics: &metrics,
                    gl_state,
                    zindex: 1,
                },
                &container,
            )?;

            // translate y into screen coordinates
            computed.translate(euclid::vec2(0., title_y));

            ui_items.extend(computed.ui_items());
            self.render_element(&computed, gl_state, None)?;
        }

        // Title text — shrink by the exact close-button pixel width so the
        // text clips precisely without leaving a visible gap or overlapping.
        let close_btn_width = cell_width * 1.75;
        let text_pixel_width = if show_close {
            (pixel_width - close_btn_width).max(cell_width)
        } else {
            pixel_width
        };
        let text_cols = (text_pixel_width / cell_width) as usize;

        self.render_screen_line(
            RenderScreenLineParams {
                top_pixel_y: title_y,
                left_pixel_x: title_x,
                pixel_width: text_pixel_width,
                stable_line_idx: None,
                line: &title_line,
                selection: 0..0,
                cursor: &Default::default(),
                palette: &palette,
                dims: &RenderableDimensions {
                    cols: text_cols,
                    physical_top: 0,
                    scrollback_rows: 0,
                    scrollback_top: 0,
                    viewport_rows: 1,
                    dpi: dims.dpi,
                    pixel_height: self.render_metrics.cell_size.height as usize,
                    pixel_width: text_pixel_width as usize,
                    reverse_video: false,
                },
                config: &config,
                cursor_border_color: LinearRgba::default(),
                foreground,
                pane: None,
                is_active: pos.is_active,
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
                use_pixel_positioning: config.experimental_pixel_positioning,
                render_metrics: self.render_metrics,
                shape_key: None,
                password_input: false,
            },
            layers,
        )?;

        Ok(ui_items)
    }
}
