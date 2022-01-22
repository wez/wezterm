use crate::color::ColorPalette;
use termwiz::hyperlink::Rule as HyperlinkRule;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewlineCanon {
    None,
    LineFeed,
    CarriageReturn,
    CarriageReturnAndLineFeed,
}

impl NewlineCanon {
    fn target(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::LineFeed => Some("\n"),
            Self::CarriageReturn => Some("\r"),
            Self::CarriageReturnAndLineFeed => Some("\r\n"),
        }
    }

    pub fn canonicalize(self, text: &str) -> String {
        let target = self.target();
        let mut buf = String::new();
        let mut iter = text.chars().peekable();
        while let Some(c) = iter.next() {
            match target {
                None => buf.push(c),
                Some(canon) => {
                    if c == '\n' {
                        buf.push_str(canon);
                    } else if c == '\r' {
                        buf.push_str(canon);
                        if let Some('\n') = iter.peek() {
                            // Paired with the \r, so consume this one
                            iter.next();
                        }
                    } else {
                        buf.push(c);
                    }
                }
            }
        }
        buf
    }
}

#[cfg(test)]
#[test]
fn test_canon() {
    assert_eq!(
        "hello\nthere",
        NewlineCanon::None.canonicalize("hello\nthere")
    );
    assert_eq!(
        "hello\r\nthere",
        NewlineCanon::CarriageReturnAndLineFeed.canonicalize("hello\nthere")
    );
    assert_eq!(
        "hello\rthere",
        NewlineCanon::CarriageReturn.canonicalize("hello\nthere")
    );
    assert_eq!(
        "hello\nthere",
        NewlineCanon::LineFeed.canonicalize("hello\nthere")
    );
    assert_eq!(
        "hello\nthere",
        NewlineCanon::LineFeed.canonicalize("hello\r\nthere")
    );
    assert_eq!(
        "hello\nthere",
        NewlineCanon::LineFeed.canonicalize("hello\rthere")
    );
    assert_eq!(
        "hello\n\nthere",
        NewlineCanon::LineFeed.canonicalize("hello\r\rthere")
    );
    assert_eq!(
        "hello\n\nthere",
        NewlineCanon::LineFeed.canonicalize("hello\r\n\rthere")
    );
    assert_eq!(
        "hello\n\nthere",
        NewlineCanon::LineFeed.canonicalize("hello\r\n\nthere")
    );
    assert_eq!(
        "hello\n\nthere",
        NewlineCanon::LineFeed.canonicalize("hello\r\n\r\nthere")
    );
    assert_eq!(
        "hello\n\n\nthere",
        NewlineCanon::LineFeed.canonicalize("hello\r\r\n\nthere")
    );
}

impl Default for NewlineCanon {
    fn default() -> Self {
        // This is a bit horrible; in general we try to stick with unix line
        // endings as the one-true representation because using canonical
        // CRLF can result in excess blank lines during a paste operation.
        // On Windows we're in a bit of a frustrating situation: pasting into
        // Windows console programs requires CRLF otherwise there is no newline
        // at all, but when in WSL, pasting with CRLF gives excess blank lines.
        //
        // To come to a compromise, if wezterm is running on Windows then we'll
        // use canonical CRLF unless the embedded application has enabled
        // bracketed paste: we can use bracketed paste mode as a signal that
        // the application will prefer newlines.
        //
        // In practice this means that unix shells and vim will get the
        // unix newlines in their pastes (which is the UX I want) and
        // cmd.exe will get CRLF.
        if cfg!(windows) {
            Self::CarriageReturnAndLineFeed
        } else {
            Self::None
        }
    }
}

/// TerminalConfiguration allows for the embedding application to pass configuration
/// information to the Terminal.
/// The configuration can be changed at runtime; provided that the implementation
/// increments the generation counter appropriately, the changes will be detected
/// and applied at the next appropriate opportunity.
pub trait TerminalConfiguration: std::fmt::Debug {
    /// Returns a generation counter for the active
    /// configuration.  If the implementation may be
    /// changed at runtime, it must increment the generation
    /// number with each change so that any caches maintained
    /// by the terminal can be flushed.
    fn generation(&self) -> usize {
        0
    }

    /// Returns the size of the scrollback in terms of the number of rows.
    fn scrollback_size(&self) -> usize {
        3500
    }

    /// Return true if the embedding application wants to use CSI-u encoding
    /// for keys that would otherwise be ambiguous.
    /// <http://www.leonerd.org.uk/hacks/fixterms/>
    fn enable_csi_u_key_encoding(&self) -> bool {
        false
    }

    /// Returns the current generation and its associated hyperlink rules.
    /// hyperlink rules are used to recognize and automatically generate
    /// hyperlink attributes for runs of text that match the provided rules.
    fn hyperlink_rules(&self) -> (usize, Vec<HyperlinkRule>) {
        (self.generation(), vec![])
    }

    /// Returns the default color palette for the application.
    /// Various escape sequences can dynamically modify the effective
    /// color palette for a terminal instance at runtime, but this method
    /// defines the initial palette.
    fn color_palette(&self) -> ColorPalette;

    /// Return true if a resize operation should consider rows that have
    /// made it to scrollback as being immutable.
    /// When immutable, the resize operation will pad out the screen height
    /// with additional blank rows and due to implementation details means
    /// that the user will need to scroll back the scrollbar post-resize
    /// than they would otherwise.
    ///
    /// When mutable, resizing the window taller won't add extra rows;
    /// instead the resize will tend to have "bottom gravity" meaning that
    /// making the window taller will reveal more history than in the other
    /// mode.
    ///
    /// mutable is generally speaking a nicer experience.
    ///
    /// On Windows, the PTY layer doesn't play well with a mutable scrollback,
    /// frequently moving the cursor up to high and erasing portions of the
    /// screen.
    ///
    /// This behavior only happens with the windows pty layer; it doesn't
    /// manifest when using eg: ssh directly to a remote unix system.
    ///
    /// Ideally we'd have this return `true` only for the native windows
    /// pty layer, but for the sake of simplicity, we make this conditional
    /// on being a windows build.
    fn resize_preserves_scrollback(&self) -> bool {
        cfg!(windows)
    }

    fn canonicalize_pasted_newlines(&self) -> NewlineCanon {
        NewlineCanon::default()
    }

    fn alternate_buffer_wheel_scroll_speed(&self) -> u8 {
        3
    }

    fn enq_answerback(&self) -> String {
        "".to_string()
    }

    fn enable_kitty_graphics(&self) -> bool {
        false
    }

    /// The default unicode version to assume.
    /// This affects how the width of certain sequences is interpreted.
    /// At the time of writing, we default to 9 even though the current
    /// version of unicode is 14.  14 introduced emoji presentation selectors
    /// that also alter the width of certain sequences, and that is too
    /// new for most deployed applications.
    // Coupled with config/src/lib.rs:default_unicode_version
    fn unicode_version(&self) -> u8 {
        9
    }

    fn debug_key_events(&self) -> bool {
        false
    }
}
