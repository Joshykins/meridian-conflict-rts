//! Dominion: the ARC's tech 4 dreadnought (`aster_t4_dreadnought`, mesh `space_dreadnought`),
//! a 490 m capital warship of the upper air laid round a spinal AEB, the heaviest bore the
//! ARC builds: an arrowhead in plan riding tall on a segmented keel blade ([`hull`]), two
//! narrower armoured hulls stacked on its deck and wrapped in swept-back vertebrae
//! ([`stacked`]). Every solid on it is shaded flat, facet by facet (`with_facets`): ARC
//! warships are hard planes meeting at clear edges. `space.ron` carries the contract's
//! numbers.
//!
//! The contract, all here:
//! - `CASEMATES`: the four Arc Cannon casemates (weapons 1..=4), each an armoured drum let
//!   into the flank carrying twin barrels one over the other, authored facing +X with the
//!   muzzles `CASEMATE_REACH` ahead of the pivot (the unit file rests them turned
//!   outboard); `RIFLES`: the two low twin bolt rifle houses on the spine (weapons 5, 6).
//! - `CELL_*`: the two blocks of hatched SAM cells of weapon 7, port block first.
//! - `hull::BORE` (`models::spinal_bore`): the spinal AEB's mouth, breech and the eight
//!   coil stages along its axis. Weapon 0's `muzzle` is `BORE.muzzle`. Its coils
//!   carry `pattern::COIL + stage` (stage 0 at the breech, 7 at the mouth), so the
//!   renderer climbs the charge along them (`renderer/titan_charge.rs`).
//! - `NOZZLES`, `LIFT_JETS`, `RIG`: the shared spacecraft rig (`capital.rs`), the same on
//!   ship's `LAMPS` (`stacked::LAMPS`, its mast's strobe with the hull's).
//!
//! ARC hardware: the guns are unlit; the AEB's coils are the one big lit feature.

use super::capital::{self, CapitalRig, Leg};
use super::*;
use crate::aster::bolt_rifle::{bolt_rifle, siege_howitzer};
use crate::builder::{chamfered_rect, CellGrid};
use crate::SpinalBore;
use glam::Vec2;

mod detail;
mod flanks;
mod hull;
mod prow;
mod stacked;
mod stern;
#[cfg(test)]
mod tests;

pub(crate) use hull::BORE as DOMINION_BORE;
pub(crate) use stacked::LAMPS as DOMINION_LAMPS;

pub(super) fn build(b: &mut MeshBuilder) {
    b.with_facets(stacked::build);
}

// ---- the contract -------------------------------------------------------------------

/// The flat belly under the hull where the legs stow, with the legs down.
const KEEL: f32 = 27.0;
/// The spinal AEB's axis height, and its mouth (the foremost point of the ship).
const AXIS_Z: f32 = 64.0;
const MUZZLE_X: f32 = 250.0;

/// The Arc Cannon casemates in weapon order 1..=4: fore port, fore starboard, aft port,
/// aft starboard. Each turns about a vertical axis through its drum and bears from 10
/// degrees off the nose round its own beam to 10 degrees off the stern; the hull within
/// reach of its barrels stays inboard of the drum.
const CASEMATES: [[f32; 3]; 4] = [
    [90.0, 62.0, 58.0],
    [90.0, -62.0, 58.0],
    [-70.0, 66.0, 58.0],
    [-70.0, -66.0, 58.0],
];
/// The twin barrels: one `CASEMATE_STACK` over and one under the pivot, from the pivot to
/// `CASEMATE_REACH` ahead of it (the unit file's `howitzer` length), their radius.
const CASEMATE_STACK: f32 = 2.5;
const CASEMATE_REACH: f32 = 26.0;
const CASEMATE_R: f32 = 1.2;
/// The drum's radius about the pivot and its half height.
const DRUM_R: f32 = 7.0;
const DRUM_H: f32 = 7.5;

/// The low twin bolt rifle houses on the spine (weapons 5 and 6): the forward one rests
/// facing ahead, the aft one astern; each sweeps 300 degrees, so nothing on the hull stands
/// above its deck within `RIFLE_REACH` and a little of its pivot. Bores `RIFLE_SPREAD`
/// either side of the pivot.
const RIFLES: [[f32; 3]; 2] = [[150.0, 0.0, 118.0], [-150.0, 0.0, 118.0]];
const RIFLE_SPREAD: f32 = 2.5;
const RIFLE_REACH: f32 = 24.0;
/// The deck the rifle houses stand on: nothing within their sweep stands above it.
const RIFLE_DECK: f32 = 114.2;

