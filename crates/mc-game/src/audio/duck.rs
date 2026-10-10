//! The battle stepping back for one moment (`Audio::duck_world`): while a Seraph is
//! gone in a dash (`warp.rs`), every world sound and loop but the dash's own is held
//! down, so its leaving, the thunder of its crossing and its landing stand out. The
//! interface and the music are left alone. The level falls fast and comes back slowly,
//! so the landing hits over a quiet field and the battle swells back in under its tail.

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
}

impl Default for Duck {
    fn default() -> Duck {
        Duck {
            level: 1.0,
            target: 1.0,
            spared: Vec::new(),
        }
    }
}

impl Duck {
    /// Holds every sound not in `spared` down by `depth` (0: not at all, 1: silent).
    pub(super) fn set(&mut self, depth: f32, spared: impl IntoIterator<Item = u32>) {
        self.target = 1.0 - depth.clamp(0.0, 1.0);
        self.spared.clear();
        self.spared.extend(spared);
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
        d.set(0.7, [3]);
        assert!(d.spares(3) && !d.spares(4));
        // A fifth of a second down: nearly all the way.
        let (_, down) = d.advance(9600);
        assert!(down < 0.42, "{down}");
        d.set(0.0, []);
        // The same back up: still well under.
        let (_, up) = d.advance(9600);
        assert!(up < 0.6, "{up}");
        assert!(ducking(d.advance(48_000 * 4)));
        d.advance(48_000 * 8);
        assert!(!ducking(d.advance(512)));
    }
}
