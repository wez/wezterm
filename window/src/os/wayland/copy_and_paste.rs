use failure::Fallible;
use filedescriptor::{FileDescriptor, Pipe};
use smithay_client_toolkit as toolkit;
use std::os::unix::io::AsRawFd;
use std::sync::{Arc, Mutex};
use toolkit::reexports::client::protocol::wl_data_offer::{Event as DataOfferEvent, WlDataOffer};
use toolkit::reexports::client::protocol::wl_data_source::WlDataSource;

#[derive(Default)]
pub struct CopyAndPaste {
    data_offer: Option<WlDataOffer>,
    last_serial: u32,
}

pub const TEXT_MIME_TYPE: &str = "text/plain;charset=utf-8";

impl CopyAndPaste {
    pub fn create() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Default::default()))
    }

    pub fn update_last_serial(&mut self, serial: u32) {
        if serial != 0 {
            self.last_serial = serial;
        }
    }

    pub fn get_clipboard_data(&mut self) -> Fallible<FileDescriptor> {
        let offer = self
            .data_offer
            .as_ref()
            .ok_or_else(|| failure::err_msg("no data offer"))?;
        let pipe = Pipe::new()?;
        offer.receive(TEXT_MIME_TYPE.to_string(), pipe.write.as_raw_fd());
        Ok(pipe.read)
    }

    pub fn handle_data_offer(&mut self, event: DataOfferEvent, offer: WlDataOffer) {
        match event {
            DataOfferEvent::Offer { mime_type } => {
                if mime_type == TEXT_MIME_TYPE {
                    offer.accept(self.last_serial, Some(mime_type));
                    self.data_offer.replace(offer);
                } else {
                    // Refuse other mime types
                    offer.accept(self.last_serial, None);
                }
            }
            DataOfferEvent::SourceActions { source_actions } => {
                log::error!("Offer source_actions {}", source_actions);
            }
            DataOfferEvent::Action { dnd_action } => {
                log::error!("Offer dnd_action {}", dnd_action);
            }
            _ => {}
        }
    }

    pub fn confirm_selection(&mut self, offer: WlDataOffer) {
        self.data_offer.replace(offer);
    }

    pub fn set_selection(&mut self, source: WlDataSource) {
        use crate::connection::ConnectionOps;
        crate::Connection::get()
            .unwrap()
            .wayland()
            .pointer
            .data_device
            .set_selection(Some(&source), self.last_serial);
    }
}
