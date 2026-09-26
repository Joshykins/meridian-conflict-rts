//! The Axis's crown: pieces of the Precursor machine first made for the polar
//! map and now standing on the tropical one (`mc_map::PropKind::Precursor{Vault,
//! Axis,Terrace,Lining}`; `mc-map/src/bake/archipelago.rs` stands the axis on
//! dry ground at 0.35, in the middle of the machine down the big island, its ring
//! hovering where the water used to be). The vault, the terrace and the lining are
//! no longer laid. Only the needle's necks, its ring and its point are lit.
//!
//! Same kit as the megastructure (`precursor_mega.rs`): chamfered girders, pale
//! alloy over a dark core, cold light in the seams. Model space as for every prop:
//! x along the heading, y left, z up, the origin on the ground.

use std::f32::consts::FRAC_PI_4;

use glam::{Affine3A, Vec3};

use super::builder::{MeshBuilder, Section};
use super::library::ModelDef;
use super::precursor::{cut_rect, dark, fine_rect, key_light, light, pale, panel, seam, v3};
use super::precursor_mega::{girder, Run};

pub(super) const MODELS: &[ModelDef] = &[
    ModelDef::new("precursor_vault", 3_000.0, VAULT_RISE + 60.0, vault),
    ModelDef::new("precursor_axis", 420.0, AXIS_TOP, axis),
    ModelDef::new("precursor_terrace", 140.0, 121.0, terrace),
    ModelDef::new("precursor_lining", 110.0, LINING_DEPTH, lining),
];

// ---- Terrace ----------------------------------------------------------------------
//
// A 200 m length of the stepped face between two of the pit's terraces: four courses of
// pale alloy, each 30 m high and 22 m behind the one below, the dark core showing in a
// recessed band along every riser with a line of light in it, a line of light along
// every nosing, and pilasters up the risers. Laid 50 m out from the step's foot on
// the terrace below, facing +x, so it stands level; its courses case the terrain's own
// risers (`mc-map/src/bake/polar.rs`: `RISERS`, `TREAD`, and 30 m risers).

pub(super) const TERRACE_SETBACK: f32 = 50.0;
pub(super) const TERRACE_TREAD: f32 = 22.0;
pub(super) const TERRACE_RISER: f32 = 30.0;
const TERRACE_COURSES: usize = 4;
const TERRACE_HALF: f32 = 100.0;

fn terrace(b: &mut MeshBuilder, _tech: u8) {
    let back = -(TERRACE_SETBACK + TERRACE_TREAD * (TERRACE_COURSES - 1) as f32 + 14.0);
    let half = TERRACE_HALF;
    for j in 0..TERRACE_COURSES {
        // Each course 3 m proud of the terrain riser it cases.
        let front = -(TERRACE_SETBACK + TERRACE_TREAD * j as f32) + 3.0;
        let (z0, z1) = (TERRACE_RISER * j as f32 - if j == 0 { 6.0 } else { 1.0 }, TERRACE_RISER * (j + 1) as f32 + 0.4);
        pale(b);
        b.chamfered_box(v3((front + back) * 0.5, 0.0, (z0 + z1) * 0.5), v3(front - back, half * 2.0, z1 - z0), 1.2);
        if b.coarse() {
            continue;
        }
        let base = TERRACE_RISER * j as f32;
        let face = |y: f32, z: f32| v3(front, y, z);
        // The dark band along the riser, the light in it, the light along the nosing.
        dark(b);
        panel(b, &[face(-half + 2.0, base + 9.0), face(half - 2.0, base + 9.0), face(half - 2.0, base + 21.0), face(-half + 2.0, base + 21.0)], Vec3::X, 0.3, 0.0);
        seam(b, face(-half + 4.0, base + 15.0) + Vec3::X * 0.3, face(half - 4.0, base + 15.0) + Vec3::X * 0.3, Vec3::X, 2.0);
        seam(b, v3(front - 2.5, -half + 3.0, z1), v3(front - 2.5, half - 3.0, z1), Vec3::Z, 1.4);
        if b.fine() {
            pale(b);
            let mut y = -half + 25.0;
            while y < half - 10.0 {
                b.chamfered_box(v3(front + 1.0, y, base + 15.0), v3(3.0, 6.0, TERRACE_RISER - 1.0), 0.8);
                y += 50.0;
            }
        }
    }
}

