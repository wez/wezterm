use anyhow::{anyhow, Result};
use clap::Parser;
use codec::SetPaneZoomed;
use mux::pane::PaneId;
use std::collections::HashMap;
use wezterm_client::client::Client;

#[derive(Debug, Parser, Clone)]
pub struct ZoomPane {
    /// Specify the target pane.
    /// The default is to use the current pane based on the
    /// environment variable WEZTERM_PANE.
    #[arg(long)]
    pane_id: Option<PaneId>,

    /// Zooms the pane if it wasn't already zoomed
    #[arg(long, default_value = "true", default_value_ifs([
        ("unzoom", "true", "false"),
        ("toggle", "true", "false"),
    ]), conflicts_with_all=&["unzoom", "toggle"])]
    zoom: bool,

    /// Unzooms the pane if it was zoomed
    #[arg(long, conflicts_with_all=&["zoom", "toggle"])]
    unzoom: bool,

    /// Toggles the zoom state of the pane
    #[arg(long, conflicts_with_all=&["zoom", "unzoom"])]
    toggle: bool,
}

impl ZoomPane {
    pub async fn run(&self, client: Client) -> Result<()> {
        let panes = client.list_panes().await?;

        let mut pane_id_to_tab_id = HashMap::new();
        let mut tab_id_to_active_zoomed_pane_id = HashMap::new();

        for tabroot in panes.tabs {
            let mut cursor = tabroot.into_tree().cursor();

            loop {
                if let Some(entry) = cursor.leaf_mut() {
                    pane_id_to_tab_id.insert(entry.pane_id, entry.tab_id);
                    if entry.is_active_pane && entry.is_zoomed_pane {
                        tab_id_to_active_zoomed_pane_id.insert(entry.tab_id, entry.pane_id);
                    }
                }
                match cursor.preorder_next() {
                    Ok(c) => cursor = c,
                    Err(_) => break,
                }
            }
        }

        let pane_id = client.resolve_pane_id(self.pane_id).await?;
        let containing_tab_id = pane_id_to_tab_id
            .get(&pane_id)
            .copied()
            .ok_or_else(|| anyhow!("unable to resolve current tab"))?;

        if self.zoom || self.unzoom {
            client
                .set_zoomed(SetPaneZoomed {
                    containing_tab_id,
                    pane_id,
                    zoomed: self.zoom,
                })
                .await?;
        }

        if self.toggle {
            let is_zoomed = tab_id_to_active_zoomed_pane_id
                .get(&containing_tab_id)
                .copied()
                == Some(pane_id);

            client
                .set_zoomed(SetPaneZoomed {
                    containing_tab_id,
                    pane_id,
                    zoomed: !is_zoomed,
                })
                .await?;
        }
        Ok(())
    }
}
