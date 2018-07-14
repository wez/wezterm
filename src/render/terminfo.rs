//! Rendering of Changes using terminfo
use cell::{AttributeChange, Blink, CellAttributes, Intensity, Underline};
use color::ColorSpec;
use failure;
use render::Renderer;
use screen::{Change, Position};
use std::io::{Error as IoError, Write};
use terminfo::{capability as cap, Database};

pub struct TerminfoRenderer {
    db: Database,
}

impl TerminfoRenderer {
    pub fn new(db: Database) -> Self {
        Self { db }
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
            ($cap_on:ident, $cap_off:ident, $value:expr) => {
                if $value {
                    if let Some(attr) = self.db.get::<cap::$cap_on>() {
                        attr.expand().to(WriteWrapper::new(out))?;
                    }
                } else {
                    if let Some(attr) = self.db.get::<cap::$cap_off>() {
                        attr.expand().to(WriteWrapper::new(out))?;
                    }
                }
            };
        }

        macro_rules! flush_pending_attr {
            () => {
                if let Some(attr) = pending_attr.take() {
                    if let Some(sgr) = self.db.get::<cap::SetAttributes>() {
                        sgr.expand()
                            .bold(attr.intensity() == Intensity::Bold)
                            .dim(attr.intensity() == Intensity::Half)
                            .underline(attr.underline() != Underline::None)
                            .blink(attr.blink() != Blink::None)
                            .reverse(attr.reverse())
                            .invisible(attr.invisible())
                            .to(WriteWrapper::new(out))?;
                    }

                    if attr.italic() != current_attr.italic() {
                        attr_on_off!(EnterItalicsMode, ExitItalicsMode, attr.italic());
                    }

                    // Note: strikethrough is not exposed in terminfo

                    let has_true_color = self.db
                        .get::<cap::TrueColor>()
                        .unwrap_or(cap::TrueColor(false))
                        .0;

                    if attr.foreground != current_attr.foreground {
                        match (has_true_color, attr.foreground.full, attr.foreground.ansi) {
                            (true, Some(tc), _) => {
                                write!(
                                    WriteWrapper::new(out),
                                    "\x1b[38;2;{};{};{}m",
                                    tc.red,
                                    tc.green,
                                    tc.blue
                                )?;
                            }
                            (_, _, ColorSpec::TrueColor(_)) => {
                                // TrueColor was specified with no fallback :-(
                            }
                            (_, _, ColorSpec::Default) => {
                                // Terminfo doesn't define a reset color to default, so
                                // we use the ANSI code.
                                write!(WriteWrapper::new(out), "\x1b[39m")?;
                            }
                            (_, _, ColorSpec::PaletteIndex(idx)) => {
                                if let Some(set) = self.db.get::<cap::SetAForeground>() {
                                    set.expand().color(idx).to(WriteWrapper::new(out))?;
                                }
                            }
                        }
                    }

                    if attr.background != current_attr.background {
                        match (has_true_color, attr.background.full, attr.background.ansi) {
                            (true, Some(tc), _) => {
                                write!(
                                    WriteWrapper::new(out),
                                    "\x1b[48;2;{};{};{}m",
                                    tc.red,
                                    tc.green,
                                    tc.blue
                                )?;
                            }
                            (_, _, ColorSpec::TrueColor(_)) => {
                                // TrueColor was specified with no fallback :-(
                            }
                            (_, _, ColorSpec::Default) => {
                                // Terminfo doesn't define a reset color to default, so
                                // we use the ANSI code.
                                write!(WriteWrapper::new(out), "\x1b[49m")?;
                            }
                            (_, _, ColorSpec::PaletteIndex(idx)) => {
                                if let Some(set) = self.db.get::<cap::SetABackground>() {
                                    set.expand().color(idx).to(WriteWrapper::new(out))?;
                                }
                            }
                        }
                    }

                    // See https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda
                    if let Some(link) = attr.hyperlink.as_ref() {
                        write!(WriteWrapper::new(out), "\x1b]8;")?;
                        if !link.id.is_empty() {
                            write!(WriteWrapper::new(out), "id={}", link.id)?;
                        }
                        write!(WriteWrapper::new(out), ";{}\x1b\\", link.url)?;
                    } else if current_attr.hyperlink.is_some() {
                        // Close out the old hyperlink
                        write!(WriteWrapper::new(out), "\x1b]8;;\x1b\\")?;
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
                Change::Attribute(AttributeChange::StrikeThrough(_)) => {
                    // Not possible via terminfo
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
                    if let Some(attr) = self.db.get::<cap::CursorHome>() {
                        attr.expand().to(WriteWrapper::new(out))?;
                    }
                }
                Change::CursorPosition {
                    x: Position::NoChange,
                    y: Position::Relative(1),
                } => {
                    if let Some(attr) = self.db.get::<cap::CursorDown>() {
                        attr.expand().to(WriteWrapper::new(out))?;
                    }
                }
                Change::CursorPosition {
                    x: Position::NoChange,
                    y: Position::Relative(-1),
                } => {
                    if let Some(attr) = self.db.get::<cap::CursorUp>() {
                        attr.expand().to(WriteWrapper::new(out))?;
                    }
                }
                Change::CursorPosition {
                    x: Position::Relative(-1),
                    y: Position::NoChange,
                } => {
                    if let Some(attr) = self.db.get::<cap::CursorLeft>() {
                        attr.expand().to(WriteWrapper::new(out))?;
                    }
                }
                Change::CursorPosition {
                    x: Position::Relative(1),
                    y: Position::NoChange,
                } => {
                    if let Some(attr) = self.db.get::<cap::CursorRight>() {
                        attr.expand().to(WriteWrapper::new(out))?;
                    }
                }
                Change::CursorPosition {
                    x: Position::Absolute(x),
                    y: Position::Absolute(y),
                } => {
                    if let Some(attr) = self.db.get::<cap::CursorAddress>() {
                        let x = (*x as u32) + 1;
                        let y = (*y as u32) + 1;
                        attr.expand().x(x).y(y).to(WriteWrapper::new(out))?;
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
    use color::AnsiColor;

    fn xterm() -> Database {
        // Load the xterm entry from the system location.
        // Depending on how things go, we may want to include
        // our own definition in the repo and load from that path
        // instead.
        Database::from_name("xterm").unwrap()
    }

    #[test]
    fn test_empty_render() {
        let mut out = Vec::<u8>::new();
        let renderer = TerminfoRenderer::new(xterm());
        let end_attr = renderer
            .render_to(&CellAttributes::default(), &[], &mut out)
            .unwrap();
        assert_eq!("", String::from_utf8(out).unwrap());
        assert_eq!(end_attr, CellAttributes::default());
    }

    #[test]
    fn test_basic_text() {
        let mut out = Vec::<u8>::new();
        let renderer = TerminfoRenderer::new(xterm());
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
        let renderer = TerminfoRenderer::new(xterm());
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

        assert_eq!(
            // The `CSI (B` sequence is rmacs
            "not \x1b(B\x1b[0;1mfoo",
            String::from_utf8(out).unwrap()
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
        let renderer = TerminfoRenderer::new(xterm());
        let end_attr = renderer
            .render_to(
                &CellAttributes::default(),
                &[
                    Change::Attribute(AttributeChange::Foreground(AnsiColor::Red.into())),
                    Change::Attribute(AttributeChange::Intensity(Intensity::Bold)),
                    Change::Text("red".into()),
                ],
                &mut out,
            )
            .unwrap();

        // Note that the render code rearranges (red,bold) to (bold,red)
        assert_eq!(
            "\x1b(B\x1b[0;1m\x1b[39mred",
            String::from_utf8(out).unwrap()
        );

        assert_eq!(
            end_attr,
            CellAttributes::default()
                .set_intensity(Intensity::Bold)
                .set_foreground(AnsiColor::Red)
                .clone()
        );
    }
}
