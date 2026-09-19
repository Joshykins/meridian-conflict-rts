//! The worker pool and the single scheduling lock everything else hangs off.

use std::cell::Cell;
use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};

use crate::graph::{GraphState, ReadyJob};
use crate::parallel::ForState;
use crate::task::{BackgroundRun, Task, TaskHandle};

/// Most worker threads a pool accepts. Worker indices are reported as `u16`.
pub const MAX_THREADS: usize = 256;

thread_local! {
    static WORKER_INDEX: Cell<Option<u16>> = const { Cell::new(None) };
}

/// Index of the pool worker running the current thread. Profiler only.
pub(crate) fn current_worker() -> Option<u16> {
    WORKER_INDEX.with(Cell::get)
}

/// User code never runs under any lock in this crate, so a poisoned lock still guards
/// consistent data.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Armed while lifetime-erased borrows are reachable from other threads. Unwinding past that
/// point would free the borrowed data under them, so an internal panic there aborts instead.
pub(crate) struct AbortOnUnwind;

impl AbortOnUnwind {
    pub(crate) fn disarm(self) {
        std::mem::forget(self);
    }
}

impl Drop for AbortOnUnwind {
    fn drop(&mut self) {
        std::process::abort();
    }
}

pub(crate) struct Queues {
    /// `parallel_for` calls that may still have unclaimed chunks. The owner removes its entry.
    pub(crate) tickets: Vec<Arc<ForState>>,
    /// Graph jobs whose dependencies have all finished, oldest first.
    pub(crate) jobs: VecDeque<ReadyJob>,
    pub(crate) background: VecDeque<Arc<dyn BackgroundRun>>,
    pub(crate) background_running: usize,
    /// Threads asleep on `waiter_cv`.
    pub(crate) waiters: usize,
    pub(crate) shutdown: bool,
}

pub(crate) enum Work {
    Ticket(Arc<ForState>),
    Job(ReadyJob),
    Background(Arc<dyn BackgroundRun>),
}

impl Queues {
    fn open_ticket(&self) -> Option<Arc<ForState>> {
        self.tickets.iter().find(|t| t.has_unclaimed()).cloned()
    }

    fn take_foreground(&mut self) -> Option<Work> {
        if let Some(ticket) = self.open_ticket() {
            return Some(Work::Ticket(ticket));
        }
        self.jobs.pop_front().map(Work::Job)
    }

    fn take_background(&mut self, limit: usize) -> Option<Work> {
        if self.background_running >= limit {
            return None;
        }
        let task = self.background.pop_front()?;
        self.background_running += 1;
        Some(Work::Background(task))
    }
}

pub(crate) struct Shared {
    queues: Mutex<Queues>,
    /// Idle workers. They can run anything, so one `notify_one` per queued item is enough.
    pub(crate) worker_cv: Condvar,
    /// Threads blocked inside `run_graph`/`parallel_for`. They are picky about what they run,
    /// so they are always woken together.
    pub(crate) waiter_cv: Condvar,
    pub(crate) threads: usize,
    background_limit: usize,
}

