use super::{GuiSystem, SessionTerminated};
use crate::config::Config;
use crate::font::FontConfiguration;
use crate::futurecore;
use crate::guicommon::tabs::TabId;
use crate::xwindows::xwin::X11TerminalWindow;
use crate::xwindows::Connection;
use crate::{spawn_window_impl, Child, MasterPty};
use failure::Error;
use mio::unix::EventedFd;
use mio::{Event, Evented, Events, Poll, PollOpt, Ready, Token};
use mio_extras::channel::{channel, Receiver as GuiReceiver, Sender as GuiSender};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::os::unix::io::AsRawFd;
use std::os::unix::io::RawFd;
use std::rc::Rc;
use std::sync::mpsc::TryRecvError;
use std::time::{Duration, Instant};
use xcb;

#[cfg(all(unix, not(target_os = "macos")))]
pub use xcb::xproto::Window as WindowId;

impl futurecore::CoreSender for GuiSender<usize> {
    fn send(&self, idx: usize) -> Result<(), Error> {
        GuiSender::send(self, idx).map_err(|e| format_err!("send: {}", e))
    }
}

impl futurecore::CoreReceiver for GuiReceiver<usize> {
    fn try_recv(&self) -> Result<usize, TryRecvError> {
        GuiReceiver::try_recv(self)
    }
}

struct TabEntry {
    tab_id: TabId,
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
    by_id: HashMap<WindowId, X11TerminalWindow>,
    tab_by_id: HashMap<TabId, Rc<TabEntry>>,
}

pub struct GuiEventLoop {
    poll: Poll,
    pub conn: Rc<Connection>,
    pub core: futurecore::Core,
    windows: Rc<RefCell<Windows>>,
    interval: Duration,
}

const TOK_CORE: usize = 0xffff_ffff;
const TOK_XCB: usize = 0xffff_fffc;

pub struct X11GuiSystem {
    event_loop: Rc<GuiEventLoop>,
}
impl X11GuiSystem {
    pub fn new() -> Result<Rc<GuiSystem>, Error> {
        let event_loop = Rc::new(GuiEventLoop::new()?);
        Ok(Rc::new(Self { event_loop }))
    }
}

