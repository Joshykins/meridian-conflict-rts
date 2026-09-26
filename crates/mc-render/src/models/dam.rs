//! The canyon map's landmark: a colossal concrete arch dam across the gorge
//! (`mc_map::PropKind::Dam`), built to the plan the baker cuts the ground for,
//! `mc_map::landmark::DAM`, so the two meet.
//!
//! Frame: x along the prop's heading is upstream (into the lake), y to its
//! left, z up; the origin is on the crest road's centreline at the arch's
//! apex, and z = 0 is the crest's ground, which the bake levels at the crest's
//! height across the road band. The crest centreline is an arc of `radius`
//! about `(-radius, 0)`; a point of the dam is placed by its angle round that
//! arc and its offset from the centreline (upstream positive), as
//! `DamPlan::arch_coords` measures it. The water stands `crest_z` under the
//! crest on both sides (the reservoir drawn down, the tailwater as low).
//!
//! What it carries: the arch itself, near-vertical upstream with a toe under
//! the water, its downstream face thickening with depth; parapets, sidewalks,
//! lamp posts and a two-lane road on the crest; a thrust block at each end,
//! keyed into the rock along the arc's tangent; four intake towers standing in
//! the lake, bridged to the crest; a gated spillway by each abutment, its
//! chute down the downstream face; penstocks down the face into a powerhouse
//! along the toe at the tailwater; and a lift tower at each end. The concrete's
//! looks (lifts, block joints, streaks, the reservoir's white mineral ring up
//! to the old full pool, the wet band over the water) are scenery.wgsl's
//! `dam_concrete`, chosen by pattern here; the ring's crisp top is an edge of
//! the mesh.

use glam::{Vec2, Vec3};
use mc_map::landmark::DAM;

use super::builder::MeshBuilder;
use super::library::ModelDef;
use super::material::*;
use crate::gpu_consts::scenery::{
    CONCRETE_CAST as CAST, CONCRETE_LINE as LINE, CONCRETE_RING as RING, CONCRETE_ROAD as ROAD,
    CONCRETE_WET as WET,
};

pub(super) const MODELS: &[ModelDef] = &[ModelDef::new("landmark_dam", 200.0, 150.0, dam)];

/// Full-detail triangle budget: one dam on one map, 400 m across.
#[cfg(test)]
pub(super) const TRIANGLES: usize = 11000;
/// The deepest the model reaches: its footing, sunk under the gorge's bed.
pub(super) const FLOOR: f32 = -(CREST_Z + BED + DAM.footing as f32);

const R: f32 = DAM.radius as f32;
const HALF_ANGLE: f32 = DAM.half_angle as f32;
const CREST_HALF: f32 = DAM.crest_half as f32;
const CREST_Z: f32 = DAM.crest_z as f32;
const BED: f32 = DAM.bed as f32;

/// The water, on both sides.
const WATER: f32 = -CREST_Z;
/// The old full pool: the top of the white ring, as the canyon's walls have it.
const RING_TOP: f32 = WATER + DAM.ring_top as f32;
/// The dark wet band over the waterline.
const WET_TOP: f32 = WATER + 1.5;
/// The road, a hair over the ground the bake levelled under it.
const DECK: f32 = 0.3;
/// Sidewalks and the tops of the parapets.
const WALK: f32 = 0.55;
const PARAPET: f32 = 1.6;
/// The parapets' inner faces and the road's kerbs, as offsets from the
/// centreline (upstream positive). The road's middle carries a double line.
const PARAPET_IN: f32 = CREST_HALF - 0.6;
const KERB_UP: f32 = 8.0;
const KERB_DOWN: f32 = -9.0;
const ROAD_MIDDLE: f32 = (KERB_UP + KERB_DOWN) * 0.5;

/// Intake towers: (angle round the arch, offset upstream of the crest's centreline).
const TOWERS: [(f32, f32); 4] = [(-0.47, 45.0), (-0.21, 41.0), (0.21, 41.0), (0.47, 45.0)];
const TOWER_R: f32 = 8.5;
/// The spillways' angle in from each end, and the gates' half width along the arc.
const SPILLWAY_IN: f32 = 0.11;
const SPILLWAY_HALF: f32 = 20.0;
/// The powerhouse along the toe: its angle either side of the apex, its roof, and
/// its downstream wall.
const POWERHOUSE_ANGLE: f32 = 0.3;
const POWERHOUSE_ROOF: f32 = WATER + 22.0;
const POWERHOUSE_OUT: f32 = -60.0;
/// Penstocks down the downstream face into the powerhouse's roof.
const PENSTOCKS: [f32; 4] = [-0.22, -0.08, 0.08, 0.22];
const PENSTOCK_R: f32 = 2.2;
/// How far the thrust blocks carry the crest past each end, along the tangent.
const THRUST_RUN: f32 = 22.0;