// ---- Lining -----------------------------------------------------------------------
//
// A 200 m length of the casing down a sheer wall to the sea: the Well's and the
// Sluice's. It hangs from a coping on the rim: the origin stands on the terrace 60 m
// back from the edge, the wall below it looking +x. Fins stand out of it every 40 m,
// the dark core in the bays between them with lines of light down every other bay; a
// band of light runs under the coping and another near the water, 520 m down (the map
// scales the piece to the height of the rim it hangs from).

pub(super) const LINING_EDGE: f32 = 60.0;
pub(super) const LINING_DEPTH: f32 = 560.0;
/// How far under the rim the water is at scale 1.
const LINING_WATER: f32 = 520.0;

fn lining(b: &mut MeshBuilder, _tech: u8) {
    let half = TERRACE_HALF;
    let e = LINING_EDGE;
    // The coping, proud of the edge.
    pale(b);
    b.chamfered_box(v3(e - 12.0, 0.0, -1.5), v3(34.0, half * 2.0, 5.0), 1.2);
    // The wall.
    if b.coarse() {
        b.chamfered_box(v3(e - 4.0, 0.0, -LINING_DEPTH * 0.5 - 2.0), v3(12.0, half * 2.0, LINING_DEPTH), 1.0);
        return;
    }
    dark(b);
    b.chamfered_box(v3(e - 6.0, 0.0, -LINING_DEPTH * 0.5 - 2.0), v3(12.0, half * 2.0, LINING_DEPTH), 1.0);
    // Fins.
    pale(b);
    let mut y = -half + 20.0;
    let mut bay = 0;
    while y < half && b.fine() {
        b.chamfered_box(v3(e + 2.0, y, -LINING_DEPTH * 0.5 - 3.0), v3(6.0, 9.0, LINING_DEPTH - 2.0), 1.0);
        // Down the middle of every other bay, a line of light.
        if bay % 2 == 0 && y + 20.0 < half {
            seam(b, v3(e + 0.1, y + 20.0, -30.0), v3(e + 0.1, y + 20.0, -LINING_WATER + 40.0), Vec3::X, 2.4);
        }
        bay += 1;
        y += 40.0;
    }
    // Bands across the fins: under the coping, and above the water.
    for z in [-12.0f32, -LINING_WATER + 30.0] {
        pale(b);
        b.chamfered_box(v3(e + 3.5, 0.0, z), v3(4.0, half * 2.0, 8.0), 0.8);
        seam(b, v3(e + 5.6, -half + 2.0, z), v3(e + 5.6, half - 2.0, z), Vec3::X, 2.2);
    }
}

// ---- Vault ----------------------------------------------------------------------
//
// An arch 6000 m from foot to foot along +x, 1600 m high at its crown: two ribs 92 m
// apart, each a girder with its dark windowed band and a line of light down it,
// bound by ties except over the crown (where the axis passes between them). Each foot
// is a battered plinth with a blade buttress fore and aft; a keystone of light hangs
// under the crown. Symmetric end for end, so the map may lay it from either foot.

/// Half the distance between the feet, and the crown's height over them. `mc-map`
/// scales the vault so its feet land on a ring either side of the well.
pub(super) const VAULT_HALF: f32 = 3_000.0;
pub(super) const VAULT_RISE: f32 = 1_600.0;
const RIB_Y: f32 = 58.0;
const RIB_HW: f32 = 23.0;
const RIB_HD: f32 = 62.0;

/// The arch's middle line at `x` (0..2 * VAULT_HALF): steep at the feet, broad over the crown.
fn vault_z(x: f32) -> f32 {
    let u = ((x - VAULT_HALF) / VAULT_HALF).clamp(-1.0, 1.0);
    VAULT_RISE * (1.0 - u * u).max(0.0).powf(0.58)
}

