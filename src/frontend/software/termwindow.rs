use crate::config::Config;
use crate::font::FontConfiguration;
use crate::frontend::guicommon::host::{HostHelper, HostImpl, TabHost};
use crate::mux::tab::{Tab, TabId};
use crate::mux::window::WindowId as MuxWindowId;
use crate::mux::Mux;
use ::window::*;
use failure::Fallible;
use promise::Future;
use std::any::Any;
use std::rc::Rc;
use std::sync::Arc;
use termwiz::color::RgbColor;

pub struct TermWindow {
    window: Option<Window>,
    fonts: Rc<FontConfiguration>,
    config: Arc<Config>,
    width: usize,
    height: usize,
    cell_size: Size,
    mux_window_id: MuxWindowId,

    last_painted_tab_id: Option<TabId>,
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
        Connection::get().unwrap().terminate_message_loop();
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn paint(&mut self, ctx: &mut dyn PaintContext) {
        let mux = Mux::get().unwrap();
        let tab = match mux.get_active_tab_for_window(self.mux_window_id) {
            Some(tab) => tab,
            None => {
                ctx.clear(Color::rgb(0, 0, 0));
                return;
            }
        };
        self.paint_tab(&tab, ctx);
    }
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
                cell_size: Size::new(cell_width as isize, cell_height as isize),
                mux_window_id,
                config: Arc::clone(config),
                fonts: Rc::clone(fontconfig),
                last_painted_tab_id: None,
            }),
        )?;

        window.show();
        Ok(())
    }

    fn paint_tab(&mut self, tab: &Rc<dyn Tab>, ctx: &mut dyn PaintContext) {
        let palette = tab.palette();
        let background_color =
            rgbcolor_to_window_color(palette.resolve_bg(term::color::ColorAttribute::Default));
        ctx.clear(background_color);

        let term = tab.renderer();
        let cursor = term.get_cursor_position();

        let cursor_rect = Rect::new(
            Point::new(
                cursor.x as isize * self.cell_size.width,
                cursor.y as isize * self.cell_size.height,
            ),
            self.cell_size,
        );
        ctx.clear_rect(cursor_rect, rgbcolor_to_window_color(palette.cursor_bg));
    }
}

fn rgbcolor_to_window_color(color: RgbColor) -> Color {
    Color::rgba(color.red, color.green, color.blue, 0xff)
}
