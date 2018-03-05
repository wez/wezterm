use super::SessionTerminated;
use failure::Error;
use futurecore;
use mio::{Event, Evented, Events, Poll, PollOpt, Ready, Token};
use mio::unix::EventedFd;
pub use mio_extras::channel::{channel, Receiver as GuiReceiver, Sender as GuiSender};
use sigchld;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::os::unix::io::RawFd;
use std::rc::Rc;
use std::sync::mpsc::TryRecvError;
use std::time::{Duration, Instant};
use xcb;
use xwindows::Connection;
use xwindows::xwin::TerminalWindow;

#[cfg(all(unix, not(target_os = "macos")))]
pub use xcb::xproto::Window as WindowId;

struct TabEntry {
    fd: RawFd,
    window_id: WindowId,
}

impl Evented for TabEntry {
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        EventedFd(&self.fd).register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        EventedFd(&self.fd).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        EventedFd(&self.fd).deregister(poll)
    }
}

#[derive(Default)]
struct Windows {
    by_id: HashMap<WindowId, TerminalWindow>,
    by_fd: HashMap<RawFd, Rc<TabEntry>>,
}

pub struct GuiEventLoop {
    poll: Poll,
    pub conn: Rc<Connection>,
    pub paster: GuiSender<WindowId>,
    paster_rx: GuiReceiver<WindowId>,
    sigchld_rx: GuiReceiver<()>,
    pub core: futurecore::Core,
    windows: Rc<RefCell<Windows>>,
    interval: Duration,
}

const TOK_CORE: usize = 0xffff_ffff;
const TOK_PASTER: usize = 0xffff_fffe;
const TOK_CHLD: usize = 0xffff_fffd;
const TOK_XCB: usize = 0xffff_fffc;

impl GuiEventLoop {
    pub fn new() -> Result<Self, Error> {
        let poll = Poll::new()?;

        let conn = Rc::new(Connection::new()?);

        poll.register(&*conn, Token(TOK_XCB), Ready::readable(), PollOpt::level())?;

        let (fut_tx, fut_rx) = channel();
        poll.register(
            &fut_rx,
            Token(TOK_CORE),
            Ready::readable(),
            PollOpt::level(),
        )?;
        let core = futurecore::Core::new(fut_tx, fut_rx);

        let (paster, paster_rx) = channel();
        poll.register(
            &paster_rx,
            Token(TOK_PASTER),
            Ready::readable(),
            PollOpt::level(),
        )?;

        let (sigchld_tx, sigchld_rx) = channel();
        poll.register(
            &sigchld_rx,
            Token(TOK_CHLD),
            Ready::readable(),
            PollOpt::level(),
        )?;
        sigchld::activate(sigchld_tx)?;

        Ok(Self {
            conn,
            poll,
            core,
            paster,
            paster_rx,
            sigchld_rx,
            interval: Duration::from_millis(50),
            windows: Rc::new(RefCell::new(Default::default())),
        })
    }

    pub fn run(&self) -> Result<(), Error> {
        let mut events = Events::with_capacity(8);

        let tok_core = Token(TOK_CORE);
        let tok_paster = Token(TOK_PASTER);
        let tok_chld = Token(TOK_CHLD);
        let tok_xcb = Token(TOK_XCB);

        self.conn.flush();
        let mut last_interval = Instant::now();

        loop {
            let now = Instant::now();
            let diff = now - last_interval;
            let period = if diff >= self.interval {
                self.do_paint();
                last_interval = now;
                self.interval
            } else {
                self.interval - diff
            };

            match self.poll.poll(&mut events, Some(period)) {
                Ok(_) => {
                    for event in &events {
                        let t = event.token();
                        if t == tok_core {
                            self.process_futures();
                        } else if t == tok_paster {
                            self.process_paste()?;
                        } else if t == tok_chld {
                            self.process_sigchld()?;
                        } else if t == tok_xcb {
                            self.process_queued_xcb()?;
                        } else {
                            self.process_pty_event(event)?;
                        }
                    }
                    // Check the window count; if after processing the futures there
                    // are no windows left, then we are done.
                    {
                        let windows = self.windows.borrow();
                        if windows.by_id.is_empty() && windows.by_fd.is_empty() {
                            debug!("No more windows; done!");
                            return Ok(());
                        }
                    }
                }

                Err(err) => {
                    bail!("polling for events: {:?}", err);
                }
            }
        }
    }

