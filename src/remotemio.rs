use failure::{err_msg, Error};
use futures::sync::oneshot;
use mio::{Event, Events, Poll, PollOpt, Ready, Token};
use mio::unix::EventedFd;
use mio_extras::channel::{channel as mio_channel, Receiver as MioReceiver, Sender as MioSender};
use std::io;
use std::os::unix::io::RawFd;
use std::sync::mpsc::TryRecvError;
use std::thread;
use std::time::{Duration, Instant};
use wakeup;

pub enum Request {
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

struct Inner {
    rx: MioReceiver<Request>,
    interval: Duration,
    wakeup: wakeup::GuiSender<Notification>,
}

pub struct IOMgr {
    tx: MioSender<Request>,
}

fn done_broke<T>(_: T) -> Error {
    err_msg("done channel broken")
}

impl IOMgr {
    pub fn new(interval: Duration, wakeup: wakeup::GuiSender<Notification>) -> Self {
        let (tx, rx) = mio_channel();
        let inner = Inner {
            rx,
            interval,
            wakeup,
        };
        thread::spawn(move || inner.run());
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
    fn run(&self) -> Result<(), Error> {
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

            match poll.poll(&mut events, Some(period)) {
                Ok(_) => for event in &events {
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
                                done.send(poll.register(&EventedFd(&fd), token, interest, opts))
                                    .map_err(done_broke)?;
                            }
                            Ok(Request::Deregister { fd, done }) => {
                                done.send(poll.deregister(&EventedFd(&fd)))
                                    .map_err(done_broke)?;
                            }
                        }
                    } else {
                        self.wakeup.send(Notification::EventReady(event))?;
                    }
                },
                _ => {}
            }
        }
    }
}
