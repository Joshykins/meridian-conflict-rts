//! The Precursor megastructure: the kit a map's one machine is built from
//! (`mc_map::PropKind::Precursor{Bastion,Boom,Tower,Span}`). Where the artifacts in
//! `precursor.rs` are monuments tens of metres high, these are pieces of a single
//! machine hundreds of metres high, laid out on one axis across a map and running
//! off its edge: bastions cut into the mountains, cantilevered booms reaching out
//! of them, towers into the clouds, and spans bridging the valleys between.
//!
//! Same language as the artifacts: pale alloy (`PRECURSOR`) over a dark core
//! (`PRECURSOR_DARK`), cold light (`GLOW_PRECURSOR`) in the seams. At this size the
//! surface pattern cuts the big faces into panels by itself; the geometry carries
//! what reads from kilometres off: long chamfered members, the dark recessed band
//! down every girder's flank (its "windows", split by pale ribs), stepped tiers,
//! open frames, and segments hovering apart over a line of light.
//!
//! Model space as for every prop: x along the heading, y left, z up, the origin on
//! the ground. The map levels a bench for each bastion and tower at their origin
//! (`mc-map/src/bake/machine.rs`), and a span stands on the bench it leaves from,
//! so decks meet at the same height. Footings go to -80 m under the bench.

use std::f32::consts::FRAC_PI_4;

use glam::{Affine3A, Vec3};

use super::builder::{MeshBuilder, Section};
use super::library::ModelDef;
use super::precursor::{cut_rect, dark, film, fine_rect, light, pale, panel, seam, v3};

pub(super) const MODELS: &[ModelDef] = &[
    ModelDef::new("precursor_bastion", 300.0, 422.0, bastion),
    ModelDef::new("precursor_boom", 400.0, 460.0, boom),
    ModelDef::new("precursor_tower", 110.0, 930.0, tower),
    ModelDef::new("precursor_span", 320.0, 105.0, span),
];

/// Deck height of a span over its bench, metres: where a bastion's first tier
/// and a tower's plinth take it in. `mc-map` lays spans to meet at this.
pub(super) const DECK_Z: f32 = 84.0;

/// A straight run from `a` along which members are laid out: `t` along it,
/// `s` across (to its left), `u` up (square to it, in the plane of `up`).
#[derive(Clone, Copy)]
pub(super) struct Run {
    pub(super) a: Vec3,
    pub(super) dir: Vec3,
    pub(super) side: Vec3,
    pub(super) up: Vec3,
    pub(super) len: f32,
}

impl Run {
    pub(super) fn new(a: Vec3, c: Vec3, up: Vec3) -> Run {
        let d = c - a;
        let len = d.length();
        let dir = d / len;
        let up = (up - dir * up.dot(dir)).normalize();
        Run { a, dir, side: up.cross(dir), up, len }
    }

    pub(super) fn at(&self, t: f32, s: f32, u: f32) -> Vec3 {
        self.a + self.dir * t + self.side * s + self.up * u
    }

    /// The section `plan` ((s, u) pairs) standing at `t`, lifted `du`.
    fn ring(&self, t: f32, plan: &[[f32; 2]], du: f32) -> Vec<Vec3> {
        plan.iter().map(|q| self.at(t, q[0], q[1] + du)).collect()
    }

    /// A prism of section `plan`, centred `du` up, from `t0` to `t1`.
    pub(super) fn solid(&self, b: &mut MeshBuilder, t0: f32, t1: f32, plan: &[[f32; 2]], du: f32) {
        b.loft(&[self.ring(t0, plan, du), self.ring(t1, plan, du)], true, true);
    }
}

