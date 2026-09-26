//! The Eye, the Naga's radar (a two-cell lot, 24 m square): a tall segmented stalk rooted
//! in five plated buttresses, carrying one great slit-pupilled eye (the faction's sigil)
//! that turns to scan.
//!
//! Shape: a knot of soft hide at the foot, five buttresses arched out to the lot's edge,
//! each plated in three pieces with gaps between them and propped by a ram from the stalk,
//! lesser roots between them. The stalk climbs in five segments, each sleeved in three
//! hide splints on bare vertebra drums. The head is a capsule of
//! soft hide plated over, a bare metal eyeball in its front under a heavy brow and lids,
//! a red iris (`GLOW_LASER`) round a black slit pupil, horns swept back off the brow.
//!
//! Rig: the head is `part::SPINNER` about the stalk's axis (`set_spinner_pivot`), and it
//! looks about (`set_spinner_scan`), not round and round; the stalk and roots are the hull.

use std::f32::consts::{PI, TAU};

use glam::{Affine3A, Vec2, Vec3};

use crate::models::builder::MeshBuilder;
use crate::models::material::*;
use crate::models::part;

use super::defense::{knot, root, stalk};
use super::kit::*;

/// Where the head turns, and the middle of the eyeball in it (facing +x).
const SPIN: Vec3 = Vec3::new(0.0, 0.0, 19.4);
const EYE: Vec3 = Vec3::new(1.1, 0.0, 20.7);
/// How much the skull is grown about its neck.
const HEAD_SCALE: f32 = 1.25;
/// The stalk: foot, top, and its radius at each.
const STALK: (f32, f32, f32, f32) = (3.4, 17.6, 1.4, 0.9);

pub(super) fn eye(b: &mut MeshBuilder, _tech: u8) {
    b.set_spinner_pivot(SPIN);
    // It looks about, slowly, one way and back: an eye, not a radar dish.
    b.set_spinner_scan();
    if b.coarse() {
        coarse(b);
        return;
    }
    base(b);
    let (z0, z1, r0, r1) = STALK;
    stalk(b, z0, z1, r0, r1, 5, 3);
    b.with_part(part::SPINNER, head);
}

/// Far off: a spread foot, a tapered stalk, the head, its red iris and the owner's colour.
fn coarse(b: &mut MeshBuilder) {
    hide(b);
    b.frustum_open(Vec3::ZERO, Vec2::splat(15.0), Vec2::splat(2.8), 3.6, Vec2::ZERO);
    b.frustum_open(v3(0.0, 0.0, 3.6), Vec2::splat(2.6), Vec2::splat(1.7), 15.0, Vec2::ZERO);
    b.with_part(part::SPINNER, |b| {
        hide(b);
        b.beam(v3(-4.25, 0.0, 21.3), v3(3.25, 0.0, 21.4), Vec2::new(4.0, 4.0), Vec2::new(5.2, 5.2));
        b.paint(GLOW_LASER);
        let x = 3.3;
        b.face(&[v3(x, -1.6, 20.0), v3(x, 1.6, 20.0), v3(x, 1.6, 22.8), v3(x, -1.6, 22.8)]);
        b.face(&[v3(x, -1.6, 20.0), v3(x, -1.6, 22.8), v3(x, 1.6, 22.8), v3(x, 1.6, 20.0)]);
        b.paint(TEAM);
        b.decal(v3(-1.25, 0.0, 23.62), Vec2::new(2.8, 2.2));
    });
}

/// The foot: a knot of soft hide, five plated buttresses propped by rams, roots between.
fn base(b: &mut MeshBuilder) {
    knot(b, 3.6, 3.8, 5, PI / 5.0);
    b.radial(5, buttress);
    for k in 0..5 {
        root(b, (k as f32 + 0.5) * TAU / 5.0, 2.6, 1.7, 7.2, 0.62);
    }
}

