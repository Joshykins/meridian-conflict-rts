//! Precursor artifacts: the map props of the Foundry's builders, standing where the
//! ice has not taken them (`mc_map::PropKind::Precursor*`). Authored at scale 1; the
//! renderer applies each prop's own scale and heading, and the map bakes what is
//! solid of each (`PropKind::solid_plan`) into the path grid.
//!
//! One language throughout, from the Precursor concept sheet: pale alloy plates
//! (`PRECURSOR`) laid over a dark core (`PRECURSOR_DARK`) with clean gaps between
//! them, so every joint reads as a dark line; cold blue-white light
//! (`GLOW_PRECURSOR`) inlaid down the seams; pieces that float apart from the body
//! with nothing holding them. Silhouettes are monumental and geometric: these are
//! seen from a hundred metres to several kilometres, so the big shapes and a few
//! strong light lines carry them, not greebles.
//!
//! Model space: x along the prop's heading, y left, z up, origin on the ground at the
//! prop's position. Footings are sunk well into the ground (to -30 m, rings and
//! shards deeper) so a prop stands on a slope without showing its underside. What is
//! outside a prop's solid plan is open ground or overhead: an arch's road, a ring's
//! eye, the space under a beacon's crown.

use std::f32::consts::{FRAC_PI_4, TAU};

use glam::{Affine3A, Vec2, Vec3};

use super::builder::{chamfered_rect, ngon, MeshBuilder, Section};
use super::library::ModelDef;
use super::material::*;
use super::part;

pub(super) const MODELS: &[ModelDef] = &[
    ModelDef::new("precursor_spire", 20.0, 144.0, spire),
    ModelDef::new("precursor_pylon", 18.0, 70.0, pylon),
    ModelDef::new("precursor_arch", 62.0, 85.0, arch),
    ModelDef::new("precursor_ring", 124.0, 163.0, ring),
    ModelDef::new("precursor_shard", 50.0, 45.0, shard),
    ModelDef::new("precursor_wall", 42.0, 18.0, wall),
    ModelDef::new("precursor_beacon", 21.0, 64.0, beacon),
    ModelDef::new("precursor_conduit", 30.5, 1.0, conduit),
    ModelDef::new("precursor_fragment", 7.0, 21.0, fragment),
];

/// Whether the beacon's crown turns (`part::SPINNER`). The entity shader spins
/// every spinner, props included, but at a unit's radar-dish rate (1.6 rad/s),
/// far too quick for a 40 m crown; off until props get a slow rate of their own.
const CROWN_TURNS: bool = false;

pub(super) fn v3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

pub(super) fn pale(b: &mut MeshBuilder) {
    b.paint(PRECURSOR);
}

pub(super) fn dark(b: &mut MeshBuilder) {
    b.paint(PRECURSOR_DARK);
}

/// A light channel, dormant: Precursor work is mostly unlit metal, its channels dark
/// glass (`PRECURSOR_INLAY`). Only the parts that work (lenses, projectors, sockets,
/// keystones) burn: `key_light`.
pub(super) fn light(b: &mut MeshBuilder) {
    b.paint(super::material::PRECURSOR_INLAY);
}

/// Live Precursor light, for the few parts of a structure that work.
pub(super) fn key_light(b: &mut MeshBuilder) {
    b.paint(GLOW_PRECURSOR);
}

/// Axis-aligned rectangle plan (x, y), for the coarse level where chamfers are wasted.
fn rect(half_x: f32, half_y: f32) -> Vec<[f32; 2]> {
    vec![
        [half_x, -half_y],
        [half_x, half_y],
        [-half_x, half_y],
        [-half_x, -half_y],
    ]
}

/// A plan with its corners cut, or a plain rectangle at the coarse level.
pub(super) fn cut_rect(b: &MeshBuilder, half_x: f32, half_y: f32, chamfer: f32) -> Vec<[f32; 2]> {
    if b.coarse() {
        rect(half_x, half_y)
    } else {
        chamfered_rect(Vec2::new(half_x, half_y), chamfer)
    }
}

/// A plan with its corners cut at full detail only.
pub(super) fn fine_rect(b: &MeshBuilder, half_x: f32, half_y: f32, chamfer: f32) -> Vec<[f32; 2]> {
    if b.fine() {
        chamfered_rect(Vec2::new(half_x, half_y), chamfer)
    } else {
        rect(half_x, half_y)
    }
}

/// [`MeshBuilder::loft_z`] without its bottom face (buried), and without its top
/// face too unless `top`.
fn buried_loft(b: &mut MeshBuilder, plan: &[[f32; 2]], sections: &[Section], top: bool) {
    let rings: Vec<Vec<Vec3>> = sections
        .iter()
        .map(|s| {
            plan.iter()
                .map(|p| (Vec2::new(p[0], p[1]) * s.scale + s.shift).extend(s.z))
                .collect()
        })
        .collect();
    b.loft(&rings, false, top);
}

/// The frame whose axes are `x`, `y`, `z` with its origin at `origin`.
fn frame(origin: Vec3, x: Vec3, y: Vec3, z: Vec3) -> Affine3A {
    Affine3A::from_cols(x.into(), y.into(), z.into(), origin.into())
}

/// A frame turned `angle` about z whose x axis runs out along that heading, and
/// whose y then points across it: for pieces laid out round an upright axis.
fn spoke(angle: f32) -> Affine3A {
    Affine3A::from_rotation_z(angle)
}

/// Planar convex outline `points` drawn in by `d` from every edge.
fn inset(points: &[Vec3], d: f32) -> Vec<Vec3> {
    let n = points.len();
    (0..n)
        .map(|i| {
            let p = points[i];
            let u = (points[(i + 1) % n] - p).normalize();
            let v = (points[(i + n - 1) % n] - p).normalize();
            let sin = u.cross(v).length().max(0.25);
            p + (u + v) * (d / sin)
        })
        .collect()
}

/// A plate laid on the planar convex outline `on`, standing `depth` off it along
/// `out`, its front face drawn in by `bevel` so its edges catch the light. No back
/// face: plates are laid on a core.
pub(super) fn panel(b: &mut MeshBuilder, on: &[Vec3], out: Vec3, depth: f32, bevel: f32) {
    let front: Vec<Vec3> = on.iter().map(|&p| p + out * depth).collect();
    let front = if bevel > 0.0 && !b.coarse() {
        inset(&front, bevel)
    } else {
        front
    };
    b.loft(&[on.to_vec(), front], false, true);
}

/// One flat polygon facing `out` (either winding), `lift` off the surface it lies on.
pub(super) fn film(b: &mut MeshBuilder, on: &[Vec3], out: Vec3, lift: f32) {
    let mut points: Vec<Vec3> = on.iter().map(|&p| p + out * lift).collect();
    let normal = (points[1] - points[0]).cross(points[2] - points[0]);
    if normal.dot(out) < 0.0 {
        points.reverse();
    }
    b.face(&points);
}

/// A light channel `width` wide from `a` to `c`, inlaid in the face whose normal is
/// `out`: a thin raised strip, or at the coarse level just a film on the face. Dormant
/// (dark glass); `key_seam` for one that burns.
pub(super) fn seam(b: &mut MeshBuilder, a: Vec3, c: Vec3, out: Vec3, width: f32) {
    light(b);
    inlay(b, a, c, out, width);
}

/// A burning line of light: `seam` for the parts that work.
pub(super) fn key_seam(b: &mut MeshBuilder, a: Vec3, c: Vec3, out: Vec3, width: f32) {
    key_light(b);
    inlay(b, a, c, out, width);
}

fn inlay(b: &mut MeshBuilder, a: Vec3, c: Vec3, out: Vec3, width: f32) {
    let side = (c - a).cross(out).normalize() * width * 0.5;
    let quad = [a - side, c - side, c + side, a + side];
    if b.coarse() {
        film(b, &quad, out, 0.08);
    } else {
        panel(b, &quad, out, 0.14, 0.0);
    }
}

