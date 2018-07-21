//! Rendering of Changes using terminfo
use caps::{Capabilities, ColorLevel};
use cell::{AttributeChange, Blink, CellAttributes, Intensity, Underline};
use color::{ColorAttribute, ColorSpec};
use escape::csi::{Cursor, Edit, EraseInDisplay, Sgr, CSI};
use escape::osc::OperatingSystemCommand;
use failure;
use render::Renderer;
use screen::{Change, Position};
use std::io::{Error as IoError, Write};
use terminal::Terminal;
use terminfo::{capability as cap, Capability as TermInfoCapability};

pub struct TerminfoRenderer {
    caps: Capabilities,
    current_attr: CellAttributes,
    pending_attr: Option<CellAttributes>,
}

impl TerminfoRenderer {
    pub fn new(caps: Capabilities) -> Self {
        Self {
            caps,
            current_attr: CellAttributes::default(),
            pending_attr: None,
        }
    }

    fn get_capability<'a, T: TermInfoCapability<'a>>(&'a self) -> Option<T> {
        self.caps.terminfo_db().and_then(|db| db.get::<T>())
    }

    fn attr_apply<F: FnOnce(&mut CellAttributes)>(&mut self, func: F) {
        self.pending_attr = Some(match self.pending_attr.take() {
            Some(mut attr) => {
                func(&mut attr);
                attr
            }
            None => {
                let mut attr = self.current_attr.clone();
                func(&mut attr);
                attr
            }
        });
    }

