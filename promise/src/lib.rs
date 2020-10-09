use anyhow::Error;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use thiserror::*;

pub mod spawn;

#[derive(Debug, Error)]
#[error("Promise was dropped before completion")]
pub struct BrokenPromise {}

#[derive(Debug)]
struct Core<T> {
    result: Option<anyhow::Result<T>>,
    waker: Option<Waker>,
}

pub struct Promise<T> {
    core: Arc<Mutex<Core<T>>>,
}

#[derive(Debug)]
pub struct Future<T> {
    core: Arc<Mutex<Core<T>>>,
}

impl<T> Default for Promise<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Promise<T> {
    pub fn new() -> Self {
        Self {
            core: Arc::new(Mutex::new(Core {
                result: None,
                waker: None,
            })),
        }
    }

    pub fn get_future(&mut self) -> Option<Future<T>> {
        Some(Future {
            core: Arc::clone(&self.core),
        })
    }

    pub fn ok(&mut self, value: T) -> bool {
        self.result(Ok(value))
    }

    pub fn err(&mut self, err: Error) -> bool {
        self.result(Err(err))
    }

    pub fn result(&mut self, result: Result<T, Error>) -> bool {
        let mut core = self.core.lock().unwrap();
        core.result.replace(result);
        if let Some(waker) = core.waker.take() {
            waker.wake();
        }
        true
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
        Self {
            core: Arc::new(Mutex::new(Core {
                result: Some(result),
                waker: None,
            })),
        }
    }
}

impl<T: Send + 'static> std::future::Future for Future<T> {
    type Output = Result<T, Error>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let waker = ctx.waker().clone();

        let mut core = self.core.lock().unwrap();
        if let Some(result) = core.result.take() {
            Poll::Ready(result)
        } else {
            core.waker.replace(waker);
            Poll::Pending
        }
    }
}
