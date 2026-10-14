//! The Progenitor's hard-light bridge (`mc_map::PropKind::PrecursorBridge`), built to
//! `mc_map::landmark::HARDLIGHT_BRIDGE`: a road of hard light from the origin along +x
//! for the plan's span, the plan's half either side, its walking surface at the
//! origin's height. The map's deck (`mc_map::deck`) is that road, so units cross on
//! it and ships sail under it. Nothing reaches down into the water: the light holds
//! it.
//!
//! The road itself is not modelled: it is hard light, drawn by the renderer over the
//! map's deck (`progenitor_fx`'s deck pieces, progenitor_fx.wgsl), clear enough to see
//! the sea through, its sides thin rails of light. Here is what projects it: at each
//! landing a projector of pale alloy plates on a dark core, level at the deck's end
//! and then a ramp down inland, the map's ground shaped to it so units walk up it.
//! Nothing stands over the span.
//!
//! Model space as for every prop: x along the heading, y left, z up, the origin on
//! the ground (the near landing).

use std::f32::consts::PI;

use glam::{Affine3A, Vec3};

use super::builder::MeshBuilder;
use super::library::ModelDef;
use super::precursor::{dark, pale, panel, v3};
use mc_map::landmark::HARDLIGHT_BRIDGE as PLAN;

pub(super) const MODELS: &[ModelDef] = &[ModelDef::new("precursor_bridge", RADIUS, HORN.1, bridge)];

/// The span and half the road's width, as the map lays the deck.
const SPAN: f32 = PLAN.span as f32;
const HALF: f32 = PLAN.half as f32;
/// A projector: level for `HEAD` inland of the deck's end, then down `GRADE` for
/// `RUN` (the map's landing is shaped to the same plan).
const HEAD: f32 = PLAN.head as f32;
const RUN: f32 = PLAN.run as f32;
const GRADE: f32 = PLAN.grade as f32;
const DROP: f32 = GRADE * RUN;
/// The authored radius: the half diagonal, projectors included.
const RADIUS: f32 = 1_060.0;
/// How far past the road a projector runs either side, its plates over the ground
/// (a step too low to see a unit sink) and its foot, well into the ground.
const PAST: f32 = 8.0;
const TOP: f32 = 0.35;
const FOOT: f32 = -DROP - 26.0;
/// The plates: the dark border left round them, the joint between, and how
/// proud of the core they stand.
const PLATES: (f32, f32, f32) = (3.0, 1.6, 0.12);
/// The dark projector bar across the lip, metres inland.
const LIP: f32 = 6.0;
/// The emitter at each corner of the lip: its length inland and its height.
const HORN: (f32, f32) = (18.0, 9.0);
/// A kerb along each side: its height.
const KERB: f32 = 1.6;

/// The projector's surface (under its plates) `x` metres along: level on its
/// head, then down its ramp.
fn surface(x: f32) -> f32 {
    if x >= -HEAD {
        0.0
    } else {
        -GRADE * (-HEAD - x)
    }
}

/// A side profile (x, z) following the surface from `x0` to `x1` (inland is
/// negative), `below` under it to `above` over it, split at the head's end so
/// each piece is convex; extruded across from `y.0` to `y.1`.
fn along_surface(b: &mut MeshBuilder, x0: f32, x1: f32, below: f32, above: f32, y: (f32, f32)) {
    let cut = |b: &mut MeshBuilder, a: f32, c: f32| {
        if c - a < 0.01 {
            return;
        }
        let profile = [
            [c, surface(c) - below],
            [c, surface(c) + above],
            [a, surface(a) + above],
            [a, surface(a) - below],
        ];
        b.extrude_y(&profile, y.0, y.1);
    };
    cut(b, x0.max(-HEAD), x1);
    cut(b, x0, x1.min(-HEAD));
}

