use failure::Error;
use futures::sync::oneshot;
use guiloop::GuiSender;
use mio::unix::EventedFd;
use mio::{Event, Evented, Events, Poll, PollOpt, Ready, Token};
use mio_extras::channel::{channel as mio_channel, Receiver as MioReceiver, Sender as MioSender};
use std::collections::HashMap;
use std::io;
use std::os::unix::io::RawFd;
use std::sync::mpsc::TryRecvError;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

enum Request {
    Register {
        fd: RawFd,
        token: Token,
        interest: Ready,
        opts: PollOpt,
        done: oneshot::Sender<io::Result<()>>,
    },
    Deregister {
        fd: RawFd,
        done: oneshot::Sender<io::Result<()>>,
    },
}

pub enum Notification {
    EventReady(Event),
    IntervalDone,
}

struct Fd {
    fd: RawFd,
}

impl Evented for Fd {
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        EventedFd(&self.fd).register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        EventedFd(&self.fd).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        EventedFd(&self.fd).deregister(poll)
    }
}

struct Inner {
    rx: MioReceiver<Request>,
    interval: Duration,
    wakeup: GuiSender<Notification>,
    fd_map: HashMap<RawFd, Arc<Fd>>,
}

/// The `IOMgr` represents a mio `Poll` instance that is driven by a separate
/// thread.  Unix/X11 systems don't really need this to be in a separate
/// thread, but for the sake of minimizing platform differences we do use the
/// same approach for all platforms.  `IOMgr` offers `register` and `deregister`
/// methods that are similar to their namesakes in `Poll`, except that `IOMgr`
/// returns `Future` instances that can be used to asynchronously handle the
/// result of those operations.
pub struct IOMgr {
    tx: MioSender<Request>,
}

impl IOMgr {
    pub fn new(interval: Duration, wakeup: GuiSender<Notification>) -> Self {
        let (tx, rx) = mio_channel();
        let mut inner = Inner {
            rx,
            interval,
            wakeup,
            fd_map: HashMap::new(),
        };
        thread::spawn(move || match inner.run() {
            Ok(_) => eprintln!("IOMgr thread completed with success"),
            Err(err) => eprintln!("IOMgr thread failed: {:?}", err),
        });
        Self { tx }
    }

    pub fn register(
        &self,
        fd: RawFd,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> Result<oneshot::Receiver<Result<(), io::Error>>, Error> {
        let (done, rx) = oneshot::channel();
        self.tx.send(Request::Register {
            fd,
            token,
            interest,
            opts,
            done,
        })?;
        Ok(rx)
    }

    pub fn deregister(&self, fd: RawFd) -> Result<oneshot::Receiver<Result<(), io::Error>>, Error> {
        let (done, rx) = oneshot::channel();
        self.tx.send(Request::Deregister { fd, done })?;
        Ok(rx)
    }
}

impl Inner {
    fn dereg(&mut self, poll: &Poll, fd: RawFd) -> Result<(), io::Error> {
        let evented = self
            .fd_map
            .get(&fd)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::Other,
                    format!("fd {} is not present in IOMgr fd_map", fd),
                )
            })?
            .clone();

        poll.deregister(&*evented)?;

        self.fd_map.remove(&fd);
        Ok(())
    }

    fn run(&mut self) -> Result<(), Error> {
        let poll = Poll::new()?;
        poll.register(&self.rx, Token(0), Ready::readable(), PollOpt::level())?;

        let mut events = Events::with_capacity(8);
        let mut last_interval = Instant::now();

        loop {
            let now = Instant::now();
            let diff = now - last_interval;
            let period = if diff >= self.interval {
                self.wakeup.send(Notification::IntervalDone)?;
                last_interval = now;
                self.interval
            } else {
                self.interval - diff
            };

            if poll.poll(&mut events, Some(period)).is_ok() {
                for event in &events {
                    if event.token() == Token(0) {
                        match self.rx.try_recv() {
                            Err(TryRecvError::Empty) => {}
                            Err(err) => bail!("IOMgr: disconnected {:?}", err),
                            Ok(Request::Register {
                                fd,
                                token,
                                interest,
                                opts,
                                done,
                            }) => {
                                let evented = Arc::new(Fd { fd });
                                self.fd_map.insert(fd, evented.clone());

                                match done.send(poll.register(&*evented, token, interest, opts)) {
                                    Ok(_) => {}
                                    Err(err) => eprintln!("done channel went away {:?}", err),
                                };
                            }
                            Ok(Request::Deregister { fd, done }) => {
                                match done.send(self.dereg(&poll, fd)) {
                                    Ok(_) => eprintln!("deregistered fd {}", fd),
                                    Err(err) => eprintln!("done channel went away {:?}", err),
                                };
                            }
                        }
                    } else {
                        self.wakeup.send(Notification::EventReady(event))?;
                    }
                }
            }
        }
    }
}
