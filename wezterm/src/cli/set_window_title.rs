use clap::Parser;
use mux::pane::PaneId;
use mux::window::WindowId;
use std::collections::HashMap;
use wezterm_client::client::Client;

#[derive(Debug, Parser, Clone)]
pub struct SetWindowTitle {
    /// Specify the target window by its id
    #[arg(long, conflicts_with_all=&["pane_id"])]
    window_id: Option<WindowId>,
    /// Specify the current pane.
    /// The default is to use the current pane based on the
    /// environment variable WEZTERM_PANE.
    ///
    /// The pane is used to figure out which window
    /// should be renamed.
    #[arg(long)]
    pane_id: Option<PaneId>,

    /// The new title for the window
    title: String,
}

impl SetWindowTitle {
    pub async fn run(self, client: Client) -> anyhow::Result<()> {
        let panes = client.list_panes().await?;

        let mut pane_id_to_window_id = HashMap::new();

        for tabroot in panes.tabs {
            let mut cursor = tabroot.into_tree().cursor();

            loop {
                if let Some(entry) = cursor.leaf_mut() {
                    pane_id_to_window_id.insert(entry.pane_id, entry.window_id);
                }
                match cursor.preorder_next() {
                    Ok(c) => cursor = c,
                    Err(_) => break,
                }
            }
        }

        let window_id = if let Some(window_id) = self.window_id {
            window_id
        } else {
            // Find the current tab from the pane id
            let pane_id = client.resolve_pane_id(self.pane_id).await?;
            pane_id_to_window_id
                .get(&pane_id)
                .copied()
                .ok_or_else(|| anyhow::anyhow!("unable to resolve current window"))?
        };

        client
            .set_window_title(codec::WindowTitleChanged {
                window_id,
                title: self.title,
            })
            .await?;
        Ok(())
    }
}
