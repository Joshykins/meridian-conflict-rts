//! Live shields as barriers to blast effects: shockwaves, tree blasts and sparks
//! stop at the glass, and air under a shield is still (common.wgsl
//! `barrier_crosses`, habitat.wgsl `shield_shelter`).

use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use mc_sim::mirror::ShieldInstance;

/// The upper half of an ellipsoid about `center`, and a wall of its radius from
/// the rim down to `min_z`. A dome on high ground drops that wall to the ground
/// (to the sea at most), as the sim's `shields::in_dome`; a hull field's wall
/// runs down to the unit's feet.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct EffectBarrier {
    pub(crate) center: [f32; 3],
    pub(crate) radius: f32,
    pub(crate) inverse_axes: [f32; 3],
    pub(crate) min_z: f32,
}

impl EffectBarrier {
    /// The barrier of a live shield. `water`: the map's sea level.
    pub(super) fn of(s: &ShieldInstance, water: f32) -> Self {
        let hull = s.packed & (1 << 25) != 0;
        let mut center = s.pos;
        if hull {
            center[2] += s.height * 0.5;
        }
        Self {
            center,
            radius: s.radius,
            inverse_axes: [
                1.0 / s.radius,
                1.0 / s.radius,
                if hull {
                    2.0 / s.height.max(0.1)
                } else {
                    1.0 / mc_data::dome_height_f32(s.radius)
                },
            ],
            min_z: if hull { s.pos[2] } else { water.min(s.pos[2]) },
        }
    }

    fn inside(&self, p: Vec3) -> bool {
        let q = (p - Vec3::from(self.center)) * Vec3::from(self.inverse_axes);
        if p.z >= self.center[2] {
            q.length_squared() < 0.9999
        } else {
            p.z >= self.min_z - 0.1 && q.truncate().length_squared() < 0.9999
        }
    }

    pub(super) fn crosses(&self, from: Vec3, to: Vec3) -> bool {
        let axes = Vec3::from(self.inverse_axes);
        let q = (from - Vec3::from(self.center)) * axes;
        let v = (to - from) * axes;
        if v.length_squared() < 0.0000001 || (self.inside(from) && self.inside(to)) {
            return false;
        }
        self.meets(q, v, from, to, false) || self.meets(q, v, from, to, true)
    }

    /// The segment meets the cap, or with `wall` the wall under it.
    fn meets(&self, q: Vec3, v: Vec3, from: Vec3, to: Vec3, wall: bool) -> bool {
        let (q, v) = if wall {
            (q.truncate().extend(0.0), v.truncate().extend(0.0))
        } else {
            (q, v)
        };
        let a = v.length_squared();
        if a < 0.0000001 {
            return false;
        }
        let b = q.dot(v);
        let disc = b * b - a * (q.length_squared() - 1.0);
        if disc <= 0.0 {
            return false;
        }
        for t in [(-b - disc.sqrt()) / a, (-b + disc.sqrt()) / a] {
            // A surface impact can emit back out, but cannot emit into the field.
            if !(-0.0001..=1.0).contains(&t) || (t <= 0.0001 && b >= 0.0) {
                continue;
            }
            let z = (from + (to - from) * t).z;
            let on = if wall {
                z < self.center[2] && z >= self.min_z - 0.1
            } else {
                z >= self.center[2]
            };
            if on {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::EffectBarrier;
    use glam::Vec3;

    #[test]
    fn shockwave_barriers_stop_crossings_but_allow_shared_interior_and_outward_sparks() {
        let b = EffectBarrier {
            center: [0.0; 3],
            radius: 10.0,
            inverse_axes: [0.1; 3],
            min_z: 0.0,
        };
        let p = |x, z| Vec3::new(x, 0.0, z);
        assert!(b.crosses(p(-20.0, 1.0), p(0.0, 1.0)));
        assert!(b.crosses(p(-20.0, 1.0), p(20.0, 1.0)));
        assert!(b.crosses(p(0.0, 1.0), p(20.0, 1.0)));
        assert!(!b.crosses(p(-2.0, 1.0), p(2.0, 1.0)));
        assert!(!b.crosses(p(-20.0, 12.0), p(20.0, 12.0)));
        assert!(!b.crosses(p(-20.0, -2.0), p(20.0, -2.0)));
        assert!(b.crosses(p(-10.0, 0.0), p(0.0, 0.0)));
        assert!(!b.crosses(p(-10.0, 0.0), p(-20.0, 0.0)));
        let hull = EffectBarrier {
            center: [0.0, 0.0, 3.0],
            radius: 4.0,
            inverse_axes: [0.25, 0.25, 1.0 / 3.0],
            min_z: 0.0,
        };
        assert!(hull.crosses(p(-8.0, 3.0), p(0.0, 3.0)));
        assert!(!hull.crosses(p(-8.0, 7.0), p(8.0, 7.0)));
    }

    #[test]
    fn a_dome_on_a_cliff_walls_off_the_ground_below_its_rim() {
        // Rim 40 m up, sea at 0: the wall runs from 40 m down to the sea.
        let b = EffectBarrier {
            center: [0.0, 0.0, 40.0],
            radius: 10.0,
            inverse_axes: [0.1; 3],
            min_z: 0.0,
        };
        let p = |x, z| Vec3::new(x, 0.0, z);
        assert!(b.crosses(p(-20.0, 5.0), p(0.0, 5.0)), "in through the wall");
        assert!(b.crosses(p(0.0, 5.0), p(20.0, 5.0)), "out through the wall");
        assert!(!b.crosses(p(-5.0, 5.0), p(5.0, 20.0)), "all under the dome");
        assert!(!b.crosses(p(-20.0, -3.0), p(20.0, -3.0)), "under the sea");
        assert!(
            !b.crosses(p(-20.0, 5.0), p(-12.0, 30.0)),
            "outside the wall"
        );
    }
}
