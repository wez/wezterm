use crate::config::Config;
use crate::font::FontConfiguration;
use crate::frontend::guicommon::host::{HostHelper, HostImpl, TabHost};
use crate::mux::tab::Tab;
use crate::mux::window::WindowId as MuxWindowId;
use crate::mux::Mux;
use ::window::*;
use failure::Fallible;
use promise::Future;
use std::any::Any;
use std::rc::Rc;
use std::sync::Arc;

pub struct TermWindow {
    window: Option<Window>,
    fonts: Rc<FontConfiguration>,
    config: Arc<Config>,
    width: usize,
    height: usize,
    cell_height: usize,
    cell_width: usize,
    mux_window_id: MuxWindowId,
}

impl WindowCallbacks for TermWindow {
    fn created(&mut self, window: &Window) {
        self.window.replace(window.clone());
    }

    fn can_close(&mut self) -> bool {
        // self.host.close_current_tab();
        true
    }

    fn destroy(&mut self) {
        /*
        Future::with_executor(Connection::executor(), move || {
        if Mux::get().unwrap().is_empty() {
            Connection::get().unwrap().terminate_message_loop();
        }
        Ok(())
        });
        */
        Connection::get().unwrap().terminate_message_loop();
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn paint(&mut self, ctx: &mut dyn PaintContext) {}
}

impl Drop for TermWindow {
    fn drop(&mut self) {
        if Mux::get().unwrap().is_empty() {
            Connection::get().unwrap().terminate_message_loop();
        }
    }
}

impl TermWindow {
    pub fn new_window(
        config: &Arc<Config>,
        fontconfig: &Rc<FontConfiguration>,
        tab: &Rc<dyn Tab>,
        mux_window_id: MuxWindowId,
    ) -> Fallible<()> {
        let (physical_rows, physical_cols) = tab.renderer().physical_dimensions();

        let metrics = fontconfig.default_font_metrics()?;
        let (cell_height, cell_width) = (
            metrics.cell_height.ceil() as usize,
            metrics.cell_width.ceil() as usize,
        );

        let width = cell_width * physical_cols;
        let height = cell_height * physical_rows;

        let window = Window::new_window(
            "wezterm",
            "wezterm",
            width,
            height,
            Box::new(Self {
                window: None,
                width,
                height,
                cell_height,
                cell_width,
                mux_window_id,
                config: Arc::clone(config),
                fonts: Rc::clone(fontconfig),
            }),
        )?;

        window.show();
        Ok(())
    }
}
