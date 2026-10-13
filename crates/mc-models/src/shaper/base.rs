//! The Shapers' base pieces (mc-sim `survival/base.rs`), in the braced blades'
//! language (printer.rs): slab blades cut on a slant, pale plates over a dark core,
//! battered courses, rings of hovering plates. Each is authored at true size at its
//! blueprint's radius and height; a deck's top is that height, so it stands clear of a
//! lot that is not quite level.
//!
//! - `progenitor_platform`: a low walkable pad, its room clear for the units it prints,
//!   the pier at its middle within the hull the sim blocks; a split spine at its rear
//!   edge (its Shaper's side) with projector arms reaching out over the deck, their
//!   dark undersides where the light comes from.
//! - `progenitor_platform_great`: the great pad, slab blades rising from its corners and
//!   leaning in over the deck, their dark tips where the light comes from.
//! - `progenitor_slip`, `progenitor_slip_great`: emitter fins on floats round open
//!   water, open to the sea in front, a lens buoy at the middle.
//! - `progenitor_pylon`: a telescoping stele under a ward ring.
//!
//! An Unmaker's silo (`progenitor_silo`) is in auxiliary.rs, a piece of its spire.

use std::f32::consts::{PI, TAU};

use glam::{Affine3A, Vec3};

use super::super::builder::{MeshBuilder, Section};
use super::super::library::ModelDef;
use super::super::precursor::{cut_rect, dark, pale, v3};
use super::kit::{course, joint, ring, shaft, Ring};

pub(super) const MODELS: [ModelDef; 5] = [
    ModelDef::new("progenitor_platform", 48.0, 3.0, platform),
    ModelDef::new("progenitor_platform_great", 96.0, 4.0, great_platform),
    ModelDef::new("progenitor_slip", 48.0, 3.0, slip),
    ModelDef::new("progenitor_slip_great", 96.0, 4.0, great_slip),
    ModelDef::new("progenitor_pylon", 14.0, 40.0, pylon),
];

/// A pad's size: its half across, how tall its pieces stand, and the pier's half.
#[derive(Clone, Copy)]
struct Pad {
    half: f32,
    tall: f32,
    pier: f32,
    great: bool,
    /// The deck's top: the blueprint's height, so it stands clear of a lot that is
    /// not quite level.
    top: f32,
}

const PAD: Pad = Pad {
    half: 46.0,
    tall: 26.0,
    pier: 7.0,
    great: false,
    top: 3.0,
};

const GREAT: Pad = Pad {
    half: 94.0,
    tall: 44.0,
    pier: 11.0,
    great: true,
    top: 4.0,
};

fn platform(b: &mut MeshBuilder, _tech: u8) {
    deck(b, PAD);
    arms(b, PAD);
    pier(b, PAD);
}

fn great_platform(b: &mut MeshBuilder, _tech: u8) {
    deck(b, GREAT);
    blades(b, GREAT);
    pier(b, GREAT);
}

fn slip(b: &mut MeshBuilder, _tech: u8) {
    fins(b, PAD);
    buoy(b, PAD);
}

fn great_slip(b: &mut MeshBuilder, _tech: u8) {
    fins(b, GREAT);
    buoy(b, GREAT);
}

/// The pad: a battered course and a deck of four plates over it with dark joints
/// between, low enough to walk on.
fn deck(b: &mut MeshBuilder, p: Pad) {
    let h = p.half;
    let top = p.top;
    course(b, 0.0, top * 0.55, h, h * 0.97);
    dark(b);
    let core = cut_rect(b, h * 0.95, h * 0.95, h * 0.26);
    b.extrude_z(&core, top * 0.4, top * 0.82);
    if b.coarse() {
        return;
    }
    // Four plates, a joint of dark between them, their outer corners cut.
    let gap = h * 0.025;
    let q = (h * 0.94 - gap) * 0.5;
    pale(b);
    for (sx, sy) in [(1.0f32, 1.0f32), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
        let c = v3(sx * (gap + q), sy * (gap + q), 0.0);
        b.at(c, |b| {
            let plate = cut_rect(b, q, q, q * 0.18);
            b.extrude_z(&plate, top * 0.6, top);
        });
    }
}