/// Offset of the upstream face at height `z` (`DamPlan::upstream_face`): a slight
/// batter, flaring into a toe from just over the waterline (it hides where the
/// gorge's ground, cut on an 8 m grid, leans out past a sheer face).
fn up_face(z: f32) -> f32 {
    DAM.upstream_face(-z as f64) as f32
}

/// Offset of the downstream face at height `z` (`DamPlan::downstream_face`):
/// plumb at the crest, battering out ever more with depth.
fn down_face(z: f32) -> f32 {
    DAM.downstream_face(-z as f64) as f32
}

/// The point at angle `a` round the arch, `o` upstream of the crest's centreline,
/// at height `z`.
fn arch(a: f32, o: f32, z: f32) -> Vec3 {
    Vec3::new((R + o) * a.cos() - R, (R + o) * a.sin(), z)
}

/// The point `y` metres along the arch (to the left) from angle `a`, `o` upstream
/// of the centreline: on the curved faces themselves, not a plane tangent to them.
fn arch_y(a: f32, y: f32, o: f32, z: f32) -> Vec3 {
    arch(a + y / (R + o), o, z)
}

fn radial(a: f32) -> Vec3 {
    Vec3::new(a.cos(), a.sin(), 0.0)
}

/// One corner of a cross-section: offset upstream of the centreline, height, and
/// the pattern of the face from it to the next corner.
#[derive(Clone, Copy)]
struct Pt {
    o: f32,
    z: f32,
    pattern: u32,
}

const fn pt(o: f32, z: f32, pattern: u32) -> Pt {
    Pt { o, z, pattern }
}

/// A place a cross-section is swept through: its centreline point and the way
/// its offsets point (upstream).
#[derive(Clone, Copy)]
struct Station {
    at: Vec3,
    out: Vec3,
}

fn arc_stations(a0: f32, a1: f32, blocks: usize) -> Vec<Station> {
    (0..=blocks)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / blocks as f32;
            Station {
                at: arch(a, 0.0, 0.0),
                out: radial(a),
            }
        })
        .collect()
}

/// Sweeps the open polyline `section` through `stations` in concrete: one face per
/// edge per block, so each block's faces end at its joints. The section runs
/// round the dam counter-clockwise seen with upstream to the right (up the
/// upstream side, across the top, down the downstream side), so every face's
/// outside is to the edge's right.
fn sweep(b: &mut MeshBuilder, stations: &[Station], section: &[Pt]) {
    for pair in stations.windows(2) {
        let (s, e) = (pair[0], pair[1]);
        let out = (s.out + e.out).normalize();
        for edge in section.windows(2) {
            let (p, q) = (edge[0], edge[1]);
            let normal = out * (q.z - p.z) + Vec3::Z * (p.o - q.o);
            let place = |st: Station, c: Pt| st.at + st.out * c.o + Vec3::Z * c.z;
            let mut quad = [place(s, p), place(s, q), place(e, q), place(e, p)];
            if (quad[1] - quad[0]).cross(quad[2] - quad[0]).dot(normal) < 0.0 {
                quad.reverse();
            }
            b.paint(CONCRETE).pattern(p.pattern);
            b.face(&quad);
        }
    }
}

/// A flat polygon `outline` (offset, height) standing at `st`, facing `facing`.
fn cap(b: &mut MeshBuilder, st: Station, outline: &[[f32; 2]], facing: Vec3, pattern: u32) {
    let mut points: Vec<Vec3> = outline
        .iter()
        .map(|&[o, z]| st.at + st.out * o + Vec3::Z * z)
        .collect();
    let n = (1..points.len() - 1)
        .map(|i| (points[i] - points[0]).cross(points[i + 1] - points[0]))
        .sum::<Vec3>();
    if n.dot(facing) < 0.0 {
        points.reverse();
    }
    b.paint(CONCRETE).pattern(pattern);
    b.face(&points);
}

