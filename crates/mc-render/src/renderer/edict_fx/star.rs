//! The Edict's star: the charge gathered at the firing point under the keel. Laid once a
//! tick as the power generators' stars are (plasma_puffs.wgsl `star_core`), from a spark to
//! a blinding white-hot point brighter than a warhead's flash: its face white, its rim the
//! prism's pastels, a red corona round it (the same star drawn in the Regency's red,
//! `nova_light`), lightning crawling in to it off the hull as it fills. In its last third of
//! a second it draws in hard, then lets go (`shot.rs`). Stalled, it gutters; let go, it
//! collapses.

use glam::Vec3;

use super::super::star_core_fx::{ARC, STAR};
use super::super::Renderer;
use super::Ship;
use crate::camera::Camera;
use crate::gpu_consts::puff;
use mc_sim::mirror::EdictStage;

/// A streamer licking off the corona.
const LICK: f32 = puff::WARP_STREAK as f32;
/// The star's light on the hull and the country: white, a breath of the prism's rose.
const LIGHT: Vec3 = Vec3::new(1.0, 0.86, 0.9);
/// The lens flare's colour at a glare of one: white going to the Regency's rose.
pub(super) const FLARE: Vec3 = Vec3::new(1.0, 0.62, 0.66);
/// Its face's radius (metres) as a spark and at its fullest.
const SPARK: f32 = 4.0;
const FULL: f32 = 95.0;
/// The share of the gather from which it draws in, and how far (its face's share left).
const DRAW_IN: f32 = 0.978;
const DRAWN: f32 = 0.38;
/// Its brightness (`appearance.rgb`) as a spark and at its fullest.
const GLARE_SPARK: f32 = 2.0;
const GLARE_FULL: f32 = 26.0;
/// Its light at its fullest (held for seconds: a warhead's flash is 5e7 for an instant)
/// and how far it reaches.
const POWER: f32 = 2.5e7;
const LIGHT_REACH: f32 = 6500.0;

/// The face's radius and its brightness at `share` of the gather.
pub(super) fn size(share: f32) -> (f32, f32) {
    let p = share.clamp(0.0, 1.0);
    let grown = SPARK + (FULL - SPARK) * p.powf(1.5);
    let drawn = ((p - DRAW_IN) / (1.0 - DRAW_IN)).clamp(0.0, 1.0);
    let r = grown * (1.0 - (1.0 - DRAWN) * drawn * drawn);
    let glare = GLARE_SPARK + (GLARE_FULL - GLARE_SPARK) * p.powi(3) * (1.0 + 0.8 * drawn);
    (r, glare)
}

/// Where the star hangs: under the keel at the firing point, its face clear of the plating
/// (its light is drawn at its face's front, so a star tucked into the hull would show
/// through it from above).
pub(super) fn centre(s: &Ship, r: f32) -> Vec3 {
    s.keel_point() - Vec3::Z * (r * 0.9 + 10.0)
}

impl Renderer {
    /// One ship's star this tick (gathering, or collapsing after it was let go).
    pub(super) fn edict_star(&mut self, s: &Ship, time: f32, near: bool, camera: &Camera) {
        let gathering = s.view.stage == EdictStage::Gathering;
        let mut share = s.view.progress;
        let mut gutter = 1.0;
        if let Some((when, was)) = s.let_go {
            // Let go: it collapses, its light dying within `FADE`.
            let k = 1.0 - (time - when) / crate::gpu_consts::edict::FADE;
            if k <= 0.0 {
                return;
            }
            share = was * k * k;
        } else if !gathering {
            return;
        } else if !s.view.powered {
            // Stalled: it gutters, catching and failing.
            gutter = 0.3 + 0.5 * self.scatter.unit();
        }
        let (r, glare) = size(share);
        let at = centre(s, r);
        let life = self.tick_seconds.clamp(0.03, 0.25) * 2.0;
        let seed = (s.id % 97) as f32 * 0.613;
        // The quad holds the corona too: the face is the middle 0.42 of it.
        let across = r / 0.42;
        // Its red corona: the star again, wider, in the Regency's red.
        let red = (3.0 + 34.0 * share * share) * gutter;
        self.push_lit(
            STAR,
            at,
            Vec3::ZERO,
            time,
            life,
            (across * 2.2, across * 2.2),
            Vec3::new(red, 0.0, 0.0),
            seed + 3.1,
        );
        self.push_lit(
            STAR,
            at,
            Vec3::ZERO,
            time,
            life,
            (across, across),
            Vec3::splat(glare * gutter),
            seed,
        );
        let fx = &mut self.edict_fx;
        let power = POWER * share.powi(3) * gutter + 2.0e5 * share;
        fx.lamps
            .push((at, LIGHT * power, 600.0 + LIGHT_REACH * share.sqrt()));
        fx.flares.push((
            at,
            (0.5 + 30.0 * share.powi(3)) * gutter,
            60.0 + 900.0 * share * share,
        ));
        if !near || camera.distance > 30_000.0 || s.let_go.is_some() {
            return;
        }
        self.edict_star_arcs(s, at, r, share, gutter, time);
        self.edict_star_licks(at, r, share, gutter, time);
    }