/// A slab blade in a frame whose x runs out from the middle: its foot on the rim at
/// `rim`, `foot` wide, rising `tall` and leaning `lean` in over the middle, its tip
/// cut on a slant.
#[derive(Clone, Copy)]
struct Blade {
    rim: f32,
    foot: f32,
    tall: f32,
    lean: f32,
}

impl Blade {
    /// The middle of its slanted tip, (x, z) in its frame: where its light leaves from.
    fn tip(&self) -> (f32, f32) {
        (self.rim - self.lean - self.foot * 0.275, self.tall * 0.92)
    }
}

/// `bl` drawn: pale up to `sheath` of its height, a dark core above it where the light
/// comes from; `t` its half thickness.
fn blade(b: &mut MeshBuilder, bl: Blade, t: f32, sheath: f32) {
    let Blade {
        rim,
        foot,
        tall,
        lean,
    } = bl;
    let profile = [
        [rim - foot, 0.0],
        [rim, 0.0],
        [rim - lean * 0.35, tall * 0.62],
        [rim - lean, tall],
        [rim - lean - foot * 0.55, tall * 0.84],
    ];
    dark(b);
    b.extrude_y_chamfered(&profile, t, t * 0.25);
    if b.coarse() {
        return;
    }
    // The pale sheath over its lower part, standing proud of the core.
    let z = tall * sheath;
    let f = z / tall;
    let outer = |k: f32| rim - lean * 0.35 * (k / 0.62).min(1.0);
    let inner = |k: f32| rim - foot + (foot - lean - foot * 0.55) * (k / 0.84).min(1.0);
    pale(b);
    let sheath = [
        [rim - foot - 0.2, 0.0],
        [rim + 0.2, 0.0],
        [outer(f) + 0.2, z],
        [inner(f) - 0.2, z * 0.92],
    ];
    b.extrude_y_chamfered(&sheath, t * 1.18, t * 0.2);
}

/// Blades rising from the great pad's rear corners (toward its Shaper, -x) and a
/// lesser pair from its front corners, leaning in over the deck, the rear pair braced
/// across the back by a lintel.
fn blades(b: &mut MeshBuilder, p: Pad) {
    for (heading, bl) in great_blades(p) {
        b.with(Affine3A::from_rotation_z(heading), |b| {
            blade(b, bl, 2.4, 0.58)
        });
    }
    // The lintel across the back, between the rear blades' feet.
    pale(b);
    let h = p.half;
    let y = h * 1.25 * (PI - 0.62f32).sin().abs() - h * 0.1;
    let top = lintel_top(p);
    b.chamfered_box(
        v3(top.x, 0.0, top.z * 0.5),
        v3(h * 0.08, y * 2.0, top.z),
        h * 0.02,
    );
}

/// The great pad's blades: each one's heading round its middle, and its shape.
fn great_blades(p: Pad) -> [(f32, Blade); 4] {
    let h = p.half;
    [
        (PI - 0.62, 1.0f32),
        (PI + 0.62, 1.0),
        (0.66, 0.62),
        (-0.66, 0.62),
    ]
    .map(|(heading, size)| {
        (
            heading,
            Blade {
                rim: h * 1.25,
                foot: h * 0.26 * size,
                tall: p.tall * size,
                lean: h * 0.62 * size,
            },
        )
    })
}

/// The middle of the top of the great pad's lintel.
fn lintel_top(p: Pad) -> Vec3 {
    v3(-p.half * 0.98, 0.0, p.tall * 0.18)
}

/// A projector arm: across the pad at `y`, its tip `reach` along x, its top at `z`
/// and its depth `th`.
#[derive(Clone, Copy)]
struct Arm {
    y: f32,
    reach: f32,
    z: f32,
    th: f32,
}

impl Arm {
    /// The tip of its dark underside, where its light leaves from.
    fn emitter(&self) -> Vec3 {
        v3(self.reach, self.y, self.z - self.th * 0.4)
    }
}

/// How far across the pad the spine's leaves reach either side of its middle.
fn spine_span(p: Pad) -> f32 {
    p.half * 0.46
}

