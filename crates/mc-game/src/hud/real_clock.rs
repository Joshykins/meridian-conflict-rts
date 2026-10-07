//! Real time spent in the match, shown beside the mission time on hover.
//!
//! Mission time follows the game speed and stops while paused; this is the wall clock.
//! It starts when the HUD first sees the match (or sees the tick go back, as a restart
//! does), so a match joined or loaded part-way counts from there.

#[derive(Default)]
pub(super) struct RealClock {
    /// `Ui::time` and the sim tick when the match was first seen.
    since: Option<(f32, u32)>,
}

impl RealClock {
    /// Seconds of real time since the match was first seen, at `now` (`Ui::time`).
    pub(super) fn elapsed(&mut self, now: f32, tick: u32) -> f32 {
        match self.since {
            Some((_, first)) if tick < first => self.since = Some((now, tick)),
            None => self.since = Some((now, tick)),
            _ => {}
        }
        self.since.map_or(0.0, |(t, _)| (now - t).max(0.0))
    }
}

#[cfg(test)]
mod tests {
    use super::RealClock;

    #[test]
    fn counts_from_first_sight_and_restarts_when_the_tick_goes_back() {
        let mut c = RealClock::default();
        assert_eq!(c.elapsed(10.0, 50), 0.0);
        assert_eq!(c.elapsed(70.0, 50), 60.0);
        assert_eq!(c.elapsed(80.0, 0), 0.0);
        assert_eq!(c.elapsed(95.0, 10), 15.0);
    }
}
