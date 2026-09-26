//! Measuring the sim: run ticks into a [`Report`], save it where asked, and
//! hold a run to a budget. Any test or probe can use these.
//!
//! ```ignore
//! let report = world.perf_ticks("paladins_vs_titan", 300, |_, _| Vec::new())?;
//! mc_sim::perf::save(&report);            // MERIDIAN_PERF_DIR=dir writes dir/<title>.json + .txt
//! mc_sim::perf::budget(&report, &[("sim.tick", 8.0), ("spatial.tested", 400_000.0)]);
//! ```
//!
//! What gets recorded each tick (`World::perf`):
//! - `sim.<phase>` spans: the broad phases `World::tick` has always timed.
//! - `fn.<system>` spans: every system call inside those phases.
//! - counters such as `spatial.tested`, `los.rays`, `terrain.raycast_steps`,
//!   and the same split by phase as `<phase>>counter` (e.g. `targeting>los.rays`).
//! - `size.*` gauges (units, projectiles, index entries, the index's widest radius)
//!   and `nav.*` path work done this tick.
//!
//! Spans inside pool chunks add up time from every worker thread, so a
//! parallel phase can show more `fn.*` time than wall time.

use mc_core::perf::Report;

use crate::command::PlayerCommand;
use crate::world::World;
use crate::SimError;

impl World {
    /// Runs `ticks` ticks, taking each tick's commands from `commands(tick_index, world)`,
    /// and returns what they cost.
    pub fn perf_ticks(
        &mut self,
        title: &str,
        ticks: u32,
        mut commands: impl FnMut(u32, &World) -> Vec<PlayerCommand>,
    ) -> Result<Report, SimError> {
        let mut report = Report::new(title);
        for t in 0..ticks {
            let c = commands(t, self);
            self.tick(&c)?;
            report.add(&self.perf, "sim.tick");
        }
        report.note("units at end", self.state.units.slots.live().to_string());
        report.note("final tick", self.state.tick.to_string());
        Ok(report)
    }
}

/// Writes `<MERIDIAN_PERF_DIR>/<title>.json` and `.txt` when that variable is
/// set, and prints the table when `MERIDIAN_PERF_PRINT=1` (or the dir is set).
pub fn save(report: &Report) {
    let dir = std::env::var_os("MERIDIAN_PERF_DIR");
    if dir.is_some() || std::env::var("MERIDIAN_PERF_PRINT").is_ok_and(|v| v == "1") {
        println!("{}", report.text());
    }
    if let Some(dir) = dir {
        let dir = std::path::PathBuf::from(dir);
        let _ = std::fs::create_dir_all(&dir);
        let file: String = report
            .title
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
            .collect();
        if let Err(e) = report.save(&dir.join(format!("{file}.json"))) {
            eprintln!("perf: could not write {}: {e}", dir.display());
        }
    }
}

/// Fails when any named value's mean per tick goes over its limit: spans in
/// milliseconds, counters in counts. Lists every miss, not just the first.
/// Set `MERIDIAN_PERF_NO_BUDGET=1` to only report (for a slow debug build).
pub fn budget(report: &Report, limits: &[(&str, f64)]) {
    let mut over = Vec::new();
    for &(name, limit) in limits {
        let span = report.mean_ms(name);
        let value = if span > 0.0 { span } else { report.mean_n(name) };
        if value > limit {
            over.push(format!("{name}: {value:.2} per tick, budget {limit}"));
        }
    }
    if over.is_empty() {
        return;
    }
    let msg = format!(
        "{} over budget:\n  {}\n{}",
        report.title,
        over.join("\n  "),
        report.text()
    );
    if std::env::var("MERIDIAN_PERF_NO_BUDGET").is_ok_and(|v| v == "1") {
        eprintln!("{msg}");
    } else {
        panic!("{msg}");
    }
}
