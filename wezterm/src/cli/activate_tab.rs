use clap::Parser;
use mux::pane::PaneId;
use mux::tab::TabId;
use std::collections::HashMap;
use wezterm_client::client::Client;

#[derive(Debug, Parser, Clone)]
pub struct ActivateTab {
    /// Specify the target tab by its id
    #[arg(long, conflicts_with_all=&["tab_index", "tab_relative", "no_wrap", "pane_id"])]
    tab_id: Option<TabId>,

    /// Specify the target tab by its index within the window
    /// that holds the current pane.
    /// Indices are 0-based, with 0 being the left-most tab.
    /// Negative numbers can be used to reference the right-most
    /// tab, so -1 is the right-most tab, -2 is the penultimate
    /// tab and so on.
    #[arg(long, allow_hyphen_values = true)]
    tab_index: Option<isize>,

    /// Specify the target tab by its relative offset.
    /// -1 selects the tab to the left. -2 two tabs to the left.
    /// 1 is one tab to the right and so on.
    ///
    /// Unless `--no-wrap` is specified, relative moves wrap
    /// around from the left-most to right-most and vice versa.
    #[arg(long, allow_hyphen_values = true)]
    tab_relative: Option<isize>,

    /// When used with tab-relative, prevents wrapping around
    /// and will instead clamp to the left-most when moving left
    /// or right-most when moving right.
    #[arg(long, requires = "tab_relative")]
    no_wrap: bool,

    /// Specify the current pane.
    /// The default is to use the current pane based on the
    /// environment variable WEZTERM_PANE.
    ///
    /// The pane is used to figure out which window
    /// contains appropriate tabs
    #[arg(long)]
    pane_id: Option<PaneId>,
}

impl ActivateTab {
    pub async fn run(&self, client: Client) -> anyhow::Result<()> {
        let panes = client.list_panes().await?;

        let mut pane_id_to_tab_id = HashMap::new();
        let mut tab_id_to_active_pane_id = HashMap::new();
        let mut tabs_by_window = HashMap::new();
        let mut window_by_tab_id = HashMap::new();

        for tabroot in panes.tabs {
            let mut cursor = tabroot.into_tree().cursor();

            loop {
                if let Some(entry) = cursor.leaf_mut() {
                    pane_id_to_tab_id.insert(entry.pane_id, entry.tab_id);
                    if entry.is_active_pane {
                        tab_id_to_active_pane_id.insert(entry.tab_id, entry.pane_id);
                    }
                    window_by_tab_id.insert(entry.tab_id, entry.window_id);
                    let win = tabs_by_window
                        .entry(entry.window_id)
                        .or_insert_with(Vec::new);
                    if win.last().copied() != Some(entry.tab_id) {
                        win.push(entry.tab_id);
                    }
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
            let current_tab_id = pane_id_to_tab_id
                .get(&pane_id)
                .copied()
                .ok_or_else(|| anyhow::anyhow!("unable to resolve current tab"))?;
            let window = window_by_tab_id
                .get(&current_tab_id)
                .copied()
                .ok_or_else(|| anyhow::anyhow!("unable to resolve current window"))?;

            let tabs = tabs_by_window
                .get(&window)
                .ok_or_else(|| anyhow::anyhow!("unable to resolve tabs for current window"))?;
            let max = tabs.len();
            anyhow::ensure!(max > 0, "window has no tabs!?");

            if let Some(tab_index) = self.tab_index {
                // This logic is coupled with TermWindow::activate_tab
                // If you update this, update that!
                let tab_idx = if tab_index < 0 {
                    max.saturating_sub(tab_index.abs() as usize)
                } else {
                    tab_index as usize
                };

                tabs.get(tab_idx)
                    .copied()
                    .ok_or_else(|| anyhow::anyhow!("tab index {tab_index} is invalid"))?
            } else if let Some(delta) = self.tab_relative {
                // This logic is coupled with TermWindow::activate_tab_relative
                // If you update this, update that!
                let wrap = !self.no_wrap;
                let active = tabs
                    .iter()
                    .position(|&tab_id| tab_id == current_tab_id)
                    .ok_or_else(|| anyhow::anyhow!("current tab is not in window!?"))?
                    as isize;

                let tab = active + delta;
                let tab_idx = if wrap {
                    let tab = if tab < 0 { max as isize + tab } else { tab };
                    (tab as usize % max) as isize
                } else {
                    if tab < 0 {
                        0
                    } else if tab >= max as isize {
                        max as isize - 1
                    } else {
                        tab
                    }
                };
                tabs.get(tab_idx as usize)
                    .copied()
                    .ok_or_else(|| anyhow::anyhow!("tab index {tab_idx} is invalid"))?
            } else {
                anyhow::bail!("impossible arguments!");
            }
        };

        // Now that we know which tab we want to activate, figure out
        // which pane will be the active pane
        let target_pane = tab_id_to_active_pane_id
            .get(&tab_id)
            .copied()
            .ok_or_else(|| {
                anyhow::anyhow!("could not determine which pane should be active for tab {tab_id}")
            })?;

        client
            .set_focused_pane_id(codec::SetFocusedPane {
                pane_id: target_pane,
            })
            .await?;
        Ok(())
    }
}
