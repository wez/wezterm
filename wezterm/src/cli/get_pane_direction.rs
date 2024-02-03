use crate::cli::activate_pane_direction::PaneDirectionParser;
use clap::Parser;
use config::keyassignment::PaneDirection;
use mux::pane::PaneId;
use wezterm_client::client::Client;

#[derive(Debug, Parser, Clone)]
pub struct GetPaneDirection {
    /// Specify the current pane.
    /// The default is to use the current pane based on the
    /// environment variable WEZTERM_PANE.
    #[arg(long)]
    pane_id: Option<PaneId>,

    /// The direction to consider.
    #[arg(value_parser=PaneDirectionParser{})]
    direction: PaneDirection,
}

impl GetPaneDirection {
    pub async fn run(&self, client: Client) -> anyhow::Result<()> {
        let pane_id = client.resolve_pane_id(self.pane_id).await?;
        let response = client
            .get_pane_direction(codec::GetPaneDirection {
                pane_id,
                direction: self.direction,
            })
            .await?;
        if let Some(pane_id) = response.pane_id {
            println!("{pane_id}");
        }
        Ok(())
    }
}
