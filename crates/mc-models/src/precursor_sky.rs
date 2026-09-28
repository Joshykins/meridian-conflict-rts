//! The Threshold's Precursor facility kit, the pieces that stand far above the
//! clouds (`mc_map::PropKind::Precursor{Heart,Halo,Monolith,Needle}`; contracts in
//! mc-map format.rs, PropKind 68..=77): the Heart, the facility's 2.8 km spire whose
//! Lens fires the replication ray; halos and slab islands hovering over the cloud
//! layer; and needles, masts into the sky.
//!
//! Same kit as the megastructure (`precursor_mega.rs`) and the polar axis
//! (`precursor_polar.rs`): pale alloy over a dark core, cold light in the seams,
//! stepped tiers, corner piers, and segments hovering apart over lines of light.
//! Model space as for every prop: x along the heading, y left, z up, the origin on
//! the ground; the floating pieces are authored at their altitude over it.

use std::f32::consts::{FRAC_PI_4, PI, TAU};

use glam::{Affine3A, Vec2, Vec3};

use super::builder::{chamfered_rect, MeshBuilder, Section};
use super::library::ModelDef;
use super::precursor::{cut_rect, dark, film, fine_rect, key_light, light, pale, panel, seam, v3};

pub(super) const MODELS: &[ModelDef] = &[
    ModelDef::new("precursor_heart", 262.0, HEART_TOP, heart),
    ModelDef::new("precursor_halo", 645.0, 1_840.0, halo),
    ModelDef::new("precursor_monolith", 445.0, MONOLITH_TOP, monolith),
    ModelDef::new("precursor_needle", 62.0, NEEDLE_TOP, needle),
];

/// Full-detail triangle budget per model: one Heart, a few of the rest on the one map.
#[cfg(test)]
pub(super) const TRIANGLES: usize = 60_000;

// ---- Kit ----------------------------------------------------------------------------

/// A section of (r, z) pairs swept round the z axis from `a0` to `a1` in `steps`.
/// Every side face is a flat trapezoid. Caps both ends when `caps` (a closed ring
/// sweeps a whole turn without them).
fn sweep(b: &mut MeshBuilder, section: &[[f32; 2]], a0: f32, a1: f32, steps: usize, caps: bool) {
    let rings: Vec<Vec<Vec3>> = (0..=steps)
        .map(|i| {
            let (s, c) = (a0 + (a1 - a0) * i as f32 / steps as f32).sin_cos();
            section
                .iter()
                .map(|q| v3(q[0] * c, q[0] * s, q[1]))
                .collect()
        })
        .collect();
    b.loft(&rings, caps, caps);
}

/// An (r, z) box section, its corners cut except at the coarse level.
fn rz_box(b: &MeshBuilder, r0: f32, r1: f32, z0: f32, z1: f32, chamfer: f32) -> Vec<[f32; 2]> {
    let (mr, mz) = ((r0 + r1) * 0.5, (z0 + z1) * 0.5);
    let half = Vec2::new((r1 - r0) * 0.5, (z1 - z0) * 0.5);
    let plan = if b.coarse() || chamfer <= 0.0 {
        vec![
            [half.x, -half.y],
            [half.x, half.y],
            [-half.x, half.y],
            [-half.x, -half.y],
        ]
    } else {
        chamfered_rect(half, chamfer)
    };
    plan.iter().map(|p| [p[0] + mr, p[1] + mz]).collect()
}

/// How many flat steps an arc of `span` radians at radius `r` is cut into.
fn arc_steps(b: &MeshBuilder, r: f32, span: f32) -> usize {
    let each = if b.fine() {
        34.0
    } else if b.mid() {
        80.0
    } else {
        1e9
    };
    ((r * span / each).ceil() as usize).max(1)
}

/// A ring of `pieces` segments hovering apart round the z axis: the section
/// `r0..r1` across, `h` either side of `z` up, `gap` metres between pieces with a
/// bar of light across each gap. At full detail each piece is pale flanges over a
/// dark core, so its faces show a dark band with a line of light in it, split by
/// pale ribs every `bay` metres (0: none).
#[derive(Clone, Copy)]
struct Hoop {
    r0: f32,
    r1: f32,
    z: f32,
    h: f32,
    pieces: usize,
    gap: f32,
    turn: f32,
    bay: f32,
}

fn hoop(b: &mut MeshBuilder, o: Hoop) {
    let span = TAU / o.pieces as f32;
    let depth = o.r1 - o.r0;
    let rm = (o.r0 + o.r1) * 0.5;
    let half_gap = o.gap * 0.5 / rm;
    let ch = (depth.min(o.h * 2.0) * 0.2).min(7.0);
    for k in 0..o.pieces {
        let a0 = o.turn + span * k as f32 + half_gap;
        let a1 = o.turn + span * (k + 1) as f32 - half_gap;
        let steps = arc_steps(b, o.r1, a1 - a0);
        hoop_piece(b, &o, a0, a1, steps, ch);
        if !b.coarse() {
            // The light across the gap, its ends buried in the pieces either side.
            light(b);
            let bar = rz_box(
                b,
                o.r0 + depth * 0.28,
                o.r1 - depth * 0.28,
                o.z - o.h * 0.34,
                o.z + o.h * 0.34,
                0.0,
            );
            let into = 2.0 / rm;
            sweep(b, &bar, a1 - into, a1 + 2.0 * half_gap + into, 1, true);
        }
    }
}

fn hoop_piece(b: &mut MeshBuilder, o: &Hoop, a0: f32, a1: f32, steps: usize, ch: f32) {
    let (r0, r1, z, h) = (o.r0, o.r1, o.z, o.h);
    let depth = r1 - r0;
    let band = h * 0.34;
    let w = (band * 0.36).clamp(0.8, 5.0);
    if !b.fine() {
        pale(b);
        let whole = rz_box(b, r0, r1, z - h, z + h, ch);
        sweep(b, &whole, a0, a1, steps, true);
        if b.mid() {
            // The line of light round the outer face, and under it.
            let lamp = rz_box(b, r1 - 0.5, r1 + 0.3, z - w, z + w, 0.0);
            sweep(b, &lamp, a0 + 3.0 / r1, a1 - 3.0 / r1, steps, true);
        }
        return;
    }
    let recess = (depth * 0.1).clamp(1.0, 4.0);
    dark(b);
    let core = rz_box(
        b,
        r0 + recess,
        r1 - recess,
        z - band - 0.6,
        z + band + 0.6,
        0.0,
    );
    sweep(b, &core, a0, a1, steps, true);
    // Flanges, chamfered on their outer corners only.
    let fh = h - band;
    let c = ch.min(fh * 0.9);
    let top = [
        [r1, z + band],
        [r1, z + h - c],
        [r1 - c, z + h],
        [r0 + c, z + h],
        [r0, z + h - c],
        [r0, z + band],
    ];
    let bottom: Vec<[f32; 2]> = top.iter().rev().map(|p| [p[0], 2.0 * z - p[1]]).collect();
    pale(b);
    sweep(b, &top, a0, a1, steps, true);
    sweep(b, &bottom, a0, a1, steps, true);
    // Ribs across the band.
    let (mid_len, into) = ((a1 - a0) * r1, 3.0 / r1);
    if o.bay > 0.0 {
        let ribs = (mid_len / o.bay).floor() as usize;
        let rib = rz_box(b, r0 + 0.4, r1 - 0.4, z - band - 0.7, z + band + 0.7, 0.0);
        for i in 1..=ribs {
            let a = a0 + (a1 - a0) * i as f32 / (ribs + 1) as f32;
            sweep(b, &rib, a - 2.0 / r1, a + 2.0 / r1, 1, true);
        }
    }
    // Lines of light down the band, outside and in.
    light(b);
    let outer = rz_box(b, r1 - recess - 0.3, r1 - recess + 0.35, z - w, z + w, 0.0);
    sweep(b, &outer, a0 + into, a1 - into, steps, true);
    let inner = rz_box(b, r0 + recess - 0.35, r0 + recess + 0.3, z - w, z + w, 0.0);
    sweep(b, &inner, a0 + into, a1 - into, steps, true);
}