/// Cross-section `section` (pairs a place function understands) swept from angle
/// `a0` to `a1` in `steps`, capped at both ends. With `place` a turn about an axis,
/// every side face is a flat trapezoid.
fn sweep(
    b: &mut MeshBuilder,
    section: &[[f32; 2]],
    a0: f32,
    a1: f32,
    steps: usize,
    place: &impl Fn([f32; 2], f32) -> Vec3,
) {
    let rings: Vec<Vec<Vec3>> = (0..=steps)
        .map(|k| {
            let a = a0 + (a1 - a0) * k as f32 / steps as f32;
            section.iter().map(|&p| place(p, a)).collect()
        })
        .collect();
    b.loft(&rings, true, true);
}

// ---- Spire -----------------------------------------------------------------------
//
// A faceted obelisk. A dark shaft of chamfered square plan tapers from ±10 m to a
// point at 122 m; each of its four faces is clad in two columns of pale plates
// split by a line of light running the height of the face, and the plates are
// cut into courses with dark gaps between. Four blade buttresses stand out of the
// corners at the foot, a collar of chevron plates floats off the upper faces, and a
// capstone hovers over the point with its underside lit.

const SPIRE_TOP: f32 = 116.0;
const SPIRE_HALF: f32 = 10.0;
const SPIRE_CHAMFER: f32 = 4.0;

/// Plan scale of the shaft at height `z`.
fn spire_scale(z: f32) -> f32 {
    1.0 - 0.78 * (z / SPIRE_TOP)
}

/// Distance from the axis to the shaft's +x face at height `z`.
fn spire_face(z: f32) -> f32 {
    SPIRE_HALF * spire_scale(z)
}

/// Half-width of the flat of a face at height `z` (between the corner chamfers).
fn spire_flat(z: f32) -> f32 {
    (SPIRE_HALF - SPIRE_CHAMFER) * spire_scale(z)
}

/// Half-width of the lit seam down the middle of a face.
fn spire_gap(z: f32) -> f32 {
    0.35 + 0.75 * spire_scale(z)
}

fn spire(b: &mut MeshBuilder, _tech: u8) {
    // Footing: sunk deep, with a bevelled shoulder where the shaft leaves it.
    pale(b);
    let foot = fine_rect(b, 13.6, 13.6, 5.0);
    if b.coarse() {
        b.loft_z(&foot, &[Section::new(-30.0, 1.0), Section::new(6.5, 0.9)]);
    } else {
        b.loft_z(
            &foot,
            &[
                Section::new(-30.0, 1.0),
                Section::new(3.0, 1.0),
                Section::new(6.5, 0.84),
            ],
        );
    }
    if b.fine() {
        // A course of light round the footing's shoulder.
        light(b);
        let ring = chamfered_rect(Vec2::splat(13.75), 5.0);
        b.loft_z(&ring, &[Section::new(1.4, 1.0), Section::new(2.2, 1.0)]);
    }

    // The shaft: dark, clad below the coarse level, then a pale point.
    if b.coarse() {
        pale(b);
    } else {
        dark(b);
    }
    let core = cut_rect(b, SPIRE_HALF, SPIRE_HALF, SPIRE_CHAMFER);
    if b.coarse() {
        b.loft_z(
            &core,
            &[
                Section::new(5.0, spire_scale(5.0)),
                Section::new(SPIRE_TOP, spire_scale(SPIRE_TOP)),
                Section::new(SPIRE_TOP + 6.0, 0.0),
            ],
        );
    } else {
        b.loft_z(
            &core,
            &[
                Section::new(5.0, spire_scale(5.0)),
                Section::new(SPIRE_TOP, spire_scale(SPIRE_TOP)),
            ],
        );
        pale(b);
        b.loft_z(
            &core,
            &[
                Section::new(SPIRE_TOP, spire_scale(SPIRE_TOP)),
                Section::new(SPIRE_TOP + 6.0, 0.0),
            ],
        );
    }

    // Each face: the seam of light and the two columns of plates either side of it.
    let slope = SPIRE_HALF * 0.78 / SPIRE_TOP;
    let out = v3(1.0, 0.0, slope).normalize();
    let courses: &[(f32, f32)] = match b.lod() {
        0 => &[
            (6.5, 31.0),
            (33.0, 55.0),
            (57.0, 76.0),
            (78.0, 95.0),
            (97.0, 114.5),
        ],
        1 => &[(6.5, 76.0), (78.0, 114.5)],
        _ => &[],
    };
    b.radial(4, |b| {
        let at = |y: f32, z: f32| v3(spire_face(z), y, z);
        // The seam, a little narrower than the gap so dark shows either side of it.
        let (z0, z1) = (6.5, 114.5);
        let (w0, w1) = (spire_gap(z0) * 0.62, spire_gap(z1) * 0.62);
        let quad = [at(-w0, z0), at(w0, z0), at(w1, z1), at(-w1, z1)];
        light(b);
        if b.coarse() {
            film(b, &quad, out, 0.1);
        } else {
            panel(b, &quad, out, 0.45, 0.0);
        }
        pale(b);
        for &(z0, z1) in courses {
            for side in [-1.0f32, 1.0] {
                let inner = |z: f32| side * spire_gap(z);
                let outer = |z: f32| side * (spire_flat(z) - 0.2);
                let quad = [
                    at(inner(z0), z0),
                    at(outer(z0), z0),
                    at(outer(z1), z1),
                    at(inner(z1), z1),
                ];
                panel(b, &quad, out, 1.3, 0.7);
            }
        }
    });

    // Buttress blades out of the four corners, a line of light down each flank.
    if b.mid() {
        let blade: [[f32; 2]; 6] = [
            [6.0, -4.0],
            [17.4, -4.0],
            [17.4, 6.0],
            [12.6, 38.0],
            [7.8, 68.0],
            [5.4, 62.0],
        ];
        let flank = |r: f32, z: f32, y: f32| v3(r, y, z);
        b.radial(4, |b| {
            b.with(spoke(FRAC_PI_4), |b| {
                // `extrude_y_chamfered` wants (x, z): the blade's radius and height.
                pale(b);
                b.extrude_y_chamfered(&blade, 2.2, 0.8);
                if b.fine() {
                    for y in [-2.2f32, 2.2] {
                        let n = v3(0.0, y.signum(), 0.0);
                        seam(b, flank(15.2, 7.0, y), flank(11.1, 37.0, y), n, 0.7);
                        seam(b, flank(10.8, 40.0, y), flank(7.4, 61.0, y), n, 0.7);
                    }
                }
            });
        });
    }

    // Above the buttresses, blades float off the corners, each over a line of light
    // down the dark corner behind it: the buttress line carried on, broken.
    if b.mid() {
        let blade: [[f32; 2]; 5] = [
            [7.3, 72.0],
            [11.5, 80.0],
            [12.0, 93.0],
            [5.0, 108.0],
            [4.9, 100.0],
        ];
        // Distance from the axis to a corner chamfer at height `z`.
        let corner =
            |z: f32| (SPIRE_HALF - SPIRE_CHAMFER * 0.5) * std::f32::consts::SQRT_2 * spire_scale(z);
        let n = v3(1.0, 0.0, corner(0.0) * 0.78 / SPIRE_TOP).normalize();
        b.radial(4, |b| {
            b.with(spoke(FRAC_PI_4), |b| {
                pale(b);
                if b.fine() {
                    b.extrude_y_chamfered(&blade, 1.8, 0.6);
                } else {
                    b.extrude_y(&blade, -1.8, 1.8);
                }
                seam(
                    b,
                    v3(corner(70.0), 0.0, 70.0),
                    v3(corner(108.0), 0.0, 108.0),
                    n,
                    1.3,
                );
            });
        });
    }

    // The capstone: a point hovering over the point, lit from beneath.
    let cap = cut_rect(b, 5.0, 5.0, 1.8);
    if b.coarse() {
        b.loft_z(
            &cap,
            &[
                Section::new(125.5, 0.0),
                Section::new(131.0, 1.0),
                Section::new(144.0, 0.0),
            ],
        );
        return;
    }
    light(b);
    b.loft_z(&cap, &[Section::new(125.5, 0.12), Section::new(130.5, 1.0)]);
    pale(b);
    b.loft_z(
        &cap,
        &[
            Section::new(130.5, 1.0),
            Section::new(133.0, 1.0),
            Section::new(144.0, 0.0),
        ],
    );
}

