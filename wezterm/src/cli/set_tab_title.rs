use clap::Parser;
use mux::pane::PaneId;
use mux::tab::TabId;
use std::collections::HashMap;
use wezterm_client::client::Client;

#[derive(Debug, Parser, Clone)]
pub struct SetTabTitle {
    /// Specify the target tab by its id
    #[arg(long, conflicts_with_all=&["pane_id"])]
    tab_id: Option<TabId>,
    /// Specify the current pane.
    /// The default is to use the current pane based on the
    /// environment variable WEZTERM_PANE.
    ///
    /// The pane is used to figure out which tab should be renamed.
    #[arg(long)]
    pane_id: Option<PaneId>,

    /// The new title for the tab
    title: String,
}

impl SetTabTitle {
    pub async fn run(self, client: Client) -> anyhow::Result<()> {
        let panes = client.list_panes().await?;

        let mut pane_id_to_tab_id = HashMap::new();

        for tabroot in panes.tabs {
            let mut cursor = tabroot.into_tree().cursor();

            loop {
                if let Some(entry) = cursor.leaf_mut() {
                    pane_id_to_tab_id.insert(entry.pane_id, entry.tab_id);
                }
                match cursor.preorder_next() {
                    Ok(c) => cursor = c,
                    Err(_) => break,
                }
            }
        }

        let tab_id = if let Some(tab_id) = self.tab_id {
            tab_id
        } else {
            // Find the current tab from the pane id
            let pane_id = client.resolve_pane_id(self.pane_id).await?;
            pane_id_to_tab_id
                .get(&pane_id)
                .copied()
                .ok_or_else(|| anyhow::anyhow!("unable to resolve current tab"))?
        };

        client
            .set_tab_title(codec::TabTitleChanged {
                tab_id,
                title: self.title,
            })
            .await?;
        Ok(())
    }
}
