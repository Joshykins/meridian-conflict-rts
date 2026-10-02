//! Running gear for the Regency's tech 1 line (the Marauder, the Sledge and the Brazier:
//! `raider`, `hover_tank`, `mobile_aa`), so the three speak one language with the
//! Artificer and the Outrider (docs/STYLE.md "The Regency look"):
//!
//! - Lift: a bronze collar under the hull and a lift bell hanging from it, a dark core
//!   ringed in red where the lift leaves ([`lift_bell`]), marked for its red plasma
//!   (`MeshBuilder::add_lift`, renderer `lift_fx.rs`).
//! - Crawl legs ([`crawl_leg`]): a bronze bone under a keeled plate to a bronze knee, a
//!   faceted shin down to a pointed foot, posed by `entity.wgsl` `crawl_leg`.
//! - Hulls lofted from a plan ([`hull`]): a seam-dark belly under dark plate.
//!
//! No rams, no gears, no ribbed shafts: the joints are plain bronze drums and balls.

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::material::*;
use crate::{part, rig};

use super::commander::form::{ring, sleeve, KEEL, OCT};
use super::kit::{dark_plate, metal, seam, v3};
use super::machine::hoop;

/// Where each lift bell's mouth opens and the lift leaves it.
pub(super) const MOUTH: f32 = 0.2;

/// A lift bell under a hull whose underside is at `deck`, at `at` in plan, `r` across: a
/// bronze collar against the hull and the bell (`part::LOCOMOTION`) hanging from it, its
/// mouth a dark core ringed in red. The bell runs deep into its collar, so no gap opens
/// as the hull heaves over it (`set_hover`).
pub(super) fn lift_bell(b: &mut MeshBuilder, at: Vec2, r: f32, deck: f32) {
    let (x, y) = (at.x, at.y);
    b.add_lift(v3(x, y, MOUTH), r * 0.7);
    let sides = b.sides(10);
    metal(b);
    b.prism(v3(x, y, deck - 0.3), sides, r, r * 0.95, 0.32);
    b.with_part(part::LOCOMOTION, |b| {
        seam(b);
        b.prism(v3(x, y, 0.28), sides, r * 0.86, r * 0.84, deck - 0.5);
        b.prism(v3(x, y, MOUTH), sides, r * 0.55, r * 0.7, 0.08);
        if b.fine() {
            b.paint(GLOW_LASER);
            hoop(b, v3(x, y, 0.3), r * 0.7, 0.08, 0.05, 12);
        }
    });
}

/// Far off: the lift under a hull as one face, down, inside `plan` at `z`.
pub(super) fn coarse_lift(b: &mut MeshBuilder, plan: &[Vec2], z: f32) {
    b.with_part(part::LOCOMOTION, |b| {
        seam(b);
        let mut face: Vec<Vec3> = plan.iter().map(|p| p.extend(z)).collect();
        // Clockwise from above: it faces the ground.
        face.reverse();
        b.face(&face);
    });
}

/// A hull lofted up from `plan` (x, y): a seam-dark belly from `z0` to `z1` drawn in to
/// `belly` of the plan, then the dark plated body up to `z2`, drawn in to `top` and
/// shifted `back` toward the tail there, so its flanks slope like glacis plates.
#[derive(Clone, Copy, Debug)]
pub(super) struct Hull<'a> {
    pub(super) plan: &'a [[f32; 2]],
    pub(super) z: [f32; 3],
    pub(super) belly: f32,
    pub(super) top: f32,
    pub(super) back: f32,
}

pub(super) fn hull(b: &mut MeshBuilder, h: &Hull) {
    let [z0, z1, z2] = h.z;
    seam(b);
    b.with_facets(|b| {
        b.loft_z(
            h.plan,
            &[Section::new(z0, h.belly), Section::new(z1 + 0.02, 0.97)],
        )
    });
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            h.plan,
            &[
                Section::new(z1, 1.0),
                Section::new(z1 + (z2 - z1) * 0.45, 0.98),
                Section::new(z2, h.top).shifted(-h.back, 0.0),
            ],
        )
    });
}

/// The way a bone running `along` in the leg's plane bends out: square to it, in the
/// plane of `along` and `up`, on `up`'s side.
fn bend(along: Vec3, up: Vec3) -> Vec3 {
    let along = along.normalize();
    (up - along * up.dot(along))
        .try_normalize()
        .unwrap_or(Vec3::X)
}

/// One left crawl leg (`MeshBuilder::set_crawl_legs`), `w` its girth: a bronze bone under
/// a keeled plate from the hip up to a bronze knee drum, a faceted shin down to a
/// pointed foot whose tip is on the ground. Runs as the current pair (`with_pair`).
pub(super) fn crawl_leg(b: &mut MeshBuilder, hip: Vec3, knee: Vec3, foot: Vec3, w: f32) {
    let out = (foot - hip).truncate().extend(0.0).normalize();
    let axis = out.cross(Vec3::Z);
    let thigh = knee - hip;
    let shin = foot - knee;
    let ankle = knee + shin * 0.74;
    b.with_part(part::LOCOMOTION, |b| {
        b.with_limb(rig::THIGH, |b| {
            let n = bend(thigh, Vec3::Z + out * 0.3);
            if b.fine() {
                // The bone under the plate, seen past its edges up close.
                metal(b);
                b.cylinder_between(hip, knee, 0.2 * w, 0.16 * w, 6);
            }
            dark_plate(b);
            sleeve(
                b,
                &[
                    ring(hip + thigh * 0.1 + n * (0.14 * w), n, 0.34 * w, 0.22 * w),
                    ring(hip + thigh * 0.55 + n * (0.2 * w), n, 0.4 * w, 0.28 * w),
                    ring(knee - thigh * 0.1 + n * (0.12 * w), n, 0.28 * w, 0.2 * w),
                ],
                &KEEL,
            );
        });
        b.with_limb(rig::SHIN, |b| {
            let n = bend(shin, out);
            metal(b);
            let sides = b.sides(8);
            b.cylinder_between(
                knee - axis * (0.26 * w),
                knee + axis * (0.26 * w),
                0.24 * w,
                0.24 * w,
                sides,
            );
            dark_plate(b);
            sleeve(
                b,
                &[
                    ring(knee + shin * 0.06, n, 0.28 * w, 0.26 * w),
                    ring(knee + shin * 0.42, n, 0.26 * w, 0.24 * w),
                    ring(ankle, n, 0.17 * w, 0.15 * w),
                ],
                &OCT,
            );
            seam(b);
            b.cylinder_between(ankle, foot + Vec3::Z * 0.01, 0.15 * w, 0.01, b.sides(6));
        });
    });
}

/// Far off: a crawl leg as two flat bones that still walk.
pub(super) fn coarse_crawl_leg(b: &mut MeshBuilder, hip: Vec3, knee: Vec3, foot: Vec3, w: f32) {
    b.with_part(part::LOCOMOTION, |b| {
        dark_plate(b);
        let up = Vec3::Z * (0.25 * w);
        b.with_limb(rig::THIGH, |b| both(b, &[hip - up, knee, hip + up]));
        b.with_limb(rig::SHIN, |b| both(b, &[knee - up, knee + up, foot]));
    });
}

/// A flat piece seen from both sides (far off, where a bone is a sliver).
fn both(b: &mut MeshBuilder, points: &[Vec3]) {
    b.face(points);
    let back: Vec<Vec3> = points.iter().rev().copied().collect();
    b.face(&back);
}
