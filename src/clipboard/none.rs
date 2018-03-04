use clipboard::{ClipboardImpl, Paste};
use failure::Error;
use guiloop::{GuiSender, WindowId};
use std::sync::mpsc::{channel, Receiver, Sender};

/// A no-op clipboard implementation
#[allow(dead_code)]
pub struct NoClipboard {
    receiver: Receiver<Paste>,
    sender: Sender<Paste>,
}

impl ClipboardImpl for NoClipboard {
    fn new(_wakeup: GuiSender<WindowId>, _window_id: WindowId) -> Result<Self, Error> {
        let (sender, receiver) = channel();
        Ok(Self { sender, receiver })
    }

    fn set_clipboard(&self, _text: Option<String>) -> Result<(), Error> {
        Ok(())
    }

    fn get_clipboard(&self) -> Result<String, Error> {
        Ok("".into())
    }

    fn try_get_paste(&self) -> Result<Option<Paste>, Error> {
        Ok(None)
    }
}
