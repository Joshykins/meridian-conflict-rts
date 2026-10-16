//! The window thread's CPU spans (`mc_core::perf`) over the last second, for a mark of a
//! bad moment (`issue_mark.rs`): where the frame's CPU time went, beside the GPU's passes
//! and the sim's phases. A live match keeps no other record of it, and a headless rerun
//! of the moment does not run the sim beside the renderer as play does.

use mc_core::perf::{Frame, Scope};
use std::collections::{BTreeMap, VecDeque};
use std::time::{Duration, Instant};

/// How much play a mark's table covers.
const KEEP: Duration = Duration::from_secs(1);

#[derive(Default)]
pub struct FrameSpans {
    open: Option<Scope>,
    frames: VecDeque<(Instant, Frame)>,
}

impl FrameSpans {
    /// At the start of the window thread's frame.
    pub fn begin(&mut self) {
        self.open = Some(Scope::begin());
    }

    /// At its end.
    pub fn end(&mut self, now: Instant) {
        let Some(scope) = self.open.take() else {
            return;
        };
        self.frames.push_back((now, scope.end()));
        while self
            .frames
            .front()
            .is_some_and(|(at, _)| now.duration_since(*at) > KEEP)
        {
            self.frames.pop_front();
        }
    }

    /// Each span's mean milliseconds a frame over the frames kept, largest first.
    pub fn means(&self) -> Vec<(&'static str, f64)> {
        let mut total: BTreeMap<&'static str, u64> = BTreeMap::new();
        for e in self.frames.iter().flat_map(|(_, f)| &f.entries) {
            if let Some(ns) = e.ns {
                *total.entry(e.name).or_default() += ns;
            }
        }
        let frames = self.frames.len().max(1) as f64;
        let mut out: Vec<(&'static str, f64)> = total
            .into_iter()
            .map(|(name, ns)| (name, ns as f64 / 1e6 / frames))
            .collect();
        out.sort_by(|a, b| b.1.total_cmp(&a.1));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_are_averaged_over_the_frames_kept() {
        let mut spans = FrameSpans::default();
        let start = Instant::now();
        for i in 0..4 {
            spans.begin();
            {
                let _t = mc_core::perf_span!("test.frame_spans");
                std::thread::sleep(Duration::from_millis(2));
            }
            spans.end(start + Duration::from_millis(100 * i));
        }
        let means = spans.means();
        let (_, ms) = means
            .iter()
            .find(|(name, _)| *name == "test.frame_spans")
            .unwrap();
        assert!(*ms >= 2.0, "{ms} ms a frame");
        // A frame more than a second after the first two drops them.
        spans.begin();
        spans.end(start + Duration::from_millis(1150));
        assert_eq!(spans.frames.len(), 3);
    }
}