/// One stage of a shaft, as the mega tower's: a dark core tapering from `h0` to `h1`
/// half across between pale corner piers, light up the middle of every face, pale
/// bands across the dark every `bands` metres.
fn stage(b: &mut MeshBuilder, z0: f32, z1: f32, h0: f32, h1: f32, bands: f32) {
    let shrink = h1 / h0;
    if b.coarse() {
        pale(b);
    } else {
        dark(b);
    }
    let core = cut_rect(b, h0, h0, h0 * 0.28);
    b.loft_z(&core, &[Section::new(z0, 1.0), Section::new(z1, shrink)]);
    if b.coarse() {
        return;
    }
    pale(b);
    let p = h0 * 0.24;
    let pier = fine_rect(b, p, p, p * 0.3);
    let c0 = h0 - p + h0 * 0.04;
    let c1 = c0 * shrink;
    for (sx, sy) in [(1.0f32, 1.0f32), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
        b.loft_z(
            &pier,
            &[
                Section::new(z0 - 0.5, 1.0).shifted(sx * c0, sy * c0),
                Section::new(z1 + 0.5, shrink).shifted(sx * c1, sy * c1),
            ],
        );
    }
    let face = |z: f32| h0 + (h1 - h0) * (z - z0) / (z1 - z0);
    let out = v3(z1 - z0, 0.0, h0 - h1).normalize();
    b.radial(4, |b| {
        seam(
            b,
            v3(face(z0 + 6.0), 0.0, z0 + 6.0),
            v3(face(z1 - 6.0), 0.0, z1 - 6.0),
            out,
            h0 * 0.08,
        );
        if b.fine() {
            pale(b);
            let mut z = z0 + bands * 0.6;
            while z < z1 - bands * 0.4 {
                let (za, zb) = (z, z + h0 * 0.14);
                let (wa, wb) = (face(za) - p * 1.5, face(zb) - p * 1.5);
                let quad = [
                    v3(face(za), -wa, za),
                    v3(face(za), wa, za),
                    v3(face(zb), wb, zb),
                    v3(face(zb), -wb, zb),
                ];
                panel(b, &quad, out, h0 * 0.03, h0 * 0.008);
                // Short lines of light either side of the middle one, under each band.
                let (zc, zd) = (za - bands * 0.34, za - 3.0);
                for y in [-0.45f32, 0.45] {
                    seam(
                        b,
                        v3(face(zc), y * face(zc), zc),
                        v3(face(zd), y * face(zd), zd),
                        out,
                        h0 * 0.035,
                    );
                }
                z += bands;
            }
        }
    });
}

/// A neck of light between two stages.
fn neck(b: &mut MeshBuilder, z0: f32, z1: f32, h: f32) {
    light(b);
    let plan = cut_rect(b, h, h, h * 0.3);
    b.loft_z(
        &plan,
        &[Section::new(z0 - 2.0, 1.0), Section::new(z1 + 2.0, 1.0)],
    );
}

/// Blades standing out of the four corners of a stage, `reach` proud of them at their
/// widest, with lines of light down both flanks near the edge.
fn corner_fins(b: &mut MeshBuilder, z0: f32, z1: f32, h0: f32, h1: f32, reach: f32, half: f32) {
    let (c0, c1) = (h0 * 1.12, h1 * 1.12);
    let (za, zb) = (z0 + (z1 - z0) * 0.12, z1 - (z1 - z0) * 0.2);
    let fin: [[f32; 2]; 5] = [
        [c0 - half * 2.0, z0 + 4.0],
        [c0 + reach * 0.3, z0 + 4.0],
        [c0 + reach, za],
        [c1 + reach * 0.55, zb],
        [c1 - half * 2.0, z1 - 6.0],
    ];
    let edge = [(v3(fin[2][0], 0.0, fin[2][1]), v3(fin[3][0], 0.0, fin[3][1]))];
    b.radial(4, |b| {
        b.with(Affine3A::from_rotation_z(FRAC_PI_4), |b| {
            pale(b);
            b.extrude_y_chamfered(&fin, half, half * 0.3);
            if b.fine() {
                for (a, c) in edge {
                    for s in [-1.0f32, 1.0] {
                        let d = v3(-(half * 1.4), s * half, 0.0);
                        seam(
                            b,
                            a + d + v3(0.0, 0.0, 10.0),
                            c + d - v3(0.0, 0.0, 10.0),
                            v3(0.0, s, 0.0),
                            half * 0.5,
                        );
                    }
                }
            }
        });
    });
}

/// The point over a shaft, hovering over its last stage on a light (`live`: a working one).
fn point(b: &mut MeshBuilder, z0: f32, top: f32, half: f32, live: bool) {
    let cap = cut_rect(b, half, half, half * 0.3);
    if !b.coarse() {
        if live {
            key_light(b);
        } else {
            light(b);
        }
        b.loft_z(
            &cap,
            &[Section::new(z0 - 12.0, 0.25), Section::new(z0, 1.0)],
        );
    }
    pale(b);
    b.loft_z(
        &cap,
        &[
            Section::new(z0, 1.0),
            Section::new(z0 + half * 0.6, 1.0),
            Section::new(top, 0.0),
        ],
    );
}

/// A stepped course of a battered base: `hb` half across at `z0` to `ht` at `z1`,
/// its corners cut `ch`, with a line of light round its nosing; on every face a dark
/// band with a line of light in it and pale pilasters standing over it every `bay`
/// metres, and, if `portal`, a tall dark portal lit up its middle in each face.
fn course(b: &mut MeshBuilder, z0: f32, z1: f32, hb: f32, ht: f32, ch: f32, bay: f32, portal: f32) {
    pale(b);
    let plan = cut_rect(b, hb, hb, ch);
    b.loft_z(
        &plan,
        &[Section::new(z0 - 1.0, 1.0), Section::new(z1, ht / hb)],
    );
    if b.coarse() {
        return;
    }
    let half = |z: f32| hb + (ht - hb) * (z - z0) / (z1 - z0);
    light(b);
    let nose = |z: f32| Section::new(z, (half(z) + 0.35) / hb);
    b.loft_z(&plan, &[nose(z1 - 2.6), nose(z1 - 0.8)]);
    let out = v3(z1 - z0, 0.0, hb - ht).normalize();
    let flat = |z: f32| half(z) * (1.0 - ch / hb);
    let p = |y: f32, z: f32| v3(half(z), y, z);
    let fine = b.fine();
    b.radial(4, |b| {
        let (za, zb) = (z0 + (z1 - z0) * 0.4, z0 + (z1 - z0) * 0.64);
        let w = flat(zb) - 4.0;
        dark(b);
        panel(
            b,
            &[p(-w, za), p(w, za), p(w, zb), p(-w, zb)],
            out,
            0.5,
            0.0,
        );
        let zm = (za + zb) * 0.5;
        seam(
            b,
            p(-w + 5.0, zm) + out * 0.5,
            p(w - 5.0, zm) + out * 0.5,
            out,
            (zb - za) * 0.26,
        );
        if portal > 0.0 {
            let (pa, pb) = (z0 + 5.0, z1 - (z1 - z0) * 0.14);
            dark(b);
            panel(
                b,
                &[p(-portal, pa), p(portal, pa), p(portal, pb), p(-portal, pb)],
                out,
                1.2,
                0.0,
            );
            let o = out * 1.2;
            seam(
                b,
                p(0.0, pa + 6.0) + o,
                p(0.0, pb - 5.0) + o,
                out,
                portal * 0.22,
            );
            seam(b, p(-portal, pb + 4.0), p(portal, pb + 4.0), out, 2.4);
            for y in [-portal - 5.0, portal + 5.0] {
                seam(b, p(y, pa + 2.0), p(y, pb + 4.0), out, 1.6);
            }
        }
        if fine && bay > 0.0 {
            pale(b);
            let n = (2.0 * w / bay).floor() as i32;
            for i in 0..=n {
                let y = -w + 2.0 * w * i as f32 / n.max(1) as f32;
                if portal > 0.0 && y.abs() < portal + 14.0 {
                    continue;
                }
                let (y0, y1) = (y - 3.5, y + 3.5);
                let (pa, pb) = (z0 + 2.0, z1 - 4.0);
                panel(
                    b,
                    &[p(y0, pa), p(y1, pa), p(y1, pb), p(y0, pb)],
                    out,
                    2.2,
                    0.6,
                );
            }
        }
    });
}

