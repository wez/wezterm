use anyhow::anyhow;
use mux::tab::TabId;
use mux::termwiztermtab::TermWizTerminal;
use mux::window::WindowId;
use mux::Mux;
use termwiz::cell::{AttributeChange, CellAttributes};
use termwiz::color::ColorAttribute;
use termwiz::input::{InputEvent, KeyCode, KeyEvent, MouseButtons, MouseEvent};
use termwiz::surface::{Change, Position};
use termwiz::terminal::Terminal;

pub fn tab_navigator(
    tab_id: TabId,
    mut term: TermWizTerminal,
    tab_list: Vec<(String, TabId, usize)>,
    mux_window_id: WindowId,
) -> anyhow::Result<()> {
    let mut active_tab_idx = tab_list
        .iter()
        .position(|(_title, id, _)| *id == tab_id)
        .unwrap_or(0);

    term.set_raw_mode()?;

    fn render(
        active_tab_idx: usize,
        tab_list: &[(String, TabId, usize)],
        term: &mut TermWizTerminal,
    ) -> anyhow::Result<()> {
        // let dims = term.get_screen_size()?;
        let mut changes = vec![
            Change::ClearScreen(ColorAttribute::Default),
            Change::CursorPosition {
                x: Position::Absolute(0),
                y: Position::Absolute(0),
            },
            Change::Text(
                "Select a tab and press Enter to activate it.  Press Escape to cancel\r\n"
                    .to_string(),
            ),
            Change::AllAttributes(CellAttributes::default()),
        ];

        for (idx, (title, _tab_id, num_panes)) in tab_list.iter().enumerate() {
            if idx == active_tab_idx {
                changes.push(AttributeChange::Reverse(true).into());
            }

            changes.push(Change::Text(format!(
                " {}. {}. {} panes\r\n",
                idx + 1,
                title,
                num_panes
            )));

            if idx == active_tab_idx {
                changes.push(AttributeChange::Reverse(false).into());
            }
        }

        term.render(&changes)?;
        term.flush()?;
        Ok(())
    }

    term.render(&[Change::Title("Tab Navigator".to_string())])?;

    render(active_tab_idx, &tab_list, &mut term)?;

    fn select_tab_by_idx(
        idx: usize,
        mux_window_id: WindowId,
        tab_list: &Vec<(String, TabId, usize)>,
    ) -> bool {
        if idx >= tab_list.len() {
            false
        } else {
            promise::spawn::spawn_into_main_thread(async move {
                let mux = Mux::get().unwrap();
                let mut window = mux
                    .get_window_mut(mux_window_id)
                    .ok_or_else(|| anyhow!("no such window"))?;

                window.set_active(idx);
                anyhow::Result::<()>::Ok(())
            })
            .detach();
            true
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
                active_tab_idx = active_tab_idx.saturating_sub(1);
            }
            InputEvent::Key(KeyEvent {
                key: KeyCode::Char('j'),
                ..
            })
            | InputEvent::Key(KeyEvent {
                key: KeyCode::DownArrow,
                ..
            }) => {
                active_tab_idx = (active_tab_idx + 1).min(tab_list.len() - 1);
            }
            InputEvent::Key(KeyEvent {
                key: KeyCode::Escape,
                ..
            }) => {
                break;
            }
            InputEvent::Key(KeyEvent {
                key: KeyCode::Char(c),
                ..
            }) => {
                if c >= '1' && c <= '9' {
                    let idx = c as u8 - '1' as u8;
                    if select_tab_by_idx(idx as usize, mux_window_id, &tab_list) {
                        break;
                    }
                }
            }
            InputEvent::Mouse(MouseEvent {
                y, mouse_buttons, ..
            }) => {
                if y > 0 && y as usize <= tab_list.len() {
                    active_tab_idx = y as usize - 1;

                    if mouse_buttons == MouseButtons::LEFT {
                        select_tab_by_idx(active_tab_idx, mux_window_id, &tab_list);
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
                select_tab_by_idx(active_tab_idx, mux_window_id, &tab_list);
                break;
            }
            _ => {}
        }
        render(active_tab_idx, &tab_list, &mut term)?;
    }

    Ok(())
}
