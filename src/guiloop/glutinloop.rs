use crate::config::Config;
use crate::font::FontConfiguration;
use crate::futurecore;
use crate::gliumwindows;
pub use crate::gliumwindows::TerminalWindow;
use crate::guiloop::SessionTerminated;
use crate::{Child, MasterPty};
use failure::Error;
use futures::future;
use glium;
use glium::glutin::EventsLoopProxy;
pub use glium::glutin::WindowId;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Read;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::thread;
use std::time::{Duration, SystemTime};

#[derive(Clone)]
pub struct GuiSender<T: Send> {
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

impl futurecore::CoreSender for GuiSender<usize> {
    fn send(&self, idx: usize) -> Result<(), Error> {
        GuiSender::send(self, idx)
    }
}

impl futurecore::CoreReceiver for Receiver<usize> {
    fn try_recv(&self) -> Result<usize, mpsc::TryRecvError> {
        Receiver::try_recv(self)
    }
}

pub fn channel<T: Send>(proxy: EventsLoopProxy) -> (GuiSender<T>, Receiver<T>) {
    // Set an upper bound on the number of items in the queue, so that
    // we don't swamp the gui loop; this puts back pressure on the
    // producer side so that we have a chance for eg: processing CTRL-C
    let (tx, rx) = mpsc::sync_channel(4);
    (GuiSender { tx, proxy }, rx)
}

#[derive(Clone)]
pub enum IOEvent {
    Data { window_id: WindowId, data: Vec<u8> },
    Terminated { window_id: WindowId },
}

/// This struct holds references to Windows.
/// The primary mapping is from `WindowId` -> `TerminalWindow`.
#[derive(Default)]
struct Windows {
    by_id: HashMap<WindowId, gliumwindows::TerminalWindow>,
}

enum SpawnRequest {
    Window,
    Tab(WindowId),
    FontLarger(WindowId),
    FontSmaller(WindowId),
    FontReset(WindowId),
}

/// The `GuiEventLoop` represents the combined gui event processor,
/// a remote (running on another thread) mio `Poll` instance, and
/// a core for spawning tasks from futures.  It acts as the manager
/// for various events and is responsible for driving things forward.
pub struct GuiEventLoop {
    pub event_loop: RefCell<glium::glutin::EventsLoop>,
    windows: Rc<RefCell<Windows>>,
    pub core: futurecore::Core,
    poll_tx: GuiSender<IOEvent>,
    poll_rx: Receiver<IOEvent>,
    spawn_tx: GuiSender<SpawnRequest>,
    spawn_rx: Receiver<SpawnRequest>,
    tick_rx: Receiver<()>,
}

const TICK_INTERVAL: Duration = Duration::from_millis(50);
const MAX_POLL_LOOP_DURATION: Duration = Duration::from_millis(500);

pub struct GlutinGuiSystem {
    event_loop: Rc<GuiEventLoop>,
}

use super::GuiSystem;

impl GlutinGuiSystem {
    pub fn new() -> Result<Rc<GuiSystem>, Error> {
        let event_loop = Rc::new(GuiEventLoop::new()?);
        Ok(Rc::new(Self { event_loop }))
    }

    fn process_spawn(
        &self,
        config: &Rc<Config>,
        fonts: &Rc<FontConfiguration>,
    ) -> Result<(), Error> {
        loop {
            match self.event_loop.spawn_rx.try_recv() {
                Ok(SpawnRequest::Window) => {
                    crate::spawn_window(&*self, None, config, fonts)?;
                }
                Ok(SpawnRequest::Tab(_window_id)) => {
                    eprintln!("Spawning tabs is not yet implemented for glutin");
                }
                Ok(SpawnRequest::FontLarger(window_id)) => {
                    let scale = fonts.get_font_scale() * 1.1;
                    if let Some(window) = self
                        .event_loop
                        .windows
                        .borrow_mut()
                        .by_id
                        .get_mut(&window_id)
                    {
                        window.scaling_changed(Some(scale)).ok();
                    }
                }
                Ok(SpawnRequest::FontSmaller(window_id)) => {
                    let scale = fonts.get_font_scale() * 0.9;
                    if let Some(window) = self
                        .event_loop
                        .windows
                        .borrow_mut()
                        .by_id
                        .get_mut(&window_id)
                    {
                        window.scaling_changed(Some(scale)).ok();
                    }
                }
                Ok(SpawnRequest::FontReset(window_id)) => {
                    if let Some(window) = self
                        .event_loop
                        .windows
                        .borrow_mut()
                        .by_id
                        .get_mut(&window_id)
                    {
                        window.scaling_changed(Some(1.0)).ok();
                    }
                }
                Err(TryRecvError::Empty) => return Ok(()),
                Err(err) => bail!("spawn_rx disconnected {:?}", err),
            }
        }
    }
}

impl GuiSystem for GlutinGuiSystem {
    fn run_forever(
        &self,
        config: &Rc<crate::config::Config>,
        fontconfig: &Rc<crate::font::FontConfiguration>,
    ) -> Result<(), Error> {
        // This convoluted run() signature is present because of this issue:
        // https://github.com/tomaka/winit/issues/413
        let myself = &self.event_loop;
        loop {
            myself.process_futures();

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
            myself.process_poll()?;
            self.process_spawn(config, fontconfig)?;
            myself.process_tick()?;
        }
    }

