use std::cell::RefCell;
use std::collections::HashMap;
use std::os::fd::AsRawFd;
use std::rc::Rc;
use std::sync::atomic::AtomicUsize;

use anyhow::{bail, Context};
use mio::unix::SourceFd;
use mio::{Events, Interest, Poll, Token};
use wayland_client::backend::WaylandError;
use wayland_client::globals::registry_queue_init;
use wayland_client::{Connection as WConnection, EventQueue};

use crate::screen::{ScreenInfo, Screens};
use crate::spawn::SPAWN_QUEUE;
use crate::{Appearance, Connection, ConnectionOps, ScreenRect};

use super::state::WaylandState;
use super::WaylandWindowInner;

pub struct WaylandConnection {
    pub(crate) should_terminate: RefCell<bool>,
    pub(crate) next_window_id: AtomicUsize,
    pub(super) gl_connection: RefCell<Option<Rc<crate::egl::GlConnection>>>,
    pub(super) connection: WConnection,
    pub(super) event_queue: RefCell<EventQueue<WaylandState>>,
    pub(super) wayland_state: RefCell<WaylandState>,
}

impl WaylandConnection {
    pub(crate) fn create_new() -> anyhow::Result<Self> {
        let conn = WConnection::connect_to_env()?;
        let (globals, event_queue) = registry_queue_init::<WaylandState>(&conn)?;
        let qh = event_queue.handle();

        let wayland_state = WaylandState::new(&globals, &qh)?;
        let wayland_connection = WaylandConnection {
            connection: conn,
            should_terminate: RefCell::new(false),
            next_window_id: AtomicUsize::new(1),
            gl_connection: RefCell::new(None),
            event_queue: RefCell::new(event_queue),
            wayland_state: RefCell::new(wayland_state),
        };

        Ok(wayland_connection)
    }

    pub(crate) fn advise_of_appearance_change(&self, appearance: crate::Appearance) {
        for win in self.wayland_state.borrow().windows.borrow().values() {
            win.borrow_mut().appearance_changed(appearance);
        }
    }

    fn run_message_loop_impl(&self) -> anyhow::Result<()> {
        const TOK_WL: usize = 0xffff_fffc;
        const TOK_SPAWN: usize = 0xffff_fffd;
        let tok_wl = Token(TOK_WL);
        let tok_spawn = Token(TOK_SPAWN);

        let mut poll = Poll::new()?;
        let mut events = Events::with_capacity(8);

        let wl_fd = {
            let read_guard = self.event_queue.borrow().prepare_read().unwrap();
            read_guard.connection_fd().as_raw_fd()
        };

        poll.registry()
            .register(&mut SourceFd(&wl_fd), tok_wl, Interest::READABLE)?;
        poll.registry().register(
            &mut SourceFd(&SPAWN_QUEUE.raw_fd()),
            tok_spawn,
            Interest::READABLE,
        )?;

        while !*self.should_terminate.borrow() {
            let timeout = if SPAWN_QUEUE.run() {
                Some(std::time::Duration::from_secs(0))
            } else {
                None
            };

            let mut event_q = self.event_queue.borrow_mut();
            {
                let mut wayland_state = self.wayland_state.borrow_mut();
                if let Err(err) = event_q.dispatch_pending(&mut wayland_state) {
                    // TODO: show the protocol error in the display
                    return Err(err)
                        .with_context(|| format!("error during event_q.dispatch protcol_error"));
                }
            }

            event_q.flush()?;
            if let Err(err) = poll.poll(&mut events, timeout) {
                if err.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                bail!("polling for events: {:?}", err);
            }

            for event in &events {
                if event.token() != tok_wl {
                    continue;
                }

                if let Some(guard) = event_q.prepare_read() {
                    if let Err(err) = guard.read() {
                        log::trace!("Event Q error: {:?}", err);
                        if let WaylandError::Protocol(perr) = err {
                            return Err(perr.into());
                            // TODO
                            // return Err(perr).with_context(|| {
                            //     format!("error during event_q.read protocol_error={:?}",
                            //             perr)
                            // })
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub(crate) fn next_window_id(&self) -> usize {
        self.next_window_id
            .fetch_add(1, ::std::sync::atomic::Ordering::Relaxed)
    }

    pub(crate) fn window_by_id(&self, window_id: usize) -> Option<Rc<RefCell<WaylandWindowInner>>> {
        self.wayland_state.borrow().window_by_id(window_id)
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

impl ConnectionOps for WaylandConnection {
    fn name(&self) -> String {
        "Wayland".to_string()
    }

    fn terminate_message_loop(&self) {
        log::trace!("Terminating Message Loop");
        *self.should_terminate.borrow_mut() = true;
    }

    fn run_message_loop(&self) -> anyhow::Result<()> {
        let res = self.run_message_loop_impl();
        // Ensure that we drop these eagerly, to avoid
        // noisy errors wrt. global destructors unwinding
        // in unexpected places
        self.wayland_state.borrow().windows.borrow_mut().clear();
        res
    }

    fn get_appearance(&self) -> Appearance {
        match promise::spawn::block_on(crate::os::xdg_desktop_portal::get_appearance()) {
            Ok(Some(appearance)) => return appearance,
            Ok(None) => {}
            Err(err) => {
                log::warn!("Unable to resolve appearance using xdg-desktop-portal: {err:#}");
            }
        }
        // fallback
        Appearance::Light
    }

    fn screens(&self) -> anyhow::Result<crate::screen::Screens> {
        log::trace!("Getting screens for wayland connection");

        if let Some(output_manager) = &self.wayland_state.borrow().output_manager {
            if let Some(screens) = output_manager.screens() {
                return Ok(screens);
            }
        }

        let mut by_name = HashMap::new();
        let mut virtual_rect: ScreenRect = euclid::rect(0, 0, 0, 0);
        let config = config::configuration();

        let output_state = &self.wayland_state.borrow().output;

        for output in output_state.outputs() {
            let info = match output_state.info(&output) {
                Some(i) => i,
                None => continue,
            };
            let name = match info.name {
                Some(n) => n.clone(),
                None => format!("{} {}", info.model, info.make),
            };

            let (width, height) = info
                .modes
                .iter()
                .find(|mode| mode.current)
                .map(|mode| mode.dimensions)
                .unwrap_or((info.physical_size.0, info.physical_size.1));

            let rect = euclid::rect(
                info.location.0 as isize,
                info.location.1 as isize,
                width as isize,
                height as isize,
            );

            let scale = info.scale_factor as f64;

            // FIXME: teach this how to resolve dpi_by_screen once
            // dispatch_pending_event knows how to do the same
            let effective_dpi = Some(config.dpi.unwrap_or(scale * crate::DEFAULT_DPI));

            virtual_rect = virtual_rect.union(&rect);
            by_name.insert(
                name.clone(),
                ScreenInfo {
                    name,
                    rect,
                    scale,
                    max_fps: None,
                    effective_dpi,
                },
            );
        }

        // // The main screen is the one either at the origin of
        // // the virtual area, or if that doesn't exist for some weird
        // // reason, the screen closest to the origin.
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
