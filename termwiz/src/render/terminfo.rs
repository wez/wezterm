//! Rendering of Changes using terminfo
use crate::caps::{Capabilities, ColorLevel};
use crate::cell::{AttributeChange, Blink, CellAttributes, Intensity, Underline};
use crate::color::{ColorAttribute, ColorSpec};
use crate::escape::csi::{Cursor, Edit, EraseInDisplay, EraseInLine, Sgr, CSI};
use crate::escape::osc::{ITermDimension, ITermFileData, ITermProprietary, OperatingSystemCommand};
use crate::escape::OneBased;
use crate::image::TextureCoordinate;
use crate::surface::{Change, CursorShape, Position};
use crate::terminal::unix::UnixTty;
use log::error;
use std::io::{Read, Write};
use terminfo::{capability as cap, Capability as TermInfoCapability};

pub struct TerminfoRenderer {
    caps: Capabilities,
    current_attr: CellAttributes,
    pending_attr: Option<CellAttributes>,
    /* TODO: we should record cursor position, shape and color here
     * so that we can optimize updating them on screen. */
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

    #[cfg_attr(feature = "cargo-clippy", allow(clippy::cognitive_complexity))]
    fn flush_pending_attr<W: UnixTty + Write>(&mut self, out: &mut W) -> anyhow::Result<()> {
        macro_rules! attr_on {
            ($cap:ident, $sgr:expr) => {
                if let Some(attr) = self.get_capability::<cap::$cap>() {
                    attr.expand().to(out.by_ref())?;
                } else {
                    write!(out, "{}", CSI::Sgr($sgr))?;
                }
            };
            ($sgr:expr) => {
                write!(out, "{}", CSI::Sgr($sgr))?;
            };
        }

        if let Some(attr) = self.pending_attr.take() {
            let mut current_foreground = self.current_attr.foreground;
            let mut current_background = self.current_attr.background;

            if !attr.attribute_bits_equal(&self.current_attr) {
                // Updating the attribute bits also resets the colors.
                current_foreground = ColorAttribute::Default;
                current_background = ColorAttribute::Default;

                // The SetAttributes capability can only handle single underline and slow blink.
                if let Some(sgr) = self.get_capability::<cap::SetAttributes>() {
                    sgr.expand()
                        .bold(attr.intensity() == Intensity::Bold)
                        .dim(attr.intensity() == Intensity::Half)
                        .underline(attr.underline() == Underline::Single)
                        .blink(attr.blink() == Blink::Slow)
                        .reverse(attr.reverse())
                        .invisible(attr.invisible())
                        .to(out.by_ref())?;
                } else {
                    attr_on!(ExitAttributeMode, Sgr::Reset);

                    match attr.intensity() {
                        Intensity::Bold => attr_on!(EnterBoldMode, Sgr::Intensity(Intensity::Bold)),
                        Intensity::Half => attr_on!(EnterDimMode, Sgr::Intensity(Intensity::Half)),
                        _ => {}
                    }

                    if attr.underline() == Underline::Single {
                        attr_on!(Sgr::Underline(Underline::Single));
                    }

                    if attr.blink() == Blink::Slow {
                        attr_on!(Sgr::Blink(Blink::Slow));
                    }

                    if attr.reverse() {
                        attr_on!(EnterReverseMode, Sgr::Inverse(true));
                    }

                    if attr.invisible() {
                        attr_on!(Sgr::Invisible(true));
                    }
                }

                if attr.underline() == Underline::Double {
                    attr_on!(Sgr::Underline(Underline::Double));
                }

                if attr.blink() == Blink::Rapid {
                    attr_on!(Sgr::Blink(Blink::Rapid));
                }

                if attr.italic() {
                    attr_on!(EnterItalicsMode, Sgr::Italic(true));
                }

                if attr.strikethrough() {
                    attr_on!(Sgr::StrikeThrough(true));
                }
            }

            let has_true_color = self.caps.color_level() == ColorLevel::TrueColor;

            if attr.foreground != current_foreground {
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
                            set.expand().color(idx).to(out.by_ref())?;
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

            if attr.background != current_background {
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
                            set.expand().color(idx).to(out.by_ref())?;
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

    fn cursor_up<W: UnixTty + Write>(&mut self, n: u32, out: &mut W) -> anyhow::Result<()> {
        if let Some(attr) = self.get_capability::<cap::ParmUpCursor>() {
            attr.expand().count(n).to(out.by_ref())?;
        } else {
            write!(out, "{}", CSI::Cursor(Cursor::Up(n)))?;
        }
        Ok(())
    }
    fn cursor_down<W: UnixTty + Write>(&mut self, n: u32, out: &mut W) -> anyhow::Result<()> {
        if let Some(attr) = self.get_capability::<cap::ParmDownCursor>() {
            attr.expand().count(n).to(out.by_ref())?;
        } else {
            write!(out, "{}", CSI::Cursor(Cursor::Down(n)))?;
        }
        Ok(())
    }

    fn cursor_left<W: UnixTty + Write>(&mut self, n: u32, out: &mut W) -> anyhow::Result<()> {
        if let Some(attr) = self.get_capability::<cap::ParmLeftCursor>() {
            attr.expand().count(n).to(out.by_ref())?;
        } else {
            write!(out, "{}", CSI::Cursor(Cursor::Left(n)))?;
        }
        Ok(())
    }
    fn cursor_right<W: UnixTty + Write>(&mut self, n: u32, out: &mut W) -> anyhow::Result<()> {
        if let Some(attr) = self.get_capability::<cap::ParmRightCursor>() {
            attr.expand().count(n).to(out.by_ref())?;
        } else {
            write!(out, "{}", CSI::Cursor(Cursor::Right(n)))?;
        }
        Ok(())
    }
}

impl TerminfoRenderer {
    #[cfg_attr(
        feature = "cargo-clippy",
        allow(clippy::cyclomatic_complexity, clippy::cognitive_complexity)
    )]
    pub fn render_to<R: Read, W: UnixTty + Write>(
        &mut self,
        changes: &[Change],
        _read: &mut R,
        out: &mut W,
    ) -> anyhow::Result<()> {
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
                            clr.expand().to(out.by_ref())?;
                        } else {
                            if let Some(attr) = self.get_capability::<cap::CursorHome>() {
                                attr.expand().to(out.by_ref())?;
                            } else {
                                write!(
                                    out,
                                    "{}",
                                    CSI::Cursor(Cursor::Position {
                                        line: OneBased::new(1),
                                        col: OneBased::new(1)
                                    })
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
                            attr.expand().to(out.by_ref())?;
                        } else {
                            write!(
                                out,
                                "{}",
                                CSI::Cursor(Cursor::Position {
                                    line: OneBased::new(1),
                                    col: OneBased::new(1)
                                })
                            )?;
                        }

                        let size = out.get_size()?;
                        let num_spaces = size.ws_col as usize * size.ws_row as usize;
                        let mut buf = Vec::with_capacity(num_spaces);
                        buf.resize(num_spaces, b' ');
                        out.write_all(buf.as_slice())?;
                    }
                }
                Change::ClearToEndOfLine(color) => {
                    // ClearScreen implicitly resets all to default
                    let defaults = CellAttributes::default()
                        .set_background(color.clone())
                        .clone();
                    if self.current_attr != defaults {
                        self.pending_attr = Some(defaults);
                        self.flush_pending_attr(out)?;
                    }
                    self.pending_attr = None;

                    // FIXME: this doesn't behave correctly for terminals without bce.
                    // If we knew the current cursor position, we would be able to
                    // emit the correctly colored background for that case.
                    if let Some(clr) = self.get_capability::<cap::ClrEol>() {
                        clr.expand().to(out.by_ref())?;
                    } else {
                        write!(
                            out,
                            "{}",
                            CSI::Edit(Edit::EraseInLine(EraseInLine::EraseToEndOfLine))
                        )?;
                    }
                }
                Change::ClearToEndOfScreen(color) => {
                    // ClearScreen implicitly resets all to default
                    let defaults = CellAttributes::default()
                        .set_background(color.clone())
                        .clone();
                    if self.current_attr != defaults {
                        self.pending_attr = Some(defaults);
                        self.flush_pending_attr(out)?;
                    }
                    self.pending_attr = None;

                    // FIXME: this doesn't behave correctly for terminals without bce.
                    // If we knew the current cursor position, we would be able to
                    // emit the correctly colored background for that case.
                    if let Some(clr) = self.get_capability::<cap::ClrEos>() {
                        clr.expand().to(out.by_ref())?;
                    } else {
                        write!(
                            out,
                            "{}",
                            CSI::Edit(Edit::EraseInDisplay(EraseInDisplay::EraseToEndOfDisplay))
                        )?;
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
                    out.by_ref().write_all(text.as_bytes())?;
                }
                Change::CursorPosition {
                    x: Position::Absolute(0),
                    y: Position::Relative(1),
                } => {
                    out.by_ref().write_all(b"\r\n")?;
                }
                Change::CursorPosition {
                    x: Position::Absolute(0),
                    y: Position::NoChange,
                }
                | Change::CursorPosition {
                    x: Position::Absolute(0),
                    y: Position::Relative(0),
                } => {
                    out.by_ref().write_all(b"\r")?;
                }
                Change::CursorPosition {
                    x: Position::Absolute(0),
                    y: Position::Absolute(0),
                } => {
                    if let Some(attr) = self.get_capability::<cap::CursorHome>() {
                        attr.expand().to(out.by_ref())?;
                    } else {
                        write!(
                            out,
                            "{}",
                            CSI::Cursor(Cursor::Position {
                                line: OneBased::new(1),
                                col: OneBased::new(1)
                            })
                        )?;
                    }
                }
                Change::CursorPosition {
                    x: Position::NoChange,
                    y: Position::Relative(n),
                } if *n > 0 => {
                    self.cursor_down(*n as u32, out)?;
                }
                Change::CursorPosition {
                    x: Position::NoChange,
                    y: Position::Relative(n),
                } if *n < 0 => {
                    self.cursor_up(-*n as u32, out)?;
                }
                Change::CursorPosition {
                    x: Position::Relative(n),
                    y: Position::NoChange,
                } if *n < 0 => {
                    self.cursor_left(-*n as u32, out)?;
                }
                Change::CursorPosition {
                    x: Position::Relative(n),
                    y: Position::NoChange,
                } if *n > 0 => {
                    self.cursor_right(*n as u32, out)?;
                }
                Change::CursorPosition {
                    x: Position::Absolute(n),
                    y: Position::NoChange,
                } => {
                    out.by_ref().write_all(b"\r")?;
                    if *n > 0 {
                        self.cursor_right(*n as u32, out)?;
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
                        attr.expand().x(x).y(y).to(out.by_ref())?;
                    } else {
                        // We need to manually convert to 1-based as the CSI representation
                        // requires it and there's no automatic conversion.
                        write!(
                            out,
                            "{}",
                            CSI::Cursor(Cursor::Position {
                                line: OneBased::from_zero_based(x),
                                col: OneBased::from_zero_based(y),
                            })
                        )?;
                    }
                }
                Change::CursorPosition { .. } => {
                    error!(
                        "unhandled CursorPosition in TerminfoRenderer::render_to: {:?}",
                        change
                    );
                }
                Change::CursorColor(_color) => {
                    // TODO: this isn't spec'd by terminfo, but some terminals
                    // support it.  Add this to capabilities?
                }
                Change::CursorShape(shape) => match shape {
                    CursorShape::Default => {
                        if let Some(normal) = self.get_capability::<cap::CursorNormal>() {
                            normal.expand().to(out.by_ref())?;
                        } else {
                            if let Some(show) = self.get_capability::<cap::CursorVisible>() {
                                show.expand().to(out.by_ref())?;
                            }
                            if let Some(reset) = self.get_capability::<cap::ResetCursorStyle>() {
                                reset.expand().to(out.by_ref())?;
                            }
                        }
                    }
                    CursorShape::Hidden => {
                        if let Some(hide) = self.get_capability::<cap::CursorInvisible>() {
                            hide.expand().to(out.by_ref())?;
                        }
                    }
                    _ => {
                        if let Some(show) = self.get_capability::<cap::CursorVisible>() {
                            show.expand().to(out.by_ref())?;
                        }
                        let param = match shape {
                            CursorShape::Default | CursorShape::Hidden => unreachable!(),
                            CursorShape::BlinkingBlock => 1,
                            CursorShape::SteadyBlock => 2,
                            CursorShape::BlinkingUnderline => 3,
                            CursorShape::SteadyUnderline => 4,
                            CursorShape::BlinkingBar => 5,
                            CursorShape::SteadyBar => 6,
                        };
                        if let Some(set) = self.get_capability::<cap::SetCursorStyle>() {
                            set.expand().kind(param).to(out.by_ref())?;
                        }
                    }
                },
                Change::Image(image) => {
                    if self.caps.iterm2_image() {
                        let data = if image.top_left == TextureCoordinate::new_f32(0.0, 0.0)
                            && image.bottom_right == TextureCoordinate::new_f32(1.0, 1.0)
                        {
                            // The whole image is requested, so we can send the
                            // original image bytes over
                            image.image.data().to_vec()
                        } else {
                            // TODO: slice out the requested region of the image,
                            // and encode as a PNG.
                            unimplemented!();
                        };

                        let file = ITermFileData {
                            name: None,
                            size: Some(data.len()),
                            width: ITermDimension::Cells(image.width as i64),
                            height: ITermDimension::Cells(image.height as i64),
                            preserve_aspect_ratio: true,
                            inline: true,
                            data,
                        };

                        let osc = OperatingSystemCommand::ITermProprietary(ITermProprietary::File(
                            Box::new(file),
                        ));

                        write!(out, "{}", osc)?;

                    // TODO: } else if self.caps.sixel() {
                    } else {
                        // Blank out the cells and move the cursor to the right spot
                        for y in 0..image.height {
                            for _ in 0..image.width {
                                write!(out, " ")?;
                            }

                            if y != image.height - 1 {
                                writeln!(out)?;
                                self.cursor_left(image.width as u32, out)?;
                            }
                        }
                        self.cursor_up(image.height as u32, out)?;
                    }
                }
                Change::ScrollRegionUp {
                    first_row,
                    region_size,
                    scroll_count,
                } => {
                    if *region_size > 0 {
                        if let Some(csr) = self.get_capability::<cap::ChangeScrollRegion>() {
                            let top = *first_row as u32;
                            let bottom = (*first_row + *region_size - 1) as u32;
                            let scroll_count = *scroll_count as u32;
                            csr.expand().top(top).bottom(bottom).to(out.by_ref())?;
                            if scroll_count > 0 {
                                if let Some(scroll) = self.get_capability::<cap::ParmIndex>() {
                                    scroll.expand().count(scroll_count).to(out.by_ref())?
                                } else {
                                    let scroll = self.get_capability::<cap::ScrollForward>();
                                    let set_position = self.get_capability::<cap::CursorAddress>();
                                    if let (Some(scroll), Some(set_position)) =
                                        (scroll, set_position)
                                    {
                                        set_position.expand().x(0).y(bottom).to(out.by_ref())?;
                                        for _ in 0..scroll_count {
                                            scroll.expand().to(out.by_ref())?
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Change::ScrollRegionDown {
                    first_row,
                    region_size,
                    scroll_count,
                } => {
                    if *region_size > 0 {
                        if let Some(csr) = self.get_capability::<cap::ChangeScrollRegion>() {
                            let top = *first_row as u32;
                            let bottom = (*first_row + *region_size - 1) as u32;
                            let scroll_count = *scroll_count as u32;
                            csr.expand().top(top).bottom(bottom).to(out.by_ref())?;
                            if scroll_count > 0 {
                                if let Some(scroll) = self.get_capability::<cap::ParmRindex>() {
                                    scroll.expand().count(scroll_count).to(out.by_ref())?
                                } else {
                                    let scroll = self.get_capability::<cap::ScrollReverse>();
                                    let set_position = self.get_capability::<cap::CursorAddress>();
                                    if let (Some(scroll), Some(set_position)) =
                                        (scroll, set_position)
                                    {
                                        set_position.expand().x(0).y(top).to(out.by_ref())?;
                                        for _ in 0..scroll_count {
                                            scroll.expand().to(out.by_ref())?
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                Change::Title(_text) => {
                    // Don't actually render this for now.
                    // The primary purpose of Change::Title at the time of
                    // writing is to transfer tab titles across domains
                    // in the wezterm multiplexer model.  It's not clear
                    // that it would be a good idea to unilaterally output
                    // eg: a title change escape sequence here in the
                    // renderer because we might be composing multiple widgets
                    // together, each with its own title.
                }
            }
        }

        self.flush_pending_attr(out)?;
        out.flush()?;
        Ok(())
    }
}

#[cfg(all(test, unix))]
mod test {
    use super::*;
    use crate::caps::ProbeHintsBuilder;
    use crate::color::{AnsiColor, ColorAttribute, RgbColor};
    use crate::escape::parser::Parser;
    use crate::escape::{Action, Esc, EscCode};
    use crate::input::InputEvent;
    use crate::terminal::unix::{Purge, SetAttributeWhen, UnixTty};
    use crate::terminal::ScreenSize;
    use crate::terminal::{cast, Terminal, TerminalWaker};
    use anyhow::bail;
    use libc::winsize;
    use std::io::{Error as IoError, ErrorKind, Read, Result as IoResult, Write};
    use std::mem;
    use std::time::Duration;
    use terminfo;
    use termios::Termios;

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
        )
        .unwrap()
    }

    fn no_terminfo_all_enabled() -> Capabilities {
        Capabilities::new_with_hints(
            ProbeHintsBuilder::default()
                .color_level(Some(ColorLevel::TrueColor))
                .build()
                .unwrap(),
        )
        .unwrap()
    }

    struct FakeTty {
        buf: Vec<u8>,
        size: winsize,
        termios: Termios,
    }

    impl FakeTty {
        fn new_with_size(width: usize, height: usize) -> Self {
            let size = winsize {
                ws_col: cast(width).unwrap(),
                ws_row: cast(height).unwrap(),
                ws_xpixel: 0,
                ws_ypixel: 0,
            };
            let buf = Vec::new();
            Self {
                size,
                buf,
                termios: unsafe { mem::zeroed() },
            }
        }
    }

    impl UnixTty for FakeTty {
        fn get_size(&mut self) -> anyhow::Result<winsize> {
            Ok(self.size.clone())
        }
        fn set_size(&mut self, size: winsize) -> anyhow::Result<()> {
            self.size = size.clone();
            Ok(())
        }
        fn get_termios(&mut self) -> anyhow::Result<Termios> {
            Ok(self.termios.clone())
        }
        fn set_termios(
            &mut self,
            termios: &Termios,
            _when: SetAttributeWhen,
        ) -> anyhow::Result<()> {
            self.termios = termios.clone();
            Ok(())
        }
        /// Waits until all written data has been transmitted.
        fn drain(&mut self) -> anyhow::Result<()> {
            Ok(())
        }
        fn purge(&mut self, _purge: Purge) -> anyhow::Result<()> {
            Ok(())
        }
    }

    impl Read for FakeTty {
        fn read(&mut self, _buf: &mut [u8]) -> Result<usize, IoError> {
            Err(IoError::new(ErrorKind::Other, "not implemented"))
        }
    }
    impl Write for FakeTty {
        fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
            self.buf.write(buf)
        }

        fn flush(&mut self) -> IoResult<()> {
            self.buf.flush()
        }
    }

    struct FakeTerm {
        read: FakeTty,
        write: FakeTty,
        renderer: TerminfoRenderer,
    }

    impl FakeTerm {
        fn new(caps: Capabilities) -> Self {
            Self::new_with_size(caps, 80, 24)
        }

        fn new_with_size(caps: Capabilities, width: usize, height: usize) -> Self {
            let read = FakeTty::new_with_size(width, height);
            let write = FakeTty::new_with_size(width, height);
            let renderer = TerminfoRenderer::new(caps);
            Self {
                read,
                write,
                renderer,
            }
        }

        fn parse(&self) -> Vec<Action> {
            let mut p = Parser::new();
            p.parse_as_vec(&self.write.buf)
        }
    }

    impl Terminal for FakeTerm {
        fn set_raw_mode(&mut self) -> anyhow::Result<()> {
            bail!("not implemented");
        }

        fn set_cooked_mode(&mut self) -> anyhow::Result<()> {
            bail!("not implemented");
        }

        fn enter_alternate_screen(&mut self) -> anyhow::Result<()> {
            bail!("not implemented");
        }

        fn exit_alternate_screen(&mut self) -> anyhow::Result<()> {
            bail!("not implemented");
        }

        fn render(&mut self, changes: &[Change]) -> anyhow::Result<()> {
            self.renderer
                .render_to(changes, &mut self.read, &mut self.write)
        }

        fn get_screen_size(&mut self) -> anyhow::Result<ScreenSize> {
            let size = self.write.get_size()?;
            Ok(ScreenSize {
                rows: cast(size.ws_row)?,
                cols: cast(size.ws_col)?,
                xpixel: cast(size.ws_xpixel)?,
                ypixel: cast(size.ws_ypixel)?,
            })
        }

        fn set_screen_size(&mut self, size: ScreenSize) -> anyhow::Result<()> {
            let size = winsize {
                ws_row: cast(size.rows)?,
                ws_col: cast(size.cols)?,
                ws_xpixel: cast(size.xpixel)?,
                ws_ypixel: cast(size.ypixel)?,
            };

            self.write.set_size(size)
        }

        fn flush(&mut self) -> anyhow::Result<()> {
            Ok(())
        }

        fn poll_input(&mut self, _wait: Option<Duration>) -> anyhow::Result<Option<InputEvent>> {
            bail!("not implemented");
        }

        fn waker(&self) -> TerminalWaker {
            unimplemented!();
        }
    }

    #[test]
    fn empty_render() {
        let mut out = FakeTerm::new(xterm_terminfo());
        out.render(&[]).unwrap();
        assert_eq!("", String::from_utf8(out.write.buf).unwrap());
        assert_eq!(out.renderer.current_attr, CellAttributes::default());
    }

    #[test]
    fn basic_text() {
        let mut out = FakeTerm::new(xterm_terminfo());
        out.render(&[Change::Text("foo".into())]).unwrap();
        assert_eq!("foo", String::from_utf8(out.write.buf).unwrap());
        assert_eq!(out.renderer.current_attr, CellAttributes::default());
    }

    #[test]
    fn bold_text() {
        let mut out = FakeTerm::new(xterm_terminfo());
        out.render(&[
            Change::Text("not ".into()),
            Change::Attribute(AttributeChange::Intensity(Intensity::Bold)),
            Change::Text("foo".into()),
        ])
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
            out.renderer.current_attr,
            CellAttributes::default()
                .set_intensity(Intensity::Bold)
                .clone()
        );
    }

    #[test]
    fn clear_screen() {
        let mut out = FakeTerm::new_with_size(xterm_terminfo(), 4, 3);
        out.render(&[Change::ClearScreen(ColorAttribute::default())])
            .unwrap();

        let result = out.parse();
        assert_eq!(
            result,
            vec![
                Action::CSI(CSI::Cursor(Cursor::Position {
                    line: OneBased::new(1),
                    col: OneBased::new(1)
                })),
                Action::CSI(CSI::Edit(Edit::EraseInDisplay(
                    EraseInDisplay::EraseDisplay,
                ))),
            ]
        );

        assert_eq!(out.renderer.current_attr, CellAttributes::default());
    }

    #[test]
    fn clear_screen_bce() {
        let mut out = FakeTerm::new_with_size(xterm_terminfo(), 4, 3);
        out.render(&[Change::ClearScreen(AnsiColor::Maroon.into())])
            .unwrap();

        let result = out.parse();
        assert_eq!(
            result,
            vec![
                Action::CSI(CSI::Sgr(Sgr::Background(AnsiColor::Maroon.into()))),
                Action::CSI(CSI::Cursor(Cursor::Position {
                    line: OneBased::new(1),
                    col: OneBased::new(1)
                })),
                Action::CSI(CSI::Edit(Edit::EraseInDisplay(
                    EraseInDisplay::EraseDisplay,
                ))),
            ]
        );

        assert_eq!(
            out.renderer.current_attr,
            CellAttributes::default()
                .set_background(AnsiColor::Maroon)
                .clone()
        );
    }

    #[test]
    fn clear_screen_no_terminfo() {
        let mut out = FakeTerm::new_with_size(no_terminfo_all_enabled(), 4, 3);
        out.render(&[Change::ClearScreen(ColorAttribute::default())])
            .unwrap();

        let result = out.parse();
        assert_eq!(
            result,
            vec![
                Action::CSI(CSI::Cursor(Cursor::Position {
                    line: OneBased::new(1),
                    col: OneBased::new(1)
                })),
                Action::CSI(CSI::Edit(Edit::EraseInDisplay(
                    EraseInDisplay::EraseDisplay,
                ))),
            ]
        );

        assert_eq!(out.renderer.current_attr, CellAttributes::default());
    }

    #[test]
    fn clear_screen_bce_no_terminfo() {
        let mut out = FakeTerm::new_with_size(no_terminfo_all_enabled(), 4, 3);
        out.render(&[Change::ClearScreen(AnsiColor::Maroon.into())])
            .unwrap();

        let result = out.parse();
        assert_eq!(
            result,
            vec![
                Action::CSI(CSI::Sgr(Sgr::Background(AnsiColor::Maroon.into()))),
                Action::CSI(CSI::Cursor(Cursor::Position {
                    line: OneBased::new(1),
                    col: OneBased::new(1)
                })),
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
            out.renderer.current_attr,
            CellAttributes::default()
                .set_background(AnsiColor::Maroon)
                .clone()
        );
    }

    #[test]
    fn bold_text_no_terminfo() {
        let mut out = FakeTerm::new(no_terminfo_all_enabled());
        out.render(&[
            Change::Text("not ".into()),
            Change::Attribute(AttributeChange::Intensity(Intensity::Bold)),
            Change::Text("foo".into()),
        ])
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
            out.renderer.current_attr,
            CellAttributes::default()
                .set_intensity(Intensity::Bold)
                .clone()
        );
    }

    #[test]
    fn red_bold_text() {
        let mut out = FakeTerm::new(xterm_terminfo());
        out.render(&[
            Change::Attribute(AttributeChange::Foreground(AnsiColor::Maroon.into())),
            Change::Attribute(AttributeChange::Intensity(Intensity::Bold)),
            Change::Text("red".into()),
            Change::Attribute(AttributeChange::Foreground(AnsiColor::Red.into())),
        ])
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
            out.renderer.current_attr,
            CellAttributes::default()
                .set_intensity(Intensity::Bold)
                .set_foreground(AnsiColor::Red)
                .clone()
        );
    }

    #[test]
    fn red_bold_text_no_terminfo() {
        let mut out = FakeTerm::new(no_terminfo_all_enabled());
        out.render(&[
            Change::Attribute(AttributeChange::Foreground(AnsiColor::Maroon.into())),
            Change::Attribute(AttributeChange::Intensity(Intensity::Bold)),
            Change::Text("red".into()),
            Change::Attribute(AttributeChange::Foreground(AnsiColor::Red.into())),
        ])
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
            out.renderer.current_attr,
            CellAttributes::default()
                .set_intensity(Intensity::Bold)
                .set_foreground(AnsiColor::Red)
                .clone()
        );
    }

    #[test]
    fn color_after_attribute_change() {
        let mut out = FakeTerm::new(xterm_terminfo());
        out.render(&[
            Change::Attribute(AttributeChange::Foreground(AnsiColor::Maroon.into())),
            Change::Attribute(AttributeChange::Intensity(Intensity::Bold)),
            Change::Text("red".into()),
            Change::Attribute(AttributeChange::Intensity(Intensity::Normal)),
            Change::Text("2".into()),
        ])
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
                // Turning off bold is translated into reset and set red again
                Action::Esc(Esc::Code(EscCode::AsciiCharacterSet)),
                Action::CSI(CSI::Sgr(Sgr::Reset)),
                Action::CSI(CSI::Sgr(Sgr::Foreground(AnsiColor::Maroon.into()))),
                Action::Print('2'),
            ]
        );

        assert_eq!(
            out.renderer.current_attr,
            CellAttributes::default()
                .set_foreground(AnsiColor::Maroon)
                .clone()
        );
    }

    #[test]
    fn truecolor() {
        let mut out = FakeTerm::new(xterm_terminfo());
        out.render(&[
            Change::Attribute(AttributeChange::Foreground(
                ColorSpec::TrueColor(RgbColor::new(255, 128, 64)).into(),
            )),
            Change::Text("A".into()),
        ])
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
        let mut out = FakeTerm::new(no_terminfo_all_enabled());
        out.render(&[
            Change::Attribute(AttributeChange::Foreground(
                ColorSpec::TrueColor(RgbColor::new(255, 128, 64)).into(),
            )),
            Change::Text("A".into()),
        ])
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
