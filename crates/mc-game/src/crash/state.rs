//! What the game was doing, for the reports: facts the game records as it goes
//! (`context`: the window, the graphics preset, the map, the match), the sim's
//! last tick, how long the run had lasted, and the errors raised before the
//! failure, each with the stack that raised it.
//!
//! An error that ends the game usually reaches the top as a sentence ("could not
//! start the renderer: ..."), its stack long unwound. The errors are kept as they
//! are raised (`on_gpu_error`, hooked into mc-render), so the report still has the
//! stack of the call that failed.

use std::collections::{BTreeMap, VecDeque};
use std::fmt::Write;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

static STARTED: OnceLock<Instant> = OnceLock::new();

/// Facts the game recorded, by name.
static CONTEXT: Mutex<BTreeMap<&'static str, String>> = Mutex::new(BTreeMap::new());

/// The sim's last tick, plus one (0: no match has run).
static SIM_TICK: AtomicU32 = AtomicU32::new(0);

/// The errors raised last, oldest first. A failure raises a handful at most: the
/// first is the cause, the rest its fallout.
const KEEP_RAISED: usize = 8;
static RAISED: Mutex<VecDeque<Raised>> = Mutex::new(VecDeque::new());

/// Of the kept errors, the newest this many carry their stack into a report.
const STACKS_SHOWN: usize = 3;

struct Raised {
    seconds: f32,
    thread: String,
    message: String,
    stack: String,
}

pub(super) fn start() {
    STARTED.get_or_init(Instant::now);
}

fn seconds() -> f32 {
    STARTED.get().map_or(0.0, |t| t.elapsed().as_secs_f32())
}

/// Records a fact about the run for any report written after it: replaces the one
/// of the same name.
pub fn context(name: &'static str, value: String) {
    if let Ok(mut map) = CONTEXT.lock() {
        map.insert(name, value);
    }
}

/// Records the window and the display it is on (`context` "window"): at creation
/// and on every resize.
pub fn note_window(window: &winit::window::Window) {
    let size = window.inner_size();
    let display = window.current_monitor().map_or_else(
        || "an unknown display".to_owned(),
        |m| {
            let s = m.size();
            let hz = m.refresh_rate_millihertz().map_or(0, |r| r / 1000);
            format!(
                "{} ({}x{} at {hz} Hz)",
                m.name().unwrap_or_else(|| "a display".into()),
                s.width,
                s.height
            )
        },
    );
    let mode = if window.fullscreen().is_some() {
        "fullscreen"
    } else {
        "windowed"
    };
    context(
        "window",
        format!(
            "{}x{} {mode}, scale {:.2}, on {display}",
            size.width,
            size.height,
            window.scale_factor()
        ),
    );
}

/// The sim finished `tick`. Called every tick: a store, nothing more.
pub fn sim_tick(tick: u32) {
    SIM_TICK.store(tick.saturating_add(1), Ordering::Relaxed);
}

/// Keeps a Vulkan error as it is raised, with its stack (`mc_render::gpu::set_error_hook`).
pub(super) fn on_gpu_error(e: &mc_render::GpuError) {
    let raised = Raised {
        seconds: seconds(),
        thread: std::thread::current()
            .name()
            .unwrap_or("<unnamed>")
            .to_owned(),
        message: e.to_string(),
        stack: stack(),
    };
    if let Ok(mut kept) = RAISED.lock() {
        if kept.len() == KEEP_RAISED {
            kept.pop_front();
        }
        kept.push_back(raised);
    }
}

/// This thread's stack: `module+offset` frames on Windows (a player's copy has no
/// `.pdb` to name them; the build's does), the named backtrace elsewhere.
fn stack() -> String {
    #[cfg(windows)]
    {
        super::stack::here()
    }
    #[cfg(not(windows))]
    {
        std::backtrace::Backtrace::force_capture().to_string()
    }
}

/// The report's `state:` section and, when any were raised, its `errors raised:`
/// one. Never waits on a lock: a report may be written from inside a panic.
pub(super) fn describe() -> String {
    let mut text = String::from("state:\n");
    let _ = writeln!(text, "  running for: {:.1} s", seconds());
    match SIM_TICK.load(Ordering::Relaxed) {
        0 => text.push_str("  sim: no match has run\n"),
        t => {
            let tick = t - 1;
            let _ = writeln!(
                text,
                "  sim: tick {tick} ({:.1} s of game time)",
                tick as f32 / mc_core::TICKS_PER_SECOND as f32
            );
        }
    }
    if let Ok(map) = CONTEXT.try_lock() {
        for (name, value) in map.iter() {
            let _ = writeln!(text, "  {name}: {value}");
        }
    }
    if let Ok(kept) = RAISED.try_lock() {
        if !kept.is_empty() {
            text.push_str("\nerrors raised (oldest first; the first is usually the cause):\n");
            let first_stack = kept.len().saturating_sub(STACKS_SHOWN);
            for (i, r) in kept.iter().enumerate() {
                let _ = writeln!(
                    text,
                    "  at {:.1} s, thread {}: {}",
                    r.seconds, r.thread, r.message
                );
                if i >= first_stack || i == 0 {
                    for line in r.stack.lines() {
                        let _ = writeln!(text, "      {line}");
                    }
                }
            }
        }
    }
    text
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_raised_error_reaches_the_report_with_its_stack() {
        super::start();
        super::context("test", "a fact".into());
        let e = mc_render::GpuError::NoDevice("raised in a test".into());
        super::on_gpu_error(&e);
        let text = super::describe();
        assert!(text.contains("test: a fact"), "{text}");
        let at = text.find("raised in a test").expect("the error is kept");
        // On Windows the frames are module+offset, named only with the build's .pdb.
        #[cfg(not(windows))]
        assert!(
            text[at..].contains("a_raised_error_reaches_the_report"),
            "its stack follows it: {text}"
        );
    }
}
