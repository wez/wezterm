use super::quickselect::compute_labels_for_alphabet;
use crate::scripting::guiwin::GuiWin;
use config::keyassignment::{InputSelector, InputSelectorEntry, KeyAssignment};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use mux::termwiztermtab::TermWizTerminal;
use mux_lua::MuxPane;
use std::rc::Rc;
use termwiz::cell::{AttributeChange, CellAttributes};
use termwiz::color::ColorAttribute;
use termwiz::input::{InputEvent, KeyCode, KeyEvent, Modifiers, MouseButtons, MouseEvent};
use termwiz::surface::{Change, Position};
use termwiz::terminal::Terminal;
use termwiz_funcs::truncate_right;

const ROW_OVERHEAD: usize = 3;

struct SelectorState {
    active_idx: usize,
    max_items: usize,
    top_row: usize,
    filter_term: String,
    filtered_entries: Vec<InputSelectorEntry>,
    pane: MuxPane,
    window: GuiWin,
    filtering: bool,
    always_fuzzy: bool,
    args: InputSelector,
    event_name: String,
    selection: String,
}

impl SelectorState {
    fn update_filter(&mut self) {
        if self.filter_term.is_empty() {
            self.filtered_entries = self.args.choices.clone();
            return;
        }

        self.filtered_entries.clear();

        let matcher = SkimMatcherV2::default();

        struct MatchResult {
            row_idx: usize,
            score: i64,
        }

        let mut scores: Vec<MatchResult> = self
            .args
            .choices
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
                .push(self.args.choices[result.row_idx].clone());
        }