/// The crest from the upstream parapet's outer face over to the downstream one's:
/// parapets, sidewalks, kerbs, and the road with a double line down its middle.
fn crest_top(fine: bool) -> Vec<Pt> {
    let mut top = vec![
        pt(CREST_HALF, DECK, CAST),
        pt(CREST_HALF, PARAPET, CAST),
        pt(PARAPET_IN, PARAPET, CAST),
        pt(PARAPET_IN, WALK, CAST),
        pt(KERB_UP, WALK, CAST),
        pt(KERB_UP, DECK, ROAD),
    ];
    if fine {
        for (o, pattern) in [
            (ROAD_MIDDLE + 0.3, LINE),
            (ROAD_MIDDLE + 0.12, ROAD),
            (ROAD_MIDDLE - 0.12, LINE),
            (ROAD_MIDDLE - 0.3, ROAD),
        ] {
            top.push(pt(o, DECK, pattern));
        }
    }
    top.extend([
        pt(KERB_DOWN, DECK, CAST),
        pt(KERB_DOWN, WALK, CAST),
        pt(-PARAPET_IN, WALK, CAST),
        pt(-PARAPET_IN, PARAPET, CAST),
        pt(-CREST_HALF, PARAPET, CAST),
        pt(-CREST_HALF, DECK, CAST),
    ]);
    top
}

/// The arch's cross-section: up the upstream face through the drowned toe, the wet
/// band, the mineral ring and the clean concrete over it; the crest; down the
/// downstream face.
fn arch_section(fine: bool) -> Vec<Pt> {
    let up: &[(f32, u32)] = if fine {
        &[
            (FLOOR, CAST),
            (WATER - 10.0, CAST),
            (WATER - 4.0, CAST),
            (WATER, WET),
            (WET_TOP, RING),
            (WATER + 7.0, RING),
            (WATER + 30.0, RING),
            (RING_TOP, CAST),
        ]
    } else {
        &[
            (FLOOR, CAST),
            (WATER - 10.0, CAST),
            (WATER, WET),
            (WET_TOP, RING),
            (RING_TOP, CAST),
        ]
    };
    let down: &[f32] = if fine {
        &[-4.0, -12.0, -24.0, -40.0, -60.0, -84.0, -110.0, FLOOR]
    } else {
        &[-12.0, -40.0, -84.0, FLOOR]
    };
    let mut section: Vec<Pt> = up.iter().map(|&(z, p)| pt(up_face(z), z, p)).collect();
    section.extend(crest_top(fine));
    section.extend(down.iter().map(|&z| pt(down_face(z), z, CAST)));
    section
}

fn dam(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        // The arch as one solid in six pieces.
        let rings: Vec<Vec<Vec3>> = arc_stations(-HALF_ANGLE, HALF_ANGLE, 6)
            .into_iter()
            .map(|st| {
                [
                    [up_face(FLOOR), FLOOR],
                    [CREST_HALF, DECK],
                    [-CREST_HALF, DECK],
                    [down_face(FLOOR), FLOOR],
                ]
                .iter()
                .map(|&[o, z]| st.at + st.out * o + Vec3::Z * z)
                .collect()
            })
            .collect();
        b.paint(CONCRETE).pattern(CAST);
        b.loft(&rings, true, true);
        return;
    }
    let fine = b.fine();
    let blocks = if fine { 28 } else { 12 };
    sweep(
        b,
        &arc_stations(-HALF_ANGLE, HALF_ANGLE, blocks),
        &arch_section(fine),
    );
    thrust_blocks(b, fine);
    for &(a, o) in &TOWERS {
        intake_tower(b, a, o, fine);
    }
    for end in [-1.0, 1.0] {
        spillway(b, end * (HALF_ANGLE - SPILLWAY_IN), fine);
    }
    powerhouse(b, fine);
    for &a in &PENSTOCKS {
        penstock(b, a, fine);
    }
    if fine {
        for end in [-1.0f32, 1.0] {
            lift_tower(b, end * (HALF_ANGLE - 0.035));
        }
        lamps(b);
    }
}

