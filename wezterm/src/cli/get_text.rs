use clap::Parser;
use mux::pane::PaneId;
use termwiz_funcs::lines_to_escapes;
use wezterm_client::client::Client;
use wezterm_term::{ScrollbackOrVisibleRowIndex, StableRowIndex};

#[derive(Debug, Parser, Clone)]
pub struct GetText {
    /// Specify the target pane.
    /// The default is to use the current pane based on the
    /// environment variable WEZTERM_PANE.
    #[arg(long)]
    pane_id: Option<PaneId>,

    /// The starting line number.
    /// 0 is the first line of terminal screen.
    /// Negative numbers proceed backwards into the scrollback.
    /// The default value is unspecified is 0, the first line of
    /// the terminal screen.
    #[arg(long, allow_hyphen_values = true)]
    start_line: Option<ScrollbackOrVisibleRowIndex>,

    /// The ending line number.
    /// 0 is the first line of terminal screen.
    /// Negative numbers proceed backwards into the scrollback.
    /// The default value if unspecified is the bottom of the
    /// the terminal screen.
    #[arg(long, allow_hyphen_values = true)]
    end_line: Option<ScrollbackOrVisibleRowIndex>,

    /// Include escape sequences that color and style the text.
    /// If omitted, unattributed text will be returned.
    #[arg(long)]
    escapes: bool,
}

impl GetText {
    pub async fn run(self, client: Client) -> anyhow::Result<()> {
        let pane_id = client.resolve_pane_id(self.pane_id).await?;

        let info = client
            .get_dimensions(codec::GetPaneRenderableDimensions { pane_id })
            .await?;

        let start_line = match self.start_line {
            None => info.dimensions.physical_top,
            Some(n) if n >= 0 => info.dimensions.physical_top + n as StableRowIndex,
            Some(n) => {
                let line = info.dimensions.physical_top as isize + n as isize;
                if line < info.dimensions.scrollback_top as isize {
                    info.dimensions.scrollback_top
                } else {
                    line as StableRowIndex
                }
            }
        };

        let end_line = match self.end_line {
            None => info.dimensions.physical_top + info.dimensions.viewport_rows as StableRowIndex,
            Some(n) if n >= 0 => info.dimensions.physical_top + n as StableRowIndex,
            Some(n) => {
                let line = info.dimensions.physical_top as isize + n as isize;
                if line < info.dimensions.scrollback_top as isize {
                    info.dimensions.scrollback_top
                } else {
                    line as StableRowIndex
                }
            }
        };

        let lines = client
            .get_lines(codec::GetLines {
                pane_id: pane_id.into(),
                lines: vec![start_line..end_line + 1],
            })
            .await?;

        let lines = lines
            .lines
            .extract_data()
            .0
            .into_iter()
            .map(|(_idx, line)| line)
            .collect();

        if self.escapes {
            println!("{}", lines_to_escapes(lines)?);
        } else {
            lines.iter().for_each(|line| println!("{}", line.as_str()));
        }
        Ok(())
    }
}
