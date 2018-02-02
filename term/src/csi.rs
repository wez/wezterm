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
    SetCursorXY(i64, i64),
    DeltaCursorXY { x: i64, y: i64 },
    EraseInLine(LineErase),
    EraseInDisplay(DisplayErase),
    SetDecPrivateMode(DecPrivateMode, bool),
    DeviceStatusReport,
    ReportCursorPosition,
    SetScrollingRegion { top: i64, bottom: i64 },
    RequestDeviceAttributes,
    DeleteLines(i64),
    InsertLines(i64),
    LinePositionAbsolute(i64),
    LinePositionRelative(i64),
    SaveCursor,
    RestoreCursor,
    ScrollLines(i64),
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
    /// While params is_some we have more data to consume.  The advance_by
    /// method updates the slice as we consume data.
    /// In a number of cases an empty params list is used to indicate
    /// default values, especially for SGR, so we need to be careful not
    /// to update params to an empty slice.
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

    /// Consume some number of elements from params and update it.
    /// Take care to avoid setting params back to an empty slice
    /// as this would trigger returning a default value and/or
    /// an unterminated parse loop.
    fn advance_by(&mut self, n: usize, params: &'a [i64]) {
        let (_, next) = params.split_at(n);
        if next.len() != 0 {
            self.params = Some(next);
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

    /// DEC Private Mode (DECSET)
    fn dec_set_mode(&mut self, params: &'a [i64]) -> Option<CSIAction> {
        match params {
            &[idx, _..] => {
                self.advance_by(1, params);
                self.parse_dec_mode(idx).map(|m| {
                    CSIAction::SetDecPrivateMode(m, true)
                })
            }
            _ => {
                println!(
                    "dec_set_mode: unhandled sequence {:?} {:?}",
                    self.intermediates,
                    params
                );
                None
            }
        }
    }

    /// Reset DEC Private Mode (DECRST)
    fn dec_reset_mode(&mut self, params: &'a [i64]) -> Option<CSIAction> {
        match params {
            &[idx, _..] => {
                self.advance_by(1, params);
                self.parse_dec_mode(idx).map(|m| {
                    CSIAction::SetDecPrivateMode(m, false)
                })
            }
            _ => {
                println!("dec_reset_mode: unhandled sequence {:?}", params);
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

    fn set_scroll_region(&mut self, params: &'a [i64]) -> Option<CSIAction> {
        match params {
            &[top, bottom] => {
                self.advance_by(2, params);
                Some(CSIAction::SetScrollingRegion {
                    top: top - 1,
                    bottom: bottom - 1,
                })
            }
            _ => {
                println!("set_scroll_region: invalid sequence: {:?}", params);
                None
            }
        }
    }
}

impl<'a> Iterator for CSIParser<'a> {
    type Item = CSIAction;

    fn next(&mut self) -> Option<CSIAction> {
        let params = self.params.take();
        match (self.byte, self.intermediates, params) {
            (_, _, None) => None,
            // CUU - Cursor Up n times
            ('A', &[], Some(&[])) => Some(CSIAction::DeltaCursorXY { x: 0, y: -1 }),
            ('A', &[], Some(&[y])) => Some(CSIAction::DeltaCursorXY { x: 0, y: -y }),

            // CUD - Cursor Down n times
            ('B', &[], Some(&[])) => Some(CSIAction::DeltaCursorXY { x: 0, y: 1 }),
            ('B', &[], Some(&[y])) => Some(CSIAction::DeltaCursorXY { x: 0, y: y }),

            // CUF - Cursor n forward
            ('C', &[], Some(&[])) => Some(CSIAction::DeltaCursorXY { x: 1, y: 0 }),
            ('C', &[], Some(&[x])) => Some(CSIAction::DeltaCursorXY { x: x, y: 0 }),

            // CUB - Cursor n backward
            ('D', &[], Some(&[])) => Some(CSIAction::DeltaCursorXY { x: -1, y: 0 }),
            ('D', &[], Some(&[x])) => Some(CSIAction::DeltaCursorXY { x: -x, y: 0 }),

            // Cursor Position (CUP)
            ('H', &[], Some(&[])) => Some(CSIAction::SetCursorXY(0, 0)),
            ('H', &[], Some(&[y, x])) => {
                // Co-ordinates are 1-based, but we want 0-based
                Some(CSIAction::SetCursorXY(x.max(1) - 1, y.max(1) - 1))
            }

            // Erase in Display (ED)
            ('J', &[], Some(&[])) |
            ('J', &[], Some(&[0])) => Some(CSIAction::EraseInDisplay(DisplayErase::Below)),
            ('J', &[], Some(&[1])) => Some(CSIAction::EraseInDisplay(DisplayErase::Above)),
            ('J', &[], Some(&[2])) => Some(CSIAction::EraseInDisplay(DisplayErase::All)),
            ('J', &[], Some(&[3])) => Some(CSIAction::EraseInDisplay(DisplayErase::SavedLines)),

            // Erase in Line (EL)
            ('K', &[], Some(&[])) |
            ('K', &[], Some(&[0])) => Some(CSIAction::EraseInLine(LineErase::ToRight)),
            ('K', &[], Some(&[1])) => Some(CSIAction::EraseInLine(LineErase::ToLeft)),
            ('K', &[], Some(&[2])) => Some(CSIAction::EraseInLine(LineErase::All)),

            // Insert Liness (IL)
            ('L', &[], Some(&[])) => Some(CSIAction::InsertLines(1)),
            ('L', &[], Some(&[n])) => Some(CSIAction::InsertLines(n)),

            // Delete Liness (DL)
            ('M', &[], Some(&[])) => Some(CSIAction::DeleteLines(1)),
            ('M', &[], Some(&[n])) => Some(CSIAction::DeleteLines(n)),

            // SU: Scroll Up Lines
            ('S', &[], Some(&[])) => Some(CSIAction::ScrollLines(-1)),
            ('S', &[], Some(&[n])) => Some(CSIAction::ScrollLines(-n)),

            // HPR - Character position Relative
            ('a', &[], Some(&[])) => Some(CSIAction::DeltaCursorXY { x: 1, y: 0 }),
            ('a', &[], Some(&[x])) => Some(CSIAction::DeltaCursorXY { x: x, y: 0 }),

            ('c', &[b'>'], Some(&[])) |
            ('c', &[], Some(&[])) |
            ('c', &[], Some(&[0])) |
            ('c', &[b'>'], Some(&[0])) => Some(CSIAction::RequestDeviceAttributes),

            // VPA: Line Position Absolute
            ('d', &[], Some(&[])) => Some(CSIAction::LinePositionAbsolute(0)),
            ('d', &[], Some(&[n])) => Some(CSIAction::LinePositionAbsolute(n)),

            // VPR: Line Position Relative
            ('e', &[], Some(&[])) => Some(CSIAction::LinePositionRelative(0)),
            ('e', &[], Some(&[n])) => Some(CSIAction::LinePositionRelative(n)),

            ('h', &[b'?'], Some(params)) => self.dec_set_mode(params),
            ('l', &[b'?'], Some(params)) => self.dec_reset_mode(params),
            ('m', &[], Some(params)) => self.sgr(params),
            ('n', &[], Some(params)) => self.dsr(params),
            ('r', &[], Some(params)) => self.set_scroll_region(params),

            // SCOSC: Save Cursor
            ('s', &[], Some(&[])) => Some(CSIAction::SaveCursor),
            // SCORC: Restore Cursor
            ('u', &[], Some(&[])) => Some(CSIAction::RestoreCursor),

            (b, i, Some(p)) => {
                println!("cSI unhandled {} {:?} {:?} ignore={}", b, p, i, self.ignore);
                None
            }
        }
    }
}
