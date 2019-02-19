use crate::cell::{AttributeChange, CellAttributes};
use crate::color::ColorAttribute;
pub use crate::image::{ImageData, TextureCoordinate};
use crate::surface::{CursorShape, Position};
use std::rc::Rc;

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
    /// Place an image at the current cursor position.
    /// The image defines the dimensions in cells.
    /// TODO: check iterm rendering behavior when the image is larger than the width of the screen.
    /// If the image is taller than the remaining space at the bottom
    /// of the screen, the screen will scroll up.
    /// The cursor Y position is unchanged by rendering the Image.
    /// The cursor X position will be incremented by `Image::width` cells.
    Image(Image),
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

/// The `Image` `Change` needs to support adding an image that spans multiple
/// rows and columns, as well as model the content for just one of those cells.
/// For instance, if some of the cells inside an image are replaced by textual
/// content, and the screen is scrolled, computing the diff change stream needs
/// to be able to express that a single cell holds a slice from a larger image.
/// The `Image` struct expresses its dimensions in cells and references a region
/// in the shared source image data using texture coordinates.
/// A 4x3 cell image would set `width=3`, `height=3`, `top_left=(0,0)`, `bottom_right=(1,1)`.
/// The top left cell from that image, if it were to be included in a diff,
/// would be recorded as `width=1`, `height=1`, `top_left=(0,0)`, `bottom_right=(1/4,1/3)`.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Image {
    /// measured in cells
    pub width: usize,
    /// measure in cells
    pub height: usize,
    /// Texture coordinate for the top left of this image block.
    /// (0,0) is the top left of the ImageData. (1, 1) is
    /// the bottom right.
    pub top_left: TextureCoordinate,
    /// Texture coordinates for the bottom right of this image block.
    pub bottom_right: TextureCoordinate,
    /// the image data
    pub image: Rc<ImageData>,
}
