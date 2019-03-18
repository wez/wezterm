use crate::config::Config;
use crate::font::{FontConfiguration, FontSystemSelection};
use crate::frontend::glium::window::GliumTerminalWindow;
use crate::frontend::guicommon::window::TerminalWindow;
use crate::frontend::FrontEnd;
use crate::mux::tab::Tab;
use crate::mux::{Mux, SessionTerminated};
use crate::spawn_tab;
use failure::Error;
use glium;
use glium::glutin::EventsLoopProxy;
use glium::glutin::WindowId;
use promise::{Executor, Future, SpawnFunc};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

#[derive(Clone)]
struct GuiSender<T: Send> {
    tx: SyncSender<T>,
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

fn channel<T: Send>(proxy: EventsLoopProxy) -> (GuiSender<T>, Receiver<T>) {
    // Set an upper bound on the number of items in the queue, so that
    // we don't swamp the gui loop; this puts back pressure on the
    // producer side so that we have a chance for eg: processing CTRL-C
    let (tx, rx) = mpsc::sync_channel(12);
    (GuiSender { tx, proxy }, rx)
}

#[derive(Clone)]
pub struct GlutinGuiExecutor {
    tx: Arc<GuiSender<SpawnFunc>>,
}

impl Executor for GlutinGuiExecutor {
    fn execute(&self, f: SpawnFunc) {
        self.tx.send(f).expect("GlutinExecutor execute failed");
    }
    fn clone_executor(&self) -> Box<Executor> {
        Box::new(GlutinGuiExecutor {
            tx: Arc::clone(&self.tx),
        })
    }
}

/// This struct holds references to Windows.
/// The primary mapping is from `WindowId` -> `GliumTerminalWindow`.
#[derive(Default)]
struct Windows {
    by_id: HashMap<WindowId, GliumTerminalWindow>,
}

/// The `GuiEventLoop` represents the combined gui event processor,
/// and a core for spawning tasks from futures.  It acts as the manager
/// for various events and is responsible for driving things forward.
pub struct GuiEventLoop {
    pub event_loop: RefCell<glium::glutin::EventsLoop>,
    windows: Rc<RefCell<Windows>>,
    gui_tx: Arc<GuiSender<SpawnFunc>>,
    gui_rx: Receiver<SpawnFunc>,
    tick_rx: Receiver<()>,
    mux: Rc<Mux>,
}

const TICK_INTERVAL: Duration = Duration::from_millis(50);
const MAX_POLL_LOOP_DURATION: Duration = Duration::from_millis(500);

pub struct GlutinFrontEnd {
    event_loop: Rc<GuiEventLoop>,
}

thread_local! {
    static GLUTIN_EVENT_LOOP: RefCell<Option<Rc<GuiEventLoop>>> = RefCell::new(None);
}

impl GlutinFrontEnd {
    pub fn try_new(mux: &Rc<Mux>) -> Result<Rc<FrontEnd>, Error> {
        let event_loop = Rc::new(GuiEventLoop::new(mux)?);
        GLUTIN_EVENT_LOOP.with(|f| *f.borrow_mut() = Some(Rc::clone(&event_loop)));
        Ok(Rc::new(Self { event_loop }))
    }
}

impl FrontEnd for GlutinFrontEnd {
    fn gui_executor(&self) -> Box<Executor> {
        self.event_loop.gui_executor()
    }

    fn run_forever(&self) -> Result<(), Error> {
        // This convoluted run() signature is present because of this issue:
        // https://github.com/tomaka/winit/issues/413
        let myself = &self.event_loop;
        loop {
            // Check the window count; if after processing the futures there
            // are no windows left, then we are done.
            {
                let windows = myself.windows.borrow();
                if windows.by_id.is_empty() {
                    debug!("No more windows; done!");
                    return Ok(());
                }
            }

            myself.run_event_loop()?;
            myself.process_gui_exec()?;
            myself.process_tick()?;
        }
    }

    fn spawn_new_window(
        &self,
        config: &Arc<Config>,
        fontconfig: &Rc<FontConfiguration>,
        tab: &Rc<Tab>,
    ) -> Result<(), Error> {
        let window = GliumTerminalWindow::new(&self.event_loop, fontconfig, config, tab)?;

        self.event_loop.add_window(window)
    }
}

impl GuiEventLoop {
    pub fn new(mux: &Rc<Mux>) -> Result<Self, Error> {
        let event_loop = glium::glutin::EventsLoop::new();

        let (gui_tx, gui_rx) = channel(event_loop.create_proxy());

        // The glutin/glium plumbing has no native tick/timer stuff, so
        // we implement one using a thread.  Nice.
        let (tick_tx, tick_rx) = channel(event_loop.create_proxy());
        thread::spawn(move || loop {
            std::thread::sleep(TICK_INTERVAL);
            if tick_tx.send(()).is_err() {
                return;
            }
        });

        Ok(Self {
            gui_rx,
            gui_tx: Arc::new(gui_tx),
            tick_rx,
            event_loop: RefCell::new(event_loop),
            windows: Rc::new(RefCell::new(Default::default())),
            mux: Rc::clone(mux),
        })
    }

