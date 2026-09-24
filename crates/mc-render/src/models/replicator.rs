//! Survival mode's hostile machinery: the Replication Engine and the
//! Replication Nodes it raises. Not Aster: no white plating, no orange lines.
//! Graphite plate over gunmetal frames, and the white-hot violet of the
//! replication light (`GLOW_VIOLET`) wherever matter is being made.
//!
//! Every piece does a job: the engine is a foundry. Its hull is the furnace,
//! each of its eight bays is a mouth in the hull feeding a gantry whose
//! projector head prints a unit onto the ground in front of it, the fins between
//! the bays carry the light up to the crown, and the crown holds the ray emitter
//! that raises nodes across the map, ringed by the Suppression Lance's collar.
//!
//! Contract with the sim (`data/factions/aster/units/survival.ron`; model space:
//! x forward, y left, z up, origin on the ground at the centre):
//! - Engine ray emitter: [`ENGINE_RAY_EMITTER`].
//! - Engine bay k at k * 45 degrees (counter-clockwise from +x): the projector
//!   head at radius [`BAY_PROJECTOR_RADIUS`], height [`BAY_PROJECTOR_Z`]; the
//!   printed unit stands on the ground at radius [`BAY_PRINT_RADIUS`].
//! - Lance muzzle (40, 0, 118), turning about the model's z axis.
//! - Node: ray lands on the crown at [`NODE_RAY_CATCH`]; prints from
//!   [`NODE_PRINT_EMITTER`] onto the ground at (+50, 0).

use std::f32::consts::TAU;

use glam::{Vec2, Vec3};

use super::builder::{ngon, MeshBuilder, Section};
use super::library::ModelDef;
use super::material::*;
use super::{part, pattern};

pub(super) const MODELS: &[ModelDef] = &[
    ModelDef::new("replication_engine", 110.0, 150.0, engine),
    ModelDef::new("replication_node", 26.0, 46.0, node),
];

/// Where the engine's node-raising ray leaves the crown.
pub const ENGINE_RAY_EMITTER: [f32; 3] = [0.0, 0.0, 140.0];
/// Print bays: eight, the first facing +x.
pub const BAY_COUNT: usize = 8;
pub const BAY_PROJECTOR_RADIUS: f32 = 100.0;
pub const BAY_PROJECTOR_Z: f32 = 55.0;
pub const BAY_PRINT_RADIUS: f32 = 150.0;
/// The Suppression Lance's muzzle (the blueprint's).
const LANCE_MUZZLE: Vec3 = Vec3::new(40.0, 0.0, 118.0);
/// Full-detail budget: the engine is one 240 m landmark per match.
pub const ENGINE_TRIANGLES: usize = 12000;
/// Where the ray lands on a node, and where a node prints from.
pub const NODE_RAY_CATCH: [f32; 3] = [0.0, 0.0, 46.0];
pub const NODE_PRINT_EMITTER: [f32; 3] = [0.0, 0.0, 40.0];

fn v3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

