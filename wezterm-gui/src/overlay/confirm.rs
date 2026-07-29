use crate::scripting::guiwin::GuiWin;
use config::keyassignment::{Confirmation, KeyAssignment};
use mux::termwiztermtab::TermWizTerminal;
use mux_lua::MuxPane;
use std::rc::Rc;
use termwiz::cell::AttributeChange;
use termwiz::color::ColorAttribute;
use termwiz::input::{InputEvent, KeyCode, KeyEvent, MouseButtons, MouseEvent};
use termwiz::surface::{Change, CursorVisibility, Position};
use termwiz::terminal::Terminal;

pub fn run_confirmation(message: &str, term: &mut TermWizTerminal) -> anyhow::Result<bool> {
    run_confirmation_impl(message, term)
}

fn run_confirmation_impl(message: &str, term: &mut TermWizTerminal) -> anyhow::Result<bool> {
    term.set_raw_mode()?;

    let size = term.get_screen_size()?;

    // Render 80% wide, centered
    let text_width = size.cols * 80 / 100;
    let x_pos = size.cols * 10 / 100;

    // Fit text to the width
    let wrapped = textwrap::fill(message, text_width);

    let message_rows = wrapped.split("\n").count();
    // Now we want to vertically center the prompt in the view.
    // After the prompt there will be a blank line and then the "buttons",
    // so we add two to the number of rows.
    let top_row = (size.rows - (message_rows + 2)) / 2;

    let button_row = top_row + message_rows + 1;
    let mut active = ActiveButton::None;

    let yes_x = x_pos;
    let yes_w = 7;

    let no_x =  yes_x + yes_w + 8 /* spacer */;
    let no_w = 6;

    #[derive(Copy, Clone, PartialEq, Eq)]
    enum ActiveButton {
        None,
        Yes,
        No,
    }

    let render = |term: &mut TermWizTerminal, active: ActiveButton| -> termwiz::Result<()> {
        let mut changes = vec![
            Change::ClearScreen(ColorAttribute::Default),
            Change::CursorVisibility(CursorVisibility::Hidden),
        ];

        for (y, row) in wrapped.split("\n").enumerate() {
            let row = row.trim_end();
            changes.push(Change::CursorPosition {
                x: Position::Absolute(x_pos),
                y: Position::Absolute(top_row + y),
            });
            changes.push(Change::Text(row.to_string()));
        }

        changes.push(Change::CursorPosition {
            x: Position::Absolute(x_pos),
            y: Position::Absolute(button_row),
        });

        if active == ActiveButton::Yes {
            changes.push(AttributeChange::Reverse(true).into());
        }
        changes.push(" [Y]es ".into());
        if active == ActiveButton::Yes {
            changes.push(AttributeChange::Reverse(false).into());
        }

        changes.push("        ".into());

        if active == ActiveButton::No {
            changes.push(AttributeChange::Reverse(true).into());
        }
        changes.push(" [N]o ".into());
        if active == ActiveButton::No {
            changes.push(AttributeChange::Reverse(false).into());
        }

        term.render(&changes)?;
        term.flush()
    };

    render(term, active)?;

    while let Ok(Some(event)) = term.poll_input(None) {
        match event {
            InputEvent::Key(KeyEvent {
                key: KeyCode::Char('y' | 'Y'),
                ..
            }) => {
                return Ok(true);
            }
            InputEvent::Key(KeyEvent {
                key: KeyCode::Char('n' | 'N'),
                ..
            })
            | InputEvent::Key(KeyEvent {
                key: KeyCode::Escape,
                ..
            }) => {
                return Ok(false);
            }
            InputEvent::Mouse(MouseEvent {
                x,
                y,
                mouse_buttons,
                ..
            }) => {
                let x = x as usize;
                let y = y as usize;
                if y == button_row && x >= yes_x && x < yes_x + yes_w {
                    active = ActiveButton::Yes;
                    if mouse_buttons == MouseButtons::LEFT {
                        return Ok(true);
                    }
                } else if y == button_row && x >= no_x && x < no_x + no_w {
                    active = ActiveButton::No;
                    if mouse_buttons == MouseButtons::LEFT {
                        return Ok(false);
                    }
                } else {
                    active = ActiveButton::None;
                }

                if mouse_buttons != MouseButtons::NONE {
                    // Treat any other mouse button as cancel
                    return Ok(false);
                }
            }
            _ => {}
        }

        render(term, active)?;
    }

    Ok(false)
}

pub fn show_confirmation_overlay(
    mut term: TermWizTerminal,
    args: Confirmation,
    window: GuiWin,
    pane: MuxPane,
) -> anyhow::Result<()> {
    let name = match *args.action {
        KeyAssignment::EmitEvent(id) => id,
        _ => anyhow::bail!("Confirmation requires action to be defined by wezterm.action_callback"),
    };

    if let Ok(confirm) = run_confirmation_impl(&args.message, &mut term) {
        if confirm {
            promise::spawn::spawn_into_main_thread(async move {
                trampoline(name, window, pane);
                anyhow::Result::<()>::Ok(())
            })
            .detach();
        } else if let Some(key_assignment) = args.cancel {
            if let KeyAssignment::EmitEvent(id) = *key_assignment {
                promise::spawn::spawn_into_main_thread(async move {
                    trampoline(id, window, pane);
                    anyhow::Result::<()>::Ok(())
                })
                .detach();
            }
        }
    }
    Ok(())
}

fn trampoline(name: String, window: GuiWin, pane: MuxPane) {
    promise::spawn::spawn(async move {
        config::with_lua_config_on_main_thread(move |lua| do_event(lua, name, window, pane)).await
    })
    .detach();
}

async fn do_event(
    lua: Option<Rc<mlua::Lua>>,
    name: String,
    window: GuiWin,
    pane: MuxPane,
) -> anyhow::Result<()> {
    if let Some(lua) = lua {
        let args = lua.pack_multi((window, pane))?;

        if let Err(err) = config::lua::emit_event(&lua, (name.clone(), args)).await {
            log::error!("while processing {} event: {:#}", name, err);
        }
    }

    Ok(())
}
