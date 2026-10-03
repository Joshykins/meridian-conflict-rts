//! The ARC Reclaimer: a salvage hovercraft with a reclaim head on an A-frame. Wreckage
//! lies on slopes, on ledges and down in gullies, so the head turns on a gun house of its
//! own (`MeshBuilder::with_house`, weapon slot 0) and pitches through a wide arc: it is
//! built round its trunnion, with open air above and below it.
//!
//! Reclaim is plant, not a weapon: dark processors, intake mouths, hoppers. The
//! light is the Materials red-orange: the intakes (`GLOW_MATERIALS`) and the glazing
//! that shows the haul (`pattern::MASS_FLOW`).

use glam::Vec3;

use super::parts::*;
use crate::builder::{MeshBuilder, Section};
use crate::material::*;
use crate::{part, pattern};

mod reclaimer;

pub(super) use reclaimer::reclaimer;

/// The light on a working intake.
const INTAKE: u32 = GLOW_MATERIALS;

/// The reclaim head as a gun house bound to `weapon`: a yaw collar at `base` height
/// under the trunnion `pivot`, the pitching lance on it (the reclaim tower's processor
/// tube, shortened: long, lean, plant on its back), `s` its scale (about 1 for a 2 m
/// head). Returns where the reclaim beam leaves (the intake mouth at rest).
fn reclaim_head(b: &mut MeshBuilder, weapon: usize, pivot: Vec3, base: f32, s: f32) -> Vec3 {
    let mut mouth = pivot;
    // Hull part, not `part::TURRET`: the shader carries a turret-part house round the unit's
    // turret pivot as well, which swung the head about the hull's origin.
    b.with_house(weapon, pivot, 0.0, |b| {
        collar(b, pivot, base, s);
        b.with_recoil(|b| mouth = lance(b, pivot, s));
    });
    mouth
}

/// What turns but does not pitch: a slewing ring and a saddle to carry the head.
fn collar(b: &mut MeshBuilder, pivot: Vec3, base: f32, s: f32) {
    let foot = v3(pivot.x, pivot.y, base);
    b.paint(ACCENT);
    b.prism(foot, b.sides(10), 0.78 * s, 0.72 * s, 0.22 * s);
    // A low saddle under the tube with a trunnion block each side.
    b.paint(PLATING);
    b.frustum(
        foot + Vec3::Z * 0.2 * s,
        v2(1.3 * s, 1.1 * s),
        v2(0.8 * s, 0.9 * s),
        (pivot.z - base - 0.4 * s).max(0.1),
        v2(-0.1 * s, 0.0),
    );
    mirror_about(b, pivot.y, |b| {
        let pivot = v3(pivot.x, 0.0, pivot.z);
        b.paint(ACCENT);
        b.block(
            v3(pivot.x - 0.3 * s, pivot.y + 0.42 * s, pivot.z - 0.5 * s),
            v3(pivot.x + 0.3 * s, pivot.y + 0.62 * s, pivot.z + 0.2 * s),
        );
    });
}

/// The intake mouth facing +x at `at`: a dark ring, collector vanes, the lit throat.
fn mouth(b: &mut MeshBuilder, at: Vec3, radius: f32) {
    let sides = b.sides(8);
    b.paint(ACCENT);
    b.cylinder_between(at - Vec3::X * 0.3 * radius, at, radius, radius * 0.9, sides);
    b.paint(INTAKE);
    b.cylinder_between(
        at - Vec3::X * 0.05,
        at + Vec3::X * 0.02,
        radius * 0.55,
        radius * 0.55,
        6,
    );
    if b.fine() {
        // Vanes across the mouth: it swallows, it does not fire.
        b.paint(ACCENT);
        for (y, z) in [(1.0, 0.0), (-1.0, 0.0)] {
            let c = at + v3(0.12 * radius, y * 0.72 * radius, z * 0.72 * radius);
            let size = v3(
                0.5 * radius,
                0.14 * radius + 0.3 * radius * z.abs(),
                0.14 * radius + 0.3 * radius * y.abs(),
            );
            b.cuboid(c, size);
        }
    }
}