/// A dark course with a line of light round it, between two stepped courses.
fn joint(b: &mut MeshBuilder, z0: f32, z1: f32, half: f32, ch: f32) {
    if b.coarse() {
        return;
    }
    dark(b);
    let plan = cut_rect(b, half, half, ch);
    b.loft_z(
        &plan,
        &[Section::new(z0 - 0.5, 1.0), Section::new(z1 + 0.5, 1.0)],
    );
    light(b);
    let zm = (z0 + z1) * 0.5;
    let s = (half + 0.3) / half;
    b.loft_z(
        &plan,
        &[Section::new(zm - 1.3, s), Section::new(zm + 1.3, s)],
    );
}

// ---- Heart --------------------------------------------------------------------------
//
// The facility's centrepiece, 2800 m. A plinth 420 m square, four battered courses
// stepping in to 300 m, each with a line of light along its nosing and a dark band
// lit along every face, portals in the first. Four great blades climb out of the
// plinth's corners, lean in to the shaft and stand up as the four pillars of the
// Lens: an open cage 600-860 m up, where the shaft breaks and a core of light hangs
// between the pillars, three rings of hovering segments round it in the shape of a
// lens (the replication ray fires from the core, `HEART_LENS`). Over the Lens the
// shaft climbs on in four stages, each hovering over the one below on a neck of light
// with a collar of segments floating round it, blades out of the corners of the lower
// two, to a point lit beneath.

pub(super) const HEART_TOP: f32 = 2_800.0;
/// The Lens's light core: where the replication ray leaves from.
pub(super) const HEART_LENS: Vec3 = Vec3::new(0.0, 0.0, 720.0);
/// The Lens's main ring, outer radius.
pub(super) const HEART_LENS_RADIUS: f32 = 264.0;
/// Heights of the collars floating round the shaft's necks.
pub(super) const HEART_COLLARS: [f32; 3] = [1_400.0, 2_000.0, 2_550.0];

const HEART_PLINTH: f32 = 210.0;
/// (z0, z1, half at z0, half at z1) of the stepped courses.
const HEART_COURSES: [(f32, f32, f32, f32); 4] = [
    (16.0, 92.0, 194.0, 182.0),
    (106.0, 172.0, 160.0, 150.0),
    (186.0, 244.0, 128.0, 120.0),
    (258.0, 300.0, 104.0, 96.0),
];
/// Where the shaft breaks for the Lens.
const LENS_FLOOR: f32 = 600.0;
const LENS_ROOF: f32 = 860.0;
/// (z0, z1, half at z0, half at z1) of the shaft's stages, the first under the Lens.
const HEART_STAGES: [(f32, f32, f32, f32); 5] = [
    (300.0, LENS_FLOOR, 80.0, 72.0),
    (LENS_ROOF, 1_380.0, 66.0, 54.0),
    (1_420.0, 1_980.0, 50.0, 40.0),
    (2_020.0, 2_530.0, 36.0, 26.0),
    (2_570.0, 2_716.0, 22.0, 12.0),
];

fn heart(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        // The base, the shaft, the Lens's light.
        pale(b);
        let base = cut_rect(b, HEART_PLINTH, HEART_PLINTH, 0.0);
        b.loft_z(
            &base,
            &[
                Section::new(-80.0, 1.0),
                Section::new(0.0, 1.0),
                Section::new(300.0, 0.46),
            ],
        );
        let shaft = cut_rect(b, 80.0, 80.0, 0.0);
        b.loft_z(
            &shaft,
            &[Section::new(299.0, 1.0), Section::new(HEART_TOP, 0.05)],
        );
        let lens = cut_rect(b, HEART_LENS_RADIUS * 0.8, HEART_LENS_RADIUS * 0.8, 0.0);
        b.loft_z(
            &lens,
            &[
                Section::new(HEART_LENS.z - 16.0, 1.0),
                Section::new(HEART_LENS.z + 16.0, 1.0),
            ],
        );
        key_light(b);
        let core = cut_rect(b, 90.0, 90.0, 0.0);
        b.loft_z(
            &core,
            &[
                Section::new(HEART_LENS.z - 60.0, 1.0),
                Section::new(HEART_LENS.z + 60.0, 1.0),
            ],
        );
        return;
    }

    // The plinth, the courses, and the joints between them.
    pale(b);
    let plan = cut_rect(b, HEART_PLINTH, HEART_PLINTH, 62.0);
    b.loft_z(
        &plan,
        &[
            Section::new(-80.0, 1.0),
            Section::new(0.0, 1.0),
            Section::new(16.0, 0.986),
        ],
    );
    light(b);
    b.loft_z(
        &plan,
        &[Section::new(11.0, 0.9905), Section::new(13.0, 0.9895)],
    );
    let mut below = 16.0;
    for (i, &(z0, z1, hb, ht)) in HEART_COURSES.iter().enumerate() {
        joint(b, below, z0, hb - 5.0, hb * 0.28 - 2.0);
        let portal = if i < 2 { 34.0 - 10.0 * i as f32 } else { 0.0 };
        course(b, z0, z1, hb, ht, hb * 0.28, 34.0, portal);
        below = z1;
    }

    // Fins flanking the portals, climbing the first two courses.
    let fin: [[f32; 2]; 6] = [
        [120.0, 0.0],
        [208.0, 0.0],
        [208.0, 18.0],
        [164.0, 176.0],
        [150.0, 182.0],
        [120.0, 182.0],
    ];
    b.radial(4, |b| {
        for y in [-74.0f32, 74.0] {
            b.with(Affine3A::from_translation(v3(0.0, y, 0.0)), |b| {
                pale(b);
                b.extrude_y_chamfered(&fin, 5.0, 1.6);
                if b.fine() {
                    for s in [-5.0f32, 5.0] {
                        seam(
                            b,
                            v3(198.0, s, 30.0),
                            v3(166.0, s, 160.0),
                            v3(0.0, s.signum(), 0.0),
                            1.8,
                        );
                    }
                }
            });
        }
    });

    // The great blades: out of the plinth's corners, in to the shaft, up round the Lens.
    let blade: [[f32; 2]; 9] = [
        [60.0, -60.0],
        [250.0, -60.0],
        [250.0, 22.0],
        [214.0, 140.0],
        [168.0, 270.0],
        [134.0, 400.0],
        [130.0, 880.0],
        [96.0, 900.0],
        [60.0, 900.0],
    ];
    b.radial(4, |b| {
        b.with(Affine3A::from_rotation_z(FRAC_PI_4), |b| {
            pale(b);
            b.extrude_y_chamfered(&blade, 9.0, 2.6);
            for s in [-9.0f32, 9.0] {
                let side = v3(0.0, s.signum(), 0.0);
                let at = |r: f32, z: f32| v3(r, s, z);
                seam(b, at(236.0, 34.0), at(206.0, 150.0), side, 2.6);
                if b.fine() {
                    seam(b, at(202.0, 172.0), at(166.0, 262.0), side, 2.6);
                    seam(b, at(158.0, 300.0), at(128.0, 404.0), side, 2.6);
                    // Up the pillar, round the Lens.
                    seam(b, at(118.0, 430.0), at(118.0, 860.0), side, 3.2);
                    seam(
                        b,
                        at(72.0, LENS_FLOOR + 30.0),
                        at(72.0, LENS_ROOF - 30.0),
                        side,
                        2.4,
                    );
                }
            }
            // The pillar's inner edge, lit toward the core.
            if b.mid() {
                light(b);
                let lamp = [
                    v3(59.6, -4.0, LENS_FLOOR + 10.0),
                    v3(59.6, 4.0, LENS_FLOOR + 10.0),
                    v3(59.6, 4.0, LENS_ROOF - 10.0),
                    v3(59.6, -4.0, LENS_ROOF - 10.0),
                ];
                film(b, &lamp, -Vec3::X, 0.05);
            }
        });
    });

    // The shaft.
    for (i, &(z0, z1, h0, h1)) in HEART_STAGES.iter().enumerate() {
        stage(b, z0, z1, h0, h1, 110.0 - 14.0 * i as f32);
        if i >= 2 {
            let (_, under, _, top) = HEART_STAGES[i - 1];
            neck(b, under, z0, top * 0.5);
        }
    }
    if b.mid() {
        let (z0, z1, h0, h1) = HEART_STAGES[1];
        corner_fins(b, z0, z1, h0, h1, 64.0, 6.0);
        let (z0, z1, h0, h1) = HEART_STAGES[2];
        corner_fins(b, z0, z1, h0, h1, 44.0, 4.5);
        let (z0, z1, h0, h1) = HEART_STAGES[3];
        corner_fins(b, z0, z1, h0, h1, 26.0, 3.2);
    }
    point(b, 2_728.0, HEART_TOP, 13.0, true);

    // Where the shaft leaves the base, four plates hovering between the blades.
    if b.mid() {
        hoop(
            b,
            Hoop {
                r0: 112.0,
                r1: 146.0,
                z: 334.0,
                h: 9.0,
                pieces: 4,
                gap: 150.0,
                turn: FRAC_PI_4,
                bay: 20.0,
            },
        );
    }

    // The collars, floating round the necks.
    let collars = [
        (HEART_COLLARS[0], 96.0, 150.0, 13.0),
        (HEART_COLLARS[1], 72.0, 114.0, 10.0),
        (HEART_COLLARS[2], 50.0, 82.0, 8.0),
    ];
    for (k, &(z, r0, r1, h)) in collars.iter().enumerate() {
        let turn = if k % 2 == 0 { PI / 8.0 } else { 0.0 };
        hoop(
            b,
            Hoop {
                r0,
                r1,
                z,
                h,
                pieces: 8,
                gap: 6.0 + h * 0.3,
                turn,
                bay: 26.0,
            },
        );
        if b.mid() {
            let (a, c) = (r1 + (r1 - r0) * 0.28, r1 + (r1 - r0) * 0.62);
            hoop(
                b,
                Hoop {
                    r0: a,
                    r1: c,
                    z: z - h * 0.1,
                    h: h * 0.42,
                    pieces: 16,
                    gap: 4.0 + h * 0.4,
                    turn: turn + PI / 16.0,
                    bay: 0.0,
                },
            );
        }
    }

    lens(b);
}

