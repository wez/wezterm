use failure::{Error, Fallible};
use failure_derive::*;
use std::sync::{Arc, Condvar, Mutex};

type NextFunc<T> = Box<dyn FnOnce(Fallible<T>) + Send>;
pub type SpawnFunc = Box<dyn FnOnce() + Send>;

#[derive(Debug, Fail)]
#[fail(display = "Promise was dropped before completion")]
pub struct BrokenPromise {}

pub trait Executor: Send {
    fn execute(&self, f: SpawnFunc);
    fn clone_executor(&self) -> Box<dyn Executor>;
}

impl Executor for Box<dyn Executor> {
    fn execute(&self, f: SpawnFunc) {
        Executor::execute(&**self, f)
    }
    fn clone_executor(&self) -> Box<dyn Executor> {
        Executor::clone_executor(&**self)
    }
}

/// An executor for spawning futures into the rayon global
/// thread pool
#[derive(Clone, Default)]
pub struct RayonExecutor {}

impl RayonExecutor {
    pub fn new() -> Self {
        Self {}
    }
}

impl Executor for RayonExecutor {
    fn execute(&self, f: SpawnFunc) {
        rayon::spawn(f);
    }
    fn clone_executor(&self) -> Box<dyn Executor> {
        Box::new(RayonExecutor::new())
    }
}

enum PromiseState<T> {
    Waiting(Arc<Core<T>>),
    Fulfilled,
}

enum FutureState<T> {
    Waiting(Arc<Core<T>>),
    Ready(Result<T, Error>),
}

struct CoreData<T> {
    result: Option<Result<T, Error>>,
    propagate: Option<NextFunc<T>>,
}

struct Core<T> {
    data: Mutex<CoreData<T>>,
    cond: Condvar,
}

pub struct Promise<T> {
    state: PromiseState<T>,
    future: Option<Future<T>>,
}

pub struct Future<T> {
    state: FutureState<T>,
}

impl<T> Default for Promise<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Drop for Promise<T> {
    fn drop(&mut self) {
        if let PromiseState::Waiting(core) = &mut self.state {
            let err = Err(BrokenPromise {}.into());
            let mut locked = core.data.lock().unwrap();
            if let Some(func) = locked.propagate.take() {
                func(err);
            } else {
                locked.result = Some(err);
            }
            core.cond.notify_one();
        }
    }
}

impl<T> Promise<T> {
    pub fn new() -> Self {
        let core = Arc::new(Core {
            data: Mutex::new(CoreData {
                result: None,
                propagate: None,
            }),
            cond: Condvar::new(),
        });

        Self {
            state: PromiseState::Waiting(Arc::clone(&core)),
            future: Some(Future {
                state: FutureState::Waiting(core),
            }),
        }
    }

    pub fn get_future(&mut self) -> Option<Future<T>> {
        self.future.take()
    }

    pub fn ok(&mut self, value: T) {
        self.result(Ok(value));
    }

    pub fn err(&mut self, err: Error) {
        self.result(Err(err));
    }

    pub fn result(&mut self, result: Result<T, Error>) {
        match std::mem::replace(&mut self.state, PromiseState::Fulfilled) {
            PromiseState::Waiting(core) => {
                let mut locked = core.data.lock().unwrap();
                match locked.propagate.take() {
                    Some(func) => func(result),
                    None => locked.result = Some(result),
                }
                core.cond.notify_one();
            }
            PromiseState::Fulfilled => panic!("Promise already fulfilled"),
        }
    }
}

