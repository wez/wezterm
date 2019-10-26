#[cfg(feature = "enable-winit")]
use crate::config::Config;
#[cfg(feature = "enable-winit")]
use crate::font::FontConfiguration;
use crate::mux::domain::DomainId;
#[cfg(feature = "enable-winit")]
use crate::mux::tab::{Tab, TabId};
#[cfg(feature = "enable-winit")]
use crate::mux::window::WindowId;
#[cfg(feature = "enable-winit")]
use crate::mux::Mux;
#[cfg(feature = "enable-winit")]
use crate::opengl::render::Renderer;
#[cfg(feature = "enable-winit")]
use crate::opengl::textureatlas::OutOfTextureSpace;
#[cfg(feature = "enable-winit")]
use glium;
#[cfg(feature = "enable-winit")]
use portable_pty::PtySize;
#[cfg(feature = "enable-winit")]
use std::rc::Rc;
#[cfg(feature = "enable-winit")]
use std::sync::Arc;

/// When spawning a tab, specify which domain should be used to
/// host/spawn that tab.
#[derive(Debug, Clone)]
pub enum SpawnTabDomain {
    /// Use the default domain
    DefaultDomain,
    /// Use the domain from the current tab in the associated window
    CurrentTabDomain,
    /// Use a specific domain by id
    Domain(DomainId),
    /// Use a specific domain by name
    DomainName(String),
}

/// Reports the currently configured physical size of the display
/// surface (physical pixels, not adjusted for dpi) and the current
/// cell dimensions, also in physical pixels
#[cfg(feature = "enable-winit")]
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
#[cfg(feature = "enable-winit")]
pub trait TerminalWindow {
    fn set_window_title(&mut self, title: &str) -> failure::Fallible<()>;
    fn get_mux_window_id(&self) -> WindowId;
    fn frame(&self) -> glium::Frame;
    fn renderer(&mut self) -> &mut Renderer;
    fn recreate_texture_atlas(&mut self, size: u32) -> failure::Fallible<()>;
    fn advise_renderer_that_scaling_has_changed(
        &mut self,
        cell_width: usize,
        cell_height: usize,
    ) -> failure::Fallible<()>;
    fn advise_renderer_of_resize(&mut self, width: u16, height: u16) -> failure::Fallible<()>;
    fn config(&self) -> &Arc<Config>;
    fn fonts(&self) -> &Rc<FontConfiguration>;
    fn get_dimensions(&self) -> Dimensions;
    fn resize_if_not_full_screen(&mut self, width: u16, height: u16) -> failure::Fallible<bool>;
    fn check_for_resize(&mut self) -> failure::Fallible<()> {
        Ok(())
    }

    fn hide_window(&mut self) {}
    fn show_window(&mut self) {}

    fn activate_tab(&mut self, tab_idx: usize) -> failure::Fallible<()> {
        let mux = Mux::get().unwrap();
        let mut window = mux
            .get_window_mut(self.get_mux_window_id())
            .ok_or_else(|| failure::format_err!("no such window"))?;

        let max = window.len();
        if tab_idx < max {
            window.set_active(tab_idx);

            drop(window);
            self.update_title();
        }
        Ok(())
    }

    fn activate_tab_relative(&mut self, delta: isize) -> failure::Fallible<()> {
        let mux = Mux::get().unwrap();
        let window = mux
            .get_window(self.get_mux_window_id())
            .ok_or_else(|| failure::format_err!("no such window"))?;

        let max = window.len();
        failure::ensure!(max > 0, "no more tabs");

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

        let title = match window.get_active() {
            Some(tab) => tab.get_title(),
            None => return,
        };

        drop(window);

        if num_tabs == 1 {
            self.set_window_title(&title).ok();
        } else {
            self.set_window_title(&format!("[{}/{}] {}", tab_no + 1, num_tabs, title))
                .ok();
        }
    }

