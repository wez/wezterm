use crate::scripting::guiwin::GuiWin;
use chrono::prelude::*;
use futures::FutureExt;
use log::Level;
use luahelper::ValuePrinter;
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
    let mut host = Some(LuaReplHost {
        history: BasicHistory::default(),
        lua,
    });

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
                    Level::Trace => AnsiColor::Fuchsia.into(),
                })
                .into(),
            );
            changes.push(Change::Text(entry.level.as_str().to_string()));
            changes.push(Change::AllAttributes(CellAttributes::default()));
            changes.push(AttributeChange::Intensity(Intensity::Bold).into());
            changes.push(Change::Text(format!(" {}", entry.target)));
            changes.push(Change::AllAttributes(CellAttributes::default()));
            changes.push(Change::Text(format!(
                " > {}\r\n",
                entry.msg.replace("\n", "\r\n")
            )));
        }
        term.render(&changes)
    }

    loop {
        print_new_log_entries(&mut term, &mut latest_log_entry)?;
        let mut editor = LineEditor::new(&mut term);
        editor.set_prompt("> ");
        if let Some(line) = editor.read_line(host.as_mut().unwrap())? {
            if line.is_empty() {
                continue;
            }
            host.as_mut().unwrap().history().add(&line);

            let expr = format!("return {}", line);

            let passed_host = host.take().unwrap();

            let (host_res, text) =
                smol::block_on(promise::spawn::spawn_into_main_thread(async move {
                    evaluate_trampoline(passed_host, expr)
                        .recv()
                        .await
                        .map_err(|e| mlua::Error::external(format!("{:#}", e)))
                        .expect("returning result not to fail")
                }));

            host.replace(host_res);

            term.render(&[Change::Text(format!("{}\r\n", text.replace("\n", "\r\n")))])?;
        } else {
            return Ok(());
        }
    }
}

// A bit of indirection because spawn_into_main_thread wants the
// overall future to be Send but mlua::Value, mlua::Chunk are not
// Send.  We need to split off the actual evaluation future to
// run separately, so we spawn it and use a channel to funnel
// the result back to the caller without blocking the gui thread.
fn evaluate_trampoline(
    host: LuaReplHost,
    expr: String,
) -> smol::channel::Receiver<(LuaReplHost, String)> {
    let (tx, rx) = smol::channel::bounded(1);
    promise::spawn::spawn(async move {
        let _ = tx.send(evaluate(host, expr).await).await;
    })
    .detach();
    rx
}

async fn evaluate(host: LuaReplHost, expr: String) -> (LuaReplHost, String) {
    let result = host
        .lua
        .load(&expr)
        .eval_async::<Value>()
        .map(|result| match result {
            Ok(result) => {
                let value = ValuePrinter(result);
                format!("{:#?}", value)
            }
            Err(err) => match &err {
                mlua::Error::SyntaxError {
                    incomplete_input: true,
                    ..
                } => "...".to_string(),
                _ => format!("{:#}", err),
            },
        })
        .await;
    (host, result)
}
