//! Scoped job graph: named closures with dependencies, run to completion before returning.

use std::any::Any;
use std::marker::PhantomData;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::pool::{current_worker, lock, AbortOnUnwind, Pool, Shared};

/// Handle to a job added earlier to the same graph. Only meaningful in that graph.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct JobId(u32);

impl JobId {
    /// Position in `GraphReport::jobs`.
    #[inline]
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// When and where one job ran. Wall-clock data for the in-game profiler; never sim input.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct JobTiming {
    pub name: &'static str,
    /// Nanoseconds from the start of graph execution to the start of this job.
    pub start_ns: u64,
    pub duration_ns: u64,
    /// Pool worker that ran the job, `None` for a thread outside the pool (usually the caller).
    pub worker: Option<u16>,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct GraphReport {
    /// One entry per job, in the order the jobs were added.
    pub jobs: Vec<JobTiming>,
    /// Wall time from the first job becoming runnable to the last one finishing.
    pub total_ns: u64,
}

const NO_WORKER: u32 = u32::MAX;

struct JobSlot {
    name: &'static str,
    /// Taken by whichever thread runs the job. See `Graph::add` for why `'static` is a lie.
    func: Mutex<Option<Box<dyn FnOnce() + Send + 'static>>>,
    /// Dependencies that have not finished yet.
    blockers: AtomicUsize,
    dependents: Vec<u32>,
    start_ns: AtomicU64,
    duration_ns: AtomicU64,
    worker: AtomicU32,
}

/// Builder handed to the closure of `Pool::run_graph`. Nothing runs until that closure returns,
/// so the order in which a frame's jobs become runnable does not depend on timing.
pub struct Graph<'env> {
    jobs: Vec<JobSlot>,
    /// Invariant in `'env`, so the lifetime job closures are checked against cannot be shrunk.
    _env: PhantomData<&'env mut &'env ()>,
}

impl<'env> Graph<'env> {
    /// Adds a job that runs after every job in `deps`. `f` may borrow anything that outlives
    /// the `run_graph` call. Panics if a `JobId` does not come from this graph.
    pub fn add<F>(&mut self, name: &'static str, deps: &[JobId], f: F) -> JobId
    where
        F: FnOnce() + Send + 'env,
    {
        let index = u32::try_from(self.jobs.len()).expect("too many jobs in one graph");
        for dep in deps {
            // Dependencies point at earlier jobs only, so the graph is acyclic by construction.
            assert!(dep.0 < index, "job `{name}` depends on a job that is not in this graph");
            self.jobs[dep.index()].dependents.push(index);
        }
        let func: Box<dyn FnOnce() + Send + 'env> = Box::new(f);
        // SAFETY: only the lifetime bound changes. `'env` outlives the `run_graph` call this
        // graph belongs to, and the closure is consumed before that call returns or unwinds:
        // - if the build closure panics, the `Graph` and its closures are dropped during unwind;
        // - otherwise `run_graph` does not return until `remaining` is zero, every job counted
        //   there has been through `ReadyJob::run`, and `run` takes the closure out of its slot
        //   and calls or drops it before it decrements `remaining`.
        // The `Graph` itself is owned by `run_graph`; user code only ever sees `&mut Graph`.
        let func = unsafe {
            std::mem::transmute::<Box<dyn FnOnce() + Send + 'env>, Box<dyn FnOnce() + Send + 'static>>(func)
        };
        self.jobs.push(JobSlot {
            name,
            func: Mutex::new(Some(func)),
            blockers: AtomicUsize::new(deps.len()),
            dependents: Vec::new(),
            start_ns: AtomicU64::new(0),
            duration_ns: AtomicU64::new(0),
            worker: AtomicU32::new(NO_WORKER),
        });
        JobId(index)
    }
}

pub(crate) struct GraphState {
    jobs: Vec<JobSlot>,
    /// Jobs that have not finished. Only written under the queue lock.
    remaining: AtomicUsize,
    /// Set by the first panic. Jobs that start afterwards drop their closure instead of calling it.
    cancelled: AtomicBool,
    panic: Mutex<Option<Box<dyn Any + Send>>>,
    epoch: Instant,
}

/// A job whose dependencies have all finished.
pub(crate) struct ReadyJob {
    graph: Arc<GraphState>,
    index: u32,
}

impl ReadyJob {
    pub(crate) fn belongs_to(&self, graph: &Arc<GraphState>) -> bool {
        Arc::ptr_eq(&self.graph, graph)
    }

