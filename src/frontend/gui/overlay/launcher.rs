//! The launcher is a menu that presents a list of activities that can
//! be launched, such as spawning a new tab in various domains or attaching
//! ssh/tls domains.
//! The launcher is implemented here as an overlay, but could potentially
//! be rendered as a popup/context menu if the system supports it; at the
//! time of writing our window layer doesn't provide an API for context
//! menus.
use crate::frontend::gui::termwindow::{ClipboardHelper, TermWindow};
use crate::keyassignment::{SpawnCommand, SpawnTabDomain};
use crate::mux::domain::{DomainId, DomainState};
use crate::mux::tab::TabId;
use crate::mux::window::WindowId;
use crate::mux::Mux;
use crate::termwiztermtab::TermWizTerminal;
use anyhow::anyhow;
use portable_pty::PtySize;
use termwiz::cell::{AttributeChange, CellAttributes};
use termwiz::color::ColorAttribute;
use termwiz::input::{InputEvent, KeyCode, KeyEvent, MouseButtons, MouseEvent};
use termwiz::surface::{Change, Position};
use termwiz::terminal::Terminal;

#[derive(Clone)]
enum Entry {
    Spawn {
        label: String,
        command: SpawnCommand,
        new_window: bool,
    },
    Attach {
        label: String,
        domain: DomainId,
    },
}

impl Entry {
    fn label(&self) -> &str {
        match self {
            Entry::Spawn { label, .. } => label,
            Entry::Attach { label, .. } => label,
        }
    }
}

pub fn launcher(
    _tab_id: TabId,
    domain_id_of_current_tab: DomainId,
    mut term: TermWizTerminal,
    mux_window_id: WindowId,
    domains: Vec<(DomainId, DomainState, String)>,
    clipboard: ClipboardHelper,
    size: PtySize,
) -> anyhow::Result<()> {
    let mut active_idx = 0;
    let mut entries = vec![];

    for (domain_id, domain_state, domain_name) in &domains {
        let entry = if *domain_state == DomainState::Attached {
            Entry::Spawn {
                label: format!("New Tab (domain `{}`)", domain_name),
                command: SpawnCommand {
                    domain: SpawnTabDomain::Domain(*domain_id),
                    ..SpawnCommand::default()
                },
                new_window: false,
            }
        } else {
            Entry::Attach {
                label: format!("Attach domain `{}`", domain_name),
                domain: *domain_id,
            }
        };

        // Preselect the entry that corresponds to the active tab
        // at the time that the launcher was set up, so that pressing
        // Enter immediately afterwards spawns a tab in the same domain.
        if *domain_id == domain_id_of_current_tab {
            active_idx = entries.len();
        }
        entries.push(entry);
    }

    fn render(
        active_idx: usize,
        entries: &[Entry],
        term: &mut TermWizTerminal,
    ) -> anyhow::Result<()> {
        let mut changes = vec![
            Change::ClearScreen(ColorAttribute::Default),
            Change::CursorPosition {
                x: Position::Absolute(0),
                y: Position::Absolute(0),
            },
            Change::Text(
                "Select an item and press Enter to launch it.  \
                Press Escape to cancel\r\n"
                    .to_string(),
            ),
            Change::AllAttributes(CellAttributes::default()),
        ];

        for (idx, entry) in entries.iter().enumerate() {
            if idx == active_idx {
                changes.push(AttributeChange::Reverse(true).into());
            }

            changes.push(Change::Text(format!(" {} \r\n", entry.label())));

            if idx == active_idx {
                changes.push(AttributeChange::Reverse(false).into());
            }
        }
        term.render(&changes)
    }

    term.render(&[Change::Title("Launcher".to_string())])?;
    render(active_idx, &entries, &mut term)?;

    fn launch(
        active_idx: usize,
        entries: &[Entry],
        size: PtySize,
        mux_window_id: WindowId,
        clipboard: ClipboardHelper,
    ) {
        match entries[active_idx].clone() {
            Entry::Spawn {
                command,
                new_window,
                ..
            } => {
                promise::spawn::spawn_into_main_thread(async move {
                    TermWindow::spawn_command_impl(
                        &command,
                        new_window,
                        size,
                        mux_window_id,
                        clipboard,
                    );
                });
            }
            Entry::Attach { domain, .. } => {
                promise::spawn::spawn_into_main_thread(async move {
                    // We can't inline do_domain_attach here directly
                    // because the compiler would then want its body
                    // to be Send :-/
                    do_domain_attach(domain);
                });
            }
        }
    }

    while let Ok(Some(event)) = term.poll_input(None) {
        match event {
            InputEvent::Key(KeyEvent {
                key: KeyCode::Char('k'),
                ..
            })
            | InputEvent::Key(KeyEvent {
                key: KeyCode::UpArrow,
                ..
            }) => {
                active_idx = active_idx.saturating_sub(1);
            }
            InputEvent::Key(KeyEvent {
                key: KeyCode::Char('j'),
                ..
            })
            | InputEvent::Key(KeyEvent {
                key: KeyCode::DownArrow,
                ..
            }) => {
                active_idx = (active_idx + 1).min(entries.len() - 1);
            }
            InputEvent::Key(KeyEvent {
                key: KeyCode::Escape,
                ..
            }) => {
                break;
            }
            InputEvent::Mouse(MouseEvent {
                y, mouse_buttons, ..
            }) => {
                if y > 0 && y as usize <= entries.len() {
                    active_idx = y as usize - 1;

                    if mouse_buttons == MouseButtons::LEFT {
                        launch(active_idx, &entries, size, mux_window_id, clipboard);
                        break;
                    } else if mouse_buttons != MouseButtons::NONE {
                        // Treat any other mouse button as cancel
                        break;
                    }
                }
            }
            InputEvent::Key(KeyEvent {
                key: KeyCode::Enter,
                ..
            }) => {
                launch(active_idx, &entries, size, mux_window_id, clipboard);
                break;
            }
            _ => {}
        }
        render(active_idx, &entries, &mut term)?;
    }

    Ok(())
}

fn do_domain_attach(domain: DomainId) {
    promise::spawn::spawn(async move {
        let mux = Mux::get().unwrap();
        let domain = mux
            .get_domain(domain)
            .ok_or_else(|| anyhow!("launcher attach called with unresolvable domain id!?"))?;
        domain.attach().await
    });
}