/// A girder along `run` from `t0` to `t1`, `hw` half across and `hd` half deep:
/// pale flanges top and bottom over a dark web set back from them, so each flank
/// shows a dark band `band` either side of its middle, bridged by pale ribs every
/// `bay` metres with a line of light down it. `segment` breaks the flanges into
/// pieces that long with a dark joint between (0: unbroken). A plain pale bar at
/// the coarse level.
pub(super) fn girder(
    b: &mut MeshBuilder,
    run: &Run,
    t0: f32,
    t1: f32,
    hw: f32,
    hd: f32,
    band: f32,
    bay: f32,
    segment: f32,
) {
    let ch = (hw.min(hd) * 0.3).min(5.0);
    if !b.fine() {
        pale(b);
        let plan = cut_rect(b, hw, hd, ch);
        run.solid(b, t0, t1, &plan, 0.0);
        if b.mid() {
            // The dark band down each flank, painted on.
            dark(b);
            for s in [-1.0f32, 1.0] {
                let quad = [run.at(t0, s * hw, -band), run.at(t1, s * hw, -band), run.at(t1, s * hw, band), run.at(t0, s * hw, band)];
                film(b, &quad, run.side * s, 0.08);
            }
        }
        return;
    }
    let recess = (hw * 0.14).clamp(0.8, 2.2);
    dark(b);
    let web = fine_rect(b, hw - recess, band + 1.0, 0.8);
    run.solid(b, t0, t1, &web, 0.0);
    // The flanges: chamfered on their outer corners only.
    let fh = (hd - band) * 0.5;
    let fc = ch.min(fh * 0.9);
    let top: Vec<[f32; 2]> = if b.fine() {
        vec![[hw, -fh], [hw, fh - fc], [hw - fc, fh], [-hw + fc, fh], [-hw, fh - fc], [-hw, -fh]]
    } else {
        vec![[hw, -fh], [hw, fh], [-hw, fh], [-hw, -fh]]
    };
    let bottom: Vec<[f32; 2]> = top.iter().map(|p| [p[0], -p[1]]).collect();
    let pieces: Vec<(f32, f32)> = if segment > 0.0 {
        let n = ((t1 - t0) / segment).round().max(1.0);
        let each = (t1 - t0) / n;
        (0..n as usize)
            .map(|i| {
                let a = t0 + each * i as f32;
                (if i == 0 { a } else { a + 1.6 }, if i + 1 == n as usize { a + each } else { a + each - 1.6 })
            })
            .collect()
    } else {
        vec![(t0, t1)]
    };
    pale(b);
    for &(a, c) in &pieces {
        run.solid(b, a, c, &top, band + fh);
        run.solid(b, a, c, &bottom, -band - fh);
    }
    // Ribs across the band, and the light down it between them.
    let ribs = ((t1 - t0) / bay).floor() as usize;
    if b.mid() && ribs > 0 {
        let step = (t1 - t0) / (ribs as f32 + 1.0);
        let rib = [[hw - 0.3, -band - 0.5], [hw - 0.3, band + 0.5], [-hw + 0.3, band + 0.5], [-hw + 0.3, -band - 0.5]];
        for i in 1..=ribs {
            let t = t0 + step * i as f32;
            run.solid(b, t - 2.0, t + 2.0, &rib, 0.0);
        }
    }
    let face = hw - recess;
    for s in [-1.0f32, 1.0] {
        seam(b, run.at(t0 + 3.0, s * face, 0.0), run.at(t1 - 3.0, s * face, 0.0), run.side * s, (band * 0.35).clamp(0.8, 2.4));
    }
}

// ---- Bastion ---------------------------------------------------------------------
//
// The machine's footing, cut into a mountain on a bench of its own. A plinth 530 m by
// 360 m battered like a dam into the ground, ribbed with fins; on it, over a dark
// course with light in it, a body whose prow rakes back 100 m as it climbs to 230 m,
// a dark band with light in it down each flank and a tall portal in the prow under a
// visor cantilevered 90 m out; on its deck a block, and at the back an open frame
// 190 m high, its window 125 m across with a lit keystone hanging in it. Solid over
// the plinth.

const PLINTH: (f32, f32) = (250.0, 170.0);
const PLINTH_TOP: f32 = 70.0;
const BODY: (f32, f32) = (230.0, 150.0);
const BODY_BASE: f32 = 84.0;
const BODY_DECK: f32 = 230.0;
/// How the body draws in by its deck: x and y scale, and how far back the prow leans.
const BODY_TOP: (f32, f32, f32) = (0.78, 0.9, -50.0);
const BODY_CHAMFER: f32 = 34.0;
const FRAME_X: f32 = -120.0;
const FRAME_TOP: f32 = 422.0;

