//! Background tasks: `'static` work that spans ticks or frames (flow fields, tile streaming).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread;

use crate::pool::lock;

enum TaskState<T> {
    Pending(Box<dyn FnOnce() -> T + Send>),
    Running,
    Done(thread::Result<T>),
    Taken,
}

pub(crate) struct Task<T> {
    name: &'static str,
    state: Mutex<TaskState<T>>,
    done_cv: Condvar,
    finished: AtomicBool,
}

/// What the background queue sees of a `Task<T>`.
pub(crate) trait BackgroundRun: Send + Sync {
    /// Runs the task unless somebody already has. Returns when this call's run, if any, is done.
    fn run(&self);
}

impl<T: Send + 'static> Task<T> {
    pub(crate) fn new(name: &'static str, f: Box<dyn FnOnce() -> T + Send>) -> Arc<Task<T>> {
        Arc::new(Task {
            name,
            state: Mutex::new(TaskState::Pending(f)),
            done_cv: Condvar::new(),
            finished: AtomicBool::new(false),
        })
    }
}

impl<T: Send + 'static> BackgroundRun for Task<T> {
    fn run(&self) {
        let f = {
            let mut state = lock(&self.state);
            match std::mem::replace(&mut *state, TaskState::Running) {
                TaskState::Pending(f) => f,
                // A `join` got here first; its queue entry is a leftover.
                other => {
                    *state = other;
                    return;
                }
            }
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        let mut state = lock(&self.state);
        *state = TaskState::Done(result);
        self.finished.store(true, Ordering::SeqCst);
        self.done_cv.notify_all();
    }
}

/// Owner's end of a background task. Dropping it detaches the task.
pub struct TaskHandle<T> {
    task: Arc<Task<T>>,
}

