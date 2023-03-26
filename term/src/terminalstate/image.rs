use crate::{Position, StableRowIndex, TerminalState};
use anyhow::Context;
use humansize::{SizeFormatter, DECIMAL};
use ordered_float::NotNan;
use std::sync::Arc;
use termwiz::cell::Cell;
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
    pub padding_left: u16,
    pub padding_top: u16,

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
    pub(crate) fn assign_image_to_cells(
        &mut self,
        params: ImageAttachParams,
    ) -> anyhow::Result<PlacementInfo> {
        let seqno = self.seqno;
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

        let aspect = source_width as f32 / source_height as f32;

        let width_in_cells = params
            .columns
            .unwrap_or_else(|| (source_width as f32 / cell_pixel_width as f32).ceil() as usize);
        let height_in_cells = params
            .rows
            .unwrap_or_else(|| (source_height as f32 / cell_pixel_height as f32).ceil() as usize);

        // Figure out the desired pixel dimensions, respecting the original
        // aspect of the picture if they specific rows/columns as the max size.
        let target_pixel_width = if params.columns.is_some() {
            if source_width > source_height {
                width_in_cells * cell_pixel_width
            } else {
                ((height_in_cells * cell_pixel_height) as f32 * aspect).ceil() as usize
            }
        } else {
            source_width as usize
        };
        let target_pixel_height = if params.rows.is_some() {
            if source_height > source_width {
                height_in_cells * cell_pixel_height
            } else {
                ((width_in_cells * cell_pixel_width) as f32 / aspect).ceil() as usize
            }
        } else {
            source_height as usize
        };

        let first_row = self.screen().visible_row_to_stable_row(self.cursor.y);

        let mut ypos = NotNan::new(params.source_origin_y as f32 / params.image_height as f32)
            .with_context(|| format!("computing ypos {params:#?}"))?;
        let start_xpos = NotNan::new(params.source_origin_x as f32 / params.image_width as f32)
            .context("computing xpos")?;

        let cursor_x = self.cursor.x;
        log::debug!(
            "image is {}x{} cells (cell is {}x{}), target pixel dims {}x{}, {:?}, (term is {}x{}@{}x{})",
            width_in_cells,
            height_in_cells,
            cell_pixel_width,
            cell_pixel_height,
            target_pixel_width,
            target_pixel_height,
            params,
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

        let mut remain_y = target_pixel_height as usize;
        for y in 0..height_in_cells {
            let padding_bottom = cell_pixel_height.saturating_sub(remain_y) as u16;
            let y_delta = (remain_y.min(cell_pixel_height) as f32) / (target_pixel_height as f32);
            remain_y = remain_y.saturating_sub(cell_pixel_height);

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
            let mut remain_x = target_pixel_width as usize;
            for x in 0..width_in_cells {
                let padding_right = cell_pixel_width.saturating_sub(remain_x) as u16;
                let x_delta = (remain_x.min(cell_pixel_width) as f32) / (target_pixel_width as f32);
                log::debug!(
                    "x_delta {} ({} px), y_delta {} ({} px), padding_right={}, padding_bottom={}",
                    x_delta,
                    x_delta * source_width as f32,
                    y_delta,
                    y_delta * source_width as f32,
                    padding_right,
                    padding_bottom
                );
                remain_x = remain_x.saturating_sub(cell_pixel_width);
                let mut cell = self
                    .screen_mut()
                    .get_cell(cursor_x + x, cursor_y)
                    .cloned()
                    .unwrap_or_else(Cell::blank);
                let img = Box::new(ImageCell::with_z_index(
                    TextureCoordinate::new(xpos, ypos),
                    TextureCoordinate::new(xpos + x_delta, ypos + y_delta),
                    params.data.clone(),
                    params.z_index,
                    params.padding_left,
                    params.padding_top,
                    padding_right,
                    padding_bottom,
                    params.image_id,
                    params.placement_id,
                ));
                match params.style {
                    ImageAttachStyle::Kitty => cell.attrs_mut().attach_image(img),
                    ImageAttachStyle::Sixel | ImageAttachStyle::Iterm => {
                        cell.attrs_mut().set_image(img)
                    }
                };

                self.screen_mut()
                    .set_cell(cursor_x + x, cursor_y, &cell, seqno);
                xpos += x_delta;
            }
            ypos += y_delta;
            if !params.do_not_move_cursor && y < height_in_cells - 1 {
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
                    &Position::Relative(0),
                );
            }
        }

        Ok(PlacementInfo {
            first_row,
            rows: height_in_cells,
            cols: width_in_cells,
        })
    }

    /// cache recent images and avoid assigning a new id for repeated data!
    pub(crate) fn raw_image_to_image_data(
        &mut self,
        data: ImageDataType,
    ) -> Result<Arc<ImageData>, termwiz::error::InternalError> {
        let key = data.compute_hash();
        if let Some(item) = self.image_cache.get(&key) {
            Ok(Arc::clone(item))
        } else {
            let data = data.swap_out()?;
            let image_data = Arc::new(ImageData::with_data(data));
            self.image_cache.put(key, Arc::clone(&image_data));
            Ok(image_data)
        }
    }
}

pub(crate) fn check_image_dimensions(width: u32, height: u32) -> anyhow::Result<()> {
    const MAX_IMAGE_SIZE: u32 = 100_000_000;
    let size = width.saturating_mul(height).saturating_mul(4);
    if size > MAX_IMAGE_SIZE {
        anyhow::bail!(
            "Ignoring image data for image with dimensions {}x{} \
             because required RAM {} > max allowed {}",
            width,
            height,
            SizeFormatter::new(size, DECIMAL),
            SizeFormatter::new(MAX_IMAGE_SIZE, DECIMAL),
        );
    }
    if size == 0 {
        anyhow::bail!("Ignoring image with 0x0 dimensions");
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub format: image::ImageFormat,
}

pub(crate) fn dimensions(data: &[u8]) -> anyhow::Result<ImageInfo> {
    let reader = image::io::Reader::new(std::io::Cursor::new(data)).with_guessed_format()?;
    let format = reader
        .format()
        .ok_or_else(|| anyhow::anyhow!("unknown format!?"))?;
    let (width, height) = reader.into_dimensions()?;
    Ok(ImageInfo {
        width,
        height,
        format,
    })
}