/// The tower's processor tube, short and lean on a trunnion near its back third.
fn lance(b: &mut MeshBuilder, p: Vec3, s: f32) -> Vec3 {
    let tip = p + Vec3::X * 2.1 * s;
    let mouth_at = tip + Vec3::X * 0.12 * s;
    if !b.fine() {
        b.paint(ACCENT);
        b.beam(
            p - Vec3::X * 0.8 * s,
            tip,
            v2(0.8 * s, 0.9 * s),
            v2(0.45 * s, 0.45 * s),
        );
        b.paint(INTAKE);
        b.cuboid(mouth_at, v3(0.1, 0.3 * s, 0.3 * s));
        return mouth_at;
    }
    reclaim_gun(b, p - Vec3::X * 0.8 * s, tip, 0.2 * s);
    // The Materials light: the lit intake over the tube's mouth, glazing down the housing's
    // flanks that shows the haul going through, a lit strip along its lid.
    mouth(b, mouth_at, 0.4 * s);
    mirror_about(b, p.y, |b| {
        b.paint(ACCENT).pattern(pattern::MASS_FLOW);
        b.block(
            v3(p.x - 0.5 * s, 0.44 * s, p.z + 0.02 * s),
            v3(p.x + 0.85 * s, 0.49 * s, p.z + 0.42 * s),
        );
    });
    // Laid over the tube's own (orange) lid strip, so the lid reads in the Materials light.
    glow_strip(
        b,
        p + v3(0.12 * s, 0.0, 0.66 * s),
        v2(1.1 * s, 0.18 * s),
        INTAKE,
    );
    if b.fine() {
        b.paint(METAL);
        b.cylinder_between(
            p - Vec3::Y * 0.55 * s,
            p + Vec3::Y * 0.55 * s,
            0.16 * s,
            0.16 * s,
            b.sides(8),
        );
    }
    mouth_at
}

/// Runs `f` twice, mirrored across the plane `y`: inside it, y is measured from there.
fn mirror_about(b: &mut MeshBuilder, y: f32, f: impl Fn(&mut MeshBuilder)) {
    b.at(v3(0.0, y, 0.0), |b| b.mirror_y(|b| f(b)));
}

// ---- running gear and hulls ------------------------------------------------

/// The faceted white shell of a salvage hull from `belly` to `deck` over a [`hull_plan`]
/// of `half_width`, drawn in toward the deck. Returns the deck.
fn shell(
    b: &mut MeshBuilder,
    rear: f32,
    front: f32,
    half_width: f32,
    belly: f32,
    deck: f32,
) -> Roof {
    let length = front - rear;
    let nose = length * 0.12;
    let (scale, shift) = (v2(0.8, 0.8), -0.045 * length);
    b.paint(PLATING);
    let waist = belly + (deck - belly) * 0.4;
    if b.coarse() {
        b.frustum_open(
            v3((rear + front) * 0.5, 0.0, belly),
            v2(length, half_width * 2.0),
            v2(length * scale.x, half_width * 2.0 * scale.y),
            deck - belly,
            v2(shift, 0.0),
        );
    } else {
        b.loft_z(
            &hull_plan(rear, front, half_width, nose),
            &[
                Section::scaled(belly, 0.95, 0.74),
                Section::new(waist, 1.0),
                Section::scaled(deck, scale.x, scale.y).shifted(shift, 0.0),
            ],
        );
    }
    Roof {
        rear: (rear + nose * 0.45) * scale.x + shift,
        front: (front - nose) * scale.x + shift,
        half_width: half_width * scale.y,
        z: deck,
    }
}

