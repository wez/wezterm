use crate::cli::resolve_pane_id;
use clap::Parser;
use mux::pane::PaneId;
use wezterm_client::client::Client;

#[derive(Debug, Parser, Clone)]
pub struct ActivatePane {
    /// Specify the target pane.
    /// The default is to use the current pane based on the
    /// environment variable WEZTERM_PANE.
    #[arg(long)]
    pane_id: Option<PaneId>,
}

impl ActivatePane {
    pub async fn run(&self, client: Client) -> anyhow::Result<()> {
        let pane_id = resolve_pane_id(&client, self.pane_id).await?;
        client
            .set_focused_pane_id(codec::SetFocusedPane { pane_id })
            .await?;
        Ok(())
    }
}
