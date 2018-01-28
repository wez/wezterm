//! Parsing CSI escape sequences

use super::*;

#[derive(Debug)]
pub enum LineErase {
    ToRight,
    ToLeft,
    All,
}

#[derive(Debug)]
pub enum DisplayErase {
    Below,
    Above,
    All,
    SavedLines,
}

#[derive(Debug)]
pub enum DecPrivateMode {
    ApplicationCursorKeys,
    BrackedPaste,
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
    EraseInDisplay(DisplayErase),
    SetDecPrivateMode(DecPrivateMode, bool),
    DeviceStatusReport,
    ReportCursorPosition,
}

/// Constrol Sequence Initiator (CSI) Parser.
/// Since many sequences allow for composition of actions by separating
/// parameters using the ; character, we need to be able to iterate over
/// the set of parsed actions from a given CSI sequence.
/// CSIParser implements an Iterator that yields CSIAction instances as
/// it parses them out from the input sequence.
pub struct CSIParser<'a> {
    intermediates: &'a [u8],
    /// From vte::Perform: this flag is set when more than two intermediates
    /// arrived and subsequent characters were ignored.
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

    /// Erase in Display (ED)
    fn ed(&mut self, params: &'a [i64]) -> Option<CSIAction> {
        match params {
            &[] => Some(CSIAction::EraseInDisplay(DisplayErase::Below)),
            &[i] => {
                self.advance_by(1, params);
                match i {
                    0 => Some(CSIAction::EraseInDisplay(DisplayErase::Below)),
                    1 => Some(CSIAction::EraseInDisplay(DisplayErase::Above)),
                    2 => Some(CSIAction::EraseInDisplay(DisplayErase::All)),
                    3 => Some(CSIAction::EraseInDisplay(DisplayErase::SavedLines)),
                    _ => {
                        println!("ed: unknown parameter {:?}", params);
                        None
                    }
                }
            }
            _ => {
                println!("ed: unhandled csi sequence {:?}", params);
                None
            }
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

    /// Device status report
    fn dsr(&mut self, params: &'a [i64]) -> Option<CSIAction> {
        match (self.intermediates, params) {
            (&[], &[5, _..]) => {
                self.advance_by(1, params);
                Some(CSIAction::DeviceStatusReport)
            }
            (&[], &[6, _..]) => {
                self.advance_by(1, params);
                Some(CSIAction::ReportCursorPosition)
            }
            _ => {
                println!(
                    "dsr: unhandled sequence {:?} {:?}",
                    self.intermediates,
                    params
                );
                None
            }
        }
    }

    fn parse_dec_mode(&self, mode: i64) -> Option<DecPrivateMode> {
        match mode {
            1 => Some(DecPrivateMode::ApplicationCursorKeys),
            2004 => Some(DecPrivateMode::BrackedPaste),
            _ => None,
        }
    }

    /// Set Mode (SM) and DEC Private Mode (DECSET)
    fn set_mode(&mut self, params: &'a [i64]) -> Option<CSIAction> {
        match (self.intermediates, params) {
            (&[b'?'], &[idx, _..]) => {
                self.advance_by(1, params);
                self.parse_dec_mode(idx).map(|m| {
                    CSIAction::SetDecPrivateMode(m, true)
                })
            }
            _ => {
                println!(
                    "set_mode: unhandled sequence {:?} {:?}",
                    self.intermediates,
                    params
                );
                None
            }
        }
    }

    /// Reset Mode (RM) and DEC Private Mode (DECRST)
    fn reset_mode(&mut self, params: &'a [i64]) -> Option<CSIAction> {
        match (self.intermediates, params) {
            (&[b'?'], &[idx, _..]) => {
                self.advance_by(1, params);
                self.parse_dec_mode(idx).map(|m| {
                    CSIAction::SetDecPrivateMode(m, false)
                })
            }
            _ => {
                println!(
                    "reset_mode: unhandled sequence {:?} {:?}",
                    self.intermediates,
                    params
                );
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
            &[38, 2, _colorspace, red, green, blue, _..] => {
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
            &[38, 2, red, green, blue, _..] => {
                // KDE konsole compatibility for truecolor foreground
                self.advance_by(5, params);
                Some(CSIAction::SetForegroundColor(
                    color::ColorAttribute::Rgb(color::RgbColor {
                        red: red as u8,
                        green: green as u8,
                        blue: blue as u8,
                    }),
                ))
            }
            &[48, 2, _colorspace, red, green, blue, _..] => {
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
            &[48, 2, red, green, blue, _..] => {
                // KDE konsole compatibility for truecolor background
                self.advance_by(5, params);
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
            ('h', Some(params)) => self.set_mode(params),
            ('l', Some(params)) => self.reset_mode(params),
            ('H', Some(params)) => self.cup(params),
            ('J', Some(params)) => self.ed(params),
            ('K', Some(params)) => self.el(params),
            ('m', Some(params)) => self.sgr(params),
            ('n', Some(params)) => self.dsr(params),
            (b, Some(p)) => {
                println!(
                    "unhandled {} {:?} {:?} ignore={}",
                    b,
                    p,
                    self.intermediates,
                    self.ignore
                );
                None
            }
        }
    }
}
