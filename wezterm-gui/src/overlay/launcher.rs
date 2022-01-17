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
use std::collections::BTreeMap;
use termwiz::cell::{AttributeChange, CellAttributes};
use termwiz::color::ColorAttribute;
use termwiz::input::{InputEvent, KeyCode, KeyEvent, MouseButtons, MouseEvent};
use termwiz::surface::{Change, Position};
use termwiz::terminal::Terminal;
use window::WindowOps;

pub use config::keyassignment::LauncherFlags;

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

pub struct LauncherTabEntry {
    pub title: String,
    pub tab_id: TabId,
    pub tab_idx: usize,
    pub pane_count: usize,
}

#[derive(Debug)]
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
    active_workspace: String,
    workspaces: Vec<String>,
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

        let active_workspace = mux.active_workspace();

        let workspaces = if flags.contains(LauncherFlags::WORKSPACES) {
            mux.iter_workspaces()
        } else {
            vec![]
        };

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
            workspaces,
            active_workspace,
        }
    }
}

const ROW_OVERHEAD: usize = 3;

struct LauncherState {
    active_idx: usize,
    max_items: usize,
    top_row: usize,
    entries: Vec<Entry>,
    filter_term: String,
    filtered_entries: Vec<Entry>,
    pane_id: PaneId,
    window: ::window::Window,
    filtering: bool,
    flags: LauncherFlags,
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

        if args.flags.contains(LauncherFlags::WORKSPACES) {
            for ws in &args.workspaces {
                if *ws != args.active_workspace {
                    self.entries.push(Entry {
                        label: format!("Switch to workspace: `{}`", ws),
                        kind: EntryKind::KeyAssignment(KeyAssignment::SwitchToWorkspace {
                            name: Some(ws.clone()),
                            spawn: None,
                        }),
                    });
                }
            }
            self.entries.push(Entry {
                label: format!(
                    "Create new Workspace (current is `{}`)",
                    args.active_workspace
                ),
                kind: EntryKind::KeyAssignment(KeyAssignment::SwitchToWorkspace {
                    name: None,
                    spawn: None,
                }),
            });
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
            // Give a consistent order to the entries
            let keys: BTreeMap<_, _> = input_map.keys.into_iter().collect();
            for ((keycode, mods), assignment) in keys {
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

        let max_items = self.max_items;

        for (row_num, (entry_idx, entry)) in self
            .filtered_entries
            .iter()
            .enumerate()
            .skip(self.top_row)
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
        match self.filtered_entries[active_idx].clone().kind {
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
        if self.active_idx < self.top_row {
            self.top_row = self.active_idx;
        }
    }

    fn move_down(&mut self) {
        self.active_idx = (self.active_idx + 1).min(self.filtered_entries.len() - 1);
        if self.active_idx + self.top_row > self.max_items {
            self.top_row = self.active_idx.saturating_sub(self.max_items);
        }
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
                    if self.filter_term.pop().is_none()
                        && !self.flags.contains(LauncherFlags::FUZZY)
                    {
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
                }) if mouse_buttons.contains(MouseButtons::VERT_WHEEL) => {
                    if mouse_buttons.contains(MouseButtons::WHEEL_POSITIVE) {
                        self.top_row = self.top_row.saturating_sub(1);
                    } else {
                        self.top_row += 1;
                        self.top_row = self.top_row.min(
                            self.filtered_entries
                                .len()
                                .saturating_sub(self.max_items)
                                .saturating_sub(1),
                        );
                    }
                    if y > 0 && y as usize <= self.filtered_entries.len() {
                        self.active_idx = self.top_row + y as usize - 1;
                    }
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
                InputEvent::Resized { rows, .. } => {
                    self.max_items = rows.saturating_sub(ROW_OVERHEAD);
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
    let size = term.get_screen_size()?;
    let max_items = size.rows.saturating_sub(ROW_OVERHEAD);
    let mut state = LauncherState {
        active_idx: 0,
        max_items,
        pane_id: args.pane_id,
        top_row: 0,
        entries: vec![],
        filter_term: String::new(),
        filtered_entries: vec![],
        window,
        filtering: args.flags.contains(LauncherFlags::FUZZY),
        flags: args.flags,
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
