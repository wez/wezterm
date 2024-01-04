// TODO: change this
#![allow(dead_code, unused)]
use std::{borrow::BorrowMut, cell::RefCell, os::fd::AsRawFd, sync::atomic::AtomicUsize};

use anyhow::{Context, bail};
use mio::{unix::SourceFd, Events, Interest, Poll, Token};
use smithay_client_toolkit::{
    delegate_registry,
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers, delegate_compositor, compositor::CompositorHandler, output::OutputHandler, delegate_xdg_shell, delegate_xdg_window, shell::xdg::window::WindowHandler,
};
use wayland_client::{globals::{registry_queue_init, GlobalList}, Connection, EventQueue, backend::{protocol::ProtocolError, WaylandError}};

use crate::{spawn::SPAWN_QUEUE, ConnectionOps};

pub struct WaylandConnection {
    pub(crate) should_terminate: RefCell<bool>,
    pub(crate) next_window_id: AtomicUsize,

    pub(crate) event_queue: RefCell<EventQueue<WaylandState>>,
    pub(crate) globals: RefCell<GlobalList>,

    pub(crate) wayland_state: RefCell<WaylandState>,
}

pub (crate) struct WaylandState {
    registry_state: RegistryState,
}

impl WaylandConnection {
    pub(crate) fn create_new() -> anyhow::Result<Self> {
        let conn = Connection::connect_to_env()?;
        let (globals, mut event_queue) = registry_queue_init::<WaylandState>(&conn)?;
        let qh = event_queue.handle();

        let wayland_state = WaylandState {
            registry_state: RegistryState::new(&globals),
        };
        let wayland_connection = WaylandConnection {
            should_terminate: RefCell::new(false),
            next_window_id: AtomicUsize::new(1),

            event_queue: RefCell::new(event_queue),
            globals: RefCell::new(globals),

            wayland_state: RefCell::new(wayland_state),
        };

        Ok(wayland_connection)
    }

    pub(crate) fn advise_of_appearance_change(&self, appearance: crate::Appearance) {}

    fn run_message_loop_impl(&self) -> anyhow::Result<()> {
        const TOK_WL: usize = 0xffff_fffc;
        const TOK_SPAWN: usize = 0xffff_fffd;
        let tok_wl = Token(TOK_WL);
        let tok_spawn = Token(TOK_SPAWN);

        let mut poll = Poll::new()?;
        let mut events = Events::with_capacity(8);

        let read_guard = self.event_queue.borrow().prepare_read()?;
        let wl_fd = read_guard.connection_fd();

        poll.registry().register(
            &mut SourceFd(&wl_fd.as_raw_fd()),
            tok_wl,
            Interest::READABLE,
        )?;
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

                let a = event_q.prepare_read();
                if let Ok(guard) = event_q.prepare_read() {
                    if let Err(err) = guard.read() {
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
}

impl ProvidesRegistryState for WaylandState {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers!();
}

impl CompositorHandler for WaylandState {
    fn scale_factor_changed(
        &mut self,
        conn: &Connection,
        qh: &wayland_client::QueueHandle<Self>,
        surface: &wayland_client::protocol::wl_surface::WlSurface,
        new_factor: i32,
    ) {
        todo!()
    }

    fn frame(
        &mut self,
        conn: &Connection,
        qh: &wayland_client::QueueHandle<Self>,
        surface: &wayland_client::protocol::wl_surface::WlSurface,
        time: u32,
    ) {
        todo!()
    }
}

impl OutputHandler for WaylandState {
    fn output_state(&mut self) -> &mut smithay_client_toolkit::output::OutputState {
        todo!()
    }

    fn new_output(
        &mut self,
        conn: &Connection,
        qh: &wayland_client::QueueHandle<Self>,
        output: wayland_client::protocol::wl_output::WlOutput,
    ) {
        todo!()
    }

    fn update_output(
        &mut self,
        conn: &Connection,
        qh: &wayland_client::QueueHandle<Self>,
        output: wayland_client::protocol::wl_output::WlOutput,
    ) {
        todo!()
    }

    fn output_destroyed(
        &mut self,
        conn: &Connection,
        qh: &wayland_client::QueueHandle<Self>,
        output: wayland_client::protocol::wl_output::WlOutput,
    ) {
        todo!()
    }
}

impl WindowHandler for WaylandState {
    fn request_close(&mut self, conn: &Connection, qh: &wayland_client::QueueHandle<Self>, window: &smithay_client_toolkit::shell::xdg::window::Window) {
        todo!()
    }

    fn configure(
        &mut self,
        conn: &Connection,
        qh: &wayland_client::QueueHandle<Self>,
        window: &smithay_client_toolkit::shell::xdg::window::Window,
        configure: smithay_client_toolkit::shell::xdg::window::WindowConfigure,
        serial: u32,
    ) {
        todo!()
    }
}

impl ConnectionOps for WaylandConnection {
    fn name(&self) -> String {
        "Wayland".to_string()
    }

    fn terminate_message_loop(&self) {
        todo!()
    }

    fn run_message_loop(&self) -> anyhow::Result<()> {
        // TODO: match
        self.run_message_loop_impl()
    }
}

delegate_xdg_shell!(WaylandState);
delegate_xdg_window!(WaylandState);
delegate_compositor!(WaylandState);
delegate_registry!(WaylandState);