/// How far up the body `z` is, 0 at its base to 1 at its deck.
fn body_t(z: f32) -> f32 {
    ((z - BODY_BASE) / (BODY_DECK - BODY_BASE)).clamp(0.0, 1.0)
}

/// The body's prow face, its back face and its flank at height `z`.
fn body_at(z: f32) -> (f32, f32, f32) {
    let t = body_t(z);
    let sx = 1.0 + (BODY_TOP.0 - 1.0) * t;
    let sy = 1.0 + (BODY_TOP.1 - 1.0) * t;
    (BODY.0 * sx + BODY_TOP.2 * t, -BODY.0 * sx + BODY_TOP.2 * t, BODY.1 * sy)
}

fn bastion(b: &mut MeshBuilder, _tech: u8) {
    let (px, py) = PLINTH;
    let plan = cut_rect(b, px, py, 40.0);
    pale(b);
    b.loft_z(&plan, &[Section::new(-80.0, 1.1), Section::new(0.0, 1.06), Section::new(PLINTH_TOP, 1.0)]);

    // Fins up the plinth, on every face.
    if b.fine() {
        let fin: [[f32; 2]; 5] = [[-6.0, -20.0], [34.0, -20.0], [34.0, 0.0], [12.0, PLINTH_TOP - 2.0], [-6.0, PLINTH_TOP - 2.0]];
        let rows: [(f32, f32, &[f32]); 2] = [
            (0.0, px, &[-120.0, -60.0, 0.0, 60.0, 120.0]),
            (std::f32::consts::FRAC_PI_2, py, &[-200.0, -120.0, -40.0, 40.0, 120.0, 200.0]),
        ];
        for (turn, half, along) in rows {
            for flip in [0.0, std::f32::consts::PI] {
                for &y in along {
                    let at = Affine3A::from_rotation_z(turn + flip) * Affine3A::from_translation(v3(half * 1.06 - 3.0, y, 0.0));
                    b.with(at, |b| {
                        pale(b);
                        b.extrude_y_chamfered(&fin, 6.0, 1.8);
                        for s in [-6.0f32, 6.0] {
                            seam(b, v3(28.0, s, 5.0), v3(12.5, s, PLINTH_TOP - 7.0), v3(0.0, s.signum(), 0.0), 1.4);
                        }
                    });
                }
            }
        }
    }

    // The body, over a dark course with a line of light in it.
    let body = cut_rect(b, BODY.0, BODY.1, BODY_CHAMFER);
    let deck = Section::scaled(BODY_DECK, BODY_TOP.0, BODY_TOP.1).shifted(BODY_TOP.2, 0.0);
    if b.coarse() {
        pale(b);
        b.loft_z(&body, &[Section::new(PLINTH_TOP - 1.0, 1.0), Section::new(BODY_BASE, 1.0), deck]);
    } else {
        dark(b);
        b.loft_z(&body, &[Section::new(PLINTH_TOP - 1.0, 0.97), Section::new(BODY_BASE + 0.5, 0.97)]);
        light(b);
        b.loft_z(&body, &[Section::new(75.6, 0.974), Section::new(78.4, 0.974)]);
        pale(b);
        b.loft_z(&body, &[Section::new(BODY_BASE, 1.0), deck]);
    }

    // Down each flank, a dark band with light in it, and a line of light under it.
    let rise = BODY_DECK - BODY_BASE;
    let flank_out = v3(0.0, rise, BODY.1 * (1.0 - BODY_TOP.1)).normalize();
    let on_flank = |x_back: f32, x_front: f32, z: f32| {
        let (front, back, y) = body_at(z);
        let inset = BODY_CHAMFER + 26.0;
        (v3((back + inset).max(x_back), y, z), v3((front - inset).min(x_front), y, z))
    };
    b.mirror_y(|b| {
        let (z0, z1) = (150.0, 174.0);
        let (a0, c0) = on_flank(-1e3, 1e3, z0);
        let (a1, c1) = on_flank(-1e3, 1e3, z1);
        dark(b);
        panel(b, &[a0, c0, c1, a1], flank_out, 0.9, 0.0);
        if !b.coarse() {
            let (a, c) = on_flank(-1e3, 1e3, (z0 + z1) * 0.5);
            seam(b, a + v3(10.0, 0.0, 0.0) + flank_out * 0.9, c - v3(10.0, 0.0, 0.0) + flank_out * 0.9, flank_out, 3.0);
            let (a, c) = on_flank(-1e3, 1e3, 112.0);
            seam(b, a, c, flank_out, 2.2);
        }
    });

    // The prow: a tall portal with light up it and over it, and lines of light either side.
    let lean = BODY.0 * (1.0 - BODY_TOP.0) - BODY_TOP.2;
    let prow_out = v3(rise, 0.0, lean).normalize();
    let prow = |y: f32, z: f32| v3(body_at(z).0, y, z);
    let (z0, z1) = (94.0, 212.0);
    dark(b);
    panel(b, &[prow(-36.0, z0), prow(36.0, z0), prow(36.0, z1), prow(-36.0, z1)], prow_out, 0.9, 0.0);
    if !b.coarse() {
        let o = prow_out * 0.9;
        seam(b, prow(0.0, z0 + 8.0) + o, prow(0.0, z1 - 6.0) + o, prow_out, 4.0);
        seam(b, prow(-36.0, z1 + 4.0), prow(36.0, z1 + 4.0), prow_out, 2.4);
        for y in [-92.0f32, 92.0] {
            seam(b, prow(y, BODY_BASE + 10.0), prow(y, BODY_DECK - 10.0), prow_out, 2.4);
        }
    }

    // The visor over the prow, lit beneath at its lip.
    let (front, _, _) = body_at(BODY_DECK);
    pale(b);
    b.chamfered_box(v3(front + 20.0, 0.0, BODY_DECK + 6.0), v3(150.0, 214.0, 16.0), 4.0);
    if !b.coarse() {
        let z = BODY_DECK - 2.05;
        let (x0, x1) = (front - 50.0, front + 93.0);
        dark(b);
        film(b, &[v3(x0, -100.0, z), v3(x1, -100.0, z), v3(x1, 100.0, z), v3(x0, 100.0, z)], v3(0.0, 0.0, -1.0), 0.0);
        light(b);
        film(b, &[v3(x1 - 16.0, -100.0, z), v3(x1, -100.0, z), v3(x1, 100.0, z), v3(x1 - 16.0, 100.0, z)], v3(0.0, 0.0, -1.0), 0.05);
    }

    // A block on the deck, on a line of light.
    let block = cut_rect(b, 64.0, 84.0, 16.0);
    let on_deck = |z: f32, s: f32| Section::new(z, s).shifted(8.0, 0.0);
    pale(b);
    b.loft_z(&block, &[on_deck(BODY_DECK - 1.0, 1.0), on_deck(BODY_DECK + 30.0, 0.92)]);
    if !b.coarse() {
        light(b);
        b.loft_z(&block, &[on_deck(BODY_DECK - 0.5, 1.03), on_deck(BODY_DECK + 2.5, 1.025)]);
    }

    // The frame: two uprights and a lintel round an open window.
    pale(b);
    let x = FRAME_X;
    for y in [-84.0f32, 84.0] {
        b.chamfered_box(v3(x, y, (BODY_DECK + FRAME_TOP) * 0.5 - 2.0), v3(50.0, 44.0, FRAME_TOP - BODY_DECK + 4.0), 6.0);
    }
    b.chamfered_box(v3(x, 0.0, FRAME_TOP - 17.0), v3(54.0, 212.0, 34.0), 6.0);
    if !b.coarse() {
        for s in [-1.0f32, 1.0] {
            seam(b, v3(x, -s * 62.05, BODY_DECK + 8.0), v3(x, -s * 62.05, FRAME_TOP - 38.0), v3(0.0, s, 0.0), 4.0);
        }
        seam(b, v3(x, -58.0, FRAME_TOP - 34.1), v3(x, 58.0, FRAME_TOP - 34.1), v3(0.0, 0.0, -1.0), 4.0);
    }
    // The keystone, hanging in the window.
    light(b);
    b.chamfered_box(v3(x, 0.0, 330.0), v3(14.0, 36.0, 26.0), 3.0);
    if b.mid() {
        pale(b);
        for s in [-1.0f32, 1.0] {
            b.chamfered_box(v3(x, s * 31.0, 330.0), v3(20.0, 14.0, 42.0), 3.0);
        }
    }
}

