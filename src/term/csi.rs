//! Parsing CSI escape sequences

use super::*;

#[derive(Debug)]
pub enum LineErase {
    ToRight,
    ToLeft,
    All,
}

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
    SetCursorXY(usize, usize),
    EraseInLine(LineErase),
}

pub struct CSIParser<'a> {
    intermediates: &'a [u8],
    ignore: bool,
    byte: char,
    params: Option<&'a [i64]>,
}

impl<'a> CSIParser<'a> {
    pub fn new<'b>(
        params: &'b [i64],
        intermediates: &'b [u8],
        ignore: bool,
        byte: char,
    ) -> CSIParser<'b> {
        CSIParser {
            intermediates,
            ignore,
            byte,
            params: Some(params),
        }
    }

    fn advance_by(&mut self, n: usize, params: &'a [i64]) {
        let (_, next) = params.split_at(n);
        if next.len() != 0 {
            self.params = Some(next);
        }
    }

    /// Erase in Line (EL)
    fn el(&mut self, params: &'a [i64]) -> Option<CSIAction> {
        match params {
            &[] => Some(CSIAction::EraseInLine(LineErase::ToRight)),
            &[i] => {
                self.advance_by(1, params);
                match i {
                    0 => Some(CSIAction::EraseInLine(LineErase::ToRight)),
                    1 => Some(CSIAction::EraseInLine(LineErase::ToLeft)),
                    2 => Some(CSIAction::EraseInLine(LineErase::All)),
                    _ => {
                        println!("el: unknown parameter {:?}", params);
                        None
                    }
                }
            }
            _ => {
                println!("el: unhandled csi sequence {:?}", params);
                None
            }
        }
    }

    /// Cursor Position (CUP)
    fn cup(&mut self, params: &'a [i64]) -> Option<CSIAction> {
        match params {
            &[] => {
                // With no parameters, home the cursor
                Some(CSIAction::SetCursorXY(0, 0))
            }
            &[x, y] => {
                self.advance_by(2, params);
                // Co-ordinates are 1-based, but we want 0-based
                Some(CSIAction::SetCursorXY((x - 1) as usize, (y - 1) as usize))
            }
            _ => {
                println!("CUP: unhandled csi sequence {:?}", params);
                None
            }
        }
    }

    /// Set Graphics Rendition (SGR)
    fn sgr(&mut self, params: &'a [i64]) -> Option<CSIAction> {
        match params {
            &[] => {
                // With no parameters, reset to default pen.
                // Note that this empty case is only possible for the initial
                // iteration.
                Some(CSIAction::SetPen(CellAttributes::default()))
            }
            &[0, _..] => {
                // Explicitly set to default pen
                self.advance_by(1, params);
                Some(CSIAction::SetPen(CellAttributes::default()))
            }
            &[38, 2, _, red, green, blue, _..] => {
                // ISO-8613-6 true color foreground
                self.advance_by(6, params);
                Some(CSIAction::SetForegroundColor(
                    color::ColorAttribute::Rgb(color::RgbColor {
                        red: red as u8,
                        green: green as u8,
                        blue: blue as u8,
                    }),
                ))
            }
            &[48, 2, _, red, green, blue, _..] => {
                // ISO-8613-6 true color background
                self.advance_by(6, params);
                Some(CSIAction::SetBackgroundColor(
                    color::ColorAttribute::Rgb(color::RgbColor {
                        red: red as u8,
                        green: green as u8,
                        blue: blue as u8,
                    }),
                ))
            }
            &[38, 5, idx, _..] => {
                // 256 color foreground color index
                self.advance_by(3, params);
                let color = color::ColorAttribute::PaletteIndex(idx as u8);
                Some(CSIAction::SetForegroundColor(color))
            }
            &[48, 5, idx, _..] => {
                // 256 color background color index
                self.advance_by(3, params);
                let color = color::ColorAttribute::PaletteIndex(idx as u8);
                Some(CSIAction::SetBackgroundColor(color))
            }
            &[1, _..] => {
                self.advance_by(1, params);
                Some(CSIAction::SetIntensity(Intensity::Bold))
            }
            &[2, _..] => {
                self.advance_by(1, params);
                Some(CSIAction::SetIntensity(Intensity::Half))
            }
            &[3, _..] => {
                self.advance_by(1, params);
                Some(CSIAction::SetItalic(true))
            }
            &[4, _..] => {
                self.advance_by(1, params);
                Some(CSIAction::SetUnderline(Underline::Single))
            }
            &[5, _..] => {
                self.advance_by(1, params);
                Some(CSIAction::SetBlink(true))
            }
            &[7, _..] => {
                self.advance_by(1, params);
                Some(CSIAction::SetReverse(true))
            }
            &[8, _..] => {
                self.advance_by(1, params);
                Some(CSIAction::SetInvisible(true))
            }
            &[9, _..] => {
                self.advance_by(1, params);
                Some(CSIAction::SetStrikethrough(true))
            }
            &[21, _..] => {
                self.advance_by(1, params);
                Some(CSIAction::SetUnderline(Underline::Double))
            }
            &[22, _..] => {
                self.advance_by(1, params);
                Some(CSIAction::SetIntensity(Intensity::Normal))
            }
            &[23, _..] => {
                self.advance_by(1, params);
                Some(CSIAction::SetItalic(false))
            }
            &[24, _..] => {
                self.advance_by(1, params);
                Some(CSIAction::SetUnderline(Underline::None))
            }
            &[25, _..] => {
                self.advance_by(1, params);
                Some(CSIAction::SetBlink(false))
            }
            &[27, _..] => {
                self.advance_by(1, params);
                Some(CSIAction::SetReverse(false))
            }
            &[28, _..] => {
                self.advance_by(1, params);
                Some(CSIAction::SetInvisible(false))
            }
            &[29, _..] => {
                self.advance_by(1, params);
                Some(CSIAction::SetStrikethrough(false))
            }
            &[idx @ 30...37, _..] => {
                self.advance_by(1, params);
                Some(CSIAction::SetForegroundColor(
                    color::ColorAttribute::PaletteIndex(idx as u8 - 30),
                ))
            }
            &[39, _..] => {
                self.advance_by(1, params);
                Some(CSIAction::SetForegroundColor(
                    color::ColorAttribute::Foreground,
                ))
            }
            &[idx @ 40...47, _..] => {
                self.advance_by(1, params);
                Some(CSIAction::SetBackgroundColor(
                    color::ColorAttribute::PaletteIndex(idx as u8 - 40),
                ))
            }
            &[49, _..] => {
                self.advance_by(1, params);
                Some(CSIAction::SetBackgroundColor(
                    color::ColorAttribute::Background,
                ))
            }
            &[idx @ 90...97, _..] => {
                // Bright foreground colors
                self.advance_by(1, params);
                Some(CSIAction::SetForegroundColor(
                    color::ColorAttribute::PaletteIndex(idx as u8 - 90 + 8),
                ))
            }
            &[idx @ 100...107, _..] => {
                // Bright background colors
                self.advance_by(1, params);
                Some(CSIAction::SetBackgroundColor(
                    color::ColorAttribute::PaletteIndex(idx as u8 - 100 + 8),
                ))
            }
            _ => {
                println!("parse_sgr: unhandled csi sequence {:?}", params);
                None
            }
        }
    }
}

impl<'a> Iterator for CSIParser<'a> {
    type Item = CSIAction;

    fn next(&mut self) -> Option<CSIAction> {
        let params = self.params.take();
        match (self.byte, params) {
            (_, None) => None,
            ('H', Some(params)) => self.cup(params),
            ('K', Some(params)) => self.el(params),
            ('m', Some(params)) => self.sgr(params),
            (b, Some(p)) => {
                println!("unhandled {} {:?}", b, p);
                None
            }
        }
    }
}
