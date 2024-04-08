use crate::wayland::read_pipe_with_timeout;
use crate::ConnectionOps;
use smithay_client_toolkit as toolkit;
use std::path::PathBuf;
use toolkit::data_device_manager::data_offer::DragOffer;
use toolkit::data_device_manager::ReadPipe;
use url::Url;

use super::data_device::URI_MIME_TYPE;
use super::WaylandConnection;

#[derive(Default)]
pub struct DragAndDrop {
    pub(super) offer: Option<SurfaceAndOffer>,
}

pub(super) struct SurfaceAndOffer {
    pub(super) window_id: usize,
    pub(super) offer: DragOffer,
}

pub(super) struct SurfaceAndPipe {
    pub(super) window_id: usize,
    pub(super) read: ReadPipe,
}

impl DragAndDrop {
    /// Takes the current offer, if any, and initiates a receive into a pipe,
    /// returning that surface and pipe descriptor.
    pub(super) fn create_pipe_for_drop(&mut self) -> Option<SurfaceAndPipe> {
        let SurfaceAndOffer { window_id, offer } = self.offer.take()?;
        let read = offer
            .receive(URI_MIME_TYPE.to_string())
            .map_err(|err| log::error!("Unable to receive data: {:#}", err))
            .ok()?;
        offer.finish();
        Some(SurfaceAndPipe { window_id, read })
    }

    pub(super) fn read_paths_from_pipe(read: ReadPipe) -> Option<Vec<PathBuf>> {
        read_pipe_with_timeout(read)
            .map_err(|err| {
                log::error!("Error while reading pipe from drop result: {:#}", err);
            })
            .ok()?
            .lines()
            .filter_map(|line| {
                if line.starts_with('#') || line.trim().is_empty() {
                    // text/uri-list: Any lines beginning with the '#' character
                    // are comment lines and are ignored during processing
                    return None;
                }
                let url = Url::parse(line)
                    .map_err(|err| {
                        log::error!("Error parsing dropped file line {} as url: {:#}", line, err);
                    })
                    .ok()?;
                url.to_file_path()
                    .map_err(|_| {
                        log::error!("Error converting url {} from line {} to pathbuf", url, line);
                    })
                    .ok()
            })
            .collect::<Vec<_>>()
            .into()
    }

    pub(super) fn dispatch_dropped_files(window_id: usize, paths: Vec<PathBuf>) {
        promise::spawn::spawn_into_main_thread(async move {
            let conn = WaylandConnection::get().unwrap().wayland();
            if let Some(handle) = conn.window_by_id(window_id) {
                let mut inner = handle.borrow_mut();
                inner.dispatch_dropped_files(paths);
            }
        })
        .detach();
    }
}
