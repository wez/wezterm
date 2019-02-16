use failure::Error;
use futures::{future, Future};
use glium;
use glium::glutin::EventsLoopProxy;
use mio;
use mio::{PollOpt, Ready, Token};
use std::cell::RefCell;
use std::collections::HashMap;
#[cfg(unix)]
use std::os::unix::io::RawFd;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::time::Duration;

pub use glium::glutin::WindowId;
pub use gliumwindows::TerminalWindow;

use futurecore;
use gliumwindows;
use remotemio;
use sigchld;

#[derive(Clone)]
pub struct GuiSender<T: Send> {
    tx: Sender<T>,
    proxy: EventsLoopProxy,
}

impl<T: Send> GuiSender<T> {
    pub fn send(&self, what: T) -> Result<(), Error> {
        match self.tx.send(what) {
            Ok(_) => {}
            Err(err) => bail!("send failed: {:?}", err),
        };
        self.proxy.wakeup()?;
        Ok(())
    }
}

pub fn channel<T: Send>(proxy: EventsLoopProxy) -> (GuiSender<T>, Receiver<T>) {
    let (tx, rx) = mpsc::channel();
    (GuiSender { tx, proxy }, rx)
}

/// This struct holds references to Windows.
/// The primary mapping is from `WindowId` -> `TerminalWindow`.
/// There is a secondary mapping from `RawFd` -> `WindowId` that
/// is used to process data to be read from the pty.
#[derive(Default)]
struct Windows {
    by_id: HashMap<WindowId, gliumwindows::TerminalWindow>,
    by_fd: HashMap<RawFd, WindowId>,
}

/// The `GuiEventLoop` represents the combined gui event processor,
/// a remote (running on another thread) mio `Poll` instance, and
/// a core for spawning tasks from futures.  It acts as the manager
/// for various events and is responsible for driving things forward.
pub struct GuiEventLoop {
    pub event_loop: RefCell<glium::glutin::EventsLoop>,
    windows: Rc<RefCell<Windows>>,
    pub core: futurecore::Core,
    poll: remotemio::IOMgr,
    poll_rx: Receiver<remotemio::Notification>,
    pub paster: GuiSender<WindowId>,
    paster_rx: Receiver<WindowId>,
    sigchld_rx: Receiver<()>,
}

impl GuiEventLoop {
    pub fn new() -> Result<Self, Error> {
        let event_loop = glium::glutin::EventsLoop::new();

        let (fut_tx, fut_rx) = channel(event_loop.create_proxy());
        let core = futurecore::Core::new(fut_tx, fut_rx);

        let (wake_tx, poll_rx) = channel(event_loop.create_proxy());
        let (paster, paster_rx) = channel(event_loop.create_proxy());
        let (sigchld_tx, sigchld_rx) = channel(event_loop.create_proxy());

        let poll = remotemio::IOMgr::new(Duration::from_millis(50), wake_tx);
        sigchld::activate(sigchld_tx)?;

        Ok(Self {
            core,
            poll,
            poll_rx,
            paster,
            paster_rx,
            sigchld_rx,
            event_loop: RefCell::new(event_loop),
            windows: Rc::new(RefCell::new(Default::default())),
        })
    }

    /// Add a window to the event loop and run it.
    pub fn add_window(&self, window: gliumwindows::TerminalWindow) -> Result<(), Error> {
        let window_id = window.window_id();
        let fd = window.pty_fd();
        let mut windows = self.windows.borrow_mut();
        windows.by_id.insert(window_id, window);
        windows.by_fd.insert(fd, window_id);
        self.poll
            .register(fd, Token(fd as usize), Ready::readable(), PollOpt::edge())?
            .wait()??;
        Ok(())
    }

    /// Process a single winit event
    fn process_gui_event(
        &self,
        event: &glium::glutin::Event,
    ) -> Result<glium::glutin::ControlFlow, Error> {
        use glium::glutin::ControlFlow::{Break, Continue};
        use glium::glutin::Event;
        let result = match *event {
            Event::WindowEvent { window_id, .. } => {
                let dead = match self.windows.borrow_mut().by_id.get_mut(&window_id) {
                    Some(window) => match window.dispatch_event(event) {
                        Ok(_) => None,
                        Err(err) => match err.downcast_ref::<super::SessionTerminated>() {
                            Some(_) => Some(window_id),
                            _ => return Err(err),
                        },
                    },
                    None => None,
                };

                if let Some(window_id) = dead {
                    self.schedule_window_close(window_id)?;
                }
                Continue
            }
            Event::Awakened => Break,
            _ => Continue,
        };
        Ok(result)
    }

