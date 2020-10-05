use crate::SpawnFunc;
use anyhow::{anyhow, Result};
use async_task::Runnable;
use std::future::Future;
use std::sync::mpsc::{sync_channel, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::task::{Poll, Waker};

pub use async_task::Task;
pub type ScheduleFunc = Box<dyn Fn(Runnable) + Send + Sync + 'static>;

fn no_schedule_configured(_: Runnable) {
    panic!("no scheduler has been configured");
}

lazy_static::lazy_static! {
    static ref ON_MAIN_THREAD: Mutex<ScheduleFunc> = Mutex::new(Box::new(no_schedule_configured));
    static ref ON_MAIN_THREAD_LOW_PRI: Mutex<ScheduleFunc> = Mutex::new(Box::new(no_schedule_configured));
}

/// Set callbacks for scheduling normal and low priority futures.
/// Why this and not "just tokio"?  In a GUI application there is typically
/// a special GUI processing loop that may need to run on the "main thread",
/// so we can't just run a tokio/mio loop in that context.
/// This particular crate has no real knowledge of how that plumbing works,
/// it just provides the abstraction for scheduling the work.
/// This function allows the embedding application to set that up.
pub fn set_schedulers(main: ScheduleFunc, low_pri: ScheduleFunc) {
    *ON_MAIN_THREAD.lock().unwrap() = Box::new(main);
    *ON_MAIN_THREAD_LOW_PRI.lock().unwrap() = Box::new(low_pri);
}

/// Spawn a new thread to execute the provided function.
/// Returns a JoinHandle that implements the Future trait
/// and that can be used to await and yield the return value
/// from the thread.
/// Can be called from any thread.
pub fn spawn_into_new_thread<F, T>(f: F) -> Task<Result<T>>
where
    F: FnOnce() -> Result<T>,
    F: Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = sync_channel(1);

    // Holds the waker that may later observe
    // during the Future::poll call.
    struct WakerHolder {
        waker: Mutex<Option<Waker>>,
    }

    let holder = Arc::new(WakerHolder {
        waker: Mutex::new(None),
    });

    let thread_waker = Arc::clone(&holder);
    std::thread::spawn(move || {
        // Run the thread
        let res = f();
        // Pass the result back
        tx.send(res).unwrap();
        // If someone polled the thread before we got here,
        // they will have populated the waker; extract it
        // and wake up the scheduler so that it will poll
        // the result again.
        let mut waker = thread_waker.waker.lock().unwrap();
        if let Some(waker) = waker.take() {
            waker.wake();
        }
    });

    struct PendingResult<T> {
        rx: Receiver<Result<T>>,
        holder: Arc<WakerHolder>,
    }

    impl<T> std::future::Future for PendingResult<T> {
        type Output = Result<T>;

        fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context) -> Poll<Self::Output> {
            match self.rx.try_recv() {
                Ok(res) => Poll::Ready(res),
                Err(TryRecvError::Empty) => {
                    let mut waker = self.holder.waker.lock().unwrap();
                    waker.replace(cx.waker().clone());
                    Poll::Pending
                }
                Err(TryRecvError::Disconnected) => {
                    Poll::Ready(Err(anyhow!("thread terminated without providing a result")))
                }
            }
        }
    }

    spawn_into_main_thread(PendingResult { rx, holder })
}

/// Spawn a future into the main thread; it will be polled in the
/// main thread.
/// This function can be called from any thread.
/// If you are on the main thread already, consider using
/// spawn() instead to lift the `Send` requirement.
pub fn spawn_into_main_thread<F, R>(future: F) -> Task<R>
where
    F: Future<Output = R> + Send + 'static,
    R: Send + 'static,
{
    let (runnable, task) =
        async_task::spawn(future, |runnable| ON_MAIN_THREAD.lock().unwrap()(runnable));
    ON_MAIN_THREAD.lock().unwrap()(runnable);
    task
}

/// Spawn a future into the main thread; it will be polled in
/// the main thread in the low priority queue--all other normal
/// priority items will be drained before considering low priority
/// spawns.
/// If you are on the main thread already, consider using `spawn_with_low_priority`
/// instead to lift the `Send` requirement.
pub fn spawn_into_main_thread_with_low_priority<F, R>(future: F) -> Task<R>
where
    F: Future<Output = R> + Send + 'static,
    R: Send + 'static,
{
    let (runnable, task) = async_task::spawn(future, |runnable| {
        ON_MAIN_THREAD_LOW_PRI.lock().unwrap()(runnable)
    });
    ON_MAIN_THREAD_LOW_PRI.lock().unwrap()(runnable);
    task
}

/// Spawn a future with normal priority.
pub fn spawn<F, R>(future: F) -> Task<R>
where
    F: Future<Output = R> + 'static,
    R: 'static,
{
    let (runnable, task) =
        async_task::spawn_local(future, |runnable| ON_MAIN_THREAD.lock().unwrap()(runnable));
    ON_MAIN_THREAD.lock().unwrap()(runnable);
    task
}

/// Spawn a future with low priority; it will be polled only after
/// all other normal priority items are processed.
pub fn spawn_with_low_priority<F, R>(future: F) -> Task<R>
where
    F: Future<Output = R> + 'static,
    R: 'static,
{
    let (runnable, task) = async_task::spawn_local(future, |runnable| {
        ON_MAIN_THREAD_LOW_PRI.lock().unwrap()(runnable)
    });
    ON_MAIN_THREAD_LOW_PRI.lock().unwrap()(runnable);
    task
}

/// Block the current thread until the passed future completes.
pub use async_std::task::block_on;

/*
pub async fn join_handle_result<T>(handle: JoinHandle<anyhow::Result<T>, ()>) -> anyhow::Result<T> {
    handle
        .await
        .ok_or_else(|| anyhow::anyhow!("task was cancelled or panicked"))?
}
*/

pub struct SimpleExecutor {
    rx: crossbeam::channel::Receiver<SpawnFunc>,
}

impl SimpleExecutor {
    pub fn new() -> Self {
        let (tx, rx) = crossbeam::channel::unbounded();

        let tx_main = tx.clone();
        let tx_low = tx.clone();
        let queue_func = move |f: SpawnFunc| {
            tx_main.send(f).ok();
        };
        let queue_func_low = move |f: SpawnFunc| {
            tx_low.send(f).ok();
        };
        set_schedulers(
            Box::new(move |task| {
                queue_func(Box::new(move || {
                    task.run();
                }))
            }),
            Box::new(move |task| {
                queue_func_low(Box::new(move || {
                    task.run();
                }))
            }),
        );
        Self { rx }
    }

    pub fn tick(&self) -> anyhow::Result<()> {
        match self.rx.recv() {
            Ok(func) => func(),
            Err(err) => anyhow::bail!("while waiting for events: {:?}", err),
        };
        Ok(())
    }
}