/// Points along the arch, closer together at the feet where it bends hardest.
fn vault_points(n: usize) -> Vec<Vec3> {
    (0..=n)
        .map(|i| {
            // Cosine spacing: fine near the feet, coarse over the crown.
            let s = i as f32 / n as f32;
            let x = VAULT_HALF * (1.0 - (s * std::f32::consts::PI).cos());
            v3(x, 0.0, vault_z(x))
        })
        .collect()
}

fn vault(b: &mut MeshBuilder, _tech: u8) {
    let segments = if b.fine() {
        44
    } else if b.mid() {
        24
    } else {
        12
    };
    let points = vault_points(segments);
    for y in [-RIB_Y, RIB_Y] {
        let o = v3(0.0, y, 0.0);
        for w in points.windows(2) {
            let d = (w[1] - w[0]).normalize();
            // Up is square to the rib in the arch's own plane, so the flanks face ±y.
            let up = v3(-d.z, 0.0, d.x);
            let run = Run::new(w[0] + o, w[1] + o, up);
            girder(b, &run, -4.0, run.len + 4.0, RIB_HW, RIB_HD, 16.0, 90.0, 0.0);
        }
    }

    // Ties between the ribs, square to the arch; none over the crown.
    dark(b);
    let tie = cut_rect(b, 12.0, 30.0, 3.0);
    let ties = if b.coarse() { 4 } else { 10 };
    for i in 1..ties {
        let s = i as f32 / ties as f32;
        if (s - 0.5).abs() < 0.12 {
            continue;
        }
        let x = VAULT_HALF * (1.0 - (s * std::f32::consts::PI).cos());
        let z = vault_z(x);
        let reach = RIB_Y - RIB_HW + 1.0;
        let run = Run::new(v3(x, -reach, z), v3(x, reach, z), Vec3::Z);
        run.solid(b, 0.0, run.len, &tie, 0.0);
    }

    // The feet.
    for x in [0.0f32, 2.0 * VAULT_HALF] {
        let facing = if x == 0.0 { 1.0f32 } else { -1.0 };
        b.with(Affine3A::from_translation(v3(x, 0.0, 0.0)) * Affine3A::from_rotation_z(if facing > 0.0 { 0.0 } else { std::f32::consts::PI }), |b| {
            vault_foot(b);
        });
    }

    // The keystone: a block of light hanging under the crown, pale caps either side.
    let crown = v3(VAULT_HALF, 0.0, VAULT_RISE - RIB_HD - 60.0);
    light(b);
    b.chamfered_box(crown, v3(40.0, 24.0, 30.0), 4.0);
    if b.mid() {
        pale(b);
        for s in [-1.0f32, 1.0] {
            b.chamfered_box(crown + v3(s * 38.0, 0.0, 0.0), v3(22.0, 30.0, 46.0), 4.0);
        }
    }
}

/// One foot, the arch leaving it along +x: a plinth battered like a dam, a dark
/// course with light in it, and blade buttresses standing out fore and aft.
fn vault_foot(b: &mut MeshBuilder) {
    pale(b);
    let plan = cut_rect(b, 70.0, 92.0, 18.0);
    b.loft_z(&plan, &[Section::new(-80.0, 1.12), Section::new(0.0, 1.08), Section::new(64.0, 1.0)]);
    if !b.coarse() {
        dark(b);
        b.loft_z(&plan, &[Section::new(63.0, 0.96), Section::new(80.0, 0.96)]);
        light(b);
        b.loft_z(&plan, &[Section::new(70.0, 0.975), Section::new(73.0, 0.975)]);
    }
    pale(b);
    let cap = cut_rect(b, 62.0, 86.0, 14.0);
    b.loft_z(&cap, &[Section::new(79.0, 1.0), Section::new(110.0, 0.9)]);
    if b.mid() {
        // Blades out of the back and the sides, leaning into the arch.
        let blade: [[f32; 2]; 5] = [[-60.0, -20.0], [-190.0, -20.0], [-190.0, 10.0], [-80.0, 260.0], [-58.0, 260.0]];
        pale(b);
        for y in [-RIB_Y, RIB_Y] {
            b.with(Affine3A::from_translation(v3(0.0, y, 0.0)), |b| {
                b.extrude_y_chamfered(&blade, 7.0, 2.0);
                if b.fine() {
                    for s in [-7.0f32, 7.0] {
                        seam(b, v3(-176.0, s, 18.0), v3(-86.0, s, 236.0), v3(0.0, s.signum(), 0.0), 2.0);
                    }
                }
            });
        }
    }
}

