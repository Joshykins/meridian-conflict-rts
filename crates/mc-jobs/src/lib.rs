//! Persistent worker pool, scoped job graph, deterministic `parallel_for`, background tasks.
//!
//! No fibers and no work stealing: one lock guards three queues, which is cheap because the
//! units of work are coarse (tens of graph jobs and a few hundred chunks per tick).
//!
//! Scheduling rules:
//! - Workers take open `parallel_for` tickets first (a thread is blocked on those right now),
//!   then ready graph jobs, then background tasks. Background tasks never run on more than half
//!   the workers, because a running task cannot be preempted when a tick starts.
//! - A thread waiting on its own graph or `parallel_for` keeps working: its own graph's ready
//!   jobs and anybody's `parallel_for` chunks. It never starts another thread's graph job or a
//!   background task, so a wait is never extended by a long unrelated job.
//! - A wait only blocks on work that some thread is already executing, and that work was started
//!   after the waiter's scope was opened. Waits-for edges therefore always point forward in
//!   time and cannot form a cycle, at any nesting depth and any thread count including zero.
//!
//! Determinism: the pool never decides *what* is computed. Chunk boundaries depend only on
//! `len` and `chunk_size`, outputs are merged in chunk order, and graph jobs see exactly the
//! results of their declared dependencies. Timings use the wall clock and are for the profiler
//! only; they must never feed the simulation.

mod graph;
mod parallel;
mod pool;
mod task;

pub use graph::{Graph, GraphReport, JobId, JobTiming};
pub use parallel::{chunk_count, chunk_range};
pub use pool::{Pool, MAX_THREADS};
pub use task::TaskHandle;