/// The Lens: a core of light in the open cage, rings of hovering segments round it.
fn lens(b: &mut MeshBuilder) {
    let c = HEART_LENS;
    // The stage under it ends in a pale cap lit on top; the stage over it starts on
    // a pale drum lit beneath: both look at the core.
    let (_, _, _, h) = HEART_STAGES[0];
    let floor = cut_rect(b, h, h, h * 0.28);
    pale(b);
    b.loft_z(
        &floor,
        &[
            Section::new(LENS_FLOOR - 1.0, 1.0),
            Section::new(LENS_FLOOR + 10.0, 0.86),
            Section::new(LENS_FLOOR + 16.0, 0.7),
        ],
    );
    let (_, _, h, _) = HEART_STAGES[1];
    let roof = cut_rect(b, h, h, h * 0.28);
    b.loft_z(
        &roof,
        &[
            Section::new(LENS_ROOF - 16.0, 0.7),
            Section::new(LENS_ROOF - 10.0, 0.86),
            Section::new(LENS_ROOF + 1.0, 1.0),
        ],
    );
    light(b);
    let pupil = cut_rect(b, 40.0, 40.0, 12.0);
    b.loft_z(
        &pupil,
        &[
            Section::new(LENS_FLOOR + 15.0, 1.0),
            Section::new(LENS_FLOOR + 17.5, 0.9),
        ],
    );
    b.loft_z(
        &pupil,
        &[
            Section::new(LENS_ROOF - 17.5, 0.9),
            Section::new(LENS_ROOF - 15.0, 1.0),
        ],
    );

    // The core: a spindle of live light (the Heart's one working part, where the ray
    // leaves), and the shards hovering round it.
    key_light(b);
    b.spheroid(c, v3(42.0, 42.0, 76.0), 4, 2);
    if b.mid() {
        // The column of light it hangs on, floor to roof.
        let column = cut_rect(b, 5.0, 5.0, 1.5);
        b.loft_z(
            &column,
            &[
                Section::new(LENS_FLOOR + 16.0, 1.0),
                Section::new(LENS_ROOF - 16.0, 1.0),
            ],
        );
    }
    b.radial(4, |b| {
        pale(b);
        let shard = [
            [62.0, c.z],
            [72.0, c.z + 50.0],
            [80.0, c.z + 42.0],
            [80.0, c.z - 42.0],
            [72.0, c.z - 50.0],
        ];
        b.extrude_y_chamfered(&shard, 9.0, 2.4);
        if b.mid() {
            light(b);
            b.chamfered_box(v3(64.6, 0.0, c.z), v3(4.0, 10.0, 50.0), 1.0);
        }
    });

    // The rings: small above and below, the great one between, turned against each other.
    let side = Hoop {
        r0: 150.0,
        r1: 192.0,
        z: c.z - 58.0,
        h: 9.0,
        pieces: 8,
        gap: 12.0,
        turn: PI / 8.0,
        bay: 22.0,
    };
    hoop(b, side);
    hoop(
        b,
        Hoop {
            z: c.z + 58.0,
            ..side
        },
    );
    hoop(
        b,
        Hoop {
            r0: 200.0,
            r1: HEART_LENS_RADIUS,
            z: c.z,
            h: 19.0,
            pieces: 12,
            gap: 16.0,
            turn: 0.0,
            bay: 24.0,
        },
    );
    if !b.mid() {
        return;
    }
    // A fine ring of live light inside the great one, between the pillars.
    key_light(b);
    let lamp = rz_box(b, 186.0, 190.0, c.z - 1.6, c.z + 1.6, 0.0);
    for k in 0..4 {
        let a = FRAC_PI_4 + k as f32 * PI * 0.5;
        sweep(
            b,
            &lamp,
            a + 0.08,
            a + PI * 0.5 - 0.08,
            arc_steps(b, 190.0, PI * 0.5),
            true,
        );
    }
    // Keys hovering off the great ring at every gap, lit on their inner faces.
    for k in 0..12 {
        let a = k as f32 * TAU / 12.0;
        b.with(Affine3A::from_rotation_z(a), |b| {
            pale(b);
            let key = [
                [276.0, c.z],
                [286.0, c.z + 44.0],
                [298.0, c.z + 4.0],
                [298.0, c.z - 4.0],
                [286.0, c.z - 44.0],
            ];
            b.extrude_y_chamfered(&key, 7.0, 2.0);
            light(b);
            b.chamfered_box(v3(276.5, 0.0, c.z), v3(2.0, 8.0, 34.0), 0.6);
        });
    }
}

// ---- Halo ---------------------------------------------------------------------------
//
// A ring 1200 m across hovering 1500 m up, tilted 30 degrees about x: 24 segments,
// each 40 m through and 60 m high, the dark band round its faces lit and ribbed, bars
// of light across the gaps; clasps hover off every segment's outer face; a fine ring
// of light hangs inside it, and a thinner pale ring over and under its inner edge.

pub(super) const HALO_Z: f32 = 1_500.0;
pub(super) const HALO_RADIUS: f32 = 600.0;
pub(super) const HALO_TILT: f32 = 30.0 * PI / 180.0;
/// Nothing of the halo is lower than this at scale 1.
#[cfg(test)]
pub(super) const HALO_FLOOR: f32 = 1_120.0;

