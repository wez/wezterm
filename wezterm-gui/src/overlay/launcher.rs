//! The launcher is a menu that presents a list of activities that can
//! be launched, such as spawning a new tab in various domains or attaching
//! ssh/tls domains.
//! The launcher is implemented here as an overlay, but could potentially
//! be rendered as a popup/context menu if the system supports it; at the
//! time of writing our window layer doesn't provide an API for context
//! menus.
use crate::termwindow::TermWindowNotif;
use anyhow::anyhow;
use config::configuration;
use config::keyassignment::{InputMap, KeyAssignment, SpawnCommand, SpawnTabDomain};
use config::lua::truncate_right;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use mux::domain::{DomainId, DomainState};
use mux::pane::PaneId;
use mux::tab::TabId;
use mux::termwiztermtab::TermWizTerminal;
use mux::window::WindowId;
use mux::Mux;
use std::collections::HashMap;
use termwiz::cell::{AttributeChange, CellAttributes};
use termwiz::color::ColorAttribute;
use termwiz::input::{InputEvent, KeyCode, KeyEvent, MouseButtons, MouseEvent};
use termwiz::surface::{Change, Position};
use termwiz::terminal::Terminal;
use window::WindowOps;

bitflags::bitflags! {
    pub struct LauncherFlags :u32 {
        const ZERO = 0;
        const WSL_DISTROS = 1;
        const TABS = 2;
        const LAUNCH_MENU_ITEMS = 4;
        const DOMAINS = 8;
        const KEY_ASSIGNMENTS = 16;
    }
}

#[derive(Clone)]
enum EntryKind {
    Attach { domain: DomainId },
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

#[allow(dead_code)]
fn enumerate_wsl_entries(entries: &mut Vec<Entry>) -> anyhow::Result<()> {
    #[cfg(windows)]
    use std::os::windows::process::CommandExt;
    let mut cmd = std::process::Command::new("wsl.exe");
    cmd.arg("-l");
    cmd.arg("-v");
    #[cfg(windows)]
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
        entries.push(Entry {
            label: label.clone(),
            kind: EntryKind::KeyAssignment(KeyAssignment::SpawnCommandInNewTab(SpawnCommand {
                args: Some(vec![
                    "wsl.exe".to_owned(),
                    "--distribution".to_owned(),
                    distro.name,
                ]),
                ..Default::default()
            })),
        });
    }

    Ok(())
}

pub struct LauncherTabEntry {
    pub title: String,
    pub tab_id: TabId,
    pub tab_idx: usize,
    pub pane_count: usize,
}

pub struct LauncherDomainEntry {
    pub domain_id: DomainId,
    pub name: String,
    pub state: DomainState,
    pub label: String,
}

pub struct LauncherArgs {
    flags: LauncherFlags,
    domains: Vec<LauncherDomainEntry>,
    tabs: Vec<LauncherTabEntry>,
    pane_id: PaneId,
    domain_id_of_current_tab: DomainId,
    title: String,
}

impl LauncherArgs {
    /// Must be called on the Mux thread!
    pub fn new(
        title: &str,
        flags: LauncherFlags,
        mux_window_id: WindowId,
        pane_id: PaneId,
        domain_id_of_current_tab: DomainId,
    ) -> Self {
        let mux = Mux::get().unwrap();

        let tabs = if flags.contains(LauncherFlags::TABS) {
            // Ideally we'd resolve the tabs on the fly once we've started the
            // overlay, but since the overlay runs in a different thread, accessing
            // the mux list is a bit awkward.  To get the ball rolling we capture
            // the list of tabs up front and live with a static list.
            let window = mux
                .get_window(mux_window_id)
                .expect("to resolve my own window_id");
            window
                .iter()
                .enumerate()
                .map(|(tab_idx, tab)| LauncherTabEntry {
                    title: tab
                        .get_active_pane()
                        .expect("tab to have a pane")
                        .get_title(),
                    tab_id: tab.tab_id(),
                    tab_idx,
                    pane_count: tab.count_panes(),
                })
                .collect()
        } else {
            vec![]
        };

        let domains = if flags.contains(LauncherFlags::DOMAINS) {
            let mut domains = mux.iter_domains();
            domains.sort_by(|a, b| {
                let a_state = a.state();
                let b_state = b.state();
                if a_state != b_state {
                    use std::cmp::Ordering;
                    return if a_state == DomainState::Attached {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    };
                }
                a.domain_id().cmp(&b.domain_id())
            });
            domains.retain(|dom| dom.spawnable());
            domains
                .iter()
                .map(|dom| {
                    let name = dom.domain_name();
                    let label = dom.domain_label();
                    let label = if name == label || label == "" {
                        format!("domain `{}`", name)
                    } else {
                        format!("domain `{}` - {}", name, label)
                    };
                    LauncherDomainEntry {
                        domain_id: dom.domain_id(),
                        name: name.to_string(),
                        state: dom.state(),
                        label,
                    }
                })
                .collect()
        } else {
            vec![]
        };

        Self {
            flags,
            domains,
            tabs,
            pane_id,
            domain_id_of_current_tab,
            title: title.to_string(),
        }
    }
}