// ---- Boom -------------------------------------------------------------------------
//
// The cantilever: two girders side by side leaning out of a shoulder block, kinked
// once at a lit knuckle and climbing 340 m over 670 m, their ends in housings with
// light in their faces. An underslung strut braces them to the knuckle and a slender
// rod runs out over them on two posts to a light at its end. Solid under the
// shoulder only: everything else is overhead.

const BOOM_Y: f32 = 36.0;
const BOOM_HW: f32 = 16.0;
const BOOM_HD: f32 = 27.0;
const BOOM_PATH: [(f32, f32); 3] = [(-40.0, 40.0), (330.0, 212.0), (630.0, 342.0)];
const ROD: [(f32, f32); 2] = [(-20.0, 132.0), (720.0, 470.0)];

/// Height of the girders' middle over `x`.
fn boom_z(x: f32) -> f32 {
    let [(x0, z0), (x1, z1), (x2, z2)] = BOOM_PATH;
    if x < x1 {
        z0 + (z1 - z0) * (x - x0) / (x1 - x0)
    } else {
        z1 + (z2 - z1) * (x - x1) / (x2 - x1)
    }
}

fn rod_z(x: f32) -> f32 {
    let [(x0, z0), (x1, z1)] = ROD;
    z0 + (z1 - z0) * (x - x0) / (x1 - x0)
}

