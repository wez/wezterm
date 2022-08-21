#![allow(dead_code)]
use super::pointer::*;
use super::window::*;
use crate::connection::ConnectionOps;
use crate::os::wayland::inputhandler::InputHandler;
use crate::os::wayland::output::OutputHandler;
use crate::os::x11::keyboard::Keyboard;
use crate::screen::{ScreenInfo, Screens};
use crate::spawn::*;
use crate::{Appearance, Connection, ScreenRect, WindowEvent};
use anyhow::{bail, Context};
use mio::unix::SourceFd;
use mio::{Events, Interest, Poll, Token};
use smithay_client_toolkit as toolkit;
use std::cell::RefCell;
use std::collections::HashMap;
use std::os::unix::fs::FileExt;
use std::os::unix::io::FromRawFd;
use std::rc::Rc;
use std::sync::atomic::AtomicUsize;
use toolkit::environment::Environment;
use toolkit::reexports::client::Display;
use toolkit::seat::SeatListener;
use toolkit::shm::AutoMemPool;
use wayland_client::protocol::wl_keyboard::{Event as WlKeyboardEvent, KeymapFormat, WlKeyboard};
use wayland_client::{EventQueue, Main};

toolkit::default_environment!(MyEnvironment, desktop,
fields=[
    output_handler: OutputHandler,
    input_handler: InputHandler,
],
singles=[
    wayland_protocols::wlr::unstable::output_management::v1::client::zwlr_output_manager_v1::ZwlrOutputManagerV1 => output_handler,
    wayland_protocols::unstable::text_input::v3::client::zwp_text_input_manager_v3::ZwpTextInputManagerV3 => input_handler,
]);

impl MyEnvironment {
    pub fn input_handler(&mut self) -> &mut InputHandler {
        &mut self.input_handler
    }
}

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
    pub(crate) pointer: RefCell<PointerDispatcher>,
    pub(crate) keyboard_mapper: RefCell<Option<Keyboard>>,
    pub(crate) keyboard_window_id: RefCell<Option<usize>>,
    pub(crate) surface_to_window_id: RefCell<HashMap<u32, usize>>,
    pub(crate) active_surface_id: RefCell<u32>,

    /// Repeats per second
    pub(crate) key_repeat_rate: RefCell<i32>,

    pub(crate) mem_pool: RefCell<AutoMemPool>,

    /// Delay before repeating, in milliseconds
    pub(crate) key_repeat_delay: RefCell<i32>,
    pub(crate) last_serial: RefCell<u32>,
    seat_listener: SeatListener,
    pub(crate) environment: Environment<MyEnvironment>,
    event_q: RefCell<EventQueue>,
    pub(crate) display: RefCell<Display>,
}

impl Drop for WaylandConnection {
    fn drop(&mut self) {
        self.environment
            .with_inner(|env| env.input_handler.shutdown());
    }
}

