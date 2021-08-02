use crate::terminalstate::image::*;
use crate::terminalstate::{ImageAttachParams, PlacementInfo};
use crate::{StableRowIndex, TerminalState};
use ::image::{DynamicImage, GenericImageView, RgbImage};
use anyhow::Context;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use termwiz::escape::apc::KittyImageData;
use termwiz::escape::apc::{
    KittyImage, KittyImageCompression, KittyImageDelete, KittyImageFormat, KittyImagePlacement,
    KittyImageTransmit, KittyImageVerbosity,
};
use termwiz::image::ImageDataType;
use termwiz::surface::change::ImageData;

#[derive(Debug, Default)]
pub struct KittyImageState {
    accumulator: Vec<KittyImage>,
    max_image_id: u32,
    number_to_id: HashMap<u32, u32>,
    id_to_data: HashMap<u32, Arc<ImageData>>,
    placements: HashMap<(u32, Option<u32>), PlacementInfo>,
    used_memory: usize,
}

impl KittyImageState {
    fn remove_data_for_id(&mut self, image_id: u32) {
        if let Some(data) = self.id_to_data.remove(&image_id) {
            self.used_memory = self.used_memory.saturating_sub(data.len());
        }
    }

    fn record_id_to_data(&mut self, image_id: u32, data: Arc<ImageData>) {
        self.remove_data_for_id(image_id);
        self.prune_unreferenced();
        self.used_memory += data.len();
        self.id_to_data.insert(image_id, data);
    }

    fn prune_unreferenced(&mut self) {
        let budget = 320 * 1024 * 1024; // FIXME: make this configurable
        if self.used_memory > budget {
            let referenced: HashSet<u32> = self.placements.keys().map(|(k, _)| *k).collect();
            let target = self.used_memory - budget;
            let mut freed = 0;
            self.id_to_data.retain(|id, data| {
                if referenced.contains(id) || freed > target {
                    true
                } else {
                    freed += data.len();
                    false
                }
            });

            log::info!(
                "using {} RAM for images, pruned {}",
                self.used_memory,
                freed
            );
            self.used_memory = self.used_memory.saturating_sub(freed);
        }
    }
}

impl TerminalState {
    fn kitty_img_place(
        &mut self,
        image_id: Option<u32>,
        image_number: Option<u32>,
        placement: KittyImagePlacement,
        verbosity: KittyImageVerbosity,
    ) -> anyhow::Result<()> {
        let image_id = match image_id {
            Some(id) => id,
            None => *self
                .kitty_img
                .number_to_id
                .get(
                    &image_number
                        .ok_or_else(|| anyhow::anyhow!("no image_id or image_number specified!"))?,
                )
                .ok_or_else(|| anyhow::anyhow!("image_number has no matching image id"))?,
        };

        log::trace!(
            "kitty_img_place image_id {:?} image_no {:?} placement {:?} verb {:?}",
            image_id,
            image_number,
            placement,
            verbosity
        );
        self.kitty_remove_placement(image_id, placement.placement_id);
        let img = Arc::clone(
            self.kitty_img
                .id_to_data
                .get(&image_id)
                .ok_or_else(|| anyhow::anyhow!("no matching image id"))?,
        );

        let (image_width, image_height) = match img.data() {
            ImageDataType::EncodedFile(data) => {
                let decoded = ::image::load_from_memory(data).context("decode png")?;
                decoded.dimensions()
            }
            ImageDataType::Rgba8 { width, height, .. } => (*width, *height),
        };

        let saved_cursor = self.cursor.clone();

        let info = self.assign_image_to_cells(ImageAttachParams {
            image_width,
            image_height,
            source_width: placement.w.unwrap_or(image_width),
            source_height: placement.h.unwrap_or(image_height),
            source_origin_x: placement.x.unwrap_or(0),
            source_origin_y: placement.y.unwrap_or(0),
            display_offset_x: placement.x_offset.unwrap_or(0),
            display_offset_y: placement.y_offset.unwrap_or(0),
            data: img,
            style: ImageAttachStyle::Kitty,
            z_index: placement.z_index.unwrap_or(0),
            columns: placement.columns.map(|x| x as usize),
            rows: placement.rows.map(|x| x as usize),
            image_id,
            placement_id: placement.placement_id,
        });

        self.kitty_img
            .placements
            .insert((image_id, placement.placement_id), info);
        log::trace!(
            "record placement for {} {:?}",
            image_id,
            placement.placement_id
        );

        if placement.do_not_move_cursor {
            self.cursor = saved_cursor;
        }

        Ok(())
    }

