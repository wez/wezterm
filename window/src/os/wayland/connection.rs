// TODO: change this
#![allow(dead_code, unused)]
use std::{
    cell::{Ref, RefCell},
    collections::HashMap,
    os::fd::AsRawFd,
    rc::Rc,
    sync::{atomic::AtomicUsize, Arc, Mutex},
};

use anyhow::{bail, Context};
use mio::{unix::SourceFd, Events, Interest, Poll, Token};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, SurfaceData},
    delegate_compositor, delegate_output, delegate_registry, delegate_shm, delegate_xdg_shell,
    delegate_xdg_window,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::{
        xdg::window::{Window, WindowConfigure, WindowHandler, WindowState as SCTKWindowState},
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use wayland_client::{
    backend::WaylandError,
    globals::{registry_queue_init, GlobalList},
    protocol::wl_surface::WlSurface,
    Connection as WConnection, EventQueue, Proxy,
};

use crate::{
    screen::{ScreenInfo, Screens},
    spawn::SPAWN_QUEUE,
    Connection, ConnectionOps, ScreenRect, WindowState,
};

use super::{PendingEvent, SurfaceUserData, WaylandWindowInner};

pub struct WaylandConnection {
    pub(crate) should_terminate: RefCell<bool>,
    pub(crate) next_window_id: AtomicUsize,

    pub(crate) windows: RefCell<HashMap<usize, Rc<RefCell<WaylandWindowInner>>>>,

    pub(crate) gl_connection: RefCell<Option<Rc<crate::egl::GlConnection>>>,

    pub(crate) globals: RefCell<GlobalList>,
    pub(crate) connection: RefCell<WConnection>,
    pub(crate) event_queue: RefCell<EventQueue<WaylandState>>,
    pub(crate) wayland_state: RefCell<WaylandState>,
}

// TODO: the SurfaceUserData should be something in WaylandConnection struct as a whole. I think?
pub(crate) struct WaylandState {
    registry_state: RegistryState,
    output_state: OutputState,

    shm: Shm,
    pub(crate) mem_pool: RefCell<SlotPool>,
}

impl WaylandConnection {
    pub(crate) fn create_new() -> anyhow::Result<Self> {
        let conn = WConnection::connect_to_env()?;
        let (globals, event_queue) = registry_queue_init::<WaylandState>(&conn)?;
        let qh = event_queue.handle();

        let shm = Shm::bind(&globals, &qh)?;
        let mem_pool = SlotPool::new(1, &shm)?;
        let wayland_state = WaylandState {
            registry_state: RegistryState::new(&globals),
            output_state: OutputState::new(&globals, &qh),
            shm,
            mem_pool: RefCell::new(mem_pool),
        };

        let wayland_connection = WaylandConnection {
            connection: RefCell::new(conn),
            should_terminate: RefCell::new(false),
            next_window_id: AtomicUsize::new(1),
            gl_connection: RefCell::new(None),
            windows: RefCell::new(HashMap::default()),

            event_queue: RefCell::new(event_queue),
            globals: RefCell::new(globals),

            wayland_state: RefCell::new(wayland_state),
        };

        Ok(wayland_connection)
    }

    pub(crate) fn advise_of_appearance_change(&self, _appearance: crate::Appearance) {}

    fn run_message_loop_impl(&self) -> anyhow::Result<()> {
        const TOK_WL: usize = 0xffff_fffc;
        const TOK_SPAWN: usize = 0xffff_fffd;
        let tok_wl = Token(TOK_WL);
        let tok_spawn = Token(TOK_SPAWN);

        let mut poll = Poll::new()?;
        let mut events = Events::with_capacity(8);

        let wl_fd = {
            let read_guard = self.event_queue.borrow().prepare_read()?;
            read_guard.connection_fd().as_raw_fd()
        };

        poll.registry()
            .register(&mut SourceFd(&wl_fd), tok_wl, Interest::READABLE)?;
        poll.registry().register(
            &mut SourceFd(&SPAWN_QUEUE.raw_fd()),
            tok_spawn,
            Interest::READABLE,
        )?;

        // JUST Realized that the reason we need the spawn executor is so we can have tasks
        // scheduled (needed to open window)
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

                if let Ok(guard) = event_q.prepare_read() {
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

impl WaylandState {
    fn handle_window_event(&self, window: &Window, event: WindowEvent) {
        // TODO: XXX: should we grouping window data and connection
        let surface_data = SurfaceUserData::from_wl(window.wl_surface());
        let window_id = surface_data.window_id;
        let wconn = WaylandConnection::get()
            .expect("should be wayland connection")
            .wayland();
        let window_inner = wconn
            .window_by_id(window_id)
            .expect("Inner Window should exist");

        let p = window_inner.borrow().pending_event.clone();
        let mut pending_event = p.lock().unwrap();

        let changed = match event {
            WindowEvent::Close => {
                // TODO: This should the new queue function
                // p.queue_close()
                if !pending_event.close {
                    pending_event.close = true;
                    true
                } else {
                    false
                }
            }
            WindowEvent::Request(configure) => {
                // TODO: This should the new queue function
                // p.queue_configure(&configure)
                //
                let mut changed;
                pending_event.had_configure_event = true;
                if let (Some(w), Some(h)) = configure.new_size {
                    changed = pending_event.configure.is_none();
                    pending_event.configure.replace((w.get(), h.get()));
                } else {
                    changed = true;
                }

                let mut state = WindowState::default();
                if configure.state.contains(SCTKWindowState::FULLSCREEN) {
                    state |= WindowState::FULL_SCREEN;
                }
                let fs_bits = SCTKWindowState::MAXIMIZED
                    | SCTKWindowState::TILED_LEFT
                    | SCTKWindowState::TILED_RIGHT
                    | SCTKWindowState::TILED_TOP
                    | SCTKWindowState::TILED_BOTTOM;
                if !((configure.state & fs_bits).is_empty()) {
                    state |= WindowState::MAXIMIZED;
                }

                log::debug!(
                    "Config: self.window_state={:?}, states: {:?} {:?}",
                    pending_event.window_state,
                    state,
                    configure.state
                );

                if pending_event.window_state.is_none() && state != WindowState::default() {
                    changed = true;
                }

                pending_event.window_state.replace(state);
                changed
            }
        };
        if changed {
            WaylandConnection::with_window_inner(window_id, move |inner| {
                inner.dispatch_pending_event();
                Ok(())
            });
        }
    }
}

impl CompositorHandler for WaylandState {
    fn scale_factor_changed(
        &mut self,
        conn: &WConnection,
        qh: &wayland_client::QueueHandle<Self>,
        surface: &wayland_client::protocol::wl_surface::WlSurface,
        new_factor: i32,
    ) {
        // We do nothing, we get the scale_factor from surface_data
    }

    fn frame(
        &mut self,
        conn: &WConnection,
        qh: &wayland_client::QueueHandle<Self>,
        surface: &wayland_client::protocol::wl_surface::WlSurface,
        time: u32,
    ) {
        log::trace!("frame: CompositorHandler");
        let surface_data = SurfaceUserData::from_wl(surface);
        let window_id = surface_data.window_id;

        WaylandConnection::with_window_inner(window_id, |inner| {
            inner.next_frame_is_ready();
            Ok(())
        });
    }
}

impl OutputHandler for WaylandState {
    fn output_state(&mut self) -> &mut smithay_client_toolkit::output::OutputState {
        log::trace!("output state: OutputHandler");
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        conn: &WConnection,
        qh: &wayland_client::QueueHandle<Self>,
        output: wayland_client::protocol::wl_output::WlOutput,
    ) {
        log::trace!("new output: OutputHandler");
    }

    fn update_output(
        &mut self,
        conn: &WConnection,
        qh: &wayland_client::QueueHandle<Self>,
        output: wayland_client::protocol::wl_output::WlOutput,
    ) {
        log::trace!("update output: OutputHandler");
        todo!()
    }

    fn output_destroyed(
        &mut self,
        conn: &WConnection,
        qh: &wayland_client::QueueHandle<Self>,
        output: wayland_client::protocol::wl_output::WlOutput,
    ) {
        log::trace!("output destroyed: OutputHandler");
        todo!()
    }
}

enum WindowEvent {
    Close,
    Request(WindowConfigure),
}

impl WindowHandler for WaylandState {
    fn request_close(
        &mut self,
        _conn: &WConnection,
        _qh: &wayland_client::QueueHandle<Self>,
        window: &Window,
    ) {
        self.handle_window_event(window, WindowEvent::Close);
    }

    fn configure(
        &mut self,
        _conn: &WConnection,
        _qh: &wayland_client::QueueHandle<Self>,
        window: &Window,
        configure: WindowConfigure,
        _serial: u32,
    ) {
        self.handle_window_event(window, WindowEvent::Request(configure));
    }
}

impl ShmHandler for WaylandState {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
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
        self.windows.borrow_mut().clear();
        res
    }

    fn screens(&self) -> anyhow::Result<crate::screen::Screens> {
        // TODO: implement for the outputhandler
        // if let Some(screens) = self
        //     .environment
        //     .with_inner(|env| env.output_handler.screens())
        // {
        //     return Ok(screens);
        // }
        //

        log::trace!("Getting screens for wayland connection");
        let mut by_name = HashMap::new();
        let mut virtual_rect: ScreenRect = euclid::rect(0, 0, 0, 0);
        let config = config::configuration();

        let output_state = &self.wayland_state.borrow().output_state;

        for output in output_state.outputs() {
            let info = output_state.info(&output).unwrap();
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

// Undocumented in sctk 0.17: This is required to use have user data with a surface
// Will be just delegate_compositor!(WaylandState, surface: [SurfaceData, SurfaceUserData]) in 0.18
wayland_client::delegate_dispatch!(WaylandState: [ WlSurface: SurfaceUserData] => CompositorState);
delegate_compositor!(WaylandState);

delegate_output!(WaylandState);

delegate_xdg_shell!(WaylandState);
delegate_xdg_window!(WaylandState);

delegate_shm!(WaylandState);

delegate_registry!(WaylandState);

impl ProvidesRegistryState for WaylandState {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers!(OutputState);
}