// ---- Pylon -----------------------------------------------------------------------
//
// Two angular blades lean in from either side of a dark column with a line of light
// up each face, and meet over it in an arrowhead with a gap between the points: the
// engineer's silhouette on the concept sheet, 70 m tall. Each blade is two plates
// with a lit gap between. A light gem hangs in the arrowhead.

fn pylon(b: &mut MeshBuilder, _tech: u8) {
    pale(b);
    let plinth = fine_rect(b, 9.0, 17.0, 3.0);
    if b.coarse() {
        buried_loft(
            b,
            &plinth,
            &[Section::new(-24.0, 1.0), Section::new(5.0, 0.9)],
            true,
        );
    } else {
        buried_loft(
            b,
            &plinth,
            &[
                Section::new(-24.0, 1.0),
                Section::new(2.5, 1.0),
                Section::new(5.0, 0.86),
            ],
            true,
        );
    }

    // The column and its light.
    dark(b);
    let column = cut_rect(b, 3.6, 2.8, 1.0);
    buried_loft(
        b,
        &column,
        &[Section::new(4.0, 1.0), Section::new(56.0, 0.85)],
        true,
    );
    for x in [-1.0f32, 1.0] {
        let n = v3(x, 0.0, 0.0);
        let at = |z: f32| v3(x * (3.6 - 0.54 * (z - 4.0) / 52.0), 0.0, z);
        seam(b, at(7.0), at(54.0), n, 1.5);
    }

    // Blades, in the (y, z) plane and `half` thick along x. The frame maps the
    // extrusion's (x, z) profile onto (y, z) and its y onto -x.
    let lower: [[f32; 2]; 6] = [
        [4.6, 2.0],
        [16.0, 2.0],
        [17.6, 20.0],
        [15.6, 34.0],
        [9.4, 34.0],
        [5.4, 14.0],
    ];
    let upper: [[f32; 2]; 5] = [
        [9.0, 36.6],
        [15.3, 36.6],
        [12.6, 50.0],
        [4.4, 70.0],
        [1.8, 66.0],
    ];
    let across = frame(Vec3::ZERO, Vec3::Y, -Vec3::X, Vec3::Z);
    b.mirror_y(|b| {
        pale(b);
        b.with(across, |b| {
            if b.coarse() {
                // One blade, gap and all.
                let whole = [
                    [4.6, 2.0],
                    [16.0, 2.0],
                    [17.6, 20.0],
                    [4.4, 70.0],
                    [1.8, 66.0],
                ];
                b.extrude_y(&whole, -4.5, 4.5);
                return;
            }
            b.extrude_y_chamfered(&lower, 5.0, 0.9);
            if b.mid() {
                b.extrude_y_chamfered(&upper, 4.0, 0.8);
            } else {
                b.extrude_y(&upper, -4.0, 4.0);
            }
        });
        // Light in the gap between the plates, and down each blade's inner edge.
        light(b);
        if b.mid() {
            b.block(v3(-2.6, 10.0, 34.6), v3(2.6, 15.0, 36.0));
        }
        if b.fine() {
            for x in [-5.0f32, 5.0] {
                let n = v3(x.signum(), 0.0, 0.0);
                seam(b, v3(x, 7.0, 16.0), v3(x, 10.8, 32.0), n, 0.8);
                let x = x * 0.8;
                seam(b, v3(x, 10.6, 39.0), v3(x, 4.4, 63.0), n, 0.8);
            }
        }
    });

    // The gem in the arrowhead.
    if b.coarse() {
        return;
    }
    light(b);
    let gem = ngon(4, 1.0);
    b.loft_z(
        &gem,
        &[
            Section::new(57.0, 0.0),
            Section::new(60.5, 2.4),
            Section::new(65.0, 0.0),
        ],
    );
}

// ---- Arch ------------------------------------------------------------------------
//
// A gateway over a road running along x. Two battered legs at y = ±46 (their inner
// faces plumb at |y| = 36, so the road between is clear at the ground), each with a
// corbel reaching in under the lintel. The lintel is three blocks: one on each leg
// and a centre span floating between them, with a lit bar hanging under it and a
// thin floating beam under that; a keystone chevron floats over the middle.

/// Leg plan at the ground: centre y, half x, half y.
const LEG_Y: f32 = 46.0;
const LEG_HALF: Vec2 = Vec2::new(12.0, 10.0);
const LEG_TOP: f32 = 60.0;
/// Plan scale of a leg at its top, and the top's centre y (inner face stays plumb).
const LEG_TOP_SCALE: Vec2 = Vec2::new(0.78, 0.72);