/// The SAM cells (weapon 7): two blocks of 4 x 2 hatched cells either side of the
/// centre line, their hatches lying on `CELL_DECK`. Firing order as the unit file's
/// muzzles: the port block's eight, then the starboard block's.
const CELL_DECK: f32 = 130.0;
const CELL_PITCH: f32 = 6.0;
const CELL_HALF: f32 = 2.4;
const CELL_CENTRE: Vec2 = Vec2::new(-17.0, 25.0);
const CELL_FIRE_PORT: [(u8, u8); 8] = [
    (1, 0),
    (3, 1),
    (2, 0),
    (0, 1),
    (3, 0),
    (1, 1),
    (0, 0),
    (2, 1),
];
const CELL_FIRE_STARBOARD: [(u8, u8); 8] = [
    (1, 1),
    (3, 0),
    (2, 1),
    (0, 0),
    (3, 1),
    (1, 0),
    (0, 1),
    (2, 0),
];
/// The armoured box each block's cells sit in (half extents about its centre).
const CELL_BOX: Vec2 = Vec2::new(14.0, 7.2);

// ---- the rig ------------------------------------------------------------------------

/// Leg size against the Bastion's (hinge 36 m over the foot).
const LEG: f32 = 0.9;
/// The bay doors' outer face, a little under the keel.
const BAY_SILL: f32 = KEEL - 0.7;
/// The hull's stern, where its raked end begins; the drives stand aft of it ([`stern`]).
const STERN: f32 = -226.0;

/// Every drive's mouth, facing aft: the two mains (the rig's pair), then the auxiliaries,
/// port before starboard ([`stern::AUX`]).
pub(crate) const NOZZLES: [[f32; 3]; 6] = [
    [stern::MAIN_X, stern::MAIN_Y, stern::MAIN_Z],
    [stern::MAIN_X, -stern::MAIN_Y, stern::MAIN_Z],
    [stern::AUX[0][0], stern::AUX[0][1], stern::AUX[0][2]],
    [stern::AUX[0][0], -stern::AUX[0][1], stern::AUX[0][2]],
    [stern::AUX[1][0], stern::AUX[1][1], stern::AUX[1][2]],
    [stern::AUX[1][0], -stern::AUX[1][1], stern::AUX[1][2]],
];
/// Downward lift jets under the belly: a pair astern, a pair under the forward hull.
pub(crate) const LIFT_JETS: [[f32; 3]; 4] = [
    [-172.0, -12.0, KEEL - 0.4],
    [-172.0, 12.0, KEEL - 0.4],
    [158.0, -11.0, KEEL - 0.4],
    [158.0, 11.0, KEEL - 0.4],
];
const LIFT_JET_SIZE: f32 = 1.25;

/// Two pairs of legs stowing into belly bays, the drives' glow and vanes, the lift jets.
/// The hull keeps a flat belly at `KEEL` over the bays and the jets: forward from x 86 to
/// 166 at least 22 either side, astern from x -184 to -142 at least 36.
pub(crate) const RIG: CapitalRig = CapitalRig {
    legs: Some([
        Leg {
            hinge: [120.0, 16.0, 36.0 * LEG],
            stow: 1.0,
            bay: [89.5, 124.0, 10.8, 21.2],
            size: LEG,
        },
        Leg {
            hinge: [-176.0, 30.0, 36.0 * LEG],
            stow: -1.0,
            bay: [-180.0, -145.5, 24.8, 35.2],
            size: LEG,
        },
    ]),
    door_hinge: KEEL - 0.3,
    drives: Some((
        [stern::MAIN_X, stern::MAIN_Z, stern::MAIN_Y, stern::MAIN_Y],
        stern::MAIN_SIZE,
    )),
    lift_jets: Some((
        [
            LIFT_JETS[3][0],
            LIFT_JETS[3][1],
            LIFT_JETS[1][0],
            LIFT_JETS[1][1],
        ],
        LIFT_JETS[0][2],
    )),
    ramp: None,
};

// ---- hull tools -----------------------------------------------------------------------

/// A closed section through a hull symmetric about y 0: `half` is the port half from the
/// keel's middle round to the deck's middle, (y, z); the ring runs up the port side and
/// back down the starboard side.
fn full_ring(x: f32, half: &[[f32; 2]]) -> Vec<Vec3> {
    let mut ring: Vec<Vec3> = half.iter().map(|p| v3(x, p[0], p[1])).collect();
    ring.extend(
        half.iter()
            .rev()
            .filter(|p| p[0] > 0.0)
            .map(|p| v3(x, -p[0], p[1])),
    );
    ring
}

/// A hull lofted through the sections `half(x)` at each of `xs`, capped both ends.
fn hull_loft(b: &mut MeshBuilder, xs: &[f32], half: &dyn Fn(f32) -> Vec<[f32; 2]>) {
    b.loft(
        &xs.iter()
            .map(|&x| full_ring(x, &half(x)))
            .collect::<Vec<_>>(),
        true,
        true,
    );
}