/// One buttress (the +x one): an arched tendon from the stalk to the lot's edge, three
/// plates along its back, a ram from high on the stalk, claws where it bites the ground.
fn buttress(b: &mut MeshBuilder) {
    under_hide(b);
    let spine = [
        (v3(1.0, 0.0, 7.0), 0.55, 0.5),
        (v3(2.7, 0.0, 5.3), 0.8, 0.66),
        (v3(5.0, 0.0, 3.1), 0.88, 0.68),
        (v3(7.5, 0.0, 1.35), 0.72, 0.58),
        (v3(9.6, 0.0, 0.46), 0.4, 0.38),
    ];
    segment(b, &spine, Vec3::Z);
    hide(b);
    let plates: [[(Vec3, f32, f32); 2]; 3] = [
        [(v3(1.5, 0.0, 7.05), 0.7, 0.38), (v3(3.0, 0.0, 5.6), 0.95, 0.46)],
        [(v3(3.6, 0.0, 4.9), 1.0, 0.48), (v3(5.7, 0.0, 3.05), 1.0, 0.46)],
        [(v3(6.3, 0.0, 2.55), 0.92, 0.42), (v3(8.3, 0.0, 1.35), 0.72, 0.34)],
    ];
    let count = if b.fine() { 3 } else { 2 };
    for p in &plates[..count] {
        shell(b, p, Vec3::Z);
    }
    if b.fine() {
        ram(b, v3(0.9, 0.0, 11.0), v3(4.3, 0.0, 4.6), 0.2);
        b.paint(GLOW_LASER);
        b.beam(v3(3.25, 0.0, 5.6), v3(3.45, 0.0, 5.35), Vec2::new(0.9, 0.12), Vec2::new(0.9, 0.12));
        b.beam(v3(5.95, 0.0, 3.1), v3(6.15, 0.0, 2.9), Vec2::new(0.9, 0.12), Vec2::new(0.9, 0.12));
        metal(b);
        b.mirror_y(|b| spike(b, v3(9.1, 0.3, 0.6), v3(10.5, 0.95, 0.02), 0.2));
        spike(b, v3(9.4, 0.0, 0.7), v3(10.9, 0.0, 0.02), 0.22);
        hide(b);
        blade(b, v3(4.9, 0.0, 3.9), v3(6.3, 0.0, 5.0), 0.34, Vec3::Y);
    }
}

/// The head: a bearing, and on it the skull.
fn head(b: &mut MeshBuilder) {
    metal(b);
    b.prism(v3(0.0, 0.0, 17.75), b.sides(10), 1.25, 1.1, 0.45);
    // Authored small and grown about the neck, so the eye is great for its stalk.
    let neck = v3(0.0, 0.0, 18.1);
    b.with(Affine3A::from_translation(neck) * Affine3A::from_scale(Vec3::splat(HEAD_SCALE)) * Affine3A::from_translation(-neck), skull);
}

