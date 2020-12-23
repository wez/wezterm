use mux::pane::PaneId;
use mux::tab::TabId;
use mux::termwiztermtab::TermWizTerminal;
use mux::window::WindowId;
use mux::Mux;
use termwiz::color::ColorAttribute;
use termwiz::input::{InputEvent, KeyCode, KeyEvent};
use termwiz::surface::{Change, Position};
use termwiz::terminal::Terminal;

pub fn confirm_close_pane(
    pane_id: PaneId,
    mut term: TermWizTerminal,
    mux_window_id: WindowId,
) -> anyhow::Result<()> {
    term.set_raw_mode()?;

    let changes = vec![
        Change::ClearScreen(ColorAttribute::Default),
        Change::CursorPosition {
            x: Position::Absolute(0),
            y: Position::Absolute(0),
        },
        Change::Text("Really kill this pane? [y/n]\r\n".to_string()),
    ];

    term.render(&changes)?;
    term.flush()?;

    while let Ok(Some(event)) = term.poll_input(None) {
        match event {
            InputEvent::Key(KeyEvent {
                key: KeyCode::Char('y'),
                ..
            }) => {
                promise::spawn::spawn_into_main_thread(async move {
                    let mux = Mux::get().unwrap();
                    let tab = match mux.get_active_tab_for_window(mux_window_id) {
                        Some(tab) => tab,
                        None => return,
                    };
                    tab.kill_pane(pane_id);
                })
                .detach();
                break;
            }
            InputEvent::Key(KeyEvent {
                key: KeyCode::Char('n'),
                ..
            })
            | InputEvent::Key(KeyEvent {
                key: KeyCode::Escape,
                ..
            }) => {
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

pub fn confirm_close_tab(
    tab_id: TabId,
    mut term: TermWizTerminal,
    _mux_window_id: WindowId,
) -> anyhow::Result<()> {
    term.set_raw_mode()?;

    let changes = vec![
        Change::ClearScreen(ColorAttribute::Default),
        Change::CursorPosition {
            x: Position::Absolute(0),
            y: Position::Absolute(0),
        },
        Change::Text("Really kill this tab and all contained panes? [y/n]\r\n".to_string()),
    ];

    term.render(&changes)?;
    term.flush()?;

    while let Ok(Some(event)) = term.poll_input(None) {
        match event {
            InputEvent::Key(KeyEvent {
                key: KeyCode::Char('y'),
                ..
            }) => {
                promise::spawn::spawn_into_main_thread(async move {
                    let mux = Mux::get().unwrap();
                    mux.remove_tab(tab_id);
                })
                .detach();
                break;
            }
            InputEvent::Key(KeyEvent {
                key: KeyCode::Char('n'),
                ..
            })
            | InputEvent::Key(KeyEvent {
                key: KeyCode::Escape,
                ..
            }) => {
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

pub fn confirm_close_window(
    mut term: TermWizTerminal,
    mux_window_id: WindowId,
) -> anyhow::Result<()> {
    term.set_raw_mode()?;

    let changes = vec![
        Change::ClearScreen(ColorAttribute::Default),
        Change::CursorPosition {
            x: Position::Absolute(0),
            y: Position::Absolute(0),
        },
        Change::Text(
            "Really kill this window and all contained tabs and panes? [y/n]\r\n".to_string(),
        ),
    ];

    term.render(&changes)?;
    term.flush()?;

    while let Ok(Some(event)) = term.poll_input(None) {
        match event {
            InputEvent::Key(KeyEvent {
                key: KeyCode::Char('y'),
                ..
            }) => {
                promise::spawn::spawn_into_main_thread(async move {
                    let mux = Mux::get().unwrap();
                    mux.kill_window(mux_window_id);
                })
                .detach();
                break;
            }
            InputEvent::Key(KeyEvent {
                key: KeyCode::Char('n'),
                ..
            })
            | InputEvent::Key(KeyEvent {
                key: KeyCode::Escape,
                ..
            }) => {
                break;
            }
            _ => {}
        }
    }

    Ok(())
}
