//! The launcher is a menu that presents a list of activities that can
//! be launched, such as spawning a new tab in various domains or attaching
//! ssh/tls domains.
//! The launcher is implemented here as an overlay, but could potentially
//! be rendered as a popup/context menu if the system supports it; at the
//! time of writing our window layer doesn't provide an API for context
//! menus.
use super::quickselect;
use crate::commands::derive_command_from_key_assignment;
use crate::inputmap::InputMap;
use crate::overlay::selector::{matcher_pattern, matcher_score};
use crate::termwindow::TermWindowNotif;
use config::configuration;
use config::keyassignment::{KeyAssignment, SpawnCommand, SpawnTabDomain};
use mux::domain::{DomainId, DomainState};
use mux::pane::PaneId;
use mux::termwiztermtab::TermWizTerminal;
use mux::window::WindowId;
use mux::Mux;
use rayon::prelude::*;
use std::collections::BTreeMap;
use termwiz::cell::{AttributeChange, CellAttributes};
use termwiz::color::ColorAttribute;
use termwiz::input::{InputEvent, KeyCode, KeyEvent, Modifiers, MouseButtons, MouseEvent};
use termwiz::surface::{Change, Position};
use termwiz::terminal::Terminal;
use termwiz_funcs::truncate_right;
use window::WindowOps;

pub use config::keyassignment::LauncherFlags;

#[derive(Clone)]
struct Entry {
    pub label: String,
    pub action: KeyAssignment,
}

pub struct LauncherTabEntry {
    pub title: String,
    pub tab_idx: usize,
    pub pane_count: Option<usize>,
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
    help_text: String,
    fuzzy_help_text: String,
    alphabet: String,
}

impl LauncherArgs {
    /// Must be called on the Mux thread!
    pub async fn new(
        title: &str,
        flags: LauncherFlags,
        mux_window_id: WindowId,
        pane_id: PaneId,
        domain_id_of_current_tab: DomainId,
        help_text: &str,
        fuzzy_help_text: &str,
        alphabet: &str,
    ) -> Self {
        let mux = Mux::get();

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
                .map(|(tab_idx, tab)| {
                    let tab_title = tab.get_title();
                    let title = if tab_title.is_empty() {
                        tab.get_active_pane()
                            .expect("tab to have a pane")
                            .get_title()
                    } else {
                        tab_title
                    };
                    LauncherTabEntry {
                        title,
                        tab_idx,
                        pane_count: tab.count_panes(),
                    }
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
            let mut d = vec![];
            for dom in domains.into_iter() {
                let name = dom.domain_name();
                let label = dom.domain_label().await;
                let label = if name == label || label == "" {
                    format!("domain `{}`", name)
                } else {
                    format!("domain `{}` - {}", name, label)
                };
                d.push(LauncherDomainEntry {
                    domain_id: dom.domain_id(),
                    name: name.to_string(),
                    state: dom.state(),
                    label,
                });
            }
            d
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
            help_text: help_text.to_string(),
            fuzzy_help_text: fuzzy_help_text.to_string(),
            alphabet: alphabet.to_string(),
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
    help_text: String,
    fuzzy_help_text: String,
    labels: Vec<String>,
    alphabet: String,
    selection: String,
    always_fuzzy: bool,
}

impl LauncherState {
    fn update_filter(&mut self) {
        if self.filter_term.is_empty() {
            self.filtered_entries = self.entries.clone();
            return;
        }

        self.filtered_entries.clear();

        let pattern = matcher_pattern(&self.filter_term);

        struct MatchResult {
            row_idx: usize,
            score: u32,
        }

        let mut scores: Vec<MatchResult> = self
            .entries
            .par_iter()
            .enumerate()
            .filter_map(|(row_idx, entry)| {
                let score = matcher_score(&pattern, &entry.label)?;
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
                    action: KeyAssignment::SpawnCommandInNewTab(item.clone()),
                });
            }
        }

        for domain in &args.domains {
            let entry = if domain.state == DomainState::Attached {
                Entry {
                    label: format!("New Tab ({})", domain.label),
                    action: KeyAssignment::SpawnCommandInNewTab(SpawnCommand {
                        domain: SpawnTabDomain::DomainName(domain.name.to_string()),
                        ..SpawnCommand::default()
                    }),
                }
            } else {
                Entry {
                    label: format!("Attach {}", domain.label),
                    action: KeyAssignment::AttachDomain(domain.name.to_string()),
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
                        action: KeyAssignment::SwitchToWorkspace {
                            name: Some(ws.clone()),
                            spawn: None,
                        },
                    });
                }
            }
            self.entries.push(Entry {
                label: format!(
                    "Create new Workspace (current is `{}`)",
                    args.active_workspace
                ),
                action: KeyAssignment::SwitchToWorkspace {
                    name: None,
                    spawn: None,
                },
            });
        }

