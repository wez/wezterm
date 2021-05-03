use chrono::prelude::*;
use log::Level;
use mux::termwiztermtab::TermWizTerminal;
use std::time::Duration;
use termwiz::cell::{AttributeChange, CellAttributes, Intensity};
use termwiz::color::AnsiColor;
use termwiz::input::{InputEvent, KeyCode, KeyEvent};
use termwiz::surface::Change;
use termwiz::terminal::Terminal;

pub fn show_debug_overlay(mut term: TermWizTerminal) -> anyhow::Result<()> {
    let log_interval = Duration::from_secs(1);
    let mut latest_log_entry = None;

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

    print_new_log_entries(&mut term, &mut latest_log_entry)?;

    while let Ok(res) = term.poll_input(Some(log_interval)) {
        match res {
            Some(event) => match event {
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Escape,
                    ..
                }) => {
                    break;
                }
                _ => {}
            },
            None => {
                print_new_log_entries(&mut term, &mut latest_log_entry)?;
            }
        }
    }

    Ok(())
}
