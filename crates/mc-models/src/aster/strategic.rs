//! The Aster strategic launchers, authored at blueprint scale (metres). Neither is a
//! gun: each keeps its rounds under a pair of blast doors that slide apart along y
//! (`part::SILO_DOOR`) to launch, and each round it holds is drawn only while it has
//! it in stock (`part::SILO_ROUND`).
//! - Tech 4, the Sunfall (8x8 lot, 26 m): a hardened launch bunker round one deep
//!   tube. Two heavy armoured leaves close the tube's square mouth, riding steel rails
//!   on the roof out to buffers and drive houses. Down the tube stands the warhead,
//!   its white re-entry nose just under the doors. The armoured warhead store lies
//!   along -y under a portal crane that lowers each finished round through its hatch;
//!   a heavy transfer duct runs from it into the bunker. The flame ducts run out along
//!   ±x in open trenches to vents; coolant tanks and their pump house stand on +y. The
//!   tube is dug in: the model's pit (`Model::pit`) shows it below ground.
//! - Tech 3, the Parhelion (4x4 lot, 20 m): a vertical-launch block of four cells in a
//!   2x2 under one pair of leaves, a slim interceptor standing in each; the tracking
//!   radar's back-to-back phased arrays turning on a mast on -x (`part::SPINNER`); the
//!   magazine on +x with its loader duct into the block; coolant, and a capacitor
//!   bank feeding the block.
//!
//! What moves: the leaves (`part::SILO_DOOR`, by `SILO_TRAVEL` / `ARRAY_TRAVEL`), the
//! rounds come and go (`part::SILO_ROUND`) and the radar turns. While a round is
//! assembling, each works a load cycle over its store (`gpu_consts::launcher`): the
//! hatch lid slides back (`part::LAUNCHER_LID`), the hoist block goes down into the hatch
//! and up again (`part::LAUNCHER_HOIST`: the silo's crane, a small gantry on the array's
//! magazine) and the lid shuts. Both are heavy plant at the scale of the rounds they
//! handle: nothing lit, no doors or windows for people.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, SQRT_2, TAU};

use glam::{Vec2, Vec3};

use super::parts::*;
use crate::builder::{chamfered_rect, ngon, MeshBuilder, Section};
use crate::material::*;
use crate::{part, pattern, Pit};

/// How far each silo leaf slides along y when fully open (`entity.wgsl` has the same).
#[cfg(test)]
pub(super) const SILO_TRAVEL: f32 = 5.2;
/// How far each interceptor-array leaf slides along y when fully open.
#[cfg(test)]
pub(super) const ARRAY_TRAVEL: f32 = 5.0;

// ---- the Sunfall silo -------------------------------------------------------------

/// The lot's slab top, and the bunker's roof.
const S_DECK: f32 = 1.2;
const S_TOP: f32 = 5.0;
/// Half the square mouth of the tube, and the hazard band round it.
const S_HOLE: f32 = 5.0;
const S_BAND: f32 = 6.2;
/// The bunker's half width at its foot and at its roof.
const S_FOOT: f32 = 16.5;
const S_ROOF: f32 = 14.5;
/// The round tube under the square mouth: its radius, where it meets the mouth's
/// square well (a ledge), and its floor.
const S_BORE: f32 = 4.6;
const S_LEDGE: f32 = 0.4;
const S_FLOOR: f32 = -17.0;
/// One leaf: half its width along x, its far edge along y, and its underside and top.
const S_LEAF_X: f32 = 5.8;
const S_LEAF_Y: f32 = 5.6;
const S_LEAF_Z0: f32 = S_TOP + 0.06;
const S_LEAF_Z1: f32 = S_TOP + 1.5;
/// The warhead: its radius, and its nose's tip, 1.5 m under the doors.
const S_ROUND_R: f32 = 2.5;
const S_NOSE: f32 = S_LEAF_Z0 - 1.5;
/// The rails the leaves ride, either side of the mouth.
const S_RAIL: (f32, f32) = (6.15, 6.75);
const S_RAIL_END: f32 = 12.2;
/// The warhead store along -y: its centre line, half length along x, and height.
const S_STORE_Y: f32 = -30.0;
const S_STORE_X: f32 = 17.0;
const S_STORE_H: f32 = 6.5;
/// The portal crane over the store: its legs at ±`S_CRANE_X`, its girder's top.
const S_CRANE_X: f32 = 20.5;
const S_CRANE_TOP: f32 = 23.5;

pub(super) fn nuke_silo(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        silo_coarse(b);
        return;
    }
    // The tube below the lot is seen through the mouth; the pit reaches just past the
    // square well's corners.
    b.set_pit(Pit {
        open: S_DECK,
        radius: S_HOLE * SQRT_2 + 0.4,
        stroke: 0.0,
        section: 0.0,
        rack: [0.0, 0.0],
    });
    slab(b, 42.0, 6.0, 0.5, S_DECK);
    bunker(b);
    tube(b);
    b.with_part(part::SILO_ROUND, warhead);
    b.with_part(part::SILO_DOOR, |b| b.mirror_y(silo_leaf));
    door_track(b);
    for yaw in [0.0, PI] {
        b.yawed(Vec3::ZERO, yaw, flame_trench);
    }
    warhead_store(b);
    crane(b);
    coolant(b);
}

/// Far off: the slab, the bunker round a dark mouth, the leaves' tops, the store and
/// the crane.
fn silo_coarse(b: &mut MeshBuilder) {
    b.paint(PLATING);
    b.cuboid_open(v3(0.0, 0.0, S_DECK * 0.5), v3(84.0, 84.0, S_DECK));
    let square = |h: f32, z: f32| vec![v3(h, -h, z), v3(h, h, z), v3(-h, h, z), v3(-h, -h, z)];
    b.loft(
        &[square(S_FOOT, S_DECK), square(S_ROOF, S_TOP)],
        false,
        false,
    );
    b.paint(PLATING_DARK);
    b.radial(4, |b| {
        b.face(&[
            v3(S_HOLE, -S_HOLE, S_TOP),
            v3(S_ROOF, -S_ROOF, S_TOP),
            v3(S_ROOF, S_ROOF, S_TOP),
            v3(S_HOLE, S_HOLE, S_TOP),
        ])
    });
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.decal(v3(0.0, 0.0, S_TOP - 1.5), v2(S_HOLE * 2.0, S_HOLE * 2.0));
    b.with_part(part::SILO_DOOR, |b| b.mirror_y(silo_leaf));
    b.paint(PLATING);
    b.cuboid_open(
        v3(0.0, S_STORE_Y, S_DECK + S_STORE_H * 0.5),
        v3(S_STORE_X * 2.0, 14.0, S_STORE_H),
    );
    // The crane: its girder and legs, which carry the silhouette's height.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.cuboid_open(
        v3(0.0, S_STORE_Y, S_CRANE_TOP - 1.5),
        v3(S_CRANE_X * 2.0 + 3.0, 3.0, 3.0),
    );
    for sx in [-1.0, 1.0] {
        b.cuboid_open(
            v3(sx * S_CRANE_X, S_STORE_Y, (S_DECK + S_CRANE_TOP) * 0.5),
            v3(2.4, 3.0, S_CRANE_TOP - S_DECK),
        );
    }
    team_panel(
        b,
        v3(0.0, S_STORE_Y, S_CRANE_TOP),
        v2(S_CRANE_X * 2.0 - 2.0, 2.2),
    );
}