fn v2(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

/// Dark trim: gunmetal-black with its outline and nothing else (plain `ACCENT` would
/// carry Aster's orange lights).
fn trim(b: &mut MeshBuilder) {
    b.paint(ACCENT).pattern(pattern::PLAIN);
}

/// The replicators' obsidian: black plate with violet lights let into it.
fn obsidian(b: &mut MeshBuilder) {
    b.paint(ACCENT).pattern(pattern::VEINED);
}

// ---- The Replication Engine ------------------------------------------------------

/// Plinth, two steps.
const PLINTH: f32 = 124.0;
const PLINTH_TOP: f32 = 5.0;
const STEP: f32 = 104.0;
const STEP_TOP: f32 = 10.0;
/// The foundry hull: octagonal, vertex radius at its foot, its eaves and its shoulder.
const HULL_FOOT: (f32, f32) = (10.0, 84.0);
const HULL_EAVE: (f32, f32) = (58.0, 77.0);
const HULL_SHOULDER: (f32, f32) = (70.0, 62.0);
/// The upper tower, turned half a face so its edges point down the bays.
const TOWER: [(f32, f32); 3] = [(70.0, 58.0), (100.0, 46.0), (108.0, 40.0)];
/// The Lance's collar: turret pivot height and its ring.
const COLLAR_LOW: f32 = 108.0;
const COLLAR_HIGH: f32 = 121.0;

/// Vertex radius of the foundry hull at height `z`.
fn hull_radius(z: f32) -> f32 {
    let lerp = |a: (f32, f32), b: (f32, f32)| a.1 + (b.1 - a.1) * ((z - a.0) / (b.0 - a.0));
    if z <= HULL_EAVE.0 {
        lerp(HULL_FOOT, HULL_EAVE)
    } else {
        lerp(HULL_EAVE, HULL_SHOULDER)
    }
}

/// Distance from the centre to the flat of an octagon of vertex radius `r`.
fn flat(r: f32) -> f32 {
    r * (TAU / 16.0).cos()
}

/// An octagonal loft (flat faces toward the bays) through `rings` of (z, vertex radius).
fn octagon(b: &mut MeshBuilder, rings: &[(f32, f32)]) {
    let sides = if b.coarse() { 4 } else { 8 };
    let profile = ngon(sides, 1.0);
    let sections: Vec<Section> = rings.iter().map(|&(z, r)| Section::new(z, r)).collect();
    b.loft_z(&profile, &sections);
}

/// A thin band of light proud of the hull at height `z`.
fn hull_band(b: &mut MeshBuilder, z: f32, height: f32) {
    b.paint(GLOW_VIOLET);
    let r0 = hull_radius(z) + 0.9;
    let r1 = hull_radius(z + height) + 0.9;
    octagon(b, &[(z, r0), (z + height, r1)]);
}

pub fn engine(b: &mut MeshBuilder, _tech: u8) {
    b.set_turret_pivot(v3(0.0, 0.0, COLLAR_LOW));
    if b.coarse() {
        engine_coarse(b);
        return;
    }
    foundation(b);
    // The furnace: everything the bays print is made in here.
    obsidian(b);
    octagon(b, &[HULL_FOOT, HULL_EAVE, HULL_SHOULDER]);
    hull_band(b, 44.0, 2.4);
    if b.fine() {
        hull_band(b, 24.0, 1.2);
    }
    obsidian(b);
    b.yawed(Vec3::ZERO, TAU / 16.0, |b| octagon(b, &TOWER));
    // The tower's crown band and the light ring on the shoulder.
    b.paint(GLOW_VIOLET);
    b.yawed(Vec3::ZERO, TAU / 16.0, |b| {
        octagon(b, &[(101.0, 45.6), (103.0, 44.8)]);
    });
    octagon(b, &[(HULL_SHOULDER.0 - 0.1, HULL_SHOULDER.1 + 0.8), (HULL_SHOULDER.0 + 1.4, 60.2)]);

    for k in 0..BAY_COUNT {
        b.yawed(Vec3::ZERO, k as f32 * TAU / BAY_COUNT as f32, bay);
        // The fins stand on the hull's edges, between the bays.
        b.yawed(Vec3::ZERO, (k as f32 + 0.5) * TAU / BAY_COUNT as f32, fin);
    }
    if b.fine() {
        // Coolant risers up the tower faces between the fins: the furnace's own plumbing.
        for k in 0..BAY_COUNT {
            b.yawed(Vec3::ZERO, k as f32 * TAU / BAY_COUNT as f32, |b| {
                b.paint(METAL);
                for y in [-3.2f32, 3.2] {
                    b.cylinder_between(v3(60.0, y, 69.0), v3(45.5, y, 99.5), 1.1, 1.1, 6);
                }
            });
        }
    }
    crown(b);
    b.with_part(part::TURRET, lance);
}

/// Two poured steps under everything, a light line where they meet.
fn foundation(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK);
    // Always eight-sided: a hexagon's corners would stand off the lot.
    let rim = ngon(8, 1.0);
    b.loft_z(
        &rim,
        &[Section::new(0.0, PLINTH), Section::new(PLINTH_TOP - 1.2, PLINTH), Section::new(PLINTH_TOP, PLINTH - 2.0)],
    );
    b.loft_z(&rim, &[Section::new(PLINTH_TOP - 0.1, STEP), Section::new(STEP_TOP, STEP - 1.5)]);
    if b.fine() {
        b.paint(GLOW_VIOLET);
        b.loft_z(&rim, &[Section::new(PLINTH_TOP - 0.2, STEP + 0.7), Section::new(PLINTH_TOP + 0.6, STEP + 0.6)]);
    }
}