    fn flush_pending_attr(&mut self, out: &mut Terminal) -> Result<(), failure::Error> {
        macro_rules! attr_on_off {
            ($cap_on:ident, $cap_off:ident, $attr:expr, $accesor:ident, $sgr:ident) => {
                let value = $attr.$accesor();
                if value != self.current_attr.$accesor() {
                    let mut out = WriteWrapper::new(out);
                    let on: bool = value.into();
                    if on {
                        if let Some(attr) = self.get_capability::<cap::$cap_on>() {
                            attr.expand().to(out)?;
                        } else {
                            write!(out, "{}", CSI::Sgr(Sgr::$sgr(value)))?;
                        }
                    } else {
                        if let Some(attr) = self.get_capability::<cap::$cap_off>() {
                            attr.expand().to(out)?;
                        } else {
                            write!(out, "{}", CSI::Sgr(Sgr::$sgr(value)))?;
                        }
                    }
                }
            };
        }

        if let Some(attr) = self.pending_attr.take() {
            if !attr.attribute_bits_equal(&self.current_attr) {
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
                    if let Some(exit) = self.get_capability::<cap::ExitAttributeMode>() {
                        exit.expand().to(WriteWrapper::new(out))?;
                    } else {
                        write!(out, "{}", CSI::Sgr(Sgr::Reset))?;
                    }

                    if attr.intensity() != self.current_attr.intensity() {
                        match attr.intensity() {
                            Intensity::Bold => {
                                if let Some(bold) = self.get_capability::<cap::EnterBoldMode>() {
                                    bold.expand().to(WriteWrapper::new(out))?;
                                } else {
                                    write!(out, "{}", CSI::Sgr(Sgr::Intensity(attr.intensity())))?;
                                }
                            }
                            Intensity::Half => {
                                if let Some(dim) = self.get_capability::<cap::EnterDimMode>() {
                                    dim.expand().to(WriteWrapper::new(out))?;
                                } else {
                                    write!(out, "{}", CSI::Sgr(Sgr::Intensity(attr.intensity())))?;
                                }
                            }
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

                    if attr.blink() != self.current_attr.blink() {
                        if let Some(attr) = self.get_capability::<cap::EnterBlinkMode>() {
                            attr.expand().to(WriteWrapper::new(out))?;
                        } else {
                            write!(out, "{}", CSI::Sgr(Sgr::Blink(attr.blink())))?;
                        }
                    }

                    if attr.reverse() != self.current_attr.reverse() {
                        if let Some(attr) = self.get_capability::<cap::EnterReverseMode>() {
                            attr.expand().to(WriteWrapper::new(out))?;
                        } else {
                            write!(out, "{}", CSI::Sgr(Sgr::Inverse(attr.reverse())))?;
                        }
                    }

                    if attr.invisible() != self.current_attr.invisible() {
                        write!(out, "{}", CSI::Sgr(Sgr::Invisible(attr.invisible())))?;
                    }
                }

                attr_on_off!(EnterItalicsMode, ExitItalicsMode, attr, italic, Italic);

                // TODO: add strikethrough to Capabilities
                if attr.strikethrough() != self.current_attr.strikethrough() {
                    write!(
                        out,
                        "{}",
                        CSI::Sgr(Sgr::StrikeThrough(attr.strikethrough()))
                    )?;
                }
            }

            let has_true_color = self.caps.color_level() == ColorLevel::TrueColor;

            if attr.foreground != self.current_attr.foreground {
                match (has_true_color, attr.foreground) {
                    (true, ColorAttribute::TrueColorWithPaletteFallback(tc, _))
                    | (true, ColorAttribute::TrueColorWithDefaultFallback(tc)) => {
                        write!(
                            out,
                            "{}",
                            CSI::Sgr(Sgr::Foreground(ColorSpec::TrueColor(tc)))
                        )?;
                    }
                    (false, ColorAttribute::TrueColorWithDefaultFallback(_))
                    | (_, ColorAttribute::Default) => {
                        // Terminfo doesn't define a reset color to default, so
                        // we use the ANSI code.
                        write!(out, "{}", CSI::Sgr(Sgr::Foreground(ColorSpec::Default)))?;
                    }
                    (false, ColorAttribute::TrueColorWithPaletteFallback(_, idx))
                    | (_, ColorAttribute::PaletteIndex(idx)) => {
                        if let Some(set) = self.get_capability::<cap::SetAForeground>() {
                            set.expand().color(idx).to(WriteWrapper::new(out))?;
                        } else {
                            write!(
                                out,
                                "{}",
                                CSI::Sgr(Sgr::Foreground(ColorSpec::PaletteIndex(idx)))
                            )?;
                        }
                    }
                }
            }

            if attr.background != self.current_attr.background {
                match (has_true_color, attr.background) {
                    (true, ColorAttribute::TrueColorWithPaletteFallback(tc, _))
                    | (true, ColorAttribute::TrueColorWithDefaultFallback(tc)) => {
                        write!(
                            out,
                            "{}",
                            CSI::Sgr(Sgr::Background(ColorSpec::TrueColor(tc)))
                        )?;
                    }
                    (false, ColorAttribute::TrueColorWithDefaultFallback(_))
                    | (_, ColorAttribute::Default) => {
                        // Terminfo doesn't define a reset color to default, so
                        // we use the ANSI code.
                        write!(out, "{}", CSI::Sgr(Sgr::Background(ColorSpec::Default)))?;
                    }
                    (false, ColorAttribute::TrueColorWithPaletteFallback(_, idx))
                    | (_, ColorAttribute::PaletteIndex(idx)) => {
                        if let Some(set) = self.get_capability::<cap::SetABackground>() {
                            set.expand().color(idx).to(WriteWrapper::new(out))?;
                        } else {
                            write!(
                                out,
                                "{}",
                                CSI::Sgr(Sgr::Background(ColorSpec::PaletteIndex(idx)))
                            )?;
                        }
                    }
                }
            }

            if self.caps.hyperlinks() {
                if let Some(link) = attr.hyperlink.as_ref() {
                    let osc = OperatingSystemCommand::SetHyperlink(Some((**link).clone()));
                    write!(out, "{}", osc)?;
                } else if self.current_attr.hyperlink.is_some() {
                    // Close out the old hyperlink
                    let osc = OperatingSystemCommand::SetHyperlink(None);
                    write!(out, "{}", osc)?;
                }
            }

            self.current_attr = attr;
        }

        Ok(())
    }
}

// The terminfo crate wants us to move the Write instance in on
// each call.  This is a little struct that allows us to do that
// without moving out the actual Write ref.
struct WriteWrapper<'a> {
    out: &'a mut Terminal,
}