/// The hardened bunker: sloped light armour on a dark plinth, a dark deck roof round
/// the tube's mouth with a hazard band at the mouth's edge.
fn bunker(b: &mut MeshBuilder) {
    let ring = |plan: &[[f32; 2]], z: f32| -> Vec<Vec3> {
        plan.iter().map(|p| v3(p[0], p[1], z)).collect()
    };
    let (foot, roof) = (
        chamfered_rect(v2(S_FOOT, S_FOOT), 5.0),
        chamfered_rect(v2(S_ROOF, S_ROOF), 4.0),
    );
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.chamfered_box(v3(0.0, 0.0, S_DECK + 0.35), v3(34.8, 34.8, 0.7), 5.2);
    b.paint(PLATING);
    b.loft(&[ring(&foot, S_DECK), ring(&roof, S_TOP)], false, false);

    // The roof, a sector per side from the hazard band out to the chamfered edge.
    let (h, m) = (S_BAND, S_ROOF - 2.0);
    b.paint(PLATING_DARK);
    b.radial(4, |b| {
        b.face(&[
            v3(h, -h, S_TOP),
            v3(m, -m, S_TOP),
            v3(S_ROOF, -S_ROOF + 4.0, S_TOP),
            v3(S_ROOF, S_ROOF - 4.0, S_TOP),
            v3(m, m, S_TOP),
            v3(h, h, S_TOP),
        ])
    });
    b.paint(ACCENT).pattern(pattern::HAZARD);
    b.radial(4, |b| {
        b.face(&[
            v3(S_HOLE, -S_HOLE, S_TOP),
            v3(h, -h, S_TOP),
            v3(h, h, S_TOP),
            v3(S_HOLE, S_HOLE, S_TOP),
        ])
    });
    // The owner's colour on the roof either side of the rails.
    for x in [-10.6, 10.6] {
        team_panel(b, v3(x, 0.0, S_TOP), v2(4.2, 7.0));
    }
}

/// Sides for a round shape with `n` at full detail: half at the middle distance.
fn round(b: &MeshBuilder, n: usize) -> usize {
    if b.fine() {
        n
    } else {
        (n / 2).max(6)
    }
}

/// A wall of a well, facing in: from `a` to `c` counter-clockwise round the inside
/// seen from above, `z0` to `z1` up.
fn wall_in(b: &mut MeshBuilder, a: Vec2, c: Vec2, z0: f32, z1: f32) {
    b.face(&[a.extend(z0), a.extend(z1), c.extend(z1), c.extend(z0)]);
}

/// The tube: the mouth's square well down to a ledge, the round bore below it lined in
/// bands, ring frames, the steel guide rails, and the floor.
fn tube(b: &mut MeshBuilder) {
    let s = S_HOLE;
    let corner = [v2(s, s), v2(-s, s), v2(-s, -s), v2(s, -s)];
    b.paint(PLATING_DARK);
    for k in 0..4 {
        wall_in(b, corner[k], corner[(k + 1) % 4], S_LEDGE, S_TOP);
    }
    // The bore: a vertex at every n-th of a turn from +x, so the ledge's sectors meet it.
    let n = if b.fine() { 16 } else { 8 };
    let at = |i: usize, r: f32| Vec2::from_angle(i as f32 * TAU / n as f32) * r;
    // Down here depth is squeezed toward the mouth (`Model::pit`): the walls go in
    // bands, so no one face spans much of it.
    let bands: &[(f32, f32, u32)] = if b.fine() {
        &[
            (S_FLOOR, -12.0, PLATING_DARK),
            (-12.0, -6.5, ACCENT),
            (-6.5, -2.0, PLATING_DARK),
            (-2.0, S_LEDGE, ACCENT),
        ]
    } else {
        &[(S_FLOOR, -6.5, PLATING_DARK), (-6.5, S_LEDGE, ACCENT)]
    };
    for &(z0, z1, m) in bands {
        b.paint(m);
        for i in 0..n {
            wall_in(b, at(i, S_BORE), at(i + 1, S_BORE), z0, z1);
        }
    }
    // The ledge between the square well and the round bore.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.radial(4, |b| {
        let mut pts = vec![v3(s, -s, S_LEDGE), v3(s, s, S_LEDGE)];
        let q = n / 4;
        for j in 0..=q {
            let a = FRAC_PI_4 - j as f32 * FRAC_PI_2 / q as f32;
            pts.push((Vec2::from_angle(a) * S_BORE).extend(S_LEDGE));
        }
        b.face(&pts);
    });
    // The floor, a fan of pieces.
    for i in 0..n {
        b.face(&[
            v3(0.0, 0.0, S_FLOOR),
            at(i, S_BORE).extend(S_FLOOR),
            at(i + 1, S_BORE).extend(S_FLOOR),
        ]);
    }
    // Ring frames, clear of the wall.
    let apothem = S_BORE * (PI / n as f32).cos();
    if b.fine() {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        for z in [-12.4, -6.9] {
            polygon_ring(b, n, S_BORE - 0.55, S_BORE - 0.04, z, z + 0.4);
        }
    }
    // The guide rails, one on every other facet.
    b.paint(METAL);
    for k in 0..8 {
        let a = (2 * k * n / 16) as f32 * TAU / n as f32 + PI / n as f32;
        b.yawed(Vec3::ZERO, a, |b| {
            b.block(
                v3(apothem - 0.48, -0.22, S_FLOOR + 0.4),
                v3(apothem - 0.14, 0.22, S_LEDGE - 0.1),
            );
        });
    }
}

/// A flat ring of `n` facets round the z axis, vertices on every n-th of a turn from +x.
fn polygon_ring(b: &mut MeshBuilder, n: usize, r0: f32, r1: f32, z0: f32, z1: f32) {
    let section = [(r0, z0), (r1, z0), (r1, z1), (r0, z1)];
    let rings: Vec<Vec<Vec3>> = (0..=n)
        .map(|i| {
            let d = Vec2::from_angle(i as f32 * TAU / n as f32);
            section.iter().map(|&(r, z)| (d * r).extend(z)).collect()
        })
        .collect();
    b.loft(&rings, false, false);
}

/// The warhead in its tube (`part::SILO_ROUND`): the white re-entry nose, a dark
/// joint ring, the owner's band, the body down to the floor, and the launch shoes
/// that ride the guide rails.
fn warhead(b: &mut MeshBuilder) {
    let sides = round(b, 12);
    let plan = ngon(sides, 1.0);
    // The same shape as the one in flight (nuke.wgsl `warhead_ring`), as big.
    let shoulder = S_NOSE - 7.2;
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[
            Section::new(S_NOSE, 0.07),
            Section::new(S_NOSE - 0.6, 0.8),
            Section::new(S_NOSE - 1.8, 1.5),
            Section::new(S_NOSE - 3.4, 2.08),
            Section::new(S_NOSE - 5.5, 2.42),
            Section::new(shoulder, S_ROUND_R),
        ],
    );
    // The body in courses of one radius, none laid over another: down the pit depth is
    // squeezed, and a band standing a few centimetres proud would fight the body under it.
    let courses = [
        (shoulder - 0.6, shoulder, ACCENT),
        (shoulder - 1.6, shoulder - 0.6, PLATING),
        (shoulder - 2.3, shoulder - 1.6, TEAM),
        (S_FLOOR + 0.3, shoulder - 2.3, PLATING),
    ];
    for (z0, z1, m) in courses {
        b.paint(m);
        b.prism(v3(0.0, 0.0, z0), sides, S_ROUND_R, S_ROUND_R, z1 - z0);
    }
    if b.fine() {
        // Launch shoes out to four of the rails.
        let n = 16;
        let apothem = S_BORE * (PI / n as f32).cos();
        b.paint(METAL);
        for k in 0..4 {
            let a = (4 * k) as f32 * TAU / n as f32 + PI / n as f32;
            b.yawed(Vec3::ZERO, a, |b| {
                for z in [shoulder - 4.0, shoulder - 10.0] {
                    b.block(
                        v3(S_ROUND_R - 0.1, -0.25, z),
                        v3(apothem - 0.56, 0.25, z + 0.5),
                    );
                }
            });
        }
    }
}

