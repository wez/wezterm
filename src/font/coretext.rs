//! We'll put macOS Core Text stuff in here
use config::{Config, TextStyle};
use failure::Error;
use font::{FontSystem, NamedFont};

pub type FontSystemImpl = CoreTextSystem;

pub struct CoreTextSystem {}

impl CoreTextSystem {
    pub fn new() -> Self {
        Self {}
    }
}

impl FontSystem for CoreTextSystem {
    fn load_font(&self, _config: &Config, _style: &TextStyle) -> Result<Box<NamedFont>, Error> {
        bail!("load_font");
    }
}