fn arch(b: &mut MeshBuilder, _tech: u8) {
    let top_y = LEG_Y - LEG_HALF.y * (1.0 - LEG_TOP_SCALE.y);
    b.mirror_y(|b| {
        // Leg.
        pale(b);
        let plan = fine_rect(b, LEG_HALF.x, LEG_HALF.y, 3.0);
        let top = Section::scaled(LEG_TOP, LEG_TOP_SCALE.x, LEG_TOP_SCALE.y).shifted(0.0, top_y);
        if b.coarse() {
            // Its top is under the horn.
            buried_loft(
                b,
                &plan,
                &[Section::new(-24.0, 1.0).shifted(0.0, LEG_Y), top],
                false,
            );
        } else {
            buried_loft(
                b,
                &plan,
                &[
                    Section::new(-24.0, 1.0).shifted(0.0, LEG_Y),
                    Section::new(0.0, 1.0).shifted(0.0, LEG_Y),
                    top,
                ],
                true,
            );
        }
        // Corbel: a dark bracket from the inner face up under the lintel.
        dark(b);
        let corbel: [[f32; 2]; 4] = [[36.2, 34.0], [36.2, LEG_TOP], [27.5, LEG_TOP], [27.5, 56.5]];
        let across = frame(Vec3::ZERO, Vec3::Y, -Vec3::X, Vec3::Z);
        b.with(across, |b| {
            if b.fine() {
                b.extrude_y_chamfered(&corbel, 7.0, 0.8);
            } else if b.mid() {
                b.extrude_y(&corbel, -7.0, 7.0);
            }
        });
        // A line of light up the inner face, and up the corbel's underside.
        let n = -Vec3::Y;
        seam(b, v3(0.0, 35.95, 3.0), v3(0.0, 35.95, 33.0), n, 2.2);
        if b.fine() {
            // The corbel's sloped underside runs from (y 36.2, z 34) to (27.5, 56.5).
            let under = v3(0.0, -22.5, -8.7).normalize();
            seam(b, v3(0.0, 35.33, 36.25), v3(0.0, 28.8, 53.1), under, 1.6);
        }
        // Plates on the leg's outer face and its two ends.
        if b.mid() {
            pale(b);
            let courses: &[(f32, f32)] = if b.fine() {
                &[(0.0, 18.0), (20.0, 38.0), (40.0, 57.0)]
            } else {
                &[(0.0, 57.0)]
            };
            let half_x = |z: f32| LEG_HALF.x * (1.0 + (LEG_TOP_SCALE.x - 1.0) * z / LEG_TOP);
            let half_y = |z: f32| LEG_HALF.y * (1.0 + (LEG_TOP_SCALE.y - 1.0) * z / LEG_TOP);
            let mid_y = |z: f32| LEG_Y + (top_y - LEG_Y) * z / LEG_TOP;
            let chamfer_x = |z: f32| 3.0 * (1.0 + (LEG_TOP_SCALE.x - 1.0) * z / LEG_TOP);
            let chamfer_y = |z: f32| 3.0 * (1.0 + (LEG_TOP_SCALE.y - 1.0) * z / LEG_TOP);
            // Outer face (+y): lies on y = mid + half_y.
            let outer = |x: f32, z: f32| v3(x, mid_y(z) + half_y(z), z);
            let lean = (half_y(LEG_TOP) + mid_y(LEG_TOP) - half_y(0.0) - mid_y(0.0)) / LEG_TOP;
            let n_outer = v3(0.0, 1.0, -lean).normalize();
            for &(z0, z1) in courses {
                let w = |z: f32| half_x(z) - chamfer_x(z) - 0.3;
                let quad = [
                    outer(-w(z0), z0),
                    outer(w(z0), z0),
                    outer(w(z1), z1),
                    outer(-w(z1), z1),
                ];
                panel(b, &quad, n_outer, 1.1, 0.6);
            }
            // Ends (±x): lie on x = ±half_x.
            for x in [-1.0f32, 1.0] {
                let end = |y: f32, z: f32| v3(x * half_x(z), y, z);
                let n_end = v3(x, 0.0, -(half_x(LEG_TOP) - half_x(0.0)) / LEG_TOP).normalize();
                for &(z0, z1) in courses {
                    let lo = |z: f32| mid_y(z) - half_y(z) + chamfer_y(z) + 0.3;
                    let hi = |z: f32| mid_y(z) + half_y(z) - chamfer_y(z) - 0.3;
                    let quad = [
                        end(lo(z0), z0),
                        end(hi(z0), z0),
                        end(hi(z1), z1),
                        end(lo(z1), z1),
                    ];
                    panel(b, &quad, n_end, 1.1, 0.6);
                }
            }
        }
        // The lintel's end block, riding the leg and sweeping up and out into a horn,
        // a line of light along its flanks.
        pale(b);
        let horn: [[f32; 2]; 5] = [
            [30.0, LEG_TOP],
            [50.0, LEG_TOP],
            [68.0, 84.0],
            [62.0, 85.5],
            [30.0, 76.0],
        ];
        let across = frame(Vec3::ZERO, Vec3::Y, -Vec3::X, Vec3::Z);
        b.with(across, |b| {
            if b.fine() {
                b.extrude_y_chamfered(&horn, 10.5, 1.2);
            } else if b.mid() {
                b.extrude_y(&horn, -10.5, 10.5);
            } else {
                let horn = [horn[0], horn[1], horn[2], horn[4]];
                b.extrude_y(&horn, -10.5, 10.5);
            }
        });
        if b.mid() {
            for x in [-10.5f32, 10.5] {
                seam(
                    b,
                    v3(x, 33.0, 64.5),
                    v3(x, 60.0, 80.5),
                    v3(x.signum(), 0.0, 0.0),
                    1.4,
                );
            }
        }
    });

    // The centre span, floating between the end blocks.
    pale(b);
    let span: [[f32; 2]; 6] = [
        [-10.0, 64.0],
        [10.0, 64.0],
        [10.0, 75.0],
        [7.5, 78.0],
        [-7.5, 78.0],
        [-10.0, 75.0],
    ];
    let span = if b.coarse() {
        vec![[-10.0, 64.0], [10.0, 64.0], [10.0, 78.0], [-10.0, 78.0]]
    } else {
        span.to_vec()
    };
    b.extrude_y_chamfered(&span, 27.0, 1.4);
    // The lit bar under it, and the floating beam under that.
    light(b);
    if b.coarse() {
        let under = [
            v3(-2.2, -24.0, 63.9),
            v3(2.2, -24.0, 63.9),
            v3(2.2, 24.0, 63.9),
            v3(-2.2, 24.0, 63.9),
        ];
        film(b, &under, -Vec3::Z, 0.0);
    } else {
        b.block(v3(-2.2, -24.0, 61.8), v3(2.2, 24.0, 63.2));
    }
    if b.mid() {
        pale(b);
        let across = frame(Vec3::ZERO, Vec3::Y, -Vec3::X, Vec3::Z);
        // Its ends are cut to points: a hexagon in the (y, z) plane, 12 m thick.
        let points: [[f32; 2]; 6] = [
            [-22.0, 59.0],
            [-18.0, 57.0],
            [18.0, 57.0],
            [22.0, 59.0],
            [18.0, 60.8],
            [-18.0, 60.8],
        ];
        b.with(across, |b| b.extrude_y_chamfered(&points, 6.0, 0.6));
    }
    // Keystone chevron over the middle.
    if b.mid() {
        pale(b);
        let key: [[f32; 2]; 5] = [
            [-9.0, 85.0],
            [0.0, 80.5],
            [9.0, 85.0],
            [9.0, 87.0],
            [-9.0, 87.0],
        ];
        let across = frame(Vec3::ZERO, Vec3::Y, -Vec3::X, Vec3::Z);
        b.with(across, |b| b.extrude_y_chamfered(&key, 6.0, 0.7));
        light(b);
        let n = Vec3::Z;
        seam(b, v3(0.0, -14.0, 78.0), v3(0.0, 14.0, 78.0), n, 1.4);
    }
}

// ---- Ring ------------------------------------------------------------------------
//
// A colossal ring standing upright across the prop's heading (axis along x), its
// centre 40 m up so a quarter of it is under the ground. Outer radius 110, a 16 m
// radial section 20 m thick, built of separate segments with dark gaps between. Its
// inner face carries a channel of light that runs on across the gaps; key blocks
// float off the outer face; the feet sink into two bevelled footing blocks.

const RING_Z: f32 = 40.0;
const RING_OUTER: f32 = 110.0;
const RING_INNER: f32 = 94.0;
const RING_HALF: f32 = 10.0;

/// The ring's cross-section point (radius, x) at angle `a` from straight up, turning toward +y.
fn ring_point(p: [f32; 2], a: f32) -> Vec3 {
    v3(p[1], p[0] * a.sin(), RING_Z + p[0] * a.cos())
}