fn halo(b: &mut MeshBuilder, _tech: u8) {
    let at =
        Affine3A::from_translation(v3(0.0, 0.0, HALO_Z)) * Affine3A::from_rotation_x(HALO_TILT);
    b.with(at, |b| {
        let (r0, r1) = (HALO_RADIUS - 20.0, HALO_RADIUS + 20.0);
        if b.coarse() {
            pale(b);
            let ring = rz_box(b, r0, r1, -30.0, 30.0, 0.0);
            sweep(b, &ring, 0.0, TAU, 6, false);
            return;
        }
        hoop(
            b,
            Hoop {
                r0,
                r1,
                z: 0.0,
                h: 30.0,
                pieces: 24,
                gap: 12.0,
                turn: 0.0,
                bay: 30.0,
            },
        );
        // The ring of live light inside it: the halo's one working part.
        key_light(b);
        let lamp = rz_box(b, r0 - 32.4, r0 - 30.0, -1.5, 1.5, 0.0);
        sweep(b, &lamp, 0.0, TAU, b.sides(96), false);
        // Thin pale rings hovering over and under the inner edge, in 48 pieces.
        for z in [-37.0f32, 37.0] {
            hoop(
                b,
                Hoop {
                    r0: r0 - 6.0,
                    r1: r0 + 12.0,
                    z,
                    h: 2.6,
                    pieces: 48,
                    gap: 5.0,
                    turn: PI / 48.0,
                    bay: 0.0,
                },
            );
        }
        // Clasps off every segment's outer face, lit where they face it.
        for k in 0..24 {
            let a = (k as f32 + 0.5) * TAU / 24.0;
            b.with(Affine3A::from_rotation_z(a), |b| {
                pale(b);
                b.chamfered_box(v3(r1 + 14.0, 0.0, 0.0), v3(14.0, 44.0, 26.0), 3.0);
                light(b);
                film(
                    b,
                    &[
                        v3(r1 + 6.9, -16.0, -6.0),
                        v3(r1 + 6.9, 16.0, -6.0),
                        v3(r1 + 6.9, 16.0, 6.0),
                        v3(r1 + 6.9, -16.0, 6.0),
                    ],
                    -Vec3::X,
                    0.05,
                );
                if b.fine() {
                    pale(b);
                    for s in [-1.0f32, 1.0] {
                        b.chamfered_box(v3(r1 + 12.0, s * 34.0, 0.0), v3(10.0, 14.0, 16.0), 2.0);
                    }
                    seam(
                        b,
                        v3(r1 + 21.0, -16.0, 0.0),
                        v3(r1 + 21.0, 16.0, 0.0),
                        Vec3::X,
                        3.0,
                    );
                }
            });
        }
    });
}

// ---- Monolith -------------------------------------------------------------------------
//
// A slab island hovering 760-1004 m up, 520 m along x and 200 m across: a slab over a
// dark course lit round, its long flanks faced with plates hovering off a line of
// light; a keel stepping in under it in four courses to its foot at 760 m, strips of
// light in every underside; on the panelled deck, terraces stepping up astern to a
// block with a cap hovering over it, and a procession of three frames forward, the
// middle one great, a lit keystone in its window; blocks trail it astern and hover
// beside the keel.

pub(super) const MONOLITH_KEEL: f32 = 760.0;
pub(super) const MONOLITH_TOP: f32 = 1_004.0;
pub(super) const MONOLITH_HALF: (f32, f32) = (260.0, 100.0);
const SLAB: (f32, f32) = (872.0, 914.0);
const DECK: f32 = 932.0;
/// (z0, z1, half x, half y) of the keel's courses, top down.
const KEEL: [(f32, f32, f32, f32); 4] = [
    (842.0, 868.0, 214.0, 80.0),
    (810.0, 838.0, 160.0, 60.0),
    (784.0, 806.0, 106.0, 40.0),
    (MONOLITH_KEEL, 780.0, 56.0, 22.0),
];

