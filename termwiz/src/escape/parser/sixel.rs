use crate::color::RgbColor;
use crate::escape::{Sixel, SixelData};
use regex::bytes::Regex;

pub struct SixelBuilder {
    pub sixel: Sixel,
    buf: Vec<u8>,
    repeat_re: Regex,
    raster_re: Regex,
    colordef_re: Regex,
    coloruse_re: Regex,
}

impl SixelBuilder {
    pub fn new(params: &[i64]) -> Self {
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
        let colordef_re = Regex::new("^#(\\d+);(\\d+);(\\d+);(\\d+);(\\d+);?").unwrap();
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

    pub fn push(&mut self, data: u8) {
        self.buf.push(data);
    }

    pub fn finish(&mut self) {
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
                    let size = w as usize * h as usize;
                    // Ideally we'd just use `try_reserve` here, but that is
                    // nightly Rust only at the time of writing this comment:
                    // <https://github.com/rust-lang/rust/issues/48043>
                    const MAX_SIXEL_SIZE: usize = 100_000_000;
                    if size > MAX_SIXEL_SIZE {
                        log::error!(
                            "Ignoring sixel data {}x{} because {} bytes > max allowed {}",
                            w,
                            h,
                            size,
                            MAX_SIXEL_SIZE
                        );
                        self.sixel.pixel_width = None;
                        self.sixel.pixel_height = None;
                        self.sixel.data.clear();
                        return;
                    }
                    self.sixel.data.reserve(size);
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
                    let rgb = RgbColor::new_8bpc(r as u8, g as u8, b as u8); // FIXME: from linear
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
    use crate::escape::parser::Parser;
    use crate::escape::{Action, Esc, EscCode};
    use pretty_assertions::assert_eq;

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
                            rgb: RgbColor::new_8bpc(0, 0, 0)
                        },
                        DefineColorMapRGB {
                            color_number: 1,
                            rgb: RgbColor::new_8bpc(255, 255, 0)
                        },
                        DefineColorMapRGB {
                            color_number: 2,
                            rgb: RgbColor::new_8bpc(0, 255, 0)
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
