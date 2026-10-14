//! Work the window waits on without stopping: fetching from the build store,
//! installing, checking files. Each job runs on a thread of its own; the
//! window reads its meter and, once, its result.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use mc_builds::versions::Progress;

/// How far a job has got.
#[derive(Clone, Debug, Default)]
pub(crate) struct Meter {
    /// What it is doing now ("Downloading", "Unpacking").
    pub(crate) stage: &'static str,
    pub(crate) progress: Option<Progress>,
    /// Bytes a second, smoothed.
    pub(crate) rate: f64,
}

impl Meter {
    /// Done over total, 0..1, when there is a total.
    pub(crate) fn fraction(&self) -> Option<f32> {
        let p = self.progress?;
        (p.total > 0).then(|| (p.done as f64 / p.total as f64).min(1.0) as f32)
    }

    /// Seconds left at the present rate.
    pub(crate) fn eta(&self) -> Option<f64> {
        let p = self.progress?;
        (self.rate > 1.0 && p.total > p.done).then(|| (p.total - p.done) as f64 / self.rate)
    }
}

/// What a job's closure reports through.
pub(crate) struct Reporter {
    meter: Arc<Mutex<Meter>>,
    cancel: Arc<AtomicBool>,
    last: Mutex<Option<(Instant, u64)>>,
}

impl Reporter {
    pub(crate) fn stage(&self, stage: &'static str) {
        let mut m = lock(&self.meter);
        m.stage = stage;
        m.progress = None;
        m.rate = 0.0;
        *lock(&self.last) = None;
    }

    pub(crate) fn progress(&self, p: Progress) {
        let now = Instant::now();
        let mut last = lock(&self.last);
        let mut m = lock(&self.meter);
        if let Some((at, done)) = *last {
            let dt = now.duration_since(at).as_secs_f64();
            if dt >= 0.25 {
                let rate = p.done.saturating_sub(done) as f64 / dt;
                m.rate = if m.rate == 0.0 {
                    rate
                } else {
                    m.rate * 0.7 + rate * 0.3
                };
                *last = Some((now, p.done));
            }
        } else {
            *last = Some((now, p.done));
        }
        m.progress = Some(p);
    }

    pub(crate) fn cancel_flag(&self) -> &AtomicBool {
        &self.cancel
    }
}

pub(crate) struct Job<T> {
    result: Arc<Mutex<Option<Result<T, String>>>>,
    meter: Arc<Mutex<Meter>>,
    cancel: Arc<AtomicBool>,
}

impl<T: Send + 'static> Job<T> {
    pub(crate) fn spawn(
        name: &str,
        work: impl FnOnce(&Reporter) -> Result<T, String> + Send + 'static,
    ) -> Job<T> {
        let job = Job {
            result: Arc::new(Mutex::new(None)),
            meter: Arc::new(Mutex::new(Meter::default())),
            cancel: Arc::new(AtomicBool::new(false)),
        };
        let reporter = Reporter {
            meter: job.meter.clone(),
            cancel: job.cancel.clone(),
            last: Mutex::new(None),
        };
        let out = job.result.clone();
        let spawned = std::thread::Builder::new()
            .name(name.to_owned())
            .spawn(move || {
                let r = work(&reporter);
                *lock(&out) = Some(r);
            });
        if let Err(e) = spawned {
            *lock(&job.result) = Some(Err(format!("could not start {name}: {e}")));
        }
        job
    }
}

impl<T> Job<T> {
    /// A job already in a given state, for the screenshot scenes.
    pub(crate) fn fixed(result: Option<Result<T, String>>, meter: Meter) -> Job<T> {
        Job {
            result: Arc::new(Mutex::new(result)),
            meter: Arc::new(Mutex::new(meter)),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn meter(&self) -> Meter {
        lock(&self.meter).clone()
    }

    pub(crate) fn running(&self) -> bool {
        lock(&self.result).is_none()
    }

    /// Looks at the result, if there is one.
    pub(crate) fn peek<R>(&self, f: impl FnOnce(Option<&Result<T, String>>) -> R) -> R {
        f(lock(&self.result).as_ref())
    }

    pub(crate) fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

impl<T> Drop for Job<T> {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// A poisoned lock still holds the last state written: a job that panicked
/// has no result, and the window says it failed.
fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}