fn ring(b: &mut MeshBuilder, _tech: u8) {
    // Segments stop where they would be wholly under the ground.
    let reach = (-RING_Z / RING_OUTER).acos() + 0.06;
    let (o, i, h) = (RING_OUTER, RING_INNER, RING_HALF);
    let (segments, steps): (usize, usize) = match b.lod() {
        0 => (11, 3),
        1 => (11, 1),
        _ => (1, 8),
    };
    let section: Vec<[f32; 2]> = match b.lod() {
        // Outer face bevelled at both edges; the inner face stepped in to a channel.
        0 => vec![
            [o - 2.0, -h],
            [o, -h + 2.0],
            [o, h - 2.0],
            [o - 2.0, h],
            [i + 1.5, h],
            [i, h - 1.5],
            [i, 3.4],
            [i + 2.2, 2.4],
            [i + 2.2, -2.4],
            [i, -3.4],
            [i, -h + 1.5],
            [i + 1.5, -h],
        ],
        1 => vec![[o, -h], [o, h], [i, h], [i, -h]],
        // Far off, a ridge toward the eye is as good as a face.
        _ => vec![[o, -h], [o, h], [i, 0.0]],
    };
    // Each segment a great pale plate, with a dark joint collar between it and the
    // next: narrower and thinner, set back, so the gaps read from far off.
    let pitch = 2.0 * reach / segments as f32;
    let joint = if b.coarse() { 0.0 } else { pitch * 0.13 };
    for k in 0..segments {
        let a0 = -reach + pitch * k as f32;
        pale(b);
        sweep(
            b,
            &section,
            a0 + joint * 0.5,
            a0 + pitch - joint * 0.5,
            steps,
            &ring_point,
        );
        if k > 0 && b.mid() {
            dark(b);
            let collar = [
                [o - 1.8, -h + 2.0],
                [o - 1.8, h - 2.0],
                [i + 2.4, h - 2.0],
                [i + 2.4, -h + 2.0],
            ];
            sweep(
                b,
                &collar,
                a0 - joint * 0.5 - 0.004,
                a0 + joint * 0.5 + 0.004,
                1,
                &ring_point,
            );
        }
    }
    // The channel of light round the inside, whole across the joints.
    if !b.coarse() {
        light(b);
        let (arcs, steps, channel): (usize, usize, Vec<[f32; 2]>) = if b.fine() {
            // Down in the groove.
            (
                4,
                8,
                vec![
                    [i + 2.1, -2.2],
                    [i + 2.1, 2.2],
                    [i + 1.4, 2.2],
                    [i + 1.4, -2.2],
                ],
            )
        } else {
            // Standing just proud of the plain inner face.
            (
                4,
                5,
                vec![
                    [i + 0.5, -2.6],
                    [i + 0.5, 2.6],
                    [i - 0.25, 2.6],
                    [i - 0.25, -2.6],
                ],
            )
        };
        let arc = 2.0 * reach / arcs as f32;
        for k in 0..arcs {
            let a0 = -reach + arc * k as f32;
            sweep(b, &channel, a0, a0 + arc, steps, &ring_point);
        }
    }

    // Key blocks floating off the outer face over three segments, each a wedge
    // pointing out, with light in the gap under it.
    if b.mid() {
        for &k in &[3usize, 5, 7] {
            let a = -reach + pitch * (k as f32 + 0.5);
            let half = if k == 5 { 0.075 } else { 0.06 };
            pale(b);
            let key = [
                [o + 3.0, -8.0],
                [o + 13.0, -3.0],
                [o + 13.0, 3.0],
                [o + 3.0, 8.0],
            ];
            sweep(b, &key, a - half, a + half, 1, &ring_point);
            light(b);
            let bar = [
                [o + 0.2, -2.4],
                [o + 2.8, -2.4],
                [o + 2.8, 2.4],
                [o + 0.2, 2.4],
            ];
            sweep(b, &bar, a - half * 0.8, a + half * 0.8, 1, &ring_point);
        }
    }

    // Footings where the ring meets the ground: bevelled blocks it sinks into.
    if b.coarse() {
        return;
    }
    let ground = (RING_INNER * RING_INNER - RING_Z * RING_Z).sqrt() - 5.0;
    let far = (RING_OUTER * RING_OUTER - RING_Z * RING_Z).sqrt() + 8.0;
    b.mirror_y(|b| {
        dark(b);
        let (y0, y1) = (ground, far);
        let lo = [
            v3(-10.0, y0, -12.0),
            v3(10.0, y0, -12.0),
            v3(10.0, y1, -12.0),
            v3(-10.0, y1, -12.0),
        ];
        let hi = [
            v3(-8.0, y0 + 4.0, 9.0),
            v3(8.0, y0 + 4.0, 9.0),
            v3(8.0, y1 - 6.0, 9.0),
            v3(-8.0, y1 - 6.0, 9.0),
        ];
        b.loft(&[lo.to_vec(), hi.to_vec()], true, true);
        if b.fine() {
            let lit = [
                v3(-8.0, y0 + 5.0, 9.0),
                v3(8.0, y0 + 5.0, 9.0),
                v3(8.0, y0 + 7.5, 9.0),
                v3(-8.0, y0 + 7.5, 9.0),
            ];
            light(b);
            film(b, &lit, Vec3::Z, 0.06);
        }
    });
}

// ---- Shard -----------------------------------------------------------------------
//
// A length of some greater structure, fallen and half buried, its raised end broken
// off. The body is a long blade-section prism, widening toward the break: a dark
// core clad in pale plates on every facet, cut into lengths with gaps between. At
// the break the plates end ragged, each on its own slanted cut, and the core runs
// on past them to a steep cut with its lit heart showing. The light lines along the
// ridges fail toward the break.

/// Cross-section (y, z) of the shard's plating at the break: a keeled blade.
const SHARD_SECTION: [[f32; 2]; 8] = [
    [0.0, 11.0],
    [8.0, 8.5],
    [14.0, 1.0],
    [10.0, -7.0],
    [0.0, -9.5],
    [-10.0, -7.0],
    [-14.0, 1.0],
    [-8.0, 8.5],
];
const SHARD_LOW: f32 = -50.0;
const SHARD_HIGH: f32 = 50.0;
/// Where each facet's plating ends at the break, and how its cut slants (dx per m
/// of y and of z).
const SHARD_BREAK: [(f32, f32, f32); 8] = [
    (44.0, 0.2, -0.5),
    (31.0, -0.3, 0.6),
    (46.0, 0.1, 0.5),
    (27.0, 0.5, -0.3),
    (38.0, -0.4, 0.4),
    (42.0, 0.3, 0.3),
    (24.0, -0.2, -0.6),
    (35.0, 0.6, 0.2),
];
/// The core's cut: steep, so the broken end faces up and out.
const SHARD_CORE_END: (f32, f32, f32) = (48.0, 0.15, 0.9);

/// The shard's section scale at `x` along it: narrower at the buried end.
fn shard_taper(x: f32) -> f32 {
    0.72 + 0.28 * (x - SHARD_LOW) / (SHARD_HIGH - SHARD_LOW)
}

fn shard(b: &mut MeshBuilder, _tech: u8) {
    let tilt = 24f32.to_radians();
    let place = Affine3A::from_translation(v3(2.0, 0.0, 12.0))
        * Affine3A::from_rotation_y(-tilt)
        * Affine3A::from_rotation_x(0.17);
    b.with(place, shard_body);

    // Pieces of it lying under the break.
    if b.mid() {
        pale(b);
        for (k, &(x, y, yaw, size)) in [
            (34.0f32, 11.0f32, 0.4f32, 6.0f32),
            (41.0, -11.0, -0.7, 4.5),
            (27.0, -8.0, 1.2, 3.5),
        ]
        .iter()
        .enumerate()
        {
            let lean = Affine3A::from_translation(v3(x, y, 0.0))
                * Affine3A::from_rotation_z(yaw)
                * Affine3A::from_rotation_x(0.25 + 0.1 * k as f32);
            b.with(lean, |b| {
                let plan = cut_rect(b, size, size * 0.6, size * 0.25);
                b.loft_z(
                    &plan,
                    &[Section::new(-2.5, 1.0), Section::new(size * 0.55, 0.8)],
                );
            });
        }
    }
}