/// Linear interpolation through a table of rows whose first column is x.
fn lerp_rows<const N: usize>(rows: &[[f32; N]], x: f32) -> [f32; N] {
    if x <= rows[0][0] {
        return rows[0];
    }
    for p in rows.windows(2) {
        if x <= p[1][0] {
            let t = (x - p[0][0]) / (p[1][0] - p[0][0]);
            let mut out = [0.0; N];
            for (i, o) in out.iter_mut().enumerate() {
                *o = p[0][i] + (p[1][i] - p[0][i]) * t;
            }
            return out;
        }
    }
    rows[rows.len() - 1]
}

/// Points along the port half section `half` between corners `from..=to`, `u` of the way
/// along each end edge, pushed `lift` metres out along the outline's normal.
fn surface_run(half: &[[f32; 2]], from: usize, to: usize, u: [f32; 2], lift: f32) -> Vec<Vec2> {
    let s: Vec<Vec2> = half.iter().map(|p| v2(p[0], p[1])).collect();
    let centre = v2(0.0, (s[0].y + s[s.len() - 1].y) * 0.5);
    let mut pts: Vec<Vec2> = vec![s[from] + (s[from + 1] - s[from]) * u[0]];
    pts.extend_from_slice(&s[from + 1..to]);
    pts.push(s[to - 1] + (s[to] - s[to - 1]) * u[1]);
    let n = pts.len();
    (0..n)
        .map(|i| {
            let a = pts[i.saturating_sub(1)];
            let c = pts[(i + 1).min(n - 1)];
            let d = (c - a).normalize_or_zero();
            let mut normal = v2(d.y, -d.x);
            if normal.dot(pts[i] - centre) < 0.0 {
                normal = -normal;
            }
            pts[i] + normal * lift
        })
        .collect()
}

/// An armour plate lying on the port side of a hull whose half section at `x` is
/// `half(x)`, from `x0` to `x1` over corners `from..=to`, `thick` metres proud.
fn plate_on(
    b: &mut MeshBuilder,
    half: &dyn Fn(f32) -> Vec<[f32; 2]>,
    x: [f32; 2],
    from: usize,
    to: usize,
    u: [f32; 2],
    thick: f32,
) {
    let ring = |x: f32| {
        let h = half(x);
        let outer = surface_run(&h, from, to, u, thick);
        let inner = surface_run(&h, from, to, u, -0.5);
        outer
            .iter()
            .chain(inner.iter().rev())
            .map(|p| v3(x, p.x, p.y))
            .collect::<Vec<_>>()
    };
    b.loft(&[ring(x[0]), ring(x[1])], true, true);
}

/// The belly: lift jets in armoured wells, a keel strake.
fn belly(b: &mut MeshBuilder) {
    for port in LIFT_JETS {
        let p = Vec3::from(port);
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.frustum(
            p + Vec3::Z * 0.8,
            v2(16.0, 15.0),
            v2(12.5, 11.5),
            1.6,
            v2(0.0, 0.0),
        );
        capital::lift_jet(b, p, LIFT_JET_SIZE);
    }
    capital::gear(b, &RIG, BAY_SILL);
}

// ---- the casemates ------------------------------------------------------------------------