fn boom(b: &mut MeshBuilder, _tech: u8) {
    // The shoulder.
    pale(b);
    let plan = cut_rect(b, 96.0, 84.0, 20.0);
    let at = |z: f32, s: f32| Section::new(z, s).shifted(-10.0, 0.0);
    b.loft_z(&plan, &[at(-80.0, 1.04), at(0.0, 1.0), at(70.0, 0.9)]);
    let top = cut_rect(b, 68.0, 64.0, 14.0);
    if !b.coarse() {
        dark(b);
        b.loft_z(&top, &[at(69.0, 1.08), at(80.0, 1.08)]);
        light(b);
        b.loft_z(&top, &[at(73.5, 1.1), at(75.5, 1.1)]);
    }
    pale(b);
    b.loft_z(&top, &[at(79.0, 1.0), at(104.0, 0.92)]);

    let [p0, p1, p2] = BOOM_PATH.map(|(x, z)| v3(x, 0.0, z));
    let (up, across) = (Vec3::Z, v3(0.0, 1.0, 0.0));
    for y in [-BOOM_Y, BOOM_Y] {
        let o = across * y;
        let lower = Run::new(p0 + o, p1 + o, up);
        let upper = Run::new(p1 + o, p2 + o, up);
        girder(b, &lower, 0.0, lower.len + 4.0, BOOM_HW, BOOM_HD, 7.5, 52.0, 0.0);
        girder(b, &upper, -4.0, upper.len, BOOM_HW, BOOM_HD, 7.5, 52.0, 0.0);
        // The knuckle, square to the bend.
        let knuckle = Run::new(p1 + o - (lower.dir + upper.dir) * 8.0, p1 + o + (lower.dir + upper.dir) * 8.0, up);
        dark(b);
        let collar = cut_rect(b, BOOM_HW + 2.5, BOOM_HD + 3.0, 4.0);
        knuckle.solid(b, 0.0, knuckle.len, &collar, 0.0);
        if b.fine() {
            light(b);
            let ring = cut_rect(b, BOOM_HW + 3.0, BOOM_HD + 3.5, 4.2);
            knuckle.solid(b, knuckle.len * 0.5 - 1.2, knuckle.len * 0.5 + 1.2, &ring, 0.0);
        }
        // The housing at the end, light in its face and under it.
        pale(b);
        let housing = cut_rect(b, BOOM_HW + 2.0, BOOM_HD + 2.5, 5.0);
        upper.solid(b, upper.len - 36.0, upper.len + 14.0, &housing, 0.0);
        light(b);
        let end = upper.len + 14.0;
        let face = [
            upper.at(end, -BOOM_HW + 3.0, -BOOM_HD + 5.0),
            upper.at(end, BOOM_HW - 3.0, -BOOM_HD + 5.0),
            upper.at(end, BOOM_HW - 3.0, BOOM_HD - 5.0),
            upper.at(end, -BOOM_HW + 3.0, BOOM_HD - 5.0),
        ];
        film(b, &face, upper.dir, 0.1);
        if b.fine() {
            let u = -BOOM_HD - 2.5;
            let under = [
                upper.at(end - 30.0, -4.0, u),
                upper.at(end - 4.0, -4.0, u),
                upper.at(end - 4.0, 4.0, u),
                upper.at(end - 30.0, 4.0, u),
            ];
            film(b, &under, -upper.up, 0.08);
        }
    }

    // Ties between the girders.
    dark(b);
    let tie = cut_rect(b, 7.0, 13.0, 2.0);
    let reach = BOOM_Y - BOOM_HW + 1.0;
    for x in [40.0f32, 190.0, 330.0, 470.0, 590.0] {
        let z = boom_z(x);
        let run = Run::new(v3(x, -reach, z), v3(x, reach, z), up);
        run.solid(b, 0.0, run.len, &tie, 0.0);
    }
    // The strut under them, up to the knuckle's tie.
    let strut = Run::new(v3(70.0, 0.0, 4.0), v3(318.0, 0.0, 182.0), up);
    girder(b, &strut, 0.0, strut.len, 11.0, 15.0, 5.0, 56.0, 0.0);

    // The rod, on two posts, with a light at its end.
    pale(b);
    let rod = Run::new(v3(ROD[0].0, 0.0, ROD[0].1), v3(ROD[1].0, 0.0, ROD[1].1), up);
    let bar = cut_rect(b, 5.0, 5.0, 1.5);
    rod.solid(b, 0.0, rod.len, &bar, 0.0);
    if b.fine() {
        dark(b);
        let post = cut_rect(b, 2.6, 4.0, 0.8);
        for x in [190.0f32, 470.0] {
            let run = Run::new(v3(x, 0.0, boom_z(x) + 12.0), v3(x, 0.0, rod_z(x)), v3(-1.0, 0.0, 0.0));
            run.solid(b, 0.0, run.len, &post, 0.0);
        }
    }
    light(b);
    let gem = cut_rect(b, 6.0, 6.0, 2.0);
    rod.solid(b, rod.len - 2.0, rod.len + 12.0, &gem, 0.0);
}