    fn kitty_img_inner(&mut self, img: KittyImage) -> anyhow::Result<()> {
        match self
            .coalesce_kitty_accumulation(img)
            .context("coalesce_kitty_accumulation")?
        {
            KittyImage::TransmitData {
                transmit,
                verbosity,
            } => {
                self.kitty_img_transmit(transmit, verbosity)?;
                Ok(())
            }
            KittyImage::TransmitDataAndDisplay {
                transmit,
                placement,
                verbosity,
            } => {
                log::trace!("TransmitDataAndDisplay {:#?} {:#?}", transmit, placement);
                let image_number = transmit.image_number;
                let image_id = self.kitty_img_transmit(transmit, verbosity)?;
                self.kitty_img_place(Some(image_id), image_number, placement, verbosity)
            }
            _ => anyhow::bail!("impossible KittImage variant"),
        }
    }

    pub(crate) fn kitty_img(&mut self, img: KittyImage) -> anyhow::Result<()> {
        log::trace!("{:?}", img);
        if !self.config.enable_kitty_graphics() {
            return Ok(());
        }
        match img {
            KittyImage::Query { transmit } => {
                let image_id = transmit.image_id.unwrap_or(0);
                let response = match transmit.data.load_data() {
                    Ok(_) => {
                        format!("\x1b_Gi={};OK\x1b\\", image_id)
                    }
                    Err(err) => {
                        format!("\x1b_Gi={};ERROR:{:#}\x1b\\", image_id, err)
                    }
                };

                log::trace!("Query Response: {}", response.escape_debug());
                write!(self.writer, "{}", response).ok();
                self.writer.flush().ok();
            }
            KittyImage::TransmitData {
                transmit,
                verbosity,
            } => {
                let more_data_follows = transmit.more_data_follows;
                let img = KittyImage::TransmitData {
                    transmit,
                    verbosity,
                };
                if more_data_follows {
                    self.kitty_img.accumulator.push(img);
                } else {
                    self.kitty_img_inner(img)?;
                }
            }
            KittyImage::TransmitDataAndDisplay {
                transmit,
                placement,
                verbosity,
            } => {
                let more_data_follows = transmit.more_data_follows;
                let img = KittyImage::TransmitDataAndDisplay {
                    transmit,
                    placement,
                    verbosity,
                };
                if more_data_follows {
                    self.kitty_img.accumulator.push(img);
                } else {
                    self.kitty_img_inner(img)?;
                }
            }
            KittyImage::Display {
                image_id,
                image_number,
                placement,
                verbosity,
            } => {
                self.kitty_img_place(image_id, image_number, placement, verbosity)?;
            }
            KittyImage::Delete {
                what:
                    KittyImageDelete::ByImageId {
                        image_id,
                        placement_id,
                        delete,
                    },
                verbosity,
            } => {
                log::trace!(
                    "remove a placement: image_id {} placement_id {:?} delete {} verb {:?}",
                    image_id,
                    placement_id,
                    delete,
                    verbosity
                );

                self.kitty_remove_placement(image_id, placement_id);

                if delete {
                    self.kitty_img.remove_data_for_id(image_id);
                }
            }
            KittyImage::Delete { what, verbosity } => {
                log::warn!("unhandled KittyImage::Delete {:?} {:?}", what, verbosity);
            }
        };

        Ok(())
    }

    fn kitty_remove_placement_from_model(
        &mut self,
        image_id: u32,
        placement_id: Option<u32>,
        info: PlacementInfo,
    ) {
        let screen = self.screen_mut();
        let range =
            screen.stable_range(&(info.first_row..info.first_row + info.rows as StableRowIndex));
        for idx in range {
            let line = screen.line_mut(idx);
            for c in line.cells_mut() {
                c.attrs_mut()
                    .detach_image_with_placement(image_id, placement_id);
            }
            line.set_dirty();
        }
    }

    fn kitty_remove_placement(&mut self, image_id: u32, placement_id: Option<u32>) {
        if placement_id.is_some() {
            if let Some(info) = self.kitty_img.placements.remove(&(image_id, placement_id)) {
                log::trace!("removed placement {} {:?}", image_id, placement_id);
                self.kitty_remove_placement_from_model(image_id, placement_id, info);
            }
        } else {
            let mut to_clear = vec![];
            for (id, p) in self.kitty_img.placements.keys() {
                if *id == image_id {
                    to_clear.push(*p);
                }
            }
            for p in to_clear.into_iter() {
                if let Some(info) = self.kitty_img.placements.remove(&(image_id, p)) {
                    self.kitty_remove_placement_from_model(image_id, p, info);
                }
            }
        }

        log::trace!(
            "after remove: there are {} placements, {} images, {} memory",
            self.kitty_img.placements.len(),
            self.kitty_img.id_to_data.len(),
            self.kitty_img.used_memory,
        );
    }