        for tab in &args.tabs {
            self.entries.push(Entry {
                label: match tab.pane_count {
                    Some(pane_count) => format!("{}. {pane_count} panes", tab.title),
                    None => format!("{}.", tab.title),
                },
                action: KeyAssignment::ActivateTab(tab.tab_idx as isize),
            });
        }

        if args.flags.contains(LauncherFlags::COMMANDS) {
            let commands = crate::commands::CommandDef::expanded_commands(&config);
            for cmd in commands {
                if matches!(
                    &cmd.action,
                    KeyAssignment::ActivateTabRelative(_) | KeyAssignment::ActivateTab(_)
                ) {
                    // Filter out some noisy, repetitive entries
                    continue;
                }
                self.entries.push(Entry {
                    label: format!("{}. {}", cmd.brief, cmd.doc),
                    action: cmd.action,
                });
            }
        }

        // Grab interesting key assignments and show those as a kind of command palette
        if args.flags.contains(LauncherFlags::KEY_ASSIGNMENTS) {
            let input_map = InputMap::new(&config);
            let mut key_entries: Vec<Entry> = vec![];
            // Give a consistent order to the entries
            let keys: BTreeMap<_, _> = input_map.keys.default.into_iter().collect();
            for ((keycode, mods), entry) in keys {
                if matches!(
                    &entry.action,
                    KeyAssignment::ActivateTabRelative(_) | KeyAssignment::ActivateTab(_)
                ) {
                    // Filter out some noisy, repetitive entries
                    continue;
                }
                if key_entries
                    .iter()
                    .find(|ent| ent.action == entry.action)
                    .is_some()
                {
                    // Avoid duplicate entries
                    continue;
                }

                let label = match derive_command_from_key_assignment(&entry.action) {
                    Some(cmd) => format!("{}. {}", cmd.brief, cmd.doc),
                    None => format!(
                        "{:?} ({} {})",
                        entry.action,
                        mods.to_string(),
                        keycode.to_string().escape_debug()
                    ),
                };

                key_entries.push(Entry {
                    label,
                    action: entry.action,
                });
            }
            key_entries.sort_by(|a, b| a.label.cmp(&b.label));
            self.entries.append(&mut key_entries);
        }
    }

    fn render(&mut self, term: &mut TermWizTerminal) -> termwiz::Result<()> {
        let size = term.get_screen_size()?;
        let max_width = size.cols.saturating_sub(6);
        let max_items = size.rows.saturating_sub(ROW_OVERHEAD);
        if max_items != self.max_items {
            self.labels = quickselect::compute_labels_for_alphabet_with_preserved_case(
                &self.alphabet,
                self.filtered_entries.len().min(max_items + 1),
            );
            self.max_items = max_items;
        }

        let mut changes = vec![
            Change::ClearScreen(ColorAttribute::Default),
            Change::CursorPosition {
                x: Position::Absolute(0),
                y: Position::Absolute(0),
            },
            Change::Text(format!(
                "{}\r\n",
                truncate_right(&self.help_text, max_width)
            )),
            Change::AllAttributes(CellAttributes::default()),
        ];

        let labels = &self.labels;
        let max_label_len = labels.iter().map(|s| s.len()).max().unwrap_or(0);
        let mut labels_iter = labels.into_iter();

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

            let mut attr = CellAttributes::blank();

            if entry_idx == self.active_idx {
                changes.push(AttributeChange::Reverse(true).into());
                attr.set_reverse(true);
            }

            // from above we know that row_num <= max_items
            // show labels as long as we have more labels left
            // and we are not filtering
            if !self.filtering {
                if let Some(label) = labels_iter.next() {
                    changes.push(Change::Text(format!(" {label:>max_label_len$}. ")));
                } else {
                    changes.push(Change::Text(" ".repeat(max_label_len + 3)));
                }
            } else if !self.always_fuzzy {
                changes.push(Change::Text(" ".repeat(max_label_len + 3)));
            } else {
                changes.push(Change::Text("    ".to_string()));
            }

            let mut line = crate::tabbar::parse_status_text(&entry.label, attr.clone());
            if line.len() > max_width {
                line.resize(max_width, termwiz::surface::SEQ_ZERO);
            }
            changes.append(&mut line.changes(&attr));
            changes.push(Change::Text(" ".to_string()));

            if entry_idx == self.active_idx {
                changes.push(AttributeChange::Reverse(false).into());
            }
            changes.push(Change::AllAttributes(CellAttributes::default()));
            changes.push(Change::Text("\r\n".to_string()));
        }

        if self.filtering || !self.filter_term.is_empty() {
            changes.append(&mut vec![
                Change::CursorPosition {
                    x: Position::Absolute(0),
                    y: Position::Absolute(0),
                },
                Change::ClearToEndOfLine(ColorAttribute::Default),
                Change::Text(truncate_right(
                    &format!("{}{}", &self.fuzzy_help_text, self.filter_term),
                    max_width,
                )),
            ]);
        }

        term.render(&changes)
    }

    fn launch(&self, active_idx: usize) -> bool {
        if let Some(entry) = self.filtered_entries.get(active_idx) {
            let assignment = entry.action.clone();
            self.window.notify(TermWindowNotif::PerformAssignment {
                pane_id: self.pane_id,
                assignment,
                tx: None,
            });
            true
        } else {
            false
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
        if self.active_idx > self.top_row + self.max_items {
            self.top_row = self.active_idx.saturating_sub(self.max_items);
        }
    }

    fn run_loop(&mut self, term: &mut TermWizTerminal) -> anyhow::Result<()> {
        while let Ok(Some(event)) = term.poll_input(None) {
            match event {
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char(c),
                    modifiers: Modifiers::NONE,
                }) if !self.filtering && self.alphabet.contains(c) => {
                    self.selection.push(c);
                    if let Some(pos) = self.labels.iter().position(|x| *x == self.selection) {
                        // since the number of labels is always <= self.max_items
                        // by construction, we have pos as usize <= self.max_items
                        // for free
                        self.active_idx = self.top_row + pos as usize;
                        if self.launch(self.active_idx) {
                            break;
                        }
                    }
                }
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('j'),
                    ..
                }) if !self.filtering && !self.alphabet.contains("j") => {
                    self.move_down();
                }
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('k'),
                    ..
                }) if !self.filtering && !self.alphabet.contains("k") => {
                    self.move_up();
                }
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('P' | 'K'),
                    modifiers: Modifiers::CTRL,
                }) => {
                    self.move_up();
                }
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('N' | 'J'),
                    modifiers: Modifiers::CTRL,
                }) => {
                    self.move_down();
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
                    if !self.filtering {
                        self.selection.pop();
                    } else {
                        if self.filter_term.pop().is_none() && !self.always_fuzzy {
                            self.filtering = false;
                        }
                        self.update_filter();
                    }
                }
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('G') | KeyCode::Char('['),
                    modifiers: Modifiers::CTRL,
                })
                | InputEvent::Key(KeyEvent {
                    key: KeyCode::Escape,
                    ..
                }) => {
                    break;
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
                            if self.launch(self.active_idx) {
                                break;
                            }
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
                    if self.launch(self.active_idx) {
                        break;
                    }
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
    initial_choice_idx: usize,
) -> anyhow::Result<()> {
    let filtering = args.flags.contains(LauncherFlags::FUZZY);
    let mut state = LauncherState {
        active_idx: initial_choice_idx,
        max_items: 0,
        pane_id: args.pane_id,
        top_row: 0,
        entries: vec![],
        filter_term: String::new(),
        filtered_entries: vec![],
        window,
        filtering,
        help_text: args.help_text.clone(),
        fuzzy_help_text: args.fuzzy_help_text.clone(),
        labels: vec![],
        selection: String::new(),
        alphabet: args.alphabet.clone(),
        always_fuzzy: filtering,
    };

    term.set_raw_mode()?;
    term.render(&[Change::Title(args.title.to_string())])?;
    state.build_entries(args);
    state.update_filter();
    state.render(&mut term)?;
    state.run_loop(&mut term)
}