impl<T: Send + 'static> TaskHandle<T> {
    pub(crate) fn new(task: Arc<Task<T>>) -> TaskHandle<T> {
        TaskHandle { task }
    }

    pub fn name(&self) -> &'static str {
        self.task.name
    }

    /// True once the task has returned or panicked. For UI and diagnostics: the sim must decide
    /// *when* to adopt a result at request time and `join` on that tick, never poll this.
    pub fn is_finished(&self) -> bool {
        self.task.finished.load(Ordering::SeqCst)
    }

    /// Returns the task's result, rethrowing its panic if it had one. A task that no worker has
    /// started yet runs right here, so this cannot deadlock from any thread, including a pool
    /// worker and including after the pool is gone. Otherwise blocks until the worker is done.
    pub fn join(self) -> T {
        self.task.run();
        let mut state = lock(&self.task.state);
        loop {
            match std::mem::replace(&mut *state, TaskState::Taken) {
                TaskState::Done(Ok(value)) => return value,
                TaskState::Done(Err(payload)) => {
                    drop(state);
                    std::panic::resume_unwind(payload)
                }
                other => {
                    *state = other;
                    state = self
                        .task
                        .done_cv
                        .wait(state)
                        .unwrap_or_else(PoisonError::into_inner);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Pool;
    use std::panic::{self, AssertUnwindSafe};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{mpsc, Arc};
    use std::time::{Duration, Instant};

    const THREAD_COUNTS: [usize; 5] = [0, 1, 2, 8, 16];

    fn wait_for(what: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !what() {
            assert!(Instant::now() < deadline, "timed out");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn join_returns_the_value() {
        for threads in THREAD_COUNTS {
            let pool = Pool::new(threads);
            let handles: Vec<_> = (0..40u64)
                .map(|i| pool.spawn_background("square", move || i * i))
                .collect();
            assert_eq!(handles[0].name(), "square");
            let values: Vec<u64> = handles.into_iter().map(|h| h.join()).collect();
            assert!(values.iter().enumerate().all(|(i, &v)| v == (i * i) as u64));
        }
    }

    #[test]
    fn is_finished_flips_when_done() {
        let pool = Pool::new(1);
        let (release, gate) = mpsc::channel::<()>();
        let handle = pool.spawn_background("gated", move || gate.recv().is_ok());
        assert!(!handle.is_finished());
        release.send(()).unwrap();
        wait_for(|| handle.is_finished());
        assert!(handle.join());
    }

    #[test]
    fn detached_tasks_still_run() {
        for threads in THREAD_COUNTS {
            let pool = Pool::new(threads);
            let count = Arc::new(AtomicUsize::new(0));
            for _ in 0..20 {
                let count = count.clone();
                drop(
                    pool.spawn_background("detached", move || count.fetch_add(1, Ordering::SeqCst)),
                );
            }
            wait_for(|| count.load(Ordering::SeqCst) == 20);
        }
    }

    #[test]
    fn join_runs_an_unstarted_task_inline() {
        // The only worker is stuck, so nothing but `join` can run the second task.
        let pool = Pool::new(1);
        let (release, gate) = mpsc::channel::<()>();
        let started = Arc::new(AtomicBool::new(false));
        let blocker = {
            let started = started.clone();
            pool.spawn_background("blocker", move || {
                started.store(true, Ordering::SeqCst);
                gate.recv().is_ok()
            })
        };
        wait_for(|| started.load(Ordering::SeqCst));
        let queued = pool.spawn_background("queued", || std::thread::current().id());
        assert_eq!(queued.join(), std::thread::current().id());
        release.send(()).unwrap();
        assert!(blocker.join());
    }

    #[test]
    fn join_from_inside_jobs() {
        for threads in THREAD_COUNTS {
            let pool = &Pool::new(threads);
            for _ in 0..50 {
                let fields: Vec<_> = (0..6u64)
                    .map(|i| {
                        pool.spawn_background("flow field", move || {
                            (0..1000u64).map(|v| v * i).sum::<u64>()
                        })
                    })
                    .collect();
                let results: Vec<_> = (0..6).map(|_| AtomicUsize::new(0)).collect();
                pool.run_graph(|g| {
                    for (field, result) in fields.into_iter().zip(&results) {
                        g.add("adopt", &[], move || {
                            // A task may also fan out before it is joined.
                            pool.parallel_for(8, 1, |_, _| {});
                            result.store(field.join() as usize, Ordering::SeqCst);
                        });
                    }
                });
                for (i, result) in results.iter().enumerate() {
                    assert_eq!(result.load(Ordering::SeqCst), 499_500 * i);
                }
            }
        }
    }

    #[test]
    fn background_tasks_can_use_the_pool() {
        for threads in [1, 2, 8] {
            let pool = Arc::new(Pool::new(threads));
            let inner = pool.clone();
            let handle = pool.spawn_background("fan out", move || {
                inner
                    .parallel_map_chunks(1000, 10, |_, range| range.len())
                    .into_iter()
                    .sum::<usize>()
            });
            drop(pool);
            // The task may now hold the last `Arc<Pool>` and drop it on a worker thread.
            assert_eq!(handle.join(), 1000);
        }
    }

    #[test]
    fn panic_surfaces_in_join() {
        for threads in [0, 2] {
            let pool = Pool::new(threads);
            let handle = pool.spawn_background("bad", || -> u32 { panic!("task failed") });
            let payload = panic::catch_unwind(AssertUnwindSafe(|| handle.join()))
                .expect_err("join must rethrow");
            assert_eq!(payload.downcast_ref::<&str>(), Some(&"task failed"));
            assert_eq!(pool.spawn_background("good", || 5).join(), 5);
        }
    }

    #[test]
    fn handle_outlives_the_pool() {
        let pool = Pool::new(1);
        let (release, gate) = mpsc::channel::<()>();
        let blocker = pool.spawn_background("blocker", move || gate.recv().is_ok());
        let queued = pool.spawn_background("queued", || 11);
        let dropper = std::thread::spawn(move || drop(pool));
        release.send(()).unwrap();
        dropper.join().unwrap();
        assert!(blocker.join());
        assert_eq!(queued.join(), 11);
    }
}
