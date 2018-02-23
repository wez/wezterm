use clipboard::ClipboardImpl;
use failure::Error;
use wakeup::Wakeup;

/// A no-op clipboard implementation
#[allow(dead_code)]
pub struct NoClipboard {}

impl ClipboardImpl for NoClipboard {
    fn new(_wakeup: Wakeup) -> Result<Self, Error> {
        Ok(Self {})
    }

    fn set_clipboard(&self, _text: Option<String>) -> Result<(), Error> {
        Ok(())
    }

    fn get_clipboard(&self) -> Result<String, Error> {
        Ok("".into())
    }
}
