//! The Regency strategic launchers, authored at blueprint scale (metres): the Mangonel,
//! the silo that throws a Pinch-fusion Warhead (8 x 8 lot, 26 m), and the Barbican, the
//! array of Gravitic Interceptors that answers one (4 x 4 lot, 20 m). Machines all through
//! (docs/STYLE.md "The Regency look"): dark plates lapped over dark bronze workings, red
//! only as inlaid lines and lens faces, no gears, rams or ribbed shafts.
//!
//! They work as ARC's Sunfall and Parhelion do (`aster::strategic`), by the same parts, so
//! the same shader moves them:
//! - The blast doors (`part::SILO_DOOR`) meet on y = 0 and slide apart along y: 5.2 m on
//!   the silo (`IconKind::Silo`), 5 m on the array. The silo's tube is at the middle,
//!   where the warhead leaves from (`mc_sim::nukes::warhead_start`); the array's four
//!   cells are where its interceptors leave from (`mc_sim::nukes` `CELLS`).
//! - The rounds held (`part::SILO_ROUND`): the warhead standing in its tube, an
//!   interceptor in each cell, the cell at quadrant (sign x, sign y) drawn while it holds.
//! - The load cycle (`gpu_consts::launcher`): a store's hatch lid (`part::LAUNCHER_LID`)
//!   slides back (along -x on the silo, -y on the array), the gravity lift's floating
//!   collar (`part::LAUNCHER_HOIST`) sinks into the well and rises again, and the lid
//!   shuts. Nothing hangs the lift (no frame, beam or cable, unlike ARC's cranes): low
//!   emitter posts round the well hold it ([`gravity_lift`]).
//!
//! The Regency warhead is theirs, not ARC's white one: a faceted dark-plate body banded
//! in bronze, a containment collar under its nose and thin red seams.
//!
//! The silo is a low armoured vault ringed by lens pylons (`silo_vault`); the array a
//! launch block beside a ringed sensor mast (`array_mast`). Picked by the user from three
//! designs each on 2026-10-02.

mod array_mast;
mod silo_vault;

pub(super) use array_mast::array_mast;
pub(super) use silo_vault::silo_vault;

use std::f32::consts::TAU;

use glam::{Vec2, Vec3};

use crate::builder::{ngon, MeshBuilder, Section};
use crate::gpu_consts::launcher;
use crate::material::*;
use crate::part;

use super::kit::{dark_plate, metal, seam, v3};
use super::machine::*;
use super::turrets::sunspear::lens;

/// The silo's (radius, height), and the array's.
pub(super) const SILO_SIZE: (f32, f32) = (42.5, 26.0);
pub(super) const ARRAY_SIZE: (f32, f32) = (18.75, 20.0);

/// The silo's bore: an octagon this far to its corners (its widest across y is
/// `cos 22.5` of it, under the 5.2 m each leaf slides).
const BORE: f32 = 5.2;
/// The warhead's radius to its corners.
const WARHEAD_R: f32 = 2.5;

/// The array's cells: centres at (±`CELL`, ±`CELL`) (`mc_sim::nukes` `CELLS`), each open
/// `CELL_HALF` either way, down to `CELL_FLOOR`.
const CELL: f32 = 2.4;
const CELL_HALF: f32 = 1.75;
const CELL_FLOOR: f32 = 1.0;
const INTERCEPTOR_R: f32 = 0.66;

/// The direction at bearing `a` (radians) round z.
fn out(a: f32) -> Vec3 {
    v3(a.cos(), a.sin(), 0.0)
}

/// An octagon's corners (a flat to +x) of radius `r` at height `z`, about `c`.
fn octagon(c: Vec2, r: f32, z: f32) -> Vec<Vec3> {
    ngon(8, r)
        .iter()
        .map(|p| v3(c.x + p[0], c.y + p[1], z))
        .collect()
}

/// A faceted octagonal drum about `c` from radius `r0` at `z0` to `r1` at `z1`, its top
/// capped or left open (for a deck or roof laid over it).
fn drum(b: &mut MeshBuilder, c: Vec2, r0: f32, r1: f32, z0: f32, z1: f32, top: bool) {
    b.with_facets(|b| b.loft(&[octagon(c, r0, z0), octagon(c, r1, z1)], false, top));
}

/// A flat ring facing up at `z` about `c`: an octagon of radius `inner` (its corners on the
/// same bearings) out to `outer`. Painted with the current brush.
fn deck_ring(b: &mut MeshBuilder, c: Vec2, inner: f32, outer: f32, z: f32) {
    let (i, o) = (octagon(c, inner, z), octagon(c, outer, z));
    for k in 0..8 {
        let j = (k + 1) % 8;
        b.face(&[i[k], o[k], o[j], i[j]]);
    }
}