/// A casemate bound to `weapon` at `pivot`, let into a hull face at |y| `face`: the fixed
/// armoured socket on the hull round the drum (a squared frame and a hood over it), and
/// the drum itself, turning with the gun, with its twin barrels one over the other and
/// their sleeve pitching and kicking in it.
fn casemate(b: &mut MeshBuilder, weapon: usize, pivot: Vec3, face: f32) {
    let s = pivot.y.signum();
    let fine = b.fine();
    let frame = |y: f32, half: Vec2, cut: f32| {
        chamfered_rect(half, cut)
            .iter()
            .map(|p| v3(pivot.x + p[0], s * y, pivot.z + p[1]))
            .collect::<Vec<_>>()
    };
    let hole = v2(DRUM_R + 0.8, DRUM_H + 0.8);
    let rim = v2(DRUM_R + 5.0, DRUM_H + 4.0);
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(
        &[
            frame(face - 1.5, hole, 2.0),
            frame(face - 1.5, rim, 4.0),
            frame(face + 1.6, rim - v2(0.8, 0.8), 3.5),
            frame(face + 1.6, hole, 2.0),
            frame(face - 1.5, hole, 2.0),
        ],
        false,
        false,
    );
    // The hood over the drum, raked down and out.
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    let top = pivot.z + DRUM_H + 1.4;
    b.loft(
        &[
            vec![
                v3(pivot.x - rim.x, s * (face - 1.0), top),
                v3(pivot.x + rim.x, s * (face - 1.0), top),
                v3(pivot.x + rim.x, s * (face - 1.0), top + 3.6),
                v3(pivot.x - rim.x, s * (face - 1.0), top + 3.6),
            ],
            vec![
                v3(pivot.x - rim.x + 3.0, s * (face + 6.0), top),
                v3(pivot.x + rim.x - 3.0, s * (face + 6.0), top),
                v3(pivot.x + rim.x - 3.0, s * (face + 6.0), top + 1.2),
                v3(pivot.x - rim.x + 3.0, s * (face + 6.0), top + 1.2),
            ],
        ],
        true,
        true,
    );
    b.with_house(weapon, pivot, 1.8, |b| {
        b.at(pivot, |b| {
            let plan = octagon(DRUM_R);
            b.paint(PLATING).pattern(pattern::AIRFRAME);
            b.loft_z(
                &plan,
                &[
                    Section::new(-DRUM_H, 0.7),
                    Section::new(-DRUM_H + 2.2, 1.0),
                    Section::new(DRUM_H - 2.2, 1.0),
                    Section::new(DRUM_H, 0.7),
                ],
            );
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.loft_z(&plan, &[Section::new(-0.7, 1.03), Section::new(0.7, 1.03)]);
            b.with_recoil(|b| {
                b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
                b.chamfered_box(v3(6.0, 0.0, 0.0), v3(8.0, 5.6, 11.6), 1.6);
                for dz in [-CASEMATE_STACK, CASEMATE_STACK] {
                    siege_howitzer(b, v3(0.0, 0.0, dz), v3(CASEMATE_REACH, 0.0, dz), CASEMATE_R);
                }
            });
            team_panel(b, v3(-1.5, 0.0, DRUM_H), v2(3.0, 3.6));
            if fine {
                b.paint(METAL).pattern(pattern::PLAIN);
                b.cuboid(v3(-2.0, 3.2, DRUM_H + 0.6), v3(3.0, 1.4, 1.2));
            }
        });
    });
}

// ---- the dorsal rifles --------------------------------------------------------------------

/// Plan of a twin bolt rifle house about its pivot: a blunt wedge face, squared cheeks, a
/// bustle.
fn rifle_plan() -> Vec<[f32; 2]> {
    vec![
        [9.0, -2.5],
        [9.0, 2.5],
        [6.0, 6.8],
        [-6.0, 7.2],
        [-8.5, 5.5],
        [-8.5, -5.5],
        [-6.0, -7.2],
        [6.0, -6.8],
    ]
}

/// A twin bolt rifle house bound to `weapon` at `pivot`, authored facing +X: a squat
/// angular house on a low ring, the two Paladin guns (the Vanes) side by side through its
/// face, their cells outboard.
fn rifle_house(b: &mut MeshBuilder, weapon: usize, pivot: Vec3) {
    b.with_house(weapon, pivot, 1.5, |b| {
        b.at(pivot, |b| {
            let plan = rifle_plan();
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.prism(v3(0.0, 0.0, -5.2), b.sides(10), 6.6, 6.6, 1.0);
            b.paint(PLATING).pattern(pattern::AIRFRAME);
            b.loft_z(
                &plan,
                &[
                    Section::new(-4.2, 1.0),
                    Section::new(-1.2, 1.0),
                    Section::scaled(3.6, 0.78, 0.8).shifted(-1.4, 0.0),
                ],
            );
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.loft_z(
                &plan,
                &[
                    Section::scaled(3.55, 0.6, 0.62).shifted(-1.8, 0.0),
                    Section::scaled(3.9, 0.58, 0.6).shifted(-1.9, 0.0),
                ],
            );
            b.with_recoil(|b| {
                b.mirror_y(|b| {
                    bolt_rifle(
                        b,
                        v3(-3.0, RIFLE_SPREAD, 0.0),
                        v3(RIFLE_REACH, RIFLE_SPREAD, 0.0),
                        1.3,
                    );
                });
            });
            team_panel(b, v3(-3.0, 0.0, 3.9), v2(3.0, 4.0));
            if b.fine() {
                b.paint(PLATING).pattern(pattern::PLAIN);
                b.chamfered_box(v3(-5.6, 0.0, 4.2), v3(1.6, 9.0, 0.8), 0.3);
            }
        });
    });
}

/// A low armoured pedestal under a rifle house at `pivot`, from `foot` up to the rifles'
/// deck, a dark band at its head.
fn rifle_pedestal(b: &mut MeshBuilder, pivot: [f32; 3], foot: f32) {
    let plan = chamfered_rect(v2(13.0, 11.0), 4.0);
    b.at(v3(pivot[0], pivot[1], 0.0), |b| {
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.loft_z(
            &plan,
            &[Section::new(foot, 1.1), Section::new(RIFLE_DECK - 0.6, 1.0)],
        );
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.loft_z(
            &plan,
            &[
                Section::new(RIFLE_DECK - 0.6, 0.94),
                Section::new(RIFLE_DECK, 0.94),
            ],
        );
    });
}