    pub(crate) fn run(self, shared: &Shared) {
        let graph = &*self.graph;
        let slot = &graph.jobs[self.index as usize];
        let func = lock(&slot.func).take();
        let start = graph.epoch.elapsed();
        if let Some(func) = func {
            let cancelled = graph.cancelled.load(Ordering::SeqCst);
            // Captured values may have destructors that panic, so the drop is guarded as well.
            let outcome = panic::catch_unwind(AssertUnwindSafe(move || if cancelled { drop(func) } else { func() }));
            if let Err(payload) = outcome {
                graph.cancelled.store(true, Ordering::SeqCst);
                lock(&graph.panic).get_or_insert(payload);
            }
        }
        let end = graph.epoch.elapsed();
        slot.start_ns.store(start.as_nanos() as u64, Ordering::Relaxed);
        slot.duration_ns.store((end - start).as_nanos() as u64, Ordering::Relaxed);
        slot.worker.store(current_worker().map_or(NO_WORKER, u32::from), Ordering::Relaxed);

        let mut queues = shared.lock();
        let mut changed = false;
        for &dependent in &slot.dependents {
            if graph.jobs[dependent as usize].blockers.fetch_sub(1, Ordering::SeqCst) == 1 {
                queues.jobs.push_back(ReadyJob { graph: self.graph.clone(), index: dependent });
                shared.worker_cv.notify_one();
                changed = true;
            }
        }
        changed |= graph.remaining.fetch_sub(1, Ordering::SeqCst) == 1;
        // The graph's owner may be asleep waiting for exactly one of these two events.
        if changed && queues.waiters > 0 {
            shared.waiter_cv.notify_all();
        }
    }
}

impl Pool {
    /// Builds a job graph with `build`, runs it, and returns once every job has finished. The
    /// calling thread runs jobs too, so this works on a pool with no workers.
    ///
    /// If a job panics, jobs that have not started yet are skipped, running jobs finish, and the
    /// first panic is rethrown here.
    ///
    /// ```
    /// let pool = mc_jobs::Pool::new(2);
    /// let costs = vec![3u32, 4, 5];
    /// let (mut total, mut max) = (0, 0);
    /// let report = pool.run_graph(|g| {
    ///     let a = g.add("total", &[], || total = costs.iter().sum());
    ///     let b = g.add("max", &[], || max = costs.iter().copied().max().unwrap());
    ///     g.add("after both", &[a, b], || {});
    /// });
    /// assert_eq!((total, max), (12, 5));
    /// assert_eq!(report.jobs[2].name, "after both");
    /// ```
    ///
    /// Jobs run after `build` returns, so they cannot borrow its locals:
    ///
    /// ```compile_fail
    /// let pool = mc_jobs::Pool::new(2);
    /// pool.run_graph(|g| {
    ///     let costs = vec![3u32, 4, 5];
    ///     g.add("dangling", &[], || assert_eq!(costs.len(), 3));
    /// });
    /// ```
    pub fn run_graph<'env, B>(&self, build: B) -> GraphReport
    where
        B: FnOnce(&mut Graph<'env>),
    {
        let mut graph = Graph { jobs: Vec::new(), _env: PhantomData };
        build(&mut graph);
        let jobs = graph.jobs;
        if jobs.is_empty() {
            return GraphReport::default();
        }

        let shared = &*self.shared;
        let state = Arc::new(GraphState {
            remaining: AtomicUsize::new(jobs.len()),
            jobs,
            cancelled: AtomicBool::new(false),
            panic: Mutex::new(None),
            epoch: Instant::now(),
        });
        // Job closures are reachable from other threads until `remaining` is zero. Nothing in
        // between runs user code outside `catch_unwind`; if it unwinds anyway, abort.
        let guard = AbortOnUnwind;
        {
            let mut queues = shared.lock();
            let mut roots = 0;
            for (index, slot) in state.jobs.iter().enumerate() {
                if slot.blockers.load(Ordering::SeqCst) == 0 {
                    queues.jobs.push_back(ReadyJob { graph: state.clone(), index: index as u32 });
                    roots += 1;
                }
            }
            // One wake per root: this thread may be drawn into somebody's `parallel_for` first.
            for _ in 0..shared.threads.min(roots) {
                shared.worker_cv.notify_one();
            }
        }
        shared.help_until(Some(&state), &|| state.remaining.load(Ordering::SeqCst) == 0);
        guard.disarm();
        let total_ns = state.epoch.elapsed().as_nanos() as u64;

        let payload = lock(&state.panic).take();
        if let Some(payload) = payload {
            panic::resume_unwind(payload);
        }
        let jobs = state
            .jobs
            .iter()
            .map(|slot| JobTiming {
                name: slot.name,
                start_ns: slot.start_ns.load(Ordering::Relaxed),
                duration_ns: slot.duration_ns.load(Ordering::Relaxed),
                worker: u16::try_from(slot.worker.load(Ordering::Relaxed)).ok(),
            })
            .collect();
        GraphReport { jobs, total_ns }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const THREAD_COUNTS: [usize; 5] = [0, 1, 2, 8, 16];

    struct Lcg(u64);

    impl Lcg {
        fn below(&mut self, bound: usize) -> usize {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((self.0 >> 33) as usize) % bound
        }
    }

    #[test]
    fn empty_graph() {
        assert_eq!(Pool::new(2).run_graph(|_| {}), GraphReport::default());
    }

    #[test]
    fn jobs_borrow_from_the_caller() {
        let pool = Pool::new(2);
        let input = [1u64, 2, 3, 4];
        let (mut sum, mut product, mut both) = (0, 0, 0);
        pool.run_graph(|g| {
            let a = g.add("sum", &[], || sum = input.iter().sum());
            let b = g.add("product", &[], || product = input.iter().product());
            // Declared dependencies are why this job may assume the other two are done; it
            // cannot borrow their outputs, so it recomputes.
            g.add("both", &[a, b], || both = input.iter().sum::<u64>() + input.iter().product::<u64>());
        });
        assert_eq!((sum, product, both), (10, 24, 34));
    }

    #[test]
    fn random_dags_respect_dependencies() {
        for threads in THREAD_COUNTS {
            let pool = Pool::new(threads);
            let mut rng = Lcg(threads as u64 + 1);
            for _ in 0..300 {
                let count = 1 + rng.below(48);
                let deps: Vec<Vec<usize>> = (0..count)
                    .map(|i| if i == 0 { Vec::new() } else { (0..rng.below(5)).map(|_| rng.below(i)).collect() })
                    .collect();
                let done: Vec<AtomicBool> = (0..count).map(|_| AtomicBool::new(false)).collect();
                let runs = AtomicUsize::new(0);
                let violations = AtomicUsize::new(0);

                let report = pool.run_graph(|g| {
                    let mut ids: Vec<JobId> = Vec::new();
                    for (i, deps) in deps.iter().enumerate() {
                        let dep_ids: Vec<JobId> = deps.iter().map(|&d| ids[d]).collect();
                        let (done, runs, violations) = (&done, &runs, &violations);
                        ids.push(g.add("node", &dep_ids, move || {
                            if deps.iter().any(|&d| !done[d].load(Ordering::SeqCst)) {
                                violations.fetch_add(1, Ordering::SeqCst);
                            }
                            if i % 7 == 0 {
                                std::thread::yield_now();
                            }
                            runs.fetch_add(1, Ordering::SeqCst);
                            done[i].store(true, Ordering::SeqCst);
                        }));
                    }
                });

                assert_eq!(violations.into_inner(), 0);
                assert_eq!(runs.into_inner(), count);
                assert_eq!(report.jobs.len(), count);
                for (i, deps) in deps.iter().enumerate() {
                    for &d in deps {
                        let (dep, job) = (&report.jobs[d], &report.jobs[i]);
                        assert!(dep.start_ns + dep.duration_ns <= job.start_ns, "{threads} threads");
                    }
                }
            }
        }
    }

    #[test]
    fn timings_are_reported() {
        let pool = Pool::new(2);
        let report = pool.run_graph(|g| {
            let a = g.add("slow", &[], || std::thread::sleep(Duration::from_millis(5)));
            g.add("after", &[a], || {});
        });
        assert_eq!(report.jobs[0].name, "slow");
        assert_eq!(report.jobs[1].name, "after");
        assert!(report.jobs[0].duration_ns >= 5_000_000);
        assert!(report.jobs[1].start_ns >= report.jobs[0].start_ns + report.jobs[0].duration_ns);
        assert!(report.total_ns >= report.jobs[1].start_ns + report.jobs[1].duration_ns);
        assert!(report.jobs.iter().all(|j| j.worker.is_none_or(|w| w < 2)));
    }

    /// A small stand-in for a sim tick: two independent systems fan out with `parallel_for`,
    /// a third consumes both.
    fn tick(pool: &Pool, units: &[u64]) -> u64 {
        let (mut moved, mut damage) = (Vec::new(), Vec::new());
        pool.run_graph(|g| {
            g.add("movement", &[], || {
                moved = pool.parallel_map_chunks(units.len(), 64, |chunk, range| {
                    units[range].iter().fold(chunk as u64, |acc, &u| acc.rotate_left(5) ^ u.wrapping_mul(31))
                });
            });
            g.add("weapons", &[], || {
                damage = pool.parallel_map_chunks(units.len(), 50, |chunk, range| {
                    units[range].iter().fold(!(chunk as u64), |acc, &u| acc.wrapping_mul(1_000_003).wrapping_add(u))
                });
            });
        });
        let mut digest = 0u64;
        pool.run_graph(|g| {
            g.add("merge", &[], || {
                digest = moved.iter().chain(&damage).fold(7, |acc, &v| acc.rotate_left(9).wrapping_add(v));
            });
        });
        digest
    }

    #[test]
    fn nested_parallel_for_matches_across_thread_counts() {
        let units: Vec<u64> = (0..5003u64).map(|i| i.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 11).collect();
        let expected = tick(&Pool::new(0), &units);
        for threads in THREAD_COUNTS {
            let pool = Pool::new(threads);
            for _ in 0..25 {
                assert_eq!(tick(&pool, &units), expected, "{threads} threads");
            }
        }
    }

    #[test]
    fn nested_graphs() {
        for threads in THREAD_COUNTS {
            let pool = Pool::new(threads);
            let total = AtomicUsize::new(0);
            pool.run_graph(|g| {
                for _ in 0..6 {
                    g.add("outer", &[], || {
                        pool.run_graph(|inner| {
                            let a = inner.add("a", &[], || {
                                total.fetch_add(1, Ordering::SeqCst);
                            });
                            inner.add("b", &[a], || {
                                total.fetch_add(10, Ordering::SeqCst);
                            });
                        });
                    });
                }
            });
            assert_eq!(total.into_inner(), 66);
        }
    }

    #[test]
    fn panic_propagates_after_the_graph_drains() {
        for threads in THREAD_COUNTS {
            let pool = Pool::new(threads);
            let independent = AtomicUsize::new(0);
            let after_panic = AtomicBool::new(false);
            let result = panic::catch_unwind(AssertUnwindSafe(|| {
                pool.run_graph(|g| {
                    let bad = g.add("bad", &[], || panic!("job failed"));
                    for _ in 0..8 {
                        g.add("independent", &[], || {
                            std::thread::sleep(Duration::from_millis(1));
                            independent.fetch_add(1, Ordering::SeqCst);
                        });
                    }
                    g.add("dependent", &[bad], || after_panic.store(true, Ordering::SeqCst));
                });
            }));
            let payload = result.expect_err("panic must reach the caller");
            assert_eq!(payload.downcast_ref::<&str>(), Some(&"job failed"));
            // A dependent of the failed job never runs; independents may or may not have started.
            assert!(!after_panic.load(Ordering::SeqCst));
            assert!(independent.load(Ordering::SeqCst) <= 8);

            let mut ok = false;
            pool.run_graph(|g| {
                g.add("ok", &[], || ok = true);
            });
            assert!(ok, "pool must stay usable after a panic");
        }
    }

    #[test]
    fn panic_while_building_runs_nothing() {
        let pool = Pool::new(2);
        let ran = AtomicBool::new(false);
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            pool.run_graph(|g| {
                g.add("never", &[], || ran.store(true, Ordering::SeqCst));
                panic!("build failed");
            });
        }));
        assert!(result.is_err());
        assert!(!ran.load(Ordering::SeqCst));
    }

    #[test]
    #[should_panic(expected = "not in this graph")]
    fn foreign_job_id_is_rejected() {
        let pool = Pool::new(0);
        let mut foreign = None;
        pool.run_graph(|g| {
            g.add("a", &[], || {});
            foreign = Some(g.add("b", &[], || {}));
        });
        pool.run_graph(|g| {
            g.add("c", &[foreign.unwrap()], || {});
        });
    }
}
