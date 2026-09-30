//! A submarine going down or coming up, at the waterline (`water_fx::sea_tick` calls
//! it for every hull on its way down or up).
//!
//! Ordered down, the boat vents its ballast tanks: white spray jets up off the deck
//! while the sea closes over it, foam washes along the hull as the deck goes under,
//! and a trail of air comes up from it once it is down. Coming up, the air comes
//! first, the sea boils over the hull, and when the deck breaks the surface water
//! pours off both sides of it. The dive itself is eased in the sim
//! (`mc_sim::naval::dive_z`) and the hull trims bow-down or bow-up with it
//! (entity.wgsl), so this is only the water.

use super::water_fx::{PUFF_DROPLET, PUFF_SPRAY};
use super::Renderer;
use glam::Vec3;
use mc_sim::mirror::{UnitInstance, UNIT_DIVE_GOAL, UNIT_DIVE_MASK};

/// A hull, as the sea sees it while it dives or surfaces.
pub(super) struct DivingHull {
    /// Where it is this tick, and where it was at the tick's start.
    pub from: Vec3,
    pub to: Vec3,
    /// Its heading as a unit vector on the water, and the one across it.
    pub fwd: Vec3,
    pub right: Vec3,
    /// Metres: half its length and beam on the water, and its height over the waterline.
    pub half_length: f32,
    pub half_beam: f32,
    pub height: f32,
}

impl Renderer {
    /// The water round a submarine on its way down or up, laid once a tick.
    pub(super) fn dive_churn(&mut self, u: &UnitInstance, hull: &DivingHull, time: f32) {
        let dive = u.status[0] & UNIT_DIVE_MASK;
        let down = u.status[0] & UNIT_DIVE_GOAL != 0;
        if (down && dive >= UNIT_DIVE_MASK) || (!down && dive == 0) {
            return;
        }
        let water = self.sea_level();
        let tick = self.tick_seconds.max(0.02);
        let DivingHull {
            from,
            to,
            fwd,
            right,
            half_length,
            half_beam,
            height,
        } = *hull;
        // Metres the deck stands over the water (negative: under it), and how near the
        // surface it is, 1 at the waterline and 0 a hull's height either side.
        let deck = to.z + height - water;
        let near = (1.0 - deck.abs() / height.max(1.0)).clamp(0.0, 1.0);
        // How much there is of it: a big boat moves more water.
        let size = (half_length / 20.0).clamp(0.6, 3.0);
        let along = |r: &mut Renderer| {
            from.lerp(to, r.scatter.unit()) + fwd * half_length * 0.85 * r.scatter.signed()
        };
        if down {
            // Venting: spray jets off the deck while there is deck above the water.
            if deck > -0.3 {
                let vents = (2.0 * size) as usize + 1;
                for _ in 0..vents {
                    if self.scatter.unit() > 0.55 + 0.4 * near {
                        continue;
                    }
                    let at = along(self);
                    let side = right * self.scatter.signed() * half_beam * 0.4;
                    let at = Vec3::new(at.x + side.x, at.y + side.y, water + deck.max(0.0) + 0.2);
                    let jet = Vec3::Z * (4.0 + self.scatter.unit() * 3.0) * size.sqrt()
                        + right * self.scatter.signed() * 1.2;
                    let start = time + self.scatter.unit() * tick;
                    let s = 0.5 + half_beam * 0.08;
                    self.push_puff(PUFF_SPRAY, at, jet, start, 1.6, (s, s * 3.5));
                    self.push_puff(PUFF_DROPLET, at, jet * 0.8, start, 1.4, (0.15, 0.3));
                }
            }
            // Down: its air comes up after it.
            if deck < -0.5 && self.scatter.unit() < 0.6 {
                let at = along(self);
                self.push_bubbles(
                    at + Vec3::Z * height * 0.5,
                    half_beam * 0.6,
                    2,
                    time,
                    tick,
                    0.5,
                );
            }
        } else if deck < -0.5 {
            // Blowing the tanks on the way up: air first, the surface boiling over it.
            if self.scatter.unit() < 0.7 {
                let at = along(self);
                self.push_bubbles(at + Vec3::Z * height, half_beam * 0.7, 3, time, tick, 0.6);
            }
        } else if deck < height * 0.9 {
            // Breaking the surface: water sheets off the deck over both sides.
            let sheets = (3.0 * size * (0.4 + near)) as usize;
            for _ in 0..sheets {
                let side = if self.scatter.unit() < 0.5 { -1.0 } else { 1.0 };
                let at = along(self) + right * side * half_beam * 0.8;
                let at = Vec3::new(at.x, at.y, water + deck.max(0.2) * 0.8);
                let vel = right * side * (1.0 + self.scatter.unit() * 1.5)
                    + Vec3::Z * (0.5 + self.scatter.unit())
                    + fwd * self.scatter.signed() * 0.5;
                let start = time + self.scatter.unit() * tick;
                self.push_puff(
                    PUFF_DROPLET,
                    at,
                    vel,
                    start,
                    1.2,
                    (0.2, 0.5 + half_beam * 0.04),
                );
            }
            if self.scatter.unit() < 0.35 * near {
                let at = along(self);
                let at = Vec3::new(at.x, at.y, water + 0.3);
                let s = 0.6 + half_beam * 0.1;
                self.push_puff(PUFF_SPRAY, at, Vec3::Z * 1.5, time, 1.8, (s, s * 3.0));
            }
        }
        // White water where the hull goes through the surface, either way: foam laid along
        // it in patches, most while the deck is at the waterline.
        if near > 0.05 && self.scatter.unit() < 0.25 + 0.45 * near {
            let at = along(self);
            let at = Vec3::new(at.x, at.y, water);
            let foam = 0.25 + 0.55 * near;
            self.push_ripple(at, time, half_beam * (1.0 + near), 3.5, 0.0, foam);
        }
    }
}
