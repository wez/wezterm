use crate::config::Config;
use crate::font::{FontConfiguration, FontSystemSelection};
use crate::frontend::guicommon::window::TerminalWindow;
use crate::frontend::xwindows::xwin::X11TerminalWindow;
use crate::frontend::xwindows::Connection;
use crate::frontend::FrontEnd;
use crate::mux::tab::Tab;
use crate::mux::Mux;
use crate::spawn_tab;
use failure::Error;
use failure::{bail, Error};
use mio::{Events, Poll, PollOpt, Ready, Token};
use mio_extras::channel::{channel, Receiver as GuiReceiver, Sender as GuiSender};
use portable_pty::PtySize;
use promise::{Executor, Future, SpawnFunc};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc::TryRecvError;
use std::sync::Arc;
use std::time::{Duration, Instant};
use xcb;

#[cfg(all(unix, not(target_os = "macos")))]
pub use xcb::xproto::Window as WindowId;

#[derive(Default)]
struct Windows {
    by_id: HashMap<WindowId, X11TerminalWindow>,
}

pub struct X11GuiExecutor {
    tx: GuiSender<SpawnFunc>,
}

impl Executor for X11GuiExecutor {
    fn execute(&self, f: SpawnFunc) {
        self.tx.send(f).expect("X11GuiExecutor execute failed");
    }
    fn clone_executor(&self) -> Box<Executor> {
        Box::new(X11GuiExecutor {
            tx: self.tx.clone(),
        })
    }
}

pub struct GuiEventLoop {
    poll: Poll,
    pub conn: Rc<Connection>,
    windows: Rc<RefCell<Windows>>,
    interval: Duration,
    gui_rx: GuiReceiver<SpawnFunc>,
    gui_tx: GuiSender<SpawnFunc>,
    mux: Rc<Mux>,
}

const TOK_XCB: usize = 0xffff_fffc;
const TOK_GUI_EXEC: usize = 0xffff_fffd;

pub struct X11FrontEnd {
    event_loop: Rc<GuiEventLoop>,
}
impl X11FrontEnd {
    pub fn try_new(mux: &Rc<Mux>) -> Result<Rc<FrontEnd>, Error> {
        let event_loop = Rc::new(GuiEventLoop::new(mux)?);
        X11_EVENT_LOOP.with(|f| *f.borrow_mut() = Some(Rc::clone(&event_loop)));
        Ok(Rc::new(Self { event_loop }))
    }
}

thread_local! {
    static X11_EVENT_LOOP: RefCell<Option<Rc<GuiEventLoop>>> = RefCell::new(None);
}

impl FrontEnd for X11FrontEnd {
    fn gui_executor(&self) -> Box<Executor> {
        self.event_loop.gui_executor()
    }

    fn run_forever(&self) -> Result<(), Error> {
        self.event_loop.run()
    }
    fn spawn_new_window(
        &self,
        config: &Arc<Config>,
        fontconfig: &Rc<FontConfiguration>,
        tab: &Rc<Tab>,
    ) -> Result<(), Error> {
        let window = X11TerminalWindow::new(&self.event_loop, fontconfig, config, tab)?;

        self.event_loop.add_window(window)
    }
}

impl GuiEventLoop {
    pub fn new(mux: &Rc<Mux>) -> Result<Self, Error> {
        let poll = Poll::new()?;

        let conn = Rc::new(Connection::new()?);

        poll.register(&*conn, Token(TOK_XCB), Ready::readable(), PollOpt::level())?;

        let (gui_tx, gui_rx) = channel();
        poll.register(
            &gui_rx,
            Token(TOK_GUI_EXEC),
            Ready::readable(),
            PollOpt::level(),
        )?;

        Ok(Self {
            conn,
            poll,
            gui_tx,
            gui_rx,
            interval: Duration::from_millis(50),
            windows: Rc::new(RefCell::new(Default::default())),
            mux: Rc::clone(mux),
        })
    }

    pub fn get() -> Option<Rc<Self>> {
        let mut res = None;
        X11_EVENT_LOOP.with(|f| {
            if let Some(me) = &*f.borrow() {
                res = Some(Rc::clone(me));
            }
        });
        res
    }

    fn gui_executor(&self) -> Box<Executor> {
        Box::new(X11GuiExecutor {
            tx: self.gui_tx.clone(),
        })
    }

    fn run(&self) -> Result<(), Error> {
        let mut events = Events::with_capacity(8);

        let tok_xcb = Token(TOK_XCB);
        let tok_gui = Token(TOK_GUI_EXEC);

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
                        if t == tok_xcb {
                            self.process_queued_xcb()?;
                        } else if t == tok_gui {
                            self.process_gui_exec()?;
                        } else {
                        }
                    }
                    self.process_sigchld();
                    // Check the window count; if after processing the futures there
                    // are no windows left, then we are done.
                    if self.mux.is_empty() {
                        debug!("No more windows; done!");
                        return Ok(());
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
    pub fn with_window<F: Send + 'static + Fn(&mut TerminalWindow) -> Result<(), Error>>(
        &self,
        window_id: WindowId,
        func: F,
    ) -> Result<(), Error> {
        Future::with_executor(
            X11GuiExecutor {
                tx: self.gui_tx.clone(),
            },
            move || {
                let myself = Self::get().expect("to be called on gui thread");
                let mut windows = myself.windows.borrow_mut();
                if let Some(window) = windows.by_id.get_mut(&window_id) {
                    func(window)
                } else {
                    bail!("no such window {:?}", window_id);
                }
            },
        );
        Ok(())
    }

    fn do_spawn_new_window(
        &self,
        config: &Arc<Config>,
        fonts: &Rc<FontConfiguration>,
    ) -> Result<(), Error> {
        let tab = self.mux.default_domain().spawn(PtySize::default(), None)?;
        let events = Self::get().expect("to be called on gui thread");
        let window = X11TerminalWindow::new(&events, &fonts, &config, &tab)?;
        events.add_window(window)
    }

    pub fn schedule_spawn_new_window(&self, config: &Arc<Config>) {
        let config = Arc::clone(config);
        Future::with_executor(
            X11GuiExecutor {
                tx: self.gui_tx.clone(),
            },
            move || {
                let myself = Self::get().expect("to be called on gui thread");
                let fonts = Rc::new(FontConfiguration::new(
                    Arc::clone(&config),
                    FontSystemSelection::get_default(),
                ));
                myself.do_spawn_new_window(&config, &fonts)
            },
        );
    }

    pub fn add_window(&self, window: X11TerminalWindow) -> Result<(), Error> {
        let window_id = window.window_id();

        let mut windows = self.windows.borrow_mut();

        windows.by_id.insert(window_id, window);
        Ok(())
    }

    fn process_gui_exec(&self) -> Result<(), Error> {
        match self.gui_rx.try_recv() {
            Ok(func) => func(),
            Err(TryRecvError::Empty) => return Ok(()),
            Err(err) => bail!("poll_rx disconnected {:?}", err),
        }
        Ok(())
    }

    fn schedule_window_close(&self, window_id: WindowId) -> Result<(), Error> {
        eprintln!("schedule_window_close {:?}", window_id);

        let mut windows = self.windows.borrow_mut();
        windows.by_id.remove(&window_id);

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
            .filter_map(|(window_id, window)| {
                if window.test_for_child_exit() {
                    Some(*window_id)
                } else {
                    None
                }
            })
            .collect();

        for window_id in window_ids {
            self.schedule_window_close(window_id).ok();
        }
    }
}