/// A roof facing up at `z` from the convex `inner` outline (counter-clockwise, round the
/// origin) out to the convex `outer` one: a sector for each inner edge, out along the
/// rays through its ends.
fn roof(b: &mut MeshBuilder, inner: &[Vec2], outer: &[Vec2], z: f32) {
    let hit = |d: Vec2| -> Vec2 {
        (0..outer.len())
            .find_map(|k| {
                let (p, q) = (outer[k], outer[(k + 1) % outer.len()]);
                let e = q - p;
                let den = d.perp_dot(e);
                if den.abs() < 1e-6 {
                    return None;
                }
                let t = p.perp_dot(e) / den;
                let s = p.perp_dot(d) / den;
                (t > 0.0 && (-1e-4..=1.0 + 1e-4).contains(&s)).then_some(d * t)
            })
            .unwrap_or(d)
    };
    let bearing = |p: Vec2| p.y.atan2(p.x).rem_euclid(TAU);
    for k in 0..inner.len() {
        let (a, c) = (inner[k], inner[(k + 1) % inner.len()]);
        let (a0, mut a1) = (bearing(a), bearing(c));
        if a1 <= a0 {
            a1 += TAU;
        }
        let mut pts = vec![a.extend(z), hit(a.normalize()).extend(z)];
        let mut corners: Vec<(f32, Vec2)> = outer
            .iter()
            .map(|&p| {
                let mut t = bearing(p);
                if t <= a0 {
                    t += TAU;
                }
                (t, p)
            })
            .filter(|&(t, _)| t < a1 - 1e-4)
            .collect();
        corners.sort_by(|x, y| x.0.total_cmp(&y.0));
        pts.extend(corners.iter().map(|&(_, p)| p.extend(z)));
        pts.push(hit(c.normalize()).extend(z));
        pts.push(c.extend(z));
        b.face(&pts);
    }
}

/// The corners of an octagon of radius `r` about the origin, in plan.
fn plan_octagon(r: f32) -> Vec<Vec2> {
    ngon(8, r).iter().map(|p| Vec2::new(p[0], p[1])).collect()
}

/// A wall of a well, facing in: from `a` to `c` counter-clockwise round the inside seen
/// from above, `z0` to `z1` up.
fn wall_in(b: &mut MeshBuilder, a: Vec2, c: Vec2, z0: f32, z1: f32) {
    b.face(&[a.extend(z0), a.extend(z1), c.extend(z1), c.extend(z0)]);
}

/// The silo's bore: an octagonal well from `top` down to `floor`, lined in courses of
/// dark plate and bronze (in bands, so no face spans much of a pit's squeezed depth),
/// bronze guide rails down every other facet, and a floor.
fn bore(b: &mut MeshBuilder, top: f32, floor: f32) {
    let ring = plan_octagon(BORE);
    let fine = b.fine();
    let mut z = top;
    let mut k = 0;
    while z > floor + 1e-3 {
        let z0 = (z - if fine { 4.0 } else { 8.0 }).max(floor);
        if k % 2 == 0 {
            dark_plate(b);
        } else {
            seam(b);
        }
        for i in 0..8 {
            wall_in(b, ring[i], ring[(i + 1) % 8], z0, z);
        }
        z = z0;
        k += 1;
    }
    seam(b);
    b.face(&ring.iter().map(|p| p.extend(floor)).collect::<Vec<_>>());
    if fine {
        // The guide rails the warhead's shoes ride, on every other facet.
        let apothem = BORE * (TAU / 16.0).cos();
        metal(b);
        for i in 0..4 {
            b.yawed(Vec3::ZERO, TAU * i as f32 / 4.0, |b| {
                b.block(
                    v3(apothem - 0.4, -0.25, floor + 0.3),
                    v3(apothem - 0.02, 0.25, top - 0.4),
                );
            });
        }
    }
}

