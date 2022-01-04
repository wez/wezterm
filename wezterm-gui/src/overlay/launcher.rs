//! The launcher is a menu that presents a list of activities that can
//! be launched, such as spawning a new tab in various domains or attaching
//! ssh/tls domains.
//! The launcher is implemented here as an overlay, but could potentially
//! be rendered as a popup/context menu if the system supports it; at the
//! time of writing our window layer doesn't provide an API for context
//! menus.
use crate::termwindow::clipboard::ClipboardHelper;
use crate::termwindow::spawn::SpawnWhere;
use crate::termwindow::{TermWindow, TermWindowNotif};
use anyhow::anyhow;
use config::keyassignment::{InputMap, KeyAssignment, SpawnCommand, SpawnTabDomain};
use config::lua::truncate_right;
use config::{configuration, TermConfig};
use mux::domain::{DomainId, DomainState};
use mux::pane::PaneId;
use mux::tab::TabId;
use mux::termwiztermtab::TermWizTerminal;
use mux::window::WindowId;
use mux::Mux;
use portable_pty::PtySize;
use std::collections::HashMap;
use std::sync::Arc;
use termwiz::cell::{AttributeChange, CellAttributes};
use termwiz::color::ColorAttribute;
use termwiz::input::{InputEvent, KeyCode, KeyEvent, MouseButtons, MouseEvent};
use termwiz::surface::{Change, Position};
use termwiz::terminal::Terminal;
use window::WindowOps;

#[derive(Clone)]
enum EntryKind {
    Spawn {
        command: SpawnCommand,
        spawn_where: SpawnWhere,
    },
    Attach {
        domain: DomainId,
    },
    KeyAssignment(KeyAssignment),
}

#[derive(Clone)]
struct Entry {
    pub label: String,
    pub kind: EntryKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WslDistro {
    name: String,
    state: String,
    version: String,
    is_default: bool,
}

/// This function parses the `wsl -l -v` output.
/// It tries to be robust in the face of future changes
/// by looking at the tabulated output headers, determining
/// where the columns are and then collecting the information
/// into a hashmap and then grokking from there.
#[allow(dead_code)]
fn parse_wsl_distro_list(output: &str) -> Vec<WslDistro> {
    let lines = output.lines().collect::<Vec<_>>();

    // Determine where the field columns start
    let mut field_starts = vec![];
    {
        let mut last_char = ' ';
        for (idx, c) in lines[0].char_indices() {
            if last_char == ' ' && c != ' ' {
                field_starts.push(idx);
            }
            last_char = c;
        }
    }

    fn field_slice(s: &str, start: usize, end: Option<usize>) -> &str {
        if let Some(end) = end {
            &s[start..end]
        } else {
            &s[start..]
        }
    }

    fn opt_field_slice(s: &str, start: usize, end: Option<usize>) -> Option<&str> {
        if let Some(end) = end {
            s.get(start..end)
        } else {
            s.get(start..)
        }
    }

    // Now build up a name -> column position map
    let mut field_map = HashMap::new();
    {
        let mut iter = field_starts.into_iter().peekable();

        while let Some(start_idx) = iter.next() {
            let end_idx = iter.peek().copied();
            let label = field_slice(&lines[0], start_idx, end_idx).trim();
            field_map.insert(label, (start_idx, end_idx));
        }
    }

    let mut result = vec![];

    // and now process the output rows
    for line in lines.iter().skip(1) {
        if line.is_empty() {
            continue;
        }

        let is_default = line.starts_with("*");

        let mut fields = HashMap::new();
        for (label, (start_idx, end_idx)) in field_map.iter() {
            if let Some(value) = opt_field_slice(line, *start_idx, *end_idx) {
                fields.insert(*label, value.trim().to_string());
            } else {
                return result;
            }
        }

        result.push(WslDistro {
            name: fields.get("NAME").cloned().unwrap_or_default(),
            state: fields.get("STATE").cloned().unwrap_or_default(),
            version: fields.get("VERSION").cloned().unwrap_or_default(),
            is_default,
        });
    }

    result
}

#[cfg(test)]
#[test]
fn test_parse_wsl_distro_list() {
    let data = "  NAME                   STATE           VERSION
* Arch                   Running         2
  docker-desktop-data    Stopped         2
  docker-desktop         Stopped         2
  Ubuntu                 Stopped         2
  nvim                   Stopped         2";

    assert_eq!(
        parse_wsl_distro_list(data),
        vec![
            WslDistro {
                name: "Arch".to_string(),
                state: "Running".to_string(),
                version: "2".to_string(),
                is_default: true
            },
            WslDistro {
                name: "docker-desktop-data".to_string(),
                state: "Stopped".to_string(),
                version: "2".to_string(),
                is_default: false
            },
            WslDistro {
                name: "docker-desktop".to_string(),
                state: "Stopped".to_string(),
                version: "2".to_string(),
                is_default: false
            },
            WslDistro {
                name: "Ubuntu".to_string(),
                state: "Stopped".to_string(),
                version: "2".to_string(),
                is_default: false
            },
            WslDistro {
                name: "nvim".to_string(),
                state: "Stopped".to_string(),
                version: "2".to_string(),
                is_default: false
            },
        ]
    );
}

#[cfg(windows)]
fn enumerate_wsl_entries(entries: &mut Vec<Entry>) -> anyhow::Result<()> {
    use std::os::windows::process::CommandExt;
    let mut cmd = std::process::Command::new("wsl.exe");
    cmd.arg("-l");
    cmd.arg("-v");
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

        String::from_utf16(wide).map_err(|_| anyhow!("wsl -l -v output is not valid utf16"))
    }