impl<T: Send + 'static> std::convert::From<Result<T, Error>> for Future<T> {
    fn from(result: Result<T, Error>) -> Future<T> {
        Future::result(result)
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
            state: FutureState::Ready(result),
        }
    }

    /// Create a future from a function that will be spawned via
    /// the provided executor
    pub fn with_executor<F, IF, EXEC>(executor: EXEC, f: F) -> Future<T>
    where
        F: FnOnce() -> IF,
        IF: Into<Future<T>>,
        IF: 'static,
        F: Send + 'static,
        EXEC: Executor + Send + 'static,
    {
        let mut promise = Promise::new();
        let future = promise.get_future().unwrap();

        let func = Box::new(f);
        let promise_chain = Box::new(move |result| promise.result(result));
        executor.execute(Box::new(move || {
            let future = func().into();
            future.chain(promise_chain);
        }));
        future
    }

    fn chain(self, f: NextFunc<T>) {
        match self.state {
            FutureState::Ready(result) => {
                f(result);
            }
            FutureState::Waiting(core) => {
                let mut locked = core.data.lock().unwrap();
                if let Some(result) = locked.result.take() {
                    f(result);
                } else {
                    locked.propagate = Some(f);
                }
            }
        }
    }

    /// Blocks until the associated promise is fulfilled
    pub fn wait(self) -> Result<T, Error> {
        match self.state {
            FutureState::Waiting(core) => {
                let mut locked = core.data.lock().unwrap();
                loop {
                    if let Some(result) = locked.result.take() {
                        return result;
                    }
                    locked = core.cond.wait(locked).unwrap();
                }
            }
            FutureState::Ready(result) => result,
        }
    }

    pub fn is_ready(&self) -> bool {
        match &self.state {
            FutureState::Waiting(core) => {
                let locked = core.data.lock().unwrap();
                locked.result.is_some()
            }
            FutureState::Ready(_) => true,
        }
    }

    /// When this future resolves, then map the result via the
    /// supplied lambda, which returns something that is convertible
    /// to a Future.
    pub fn then<U, F, IF>(self, f: F) -> Future<U>
    where
        F: FnOnce(Result<T, Error>) -> IF,
        IF: Into<Future<U>>,
        IF: 'static,
        F: Send + 'static,
        U: Send + 'static,
    {
        let mut promise = Promise::new();
        let future = promise.get_future().unwrap();
        let func = Box::new(f);

        let promise_chain = Box::new(move |result| promise.result(result));

        self.chain(Box::new(move |result| {
            let future = func(result).into();
            future.chain(promise_chain);
        }));
        future
    }

    /// When this future resolves successfully, map the result via
    /// the supplied lambda, which returns something that is convertible
    /// to a Future.
    /// When this future resolves with an error, the error is propagated
    /// along as the error value of the returned future.
    pub fn map<U, F, IF>(self, f: F) -> Future<U>
    where
        F: FnOnce(T) -> IF,
        IF: Into<Future<U>>,
        IF: 'static,
        F: Send + 'static,
        U: Send + 'static,
    {
        let mut promise = Promise::new();
        let future = promise.get_future().unwrap();
        let func = Box::new(f);

        let promise_chain = Box::new(move |result| promise.result(result));

        self.chain(Box::new(move |result| {
            let future = match result {
                Ok(value) => func(value).into(),
                Err(err) => Err(err).into(),
            };
            future.chain(promise_chain);
        }));
        future
    }

    /// When this future resolves with an error, map the error result
    /// via the supplied lambda, with returns something that is convertible
    /// to a Future.
    /// When this future resolves successfully, the value is propagated
    /// along as the Ok value of the returned future.
    pub fn map_err<F, IF>(self, f: F) -> Future<T>
    where
        F: FnOnce(Error) -> IF,
        IF: Into<Future<T>>,
        IF: 'static,
        F: Send + 'static,
    {
        let mut promise = Promise::new();
        let future = promise.get_future().unwrap();
        let func = Box::new(f);

        let promise_chain = Box::new(move |result| promise.result(result));

        self.chain(Box::new(move |result| {
            let future = match result {
                Ok(value) => Ok(value).into(),
                Err(err) => func(err).into(),
            };
            future.chain(promise_chain);
        }));
        future
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use failure::{err_msg, format_err};
    #[test]
    fn basic_promise() {
        let mut p = Promise::new();
        p.ok(true);
        assert_eq!(p.get_future().unwrap().wait().unwrap(), true);
    }

    #[test]
    fn basic_promise_future_first() {
        let mut p = Promise::new();
        let f = p.get_future().unwrap();
        p.ok(true);
        assert_eq!(f.wait().unwrap(), true);
    }

    #[test]
    fn promise_chain() {
        let mut p = Promise::new();
        let f = p
            .get_future()
            .unwrap()
            .then(|result| Ok(result.unwrap() + 1))
            .then(|result| Ok(result.unwrap() + 3));
        p.ok(1);
        assert_eq!(f.wait().unwrap(), 5);
    }

    #[test]
    fn promise_map() {
        let mut p = Promise::new();
        let f = p.get_future().unwrap().map(|value| Ok(value + 1));
        p.ok(1);
        assert_eq!(f.wait().unwrap(), 2);
    }

    #[test]
    fn promise_map_err() {
        let mut p = Promise::new();
        let f: Future<usize> = p
            .get_future()
            .unwrap()
            .map(|_value| Err(err_msg("boo")))
            .map_err(|err| Err(format_err!("whoops: {}", err)));
        p.ok(1);
        assert_eq!(format!("{}", f.wait().unwrap_err()), "whoops: boo");
    }

    #[test]
    fn promise_chain_future() {
        let mut p = Promise::new();
        let f = p
            .get_future()
            .unwrap()
            .then(|result| Future::ok(result.unwrap() + 1))
            .then(|result| Ok(result.unwrap() + 3));
        p.ok(1);
        assert_eq!(f.wait().unwrap(), 5);
    }

    #[test]
    fn promise_thread() {
        let mut p = Promise::new();
        let f = p.get_future().unwrap();

        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::new(0, 500));
            p.ok(123);
        });

        let f2 = f.then(move |result| Ok(result.unwrap() * 2));

        assert_eq!(f2.wait().unwrap(), 246);
    }

    #[test]
    fn promise_thread_slow_chain() {
        let mut p = Promise::new();
        let f = p.get_future().unwrap();

        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::new(0, 500));
            p.ok(123);
        });

        std::thread::sleep(std::time::Duration::new(1, 0));

        let f2 = f.then(move |result| Ok(result.unwrap() * 2));

        assert_eq!(f2.wait().unwrap(), 246);
    }

    #[test]
    fn via_rayon() {
        let f = Future::with_executor(RayonExecutor::new(), || Ok(true));
        assert_eq!(f.wait().unwrap(), true);
    }
}