fn monolith(b: &mut MeshBuilder, _tech: u8) {
    let (hx, hy) = MONOLITH_HALF;
    let slab = cut_rect(b, hx, hy, 34.0);
    pale(b);
    if b.coarse() {
        b.loft_z(
            &slab,
            &[
                Section::new(SLAB.0, 0.94),
                Section::new(SLAB.0 + 10.0, 1.0),
                Section::new(DECK, 0.97),
            ],
        );
        let keel = cut_rect(b, KEEL[0].2, KEEL[0].3, 0.0);
        b.loft_z(
            &keel,
            &[
                Section::new(MONOLITH_KEEL, 0.26),
                Section::new(SLAB.0 + 1.0, 1.0),
            ],
        );
        let top = cut_rect(b, 150.0, 70.0, 0.0);
        b.loft_z(
            &top,
            &[
                Section::new(DECK - 1.0, 1.0).shifted(-70.0, 0.0),
                Section::new(975.0, 0.4).shifted(-146.0, 0.0),
            ],
        );
        return;
    }
    // The slab, the course over it, the deck.
    b.loft_z(
        &slab,
        &[
            Section::new(SLAB.0, 0.94),
            Section::new(SLAB.0 + 10.0, 1.0),
            Section::new(SLAB.1, 1.0),
        ],
    );
    joint_xy(b, SLAB.1, SLAB.1 + 5.0, hx - 6.0, hy - 6.0, 30.0);
    pale(b);
    let deck = cut_rect(b, hx - 4.0, hy - 4.0, 32.0);
    b.loft_z(
        &deck,
        &[Section::new(SLAB.1 + 5.0, 1.0), Section::new(DECK, 0.985)],
    );
    light(b);
    let nose = |z: f32| Section::scaled(z, (hx - 3.4) / (hx - 4.0), (hy - 3.4) / (hy - 4.0));
    b.loft_z(&deck, &[nose(DECK - 3.0), nose(DECK - 1.2)]);

    // The long flanks: a dark face lit along its middle, plates hovering off it.
    let (za, zb) = (SLAB.0 + 8.0, SLAB.1 - 4.0);
    b.mirror_y(|b| {
        let (x, y) = (hx - 38.0, hy);
        dark(b);
        panel(
            b,
            &[v3(-x, y, za), v3(x, y, za), v3(x, y, zb), v3(-x, y, zb)],
            Vec3::Y,
            0.5,
            0.0,
        );
        seam(
            b,
            v3(-x + 4.0, y + 0.5, (za + zb) * 0.5),
            v3(x - 4.0, y + 0.5, (za + zb) * 0.5),
            Vec3::Y,
            3.6,
        );
        pale(b);
        if b.fine() {
            let n = 10;
            let each = 2.0 * (x - 4.0) / n as f32;
            for i in 0..n {
                let cx = -x + 4.0 + each * (i as f32 + 0.5);
                b.chamfered_box(
                    v3(cx, y + 6.5, (za + zb) * 0.5),
                    v3(each - 7.0, 3.0, zb - za - 2.0),
                    1.0,
                );
                // Light under each plate's foot.
                seam(
                    b,
                    v3(cx - each * 0.3, y + 0.6, za + 3.0),
                    v3(cx + each * 0.3, y + 0.6, za + 3.0),
                    Vec3::Y,
                    1.6,
                );
            }
        } else {
            b.chamfered_box(
                v3(0.0, y + 6.5, (za + zb) * 0.5),
                v3(2.0 * x - 8.0, 3.0, zb - za - 2.0),
                1.0,
            );
        }
    });
    // The ends: a dark band lit along, pilasters over it.
    for flip in [0.0f32, PI] {
        b.with(Affine3A::from_rotation_z(flip), |b| {
            let (x, w) = (hx, hy - 40.0);
            dark(b);
            panel(
                b,
                &[
                    v3(x, -w, 884.0),
                    v3(x, w, 884.0),
                    v3(x, w, 902.0),
                    v3(x, -w, 902.0),
                ],
                Vec3::X,
                0.5,
                0.0,
            );
            seam(
                b,
                v3(x + 0.5, -w + 6.0, 893.0),
                v3(x + 0.5, w - 6.0, 893.0),
                Vec3::X,
                4.2,
            );
            if b.fine() {
                pale(b);
                for i in 0..=4 {
                    let y = -w + 2.0 * w * i as f32 / 4.0;
                    panel(
                        b,
                        &[
                            v3(x, y - 3.0, SLAB.0 + 12.0),
                            v3(x, y + 3.0, SLAB.0 + 12.0),
                            v3(x, y + 3.0, SLAB.1 - 3.0),
                            v3(x, y - 3.0, SLAB.1 - 3.0),
                        ],
                        Vec3::X,
                        2.0,
                        0.5,
                    );
                }
            }
        });
    }

    // The keel, stepping in under it; strips of light in every underside.
    let mut above = (SLAB.0, hx * 0.94, hy * 0.94);
    for (i, &(z0, z1, kx, ky)) in KEEL.iter().enumerate() {
        joint_xy(b, z1, above.0, kx - 5.0, ky - 5.0, (ky * 0.3).min(24.0));
        pale(b);
        let plan = cut_rect(b, kx, ky, (ky * 0.34).min(28.0));
        b.loft_z(
            &plan,
            &[
                Section::new(z0, 0.93),
                Section::new(z0 + 6.0, 1.0),
                Section::new(z1 + 0.5, 1.0),
            ],
        );
        light(b);
        let s = |z: f32| Section::scaled(z, (kx + 0.4) / kx, (ky + 0.4) / ky);
        b.loft_z(&plan, &[s(z0 + 7.0), s(z0 + 9.0)]);
        // The underside of the course above, round this one: strips of light along it
        // (where they pass over this course they are buried in the joint).
        let (ux, uy, uz) = (above.1 - 14.0, above.2 - 12.0, above.0 - 0.1);
        let down = -Vec3::Z;
        let strip = |b: &mut MeshBuilder, y0: f32, y1: f32| {
            film(
                b,
                &[
                    v3(-ux, y0, uz),
                    v3(ux, y0, uz),
                    v3(ux, y1, uz),
                    v3(-ux, y1, uz),
                ],
                down,
                0.0,
            );
        };
        let pitch = if b.fine() { 12.0 } else { 36.0 };
        let rows = (uy / pitch).floor() as i32;
        light(b);
        for j in -rows..=rows {
            let y = j as f32 * pitch;
            strip(b, y - 2.5, y + 2.5);
        }
        if b.fine() {
            // Pilasters down the course's long flanks.
            pale(b);
            let n = (kx / 26.0).floor() as i32;
            for side in [-1.0f32, 1.0] {
                for j in -n..=n {
                    let x = j as f32 * kx / (n as f32 + 0.5);
                    let y = side * ky;
                    panel(
                        b,
                        &[
                            v3(x - 2.5, y, z0 + 10.0),
                            v3(x + 2.5, y, z0 + 10.0),
                            v3(x + 2.5, y, z1 - 2.0),
                            v3(x - 2.5, y, z1 - 2.0),
                        ],
                        Vec3::Y * side,
                        1.6,
                        0.4,
                    );
                }
            }
        }
        if i == KEEL.len() - 1 {
            // The keel's foot: one great panel of light.
            light(b);
            film(
                b,
                &[
                    v3(-kx * 0.8, -ky * 0.6, z0 - 0.1),
                    v3(kx * 0.8, -ky * 0.6, z0 - 0.1),
                    v3(kx * 0.8, ky * 0.6, z0 - 0.1),
                    v3(-kx * 0.8, ky * 0.6, z0 - 0.1),
                ],
                down,
                0.0,
            );
        }
        above = (z0, kx * 0.93, ky * 0.93);
    }

    // The deck, laid in plates over the dark with light down three of its joints.
    if b.fine() {
        let (dx, dy) = (hx - 22.0, hy - 16.0);
        let z = DECK - 0.2;
        dark(b);
        film(
            b,
            &[
                v3(-dx, -dy, z),
                v3(dx, -dy, z),
                v3(dx, dy, z),
                v3(-dx, dy, z),
            ],
            Vec3::Z,
            0.25,
        );
        pale(b);
        let (nx, ny) = (12, 4);
        let (cx, cy) = (2.0 * dx / nx as f32, 2.0 * dy / ny as f32);
        for i in 0..nx {
            for j in 0..ny {
                let (x0, y0) = (-dx + cx * i as f32 + 1.8, -dy + cy * j as f32 + 1.8);
                let (x1, y1) = (x0 + cx - 3.6, y0 + cy - 3.6);
                panel(
                    b,
                    &[v3(x0, y0, z), v3(x1, y0, z), v3(x1, y1, z), v3(x0, y1, z)],
                    Vec3::Z,
                    1.0,
                    0.8,
                );
            }
        }
        for y in [-cy, cy] {
            seam(
                b,
                v3(-dx + 4.0, y, z + 0.3),
                v3(dx - 4.0, y, z + 0.3),
                Vec3::Z,
                1.8,
            );
        }
    }
    // The lit way down the middle, from the terraces to the prow.
    seam(
        b,
        v3(-12.0, 0.0, DECK + 0.9),
        v3(hx - 18.0, 0.0, DECK + 0.9),
        Vec3::Z,
        5.0,
    );

    // Terraces stepping up astern, lit along their nosings, a dark band round each.
    let terraces = [
        (-70.0f32, 150.0f32, 72.0f32, 946.0f32),
        (-110.0, 104.0, 58.0, 960.0),
        (-146.0, 62.0, 44.0, 974.0),
        (-160.0, 30.0, 26.0, 990.0),
    ];
    let mut floor = DECK;
    for (x, lx, ly, z) in terraces {
        pale(b);
        let plan = cut_rect(b, lx, ly, ly * 0.32);
        let at = |z: f32, s: f32| Section::new(z, s).shifted(x, 0.0);
        b.loft_z(&plan, &[at(floor - 1.0, 1.0), at(z, 0.98)]);
        light(b);
        b.loft_z(&plan, &[at(z - 2.4, 0.9865), at(z - 0.8, 0.9845)]);
        if b.mid() {
            dark(b);
            let m = (floor + z) * 0.5;
            b.loft_z(&plan, &[at(m - 2.5, 0.9925), at(m + 2.5, 0.9905)]);
        }
        floor = z;
    }
    // The cap hovering over the top block, lit beneath.
    let cap = cut_rect(b, 22.0, 18.0, 6.0);
    let at = |z: f32, s: f32| Section::new(z, s).shifted(-160.0, 0.0);
    light(b);
    b.loft_z(&cap, &[at(991.0, 0.5), at(996.0, 0.9)]);
    pale(b);
    b.loft_z(&cap, &[at(996.0, 1.0), at(MONOLITH_TOP, 0.8)]);

    // Frames forward: an open square on the axis, the middle one great.
    for (x, half, top) in [
        (64.0f32, 46.0f32, 968.0f32),
        (150.0, 72.0, MONOLITH_TOP),
        (228.0, 36.0, 958.0),
    ] {
        let post = (half * 0.26).max(9.0);
        let lintel = (top - DECK) * 0.2;
        pale(b);
        for y in [-half + post * 0.5, half - post * 0.5] {
            b.chamfered_box(
                v3(x, y, (DECK + top) * 0.5 - 1.0),
                v3(post * 1.1, post, top - DECK + 2.0),
                post * 0.2,
            );
        }
        b.chamfered_box(
            v3(x, 0.0, top - lintel * 0.5),
            v3(post * 1.2, half * 2.0 + 4.0, lintel),
            post * 0.2,
        );
        if !b.mid() {
            continue;
        }
        let inner = half - post;
        for s in [-1.0f32, 1.0] {
            seam(
                b,
                v3(x, s * (inner - 0.05), DECK + 4.0),
                v3(x, s * (inner - 0.05), top - lintel - 3.0),
                v3(0.0, -s, 0.0),
                post * 0.3,
            );
        }
        seam(
            b,
            v3(x, -inner + 3.0, top - lintel - 0.05),
            v3(x, inner - 3.0, top - lintel - 0.05),
            -Vec3::Z,
            post * 0.3,
        );
        // The keystone hanging in the window: live in the great frame only.
        let kz = DECK + (top - lintel - DECK) * 0.62;
        if top == MONOLITH_TOP {
            key_light(b);
        } else {
            light(b);
        }
        b.chamfered_box(
            v3(x, 0.0, kz),
            v3(post * 0.6, inner * 0.36, (top - DECK) * 0.18),
            1.2,
        );
        if b.fine() {
            pale(b);
            for s in [-1.0f32, 1.0] {
                b.chamfered_box(
                    v3(x, s * inner * 0.3, kz),
                    v3(post * 0.9, inner * 0.2, (top - DECK) * 0.3),
                    1.2,
                );
            }
        }
    }
    // Pylons at the deck's corners, their lights hovering over them.
    for (sx, sy) in [(1.0f32, 1.0f32), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
        let c = v3(sx * (hx - 34.0), sy * (hy - 22.0), 0.0);
        pale(b);
        b.chamfered_box(c + v3(0.0, 0.0, DECK + 13.0), v3(12.0, 12.0, 28.0), 3.0);
        light(b);
        b.chamfered_box(c + v3(0.0, 0.0, DECK + 32.0), v3(7.0, 7.0, 7.0), 2.0);
        if b.fine() {
            for s in [-6.0f32, 6.0] {
                seam(
                    b,
                    c + v3(s, 0.0, DECK + 4.0),
                    c + v3(s, 0.0, DECK + 24.0),
                    v3(s.signum(), 0.0, 0.0),
                    1.6,
                );
            }
        }
    }

    // Blocks trailing astern, hovering beside the keel and off the prow, lit beneath.
    let blocks: [(Vec3, Vec3); 12] = [
        (v3(-318.0, 0.0, 888.0), v3(46.0, 74.0, 28.0)),
        (v3(-366.0, 8.0, 864.0), v3(32.0, 50.0, 22.0)),
        (v3(-404.0, -6.0, 846.0), v3(20.0, 32.0, 16.0)),
        (v3(-432.0, 4.0, 832.0), v3(12.0, 20.0, 10.0)),
        (v3(-120.0, 130.0, 836.0), v3(64.0, 20.0, 24.0)),
        (v3(30.0, 134.0, 850.0), v3(44.0, 18.0, 20.0)),
        (v3(150.0, 126.0, 862.0), v3(28.0, 14.0, 14.0)),
        (v3(-70.0, -132.0, 842.0), v3(72.0, 22.0, 26.0)),
        (v3(90.0, -128.0, 858.0), v3(40.0, 16.0, 18.0)),
        (v3(190.0, -122.0, 870.0), v3(22.0, 12.0, 12.0)),
        (v3(302.0, 0.0, 896.0), v3(24.0, 64.0, 18.0)),
        (v3(330.0, 0.0, 914.0), v3(12.0, 34.0, 10.0)),
    ];
    for (c, s) in blocks {
        let ch = s.min_element() * 0.22;
        pale(b);
        b.chamfered_box(c, s, ch);
        if b.fine() {
            dark(b);
            b.chamfered_box(c, v3(s.x + 0.8, s.y + 0.8, s.z * 0.26), ch);
            light(b);
            b.chamfered_box(c, v3(s.x + 1.0, s.y + 1.0, s.z * 0.06), ch);
            pale(b);
            b.chamfered_box(
                c + v3(0.0, 0.0, s.z * 0.5 + 1.0),
                v3(s.x * 0.6, s.y * 0.6, 2.4),
                ch * 0.6,
            );
        }
        if b.mid() {
            light(b);
            let z = c.z - s.z * 0.5 - 0.1;
            let (lx, ly) = (s.x * 0.32, s.y * 0.32);
            film(
                b,
                &[
                    v3(c.x - lx, c.y - ly, z),
                    v3(c.x + lx, c.y - ly, z),
                    v3(c.x + lx, c.y + ly, z),
                    v3(c.x - lx, c.y + ly, z),
                ],
                -Vec3::Z,
                0.0,
            );
        }
    }
}

