use crate::connection::ConnectionOps;
use crate::wayland::{read_pipe_with_timeout, WaylandConnection};
use filedescriptor::{FileDescriptor, Pipe};
use smithay_client_toolkit as toolkit;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use toolkit::reexports::client::protocol::wl_data_device::Event as DataDeviceEvent;
use toolkit::reexports::client::protocol::wl_data_offer::WlDataOffer;
use url::Url;
use wayland_client::protocol::wl_data_device_manager::DndAction;

#[derive(Default)]
pub struct DragAndDrop {
    offer: Option<SurfaceAndOffer>,
}

struct SurfaceAndOffer {
    surface_id: u32,
    offer: WlDataOffer,
}

struct SurfaceAndPipe {
    surface_id: u32,
    read: FileDescriptor,
}

pub const URI_MIME_TYPE: &str = "text/uri-list";

impl DragAndDrop {
    /// Takes the current offer, if any, and initiates a receive into a pipe,
    /// returning that surface and pipe descriptor.
    fn create_pipe_for_drop(&mut self) -> Option<SurfaceAndPipe> {
        let SurfaceAndOffer { surface_id, offer } = self.offer.take()?;
        let pipe = Pipe::new()
            .map_err(|err| log::error!("Unable to create pipe: {:#}", err))
            .ok()?;
        offer.receive(URI_MIME_TYPE.to_string(), pipe.write.as_raw_fd());
        let read = pipe.read;
        offer.finish();
        Some(SurfaceAndPipe { surface_id, read })
    }

    fn read_paths_from_pipe(read: FileDescriptor) -> Option<Vec<PathBuf>> {
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

    fn dispatch_dropped_files(surface_id: u32, paths: Vec<PathBuf>) {
        promise::spawn::spawn_into_main_thread(async move {
            let conn = WaylandConnection::get().unwrap().wayland();
            if let Some(&window_id) = conn.surface_to_window_id.borrow().get(&surface_id) {
                if let Some(handle) = conn.window_by_id(window_id) {
                    let mut inner = handle.borrow_mut();
                    inner.dispatch_dropped_files(paths);
                }
            };
        })
        .detach();
    }

    pub fn handle_data_event(&mut self, event: DataDeviceEvent) {
        match event {
            DataDeviceEvent::Enter {
                serial,
                surface,
                id,
                ..
            } => {
                if let Some(offer) = id {
                    offer.accept(serial, Some(URI_MIME_TYPE.to_string()));
                    offer.set_actions(DndAction::None | DndAction::Copy, DndAction::None);
                    self.offer = Some(SurfaceAndOffer {
                        surface_id: surface.as_ref().id(),
                        offer,
                    });
                }
            }
            DataDeviceEvent::Leave => {
                if let Some(SurfaceAndOffer { offer, .. }) = self.offer.take() {
                    offer.destroy();
                }
            }
            DataDeviceEvent::Motion { .. } => {}
            DataDeviceEvent::Drop => {
                if let Some(SurfaceAndPipe { surface_id, read }) = self.create_pipe_for_drop() {
                    std::thread::spawn(move || {
                        if let Some(paths) = Self::read_paths_from_pipe(read) {
                            Self::dispatch_dropped_files(surface_id, paths);
                        }
                    });
                }
            }
            _ => {}
        }
    }
}
