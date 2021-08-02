use crate::terminalstate::image::*;
use crate::TerminalState;
use ::image::imageops::FilterType;
use ::image::ImageFormat;
use log::error;
use termwiz::escape::osc::ITermFileData;
use termwiz::image::ImageDataType;

impl TerminalState {
    pub(crate) fn set_image(&mut self, image: ITermFileData) {
        if !image.inline {
            error!(
                "Ignoring file download request name={:?} size={}",
                image.name,
                image.data.len()
            );
            return;
        }

        struct Info {
            width: u32,
            height: u32,
            format: ImageFormat,
        }

        fn dimensions(data: &[u8]) -> anyhow::Result<Info> {
            let reader =
                image::io::Reader::new(std::io::Cursor::new(data)).with_guessed_format()?;
            let format = reader
                .format()
                .ok_or_else(|| anyhow::anyhow!("unknown format!?"))?;
            let (width, height) = reader.into_dimensions()?;
            Ok(Info {
                width,
                height,
                format,
            })
        }

        let info = match dimensions(&image.data) {
            Ok(dims) => dims,
            Err(e) => {
                error!(
                    "Unable to decode image: {}: size={} {:?}",
                    e,
                    image.data.len(),
                    image
                );
                return;
            }
        };

        // Figure out the dimensions.
        let physical_cols = self.screen().physical_cols;
        let physical_rows = self.screen().physical_rows;
        let cell_pixel_width = self.pixel_width / physical_cols;
        let cell_pixel_height = self.pixel_height / physical_rows;

        let width = image.width.to_pixels(cell_pixel_width, physical_cols);
        let height = image.height.to_pixels(cell_pixel_height, physical_rows);

        // Compute any Automatic dimensions
        let aspect = info.width as f32 / info.height as f32;

        let (width, height) = match (width, height) {
            (None, None) => {
                // Take the image's native size
                let width = info.width as usize;
                let height = info.height as usize;
                // but ensure that it fits
                if width as usize > self.pixel_width || height as usize > self.pixel_height {
                    let width = width as f32;
                    let height = height as f32;
                    let mut candidates = vec![];

                    let x_scale = self.pixel_width as f32 / width;
                    if height * x_scale <= self.pixel_height as f32 {
                        candidates.push((self.pixel_width, (height * x_scale) as usize));
                    }
                    let y_scale = self.pixel_height as f32 / height;
                    if width * y_scale <= self.pixel_width as f32 {
                        candidates.push(((width * y_scale) as usize, self.pixel_height));
                    }

                    candidates.sort_by(|a, b| (a.0 * a.1).cmp(&(b.0 * b.1)));

                    candidates.pop().unwrap()
                } else {
                    (width, height)
                }
            }
            (Some(w), None) => {
                let h = w as f32 / aspect;
                (w, h as usize)
            }
            (None, Some(h)) => {
                let w = h as f32 * aspect;
                (w as usize, h)
            }
            (Some(w), Some(_)) if image.preserve_aspect_ratio => {
                let h = w as f32 / aspect;
                (w, h as usize)
            }
            (Some(w), Some(h)) => (w, h),
        };

        let downscaled = (width < info.width as usize) || (height < info.height as usize);
        let data = match (downscaled, info.format) {
            (true, ImageFormat::Gif) | (true, ImageFormat::Png) | (false, _) => {
                // Don't resample things that might be animations,
                // or things that don't need resampling
                ImageDataType::EncodedFile(image.data)
            }
            (true, _) => match ::image::load_from_memory(&image.data) {
                Ok(im) => {
                    let im = im.resize_exact(width as u32, height as u32, FilterType::CatmullRom);
                    let data = im.into_rgba8().into_vec().into_boxed_slice();
                    ImageDataType::Rgba8 {
                        width: width as u32,
                        height: height as u32,
                        data,
                    }
                }
                Err(_) => ImageDataType::EncodedFile(image.data),
            },
        };

        let image_data = self.raw_image_to_image_data(data);
        self.assign_image_to_cells(ImageAttachParams {
            image_width: width as u32,
            image_height: height as u32,
            source_width: width as u32,
            source_height: height as u32,
            source_origin_x: 0,
            source_origin_y: 0,
            display_offset_x: 0,
            display_offset_y: 0,
            z_index: 0,
            columns: None,
            rows: None,
            data: image_data,
            style: ImageAttachStyle::Iterm,
            image_id: 0,
            placement_id: None,
        });
    }
}