    fn spawn_new_window(
        &self,
        terminal: term::Terminal,
        master: MasterPty,
        child: Child,
        config: &Rc<crate::config::Config>,
        fontconfig: &Rc<crate::font::FontConfiguration>,
    ) -> Result<(), Error> {
        let window = TerminalWindow::new(
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
        let event_loop = glium::glutin::EventsLoop::new();

        let (fut_tx, fut_rx) = channel(event_loop.create_proxy());
        let fut_tx2 = fut_tx.clone();
        let core = futurecore::Core::new(Box::new(fut_tx), Box::new(fut_tx2), Box::new(fut_rx));

        let (spawn_tx, spawn_rx) = channel(event_loop.create_proxy());
        let (poll_tx, poll_rx) = channel(event_loop.create_proxy());

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
            core,
            spawn_tx,
            spawn_rx,
            poll_tx,
            poll_rx,
            tick_rx,
            event_loop: RefCell::new(event_loop),
            windows: Rc::new(RefCell::new(Default::default())),
        })
    }

    pub fn request_increase_font_size(&self, window_id: WindowId) -> Result<(), Error> {
        self.spawn_tx.send(SpawnRequest::FontLarger(window_id))
    }
    pub fn request_decrease_font_size(&self, window_id: WindowId) -> Result<(), Error> {
        self.spawn_tx.send(SpawnRequest::FontSmaller(window_id))
    }
    pub fn request_reset_font_size(&self, window_id: WindowId) -> Result<(), Error> {
        self.spawn_tx.send(SpawnRequest::FontReset(window_id))
    }

    pub fn request_spawn_window(&self) -> Result<(), Error> {
        self.spawn_tx.send(SpawnRequest::Window)
    }

    pub fn request_spawn_tab(&self, window_id: WindowId) -> Result<(), Error> {
        self.spawn_tx.send(SpawnRequest::Tab(window_id))
    }

    /// Add a window to the event loop and run it.
    pub fn add_window(&self, window: gliumwindows::TerminalWindow) -> Result<(), Error> {
        let window_id = window.window_id();
        let mut pty = window.pty().try_clone()?;
        pty.clear_nonblocking()?;
        let mut windows = self.windows.borrow_mut();
        windows.by_id.insert(window_id, window);

        let tx = self.poll_tx.clone();

        thread::spawn(move || {
            const BUFSIZE: usize = 32 * 1024;
            let mut buf = [0; BUFSIZE];
            loop {
                match pty.read(&mut buf) {
                    Ok(size) => {
                        if tx
                            .send(IOEvent::Data {
                                window_id,
                                data: buf[0..size].to_vec(),
                            })
                            .is_err()
                        {
                            return;
                        }
                    }
                    Err(err) => {
                        eprintln!("window {:?} {:?}", window_id, err);
                        tx.send(IOEvent::Terminated { window_id }).ok();
                        return;
                    }
                }
            }
        });

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
        let windows = Rc::clone(&self.windows);

        self.core.spawn(futures::future::lazy(move || {
            let mut windows = windows.borrow_mut();
            windows.by_id.remove(&window_id);
            future::ok(())
        }));

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
            match self.poll_rx.try_recv() {
                Ok(IOEvent::Data { window_id, data }) => {
                    let mut windows = self.windows.borrow_mut();
                    windows.by_id.get_mut(&window_id).map(|window| {
                        window.process_data_read_from_pty(&data);
                        Some(())
                    });
                }
                Ok(IOEvent::Terminated { window_id }) => {
                    self.schedule_window_close(window_id)?;
                }
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
            .filter_map(|(window_id, window)| match window.test_for_child_exit() {
                Ok(_) => None,
                Err(_) => Some(*window_id),
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
}