    /// Crimson streamers licking off the star's corona, more and longer as it fills: the
    /// Regency's red round the white.
    fn edict_star_licks(&mut self, at: Vec3, r: f32, share: f32, gutter: f32, time: f32) {
        if share < 0.3 {
            return;
        }
        let licks = (share * 3.0 * gutter) as usize;
        for _ in 0..licks {
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed() * 0.6,
            )
            .normalize_or(Vec3::X);
            let reach = r * (0.8 + 1.2 * self.scatter.unit()) * (0.5 + share);
            let (when, lasts) = (self.scatter.unit(), self.scatter.unit());
            self.push_warp(
                LICK,
                at + out * r * 0.8,
                out * reach,
                time + when * self.tick_seconds,
                0.25 + 0.2 * lasts,
                (r * 0.3, r * 0.1),
                Vec3::new(1.0, 0.06, 0.05) * (3.0 + 4.0 * share),
                0.4,
            );
        }
    }

    /// Lightning crawling in to the star off the hull round the firing point, more and
    /// longer as it fills, white-pink at the heart of each stroke.
    fn edict_star_arcs(&mut self, s: &Ship, at: Vec3, r: f32, share: f32, gutter: f32, time: f32) {
        if share < 0.2 {
            return;
        }
        let (fwd, side) = s.axes();
        let keel = s.keel_point();
        let strokes = (share * 4.0 * gutter) as usize + usize::from(self.scatter.unit() < share);
        for _ in 0..strokes {
            // A point on the hull's underside near the firing point, the reach of the
            // crawl shrinking in as the star fills.
            let spread = 120.0 + 520.0 * (1.0 - share);
            let off = fwd * self.scatter.signed() * spread
                + side * self.scatter.signed() * (40.0 + 120.0 * (1.0 - share));
            let from = keel + off + Vec3::Z * (10.0 + 30.0 * self.scatter.unit());
            let to = at + (from - at).normalize_or(Vec3::Z) * r * 0.7;
            let tint = Vec3::new(1.0, 0.55 + 0.4 * share, 0.62 + 0.3 * share);
            let lasts = 0.06 + 0.08 * self.scatter.unit();
            let thick = 4.0 + 10.0 * share;
            let when = time + self.scatter.unit() * self.tick_seconds;
            self.push_lit(
                ARC,
                from,
                to - from,
                when,
                lasts,
                (thick, thick),
                tint * (3.0 + 5.0 * share),
                0.0,
            );
        }
    }

    /// A gather let go before the shot: the star gutters out where it hung, a red pulse and
    /// a few last arcs.
    pub(super) fn edict_let_go(&mut self, from: Vec3, share: f32, time: f32) {
        if share <= 0.02 {
            return;
        }
        let (r, _) = size(share);
        let at = from - Vec3::Z * (r * 0.9 + 10.0);
        self.push_lit(
            STAR,
            at,
            Vec3::ZERO,
            time,
            0.5,
            (r * 4.0, r * 4.0),
            Vec3::new(2.0 + 4.0 * share, 0.0, 0.0),
            0.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_star_grows_then_draws_in_at_the_end() {
        let (spark, dim) = size(0.0);
        let (full, _) = size(DRAW_IN);
        let (drawn, blaze) = size(1.0);
        assert!(spark < 10.0 && dim < 3.0);
        assert!(full > 80.0);
        assert!(drawn < full * 0.5, "drawn in to {drawn} from {full}");
        assert!(blaze > GLARE_FULL);
    }
}
