use crate::customglyph::*;
use crate::termwindow::box_model::*;
use crate::termwindow::render::corners::*;
use crate::termwindow::{TabBarItem, UIItemType};
use crate::utilsprites::RenderMetrics;
use config::{ConfigHandle, Dimension, IntegratedTitleButtonColor};
use std::rc::Rc;
use wezterm_font::LoadedFont;
use window::color::LinearRgba;
use window::{IntegratedTitleButton, IntegratedTitleButtonStyle as Style};

pub struct WindowButtonColors {
    pub colors: ElementColors,
    pub hover_colors: ElementColors,
}

fn auto_button_color(
    background_lightness: f64,
    foreground: IntegratedTitleButtonColor,
) -> LinearRgba {
    match foreground {
        IntegratedTitleButtonColor::Custom(color) => color.to_linear(),
        IntegratedTitleButtonColor::Auto => {
            if background_lightness > 0.5 {
                LinearRgba(0.0, 0.0, 0.0, 1.0)
            } else {
                LinearRgba(1.0, 1.0, 1.0, 1.0)
            }
        }
    }
}

mod windows {
    use super::*;

    pub const CLOSE: &[Poly] = &[Poly {
        path: &[
            PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
            PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Zero),
            PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
        ],
        intensity: BlockAlpha::Full,
        style: PolyStyle::OutlineThin,
    }];

    pub const HIDE: &[Poly] = &[Poly {
        path: &[
            PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(6, 10)),
            PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(6, 10)),
        ],
        intensity: BlockAlpha::Full,
        style: PolyStyle::OutlineThin,
    }];

    pub const MAXIMIZE: &[Poly] = &[Poly {
        path: &[
            PolyCommand::MoveTo(BlockCoord::Frac(2, 10), BlockCoord::Frac(1, 10)),
            PolyCommand::LineTo(BlockCoord::Frac(9, 10), BlockCoord::Frac(1, 10)),
            PolyCommand::LineTo(BlockCoord::Frac(10, 10), BlockCoord::Frac(2, 10)),
            PolyCommand::LineTo(BlockCoord::Frac(10, 10), BlockCoord::Frac(9, 10)),
            PolyCommand::LineTo(BlockCoord::Frac(9, 10), BlockCoord::Frac(10, 10)),
            PolyCommand::LineTo(BlockCoord::Frac(2, 10), BlockCoord::Frac(10, 10)),
            PolyCommand::LineTo(BlockCoord::Frac(1, 10), BlockCoord::Frac(9, 10)),
            PolyCommand::LineTo(BlockCoord::Frac(1, 10), BlockCoord::Frac(2, 10)),
            PolyCommand::LineTo(BlockCoord::Frac(2, 10), BlockCoord::Frac(1, 10)),
        ],
        intensity: BlockAlpha::Full,
        style: PolyStyle::OutlineThin,
    }];

    pub const RESTORE: &[Poly] = &[
        Poly {
            path: &[
                PolyCommand::MoveTo(BlockCoord::Frac(5, 20), BlockCoord::Frac(1, 10)),
                PolyCommand::LineTo(BlockCoord::Frac(8, 10), BlockCoord::Frac(1, 10)),
                PolyCommand::LineTo(BlockCoord::Frac(10, 10), BlockCoord::Frac(3, 10)),
                PolyCommand::LineTo(BlockCoord::Frac(10, 10), BlockCoord::Frac(15, 20)),
            ],
            intensity: BlockAlpha::Full,
            style: PolyStyle::OutlineThin,
        },
        Poly {
            path: &[
                PolyCommand::MoveTo(BlockCoord::Frac(2, 10), BlockCoord::Frac(3, 10)),
                PolyCommand::LineTo(BlockCoord::Frac(7, 10), BlockCoord::Frac(3, 10)),
                PolyCommand::LineTo(BlockCoord::Frac(8, 10), BlockCoord::Frac(4, 10)),
                PolyCommand::LineTo(BlockCoord::Frac(8, 10), BlockCoord::Frac(9, 10)),
                PolyCommand::LineTo(BlockCoord::Frac(7, 10), BlockCoord::Frac(10, 10)),
                PolyCommand::LineTo(BlockCoord::Frac(2, 10), BlockCoord::Frac(10, 10)),
                PolyCommand::LineTo(BlockCoord::Frac(1, 10), BlockCoord::Frac(9, 10)),
                PolyCommand::LineTo(BlockCoord::Frac(1, 10), BlockCoord::Frac(4, 10)),
                PolyCommand::LineTo(BlockCoord::Frac(2, 10), BlockCoord::Frac(3, 10)),
            ],
            intensity: BlockAlpha::Full,
            style: PolyStyle::OutlineThin,
        },
    ];

    pub fn sized_poly(poly: &'static [Poly]) -> SizedPoly {
        let scale = 72.0 / 96.0;
        let size = Dimension::Points(10. * scale);
        SizedPoly {
            poly,
            width: size,
            height: size,
        }
    }

    pub fn window_button_colors(
        background_lightness: f64,
        foreground: config::IntegratedTitleButtonColor,
        window_button: IntegratedTitleButton,
    ) -> WindowButtonColors {
        let foreground = auto_button_color(background_lightness, foreground);
        let colors = ElementColors {
            border: BorderColor::new(LinearRgba::TRANSPARENT),
            bg: LinearRgba::TRANSPARENT.into(),
            text: foreground.into(),
        };

        let hover_colors = if window_button == IntegratedTitleButton::Close {
            ElementColors {
                border: BorderColor::new(LinearRgba::TRANSPARENT),
                bg: LinearRgba(1.0, 0.0, 0.0, 1.0).into(),
                text: LinearRgba(1.0, 1.0, 1.0, 1.0).into(),
            }
        } else {
            ElementColors {
                border: BorderColor::new(LinearRgba::TRANSPARENT),
                bg: foreground.mul_alpha(0.1).into(),
                text: foreground.into(),
            }
        };

        WindowButtonColors {
            colors,
            hover_colors,
        }
    }
}

