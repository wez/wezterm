use super::*;

#[derive(Debug)]
pub enum CSIAction {
    SetPen(CellAttributes),
    SetForegroundColor(color::ColorAttribute),
    SetBackgroundColor(color::ColorAttribute),
    SetIntensity(Intensity),
    SetUnderline(Underline),
    SetItalic(bool),
    SetBlink(bool),
    SetReverse(bool),
    SetStrikethrough(bool),
    SetInvisible(bool),
}

impl CSIAction {
    /// Parses out a "Set Graphics Rendition" action.
    /// Returns the decoded action plus the unparsed remainder of the
    /// parameter stream.  Returns None if we couldn't decode one of
    /// the parameter elements.
    pub fn parse_sgr(params: &[i64]) -> Option<(CSIAction, &[i64])> {
        if params.len() > 5 {
            // ISO-8613-6 foreground and background color specification
            // using full RGB color codes.
            if params[0] == 38 && params[1] == 2 {
                return Some((
                    CSIAction::SetForegroundColor(
                        color::ColorAttribute::Rgb(color::RgbColor {
                            red: params[3] as u8,
                            green: params[4] as u8,
                            blue: params[5] as u8,
                        }),
                    ),
                    &params[6..],
                ));
            }
            if params[0] == 48 && params[1] == 2 {
                return Some((
                    CSIAction::SetBackgroundColor(
                        color::ColorAttribute::Rgb(color::RgbColor {
                            red: params[3] as u8,
                            green: params[4] as u8,
                            blue: params[5] as u8,
                        }),
                    ),
                    &params[6..],
                ));
            }
        }
        if params.len() > 2 {
            // Some special look-ahead cases for 88 and 256 color support
            if params[0] == 38 && params[1] == 5 {
                // 38;5;IDX -> foreground color
                let color = color::ColorAttribute::PaletteIndex(params[2] as u8);
                return Some((CSIAction::SetForegroundColor(color), &params[3..]));
            }

            if params[0] == 48 && params[1] == 5 {
                // 48;5;IDX -> background color
                let color = color::ColorAttribute::PaletteIndex(params[2] as u8);
                return Some((CSIAction::SetBackgroundColor(color), &params[3..]));
            }
        }

        let p = params[0];
        match p {
            0 => Some((CSIAction::SetPen(CellAttributes::default()), &params[1..])),
            1 => Some((CSIAction::SetIntensity(Intensity::Bold), &params[1..])),
            2 => Some((CSIAction::SetIntensity(Intensity::Half), &params[1..])),
            3 => Some((CSIAction::SetItalic(true), &params[1..])),
            4 => Some((CSIAction::SetUnderline(Underline::Single), &params[1..])),
            5 => Some((CSIAction::SetBlink(true), &params[1..])),
            7 => Some((CSIAction::SetReverse(true), &params[1..])),
            8 => Some((CSIAction::SetInvisible(true), &params[1..])),
            9 => Some((CSIAction::SetStrikethrough(true), &params[1..])),
            21 => Some((CSIAction::SetUnderline(Underline::Double), &params[1..])),
            22 => Some((CSIAction::SetIntensity(Intensity::Normal), &params[1..])),
            23 => Some((CSIAction::SetItalic(false), &params[1..])),
            24 => Some((CSIAction::SetUnderline(Underline::None), &params[1..])),
            25 => Some((CSIAction::SetBlink(false), &params[1..])),
            27 => Some((CSIAction::SetReverse(false), &params[1..])),
            28 => Some((CSIAction::SetInvisible(false), &params[1..])),
            29 => Some((CSIAction::SetStrikethrough(false), &params[1..])),
            30...37 => {
                Some((
                    CSIAction::SetForegroundColor(
                        color::ColorAttribute::PaletteIndex(p as u8 - 30),
                    ),
                    &params[1..],
                ))
            }
            39 => {
                Some((
                    CSIAction::SetForegroundColor(
                        color::ColorAttribute::Foreground,
                    ),
                    &params[1..],
                ))
            }
            90...97 => {
                // Bright foreground colors
                Some((
                    CSIAction::SetForegroundColor(
                        color::ColorAttribute::PaletteIndex(p as u8 - 90 + 8),
                    ),
                    &params[1..],
                ))
            }
            40...47 => {
                Some((
                    CSIAction::SetBackgroundColor(
                        color::ColorAttribute::PaletteIndex(p as u8 - 40),
                    ),
                    &params[1..],
                ))
            }
            49 => {
                Some((
                    CSIAction::SetBackgroundColor(
                        color::ColorAttribute::Background,
                    ),
                    &params[1..],
                ))
            }
            100...107 => {
                // Bright background colors
                Some((
                    CSIAction::SetBackgroundColor(
                        color::ColorAttribute::PaletteIndex(p as u8 - 100 + 8),
                    ),
                    &params[1..],
                ))
            }
            _ => None,
        }
    }
}