/// Each end of the arch carries on along its tangent into the rock as a thrust
/// block: the crest, its road and parapets run on over it to the level ground the
/// bake leaves past the abutment, and its flanks drop into the gorge's walls.
fn thrust_blocks(b: &mut MeshBuilder, fine: bool) {
    let section: Vec<Pt> = [pt(CREST_HALF + 6.0, -50.0, CAST)]
        .into_iter()
        .chain(crest_top(fine))
        .chain([pt(-CREST_HALF - 16.0, -40.0, CAST)])
        .collect();
    for end in [-1.0f32, 1.0] {
        let a = end * HALF_ANGLE;
        let out = radial(a);
        let along = Vec3::new(-a.sin(), a.cos(), 0.0) * end;
        let start = Station {
            at: arch(a, 0.0, 0.0),
            out,
        };
        let far = Station {
            at: start.at + along * THRUST_RUN,
            out,
        };
        let stations = if end < 0.0 {
            [far, start]
        } else {
            [start, far]
        };
        sweep(b, &stations, &section);
        // The far end, where the road leaves it for the ground.
        cap(
            b,
            far,
            &[
                [CREST_HALF + 6.0, -50.0],
                [CREST_HALF, DECK],
                [-CREST_HALF, DECK],
                [-CREST_HALF - 16.0, -40.0],
            ],
            along,
            CAST,
        );
        for (o0, o1, top) in [
            (PARAPET_IN, CREST_HALF, PARAPET),
            (KERB_UP, PARAPET_IN, WALK),
            (-PARAPET_IN, KERB_DOWN, WALK),
            (-CREST_HALF, -PARAPET_IN, PARAPET),
        ] {
            cap(
                b,
                far,
                &[[o0, DECK], [o1, DECK], [o1, top], [o0, top]],
                along,
                CAST,
            );
        }
    }
}

/// An intake tower standing in the lake at angle `a`, `o` upstream: a fluted drum
/// from the gorge's floor, ringed white to the old full pool, a crown with a
/// cornice and a lantern, and a bridge to the crest.
fn intake_tower(b: &mut MeshBuilder, a: f32, o: f32, fine: bool) {
    let c = arch(a, o, 0.0);
    let sides = if fine { 24 } else { 12 };
    let ring = |r: f32, z: f32, fluted: bool| -> Vec<Vec3> {
        (0..sides)
            .map(|i| {
                let t = i as f32 * std::f32::consts::TAU / sides as f32;
                let r = if fluted && i % 2 == 1 { r - 0.6 } else { r };
                c + Vec3::new(t.cos() * r, t.sin() * r, z)
            })
            .collect()
    };
    let bands: &[(f32, f32, u32)] = &[
        (FLOOR, WATER, CAST),
        (WATER, WET_TOP, WET),
        (WET_TOP, RING_TOP, RING),
        (RING_TOP, DECK + 1.0, CAST),
    ];
    for &(z0, z1, pattern) in bands {
        b.paint(CONCRETE).pattern(pattern);
        b.loft(
            &[ring(TOWER_R, z0, fine), ring(TOWER_R, z1, fine)],
            false,
            false,
        );
    }
    // Crown: a cornice, a setback drum with its windows, a lantern roof.
    b.paint(CONCRETE).pattern(CAST);
    b.loft(
        &[
            ring(TOWER_R, DECK + 1.0, false),
            ring(TOWER_R + 0.8, DECK + 1.6, false),
            ring(TOWER_R + 0.8, DECK + 2.6, false),
            ring(TOWER_R - 0.8, DECK + 2.6, false),
            ring(TOWER_R - 0.8, DECK + 8.0, fine),
            ring(TOWER_R - 0.2, DECK + 8.6, false),
            ring(TOWER_R - 0.2, DECK + 9.4, false),
        ],
        false,
        true,
    );
    if fine {
        b.paint(WINDOWS);
        for k in 0..8 {
            let t = (k as f32 + 0.25) * std::f32::consts::TAU / 8.0;
            let d = Vec3::new(t.cos(), t.sin(), 0.0);
            let side = Vec3::new(-t.sin(), t.cos(), 0.0);
            let base = c + d * (TOWER_R - 0.72);
            b.face(&[
                base - side * 0.5 + Vec3::Z * (DECK + 3.4),
                base + side * 0.5 + Vec3::Z * (DECK + 3.4),
                base + side * 0.5 + Vec3::Z * (DECK + 7.2),
                base - side * 0.5 + Vec3::Z * (DECK + 7.2),
            ]);
        }
    }
    // A stepped concrete lantern, fins round the drum, a bronze finial.
    b.paint(CONCRETE).pattern(CAST);
    b.loft(
        &[
            ring(TOWER_R - 1.6, DECK + 9.4, false),
            ring(TOWER_R - 1.6, DECK + 11.0, false),
            ring(TOWER_R - 3.4, DECK + 11.0, false),
            ring(TOWER_R - 3.4, DECK + 12.6, false),
            ring(1.4, DECK + 13.4, false),
        ],
        false,
        true,
    );
    if fine {
        for k in 0..8 {
            let t = k as f32 * std::f32::consts::TAU / 8.0;
            let d = Vec3::new(t.cos(), t.sin(), 0.0);
            b.beam(
                c + d * (TOWER_R - 1.0) + Vec3::Z * (DECK + 2.6),
                c + d * (TOWER_R - 1.0) + Vec3::Z * (DECK + 10.2),
                Vec2::new(0.7, 1.6),
                Vec2::new(0.7, 1.0),
            );
        }
        b.paint(METAL);
        b.cylinder_between(
            c + Vec3::Z * (DECK + 13.2),
            c + Vec3::Z * (DECK + 16.5),
            0.25,
            0.05,
            5,
        );
    }
    // The bridge to the crest, its deck level with the road.
    let from = arch(a, o - TOWER_R + 0.3, DECK - 0.6);
    let to = arch(a, CREST_HALF - 0.3, DECK - 0.6);
    b.paint(CONCRETE).pattern(CAST);
    b.beam(from, to, Vec2::new(5.0, 1.2), Vec2::new(5.0, 1.2));
    if fine {
        let side = Vec3::new(-a.sin(), a.cos(), 0.0);
        for s in [-2.35f32, 2.35] {
            b.beam(
                from + side * s + Vec3::Z * 1.15,
                to + side * s + Vec3::Z * 1.15,
                Vec2::new(0.25, 1.1),
                Vec2::new(0.25, 1.1),
            );
        }
    }
}

