//! Aster land reclaimers: the Gleaner (tech 1, a salvage truck with a head on an
//! A-frame) and the Thresher (tech 2, a wheeled carrier with three reclaim heads and
//! missile-defence lasers). Wreckage lies on slopes, on ledges and down in gullies, so every head
//! turns on a gun house of its own (`MeshBuilder::with_house`, weapon slot = head
//! index) and pitches through a wide arc: each head is built round its trunnion,
//! with open air above and below it.
//!
//! Reclaim is plant, not a weapon: dark processors, intake mouths, hoppers. The
//! light is the Materials red-orange: the intakes (`GLOW_MATERIALS`) and the glazing
//! that shows the haul (`pattern::MASS_FLOW`).
//!
//! The Cradle head is the ARC reclaim turret other models build theirs from:
//! [`cradle_turret`] (a whole house) and [`cradle_head`] (the pitching head alone).

use glam::Vec3;

use super::parts::*;
use crate::builder::{MeshBuilder, Section};
use crate::material::*;
use crate::{part, pattern};

mod gleaner;
mod thresher;

pub(super) use gleaner::gleaner;
pub(super) use thresher::thresher;

/// The light on a working intake.
const INTAKE: u32 = GLOW_MATERIALS;

/// The look of a reclaim head. Each is built facing +x round its trunnion.
#[derive(Clone, Copy)]
enum Head {
    /// A boxy processor slung between two yoke cheeks, a short intake snout.
    Cradle,
    /// The reclaim tower's processor tube, shortened: long, lean, plant on its back.
    Lance,
}

/// One reclaim head as a gun house bound to `weapon`: a yaw collar at `base` height
/// under the trunnion `pivot`, the pitching head on it, `s` its scale (about 1 for a
/// 2 m head). Returns where the reclaim beam leaves (the intake mouth at rest).
fn reclaim_head(
    b: &mut MeshBuilder,
    weapon: usize,
    pivot: Vec3,
    base: f32,
    s: f32,
    head: Head,
) -> Vec3 {
    let mut mouth = pivot;
    // Hull part, not `part::TURRET`: the shader carries a turret-part house round the unit's
    // turret pivot as well, which swung the head about the hull's origin.
    b.with_house(weapon, pivot, 0.0, |b| {
        collar(b, pivot, base, s, head);
        b.with_recoil(|b| {
            mouth = match head {
                Head::Cradle => cradle_head(b, pivot, s),
                Head::Lance => lance(b, pivot, s),
            };
        });
    });
    mouth
}