/// An open salvage hopper on a deck at `at`: a glazed bin that shows the haul pouring in
/// while the unit reclaims (`pattern::MASS_FLOW`), a metal rim, scrap heaped in it.
pub(super) fn hopper(b: &mut MeshBuilder, at: Vec3, size: glam::Vec2, depth: f32) {
    b.paint(ACCENT).pattern(pattern::MASS_FLOW);
    b.frustum(at, size * 0.86, size, depth, v2(0.0, 0.0));
    b.pattern(pattern::GENERIC);
    if !b.fine() {
        return;
    }
    b.paint(METAL);
    b.frustum(
        at + Vec3::Z * depth,
        size * 1.02,
        size * 1.02,
        0.08,
        v2(0.0, 0.0),
    );
    if b.fine() {
        // Scrap heaped in the bin: tumbled plates.
        b.paint(PLATING_DARK);
        for (i, (u, v)) in [(-0.2, -0.15), (0.18, 0.12), (0.25, -0.2)]
            .into_iter()
            .enumerate()
        {
            let c = at + v3(u * size.x, v * size.y, depth + 0.05);
            let tilt = 0.3 + 0.25 * i as f32;
            b.pitched(c, tilt, |b| {
                b.cuboid(Vec3::ZERO, v3(size.x * 0.28, size.y * 0.24, 0.12));
            });
        }
    }
}

/// A glazed chute from `top` down to `bottom` carrying the haul (`pattern::MASS_FLOW`):
/// a dark square duct `width` across, a metal band at each end.
pub(super) fn chute(b: &mut MeshBuilder, top: Vec3, bottom: Vec3, width: f32) {
    b.paint(ACCENT).pattern(pattern::MASS_FLOW);
    b.beam(top, bottom, v2(width, width), v2(width, width));
    b.pattern(pattern::GENERIC);
    if !b.fine() {
        return;
    }
    b.paint(METAL);
    let d = (bottom - top).normalize_or_zero() * 0.12;
    for end in [top + d, bottom - d] {
        b.beam(
            end - d,
            end + d,
            v2(width * 1.25, width * 1.25),
            v2(width * 1.25, width * 1.25),
        );
    }
}

// ---- coarse level ------------------------------------------------------------

/// The coarse level of any land reclaimer, a few boxes: the running gear as one dark
/// slab `gear` (x rear..front, half width, height), a white hull up to `deck`, and a
/// block per head on its house so the heads still turn. `masts` pairs a head's pivot
/// with the z its column rises from (none for a head sat on the hull).
pub(super) fn coarse(
    b: &mut MeshBuilder,
    gear: (f32, f32, f32, f32),
    deck: f32,
    heads: &[(Vec3, f32)],
) {
    let (rear, front, half, height) = gear;
    let length = front - rear;
    let mid = (rear + front) * 0.5;
    b.with_part(part::LOCOMOTION, |b| {
        b.paint(TREAD);
        b.cuboid_open(v3(mid, 0.0, height * 0.5), v3(length, half * 2.0, height));
    });
    b.paint(PLATING);
    b.frustum_open(
        v3(mid, 0.0, height),
        v2(length * 0.96, half * 1.7),
        v2(length * 0.78, half * 1.36),
        deck - height,
        v2(-0.04 * length, 0.0),
    );
    for (i, &(pivot, foot)) in heads.iter().enumerate() {
        b.with_house(i, pivot, 0.0, |b| {
            // One head's column fits the budget; three heads sit on their blocks.
            if heads.len() == 1 {
                b.paint(ACCENT);
                b.cuboid_open(
                    v3(pivot.x, pivot.y, (foot + pivot.z) * 0.5),
                    v3(0.8, 0.8, pivot.z - foot),
                );
            }
            b.with_recoil(|b| {
                b.paint(PLATING);
                b.cuboid_open(pivot + Vec3::X * 0.4, v3(2.0, 1.0, 0.9));
            });
        });
    }
}