/// The Regency warhead standing in its tube (`part::SILO_ROUND`, set by the caller), its
/// nose's tip at `nose` and its foot at `base`: a faceted dark-plate nose with a bronze
/// tip, under it the bronze containment collar between two thin red seams, the body in
/// dark plate banded in bronze, red seam lines down four of its facets, bronze launch
/// shoes out to the tube's rails.
fn warhead(b: &mut MeshBuilder, nose: f32, base: f32) {
    let r = WARHEAD_R;
    let fine = b.fine();
    let plan = ngon(8, 1.0);
    let shoulder = nose - r * 2.9;
    b.with_facets(|b| {
        dark_plate(b);
        let tip = nose - 0.7;
        let ogive: &[(f32, f32)] = if fine {
            &[
                (tip, 0.32),
                (nose - 1.6, 0.62),
                (nose - 3.0, 0.88),
                (shoulder, 1.0),
            ]
        } else {
            &[(tip, 0.32), (nose - 2.2, 0.76), (shoulder, 1.0)]
        };
        let sections: Vec<Section> = ogive.iter().map(|&(z, s)| Section::new(z, r * s)).collect();
        b.loft_z(&plan, &sections);
        metal(b);
        b.loft_z(
            &plan,
            &[Section::new(tip, r * 0.32), Section::new(nose, 0.06)],
        );
        // The body in courses of one radius (a band proud of it would fight it down the
        // pit, where depth is squeezed); the red seams a little sunk.
        let courses: &[(f32, f32, u32)] = &[
            (shoulder - 0.2, shoulder, GLOW_LASER),
            (shoulder - 1.6, shoulder - 0.2, METAL),
            (shoulder - 1.8, shoulder - 1.6, GLOW_LASER),
            (shoulder - 7.0, shoulder - 1.8, PLATING_DARK),
            (shoulder - 7.8, shoulder - 7.0, METAL),
            (base + 1.0, shoulder - 7.8, PLATING_DARK),
            (base, base + 1.0, METAL),
        ];
        for &(z0, z1, m) in courses {
            if m == GLOW_LASER {
                b.paint(m);
            } else if m == METAL {
                metal(b);
            } else {
                dark_plate(b);
            }
            let k = if m == GLOW_LASER { 0.96 } else { 1.0 };
            b.loft_z(&plan, &[Section::new(z0, r * k), Section::new(z1, r * k)]);
        }
    });
    if fine {
        // Red seam lines down four of the facets, under the collar.
        let apothem = r * (TAU / 16.0).cos();
        for i in 0..4 {
            let a = TAU * i as f32 / 4.0;
            let d = out(a);
            red_slot(
                b,
                d * apothem + Vec3::Z * (shoulder - 4.2),
                d,
                Vec3::Z,
                3.6,
                0.1,
            );
        }
        // Launch shoes out to the rails, on the diagonals' facets.
        let rail = BORE * (TAU / 16.0).cos() - 0.4;
        metal(b);
        for i in 0..4 {
            b.yawed(Vec3::ZERO, TAU * i as f32 / 4.0, |b| {
                for z in [shoulder - 3.0, base + 3.0] {
                    b.block(v3(apothem - 0.05, -0.22, z), v3(rail - 0.04, 0.22, z + 0.5));
                }
            });
        }
    }
}

/// A Gravitic Interceptor in its cell at `at` (`part::SILO_ROUND`, set by the caller), its
/// nose's tip at `nose`: a slim faceted dark body from the cell's floor, a bronze band and
/// a thin red seam under a dark nose with a bronze tip.
fn interceptor(b: &mut MeshBuilder, at: Vec2, nose: f32) {
    let r = INTERCEPTOR_R;
    let fine = b.fine();
    let plan = ngon(if fine { 8 } else { 6 }, 1.0);
    let shoulder = nose - 2.3;
    let s = |z: f32, k: f32| Section::new(z, r * k).shifted(at.x, at.y);
    b.with_facets(|b| {
        metal(b);
        b.loft_z(&plan, &[s(nose - 0.35, 0.3), s(nose, 0.05)]);
        dark_plate(b);
        b.loft_z(
            &plan,
            &[s(shoulder, 1.0), s(nose - 1.1, 0.72), s(nose - 0.35, 0.3)],
        );
        b.paint(GLOW_LASER);
        b.loft_z(&plan, &[s(shoulder - 0.12, 0.95), s(shoulder, 0.95)]);
        metal(b);
        b.loft_z(&plan, &[s(shoulder - 0.6, 1.0), s(shoulder - 0.12, 1.0)]);
        dark_plate(b);
        b.loft_z(&plan, &[s(CELL_FLOOR + 0.05, 1.0), s(shoulder - 0.6, 1.0)]);
    });
}