mod gnome {
    use super::*;

    pub const CLOSE: &[Poly] = &[Poly {
        path: &[
            PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
            PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Zero),
            PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
        ],
        intensity: BlockAlpha::Full,
        style: PolyStyle::Outline,
    }];

    pub const HIDE: &[Poly] = &[Poly {
        path: &[
            PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(15, 16)),
            PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(15, 16)),
        ],
        intensity: BlockAlpha::Full,
        style: PolyStyle::Outline,
    }];

    pub const MAXIMIZE: &[Poly] = &[Poly {
        path: &[
            PolyCommand::LineTo(BlockCoord::Frac(1, 16), BlockCoord::Frac(15, 16)),
            PolyCommand::LineTo(BlockCoord::Frac(15, 16), BlockCoord::Frac(15, 16)),
            PolyCommand::LineTo(BlockCoord::Frac(15, 16), BlockCoord::Frac(1, 16)),
            PolyCommand::LineTo(BlockCoord::Frac(1, 16), BlockCoord::Frac(1, 16)),
        ],
        intensity: BlockAlpha::Full,
        style: PolyStyle::Outline,
    }];

    pub const RESTORE: &[Poly] = &[Poly {
        path: &[
            PolyCommand::MoveTo(BlockCoord::Frac(3, 16), BlockCoord::Frac(3, 16)),
            PolyCommand::LineTo(BlockCoord::Frac(3, 16), BlockCoord::Frac(13, 16)),
            PolyCommand::LineTo(BlockCoord::Frac(13, 16), BlockCoord::Frac(13, 16)),
            PolyCommand::LineTo(BlockCoord::Frac(13, 16), BlockCoord::Frac(3, 16)),
            PolyCommand::LineTo(BlockCoord::Frac(3, 16), BlockCoord::Frac(3, 16)),
        ],
        intensity: BlockAlpha::Full,
        style: PolyStyle::Outline,
    }];

    pub fn sized_poly(poly: &'static [Poly]) -> SizedPoly {
        let size = Dimension::Pixels(8.);
        SizedPoly {
            poly,
            width: size,
            height: size,
        }
    }

    pub fn window_button_colors(
        background_lightness: f64,
        foreground: config::IntegratedTitleButtonColor,
        _window_button: IntegratedTitleButton,
    ) -> WindowButtonColors {
        let foreground = auto_button_color(background_lightness, foreground);
        WindowButtonColors {
            colors: ElementColors {
                border: BorderColor::new(foreground.mul_alpha(0.1)),
                bg: foreground.mul_alpha(0.1).into(),
                text: foreground.into(),
            },
            hover_colors: ElementColors {
                border: BorderColor::new(foreground.mul_alpha(0.15)),
                bg: foreground.mul_alpha(0.15).into(),
                text: foreground.into(),
            },
        }
    }
}