impl Shared {
    pub(crate) fn lock(&self) -> MutexGuard<'_, Queues> {
        lock(&self.queues)
    }

    /// Wakes scope waiters so they re-check their completion condition. Taking the lock first
    /// closes the window between a waiter's check and its sleep.
    pub(crate) fn notify_waiters(&self) {
        let queues = self.lock();
        if queues.waiters > 0 {
            self.waiter_cv.notify_all();
        }
    }

    pub(crate) fn publish_ticket(&self, ticket: Arc<ForState>, chunks: usize) {
        let mut queues = self.lock();
        queues.tickets.push(ticket);
        // The owner runs one chunk itself.
        for _ in 0..self.threads.min(chunks - 1) {
            self.worker_cv.notify_one();
        }
        if queues.waiters > 0 {
            self.waiter_cv.notify_all();
        }
    }

    pub(crate) fn retire_ticket(&self, ticket: &Arc<ForState>) {
        self.lock().tickets.retain(|t| !Arc::ptr_eq(t, ticket));
    }

    /// Blocks until `done()`, running foreground work in the meantime: ready jobs of `own`
    /// graph and open `parallel_for` tickets from anywhere. `done` must only turn true via a
    /// path that ends in `notify_waiters` or a notification sent under the queue lock.
    pub(crate) fn help_until(&self, own: Option<&Arc<GraphState>>, done: &dyn Fn() -> bool) {
        loop {
            let work = {
                let mut queues = self.lock();
                loop {
                    if done() {
                        return;
                    }
                    if let Some(ticket) = queues.open_ticket() {
                        break Work::Ticket(ticket);
                    }
                    let own_job = own.and_then(|g| queues.jobs.iter().position(|j| j.belongs_to(g)));
                    if let Some(job) = own_job.and_then(|at| queues.jobs.remove(at)) {
                        break Work::Job(job);
                    }
                    queues.waiters += 1;
                    queues = self.waiter_cv.wait(queues).unwrap_or_else(PoisonError::into_inner);
                    queues.waiters -= 1;
                }
            };
            match work {
                Work::Ticket(ticket) => ticket.help(self, done),
                Work::Job(job) => job.run(self),
                Work::Background(_) => unreachable!("waiters never take background tasks"),
            }
        }
    }
}

fn worker_main(shared: Arc<Shared>, index: usize) {
    WORKER_INDEX.with(|w| w.set(Some(index as u16)));
    loop {
        let work = {
            let mut queues = shared.lock();
            loop {
                if let Some(work) = queues.take_foreground() {
                    break work;
                }
                // Foreground queues are empty here: nobody can be inside a scope while the
                // pool is being dropped. Unstarted background tasks are left to their handles.
                if queues.shutdown {
                    return;
                }
                if let Some(work) = queues.take_background(shared.background_limit) {
                    break work;
                }
                queues = shared.worker_cv.wait(queues).unwrap_or_else(PoisonError::into_inner);
            }
        };
        match work {
            Work::Ticket(ticket) => ticket.help(&shared, &|| false),
            Work::Job(job) => job.run(&shared),
            Work::Background(task) => {
                task.run();
                shared.lock().background_running -= 1;
            }
        }
    }
}

/// A fixed set of worker threads that lives as long as the game. Share it by `&Pool` or
/// `Arc<Pool>`; any number of threads may submit work at once.
pub struct Pool {
    pub(crate) shared: Arc<Shared>,
    workers: Vec<JoinHandle<()>>,
}

impl Pool {
    /// `threads` workers named `mc-worker-N`. Zero is valid and means "caller thread only":
    /// all work runs inside `run_graph`/`parallel_for`/`spawn_background` on the calling thread.
    pub fn new(threads: usize) -> Pool {
        assert!(threads <= MAX_THREADS, "pool of {threads} threads exceeds MAX_THREADS ({MAX_THREADS})");
        let shared = Arc::new(Shared {
            queues: Mutex::new(Queues {
                tickets: Vec::new(),
                jobs: VecDeque::new(),
                background: VecDeque::new(),
                background_running: 0,
                waiters: 0,
                shutdown: false,
            }),
            worker_cv: Condvar::new(),
            waiter_cv: Condvar::new(),
            threads,
            background_limit: (threads / 2).max(1),
        });
        let workers = (0..threads)
            .map(|index| {
                let shared = shared.clone();
                thread::Builder::new()
                    .name(format!("mc-worker-{index}"))
                    .spawn(move || worker_main(shared, index))
                    .expect("failed to spawn worker thread")
            })
            .collect();
        Pool { shared, workers }
    }

    /// One worker per hardware thread, minus one because submitting threads do their share.
    pub fn with_default_threads() -> Pool {
        let hardware = thread::available_parallelism().map_or(1, |n| n.get());
        Pool::new(hardware.saturating_sub(1).clamp(1, MAX_THREADS))
    }