/// A gated spillway at angle `a` by an abutment: a weir against the upstream face
/// with its crest at the old full pool, piers carrying a service deck and a
/// gantry crane, drum gates between them, high and dry now; and its chute down
/// the downstream face to the tailwater.
fn spillway(b: &mut MeshBuilder, a: f32, fine: bool) {
    let weir_out = CREST_HALF + 18.0;
    let place = |y: f32, o: f32, z: f32| arch_y(a, y, o, z);
    // The weir's body, a curved wall along the face.
    let piece = |o0: f32, o1: f32, z0: f32, z1: f32| -> Vec<Vec<Vec3>> {
        [-SPILLWAY_HALF, SPILLWAY_HALF]
            .iter()
            .map(|&y| {
                vec![
                    place(y, o0, z0),
                    place(y, o1, z0),
                    place(y, o1, z1),
                    place(y, o0, z1),
                ]
            })
            .collect()
    };
    b.paint(CONCRETE).pattern(RING);
    b.loft(
        &piece(CREST_HALF - 1.0, weir_out - 4.0, WATER - 12.0, RING_TOP),
        true,
        true,
    );
    // Piers with rounded noses, white below the old pool's line.
    let piers = if fine { 5 } else { 3 };
    for k in 0..piers {
        let y = -SPILLWAY_HALF + 2.0 * SPILLWAY_HALF * k as f32 / (piers - 1) as f32;
        let pier = |o0: f32, o1: f32, z0: f32, z1: f32| -> Vec<Vec<Vec3>> {
            [y - 1.2, y + 1.2]
                .iter()
                .map(|&y| {
                    vec![
                        place(y, o0, z0),
                        place(y, o1, z0),
                        place(y, o1, z1),
                        place(y, o0, z1),
                    ]
                })
                .collect()
        };
        b.paint(CONCRETE).pattern(RING);
        b.loft(
            &pier(CREST_HALF - 1.0, weir_out, WATER - 12.0, RING_TOP),
            true,
            true,
        );
        b.paint(CONCRETE).pattern(CAST);
        b.loft(
            &pier(CREST_HALF - 1.0, weir_out, RING_TOP, DECK),
            true,
            true,
        );
        if fine {
            b.paint(CONCRETE).pattern(RING);
            b.prism(
                place(y, weir_out, WATER - 12.0),
                8,
                1.3,
                1.3,
                RING_TOP - (WATER - 12.0),
            );
        }
    }
    // Drum gates between the piers, raised: steel faces from the weir's crest.
    b.paint(METAL);
    for k in 0..piers - 1 {
        let y0 = -SPILLWAY_HALF + 2.0 * SPILLWAY_HALF * k as f32 / (piers - 1) as f32 + 1.2;
        let y1 = -SPILLWAY_HALF + 2.0 * SPILLWAY_HALF * (k + 1) as f32 / (piers - 1) as f32 - 1.2;
        let o = weir_out - 7.0;
        b.beam(
            place(y0, o, RING_TOP + 2.8),
            place(y1, o, RING_TOP + 2.8),
            Vec2::new(1.4, 5.6),
            Vec2::new(1.4, 5.6),
        );
    }
    // A service walk along the crest side of the piers, and a gantry crane over the gates.
    b.paint(CONCRETE).pattern(CAST);
    b.loft(
        &piece(CREST_HALF - 0.5, CREST_HALF + 4.0, DECK - 1.2, DECK),
        true,
        true,
    );
    if fine {
        b.paint(METAL);
        let y = SPILLWAY_HALF * 0.3;
        for o in [CREST_HALF + 3.0, weir_out - 1.0] {
            for dy in [-2.5f32, 2.5] {
                b.cylinder_between(
                    place(y + dy, o, DECK),
                    place(y, o, DECK + 11.0),
                    0.35,
                    0.3,
                    4,
                );
            }
        }
        b.beam(
            place(y, CREST_HALF + 2.0, DECK + 11.5),
            place(y, weir_out, DECK + 11.5),
            Vec2::new(1.6, 1.4),
            Vec2::new(1.6, 1.4),
        );
        b.cuboid(
            place(y, weir_out - 6.0, DECK + 10.2),
            Vec3::new(3.0, 2.4, 1.6),
        );
    }
    // The chute: two walls down the downstream face and a stained floor between them.
    let zs: &[f32] = if fine {
        &[RING_TOP, -16.0, -24.0, -34.0, -46.0, -58.0, WATER + 1.0]
    } else {
        &[RING_TOP, -34.0, WATER + 1.0]
    };
    let lean = |z: f32| {
        // The downstream face's outward normal in (offset, height).
        let (o0, o1) = (down_face(z + 1.0), down_face(z - 1.0));
        Vec2::new(-2.0, o0 - o1).normalize()
    };
    let at = |y: f32, z: f32, lift: f32| {
        let n = lean(z);
        place(y, down_face(z) + n.x * lift, z + n.y * lift)
    };
    let half = 9.0;
    for side in [-1.0f32, 1.0] {
        let rings: Vec<Vec<Vec3>> = zs
            .iter()
            .map(|&z| {
                let (y0, y1) = (side * half, side * (half + 1.2));
                vec![
                    at(y0, z, -1.0),
                    at(y1, z, -1.0),
                    at(y1, z, 3.2),
                    at(y0, z, 3.2),
                ]
            })
            .collect();
        b.paint(CONCRETE).pattern(CAST);
        b.loft(&rings, true, true);
    }
    b.paint(CONCRETE).pattern(WET);
    for pair in zs.windows(2) {
        let (z0, z1) = (pair[0], pair[1]);
        b.face(&[
            at(half, z0, 0.35),
            at(-half, z0, 0.35),
            at(-half, z1, 0.35),
            at(half, z1, 0.35),
        ]);
    }
}

