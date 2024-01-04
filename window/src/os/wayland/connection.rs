// TODO: change this
#![allow(dead_code, unused)]
use crate::ConnectionOps;

pub struct WaylandConnection {}

impl WaylandConnection {
    pub(crate) fn create_new() -> anyhow::Result<Self> {
        Ok( WaylandConnection{} )
    }

    pub(crate) fn advise_of_appearance_change(&self, appearance: crate::Appearance) {}
}

impl ConnectionOps for WaylandConnection {
    fn name(&self) -> String {
        todo!()
    }

    fn terminate_message_loop(&self) {
        todo!()
    }

    fn run_message_loop(&self) -> anyhow::Result<()> {
        todo!()
    }
}
