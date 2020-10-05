//! The launcher is a menu that presents a list of activities that can
//! be launched, such as spawning a new tab in various domains or attaching
//! ssh/tls domains.
//! The launcher is implemented here as an overlay, but could potentially
//! be rendered as a popup/context menu if the system supports it; at the
//! time of writing our window layer doesn't provide an API for context
//! menus.
use crate::gui::termwindow::{ClipboardHelper, SpawnWhere, TermWindow};
use crate::termwiztermtab::TermWizTerminal;
use anyhow::anyhow;
use config::configuration;
use config::keyassignment::{SpawnCommand, SpawnTabDomain};
use mux::domain::{DomainId, DomainState};
use mux::tab::TabId;
use mux::window::WindowId;
use mux::Mux;
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
        spawn_where: SpawnWhere,
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

#[cfg(windows)]
fn enumerate_wsl_entries(entries: &mut Vec<Entry>) -> anyhow::Result<()> {
    use std::os::windows::process::CommandExt;
    let mut cmd = std::process::Command::new("wsl.exe");
    cmd.arg("-l");
    cmd.creation_flags(winapi::um::winbase::CREATE_NO_WINDOW);
    let output = cmd.output()?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    anyhow::ensure!(
        output.status.success(),
        "wsl -l command invocation failed: {}",
        stderr
    );

    /// Ungh: https://github.com/microsoft/WSL/issues/4456
    fn utf16_to_utf8(bytes: &[u8]) -> anyhow::Result<String> {
        if bytes.len() % 2 != 0 {
            anyhow::bail!("input data has odd length, cannot be utf16");
        }

        // This is "safe" because we checked that the length seems reasonable,
        // and our new slice is within those same bounds.
        let wide: &[u16] =
            unsafe { std::slice::from_raw_parts(bytes.as_ptr() as *const u16, bytes.len() / 2) };

        String::from_utf16(wide).map_err(|_| anyhow!("wsl -l output is not valid utf16"))
    }

    let wsl_list = utf16_to_utf8(&output.stdout)?.replace("\r\n", "\n");
    for line in wsl_list.lines().skip(1) {
        // Remove the "(Default)" marker, if present, to leave just the distro name
        let distro = line.replace(" (Default)", "");
        let label = format!("{} (WSL)", distro);

        entries.push(Entry::Spawn {
            label: label.clone(),
            command: SpawnCommand {
                label: Some(label),
                args: Some(vec![
                    "wsl.exe".to_owned(),
                    "--distribution".to_owned(),
                    distro.to_owned(),
                ]),
                ..Default::default()
            },
            spawn_where: SpawnWhere::NewTab,
        });
    }

    Ok(())
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

    term.set_raw_mode()?;

    let config = configuration();

    // Pull in the user defined entries from the launch_menu
    // section of the configuration.
    for item in &config.launch_menu {
        entries.push(Entry::Spawn {
            label: match item.label.as_ref() {
                Some(label) => label.to_string(),
                None => match item.args.as_ref() {
                    Some(args) => args.join(" "),
                    None => "(default shell)".to_string(),
                },
            },
            command: item.clone(),
            spawn_where: SpawnWhere::NewTab,
        });
    }

    #[cfg(windows)]
    {
        if config.add_wsl_distributions_to_launch_menu {
            let _ = enumerate_wsl_entries(&mut entries);
        }
    }

    for (domain_id, domain_state, domain_name) in &domains {
        let entry = if *domain_state == DomainState::Attached {
            Entry::Spawn {
                label: format!("New Tab ({})", domain_name),
                command: SpawnCommand {
                    domain: SpawnTabDomain::DomainName(domain_name.to_string()),
                    ..SpawnCommand::default()
                },
                spawn_where: SpawnWhere::NewTab,
            }
        } else {
            Entry::Attach {
                label: format!("Attach {}", domain_name),
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
                spawn_where,
                ..
            } => {
                promise::spawn::spawn_into_main_thread(async move {
                    TermWindow::spawn_command_impl(
                        &command,
                        spawn_where,
                        size,
                        mux_window_id,
                        clipboard,
                    );
                })
                .detach();
            }
            Entry::Attach { domain, .. } => {
                promise::spawn::spawn_into_main_thread(async move {
                    // We can't inline do_domain_attach here directly
                    // because the compiler would then want its body
                    // to be Send :-/
                    do_domain_attach(domain);
                })
                .detach();
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
                    }
                }
                if mouse_buttons != MouseButtons::NONE {
                    // Treat any other mouse button as cancel
                    break;
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
    })
    .detach();
}