/// The head above its bearing (at authoring size): a stem, a plated capsule, the eyeball
/// in its front under lids and brow, horns raked back, optic cables to the stem.
fn skull(b: &mut MeshBuilder) {
    let sides = b.sides(10);
    under_hide(b);
    b.cylinder_between(v3(0.0, 0.0, 18.1), v3(-0.5, 0.0, 19.6), 0.95, 0.85, sides);
    // The capsule: soft hide from the eyeball back, plated over the top and the flanks.
    segment(
        b,
        &[(EYE - v3(0.6, 0.0, 0.0), 1.9, 1.85), (v3(-1.2, 0.0, 20.8), 2.2, 2.0), (v3(-2.9, 0.0, 20.6), 1.6, 1.5), (v3(-3.9, 0.0, 20.3), 0.6, 0.6)],
        Vec3::Z,
    );
    hide(b);
    shell(b, &[(v3(0.2, 0.0, 22.0), 1.4, 0.5), (v3(-1.3, 0.0, 22.15), 1.6, 0.55), (v3(-3.0, 0.0, 21.65), 1.1, 0.4)], Vec3::Z);
    b.mirror_y(|b| {
        hide(b);
        shell(b, &[(v3(0.0, 1.85, 20.6), 1.2, 0.4), (v3(-1.5, 2.05, 20.7), 1.45, 0.45), (v3(-3.0, 1.55, 20.5), 0.95, 0.32)], v3(0.0, 1.0, 0.1));
    });
    eyeball(b);
    lids(b);
    if b.fine() {
        hide(b);
        spike(b, v3(-3.4, 0.0, 21.4), v3(-5.3, 0.0, 22.2), 0.34);
        b.mirror_y(|b| {
            b.paint(GLOW_LASER);
            b.beam(v3(-0.4, 1.35, 21.75), v3(-2.6, 1.25, 21.45), Vec2::new(0.1, 0.1), Vec2::new(0.1, 0.1));
            metal(b);
            cable(b, &[v3(-3.3, 0.7, 19.9), v3(-2.4, 0.9, 18.9), v3(-0.9, 0.75, 18.5)], 0.14);
        });
    }
    // The owner's colour: a chevron on the capsule's back plate.
    b.paint(TEAM);
    b.mirror_y(|b| b.beam(v3(-0.6, 0.3, 22.72), v3(-1.9, 1.05, 22.57), Vec2::new(0.45, 0.08), Vec2::new(0.45, 0.08)));
}

/// The eyeball: bare metal, a red iris standing proud of it, a black slit pupil.
fn eyeball(b: &mut MeshBuilder) {
    metal(b);
    let (sides, rings) = if b.fine() { (10, 7) } else { (9, 5) };
    b.spheroid(EYE, v3(1.9, 2.0, 2.0), sides, rings);
    b.paint(GLOW_LASER);
    let n = b.sides(14);
    b.cylinder_between(EYE + v3(1.3, 0.0, 0.0), EYE + v3(1.95, 0.0, 0.0), 1.45, 1.2, n);
    // The pupil: a pointed slit, upright.
    hide(b);
    let at = |y: f32, z: f32| EYE + v3(1.97, y, z);
    slab(b, [at(0.0, -1.05), at(-0.2, 0.0), at(0.0, 1.05), at(0.2, 0.0)], Vec3::X * 0.08);
}

/// The lids and brow: plates arched over and under the eyeball, the brow heavy and
/// horned, a lit seam under its lip.
fn lids(b: &mut MeshBuilder) {
    let arc = |z: f32, lift: f32, w: f32, h: f32| -> Vec<(Vec3, f32, f32)> {
        [-1.0f32, -0.55, 0.0, 0.55, 1.0]
            .into_iter()
            .map(|s| {
                let c = 1.0 - s * s;
                (EYE + v3(0.35 + 0.65 * c, s * 2.05, z + lift * c), w * (0.75 + 0.25 * c), h)
            })
            .collect()
    };
    hide(b);
    shell(b, &arc(1.05, 0.95, 0.85, 0.34), v3(0.55, 0.0, 1.0));
    shell(b, &arc(-1.05, -0.9, 0.8, 0.3), v3(0.55, 0.0, -1.0));
    // The brow: heavier, higher, jutting over the upper lid.
    shell(b, &arc(1.75, 1.05, 0.95, 0.5), v3(0.35, 0.0, 1.0));
    if b.fine() {
        b.mirror_y(|b| {
            hide(b);
            blade(b, EYE + v3(0.2, 1.6, 2.3), EYE + v3(-2.4, 2.4, 4.1), 0.42, v3(0.0, 1.0, 0.2));
            blade(b, EYE + v3(-0.6, 2.1, 1.3), EYE + v3(-2.8, 2.7, 2.1), 0.32, v3(0.0, 1.0, 0.2));
        });
        b.paint(GLOW_LASER);
        b.beam(EYE + v3(1.2, -1.0, 2.25), EYE + v3(1.2, 1.0, 2.25), Vec2::new(0.1, 0.08), Vec2::new(0.1, 0.08));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn eye_fits() {
        super::super::check("naga_eye", 7.0, 24.0, Some(2), &[]);
    }
}
