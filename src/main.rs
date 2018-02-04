#[macro_use]
extern crate failure;
extern crate unicode_width;
extern crate harfbuzz_sys;
#[cfg(not(target_os = "macos"))]
extern crate fontconfig; // from servo-fontconfig
#[cfg(not(target_os = "macos"))]
extern crate freetype;
extern crate resize;
extern crate libc;
extern crate mio;
extern crate term;
#[macro_use]
pub mod log;

use failure::Error;

extern crate xcb;
extern crate xcb_util;

use mio::{Events, Poll, PollOpt, Ready, Token};
use mio::unix::EventedFd;
use std::os::unix::io::AsRawFd;
use std::process::Command;
use std::time::Duration;

mod xgfx;
mod xkeysyms;
mod font;
use font::{Font, FontPattern, ftwrap};

mod pty;
mod sigchld;
mod xwin;
use xwin::TerminalWindow;

fn dispatch_gui(
    conn: &xgfx::Connection,
    event: xcb::GenericEvent,
    window: &mut TerminalWindow,
) -> Result<(), Error> {
    let r = event.response_type() & 0x7f;
    match r {
        xcb::EXPOSE => {
            let expose: &xcb::ExposeEvent = unsafe { xcb::cast_event(&event) };
            window.expose(
                expose.x(),
                expose.y(),
                expose.width(),
                expose.height(),
            )?;
        }
        xcb::CONFIGURE_NOTIFY => {
            let cfg: &xcb::ConfigureNotifyEvent = unsafe { xcb::cast_event(&event) };
            window.resize_surfaces(cfg.width(), cfg.height())?;
        }
        xcb::KEY_PRESS => {
            let key_press: &xcb::KeyPressEvent = unsafe { xcb::cast_event(&event) };
            window.key_down(key_press)?;
        }
        xcb::KEY_RELEASE => {
            let key_press: &xcb::KeyPressEvent = unsafe { xcb::cast_event(&event) };
            window.key_up(key_press)?;
        }
        xcb::BUTTON_PRESS => {
            let button_press: &xcb::ButtonPressEvent = unsafe { xcb::cast_event(&event) };
            debug!(
                "BUTTON: detail={:x} state={:x} @ {},{}",
                button_press.detail(),
                button_press.state(),
                button_press.event_x(),
                button_press.event_y()
            );
            match button_press.detail() {
                4 => window.mouse_wheel(-1),
                5 => window.mouse_wheel(1),
                _ => {}
            }
        }
        xcb::CLIENT_MESSAGE => {
            let msg: &xcb::ClientMessageEvent = unsafe { xcb::cast_event(&event) };
            println!("CLIENT_MESSAGE {:?}", msg.data().data32());
            if msg.data().data32()[0] == conn.atom_delete() {
                // TODO: cleaner exit handling
                bail!("window close requested!");
            }
        }
        _ => {}
    }
    Ok(())
}

fn run() -> Result<(), Error> {
    let poll = Poll::new()?;
    let conn = xgfx::Connection::new()?;

    let waiter = sigchld::ChildWaiter::new()?;

    // First step is to figure out the font metrics so that we know how
    // big things are going to be.

    let mut pattern = FontPattern::parse("Operator Mono SSm Lig:size=10")?;
    pattern.add_double("dpi", 96.0)?;
    let mut font = Font::new(pattern)?;
    // we always load the cell_height for font 0,
    // regardless of which font we are shaping here,
    // so that we can scale glyphs appropriately
    let (cell_height, cell_width, _) = font.get_metrics()?;

    let initial_cols = 80u16;
    let initial_rows = 24u16;
    let initial_pixel_width = initial_cols * cell_width.ceil() as u16;
    let initial_pixel_height = initial_rows * cell_height.ceil() as u16;

    let (master, slave) = pty::openpty(
        initial_rows,
        initial_cols,
        initial_pixel_width,
        initial_pixel_height,
    )?;

    let cmd = Command::new("zsh");
    let child = slave.spawn_command(cmd)?;
    eprintln!("spawned: {:?}", child);

    // Ask mio to watch the pty for input from the child process
    poll.register(
        &master,
        Token(0),
        Ready::readable(),
        PollOpt::edge(),
    )?;
    // Ask mio to monitor the X connection fd
    poll.register(
        &EventedFd(&conn.as_raw_fd()),
        Token(1),
        Ready::readable(),
        PollOpt::edge(),
    )?;

    poll.register(
        &waiter,
        Token(2),
        Ready::readable(),
        PollOpt::edge(),
    )?;

    let terminal = term::Terminal::new(initial_rows as usize, initial_cols as usize, 3000);
    //    let message = "; ❤ 😍🤢\n\x1b[91;mw00t\n\x1b[37;104;m bleet\x1b[0;m.";
    //    terminal.advance_bytes(message);
    // !=

    let mut window = TerminalWindow::new(
        &conn,
        initial_pixel_width,
        initial_pixel_height,
        terminal,
        master,
        child,
        font,
    )?;

    window.show();

    let mut events = Events::with_capacity(8);
    conn.flush();

    loop {
        if poll.poll(&mut events, Some(Duration::new(0, 0)))? == 0 {
            // No immediately ready events.  Before we go to sleep,
            // make sure we've flushed out any pending X work.
            if window.need_paint() {
                window.paint()?;
            }
            conn.flush();

            poll.poll(&mut events, None)?;
        }

        for event in &events {
            if event.token() == Token(0) && event.readiness().is_readable() {
                window.handle_pty_readable_event();
            }
            if event.token() == Token(1) && event.readiness().is_readable() {
                // Each time the XCB Connection FD shows as readable, we perform
                // a single poll against the connection and then eagerly consume
                // all of the queued events that came along as part of that batch.
                // This is important because we can't assume that one readiness
                // event from the kerenl maps to a single XCB event.  We need to be
                // sure that all buffered/queued events are consumed before we
                // allow the mio poll() routine to put us to sleep, otherwise we
                // will effectively hang without updating all the state.
                match conn.poll_for_event() {
                    Some(event) => {
                        dispatch_gui(&conn, event, &mut window)?;
                        // Since we read one event from the connection, we must
                        // now eagerly consume the rest of the queued events.
                        loop {
                            match conn.poll_for_queued_event() {
                                Some(event) => dispatch_gui(&conn, event, &mut window)?,
                                None => break,
                            }
                        }
                    }
                    None => {}
                }

                // If we got disconnected from the display server, we cannot continue
                conn.has_error()?;
            }

            if event.token() == Token(2) {
                println!("sigchld ready");
                let pid = waiter.read_one()?;
                println!("got sigchld from pid {}", pid);
                window.test_for_child_exit()?;
            }
        }
    }
}

fn main() {
    run().unwrap();
}
