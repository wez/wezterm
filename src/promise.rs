use boxfnonce::SendBoxFnOnce;
use failure::Error;
use std::sync::{Arc, Condvar, Mutex};

type NextFunc<T> = SendBoxFnOnce<'static, (Result<T, Error>,)>;

enum PromiseState<T> {
    Waiting(Arc<Core<T>>),
    Fulfilled,
}

enum FutureState<T> {
    Waiting(Arc<Core<T>>),
    Ready(Result<T, Error>),
    Done,
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
                    Some(func) => func.call(result),
                    None => locked.result = Some(result),
                }
                core.cond.notify_one();
            }
            PromiseState::Fulfilled => panic!("Promise already fulfilled"),
        }
    }
}

impl<T: 'static> std::convert::From<Result<T, Error>> for Future<T> {
    fn from(result: Result<T, Error>) -> Future<T> {
        Future::result(result)
    }
}

impl<T: 'static> Future<T> {
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

    fn chain(self, f: NextFunc<T>) {
        match self.state {
            FutureState::Done => panic!("chaining an already done future"),
            FutureState::Ready(result) => {
                f.call(result);
            }
            FutureState::Waiting(core) => {
                let mut locked = core.data.lock().unwrap();
                if let Some(result) = locked.result.take() {
                    f.call(result);
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
            FutureState::Done => bail!("Future is already done"),
        }
    }

    pub fn then<U, F, IF>(self, f: F) -> Future<U>
    where
        F: FnOnce(Result<T, Error>) -> IF,
        IF: Into<Future<U>>,
        IF: 'static,
        F: Send + 'static,
        U: Send + 'static,
    {
        let mut promise = Promise::<U>::new();
        let future = promise.get_future().unwrap();
        let func = SendBoxFnOnce::from(f);

        let promise_chain = NextFunc::from(move |result| promise.result(result));

        self.chain(SendBoxFnOnce::from(move |result| {
            let future = func.call(result).into();
            future.chain(promise_chain);
        }));
        future
    }
}

#[cfg(test)]
mod test {
    use super::*;
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
}