/// One print bay, facing +x: a mouth in the hull, a cantilever out to the projector
/// head, the head's legs down to the plinth and the guide lights on the bed.
fn bay(b: &mut MeshBuilder) {
    let face = flat(hull_radius(12.0));
    // The mouth: a heavy frame let into the hull face, the furnace glowing in it.
    trim(b);
    if b.fine() {
        b.block(v3(face - 8.0, -15.0, STEP_TOP), v3(face + 3.0, -10.5, 42.0));
        b.block(v3(face - 8.0, 10.5, STEP_TOP), v3(face + 3.0, 15.0, 42.0));
    }
    b.block(v3(face - 8.0, -15.0, 38.0), v3(face + 3.5, 15.0, 45.0));
    b.paint(GLOW_VIOLET);
    b.block(v3(face - 6.0, -10.5, STEP_TOP), v3(face + 0.6, 10.5, 38.0));
    if b.fine() {
        // Louvres across the mouth: the light reads through slats, not as a flat panel.
        b.paint(METAL);
        let mut z = STEP_TOP + 3.0;
        while z < 36.0 {
            b.block(v3(face + 0.6, -10.5, z), v3(face + 1.8, 10.5, z + 1.1));
            z += 4.0;
        }
    }

    // The cantilever: out of the tower, over the eaves, to the projector head.
    let head = v3(BAY_PROJECTOR_RADIUS, 0.0, BAY_PROJECTOR_Z);
    let root = v3(40.0, 0.0, 100.0);
    let knee = v3(78.0, 0.0, 74.0);
    obsidian(b);
    b.beam(root, knee, v2(9.0, 10.0), v2(8.0, 8.5));
    b.beam(knee, head + v3(-7.0, 0.0, 7.0), v2(8.0, 8.5), v2(7.0, 7.0));
    // The feed: a conduit of light down the cantilever's back.
    b.paint(GLOW_VIOLET);
    b.beam(root + v3(0.0, 0.0, 5.4), knee + v3(0.0, 0.0, 4.6), v2(2.2, 0.8), v2(2.0, 0.8));
    if b.mid() {
        b.beam(knee + v3(0.0, 0.0, 4.6), head + v3(-6.4, 0.0, 11.0), v2(2.0, 0.8), v2(1.8, 0.8));
    }
    // The owner's colour on the cantilever's top, seen from above.
    b.paint(TEAM);
    b.beam(v3(58.0, 0.0, 89.4), v3(70.0, 0.0, 80.2), v2(4.0, 0.5), v2(4.0, 0.5));

    // The projector head, aimed down at the printed unit's middle.
    let aim = (v3(BAY_PRINT_RADIUS, 0.0, 6.0) - head).normalize();
    trim(b);
    let sides = if b.fine() { 8 } else { 4 };
    b.cylinder_between(head - aim * 11.0, head - aim * 1.4, 5.0, 6.4, sides);
    if b.fine() {
        b.paint(METAL);
        b.cylinder_between(head - aim * 1.6, head + aim * 0.4, 6.8, 6.0, sides);
    }
    b.paint(GLOW_VIOLET);
    b.cylinder_between(head - aim * 0.2, head + aim * 0.7, 4.6, 3.6, sides);

    // Its legs, down to the plinth either side of the bed.
    b.paint(METAL);
    for y in [-1.0f32, 1.0] {
        b.beam(v3(96.0, 6.0 * y, 50.0), v3(110.0, 12.0 * y, PLINTH_TOP), v2(2.6, 3.0), v2(3.6, 4.0));
    }
    if b.fine() {
        b.beam(v3(104.0, -10.0, 26.0), v3(104.0, 10.0, 26.0), v2(1.6, 1.6), v2(1.6, 1.6));
        // Guide lights along the print bed, out to where the unit stands up.
        b.paint(GLOW_VIOLET);
        for y in [-7.0f32, 7.0] {
            b.block(v3(STEP + 1.0, y - 0.5, PLINTH_TOP - 0.2), v3(111.0, y + 0.5, PLINTH_TOP + 0.35));
        }
    }
}

