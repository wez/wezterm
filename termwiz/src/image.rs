//! Images.
//! This module has some helpers for modeling terminal cells that are filled
//! with image data.
//! We're targeting the iTerm image protocol initially, with sixel as an obvious
//! follow up.
// Kitty has an extensive and complex graphics protocol that seems difficult
// to model.  Its docs are here:
// <https://github.com/kovidgoyal/kitty/blob/master/docs/graphics-protocol.rst>
// Both iTerm2 and Sixel appear to have semantics that allow replacing the
// contents of a single chararcter cell with image data, whereas the kitty
// protocol appears to track the images out of band as attachments with
// z-order.

use ordered_float::NotNaN;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureCoordinate {
    pub x: NotNaN<f32>,
    pub y: NotNaN<f32>,
}

impl TextureCoordinate {
    pub fn new(x: NotNaN<f32>, y: NotNaN<f32>) -> Self {
        Self { x, y }
    }

    pub fn new_f32(x: f32, y: f32) -> Self {
        let x = NotNaN::new(x).unwrap();
        let y = NotNaN::new(y).unwrap();
        Self::new(x, y)
    }
}

/// Tracks data for displaying an image in the place of the normal cell
/// character data.  Since an Image can span multiple cells, we need to logically
/// carve up the image and track each slice of it.  Each cell needs to know
/// its "texture coordinates" within that image so that we can render the
/// right slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageCell {
    /// Texture coordinate for the top left of this cell.
    /// (0,0) is the top left of the ImageData. (1, 1) is
    /// the bottom right.
    top_left: TextureCoordinate,
    /// Texture coordinates for the bottom right of this cell.
    bottom_right: TextureCoordinate,
    /// References the underlying image data
    data: Rc<ImageData>,
}

impl ImageCell {
    pub fn new(
        top_left: TextureCoordinate,
        bottom_right: TextureCoordinate,
        data: Rc<ImageData>,
    ) -> Self {
        Self {
            top_left,
            bottom_right,
            data,
        }
    }
}

static IMAGE_ID: ::std::sync::atomic::AtomicUsize = ::std::sync::atomic::ATOMIC_USIZE_INIT;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageData {
    id: usize,
    /// The image data bytes.  Data is the native image file format
    data: Vec<u8>,
}

impl ImageData {
    /// Create a new ImageData struct with the provided raw data.
    pub fn with_raw_data(data: Vec<u8>) -> Self {
        let id = IMAGE_ID.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed);
        Self { id, data }
    }

    #[inline]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    #[inline]
    pub fn id(&self) -> usize {
        self.id
    }
}
