//! The Edict's cut through the cloud deck (clouds_sim.wgsl `cs_force`, kind `disturber::CUT`;
//! renderer/edict_fx): the one thing in the game that clears the clouds rather than stirring
//! them (approved 2026-10-13 as the exception to "disturbed, never cleared"). The shot cuts
//! a lane along its line, from under the firing point out to the mark, and a clearing round
//! the mark (a ring, or a round clearing), the cover in them gone within a second and the air churned; let go, the deck heals
//! back over them as any other cloud does, over about a minute.
use super::*;
use crate::gpu_consts::disturber::CUT;

/// At most this many cuts stir the weather at once, with room of their own in
/// `MAX_DISTURBERS`: two a shot (the lane and the clearing), a few shots at once. A cosmetic
/// cap: past it the newest cut leaves the cloud alone.
pub(super) const MOST_CUTS: usize = 8;
/// Seconds a cut holds the cover off before letting the deck heal.
const HOLD: f32 = 4.0;
/// Seconds the cut takes to bite to its full width.
const BITE: f32 = 0.35;

/// One cut: a lane from `from` to `to`, or a clearing round `from`.
#[derive(Clone, Copy, Debug)]
pub(super) struct Cut {
    from: Vec2,
    to: Vec2,
    lane: bool,
    /// A clearing's ring radius (zero: round), and the cut's half width, metres.
    ring: f32,
    half: f32,
    start: f32,
}

impl Sky {
    /// The Edict's shot cuts the deck: a lane `half` metres either side of the line from
    /// `from` to `to` (both on the ground's plane), and a ring round `to`: its radius (zero
    /// for a round clearing) and half width.
    pub(crate) fn cut(
        &mut self,
        from: Vec2,
        to: Vec2,
        half: f32,
        (ring, ring_half): (f32, f32),
        time: f32,
    ) {
        if !(from.is_finite() && to.is_finite() && half > 1.0 && half.is_finite()) {
            return;
        }
        for cut in [
            Cut {
                from,
                to,
                lane: true,
                ring: 0.0,
                half,
                start: time,
            },
            Cut {
                from: to,
                to,
                lane: false,
                ring: ring.max(0.0),
                half: ring_half.max(half),
                start: time,
            },
        ] {
            if self.cuts.len() < MOST_CUTS {
                self.cuts.push(cut);
            }
        }
    }

    /// The cuts still holding, as disturbers; spent ones are dropped.
    pub(super) fn cut_disturbers(&mut self, now: f32) -> impl Iterator<Item = Disturber> + '_ {
        self.cuts
            .retain(|c| now >= c.start - 1.0 && now - c.start < HOLD);
        self.cuts.iter().map(move |c| {
            let age = (now - c.start).max(0.0);
            // Bites to its width at once, holds, and lets go at the end of its hold.
            let hold = (age / BITE).min(1.0) * (1.0 - ((age - (HOLD - 0.4)) / 0.4).clamp(0.0, 1.0));
            Disturber {
                a: [c.from.x, c.from.y, c.half, CUT],
                b: [hold, c.ring, 0.0, age],
                c: [c.to.x, c.to.y, if c.lane { 1.0 } else { 0.0 }, 0.0],
            }
        })
    }
}