/// A blade fin on a hull edge, facing +x: from the plinth up to the crown,
/// its top edge a line of light carried up to the emitter.
fn fin(b: &mut MeshBuilder) {
    // (radius, z), convex, counter-clockwise.
    let profile = [[72.0, 8.0], [114.0, 3.0], [114.0, 12.0], [52.0, 110.0], [40.0, 114.0]];
    obsidian(b);
    let half = if b.fine() { 2.6 } else { 2.2 };
    if b.fine() {
        b.extrude_y_chamfered(&profile, half, 0.6);
    } else {
        b.extrude_y(&profile, -half, half);
    }
    b.paint(GLOW_VIOLET);
    let (lo, hi) = (v3(114.0, 0.0, 12.0), v3(52.0, 0.0, 110.0));
    let up = Vec3::new(98.0, 0.0, 62.0).normalize() * 0.5;
    b.beam(lo + up, hi + up, v2(1.2, 1.0), v2(1.0, 1.0));
    if b.fine() {
        // Rungs of dark plate across the blade, so its light reads as a run, not a tube.
        trim(b);
        for i in 1..6 {
            let t = i as f32 / 6.0;
            let p = lo.lerp(hi, t) + up * 1.4;
            b.cuboid(p, v3(2.2, half * 2.0 + 0.6, 1.6));
        }
    }
}

/// A ring of `segments` straight tubes round the z axis at `center`.
fn ring(b: &mut MeshBuilder, center: Vec3, radius: f32, tube: f32, segments: usize) {
    for i in 0..segments {
        let a0 = i as f32 * TAU / segments as f32;
        let a1 = (i + 1) as f32 * TAU / segments as f32;
        // Overlap the joints a little so the ring reads as one piece.
        let p = |a: f32| center + Vec3::new(a.cos(), a.sin(), 0.0) * radius;
        let (p0, p1) = (p(a0), p(a1));
        let d = (p1 - p0) * 0.04;
        b.cylinder_between(p0 - d, p1 + d, tube, tube, if b.fine() { 6 } else { 4 });
    }
}

