use mux::pane::PaneId;
use mux::tab::TabId;
use mux::termwiztermtab::TermWizTerminal;
use mux::window::WindowId;
use mux::Mux;
use termwiz::cell::AttributeChange;
use termwiz::color::ColorAttribute;
use termwiz::input::{InputEvent, KeyCode, KeyEvent, MouseButtons, MouseEvent};
use termwiz::surface::{Change, CursorVisibility, Position};
use termwiz::terminal::Terminal;

fn run_confirmation_app(message: &str, term: &mut TermWizTerminal) -> anyhow::Result<bool> {
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

    #[derive(Copy, Clone, PartialEq, Eq)]
    enum ActiveButton {
        None,
        Yes,
        No,
    }

    let render = |term: &mut TermWizTerminal, active: ActiveButton| -> anyhow::Result<()> {
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
        changes.push(" Yes ".into());
        if active == ActiveButton::Yes {
            changes.push(AttributeChange::Reverse(false).into());
        }

        changes.push("        ".into());

        if active == ActiveButton::No {
            changes.push(AttributeChange::Reverse(true).into());
        }
        changes.push(" No ".into());
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
                key: KeyCode::Char('y'),
                ..
            }) => {
                return Ok(true);
            }
            InputEvent::Key(KeyEvent {
                key: KeyCode::Char('n'),
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
                if y == button_row && x >= x_pos && x <= x_pos + 5 {
                    active = ActiveButton::Yes;
                    if mouse_buttons == MouseButtons::LEFT {
                        return Ok(true);
                    }
                } else if y == button_row && x >= x_pos + 14 && x <= x_pos + 20 {
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

pub fn confirm_close_pane(
    pane_id: PaneId,
    mut term: TermWizTerminal,
    mux_window_id: WindowId,
) -> anyhow::Result<()> {
    if run_confirmation_app("🛑 Really kill this pane?", &mut term)? {
        promise::spawn::spawn_into_main_thread(async move {
            let mux = Mux::get().unwrap();
            let tab = match mux.get_active_tab_for_window(mux_window_id) {
                Some(tab) => tab,
                None => return,
            };
            tab.kill_pane(pane_id);
        })
        .detach();
    }

    Ok(())
}

pub fn confirm_close_tab(
    tab_id: TabId,
    mut term: TermWizTerminal,
    _mux_window_id: WindowId,
) -> anyhow::Result<()> {
    if run_confirmation_app(
        "🛑 Really kill this tab and all contained panes?",
        &mut term,
    )? {
        promise::spawn::spawn_into_main_thread(async move {
            let mux = Mux::get().unwrap();
            mux.remove_tab(tab_id);
        })
        .detach();
    }

    Ok(())
}

pub fn confirm_close_window(
    mut term: TermWizTerminal,
    mux_window_id: WindowId,
) -> anyhow::Result<()> {
    if run_confirmation_app(
        "🛑 Really kill this window and all contained tabs and panes?",
        &mut term,
    )? {
        promise::spawn::spawn_into_main_thread(async move {
            let mux = Mux::get().unwrap();
            mux.kill_window(mux_window_id);
        })
        .detach();
    }

    Ok(())
}

pub fn confirm_quit_program(mut term: TermWizTerminal) -> anyhow::Result<()> {
    if run_confirmation_app("🛑 Really Quit WezTerm?", &mut term)? {
        promise::spawn::spawn_into_main_thread(async move {
            use ::window::{Connection, ConnectionOps};
            let con = Connection::get().expect("call on gui thread");
            con.terminate_message_loop();
        })
        .detach();
    }

    Ok(())
}