/// The powerhouse along the toe: a long curved hall on the tailwater, its back in
/// the dam, a row of tall windows, transformers and a crane on its roof.
fn powerhouse(b: &mut MeshBuilder, fine: bool) {
    let blocks = if fine { 12 } else { 4 };
    let back = down_face(POWERHOUSE_ROOF) + 1.0;
    let section = [
        pt(back, POWERHOUSE_ROOF, CAST),
        pt(POWERHOUSE_OUT + 1.2, POWERHOUSE_ROOF, CAST),
        pt(POWERHOUSE_OUT + 1.2, POWERHOUSE_ROOF + 1.1, CAST),
        pt(POWERHOUSE_OUT, POWERHOUSE_ROOF + 1.1, CAST),
        pt(POWERHOUSE_OUT, WET_TOP, WET),
        pt(POWERHOUSE_OUT, WATER - 8.0, CAST),
    ];
    let stations = arc_stations(-POWERHOUSE_ANGLE, POWERHOUSE_ANGLE, blocks);
    sweep(b, &stations, &section);
    let outline: Vec<[f32; 2]> = [
        [back, WATER - 8.0],
        [POWERHOUSE_OUT, WATER - 8.0],
        [POWERHOUSE_OUT, POWERHOUSE_ROOF + 1.1],
        [POWERHOUSE_OUT + 1.2, POWERHOUSE_ROOF + 1.1],
        [POWERHOUSE_OUT + 1.2, POWERHOUSE_ROOF],
        [back, POWERHOUSE_ROOF],
    ]
    .to_vec();
    for (st, facing) in [
        (stations[0], -tangent(-POWERHOUSE_ANGLE)),
        (stations[blocks], tangent(POWERHOUSE_ANGLE)),
    ] {
        cap(b, st, &outline, facing, CAST);
    }
    // Tall windows between pilasters, lit from within.
    b.paint(WINDOWS);
    let per = if fine { 3 } else { 1 };
    for i in 0..blocks * per {
        let t0 = -POWERHOUSE_ANGLE + 2.0 * POWERHOUSE_ANGLE * i as f32 / (blocks * per) as f32;
        let t1 = t0 + 2.0 * POWERHOUSE_ANGLE / (blocks * per) as f32;
        let (a0, a1) = (t0 + (t1 - t0) * 0.32, t1 - (t1 - t0) * 0.32);
        let o = POWERHOUSE_OUT - 0.12;
        let (z0, z1) = (WATER + 6.0, POWERHOUSE_ROOF - 3.0);
        b.face(&[
            arch(a1, o, z0),
            arch(a0, o, z0),
            arch(a0, o, z1),
            arch(a1, o, z1),
        ]);
    }
    if fine {
        // Transformers in a row along the roof's downstream side, and a crane.
        b.paint(METAL);
        for k in 0..8 {
            let a = -POWERHOUSE_ANGLE * 0.85 + POWERHOUSE_ANGLE * 1.7 * (k as f32 + 0.5) / 8.0;
            let c = arch(a, POWERHOUSE_OUT + 8.0, POWERHOUSE_ROOF + 2.2);
            b.yawed(c, a, |b| b.cuboid(Vec3::ZERO, Vec3::new(5.0, 7.0, 4.4)));
        }
        let a = POWERHOUSE_ANGLE * 0.35;
        for o in [back - 2.0, POWERHOUSE_OUT + 3.0] {
            b.cylinder_between(
                arch(a, o, POWERHOUSE_ROOF),
                arch(a, o, POWERHOUSE_ROOF + 12.0),
                0.5,
                0.4,
                4,
            );
        }
        b.beam(
            arch(a, back - 1.0, POWERHOUSE_ROOF + 12.5),
            arch(a, POWERHOUSE_OUT + 2.0, POWERHOUSE_ROOF + 12.5),
            Vec2::new(1.8, 1.6),
            Vec2::new(1.8, 1.6),
        );
    }
}