/// [`joint`] for a plan that is not square.
fn joint_xy(b: &mut MeshBuilder, z0: f32, z1: f32, hx: f32, hy: f32, ch: f32) {
    if b.coarse() {
        return;
    }
    dark(b);
    let plan = cut_rect(b, hx, hy, ch);
    b.loft_z(
        &plan,
        &[Section::new(z0 - 0.5, 1.0), Section::new(z1 + 0.5, 1.0)],
    );
    light(b);
    let zm = (z0 + z1) * 0.5;
    let s = Section::scaled(zm - 1.0, (hx + 0.3) / hx, (hy + 0.3) / hy);
    b.loft_z(&plan, &[s, Section { z: zm + 1.0, ..s }]);
}

// ---- Needle -------------------------------------------------------------------------
//
// A mast 1800 m into the sky off a plinth 88 m square: two stepped courses, blades
// out of the corners, then four stages of shaft on necks of light with a collar of
// segments floating round each neck (600, 1100, 1500 m), and a light hovering over
// its point.

pub(super) const NEEDLE_TOP: f32 = 1_800.0;
pub(super) const NEEDLE_COLLARS: [f32; 3] = [600.0, 1_100.0, 1_500.0];
const NEEDLE_STAGES: [(f32, f32, f32, f32); 4] = [
    (64.0, 590.0, 24.0, 19.0),
    (610.0, 1_090.0, 17.0, 13.5),
    (1_110.0, 1_490.0, 12.0, 9.0),
    (1_510.0, 1_724.0, 8.0, 4.5),
];

fn needle(b: &mut MeshBuilder, _tech: u8) {
    pale(b);
    let foot = cut_rect(b, 44.0, 44.0, 12.0);
    b.loft_z(
        &foot,
        &[
            Section::new(-80.0, 1.0),
            Section::new(0.0, 1.0),
            Section::new(34.0, 0.88),
        ],
    );
    if b.coarse() {
        let shaft = cut_rect(b, 24.0, 24.0, 0.0);
        b.loft_z(
            &shaft,
            &[Section::new(33.0, 1.0), Section::new(1_730.0, 0.18)],
        );
        key_light(b);
        let gem = cut_rect(b, 7.0, 7.0, 0.0);
        b.loft_z(
            &gem,
            &[Section::new(1_730.0, 1.0), Section::new(NEEDLE_TOP, 0.2)],
        );
        return;
    }
    light(b);
    b.loft_z(
        &foot,
        &[Section::new(16.0, 0.9465), Section::new(18.0, 0.9395)],
    );
    joint(b, 34.0, 40.0, 33.0, 8.0);
    course(b, 40.0, 64.0, 34.0, 30.0, 9.0, 0.0, 0.0);

    // Blades out of the corners, leaning in to the shaft.
    let blade: [[f32; 2]; 6] = [
        [20.0, -40.0],
        [56.0, -40.0],
        [56.0, 8.0],
        [42.0, 120.0],
        [34.0, 300.0],
        [24.0, 312.0],
    ];
    b.radial(4, |b| {
        b.with(Affine3A::from_rotation_z(FRAC_PI_4), |b| {
            pale(b);
            b.extrude_y_chamfered(&blade, 4.2, 1.2);
            if b.fine() {
                for s in [-4.2f32, 4.2] {
                    seam(
                        b,
                        v3(52.0, s, 16.0),
                        v3(40.0, s, 116.0),
                        v3(0.0, s.signum(), 0.0),
                        1.2,
                    );
                    seam(
                        b,
                        v3(39.0, s, 134.0),
                        v3(34.0, s, 236.0),
                        v3(0.0, s.signum(), 0.0),
                        1.2,
                    );
                }
            }
        });
    });

    for (i, &(z0, z1, h0, h1)) in NEEDLE_STAGES.iter().enumerate() {
        stage(b, z0, z1, h0, h1, 70.0 - 10.0 * i as f32);
        if i > 0 {
            let (_, under, _, top) = NEEDLE_STAGES[i - 1];
            neck(b, under, z0, top * 0.5);
            let z = NEEDLE_COLLARS[i - 1];
            let (r0, r1) = (top * 1.9, top * 1.9 + 16.0 - 2.0 * i as f32);
            let turn = if i % 2 == 0 { PI / 6.0 } else { 0.0 };
            let h = 5.0 - 0.6 * i as f32;
            hoop(
                b,
                Hoop {
                    r0,
                    r1,
                    z,
                    h,
                    pieces: 6,
                    gap: 3.0,
                    turn,
                    bay: 0.0,
                },
            );
            if b.mid() {
                let a = r1 + (r1 - r0) * 0.3;
                hoop(
                    b,
                    Hoop {
                        r0: a,
                        r1: a + (r1 - r0) * 0.3,
                        z,
                        h: h * 0.4,
                        pieces: 12,
                        gap: 2.4,
                        turn: turn + PI / 12.0,
                        bay: 0.0,
                    },
                );
            }
        }
    }
    point(b, 1_730.0, 1_772.0, 5.0, false);
    // The live light over the point.
    key_light(b);
    b.spheroid(v3(0.0, 0.0, 1_786.0), v3(7.0, 7.0, 14.0), 4, 2);
}

#[cfg(test)]
mod tests {
    use super::super::{build_model, MeshLod};
    use super::*;

