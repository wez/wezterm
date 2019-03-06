use crate::config::Config;
use crate::mux::Mux;
use crate::server::{UnixListener, UnixStream};
use failure::Error;
use std::io::{Read, Write};
use std::rc::Rc;
use std::sync::Arc;

pub trait SocketLike: Read + Write + Send {}

pub trait Acceptor {
    fn accept(&self) -> Result<Box<SocketLike>, Error>;
}

impl SocketLike for UnixStream {}

impl Acceptor for UnixListener {
    fn accept(&self) -> Result<Box<SocketLike>, Error> {
        let (stream, _addr) = UnixListener::accept(self)?;
        let timeout = std::time::Duration::new(60, 0);
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        Ok(Box::new(stream))
    }
}

pub struct Listener {
    acceptor: Box<Acceptor>,
}

impl Listener {
    pub fn new(acceptor: Box<Acceptor>) -> Self {
        Self { acceptor }
    }
}

pub struct ClientSession {}

pub fn run_mux_server(config: Arc<Config>) -> Result<(), Error> {
    let mux = Rc::new(Mux::default());
    Mux::set_mux(&mux);

    Ok(())
}
