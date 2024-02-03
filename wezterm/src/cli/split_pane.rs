use crate::cli::resolve_relative_cwd;
use clap::{Parser, ValueHint};
use mux::pane::PaneId;
use mux::tab::{SplitDirection, SplitRequest, SplitSize};
use portable_pty::cmdbuilder::CommandBuilder;
use std::ffi::OsString;
use wezterm_client::client::Client;

#[derive(Debug, Parser, Clone)]
pub struct SplitPane {
    /// Specify the pane that should be split.
    /// The default is to use the current pane based on the
    /// environment variable WEZTERM_PANE.
    #[arg(long)]
    pane_id: Option<PaneId>,

    /// Equivalent to `--right`. If neither this nor any other direction
    /// is specified, the default is equivalent to `--bottom`.
    #[arg(long, conflicts_with_all=&["left", "right", "top", "bottom"])]
    horizontal: bool,

    /// Split horizontally, with the new pane on the left
    #[arg(long, conflicts_with_all=&["right", "top", "bottom"])]
    left: bool,

    /// Split horizontally, with the new pane on the right
    #[arg(long, conflicts_with_all=&["left", "top", "bottom"])]
    right: bool,

    /// Split vertically, with the new pane on the top
    #[arg(long, conflicts_with_all=&["left", "right", "bottom"])]
    top: bool,

    /// Split vertically, with the new pane on the bottom
    #[arg(long, conflicts_with_all=&["left", "right", "top"])]
    bottom: bool,

    /// Rather than splitting the active pane, split the entire
    /// window.
    #[arg(long)]
    top_level: bool,

    /// The number of cells that the new split should have.
    /// If omitted, 50% of the available space is used.
    #[arg(long)]
    cells: Option<usize>,

    /// Specify the number of cells that the new split should
    /// have, expressed as a percentage of the available space.
    #[arg(long, conflicts_with = "cells")]
    percent: Option<u8>,

    /// Specify the current working directory for the initially
    /// spawned program
    #[arg(long, value_parser, value_hint=ValueHint::DirPath)]
    cwd: Option<OsString>,

    /// Instead of spawning a new command, move the specified
    /// pane into the newly created split.
    #[arg(long, conflicts_with_all=&["cwd", "prog"])]
    move_pane_id: Option<PaneId>,

    /// Instead of executing your shell, run PROG.
    /// For example: `wezterm cli split-pane -- bash -l` will spawn bash
    /// as if it were a login shell.
    #[arg(value_parser, value_hint=ValueHint::CommandWithArguments, num_args=1..)]
    prog: Vec<OsString>,
}

impl SplitPane {
    pub async fn run(self, client: Client) -> anyhow::Result<()> {
        let pane_id = client.resolve_pane_id(self.pane_id).await?;

        let direction = if self.left || self.right || self.horizontal {
            SplitDirection::Horizontal
        } else if self.top || self.bottom {
            SplitDirection::Vertical
        } else {
            SplitDirection::Vertical
        };
        let target_is_second = !(self.left || self.top);
        let size = match (self.cells, self.percent) {
            (Some(c), _) => SplitSize::Cells(c),
            (_, Some(p)) => SplitSize::Percent(p),
            (None, None) => SplitSize::Percent(50),
        };

        let split_request = SplitRequest {
            direction,
            target_is_second,
            size,
            top_level: self.top_level,
        };

        let spawned = client
            .split_pane(codec::SplitPane {
                pane_id,
                split_request,
                domain: config::keyassignment::SpawnTabDomain::CurrentPaneDomain,
                command: if self.prog.is_empty() {
                    None
                } else {
                    let builder = CommandBuilder::from_argv(self.prog);
                    Some(builder)
                },
                command_dir: resolve_relative_cwd(self.cwd)?,
                move_pane_id: self.move_pane_id,
            })
            .await?;

        log::debug!("{:?}", spawned);
        println!("{}", spawned.pane_id);
        Ok(())
    }
}