fn tangent(a: f32) -> Vec3 {
    Vec3::new(-a.sin(), a.cos(), 0.0)
}

/// A penstock at angle `a` from the intake level down the downstream face into
/// the powerhouse's roof, with concrete anchor collars at its bends.
fn penstock(b: &mut MeshBuilder, a: f32, fine: bool) {
    let zs: &[f32] = if fine {
        &[-8.0, -16.0, -26.0, POWERHOUSE_ROOF - 1.0]
    } else {
        &[-8.0, POWERHOUSE_ROOF - 1.0]
    };
    let at = |z: f32| arch(a, down_face(z) - PENSTOCK_R - 0.5, z);
    b.paint(METAL);
    let sides = if fine { 10 } else { 6 };
    for pair in zs.windows(2) {
        b.cylinder_between(at(pair[0]), at(pair[1]), PENSTOCK_R, PENSTOCK_R, sides);
    }
    b.paint(CONCRETE).pattern(CAST);
    for &z in &zs[..zs.len() - 1] {
        let c = at(z);
        b.yawed(c, a, |b| {
            b.cuboid(
                Vec3::X * 0.6,
                Vec3::new(PENSTOCK_R * 2.0 + 1.6, PENSTOCK_R * 2.0 + 1.4, 2.4),
            )
        });
    }
}

/// A lift tower on the downstream parapet near an end: a stepped shaft down the
/// face with slit windows, standing over the crest.
fn lift_tower(b: &mut MeshBuilder, a: f32) {
    let o = -CREST_HALF - 3.0;
    let c = arch(a, o, 0.0);
    b.paint(CONCRETE).pattern(CAST);
    // Built in a frame at the tower's middle, x upstream along the arch's radius.
    b.yawed(c, a, |b| {
        b.block(Vec3::new(-5.0, -4.0, -34.0), Vec3::new(4.0, 4.0, 11.0));
        b.block(Vec3::new(-4.2, -3.2, 11.0), Vec3::new(3.2, 3.2, 14.0));
        b.block(Vec3::new(-3.2, -2.4, 14.0), Vec3::new(2.2, 2.4, 15.8));
        b.paint(WINDOWS);
        for y in [-2.2f32, 0.0, 2.2] {
            let x = -5.02;
            b.face(&[
                Vec3::new(x, y + 0.3, -20.0),
                Vec3::new(x, y - 0.3, -20.0),
                Vec3::new(x, y - 0.3, 9.0),
                Vec3::new(x, y + 0.3, 9.0),
            ]);
        }
    });
}