    fn paint_if_needed(&mut self) -> failure::Fallible<()> {
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

    fn paint(&mut self) -> failure::Fallible<()> {
        let mux = Mux::get().unwrap();
        let tab = match mux.get_active_tab_for_window(self.get_mux_window_id()) {
            Some(tab) => tab,
            None => return Ok(()),
        };

        let start = std::time::Instant::now();
        let mut target = self.frame();
        let res = {
            let renderer = self.renderer();
            let palette = tab.palette();
            renderer.paint(&mut target, &mut *tab.renderer(), &palette)
        };

        // Ensure that we finish() the target before we let the
        // error bubble up, otherwise we lose the context.
        target
            .finish()
            .expect("target.finish failed and we don't know how to recover");
        log::debug!("paint elapsed={:?}", start.elapsed());

        // The only error we want to catch is texture space related;
        // when that happens we need to blow our glyph cache and
        // allocate a newer bigger texture.
        match res {
            Err(err) => {
                if let Some(&OutOfTextureSpace { size }) = err.downcast_ref::<OutOfTextureSpace>() {
                    log::error!("out of texture space, allocating {}", size);
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

    fn spawn_tab(&mut self, domain: &SpawnTabDomain) -> failure::Fallible<TabId> {
        let dims = self.get_dimensions();

        let rows = (dims.height as usize + 1) / dims.cell_height;
        let cols = (dims.width as usize + 1) / dims.cell_width;

        let size = PtySize {
            rows: rows as u16,
            cols: cols as u16,
            pixel_width: dims.width,
            pixel_height: dims.height,
        };

        let mux = Mux::get().unwrap();

        let domain = match domain {
            SpawnTabDomain::DefaultDomain => mux.default_domain().clone(),
            SpawnTabDomain::CurrentTabDomain => {
                let tab = match mux.get_active_tab_for_window(self.get_mux_window_id()) {
                    Some(tab) => tab,
                    None => failure::bail!("window has no tabs?"),
                };
                mux.get_domain(tab.domain_id()).ok_or_else(|| {
                    failure::format_err!("current tab has unresolvable domain id!?")
                })?
            }
            SpawnTabDomain::Domain(id) => mux.get_domain(*id).ok_or_else(|| {
                failure::format_err!("spawn_tab called with unresolvable domain id!?")
            })?,
            SpawnTabDomain::DomainName(name) => mux.get_domain_by_name(&name).ok_or_else(|| {
                failure::format_err!("spawn_tab called with unresolvable domain name {}", name)
            })?,
        };
        let tab = domain.spawn(size, None, self.get_mux_window_id())?;
        let tab_id = tab.tab_id();

        let len = {
            let window = mux
                .get_window(self.get_mux_window_id())
                .ok_or_else(|| failure::format_err!("no such window!?"))?;
            window.len()
        };
        self.activate_tab(len - 1)?;
        Ok(tab_id)
    }

    fn resize_surfaces(&mut self, width: u16, height: u16, force: bool) -> failure::Fallible<bool> {
        let dims = self.get_dimensions();

        if force || width != dims.width || height != dims.height {
            log::debug!("resize {},{}", width, height);

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
                .ok_or_else(|| failure::format_err!("no such window!?"))?;
            for tab in window.iter() {
                tab.resize(PtySize {
                    rows,
                    cols,
                    pixel_width: width as u16,
                    pixel_height: height as u16,
                })?;
            }

            Ok(true)
        } else {
            log::debug!("ignoring extra resize");
            Ok(false)
        }
    }

    fn scaling_changed(
        &mut self,
        font_scale: Option<f64>,
        dpi_scale: Option<f64>,
        width: u16,
        height: u16,
    ) -> failure::Fallible<()> {
        let fonts = self.fonts();
        let dpi_scale = dpi_scale.unwrap_or_else(|| fonts.get_dpi_scale());
        let font_scale = font_scale.unwrap_or_else(|| fonts.get_font_scale());
        log::debug!(
            "TerminalWindow::scaling_changed dpi_scale={} font_scale={}",
            dpi_scale,
            font_scale
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
    }

    // let_and_return is needed here to satisfy the borrow checker
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::let_and_return))]
    fn test_for_child_exit(&mut self) -> bool {
        let mux = Mux::get().unwrap();
        let window = match mux.get_window(self.get_mux_window_id()) {
            Some(window) => window,
            None => return true,
        };
        let dead_tabs: Vec<Rc<dyn Tab>> = window
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