/// The shard in its own frame: axis along x, the break at +x.
fn shard_body(b: &mut MeshBuilder) {
    let section = |k: usize| Vec2::from(SHARD_SECTION[k % 8]);
    // A section point `p` placed at `x`, at the section scale for `nominal` (one per
    // ring, so a slanted end stays a flat face).
    let at = |x: f32, p: Vec2, nominal: f32| {
        let q = p * shard_taper(nominal);
        v3(x, q.x, q.y)
    };
    // The broken end's cut, on a slant: x as a function of (y, z).
    let cut = |(end, dy, dz): (f32, f32, f32), p: Vec2| end + dy * p.x + dz * p.y;
    let core = |scale: f32, end: bool| -> Vec<Vec3> {
        (0..8)
            .map(|k| {
                let p = section(k) * scale;
                if end {
                    at(cut(SHARD_CORE_END, p), p, SHARD_CORE_END.0)
                } else {
                    at(SHARD_LOW, p, SHARD_LOW)
                }
            })
            .collect()
    };
    if b.coarse() {
        pale(b);
        b.loft(&[core(1.0, false), core(1.0, true)], true, true);
        light(b);
        let p = section(0) * 1.01;
        let ridge = [
            at(-40.0, p - Vec2::X * 1.2, -40.0),
            at(20.0, p - Vec2::X * 1.2, 20.0),
            at(20.0, p + Vec2::X * 1.2, 20.0),
            at(-40.0, p + Vec2::X * 1.2, -40.0),
        ];
        film(b, &ridge, Vec3::Z, 0.0);
        return;
    }

    // Core, dark, running past the plating to its cut.
    dark(b);
    b.loft(&[core(0.84, false), core(0.84, true)], true, true);
    // Its lit heart, showing at the break.
    light(b);
    let heart = ngon(if b.fine() { 6 } else { 4 }, 3.0);
    let heart_ring = |x: f32| -> Vec<Vec3> {
        heart
            .iter()
            .map(|p| at(x, Vec2::new(p[0], p[1]), x))
            .collect()
    };
    b.loft(&[heart_ring(36.0), heart_ring(51.0)], false, true);

    // Plating: every facet in lengths, its edges drawn back so each facet's plates
    // stand apart from the next.
    pale(b);
    let lengths: &[(f32, f32)] = if b.fine() {
        &[
            (SHARD_LOW, -27.0),
            (-25.5, -4.0),
            (-2.5, 18.0),
            (19.5, 60.0),
        ]
    } else {
        &[(SHARD_LOW, -4.0), (-2.5, 60.0)]
    };
    for (k, &broken) in SHARD_BREAK.iter().enumerate() {
        let (a, c) = (section(k), section(k + 1));
        let along = (c - a).normalize();
        let edge = 0.45;
        // The plate's cross-section: its back on the core, its face on the outline.
        let plate = [
            a * 0.84 + along * edge,
            c * 0.84 - along * edge,
            c - along * edge * 2.2,
            a + along * edge * 2.2,
        ];
        for &(x0, x1) in lengths {
            let last = x1 > SHARD_HIGH;
            if last && x0 >= broken.0 - 3.0 {
                continue;
            }
            let bevel = if b.fine() { 0.9 } else { 0.0 };
            // A ring of the plate at `x` (or at its cut, `back` short of it), its front
            // edge (points 2, 3) pulled in along the facet by `shrink`.
            let ring = |x: Option<f32>, shrink: f32, back: f32| -> Vec<Vec3> {
                let mut q = plate;
                q[2] -= along * shrink;
                q[3] += along * shrink;
                let nominal = x.unwrap_or(broken.0) - back;
                q.iter()
                    .map(|&p| at(x.unwrap_or_else(|| cut(broken, p)) - back, p, nominal))
                    .collect()
            };
            let end = if last { None } else { Some(x1) };
            if bevel > 0.0 {
                b.loft(
                    &[
                        ring(Some(x0), bevel, 0.0),
                        ring(Some(x0 + bevel), 0.0, 0.0),
                        ring(end, 0.0, bevel),
                        ring(end, bevel, 0.0),
                    ],
                    true,
                    true,
                );
            } else {
                b.loft(&[ring(Some(x0), 0.0, 0.0), ring(end, 0.0, 0.0)], true, true);
            }
        }
    }

    // Light along the ridges between facets, in dashes that fail toward the break.
    let dashes: &[(f32, f32)] = if b.fine() {
        &[
            (-46.0, -30.0),
            (-24.0, -8.0),
            (-2.0, 9.0),
            (14.0, 19.0),
            (23.0, 25.0),
        ]
    } else {
        &[(-46.0, -8.0), (-2.0, 19.0)]
    };
    for &ridge in &[0usize, 2, 6] {
        let p = section(ridge) * 0.95;
        let n = Vec3::new(0.0, p.x, p.y).normalize();
        for &(x0, x1) in dashes {
            seam(b, at(x0, p, x0), at(x1, p, x1), n, 1.3);
        }
    }
}

// ---- Wall ------------------------------------------------------------------------
//
// A revetment along a terrace edge: 80 m along y, its face at x = 0 looking +x, the
// body back to x = -16 and down to -24. The face is battered in two courses with a
// recessed groove between them carrying a line of light, and a coping lip on top.
// Plates are laid in 10 m bays, staggered between courses; two buttresses rise
// through the coping. Every piece stops short of the ends, so walls laid end to end
// read as one: the plate joints and buttresses keep their rhythm across the join.
// ~100-200 per map: the full level stays under 600 triangles.

const WALL_HALF: f32 = 40.0;

fn wall(b: &mut MeshBuilder, _tech: u8) {
    // Body profile (x, z): toe at the face, lower course battered back to the groove,
    // the groove, the upper course, the coping lip, the top back to x = -16.
    let body: Vec<[f32; 2]> = if b.coarse() {
        vec![
            [-16.0, -24.0],
            [0.0, -24.0],
            [0.0, 0.0],
            [-3.2, 18.0],
            [-16.0, 18.0],
        ]
    } else {
        vec![
            [-16.0, -24.0],
            [-0.6, -24.0],
            [-0.6, 0.0],
            [-2.0, 7.0],
            [-2.9, 7.0],
            [-2.9, 8.8],
            [-2.2, 8.8],
            [-3.6, 15.8],
            [-1.4, 16.3],
            [-1.4, 17.2],
            [-2.2, 18.0],
            [-16.0, 18.0],
        ]
    };
    pale(b);
    b.extrude_y(&body, -WALL_HALF, WALL_HALF);

    // The lit groove.
    light(b);
    let x = if b.coarse() { -1.35 } else { -2.75 };
    let z = if b.coarse() { 7.5 } else { 7.2 };
    film(
        b,
        &[
            v3(x, -WALL_HALF, z),
            v3(x, WALL_HALF, z),
            v3(x, WALL_HALF, z + 1.4),
            v3(x, -WALL_HALF, z + 1.4),
        ],
        Vec3::X,
        0.0,
    );
    if b.coarse() {
        return;
    }

    // Plates on the two courses: the lower from (-0.6, -2) to (-2, 7), the upper from
    // (-2.2, 8.8) to (-3.6, 15.8), staggered by half a bay.
    let course = |z0: f32, x0: f32, z1: f32, x1: f32| {
        move |y: f32, z: f32| v3(x0 + (x1 - x0) * (z - z0) / (z1 - z0), y, z)
    };
    let lower = course(0.0, -0.6, 7.0, -2.0);
    let upper = course(8.8, -2.2, 15.8, -3.6);
    let n_lower = v3(7.0, 0.0, 1.4).normalize();
    let n_upper = v3(7.0, 0.0, 1.4).normalize();
    pale(b);
    let joint = 0.45;
    let (bay, offset_lower, offset_upper) = if b.fine() {
        (10.0, 0.0, 5.0)
    } else {
        (20.0, 0.0, 10.0)
    };
    let bays = |offset: f32| -> Vec<(f32, f32)> {
        let mut out = Vec::new();
        let mut y = -WALL_HALF + offset - bay;
        while y < WALL_HALF {
            let (y0, y1) = (
                (y + joint).max(-WALL_HALF + joint),
                (y + bay - joint).min(WALL_HALF - joint),
            );
            if y1 - y0 > 1.0 {
                out.push((y0, y1));
            }
            y += bay;
        }
        out
    };
    if b.fine() {
        for (y0, y1) in bays(offset_lower) {
            let quad = [
                lower(y0, -2.0),
                lower(y1, -2.0),
                lower(y1, 6.6),
                lower(y0, 6.6),
            ];
            panel(b, &quad, n_lower, 0.55, 0.35);
        }
    }
    for (y0, y1) in bays(offset_upper) {
        let quad = [
            upper(y0, 9.2),
            upper(y1, 9.2),
            upper(y1, 15.5),
            upper(y0, 15.5),
        ];
        panel(b, &quad, n_upper, 0.55, 0.35);
    }

    // Buttresses at y = ±20, rising through the coping with a lit slit up their face.
    for y in [-20.0f32, 20.0] {
        pale(b);
        let plan = if b.fine() {
            chamfered_rect(Vec2::new(2.1, 2.3), 0.8)
        } else {
            rect(2.1, 2.3)
        };
        let sections = if b.fine() {
            vec![
                Section::new(-2.0, 1.0).shifted(-2.1, y),
                Section::new(19.0, 1.0).shifted(-3.1, y),
                Section::new(21.5, 0.35).shifted(-3.4, y),
            ]
        } else {
            vec![
                Section::new(-2.0, 1.0).shifted(-2.1, y),
                Section::new(20.5, 0.9).shifted(-3.2, y),
            ]
        };
        b.loft_z(&plan, &sections);
        if b.fine() {
            let n = v3(21.0, 0.0, 1.0).normalize();
            seam(b, v3(-0.05, y, 1.0), v3(-0.95, y, 18.0), n, 0.5);
        }
    }
}

