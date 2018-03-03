use failure::Error;
use glium::glutin::EventsLoopProxy;
use std::sync::mpsc::{self, Receiver, Sender};

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