/// Both dorsal rifle houses.
fn rifles(b: &mut MeshBuilder) {
    for (i, p) in RIFLES.into_iter().enumerate() {
        rifle_house(b, 5 + i, Vec3::from(p));
    }
}

// ---- the SAM cells ----------------------------------------------------------------------

/// Both blocks of SAM cells: armoured boxes from `foot` up to the cell deck under a dark
/// coaming, eight hatches each hinged on its outboard edge, a missile standing in each cell.
fn sam_cells(b: &mut MeshBuilder, foot: f32) {
    for (side, fire) in [(1.0f32, &CELL_FIRE_PORT), (-1.0, &CELL_FIRE_STARBOARD)] {
        let c = v2(CELL_CENTRE.x, CELL_CENTRE.y * side);
        let plan = chamfered_rect(CELL_BOX, 1.6);
        b.at(c.extend(0.0), |b| {
            b.paint(PLATING).pattern(pattern::AIRFRAME);
            b.loft_z(
                &plan,
                &[Section::new(foot, 1.06), Section::new(CELL_DECK - 0.6, 1.0)],
            );
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.loft_z(
                &plan,
                &[
                    Section::new(CELL_DECK - 0.6, 1.0),
                    Section::new(CELL_DECK, 0.97),
                ],
            );
        });
        let grid = CellGrid {
            centre: c,
            deck: CELL_DECK,
            pitch: CELL_PITCH,
            half: CELL_HALF,
            nx: 4,
            ny: 2,
            hinge_y: true,
        };
        for m in b.cell_block(grid, fire) {
            let out = (m.y - c.y).signum();
            let hinge = m.y + out * CELL_HALF;
            b.with_part(part::CELL_HATCH, |b| {
                b.paint(PLATING).pattern(pattern::PLAIN);
                b.cuboid(
                    v3(m.x, m.y, CELL_DECK + 0.2),
                    v3(CELL_HALF * 2.0, CELL_HALF * 2.0, 0.4),
                );
                if b.fine() {
                    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
                    for dx in [-1.2, 1.2] {
                        b.cuboid(
                            v3(m.x + dx, m.y, CELL_DECK + 0.5),
                            v3(0.4, CELL_HALF * 1.6, 0.2),
                        );
                    }
                    b.paint(METAL).pattern(pattern::PLAIN);
                    for dx in [-1.4, 1.4] {
                        b.cylinder_between(
                            v3(m.x + dx - 0.5, hinge, CELL_DECK + 0.15),
                            v3(m.x + dx + 0.5, hinge, CELL_DECK + 0.15),
                            0.3,
                            0.3,
                            4,
                        );
                    }
                }
                b.paint(ACCENT).pattern(pattern::PLAIN);
                b.decal(
                    v3(m.x, m.y - out * 1.5, CELL_DECK + 0.42),
                    v2(CELL_HALF * 1.6, 0.5),
                );
            });
            b.with_part(part::CELL_ROUND, |b| {
                let top = CELL_DECK - 0.4;
                let foot = if b.fine() { top - 11.0 } else { top - 4.0 };
                b.paint(PLATING).pattern(pattern::PLAIN);
                b.cylinder_between(
                    v3(m.x, m.y, foot),
                    v3(m.x, m.y, top - 2.0),
                    1.0,
                    1.0,
                    b.sides(6),
                );
                b.paint(PLATING_DARK).pattern(pattern::PLAIN);
                b.cylinder_between(
                    v3(m.x, m.y, top - 2.0),
                    v3(m.x, m.y, top),
                    1.0,
                    0.12,
                    b.sides(6),
                );
            });
        }
    }
}

// ---- the spinal AEB ----------------------------------------------------------------------

/// Paints what follows as the AEB's live light for coil stage `stage` (0 at the breech,
/// 7 at the mouth).
fn coil_light(b: &mut MeshBuilder, stage: usize) {
    b.paint(GLOW)
        .pattern(pattern::COIL + (stage as u32).min(pattern::COIL_STAGES - 1));
}

/// An octagon of radius `r` round the bore axis (flat on top), as (y, z) about the axis.
fn octagon(r: f32) -> Vec<[f32; 2]> {
    (0..8)
        .map(|k| {
            let a = (k as f32 + 0.5) * std::f32::consts::TAU / 8.0;
            [a.cos() * r, a.sin() * r]
        })
        .collect()
}

