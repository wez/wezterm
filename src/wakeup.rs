use failure::Error;
use glium::glutin::EventsLoopProxy;
use std::sync::mpsc::{channel, Receiver, Sender};

#[derive(Debug)]
pub enum WakeupMsg {
    PtyReadable,
    SigChld,
    Paint,
}

#[derive(Clone)]
pub struct Wakeup {
    sender: Sender<WakeupMsg>,
    proxy: EventsLoopProxy,
}

impl Wakeup {
    pub fn new(proxy: EventsLoopProxy) -> (Receiver<WakeupMsg>, Self) {
        let (sender, receiver) = channel();
        (receiver, Self { sender, proxy })
    }
    pub fn send(&mut self, what: WakeupMsg) -> Result<(), Error> {
        self.sender.send(what)?;
        self.proxy.wakeup()?;
        Ok(())
    }
}
