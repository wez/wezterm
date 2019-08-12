use crate::bitmaps::BitmapImage;
use core_graphics::base::{
    kCGBitmapByteOrder32Little, kCGImageAlphaPremultipliedFirst, kCGRenderingIntentDefault,
};
use core_graphics::color_space::CGColorSpace;
use core_graphics::data_provider::CGDataProvider;
use core_graphics::image::CGImage;
use std::marker::PhantomData;

/// Allows referencing a BitmapImage as a CGImage.
/// The CGImage points to the data owned by the BitmapImage.
/// This type is set up to borrow that data; the compiler
/// will enforce the borrow and keep the code safe.
pub struct BitmapRef<'a> {
    image: CGImage,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> std::ops::Deref for BitmapRef<'a> {
    type Target = CGImage;
    fn deref(&self) -> &CGImage {
        &self.image
    }
}

impl<'a> BitmapRef<'a> {
    pub fn with_image(image: &'a dyn BitmapImage) -> Self {
        let (width, height) = image.image_dimensions();
        let byte_size = width * height * 4;

        // This is safe because BitmapRef<'a> borrows the
        // data from BitmapImage and the compiler will ensure
        // that the lifetime is maintained
        let slice = unsafe {
            let data = image.pixel_data();
            std::slice::from_raw_parts(data, byte_size)
        };
        // This is also safe for the same reason as above
        let provider = unsafe { CGDataProvider::from_slice(slice) };

        let should_interpolate = true;
        let bytes_per_row = width * 4;
        let image = CGImage::new(
            width,
            height,
            8,
            32,
            bytes_per_row,
            &CGColorSpace::create_device_rgb(),
            kCGImageAlphaPremultipliedFirst | kCGBitmapByteOrder32Little,
            &provider,
            should_interpolate,
            kCGRenderingIntentDefault,
        );
        BitmapRef {
            image,
            _phantom: PhantomData,
        }
    }
}
