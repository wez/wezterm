use failure::Error;
use glium::glutin::{EventsLoopProxy, WindowId};
use std::sync::mpsc::{self, Receiver, Sender};

#[derive(Debug)]
pub enum WakeupMsg {
    PtyReadable(WindowId),
    SigChld,
    Paint,
    Paste(WindowId),
}

#[derive(Clone)]
pub struct Wakeup {
    sender: Sender<WakeupMsg>,
    proxy: EventsLoopProxy,
}

impl Wakeup {
    pub fn new(proxy: EventsLoopProxy) -> (Receiver<WakeupMsg>, Self) {
        let (sender, receiver) = mpsc::channel();
        (receiver, Self { sender, proxy })
    }
    pub fn send(&mut self, what: WakeupMsg) -> Result<(), Error> {
        self.sender.send(what)?;
        self.proxy.wakeup()?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct GuiSender<T: Send> {
    tx: Sender<T>,
    proxy: EventsLoopProxy,
}

impl<T: Send> GuiSender<T> {
    pub fn send(&self, what: T) -> Result<(), Error> {
        match self.tx.send(what) {
            Ok(_) => {}
            Err(err) => bail!("send failed: {:?}", err),
        };
        self.proxy.wakeup()?;
        Ok(())
    }
}

pub fn channel<T: Send>(proxy: EventsLoopProxy) -> (GuiSender<T>, Receiver<T>) {
    let (tx, rx) = mpsc::channel();
    (GuiSender { tx, proxy }, rx)
}
