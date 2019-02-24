use crate::guicommon::tabs::Tabs;
use failure::Error;

pub trait TerminalWindow {
    fn get_tabs(&mut self) -> &mut Tabs;
    fn set_window_title(&mut self, title: &str) -> Result<(), Error>;

    fn activate_tab(&mut self, tab_idx: usize) -> Result<(), Error> {
        let max = self.get_tabs().len();
        if tab_idx < max {
            self.get_tabs().set_active(tab_idx);
            self.update_title();
        }
        Ok(())
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
}