/// The array's four cells sunk in a roof at `top`: square wells round (±`CELL`, ±`CELL`)
/// down to `CELL_FLOOR`, bronze guide rails in their corners, a red mark on each floor.
fn cells(b: &mut MeshBuilder, top: f32) {
    let h = CELL_HALF;
    for (sx, sy) in [(-1.0, -1.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
        let m = Vec2::new(sx * CELL, sy * CELL);
        let corner = [
            m + Vec2::new(h, h),
            m + Vec2::new(-h, h),
            m + Vec2::new(-h, -h),
            m + Vec2::new(h, -h),
        ];
        dark_plate(b);
        for k in 0..4 {
            wall_in(b, corner[k], corner[(k + 1) % 4], CELL_FLOOR, top);
        }
        seam(b);
        b.decal(m.extend(CELL_FLOOR), Vec2::splat(h * 2.0));
        if b.fine() {
            metal(b);
            for k in corner {
                let at = m + (k - m) * 0.88;
                b.cuboid(
                    at.extend((CELL_FLOOR + top) * 0.5),
                    v3(0.16, 0.16, top - CELL_FLOOR - 0.2),
                );
            }
        }
    }
}

/// The roof over the four cells at `top`: the web between them and the deck out to
/// `outer` (convex, counter-clockwise).
fn cell_roof(b: &mut MeshBuilder, top: f32, outer: &[Vec2]) {
    let (i, w) = (CELL + CELL_HALF, CELL - CELL_HALF);
    let square = [
        Vec2::new(i, -i),
        Vec2::new(i, i),
        Vec2::new(-i, i),
        Vec2::new(-i, -i),
    ];
    roof(b, &square, outer, top);
    b.decal(v3(0.0, 0.0, top), Vec2::new(w * 2.0, i * 2.0));
    for sx in [-1.0, 1.0] {
        b.decal(v3(sx * CELL, 0.0, top), Vec2::new(CELL_HALF * 2.0, w * 2.0));
    }
}

/// The owner's colour on a flat top: a strip `half` either way along `along` at `at`, a
/// third of that across, let in like the red lines.
fn team_tab(b: &mut MeshBuilder, at: Vec3, along: Vec3, half: f32) {
    let along = along.normalize() * half;
    let across = Vec3::Z.cross(along) * 0.3;
    b.paint(TEAM);
    b.face(&[
        at - along - across,
        at + along - across,
        at + along + across,
        at - along + across,
    ]);
}

/// How a launcher works its load cycle: the lid's travel and the hoist's drop and split
/// (`gpu_consts::launcher`).
#[derive(Clone, Copy)]
struct Load {
    /// The lid slides back along -x (the silo) or -y (the array).
    along_x: bool,
    drop: f32,
    split: f32,
}

const SILO_LOAD: Load = Load {
    along_x: true,
    drop: launcher::SILO_HOIST_DROP,
    split: launcher::SILO_HOIST_SPLIT,
};
const ARRAY_LOAD: Load = Load {
    along_x: false,
    drop: launcher::ARRAY_HOIST_DROP,
    split: launcher::ARRAY_HOIST_SPLIT,
};

/// How a store's gravity lift is drawn ([`gravity_lift`]): its emitter posts round the
/// well (how many, and the bearing of the first, degrees), its rings, and whether the
/// round it is bringing up shows in them.
#[derive(Clone, Copy)]
struct Lift {
    posts: usize,
    first: f32,
    rings: usize,
    round: bool,
}

/// A store's loading well in a roof, `c` its middle on the roof, an octagon of radius
/// `r` (corners on the octagon's usual bearings, a flat to +x), and the gravity lift that
/// brings each round up out of it while it assembles (`gpu_consts::launcher`):
/// - the well: a bronze-lipped octagonal coaming, a seam-dark opening, and its armoured
///   lid (`part::LAUNCHER_LID`), lapped plates sliding back along -x (the silo) or -y (the
///   array) to open it;
/// - the lift (`part::LAUNCHER_HOIST`): a containment collar floating over the well, plated
///   and bronze-rimmed with a red seam, `lift.rings` of them stacked, the next round's
///   core held in them; it sinks into the open well and rises out again. Nothing hangs it:
///   it floats, held by low emitter posts round the coaming, each a plated stub angled in
///   at it with a lens in its face, all below the collar.
fn gravity_lift(b: &mut MeshBuilder, c: Vec3, r: f32, load: Load, lift: Lift) {
    let fine = b.fine();
    let rim = r * 0.18 + 0.15;
    // The coaming.
    seam(b);
    b.yawed(c, TAU / 16.0, |b| {
        hoop(b, Vec3::Z * 0.12, r + rim * 0.5, rim, 0.24, 8)
    });
    if fine {
        metal(b);
        b.yawed(c, TAU / 16.0, |b| {
            hoop(b, Vec3::Z * 0.27, r + rim * 0.15, rim * 0.3, 0.06, 8)
        });
    }
    dark_plate(b);
    b.face(&octagon(c.truncate(), r, c.z + 0.02));
    let under = c.z + 0.26;
    // The lid: an octagonal plate, a second lapped over it toward the way it slides back.
    b.with_part(part::LAUNCHER_LID, |b| {
        let plan: Vec<[f32; 2]> = ngon(8, r + rim * 0.6).to_vec();
        let f = Frame::new(c.truncate().extend(under), Vec3::X, Vec3::Z);
        let thick = (r * 0.1).clamp(0.18, 0.4);
        dark_plate(b);
        armour(b, &f, &plan, thick);
        if fine {
            let (u, n) = if load.along_x {
                (Vec3::X, Vec3::Z)
            } else {
                (Vec3::Y, Vec3::Z)
            };
            armour(
                b,
                &Frame::new(c.truncate().extend(under + thick) - u * (r * 0.7), u, n),
                &swept(r * 1.5, r * 0.55, 0.0, 0.6),
                thick * 0.6,
            );
        }
    });
    // The collar's foot, let down by the load's drop, stops on the opening.
    let foot = under + load.drop + 0.12;
    let band = (r * 0.28).clamp(0.3, 0.9);
    let h = (r * 0.16).clamp(0.22, 0.6);
    let ring_r = r + rim * 0.5;
    let top = foot + h * lift.rings as f32 * 1.6;
    debug_assert!(top < load.split - 0.35);
    b.with_part(part::LAUNCHER_HOIST, |b| {
        let segs = if fine { 12 } else { 8 };
        for k in 0..lift.rings {
            let z = foot + h * 0.5 + k as f32 * h * 1.6;
            let rr = ring_r - k as f32 * band * 0.35;
            dark_plate(b);
            b.yawed(c.truncate().extend(0.0), TAU / 16.0, |b| {
                hoop(b, Vec3::Z * z, rr, band, h, segs.min(8))
            });
            metal(b);
            hoop(
                b,
                c.truncate().extend(0.0) + Vec3::Z * (z + h * 0.5 + 0.04),
                rr + band * 0.3,
                band * 0.35,
                0.08,
                segs,
            );
            if !b.coarse() {
                b.paint(GLOW_LASER);
                hoop(
                    b,
                    c.truncate().extend(0.0) + Vec3::Z * z,
                    rr - band * 0.5 - 0.02,
                    0.05,
                    h * 0.3,
                    segs,
                );
            }
        }
        if lift.round && !b.coarse() {
            // The next round's core, held in the collar: a stub of the warhead's plate.
            let rc = r * 0.42;
            dark_plate(b);
            b.with_facets(|b| {
                b.loft_z(
                    &ngon(8, 1.0),
                    &[
                        Section::new(foot, rc * 0.7).shifted(c.x, c.y),
                        Section::new(foot + h * 0.4, rc).shifted(c.x, c.y),
                        Section::new(top - h * 0.4, rc).shifted(c.x, c.y),
                        Section::new(top + h * 0.4, rc * 0.6).shifted(c.x, c.y),
                    ],
                )
            });
            metal(b);
            b.with_facets(|b| {
                b.loft_z(
                    &ngon(8, 1.0),
                    &[
                        Section::new(foot + h * 0.6, rc * 1.03).shifted(c.x, c.y),
                        Section::new(foot + h * 1.2, rc * 1.03).shifted(c.x, c.y),
                    ],
                )
            });
        }
    });
    // The emitter posts: low plated stubs on the coaming, angled in at the collar, each
    // with a lens aimed at it, all under it.
    let aim = c + Vec3::Z * (foot + h * 0.5 - c.z);
    for k in 0..lift.posts {
        let a = (lift.first + 360.0 * k as f32 / lift.posts as f32).to_radians();
        let d = out(a);
        let root = c + d * (r + rim + r * 0.35 + 0.4);
        let head = root - d * (r * 0.2 + 0.2) + Vec3::Z * (foot - c.z - h * 0.4);
        let w = (r * 0.3).clamp(0.4, 1.1);
        dark_plate(b);
        b.yawed(root, a, |b| {
            b.frustum(
                Vec3::ZERO,
                Vec2::new(w * 2.4, w * 2.0),
                Vec2::new(w * 1.6, w * 1.4),
                w * 0.8,
                Vec2::new(-w * 0.2, 0.0),
            )
        });
        b.beam(
            root + Vec3::Z * (w * 0.6) + d * (w * 0.3),
            head,
            Vec2::new(w * 1.5, w * 1.3),
            Vec2::new(w * 1.2, w * 1.0),
        );
        if !b.coarse() {
            let aim_d = (aim - head).normalize();
            lens(b, head + aim_d * (w * 0.4), aim, w * 0.55);
            if fine {
                // Plates lapped down its back into a spike.
                let up = (head - root).normalize();
                let back = (d - up * d.dot(up)).normalize();
                dark_plate(b);
                armour(
                    b,
                    &Frame::new(head + back * (w * 0.6), -up, back),
                    &swept(head.distance(root) + w, w * 0.65, 0.0, 0.55),
                    w * 0.2,
                );
            }
        }
    }
}

/// A blast-door leaf, the +y one (`part::SILO_DOOR` and mirrored by the caller): `outline`
/// its plan (x, y >= 0, counter-clockwise, its meeting edge on y = 0), a plate from `z0`
/// up `thick`, and `layers` more plates lapped over it, each drawn in by `step` and a
/// little higher; a red line let into the top plate along the meeting edge.
fn leaf(b: &mut MeshBuilder, outline: &[Vec2], z0: f32, thick: f32, layers: usize, step: f32) {
    let (lo, hi) = outline.iter().fold(
        (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)),
        |(lo, hi), &p| (lo.min(p), hi.max(p)),
    );
    let mid = Vec2::new((lo.x + hi.x) * 0.5, 0.0);
    let f = Frame::new(Vec3::Z * z0, Vec3::X, Vec3::Z);
    let mut z = 0.0;
    let mut top = 0.0;
    for k in 0..=layers {
        let shrink = |p: Vec2| -> [f32; 2] {
            let d = p - mid;
            let s = Vec2::new(
                (1.0 - step * k as f32 / (hi.x - mid.x).max(0.1)).max(0.2),
                (1.0 - step * k as f32 / hi.y.max(0.1)).max(0.2),
            );
            let q = mid + d * s;
            [q.x, q.y.max(0.02)]
        };
        let plan: Vec<[f32; 2]> = outline.iter().map(|&p| shrink(p)).collect();
        let t = if k == 0 { thick } else { thick * 0.45 };
        dark_plate(b);
        if b.coarse() {
            b.face(
                &plan
                    .iter()
                    .map(|p| f.at(p[0], p[1], z + t))
                    .collect::<Vec<_>>(),
            );
            return;
        }
        armour(
            b,
            &Frame {
                o: f.at(0.0, 0.0, z),
                ..f
            },
            &plan,
            t,
        );
        top = z + t;
        z += t * 0.92;
        if !b.fine() {
            break;
        }
    }
    if b.fine() {
        let w = (hi.x - lo.x) * 0.5 - step * layers as f32;
        red_slot(
            b,
            v3(mid.x, 0.35, z0 + top),
            Vec3::Z,
            Vec3::X,
            w.max(0.5),
            0.12,
        );
    }
}

