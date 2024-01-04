// TODO: change this
#![allow(dead_code, unused)]
use std::{borrow::BorrowMut, cell::RefCell};

use anyhow::Context;
use smithay_client_toolkit::{
    delegate_registry,
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
};
use wayland_client::{globals::registry_queue_init, Connection, EventQueue};

use crate::ConnectionOps;

pub struct WaylandConnection {
    event_queue: RefCell<EventQueue<WaylandState>>,
    wayland_state: RefCell<WaylandState>,
}

struct WaylandState {
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
            event_queue: RefCell::new(event_queue),
            wayland_state: RefCell::new(wayland_state),
        };

        Ok(wayland_connection)
    }

    pub(crate) fn advise_of_appearance_change(&self, appearance: crate::Appearance) {}
}

impl ProvidesRegistryState for WaylandState {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers!();
}

impl ConnectionOps for WaylandConnection {
    fn name(&self) -> String {
        todo!()
    }

    fn terminate_message_loop(&self) {
        todo!()
    }

    fn run_message_loop(&self) -> anyhow::Result<()> {
        loop {
            let mut event_q = self.event_queue.borrow_mut();
            let mut wayland_state = self.wayland_state.borrow_mut();
            if let Err(err) = event_q.dispatch_pending(&mut wayland_state) {
                // TODO: show the protocol error in the display
                return Err(err)
                    .with_context(|| format!("error during event_q.dispatch protcol_error"));
            }
        }
        Ok(())
    }
}

delegate_registry!(WaylandState);