/// One blast-door leaf (the +y one; `part::SILO_DOOR` and mirrored by the caller):
/// a thick armoured slab, a hazard-striped nosing a step lower on the meeting edge,
/// dark ribs across it, the owner's colour, and carriages over the rails.
fn silo_leaf(b: &mut MeshBuilder) {
    let (x, y0, y1, z0, z1) = (S_LEAF_X, 0.03, S_LEAF_Y, S_LEAF_Z0, S_LEAF_Z1);
    if b.coarse() {
        b.paint(PLATING);
        b.decal(v3(0.0, (y0 + y1) * 0.5, z1), v2(x * 2.0, y1 - y0));
        return;
    }
    let edge = 0.9;
    b.paint(ACCENT).pattern(pattern::HAZARD);
    b.block(v3(-x, y0, z0), v3(x, edge, z1 - 0.3));
    b.paint(PLATING);
    b.plate(
        v3(0.0, (edge + y1) * 0.5, z0),
        v2(x * 2.0, y1 - edge),
        z1 - z0,
        0.35,
    );
    if b.fine() {
        b.paint(PLATING_DARK);
        for rx in [-3.7, 0.0, 3.7] {
            b.block(
                v3(rx - 0.3, edge + 0.55, z1 - 0.05),
                v3(rx + 0.3, y1 - 0.55, z1 + 0.28),
            );
        }
    }
    team_panel(b, v3(-1.85, (edge + y1) * 0.5, z1), v2(2.3, 2.8));
    b.paint(ACCENT).pattern(pattern::PLAIN);
    for sx in [-1.0f32, 1.0] {
        let (a, c) = (sx * (x - 0.05), sx * (S_RAIL.1 + 0.2));
        b.block(v3(a.min(c), 1.6, S_TOP + 0.6), v3(a.max(c), 4.8, z1 - 0.25));
    }
}

/// The fixed track the leaves run on: a steel rail either side of the mouth, buffers
/// where the open leaves stop, and a drive house at each end.
fn door_track(b: &mut MeshBuilder) {
    for sx in [-1.0f32, 1.0] {
        let (a, c) = (sx * S_RAIL.0, sx * S_RAIL.1);
        b.paint(METAL);
        b.block(
            v3(a.min(c), -S_RAIL_END, S_TOP - 0.05),
            v3(a.max(c), S_RAIL_END, S_TOP + 0.45),
        );
        b.paint(ACCENT).pattern(pattern::PLAIN);
        for sy in [-1.0f32, 1.0] {
            let (p, q) = (sy * 11.2, sy * 12.4);
            let (u, w) = (sx * (S_RAIL.0 - 0.25), sx * (S_RAIL.1 + 0.25));
            b.block(
                v3(u.min(w), p.min(q), S_TOP - 0.05),
                v3(u.max(w), p.max(q), S_TOP + 0.95),
            );
        }
    }
    b.mirror_y(|b| {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.chamfered_box(v3(0.0, 13.4, S_TOP + 0.55), v3(9.0, 1.6, 1.2), 0.4);
        if b.fine() {
            b.paint(METAL);
            for x in [-2.6, 2.6] {
                b.cylinder_between(
                    v3(x, 12.55, S_TOP + 0.6),
                    v3(x, 11.4, S_TOP + 0.6),
                    0.28,
                    0.28,
                    6,
                );
            }
        }
    });
}

/// A flame duct out along +x (the caller turns it for -x): an open trench from the
/// bunker, dark and heat-stained inside, gratings across it, to a louvred vent.
fn flame_trench(b: &mut MeshBuilder) {
    let (x0, x1, half) = (15.5, 27.0, 2.4);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.mirror_y(|b| {
        b.block(
            v3(x0, half, S_DECK - 0.05),
            v3(x1, half + 0.8, S_DECK + 1.4),
        )
    });
    b.paint(PLATING_DARK).pattern(pattern::NONE);
    b.decal(
        v3((x0 + x1) * 0.5, 0.0, S_DECK + 0.08),
        v2(x1 - x0, half * 2.0),
    );
    if b.fine() {
        b.paint(METAL);
        for x in [18.5, 21.5, 24.5] {
            b.block(
                v3(x - 0.2, -half - 0.2, S_DECK + 1.0),
                v3(x + 0.2, half + 0.2, S_DECK + 1.3),
            );
        }
    }
    // The vent: a dark housing, a deflector hood over it leaning out, louvres on top.
    let c = v3(31.0, 0.0, S_DECK);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.chamfered_box(c + v3(0.0, 0.0, 1.6), v3(8.0, 9.4, 3.2), 1.2);
    b.paint(PLATING_DARK);
    b.frustum(
        c + v3(0.0, 0.0, 3.2),
        v2(7.2, 8.4),
        v2(6.2, 7.6),
        1.2,
        v2(0.5, 0.0),
    );
    if b.fine() {
        louvres(b, c + v3(0.5, 0.0, 4.4), v2(5.2, 6.4), 6);
        b.paint(ACCENT).pattern(pattern::HAZARD);
        b.block(c + v3(-4.2, -4.9, 0.0), c + v3(-3.9, 4.9, 2.6));
    }
}