    #[test]
    fn sky_builds_within_budget() {
        for def in MODELS {
            let model = build_model(def.key).unwrap();
            let tris: Vec<usize> = model.lods.iter().map(|l| l.indices.len() / 3).collect();
            let low = model.lods[0]
                .vertices
                .iter()
                .map(|v| v.pos[2])
                .fold(f32::MAX, f32::min);
            let high = model.lods[0]
                .vertices
                .iter()
                .map(|v| v.pos[2])
                .fold(f32::MIN, f32::max);
            let reach = model.lods[0]
                .vertices
                .iter()
                .map(|v| v.pos[0].hypot(v.pos[1]))
                .fold(0.0, f32::max);
            println!(
                "{}: triangles {tris:?}, z {low:.0}..{high:.0}, reach {reach:.0}, bounds {:.0}",
                def.key, model.bounds_radius
            );
            assert!(tris[0] <= TRIANGLES, "{}: {} triangles", def.key, tris[0]);
            assert!(tris[1] < tris[0], "{}: LOD1 not lighter", def.key);
            assert!(
                tris[2] < 60,
                "{}: coarse LOD {} triangles",
                def.key,
                tris[2]
            );
            // Mostly dormant: live light only at the working parts.
            let area = |m: u32| -> f32 {
                let lod = &model.lods[0];
                lod.indices
                    .chunks(3)
                    .filter(|t| lod.vertices[t[0] as usize].material == m)
                    .map(|t| {
                        let [a, c, d] =
                            [t[0], t[1], t[2]].map(|i| Vec3::from(lod.vertices[i as usize].pos));
                        (c - a).cross(d - a).length() * 0.5
                    })
                    .sum()
            };
            let (live, dormant) = (
                area(super::super::material::GLOW_PRECURSOR),
                area(super::super::material::PRECURSOR_INLAY),
            );
            println!(
                "{}: live light {:.1}% of the light",
                def.key,
                100.0 * live / (live + dormant)
            );
            // The halo's one working part is a fine ring 3.5 km round: a larger share.
            let share = if def.key == "precursor_halo" {
                0.15
            } else {
                0.1
            };
            assert!(
                live > 0.0 && live < share * (live + dormant),
                "{}: live light {live} of {}",
                def.key,
                live + dormant
            );
            assert!(
                tris[1] as f32 <= tris[0] as f32 * 0.45 + 20.0,
                "{}: reduced LOD {} of {}",
                def.key,
                tris[1],
                tris[0]
            );
            let floor = match def.key {
                "precursor_halo" => HALO_FLOOR - 20.0,
                "precursor_monolith" => MONOLITH_KEEL - 10.0,
                _ => -80.0,
            };
            for lod in &model.lods {
                let low = lod
                    .vertices
                    .iter()
                    .map(|v| v.pos[2])
                    .fold(f32::MAX, f32::min);
                assert!(low >= floor, "{}: {low} below its floor {floor}", def.key);
                for v in &lod.vertices {
                    assert!(
                        v.pos.iter().chain(&v.normal).all(|c| c.is_finite()),
                        "{}",
                        def.key
                    );
                    assert!(
                        (Vec3::from(v.normal).length() - 1.0).abs() < 1e-4,
                        "{}: unit normal",
                        def.key
                    );
                }
                // As `models::tests::meshes_are_valid`: no slivers, windings agree with normals.
                for t in lod.indices.chunks(3) {
                    let [a, c, d] =
                        [t[0], t[1], t[2]].map(|i| Vec3::from(lod.vertices[i as usize].pos));
                    let geometric = (c - a).cross(d - a);
                    assert!(
                        geometric.length() * 0.5 > 1e-7,
                        "{}: degenerate triangle at {a}",
                        def.key
                    );
                    for &i in t {
                        let shading = Vec3::from(lod.vertices[i as usize].normal);
                        assert!(
                            geometric.normalize().dot(shading) > 0.5,
                            "{}: winding disagrees at {a}",
                            def.key
                        );
                    }
                }
            }
        }
    }

    /// The Heart and the needle stand on their plinths: nothing under 60 m reaches
    /// outside the solid plan (a square `half` either side), bar a little for the
    /// blades' diagonal feet inside the square's corners.
    #[test]
    fn grounded_pieces_stand_in_their_plans() {
        for (key, half) in [
            ("precursor_heart", HEART_PLINTH),
            ("precursor_needle", 44.0),
        ] {
            let model = build_model(key).unwrap();
            for v in model.lods.iter().flat_map(|l| &l.vertices) {
                if v.pos[2] < 60.0 {
                    assert!(
                        v.pos[0].abs() <= half + 0.5 && v.pos[1].abs() <= half + 0.5,
                        "{key}: {:?} outside the plan",
                        v.pos
                    );
                }
            }
        }
    }

    /// `mesh` cut to the slab between `z0` and `z1`: close-ups of a tall model.
    pub(super) fn clip(mesh: &MeshLod, z0: f32, z1: f32) -> MeshLod {
        let mut out = MeshLod::default();
        for t in mesh.indices.chunks(3) {
            let mut poly: Vec<_> = t.iter().map(|&i| mesh.vertices[i as usize]).collect();
            for (plane, up) in [(z0, 1.0f32), (z1, -1.0)] {
                let d = |v: &super::super::MeshVertex| (v.pos[2] - plane) * up;
                let mut next = Vec::new();
                for i in 0..poly.len() {
                    let (a, c) = (poly[i], poly[(i + 1) % poly.len()]);
                    let (da, dc) = (d(&a), d(&c));
                    if da >= 0.0 {
                        next.push(a);
                    }
                    if da * dc < 0.0 {
                        let k = da / (da - dc);
                        let mut v = a;
                        for j in 0..3 {
                            v.pos[j] = a.pos[j] + (c.pos[j] - a.pos[j]) * k;
                        }
                        next.push(v);
                    }
                }
                poly = next;
            }
            if poly.len() >= 3 {
                let base = out.vertices.len() as u32;
                out.vertices.extend(&poly);
                for i in 1..poly.len() as u32 - 1 {
                    out.indices.extend([base, base + i, base + i + 1]);
                }
            }
        }
        out
    }
}

/// Software previews: `SKY_DUMP_DIR=... cargo test -p mc-models --lib sky_previews -- --ignored`.
/// Whole models, and close-ups of the Heart's base, Lens, collars and point.
#[cfg(test)]
#[test]
#[ignore]
fn sky_previews() {
    let dir = std::path::PathBuf::from(std::env::var_os("SKY_DUMP_DIR").expect("SKY_DUMP_DIR"));
    std::fs::create_dir_all(&dir).unwrap();
    for def in MODELS {
        let model = super::build_model(def.key).unwrap();
        for (lod, yaw) in [(0usize, -38.0f32), (0, 142.0), (1, -38.0), (2, -38.0)] {
            super::preview::render(&model.lods[lod], 768, yaw)
                .write_ppm(&dir.join(format!("{}_{lod}_{}.ppm", def.key, yaw as i32)))
                .unwrap();
        }
    }
    let heart = super::build_model("precursor_heart").unwrap();
    let needle = super::build_model("precursor_needle").unwrap();
    let views: [(&str, &super::Model, usize, f32, f32); 9] = [
        ("heart_base", &heart, 0, -80.0, 420.0),
        ("heart_lens", &heart, 0, 440.0, 1_000.0),
        ("heart_lens1", &heart, 1, 440.0, 1_000.0),
        ("heart_mid", &heart, 0, 1_000.0, 1_700.0),
        ("heart_upper", &heart, 0, 1_700.0, 2_300.0),
        ("heart_top", &heart, 0, 2_300.0, 2_800.0),
        ("heart_lower", &heart, 0, -80.0, 1_500.0),
        ("needle_base", &needle, 0, -80.0, 300.0),
        ("needle_top", &needle, 0, 1_000.0, 1_800.0),
    ];
    for (name, model, lod, z0, z1) in views {
        let mesh = tests::clip(&model.lods[lod], z0, z1);
        super::preview::render(&mesh, 768, -38.0)
            .write_ppm(&dir.join(format!("{name}.ppm")))
            .unwrap();
    }
}