impl<'a> WriteWrapper<'a> {
    fn new(out: &'a mut Terminal) -> Self {
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
        &mut self,
        start_attr: &CellAttributes,
        changes: &[Change],
        out: &mut Terminal,
    ) -> Result<CellAttributes, failure::Error> {
        self.current_attr = start_attr.clone();
        self.pending_attr = None;

        macro_rules! record {
            ($accesor:ident, $value:expr) => {
                self.attr_apply(|attr| {
                    attr.$accesor(*$value);
                });
            };
        }

        for change in changes {
            match change {
                Change::ClearScreen(color) => {
                    // ClearScreen implicitly resets all to default
                    let defaults = CellAttributes::default()
                        .set_background(color.clone())
                        .clone();
                    if self.current_attr != defaults {
                        self.pending_attr = Some(defaults);
                        self.flush_pending_attr(out)?;
                    }
                    self.pending_attr = None;

                    if self.current_attr.background == ColorAttribute::Default || self.caps.bce() {
                        // The erase operation respects "background color erase",
                        // or we're clearing to the default background color, so we can
                        // simply emit a clear screen op.
                        if let Some(clr) = self.get_capability::<cap::ClearScreen>() {
                            clr.expand().to(WriteWrapper::new(out))?;
                        } else {
                            if let Some(attr) = self.get_capability::<cap::CursorHome>() {
                                attr.expand().to(WriteWrapper::new(out))?;
                            } else {
                                write!(
                                    out,
                                    "{}",
                                    CSI::Cursor(Cursor::Position { line: 1, col: 1 })
                                )?;
                            }

                            write!(
                                out,
                                "{}",
                                CSI::Edit(Edit::EraseInDisplay(EraseInDisplay::EraseDisplay))
                            )?;
                        }
                    } else {
                        // We're setting the background to a specific color, so we get to
                        // paint the whole thing.

                        if let Some(attr) = self.get_capability::<cap::CursorHome>() {
                            attr.expand().to(WriteWrapper::new(out))?;
                        } else {
                            write!(out, "{}", CSI::Cursor(Cursor::Position { line: 1, col: 1 }))?;
                        }

                        let size = out.get_screen_size()?;
                        let num_spaces = size.cols * size.rows;
                        let mut buf = Vec::with_capacity(num_spaces);
                        buf.resize(num_spaces, b' ');
                        out.write(buf.as_slice())?;
                    }
                }
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
                    self.attr_apply(|attr| attr.foreground = *col);
                }
                Change::Attribute(AttributeChange::Background(col)) => {
                    self.attr_apply(|attr| attr.background = *col);
                }
                Change::Attribute(AttributeChange::Hyperlink(link)) => {
                    self.attr_apply(|attr| attr.hyperlink = link.clone());
                }
                Change::AllAttributes(all) => {
                    self.pending_attr = Some(all.clone());
                }
                Change::Text(text) => {
                    self.flush_pending_attr(out)?;
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
                        write!(out, "{}", CSI::Cursor(Cursor::Position { line: 1, col: 1 }))?;
                    }
                }
                Change::CursorPosition {
                    x: Position::NoChange,
                    y: Position::Relative(1),
                } => {
                    if let Some(attr) = self.get_capability::<cap::CursorDown>() {
                        attr.expand().to(WriteWrapper::new(out))?;
                    } else {
                        write!(out, "{}", CSI::Cursor(Cursor::Down(1)))?;
                    }
                }
                Change::CursorPosition {
                    x: Position::NoChange,
                    y: Position::Relative(-1),
                } => {
                    if let Some(attr) = self.get_capability::<cap::CursorUp>() {
                        attr.expand().to(WriteWrapper::new(out))?;
                    } else {
                        write!(out, "{}", CSI::Cursor(Cursor::Up(1)))?;
                    }
                }
                Change::CursorPosition {
                    x: Position::Relative(-1),
                    y: Position::NoChange,
                } => {
                    if let Some(attr) = self.get_capability::<cap::CursorLeft>() {
                        attr.expand().to(WriteWrapper::new(out))?;
                    } else {
                        write!(out, "{}", CSI::Cursor(Cursor::Left(1)))?;
                    }
                }
                Change::CursorPosition {
                    x: Position::Relative(1),
                    y: Position::NoChange,
                } => {
                    if let Some(attr) = self.get_capability::<cap::CursorRight>() {
                        attr.expand().to(WriteWrapper::new(out))?;
                    } else {
                        write!(out, "{}", CSI::Cursor(Cursor::Right(1)))?;
                    }
                }
                Change::CursorPosition {
                    x: Position::Absolute(x),
                    y: Position::Absolute(y),
                } => {
                    let x = *x as u32;
                    let y = *y as u32;
                    if let Some(attr) = self.get_capability::<cap::CursorAddress>() {
                        // terminfo expansion automatically converts coordinates to 1-based,
                        // so we can pass in the 0-based coordinates as-is
                        attr.expand().x(x).y(y).to(WriteWrapper::new(out))?;
                    } else {
                        // We need to manually convert to 1-based as the CSI representation
                        // requires it and there's no automatic conversion.
                        write!(
                            out,
                            "{}",
                            CSI::Cursor(Cursor::Position {
                                line: x + 1,
                                col: y + 1,
                            })
                        )?;
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

        self.flush_pending_attr(out)?;
        out.flush()?;
        Ok(self.current_attr.clone())
    }
}

#[cfg(all(test, unix))]
mod test {
    use super::*;
    use caps::ProbeHintsBuilder;
    use color::{AnsiColor, ColorAttribute, RgbColor};
    use escape::parser::Parser;
    use escape::{Action, Esc, EscCode};
    use failure::Error;
    use terminal::ScreenSize;
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

    use std::io::{Read, Result as IOResult, Write};

    struct FakeTerm {
        buf: Vec<u8>,
        size: ScreenSize,
    }

    impl FakeTerm {
        fn new() -> Self {
            Self::new_with_size(80, 24)
        }

        fn new_with_size(width: usize, height: usize) -> Self {
            let size = ScreenSize {
                cols: width,
                rows: height,
                xpixel: 0,
                ypixel: 0,
            };
            let buf = Vec::new();
            Self { size, buf }
        }

        fn parse(&self) -> Vec<Action> {
            let mut p = Parser::new();
            p.parse_as_vec(&self.buf)
        }
    }

    impl Write for FakeTerm {
        fn write(&mut self, buf: &[u8]) -> IOResult<usize> {
            self.buf.write(buf)
        }

        fn flush(&mut self) -> IOResult<()> {
            self.buf.flush()
        }
    }

    impl Read for FakeTerm {
        fn read(&mut self, _buf: &mut [u8]) -> IOResult<usize> {
            Ok(0)
        }
    }

    impl Terminal for FakeTerm {
        fn set_raw_mode(&mut self) -> Result<(), Error> {
            bail!("not implemented");
        }

        fn get_screen_size(&mut self) -> Result<ScreenSize, Error> {
            Ok(self.size.clone())
        }
        fn set_screen_size(&mut self, size: ScreenSize) -> Result<(), Error> {
            self.size = size;
            Ok(())
        }
    }

    #[test]
    fn empty_render() {
        let mut out = FakeTerm::new();
        let mut renderer = TerminfoRenderer::new(xterm_terminfo());
        let end_attr = renderer
            .render_to(&CellAttributes::default(), &[], &mut out)
            .unwrap();
        assert_eq!("", String::from_utf8(out.buf).unwrap());
        assert_eq!(end_attr, CellAttributes::default());
    }

    #[test]
    fn basic_text() {
        let mut out = FakeTerm::new();
        let mut renderer = TerminfoRenderer::new(xterm_terminfo());
        let end_attr = renderer
            .render_to(
                &CellAttributes::default(),
                &[Change::Text("foo".into())],
                &mut out,
            )
            .unwrap();
        assert_eq!("foo", String::from_utf8(out.buf).unwrap());
        assert_eq!(end_attr, CellAttributes::default());
    }

    #[test]
    fn bold_text() {
        let mut out = FakeTerm::new();
        let mut renderer = TerminfoRenderer::new(xterm_terminfo());
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

        let result = out.parse();
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
    fn clear_screen() {
        let mut out = FakeTerm::new_with_size(4, 3);
        let mut renderer = TerminfoRenderer::new(xterm_terminfo());
        let end_attr = renderer
            .render_to(
                &CellAttributes::default(),
                &[Change::ClearScreen(ColorAttribute::default())],
                &mut out,
            )
            .unwrap();

        let result = out.parse();
        assert_eq!(
            result,
            vec![
                Action::CSI(CSI::Cursor(Cursor::Position { line: 1, col: 1 })),
                Action::CSI(CSI::Edit(Edit::EraseInDisplay(
                    EraseInDisplay::EraseDisplay,
                ))),
            ]
        );

        assert_eq!(end_attr, CellAttributes::default());
    }

    #[test]
    fn clear_screen_bce() {
        let mut out = FakeTerm::new_with_size(4, 3);
        let mut renderer = TerminfoRenderer::new(xterm_terminfo());
        let end_attr = renderer
            .render_to(
                &CellAttributes::default(),
                &[Change::ClearScreen(AnsiColor::Maroon.into())],
                &mut out,
            )
            .unwrap();

        let result = out.parse();
        assert_eq!(
            result,
            vec![
                Action::CSI(CSI::Sgr(Sgr::Background(AnsiColor::Maroon.into()))),
                Action::CSI(CSI::Cursor(Cursor::Position { line: 1, col: 1 })),
                Action::CSI(CSI::Edit(Edit::EraseInDisplay(
                    EraseInDisplay::EraseDisplay,
                ))),
            ]
        );

        assert_eq!(
            end_attr,
            CellAttributes::default()
                .set_background(AnsiColor::Maroon)
                .clone()
        );
    }

    #[test]
    fn clear_screen_no_terminfo() {
        let mut out = FakeTerm::new_with_size(4, 3);
        let mut renderer = TerminfoRenderer::new(no_terminfo_all_enabled());
        let end_attr = renderer
            .render_to(
                &CellAttributes::default(),
                &[Change::ClearScreen(ColorAttribute::default())],
                &mut out,
            )
            .unwrap();

        let result = out.parse();
        assert_eq!(
            result,
            vec![
                Action::CSI(CSI::Cursor(Cursor::Position { line: 1, col: 1 })),
                Action::CSI(CSI::Edit(Edit::EraseInDisplay(
                    EraseInDisplay::EraseDisplay,
                ))),
            ]
        );

        assert_eq!(end_attr, CellAttributes::default());
    }

    #[test]
    fn clear_screen_bce_no_terminfo() {
        let mut out = FakeTerm::new_with_size(4, 3);
        let mut renderer = TerminfoRenderer::new(no_terminfo_all_enabled());
        let end_attr = renderer
            .render_to(
                &CellAttributes::default(),
                &[Change::ClearScreen(AnsiColor::Maroon.into())],
                &mut out,
            )
            .unwrap();

        let result = out.parse();
        assert_eq!(
            result,
            vec![
                Action::CSI(CSI::Sgr(Sgr::Background(AnsiColor::Maroon.into()))),
                Action::CSI(CSI::Cursor(Cursor::Position { line: 1, col: 1 })),
                // bce is not known to be available, so we emit a bunch of spaces.
                // TODO: could we use ECMA-48 REP for this?
                Action::Print(' '),
                Action::Print(' '),
                Action::Print(' '),
                Action::Print(' '),
                Action::Print(' '),
                Action::Print(' '),
                Action::Print(' '),
                Action::Print(' '),
                Action::Print(' '),
                Action::Print(' '),
                Action::Print(' '),
                Action::Print(' '),
            ]
        );

        assert_eq!(
            end_attr,
            CellAttributes::default()
                .set_background(AnsiColor::Maroon)
                .clone()
        );
    }

    #[test]
    fn bold_text_no_terminfo() {
        let mut out = FakeTerm::new();
        let mut renderer = TerminfoRenderer::new(no_terminfo_all_enabled());
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

        let result = out.parse();
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
    fn red_bold_text() {
        let mut out = FakeTerm::new();
        let mut renderer = TerminfoRenderer::new(xterm_terminfo());
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

        let result = out.parse();
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
    fn red_bold_text_no_terminfo() {
        let mut out = FakeTerm::new();
        let mut renderer = TerminfoRenderer::new(no_terminfo_all_enabled());
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

        let result = out.parse();
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
        let mut out = FakeTerm::new();
        let mut renderer = TerminfoRenderer::new(xterm_terminfo());
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

        let result = out.parse();
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
        let mut out = FakeTerm::new();
        let mut renderer = TerminfoRenderer::new(no_terminfo_all_enabled());
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

        let result = out.parse();
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
