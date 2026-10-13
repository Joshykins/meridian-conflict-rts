//! The battle stepping back for one moment (`Audio::duck_world`): while a Seraph is
//! gone in a dash (`warp.rs`), every world sound and loop but the dash's own is held
//! down, so its leaving, the thunder of its crossing and its landing stand out; while a
//! Regent gathers its Edict and just after the shot (`edict.rs`), the same for the
//! Edict's. The interface and the music are left alone. The level falls fast and comes
//! back slowly, so the landing hits over a quiet field and the battle swells back in
//! under its tail.
//!
//! Several may ask in one tick: each asks (`Duck::request`), the deepest wins and every
//! asker's own sounds are spared, and the tick's sounds end by settling what was asked
//! (`Duck::settle`, `Audio::settle_duck`); a tick nobody asks lets the battle back.

/// Per output frame, the share of the way the level moves toward its target: going
/// down over about a tenth of a second, coming back over about a second and a half
/// (at 48 kHz).
const FALL: f32 = 0.0002;
const RISE: f32 = 0.000_014;

pub(super) struct Duck {
    /// The gain on every sound not spared, now and where it is going.
    level: f32,
    target: f32,
    /// Sound keys (`super::sound_key`) heard at full gain through the duck.
    spared: Vec<u32>,
    /// What this tick's askers want: the deepest depth, and all their spared keys.
    asked: Option<(f32, Vec<u32>)>,
}

impl Default for Duck {
    fn default() -> Duck {
        Duck {
            level: 1.0,
            target: 1.0,
            spared: Vec::new(),
            asked: None,
        }
    }
}

impl Duck {
    /// Asks, for this tick, that every sound not in `spared` be held down by `depth` (0:
    /// not at all, 1: silent). With others asking too, the deepest depth wins and all of
    /// their spared sounds are spared.
    pub(super) fn request(&mut self, depth: f32, spared: impl IntoIterator<Item = u32>) {
        let (deepest, keys) = self.asked.get_or_insert_with(|| (0.0, Vec::new()));
        *deepest = deepest.max(depth.clamp(0.0, 1.0));
        keys.extend(spared);
    }

    /// Takes up what this tick's askers want, or lets the battle back if none asked.
    pub(super) fn settle(&mut self) {
        let (depth, mut keys) = self.asked.take().unwrap_or_default();
        keys.sort_unstable();
        keys.dedup();
        self.target = 1.0 - depth;
        self.spared = keys;
    }

    /// Moves the level on by `frames` output frames: (its level at the start of them,
    /// at their end), for [`gain`] to run between.
    pub(super) fn advance(&mut self, frames: usize) -> (f32, f32) {
        let start = self.level;
        let k = if self.target < self.level { FALL } else { RISE };
        // A one-pole glide, over the whole block at once.
        self.level = self.target + (self.level - self.target) * (1.0 - k).powi(frames as i32);
        if self.level > 0.999 {
            self.level = 1.0;
        }
        (start, self.level)
    }

    /// Whether a sound plays through the duck at full gain.
    pub(super) fn spares(&self, key: u32) -> bool {
        self.spared.contains(&key)
    }
}

/// Whether a block that [`Duck::advance`] spans as `span` holds anything down at all.
pub(super) fn ducking(span: (f32, f32)) -> bool {
    span.0.min(span.1) < 1.0
}

/// The gain `i` frames into a block of `frames` that [`Duck::advance`] spans as `span`.
pub(super) fn gain(span: (f32, f32), i: usize, frames: usize) -> f32 {
    span.0 + (span.1 - span.0) * i as f32 / frames.max(1) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_fast_and_comes_back_slowly() {
        let mut d = Duck::default();
        d.request(0.7, [3]);
        d.settle();
        assert!(d.spares(3) && !d.spares(4));
        // A fifth of a second down: nearly all the way.
        let (_, down) = d.advance(9600);
        assert!(down < 0.42, "{down}");
        d.settle();
        // The same back up: still well under.
        let (_, up) = d.advance(9600);
        assert!(up < 0.6, "{up}");
        assert!(ducking(d.advance(48_000 * 4)));
        d.advance(48_000 * 8);
        assert!(!ducking(d.advance(512)));
    }

    #[test]
    fn askers_in_one_tick_merge_and_a_quiet_tick_lets_go() {
        let mut d = Duck::default();
        d.request(0.4, [1]);
        d.request(0.8, [2, 1]);
        d.settle();
        assert!(d.spares(1) && d.spares(2) && !d.spares(3));
        let (_, down) = d.advance(48_000);
        assert!(down < 0.25, "the deepest asked wins: {down}");
        // Nobody asked this tick: it comes back.
        d.settle();
        assert!(!d.spares(1));
        d.advance(48_000 * 12);
        assert!(!ducking(d.advance(512)));
    }
}