/// One charge coil round the bore at `x`: a dark octagonal ring `r` out, `width` long,
/// with its live band of stage `stage` standing round its middle, and a lit inner face.
fn coil_ring(b: &mut MeshBuilder, x: f32, r: f32, width: f32, stage: usize) {
    let c = v3(x, 0.0, AXIS_Z);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    octagon_collar(
        b,
        c,
        [-width * 0.5, width * 0.5],
        [r, r],
        [r * 0.72, r * 0.72],
    );
    coil_light(b, stage);
    octagon_collar(
        b,
        c,
        [-width * 0.18, width * 0.18],
        [r + 0.5, r + 0.5],
        [r - 0.2, r - 0.2],
    );
}

/// A closed octagonal annulus about the x axis through `c`, from `x[0]` to `x[1]`
/// (relative to `c`), outer and inner radii at each end.
fn octagon_collar(b: &mut MeshBuilder, c: Vec3, x: [f32; 2], outer: [f32; 2], inner: [f32; 2]) {
    let ring = |x: f32, r: f32| {
        octagon(r)
            .iter()
            .map(|p| c + v3(x, p[0], p[1]))
            .collect::<Vec<_>>()
    };
    let rings = vec![
        ring(x[0], inner[0]),
        ring(x[0], outer[0]),
        ring(x[1], outer[1]),
        ring(x[1], inner[1]),
        ring(x[0], inner[0]),
    ];
    b.loft(&rings, false, false);
}

/// The coils of `bore`'s first `stages` stages: `per` rings to each stage, `pitch` apart
/// about the stage's centre, `bore.coil_radius` out; and the dark bore tube they ring,
/// from the breech to `tube_end`.
fn coils(
    b: &mut MeshBuilder,
    bore: &SpinalBore,
    stages: usize,
    per: usize,
    pitch: f32,
    tube_end: f32,
) {
    let r = bore.coil_radius;
    b.paint(TREAD).pattern(pattern::NONE);
    let tube = octagon(r * 0.55)
        .iter()
        .map(|p| [p[0], p[1] + AXIS_Z])
        .collect::<Vec<_>>();
    b.extrude_x(&tube, bore.breech[0], tube_end);
    let per = if b.fine() { per } else { per.min(2) };
    for (stage, c) in bore.coils.iter().enumerate().take(stages) {
        for k in 0..per {
            let x = c[0] + (k as f32 - (per as f32 - 1.0) * 0.5) * pitch;
            coil_ring(b, x, r, pitch * 0.55, stage);
        }
    }
}

/// The AEB's mouth let flush into a flat face at `MUZZLE_X`: an octagonal throat `r`
/// across, a dark frame round it standing a hand proud, the last coil's light ringing the
/// throat a little way in, and a black throat back to the bore.
fn bore_mouth(b: &mut MeshBuilder, r: f32, frame: f32) {
    let at = |x: f32, r: f32| {
        octagon(r)
            .iter()
            .map(|p| v3(x, p[0], p[1] + AXIS_Z))
            .collect::<Vec<_>>()
    };
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft(
        &[
            at(MUZZLE_X, r),
            at(MUZZLE_X, r + frame),
            at(MUZZLE_X - 1.6, r + frame),
            at(MUZZLE_X - 1.6, r),
            at(MUZZLE_X, r),
        ],
        false,
        false,
    );
    b.paint(TREAD).pattern(pattern::NONE);
    throat(b, MUZZLE_X - 1.0, 29.0, [r, r * 0.7]);
    coil_light(b, 7);
    let c = v3(MUZZLE_X, 0.0, AXIS_Z);
    for x in [-5.0, -11.0] {
        octagon_collar(
            b,
            c,
            [x - 1.2, x + 1.2],
            [r + 0.1, r + 0.1],
            [r - 0.9, r - 0.9],
        );
    }
    if b.fine() {
        b.paint(METAL).pattern(pattern::PLAIN);
        for p in octagon(r + frame * 0.5) {
            b.cuboid(v3(MUZZLE_X - 0.2, p[0], AXIS_Z + p[1]), v3(0.4, 1.2, 1.2));
        }
    }
}

/// A flat bow face open round the mouth: each edge of the hull's last section `outer`
/// (all on one x) joined to a ring `r` out from the bore's axis, facing forward.
fn bow_face(b: &mut MeshBuilder, outer: &[Vec3], r: f32) {
    let inner: Vec<Vec3> = outer
        .iter()
        .map(|p| {
            let d = v2(p.y, p.z - AXIS_Z).normalize_or_zero() * r;
            v3(p.x, d.x, AXIS_Z + d.y)
        })
        .collect();
    let n = outer.len();
    for i in 0..n {
        let j = (i + 1) % n;
        let mut quad = vec![outer[i], outer[j], inner[j], inner[i]];
        if (quad[1] - quad[0]).cross(quad[2] - quad[0]).x < 0.0 {
            quad.reverse();
        }
        b.face(&quad);
    }
}

