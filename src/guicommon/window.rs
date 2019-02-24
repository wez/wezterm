use crate::config::Config;
use crate::font::FontConfiguration;
use crate::guicommon::tabs::{Tab, TabId, Tabs};
use crate::opengl::render::Renderer;
use crate::opengl::textureatlas::OutOfTextureSpace;
use crate::pty::unix::openpty;
use failure::Error;
use glium;
use std::cell::RefMut;
use std::rc::Rc;

pub struct Dimensions {
    pub width: u16,
    pub height: u16,
    pub cell_height: usize,
    pub cell_width: usize,
}

pub trait TerminalWindow {
    fn get_tabs_mut(&mut self) -> &mut Tabs;
    fn get_tabs(&self) -> &Tabs;
    fn set_window_title(&mut self, title: &str) -> Result<(), Error>;
    fn frame(&self) -> glium::Frame;
    fn renderer(&mut self) -> &mut Renderer;
    fn renderer_and_terminal(&mut self) -> (&mut Renderer, RefMut<term::Terminal>);
    fn recreate_texture_atlas(&mut self, size: u32) -> Result<(), Error>;
    fn tab_was_created(&mut self, tab_id: TabId) -> Result<(), Error>;
    fn config(&self) -> &Rc<Config>;
    fn fonts(&self) -> &Rc<FontConfiguration>;
    fn get_dimensions(&self) -> Dimensions;

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

        let tab = Tab::new(terminal, process, pty);
        let tab_id = tab.tab_id();

        self.get_tabs_mut().push(tab);
        let len = self.get_tabs().len();
        self.activate_tab(len - 1)?;

        self.tab_was_created(tab_id)?;

        Ok(tab_id)
    }
}
