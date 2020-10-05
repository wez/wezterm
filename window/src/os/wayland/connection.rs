#![allow(dead_code)]
use super::keyboard::KeyboardDispatcher;
use super::pointer::*;
use super::window::*;
use crate::connection::ConnectionOps;
use crate::spawn::*;
use crate::timerlist::{TimerEntry, TimerList};
use crate::Connection;
use anyhow::{anyhow, bail, Context};
use smithay_client_toolkit as toolkit;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::AtomicUsize;
use std::time::{Duration, Instant};
use toolkit::environment::Environment;
use toolkit::reexports::calloop::{EventLoop, EventSource, Interest, Mode, Poll, Readiness, Token};
use toolkit::reexports::client::Display;
use toolkit::WaylandSource;

toolkit::default_environment!(MyEnvironment, desktop);

pub struct WaylandConnection {
    should_terminate: RefCell<bool>,
    timers: RefCell<TimerList>,
    pub(crate) next_window_id: AtomicUsize,
    pub(crate) windows: RefCell<HashMap<usize, Rc<RefCell<WaylandWindowInner>>>>,

    // Take care with the destruction order: the underlying wayland
    // libraries are not safe and require destruction in reverse
    // creation order.  This list of fields must reflect that otherwise
    // we'll segfault on shutdown.
    // Rust guarantees that struct fields are dropped in the order
    // they appear in the struct, so the Display must be at the
    // bottom of this list.
    pub(crate) pointer: PointerDispatcher,
    pub(crate) keyboard: KeyboardDispatcher,
    pub(crate) environment: RefCell<Environment<MyEnvironment>>,
    event_q: RefCell<EventLoop<()>>,
    pub(crate) display: RefCell<Display>,
    #[cfg(feature = "opengl")]
    pub(crate) gl_connection: RefCell<Option<Rc<crate::egl::GlConnection>>>,
}

impl WaylandConnection {
    pub fn create_new() -> anyhow::Result<Self> {
        let (environment, display, event_q) =
            toolkit::init_default_environment!(MyEnvironment, desktop)?;
        let event_loop = toolkit::reexports::calloop::EventLoop::<()>::new()?;

        let keyboard = KeyboardDispatcher::new();
        let mut pointer = None;

        for seat in environment.get_all_seats() {
            if let Some((has_kbd, has_ptr)) = toolkit::seat::with_seat_data(&seat, |seat_data| {
                (
                    seat_data.has_keyboard && !seat_data.defunct,
                    seat_data.has_pointer && !seat_data.defunct,
                )
            }) {
                if has_kbd {
                    keyboard.register(event_loop.handle(), &seat)?;
                }
                if has_ptr {
                    pointer.replace(PointerDispatcher::register(
                        &seat,
                        environment.require_global(),
                        environment.require_global(),
                        environment.require_global(),
                    )?);
                }
            }
        }

        WaylandSource::new(event_q)
            .quick_insert(event_loop.handle())
            .map_err(|e| anyhow!("failed to setup WaylandSource: {:?}", e))?;

        Ok(Self {
            display: RefCell::new(display),
            event_q: RefCell::new(event_loop),
            environment: RefCell::new(environment),
            should_terminate: RefCell::new(false),
            timers: RefCell::new(TimerList::new()),
            next_window_id: AtomicUsize::new(1),
            windows: RefCell::new(HashMap::new()),
            keyboard,
            pointer: pointer.unwrap(),
            #[cfg(feature = "opengl")]
            gl_connection: RefCell::new(None),
        })
    }

    pub(crate) fn next_window_id(&self) -> usize {
        self.next_window_id
            .fetch_add(1, ::std::sync::atomic::Ordering::Relaxed)
    }

    fn flush(&self) -> anyhow::Result<()> {
        if let Err(e) = self.display.borrow_mut().flush() {
            if e.kind() != ::std::io::ErrorKind::WouldBlock {
                bail!("Error while flushing display: {}", e);
            }
        }
        Ok(())
    }

    fn do_paint(&self) {}

    pub(crate) fn window_by_id(&self, window_id: usize) -> Option<Rc<RefCell<WaylandWindowInner>>> {
        self.windows.borrow().get(&window_id).map(Rc::clone)
    }

