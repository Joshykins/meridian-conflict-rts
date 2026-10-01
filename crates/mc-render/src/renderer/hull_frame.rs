//! A capital ship's hull as the entity shader draws it: placed, turned, pitched by its
//! spinal lay and rolled by its bank, so effects on its guns are laid where the guns are
//! drawn. Presentation only.

use glam::Vec3;
use mc_sim::mirror::UnitInstance;

/// The hull's pitch as the sim lays it (a spinal gun's slot 0, `hull_pitched`), for a
/// capital ship; zero for anything else.
pub(super) fn hull_pitch(u: &UnitInstance, mesh: &str) -> f32 {
    if crate::models::capital_rig(mesh).is_some() {
        u.arm_pitch[1]
    } else {
        0.0
    }
}

/// A hull as entity.wgsl `vs_main` draws it `f` of the way through the tick: at its
/// place, turned by its heading, pitched by a capital ship's spinal lay (`hull_pitch`)
/// and rolled by its bank. Effects on its gun houses are laid in it: laid level, a
/// pitched ship's guns charge and fire well off their barrels.
#[derive(Clone, Copy)]
pub(super) struct DrawnHull {
    pos: Vec3,
    heading: f32,
    pitch: f32,
    bank: f32,
}

impl DrawnHull {
    pub(super) fn of(u: &UnitInstance, mesh: &str, f: f32) -> Self {
        let turn = (u.heading - u.prev_heading + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        let pitch = if crate::models::capital_rig(mesh).is_some() {
            u.arm_pitch[0] + (u.arm_pitch[1] - u.arm_pitch[0]) * f
        } else {
            0.0
        };
        Self {
            pos: Vec3::from(u.prev_pos).lerp(Vec3::from(u.pos), f),
            heading: u.prev_heading + turn * f,
            pitch,
            bank: u._pad2[0] + (u._pad2[1] - u._pad2[0]) * f,
        }
    }

    /// A direction in the hull's frame (+X forward) in the world.
    pub(super) fn turn(&self, v: Vec3) -> Vec3 {
        let (s, c) = self.bank.sin_cos();
        let rolled = Vec3::new(v.x, v.y * c - v.z * s, v.y * s + v.z * c);
        rot_z(rot_xz(rolled, self.pitch), self.heading)
    }

    /// A point on the hull's model (metres, +X forward) in the world.
    pub(super) fn place(&self, local: Vec3) -> Vec3 {
        self.pos + self.turn(local)
    }
}

fn rot_z(v: Vec3, a: f32) -> Vec3 {
    let (s, c) = a.sin_cos();
    Vec3::new(v.x * c - v.y * s, v.x * s + v.y * c, v.z)
}

fn rot_xz(v: Vec3, a: f32) -> Vec3 {
    let (s, c) = a.sin_cos();
    Vec3::new(v.x * c - v.z * s, v.y, v.x * s + v.z * c)
}