        self.active_idx = 0;
        self.top_row = 0;
    }

    fn render(&mut self, term: &mut TermWizTerminal) -> termwiz::Result<()> {
        let size = term.get_screen_size()?;
        let max_width = size.cols.saturating_sub(6);
        let desc = &self.args.description;

        let mut changes = vec![
            Change::ClearScreen(ColorAttribute::Default),
            Change::CursorPosition {
                x: Position::Absolute(0),
                y: Position::Absolute(0),
            },
            Change::Text(format!("{}\r\n", truncate_right(desc, max_width))),
            Change::AllAttributes(CellAttributes::default()),
        ];

        let max_items = self.max_items;
        let alphabet = &self.args.alphabet;
        let labels = compute_labels_for_alphabet(alphabet, max_items + 1);
        let num_labels = labels.len();
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
            if !self.filtering && row_num < labels_len {
                if let Some(s) = labels.next() {
                    let ex_spaces = " ".to_string().repeat(max_label_len - s.len() + 1);
                    changes.push(Change::Text(format!("{}{}. ", ex_spaces, s)));
                }
            } else if !self.filtering {
                changes.push(Change::Text(format!(
                    "{}",
                    " ".to_string().repeat(max_label_len + 3)
                )));
            } else {
                changes.push(Change::Text("    ".to_string()));
            }

            let mut line = crate::tabbar::parse_status_text(&entry.label, attr.clone());
            if line.len() > max_width {
                line.resize(max_width, termwiz::surface::SEQ_ZERO);
            }
            changes.append(&mut line.changes(&attr));
            if entry_idx == self.active_idx {
                changes.push(AttributeChange::Reverse(false).into());
            }
            changes.push(Change::AllAttributes(CellAttributes::default()));
            changes.push(Change::Text(" \r\n".to_string()));
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

    fn trigger_event(&self, entry: Option<InputSelectorEntry>) {
        let name = self.event_name.clone();
        let window = self.window.clone();
        let pane = self.pane.clone();

        promise::spawn::spawn_into_main_thread(async move {
            trampoline(name, window, pane, entry);
            anyhow::Result::<()>::Ok(())
        })
        .detach();
    }

    fn launch(&self, active_idx: usize) -> bool {
        if let Some(entry) = self.filtered_entries.get(active_idx).cloned() {
            self.trigger_event(Some(entry));
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
        if self.active_idx + self.top_row > self.max_items {
            self.top_row = self.active_idx.saturating_sub(self.max_items);
        }
    }

    fn run_loop(&mut self, term: &mut TermWizTerminal) -> anyhow::Result<()> {
        let max_items = self.max_items;
        let alphabet = self.args.alphabet.to_lowercase();
        let alphabet_has_j = alphabet.contains("j");
        let alphabet_has_k = alphabet.contains("k");
        let labels = compute_labels_for_alphabet(&alphabet, max_items + 1);

        while let Ok(Some(event)) = term.poll_input(None) {
            match event {
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char(c),
                    modifiers: Modifiers::NONE,
                }) if !self.filtering && alphabet.contains(c) => {
                    self.selection.push(c);
                    if let Some(pos) = labels.iter().position(|x| *x == self.selection) {
                        if pos as usize <= max_items && self.launch(self.top_row + pos as usize) {
                            break;
                        }
                    }
                }
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('j'),
                    ..
                }) if !self.filtering && !alphabet_has_j => {
                    self.move_down();
                }
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('k'),
                    ..
                }) if !self.filtering && !alphabet_has_k => {
                    self.move_up();
                }
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('J'),
                    ..
                }) if !self.filtering => {
                    self.move_down();
                }
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('K'),
                    ..
                }) if !self.filtering => {
                    self.move_up();
                }
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('P'),
                    modifiers: Modifiers::CTRL,
                }) => {
                    self.move_up();
                }
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('N'),
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
                    if !self.filtering && !self.selection.is_empty() {
                        self.selection.pop();
                    }
                    if self.filter_term.pop().is_none() && !self.always_fuzzy {
                        self.filtering = false;
                    }
                    self.update_filter();
                }
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('G'),
                    modifiers: Modifiers::CTRL,
                })
                | InputEvent::Key(KeyEvent {
                    key: KeyCode::Escape,
                    ..
                }) => {
                    self.trigger_event(None);
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
                        self.trigger_event(None);
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

fn trampoline(name: String, window: GuiWin, pane: MuxPane, entry: Option<InputSelectorEntry>) {
    promise::spawn::spawn(async move {
        config::with_lua_config_on_main_thread(move |lua| do_event(lua, name, window, pane, entry))
            .await
    })
    .detach();
}

async fn do_event(
    lua: Option<Rc<mlua::Lua>>,
    name: String,
    window: GuiWin,
    pane: MuxPane,
    entry: Option<InputSelectorEntry>,
) -> anyhow::Result<()> {
    if let Some(lua) = lua {
        let id = entry.as_ref().map(|entry| entry.id.clone());
        let label = entry.as_ref().map(|entry| entry.label.to_string());

        let args = lua.pack_multi((window, pane, id, label))?;

        if let Err(err) = config::lua::emit_event(&lua, (name.clone(), args)).await {
            log::error!("while processing {} event: {:#}", name, err);
        }
    }

    Ok(())
}

pub fn selector(
    mut term: TermWizTerminal,
    args: InputSelector,
    window: GuiWin,
    pane: MuxPane,
) -> anyhow::Result<()> {
    let event_name = match *args.action {
        KeyAssignment::EmitEvent(ref id) => id.to_string(),
        _ => {
            anyhow::bail!("InputSelector requires action to be defined by wezterm.action_callback")
        }
    };
    let size = term.get_screen_size()?;
    let max_items = size.rows.saturating_sub(ROW_OVERHEAD);
    let mut state = SelectorState {
        active_idx: 0,
        max_items,
        pane,
        top_row: 0,
        filter_term: String::new(),
        filtered_entries: vec![],
        window,
        filtering: args.fuzzy,
        always_fuzzy: args.fuzzy,
        args,
        event_name,
        selection: String::new(),
    };

    term.set_raw_mode()?;
    term.render(&[Change::Title(state.args.title.to_string())])?;
    state.update_filter();
    state.render(&mut term)?;
    state.run_loop(&mut term)
}