    let wsl_list = utf16_to_utf8(&output.stdout)?.replace("\r\n", "\n");

    for distro in parse_wsl_distro_list(&wsl_list) {
        let label = format!("{} (WSL)", distro.name);
        entries.push(Entry::Spawn {
            label: label.clone(),
            command: SpawnCommand {
                label: Some(label),
                args: Some(vec![
                    "wsl.exe".to_owned(),
                    "--distribution".to_owned(),
                    distro.name,
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
    pane_id: PaneId,
    domain_id_of_current_tab: DomainId,
    mut term: TermWizTerminal,
    mux_window_id: WindowId,
    domains: Vec<(DomainId, String, DomainState, String)>,
    clipboard: ClipboardHelper,
    size: PtySize,
    term_config: Arc<TermConfig>,
    window: ::window::Window,
) -> anyhow::Result<()> {
    let mut active_idx = 0;
    let mut top_row;
    let mut entries = vec![];

    term.set_raw_mode()?;

    let config = configuration();

    // Pull in the user defined entries from the launch_menu
    // section of the configuration.
    for item in &config.launch_menu {
        entries.push(Entry {
            label: match item.label.as_ref() {
                Some(label) => label.to_string(),
                None => match item.args.as_ref() {
                    Some(args) => args.join(" "),
                    None => "(default shell)".to_string(),
                },
            },
            kind: EntryKind::Spawn {
                command: item.clone(),
                spawn_where: SpawnWhere::NewTab,
            },
        });
    }

    #[cfg(windows)]
    {
        if config.add_wsl_distributions_to_launch_menu {
            let _ = enumerate_wsl_entries(&mut entries);
        }
    }

    for (domain_id, domain_name, domain_state, domain_label) in &domains {
        let entry = if *domain_state == DomainState::Attached {
            Entry {
                label: format!("New Tab ({})", domain_label),
                kind: EntryKind::Spawn {
                    command: SpawnCommand {
                        domain: SpawnTabDomain::DomainName(domain_name.to_string()),
                        ..SpawnCommand::default()
                    },
                    spawn_where: SpawnWhere::NewTab,
                },
            }
        } else {
            Entry {
                label: format!("Attach {}", domain_label),
                kind: EntryKind::Attach { domain: *domain_id },
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

    // Grab interestig key assignments and show those as a kind of command palette
    let input_map = InputMap::new(&config);
    let mut key_entries: Vec<Entry> = vec![];
    for ((keycode, mods), assignment) in input_map.keys {
        if matches!(
            &assignment,
            KeyAssignment::ActivateTabRelative(_) | KeyAssignment::ActivateTab(_)
        ) {
            // Filter out some noisy, repetitive entries
            continue;
        }
        if key_entries
            .iter()
            .find(|ent| match &ent.kind {
                EntryKind::KeyAssignment(a) => a == &assignment,
                _ => false,
            })
            .is_some()
        {
            // Avoid duplicate entries
            continue;
        }
        key_entries.push(Entry {
            label: format!(
                "{:?} ({} {})",
                assignment,
                mods.to_string(),
                keycode.to_string()
            ),
            kind: EntryKind::KeyAssignment(assignment),
        });
    }
    key_entries.sort_by(|a, b| a.label.cmp(&b.label));
    entries.append(&mut key_entries);

    fn render(
        active_idx: usize,
        entries: &[Entry],
        term: &mut TermWizTerminal,
    ) -> termwiz::Result<usize> {
        let size = term.get_screen_size()?;
        let max_width = size.cols.saturating_sub(6);

        let mut changes = vec![
            Change::ClearScreen(ColorAttribute::Default),
            Change::CursorPosition {
                x: Position::Absolute(0),
                y: Position::Absolute(0),
            },
            Change::Text(format!(
                "{}\r\n",
                truncate_right(
                    "Select an item and press Enter to launch it.  \
                     Press Escape to cancel",
                    max_width
                )
            )),
            Change::AllAttributes(CellAttributes::default()),
        ];

        let max_items = size.rows - 3;
        let num_items = entries.len();

        let skip = if num_items - active_idx < max_items {
            // Align to bottom
            (num_items - max_items) - 1
        } else {
            active_idx.saturating_sub(2)
        };

        for (row_num, (entry_idx, entry)) in entries.iter().enumerate().skip(skip).enumerate() {
            if row_num > max_items {
                break;
            }
            if entry_idx == active_idx {
                changes.push(AttributeChange::Reverse(true).into());
            }

            let label = truncate_right(&entry.label, max_width);
            if row_num < 9 {
                changes.push(Change::Text(format!(" {}. {} \r\n", row_num + 1, label)));
            } else {
                changes.push(Change::Text(format!("    {} \r\n", label)));
            }

            if entry_idx == active_idx {
                changes.push(AttributeChange::Reverse(false).into());
            }
        }
        term.render(&changes)?;
        Ok(skip)
    }

    term.render(&[Change::Title("Launcher".to_string())])?;
    top_row = render(active_idx, &entries, &mut term)?;

    fn launch(
        active_idx: usize,
        entries: &[Entry],
        size: PtySize,
        mux_window_id: WindowId,
        clipboard: ClipboardHelper,
        term_config: Arc<TermConfig>,
        window: &::window::Window,
        pane_id: PaneId,
    ) {
        match entries[active_idx].clone().kind {
            EntryKind::Spawn {
                command,
                spawn_where,
            } => {
                promise::spawn::spawn_into_main_thread(async move {
                    TermWindow::spawn_command_impl(
                        &command,
                        spawn_where,
                        size,
                        mux_window_id,
                        clipboard,
                        term_config,
                    );
                })
                .detach();
            }
            EntryKind::Attach { domain } => {
                promise::spawn::spawn_into_main_thread(async move {
                    // We can't inline do_domain_attach here directly
                    // because the compiler would then want its body
                    // to be Send :-/
                    do_domain_attach(domain);
                })
                .detach();
            }
            EntryKind::KeyAssignment(assignment) => {
                window.notify(TermWindowNotif::PerformAssignment {
                    pane_id,
                    assignment,
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
                    active_idx = top_row + y as usize - 1;

                    if mouse_buttons == MouseButtons::LEFT {
                        launch(
                            active_idx,
                            &entries,
                            size,
                            mux_window_id,
                            clipboard,
                            term_config,
                            &window,
                            pane_id,
                        );
                        break;
                    }
                }
                if mouse_buttons != MouseButtons::NONE {
                    // Treat any other mouse button as cancel
                    break;
                }
            }
            InputEvent::Key(KeyEvent {
                key: KeyCode::Char(c),
                ..
            }) if c >= '1' && c <= '9' => {
                launch(
                    top_row + (c as u32 - '1' as u32) as usize,
                    &entries,
                    size,
                    mux_window_id,
                    clipboard,
                    term_config,
                    &window,
                    pane_id,
                );
                break;
            }
            InputEvent::Key(KeyEvent {
                key: KeyCode::Enter,
                ..
            }) => {
                launch(
                    active_idx,
                    &entries,
                    size,
                    mux_window_id,
                    clipboard,
                    term_config,
                    &window,
                    pane_id,
                );
                break;
            }
            _ => {}
        }
        top_row = render(active_idx, &entries, &mut term)?;
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
