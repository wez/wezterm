use crate::config::Config;
use crate::font::FontConfiguration;
use crate::guicommon::tabs::{Tab, TabId, Tabs};
use crate::mux::renderable::Renderable;
use crate::opengl::render::Renderer;
use crate::opengl::textureatlas::OutOfTextureSpace;
use crate::openpty;
use failure::Error;
use glium;
use std::rc::Rc;

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
    fn get_tabs_mut(&mut self) -> &mut Tabs;
    fn get_tabs(&self) -> &Tabs;
    fn set_window_title(&mut self, title: &str) -> Result<(), Error>;
    fn frame(&self) -> glium::Frame;
    fn renderer(&mut self) -> &mut Renderer;
    fn renderer_and_tab(&mut self) -> (&mut Renderer, &Tab);
    fn recreate_texture_atlas(&mut self, size: u32) -> Result<(), Error>;
    fn advise_renderer_that_scaling_has_changed(
        &mut self,
        cell_width: usize,
        cell_height: usize,
    ) -> Result<(), Error>;
    fn advise_renderer_of_resize(&mut self, width: u16, height: u16) -> Result<(), Error>;
    fn tab_was_created(&mut self, tab: &Rc<Tab>) -> Result<(), Error>;
    fn deregister_tab(&mut self, tab_id: TabId) -> Result<(), Error>;
    fn config(&self) -> &Rc<Config>;
    fn fonts(&self) -> &Rc<FontConfiguration>;
    fn get_dimensions(&self) -> Dimensions;
    fn resize_if_not_full_screen(&mut self, width: u16, height: u16) -> Result<bool, Error>;
    fn check_for_resize(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn activate_tab(&mut self, tab_idx: usize) -> Result<(), Error> {
        let max = self.get_tabs().len();
        if tab_idx < max {
            self.get_tabs_mut().set_active(tab_idx);
            self.update_title();
        }
        Ok(())
    }

    fn activate_tab_relative(&mut self, delta: isize) -> Result<(), Error> {
        let max = self.get_tabs().len();
        let active = self.get_tabs().get_active_idx() as isize;
        let tab = active + delta;
        let tab = if tab < 0 { max as isize + tab } else { tab };
        self.activate_tab(tab as usize % max)
    }

    fn update_title(&mut self) {
        let num_tabs = self.get_tabs().len();

        if num_tabs == 0 {
            return;
        }
        let tab_no = self.get_tabs().get_active_idx();

        let title = self.get_tabs().get_active().unwrap().get_title();

        if num_tabs == 1 {
            self.set_window_title(&title).ok();
        } else {
            self.set_window_title(&format!("[{}/{}] {}", tab_no + 1, num_tabs, title))
                .ok();
        }
    }

    fn paint_if_needed(&mut self) -> Result<(), Error> {
        let tab = match self.get_tabs().get_active() {
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

        let res = {
            let (renderer, tab) = self.renderer_and_tab();
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
                    self.get_tabs_mut()
                        .get_active()
                        .unwrap()
                        .renderer()
                        .make_all_lines_dirty();
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

        let terminal = term::Terminal::new(
            rows,
            cols,
            config.scrollback_lines.unwrap_or(3500),
            config.hyperlink_rules.clone(),
        );

        let tab = Rc::new(Tab::new(terminal, process, pty));
        let tab_id = tab.tab_id();

        self.get_tabs_mut().push(&tab);
        let len = self.get_tabs().len();
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

            let tabs = self.get_tabs();
            for tab in tabs.iter() {
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
        if let Some(tab) = self.get_tabs().get_active() {
            tab.renderer().make_all_lines_dirty();
        }
        fonts.change_scaling(font_scale, dpi_scale);

        let metrics = fonts.default_font_metrics()?;
        let (cell_height, cell_width) = (metrics.cell_height, metrics.cell_width);

        // It is desirable to preserve the terminal rows/cols when scaling,
        // so we query for that information here.
        // If the backend supports `resize_if_not_full_screen` then we'll try
        // to resize the window to match the new cell metrics.
        let (rows, cols) = {
            self.get_tabs()
                .get_active()
                .unwrap()
                .renderer()
                .physical_dimensions()
        };

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
        self.get_tabs_mut().remove_by_id(tab_id);
        if let Some(active) = self.get_tabs().get_active() {
            active.renderer().make_all_lines_dirty();
            self.update_title();
        }
        self.deregister_tab(tab_id).ok();
    }
    fn test_for_child_exit(&mut self) -> bool {
        let tabs = self.get_tabs();
        let dead_tabs: Vec<Rc<Tab>> = tabs
            .iter()
            .filter_map(|tab| {
                if tab.is_dead() {
                    Some(Rc::clone(tab))
                } else {
                    None
                }
            })
            .collect();
        for tab in dead_tabs {
            self.tab_did_terminate(tab.tab_id());
        }
        self.get_tabs().is_empty()
    }
}