/// The central spire over the collar, and the emitter cage round the ray's crystal.
fn crown(b: &mut MeshBuilder) {
    let emitter = Vec3::from(ENGINE_RAY_EMITTER);
    obsidian(b);
    let sides = b.sides(8);
    // Column through the Lance's collar up to the cage's foot.
    b.prism(v3(0.0, 0.0, 104.0), sides, 18.0, 13.0, 22.0);
    trim(b);
    b.prism(v3(0.0, 0.0, 125.5), sides, 15.0, 10.0, 3.5);
    // Eight claws round the crystal: out, up, and in over it, holding a halo of light.
    let claws = if b.fine() { 8 } else { 4 };
    let halo = v3(0.0, 0.0, 150.0);
    for k in 0..claws {
        let a = (k as f32 + 0.5) * TAU / claws as f32;
        b.yawed(Vec3::ZERO, a, |b| {
            obsidian(b);
            let p0 = v3(11.0, 0.0, 127.0);
            let p1 = v3(21.0, 0.0, 148.0);
            let p2 = v3(7.0, 0.0, 174.0);
            b.beam(p0, p1, v2(4.4, 5.2), v2(3.2, 3.4));
            b.beam(p1, p2, v2(3.2, 3.4), v2(0.9, 1.0));
            if b.fine() {
                b.paint(GLOW_VIOLET);
                let inward = v3(-1.8, 0.0, 0.0);
                b.beam(p0 + inward, p1 + inward, v2(0.9, 0.8), v2(0.7, 0.6));
            }
        });
    }
    b.paint(GLOW_VIOLET);
    if b.mid() {
        ring(b, halo, 19.6, 0.8, if b.fine() { 24 } else { 8 });
    }
    // The crystal the ray leaves from.
    let rings = if b.fine() { 4 } else { 2 };
    b.spheroid(emitter, v3(7.5, 7.5, 13.0), b.sides(8), rings);
}

/// The Suppression Lance on its collar (`part::TURRET`): a ring round the spire and a
/// twin-railed lance ending at the blueprint's muzzle.
fn lance(b: &mut MeshBuilder) {
    let sides = b.sides(12);
    trim(b);
    b.prism(v3(0.0, 0.0, COLLAR_LOW), sides, 27.0, 24.0, COLLAR_HIGH - COLLAR_LOW);
    b.paint(GLOW_VIOLET);
    if b.mid() {
        b.prism(v3(0.0, 0.0, COLLAR_LOW + 3.0), sides, 27.4, 27.0, 1.0);
    }
    // Housing on the collar's front, then the rails to the muzzle.
    obsidian(b);
    b.beam(v3(6.0, 0.0, LANCE_MUZZLE.z + 1.0), v3(27.0, 0.0, LANCE_MUZZLE.z + 0.5), v2(9.0, 8.0), v2(6.5, 5.0));
    b.paint(METAL);
    for y in [-1.9f32, 1.9] {
        b.beam(v3(24.0, y, LANCE_MUZZLE.z), v3(LANCE_MUZZLE.x - 0.3, y, LANCE_MUZZLE.z), v2(1.4, 2.4), v2(1.2, 2.0));
    }
    // The bore between the rails, lit, ending at the muzzle.
    b.paint(GLOW_VIOLET);
    b.cylinder_between(v3(22.0, 0.0, LANCE_MUZZLE.z), LANCE_MUZZLE, 0.9, 0.9, b.sides(6));
    if b.fine() {
        b.paint(TEAM);
        b.beam(v3(9.0, 0.0, LANCE_MUZZLE.z + 5.0), v3(21.0, 0.0, LANCE_MUZZLE.z + 3.9), v2(3.0, 0.4), v2(3.0, 0.4));
    }
}

/// A handful of solids: plinth, hull, spire, crystal, the Lance.
fn engine_coarse(b: &mut MeshBuilder) {
    obsidian(b);
    b.cuboid(v3(0.0, 0.0, STEP_TOP * 0.5), v3(210.0, 210.0, STEP_TOP));
    b.yawed(Vec3::ZERO, TAU / 8.0, |b| {
        b.frustum(v3(0.0, 0.0, STEP_TOP), v2(150.0, 150.0), v2(80.0, 80.0), 96.0, Vec2::ZERO);
    });
    b.frustum(v3(0.0, 0.0, 104.0), v2(30.0, 30.0), v2(4.0, 4.0), 70.0, Vec2::ZERO);
    b.paint(GLOW_VIOLET);
    b.spheroid(Vec3::from(ENGINE_RAY_EMITTER), v3(10.0, 10.0, 14.0), 4, 2);
    b.with_part(part::TURRET, |b| {
        obsidian(b);
        b.beam(v3(0.0, 0.0, LANCE_MUZZLE.z), LANCE_MUZZLE, v2(10.0, 8.0), v2(4.0, 3.0));
    });
}

