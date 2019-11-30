#![allow(dead_code)]
use super::keyboard::KeyboardDispatcher;
use super::pointer::*;
use super::window::*;
use crate::connection::ConnectionOps;
use crate::spawn::*;
use crate::tasks::{Task, Tasks};
use crate::timerlist::{TimerEntry, TimerList};
use crate::Connection;
use failure::Fallible;
use mio::unix::EventedFd;
use mio::{Evented, Events, Poll, PollOpt, Ready, Token};
use promise::BasicExecutor;
use smithay_client_toolkit as toolkit;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::AtomicUsize;
use std::time::{Duration, Instant};
use toolkit::reexports::client::protocol::wl_seat::{Event as SeatEvent, WlSeat};
use toolkit::reexports::client::{Display, EventQueue};
use toolkit::Environment;

pub struct WaylandConnection {
    pub(crate) display: RefCell<Display>,
    event_q: RefCell<EventQueue>,
    pub(crate) environment: RefCell<Environment>,
    should_terminate: RefCell<bool>,
    timers: RefCell<TimerList>,
    pub(crate) tasks: Tasks,
    pub(crate) next_window_id: AtomicUsize,
    pub(crate) windows: RefCell<HashMap<usize, Rc<RefCell<WaylandWindowInner>>>>,
    pub(crate) seat: WlSeat,
    pub(crate) keyboard: KeyboardDispatcher,
    pub(crate) pointer: PointerDispatcher,
}

impl Evented for WaylandConnection {
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> std::io::Result<()> {
        EventedFd(&self.event_q.borrow().get_connection_fd()).register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> std::io::Result<()> {
        EventedFd(&self.event_q.borrow().get_connection_fd())
            .reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> std::io::Result<()> {
        EventedFd(&self.event_q.borrow().get_connection_fd()).deregister(poll)
    }
}

impl WaylandConnection {
    pub fn create_new() -> Fallible<Self> {
        let (display, mut event_q) = Display::connect_to_env()?;
        let environment = Environment::from_display(&*display, &mut event_q)?;

        let seat = environment
            .manager
            .instantiate_range(1, 6, move |seat| {
                seat.implement_closure(
                    move |event, _seat| {
                        if let SeatEvent::Name { name } = event {
                            log::error!("seat name is {}", name);
                        }
                    },
                    (),
                )
            })
            .map_err(|_| failure::format_err!("Failed to create seat"))?;
        let keyboard = KeyboardDispatcher::register(&seat)?;
        let pointer = PointerDispatcher::register(&seat, &environment.data_device_manager)?;

        Ok(Self {
            display: RefCell::new(display),
            event_q: RefCell::new(event_q),
            environment: RefCell::new(environment),
            should_terminate: RefCell::new(false),
            timers: RefCell::new(TimerList::new()),
            tasks: Default::default(),
            next_window_id: AtomicUsize::new(1),
            windows: RefCell::new(HashMap::new()),
            seat,
            keyboard,
            pointer,
        })
    }

    pub(crate) fn next_window_id(&self) -> usize {
        self.next_window_id
            .fetch_add(1, ::std::sync::atomic::Ordering::Relaxed)
    }

    pub fn executor() -> impl BasicExecutor {
        SpawnQueueExecutor {}
    }

    pub fn low_pri_executor() -> impl BasicExecutor {
        LowPriSpawnQueueExecutor {}
    }

    fn flush(&self) -> Fallible<()> {
        if let Err(e) = self.display.borrow_mut().flush() {
            if e.kind() != ::std::io::ErrorKind::WouldBlock {
                failure::bail!("Error while flushing display: {}", e);
            }
        }
        Ok(())
    }

    fn do_paint(&self) {}

    fn process_queued_events(&self) -> Fallible<()> {
        {
            let mut event_q = self.event_q.borrow_mut();
            if let Some(guard) = event_q.prepare_read() {
                if let Err(e) = guard.read_events() {
                    if e.kind() != ::std::io::ErrorKind::WouldBlock {
                        failure::bail!("Error while reading events: {}", e);
                    }
                }
            }
            event_q.dispatch_pending()?;
        }
        self.flush()?;
        Ok(())
    }

    pub(crate) fn window_by_id(&self, window_id: usize) -> Option<Rc<RefCell<WaylandWindowInner>>> {
        self.windows.borrow().get(&window_id).map(Rc::clone)
    }

    pub(crate) fn with_window_inner<
        R,
        F: FnMut(&mut WaylandWindowInner) -> Fallible<R> + Send + 'static,
    >(
        window: usize,
        mut f: F,
    ) -> promise::Future<R>
    where
        R: Send + 'static,
    {
        let mut prom = promise::Promise::new();
        let future = prom.get_future().unwrap();

        SpawnQueueExecutor {}.execute(Box::new(move || {
            if let Some(handle) = Connection::get().unwrap().wayland().window_by_id(window) {
                let mut inner = handle.borrow_mut();
                prom.result(f(&mut inner));
            }
        }));

        future
    }
}

impl ConnectionOps for WaylandConnection {
    fn spawn_task<F: std::future::Future<Output = ()> + 'static>(&self, future: F) {
        let id = self.tasks.add_task(Task(Box::pin(future)));
        Connection::wake_task_by_id(id);
    }

    fn wake_task_by_id(_slot: usize) {
        panic!("use Connection::wake_task_by_id instead of WaylandConnection::wake_task_by_id");
    }

    fn terminate_message_loop(&self) {
        *self.should_terminate.borrow_mut() = true;
    }

    fn run_message_loop(&self) -> Fallible<()> {
        println!("run_message_loop:flush");
        self.flush()?;

        const TOK_WAYLAND: usize = 0xffff_fffc;
        const TOK_SPAWN: usize = 0xffff_fffd;
        let tok_wayland = Token(TOK_WAYLAND);
        let tok_spawn = Token(TOK_SPAWN);

        let poll = Poll::new()?;
        let mut events = Events::with_capacity(8);
        poll.register(self, tok_wayland, Ready::readable(), PollOpt::level())?;
        poll.register(
            &*SPAWN_QUEUE,
            tok_spawn,
            Ready::readable(),
            PollOpt::level(),
        )?;

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

            // Process any events that might have accumulated in the local
            // buffer (eg: due to a flush) before we potentially go to sleep.
            // The locally queued events won't mark the fd as ready, so we
            // could potentially sleep when there is work to be done if we
            // relied solely on that.
            self.process_queued_events()?;

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

            match poll.poll(&mut events, Some(period)) {
                Ok(_) => {
                    // We process both event sources unconditionally
                    // in the loop above anyway; we're just using
                    // this to get woken up.
                }

                Err(err) => {
                    failure::bail!("polling for events: {:?}", err);
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