/// The platform's three arms, the middle reaching furthest.
fn platform_arms(p: Pad) -> [Arm; 3] {
    let h = p.half;
    [-1.0f32, 0.0, 1.0].map(|f| Arm {
        y: f * spine_span(p) * 0.78,
        reach: h * (0.15 - 0.3 * f.abs()),
        z: p.tall * (0.72 - 0.08 * f.abs()),
        th: p.tall * 0.08,
    })
}

/// A split spine at the pad's rear edge with projector arms reaching out over the deck
/// from near its top, braced under, their dark undersides where the light comes from.
fn arms(b: &mut MeshBuilder, p: Pad) {
    let h = p.half;
    let x0 = -h * 1.02;
    let spine_w = h * 0.12;
    let tall = p.tall * 1.15;
    // The spine: a slab across the back, its top cut on a slant.
    pale(b);
    let spine = [
        [x0, 0.0],
        [x0 + spine_w, 0.0],
        [x0 + spine_w * 0.8, tall * 0.92],
        [x0, tall],
    ];
    let span = spine_span(p);
    // Two leaves with a slot of sky between them, as the Shaper's spine.
    let leaf = span * 0.46;
    for y in [-1.0f32, 1.0] {
        b.at(v3(0.0, y * (span - leaf), 0.0), |b| {
            b.extrude_y_chamfered(&spine, leaf, spine_w * 0.15);
        });
    }
    let t = 1.4;
    for Arm { y, reach, z, th } in platform_arms(p) {
        b.at(v3(0.0, y, 0.0), |b| {
            dark(b);
            let under = [
                [x0 + spine_w, z - th],
                [reach, z - th * 0.4],
                [reach - th * 0.4, z],
                [x0 + spine_w, z],
            ];
            b.extrude_y_chamfered(&under, t * 0.8, t * 0.15);
            pale(b);
            let arm = [
                [x0 + spine_w * 0.5, z - th * 0.1],
                [reach - th * 0.5, z + th * 0.15],
                [reach - th * 1.2, z + th * 0.7],
                [x0 + spine_w * 0.5, z + th * 1.1],
            ];
            b.extrude_y_chamfered(&arm, t, t * 0.2);
            if !b.coarse() {
                // A brace from low on the spine up under the arm.
                let brace = [
                    [x0 + spine_w, z * 0.3],
                    [x0 + spine_w + h * 0.04, z * 0.3],
                    [x0 + spine_w + h * 0.42, z - th * 0.9],
                    [x0 + spine_w + h * 0.36, z - th * 0.9],
                ];
                b.extrude_y_chamfered(&brace, t * 0.6, t * 0.1);
            }
        });
    }
}

/// Emitter fins round a slip's water leaning in, each on a pale float, open in front
/// (+x) where the ships leave.
fn fins(b: &mut MeshBuilder, p: Pad) {
    let h = p.half;
    let t = if p.great { 1.8 } else { 1.1 };
    for (a, bl) in slip_fins(p) {
        b.with(Affine3A::from_rotation_z(a), |b| {
            pale(b);
            b.chamfered_box(
                v3(bl.rim - h * 0.09, 0.0, 0.7),
                v3(h * 0.2, t * 5.0, 1.4),
                t,
            );
            b.at(v3(0.0, 0.0, FIN_FLOAT), |b| blade(b, bl, t, 0.5));
        });
    }
}

/// How high a fin stands on its float.
const FIN_FLOAT: f32 = 1.4;

/// A slip's fins: each one's heading round its middle, and its shape.
fn slip_fins(p: Pad) -> Vec<(f32, Blade)> {
    let h = p.half;
    let n = if p.great { 12 } else { 8 };
    let open = 0.55f32;
    let span = (TAU - 2.0 * open) / n as f32;
    (0..n)
        .map(|k| {
            (
                open + span * (k as f32 + 0.5),
                Blade {
                    rim: h * 1.02,
                    foot: h * 0.16,
                    tall: p.tall * 0.55,
                    lean: h * 0.22,
                },
            )
        })
        .collect()
}