/// A black throat going back `depth` from `x` on the bore's axis, `r` across at its mouth
/// and its floor: a thick-walled shell whose inner wall faces the axis, closed by a floor
/// facing forward, so it reads as a hole from any side.
fn throat(b: &mut MeshBuilder, x: f32, depth: f32, r: [f32; 2]) {
    let c = v3(x, 0.0, AXIS_Z);
    b.paint(TREAD).pattern(pattern::NONE);
    octagon_collar(b, c, [-depth, 0.0], [r[1] + 0.6, r[0] + 0.6], [r[1], r[0]]);
    let floor = octagon(r[1] + 0.3)
        .iter()
        .map(|p| [p[0], p[1] + AXIS_Z])
        .collect::<Vec<_>>();
    b.extrude_x(&floor, x - depth - 2.0, x - depth + 0.2);
}

/// A lit grille over coil stage `stage` at `x`, let into a deck at `top`: a dark frame
/// `half` across either side of `y`, the stage's light in bars under louvres.
fn grille(b: &mut MeshBuilder, x: f32, y: f32, top: f32, half: f32, stage: usize) {
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.block(
        v3(x - 7.0, y - half, top - 0.3),
        v3(x + 7.0, y + half, top + 0.5),
    );
    coil_light(b, stage);
    let bars = if b.fine() { 4 } else { 2 };
    for k in 0..bars {
        let bx = x - 5.0 + k as f32 * 10.0 / (bars - 1) as f32;
        b.block(
            v3(bx - 0.9, y - half + 1.2, top + 0.5),
            v3(bx + 0.9, y + half - 1.2, top + 0.8),
        );
    }
    if b.fine() {
        b.paint(PLATING_DARK).pattern(pattern::PLAIN);
        for k in 0..3 {
            let bx = x - 3.3 + k as f32 * 3.3;
            b.block(
                v3(bx - 0.5, y - half + 0.8, top + 0.5),
                v3(bx + 0.5, y + half - 0.8, top + 1.3),
            );
        }
    }
}

/// A vertebra across the deck at `x`, `length` long: a band between the port half's
/// `under` and `over` outlines (y, z from the centre line out, the same count), mirrored
/// to starboard, its tips swept back `sweep` per metre out so it reads as a chevron; a dark
/// band down its front face and a thin lit seam across its crown.
fn rib(
    b: &mut MeshBuilder,
    x: f32,
    under: &[[f32; 2]],
    over: &[[f32; 2]],
    length: f32,
    sweep: f32,
) {
    let ring = |x0: f32, lift: f32, shrink: f32| {
        over.iter()
            .map(|p| v3(x0 - p[0] * sweep, p[0], p[1] + lift))
            .chain(
                under
                    .iter()
                    .rev()
                    .map(|p| v3(x0 - p[0] * sweep, p[0], p[1] + shrink)),
            )
            .collect::<Vec<_>>()
    };
    let (a, f) = (x - length * 0.5, x + length * 0.5);
    b.mirror_y(|b| {
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.loft(&[ring(a, 0.0, 0.0), ring(f, 0.0, 0.0)], true, true);
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.loft(
            &[ring(f - 0.3, -0.6, 0.6), ring(f + 0.5, -0.6, 0.6)],
            true,
            true,
        );
    });
    if b.fine() {
        let crown = over[0][1];
        b.paint(GLOW).pattern(pattern::PLAIN);
        b.block(
            v3(f + 0.45, -5.0, crown - 1.6),
            v3(f + 0.65, 5.0, crown - 1.2),
        );
    }
}

/// Lamp fittings for `lamps`: flood housings under the belly, the sidelights, strobes.
fn lamps(b: &mut MeshBuilder, l: &crate::CapitalLamps) {
    if !b.mid() {
        return;
    }
    for p in l.floods {
        let p = Vec3::from(*p);
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.cuboid(p + Vec3::Z * 1.2, v3(5.0, 5.0, 2.2));
        b.paint(GLOW_LAMP);
        b.cuboid(p + Vec3::Z * 0.05, v3(3.8, 3.8, 0.2));
    }
    for (p, glow) in [
        (l.nav_port, GLOW_NAV_RED),
        (l.nav_starboard, GLOW_NAV_GREEN),
    ] {
        let p = Vec3::from(p);
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.cuboid(p - Vec3::Y * p.y.signum() * 1.1, v3(4.4, 1.4, 3.4));
        b.paint(glow);
        b.cuboid(p, v3(2.6, 1.0, 2.0));
    }
    b.paint(GLOW_LAMP);
    for p in l.strobes {
        b.cuboid(Vec3::from(*p), v3(1.6, 1.6, 1.6));
    }
}

