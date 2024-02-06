use clap::Parser;
use mux::pane::PaneId;
use mux::window::WindowId;
use wezterm_client::client::Client;

#[derive(Debug, Parser, Clone)]
pub struct MovePaneToNewTab {
    /// Specify the pane that should be moved.
    /// The default is to use the current pane based on the
    /// environment variable WEZTERM_PANE.
    #[arg(long)]
    pane_id: Option<PaneId>,

    /// Specify the window into which the new tab will be
    /// created.
    /// If omitted, the window associated with the current
    /// pane is used.
    #[arg(long)]
    window_id: Option<WindowId>,

    /// Create tab in a new window, rather than the window
    /// currently containing the pane.
    #[arg(long, conflicts_with = "window_id")]
    new_window: bool,

    /// If creating a new window, override the default workspace name
    /// with the provided name.  The default name is "default".
    #[arg(long)]
    workspace: Option<String>,
}

impl MovePaneToNewTab {
    pub async fn run(&self, client: Client) -> anyhow::Result<()> {
        let pane_id = client.resolve_pane_id(self.pane_id).await?;
        let window_id = if self.new_window {
            None
        } else {
            match self.window_id {
                Some(w) => Some(w),
                None => {
                    let panes = client.list_panes().await?;
                    let mut window_id = None;
                    'outer_move: for tabroot in panes.tabs {
                        let mut cursor = tabroot.into_tree().cursor();

                        loop {
                            if let Some(entry) = cursor.leaf_mut() {
                                if entry.pane_id == pane_id {
                                    window_id.replace(entry.window_id);
                                    break 'outer_move;
                                }
                            }
                            match cursor.preorder_next() {
                                Ok(c) => cursor = c,
                                Err(_) => break,
                            }
                        }
                    }
                    window_id
                }
            }
        };

        let moved = client
            .move_pane_to_new_tab(codec::MovePaneToNewTab {
                pane_id,
                window_id,
                workspace_for_new_window: self.workspace.clone(),
            })
            .await?;

        log::debug!("{:?}", moved);
        Ok(())
    }
}