/// The warhead store along -y: a long armoured casemate with sloped sides like the
/// bunker's, the owner's colour on its roof, a hazard-edged hatch the crane lowers the
/// rounds through, and a heavy transfer duct from it into the bunker.
fn warhead_store(b: &mut MeshBuilder) {
    let ring = |plan: &[[f32; 2]], z: f32| -> Vec<Vec3> {
        plan.iter().map(|p| v3(p[0], p[1] + S_STORE_Y, z)).collect()
    };
    let top = S_DECK + S_STORE_H;
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.chamfered_box(
        v3(0.0, S_STORE_Y, S_DECK + 0.3),
        v3(S_STORE_X * 2.0 + 1.6, 15.6, 0.6),
        2.0,
    );
    b.paint(PLATING);
    b.loft(
        &[
            ring(&chamfered_rect(v2(S_STORE_X, 7.0), 2.5), S_DECK),
            ring(&chamfered_rect(v2(S_STORE_X - 2.0, 5.0), 1.8), top),
        ],
        false,
        false,
    );
    b.paint(PLATING_DARK);
    b.face(
        &chamfered_rect(v2(S_STORE_X - 2.0, 5.0), 1.8)
            .iter()
            .map(|p| v3(p[0], p[1] + S_STORE_Y, top))
            .collect::<Vec<_>>(),
    );
    team_panel(b, v3(-9.0, S_STORE_Y, top), v2(8.0, 6.0));
    // The hatch under the crane's hook: a hazard frame round a dark opening, and an
    // armoured lid (`part::LAUNCHER_LID`) that slides back along -x onto the roof while a
    // round is loaded.
    b.paint(ACCENT).pattern(pattern::HAZARD);
    b.block(
        v3(1.2, S_STORE_Y - 3.6, top - 0.02),
        v3(10.8, S_STORE_Y + 3.6, top + 0.2),
    );
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.decal(v3(6.0, S_STORE_Y, top + 0.21), v2(8.0, 6.0));
    b.with_part(part::LAUNCHER_LID, |b| {
        b.paint(PLATING_DARK);
        b.block(
            v3(1.8, S_STORE_Y - 3.0, top + 0.18),
            v3(10.2, S_STORE_Y + 3.0, top + 0.6),
        );
        if b.fine() {
            b.paint(ACCENT).pattern(pattern::PLAIN);
            for x in [4.0, 6.0, 8.0] {
                b.block(
                    v3(x - 0.2, S_STORE_Y - 2.8, top + 0.58),
                    v3(x + 0.2, S_STORE_Y + 2.8, top + 0.8),
                );
            }
        }
    });
    // The transfer duct: a heavy armoured box from the store into the bunker's flank.
    b.paint(PLATING);
    b.beam(
        v3(0.0, S_STORE_Y + 6.5, S_DECK + 2.2),
        v3(0.0, -14.6, S_DECK + 2.2),
        v2(6.0, 4.4),
        v2(6.0, 4.4),
    );
    if b.fine() {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.beam(
            v3(0.0, S_STORE_Y + 6.5, S_DECK + 4.6),
            v3(0.0, -14.8, S_DECK + 4.6),
            v2(5.0, 0.5),
            v2(5.0, 0.5),
        );
    }
}

/// The portal crane over the store: two braced box legs on bogies, a box girder
/// across the top, and a trolley over the hatch with its hoist block let down on
/// cables (`part::LAUNCHER_HOIST`: the block goes down into the open hatch and back up
/// while a round is loaded, the cables stretching from the trolley). Everything at the
/// scale of a 36 m warhead.
fn crane(b: &mut MeshBuilder) {
    let y = S_STORE_Y;
    let girder = S_CRANE_TOP - 3.0;
    for sx in [-1.0f32, 1.0] {
        let x = sx * S_CRANE_X;
        // The bogie on the deck, and the leg: a tapering box up to the girder.
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.chamfered_box(v3(x, y, S_DECK + 0.9), v3(3.4, 12.0, 1.8), 0.5);
        b.paint(PLATING);
        b.beam(
            v3(x, y, S_DECK + 1.8),
            v3(x, y, girder),
            v2(2.6, 4.2),
            v2(2.0, 2.6),
        );
        // Raking struts from the bogie's ends to the leg.
        if b.fine() {
            b.paint(ACCENT).pattern(pattern::PLAIN);
            for sy in [-1.0f32, 1.0] {
                b.beam(
                    v3(x, y + sy * 5.2, S_DECK + 1.8),
                    v3(x, y + sy * 1.0, girder - 4.0),
                    v2(1.1, 1.1),
                    v2(1.0, 1.0),
                );
            }
        }
    }
    // The girder, the owner's colour along its top, end housings.
    b.paint(PLATING);
    b.chamfered_box(
        v3(0.0, y, girder + 1.5),
        v3(S_CRANE_X * 2.0 + 3.0, 3.0, 3.0),
        0.4,
    );
    b.paint(PLATING).pattern(pattern::TEAM_BAND);
    b.plate(
        v3(0.0, y, girder + 3.0),
        v2(S_CRANE_X * 2.0 - 2.0, 2.2),
        0.1,
        0.05,
    );
    b.paint(ACCENT).pattern(pattern::PLAIN);
    for sx in [-1.0f32, 1.0] {
        b.chamfered_box(
            v3(sx * (S_CRANE_X + 0.4), y, girder + 3.5),
            v3(3.6, 3.8, 1.8),
            0.4,
        );
    }
    // The trolley over the hatch and its hoist block.
    let t = v3(6.0, y, girder);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.chamfered_box(t + v3(0.0, 0.0, 3.8), v3(5.0, 4.2, 1.8), 0.4);
    let hook = S_DECK + S_STORE_H + 4.0;
    b.with_part(part::LAUNCHER_HOIST, |b| {
        if b.fine() {
            b.paint(METAL);
            for dx in [-1.2, 1.2] {
                b.cylinder_between(
                    t + v3(dx, 0.0, 0.0),
                    v3(t.x + dx, y, hook + 1.4),
                    0.12,
                    0.12,
                    4,
                );
            }
        }
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.chamfered_box(v3(t.x, y, hook + 0.7), v3(3.4, 2.2, 1.4), 0.3);
    });
}

/// Coolant on +y: two big tanks on a skid either side of the axis, their pump house,
/// and pipes into the bunker's flank.
fn coolant(b: &mut MeshBuilder) {
    let base = S_DECK + 0.5;
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.chamfered_box(v3(0.0, 29.0, S_DECK + 0.25), v3(40.0, 13.0, 0.5), 1.0);
    for x in [-13.0, 13.0] {
        tank(b, v3(x, 29.0, base), 4.4, 7.5);
    }
    // The pump house between the tanks: a plain armoured block.
    b.paint(PLATING);
    b.chamfered_box(v3(0.0, 29.0, base + 2.4), v3(9.0, 9.0, 4.8), 1.0);
    team_panel(b, v3(0.0, 29.0, base + 4.8), v2(5.0, 5.0));
    b.paint(METAL);
    for sx in [-1.0f32, 1.0] {
        pipe(
            b,
            &[
                v3(sx * 8.8, 29.0, base + 1.6),
                v3(sx * 4.5, 29.0, base + 1.6),
            ],
            0.8,
        );
        pipe(
            b,
            &[
                v3(sx * 2.4, 24.5, base + 2.0),
                v3(sx * 2.4, 19.0, base + 2.0),
                v3(sx * 2.4, 15.6, 3.4),
            ],
            0.8,
        );
    }
}

// ---- the Parhelion interceptor array -----------------------------------------------

const A_DECK: f32 = 0.9;
/// The launch block's roof and its half size there (x, y); the cells go down to `A_CELL_FLOOR`.
const A_TOP: f32 = 6.0;
const A_BLOCK: Vec2 = Vec2::new(6.9, 11.3);
const A_CELL_FLOOR: f32 = 1.2;
/// Cell centres at (±`A_CELL`, ±`A_CELL`), each open `A_CELL_HALF` either way; the band
/// round the four runs out to `A_BAND`.
const A_CELL: f32 = 2.4;
const A_CELL_HALF: f32 = 2.0;
const A_BAND: f32 = 5.0;
const A_LEAF_X: f32 = 4.9;
const A_LEAF_Y: f32 = 4.85;
const A_LEAF_Z0: f32 = A_TOP + 0.05;
const A_LEAF_Z1: f32 = A_TOP + 1.0;
/// An interceptor: its radius, and its nose's tip, a metre under the doors.
const A_ROUND_R: f32 = 0.7;
const A_NOSE: f32 = A_LEAF_Z0 - 1.0;
const A_RAIL: (f32, f32) = (5.3, 5.85);
/// The radar mast, and its turning head's pivot.
const A_MAST: Vec2 = Vec2::new(-13.5, 0.0);
const A_HEAD: f32 = 17.6;