    fn kitty_img_transmit(
        &mut self,
        transmit: KittyImageTransmit,
        verbosity: KittyImageVerbosity,
    ) -> anyhow::Result<u32> {
        let (image_id, image_number) = match (transmit.image_id, transmit.image_number) {
            (Some(_), Some(_)) => {
                // TODO: send an EINVAL error back here
                anyhow::bail!("cannot use both i= and I= in the same request");
            }
            (None, None) => {
                // Assume image id 0
                (0, None)
            }
            (Some(id), None) => (id, None),
            (None, Some(no)) => {
                let id = self.kitty_img.max_image_id + 1;
                self.kitty_img.number_to_id.insert(no, id);
                (id, Some(no))
            }
        };

        self.kitty_img.max_image_id = self.kitty_img.max_image_id.max(image_id);
        log::trace!("transmit {:?}", transmit);

        let data = transmit
            .data
            .load_data()
            .context("data should have been materialized in coalesce_kitty_accumulation")?;

        let data = match transmit.compression {
            KittyImageCompression::None => data,
            KittyImageCompression::Deflate => deflate::deflate_bytes(&data),
        };

        let img = match transmit.format {
            None | Some(KittyImageFormat::Rgba) | Some(KittyImageFormat::Rgb) => {
                let (width, height) = match (transmit.width, transmit.height) {
                    (Some(w), Some(h)) => (w, h),
                    _ => {
                        anyhow::bail!("missing width/height info for kitty img");
                    }
                };

                let data = match transmit.format {
                    Some(KittyImageFormat::Rgb) => {
                        let img = DynamicImage::ImageRgb8(
                            RgbImage::from_vec(width, height, data)
                                .ok_or_else(|| anyhow::anyhow!("failed to decode image"))?,
                        );
                        let img = img.into_rgba8();
                        img.into_vec().into_boxed_slice()
                    }
                    _ => data.into_boxed_slice(),
                };

                let image_data = ImageDataType::Rgba8 {
                    width,
                    height,
                    data,
                };

                self.raw_image_to_image_data(image_data)
            }
            Some(KittyImageFormat::Png) => {
                let decoded = image::load_from_memory(&data).context("decode png")?;
                let (width, height) = decoded.dimensions();
                let data = decoded.into_rgba8().into_vec().into_boxed_slice();
                let image_data = ImageDataType::Rgba8 {
                    width,
                    height,
                    data,
                };
                self.raw_image_to_image_data(image_data)
            }
        };

        self.kitty_img.record_id_to_data(image_id, img);

        if let Some(no) = image_number {
            match verbosity {
                KittyImageVerbosity::Verbose => {
                    write!(self.writer, "\x1b_Gi={},I={};OK\x1b\\", image_id, no).ok();
                    self.writer.flush().ok();
                }
                _ => {}
            }
        }

        Ok(image_id)
    }

    fn coalesce_kitty_accumulation(&mut self, img: KittyImage) -> anyhow::Result<KittyImage> {
        log::trace!(
            "coalesce: accumulator={:#?} img:{:#?}",
            self.kitty_img.accumulator,
            img
        );
        if self.kitty_img.accumulator.is_empty() {
            Ok(img)
        } else {
            let mut data = vec![];
            let mut trans;
            let place;
            let final_verbosity = img.verbosity();

            self.kitty_img.accumulator.push(img);

            let mut empty_data = KittyImageData::Direct(String::new());
            match self.kitty_img.accumulator.remove(0) {
                KittyImage::TransmitData { transmit, .. } => {
                    trans = transmit;
                    place = None;
                    std::mem::swap(&mut empty_data, &mut trans.data);
                }
                KittyImage::TransmitDataAndDisplay {
                    transmit,
                    placement,
                    ..
                } => {
                    place = Some(placement);
                    trans = transmit;
                    std::mem::swap(&mut empty_data, &mut trans.data);
                }
                _ => unreachable!(),
            }
            data.push(empty_data);

            for item in self.kitty_img.accumulator.drain(..) {
                match item {
                    KittyImage::TransmitData { transmit, .. }
                    | KittyImage::TransmitDataAndDisplay { transmit, .. } => {
                        data.push(transmit.data);
                    }
                    _ => unreachable!(),
                }
            }

            let mut b64_encoded = String::new();
            for data in data.into_iter() {
                match data {
                    KittyImageData::Direct(b) => {
                        b64_encoded.push_str(&b);
                    }
                    data => {
                        anyhow::bail!("expected data chunks to be Direct data, found {:#?}", data)
                    }
                }
            }

            trans.data = KittyImageData::Direct(b64_encoded);

            if let Some(placement) = place {
                Ok(KittyImage::TransmitDataAndDisplay {
                    transmit: trans,
                    placement,
                    verbosity: final_verbosity,
                })
            } else {
                Ok(KittyImage::TransmitData {
                    transmit: trans,
                    verbosity: final_verbosity,
                })
            }
        }
    }
}