pub fn window_button_element(
    window_button: IntegratedTitleButton,
    is_maximized: bool,
    font: &Rc<LoadedFont>,
    metrics: &RenderMetrics,
    config: &ConfigHandle,
) -> Element {
    let style = config.integrated_title_button_style;

    if style == Style::MacOsNative {
        return Element::new(font, ElementContent::Text(String::new()));
    }

    let poly = {
        let (close, hide, maximize, restore) = match style {
            Style::Windows => {
                use self::windows::{CLOSE, HIDE, MAXIMIZE, RESTORE};
                (CLOSE, HIDE, MAXIMIZE, RESTORE)
            }
            Style::Gnome => {
                use self::gnome::{CLOSE, HIDE, MAXIMIZE, RESTORE};
                (CLOSE, HIDE, MAXIMIZE, RESTORE)
            }
            Style::MacOsNative => unreachable!(),
        };
        let poly = match window_button {
            IntegratedTitleButton::Hide => hide,
            IntegratedTitleButton::Maximize => {
                if is_maximized {
                    restore
                } else {
                    maximize
                }
            }
            IntegratedTitleButton::Close => close,
        };

        match style {
            Style::Windows => self::windows::sized_poly(poly),
            Style::Gnome => self::gnome::sized_poly(poly),
            Style::MacOsNative => unreachable!(),
        }
    };

    let element = Element::new(
        &font,
        ElementContent::Poly {
            line_width: metrics.underline_height.max(2),
            poly,
        },
    );

    let element = match style {
        Style::Windows => {
            let left_padding = match window_button {
                IntegratedTitleButton::Hide => 17.0,
                _ => 18.0,
            };
            let scale = 72.0 / 96.0;

            element
                .zindex(1)
                .vertical_align(VerticalAlign::Middle)
                .padding(BoxDimension {
                    left: Dimension::Points(left_padding * scale),
                    right: Dimension::Points(18. * scale),
                    top: Dimension::Points(10. * scale),
                    bottom: Dimension::Points(10. * scale),
                })
        }
        Style::Gnome => {
            let dim = Dimension::Pixels(7.);
            let border_corners_size = Dimension::Pixels(12.);
            element
                .zindex(1)
                .vertical_align(VerticalAlign::Middle)
                .padding(BoxDimension {
                    left: dim,
                    right: dim,
                    top: dim,
                    bottom: dim,
                })
                .border(BoxDimension::new(Dimension::Pixels(1.)))
                .border_corners(Some(Corners {
                    top_left: SizedPoly {
                        width: border_corners_size,
                        height: border_corners_size,
                        poly: TOP_LEFT_ROUNDED_CORNER,
                    },
                    top_right: SizedPoly {
                        width: border_corners_size,
                        height: border_corners_size,
                        poly: TOP_RIGHT_ROUNDED_CORNER,
                    },
                    bottom_left: SizedPoly {
                        width: border_corners_size,
                        height: border_corners_size,
                        poly: BOTTOM_LEFT_ROUNDED_CORNER,
                    },
                    bottom_right: SizedPoly {
                        width: border_corners_size,
                        height: border_corners_size,
                        poly: BOTTOM_RIGHT_ROUNDED_CORNER,
                    },
                }))
                .margin(BoxDimension {
                    left: dim,
                    right: dim,
                    top: dim,
                    bottom: dim,
                })
        }
        Style::MacOsNative => unreachable!(),
    };

    let foreground = config.integrated_title_button_color.clone();
    let background_lightness = {
        let bg: config::RgbaColor = config.window_frame.active_titlebar_bg.into();
        let (_h, _s, l, _a) = bg.to_hsla();
        l
    };

    let window_button_colors_fn = match style {
        Style::Windows => self::windows::window_button_colors,
        Style::Gnome => self::gnome::window_button_colors,
        Style::MacOsNative => unreachable!(),
    };

    let colors = window_button_colors_fn(background_lightness, foreground, window_button);

    let element = element
        .item_type(UIItemType::TabBar(TabBarItem::WindowButton(window_button)))
        .colors(colors.colors)
        .hover_colors(Some(colors.hover_colors));

    element
}
