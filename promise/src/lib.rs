use anyhow::Error;
use smol::channel::{bounded, Receiver, Sender};
use std::pin::Pin;
use std::task::{Context, Poll};
use thiserror::*;

pub mod spawn;
pub type SpawnFunc = Box<dyn FnOnce() + Send>;

#[derive(Debug, Error)]
#[error("Promise was dropped before completion")]
pub struct BrokenPromise {}

pub struct Promise<T> {
    tx: Option<Sender<anyhow::Result<T>>>,
    rx: Option<Receiver<anyhow::Result<T>>>,
}

pub struct Future<T> {
    rx: Receiver<anyhow::Result<T>>,
}

impl<T> Default for Promise<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Promise<T> {
    pub fn new() -> Self {
        let (tx, rx) = bounded(1);
        Self {
            tx: Some(tx),
            rx: Some(rx),
        }
    }

    pub fn get_future(&mut self) -> Option<Future<T>> {
        self.rx.take().map(|rx| Future { rx })
    }

    pub fn ok(&mut self, value: T) {
        self.result(Ok(value));
    }

    pub fn err(&mut self, err: Error) {
        self.result(Err(err));
    }

    pub fn result(&mut self, result: Result<T, Error>) {
        self.tx
            .take()
            .expect("Promise already fulfilled")
            .try_send(result)
            .ok();
    }
}

impl<T: Send + 'static> Future<T> {
    /// Create a leaf future which is immediately ready with
    /// the provided value
    pub fn ok(value: T) -> Self {
        Self::result(Ok(value))
    }

    /// Create a leaf future which is immediately ready with
    /// the provided error
    pub fn err(err: Error) -> Self {
        Self::result(Err(err))
    }

    /// Create a leaf future which is immediately ready with
    /// the provided result
    pub fn result(result: Result<T, Error>) -> Self {
        let mut promise = Promise::new();
        let future = promise.get_future().unwrap();
        promise.result(result);
        future
    }
}

impl<T: Send + 'static> std::future::Future for Future<T> {
    type Output = Result<T, Error>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let rx = unsafe { &mut self.get_unchecked_mut().rx };
        let f = rx.recv();
        smol::pin!(f);
        match f.poll(ctx) {
            Poll::Ready(Ok(res)) => Poll::Ready(res),
            Poll::Ready(Err(_)) => Poll::Ready(Err(BrokenPromise {}.into())),
            Poll::Pending => Poll::Pending,
        }
    }
}