struct LauncherState {
    active_idx: usize,
    top_row: usize,
    entries: Vec<Entry>,
    filter_term: String,
    filtered_entries: Vec<Entry>,
    pane_id: PaneId,
    window: ::window::Window,
    filtering: bool,
}

impl LauncherState {
    fn update_filter(&mut self) {
        if self.filter_term.is_empty() {
            self.filtered_entries = self.entries.clone();
            return;
        }

        self.filtered_entries.clear();

        let matcher = SkimMatcherV2::default();

        struct MatchResult {
            row_idx: usize,
            score: i64,
        }

        let mut scores: Vec<MatchResult> = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(row_idx, entry)| {
                let score = matcher.fuzzy_match(&entry.label, &self.filter_term)?;
                Some(MatchResult { row_idx, score })
            })
            .collect();

        scores.sort_by(|a, b| a.score.cmp(&b.score).reverse());

        for result in scores {
            self.filtered_entries
                .push(self.entries[result.row_idx].clone());
        }

        self.active_idx = 0;
        self.top_row = 0;
    }

    fn build_entries(&mut self, args: LauncherArgs) {
        let config = configuration();
        // Pull in the user defined entries from the launch_menu
        // section of the configuration.
        if args.flags.contains(LauncherFlags::LAUNCH_MENU_ITEMS) {
            for item in &config.launch_menu {
                self.entries.push(Entry {
                    label: match item.label.as_ref() {
                        Some(label) => label.to_string(),
                        None => match item.args.as_ref() {
                            Some(args) => args.join(" "),
                            None => "(default shell)".to_string(),
                        },
                    },
                    kind: EntryKind::KeyAssignment(KeyAssignment::SpawnCommandInNewTab(
                        item.clone(),
                    )),
                });
            }
        }

        #[cfg(windows)]
        if args.flags.contains(LauncherFlags::WSL_DISTROS) {
            let _ = enumerate_wsl_entries(&mut self.entries);
        }

        for domain in &args.domains {
            let entry = if domain.state == DomainState::Attached {
                Entry {
                    label: format!("New Tab ({})", domain.label),
                    kind: EntryKind::KeyAssignment(KeyAssignment::SpawnCommandInNewTab(
                        SpawnCommand {
                            domain: SpawnTabDomain::DomainName(domain.name.to_string()),
                            ..SpawnCommand::default()
                        },
                    )),
                }
            } else {
                Entry {
                    label: format!("Attach {}", domain.label),
                    kind: EntryKind::Attach {
                        domain: domain.domain_id,
                    },
                }
            };

            // Preselect the entry that corresponds to the active tab
            // at the time that the launcher was set up, so that pressing
            // Enter immediately afterwards spawns a tab in the same domain.
            if domain.domain_id == args.domain_id_of_current_tab {
                self.active_idx = self.entries.len();
            }
            self.entries.push(entry);
        }

        for tab in &args.tabs {
            self.entries.push(Entry {
                label: format!("{}. {} panes", tab.title, tab.pane_count),
                kind: EntryKind::KeyAssignment(KeyAssignment::ActivateTab(tab.tab_idx as isize)),
            });
        }

        // Grab interestig key assignments and show those as a kind of command palette
        if args.flags.contains(LauncherFlags::KEY_ASSIGNMENTS) {
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
            self.entries.append(&mut key_entries);
        }
    }

    fn render(&mut self, term: &mut TermWizTerminal) -> termwiz::Result<()> {
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
                    "Select an item and press Enter=launch  \
                     Esc=cancel  /=filter",
                    max_width
                )
            )),
            Change::AllAttributes(CellAttributes::default()),
        ];

        let max_items = size.rows - 3;
        let num_items = self.filtered_entries.len();

        let skip = if num_items < max_items {
            0
        } else if num_items - self.active_idx < max_items {
            // Align to bottom
            (num_items - max_items).saturating_sub(1)
        } else {
            self.active_idx.saturating_sub(2)
        };

        for (row_num, (entry_idx, entry)) in self
            .filtered_entries
            .iter()
            .enumerate()
            .skip(skip)
            .enumerate()
        {
            if row_num > max_items {
                break;
            }
            if entry_idx == self.active_idx {
                changes.push(AttributeChange::Reverse(true).into());
            }

            let label = truncate_right(&entry.label, max_width);
            if row_num < 9 && !self.filtering {
                changes.push(Change::Text(format!(" {}. {} \r\n", row_num + 1, label)));
            } else {
                changes.push(Change::Text(format!("    {} \r\n", label)));
            }

            if entry_idx == self.active_idx {
                changes.push(AttributeChange::Reverse(false).into());
            }
        }
        self.top_row = skip;

        if self.filtering || !self.filter_term.is_empty() {
            changes.append(&mut vec![
                Change::CursorPosition {
                    x: Position::Absolute(0),
                    y: Position::Absolute(0),
                },
                Change::ClearToEndOfLine(ColorAttribute::Default),
                Change::Text(truncate_right(
                    &format!("Fuzzy matching: {}", self.filter_term),
                    max_width,
                )),
            ]);
        }

        term.render(&changes)
    }

    fn launch(&self, active_idx: usize) {
        match self.entries[active_idx].clone().kind {
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
                self.window.notify(TermWindowNotif::PerformAssignment {
                    pane_id: self.pane_id,
                    assignment,
                });
            }
        }
    }

    fn move_up(&mut self) {
        self.active_idx = self.active_idx.saturating_sub(1);
    }

    fn move_down(&mut self) {
        self.active_idx = (self.active_idx + 1).min(self.filtered_entries.len() - 1);
    }

    fn run_loop(&mut self, term: &mut TermWizTerminal) -> anyhow::Result<()> {
        while let Ok(Some(event)) = term.poll_input(None) {
            match event {
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char(c),
                    ..
                }) if !self.filtering && c >= '1' && c <= '9' => {
                    self.launch(self.top_row + (c as u32 - '1' as u32) as usize);
                    break;
                }
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('j'),
                    ..
                }) if !self.filtering => {
                    self.move_down();
                }
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('k'),
                    ..
                }) if !self.filtering => {
                    self.move_up();
                }
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('/'),
                    ..
                }) if !self.filtering => {
                    self.filtering = true;
                }
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Backspace,
                    ..
                }) => {
                    if self.filter_term.pop().is_none() {
                        self.filtering = false;
                    }
                    self.update_filter();
                }
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char(c),
                    ..
                }) if self.filtering => {
                    self.filter_term.push(c);
                    self.update_filter();
                }
                InputEvent::Key(KeyEvent {
                    key: KeyCode::UpArrow,
                    ..
                }) => {
                    self.move_up();
                }
                InputEvent::Key(KeyEvent {
                    key: KeyCode::DownArrow,
                    ..
                }) => {
                    self.move_down();
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
                    if y > 0 && y as usize <= self.filtered_entries.len() {
                        self.active_idx = self.top_row + y as usize - 1;

                        if mouse_buttons == MouseButtons::LEFT {
                            self.launch(self.active_idx);
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
                    self.launch(self.active_idx);
                    break;
                }
                _ => {}
            }
            self.render(term)?;
        }

        Ok(())
    }
}

pub fn launcher(
    args: LauncherArgs,
    mut term: TermWizTerminal,
    window: ::window::Window,
) -> anyhow::Result<()> {
    let mut state = LauncherState {
        active_idx: 0,
        pane_id: args.pane_id,
        top_row: 0,
        entries: vec![],
        filter_term: String::new(),
        filtered_entries: vec![],
        window,
        filtering: false,
    };

    term.set_raw_mode()?;
    term.render(&[Change::Title(args.title.to_string())])?;
    state.build_entries(args);
    state.update_filter();
    state.render(&mut term)?;
    state.run_loop(&mut term)
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
