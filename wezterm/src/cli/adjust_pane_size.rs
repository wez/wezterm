use crate::cli::activate_pane_direction::PaneDirectionParser;
use crate::cli::resolve_pane_id;
use clap::Parser;
use codec::{AdjustPaneSize, Pdu};
use config::keyassignment::PaneDirection;
use mux::pane::PaneId;
use wezterm_client::client::Client;

#[derive(Debug, Parser, Clone)]
pub struct CliAdjustPaneSize {
    /// Specify the direction to resize in
    #[arg(value_parser=PaneDirectionParser{})]
    direction: PaneDirection,
    /// Specify the number of cells to resize by, defaults to 1.
    #[arg(long)]
    amount: Option<usize>,
}

impl Into<AdjustPaneSize> for CliAdjustPaneSize {
    fn into(self) -> AdjustPaneSize {
        AdjustPaneSize {
            direction: self.direction,
            amount: self.amount.unwrap_or(1),
        }
    }
}

impl CliAdjustPaneSize {
    pub async fn run(&self, client: Client) -> anyhow::Result<()> {
        let pane_id = resolve_pane_id(&client, self.pane_id).await?;
        client.adjust_pane_size(Pdu::AdjustPaneSize(self.into()))
    }
}
