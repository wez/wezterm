use crate::terminalstate::image::*;
use crate::terminalstate::{ImageAttachParams, PlacementInfo};
use crate::{StableRowIndex, TerminalState};
use ::image::{
    DynamicImage, GenericImage, GenericImageView, ImageBuffer, RgbImage, Rgba, RgbaImage,
};
use anyhow::Context;
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;
use termwiz::escape::apc::{
    KittyFrameCompositionMode, KittyImage, KittyImageCompression, KittyImageData, KittyImageDelete,
    KittyImageFormat, KittyImageFrame, KittyImageFrameCompose, KittyImagePlacement,
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
        if image_id != 0 {
            self.remove_data_for_id(image_id);
        }
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
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "image_number has no matching image id {:?} in number_to_id",
                        image_number
                    )
                })?,
        };

        log::trace!(
            "kitty_img_place image_id {:?} image_no {:?} placement {:?} verb {:?}",
            image_id,
            image_number,
            placement,
            verbosity
        );
        if image_id != 0 {
            self.kitty_remove_placement(image_id, placement.placement_id);
        }
        let img = Arc::clone(self.kitty_img.id_to_data.get(&image_id).ok_or_else(|| {
            anyhow::anyhow!(
                "no matching image id {} in id_to_data for image_number {:?}",
                image_id,
                image_number
            )
        })?);

        let (image_width, image_height) = img.data().dimensions()?;

        let info = self.assign_image_to_cells(ImageAttachParams {
            image_width,
            image_height,
            source_width: placement.w,
            source_height: placement.h,
            source_origin_x: placement.x.unwrap_or(0),
            source_origin_y: placement.y.unwrap_or(0),
            cell_padding_left: placement.x_offset.unwrap_or(0) as u16,
            cell_padding_top: placement.y_offset.unwrap_or(0) as u16,
            data: img,
            style: ImageAttachStyle::Kitty,
            z_index: placement.z_index.unwrap_or(0),
            columns: placement.columns.map(|x| x as usize),
            rows: placement.rows.map(|x| x as usize),
            image_id: Some(image_id),
            placement_id: placement.placement_id,
            do_not_move_cursor: placement.do_not_move_cursor,
        })?;

        self.kitty_img
            .placements
            .insert((image_id, placement.placement_id), info);
        log::trace!(
            "record placement for {} (image_number {:?}) {:?}",
            image_id,
            image_number,
            placement.placement_id
        );

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
        let verbosity = img.verbosity();
        match img {
            KittyImage::Query { transmit } => match transmit.data.load_data() {
                Ok(_) => {
                    self.kitty_send_response(
                        verbosity,
                        true,
                        transmit.image_id,
                        transmit.image_number,
                        "OK".to_string(),
                    );
                }
                Err(err) => {
                    self.kitty_send_response(
                        verbosity,
                        false,
                        transmit.image_id,
                        transmit.image_number,
                        format!("ERROR:{:#}", err),
                    );
                }
            },
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
            KittyImage::Delete {
                what: KittyImageDelete::All { delete },
                verbosity: _,
            } => {
                self.kitty_remove_all_placements(delete);
            }
            KittyImage::Delete { what, verbosity } => {
                log::warn!("unhandled KittyImage::Delete {:?} {:?}", what, verbosity);
            }
            KittyImage::TransmitFrame {
                transmit,
                frame,
                verbosity,
            } => {
                if let Err(err) = self.kitty_frame_transmit(transmit, frame, verbosity) {
                    log::error!("Error {:#} while handling KittyImage::TransmitFrame", err,);
                }
            }
            KittyImage::ComposeFrame { frame, verbosity } => {
                if let Err(err) = self.kitty_frame_compose(frame, verbosity) {
                    log::error!("Error {:#} while handling KittyImage::ComposeFrame", err);
                }
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
        let seqno = self.seqno;
        let screen = self.screen_mut();
        let range =
            screen.stable_range(&(info.first_row..info.first_row + info.rows as StableRowIndex));
        for idx in range {
            let line = screen.line_mut(idx);
            for c in line.cells_mut() {
                c.attrs_mut()
                    .detach_image_with_placement(image_id, placement_id);
            }
            line.update_last_change_seqno(seqno);
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

    pub(crate) fn kitty_remove_all_placements(&mut self, delete: bool) {
        for ((image_id, p), info) in std::mem::take(&mut self.kitty_img.placements).into_iter() {
            self.kitty_remove_placement_from_model(image_id, p, info);
        }
        if delete {
            self.kitty_img.id_to_data.clear();
            self.kitty_img.used_memory = 0;
            self.kitty_img.number_to_id.clear();
        }
    }

    fn kitty_send_response(
        &mut self,
        verbosity: KittyImageVerbosity,
        success: bool,
        image_id: Option<u32>,
        image_no: Option<u32>,
        message: String,
    ) {
        match verbosity {
            KittyImageVerbosity::Verbose => {}
            KittyImageVerbosity::OnlyErrors => {
                if success {
                    return;
                }
            }
            KittyImageVerbosity::Quiet => {
                return;
            }
        }

        log::trace!("Query Response: {}", message);

        match (image_id, image_no) {
            (Some(id), Some(no)) => {
                write!(self.writer, "\x1b_GI={},i={};{}\x1b\\", no, id, message).ok();
            }
            (Some(id), None) => {
                write!(self.writer, "\x1b_Gi={};{}\x1b\\", id, message).ok();
            }
            (None, Some(no)) => {
                write!(self.writer, "\x1b_GI={};{}\x1b\\", no, message).ok();
            }
            (None, None) => {
                write!(self.writer, "\x1b_G{}\x1b\\", message).ok();
            }
        }
        self.writer.flush().ok();
    }

    fn kitty_frame_compose(
        &mut self,
        frame: KittyImageFrameCompose,
        verbosity: KittyImageVerbosity,
    ) -> anyhow::Result<()> {
        let image_id = match frame.image_number {
            Some(no) => match self.kitty_img.number_to_id.get(&no) {
                Some(id) => *id,
                None => {
                    self.kitty_send_response(
                        verbosity,
                        false,
                        frame.image_id,
                        frame.image_number,
                        "ENOENT".to_string(),
                    );
                    anyhow::bail!("no such image_number {}", no);
                }
            },
            None => frame.image_id.ok_or_else(|| {
                self.kitty_send_response(
                    verbosity,
                    false,
                    frame.image_id,
                    frame.image_number,
                    "ENOENT".to_string(),
                );
                anyhow::anyhow!("no image_id")
            })?,
        };

        let src_frame = frame.source_frame.ok_or_else(|| {
            self.kitty_send_response(
                verbosity,
                false,
                frame.image_id,
                frame.image_number,
                "ENOENT".to_string(),
            );
            anyhow::anyhow!("missing source frame")
        })? as usize;
        let target_frame = frame.target_frame.ok_or_else(|| {
            self.kitty_send_response(
                verbosity,
                false,
                frame.image_id,
                frame.image_number,
                "ENOENT".to_string(),
            );
            anyhow::anyhow!("missing target frame")
        })? as usize;

        let img = self
            .kitty_img
            .id_to_data
            .get(&image_id)
            .ok_or_else(|| anyhow::anyhow!("invalid image id {}", image_id))?;

        let mut img = img.data();
        match &mut *img {
            ImageDataType::EncodedLease(_) | ImageDataType::EncodedFile(_) => {
                anyhow::bail!("invalid image type")
            }
            ImageDataType::Rgba8 {
                width,
                height,
                data,
                hash,
            } => {
                anyhow::ensure!(
                    src_frame == target_frame && src_frame == 1,
                    "src_frame={} target_frame={} but there is only a single frame",
                    src_frame,
                    target_frame
                );

                let src = clip_view(
                    *width,
                    *height,
                    data.as_mut_slice(),
                    frame.src_x,
                    frame.src_y,
                    frame.w,
                    frame.h,
                )?;

                let mut dest: ImageBuffer<Rgba<u8>, &mut [u8]> =
                    ImageBuffer::from_raw(*width, *height, data.as_mut_slice())
                        .ok_or_else(|| anyhow::anyhow!("ill formed image"))?;

                blit(
                    &mut dest,
                    &src,
                    frame.x.unwrap_or(0),
                    frame.y.unwrap_or(0),
                    frame.composition_mode,
                )?;

                drop(dest);

                *hash = ImageDataType::hash_bytes(data);
            }
            ImageDataType::AnimRgba8 {
                width,
                height,
                frames,
                hashes,
                ..
            } => {
                anyhow::ensure!(
                    src_frame > 0 && src_frame <= frames.len(),
                    "src_frame {} is out of range",
                    src_frame
                );
                anyhow::ensure!(
                    target_frame > 0 && target_frame <= frames.len(),
                    "target_frame {} is out of range",
                    target_frame
                );

                let src = clip_view(
                    *width,
                    *height,
                    frames[src_frame - 1].as_mut_slice(),
                    frame.src_x,
                    frame.src_y,
                    frame.w,
                    frame.h,
                )?;

                let mut dest: ImageBuffer<Rgba<u8>, &mut [u8]> =
                    ImageBuffer::from_raw(*width, *height, frames[target_frame - 1].as_mut_slice())
                        .ok_or_else(|| anyhow::anyhow!("ill formed image"))?;

                blit(
                    &mut dest,
                    &src,
                    frame.x.unwrap_or(0),
                    frame.y.unwrap_or(0),
                    frame.composition_mode,
                )?;

                drop(dest);
                hashes[target_frame - 1] = ImageDataType::hash_bytes(&frames[target_frame - 1]);
            }
        }

        Ok(())
    }

    fn kitty_frame_transmit(
        &mut self,
        mut transmit: KittyImageTransmit,
        frame: KittyImageFrame,
        verbosity: KittyImageVerbosity,
    ) -> anyhow::Result<()> {
        if let Some(no) = transmit.image_number.take() {
            match self.kitty_img.number_to_id.get(&no) {
                Some(id) => {
                    transmit.image_id.replace(*id);
                }
                None => {
                    transmit.image_number.replace(no);
                }
            }
        }

        let (image_id, image_number, img) = self.kitty_img_transmit_inner(transmit)?;

        let img = match img.decode() {
            ImageDataType::Rgba8 {
                data,
                width,
                height,
                ..
            } => RgbaImage::from_vec(width, height, data)
                .ok_or_else(|| anyhow::anyhow!("data isn't rgba8"))?,
            wat => anyhow::bail!("data isn't rgba8 {:?}", wat),
        };

        let background_pixel = frame.background_pixel.unwrap_or(0);
        let background_pixel = Rgba([
            ((background_pixel >> 24) & 0xff) as u8,
            ((background_pixel >> 16) & 0xff) as u8,
            ((background_pixel >> 8) & 0xff) as u8,
            (background_pixel & 0xff) as u8,
        ]);

        let anim = match self.kitty_img.id_to_data.get(&image_id) {
            Some(anim) => anim,
            None => {
                self.kitty_send_response(
                    verbosity,
                    false,
                    Some(image_id),
                    image_number,
                    "ENOENT".to_string(),
                );
                anyhow::bail!(
                    "no matching image id {} in id_to_data for image_number {:?}",
                    image_id,
                    image_number
                )
            }
        };

        let mut anim = anim.data();
        let x = frame.x.unwrap_or(0);
        let y = frame.y.unwrap_or(0);
        let frame_gap = Duration::from_millis(match frame.duration_ms {
            None | Some(0) => 40,
            Some(n) => n.into(),
        });

        match &mut *anim {
            ImageDataType::EncodedLease(_) | ImageDataType::EncodedFile(_) => {
                anyhow::bail!("Expected decoded image for image id {}", image_id)
            }
            ImageDataType::Rgba8 {
                data,
                width,
                height,
                hash,
            } => {
                let base_frame = match frame.base_frame {
                    Some(1) => Some(1),
                    None => None,
                    Some(n) => anyhow::bail!(
                        "attempted to copy frame {} but there is only a single frame",
                        n
                    ),
                };

                match frame.frame_number {
                    Some(1) => {
                        // Edit in place
                        let len = data.len();
                        let mut anim_img: ImageBuffer<Rgba<u8>, &mut [u8]> =
                            ImageBuffer::from_raw(*width, *height, data.as_mut_slice())
                                .ok_or_else(|| {
                                    anyhow::anyhow!(
                                        "ImageBuffer::from_raw failed for single \
                                         frame of {}x{} ({} bytes)",
                                        width,
                                        height,
                                        len
                                    )
                                })?;

                        blit(&mut anim_img, &img, x, y, frame.composition_mode)?;

                        drop(anim_img);
                        *hash = ImageDataType::hash_bytes(data);
                    }
                    Some(2) | None => {
                        // Create a second frame

                        let mut new_frame = if base_frame.is_some() {
                            RgbaImage::from_vec(*width, *height, data.clone()).unwrap()
                        } else {
                            RgbaImage::from_pixel(*width, *height, background_pixel)
                        };

                        blit(&mut new_frame, &img, x, y, frame.composition_mode)?;

                        let new_frame_data = new_frame.into_vec();
                        let new_frame_hash = ImageDataType::hash_bytes(&new_frame_data);

                        let frames = vec![std::mem::take(data), new_frame_data];
                        let durations = vec![Duration::from_millis(0), frame_gap];
                        let hashes = vec![*hash, new_frame_hash];

                        *anim = ImageDataType::AnimRgba8 {
                            width: *width,
                            height: *height,
                            frames,
                            durations,
                            hashes,
                        };
                    }
                    Some(n) => anyhow::bail!(
                        "attempted to edit frame {} but there is only a single frame",
                        n
                    ),
                }
            }
            ImageDataType::AnimRgba8 {
                width,
                height,
                frames,
                durations,
                hashes,
            } => {
                let frame_no = frame.frame_number.unwrap_or(frames.len() as u32 + 1);
                if frame_no == frames.len() as u32 + 1 {
                    // Append a new frame

                    let mut new_frame = match frame.base_frame {
                        None => RgbaImage::from_pixel(*width, *height, background_pixel),
                        Some(n) => {
                            let n = n as usize;
                            anyhow::ensure!(
                                n > 0 && n <= frames.len(),
                                "attempted to copy frame {} which is outside range 1-{}",
                                n,
                                frames.len()
                            );
                            RgbaImage::from_vec(*width, *height, frames[n - 1].clone()).unwrap()
                        }
                    };

                    blit(&mut new_frame, &img, x, y, frame.composition_mode)?;

                    let new_frame_data = new_frame.into_vec();
                    let new_frame_hash = ImageDataType::hash_bytes(&new_frame_data);

                    frames.push(new_frame_data);
                    hashes.push(new_frame_hash);
                    durations.push(frame_gap);
                } else {
                    anyhow::ensure!(
                        frame_no > 0 && frame_no <= frames.len() as u32,
                        "attempted to edit frame {} which is outside range 1-{}",
                        frame_no,
                        frames.len()
                    );

                    let frame_no = frame_no as usize;

                    let len = frames[frame_no - 1].len();
                    let mut anim_img: ImageBuffer<Rgba<u8>, &mut [u8]> =
                        ImageBuffer::from_raw(*width, *height, frames[frame_no - 1].as_mut_slice())
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "ImageBuffer::from_raw failed for single \
                                         frame of {}x{} ({} bytes)",
                                    width,
                                    height,
                                    len
                                )
                            })?;

                    blit(&mut anim_img, &img, x, y, frame.composition_mode)?;

                    drop(anim_img);
                    hashes[frame_no - 1] = ImageDataType::hash_bytes(&frames[frame_no - 1]);
                }
            }
        }

        Ok(())
    }

    fn kitty_img_transmit_inner(
        &mut self,
        transmit: KittyImageTransmit,
    ) -> anyhow::Result<(u32, Option<u32>, ImageDataType)> {
        log::trace!("transmit {:?}", transmit);
        let (id, no) = match (transmit.image_id, transmit.image_number) {
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

        let data = transmit
            .data
            .load_data()
            .context("data should have been materialized in coalesce_kitty_accumulation")?;

        let data = match transmit.compression {
            KittyImageCompression::None => data,
            KittyImageCompression::Deflate => {
                miniz_oxide::inflate::decompress_to_vec_zlib(&data)
                    .map_err(|e| anyhow::anyhow!("decompressing data: {:?}", e))?
            }
        };

        let img = match transmit.format {
            None | Some(KittyImageFormat::Rgba) | Some(KittyImageFormat::Rgb) => {
                let (width, height) = match (transmit.width, transmit.height) {
                    (Some(w), Some(h)) => (w, h),
                    _ => {
                        anyhow::bail!("missing width/height info for kitty img");
                    }
                };

                check_image_dimensions(width, height)?;

                let data = match transmit.format {
                    Some(KittyImageFormat::Rgb) => {
                        let img = DynamicImage::ImageRgb8(
                            RgbImage::from_vec(width, height, data)
                                .ok_or_else(|| anyhow::anyhow!("failed to decode image"))?,
                        );
                        let img = img.into_rgba8();
                        img.into_vec()
                    }
                    _ => data,
                };

                anyhow::ensure!(
                    width * height * 4 == data.len() as u32,
                    "transmit data len is {} but it doesn't match width*height*4 {}x{}x4 = {}",
                    data.len(),
                    width,
                    height,
                    width * height * 4
                );

                ImageDataType::new_single_frame(width, height, data)
            }
            Some(KittyImageFormat::Png) => {
                let info = dimensions(&data)?;
                check_image_dimensions(info.width, info.height)?;
                let decoded = image::load_from_memory(&data).context("decode png")?;
                let (width, height) = decoded.dimensions();
                let data = decoded.into_rgba8().into_vec();
                ImageDataType::new_single_frame(width, height, data)
            }
        };

        Ok((id, no, img))
    }

    fn kitty_img_transmit(
        &mut self,
        transmit: KittyImageTransmit,
        verbosity: KittyImageVerbosity,
    ) -> anyhow::Result<u32> {
        let (image_id, image_number, img) = self.kitty_img_transmit_inner(transmit)?;
        self.kitty_img.max_image_id = self.kitty_img.max_image_id.max(image_id);

        let img = self
            .raw_image_to_image_data(img)
            .context("storing image data")?;
        self.kitty_img.record_id_to_data(image_id, img);

        if image_number.is_some() {
            self.kitty_send_response(
                verbosity,
                true,
                Some(image_id),
                image_number,
                "OK".to_string(),
            );
        }

        Ok(image_id)
    }

    fn coalesce_kitty_accumulation(&mut self, img: KittyImage) -> anyhow::Result<KittyImage> {
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

            let mut b64_decoded = vec![];
            for mut data in data.into_iter() {
                match &mut data {
                    KittyImageData::DirectBin(b) => {
                        b64_decoded.append(b);
                    }
                    KittyImageData::Direct(b) => {
                        if !b.is_empty() {
                            b64_decoded.append(&mut data.load_data()?);
                        }
                    }
                    data => {
                        anyhow::bail!("expected data chunks to be Direct data, found {:#?}", data)
                    }
                }
            }

            trans.data = KittyImageData::DirectBin(b64_decoded);

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

/// Make a copy of the source region.
/// Ideally we wouldn't need this, but Rust's mutability rules
/// make it very awkward to mutably reference a frame while
/// an immutable reference exists to a separate frame.
fn clip_view(
    width: u32,
    height: u32,
    data: &mut [u8],
    src_x: Option<u32>,
    src_y: Option<u32>,
    view_width: Option<u32>,
    view_height: Option<u32>,
) -> anyhow::Result<RgbaImage> {
    let src = ImageBuffer::from_raw(width, height, data)
        .ok_or_else(|| anyhow::anyhow!("ill formed image"))?;

    let src_x = src_x.unwrap_or(0);
    let src_y = src_y.unwrap_or(0);

    let view_width = view_width.unwrap_or(width);
    let view_height = view_height.unwrap_or(height);

    let (view_width, view_height) =
        image::imageops::overlay_bounds((width, height), (view_width, view_height), src_x, src_y);

    let view = src.view(src_x, src_y, view_width, view_height);

    let mut tmp = RgbaImage::new(view_width, view_height);
    tmp.copy_from(&*view, 0, 0).context("copy source image")?;
    Ok(tmp)
}

fn blit<D, S, P>(
    dest: &mut D,
    src: &S,
    x: u32,
    y: u32,
    mode: KittyFrameCompositionMode,
) -> anyhow::Result<()>
where
    D: GenericImage<Pixel = P>,
    S: GenericImageView<Pixel = P>,
{
    match mode {
        KittyFrameCompositionMode::Overwrite => {
            ::image::imageops::replace(dest, src, x.into(), y.into());
        }
        KittyFrameCompositionMode::AlphaBlending => {
            ::image::imageops::overlay(dest, src, x.into(), y.into());
        }
    }
    Ok(())
}