// ---- Tower ------------------------------------------------------------------------
//
// A spine into the clouds, 930 m: a plinth, then three stages of dark shaft between
// pale corner piers, each stage hovering over the one below on a neck of light, and a
// point hovering over the last. Four blade buttresses stand out of the corners at the
// foot. Light runs up the middle of every face; pale bands cross the dark between
// the piers.

const STAGES: [(f32, f32, f32, f32); 3] = [
    (50.0, 360.0, 48.0, 40.0),
    (378.0, 630.0, 38.0, 30.0),
    (648.0, 842.0, 28.0, 15.0),
];

fn tower(b: &mut MeshBuilder, _tech: u8) {
    pale(b);
    let plinth = cut_rect(b, 76.0, 76.0, 24.0);
    b.loft_z(&plinth, &[Section::new(-80.0, 1.0), Section::new(0.0, 1.0), Section::new(52.0, 0.86)]);
    if !b.coarse() {
        light(b);
        b.loft_z(&plinth, &[Section::new(18.0, 0.965), Section::new(21.0, 0.962)]);
    }

    for (i, &(z0, z1, h0, h1)) in STAGES.iter().enumerate() {
        let shrink = h1 / h0;
        // The shaft.
        if b.coarse() {
            pale(b);
        } else {
            dark(b);
        }
        let core = cut_rect(b, h0, h0, h0 * 0.28);
        b.loft_z(&core, &[Section::new(z0, 1.0), Section::new(z1, shrink)]);
        // The neck under it, lit.
        let below = if i == 0 { 50.0 } else { STAGES[i - 1].1 };
        if i > 0 {
            light(b);
            let neck = cut_rect(b, h0 * 0.5, h0 * 0.5, h0 * 0.15);
            b.loft_z(&neck, &[Section::new(below - 2.0, 1.0), Section::new(z0 + 2.0, 1.0)]);
        }
        if !b.fine() {
            continue;
        }
        // Corner piers standing out of the shaft.
        pale(b);
        let p = h0 * 0.24;
        let pier = fine_rect(b, p, p, p * 0.3);
        let c0 = h0 - p + 1.8;
        let c1 = (h0 - p + 1.8) * shrink;
        for (sx, sy) in [(1.0f32, 1.0f32), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
            b.loft_z(
                &pier,
                &[
                    Section::new(z0 - 0.5, 1.0).shifted(sx * c0, sy * c0),
                    Section::new(z1 + 0.5, shrink).shifted(sx * c1, sy * c1),
                ],
            );
        }
        // Light up the middle of each face; pale bands across it.
        let face = |z: f32| h0 + (h1 - h0) * (z - z0) / (z1 - z0);
        let out = v3(z1 - z0, 0.0, h0 - h1).normalize();
        b.radial(4, |b| {
            seam(b, v3(face(z0 + 6.0), 0.0, z0 + 6.0), v3(face(z1 - 6.0), 0.0, z1 - 6.0), out, h0 * 0.07);
            if b.fine() {
                pale(b);
                let mut z = z0 + 70.0;
                while z < z1 - 40.0 {
                    let (za, zb) = (z, z + 6.0);
                    let (wa, wb) = (face(za) - p * 1.5, face(zb) - p * 1.5);
                    let quad = [v3(face(za), -wa, za), v3(face(za), wa, za), v3(face(zb), wb, zb), v3(face(zb), -wb, zb)];
                    panel(b, &quad, out, 1.2, 0.4);
                    z += 90.0;
                }
            }
        });
    }

    // Buttress blades out of the corners at the foot.
    if b.mid() {
        let blade: [[f32; 2]; 6] = [[40.0, -30.0], [122.0, -30.0], [122.0, 18.0], [86.0, 150.0], [58.0, 290.0], [52.0, 276.0]];
        b.radial(4, |b| {
            b.with(Affine3A::from_rotation_z(FRAC_PI_4), |b| {
                pale(b);
                b.extrude_y_chamfered(&blade, 5.5, 1.6);
                if b.fine() {
                    for y in [-5.5f32, 5.5] {
                        seam(b, v3(112.0, y, 14.0), v3(84.0, y, 148.0), v3(0.0, y.signum(), 0.0), 1.6);
                        seam(b, v3(82.0, y, 162.0), v3(59.0, y, 272.0), v3(0.0, y.signum(), 0.0), 1.6);
                    }
                }
            });
        });
    }

    // The point, hovering over the last stage, lit beneath.
    let cap = cut_rect(b, 13.0, 13.0, 4.0);
    if !b.coarse() {
        light(b);
        b.loft_z(&cap, &[Section::new(850.0, 0.25), Section::new(862.0, 1.0)]);
    }
    pale(b);
    b.loft_z(&cap, &[Section::new(862.0, 1.0), Section::new(870.0, 1.0), Section::new(930.0, 0.0)]);
}