impl WaylandConnection {
    pub fn create_new() -> anyhow::Result<Self> {
        let (environment, display, event_q) = toolkit::new_default_environment!(
            MyEnvironment,
            desktop,
            fields = [
                output_handler: OutputHandler::new(),
                input_handler: InputHandler::new(),
            ]
        )?;

        let mut pointer = None;
        let mut seat_keyboards = HashMap::new();

        for seat in environment.get_all_seats() {
            if let Some((has_kbd, has_ptr, name)) =
                toolkit::seat::with_seat_data(&seat, |seat_data| {
                    (
                        seat_data.has_keyboard && !seat_data.defunct,
                        seat_data.has_pointer && !seat_data.defunct,
                        seat_data.name.clone(),
                    )
                })
            {
                if has_kbd {
                    let keyboard = seat.get_keyboard();
                    keyboard.quick_assign(|keyboard, event, _| {
                        let conn = Connection::get().unwrap().wayland();
                        if let Err(err) = conn.keyboard_event(keyboard, event) {
                            log::error!("keyboard_event: {:#}", err);
                        }
                    });
                    environment.with_inner(|env| env.input_handler.advise_seat(&seat, &keyboard));
                    seat_keyboards.insert(name, keyboard);
                }
                if has_ptr {
                    pointer.replace(PointerDispatcher::register(
                        &seat,
                        environment.require_global(),
                        environment.require_global(),
                        environment.require_global(),
                        environment.get_primary_selection_manager(),
                    )?);
                }
            }
        }

        let seat_listener;
        {
            let env = environment.clone();
            seat_listener = environment.listen_for_seats(move |seat, seat_data, _| {
                if seat_data.has_keyboard {
                    if !seat_data.defunct {
                        // We only want to assign a new keyboard object if we don't already have
                        // one for this seat. When a seat is being created or updated, the listener
                        // can receive the same seat multiple times: for example, when switching
                        // back from another virtual console, the same seat is usually seen four
                        // times with different data flags:
                        //
                        // has_pointer: true;  has_keyboard: false
                        // has_pointer: false; has_keyboard: false
                        // has_pointer: false; has_keyboard: true
                        // has_pointer: true;  has_keyboard: true
                        //
                        // This is essentially telling the client to re-assign its keyboard and
                        // pointer, but that means that this listener will fire twice with
                        // has_keyboard set to true. If we assign a handler both times, then we end
                        // up handling key events twice.
                        if !seat_keyboards.contains_key(&seat_data.name) {
                            let keyboard = seat.get_keyboard();

                            keyboard.quick_assign(|keyboard, event, _| {
                                let conn = Connection::get().unwrap().wayland();
                                if let Err(err) = conn.keyboard_event(keyboard, event) {
                                    log::error!("keyboard_event: {:#}", err);
                                }
                            });
                            env.with_inner(|env| env.input_handler.advise_seat(&seat, &keyboard));
                            seat_keyboards.insert(seat_data.name.clone(), keyboard);
                        }
                    } else {
                        env.with_inner(|env| env.input_handler.seat_defunct(&seat));
                    }
                } else {
                    // If we previously had a keyboard object on this seat, it's no longer valid if
                    // has_keyboard is false, so we remove the keyboard object we knew about and
                    // thereby ensure that we assign a new keyboard object next time the listener
                    // fires for this seat with has_keyboard = true.
                    seat_keyboards.remove(&seat_data.name);
                }
                if seat_data.has_pointer && !seat_data.defunct {
                    let conn = Connection::get().unwrap().wayland();
                    conn.pointer.borrow_mut().seat_changed(&seat);
                }
            });
        }

        let mem_pool = environment.create_auto_pool()?;

        Ok(Self {
            display: RefCell::new(display),
            environment,
            should_terminate: RefCell::new(false),
            next_window_id: AtomicUsize::new(1),
            windows: RefCell::new(HashMap::new()),
            event_q: RefCell::new(event_q),
            pointer: RefCell::new(pointer.unwrap()),
            seat_listener,
            mem_pool: RefCell::new(mem_pool),
            gl_connection: RefCell::new(None),
            keyboard_mapper: RefCell::new(None),
            key_repeat_rate: RefCell::new(25),
            key_repeat_delay: RefCell::new(400),
            keyboard_window_id: RefCell::new(None),
            last_serial: RefCell::new(0),
            surface_to_window_id: RefCell::new(HashMap::new()),
            active_surface_id: RefCell::new(0),
        })
    }