// ---- Axis -----------------------------------------------------------------------
//
// The needle at the pole: from the floor of the well, 1000 m under the water at scale
// 1, to a point 3600 m over it (the polar map lays it at 0.6 in a Well 600 m deep). A
// dark shaft between pale corner piers, stepped in three stages on necks of light; a
// ring hovering flat just over the water, and a lit point hovering over the top.

pub(super) const AXIS_TOP: f32 = 4_600.0;
/// Height of the water over the axis's foot.
const WATERLINE: f32 = 1_000.0;
const AXIS_STAGES: [(f32, f32, f32, f32); 3] = [
    (0.0, 1_900.0, 110.0, 66.0),
    (1_940.0, 3_560.0, 60.0, 40.0),
    (3_600.0, 4_420.0, 36.0, 22.0),
];

fn axis(b: &mut MeshBuilder, _tech: u8) {
    // The foot, under the water.
    pale(b);
    let foot = cut_rect(b, 150.0, 150.0, 44.0);
    b.loft_z(&foot, &[Section::new(-80.0, 1.0), Section::new(0.0, 1.0), Section::new(260.0, 0.66)]);

    for (i, &(z0, z1, h0, h1)) in AXIS_STAGES.iter().enumerate() {
        let shrink = h1 / h0;
        if b.coarse() {
            pale(b);
        } else {
            dark(b);
        }
        let core = cut_rect(b, h0, h0, h0 * 0.3);
        b.loft_z(&core, &[Section::new(z0, 1.0), Section::new(z1, shrink)]);
        if i > 0 {
            // The necks between the stages are lit: the needle is still working.
            key_light(b);
            let below = AXIS_STAGES[i - 1].1;
            let neck = cut_rect(b, h0 * 0.55, h0 * 0.55, h0 * 0.16);
            b.loft_z(&neck, &[Section::new(below - 2.0, 1.0), Section::new(z0 + 2.0, 1.0)]);
        }
        if b.coarse() {
            continue;
        }
        // Corner piers.
        pale(b);
        let p = h0 * 0.26;
        let pier = fine_rect(b, p, p, p * 0.3);
        let c0 = h0 - p + 2.0;
        let c1 = (h0 - p + 2.0) * shrink;
        for (sx, sy) in [(1.0f32, 1.0f32), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
            b.loft_z(
                &pier,
                &[Section::new(z0 - 0.5, 1.0).shifted(sx * c0, sy * c0), Section::new(z1 + 0.5, shrink).shifted(sx * c1, sy * c1)],
            );
        }
        // Light up each face, pale bands across it.
        let face = |z: f32| h0 + (h1 - h0) * (z - z0) / (z1 - z0);
        let out = v3(z1 - z0, 0.0, h0 - h1).normalize();
        b.radial(4, |b| {
            seam(b, v3(face(z0 + 8.0), 0.0, z0 + 8.0), v3(face(z1 - 8.0), 0.0, z1 - 8.0), out, h0 * 0.08);
            if b.fine() {
                pale(b);
                let mut z = z0 + 160.0;
                while z < z1 - 80.0 {
                    let (za, zb) = (z, z + 10.0);
                    let (wa, wb) = (face(za) - p * 1.5, face(zb) - p * 1.5);
                    let quad = [v3(face(za), -wa, za), v3(face(za), wa, za), v3(face(zb), wb, zb), v3(face(zb), -wb, zb)];
                    panel(b, &quad, out, 1.6, 0.5);
                    z += 150.0;
                }
            }
        });
    }

    // The ring hovering flat over the water: eight segments round the shaft, light
    // under each, a gap of light between them.
    let ring_z = WATERLINE + 70.0;
    let segments = if b.coarse() { 4 } else { 8 };
    for k in 0..segments {
        let a = k as f32 / segments as f32 * std::f32::consts::TAU;
        b.with(Affine3A::from_rotation_z(a), |b| {
            let (r0, r1) = (230.0f32, 300.0f32);
            let half = (std::f32::consts::PI / segments as f32 - 0.05) * r0;
            pale(b);
            b.chamfered_box(v3((r0 + r1) * 0.5, 0.0, ring_z), v3(r1 - r0, half * 2.0, 18.0), 4.0);
            if !b.coarse() {
                key_light(b);
                b.chamfered_box(v3((r0 + r1) * 0.5, 0.0, ring_z - 10.0), v3((r1 - r0) * 0.6, half * 1.6, 2.0), 0.5);
            }
        });
    }

    // Blades standing out of the corners where the shaft leaves the water.
    if b.mid() {
        let blade: [[f32; 2]; 5] = [[70.0, WATERLINE - 120.0], [200.0, WATERLINE - 120.0], [200.0, WATERLINE + 40.0], [110.0, WATERLINE + 420.0], [80.0, WATERLINE + 420.0]];
        b.radial(4, |b| {
            b.with(Affine3A::from_rotation_z(FRAC_PI_4), |b| {
                pale(b);
                b.extrude_y_chamfered(&blade, 8.0, 2.0);
                if b.fine() {
                    for y in [-8.0f32, 8.0] {
                        seam(b, v3(188.0, y, WATERLINE + 30.0), v3(108.0, y, WATERLINE + 396.0), v3(0.0, y.signum(), 0.0), 2.4);
                    }
                }
            });
        });
    }

    // The point, hovering over the last stage.
    let cap = cut_rect(b, 20.0, 20.0, 6.0);
    if !b.coarse() {
        key_light(b);
        b.loft_z(&cap, &[Section::new(4_432.0, 0.3), Section::new(4_446.0, 1.0)]);
    }
    pale(b);
    b.loft_z(&cap, &[Section::new(4_446.0, 1.0), Section::new(4_456.0, 1.0), Section::new(AXIS_TOP, 0.0)]);
}

