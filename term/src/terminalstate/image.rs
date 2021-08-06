use crate::{Position, StableRowIndex, TerminalState};
use ordered_float::NotNan;
use std::sync::Arc;
use termwiz::cell::{Cell, CellAttributes};
use termwiz::image::{ImageCell, ImageDataType};
use termwiz::surface::change::ImageData;
use termwiz::surface::TextureCoordinate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacementInfo {
    pub first_row: StableRowIndex,
    pub rows: usize,
    pub cols: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ImageAttachParams {
    /// Dimensions of the underlying ImageData, in pixels
    pub image_width: u32,
    pub image_height: u32,

    /// Dimensions of the area of the image to be displayed, in pixels
    pub source_width: u32,
    pub source_height: u32,

    /// Origin of the source data region, top left corner in pixels
    pub source_origin_x: u32,
    pub source_origin_y: u32,

    /// When rendering in the cell, use this offset from the top left
    /// of the cell
    pub display_offset_x: u32,
    pub display_offset_y: u32,

    /// Plane on which to display the image
    pub z_index: i32,

    /// Desired number of cells to span.
    /// If None, then compute based on source_width and source_height
    pub columns: Option<usize>,
    pub rows: Option<usize>,

    pub image_id: Option<u32>,
    pub placement_id: Option<u32>,

    pub style: ImageAttachStyle,
    pub do_not_move_cursor: bool,

    pub data: Arc<ImageData>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageAttachStyle {
    Sixel,
    Iterm,
    Kitty,
}

impl TerminalState {
    pub(crate) fn assign_image_to_cells(&mut self, params: ImageAttachParams) -> PlacementInfo {
        let physical_cols = self.screen().physical_cols;
        let physical_rows = self.screen().physical_rows;
        let cell_pixel_width = self.pixel_width / physical_cols;
        let cell_pixel_height = self.pixel_height / physical_rows;

        let avail_width = params.image_width.saturating_sub(params.source_origin_x);
        let avail_height = params.image_height.saturating_sub(params.source_origin_y);
        let source_width = params.source_width.min(params.image_width).min(avail_width);
        let source_height = params
            .source_height
            .min(params.image_height)
            .min(avail_height);

        let width_in_cells = params
            .columns
            .unwrap_or_else(|| (source_width as f32 / cell_pixel_width as f32).ceil() as usize);
        let height_in_cells = params
            .rows
            .unwrap_or_else(|| (source_height as f32 / cell_pixel_height as f32).ceil() as usize);

        let first_row = self.screen().visible_row_to_stable_row(self.cursor.y);

        let mut ypos =
            NotNan::new(params.source_origin_y as f32 / params.image_height as f32).unwrap();
        let start_xpos =
            NotNan::new(params.source_origin_x as f32 / params.image_width as f32).unwrap();

        let cursor_x = self.cursor.x;
        let x_delta = (source_width as f32 / params.image_width as f32) / width_in_cells as f32;
        let y_delta = (source_height as f32 / params.image_height as f32) / height_in_cells as f32;
        log::debug!(
            "image is {}x{} cells, {:?}, x_delta:{} y_delta:{} ({}x{}@{}x{})",
            width_in_cells,
            height_in_cells,
            params,
            x_delta,
            y_delta,
            physical_cols,
            physical_rows,
            self.pixel_width,
            self.pixel_height
        );

        let height_in_cells = if params.do_not_move_cursor {
            height_in_cells.min(self.screen().physical_rows - self.cursor.y as usize)
        } else {
            height_in_cells
        };

        for y in 0..height_in_cells {
            let mut xpos = start_xpos;
            let cursor_y = if params.do_not_move_cursor {
                self.cursor.y + y as i64
            } else {
                self.cursor.y
            };
            log::debug!(
                "setting cells for y={} x=[{}..{}]",
                cursor_y,
                cursor_x,
                cursor_x + width_in_cells
            );
            for x in 0..width_in_cells {
                let mut cell = self
                    .screen()
                    .get_cell(cursor_x + x, cursor_y)
                    .cloned()
                    .unwrap_or_else(|| Cell::new(' ', CellAttributes::default()));
                let img = Box::new(ImageCell::with_z_index(
                    TextureCoordinate::new(xpos, ypos),
                    TextureCoordinate::new(xpos + x_delta, ypos + y_delta),
                    params.data.clone(),
                    params.z_index,
                    params.display_offset_x,
                    params.display_offset_y,
                    params.image_id,
                    params.placement_id,
                ));
                match params.style {
                    ImageAttachStyle::Kitty => cell.attrs_mut().attach_image(img),
                    ImageAttachStyle::Sixel | ImageAttachStyle::Iterm => {
                        cell.attrs_mut().set_image(img)
                    }
                };

                self.screen_mut().set_cell(cursor_x + x, cursor_y, &cell);
                xpos += x_delta;
            }
            ypos += y_delta;
            if !params.do_not_move_cursor {
                self.new_line(false);
            }
        }

        if !params.do_not_move_cursor {
            // Sixel places the cursor under the left corner of the image,
            // unless sixel_scrolls_right is enabled.
            // iTerm places it after the bottom right corner.
            let bottom_right = match params.style {
                ImageAttachStyle::Kitty | ImageAttachStyle::Iterm => true,
                ImageAttachStyle::Sixel => self.sixel_scrolls_right,
            };

            if bottom_right {
                self.set_cursor_pos(
                    &Position::Relative(width_in_cells as i64),
                    &Position::Relative(-1),
                );
            }
        }

        PlacementInfo {
            first_row,
            rows: height_in_cells,
            cols: width_in_cells,
        }
    }

    /// cache recent images and avoid assigning a new id for repeated data!
    pub(crate) fn raw_image_to_image_data(&mut self, data: ImageDataType) -> Arc<ImageData> {
        let data = data.decode();
        let key = data.compute_hash();
        if let Some(item) = self.image_cache.get(&key) {
            Arc::clone(item)
        } else {
            let image_data = Arc::new(ImageData::with_data(data));
            self.image_cache.put(key, Arc::clone(&image_data));
            image_data
        }
    }
}