/// The floods under the belly, shared by every design (they hang under the rig's belly);
/// each design has its own sidelights and strobes.
const FLOODS: [[f32; 3]; 4] = [
    [140.0, 8.0, KEEL - 1.1],
    [140.0, -8.0, KEEL - 1.1],
    [-150.0, 12.0, KEEL - 1.1],
    [-150.0, -12.0, KEEL - 1.1],
];

/// A shield projector: an armoured drum carrying a pale lens, the hull field thrown from
/// its face (`set_shield_emitter`).
fn shield_projector(b: &mut MeshBuilder, at: Vec3) {
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.prism(at - Vec3::Z * 4.0, b.sides(10), 5.2, 4.6, 3.2);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.prism(at - Vec3::Z * 0.8, b.sides(10), 4.6, 4.0, 0.6);
    b.paint(GLASS).pattern(pattern::PLAIN);
    b.prism(at - Vec3::Z * 0.2, 8, 3.8, 2.2, 2.4);
    b.set_shield_emitter(at + Vec3::Z * 1.0);
}

// ---- superstructure ------------------------------------------------------------------------

/// A plan pointed forward: `half_length` either way of its middle, `half_width` across,
/// the nose `nose` across its flat tip.
fn pointed_plan(half_length: f32, half_width: f32, nose: f32) -> Vec<[f32; 2]> {
    let k = (half_width - nose) * 0.8;
    let c = (half_width * 0.25).min(3.0);
    vec![
        [half_length, -nose],
        [half_length, nose],
        [half_length - k, half_width],
        [-half_length + c, half_width],
        [-half_length, half_width - c],
        [-half_length, -half_width + c],
        [-half_length + c, -half_width],
        [half_length - k, -half_width],
    ]
}

/// One armoured tier of a superstructure from `z0` to `z1` between `back` and `front`
/// (x): a recessed dark band at its foot, then light plating, its sides drawn in and its
/// face raked back by `rake` at the top.
fn tier(b: &mut MeshBuilder, x: [f32; 2], half_width: f32, z: [f32; 2], rake: f32, nose: f32) {
    let [back, front] = x;
    let [z0, z1] = z;
    let plan = pointed_plan((front - back) * 0.5, half_width, nose);
    let band = 1.2f32.min((z1 - z0) * 0.3);
    b.at(v3((front + back) * 0.5, 0.0, 0.0), |b| {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.loft_z(
            &plan,
            &[Section::new(z0, 0.96), Section::new(z0 + band, 0.96)],
        );
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        let top = 1.0 - rake / (front - back);
        b.loft_z(
            &plan,
            &[
                Section::new(z0 + band, 1.0),
                Section::scaled(z1, top, 1.0 - 2.0 / half_width).shifted(-rake * 0.5, 0.0),
            ],
        );
    });
}

/// A mast from `foot` to `top` at `x` on the centre line carrying the search radar's
/// turning array under a strobe, and yards.
fn mast(b: &mut MeshBuilder, x: f32, foot: f32, top: f32) {
    let rz = top - 7.0;
    b.paint(METAL).pattern(pattern::PLAIN);
    b.beam(
        v3(x, 0.0, foot),
        v3(x, 0.0, top),
        v2(3.6, 3.6),
        v2(1.4, 1.4),
    );
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    b.prism(v3(x, 0.0, rz - 3.0), b.sides(8), 3.4, 2.8, 2.0);
    if b.mid() {
        b.set_spinner_pivot(v3(x, 0.0, rz));
        b.with_part(part::SPINNER, |b| {
            b.paint(PLATING).pattern(pattern::PLAIN);
            b.loft(
                &[
                    vec![
                        v3(x + 2.2, -20.0, rz - 0.8),
                        v3(x + 2.2, 20.0, rz - 0.8),
                        v3(x + 0.8, 20.0, rz + 7.0),
                        v3(x + 0.8, -20.0, rz + 7.0),
                    ],
                    vec![
                        v3(x + 0.3, -20.0, rz - 0.8),
                        v3(x + 0.3, 20.0, rz - 0.8),
                        v3(x - 1.1, 20.0, rz + 7.0),
                        v3(x - 1.1, -20.0, rz + 7.0),
                    ],
                ],
                true,
                true,
            );
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.cuboid(v3(x - 1.8, 0.0, rz + 3.0), v3(3.0, 34.0, 1.6));
            b.cuboid(v3(x, 0.0, rz + 0.2), v3(3.4, 4.0, 2.0));
        });
    }
    if b.fine() {
        b.paint(METAL).pattern(pattern::PLAIN);
        b.beam(
            v3(x, -9.0, top - 14.0),
            v3(x, 9.0, top - 14.0),
            v2(0.8, 0.8),
            v2(0.8, 0.8),
        );
        b.paint(GLOW_RED);
        b.mirror_y(|b| b.cuboid(v3(x, 8.6, top - 13.0), v3(0.9, 0.9, 0.9)));
    }
}
