use cell::{AttributeChange, CellAttributes};
use color::ColorAttribute;
use surface::{CursorShape,Position};

/// `Change` describes an update operation to be applied to a `Surface`.
/// Changes to the active attributes (color, style), moving the cursor
/// and outputting text are examples of some of the values.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Change {
    /// Change a single attribute
    Attribute(AttributeChange),
    /// Change all possible attributes to the given set of values
    AllAttributes(CellAttributes),
    /// Add printable text.
    /// Control characters are rendered inert by transforming them
    /// to space.  CR and LF characters are interpreted by moving
    /// the cursor position.  CR moves the cursor to the start of
    /// the line and LF moves the cursor down to the next line.
    /// You typically want to use both together when sending in
    /// a line break.
    Text(String),
    /// Clear the screen to the specified color.
    /// Implicitly clears all attributes prior to clearing the screen.
    /// Moves the cursor to the home position (top left).
    ClearScreen(ColorAttribute),
    /// Clear from the current cursor X position to the rightmost
    /// edge of the screen.  The background color is set to the
    /// provided color.  The cursor position remains unchanged.
    ClearToEndOfLine(ColorAttribute),
    /// Clear from the current cursor X position to the rightmost
    /// edge of the screen on the current line.  Clear all of the
    /// lines below the current cursor Y position.  The background
    /// color is set ot the provided color.  The cursor position
    /// remains unchanged.
    ClearToEndOfScreen(ColorAttribute),
    /// Move the cursor to the specified `Position`.
    CursorPosition { x: Position, y: Position },
    /// Change the cursor color.
    CursorColor(ColorAttribute),
    /// Change the cursor shape
    CursorShape(CursorShape),
    /* ChangeScrollRegion{top: usize, bottom: usize}, */
}

impl Change {
    pub fn is_text(&self) -> bool {
        match self {
            Change::Text(_) => true,
            _ => false,
        }
    }

    pub fn text(&self) -> &str {
        match self {
            Change::Text(text) => text,
            _ => panic!("you must use Change::is_text() to guard calls to Change::text()"),
        }
    }
}

impl<S: Into<String>> From<S> for Change {
    fn from(s: S) -> Self {
        Change::Text(s.into())
    }
}

impl From<AttributeChange> for Change {
    fn from(c: AttributeChange) -> Self {
        Change::Attribute(c)
    }
}
