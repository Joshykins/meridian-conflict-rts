//! Plasma left burning on the ground where a held beam whose strikes burn
//! (`Weapon::burn_ticks`, the Reaper's Twin Plasmeric Beams) has raked it: the sim's
//! burning patches (`Fires::plasma`) are drawn here, laid as the beam sears the line, not
//! as fire. Low clumps of red plasma wake that keep flaring up along the line for as long
//! as it burns, embers rising off them, a red light over it, and dirt blown out where the
//! beam bites. Hard-edged, no mist (docs/STYLE.md "No mist").

use super::{Glow, HOT, MOTE, RED, WAKE};
use crate::renderer::Renderer;
use glam::{Vec2, Vec3};

/// Metres apart along the line the burning clumps are laid.
const STEP: f32 = 4.0;
/// Seconds one clump of the burning plasma lasts; fresh ones flare up through the burn.
const CLUMP_LIFE: f32 = 2.2;

impl Renderer {
    /// A held beam that leaves the ground burning struck open ground at `at` this tick,
    /// having swept on from `from` (where it struck before, when that was near). Its
    /// stream is `width` metres; the ground burns `burn` seconds.
    pub(in crate::renderer) fn plasma_left_burning(
        &mut self,
        from: Option<Vec2>,
        at: Vec3,
        width: f32,
        burn: f32,
        time: f32,
    ) {
        let to = at.truncate();
        let from = from.unwrap_or(to);
        let length = from.distance(to);
        let steps = (length / STEP).ceil().max(1.0) as usize;
        let across = (to - from).normalize_or(Vec2::X).perp();
        // Clumps flare up one after another through the burn; the last is gone with it.
        let flares = ((burn - CLUMP_LIFE * 0.5) / (CLUMP_LIFE * 0.7))
            .ceil()
            .max(1.0) as usize;
        for k in 1..=steps {
            let along = from.lerp(to, k as f32 / steps as f32);
            for f in 0..flares {
                let off = across * width * 1.6 * self.scatter.signed();
                let p = along + off;
                let ground = self.ground_height(p);
                let size = width * (0.8 + 0.6 * self.scatter.unit());
                let heat = 0.15 + 0.35 * self.scatter.unit();
                let start = time + f as f32 * CLUMP_LIFE * 0.7 + 0.3 * self.scatter.unit();
                let life =
                    (CLUMP_LIFE * (0.8 + 0.4 * self.scatter.unit())).min(time + burn - start);
                if life <= 0.2 {
                    continue;
                }
                self.push_lit(
                    WAKE,
                    p.extend(ground + size * 0.3),
                    Vec3::Z * 0.5,
                    start,
                    life,
                    (size, size * 1.4),
                    RED.lerp(HOT, heat) * 1.3,
                    0.0,
                );
            }
            // Embers lifting off it now and then while it burns.
            let p = along + across * width * self.scatter.signed();
            let ground = self.ground_height(p);
            let start = time + self.scatter.unit() * burn * 0.9;
            let drift = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0);
            let rise = 2.0 + 3.0 * self.scatter.unit();
            let life = 0.7 + 0.6 * self.scatter.unit();
            let heat = 0.3 + 0.4 * self.scatter.unit();
            self.push_lit(
                MOTE,
                p.extend(ground + 0.3),
                drift * 1.5 + Vec3::Z * rise,
                start,
                life,
                (0.2, 0.08),
                RED.lerp(HOT, heat) * 3.0,
                0.0,
            );
        }
        // Dirt blown out where the beam bites, and the burning line's red light on all
        // round it.
        self.plasma_dirt(at, width * 2.5, 4, time);
        self.plasma_fx.guns.light(Glow {
            pos: at + Vec3::Z * 1.5,
            color: RED * 30.0,
            range: 12.0 + width * 4.0,
            start: time,
            life: burn,
            pulse: 0.6,
        });
    }
}
