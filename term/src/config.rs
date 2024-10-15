use crate::color::ColorPalette;
use downcast_rs::{impl_downcast, Downcast};
use termwiz::cell::UnicodeVersion;
use termwiz::surface::{Line, SequenceNo};
use wezterm_bidi::ParagraphDirectionHint;

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
            // For compatibility with the `nano` editor, which unfortunately
            // treats \n as a shortcut that justifies text
            // <https://savannah.gnu.org/bugs/?49176>, we default to
            // \r which is typically fine.
            // <https://github.com/wez/wezterm/issues/1575>
            Self::CarriageReturn
        }
    }
}

/// TerminalConfiguration allows for the embedding application to pass configuration
/// information to the Terminal.
/// The configuration can be changed at runtime; provided that the implementation
/// increments the generation counter appropriately, the changes will be detected
/// and applied at the next appropriate opportunity.
pub trait TerminalConfiguration: Downcast + std::fmt::Debug + Send + Sync {
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

    /// Returns the default color palette for the application.
    /// Various escape sequences can dynamically modify the effective
    /// color palette for a terminal instance at runtime, but this method
    /// defines the initial palette.
    fn color_palette(&self) -> ColorPalette;

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

    fn enable_kitty_keyboard(&self) -> bool {
        false
    }

    fn enable_osc52_clipboard_reading(&self) -> bool {
        false
    }

    /// The default unicode version to assume.
    /// This affects how the width of certain sequences is interpreted.
    /// At the time of writing, we default to 9 even though the current
    /// version of unicode is 14.  14 introduced emoji presentation selectors
    /// that also alter the width of certain sequences, and that is too
    /// new for most deployed applications.
    // Coupled with config/src/lib.rs:default_unicode_version
    fn unicode_version(&self) -> UnicodeVersion {
        UnicodeVersion {
            version: 9,
            ambiguous_are_wide: false,
        }
    }

    /// Whether to normalize incoming text runs to
    /// canonical NFC unicode representation
    fn normalize_output_to_unicode_nfc(&self) -> bool {
        false
    }

    fn debug_key_events(&self) -> bool {
        false
    }

    /// Returns (bidi_enabled, direction hint) that should be used
    /// unless an escape sequence has changed the default mode
    fn bidi_mode(&self) -> BidiMode {
        BidiMode {
            enabled: false,
            hint: ParagraphDirectionHint::LeftToRight,
        }
    }

    /// Disabled by default per:
    /// <https://marc.info/?l=bugtraq&m=104612710031920&w=2>
    fn enable_title_reporting(&self) -> bool {
        false
    }

    fn log_unknown_escape_sequences(&self) -> bool {
        false
    }
}
impl_downcast!(TerminalConfiguration);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BidiMode {
    pub enabled: bool,
    pub hint: ParagraphDirectionHint,
}

impl BidiMode {
    pub fn apply_to_line(&self, line: &mut Line, seqno: SequenceNo) {
        line.set_bidi_info(self.enabled, self.hint, seqno);
    }
}
