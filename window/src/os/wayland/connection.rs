#![allow(dead_code)]
use super::pointer::*;
use super::window::*;
use crate::connection::ConnectionOps;
use crate::os::x11::keyboard::Keyboard;
use crate::spawn::*;
use crate::Connection;
use anyhow::{bail, Context};
use mio::unix::EventedFd;
use mio::{Evented, Events, Poll, PollOpt, Ready, Token};
use smithay_client_toolkit as toolkit;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Read;
use std::os::unix::io::FromRawFd;
use std::rc::Rc;
use std::sync::atomic::AtomicUsize;
use toolkit::environment::Environment;
use toolkit::reexports::client::Display;
use toolkit::seat::SeatListener;
use wayland_client::protocol::wl_keyboard::{Event as WlKeyboardEvent, KeymapFormat, WlKeyboard};
use wayland_client::{EventQueue, Main};

toolkit::default_environment!(MyEnvironment, desktop);

pub struct WaylandConnection {
    should_terminate: RefCell<bool>,
    pub(crate) next_window_id: AtomicUsize,
    pub(crate) windows: RefCell<HashMap<usize, Rc<RefCell<WaylandWindowInner>>>>,

    // Take care with the destruction order: the underlying wayland
    // libraries are not safe and require destruction in reverse
    // creation order.  This list of fields must reflect that otherwise
    // we'll segfault on shutdown.
    // Rust guarantees that struct fields are dropped in the order
    // they appear in the struct, so the Display must be at the
    // bottom of this list, and opengl, which depends on everything
    // must be ahead of the rest.
    pub(crate) gl_connection: RefCell<Option<Rc<crate::egl::GlConnection>>>,
    pub(crate) pointer: PointerDispatcher,
    pub(crate) keyboard_mapper: RefCell<Option<Keyboard>>,
    pub(crate) keyboard_window_id: RefCell<Option<usize>>,
    pub(crate) surface_to_window_id: RefCell<HashMap<u32, usize>>,

    /// Repeats per second
    pub(crate) key_repeat_rate: RefCell<i32>,

    /// Delay before repeating, in milliseconds
    pub(crate) key_repeat_delay: RefCell<i32>,
    pub(crate) last_serial: RefCell<u32>,
    seat_listener: SeatListener,
    pub(crate) environment: RefCell<Environment<MyEnvironment>>,
    event_q: RefCell<EventQueue>,
    pub(crate) display: RefCell<Display>,
}