/// Where a blade at `heading` round the piece's middle, standing `z` up, lets its
/// light go, in the piece's frame.
fn tip_at(heading: f32, bl: Blade, z: f32) -> Vec3 {
    let (x, tz) = bl.tip();
    let (s, c) = heading.sin_cos();
    v3(x * c, x * s, z + tz)
}

/// Where a print platform's or slip's print beams leave from, in its own frame (x along
/// its heading, y left, z up from its foot), by mc-sim's `Kit` order: the platform's
/// arm tips, the great platform's blade tips and lintel, a slip's fin tips. mc-sim
/// `survival::PAD_EMITTERS` is the sim's copy, held equal by a test here.
pub fn pad_emitters(piece: usize) -> Vec<[f32; 3]> {
    let points: Vec<Vec3> = match piece {
        0 => platform_arms(PAD).iter().map(Arm::emitter).collect(),
        1 => great_blades(GREAT)
            .iter()
            .map(|&(a, bl)| tip_at(a, bl, 0.0))
            .chain([lintel_top(GREAT)])
            .collect(),
        2 => slip_fins(PAD)
            .iter()
            .map(|&(a, bl)| tip_at(a, bl, FIN_FLOAT))
            .collect(),
        _ => slip_fins(GREAT)
            .iter()
            .map(|&(a, bl)| tip_at(a, bl, FIN_FLOAT))
            .collect(),
    };
    points.iter().map(|p| p.to_array()).collect()
}

/// The pier at the pad's middle (the hull the sim blocks): a battered plinth with a
/// dark lens block on it, cut on a slant.
fn pier(b: &mut MeshBuilder, p: Pad) {
    let w = p.pier;
    let z = p.top;
    course(b, z * 0.5, z + w * 0.35, w, w * 0.8);
    dark(b);
    let lens = [
        [-w * 0.55, z],
        [w * 0.55, z],
        [w * 0.5, z + w * 1.1],
        [-w * 0.3, z + w * 1.5],
    ];
    b.extrude_y_chamfered(&lens, w * 0.45, w * 0.08);
}

/// A slip's middle (the hull the sim blocks): a lens buoy, a dark block on a pale float.
fn buoy(b: &mut MeshBuilder, p: Pad) {
    let w = p.pier;
    pale(b);
    let float = cut_rect(b, w, w, w * 0.3);
    b.loft_z(
        &float,
        &[Section::new(0.0, 0.9), Section::new(w * 0.2, 1.0)],
    );
    dark(b);
    b.at(v3(0.0, 0.0, w * 0.2), |b| {
        let lens = [
            [-w * 0.5, 0.0],
            [w * 0.5, 0.0],
            [w * 0.42, w * 0.9],
            [-w * 0.25, w * 1.25],
        ];
        b.extrude_y_chamfered(&lens, w * 0.4, w * 0.08);
    });
}

/// The pylon's plinth, battered, under every take.
fn plinth(b: &mut MeshBuilder) {
    course(b, 0.0, 2.2, 11.0, 9.6);
    if !b.coarse() {
        joint(b, 2.2, 2.6, 8.4);
    }
}

/// A telescoping stele, its top cut on a slant, a ward ring of three plates hovering
/// round it.
fn pylon(b: &mut MeshBuilder, _tech: u8) {
    plinth(b);
    let mut c = 4.2f32;
    let mut z = 2.6;
    for k in 0..3 {
        let run = [12.0, 10.0, 8.0][k];
        shaft(b, z, z + run, c, c * 0.92, 0.0);
        if k < 2 && !b.coarse() {
            joint(b, z + run - 0.2, z + run + 0.4, c * 1.06);
        }
        z += run;
        c *= 0.8;
    }
    pale(b);
    let tip = [
        [-c * 1.1, z - 0.4],
        [c * 1.1, z - 0.4],
        [c * 0.9, z + 5.5],
        [-c * 0.5, z + 8.0],
    ];
    b.extrude_y_chamfered(&tip, c * 0.9, 0.3);
    ring(
        b,
        Ring {
            r0: 6.2,
            r1: 9.0,
            z: 24.0,
            h: 0.55,
            pieces: 3,
            gap: 4.0,
            turn: PI / 6.0,
        },
    );
}
