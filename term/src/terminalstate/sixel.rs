use crate::terminalstate::image::*;
use crate::terminalstate::{default_color_map, ImageAttachParams};
use crate::TerminalState;
use ::image::RgbaImage;
use termwiz::color::RgbColor;
use termwiz::escape::{Sixel, SixelData};
use termwiz::image::ImageDataType;

impl TerminalState {
    pub(crate) fn sixel(&mut self, sixel: Box<Sixel>) {
        let (width, height) = sixel.dimensions();

        let mut private_color_map;
        let color_map = if self.use_private_color_registers_for_each_graphic {
            private_color_map = default_color_map();
            &mut private_color_map
        } else {
            &mut self.color_map
        };

        let mut image = if sixel.background_is_transparent {
            RgbaImage::new(width, height)
        } else {
            let background_color = color_map
                .get(&0)
                .cloned()
                .unwrap_or(RgbColor::new_8bpc(0, 0, 0));
            let (red, green, blue) = background_color.to_tuple_rgb8();
            RgbaImage::from_pixel(width, height, [red, green, blue, 0xffu8].into())
        };

        let mut x = 0;
        let mut y = 0;
        let mut foreground_color = RgbColor::new_8bpc(0, 0xff, 0);

        let mut emit_sixel = |d: &u8, foreground_color: &RgbColor, x: u32, y: u32| {
            let (red, green, blue) = foreground_color.to_tuple_rgb8();
            for bitno in 0..6 {
                if y + bitno >= height {
                    break;
                }
                let on = (d & (1 << bitno)) != 0;
                if on {
                    image.get_pixel_mut(x, y + bitno).0 = [red, green, blue, 0xffu8];
                }
            }
        };

        for d in &sixel.data {
            match d {
                SixelData::Data(d) => {
                    emit_sixel(d, &foreground_color, x, y);
                    x += 1;
                }

                SixelData::Repeat { repeat_count, data } => {
                    for _ in 0..*repeat_count {
                        emit_sixel(data, &foreground_color, x, y);
                        x += 1;
                    }
                }

                SixelData::CarriageReturn => x = 0,
                SixelData::NewLine => {
                    x = 0;
                    y += 6;
                }

                SixelData::DefineColorMapRGB { color_number, rgb } => {
                    color_map.insert(*color_number, *rgb);
                }

                SixelData::DefineColorMapHSL {
                    color_number,
                    hue_angle,
                    saturation,
                    lightness,
                } => {
                    use palette::encoding::pixel::Pixel;
                    // Sixel's hue angles are: blue=0, red=120, green=240,
                    // whereas Hsl has red=0, green=120, blue=240.
                    // Looking at red, we need to rotate left by 120 to
                    // go from sixel red to palette::RgbHue red.
                    // Negative values wrap around the circle.
                    // https://github.com/wez/wezterm/issues/775
                    let angle = (*hue_angle as f32) - 120.0;
                    let angle = if angle < 0. { 360.0 + angle } else { angle };
                    let hue = palette::RgbHue::from_degrees(angle);
                    let hsl =
                        palette::Hsl::new(hue, *saturation as f32 / 100., *lightness as f32 / 100.);
                    let rgb: palette::Srgb = hsl.into();
                    let rgb: [u8; 3] = rgb.into_linear().into_format().into_raw();

                    color_map.insert(*color_number, RgbColor::new_8bpc(rgb[0], rgb[1], rgb[2]));
                }

                SixelData::SelectColorMapEntry(n) => {
                    foreground_color = color_map.get(n).cloned().unwrap_or_else(|| {
                        log::error!("sixel selected noexistent colormap entry {}", n);
                        RgbColor::new_8bpc(255, 255, 255)
                    });
                }
            }
        }

        let data = image.into_vec();
        let image_data = ImageDataType::new_single_frame(width, height, data);

        let image_data = self.raw_image_to_image_data(image_data);
        self.assign_image_to_cells(ImageAttachParams {
            image_width: width,
            image_height: height,
            source_width: width,
            source_height: height,
            rows: None,
            columns: None,
            source_origin_x: 0,
            source_origin_y: 0,
            display_offset_x: 0,
            display_offset_y: 0,
            data: image_data,
            style: ImageAttachStyle::Sixel,
            z_index: 0,
            image_id: None,
            placement_id: None,
            do_not_move_cursor: false,
        });
    }
}