impl super::GuiSystem for X11GuiSystem {
    fn run_forever(&self) -> Result<(), Error> {
        self.event_loop.run()
    }
    fn spawn_new_window(
        &self,
        terminal: term::Terminal,
        master: MasterPty,
        child: Child,
        config: &Rc<Config>,
        fontconfig: &Rc<FontConfiguration>,
    ) -> Result<(), Error> {
        let window = X11TerminalWindow::new(
            &self.event_loop,
            terminal,
            master,
            child,
            fontconfig,
            config,
        )?;

        self.event_loop.add_window(window)
    }
}

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
        let fut_tx2 = fut_tx.clone();
        let core = futurecore::Core::new(Box::new(fut_tx), Box::new(fut_tx2), Box::new(fut_rx));

        Ok(Self {
            conn,
            poll,
            core,
            interval: Duration::from_millis(50),
            windows: Rc::new(RefCell::new(Default::default())),
        })
    }

    fn run(&self) -> Result<(), Error> {
        let mut events = Events::with_capacity(8);

        let tok_core = Token(TOK_CORE);
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
                        } else if t == tok_xcb {
                            self.process_queued_xcb()?;
                        } else {
                            self.process_pty_event(event)?;
                        }
                    }
                    self.process_sigchld();
                    // Check the window count; if after processing the futures there
                    // are no windows left, then we are done.
                    {
                        let windows = self.windows.borrow();
                        if windows.by_id.is_empty() && windows.tab_by_id.is_empty() {
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

    /// Run a function with access to the mutable version of the window with
    /// the specified window id
    pub fn with_window<F: FnOnce(&mut X11TerminalWindow) -> Result<(), Error>>(
        &self,
        window_id: WindowId,
        func: F,
    ) -> Result<(), Error> {
        let mut windows = self.windows.borrow_mut();

        let window = windows
            .by_id
            .get_mut(&window_id)
            .ok_or_else(|| format_err!("no window_id {:?} in the windows_by_id map", window_id))?;

        func(window)
    }

    /// Spawn a new tab in the specified window.  This method registers
    /// the returned pty fd as a TabEntry so that events are wired up.
    pub fn spawn_tab(&self, window_id: WindowId) -> Result<(), Error> {
        let mut windows = self.windows.borrow_mut();

        let (tab_id, fd) = {
            let window = windows.by_id.get_mut(&window_id).ok_or_else(|| {
                format_err!("no window_id {:?} in the windows_by_id map", window_id)
            })?;

            let tab_id = window.spawn_tab()?;

            let fd = window.tabs().get_by_id(tab_id).unwrap().pty().as_raw_fd();

            (tab_id, fd)
        };

        eprintln!("spawned new tab with fd = {}", fd);

        let entry = Rc::new(TabEntry {
            fd,
            tab_id,
            window_id,
        });
        windows.tab_by_id.insert(tab_id, Rc::clone(&entry));
        self.poll.register(
            &*entry,
            Token(tab_id as usize),
            Ready::readable(),
            PollOpt::edge(),
        )?;

        Ok(())
    }

    pub fn spawn_window(
        &self,
        event_loop: &Rc<Self>,
        config: &Rc<Config>,
        fontconfig: &Rc<FontConfiguration>,
    ) -> Result<(), Error> {
        let (terminal, master, child, fontconfig) = spawn_window_impl(None, config, fontconfig)?;

        let window =
            X11TerminalWindow::new(event_loop, terminal, master, child, &fontconfig, config)?;

        self.add_window(window)
    }

    pub fn add_window(&self, window: X11TerminalWindow) -> Result<(), Error> {
        let window_id = window.window_id();

        let mut windows = self.windows.borrow_mut();

        for tab in window.tabs().iter() {
            let fd = tab.pty().as_raw_fd();
            let tab_id = tab.tab_id();
            let entry = Rc::new(TabEntry {
                fd,
                tab_id,
                window_id,
            });
            windows.tab_by_id.insert(tab_id, Rc::clone(&entry));
            self.poll.register(
                &*entry,
                Token(tab_id as usize),
                Ready::readable(),
                PollOpt::edge(),
            )?;
        }
        windows.by_id.insert(window_id, window);
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
        // The token is the tab_id
        let tab_id = event.token().0 as TabId;

        let (window_id, tab_id, result) = {
            let mut windows = self.windows.borrow_mut();

            let entry = windows
                .tab_by_id
                .get(&tab_id)
                .ok_or_else(|| {
                    format_err!(
                        "tab_id {} has no associated window in windows_tab_by_id map",
                        tab_id
                    )
                })
                .map(Rc::clone)?;

            let window = windows.by_id.get_mut(&entry.window_id).ok_or_else(|| {
                format_err!(
                    "tab {} -> window_id {:?} but no associated window is in the windows_tab_by_id map",
                    tab_id,
                    entry.window_id
                )
            })?;
            (
                entry.window_id,
                entry.tab_id,
                window.try_read_pty(entry.tab_id),
            )
        };

        if let Err(err) = result {
            if err.downcast_ref::<SessionTerminated>().is_some() {
                self.windows
                    .borrow_mut()
                    .by_id
                    .get_mut(&window_id)
                    .map(|window| {
                        window.tab_did_terminate(tab_id);
                        Some(())
                    });
            } else {
                bail!("{:?}", err);
            }
        }
        Ok(())
    }

    pub fn deregister_tab(&self, tab_id: TabId) -> Result<(), Error> {
        let mut windows = self.windows.borrow_mut();
        if let Some(entry) = windows.tab_by_id.get_mut(&tab_id) {
            self.poll.deregister(&**entry).ok();
        }
        windows.tab_by_id.remove(&tab_id);
        Ok(())
    }

    fn schedule_window_close(&self, window_id: WindowId) -> Result<(), Error> {
        eprintln!("schedule_window_close {:?}", window_id);

        let mut windows = self.windows.borrow_mut();

        let tab_ids: Vec<TabId> = windows
            .tab_by_id
            .iter()
            .filter_map(|(tab_id, entry)| {
                if entry.window_id == window_id {
                    Some(*tab_id)
                } else {
                    None
                }
            })
            .collect();

        for tab_id in &tab_ids {
            if let Some(entry) = windows.tab_by_id.get_mut(&tab_id) {
                self.poll.deregister(&**entry).ok();
            }
            windows.tab_by_id.remove(&tab_id);
        }

        windows.by_id.remove(&window_id);
        eprintln!(
            "deregistered tabs {:?}.  Remaining counts {} {}",
            tab_ids,
            windows.tab_by_id.len(),
            windows.by_id.len()
        );

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
        } else {
            let r = event.response_type() & 0x7f;
            if r == self.conn.kbd_ev {
                // key press/release are not processed here.
                // xkbcommon depends on those events in order to:
                //    - update modifiers state
                //    - update keymap/state on keyboard changes
                self.conn.keyboard.process_xkb_event(&self.conn, event)?;
            }
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

    /// If we were signalled by a child process completion, zip through
    /// the windows and have then notice and prepare to close.
    fn process_sigchld(&self) {
        let window_ids: Vec<WindowId> = self
            .windows
            .borrow_mut()
            .by_id
            .iter_mut()
            .filter_map(|(window_id, window)| match window.test_for_child_exit() {
                false => None,
                true => Some(*window_id),
            })
            .collect();

        for window_id in window_ids {
            self.schedule_window_close(window_id).ok();
        }
    }
}
