use crate::color::RgbColor;
use crate::escape::{
    Action, DeviceControlMode, EnterDeviceControlMode, Esc, OperatingSystemCommand,
    ShortDeviceControl, Sixel, SixelData, CSI,
};
use log::error;
use num_traits::FromPrimitive;
use regex::bytes::Regex;
use std::cell::RefCell;
use vtparse::{VTActor, VTParser};

struct SixelBuilder {
    sixel: Sixel,
    buf: Vec<u8>,
    repeat_re: Regex,
    raster_re: Regex,
    colordef_re: Regex,
    coloruse_re: Regex,
}

#[derive(Default)]
struct ParseState {
    sixel: Option<SixelBuilder>,
    dcs: Option<ShortDeviceControl>,
}

/// The `Parser` struct holds the state machine that is used to decode
/// a sequence of bytes.  The byte sequence can be streaming into the
/// state machine.
/// You can either have the parser trigger a callback as `Action`s are
/// decoded, or have it return a `Vec<Action>` holding zero-or-more
/// decoded actions.
pub struct Parser {
    state_machine: VTParser,
    state: RefCell<ParseState>,
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

impl Parser {
    pub fn new() -> Self {
        Self {
            state_machine: VTParser::new(),
            state: RefCell::new(Default::default()),
        }
    }

    pub fn parse<F: FnMut(Action)>(&mut self, bytes: &[u8], mut callback: F) {
        let mut perform = Performer {
            callback: &mut callback,
            state: &mut self.state.borrow_mut(),
        };
        self.state_machine.parse(bytes, &mut perform);
    }

    /// A specialized version of the parser that halts after recognizing the
    /// first action from the stream of bytes.  The return value is the action
    /// that was recognized and the length of the byte stream that was fed in
    /// to the parser to yield it.
    pub fn parse_first(&mut self, bytes: &[u8]) -> Option<(Action, usize)> {
        // holds the first action.  We need to use RefCell to deal with
        // the Performer holding a reference to this via the closure we set up.
        let first = RefCell::new(None);
        // will hold the iterator index when we emit an action
        let mut first_idx = None;
        {
            let mut perform = Performer {
                callback: &mut |action| {
                    // capture the action, but only if it is the first one
                    // we've seen.  Preserve an existing one if any.
                    if first.borrow().is_some() {
                        return;
                    }
                    *first.borrow_mut() = Some(action);
                },
                state: &mut self.state.borrow_mut(),
            };
            for (idx, b) in bytes.iter().enumerate() {
                self.state_machine.parse_byte(*b, &mut perform);
                if first.borrow().is_some() {
                    // if we recognized an action, record the iterator index
                    first_idx = Some(idx);
                    break;
                }
            }
        }

        match (first.into_inner(), first_idx) {
            // if we matched an action, transform the iterator index to
            // the length of the string that was consumed (+1)
            (Some(action), Some(idx)) => Some((action, idx + 1)),
            _ => None,
        }
    }

    pub fn parse_as_vec(&mut self, bytes: &[u8]) -> Vec<Action> {
        let mut result = Vec::new();
        self.parse(bytes, |action| result.push(action));
        result
    }

    /// Similar to `parse_first` but collects all actions from the first sequence.
    pub fn parse_first_as_vec(&mut self, bytes: &[u8]) -> Option<(Vec<Action>, usize)> {
        let mut actions = Vec::new();
        let mut first_idx = None;
        for (idx, b) in bytes.iter().enumerate() {
            self.state_machine.parse_byte(
                *b,
                &mut Performer {
                    callback: &mut |action| actions.push(action),
                    state: &mut self.state.borrow_mut(),
                },
            );
            if !actions.is_empty() {
                // if we recognized any actions, record the iterator index
                first_idx = Some(idx);
                break;
            }
        }
        first_idx.map(|idx| (actions, idx + 1))
    }
}

struct Performer<'a, F: FnMut(Action) + 'a> {
    callback: &'a mut F,
    state: &'a mut ParseState,
}

fn is_short_dcs(intermediates: &[u8], byte: u8) -> bool {
    if intermediates == &[b'$'] && byte == b'q' {
        // DECRQSS
        true
    } else {
        false
    }
}

impl<'a, F: FnMut(Action)> VTActor for Performer<'a, F> {
    fn print(&mut self, c: char) {
        (self.callback)(Action::Print(c));
    }