    pub fn get() -> Option<Rc<Self>> {
        let mut res = None;
        GLUTIN_EVENT_LOOP.with(|f| {
            if let Some(me) = &*f.borrow() {
                res = Some(Rc::clone(me));
            }
        });
        res
    }

    fn gui_executor(&self) -> Box<Executor> {
        Box::new(GlutinGuiExecutor {
            tx: self.gui_tx.clone(),
        })
    }

    pub fn register_tab(&self, tab: &Rc<Tab>) -> Result<(), Error> {
        self.mux.add_tab(self.gui_executor(), tab)
    }

    fn do_spawn_new_window(
        &self,
        config: &Arc<Config>,
        fonts: &Rc<FontConfiguration>,
    ) -> Result<(), Error> {
        let tab = spawn_tab(&config, None)?;
        self.mux.add_tab(self.gui_executor(), &tab)?;
        let events = Self::get().expect("to be called on gui thread");
        let window = GliumTerminalWindow::new(&events, &fonts, &config, &tab)?;

        events.add_window(window)
    }

    pub fn schedule_spawn_new_window(&self, config: &Arc<Config>) {
        let config = Arc::clone(config);
        Future::with_executor(
            GlutinGuiExecutor {
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

    pub fn with_window<F: Send + 'static + Fn(&mut TerminalWindow) -> Result<(), Error>>(
        &self,
        window_id: WindowId,
        func: F,
    ) {
        Future::with_executor(
            GlutinGuiExecutor {
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
    }

    /// Add a window to the event loop and run it.
    pub fn add_window(&self, window: GliumTerminalWindow) -> Result<(), Error> {
        let window_id = window.window_id();
        let mut windows = self.windows.borrow_mut();
        windows.by_id.insert(window_id, window);
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
                        Err(err) => match err.downcast_ref::<SessionTerminated>() {
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
        Future::with_executor(
            GlutinGuiExecutor {
                tx: self.gui_tx.clone(),
            },
            move || {
                let events = Self::get().expect("to be called on gui thread");
                let mut windows = events.windows.borrow_mut();

                windows.by_id.remove(&window_id);
                Ok(())
            },
        );

        Ok(())
    }

    /// Run through all of the windows and cause them to paint if they need it.
    /// This happens ~50ms or so.
    fn do_paint(&self) {
        for window in &mut self.windows.borrow_mut().by_id.values_mut() {
            window.paint_if_needed().unwrap();
        }
    }

    fn process_gui_exec(&self) -> Result<(), Error> {
        let start = SystemTime::now();
        loop {
            match start.elapsed() {
                Ok(elapsed) if elapsed > MAX_POLL_LOOP_DURATION => {
                    return Ok(());
                }
                Err(_) => {
                    return Ok(());
                }
                _ => {}
            }
            match self.gui_rx.try_recv() {
                Ok(func) => func.call(),
                Err(TryRecvError::Empty) => return Ok(()),
                Err(err) => bail!("poll_rx disconnected {:?}", err),
            }
        }
    }

    fn process_tick(&self) -> Result<(), Error> {
        loop {
            match self.tick_rx.try_recv() {
                Ok(_) => {
                    self.test_for_child_exit();
                    self.do_paint();
                }
                Err(TryRecvError::Empty) => return Ok(()),
                Err(err) => bail!("tick_rx disconnected {:?}", err),
            }
        }
    }

    fn test_for_child_exit(&self) {
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

    /// Runs the winit event loop.  This blocks until a wakeup signal
    /// is delivered to the event loop.  The `GuiSender` is our way
    /// of trigger those wakeups.
    fn run_event_loop(&self) -> Result<(), Error> {
        let mut event_loop = self.event_loop.borrow_mut();
        event_loop.run_forever(|event| {
            use glium::glutin::ControlFlow::{Break, Continue};

            let result = self.process_gui_event(&event);

            match result {
                Ok(Continue) => {
                    self.do_paint();
                    Continue
                }
                Ok(Break) => Break,
                Err(err) => {
                    eprintln!("Error in event loop: {:?}", err);
                    Break
                }
            }
        });
        Ok(())
    }
}
