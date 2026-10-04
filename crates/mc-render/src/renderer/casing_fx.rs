//! Spent casings thrown out of a gun's breech (`Weapon::casings`), one per round as each
//! round is drawn leaving: out of the side of the gun away from the hull and up, from the
//! ground; straight down out of an aircraft. Each tumbles to the ground (or splashes).

use super::{Renderer, PUFF_CASING};
use glam::Vec3;

/// Where and how one shot's casings leave the gun.
pub(super) struct Breech {
    /// The breech as the gun is drawn, and the bore's direction.
    pub at: Vec3,
    pub dir: Vec3,
    /// 1: thrown to the gun's right; -1: to its left (a gun on the hull's left side).
    pub outboard: f32,
    /// The gun's travel a tick, and ticks between its rounds (`mirror::round_gap`).
    pub travel: Vec3,
    pub gap_ticks: f32,
    /// An aircraft's speed: its casings leave at it and fall away behind.
    pub flying: Option<f32>,
    /// Metres across a casing.
    pub size: f32,
    /// Metres from the breech back to the muzzle (`Weapon::casings`): a long gun is a big
    /// one, high up on a big hull, and throws its casings harder and further, clear of
    /// its own housing.
    pub reach: f32,
}

/// How much harder than a light gun's a gun `reach` metres long throws its casings.
fn kick(reach: f32) -> f32 {
    (reach / 6.0).clamp(1.0, 3.0)
}

impl Renderer {
    /// One casing for each of a shot's `rounds`, `round_gap` seconds apart from `time`.
    pub(super) fn casings_thrown(&mut self, b: &Breech, rounds: u8, round_gap: f32, time: f32) {
        let side = b.dir.cross(Vec3::Z).normalize_or(Vec3::Y) * b.outboard;
        // They leave at the gun's own speed, then the air takes it off them.
        let carried = b.travel / self.tick_seconds.max(0.01);
        let kick = kick(b.reach);
        // The firer's own: a hull field round the breech must not hide them, as it would
        // anything it stands between the muzzle (the effect's origin) and.
        let outbound = std::mem::replace(&mut self.effect_outbound, true);
        let size = b.size * (0.7 + 0.3 * kick);
        for k in 0..rounds.max(1) {
            let jitter = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed(),
            );
            let (vel, life) = match b.flying {
                Some(speed) => {
                    let ahead = Vec3::new(b.dir.x, b.dir.y, 0.0).normalize_or_zero() * speed;
                    let drop = Vec3::Z * -(6.0 + 3.0 * self.scatter.unit());
                    (ahead + drop + side * jitter.x * 2.0 + jitter * 1.2, 1.6)
                }
                None => {
                    let throw = side * (4.0 + 3.0 * self.scatter.unit()) * kick
                        + Vec3::Z * (3.0 + 2.0 * self.scatter.unit()) * kick.sqrt()
                        - b.dir * kick;
                    // From high up they take longer to come down, and lie a while.
                    (
                        carried + throw + jitter * 1.1 * kick,
                        2.4 + 0.6 * self.scatter.unit() + (kick - 1.0) * 0.9,
                    )
                }
            };
            let shift = mc_sim::mirror::round_shift(b.travel.to_array(), k, b.gap_ticks);
            let (from, start) = (b.at + Vec3::from(shift), time + k as f32 * round_gap);
            self.push_puff(PUFF_CASING, from, vel, start, life, (size, size));
            self.casing_splash(from, vel, start, life, k % 2 == 0);
        }
        self.effect_outbound = outbound;
    }
}

#[cfg(test)]
mod tests {
    use super::kick;

    /// Every light gun keeps the throw it had; a giant's arm gun throws three times as hard.
    #[test]
    fn only_long_guns_throw_harder() {
        assert_eq!(kick(5.5), 1.0);
        assert_eq!(kick(27.5), 3.0);
    }
}