    pub fn spawn_tab(&self, window_id: WindowId) -> Result<(), Error> {
        let mut windows = self.windows.borrow_mut();

        let fd = {
            let mut window = windows.by_id.get_mut(&window_id).ok_or_else(|| {
                format_err!("no window_id {:?} in the windows_by_id map", window_id)
            })?;

            window.spawn_tab()?
        };

        eprintln!("spawned new tab with fd = {}", fd);

        let entry = Rc::new(TabEntry { fd, window_id });
        windows.by_fd.insert(fd, Rc::clone(&entry));
        self.poll.register(
            &*entry,
            Token(fd as usize),
            Ready::readable(),
            PollOpt::edge(),
        )?;

        Ok(())
    }

    pub fn add_window(&self, window: TerminalWindow) -> Result<(), Error> {
        let window_id = window.window_id();
        let fds = window.pty_fds();

        let mut windows = self.windows.borrow_mut();
        windows.by_id.insert(window_id, window);

        for fd in fds {
            let entry = Rc::new(TabEntry { fd, window_id });
            windows.by_fd.insert(fd, Rc::clone(&entry));
            self.poll.register(
                &*entry,
                Token(fd as usize),
                Ready::readable(),
                PollOpt::edge(),
            )?;
        }
        Ok(())
    }

    /// Loop through the core and dispatch any tasks that have been
    /// notified as ready to run.  Returns once all such tasks have
    /// been polled and there are no more pending task notifications.
    fn process_futures(&self) {
        loop {
            if !self.core.turn() {
                break;
            }
        }
    }

    /// Process an even from the remote mio instance.
    /// At this time, all such events correspond to readable events
    /// for the pty associated with a window.
    fn process_pty_event(&self, event: Event) -> Result<(), Error> {
        // The token is the fd
        let fd = event.token().0 as RawFd;

        let (window_id, result) = {
            let mut windows = self.windows.borrow_mut();

            let entry = windows
                .by_fd
                .get(&fd)
                .ok_or_else(|| {
                    format_err!("fd {} has no associated window in windows_by_fd map", fd)
                })
                .map(|w| Rc::clone(w))?;

            let window = windows.by_id.get_mut(&entry.window_id).ok_or_else(|| {
                format_err!(
                    "fd {} -> window_id {:?} but no associated window is in the windows_by_id map",
                    fd,
                    entry.window_id
                )
            })?;
            (entry.window_id, window.try_read_pty(fd))
        };

        if let Err(err) = result {
            if err.downcast_ref::<SessionTerminated>().is_some() {
                self.schedule_window_close(window_id, Some(fd))?;
            } else {
                bail!("{:?}", err);
            }
        }
        Ok(())
    }

    fn schedule_window_close(&self, window_id: WindowId, fd: Option<RawFd>) -> Result<(), Error> {
        let mut windows = self.windows.borrow_mut();

        let (fds, window_close) = {
            let window = windows.by_id.get_mut(&window_id).ok_or_else(|| {
                format_err!("no window_id {:?} in the windows_by_id map", window_id)
            })?;

            let all_fds = window.pty_fds();
            let num_fds = all_fds.len();

            // If no fd was specified, close all of them

            let close_fds = match fd {
                Some(fd) => vec![fd],
                None => all_fds,
            };

            let window_close = close_fds.len() == num_fds;
            (close_fds, window_close)
        };

        for fd in &fds {
            if let Some(entry) = windows.by_fd.get_mut(fd) {
                self.poll.deregister(&**entry)?;
            }
            windows.by_fd.remove(fd);
        }

        if window_close {
            windows.by_id.remove(&window_id);
        } else {
            let window = windows.by_id.get_mut(&window_id).ok_or_else(|| {
                format_err!("no window_id {:?} in the windows_by_id map", window_id)
            })?;
            for fd in fds {
                window.close_tab_for_fd(fd)?;
            }
        }

        Ok(())
    }

