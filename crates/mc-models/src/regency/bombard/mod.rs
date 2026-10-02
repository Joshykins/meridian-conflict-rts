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
//! Rig: the hull is `HULL`. The legs walk as a biped's (`MeshBuilder::set_legs`, posed by
//! `entity.wgsl` `walk_leg`): thigh and shin solved to put the ankle where the boot has to
//! be, the boot (`rig::FOOT`) flat on the ground under it while planted, rolling off its
//! toe and reaching heel first for the next step, the hull settling onto bent knees and
//! riding over the planted leg. The ring turns about the unit's middle (`part::TURRET`);
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

/// The left leg at rest: hip, knee and ankle. Long and lean, the knee thrust forward.
const HIP: Vec3 = Vec3::new(-0.2, 1.15, 2.7 + LIFT);
const KNEE: Vec3 = Vec3::new(0.9, 1.45, 2.05);
const ANKLE: Vec3 = Vec3::new(-0.15, 1.6, 0.6);
/// The gait: ground to a full cycle, the share of it each boot is planted, how high a
/// boot is lifted on its way forward, and how far the hull settles onto its knees in
/// stride. The legs reach about 4 m, so a stride of 8 needs the knees bent.
const STRIDE: f32 = 8.0;
const STANCE: f32 = 0.5;
const STEP: f32 = 0.55;
const CROUCH: f32 = 0.2;
/// The hull's heft in its stride (`Legs::sway`): roll over the planted leg, the nose's
/// dip and the settle onto the knees as each boot comes down.
const SWAY: (f32, f32, f32) = (0.04, 0.035, 0.14);
/// The boot's sole: behind the ankle (the heel), ahead of it (the toe), across.
const SOLE: (f32, f32, f32) = (-0.9, 1.1, 0.8);

pub(super) fn bombard(b: &mut MeshBuilder, _tech: u8) {
    b.set_legs(HIP, KNEE, ANKLE, STRIDE, STANCE, STEP);
    b.set_walk_crouch(CROUCH);
    b.set_walk_sway(SWAY.0, SWAY.1, SWAY.2);
    b.set_foot(SOLE.0, SOLE.1, SOLE.2);
    b.set_turret_pivot(RING);
    b.set_arm_pivot(PIVOT);
    b.set_dust_line(1.0);
    if b.coarse() {
        body::coarse(b);
        return;
    }
    body::hull(b);
    b.mirror_y(legs::leg);
    pod::mount(b);
}

/// Runs `f` with the hull's frame: authored low, raised `LIFT` onto the legs.
fn lifted(b: &mut MeshBuilder, f: impl FnOnce(&mut MeshBuilder)) {
    b.with(glam::Affine3A::from_translation(Vec3::Z * LIFT), f);
}
