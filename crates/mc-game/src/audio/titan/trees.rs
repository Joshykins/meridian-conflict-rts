//! Trees going over under a big walker's feet or a settling capital ship's hull
//! (`SimEvent::TreeTrampled`): `tree_fall` is the trunk snapping, the crown rushing down
//! and the crash as it lands, timed to the fall the renderer draws (fallen_trees.rs).
//! Presentation only.
//!
//! - Each tree is heard from where the walker stood as it went over (the event carries
//!   no height; the ear only measures height for the free camera, and a tree is within
//!   a walker's reach of it).
//! - A giant wading through a wood fells trees every tick, and a ship coming down
//!   presses dozens flat at once. A deliberate cap: falls start at most `RATE` a second
//!   (up to `BURST` together), the loudest first, so a forest going over is a run of
//!   crashes and never crowds the battle out of the mixer's voices.
//! - Each tree has its own pitch and a small lag, picked from its prop index, so a row
//!   going over together does not sound in step.

use crate::audio::Audio;
use glam::Vec3;
use mc_data::SoundId;
use mc_sim::SimEvent;

/// Falls heard a second, and the most heard together after a quiet spell.
const RATE: f32 = 2.5;
const BURST: f32 = 4.0;
/// A tree's pitch lies within this share either side of the recipe's.
const PITCH_SPREAD: f32 = 0.12;
/// A tree's fall starts up to this many seconds after its tick (the renderer's own
/// scatter is 0.15 s).
const MOST_LAG: f32 = 0.15;

pub(super) struct TreeFalls {
    /// Falls that may start now, refilled at `RATE` up to `BURST`.
    credit: f32,
}

impl Default for TreeFalls {
    fn default() -> Self {
        Self { credit: BURST }
    }
}

impl TreeFalls {
    /// This tick's felled trees, heard at `focus_height` over the ground they stood on.
    pub(super) fn tick(
        &mut self,
        events: &[SimEvent],
        sound: Option<SoundId>,
        audio: &Audio,
        focus_height: f32,
        tick_seconds: f32,
        hear: impl Fn(Vec3) -> (f32, f32),
    ) {
        self.credit = (self.credit + RATE * tick_seconds).min(BURST);
        let Some(sound) = sound else {
            return;
        };
        let mut falls: Vec<(f32, f32, u32)> = events
            .iter()
            .filter_map(|event| {
                let SimEvent::TreeTrampled { prop, from, .. } = event else {
                    return None;
                };
                let [x, y] = from.to_f32();
                let (gain, pan) = hear(Vec3::new(x, y, focus_height));
                (gain > 0.01).then_some((gain, pan, *prop))
            })
            .collect();
        falls.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (gain, pan, prop) in falls {
            if self.credit < 1.0 {
                break;
            }
            self.credit -= 1.0;
            let pitch = 1.0 + PITCH_SPREAD * (roll(prop, 1) * 2.0 - 1.0);
            audio.play_world_after(sound, gain, pan, pitch, MOST_LAG * roll(prop, 2));
        }
    }
}

/// A number in 0..1 that stays the same for one prop and `salt`.
fn roll(prop: u32, salt: u32) -> f32 {
    let mut h = prop.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 65535.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolls_spread_over_the_whole_range() {
        let rolls: Vec<f32> = (0..400).map(|p| roll(p, 1)).collect();
        assert!(rolls.iter().all(|r| (0.0..=1.0).contains(r)));
        assert!(rolls.iter().any(|&r| r < 0.1) && rolls.iter().any(|&r| r > 0.9));
        assert_ne!(roll(7, 1), roll(7, 2));
    }
}
