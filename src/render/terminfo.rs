//! Rendering of Changes using terminfo
use caps::{Capabilities, ColorLevel};
use cell::{AttributeChange, Blink, CellAttributes, Intensity, Underline};
use color::ColorSpec;
use escape::csi::{Cursor, Sgr, CSI};
use escape::osc::OperatingSystemCommand;
use escape::EncodeEscape;
use failure;
use render::Renderer;
use screen::{Change, Position};
use std::io::{Error as IoError, Write};
use terminfo::{capability as cap, Capability as TermInfoCapability};

pub struct TerminfoRenderer {
    caps: Capabilities,
}

impl TerminfoRenderer {
    pub fn new(caps: Capabilities) -> Self {
        Self { caps }
    }

    fn get_capability<'a, T: TermInfoCapability<'a>>(&'a self) -> Option<T> {
        self.caps.terminfo_db().and_then(|db| db.get::<T>())
    }
}

// The terminfo crate wants us to move the Write instance in on
// each call.  This is a little struct that allows us to do that
// without moving out the actual Write ref.
struct WriteWrapper<'a> {
    out: &'a mut Write,
}

impl<'a> WriteWrapper<'a> {
    fn new(out: &'a mut Write) -> Self {
        Self { out }
    }
}

impl<'a> Write for WriteWrapper<'a> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, IoError> {
        self.out.write(buf)
    }

    fn flush(&mut self) -> Result<(), IoError> {
        self.out.flush()
    }
}

