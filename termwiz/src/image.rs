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
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_derive::*;
use std::sync::Arc;

fn deserialize_notnan<'de, D>(deserializer: D) -> Result<NotNaN<f32>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = f32::deserialize(deserializer)?;
    NotNaN::new(value).map_err(|e| serde::de::Error::custom(format!("{:?}", e)))
}

#[cfg_attr(feature = "cargo-clippy", allow(clippy::trivially_copy_pass_by_ref))]
fn serialize_notnan<S>(value: &NotNaN<f32>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    value.into_inner().serialize(serializer)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextureCoordinate {
    #[serde(
        deserialize_with = "deserialize_notnan",
        serialize_with = "serialize_notnan"
    )]
    pub x: NotNaN<f32>,
    #[serde(
        deserialize_with = "deserialize_notnan",
        serialize_with = "serialize_notnan"
    )]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageCell {
    /// Texture coordinate for the top left of this cell.
    /// (0,0) is the top left of the ImageData. (1, 1) is
    /// the bottom right.
    top_left: TextureCoordinate,
    /// Texture coordinates for the bottom right of this cell.
    bottom_right: TextureCoordinate,
    /// References the underlying image data
    data: Arc<ImageData>,
}

impl ImageCell {
    pub fn new(
        top_left: TextureCoordinate,
        bottom_right: TextureCoordinate,
        data: Arc<ImageData>,
    ) -> Self {
        Self {
            top_left,
            bottom_right,
            data,
        }
    }
}

static IMAGE_ID: ::std::sync::atomic::AtomicUsize = ::std::sync::atomic::AtomicUsize::new(0);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