// ---- Beacon ----------------------------------------------------------------------
//
// An octagonal plinth, a housing of four blades round a column of light, and a crown
// of six arcs hovering over it with their inner rims lit (the Tech III engineer's
// C-arcs), under a floating cap. The column runs from the plinth up through the
// crown to the cap.

const CROWN_Z: f32 = 42.0;

fn beacon(b: &mut MeshBuilder, _tech: u8) {
    let sides = if b.coarse() { 4 } else { 8 };
    let oct = ngon(sides, 1.0);
    pale(b);
    if b.coarse() {
        // No bottom: under the ground.
        let ring = |z: f32, r: f32| -> Vec<Vec3> {
            oct.iter().map(|p| v3(p[0] * r, p[1] * r, z)).collect()
        };
        b.loft(&[ring(-20.0, 16.0), ring(8.0, 14.0)], false, true);
    } else {
        b.loft_z(
            &oct,
            &[
                Section::new(-20.0, 16.0),
                Section::new(2.0, 16.0),
                Section::new(4.0, 14.4),
            ],
        );
        b.loft_z(&oct, &[Section::new(3.9, 12.2), Section::new(8.0, 11.0)]);
    }
    if b.mid() {
        // A course of light round the step.
        light(b);
        b.loft_z(&oct, &[Section::new(5.2, 11.85), Section::new(6.0, 11.7)]);
    }

    // The column: one of the few lights a Precursor site keeps burning.
    key_light(b);
    let column = ngon(
        match b.lod() {
            0 => 6,
            1 => 4,
            _ => 3,
        },
        2.1,
    );
    if b.coarse() {
        // Its foot is buried in the plinth; far off the column is the tip.
        let ring = |z: f32| -> Vec<Vec3> { column.iter().map(|p| v3(p[0], p[1], z)).collect() };
        b.loft(&[ring(7.5), ring(56.0)], false, true);
    } else {
        b.loft_z(&column, &[Section::new(7.5, 1.0), Section::new(54.0, 1.0)]);
    }

    // The housing: four blades round the column's foot.
    if !b.coarse() {
        let blade: [[f32; 2]; 6] = [
            [2.8, 7.5],
            [9.5, 7.5],
            [9.5, 11.5],
            [5.2, 29.0],
            [3.3, 33.0],
            [2.8, 28.0],
        ];
        b.radial(4, |b| {
            b.with(spoke(FRAC_PI_4), |b| {
                pale(b);
                b.extrude_y_chamfered(&blade, 1.9, 0.6);
            });
        });
    }

    // The crown: six arcs, each drooping outward, rims lit toward the column.
    let arcs = if b.coarse() { 3 } else { 6 };
    let span = TAU / arcs as f32;
    let (steps, section): (usize, Vec<[f32; 2]>) = match b.lod() {
        0 => (
            4,
            vec![
                [12.4, -1.6],
                [13.4, -2.6],
                [20.0, -1.6],
                [20.6, 0.4],
                [19.4, 1.8],
                [13.4, 2.4],
            ],
        ),
        1 => (
            2,
            vec![[12.4, -2.4], [20.4, -1.4], [20.4, 1.4], [12.4, 2.4]],
        ),
        _ => (2, vec![[12.4, -1.8], [20.4, -1.0], [16.4, 1.8]]),
    };
    let place = |p: [f32; 2], a: f32| {
        v3(
            p[0] * a.cos(),
            p[0] * a.sin(),
            CROWN_Z + p[1] - (p[0] - 12.0) * 0.28,
        )
    };
    let crown = |b: &mut MeshBuilder| {
        for k in 0..arcs {
            let a0 = span * k as f32 + span * 0.14;
            let a1 = span * (k + 1) as f32 - span * 0.14;
            pale(b);
            sweep(b, &section, a0, a1, steps, &place);
            if b.mid() {
                light(b);
                let rim = [[11.9, -1.1], [12.5, -1.1], [12.5, 1.1], [11.9, 1.1]];
                let rim_steps = if b.fine() { steps } else { 1 };
                sweep(b, &rim, a0 + 0.04, a1 - 0.04, rim_steps, &place);
            }
        }
    };
    if CROWN_TURNS {
        b.with_part(part::SPINNER, crown);
    } else {
        crown(b);
    }

    // The floating cap over the column.
    pale(b);
    if !b.coarse() {
        b.loft_z(
            &ngon(if b.fine() { 6 } else { 4 }, 1.0),
            &[
                Section::new(55.5, 0.4),
                Section::new(58.0, 3.6),
                Section::new(59.2, 3.6),
                Section::new(64.0, 0.0),
            ],
        );
    }
}

// ---- Conduit ---------------------------------------------------------------------
//
// A channel of light let into the ground: 60 m along x, ±5 m across, its curbs 0.4 m
// above the ground and its body down to -3. Curbs in three lengths with joints
// between, dark clamps across the middle of each; everything stops short of the
// ends so conduits laid in lines keep one rhythm.

const CONDUIT_HALF: f32 = 30.0;

fn conduit(b: &mut MeshBuilder, _tech: u8) {
    // Everything that shows stands a little above the ground, or the terrain hides it.
    dark(b);
    b.block(v3(-CONDUIT_HALF, -5.0, -3.0), v3(CONDUIT_HALF, 5.0, 0.1));
    light(b);
    b.block(v3(-CONDUIT_HALF, -1.8, -0.9), v3(CONDUIT_HALF, 1.8, 0.22));

    pale(b);
    let curb: Vec<[f32; 2]> = if b.coarse() {
        vec![[3.2, -0.9], [5.0, -0.9], [5.0, 0.4], [3.2, 0.4]]
    } else {
        vec![
            [3.2, -0.9],
            [5.0, -0.9],
            [5.0, 0.05],
            [4.6, 0.4],
            [3.6, 0.4],
            [3.2, 0.05],
        ]
    };
    let lengths: &[(f32, f32)] = if b.fine() {
        &[(-29.7, -10.3), (-9.7, 9.7), (10.3, 29.7)]
    } else {
        &[(-CONDUIT_HALF, CONDUIT_HALF)]
    };
    b.mirror_y(|b| {
        for &(x0, x1) in lengths {
            b.extrude_x(&curb, x0, x1);
        }
    });
    if b.fine() {
        dark(b);
        for x in [-20.0f32, 0.0, 20.0] {
            b.block(v3(x - 0.9, -4.8, -0.6), v3(x + 0.9, 4.8, 0.32));
        }
    }
}

// ---- Fragment --------------------------------------------------------------------
//
// A broken stub a few metres across with what was above it hanging over it in
// pieces, gaps between them and light in the gaps.