impl Renderer for TerminfoRenderer {
    fn render_to(
        &self,
        start_attr: &CellAttributes,
        changes: &[Change],
        out: &mut Write,
    ) -> Result<CellAttributes, failure::Error> {
        let mut current_attr = start_attr.clone();
        let mut pending_attr: Option<CellAttributes> = None;

        macro_rules! attr_apply {
            ($apply:expr) => {
                pending_attr = Some(match pending_attr {
                    Some(mut attr) => {
                        $apply(&mut attr);
                        attr
                    }
                    None => {
                        let mut attr = current_attr.clone();
                        $apply(&mut attr);
                        attr
                    }
                });
            };
        }
        macro_rules! record {
            ($accesor:ident, $value:expr) => {
                attr_apply!(|attr: &mut CellAttributes| {
                    attr.$accesor(*$value);
                });
            };
        }

        macro_rules! attr_on_off {
            ($cap_on:ident, $cap_off:ident, $attr:expr, $accesor:ident, $sgr:ident) => {
                let value = $attr.$accesor();
                if value != current_attr.$accesor() {
                    let mut out = WriteWrapper::new(out);
                    let on: bool = value.into();
                    if on {
                        if let Some(attr) = self.get_capability::<cap::$cap_on>() {
                            attr.expand().to(out)?;
                        } else {
                            CSI::Sgr(Sgr::$sgr(value)).encode_escape(&mut out)?;
                        }
                    } else {
                        if let Some(attr) = self.get_capability::<cap::$cap_off>() {
                            attr.expand().to(out)?;
                        } else {
                            CSI::Sgr(Sgr::$sgr(value)).encode_escape(&mut out)?;
                        }
                    }
                }
            };
        }

        macro_rules! flush_pending_attr {
            () => {
                if let Some(attr) = pending_attr.take() {
                    if !attr.attribute_bits_equal(&current_attr) {
                        if let Some(sgr) = self.get_capability::<cap::SetAttributes>() {
                            sgr.expand()
                                .bold(attr.intensity() == Intensity::Bold)
                                .dim(attr.intensity() == Intensity::Half)
                                .underline(attr.underline() != Underline::None)
                                .blink(attr.blink() != Blink::None)
                                .reverse(attr.reverse())
                                .invisible(attr.invisible())
                                .to(WriteWrapper::new(out))?;
                        } else {
                            if let Some(exit) = self.get_capability::<cap::ExitAttributeMode>()
                            {
                                exit.expand().to(WriteWrapper::new(out))?;
                            } else {
                                CSI::Sgr(Sgr::Reset)
                                    .encode_escape(&mut WriteWrapper::new(out))?;
                            }

                            if attr.intensity() != current_attr.intensity() {
                                match attr.intensity() {
                                    Intensity::Bold => if let Some(bold) =
                                        self.get_capability::<cap::EnterBoldMode>()
                                    {
                                        bold.expand().to(WriteWrapper::new(out))?;
                                    } else {
                                        CSI::Sgr(Sgr::Intensity(attr.intensity()))
                                            .encode_escape(&mut WriteWrapper::new(out))?;
                                    },
                                    Intensity::Half => if let Some(dim) =
                                        self.get_capability::<cap::EnterDimMode>()
                                    {
                                        dim.expand().to(WriteWrapper::new(out))?;
                                    } else {
                                        CSI::Sgr(Sgr::Intensity(attr.intensity()))
                                            .encode_escape(&mut WriteWrapper::new(out))?;
                                    },
                                    _ => {}
                                }
                            }

                            attr_on_off!(
                                EnterUnderlineMode,
                                ExitUnderlineMode,
                                attr,
                                underline,
                                Underline
                            );

                            attr_on_off!(
                                EnterUnderlineMode,
                                ExitUnderlineMode,
                                attr,
                                underline,
                                Underline
                            );

                            if attr.blink() != current_attr.blink() {
                                if let Some(attr) = self.get_capability::<cap::EnterBlinkMode>()
                                {
                                    attr.expand().to(WriteWrapper::new(out))?;
                                } else {
                                    CSI::Sgr(Sgr::Blink(attr.blink()))
                                        .encode_escape(&mut WriteWrapper::new(out))?;
                                }
                            }

                            if attr.reverse() != current_attr.reverse() {
                                if let Some(attr) =
                                    self.get_capability::<cap::EnterReverseMode>()
                                {
                                    attr.expand().to(WriteWrapper::new(out))?;
                                } else {
                                    CSI::Sgr(Sgr::Inverse(attr.reverse()))
                                        .encode_escape(&mut WriteWrapper::new(out))?;
                                }
                            }

                            if attr.invisible() != current_attr.invisible() {
                                CSI::Sgr(Sgr::Invisible(attr.invisible()))
                                    .encode_escape(&mut WriteWrapper::new(out))?;
                            }
                        }

                        attr_on_off!(EnterItalicsMode, ExitItalicsMode, attr, italic, Italic);

                        // TODO: add strikethrough to Capabilities
                        if attr.strikethrough() != current_attr.strikethrough() {
                            CSI::Sgr(Sgr::StrikeThrough(attr.strikethrough()))
                                .encode_escape(&mut WriteWrapper::new(out))?;
                        }
                    }

                    let has_true_color = self.caps.color_level() == ColorLevel::TrueColor;

                    if attr.foreground != current_attr.foreground {
                        match (has_true_color, attr.foreground.full, attr.foreground.ansi) {
                            (true, Some(tc), _) | (true, _, ColorSpec::TrueColor(tc)) => {
                                CSI::Sgr(Sgr::Foreground(ColorSpec::TrueColor(tc)))
                                    .encode_escape(&mut WriteWrapper::new(out))?;
                            }
                            (false, _, ColorSpec::TrueColor(_)) => {
                                // TrueColor was specified with no fallback :-(
                            }
                            (_, _, ColorSpec::Default) => {
                                // Terminfo doesn't define a reset color to default, so
                                // we use the ANSI code.
                                CSI::Sgr(Sgr::Foreground(ColorSpec::Default))
                                    .encode_escape(&mut WriteWrapper::new(out))?;
                            }
                            (_, _, ColorSpec::PaletteIndex(idx)) => {
                                if let Some(set) = self.get_capability::<cap::SetAForeground>()
                                {
                                    set.expand().color(idx).to(WriteWrapper::new(out))?;
                                } else {
                                    CSI::Sgr(Sgr::Foreground(ColorSpec::PaletteIndex(idx)))
                                        .encode_escape(&mut WriteWrapper::new(out))?;
                                }
                            }
                        }
                    }

                    if attr.background != current_attr.background {
                        match (has_true_color, attr.background.full, attr.background.ansi) {
                            (true, Some(tc), _) | (true, _, ColorSpec::TrueColor(tc)) => {
                                CSI::Sgr(Sgr::Background(ColorSpec::TrueColor(tc)))
                                    .encode_escape(&mut WriteWrapper::new(out))?;
                            }
                            (false, _, ColorSpec::TrueColor(_)) => {
                                // TrueColor was specified with no fallback :-(
                            }
                            (_, _, ColorSpec::Default) => {
                                // Terminfo doesn't define a reset color to default, so
                                // we use the ANSI code.
                                CSI::Sgr(Sgr::Background(ColorSpec::Default))
                                    .encode_escape(&mut WriteWrapper::new(out))?;
                            }
                            (_, _, ColorSpec::PaletteIndex(idx)) => {
                                if let Some(set) = self.get_capability::<cap::SetABackground>()
                                {
                                    set.expand().color(idx).to(WriteWrapper::new(out))?;
                                } else {
                                    CSI::Sgr(Sgr::Background(ColorSpec::PaletteIndex(idx)))
                                        .encode_escape(&mut WriteWrapper::new(out))?;
                                }
                            }
                        }
                    }

                    if self.caps.hyperlinks() {
                        if let Some(link) = attr.hyperlink.as_ref() {
                            let osc =
                                OperatingSystemCommand::SetHyperlink(Some((**link).clone()));
                            osc.encode_escape(&mut WriteWrapper::new(out))?;
                        } else if current_attr.hyperlink.is_some() {
                            // Close out the old hyperlink
                            let osc = OperatingSystemCommand::SetHyperlink(None);
                            osc.encode_escape(&mut WriteWrapper::new(out))?;
                        }
                    }

                    current_attr = attr;
                }
            };
        }

        for change in changes {
            match change {
                Change::Attribute(AttributeChange::Intensity(value)) => {
                    record!(set_intensity, value);
                }
                Change::Attribute(AttributeChange::Italic(value)) => {
                    record!(set_italic, value);
                }
                Change::Attribute(AttributeChange::Reverse(value)) => {
                    record!(set_reverse, value);
                }
                Change::Attribute(AttributeChange::StrikeThrough(value)) => {
                    record!(set_strikethrough, value);
                }
                Change::Attribute(AttributeChange::Blink(value)) => {
                    record!(set_blink, value);
                }
                Change::Attribute(AttributeChange::Invisible(value)) => {
                    record!(set_invisible, value);
                }
                Change::Attribute(AttributeChange::Underline(value)) => {
                    record!(set_underline, value);
                }
                Change::Attribute(AttributeChange::Foreground(col)) => {
                    attr_apply!(|attr: &mut CellAttributes| attr.foreground = *col);
                }
                Change::Attribute(AttributeChange::Background(col)) => {
                    attr_apply!(|attr: &mut CellAttributes| attr.background = *col);
                }
                Change::Attribute(AttributeChange::Hyperlink(link)) => {
                    attr_apply!(|attr: &mut CellAttributes| attr.hyperlink = link.clone());
                }
                Change::AllAttributes(all) => {
                    pending_attr = Some(all.clone());
                }
                Change::Text(text) => {
                    flush_pending_attr!();
                    WriteWrapper::new(out).write_all(text.as_bytes())?;
                }
                Change::CursorPosition {
                    x: Position::Absolute(0),
                    y: Position::Relative(1),
                } => {
                    WriteWrapper::new(out).write_all(b"\r\n")?;
                }
                Change::CursorPosition {
                    x: Position::Absolute(0),
                    y: Position::Absolute(0),
                } => {
                    if let Some(attr) = self.get_capability::<cap::CursorHome>() {
                        attr.expand().to(WriteWrapper::new(out))?;
                    } else {
                        CSI::Cursor(Cursor::Position { line: 1, col: 1 })
                            .encode_escape(&mut WriteWrapper::new(out))?
                    }
                }
                Change::CursorPosition {
                    x: Position::NoChange,
                    y: Position::Relative(1),
                } => {
                    if let Some(attr) = self.get_capability::<cap::CursorDown>() {
                        attr.expand().to(WriteWrapper::new(out))?;
                    } else {
                        CSI::Cursor(Cursor::Down(1)).encode_escape(&mut WriteWrapper::new(out))?
                    }
                }
                Change::CursorPosition {
                    x: Position::NoChange,
                    y: Position::Relative(-1),
                } => {
                    if let Some(attr) = self.get_capability::<cap::CursorUp>() {
                        attr.expand().to(WriteWrapper::new(out))?;
                    } else {
                        CSI::Cursor(Cursor::Up(1)).encode_escape(&mut WriteWrapper::new(out))?
                    }
                }
                Change::CursorPosition {
                    x: Position::Relative(-1),
                    y: Position::NoChange,
                } => {
                    if let Some(attr) = self.get_capability::<cap::CursorLeft>() {
                        attr.expand().to(WriteWrapper::new(out))?;
                    } else {
                        CSI::Cursor(Cursor::Left(1)).encode_escape(&mut WriteWrapper::new(out))?
                    }
                }
                Change::CursorPosition {
                    x: Position::Relative(1),
                    y: Position::NoChange,
                } => {
                    if let Some(attr) = self.get_capability::<cap::CursorRight>() {
                        attr.expand().to(WriteWrapper::new(out))?;
                    } else {
                        CSI::Cursor(Cursor::Right(1)).encode_escape(&mut WriteWrapper::new(out))?
                    }
                }
                Change::CursorPosition {
                    x: Position::Absolute(x),
                    y: Position::Absolute(y),
                } => {
                    let x = (*x as u32) + 1;
                    let y = (*y as u32) + 1;
                    if let Some(attr) = self.get_capability::<cap::CursorAddress>() {
                        attr.expand().x(x).y(y).to(WriteWrapper::new(out))?;
                    } else {
                        CSI::Cursor(Cursor::Position { line: x, col: y })
                            .encode_escape(&mut WriteWrapper::new(out))?
                    }
                }
                Change::CursorPosition { .. } => {
                    eprintln!(
                        "unhandled CursorPosition in TerminfoRenderer::render_to: {:?}",
                        change
                    );
                }
            }
        }

        flush_pending_attr!();
        Ok(current_attr)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use caps::ProbeHintsBuilder;
    use color::{AnsiColor, RgbColor};
    use escape::parser::Parser;
    use escape::{Action, Esc, EscCode};
    use terminfo;

    /// Return Capabilities loaded from the included xterm terminfo data
    fn xterm_terminfo() -> Capabilities {
        // Load our own compiled data so that the tests have an
        // environment that doesn't vary machine by machine.
        let data = include_bytes!("../../data/xterm-256color");
        Capabilities::new_with_hints(
            ProbeHintsBuilder::default()
                .terminfo_db(Some(
                    terminfo::Database::from_buffer(data.as_ref()).unwrap(),
                ))
                .build()
                .unwrap(),
        ).unwrap()
    }

    fn no_terminfo_all_enabled() -> Capabilities {
        Capabilities::new_with_hints(
            ProbeHintsBuilder::default()
                .color_level(Some(ColorLevel::TrueColor))
                .build()
                .unwrap(),
        ).unwrap()
    }

    fn parse(bytes: &[u8]) -> Vec<Action> {
        let mut p = Parser::new();
        p.parse_as_vec(bytes)
    }

    #[test]
    fn test_empty_render() {
        let mut out = Vec::<u8>::new();
        let renderer = TerminfoRenderer::new(xterm_terminfo());
        let end_attr = renderer
            .render_to(&CellAttributes::default(), &[], &mut out)
            .unwrap();
        assert_eq!("", String::from_utf8(out).unwrap());
        assert_eq!(end_attr, CellAttributes::default());
    }

    #[test]
    fn test_basic_text() {
        let mut out = Vec::<u8>::new();
        let renderer = TerminfoRenderer::new(xterm_terminfo());
        let end_attr = renderer
            .render_to(
                &CellAttributes::default(),
                &[Change::Text("foo".into())],
                &mut out,
            )
            .unwrap();
        assert_eq!("foo", String::from_utf8(out).unwrap());
        assert_eq!(end_attr, CellAttributes::default());
    }

    #[test]
    fn test_bold_text() {
        let mut out = Vec::<u8>::new();
        let renderer = TerminfoRenderer::new(xterm_terminfo());
        let end_attr = renderer
            .render_to(
                &CellAttributes::default(),
                &[
                    Change::Text("not ".into()),
                    Change::Attribute(AttributeChange::Intensity(Intensity::Bold)),
                    Change::Text("foo".into()),
                ],
                &mut out,
            )
            .unwrap();

        let result = parse(&out);
        assert_eq!(
            result,
            vec![
                Action::Print('n'),
                Action::Print('o'),
                Action::Print('t'),
                Action::Print(' '),
                Action::Esc(Esc::Code(EscCode::AsciiCharacterSet)),
                Action::CSI(CSI::Sgr(Sgr::Reset)),
                Action::CSI(CSI::Sgr(Sgr::Intensity(Intensity::Bold))),
                Action::Print('f'),
                Action::Print('o'),
                Action::Print('o'),
            ]
        );

        assert_eq!(
            end_attr,
            CellAttributes::default()
                .set_intensity(Intensity::Bold)
                .clone()
        );
    }

    #[test]
    fn test_bold_text_no_terminfo() {
        let mut out = Vec::<u8>::new();
        let renderer = TerminfoRenderer::new(no_terminfo_all_enabled());
        let end_attr = renderer
            .render_to(
                &CellAttributes::default(),
                &[
                    Change::Text("not ".into()),
                    Change::Attribute(AttributeChange::Intensity(Intensity::Bold)),
                    Change::Text("foo".into()),
                ],
                &mut out,
            )
            .unwrap();

        let result = parse(&out);
        assert_eq!(
            result,
            vec![
                Action::Print('n'),
                Action::Print('o'),
                Action::Print('t'),
                Action::Print(' '),
                Action::CSI(CSI::Sgr(Sgr::Reset)),
                Action::CSI(CSI::Sgr(Sgr::Intensity(Intensity::Bold))),
                Action::Print('f'),
                Action::Print('o'),
                Action::Print('o'),
            ]
        );

        assert_eq!(
            end_attr,
            CellAttributes::default()
                .set_intensity(Intensity::Bold)
                .clone()
        );
    }

    #[test]
    fn test_red_bold_text() {
        let mut out = Vec::<u8>::new();
        let renderer = TerminfoRenderer::new(xterm_terminfo());
        let end_attr = renderer
            .render_to(
                &CellAttributes::default(),
                &[
                    Change::Attribute(AttributeChange::Foreground(AnsiColor::Maroon.into())),
                    Change::Attribute(AttributeChange::Intensity(Intensity::Bold)),
                    Change::Text("red".into()),
                    Change::Attribute(AttributeChange::Foreground(AnsiColor::Red.into())),
                ],
                &mut out,
            )
            .unwrap();

        let result = parse(&out);
        assert_eq!(
            result,
            vec![
                Action::Esc(Esc::Code(EscCode::AsciiCharacterSet)),
                Action::CSI(CSI::Sgr(Sgr::Reset)),
                // Note that the render code rearranges (red,bold) to (bold,red)
                Action::CSI(CSI::Sgr(Sgr::Intensity(Intensity::Bold))),
                Action::CSI(CSI::Sgr(Sgr::Foreground(AnsiColor::Maroon.into()))),
                Action::Print('r'),
                Action::Print('e'),
                Action::Print('d'),
                Action::CSI(CSI::Sgr(Sgr::Foreground(AnsiColor::Red.into()))),
            ]
        );

        assert_eq!(
            end_attr,
            CellAttributes::default()
                .set_intensity(Intensity::Bold)
                .set_foreground(AnsiColor::Red)
                .clone()
        );
    }

    #[test]
    fn test_red_bold_text_no_terminfo() {
        let mut out = Vec::<u8>::new();
        let renderer = TerminfoRenderer::new(no_terminfo_all_enabled());
        let end_attr = renderer
            .render_to(
                &CellAttributes::default(),
                &[
                    Change::Attribute(AttributeChange::Foreground(AnsiColor::Maroon.into())),
                    Change::Attribute(AttributeChange::Intensity(Intensity::Bold)),
                    Change::Text("red".into()),
                    Change::Attribute(AttributeChange::Foreground(AnsiColor::Red.into())),
                ],
                &mut out,
            )
            .unwrap();

        let result = parse(&out);
        assert_eq!(
            result,
            vec![
                Action::CSI(CSI::Sgr(Sgr::Reset)),
                // Note that the render code rearranges (red,bold) to (bold,red)
                Action::CSI(CSI::Sgr(Sgr::Intensity(Intensity::Bold))),
                Action::CSI(CSI::Sgr(Sgr::Foreground(AnsiColor::Maroon.into()))),
                Action::Print('r'),
                Action::Print('e'),
                Action::Print('d'),
                Action::CSI(CSI::Sgr(Sgr::Foreground(AnsiColor::Red.into()))),
            ]
        );

        assert_eq!(
            end_attr,
            CellAttributes::default()
                .set_intensity(Intensity::Bold)
                .set_foreground(AnsiColor::Red)
                .clone()
        );
    }

    #[test]
    fn truecolor() {
        let mut out = Vec::<u8>::new();
        let renderer = TerminfoRenderer::new(xterm_terminfo());
        renderer
            .render_to(
                &CellAttributes::default(),
                &[
                    Change::Attribute(AttributeChange::Foreground(
                        ColorSpec::TrueColor(RgbColor::new(255, 128, 64)).into(),
                    )),
                    Change::Text("A".into()),
                ],
                &mut out,
            )
            .unwrap();

        let result = parse(&out);
        assert_eq!(
            result,
            vec![
                Action::CSI(CSI::Sgr(Sgr::Foreground(
                    ColorSpec::TrueColor(RgbColor::new(255, 128, 64)).into(),
                ))),
                Action::Print('A'),
            ]
        );
    }

    #[test]
    fn truecolor_no_terminfo() {
        let mut out = Vec::<u8>::new();
        let renderer = TerminfoRenderer::new(no_terminfo_all_enabled());
        renderer
            .render_to(
                &CellAttributes::default(),
                &[
                    Change::Attribute(AttributeChange::Foreground(
                        ColorSpec::TrueColor(RgbColor::new(255, 128, 64)).into(),
                    )),
                    Change::Text("A".into()),
                ],
                &mut out,
            )
            .unwrap();

        let result = parse(&out);
        assert_eq!(
            result,
            vec![
                Action::CSI(CSI::Sgr(Sgr::Foreground(
                    ColorSpec::TrueColor(RgbColor::new(255, 128, 64)).into(),
                ))),
                Action::Print('A'),
            ]
        );
    }
}