    /// Worker threads, not counting callers that help while they wait.
    pub fn thread_count(&self) -> usize {
        self.shared.threads
    }

    /// Queues work that outlives the current tick or frame. Runs on a worker when no foreground
    /// work is queued. Dropping the handle detaches the task; a detached task that has not
    /// started when the pool is dropped never runs.
    pub fn spawn_background<T, F>(&self, name: &'static str, f: F) -> TaskHandle<T>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        let task = Task::new(name, Box::new(f));
        if self.shared.threads == 0 {
            // Nobody else will ever run it, and a detached task must still happen.
            task.run();
        } else {
            self.shared.lock().background.push_back(task.clone());
            self.shared.worker_cv.notify_one();
        }
        TaskHandle::new(task)
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        self.shared.lock().shutdown = true;
        self.shared.worker_cv.notify_all();
        let current = thread::current().id();
        for worker in self.workers.drain(..) {
            // The last `Arc<Pool>` can die inside a background task; a worker cannot join itself.
            if worker.thread().id() != current {
                let _ = worker.join();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[test]
    fn pool_is_shareable() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Pool>();
        assert_send_sync::<TaskHandle<Vec<u8>>>();
    }

    #[test]
    fn thread_counts() {
        assert_eq!(Pool::new(0).thread_count(), 0);
        assert_eq!(Pool::new(3).thread_count(), 3);
        assert!(Pool::with_default_threads().thread_count() >= 1);
    }

    #[test]
    fn drop_while_idle() {
        for threads in [0, 1, 2, 8, 16] {
            drop(Pool::new(threads));
        }
        // Workers that have gone to sleep at least once.
        let pool = Pool::new(4);
        pool.parallel_for(64, 1, |_, _| {});
        thread::sleep(Duration::from_millis(20));
        drop(pool);
    }

    #[test]
    fn workers_are_named() {
        let pool = Pool::new(1);
        let handle = pool.spawn_background("name", || thread::current().name().map(str::to_owned));
        // Joining straight away would run the task on this thread.
        while !handle.is_finished() {
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(handle.join().as_deref(), Some("mc-worker-0"));
    }

    #[test]
    fn foreground_is_taken_before_background() {
        let pool = Pool::new(0);
        let body: &'static (dyn Fn(usize) + Sync) = Box::leak(Box::new(|_: usize| {}));
        let ticket = Arc::new(ForState::new(2, body));
        let mut queues = pool.shared.lock();
        queues.background.push_back(Task::new("bg", Box::new(|| ())));
        queues.background.push_back(Task::new("bg", Box::new(|| ())));
        queues.tickets.push(ticket.clone());

        assert!(matches!(queues.take_foreground(), Some(Work::Ticket(_))));
        drop(queues);
        ticket.help(&pool.shared, &|| false);
        let mut queues = pool.shared.lock();
        assert!(queues.take_foreground().is_none());
        assert!(matches!(queues.take_background(1), Some(Work::Background(_))));
        // The cap holds the second task back until the first is done.
        assert!(queues.take_background(1).is_none());
        queues.background_running -= 1;
        assert!(queues.take_background(1).is_some());
    }

    #[test]
    fn several_threads_submit_at_once() {
        let pool = Pool::new(4);
        let total = AtomicUsize::new(0);
        thread::scope(|s| {
            for _ in 0..3 {
                s.spawn(|| {
                    for round in 0..200 {
                        if round % 2 == 0 {
                            pool.parallel_for(100, 7, |_, range| {
                                total.fetch_add(range.len(), Ordering::Relaxed);
                            });
                        } else {
                            pool.run_graph(|g| {
                                let a = g.add("a", &[], || {
                                    total.fetch_add(40, Ordering::Relaxed);
                                });
                                g.add("b", &[a], || {
                                    total.fetch_add(60, Ordering::Relaxed);
                                });
                            });
                        }
                    }
                });
            }
        });
        assert_eq!(total.load(Ordering::Relaxed), 3 * 200 * 100);
    }
}