/// The plates over the head and down the ramp, rows along it and columns across,
/// every other row of the ramp's staggered by half a plate.
fn plates(b: &mut MeshBuilder) {
    let (border, joint, proud) = PLATES;
    let half_y = HALF + PAST;
    let (head_rows, ramp_rows, cols) = match b.lod() {
        0 => (2, 6, 8),
        _ => (1, 2, 4),
    };
    let (y0, y1) = (-half_y + border, half_y - border);
    let width = (y1 - y0 - joint * (cols - 1) as f32) / cols as f32;
    let course = |b: &mut MeshBuilder, x0: f32, x1: f32, stagger: bool| {
        let mut edges = vec![y0];
        let mut y = y0 + if stagger { width * 0.5 } else { width } + joint;
        while y < y1 - 1.0 {
            edges.push(y);
            y += width + joint;
        }
        edges.push(y1 + joint);
        let out = if x1 <= -HEAD {
            v3(-GRADE, 0.0, 1.0).normalize()
        } else {
            Vec3::Z
        };
        let at = |x: f32, y: f32| v3(x, y, surface(x) + TOP);
        for w in edges.windows(2) {
            let (a, c) = (w[0], w[1] - joint);
            panel(
                b,
                &[at(x0, a), at(x1, a), at(x1, c), at(x0, c)],
                out,
                proud,
                0.4,
            );
        }
    };
    pale(b);
    let row = (HEAD - LIP - border - joint * head_rows as f32) / head_rows as f32;
    for r in 0..head_rows {
        let x1 = -LIP - joint - r as f32 * (row + joint);
        course(b, x1 - row, x1, false);
    }
    let row = (RUN - border - joint * ramp_rows as f32) / ramp_rows as f32;
    for r in 0..ramp_rows {
        let x1 = -HEAD - joint - r as f32 * (row + joint);
        course(b, x1 - row, x1, b.fine() && r % 2 == 1);
    }
}

/// A projector in its own frame: the road leaves its lip at x = 0 along +x, the
/// projector inland toward -x.
fn projector(b: &mut MeshBuilder) {
    let half_y = HALF + PAST;
    // The dark core, its sides the walls the ground falls away from.
    dark(b);
    b.extrude_y(
        &[
            [0.0, FOOT],
            [0.0, TOP],
            [-HEAD, TOP],
            [-HEAD - RUN, TOP - DROP],
            [-HEAD - RUN, FOOT],
        ],
        -half_y,
        half_y,
    );
    if b.coarse() {
        // Far off the core's plain faces carry it (the surface draws its panels).
        return;
    }
    plates(b);
    kerbs(b);
}

/// Low kerbs down both sides, buttresses down the side walls, and at each corner
/// of the lip an emitter whose sloped face looks along the rail it projects.
fn kerbs(b: &mut MeshBuilder) {
    let (len, tall) = HORN;
    b.mirror_y(|b| {
        pale(b);
        along_surface(b, -HEAD - RUN, -len, 2.0, KERB, (HALF, HALF + PAST));
        // The emitter: a block stepping down to a sloped face over the road's end.
        b.extrude_y(
            &[
                [0.0, -2.0],
                [0.0, 3.0],
                [-6.0, tall],
                [-len, tall],
                [-len, -2.0],
            ],
            HALF - 1.0,
            HALF + PAST + 2.0,
        );
        if b.mid() {
            dark(b);
            let face = |x: f32, z: f32, y: f32| v3(x, y, z);
            panel(
                b,
                &[
                    face(-0.8, 3.6, HALF + 1.0),
                    face(-0.8, 3.6, HALF + PAST),
                    face(-5.2, 8.0, HALF + PAST),
                    face(-5.2, 8.0, HALF + 1.0),
                ],
                v3(6.0, 0.0, 6.0).normalize(),
                0.2,
                0.3,
            );
        }
        // Buttresses down the side wall, each sloping off below the surface.
        pale(b);
        let step = if b.fine() { 26.0 } else { 52.0 };
        let mut x = -12.0;
        while x > -HEAD - RUN + 6.0 {
            let s = surface(x);
            b.extrude_x(
                &[
                    [HALF + PAST, FOOT],
                    [HALF + PAST + 7.0, FOOT],
                    [HALF + PAST + 7.0, s - 7.0],
                    [HALF + PAST, s - 0.4],
                ],
                x - 1.6,
                x + 1.6,
            );
            x -= step;
        }
    });
}

/// A projector at each landing, the far one turned back along the road.
fn bridge(b: &mut MeshBuilder, _tech: u8) {
    projector(b);
    let far = Affine3A::from_translation(v3(SPAN, 0.0, 0.0)) * Affine3A::from_rotation_z(PI);
    b.with(far, projector);
}
