//! The Sower (`regency_t2_bombard`): the Regency's tech 2 bombardment walker. A long armoured
//! hull carried high on two tall forward-kneed legs in armoured boots, a keeled sensor head
//! with red slit optics; on its back a turning ring carries the launcher pod, two tubes
//! under a faceted shroud, hinged at its rear so it rises to its launch angle (`rake`) and
//! lobs its Heavy Gravitic Seekers high.
//!
//! Finish (docs/STYLE.md "The Regency look"): dark faceted plate (`form::sleeve`,
//! `form::blade`), dark bronze in the gaps (hip drums, the hinge, the ring), red optics and
//! the red-lit tube mouths. No violet: it does not build.
//!
//! Rig: the hull is `HULL`. The legs are one pair of two bones posed by `entity.wgsl`
//! `crawl_leg` (`MeshBuilder::set_crawl_legs`), the right half a cycle behind the left. The ring turns about the unit's middle (`part::TURRET`);
//! the pod pitches about the hinge (`rig::ARM_GUN` about `PIVOT`). The numbers match
//! `regency_t2_bombard` in `data/factions/regency/units/land_t2.ron`.

mod body;
mod legs;
mod pod;
#[cfg(test)]
mod tests;

use glam::Vec3;

use crate::builder::MeshBuilder;

/// The blueprint's radius and height: the model is authored at its unit's size.
pub(super) const RADIUS: f32 = 5.0;
pub(super) const HEIGHT: f32 = 7.0;

/// How far the hull (`body::hull`, authored low) is raised onto the legs.
const LIFT: f32 = 1.3;
/// The pod's hinge: the weapon's `pivot`.
const PIVOT: Vec3 = Vec3::new(-1.0, 0.0, 4.6 + LIFT);
/// The tube mouths: the weapon's `muzzles` (its `muzzle` is between them).
const MUZZLES: [Vec3; 2] = [
    Vec3::new(2.0, -0.62, 4.9 + LIFT),
    Vec3::new(2.0, 0.62, 4.9 + LIFT),
];
/// Where the ring on the back turns.
const RING: Vec3 = Vec3::new(0.0, 0.0, 3.85 + LIFT);

/// The left leg at rest: hip, knee, the boot's sole under the ankle, and where in the
/// cycle it lifts. Long and lean, the knee thrust forward.
const LEG: (Vec3, Vec3, Vec3, f32) = (
    Vec3::new(-0.2, 1.15, 2.7 + LIFT),
    Vec3::new(0.9, 1.45, 2.05),
    Vec3::new(-0.15, 1.6, 0.0),
    0.0,
);

pub(super) fn bombard(b: &mut MeshBuilder, _tech: u8) {
    b.set_crawl_legs(&[LEG], 4.0, 0.6, 0.7);
    b.set_turret_pivot(RING);
    b.set_arm_pivot(PIVOT);
    b.set_dust_line(1.0);
    let (hip, knee, foot, _) = LEG;
    if b.coarse() {
        body::coarse(b, hip, knee, foot);
        return;
    }
    body::hull(b, hip, foot);
    b.mirror_y(|b| b.with_pair(0, |b| legs::leg(b, hip, knee, foot)));
    pod::mount(b);
}

/// Runs `f` with the hull's frame: authored low, raised `LIFT` onto the legs.
fn lifted(b: &mut MeshBuilder, f: impl FnOnce(&mut MeshBuilder)) {
    b.with(glam::Affine3A::from_translation(Vec3::Z * LIFT), f);
}
