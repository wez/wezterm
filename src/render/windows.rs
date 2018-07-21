//! A Renderer for windows consoles

use caps::Capabilities;
use cell::CellAttributes;
use failure;
use render::Renderer;
use screen::Change;
use terminal::Terminal;

pub struct WindowsConsoleRenderer {
    caps: Capabilities,
}

impl WindowsConsoleRenderer {
    pub fn new(caps: Capabilities) -> Self {
        Self { caps }
    }
}

impl Renderer for WindowsConsoleRenderer {
    fn render_to(
        &mut self,
        starting_attr: &CellAttributes,
        changes: &[Change],
        out: &mut Terminal,
    ) -> Result<CellAttributes, failure::Error> {
        // ze goggles!
        Ok(starting_attr.clone())
    }
}
