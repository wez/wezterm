use crate::cli::activate_pane_direction::PaneDirectionParser;
use clap::Parser;
use codec::AdjustPaneSize;
use config::keyassignment::PaneDirection;
use mux::pane::PaneId;
use wezterm_client::client::Client;

#[derive(Debug, Parser, Clone)]
pub struct CliAdjustPaneSize {
    /// Specify the target pane.
    /// The default is to use the current pane based on the
    /// environment variable WEZTERM_PANE.
    #[arg(long)]
    pane_id: Option<PaneId>,
    /// Specify the direction to resize in
    #[arg(value_parser=PaneDirectionParser{})]
    direction: PaneDirection,
    /// Specify the number of cells to resize by, defaults to 1.
    #[arg(long)]
    amount: Option<usize>,
}

impl CliAdjustPaneSize {
    pub async fn run(&self, client: Client) -> anyhow::Result<()> {
        let pane_id = client.resolve_pane_id(self.pane_id).await?;
        match client
            .adjust_pane_size(AdjustPaneSize {
                pane_id,
                direction: self.direction,
                amount: self.amount.unwrap_or(1),
            })
            .await
        {
            Ok(_) => Ok(()),
            Err(err) => Err(err),
        }
    }
}
