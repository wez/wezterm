use crate::config::Config;
use crate::font::FontConfiguration;
use crate::guicommon::tabs::LocalTab;
use crate::mux::tab::{Tab, TabId};
use crate::mux::window::WindowId;
use crate::mux::Mux;
use crate::opengl::render::Renderer;
use crate::opengl::textureatlas::OutOfTextureSpace;
use crate::openpty;
use failure::Error;
use glium;
use std::rc::Rc;
use std::sync::Arc;

/// Reports the currently configured physical size of the display
/// surface (physical pixels, not adjusted for dpi) and the current
/// cell dimensions, also in physical pixels
pub struct Dimensions {
    pub width: u16,
    pub height: u16,
    pub cell_height: usize,
    pub cell_width: usize,
}

/// This trait is used to share implementations of common code between
/// the different GUI systems.
/// A number of methods need to be provided by the window in order to
/// unlock the use of the provided methods towards the bottom of the trait.
pub trait TerminalWindow {
    fn set_window_title(&mut self, title: &str) -> Result<(), Error>;
    fn get_mux_window_id(&self) -> WindowId;
    fn frame(&self) -> glium::Frame;
    fn renderer(&mut self) -> &mut Renderer;
    fn recreate_texture_atlas(&mut self, size: u32) -> Result<(), Error>;
    fn advise_renderer_that_scaling_has_changed(
        &mut self,
        cell_width: usize,
        cell_height: usize,
    ) -> Result<(), Error>;
    fn advise_renderer_of_resize(&mut self, width: u16, height: u16) -> Result<(), Error>;
    fn tab_was_created(&mut self, tab: &Rc<Tab>) -> Result<(), Error>;
    fn deregister_tab(&mut self, tab_id: TabId) -> Result<(), Error>;
    fn config(&self) -> &Arc<Config>;
    fn fonts(&self) -> &Rc<FontConfiguration>;
    fn get_dimensions(&self) -> Dimensions;
    fn resize_if_not_full_screen(&mut self, width: u16, height: u16) -> Result<bool, Error>;
    fn check_for_resize(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn activate_tab(&mut self, tab_idx: usize) -> Result<(), Error> {
        let mux = Mux::get().unwrap();
        let mut window = mux
            .get_window_mut(self.get_mux_window_id())
            .ok_or_else(|| format_err!("no such window"))?;

        let max = window.len();
        if tab_idx < max {
            window.set_active(tab_idx);

            drop(window);
            self.update_title();
        }
        Ok(())
    }

    fn activate_tab_relative(&mut self, delta: isize) -> Result<(), Error> {
        let mux = Mux::get().unwrap();
        let window = mux
            .get_window(self.get_mux_window_id())
            .ok_or_else(|| format_err!("no such window"))?;

        let max = window.len();
        let active = window.get_active_idx() as isize;
        let tab = active + delta;
        let tab = if tab < 0 { max as isize + tab } else { tab };
        drop(window);
        self.activate_tab(tab as usize % max)
    }

    fn update_title(&mut self) {
        let mux = Mux::get().unwrap();
        let window = match mux.get_window(self.get_mux_window_id()) {
            Some(window) => window,
            _ => return,
        };
        let num_tabs = window.len();

        if num_tabs == 0 {
            return;
        }
        let tab_no = window.get_active_idx();

        let title = window.get_active().unwrap().get_title();

        drop(window);

        if num_tabs == 1 {
            self.set_window_title(&title).ok();
        } else {
            self.set_window_title(&format!("[{}/{}] {}", tab_no + 1, num_tabs, title))
                .ok();
        }
    }

    fn paint_if_needed(&mut self) -> Result<(), Error> {
        let mux = Mux::get().unwrap();
        let tab = match mux.get_active_tab_for_window(self.get_mux_window_id()) {
            Some(tab) => tab,
            None => return Ok(()),
        };
        if tab.renderer().has_dirty_lines() {
            self.paint()?;
        }
        self.update_title();
        Ok(())
    }

    fn paint(&mut self) -> Result<(), Error> {
        let mut target = self.frame();

        let mux = Mux::get().unwrap();
        let tab = match mux.get_active_tab_for_window(self.get_mux_window_id()) {
            Some(tab) => tab,
            None => return Ok(()),
        };

        let res = {
            let renderer = self.renderer();
            renderer.paint(&mut target, &mut *tab.renderer())
        };

        // Ensure that we finish() the target before we let the
        // error bubble up, otherwise we lose the context.
        target
            .finish()
            .expect("target.finish failed and we don't know how to recover");

        // The only error we want to catch is texture space related;
        // when that happens we need to blow our glyph cache and
        // allocate a newer bigger texture.
        match res {
            Err(err) => {
                if let Some(&OutOfTextureSpace { size }) = err.downcast_ref::<OutOfTextureSpace>() {
                    eprintln!("out of texture space, allocating {}", size);
                    self.recreate_texture_atlas(size)?;
                    tab.renderer().make_all_lines_dirty();
                    // Recursively initiate a new paint
                    return self.paint();
                }
                Err(err)
            }
            Ok(_) => Ok(()),
        }
    }

    fn spawn_tab(&mut self) -> Result<TabId, Error> {
        let config = self.config();

        let dims = self.get_dimensions();

        let rows = (dims.height as usize + 1) / dims.cell_height;
        let cols = (dims.width as usize + 1) / dims.cell_width;

        let (pty, slave) = openpty(rows as u16, cols as u16, dims.width, dims.height)?;
        let cmd = config.build_prog(None)?;

        let process = slave.spawn_command(cmd)?;
        eprintln!("spawned: {:?}", process);

        let mux = Mux::get().unwrap();

        let terminal = term::Terminal::new(
            rows,
            cols,
            config.scrollback_lines.unwrap_or(3500),
            config.hyperlink_rules.clone(),
        );

        let tab: Rc<Tab> = Rc::new(LocalTab::new(terminal, process, pty));
        let tab_id = tab.tab_id();

        let len = {
            let mut window = mux
                .get_window_mut(self.get_mux_window_id())
                .ok_or_else(|| format_err!("no such window!?"))?;
            window.push(&tab);
            window.len()
        };
        self.activate_tab(len - 1)?;

        self.tab_was_created(&tab)?;

        Ok(tab_id)
    }

    fn resize_surfaces(&mut self, width: u16, height: u16, force: bool) -> Result<bool, Error> {
        let dims = self.get_dimensions();

        if force || width != dims.width || height != dims.height {
            debug!("resize {},{}", width, height);

            self.advise_renderer_of_resize(width, height)?;

            // The +1 in here is to handle an irritating case.
            // When we get N rows with a gap of cell_height - 1 left at
            // the bottom, we can usually squeeze that extra row in there,
            // so optimistically pretend that we have that extra pixel!
            let rows = ((height as usize + 1) / dims.cell_height) as u16;
            let cols = ((width as usize + 1) / dims.cell_width) as u16;

            let mux = Mux::get().unwrap();
            let window = mux
                .get_window(self.get_mux_window_id())
                .ok_or_else(|| format_err!("no such window!?"))?;
            for tab in window.iter() {
                tab.resize(rows, cols, width as u16, height as u16)?;
            }

            Ok(true)
        } else {
            debug!("ignoring extra resize");
            Ok(false)
        }
    }

    fn scaling_changed(
        &mut self,
        font_scale: Option<f64>,
        dpi_scale: Option<f64>,
        width: u16,
        height: u16,
    ) -> Result<(), Error> {
        let fonts = self.fonts();
        let dpi_scale = dpi_scale.unwrap_or_else(|| fonts.get_dpi_scale());
        let font_scale = font_scale.unwrap_or_else(|| fonts.get_font_scale());
        eprintln!(
            "TerminalWindow::scaling_changed dpi_scale={} font_scale={}",
            dpi_scale, font_scale
        );
        let mux = Mux::get().unwrap();
        let tab = match mux.get_active_tab_for_window(self.get_mux_window_id()) {
            Some(tab) => tab,
            None => return Ok(()),
        };
        tab.renderer().make_all_lines_dirty();
        fonts.change_scaling(font_scale, dpi_scale);

        let metrics = fonts.default_font_metrics()?;
        let (cell_height, cell_width) = (metrics.cell_height, metrics.cell_width);

        // It is desirable to preserve the terminal rows/cols when scaling,
        // so we query for that information here.
        // If the backend supports `resize_if_not_full_screen` then we'll try
        // to resize the window to match the new cell metrics.
        let (rows, cols) = { tab.renderer().physical_dimensions() };

        self.advise_renderer_that_scaling_has_changed(
            cell_width.ceil() as usize,
            cell_height.ceil() as usize,
        )?;
        if !self.resize_if_not_full_screen(
            cell_width.ceil() as u16 * cols as u16,
            cell_height.ceil() as u16 * rows as u16,
        )? {
            self.resize_surfaces(width, height, true)?;
        }
        Ok(())
    }

    fn tab_did_terminate(&mut self, tab_id: TabId) {
        let mux = Mux::get().unwrap();
        let mut window = match mux.get_window_mut(self.get_mux_window_id()) {
            Some(window) => window,
            None => return,
        };

        window.remove_by_id(tab_id);

        if let Some(active) = window.get_active() {
            active.renderer().make_all_lines_dirty();
        }
        drop(window);
        self.update_title();
        self.deregister_tab(tab_id).ok();
    }
    fn test_for_child_exit(&mut self) -> bool {
        let mux = Mux::get().unwrap();
        let window = match mux.get_window(self.get_mux_window_id()) {
            Some(window) => window,
            None => return true,
        };
        let dead_tabs: Vec<Rc<Tab>> = window
            .iter()
            .filter_map(|tab| {
                if tab.is_dead() {
                    Some(Rc::clone(tab))
                } else {
                    None
                }
            })
            .collect();
        drop(window);
        for tab in dead_tabs {
            self.tab_did_terminate(tab.tab_id());
        }
        let empty = match mux.get_window(self.get_mux_window_id()) {
            Some(window) => window.is_empty(),
            None => true,
        };
        empty
    }
}
