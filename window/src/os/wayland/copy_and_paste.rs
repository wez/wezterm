use anyhow::{anyhow, bail, Context, Error};
use filedescriptor::{FileDescriptor, Pipe};
use smithay_client_toolkit as toolkit;
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::os::unix::prelude::{FromRawFd, IntoRawFd};
use std::sync::{Arc, Mutex};
use toolkit::primary_selection::*;
use toolkit::reexports::client::protocol::wl_data_offer::{Event as DataOfferEvent, WlDataOffer};
use wayland_client::protocol::wl_data_device_manager::WlDataDeviceManager;
use wayland_client::protocol::wl_data_source::Event as DataSourceEvent;

use crate::connection::ConnectionOps;
use crate::Clipboard;

#[derive(Default)]
pub struct CopyAndPaste {
    data_offer: Option<WlDataOffer>,
}

impl std::fmt::Debug for CopyAndPaste {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        fmt.debug_struct("CopyAndPaste")
            .field("data_offer", &self.data_offer.is_some())
            .finish()
    }
}

pub const TEXT_MIME_TYPE: &str = "text/plain;charset=utf-8";

impl CopyAndPaste {
    pub fn create() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Default::default()))
    }

    pub fn get_clipboard_data(&mut self, clipboard: Clipboard) -> anyhow::Result<FileDescriptor> {
        let conn = crate::Connection::get().unwrap().wayland();
        let pointer = conn.pointer.borrow();
        let primary_selection = if let Clipboard::PrimarySelection = clipboard {
            pointer.primary_selection_device.as_ref()
        } else {
            None
        };
        match primary_selection {
            Some(device) => {
                let pipe = device.with_selection(|offer| {
                    offer
                        .ok_or_else(|| anyhow!("no primary selection offer"))
                        .and_then(|o| {
                            o.receive(TEXT_MIME_TYPE.to_string())
                                .with_context(|| "failed to open read pipe".to_string())
                        })
                })?;
                Ok(unsafe { FileDescriptor::from_raw_fd(pipe.into_raw_fd()) })
            }
            None => {
                let offer = self
                    .data_offer
                    .as_ref()
                    .ok_or_else(|| anyhow!("no data offer"))?;
                let pipe = Pipe::new().map_err(Error::msg)?;
                offer.receive(TEXT_MIME_TYPE.to_string(), pipe.write.as_raw_fd());
                Ok(pipe.read)
            }
        }
    }

    pub fn set_clipboard_data(&mut self, clipboard: Clipboard, data: String) {
        let conn = crate::Connection::get().unwrap().wayland();
        let last_serial = *conn.last_serial.borrow();
        let pointer = conn.pointer.borrow();
        let primary_selection = if let Clipboard::PrimarySelection = clipboard {
            conn.environment
                .get_primary_selection_manager()
                .zip(pointer.primary_selection_device.as_ref())
        } else {
            None
        };

        match primary_selection {
            Some((manager, device)) => {
                let source = PrimarySelectionSource::new(
                    &manager,
                    &[TEXT_MIME_TYPE.to_string()],
                    move |event, _dispatch_data| match event {
                        PrimarySelectionSourceEvent::Cancelled => {
                            crate::Connection::get()
                                .unwrap()
                                .wayland()
                                .pointer
                                .borrow()
                                .data_device
                                .set_selection(None, 0);
                        }
                        PrimarySelectionSourceEvent::Send { pipe, .. } => {
                            let fd = unsafe { FileDescriptor::from_raw_fd(pipe.into_raw_fd()) };
                            write_selection_to_pipe(fd, &data);
                        }
                    },
                );
                device.set_selection(&Some(source), last_serial)
            }
            None => {
                let source = conn
                    .environment
                    .require_global::<WlDataDeviceManager>()
                    .create_data_source();
                source.quick_assign(move |_source, event, _dispatch_data| {
                    if let DataSourceEvent::Send { fd, .. } = event {
                        let fd = unsafe { FileDescriptor::from_raw_fd(fd) };
                        write_selection_to_pipe(fd, &data);
                    }
                });
                source.offer(TEXT_MIME_TYPE.to_string());
                conn.pointer
                    .borrow()
                    .data_device
                    .set_selection(Some(&source), last_serial);
            }
        }
    }

    pub fn handle_data_offer(&mut self, event: DataOfferEvent, offer: WlDataOffer) {
        match event {
            DataOfferEvent::Offer { mime_type } => {
                let conn = crate::Connection::get().unwrap().wayland();
                let last_serial = *conn.last_serial.borrow();
                if mime_type == TEXT_MIME_TYPE {
                    offer.accept(last_serial, Some(mime_type));
                    self.data_offer.replace(offer);
                } else {
                    // Refuse other mime types
                    offer.accept(last_serial, None);
                }
            }
            DataOfferEvent::SourceActions { .. } | DataOfferEvent::Action { .. } => {
                // ignore drag and drop events
            }
            _ => {}
        }
    }

    pub fn confirm_selection(&mut self, offer: WlDataOffer) {
        self.data_offer.replace(offer);
    }
}

fn write_selection_to_pipe(fd: FileDescriptor, text: &str) {
    if let Err(e) = write_pipe_with_timeout(fd, text.as_bytes()) {
        log::error!("while sending primary selection to pipe: {}", e);
    }
}

fn write_pipe_with_timeout(mut file: FileDescriptor, data: &[u8]) -> anyhow::Result<()> {
    file.set_non_blocking(true)?;
    let mut pfd = libc::pollfd {
        fd: file.as_raw_fd(),
        events: libc::POLLOUT,
        revents: 0,
    };

    let mut buf = data;

    while !buf.is_empty() {
        if unsafe { libc::poll(&mut pfd, 1, 3000) == 1 } {
            match file.write(buf) {
                Ok(size) if size == 0 => {
                    bail!("zero byte write");
                }
                Ok(size) => {
                    buf = &buf[size..];
                }
                Err(e) => bail!("error writing to pipe: {}", e),
            }
        } else {
            bail!("timed out writing to pipe");
        }
    }

    Ok(())
}