/// A pair of bronze rails along y at x = ±`x` from -`y` to `y` on a deck at `z`, plated
/// buffers at their ends where the open leaves stop.
fn rails(b: &mut MeshBuilder, x: f32, y: f32, z: f32) {
    for sx in [-1.0f32, 1.0] {
        metal(b);
        b.block(v3(sx * x - 0.3, -y, z), v3(sx * x + 0.3, y, z + 0.32));
        dark_plate(b);
        for sy in [-1.0f32, 1.0] {
            let (p, q) = (sy * (y - 0.9), sy * (y + 0.2));
            b.block(
                v3(sx * x - 0.6, p.min(q), z),
                v3(sx * x + 0.6, p.max(q), z + 0.9),
            );
        }
    }
}

/// A leaf's carriages over its rails at x = ±`x` (inside the leaf's part), from `y0` to
/// `y1` along it, hung from its plate at `z1` down to the rail's top at `rail`.
fn carriages(b: &mut MeshBuilder, x: f32, y0: f32, y1: f32, rail: f32, z1: f32) {
    if b.coarse() {
        return;
    }
    metal(b);
    for sx in [-1.0f32, 1.0] {
        b.block(
            v3(sx * x - 0.45, y0, rail + 0.34),
            v3(sx * x + 0.45, y1, z1),
        );
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    use super::{ARRAY_SIZE, CELL, CELL_HALF, SILO_SIZE};
    use crate::gpu_consts::launcher as l;
    use crate::{build_model_scaled, part, Model};

    /// How far each leaf slides along y fully open (`entity.wgsl`): the silo's, the array's.
    const SILO_TRAVEL: f32 = 5.2;
    const ARRAY_TRAVEL: f32 = 5.0;

    const SILOS: [&str; 1] = ["regency_nuke_silo"];
    const ARRAYS: [&str; 1] = ["regency_nuke_defense"];

    fn built(key: &str) -> Model {
        let (r, h) = if key.starts_with("regency_nuke_silo") {
            SILO_SIZE
        } else {
            ARRAY_SIZE
        };
        build_model_scaled(key, r, h, 4).unwrap()
    }

    fn every() -> impl Iterator<Item = (&'static str, bool)> {
        SILOS
            .iter()
            .map(|&k| (k, true))
            .chain(ARRAYS.iter().map(|&k| (k, false)))
    }

    /// The Regency's checks for each design: its lot, its height, team colour and dark
    /// plate at every level, its budget, and nothing in the octagon's cut corners.
    #[test]
    fn launchers_fit_their_lots() {
        for (key, silo) in every() {
            let ((r, h), cells) = if silo {
                (SILO_SIZE, 8)
            } else {
                (ARRAY_SIZE, 4)
            };
            super::super::check_at(key, 4, r, h, Some(cells), &[]);
            let half = mc_map::BUILD_CELL_M as f32 * 0.5 * cells as f32;
            let model = built(key);
            let tris = model.lods.each_ref().map(|l| l.indices.len() / 3);
            println!("{key}: {tris:?} triangles");
            for (l, lod) in model.lods.iter().enumerate() {
                let corner = lod
                    .vertices
                    .iter()
                    .map(|v| v.pos[0].abs() + v.pos[1].abs())
                    .fold(0.0, f32::max);
                assert!(
                    corner <= half * 1.45,
                    "{key} lod{l}: {corner} into the lot's cut corners"
                );
            }
        }
    }

    /// Doors at every level and the rounds at the two nearer ones; the rounds stand under
    /// the shut doors, the silo's at the middle (where the warhead leaves from) and the
    /// array's one to a quadrant over each cell.
    #[test]
    fn launchers_tag_doors_and_rounds() {
        for (key, silo) in every() {
            let model = built(key);
            for (l, lod) in model.lods.iter().enumerate() {
                assert!(
                    lod.vertices.iter().any(|v| v.part == part::SILO_DOOR),
                    "{key} lod{l}: doors"
                );
                if l < 2 {
                    assert!(
                        lod.vertices.iter().any(|v| v.part == part::SILO_ROUND),
                        "{key} lod{l}: rounds"
                    );
                }
            }
            let mesh = &model.lods[0];
            let of = |p: u32| mesh.vertices.iter().filter(move |v| v.part == p);
            let top = of(part::SILO_ROUND)
                .map(|v| v.pos[2])
                .fold(f32::MIN, f32::max);
            let door = of(part::SILO_DOOR)
                .map(|v| v.pos[2])
                .fold(f32::MAX, f32::min);
            assert!(
                top < door - 0.5,
                "{key}: rounds {top} under the doors {door}"
            );
            if silo {
                let off = of(part::SILO_ROUND)
                    .map(|v| v.pos[0].hypot(v.pos[1]))
                    .fold(0.0, f32::max);
                assert!(off < super::BORE, "{key}: warhead {off} off the middle");
            } else {
                for v in of(part::SILO_ROUND) {
                    let (x, y) = (v.pos[0].abs(), v.pos[1].abs());
                    assert!(
                        (x - CELL).abs() < CELL_HALF && (y - CELL).abs() < CELL_HALF,
                        "{key}: round at {:?} outside its cell",
                        v.pos
                    );
                }
            }
        }
    }

    /// Slid open by their travel, no leaf (nor its carriages) is left over the opening:
    /// the bore, or any of the four cells.
    #[test]
    fn open_leaves_clear_the_openings() {
        for (key, silo) in every() {
            let travel = if silo { SILO_TRAVEL } else { ARRAY_TRAVEL };
            // The openings in plan, shrunk a hand's breadth.
            let open: Vec<(glam::Vec2, glam::Vec2)> = if silo {
                let h = super::BORE * (std::f32::consts::TAU / 16.0).cos() - 0.1;
                vec![(glam::Vec2::splat(-h), glam::Vec2::splat(h))]
            } else {
                [(-1.0, -1.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)]
                    .map(|(sx, sy)| {
                        let m = glam::Vec2::new(sx * CELL, sy * CELL);
                        let h = glam::Vec2::splat(CELL_HALF - 0.1);
                        (m - h, m + h)
                    })
                    .to_vec()
            };
            for (l, mesh) in built(key).lods.iter().enumerate() {
                for t in mesh.indices.chunks(3) {
                    let v = [t[0], t[1], t[2]].map(|i| mesh.vertices[i as usize]);
                    if v[0].part != part::SILO_DOOR {
                        continue;
                    }
                    let p =
                        v.map(|v| glam::Vec2::new(v.pos[0], v.pos[1] + travel * v.pos[1].signum()));
                    let (lo, hi) = (p[0].min(p[1]).min(p[2]), p[0].max(p[1]).max(p[2]));
                    for &(a, c) in &open {
                        assert!(
                            hi.x <= a.x || lo.x >= c.x || hi.y <= a.y || lo.y >= c.y,
                            "{key} lod{l}: an open leaf over the opening at {lo}..{hi}"
                        );
                    }
                }
                // Shut, they reach past every opening's edges.
                let (lo, hi) = mesh
                    .vertices
                    .iter()
                    .filter(|v| v.part == part::SILO_DOOR)
                    .map(|v| glam::Vec2::new(v.pos[0], v.pos[1]))
                    .fold(
                        (glam::Vec2::splat(f32::MAX), glam::Vec2::splat(f32::MIN)),
                        |(lo, hi), p| (lo.min(p), hi.max(p)),
                    );
                for &(a, c) in &open {
                    assert!(
                        lo.cmple(a).all() && hi.cmpge(c).all(),
                        "{key} lod{l}: the shut leaves {lo}..{hi} short of {a}..{c}"
                    );
                }
            }
        }
    }

    /// The load cycle's plant up close: a lid and a lift on each, the lift wholly under the
    /// split (it floats, nothing holds it from above), and let down it stops on the
    /// opening, not through it.
    #[test]
    fn load_cycle_plant_is_rigged_where_the_shader_moves_it() {
        for (key, silo) in every() {
            let (split, drop) = if silo {
                (l::SILO_HOIST_SPLIT, l::SILO_HOIST_DROP)
            } else {
                (l::ARRAY_HOIST_SPLIT, l::ARRAY_HOIST_DROP)
            };
            let model = built(key);
            let z = |p: u32| -> Vec<f32> {
                model.lods[0]
                    .vertices
                    .iter()
                    .filter(|v| v.part == p)
                    .map(|v| v.pos[2])
                    .collect()
            };
            let (lid, hoist) = (z(part::LAUNCHER_LID), z(part::LAUNCHER_HOIST));
            assert!(!lid.is_empty() && !hoist.is_empty(), "{key}: rigged");
            // Nothing hangs the lift: all of it is under the split, so it sinks and
            // rises as one.
            assert!(
                hoist.iter().all(|&h| h < split - 0.3),
                "{key}: the lift reaches the split"
            );
            let low = hoist.iter().copied().fold(f32::MAX, f32::min);
            let floor = lid.iter().copied().fold(f32::MAX, f32::min);
            assert!(
                low - drop >= floor && low - drop < floor + 0.3,
                "{key}: block let down to {} over the opening at {floor}",
                low - drop
            );
        }
    }

    /// The lid slides back over its own store's roof, clear of everything fixed above it.
    #[test]
    fn lids_slide_clear() {
        for (key, silo) in every() {
            let travel = if silo {
                Vec3::new(-l::SILO_LID_TRAVEL, 0.0, 0.0)
            } else {
                Vec3::new(0.0, -l::ARRAY_LID_TRAVEL, 0.0)
            };
            let model = built(key);
            let mesh = &model.lods[0];
            let lid: Vec<Vec3> = mesh
                .vertices
                .iter()
                .filter(|v| v.part == part::LAUNCHER_LID)
                .map(|v| Vec3::from(v.pos) + travel)
                .collect();
            let (lo, hi) = lid.iter().fold(
                (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
                |(lo, hi), &p| (lo.min(p), hi.max(p)),
            );
            // Anything fixed inside the slid lid's box, but for what it lies on.
            let inside = mesh.vertices.iter().find(|v| {
                let p = Vec3::from(v.pos);
                v.part == 0
                    && p.x > lo.x + 0.05
                    && p.x < hi.x - 0.05
                    && p.y > lo.y + 0.05
                    && p.y < hi.y - 0.05
                    && p.z > lo.z + 0.05
                    && p.z < hi.z - 0.05
            });
            assert!(
                inside.is_none(),
                "{key}: the open lid runs into {:?}",
                inside.map(|v| v.pos)
            );
        }
    }
}