// ---- The Replication Node ---------------------------------------------------------

/// Top of the node's footing (a pontoon when it stands in the sea).
const FOOTING: f32 = 4.0;

pub fn node(b: &mut MeshBuilder, _tech: u8) {
    let print = Vec3::from(NODE_PRINT_EMITTER);
    let catch = Vec3::from(NODE_RAY_CATCH);
    if b.coarse() {
        obsidian(b);
        b.yawed(Vec3::ZERO, TAU / 8.0, |b| {
            b.frustum(Vec3::ZERO, v2(40.0, 40.0), v2(26.0, 26.0), FOOTING, Vec2::ZERO);
        });
        b.frustum(v3(0.0, 0.0, FOOTING), v2(15.0, 15.0), v2(6.0, 6.0), 34.0, Vec2::ZERO);
        b.paint(GLOW_VIOLET);
        b.spheroid(v3(0.0, 0.0, 44.0), v3(4.0, 4.0, 8.0), 4, 2);
        return;
    }
    // Footing: reads as a poured footing ashore and a pontoon afloat.
    b.paint(PLATING_DARK);
    let rim = ngon(8, 1.0);
    b.loft_z(&rim, &[Section::new(0.0, 16.0), Section::new(FOOTING - 1.0, 16.0), Section::new(FOOTING, 14.5)]);
    if b.fine() {
        b.paint(GLOW_VIOLET);
        b.loft_z(&rim, &[Section::new(1.2, 16.35), Section::new(2.0, 16.35)]);
    }
    // Four splayed legs out to the corners of the lot, each on a pad.
    for k in 0..4 {
        b.yawed(Vec3::ZERO, (k as f32 + 0.5) * TAU / 4.0, |b| {
            obsidian(b);
            b.beam(v3(25.0, 0.0, 1.5), v3(7.0, 0.0, 24.0), v2(3.0, 3.4), v2(2.2, 2.4));
            if b.fine() {
                trim(b);
                b.cuboid(v3(25.0, 0.0, 1.4), v3(6.5, 6.5, 2.8));
                b.paint(GLOW_VIOLET);
                b.beam(v3(25.4, 0.0, 3.4), v3(8.4, 0.0, 25.0), v2(0.7, 0.5), v2(0.5, 0.4));
            }
        });
    }
    // The pylon.
    obsidian(b);
    let hex = ngon(b.sides(6), 1.0);
    b.yawed(Vec3::ZERO, TAU / 12.0, |b| {
        b.loft_z(&hex, &[Section::new(FOOTING - 0.2, 9.0), Section::new(33.0, 5.6), Section::new(36.0, 4.6)]);
    });
    // Lines of light up three of its faces, carried to the print head.
    if b.fine() {
        b.paint(GLOW_VIOLET);
        for k in 0..3 {
            b.yawed(Vec3::ZERO, (k as f32 * 2.0 + 1.0) * TAU / 6.0 + TAU / 12.0, |b| {
                // On a face: the hexagon's face normal at this yaw is +x.
                let at = |z: f32| (9.0 + (5.6 - 9.0) * ((z - FOOTING) / (33.0 - FOOTING))) * (TAU / 12.0).cos() + 0.15;
                b.beam(v3(at(8.0), 0.0, 8.0), v3(at(31.0), 0.0, 31.0), v2(1.0, 0.4), v2(0.8, 0.4));
            });
        }
    }
    // The print head: a ring round the print orb on the pylon's top.
    trim(b);
    b.prism(v3(0.0, 0.0, 35.6), b.sides(8), 5.6, 4.2, 1.6);
    b.paint(METAL);
    ring(b, print, 5.4, 0.55, if b.fine() { 12 } else { 6 });
    b.paint(GLOW_VIOLET);
    b.spheroid(print, v3(3.0, 3.0, 2.8), b.sides(8), if b.fine() { 4 } else { 2 });
    // The crown that catches the ray: prongs up from the ring round the receiver crystal.
    for k in 0..3 {
        b.yawed(Vec3::ZERO, k as f32 * TAU / 3.0 + TAU / 6.0, |b| {
            obsidian(b);
            b.beam(v3(5.2, 0.0, 39.0), v3(6.0, 0.0, 47.0), v2(1.4, 1.6), v2(1.2, 1.2));
            b.beam(v3(6.0, 0.0, 47.0), v3(2.0, 0.0, 54.0), v2(1.2, 1.2), v2(0.4, 0.4));
        });
    }
    b.paint(GLOW_VIOLET);
    b.spheroid(catch + v3(0.0, 0.0, 1.0), v3(2.2, 2.2, 3.4), b.sides(6), 2);
    // The owner's colour on the footing's deck.
    b.paint(TEAM);
    b.cuboid(v3(-11.0, 0.0, FOOTING + 0.1), v3(3.0, 8.0, 0.4));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{build_model_scaled, Model};

    fn tops(m: &Model, lod: usize) -> (f32, f32, f32) {
        let v = &m.lods[lod].vertices;
        let top = v.iter().map(|v| v.pos[2]).fold(0.0, f32::max);
        let x = v.iter().map(|v| v.pos[0].abs()).fold(0.0, f32::max);
        let y = v.iter().map(|v| v.pos[1].abs()).fold(0.0, f32::max);
        (top, x, y)
    }

    fn nearest(m: &Model, lod: usize, p: Vec3, material: u32) -> f32 {
        let mesh = &m.lods[lod];
        mesh.indices
            .chunks(3)
            .filter(|t| mesh.vertices[t[0] as usize].material == material)
            .flat_map(|t| t.iter().map(|&i| Vec3::from(mesh.vertices[i as usize].pos)))
            .map(|q| q.distance(p))
            .fold(f32::MAX, f32::min)
    }

    #[test]
    fn engine_keeps_the_sims_contract() {
        let m = build_model_scaled("replication_engine", 110.0, 150.0, 5).unwrap();
        for lod in 0..3 {
            let (top, x, y) = tops(&m, lod);
            assert!(top >= 120.0 && top <= 187.0, "lod{lod} top {top}");
            assert!(x <= 120.0 && y <= 120.0, "lod{lod} extent {x} x {y} outside the 20x20 lot");
            // Nothing of the turret past the muzzle.
            let over = m.lods[lod]
                .vertices
                .iter()
                .filter(|v| v.part == part::TURRET)
                .map(|v| v.pos[0] - LANCE_MUZZLE.x)
                .fold(f32::MIN, f32::max);
            assert!(over < 0.5, "lod{lod}: lance overshoots its muzzle by {over}");
        }
        assert!(Vec3::from(m.turret_pivot).truncate().length() < 1e-4);
        // The ray leaves from inside the crystal; each bay's head has its lens at the contract point.
        assert!(nearest(&m, 0, Vec3::from(ENGINE_RAY_EMITTER), GLOW_VIOLET) < 12.0);
        for k in 0..BAY_COUNT {
            let a = k as f32 * TAU / BAY_COUNT as f32;
            let head = Vec3::new(a.cos() * BAY_PROJECTOR_RADIUS, a.sin() * BAY_PROJECTOR_RADIUS, BAY_PROJECTOR_Z);
            assert!(nearest(&m, 0, head, GLOW_VIOLET) < 5.0, "bay {k}: no lens at the projector");
            // The printed unit's spot is clear of the model.
            let spot = Vec3::new(a.cos() * BAY_PRINT_RADIUS, a.sin() * BAY_PRINT_RADIUS, 0.0);
            assert!(m.lods[0].vertices.iter().all(|v| Vec3::from(v.pos).truncate().distance(spot.truncate()) > 25.0));
        }
        // The muzzle is on the Lance's bore.
        assert!(nearest(&m, 0, LANCE_MUZZLE, GLOW_VIOLET) < 1.5);
        let tris: Vec<usize> = m.lods.iter().map(|l| l.indices.len() / 3).collect();
        assert!(tris[2] < 60 && tris[1] as f32 <= tris[0] as f32 * 0.45 + 20.0, "{tris:?}");
        assert!(tris[0] <= ENGINE_TRIANGLES, "{tris:?}");
    }

    /// Software previews of both models: `MODEL_DUMP_DIR` (default target/model-dump).
    #[test]
    #[ignore = "writes inspection files"]
    fn dump_replicators() {
        let dir = std::path::PathBuf::from(std::env::var("MODEL_DUMP_DIR").unwrap_or_else(|_| "target/model-dump".into()));
        std::fs::create_dir_all(&dir).unwrap();
        for (key, r, h) in [("replication_engine", 110.0, 150.0), ("replication_node", 26.0, 46.0)] {
            let m = build_model_scaled(key, r, h, 3).unwrap();
            for az in [-38.0f32, 60.0, 142.0] {
                crate::models::preview::render(&m.lods[0], 768, az)
                    .write_ppm(&dir.join(format!("{key}_{az}.ppm")))
                    .unwrap();
            }
            for lod in 1..3 {
                crate::models::preview::render(&m.lods[lod], 384, -38.0)
                    .write_ppm(&dir.join(format!("{key}_lod{lod}.ppm")))
                    .unwrap();
            }
            println!("{key}: {:?}", m.lods.iter().map(|l| l.indices.len() / 3).collect::<Vec<_>>());
        }
    }

    /// The library-wide mesh checks, run on these two alone (the shared ones stop at the
    /// first model that fails anywhere in the roster).
    #[test]
    fn meshes_are_sound() {
        for (key, r, h) in [("replication_engine", 110.0, 150.0), ("replication_node", 26.0, 46.0)] {
            let m = build_model_scaled(key, r, h, 3).unwrap();
            for (lod, mesh) in m.lods.iter().enumerate() {
                for v in &mesh.vertices {
                    assert!(v.pos[2] >= -1e-3, "{key} lod{lod}: below ground");
                    assert!((Vec3::from(v.normal).length() - 1.0).abs() < 1e-4);
                    assert!(v.material <= GLOW_VIOLET && v.part <= part::TURRET);
                    assert!(Vec3::from(v.pos).length() <= m.bounds_radius + 1e-3);
                }
                for t in mesh.indices.chunks(3) {
                    let [a, b, c] = [t[0], t[1], t[2]].map(|i| Vec3::from(mesh.vertices[i as usize].pos));
                    let g = (b - a).cross(c - a);
                    assert!(g.length() * 0.5 > 1e-7, "{key} lod{lod}: degenerate at {a}");
                    for &i in t {
                        let n = Vec3::from(mesh.vertices[i as usize].normal);
                        assert!(g.normalize().dot(n) > 0.999, "{key} lod{lod}: winding at {a}");
                    }
                }
            }
        }
    }

    #[test]
    fn node_keeps_the_sims_contract() {
        let m = build_model_scaled("replication_node", 26.0, 46.0, 3).unwrap();
        for lod in 0..3 {
            let (top, x, y) = tops(&m, lod);
            assert!(top >= 46.0 * 0.8 && top <= 46.0 * 1.25, "lod{lod} top {top}");
            assert!(x <= 30.0 && y <= 30.0 && x >= 16.0, "lod{lod} extent {x} x {y}");
        }
        assert!(nearest(&m, 0, Vec3::from(NODE_RAY_CATCH), GLOW_VIOLET) < 4.0);
        assert!(nearest(&m, 0, Vec3::from(NODE_PRINT_EMITTER), GLOW_VIOLET) < 3.5);
    }
}