    /// Spawns a future that will gracefully shut down the resources associated
    /// with the specified window.
    fn schedule_window_close(&self, window_id: WindowId) -> Result<(), Error> {
        let fd = {
            let mut windows = self.windows.borrow_mut();

            let window = windows.by_id.get_mut(&window_id).ok_or_else(|| {
                format_err!("no window_id {:?} in the windows_by_id map", window_id)
            })?;
            window.pty_fd()
        };

        let windows = Rc::clone(&self.windows);

        self.core.spawn(self.poll.deregister(fd)?.then(move |_| {
            let mut windows = windows.borrow_mut();
            windows.by_id.remove(&window_id);
            windows.by_fd.remove(&fd);
            future::ok(())
        }));

        Ok(())
    }

    /// Process an even from the remote mio instance.
    /// At this time, all such events correspond to readable events
    /// for the pty associated with a window.
    fn process_pty_event(&self, event: mio::Event) -> Result<(), Error> {
        // The token is the fd
        let fd = event.token().0 as RawFd;

        let (window_id, result) = {
            let mut windows = self.windows.borrow_mut();

            let window_id = windows
                .by_fd
                .get(&fd)
                .ok_or_else(|| {
                    format_err!("fd {} has no associated window in windows_by_fd map", fd)
                })
                .map(|w| *w)?;

            let window = windows.by_id.get_mut(&window_id).ok_or_else(|| {
                format_err!(
                    "fd {} -> window_id {:?} but no associated window is in the windows_by_id map",
                    fd,
                    window_id
                )
            })?;
            (window_id, window.try_read_pty())
        };

        if let Err(err) = result {
            if err.downcast_ref::<super::SessionTerminated>().is_some() {
                self.schedule_window_close(window_id)?;
            } else {
                bail!("{:?}", err);
            }
        }
        Ok(())
    }

    /// Run through all of the windows and cause them to paint if they need it.
    /// This happens ~50ms or so.
    fn do_paint(&self) {
        for window in &mut self.windows.borrow_mut().by_id.values_mut() {
            window.paint_if_needed().unwrap();
        }
    }

    /// Process events from the mio Poll instance.  We may have a pty
    /// event or our interval timer may have expired, indicating that
    /// we need to paint.
    fn process_poll(&self) -> Result<(), Error> {
        loop {
            match self.poll_rx.try_recv() {
                Ok(remotemio::Notification::EventReady(event)) => {
                    match self.process_pty_event(event) {
                        Ok(_) => {}
                        Err(err) => eprintln!("process_pty_event: {:?}", err),
                    }
                }
                Ok(remotemio::Notification::IntervalDone) => self.do_paint(),
                Err(TryRecvError::Empty) => return Ok(()),
                Err(err) => bail!("poll_rx disconnected {:?}", err),
            }
        }
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
                    let window_ids: Vec<WindowId> = self
                        .windows
                        .borrow_mut()
                        .by_id
                        .iter_mut()
                        .filter_map(|(window_id, window)| match window.test_for_child_exit() {
                            Ok(_) => None,
                            Err(_) => Some(*window_id),
                        })
                        .collect();

                    for window_id in window_ids {
                        self.schedule_window_close(window_id)?;
                    }
                }
                Err(TryRecvError::Empty) => return Ok(()),
                Err(err) => bail!("paster_rx disconnected {:?}", err),
            }
        }
    }

    /// Runs the winit event loop.  This blocks until a wakeup signal
    /// is delivered to the event loop.  The `GuiSender` is our way
    /// of trigger those wakeups.
    fn run_event_loop(&self) -> Result<(), Error> {
        let mut event_loop = self.event_loop.borrow_mut();
        event_loop.run_forever(|event| {
            use glium::glutin::ControlFlow::{Break, Continue};

            let result = self.process_gui_event(&event);

            match result {
                Ok(Continue) => Continue,
                Ok(Break) => Break,
                Err(err) => {
                    eprintln!("Error in event loop: {:?}", err);
                    Break
                }
            }
        });
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

    /// Run the event loop.  Does not return until there is either a fatal
    /// error, or until there are no more windows left to manage.
    pub fn run(&self) -> Result<(), Error> {
        loop {
            self.process_futures();

            // Check the window count; if after processing the futures there
            // are no windows left, then we are done.
            {
                let windows = self.windows.borrow();
                if windows.by_id.is_empty() && windows.by_fd.is_empty() {
                    debug!("No more windows; done!");
                    return Ok(());
                }
            }

            self.run_event_loop()?;
            self.process_poll()?;
            self.process_paste()?;
            self.process_sigchld()?;
        }
    }
}