    fn keyboard_event(
        &self,
        keyboard: Main<WlKeyboard>,
        event: WlKeyboardEvent,
    ) -> anyhow::Result<()> {
        match &event {
            WlKeyboardEvent::Enter {
                serial, surface, ..
            } => {
                // update global active surface id
                *self.active_surface_id.borrow_mut() = surface.as_ref().id();

                *self.last_serial.borrow_mut() = *serial;
                if let Some(&window_id) = self
                    .surface_to_window_id
                    .borrow()
                    .get(&surface.as_ref().id())
                {
                    self.keyboard_window_id.borrow_mut().replace(window_id);
                    self.environment.with_inner(|env| {
                        if let Some(input) =
                            env.input_handler.get_text_input_for_keyboard(&keyboard)
                        {
                            input.enable();
                            input.commit();
                        }
                        env.input_handler.advise_surface(&surface, &keyboard);
                    });
                } else {
                    log::warn!("{:?}, no known surface", event);
                }
            }
            WlKeyboardEvent::Leave { serial, .. } => {
                if let Some(input) = self
                    .environment
                    .with_inner(|env| env.input_handler.get_text_input_for_keyboard(&keyboard))
                {
                    input.disable();
                    input.commit();
                }
                *self.last_serial.borrow_mut() = *serial;
            }
            WlKeyboardEvent::Key { serial, .. } | WlKeyboardEvent::Modifiers { serial, .. } => {
                *self.last_serial.borrow_mut() = *serial;
            }
            WlKeyboardEvent::RepeatInfo { rate, delay } => {
                *self.key_repeat_rate.borrow_mut() = *rate;
                *self.key_repeat_delay.borrow_mut() = *delay;
            }
            WlKeyboardEvent::Keymap { format, fd, size } => {
                let file = unsafe { std::fs::File::from_raw_fd(*fd) };
                match format {
                    KeymapFormat::XkbV1 => {
                        let mut data = vec![0u8; *size as usize];
                        file.read_exact_at(&mut data, 0)?;
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

    pub(crate) fn dispatch_to_focused_window(&self, event: WindowEvent) {
        if let Some(&window_id) = self.keyboard_window_id.borrow().as_ref() {
            if let Some(win) = self.window_by_id(window_id) {
                let mut inner = win.borrow_mut();
                inner.events.dispatch(event);
            }
        }
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

    fn run_message_loop_impl(&self) -> anyhow::Result<()> {
        const TOK_WL: usize = 0xffff_fffc;
        const TOK_SPAWN: usize = 0xffff_fffd;
        let tok_wl = Token(TOK_WL);
        let tok_spawn = Token(TOK_SPAWN);

        let mut poll = Poll::new()?;
        let mut events = Events::with_capacity(8);
        poll.registry().register(
            &mut SourceFd(&self.display.borrow().get_connection_fd()),
            tok_wl,
            Interest::READABLE,
        )?;
        poll.registry().register(
            &mut SourceFd(&SPAWN_QUEUE.raw_fd()),
            tok_spawn,
            Interest::READABLE,
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
                if err.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
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
        Ok(())
    }

    pub(crate) fn advise_of_appearance_change(&self, appearance: crate::Appearance) {
        for win in self.windows.borrow().values() {
            win.borrow_mut().appearance_changed(appearance);
        }
    }
}

impl ConnectionOps for WaylandConnection {
    fn terminate_message_loop(&self) {
        *self.should_terminate.borrow_mut() = true;
    }

    fn get_appearance(&self) -> Appearance {
        match promise::spawn::block_on(crate::os::xdg_desktop_portal::get_appearance()) {
            Ok(appearance) => return appearance,
            Err(err) => {
                log::debug!("Unable to resolve appearance using xdg-desktop-portal: {err:#}");
            }
        }
        // fallback
        Appearance::Light
    }

    fn run_message_loop(&self) -> anyhow::Result<()> {
        let res = self.run_message_loop_impl();
        // Ensure that we drop these eagerly, to avoid
        // noisy errors wrt. global destructors unwinding
        // in unexpected places
        self.windows.borrow_mut().clear();
        res
    }

    fn screens(&self) -> anyhow::Result<Screens> {
        if let Some(screens) = self
            .environment
            .with_inner(|env| env.output_handler.screens())
        {
            return Ok(screens);
        }

        let mut by_name = HashMap::new();
        let mut virtual_rect: ScreenRect = euclid::rect(0, 0, 0, 0);
        for output in self.environment.get_all_outputs() {
            toolkit::output::with_output_info(&output, |info| {
                let name = if info.name.is_empty() {
                    format!("{} {}", info.model, info.make)
                } else {
                    info.name.clone()
                };

                let (width, height) = info
                    .modes
                    .iter()
                    .find(|mode| mode.is_current)
                    .map(|mode| mode.dimensions)
                    .unwrap_or((info.physical_size.0, info.physical_size.1));

                let rect = euclid::rect(
                    info.location.0 as isize,
                    info.location.1 as isize,
                    width as isize,
                    height as isize,
                );

                let scale = info.scale_factor as f64;

                virtual_rect = virtual_rect.union(&rect);
                by_name.insert(
                    name.clone(),
                    ScreenInfo {
                        name,
                        rect,
                        scale,
                        max_fps: None,
                    },
                );
            });
        }

        // The main screen is the one either at the origin of
        // the virtual area, or if that doesn't exist for some weird
        // reason, the screen closest to the origin.
        let main = by_name
            .values()
            .min_by_key(|screen| {
                screen
                    .rect
                    .origin
                    .to_f32()
                    .distance_to(euclid::Point2D::origin())
                    .abs() as isize
            })
            .ok_or_else(|| anyhow::anyhow!("no screens were found"))?
            .clone();

        // We don't yet know how to determine the active screen,
        // so assume the main screen.
        let active = main.clone();

        Ok(Screens {
            main,
            active,
            by_name,
            virtual_rect,
        })
    }
}