/// What turns but does not pitch: a slewing ring, and a yoke or a cup to carry the head.
fn collar(b: &mut MeshBuilder, pivot: Vec3, base: f32, s: f32, head: Head) {
    let foot = v3(pivot.x, pivot.y, base);
    b.paint(ACCENT);
    b.prism(foot, b.sides(10), 0.78 * s, 0.72 * s, 0.22 * s);
    match head {
        Head::Cradle => {
            // Two cheeks rising past the trunnion, a bridge between them low down.
            let top = pivot.z + 0.42 * s;
            b.paint(PLATING);
            b.block(
                v3(pivot.x - 0.55 * s, pivot.y - 0.36 * s, base + 0.2 * s),
                v3(pivot.x + 0.3 * s, pivot.y + 0.36 * s, base + 0.42 * s),
            );
            mirror_about(b, pivot.y, |b| {
                let pivot = v3(pivot.x, 0.0, pivot.z);
                b.paint(PLATING);
                b.extrude_y(
                    &[
                        [pivot.x - 0.5 * s, base + 0.2 * s],
                        [pivot.x + 0.34 * s, base + 0.2 * s],
                        [pivot.x + 0.3 * s, top - 0.1 * s],
                        [pivot.x + 0.08 * s, top],
                        [pivot.x - 0.3 * s, top],
                        [pivot.x - 0.5 * s, pivot.z - 0.1 * s],
                    ],
                    pivot.y + 0.62 * s,
                    pivot.y + 0.8 * s,
                );
                if b.fine() {
                    b.paint(METAL);
                    b.cylinder_between(
                        v3(pivot.x, pivot.y + 0.56 * s, pivot.z),
                        v3(pivot.x, pivot.y + 0.9 * s, pivot.z),
                        0.2 * s,
                        0.16 * s,
                        b.sides(8),
                    );
                }
            });
        }
        Head::Lance => {
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
    }
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

/// The Cradle reclaim turret, whole: a gun house bound to `weapon` with its slewing ring
/// at `base` height, yoke cheeks up to the trunnion `pivot`, and the processor head pitching
/// on them, `s` its scale (1 for a 2 m head). Returns the intake mouth at rest (the beam's
/// emitter). The reclaim turret other models build theirs from (the user's pick, 2026-09-28).
pub(crate) fn cradle_turret(
    b: &mut MeshBuilder,
    weapon: usize,
    pivot: Vec3,
    base: f32,
    s: f32,
) -> Vec3 {
    reclaim_head(b, weapon, pivot, base, s, Head::Cradle)
}

/// The Cradle's pitching head alone, facing +x round its trunnion `p`: a boxy processor
/// on side trunnions, a snout forward and a charge pack aft, `s` its scale. For a model
/// that builds its own house and mount, called inside `with_recoil`. Returns the intake
/// mouth.
pub(crate) fn cradle_head(b: &mut MeshBuilder, p: Vec3, s: f32) -> Vec3 {
    let tip = p + Vec3::X * 1.55 * s;
    if !b.fine() {
        b.paint(PLATING);
        b.cuboid(p + Vec3::X * 0.1 * s, v3(1.5 * s, 1.1 * s, 0.9 * s));
        b.paint(METAL);
        b.beam(
            p + Vec3::X * 0.8 * s,
            tip,
            v2(0.6 * s, 0.6 * s),
            v2(0.5 * s, 0.5 * s),
        );
        b.paint(INTAKE);
        b.cuboid(tip, v3(0.1, 0.35 * s, 0.35 * s));
        return tip;
    }
    b.paint(PLATING);
    b.chamfered_box(
        p + Vec3::X * 0.1 * s,
        v3(1.5 * s, 1.1 * s, 0.86 * s),
        0.22 * s,
    );
    b.paint(ACCENT);
    b.chamfered_box(
        p + v3(0.1 * s, 0.0, -0.1 * s),
        v3(1.56 * s, 1.16 * s, 0.22 * s),
        0.24 * s,
    );
    // Roof: a white lid with the intake's warning strip.
    b.paint(PLATING);
    b.plate(
        p + v3(-0.05 * s, 0.0, 0.43 * s),
        v2(1.1 * s, 0.8 * s),
        0.08 * s,
        0.03 * s,
    );
    if b.fine() {
        glow_strip(
            b,
            p + v3(0.45 * s, 0.0, 0.43 * s),
            v2(0.1 * s, 0.6 * s),
            INTAKE,
        );
    }
    // Charge pack behind the trunnion balances the snout.
    b.paint(ACCENT);
    b.block(
        p + v3(-1.05 * s, -0.38 * s, -0.3 * s),
        p + v3(-0.6 * s, 0.38 * s, 0.32 * s),
    );
    // Snout: a tapering tube out of the box, two collars, then the mouth.
    b.paint(METAL);
    b.cylinder_between(
        p + Vec3::X * 0.8 * s,
        tip - Vec3::X * 0.1 * s,
        0.34 * s,
        0.28 * s,
        b.sides(8),
    );
    if b.fine() {
        b.paint(ACCENT);
        b.cylinder_between(
            p + Vec3::X * 1.0 * s,
            p + Vec3::X * 1.1 * s,
            0.37 * s,
            0.37 * s,
            8,
        );
        // Feed pipes from the pack along the flanks into the snout.
        b.paint(METAL);
        mirror_about(b, p.y, |b| {
            let p = v3(p.x, 0.0, p.z);
            b.cylinder_between(
                p + v3(-0.8 * s, 0.5 * s, 0.1 * s),
                p + v3(0.85 * s, 0.5 * s, 0.1 * s),
                0.07 * s,
                0.07 * s,
                5,
            );
        });
    }
    mouth(b, tip, 0.4 * s);
    tip
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

/// An off-road tyre on the +y side, axis across the body: eight-sided like the other
/// running gear's round parts at this size, a hub cap at full detail.
fn tyre(b: &mut MeshBuilder, center: Vec3, radius: f32, width: f32) {
    b.with_part(part::LOCOMOTION, |b| {
        let half = Vec3::Y * (width * 0.5);
        b.paint(TREAD);
        b.cylinder_between(center - half, center + half, radius, radius, b.sides(8));
        if b.fine() {
            b.paint(PLATING);
            b.cylinder_between(
                center + half,
                center + half + Vec3::Y * 0.08,
                radius * 0.55,
                radius * 0.4,
                6,
            );
        }
    });
}

/// A wheeled hull: `axles` (x of each) with wheels of `wheel_r` at `track` half-gauge,
/// a dark belly tub between them, a faceted white shell from `belly` to `deck` over
/// a [`hull_plan`] of `half_width`, fenders over the wheels. Returns the deck.
pub(super) struct WheeledHull<'a> {
    pub rear: f32,
    pub front: f32,
    pub half_width: f32,
    pub belly: f32,
    pub deck: f32,
    pub axles: &'a [f32],
    pub wheel_r: f32,
    pub wheel_w: f32,
    pub track: f32,
}

pub(super) fn wheeled_hull(b: &mut MeshBuilder, h: &WheeledHull) -> Roof {
    let length = h.front - h.rear;
    let nose = length * 0.12;
    let (scale, shift) = (v2(0.8, 0.8), -0.045 * length);
    b.mirror_y(|b| {
        for &x in h.axles {
            tyre(b, v3(x, h.track, h.wheel_r), h.wheel_r, h.wheel_w);
        }
    });
    b.paint(ACCENT);
    // Axle tub between the wheels, kept inside the belly's V.
    let tub = (h.half_width * 0.7).min(h.track - h.wheel_w * 0.5);
    b.block(
        v3(h.rear + 0.9, -tub, h.wheel_r * 0.55),
        v3(h.front - 1.2, tub, h.belly + 0.1),
    );
    b.paint(PLATING);
    let waist = h.belly + (h.deck - h.belly) * 0.4;
    if b.coarse() {
        b.frustum_open(
            v3((h.rear + h.front) * 0.5, 0.0, h.belly),
            v2(length, h.half_width * 2.0),
            v2(length * scale.x, h.half_width * 2.0 * scale.y),
            h.deck - h.belly,
            v2(shift, 0.0),
        );
    } else {
        b.loft_z(
            &hull_plan(h.rear, h.front, h.half_width, nose),
            &[
                Section::scaled(h.belly, 0.95, 0.74),
                Section::new(waist, 1.0),
                Section::scaled(h.deck, scale.x, scale.y).shifted(shift, 0.0),
            ],
        );
    }
    if !b.coarse() {
        // Fenders: a dark flared arch over each pair of wheels, a white lip on it.
        b.mirror_y(|b| {
            let axles = h.axles;
            let mut i = 0;
            while i < axles.len() {
                let j = if i + 1 < axles.len() && (axles[i + 1] - axles[i]).abs() < h.wheel_r * 2.6
                {
                    i + 1
                } else {
                    i
                };
                let (x0, x1) = (axles[i].min(axles[j]), axles[i].max(axles[j]));
                let z = h.wheel_r * 2.0 + 0.12;
                b.paint(ACCENT);
                b.block(
                    v3(x0 - h.wheel_r * 1.1, h.half_width * 0.9, z - 0.1),
                    v3(x1 + h.wheel_r * 1.1, h.track + h.wheel_w * 0.55, z + 0.08),
                );
                if b.fine() {
                    b.paint(PLATING);
                    b.block(
                        v3(x0 - h.wheel_r * 1.15, h.track + h.wheel_w * 0.45, z - 0.3),
                        v3(x1 + h.wheel_r * 1.15, h.track + h.wheel_w * 0.6, z + 0.1),
                    );
                }
                i = j + 1;
            }
        });
    }
    Roof {
        rear: (h.rear + nose * 0.45) * scale.x + shift,
        front: (h.front - nose) * scale.x + shift,
        half_width: h.half_width * scale.y,
        z: h.deck,
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