// ---- Span -------------------------------------------------------------------------
//
// A bridge girder 1000 m long from its origin (where it leaves a bench) along +x, its
// deck at `DECK_Z`: the flanks' dark band split by ribs, the pale flanges in 100 m
// segments with dark joints, a keel under it with light along its foot, parapets and
// a line of light down the deck. The far end is a face of light: spans laid toward
// each other from two benches stop a few metres short, and the light fills the gap.
// Nothing on the ground: the valley under it stays open.

const SPAN_LEN: f32 = 1_000.0;
const SPAN_HW: f32 = 32.0;
const SPAN_HD: f32 = 22.0;

fn span(b: &mut MeshBuilder, _tech: u8) {
    let run = Run::new(v3(0.0, 0.0, DECK_Z), v3(1.0, 0.0, DECK_Z), Vec3::Z);
    let (t0, t1) = (-40.0, SPAN_LEN);
    girder(b, &run, t0, t1, SPAN_HW, SPAN_HD, 8.0, 62.0, 125.0);
    // The keel.
    dark(b);
    let keel = cut_rect(b, 16.0, 10.0, 3.5);
    run.solid(b, t0, t1 - 6.0, &keel, -SPAN_HD - 8.0);
    if !b.coarse() {
        seam(b, run.at(t0 + 30.0, 0.0, -SPAN_HD - 18.0), run.at(t1 - 12.0, 0.0, -SPAN_HD - 18.0), -Vec3::Z, 3.6);
        // Parapets and the light down the deck.
        pale(b);
        let rail = cut_rect(b, 2.4, 3.5, 0.8);
        for s in [-SPAN_HW + 2.4, SPAN_HW - 2.4] {
            let rail_run = Run::new(run.at(0.0, s, SPAN_HD + 3.4), run.at(1.0, s, SPAN_HD + 3.4), Vec3::Z);
            rail_run.solid(b, t0, t1, &rail, 0.0);
        }
        let z = SPAN_HD + 0.05;
        seam(b, run.at(t0 + 20.0, 0.0, z), run.at(t1 - 8.0, 0.0, z), Vec3::Z, 3.5);
    }
    // The end: a face of light.
    light(b);
    let face = [
        run.at(t1, -SPAN_HW + 2.0, -SPAN_HD + 2.0),
        run.at(t1, SPAN_HW - 2.0, -SPAN_HD + 2.0),
        run.at(t1, SPAN_HW - 2.0, SPAN_HD - 2.0),
        run.at(t1, -SPAN_HW + 2.0, SPAN_HD - 2.0),
    ];
    film(b, &face, Vec3::X, 0.1);
}