fn fragment(b: &mut MeshBuilder, _tech: u8) {
    // Collar, sunk.
    if !b.coarse() {
        dark(b);
        let collar = fine_rect(b, 5.8, 5.8, 1.6);
        buried_loft(
            b,
            &collar,
            &[Section::new(-4.0, 1.0), Section::new(1.2, 0.95)],
            true,
        );
    }
    // The stub, its top snapped off on a slant.
    pale(b);
    let stub: [[f32; 2]; 6] = [
        [-4.4, -2.0],
        [4.4, -2.0],
        [4.4, 4.5],
        [1.6, 6.4],
        [-1.2, 5.4],
        [-4.4, 7.0],
    ];
    let stub_coarse = [[-4.4, -4.0], [4.4, -4.0], [4.4, 4.5], [-4.4, 7.0]];
    if b.coarse() {
        b.extrude_y(&stub_coarse, -4.2, 4.2);
    } else {
        b.extrude_y_chamfered(&stub, 4.2, 0.6);
    }
    if b.fine() {
        seam(b, v3(4.42, 0.0, 1.5), v3(4.42, 0.0, 3.9), Vec3::X, 0.6);
        seam(b, v3(0.0, -4.22, 1.5), v3(0.0, -4.22, 5.0), -Vec3::Y, 0.6);
    }

    // The pieces overhead, each set a little askew.
    let piece = |b: &mut MeshBuilder, at: Vec3, yaw: f32, roll: f32, size: Vec3, chamfer: f32| {
        let f = Affine3A::from_translation(at)
            * Affine3A::from_rotation_z(yaw)
            * Affine3A::from_rotation_x(roll);
        b.with(f, |b| {
            let plan = fine_rect(b, size.x, size.y, chamfer);
            if b.fine() {
                b.loft_z(
                    &plan,
                    &[
                        Section::new(-size.z, 0.9),
                        Section::new(-size.z + 0.5, 1.0),
                        Section::new(size.z - 0.5, 1.0),
                        Section::new(size.z, 0.9),
                    ],
                );
            } else {
                b.loft_z(
                    &plan,
                    &[Section::new(-size.z, 1.0), Section::new(size.z, 1.0)],
                );
            }
        });
    };
    pale(b);
    piece(b, v3(0.6, 0.2, 10.2), 0.18, 0.13, v3(3.8, 3.6, 1.2), 1.2);
    piece(b, v3(-0.6, 0.4, 14.6), -0.14, -0.16, v3(3.0, 2.8, 1.3), 0.9);
    if b.mid() {
        piece(b, v3(5.2, -3.9, 12.6), 0.5, 0.4, v3(1.0, 1.4, 2.4), 0.3);
    }
    // Cap: a point, hovering.
    let cap = cut_rect(b, 2.6, 2.6, 0.9);
    b.loft_z(
        &cap,
        &[
            Section::new(17.9, 0.75),
            Section::new(18.9, 0.75),
            Section::new(21.0, 0.0),
        ],
    );

    // Light in the gaps.
    if b.coarse() {
        return;
    }
    light(b);
    b.loft_z(
        &ngon(4, 1.0),
        &[
            Section::new(7.3, 0.0),
            Section::new(8.0, 4.8),
            Section::new(8.6, 0.0),
        ],
    );
    if b.mid() {
        b.loft_z(
            &ngon(4, 1.0),
            &[
                Section::new(11.9, 0.0),
                Section::new(12.4, 4.0),
                Section::new(12.9, 0.0),
            ],
        );
        b.loft_z(
            &ngon(4, 1.0),
            &[
                Section::new(16.4, 0.0),
                Section::new(16.8, 3.0),
                Section::new(17.2, 0.0),
            ],
        );
    }
}

#[cfg(test)]
mod checks {
    use glam::Vec3;

    use super::super::library::build_lod;
    use super::super::{build_model, material, preview, MeshLod, LOD_COUNT};
    use super::MODELS;

    fn position(mesh: &MeshLod, index: u32) -> Vec3 {
        Vec3::from(mesh.vertices[index as usize].pos)
    }

    /// The library-wide mesh checks (`tests::meshes_are_valid`, `solids_face_outward`,
    /// `bounds_hold_every_lod`) for these models alone, so they are covered even
    /// while another model trips the library-wide ones first.
    #[test]
    fn precursor_meshes_valid() {
        for def in MODELS {
            let model = build_model(def.key).unwrap();
            for (lod, mesh) in model.lods.iter().enumerate() {
                let name = format!("{} lod{lod}", def.key);
                assert!(
                    !mesh.indices.is_empty() && mesh.indices.len() % 3 == 0,
                    "{name}"
                );
                for v in &mesh.vertices {
                    assert!(
                        (Vec3::from(v.normal).length() - 1.0).abs() < 1e-4,
                        "{name}: normal"
                    );
                    assert!(v.pos[2] >= -80.0, "{name}: too deep");
                    assert!(v.material <= material::LAST, "{name}: material");
                    assert!(
                        Vec3::from(v.pos).length() <= model.bounds_radius + 1e-3,
                        "{name}: bounds"
                    );
                }
                for t in mesh.indices.chunks(3) {
                    let [a, b, c] = [t[0], t[1], t[2]].map(|i| position(mesh, i));
                    let geometric = (b - a).cross(c - a);
                    assert!(geometric.length() * 0.5 > 1e-7, "{name}: degenerate at {a}");
                    for &i in t {
                        let shading = Vec3::from(mesh.vertices[i as usize].normal);
                        assert!(
                            geometric.normalize().dot(shading) > 0.5,
                            "{name}: winding at {a}"
                        );
                    }
                    let m = [t[0], t[1], t[2]].map(|i| mesh.vertices[i as usize].material);
                    assert!(
                        m[0] == m[1] && m[1] == m[2],
                        "{name}: one material per triangle"
                    );
                }
                let builder = build_lod(def.key, lod, 1);
                let mesh = builder.mesh();
                for range in builder.solids() {
                    let volume: f32 = mesh.indices[range.clone()]
                        .chunks(3)
                        .map(|t| {
                            position(mesh, t[0])
                                .dot(position(mesh, t[1]).cross(position(mesh, t[2])))
                        })
                        .sum();
                    assert!(
                        volume > 0.0,
                        "{name}: inside-out solid at {}",
                        position(mesh, mesh.indices[range.start])
                    );
                }
            }
            let [full, mid, _] = [0, 1, 2].map(|lod| triangles(&model.lods[lod]));
            assert!(
                mid as f32 <= full as f32 * 0.45 + 20.0,
                "{}: reduced {mid} of {full}",
                def.key
            );
        }
    }

    fn triangles(mesh: &MeshLod) -> usize {
        mesh.indices.len() / 3
    }

    /// What shows above ground: triangles with any corner above z = -0.5.
    fn above_ground(mesh: &MeshLod) -> MeshLod {
        let indices = mesh
            .indices
            .chunks(3)
            .filter(|t| t.iter().any(|&i| mesh.vertices[i as usize].pos[2] > -0.5))
            .flatten()
            .copied()
            .collect();
        MeshLod {
            vertices: mesh.vertices.clone(),
            indices,
        }
    }

    #[test]
    fn precursor_budgets() {
        let mut failed = Vec::new();
        for def in MODELS {
            let model = build_model(def.key).unwrap();
            let [full, mid, coarse] = [0, 1, 2].map(|lod| triangles(&model.lods[lod]));
            let budget = match def.key {
                "precursor_wall" | "precursor_conduit" => 600,
                "precursor_fragment" => 800,
                _ => 4000,
            };
            println!("{}: {full}/{mid}/{coarse}", def.key);
            if full > budget || coarse >= 60 || mid as f32 > full as f32 * 0.45 + 20.0 {
                failed.push(def.key);
            }
        }
        assert!(failed.is_empty(), "over budget: {failed:?}");
    }

    /// Preview renders of the precursor props, above the ground only:
    /// `PRECURSOR_DUMP_DIR=... cargo test -p mc-models -- --ignored dump_precursor`
    #[test]
    #[ignore = "writes inspection files"]
    fn dump_precursor() {
        let dir = std::path::PathBuf::from(
            std::env::var_os("PRECURSOR_DUMP_DIR").expect("PRECURSOR_DUMP_DIR"),
        );
        std::fs::create_dir_all(&dir).unwrap();
        for def in MODELS {
            let model = build_model(def.key).unwrap();
            for lod in 0..LOD_COUNT {
                let mesh = above_ground(&model.lods[lod]);
                let size = if lod == 0 { 640 } else { 320 };
                for (tag, azimuth) in [("", -38.0), ("_rear", 142.0), ("_side", 90.0)] {
                    if lod > 0 && !tag.is_empty() {
                        continue;
                    }
                    preview::render(&mesh, size, azimuth)
                        .write_ppm(&dir.join(format!("{}_lod{lod}{tag}.ppm", def.key)))
                        .unwrap();
                }
            }
            let counts = [0, 1, 2].map(|lod| triangles(&model.lods[lod]));
            println!("{} {:?} bounds {:.1}", def.key, counts, model.bounds_radius);
        }
    }
}
