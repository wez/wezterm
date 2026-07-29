use crate::wayland::read_pipe_with_timeout;
use smithay_client_toolkit as toolkit;
use std::path::PathBuf;
use toolkit::data_device_manager::data_offer::DragOffer;
use toolkit::data_device_manager::ReadPipe;
use url::Url;

use super::data_device::URI_MIME_TYPE;
use super::WaylandConnection;

/// Represents an active drag-and-drop session for a window.
///
/// Created when a data device enter event occurs, and holds the drag offer
/// until the drop is performed or the session is cancelled.
pub struct DragAndDropSession {
    pub window_id: usize,
    pub drag_offer: DragOffer,
}

/// A window ID paired with a read pipe for receiving drag-and-drop'ed content data.
pub struct WindowAndPipe {
    pub window_id: usize,
    pub read: ReadPipe,
}

impl DragAndDropSession {
    /// Takes the current drag offer and initiates a receive into a pipe,
    /// returning that window and pipe descriptor.
    pub fn create_pipe_for_drop(&mut self) -> Option<WindowAndPipe> {
        let read = self
            .drag_offer
            .receive(URI_MIME_TYPE.to_string())
            .map_err(|err| log::error!("Unable to receive data: {:#}", err))
            .ok()?;
        self.drag_offer.finish();
        Some(WindowAndPipe {
            window_id: self.window_id,
            read,
        })
    }

    pub fn read_paths_from_pipe(read: ReadPipe) -> Option<Vec<PathBuf>> {
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

    pub fn dispatch_dropped_files(window_id: usize, paths: Vec<PathBuf>) {
        WaylandConnection::with_window_inner(window_id, move |inner| {
            inner.dispatch_dropped_files(paths);
            Ok(())
        });
    }
}
