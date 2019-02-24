use crate::guicommon::tabs::Tabs;
use crate::opengl::render::Renderer;
use crate::opengl::textureatlas::OutOfTextureSpace;
use failure::Error;
use glium;
use glium::backend::Facade;
use std::cell::RefMut;

pub trait TerminalWindow {
    fn get_tabs_mut(&mut self) -> &mut Tabs;
    fn get_tabs(&self) -> &Tabs;
    fn set_window_title(&mut self, title: &str) -> Result<(), Error>;
    fn frame(&self) -> glium::Frame;
    fn renderer(&mut self) -> &mut Renderer;
    fn renderer_and_terminal(&mut self) -> (&mut Renderer, RefMut<term::Terminal>);
    fn recreate_texture_atlas(&mut self, size: u32) -> Result<(), Error>;

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

        let title = {
            let terminal = self.get_tabs().get_active().unwrap().terminal();
            terminal.get_title().to_owned()
        };

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
        if tab.terminal().has_dirty_lines() {
            self.paint()?;
        }
        Ok(())
    }

    fn paint(&mut self) -> Result<(), Error> {
        let mut target = self.frame();

        let res = {
            let (renderer, mut terminal) = self.renderer_and_terminal();
            renderer.paint(&mut target, &mut terminal)
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
                        .terminal()
                        .make_all_lines_dirty();
                    // Recursively initiate a new paint
                    return self.paint();
                }
                Err(err)
            }
            Ok(_) => Ok(()),
        }
    }
}