/// Full-detail triangle budget for one megastructure piece: a handful stand on a map.
#[cfg(test)]
pub(super) const TRIANGLES: usize = 16_000;

#[cfg(test)]
mod tests {
    use super::super::build_model;
    use super::*;

    #[test]
    fn pieces_build_within_budget() {
        let mut reduced = Vec::new();
        for def in MODELS {
            let model = build_model(def.key).unwrap();
            let tris: Vec<usize> = model.lods.iter().map(|l| l.indices.len() / 3).collect();
            let low = model.lods[0].vertices.iter().map(|v| v.pos[2]).fold(f32::MAX, f32::min);
            let high = model.lods[0].vertices.iter().map(|v| v.pos[2]).fold(f32::MIN, f32::max);
            println!("{}: triangles {tris:?}, z {low:.0}..{high:.0}, bounds {:.0}", def.key, model.bounds_radius);
            assert!(tris[0] <= TRIANGLES, "{}: {} triangles", def.key, tris[0]);
            reduced.push((def.key, tris[1] as f32 <= tris[0] as f32 * 0.45 + 20.0));
            assert!(low >= -80.0, "{}: below the footing", def.key);
            for lod in &model.lods {
                for v in &lod.vertices {
                    assert!(v.pos.iter().chain(&v.normal).all(|c| c.is_finite()), "{}", def.key);
                    assert!((Vec3::from(v.normal).length() - 1.0).abs() < 1e-4, "{}: unit normal", def.key);
                }
            }
        }
        assert!(reduced.iter().all(|r| r.1), "reduced LODs too heavy: {reduced:?}");
    }
}

/// Software previews of the pieces: `MEGA_DUMP_DIR=... cargo test -p mc-render --lib
/// mega_previews -- --ignored`.
#[cfg(test)]
#[test]
#[ignore]
fn mega_previews() {
    let dir = std::path::PathBuf::from(std::env::var_os("MEGA_DUMP_DIR").expect("MEGA_DUMP_DIR"));
    std::fs::create_dir_all(&dir).unwrap();
    for def in MODELS {
        let model = super::build_model(def.key).unwrap();
        for (lod, yaw) in [(0usize, -38.0f32), (0, 142.0), (1, -38.0)] {
            super::preview::render(&model.lods[lod], 768, yaw)
                .write_ppm(&dir.join(format!("{}_{lod}_{}.ppm", def.key, yaw as i32)))
                .unwrap();
        }
    }
}