    fn execute_c0_or_c1(&mut self, byte: u8) {
        match FromPrimitive::from_u8(byte) {
            Some(code) => (self.callback)(Action::Control(code)),
            None => error!(
                "impossible C0/C1 control code {:?} 0x{:x} was dropped",
                byte as char, byte
            ),
        }
    }

    fn dcs_hook(
        &mut self,
        byte: u8,
        params: &[i64],
        intermediates: &[u8],
        ignored_extra_intermediates: bool,
    ) {
        if byte == b'q' && intermediates.is_empty() && !ignored_extra_intermediates {
            self.state.sixel.replace(SixelBuilder::new(params));
        } else if !ignored_extra_intermediates && is_short_dcs(intermediates, byte) {
            self.state.sixel.take();
            self.state.dcs.replace(ShortDeviceControl {
                params: params.to_vec(),
                intermediates: intermediates.to_vec(),
                byte,
                data: vec![],
            });
        } else {
            (self.callback)(Action::DeviceControl(DeviceControlMode::Enter(Box::new(
                EnterDeviceControlMode {
                    byte,
                    params: params.to_vec(),
                    intermediates: intermediates.to_vec(),
                    ignored_extra_intermediates,
                },
            ))));
        }
    }

    fn dcs_put(&mut self, data: u8) {
        if let Some(dcs) = self.state.dcs.as_mut() {
            dcs.data.push(data);
        } else if let Some(sixel) = self.state.sixel.as_mut() {
            sixel.push(data);
        } else {
            (self.callback)(Action::DeviceControl(DeviceControlMode::Data(data)));
        }
    }

    fn dcs_unhook(&mut self) {
        if let Some(dcs) = self.state.dcs.take() {
            (self.callback)(Action::DeviceControl(
                DeviceControlMode::ShortDeviceControl(Box::new(dcs)),
            ));
        } else if let Some(mut sixel) = self.state.sixel.take() {
            sixel.finish();
            (self.callback)(Action::Sixel(Box::new(sixel.sixel)));
        } else {
            (self.callback)(Action::DeviceControl(DeviceControlMode::Exit));
        }
    }

    fn osc_dispatch(&mut self, osc: &[&[u8]]) {
        let osc = OperatingSystemCommand::parse(osc);
        (self.callback)(Action::OperatingSystemCommand(Box::new(osc)));
    }

    fn csi_dispatch(
        &mut self,
        params: &[i64],
        intermediates: &[u8],
        ignored_extra_intermediates: bool,
        control: u8,
    ) {
        for action in CSI::parse(
            params,
            intermediates,
            ignored_extra_intermediates,
            control as char,
        ) {
            (self.callback)(Action::CSI(action));
        }
    }

    fn esc_dispatch(
        &mut self,
        _params: &[i64],
        intermediates: &[u8],
        _ignored_extra_intermediates: bool,
        control: u8,
    ) {
        // It doesn't appear to be possible for params.len() > 1 due to the way
        // that the state machine in vte functions.  As such, it also seems to
        // be impossible for ignored_extra_intermediates to be true too.
        (self.callback)(Action::Esc(Esc::parse(
            if intermediates.len() == 1 {
                Some(intermediates[0])
            } else {
                None
            },
            control,
        )));
    }
}

impl SixelBuilder {
    fn new(params: &[i64]) -> Self {
        let pan = match params.get(0).unwrap_or(&0) {
            7 | 8 | 9 => 1,
            0 | 1 | 5 | 6 => 2,
            3 | 4 => 3,
            2 => 5,
            _ => 2,
        };
        let background_is_transparent = match params.get(1).unwrap_or(&0) {
            1 => true,
            _ => false,
        };
        let horizontal_grid_size = params.get(2).map(|&x| x);

        let repeat_re = Regex::new("^!(\\d+)([\x3f-\x7e])").unwrap();
        let raster_re = Regex::new("^\"(\\d+);(\\d+)(;(\\d+))?(;(\\d+))?").unwrap();
        let colordef_re = Regex::new("^#(\\d+);(\\d+);(\\d+);(\\d+);(\\d+)").unwrap();
        let coloruse_re = Regex::new("^#(\\d+)([^;\\d]|$)").unwrap();

        Self {
            sixel: Sixel {
                pan,
                pad: 1,
                pixel_width: None,
                pixel_height: None,
                background_is_transparent,
                horizontal_grid_size,
                data: vec![],
            },
            buf: vec![],
            repeat_re,
            raster_re,
            colordef_re,
            coloruse_re,
        }
    }