    pub(crate) fn with_window_inner<
        R,
        F: FnMut(&mut WaylandWindowInner) -> anyhow::Result<R> + Send + 'static,
    >(
        window: usize,
        mut f: F,
    ) -> promise::Future<R>
    where
        R: Send + 'static,
    {
        let mut prom = promise::Promise::new();
        let future = prom.get_future().unwrap();

        promise::spawn::spawn_into_main_thread(async move {
            if let Some(handle) = Connection::get().unwrap().wayland().window_by_id(window) {
                let mut inner = handle.borrow_mut();
                prom.result(f(&mut inner));
            }
        })
        .detach();

        future
    }
}

struct SpawnQueueSource {}
impl EventSource for SpawnQueueSource {
    type Event = ();
    type Metadata = ();
    type Ret = ();

    fn process_events<F>(
        &mut self,
        _readiness: Readiness,
        _token: Token,
        mut callback: F,
    ) -> std::io::Result<()>
    where
        F: FnMut(Self::Event, &mut Self::Metadata) -> Self::Ret,
    {
        callback((), &mut ());
        Ok(())
    }

    fn register(&mut self, poll: &mut Poll, token: Token) -> std::io::Result<()> {
        poll.register(SPAWN_QUEUE.raw_fd(), Interest::Readable, Mode::Level, token)
    }

    fn reregister(&mut self, poll: &mut Poll, token: Token) -> std::io::Result<()> {
        poll.register(SPAWN_QUEUE.raw_fd(), Interest::Readable, Mode::Level, token)
    }

    fn unregister(&mut self, poll: &mut Poll) -> std::io::Result<()> {
        poll.unregister(SPAWN_QUEUE.raw_fd())
    }
}

impl ConnectionOps for WaylandConnection {
    fn terminate_message_loop(&self) {
        *self.should_terminate.borrow_mut() = true;
    }

    fn run_message_loop(&self) -> anyhow::Result<()> {
        self.flush()?;

        self.event_q
            .borrow_mut()
            .handle()
            .insert_source(SpawnQueueSource {}, move |_, _, _| {
                // In theory, we'd SPAWN_QUEUE.run() here but we
                // prefer to defer that to the loop below where we
                // can have better control over the event_q borrow,
                // and so that we can inspect its return code.
            })
            .map_err(|e| anyhow!("failed to insert SpawnQueueSource: {:?}", e))?;

        let paint_interval = Duration::from_millis(25);
        let mut last_interval = Instant::now();

        while !*self.should_terminate.borrow() {
            self.timers.borrow_mut().run_ready();

            let now = Instant::now();
            let diff = now - last_interval;
            let period = if diff >= paint_interval {
                self.do_paint();
                last_interval = now;
                paint_interval
            } else {
                paint_interval - diff
            };

            // Check the spawn queue before we try to sleep; there may
            // be work pending and we don't guarantee that there is a
            // 1:1 wakeup to queued function, so we need to be assertive
            // in order to avoid missing wakeups
            let period = if SPAWN_QUEUE.run() {
                // if we processed one, we don't want to sleep because
                // there may be others to deal with
                Duration::new(0, 0)
            } else {
                self.timers
                    .borrow()
                    .time_until_due(Instant::now())
                    .map(|duration| duration.min(period))
                    .unwrap_or(period)
            };

            self.flush()?;

            {
                let mut event_q = self.event_q.borrow_mut();
                if let Err(err) = event_q.dispatch(Some(period), &mut ()) {
                    if err.kind() != std::io::ErrorKind::WouldBlock
                        && err.kind() != std::io::ErrorKind::Interrupted
                    {
                        return Err(err).context("error during event_q.dispatch");
                    }
                }
            }
        }
        self.windows.borrow_mut().clear();

        Ok(())
    }

    fn schedule_timer<F: FnMut() + 'static>(&self, interval: std::time::Duration, callback: F) {
        self.timers.borrow_mut().insert(TimerEntry {
            callback: Box::new(callback),
            due: Instant::now(),
            interval,
        });
    }
}