    fn process_window_event(
        &self,
        window_id: WindowId,
        event: &xcb::GenericEvent,
    ) -> Result<(), Error> {
        let mut windows = self.windows.borrow_mut();
        if let Some(window) = windows.by_id.get_mut(&window_id) {
            window.dispatch_event(event)?;
        }
        Ok(())
    }

    fn window_id_from_event(event: &xcb::GenericEvent) -> Option<WindowId> {
        match event.response_type() & 0x7f {
            xcb::EXPOSE => {
                let expose: &xcb::ExposeEvent = unsafe { xcb::cast_event(event) };
                Some(expose.window())
            }
            xcb::CONFIGURE_NOTIFY => {
                let cfg: &xcb::ConfigureNotifyEvent = unsafe { xcb::cast_event(event) };
                Some(cfg.window())
            }
            xcb::KEY_PRESS | xcb::KEY_RELEASE => {
                let key_press: &xcb::KeyPressEvent = unsafe { xcb::cast_event(event) };
                Some(key_press.event())
            }
            xcb::MOTION_NOTIFY => {
                let motion: &xcb::MotionNotifyEvent = unsafe { xcb::cast_event(event) };
                Some(motion.event())
            }
            xcb::BUTTON_PRESS | xcb::BUTTON_RELEASE => {
                let button_press: &xcb::ButtonPressEvent = unsafe { xcb::cast_event(event) };
                Some(button_press.event())
            }
            xcb::CLIENT_MESSAGE => {
                let msg: &xcb::ClientMessageEvent = unsafe { xcb::cast_event(event) };
                Some(msg.window())
            }
            _ => None,
        }
    }

    fn process_xcb_event(&self, event: &xcb::GenericEvent) -> Result<(), Error> {
        if let Some(window_id) = Self::window_id_from_event(event) {
            self.process_window_event(window_id, event)?;
        }
        Ok(())
    }

    fn process_queued_xcb(&self) -> Result<(), Error> {
        match self.conn.poll_for_event() {
            None => match self.conn.has_error() {
                Ok(_) => (),
                Err(err) => {
                    bail!("clipboard window connection is broken: {:?}", err);
                }
            },
            Some(event) => match self.process_xcb_event(&event) {
                Ok(_) => (),
                Err(err) => return Err(err),
            },
        }
        self.conn.flush();

        loop {
            match self.conn.poll_for_queued_event() {
                None => return Ok(()),
                Some(event) => self.process_xcb_event(&event)?,
            }
            self.conn.flush();
        }
    }

    /// Run through all of the windows and cause them to paint if they need it.
    /// This happens ~50ms or so.
    fn do_paint(&self) {
        for window in &mut self.windows.borrow_mut().by_id.values_mut() {
            window.paint_if_needed().unwrap();
        }
        self.conn.flush();
    }

    /// Process paste notifications and route them to their owning windows.
    fn process_paste(&self) -> Result<(), Error> {
        loop {
            match self.paster_rx.try_recv() {
                Ok(window_id) => {
                    self.windows
                        .borrow_mut()
                        .by_id
                        .get_mut(&window_id)
                        .map(|w| w.process_clipboard());
                }
                Err(TryRecvError::Empty) => return Ok(()),
                Err(err) => bail!("paster_rx disconnected {:?}", err),
            }
        }
    }

    /// If we were signalled by a child process completion, zip through
    /// the windows and have then notice and prepare to close.
    fn process_sigchld(&self) -> Result<(), Error> {
        loop {
            match self.sigchld_rx.try_recv() {
                Ok(_) => {
                    let window_ids: Vec<WindowId> = self.windows
                        .borrow_mut()
                        .by_id
                        .iter_mut()
                        .filter_map(|(window_id, window)| match window.test_for_child_exit() {
                            Ok(_) => None,
                            Err(_) => Some(*window_id),
                        })
                        .collect();

                    for window_id in window_ids {
                        self.schedule_window_close(window_id, None)?;
                    }
                }
                Err(TryRecvError::Empty) => return Ok(()),
                Err(err) => bail!("paster_rx disconnected {:?}", err),
            }
        }
    }
}
