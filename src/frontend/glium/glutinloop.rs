use crate::config::Config;
use crate::font::FontConfiguration;
use crate::frontend::glium::window::GliumTerminalWindow;
use crate::frontend::guicommon::window::TerminalWindow;
use crate::frontend::{front_end, FrontEnd};
use crate::mux::tab::Tab;
use crate::mux::window::WindowId as MuxWindowId;
use crate::mux::{Mux, SessionTerminated};
use failure::{bail, Error, Fallible};
use glium;
use glium::glutin::EventsLoopProxy;
use glium::glutin::WindowId;
use log::{debug, error};
use promise::{BasicExecutor, Executor, Future, SpawnFunc};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

/// The GuiSender is used as a handle that allows sending SpawnFunc
/// instances to be executed on the gui thread.
#[derive(Clone)]
struct GuiSender {
    tx: Sender<SpawnFunc>,
    proxy: EventsLoopProxy,
}

impl GuiSender {
    pub fn send(&self, what: SpawnFunc) -> Result<(), Error> {
        if let Err(err) = self.tx.send(what) {
            bail!("send failed: {:?}", err);
        }
        self.proxy.wakeup()?;
        Ok(())
    }

    fn new(proxy: EventsLoopProxy) -> (GuiSender, Receiver<SpawnFunc>) {
        let (tx, rx) = mpsc::channel();
        (GuiSender { tx, proxy }, rx)
    }
}

#[derive(Clone)]
pub struct GlutinGuiExecutor {
    tx: GuiSender,
}

impl BasicExecutor for GlutinGuiExecutor {
    fn execute(&self, f: SpawnFunc) {
        self.tx.send(f).expect("GlutinExecutor execute failed");
    }
}
impl Executor for GlutinGuiExecutor {
    fn clone_executor(&self) -> Box<dyn Executor> {
        Box::new(GlutinGuiExecutor {
            tx: self.tx.clone(),
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
    gui_tx: GuiSender,
    gui_rx: Receiver<SpawnFunc>,
    tick_rx: Receiver<()>,
}

const TICK_INTERVAL: Duration = Duration::from_millis(50);
const MAX_POLL_LOOP_DURATION: Duration = Duration::from_millis(500);

pub struct GlutinFrontEnd {
    event_loop: Rc<GuiEventLoop>,
}

impl GlutinFrontEnd {
    pub fn try_new(mux: &Rc<Mux>) -> Result<Rc<dyn FrontEnd>, Error> {
        let event_loop = Rc::new(GuiEventLoop::new(mux)?);
        Ok(Rc::new(Self { event_loop }))
    }
}

impl FrontEnd for GlutinFrontEnd {
    fn gui_executor(&self) -> Box<dyn Executor> {
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
        tab: &Rc<dyn Tab>,
        window_id: MuxWindowId,
    ) -> Fallible<()> {
        let window =
            GliumTerminalWindow::new(&self.event_loop, fontconfig, config, tab, window_id)?;
        self.event_loop.add_window(window)
    }
}

impl GuiEventLoop {
    pub fn new(_mux: &Rc<Mux>) -> Result<Self, Error> {
        let event_loop = glium::glutin::EventsLoop::new();

        let (gui_tx, gui_rx) = GuiSender::new(event_loop.create_proxy());

        // The glutin/glium plumbing has no native tick/timer stuff, so
        // we implement one using a thread.  Nice.
        let proxy = event_loop.create_proxy();
        let (tick_tx, tick_rx) = mpsc::channel();
        thread::spawn(move || loop {
            std::thread::sleep(TICK_INTERVAL);
            if tick_tx.send(()).is_err() {
                return;
            }
            if proxy.wakeup().is_err() {
                return;
            }
        });

        Ok(Self {
            gui_rx,
            gui_tx,
            tick_rx,
            event_loop: RefCell::new(event_loop),
            windows: Rc::new(RefCell::new(Default::default())),
        })
    }

    fn gui_executor(&self) -> Box<dyn Executor> {
        Box::new(GlutinGuiExecutor {
            tx: self.gui_tx.clone(),
        })
    }

    pub fn with_window<F: Send + 'static + Fn(&mut dyn TerminalWindow) -> Result<(), Error>>(
        &self,
        window_id: WindowId,
        func: F,
    ) {
        Future::with_executor(
            GlutinGuiExecutor {
                tx: self.gui_tx.clone(),
            },
            move || {
                let front_end = front_end().expect("to be called on gui thread");
                let front_end = front_end
                    .downcast_ref::<GlutinFrontEnd>()
                    .expect("front_end to be GlutinFrontEnd");
                let mut windows = front_end.event_loop.windows.borrow_mut();
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
                let front_end = front_end().expect("to be called on gui thread");
                let front_end = front_end
                    .downcast_ref::<GlutinFrontEnd>()
                    .expect("front_end to be GlutinFrontEnd");

                let mut windows = front_end.event_loop.windows.borrow_mut();

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
                Ok(func) => func(),
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
                Ok(Continue) => Continue,
                Ok(Break) => Break,
                Err(err) => {
                    error!("Error in event loop: {:?}", err);
                    Break
                }
            }
        });
        Ok(())
    }
}