    fn push(&mut self, data: u8) {
        self.buf.push(data);
    }

    fn finish(&mut self) {
        fn cap_int<T: std::str::FromStr>(m: regex::bytes::Match) -> Option<T> {
            let bytes = m.as_bytes();
            // Safe because we matched digits from the regex
            let s = unsafe { std::str::from_utf8_unchecked(bytes) };
            s.parse::<T>().ok()
        }

        let mut remainder = &self.buf[..];

        while !remainder.is_empty() {
            let data = remainder[0];

            if data == b'$' {
                self.sixel.data.push(SixelData::CarriageReturn);
                remainder = &remainder[1..];
                continue;
            }

            if data == b'-' {
                self.sixel.data.push(SixelData::NewLine);
                remainder = &remainder[1..];
                continue;
            }

            if data >= 0x3f && data <= 0x7e {
                self.sixel.data.push(SixelData::Data(data - 0x3f));
                remainder = &remainder[1..];
                continue;
            }

            if let Some(c) = self.raster_re.captures(remainder) {
                let all = c.get(0).unwrap();
                let matched_len = all.as_bytes().len();

                let pan = cap_int(c.get(1).unwrap()).unwrap_or(2);
                let pad = cap_int(c.get(2).unwrap()).unwrap_or(1);
                let pixel_width = c.get(4).and_then(cap_int);
                let pixel_height = c.get(6).and_then(cap_int);

                self.sixel.pan = pan;
                self.sixel.pad = pad;
                self.sixel.pixel_width = pixel_width;
                self.sixel.pixel_height = pixel_height;

                if let (Some(w), Some(h)) = (pixel_width, pixel_height) {
                    self.sixel.data.reserve(w as usize * h as usize);
                }

                remainder = &remainder[matched_len..];
                continue;
            }

            if let Some(c) = self.coloruse_re.captures(remainder) {
                let all = c.get(0).unwrap();
                let matched_len = all.as_bytes().len();

                let color_number = cap_int(c.get(1).unwrap()).unwrap_or(0);

                self.sixel
                    .data
                    .push(SixelData::SelectColorMapEntry(color_number));

                let pop_len = matched_len - c.get(2).unwrap().as_bytes().len();

                remainder = &remainder[pop_len..];
                continue;
            }

            if let Some(c) = self.colordef_re.captures(remainder) {
                let all = c.get(0).unwrap();
                let matched_len = all.as_bytes().len();

                let color_number = cap_int(c.get(1).unwrap()).unwrap_or(0);
                let system = cap_int(c.get(2).unwrap()).unwrap_or(1);
                let a = cap_int(c.get(3).unwrap()).unwrap_or(0);
                let b = cap_int(c.get(4).unwrap()).unwrap_or(0);
                let c = cap_int(c.get(5).unwrap()).unwrap_or(0);

                if system == 1 {
                    self.sixel.data.push(SixelData::DefineColorMapHSL {
                        color_number,
                        hue_angle: a,
                        lightness: b,
                        saturation: c,
                    });
                } else {
                    let r = a as f32 * 255.0 / 100.;
                    let g = b as f32 * 255.0 / 100.;
                    let b = c as f32 * 255.0 / 100.;
                    let rgb = RgbColor::new(r as u8, g as u8, b as u8); // FIXME: from linear
                    self.sixel
                        .data
                        .push(SixelData::DefineColorMapRGB { color_number, rgb });
                }

                remainder = &remainder[matched_len..];
                continue;
            }

            if let Some(c) = self.repeat_re.captures(remainder) {
                let all = c.get(0).unwrap();
                let matched_len = all.as_bytes().len();

                let repeat_count = cap_int(c.get(1).unwrap()).unwrap_or(1);
                let data = c.get(2).unwrap().as_bytes()[0] - 0x3f;
                self.sixel
                    .data
                    .push(SixelData::Repeat { repeat_count, data });
                remainder = &remainder[matched_len..];
                continue;
            }

            log::error!(
                "finished sixel parse with {} bytes pending {:?}",
                remainder.len(),
                std::str::from_utf8(&remainder[0..24.min(remainder.len())])
            );

            break;
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::cell::{Intensity, Underline};
    use crate::escape::csi::Sgr;
    use crate::escape::EscCode;
    use std::io::Write;

    fn encode(seq: &Vec<Action>) -> String {
        let mut res = Vec::new();
        for s in seq {
            write!(res, "{}", s).unwrap();
        }
        String::from_utf8(res).unwrap()
    }

    #[test]
    fn basic_parse() {
        let mut p = Parser::new();
        let actions = p.parse_as_vec(b"hello");
        assert_eq!(
            vec![
                Action::Print('h'),
                Action::Print('e'),
                Action::Print('l'),
                Action::Print('l'),
                Action::Print('o'),
            ],
            actions
        );
        assert_eq!(encode(&actions), "hello");
    }

    #[test]
    fn basic_bold() {
        let mut p = Parser::new();
        let actions = p.parse_as_vec(b"\x1b[1mb");
        assert_eq!(
            vec![
                Action::CSI(CSI::Sgr(Sgr::Intensity(Intensity::Bold))),
                Action::Print('b'),
            ],
            actions
        );
        assert_eq!(encode(&actions), "\x1b[1mb");
    }

    #[test]
    fn basic_bold_italic() {
        let mut p = Parser::new();
        let actions = p.parse_as_vec(b"\x1b[1;3mb");
        assert_eq!(
            vec![
                Action::CSI(CSI::Sgr(Sgr::Intensity(Intensity::Bold))),
                Action::CSI(CSI::Sgr(Sgr::Italic(true))),
                Action::Print('b'),
            ],
            actions
        );

        assert_eq!(encode(&actions), "\x1b[1m\x1b[3mb");
    }

    #[test]
    fn fancy_underline() {
        let mut p = Parser::new();

        // Kitty underline sequences use a `:` which is explicitly invalid
        // and deleted by the dec/ansi vtparser
        let actions = p.parse_as_vec(b"\x1b[4:0mb");
        assert_eq!(
            vec![
                // NO: Action::CSI(CSI::Sgr(Sgr::Underline(Underline::None))),
                Action::Print('b'),
            ],
            actions
        );

        let actions = p.parse_as_vec(b"\x1b[60;61;62mb");
        assert_eq!(
            vec![
                Action::CSI(CSI::Sgr(Sgr::Underline(Underline::Curly))),
                Action::CSI(CSI::Sgr(Sgr::Underline(Underline::Dotted))),
                Action::CSI(CSI::Sgr(Sgr::Underline(Underline::Dashed))),
                Action::Print('b'),
            ],
            actions
        );

        assert_eq!(encode(&actions), "\x1b[60m\x1b[61m\x1b[62mb");
    }

    #[test]
    fn basic_osc() {
        let mut p = Parser::new();
        let actions = p.parse_as_vec(b"\x1b]0;hello\x07");
        assert_eq!(
            vec![Action::OperatingSystemCommand(Box::new(
                OperatingSystemCommand::SetIconNameAndWindowTitle("hello".to_owned()),
            ))],
            actions
        );
        assert_eq!(encode(&actions), "\x1b]0;hello\x1b\\");

        let actions = p.parse_as_vec(b"\x1b]532534523;hello\x07");
        assert_eq!(
            vec![Action::OperatingSystemCommand(Box::new(
                OperatingSystemCommand::Unspecified(vec![b"532534523".to_vec(), b"hello".to_vec()]),
            ))],
            actions
        );
        assert_eq!(encode(&actions), "\x1b]532534523;hello\x1b\\");
    }

    #[test]
    fn test_emoji_title_osc() {
        let input = "\x1b]0;\u{1f915}\x07";
        let mut p = Parser::new();
        let actions = p.parse_as_vec(input.as_bytes());
        assert_eq!(
            vec![Action::OperatingSystemCommand(Box::new(
                OperatingSystemCommand::SetIconNameAndWindowTitle("\u{1f915}".to_owned()),
            ))],
            actions
        );
        assert_eq!(encode(&actions), "\x1b]0;\u{1f915}\x1b\\");
    }

    #[test]
    fn basic_esc() {
        let mut p = Parser::new();
        let actions = p.parse_as_vec(b"\x1bH");
        assert_eq!(
            vec![Action::Esc(Esc::Code(EscCode::HorizontalTabSet))],
            actions
        );
        assert_eq!(encode(&actions), "\x1bH");

        let actions = p.parse_as_vec(b"\x1b%H");
        assert_eq!(
            vec![Action::Esc(Esc::Unspecified {
                intermediate: Some(b'%'),
                control: b'H',
            })],
            actions
        );
        assert_eq!(encode(&actions), "\x1b%H");
    }

    #[test]
    fn sixel() {
        let mut p = Parser::new();
        let actions = p.parse_as_vec(b"\x1bP1;2;3;q@\x1b\\");
        assert_eq!(
            vec![
                Action::Sixel(Box::new(Sixel {
                    pan: 2,
                    pad: 1,
                    pixel_width: None,
                    pixel_height: None,
                    background_is_transparent: false,
                    horizontal_grid_size: Some(3),
                    data: vec![SixelData::Data(1)]
                })),
                Action::Esc(Esc::Code(EscCode::StringTerminator)),
            ],
            actions
        );

        assert_eq!(format!("{}", actions[0]), "\x1bP0;0;3q@");

        // This is the "HI" example from wikipedia
        let mut p = Parser::new();
        let actions = p.parse_as_vec(
            b"\x1bPq\
        #0;2;0;0;0#1;2;100;100;0#2;2;0;100;0\
        #1~~@@vv@@~~@@~~$\
        #2??}}GG}}??}}??-\
        #1!14@\
        \x1b\\",
        );

        assert_eq!(
            format!("{}", actions[0]),
            "\x1bP0;0q\
        #0;2;0;0;0#1;2;100;100;0#2;2;0;100;0\
        #1~~@@vv@@~~@@~~$\
        #2??}}GG}}??}}??-\
        #1!14@"
        );

        use SixelData::*;
        assert_eq!(
            vec![
                Action::Sixel(Box::new(Sixel {
                    pan: 2,
                    pad: 1,
                    pixel_width: None,
                    pixel_height: None,
                    background_is_transparent: false,
                    horizontal_grid_size: None,
                    data: vec![
                        DefineColorMapRGB {
                            color_number: 0,
                            rgb: RgbColor {
                                red: 0,
                                green: 0,
                                blue: 0
                            }
                        },
                        DefineColorMapRGB {
                            color_number: 1,
                            rgb: RgbColor {
                                red: 255,
                                green: 255,
                                blue: 0
                            }
                        },
                        DefineColorMapRGB {
                            color_number: 2,
                            rgb: RgbColor {
                                red: 0,
                                green: 255,
                                blue: 0
                            }
                        },
                        SelectColorMapEntry(1),
                        Data(63),
                        Data(63),
                        Data(1),
                        Data(1),
                        Data(55),
                        Data(55),
                        Data(1),
                        Data(1),
                        Data(63),
                        Data(63),
                        Data(1),
                        Data(1),
                        Data(63),
                        Data(63),
                        CarriageReturn,
                        SelectColorMapEntry(2),
                        Data(0),
                        Data(0),
                        Data(62),
                        Data(62),
                        Data(8),
                        Data(8),
                        Data(62),
                        Data(62),
                        Data(0),
                        Data(0),
                        Data(62),
                        Data(62),
                        Data(0),
                        Data(0),
                        NewLine,
                        SelectColorMapEntry(1),
                        Repeat {
                            repeat_count: 14,
                            data: 1
                        }
                    ]
                })),
                Action::Esc(Esc::Code(EscCode::StringTerminator)),
            ],
            actions
        );
    }
}