impl WaylandConnection {
    pub fn create_new() -> anyhow::Result<Self> {
        let (environment, display, event_q) =
            toolkit::new_default_environment!(MyEnvironment, desktop)?;

        let mut pointer = None;

        for seat in environment.get_all_seats() {
            if let Some((has_kbd, has_ptr)) = toolkit::seat::with_seat_data(&seat, |seat_data| {
                (
                    seat_data.has_keyboard && !seat_data.defunct,
                    seat_data.has_pointer && !seat_data.defunct,
                )
            }) {
                if has_kbd {
                    let keyboard = seat.get_keyboard();
                    keyboard.quick_assign(|keyboard, event, _| {
                        let conn = Connection::get().unwrap().wayland();
                        if let Err(err) = conn.keyboard_event(keyboard, event) {
                            log::error!("keyboard_event: {:#}", err);
                        }
                    });
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

        let seat_listener;
        {
            seat_listener = environment.listen_for_seats(move |seat, seat_data, _| {
                if seat_data.has_keyboard {
                    if !seat_data.defunct {
                        let keyboard = seat.get_keyboard();
                        keyboard.quick_assign(|keyboard, event, _| {
                            let conn = Connection::get().unwrap().wayland();
                            if let Err(err) = conn.keyboard_event(keyboard, event) {
                                log::error!("keyboard_event: {:#}", err);
                            }
                        });
                    }
                }
                if seat_data.has_pointer {
                    // TODO: ideally do something similar to the keyboard state,
                    // but the pointer state has a lot of other stuff floating
                    // around it so it's not so clear cut right now.
                    log::error!(
                        "seat {} changed; it has a pointer that is
                        defunct={} and we don't know what to do about it",
                        seat_data.name,
                        seat_data.defunct
                    );
                }
            });
        }

        Ok(Self {
            display: RefCell::new(display),
            environment: RefCell::new(environment),
            should_terminate: RefCell::new(false),
            next_window_id: AtomicUsize::new(1),
            windows: RefCell::new(HashMap::new()),
            event_q: RefCell::new(event_q),
            pointer: pointer.unwrap(),
            seat_listener,
            gl_connection: RefCell::new(None),
            keyboard_mapper: RefCell::new(None),
            key_repeat_rate: RefCell::new(25),
            key_repeat_delay: RefCell::new(400),
            keyboard_window_id: RefCell::new(None),
            last_serial: RefCell::new(0),
            surface_to_window_id: RefCell::new(HashMap::new()),
        })
    }

    fn keyboard_event(
        &self,
        _pointer: Main<WlKeyboard>,
        event: WlKeyboardEvent,
    ) -> anyhow::Result<()> {
        match &event {
            WlKeyboardEvent::Enter {
                serial, surface, ..
            } => {
                *self.last_serial.borrow_mut() = *serial;
                if let Some(&window_id) = self
                    .surface_to_window_id
                    .borrow()
                    .get(&surface.as_ref().id())
                {
                    self.keyboard_window_id.borrow_mut().replace(window_id);
                } else {
                    log::warn!("{:?}, no known surface", event);
                }
            }
            WlKeyboardEvent::Leave { serial, .. }
            | WlKeyboardEvent::Key { serial, .. }
            | WlKeyboardEvent::Modifiers { serial, .. } => {
                *self.last_serial.borrow_mut() = *serial;
            }
            WlKeyboardEvent::RepeatInfo { rate, delay } => {
                *self.key_repeat_rate.borrow_mut() = *rate;
                *self.key_repeat_delay.borrow_mut() = *delay;
            }
            WlKeyboardEvent::Keymap { format, fd, size } => {
                let mut file = unsafe { std::fs::File::from_raw_fd(*fd) };
                match format {
                    KeymapFormat::XkbV1 => {
                        let mut data = vec![0u8; *size as usize];
                        file.read_exact(&mut data)?;
                        // Dance around CString panicing on the NUL terminator
                        // in the xkbcommon crate
                        while let Some(0) = data.last() {
                            data.pop();
                        }
                        let s = String::from_utf8(data)?;
                        match Keyboard::new_from_string(s) {
                            Ok(k) => {
                                self.keyboard_mapper.replace(Some(k));
                            }
                            Err(err) => {
                                log::error!("Error processing keymap change: {:#}", err);
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        if let Some(&window_id) = self.keyboard_window_id.borrow().as_ref() {
            if let Some(win) = self.window_by_id(window_id) {
                let mut inner = win.borrow_mut();
                inner.keyboard_event(event);
            }
        }

        Ok(())
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

    pub(crate) fn window_by_id(&self, window_id: usize) -> Option<Rc<RefCell<WaylandWindowInner>>> {
        self.windows.borrow().get(&window_id).map(Rc::clone)
    }

    pub(crate) fn with_window_inner<
        R,
        F: FnOnce(&mut WaylandWindowInner) -> anyhow::Result<R> + Send + 'static,
    >(
        window: usize,
        f: F,
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

impl Evented for WaylandConnection {
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> std::io::Result<()> {
        EventedFd(&self.display.borrow().get_connection_fd()).register(poll, token, interest, opts)
    }
    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> std::io::Result<()> {
        EventedFd(&self.display.borrow().get_connection_fd())
            .reregister(poll, token, interest, opts)
    }
    fn deregister(&self, poll: &Poll) -> std::io::Result<()> {
        EventedFd(&self.display.borrow().get_connection_fd()).deregister(poll)
    }
}

impl ConnectionOps for WaylandConnection {
    fn terminate_message_loop(&self) {
        *self.should_terminate.borrow_mut() = true;
    }

    fn run_message_loop(&self) -> anyhow::Result<()> {
        const TOK_WL: usize = 0xffff_fffc;
        const TOK_SPAWN: usize = 0xffff_fffd;
        let tok_wl = Token(TOK_WL);
        let tok_spawn = Token(TOK_SPAWN);

        let poll = Poll::new()?;
        let mut events = Events::with_capacity(8);
        poll.register(self, tok_wl, Ready::readable(), PollOpt::level())?;
        poll.register(
            &*SPAWN_QUEUE,
            tok_spawn,
            Ready::readable(),
            PollOpt::level(),
        )?;

        while !*self.should_terminate.borrow() {
            // Check the spawn queue before we try to sleep; there may
            // be work pending and we don't guarantee that there is a
            // 1:1 wakeup to queued function, so we need to be assertive
            // in order to avoid missing wakeups
            let timeout = if SPAWN_QUEUE.run() {
                // if we processed one, we don't want to sleep because
                // there may be others to deal with
                Some(std::time::Duration::from_secs(0))
            } else {
                None
            };

            {
                let mut event_q = self.event_q.borrow_mut();
                if let Err(err) = event_q.dispatch_pending(&mut (), |_, _, _| {}) {
                    return Err(err).with_context(|| {
                        format!(
                            "error during event_q.dispatch protocol_error={:?}",
                            self.display.borrow().protocol_error()
                        )
                    });
                }
            }

            self.flush()?;
            if let Err(err) = poll.poll(&mut events, timeout) {
                bail!("polling for events: {:?}", err);
            }

            for event in &events {
                if event.token() == tok_wl {
                    let event_q = self.event_q.borrow();
                    if let Some(guard) = event_q.prepare_read() {
                        if let Err(err) = guard.read_events() {
                            if err.kind() != std::io::ErrorKind::WouldBlock
                                && err.kind() != std::io::ErrorKind::Interrupted
                            {
                                return Err(err).with_context(|| {
                                    format!(
                                        "error during event_q.read_events protocol_error={:?}",
                                        self.display.borrow().protocol_error()
                                    )
                                });
                            }
                        }
                    }
                }
            }
        }
        self.windows.borrow_mut().clear();

        Ok(())
    }
}