/// Full-detail triangle budget: three vaults and one axis on the one map.
#[cfg(test)]
pub(super) const TRIANGLES: usize = 60_000;

#[cfg(test)]
mod tests {
    use super::super::build_model;
    use super::*;

    #[test]
    fn crown_builds_within_budget() {
        for def in MODELS {
            let model = build_model(def.key).unwrap();
            let tris: Vec<usize> = model.lods.iter().map(|l| l.indices.len() / 3).collect();
            let low = model.lods[0].vertices.iter().map(|v| v.pos[2]).fold(f32::MAX, f32::min);
            let high = model.lods[0].vertices.iter().map(|v| v.pos[2]).fold(f32::MIN, f32::max);
            println!("{}: triangles {tris:?}, z {low:.0}..{high:.0}, bounds {:.0}", def.key, model.bounds_radius);
            assert!(tris[0] <= TRIANGLES, "{}: {} triangles", def.key, tris[0]);
            assert!(tris[1] < tris[0], "{}: LOD1 not lighter", def.key);
            let floor = if def.key == "precursor_lining" { -LINING_DEPTH - 10.0 } else { -80.0 };
            assert!(low >= floor, "{}: below the footing", def.key);
            for lod in &model.lods {
                for v in &lod.vertices {
                    assert!(v.pos.iter().chain(&v.normal).all(|c| c.is_finite()), "{}", def.key);
                }
            }
        }
    }
}

/// Software previews: `POLAR_DUMP_DIR=... cargo test -p mc-render --lib polar_previews -- --ignored`.
#[cfg(test)]
#[test]
#[ignore]
fn polar_previews() {
    let dir = std::path::PathBuf::from(std::env::var_os("POLAR_DUMP_DIR").expect("POLAR_DUMP_DIR"));
    std::fs::create_dir_all(&dir).unwrap();
    for def in MODELS {
        let model = super::build_model(def.key).unwrap();
        for (lod, yaw) in [(0usize, -38.0f32), (1, -38.0)] {
            super::preview::render(&model.lods[lod], 768, yaw)
                .write_ppm(&dir.join(format!("{}_{lod}_{}.ppm", def.key, yaw as i32)))
                .unwrap();
        }
    }
}