pub(super) fn nuke_defense(b: &mut MeshBuilder, _tech: u8) {
    b.set_spinner_pivot(A_MAST.extend(A_HEAD));
    if b.coarse() {
        array_coarse(b);
        return;
    }
    slab(b, 19.2, 3.0, 0.4, A_DECK);
    launch_block(b);
    b.with_part(part::SILO_ROUND, |b| {
        for (sx, sy) in [(-1.0, -1.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
            interceptor(b, v2(sx * A_CELL, sy * A_CELL));
        }
    });
    b.with_part(part::SILO_DOOR, |b| b.mirror_y(array_leaf));
    array_track(b);
    radar(b);
    magazine(b);
    // Coolant on -x, piped into the block's flanks.
    b.mirror_y(|b| {
        tank(b, v3(-13.5, 11.0, A_DECK), 1.7, 3.6);
        b.paint(METAL);
        pipe(
            b,
            &[
                v3(-11.9, 10.4, A_DECK + 2.6),
                v3(-9.0, 9.0, A_DECK + 2.6),
                v3(-7.0, 8.0, 3.2),
            ],
            0.35,
        );
    });
    // The capacitor bank on +x+y, its busbar into the block.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.block(v3(10.4, 8.6, A_DECK), v3(17.6, 11.6, A_DECK + 0.4));
    for x in [11.8, 14.0, 16.2] {
        capacitor(b, v3(x, 10.1, A_DECK + 0.4), 0.9, 2.6);
    }
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.beam(
        v3(10.4, 9.4, A_DECK + 0.3),
        v3(7.1, 7.6, A_DECK + 0.3),
        v2(1.0, 0.6),
        v2(1.0, 0.6),
    );
}

fn array_coarse(b: &mut MeshBuilder) {
    b.paint(PLATING);
    b.cuboid_open(v3(0.0, 0.0, A_DECK * 0.5), v3(38.4, 38.4, A_DECK));
    b.cuboid_open(
        v3(0.0, 0.0, A_TOP * 0.5),
        v3(A_BLOCK.x * 2.0 + 0.6, A_BLOCK.y * 2.0 + 0.6, A_TOP),
    );
    b.with_part(part::SILO_DOOR, |b| b.mirror_y(array_leaf));
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.cuboid_open(A_MAST.extend(A_HEAD * 0.5), v3(2.2, 2.2, A_HEAD));
    b.with_part(part::SPINNER, |b| {
        b.paint(PLATING);
        b.cuboid_open(A_MAST.extend(A_HEAD + 1.7), v3(1.8, 5.0, 3.4));
    });
    b.paint(PLATING);
    b.cuboid_open(
        v3(13.5, -2.0, (A_DECK + 5.5) * 0.5),
        v3(7.0, 12.0, 5.5 - A_DECK),
    );
    team_panel(b, v3(13.5, -2.0, 5.5), v2(4.5, 5.0));
}

/// The vertical-launch block: sloped light sides on a dark plinth, four cells sunk in
/// its roof, the web between them, a hazard band
/// round the four, and a dark deck out to the edge.
fn launch_block(b: &mut MeshBuilder) {
    let ring = |plan: &[[f32; 2]], z: f32| -> Vec<Vec3> {
        plan.iter().map(|p| v3(p[0], p[1], z)).collect()
    };
    let foot = chamfered_rect(A_BLOCK + Vec2::splat(0.6), 1.8);
    let roof = chamfered_rect(A_BLOCK, 1.4);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.chamfered_box(
        v3(0.0, 0.0, A_DECK + 0.3),
        (A_BLOCK * 2.0 + Vec2::splat(1.9)).extend(0.6),
        2.2,
    );
    b.paint(PLATING);
    b.loft(&[ring(&foot, A_DECK), ring(&roof, A_TOP)], false, false);

    let (bx, by, c) = (A_BLOCK.x, A_BLOCK.y, 1.4);
    b.paint(PLATING_DARK);
    b.mirror_y(|b| {
        b.face(&[
            v3(-bx, A_BAND, A_TOP),
            v3(bx, A_BAND, A_TOP),
            v3(bx, by - c, A_TOP),
            v3(bx - c, by, A_TOP),
            v3(-bx + c, by, A_TOP),
            v3(-bx, by - c, A_TOP),
        ])
    });
    for yaw in [0.0, PI] {
        b.yawed(Vec3::ZERO, yaw, |b| {
            b.face(&[
                v3(A_BAND, -A_BAND, A_TOP),
                v3(bx, -A_BAND, A_TOP),
                v3(bx, A_BAND, A_TOP),
                v3(A_BAND, A_BAND, A_TOP),
            ])
        });
    }
    let (inner, web) = (A_CELL + A_CELL_HALF, A_CELL - A_CELL_HALF);
    b.paint(ACCENT).pattern(pattern::HAZARD);
    b.radial(4, |b| {
        b.face(&[
            v3(inner, -inner, A_TOP),
            v3(A_BAND, -A_BAND, A_TOP),
            v3(A_BAND, A_BAND, A_TOP),
            v3(inner, inner, A_TOP),
        ])
    });
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.decal(v3(0.0, 0.0, A_TOP), v2(web * 2.0, inner * 2.0));
    for sx in [-1.0, 1.0] {
        b.decal(
            v3(sx * A_CELL, 0.0, A_TOP),
            v2(A_CELL_HALF * 2.0, web * 2.0),
        );
    }

    // The cells.
    for (sx, sy) in [(-1.0, -1.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
        let m = v2(sx * A_CELL, sy * A_CELL);
        let h = A_CELL_HALF;
        let corner = [m + v2(h, h), m + v2(-h, h), m + v2(-h, -h), m + v2(h, -h)];
        b.paint(PLATING_DARK);
        for k in 0..4 {
            wall_in(b, corner[k], corner[(k + 1) % 4], A_CELL_FLOOR, A_TOP);
        }
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.decal(m.extend(A_CELL_FLOOR), v2(h * 2.0, h * 2.0));
        if b.fine() {
            // Steel guide rails down the cell's corners.
            b.paint(METAL);
            for k in corner {
                let at = m + (k - m) * 0.86;
                b.cuboid(
                    at.extend((A_CELL_FLOOR + A_TOP) * 0.5 - 0.2),
                    v3(0.14, 0.14, A_TOP - A_CELL_FLOOR - 0.6),
                );
            }
        }
    }
}

/// An interceptor in its cell (`part::SILO_ROUND`): a slim white nose on a dark body.
fn interceptor(b: &mut MeshBuilder, at: Vec2) {
    let sides = if b.fine() { 10 } else { 6 };
    let plan = ngon(sides, 1.0);
    let shoulder = A_NOSE - 2.5;
    let nose: &[(f32, f32)] = if b.fine() {
        &[
            (0.0, 0.03),
            (0.35, 0.27),
            (0.95, 0.5),
            (1.75, 0.65),
            (2.5, A_ROUND_R),
        ]
    } else {
        &[(0.0, 0.03), (0.9, 0.5), (2.5, A_ROUND_R)]
    };
    let sections: Vec<Section> = nose
        .iter()
        .map(|&(d, r)| Section::new(A_NOSE - d, r).shifted(at.x, at.y))
        .collect();
    b.paint(PLATING);
    b.loft_z(&plan, &sections);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.prism(
        at.extend(shoulder - 0.3),
        sides,
        A_ROUND_R + 0.03,
        A_ROUND_R + 0.03,
        0.3,
    );
    b.paint(PLATING_DARK);
    b.prism(
        at.extend(A_CELL_FLOOR + 0.1),
        sides,
        A_ROUND_R,
        A_ROUND_R,
        shoulder - 0.3 - A_CELL_FLOOR - 0.1,
    );
}

/// One leaf of the array's doors (the +y one): an armoured slab over two cells, a
/// hazard nosing, the owner's colour, carriages over the rails.
fn array_leaf(b: &mut MeshBuilder) {
    let (x, y0, y1, z0, z1) = (A_LEAF_X, 0.03, A_LEAF_Y, A_LEAF_Z0, A_LEAF_Z1);
    if b.coarse() {
        b.paint(PLATING);
        b.decal(v3(0.0, (y0 + y1) * 0.5, z1), v2(x * 2.0, y1 - y0));
        return;
    }
    let edge = 0.6;
    b.paint(ACCENT).pattern(pattern::HAZARD);
    b.block(v3(-x, y0, z0), v3(x, edge, z1 - 0.2));
    b.paint(PLATING);
    b.plate(
        v3(0.0, (edge + y1) * 0.5, z0),
        v2(x * 2.0, y1 - edge),
        z1 - z0,
        0.25,
    );
    if b.fine() {
        b.paint(PLATING_DARK);
        b.block(
            v3(-0.25, edge + 0.4, z1 - 0.05),
            v3(0.25, y1 - 0.4, z1 + 0.2),
        );
    }
    team_panel(b, v3(-2.4, (edge + y1) * 0.5, z1), v2(2.2, 2.2));
    b.paint(ACCENT).pattern(pattern::PLAIN);
    for sx in [-1.0f32, 1.0] {
        let (a, c) = (sx * (x - 0.05), sx * (A_RAIL.1 + 0.15));
        b.block(v3(a.min(c), 1.0, A_TOP + 0.5), v3(a.max(c), 4.0, z1 - 0.2));
    }
}

/// The array's door track: rails either side, buffers, drive housings at the ends.
fn array_track(b: &mut MeshBuilder) {
    for sx in [-1.0f32, 1.0] {
        let (a, c) = (sx * A_RAIL.0, sx * A_RAIL.1);
        b.paint(METAL);
        b.block(
            v3(a.min(c), -10.8, A_TOP - 0.05),
            v3(a.max(c), 10.8, A_TOP + 0.4),
        );
        b.paint(ACCENT).pattern(pattern::PLAIN);
        for sy in [-1.0f32, 1.0] {
            let (p, q) = (sy * 10.1, sy * 10.7);
            let (u, w) = (sx * (A_RAIL.0 - 0.1), sx * (A_RAIL.1 + 0.1));
            b.block(
                v3(u.min(w), p.min(q), A_TOP - 0.05),
                v3(u.max(w), p.max(q), A_TOP + 0.8),
            );
        }
    }
    b.mirror_y(|b| {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.chamfered_box(v3(0.0, 10.5, A_TOP + 0.45), v3(6.0, 1.0, 1.0), 0.3);
        if b.fine() {
            b.paint(METAL);
            for x in [-1.8, 1.8] {
                b.cylinder_between(
                    v3(x, 9.95, A_TOP + 0.5),
                    v3(x, 9.3, A_TOP + 0.5),
                    0.2,
                    0.2,
                    6,
                );
            }
        }
    });
}

/// The tracking radar on -x: a housing, a mast, a turntable and, turning on it
/// (`part::SPINNER`), two phased arrays back to back, dark ribbed faces.
fn radar(b: &mut MeshBuilder) {
    let c = A_MAST.extend(A_DECK);
    b.paint(PLATING);
    b.chamfered_box(c + v3(0.0, 0.0, 2.2), v3(6.0, 7.0, 4.4), 1.0);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.cuboid(c + v3(3.02, 0.0, 2.6), v3(0.1, 4.0, 1.4));
    b.paint(PLATING);
    b.prism(
        c + Vec3::Z * 4.4,
        round(b, 8),
        1.1,
        0.8,
        A_HEAD - 0.4 - A_DECK - 4.4,
    );
    if b.fine() {
        // Cooling louvres on the transmitter housing, and the waveguide up the mast.
        louvres(b, c + v3(-1.6, 2.0, 4.4), v2(1.8, 1.6), 3);
        b.paint(METAL);
        b.cylinder_between(
            c + v3(1.9, -1.4, 4.4),
            c + v3(1.3, -0.5, 5.6),
            0.18,
            0.18,
            6,
        );
        b.cylinder_between(
            c + v3(1.3, -0.5, 5.6),
            c + v3(0.95, -0.35, A_HEAD - 0.5 - A_DECK),
            0.18,
            0.18,
            6,
        );
        b.paint(ACCENT).pattern(pattern::PLAIN);
        for z in [9.0, 13.0] {
            b.prism(A_MAST.extend(z), 8, 1.05, 1.02, 0.4);
        }
    }
    b.paint(METAL);
    b.prism(A_MAST.extend(A_HEAD - 0.4), round(b, 10), 1.5, 1.4, 0.4);
    b.with_part(part::SPINNER, |b| {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.prism(A_MAST.extend(A_HEAD), round(b, 10), 1.25, 1.2, 0.3);
        b.paint(PLATING);
        b.block(
            A_MAST.extend(A_HEAD + 0.3) + v3(-2.0, -2.4, 0.0),
            A_MAST.extend(A_HEAD + 0.6) + v3(2.0, 2.4, 0.0),
        );
        for yaw in [0.0, PI] {
            b.yawed(A_MAST.extend(A_HEAD), yaw, |b| {
                b.pitched(v3(1.0, 0.0, 2.2), 0.35, array_face);
            });
        }
    });
}

/// One face of the tracking radar, centred at its frame's origin, looking along +x.
fn array_face(b: &mut MeshBuilder) {
    b.paint(PLATING);
    if b.fine() {
        b.chamfered_box(Vec3::ZERO, v3(0.5, 5.0, 3.4), 0.2);
    } else {
        b.cuboid(Vec3::ZERO, v3(0.5, 5.0, 3.4));
    }
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.cuboid(v3(0.27, 0.0, 0.0), v3(0.08, 4.4, 2.8));
    if b.fine() {
        b.paint(PLATING_DARK);
        for z in [-0.7, 0.7] {
            b.cuboid(v3(0.35, 0.0, z), v3(0.08, 4.0, 0.2));
        }
    }
}

/// The magazine on +x: a white block holding the rounds, the owner's colour on its
/// roof, the loader duct into the block.
fn magazine(b: &mut MeshBuilder) {
    let c = v3(13.5, -2.0, A_DECK);
    let top = 5.5;
    b.paint(PLATING);
    b.chamfered_box(
        c.truncate().extend((A_DECK + top) * 0.5),
        v3(7.0, 12.0, top - A_DECK),
        1.0,
    );
    team_panel(b, c.truncate().extend(top), v2(4.5, 5.0));
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.cuboid_open(v3(8.6, -2.0, A_DECK + 1.4), v3(3.4, 3.0, 2.8));
    if b.fine() {
        // The loading hatch the rounds go in by, hazard-edged round a dark opening, its
        // lid (`part::LAUNCHER_LID`, slid back along -y while a round is loaded), and a
        // lift beam aft.
        b.paint(ACCENT).pattern(pattern::HAZARD);
        b.block(v3(11.2, 1.0, top - 0.02), v3(15.8, 3.6, top + 0.12));
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.decal(v3(13.5, 2.3, top + 0.13), v2(3.8, 2.0));
        b.with_part(part::LAUNCHER_LID, |b| {
            b.paint(PLATING_DARK);
            b.block(v3(11.6, 1.3, top + 0.1), v3(15.4, 3.3, top + 0.25));
        });
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.block(v3(11.0, -7.0, top), v3(16.0, -5.6, top + 0.4));
        hatch_hoist(b, v3(13.5, 2.3, top));
    }
}

/// A small gantry over the magazine's hatch (`at` its middle on the roof): two legs
/// either end, a beam, a trolley, and the hoist block on two cables
/// (`part::LAUNCHER_HOIST`), let down through the open hatch and back up.
fn hatch_hoist(b: &mut MeshBuilder, at: Vec3) {
    let (span, beam) = (2.8, 3.0);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    for sx in [-1.0, 1.0] {
        b.cuboid(at + v3(sx * span, 0.0, beam * 0.5), v3(0.4, 0.5, beam));
    }
    b.paint(PLATING);
    b.cuboid(
        at + v3(0.0, 0.0, beam + 0.2),
        v3(span * 2.0 + 0.6, 0.45, 0.4),
    );
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.cuboid(at + v3(0.0, 0.0, beam - 0.2), v3(1.0, 0.8, 0.4));
    let block = 1.4;
    b.with_part(part::LAUNCHER_HOIST, |b| {
        b.paint(METAL);
        for dx in [-0.3, 0.3] {
            b.cylinder_between(
                at + v3(dx, 0.0, beam - 0.4),
                at + v3(dx, 0.0, block + 0.5),
                0.05,
                0.05,
                4,
            );
        }
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.cuboid(at + v3(0.0, 0.0, block + 0.25), v3(0.9, 0.7, 0.5));
    });
}

// ---- the kit ---------------------------------------------------------------------

/// The lot's slab: a dark foot and a light deck, `half` metres out.
fn slab(b: &mut MeshBuilder, half: f32, chamfer: f32, foot: f32, deck: f32) {
    let plan = chamfered_rect(v2(half, half), chamfer);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft_z(&plan, &[Section::new(0.0, 1.0), Section::new(foot, 1.0)]);
    b.paint(PLATING);
    b.loft_z(&plan, &[Section::new(foot, 0.99), Section::new(deck, 0.97)]);
}

/// A pipe run through `points`, with a flange at each joint.
fn pipe(b: &mut MeshBuilder, points: &[Vec3], radius: f32) {
    let sides = round(b, 8);
    for pair in points.windows(2) {
        b.cylinder_between(pair[0], pair[1], radius, radius, sides);
    }
    if b.fine() {
        for &p in &points[1..points.len() - 1] {
            b.spheroid(p, Vec3::splat(radius * 1.25), 6, 2);
        }
    }
}

/// Louvres lying on a horizontal surface: a dark tray and plain slats across it, unlit
/// (the shared `vent` lights its slats' black).
fn louvres(b: &mut MeshBuilder, base_center: Vec3, size: Vec2, slats: usize) {
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.plate(base_center, size, 0.08, 0.04);
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    let pitch = size.x / slats as f32;
    for i in 0..slats {
        let x = base_center.x - size.x * 0.5 + pitch * (i as f32 + 0.5);
        b.plate(
            v3(x, base_center.y, base_center.z + 0.08),
            v2(pitch * 0.42, size.y * 0.78),
            0.12,
            0.03,
        );
    }
}

/// A coolant tank: a banded drum with a domed head.
fn tank(b: &mut MeshBuilder, base: Vec3, r: f32, h: f32) {
    let sides = round(b, 12);
    b.paint(PLATING);
    b.loft_z(
        &ngon(sides, 1.0),
        &[
            Section::new(base.z, r).shifted(base.x, base.y),
            Section::new(base.z + h, r).shifted(base.x, base.y),
            Section::new(base.z + h + r * 0.35, r * 0.7).shifted(base.x, base.y),
            Section::new(base.z + h + r * 0.5, r * 0.25).shifted(base.x, base.y),
        ],
    );
    if b.fine() {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        for z in [0.3, 0.62] {
            b.prism(base + Vec3::Z * h * z, sides, r + 0.08, r + 0.08, h * 0.08);
        }
    }
}

/// A capacitor can: a dark drum with a steel crown.
fn capacitor(b: &mut MeshBuilder, base: Vec3, r: f32, h: f32) {
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.prism(base, 6, r, r, h);
    if b.fine() {
        b.paint(METAL);
        b.prism(base + Vec3::Z * h, 6, r * 0.85, r * 0.6, r * 0.3);
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    use super::{ARRAY_TRAVEL, SILO_TRAVEL};
    use crate::{build_model_scaled, part, MeshLod, Model};

    const SILO: (f32, f32, u8) = (42.5, 26.0, 4);
    const ARRAY: (f32, f32, u8) = (18.75, 20.0, 3);

    fn built(key: &str, (r, h, tech): (f32, f32, u8)) -> Model {
        build_model_scaled(key, r, h, tech).unwrap()
    }

    fn tris(mesh: &MeshLod) -> usize {
        mesh.indices.len() / 3
    }

    #[test]
    fn launchers_fit_their_budgets_and_tag_doors_and_rounds() {
        for (key, size) in [("nuke_silo", SILO), ("nuke_defense", ARRAY)] {
            let model = built(key, size);
            let [full, mid, coarse] = [0, 1, 2].map(|l| tris(&model.lods[l]));
            println!("{key}: {full}/{mid}/{coarse}");
            // The silo's crane keeps its height far off.
            assert!(
                full <= 6000 && coarse < if key == "nuke_silo" { 80 } else { 60 },
                "{key}: {full}/{mid}/{coarse}"
            );
            assert!(
                mid as f32 <= full as f32 * 0.45 + 20.0,
                "{key}: reduced {mid} of {full}"
            );
            for (l, lod) in model.lods.iter().enumerate() {
                assert!(
                    lod.vertices.iter().any(|v| v.part == part::SILO_DOOR),
                    "{key} lod{l}: doors"
                );
            }
            for lod in &model.lods[..2] {
                assert!(
                    lod.vertices.iter().any(|v| v.part == part::SILO_ROUND),
                    "{key}: rounds"
                );
            }
        }
        assert!(built("nuke_defense", ARRAY).lods[..2]
            .iter()
            .all(|l| l.vertices.iter().any(|v| v.part == part::SPINNER)));
    }

    /// The shader reads the sim's rounds word by `gpu_consts::launcher`'s copy of it.
    #[test]
    fn the_rounds_word_matches_the_sim() {
        use crate::gpu_consts::launcher as l;
        use mc_sim::nukes as n;
        assert_eq!(l::STOCK_MASK, n::LAUNCHER_STOCK_MASK);
        assert_eq!(l::CAPACITY_SHIFT, n::LAUNCHER_CAPACITY_SHIFT);
        assert_eq!(l::MARK, n::LAUNCHER_MARK);
        assert_eq!(l::MANUAL, n::LAUNCHER_MANUAL);
        assert_eq!(l::QUEUED_SHIFT, n::LAUNCHER_QUEUED_SHIFT);
        assert_eq!(l::ICON_SILO, mc_data::IconKind::Silo as u32);
    }

    /// The shaders read a ship's jump by `gpu_consts::warp_status`'s copy of the bits.
    #[test]
    fn the_warp_bits_match_the_sim() {
        use crate::gpu_consts::warp_status as w;
        use mc_sim::mirror as m;
        assert_eq!(w::DAMPED, m::UNIT_WARP_DAMPED);
        assert_eq!(w::IN_WARP, m::UNIT_IN_WARP);
    }

    /// The shader reads a walker's twin arm gun by `gpu_consts::arm_twin`'s copy of the bits.
    #[test]
    fn the_twin_arm_bits_match_the_sim() {
        use crate::gpu_consts::arm_twin as a;
        use mc_sim::mirror as m;
        assert_eq!(a::SHIFT, m::UNIT_TWIN_SHIFT);
        assert_eq!(a::MASK, m::UNIT_TWIN_MASK);
        assert_eq!(a::RIGHT, m::UNIT_TWIN_RIGHT);
        assert_eq!(crate::gpu_consts::unit_house::SHIFT, m::UNIT_HOUSE_SHIFT);
        const {
            assert!(
                a::MASK << a::SHIFT & (a::RIGHT | 3) == 0 && a::RIGHT < 1 << m::UNIT_HOUSE_SHIFT
            )
        };
    }

    /// The load cycle's plant up close: a lid and a hoist on each, the hoist's cables
    /// held at the trolley above the split and its block below it, clear of the split
    /// either way, and the block let down stopping on the opening, not through it.
    #[test]
    fn load_cycle_plant_is_rigged_where_the_shader_moves_it() {
        use crate::gpu_consts::launcher as l;
        for (key, size, split, drop) in [
            ("nuke_silo", SILO, l::SILO_HOIST_SPLIT, l::SILO_HOIST_DROP),
            (
                "nuke_defense",
                ARRAY,
                l::ARRAY_HOIST_SPLIT,
                l::ARRAY_HOIST_DROP,
            ),
        ] {
            let model = built(key, size);
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
            assert!(
                hoist.iter().all(|&h| (h - split).abs() > 0.3),
                "{key}: split"
            );
            assert!(hoist.iter().any(|&h| h > split), "{key}: cable tops");
            let low = hoist.iter().copied().fold(f32::MAX, f32::min);
            let floor = lid.iter().copied().fold(f32::MAX, f32::min);
            assert!(
                low - drop >= floor && low - drop < floor + 0.3,
                "{key}: block let down to {} over the opening at {floor}",
                low - drop
            );
        }
    }

    /// Sound meshes: unit normals agreeing with the winding, one material and part per
    /// triangle, nothing below the ground but the silo's tube.
    #[test]
    fn launchers_are_sound_meshes() {
        for (key, size, floor) in [("nuke_silo", SILO, -17.01), ("nuke_defense", ARRAY, -1e-3)] {
            for mesh in &built(key, size).lods {
                for t in mesh.indices.chunks(3) {
                    let v = [0, 1, 2].map(|k| mesh.vertices[t[k] as usize]);
                    let p = v.map(|v| Vec3::from(v.pos));
                    let n = (p[1] - p[0]).cross(p[2] - p[0]);
                    assert!(n.length() > 2e-7, "{key}: degenerate at {}", p[0]);
                    for v in &v {
                        assert!(
                            n.normalize().dot(Vec3::from(v.normal)) > 0.5,
                            "{key}: winding at {}",
                            p[0]
                        );
                        assert!(v.pos[2] >= floor, "{key}: below ground at {}", p[0]);
                    }
                    assert!(v[0].material == v[1].material && v[1].material == v[2].material);
                    assert!(v[0].part == v[1].part && v[1].part == v[2].part);
                }
            }
        }
    }

    /// The leaves meet on y = 0 and, slid by their travel, clear the opening.
    #[test]
    fn open_leaves_clear_the_opening() {
        for (key, size, travel, half) in [
            ("nuke_silo", SILO, SILO_TRAVEL, 5.0),
            ("nuke_defense", ARRAY, ARRAY_TRAVEL, 5.0),
        ] {
            let model = built(key, size);
            for lod in &model.lods {
                let leaves: Vec<Vec3> = lod
                    .vertices
                    .iter()
                    .filter(|v| v.part == part::SILO_DOOR)
                    .map(|v| Vec3::from(v.pos))
                    .collect();
                let near = leaves.iter().map(|p| p.y.abs()).fold(f32::MAX, f32::min);
                assert!(near < 0.1, "{key}: leaves meet on y = 0 ({near})");
                assert!(
                    near + travel >= half,
                    "{key}: open leaves clear the opening"
                );
            }
        }
    }

    /// Where the rounds stand, for the sim session's launch effects.
    #[test]
    fn rounds_sit_under_the_doors() {
        for (key, size) in [("nuke_silo", SILO), ("nuke_defense", ARRAY)] {
            let model = built(key, size);
            let z = |part: u32, f: fn(f32, f32) -> f32, init: f32| {
                model.lods[0]
                    .vertices
                    .iter()
                    .filter(|v| v.part == part)
                    .map(|v| v.pos[2])
                    .fold(init, f)
            };
            let (top, low) = (
                z(part::SILO_ROUND, f32::max, f32::MIN),
                z(part::SILO_ROUND, f32::min, f32::MAX),
            );
            let door = z(part::SILO_DOOR, f32::min, f32::MAX);
            println!("{key}: rounds z {low:.2}..{top:.2}, door underside {door:.2}");
            assert!(top < door - 0.5, "{key}: rounds stand under the doors");
        }
    }

    /// Previews with the doors shut and open: `MODEL_DUMP_DIR=... cargo test -p mc-models
    /// -- --ignored strategic_previews`.
    #[test]
    #[ignore = "writes preview images"]
    fn strategic_previews() {
        let dir =
            std::path::PathBuf::from(std::env::var_os("MODEL_DUMP_DIR").expect("MODEL_DUMP_DIR"));
        std::fs::create_dir_all(&dir).unwrap();
        for (key, size, travel) in [
            ("nuke_silo", SILO, SILO_TRAVEL),
            ("nuke_defense", ARRAY, ARRAY_TRAVEL),
        ] {
            let model = built(key, size);
            for (l, lod) in model.lods.iter().enumerate() {
                let mut open = lod.clone();
                for v in &mut open.vertices {
                    if v.part == part::SILO_DOOR {
                        v.pos[1] += travel * v.pos[1].signum();
                    }
                }
                let res = if l == 0 { 768 } else { 256 };
                for az in [-38.0f32, 52.0, 142.0, 232.0] {
                    if l > 0 && az != -38.0 {
                        continue;
                    }
                    crate::preview::render(lod, res, az)
                        .write_ppm(&dir.join(format!("{key}_l{l}_{}.ppm", az as i32)))
                        .unwrap();
                    crate::preview::render(&open, res, az)
                        .write_ppm(&dir.join(format!("{key}_l{l}_{}_open.ppm", az as i32)))
                        .unwrap();
                }
            }
        }
    }
}
