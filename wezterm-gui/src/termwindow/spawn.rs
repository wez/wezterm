use crate::spawn::SpawnWhere;
use config::keyassignment::{SpawnCommand, SpawnTabDomain};
use config::TermConfig;
use std::sync::Arc;

impl super::TermWindow {
    pub fn spawn_command(&self, spawn: &SpawnCommand, spawn_where: SpawnWhere) {
        let size = if spawn_where == SpawnWhere::NewWindow {
            self.config.initial_size(
                self.dimensions.dpi as u32,
                crate::cell_pixel_dims(&self.config, self.dimensions.dpi as f64).ok(),
            )
        } else {
            self.terminal_size
        };
        let term_config = Arc::new(TermConfig::with_config(self.config.clone()));

        crate::spawn::spawn_command_impl(
            spawn,
            spawn_where,
            size,
            Some(self.mux_window_id),
            term_config,
        )
    }

    pub fn spawn_tab(&mut self, domain: &SpawnTabDomain) {
        self.spawn_command(
            &SpawnCommand {
                domain: domain.clone(),
                ..Default::default()
            },
            SpawnWhere::NewTab,
        );
    }
}
