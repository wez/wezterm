use crate::domain::DomainId;
use crate::tmux::{TmuxDomain, TmuxDomainState};
use crate::Mux;
use anyhow::anyhow;
use tmux_cc::*;

pub(crate) trait TmuxCommand {
    fn get_command(&self) -> String;
    fn process_result(&self, domain_id: DomainId, result: &Guarded) -> anyhow::Result<()>;
}

#[derive(Debug)]
pub(crate) struct PaneItem {
    session_id: TmuxSessionId,
    window_id: TmuxWindowId,
    pane_id: TmuxPaneId,
    pane_index: u64,
    cursor_x: u64,
    cursor_y: u64,
    pane_width: u64,
    pane_height: u64,
    pane_left: u64,
    pane_top: u64,
}

impl TmuxDomainState {
    fn sync_pane_state(&self, panes: &[PaneItem]) -> anyhow::Result<()> {
        Ok(())
    }
}

pub(crate) enum TmuxCommandResult {
    PaneList(Vec<PaneItem>),
}

pub(crate) struct ListAllPanes;
impl TmuxCommand for ListAllPanes {
    fn get_command(&self) -> String {
        "list-panes -aF '#{session_id} #{window_id} #{pane_id} \
            #{pane_index} #{cursor_x} #{cursor_y} #{pane_width} #{pane_height} \
            #{pane_left} #{pane_top}'\n"
            .to_owned()
    }

    fn process_result(&self, domain_id: DomainId, result: &Guarded) -> anyhow::Result<()> {
        let mut items = vec![];

        for line in result.output.split('\n') {
            if line.is_empty() {
                continue;
            }
            let mut fields = line.split(' ');
            let session_id = fields.next().ok_or_else(|| anyhow!("missing session_id"))?;
            let window_id = fields.next().ok_or_else(|| anyhow!("missing window_id"))?;
            let pane_id = fields.next().ok_or_else(|| anyhow!("missing pane_id"))?;
            let pane_index = fields
                .next()
                .ok_or_else(|| anyhow!("missing pane_index"))?
                .parse()?;
            let cursor_x = fields
                .next()
                .ok_or_else(|| anyhow!("missing cursor_x"))?
                .parse()?;
            let cursor_y = fields
                .next()
                .ok_or_else(|| anyhow!("missing cursor_y"))?
                .parse()?;
            let pane_width = fields
                .next()
                .ok_or_else(|| anyhow!("missing pane_width"))?
                .parse()?;
            let pane_height = fields
                .next()
                .ok_or_else(|| anyhow!("missing pane_height"))?
                .parse()?;
            let pane_left = fields
                .next()
                .ok_or_else(|| anyhow!("missing pane_left"))?
                .parse()?;
            let pane_top = fields
                .next()
                .ok_or_else(|| anyhow!("missing pane_top"))?
                .parse()?;

            // These ids all have various sigils such as `$`, `%`, `@`,
            // so skip those prior to parsing them
            let session_id = session_id[1..].parse()?;
            let window_id = window_id[1..].parse()?;
            let pane_id = pane_id[1..].parse()?;

            items.push(PaneItem {
                session_id,
                window_id,
                pane_id,
                pane_index,
                cursor_x,
                cursor_y,
                pane_width,
                pane_height,
                pane_left,
                pane_top,
            });
        }

        log::info!("panes in domain_id {}: {:?}", domain_id, items);
        let mux = Mux::get().expect("to be called on main thread");
        if let Some(domain) = mux.get_domain(domain_id) {
            if let Some(tmux_domain) = domain.downcast_ref::<TmuxDomain>() {
                return tmux_domain.inner.sync_pane_state(&items);
            }
        }
        anyhow::bail!("Tmux domain lost");
    }
}