/// Lamp posts along both sidewalks, one at every other block joint, their heads
/// reaching out over the road.
fn lamps(b: &mut MeshBuilder) {
    let joints = 28;
    for i in (2..joints - 1).step_by(2) {
        let a = -HALF_ANGLE + 2.0 * HALF_ANGLE * i as f32 / joints as f32;
        for side in [-1.0f32, 1.0] {
            let o = side * (PARAPET_IN - 0.7);
            let foot = arch(a, o, WALK);
            let toward = -radial(a) * side;
            b.paint(METAL);
            b.cylinder_between(foot, foot + Vec3::Z * 7.2, 0.16, 0.11, 6);
            b.cylinder_between(
                foot + Vec3::Z * 7.0,
                foot + toward * 1.5 + Vec3::Z * 7.3,
                0.07,
                0.06,
                4,
            );
            let head = foot + toward * 1.7 + Vec3::Z * 7.15;
            b.yawed(head, a, |b| {
                b.cuboid(Vec3::ZERO, Vec3::new(0.8, 0.5, 0.3));
                b.paint(GLOW_LAMP);
                b.cuboid(-Vec3::Z * 0.2, Vec3::new(0.6, 0.36, 0.1));
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{build_model, material};

    /// The crest covers the band the bake levels for the road, stands just over it
    /// across its whole width, and the arch reaches the abutments.
    #[test]
    fn the_crest_covers_the_road_band() {
        let model = build_model("landmark_dam").unwrap();
        for mesh in &model.lods {
            // Top of the model over points across the band, at several angles.
            for a in [-0.7f32, -0.35, 0.0, 0.3, 0.72] {
                for o in [-(DAM.road_down as f32), -5.0, 0.0, 5.0, DAM.road_up as f32] {
                    let p = arch(a, o, 0.0);
                    let top = mesh
                        .indices
                        .chunks(3)
                        // The crest itself, not the lamps over it.
                        .filter(|t| mesh.vertices[t[0] as usize].material == material::CONCRETE)
                        .filter_map(|t| {
                            let [v0, v1, v2] = [t[0], t[1], t[2]]
                                .map(|i| Vec3::from(mesh.vertices[i as usize].pos));
                            height_at(v0, v1, v2, p.truncate())
                        })
                        .fold(f32::MIN, f32::max);
                    assert!((0.25..=1.7).contains(&top), "a {a} o {o}: crest at {top}");
                }
            }
        }
    }

    /// Height of triangle `abc` over `p` in plan, if it covers it.
    fn height_at(a: Vec3, b: Vec3, c: Vec3, p: Vec2) -> Option<f32> {
        let (a2, b2, c2) = (a.truncate(), b.truncate(), c.truncate());
        let d = (b2 - a2).perp_dot(c2 - a2);
        if d.abs() < 1e-6 {
            return None;
        }
        let u = (b2 - p).perp_dot(c2 - p) / d;
        let v = (c2 - p).perp_dot(a2 - p) / d;
        let w = 1.0 - u - v;
        (u >= 0.0 && v >= 0.0 && w >= 0.0).then_some(a.z * u + b.z * v + c.z * w)
    }

    /// The mineral ring's faces end at the old full pool and start over the wet
    /// band; the lamps glow; the dam reaches down to its footing and no further.
    #[test]
    fn ring_band_and_footing() {
        let model = build_model("landmark_dam").unwrap();
        let mesh = &model.lods[0];
        let ring: Vec<f32> = mesh
            .vertices
            .iter()
            .filter(|v| v.material == material::CONCRETE && v.surface & 0xFF == RING)
            .map(|v| v.pos[2])
            .collect();
        let top = ring.iter().copied().fold(f32::MIN, f32::max);
        assert!((top - RING_TOP).abs() < 0.01, "ring top {top}");
        assert!(mesh
            .vertices
            .iter()
            .any(|v| v.material == material::GLOW_LAMP));
        let low = mesh
            .vertices
            .iter()
            .map(|v| v.pos[2])
            .fold(f32::MAX, f32::min);
        assert!((low - FLOOR).abs() < 0.01, "{low}");
        const { assert!(RING_TOP > WET_TOP && WET_TOP > WATER) };
    }
}
