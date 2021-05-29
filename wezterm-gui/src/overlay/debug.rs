use crate::scripting::guiwin::GuiWin;
use chrono::prelude::*;
use log::Level;
use luahelper::ValueWrapper;
use mlua::Value;
use mux::termwiztermtab::TermWizTerminal;
use termwiz::cell::{AttributeChange, CellAttributes, Intensity};
use termwiz::color::AnsiColor;
use termwiz::input::{InputEvent, KeyCode, KeyEvent};
use termwiz::lineedit::*;
use termwiz::surface::Change;
use termwiz::terminal::Terminal;

struct LuaReplHost {
    history: BasicHistory,
    lua: mlua::Lua,
}

impl LineEditorHost for LuaReplHost {
    fn history(&mut self) -> &mut dyn History {
        &mut self.history
    }

    fn resolve_action(
        &mut self,
        event: &InputEvent,
        editor: &mut LineEditor<'_>,
    ) -> Option<Action> {
        let (line, _cursor) = editor.get_line_and_cursor();
        if line.is_empty()
            && matches!(
                event,
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Escape,
                    ..
                })
            )
        {
            Some(Action::Cancel)
        } else {
            None
        }
    }

    fn render_preview(&self, line: &str) -> Vec<OutputElement> {
        let expr = format!("return {}", line);
        let mut preview = vec![];

        let chunk = self.lua.load(&expr);
        match chunk.into_function() {
            Ok(_) => {}
            Err(err) => {
                let text = match &err {
                    mlua::Error::SyntaxError {
                        incomplete_input: true,
                        ..
                    } => "...".to_string(),
                    _ => format!("{:#}", err),
                };
                preview.push(OutputElement::Text(text));
            }
        }

        preview
    }
}

pub fn show_debug_overlay(mut term: TermWizTerminal, gui_win: GuiWin) -> anyhow::Result<()> {
    term.no_grab_mouse_in_raw_mode();

    let lua = config::Config::load()?
        .lua
        .ok_or_else(|| anyhow::anyhow!("failed to setup lua context"))?;
    lua.load("wezterm = require 'wezterm'").exec()?;
    lua.globals().set("window", gui_win)?;

    let mut latest_log_entry = None;
    let mut host = LuaReplHost {
        history: BasicHistory::default(),
        lua,
    };

    term.render(&[Change::Title("Debug".to_string())])?;

    fn print_new_log_entries(
        term: &mut TermWizTerminal,
        latest: &mut Option<DateTime<Local>>,
    ) -> termwiz::Result<()> {
        let entries = env_bootstrap::ringlog::get_entries();
        let mut changes = vec![];
        for entry in entries {
            if let Some(latest) = latest {
                if entry.then <= *latest {
                    // already seen this one
                    continue;
                }
            }
            latest.replace(entry.then);

            changes.push(Change::AllAttributes(CellAttributes::default()));
            changes.push(Change::Text(entry.then.format("%H:%M:%S%.3f ").to_string()));

            changes.push(
                AttributeChange::Foreground(match entry.level {
                    Level::Error => AnsiColor::Maroon.into(),
                    Level::Warn => AnsiColor::Red.into(),
                    Level::Info => AnsiColor::Green.into(),
                    Level::Debug => AnsiColor::Blue.into(),
                    Level::Trace => AnsiColor::Fuschia.into(),
                })
                .into(),
            );
            changes.push(Change::Text(
                match entry.level {
                    Level::Error => "ERROR",
                    Level::Warn => "WARNING",
                    Level::Info => "INFO",
                    Level::Debug => "DEBUG",
                    Level::Trace => "TRACE",
                }
                .to_string(),
            ));
            changes.push(Change::AllAttributes(CellAttributes::default()));
            changes.push(AttributeChange::Intensity(Intensity::Bold).into());
            changes.push(Change::Text(format!(" {}", entry.target)));
            changes.push(Change::AllAttributes(CellAttributes::default()));
            changes.push(Change::Text(format!(" > {}\r\n", entry.msg)));
        }
        term.render(&changes)
    }

    loop {
        print_new_log_entries(&mut term, &mut latest_log_entry)?;
        let mut editor = LineEditor::new(&mut term);
        editor.set_prompt("> ");
        if let Some(line) = editor.read_line(&mut host)? {
            if line.is_empty() {
                continue;
            }
            host.history().add(&line);

            let expr = format!("return {}", line);
            let chunk = host.lua.load(&expr);
            match chunk.eval::<Value>() {
                Ok(result) => {
                    let text = format!("{:?}", ValueWrapper(result));
                    term.render(&[Change::Text(format!("{}\r\n", text.replace("\n", "\r\n")))])?;
                }
                Err(err) => {
                    let text = match &err {
                        mlua::Error::SyntaxError {
                            incomplete_input: true,
                            ..
                        } => "...".to_string(),
                        _ => format!("{:#}", err),
                    };
                    term.render(&[Change::Text(format!("{}\r\n", text.replace("\n", "\r\n")))])?;
                }
            }
        } else {
            return Ok(());
        }
    }
}
