//! The Threshold's working halls: the pieces of the Precursor facility that make
//! things (`mc_map::PropKind::Precursor{Forge,Cradle,SeaGate,Rampart}`, laid by the
//! Threshold survival map). Print halls whose bays open on the parade ground, berths
//! where Shapers form under reaching arms, a harbour under a great arch that prints
//! ships, and the stepped rampart that clads the plateau they stand on.
//!
//! Same kit and language as the megastructure (`precursor_mega.rs`): pale alloy over
//! a dark core, cold light in the seams; chamfered members, dark recessed bands split
//! by pale ribs, stepped tiers, segments hovering apart over lines of light. Model
//! space as for every prop: x along the heading (every piece faces +x), y left, z
//! up, the origin on the ground. Footings go to -80 m. The contract numbers (bays,
//! slots, slips, emitters, clear zones) are exported below and mirrored in mc-map's
//! `solid_plan()`.

use std::f32::consts::{FRAC_PI_2, TAU};

use glam::{Affine3A, Vec2, Vec3};

use super::builder::{MeshBuilder, Section};
use super::library::ModelDef;
use super::precursor::{cut_rect, dark, film, fine_rect, key_light, key_seam, light, pale, panel, seam, v3};
use super::precursor_mega::{girder, Run};

pub(super) const MODELS: &[ModelDef] = &[
    ModelDef::new("precursor_forge", 456.0, FORGE_TOP, forge),
    ModelDef::new("precursor_cradle", 200.0, CRADLE_TOP, cradle),
    ModelDef::new("precursor_seagate", 505.0, SEAGATE_TOP, seagate),
    ModelDef::new("precursor_rampart", 150.0, RAMPART_MERLON_TOP, rampart),
];

/// Full-detail triangle budget per model: one map, a few of each, hundreds of metres high.
pub(super) const TRIANGLES: usize = 60_000;

// ---- Kit ----------------------------------------------------------------------------

/// A flat face: `o` a point on it, `u` along it, `v` up it (need not be unit: a
/// battered face takes `v` with its lean so `t` stays height), `n` out of it.
#[derive(Clone, Copy)]
struct Face {
    o: Vec3,
    u: Vec3,
    v: Vec3,
    n: Vec3,
}

impl Face {
    /// An upright face through `o` looking along `n`, `s` running along `u`, `t` up.
    fn wall(o: Vec3, u: Vec3, n: Vec3) -> Face {
        Face { o, u, v: n.cross(u), n }
    }

    fn at(&self, s: f32, t: f32) -> Vec3 {
        self.o + self.u * s + self.v * t
    }

    fn quad(&self, s0: f32, t0: f32, s1: f32, t1: f32) -> [Vec3; 4] {
        [self.at(s0, t0), self.at(s1, t0), self.at(s1, t1), self.at(s0, t1)]
    }

    fn lift(&self, d: f32) -> Face {
        Face { o: self.o + self.n * d, ..*self }
    }
}

/// A dark band on face `f` from `s.0` to `s.1` along it and `t.0` to `t.1` up it,
/// set between pale rails that stand proud of it and split by pale ribs about every
/// `every` metres (none if 0), a line of light `glow` wide down its middle (none if 0).
fn windows(b: &mut MeshBuilder, f: &Face, s: (f32, f32), t: (f32, f32), every: f32, glow: f32) {
    let ((s0, s1), (t0, t1)) = (s, t);
    dark(b);
    panel(b, &f.quad(s0, t0, s1, t1), f.n, 0.5, 0.0);
    if b.coarse() {
        return;
    }
    if glow > 0.0 {
        let on = f.lift(0.5);
        let m = (t0 + t1) * 0.5;
        seam(b, on.at(s0 + 2.0, m), on.at(s1 - 2.0, m), f.n, glow);
    }
    if !b.fine() {
        return;
    }
    pale(b);
    panel(b, &f.quad(s0, t1, s1, t1 + 1.8), f.n, 2.2, 0.5);
    panel(b, &f.quad(s0, t0 - 1.8, s1, t0), f.n, 2.2, 0.5);
    if every > 0.0 {
        let n = ((s1 - s0) / every).round().max(1.0) as usize;
        let step = (s1 - s0) / n as f32;
        for i in 1..n {
            let s = s0 + step * i as f32;
            panel(b, &f.quad(s - 1.2, t0, s + 1.2, t1), f.n, 1.7, 0.4);
        }
    }
}

/// Blocks hovering in a row from `a` to `c` over a line of light laid on the surface
/// there: each `across` wide and `high` tall, about `seg` long with `gap` between,
/// their undersides `lift` over the surface.
#[allow(clippy::too_many_arguments)]
fn hover_row(b: &mut MeshBuilder, a: Vec3, c: Vec3, across: f32, high: f32, seg: f32, gap: f32, lift: f32) {
    let run = Run::new(a, c, Vec3::Z);
    let n = ((run.len + gap) / (seg + gap)).round().max(1.0);
    let each = (run.len - gap * (n - 1.0)) / n;
    light(b);
    let line = cut_rect(b, across * 0.22, 0.6, 0.2);
    run.solid(b, 0.0, run.len, &line, 0.6);
    pale(b);
    let block = cut_rect(b, across * 0.5, high * 0.5, (across.min(high) * 0.2).min(3.0));
    for i in 0..n as usize {
        let t0 = (each + gap) * i as f32;
        run.solid(b, t0, t0 + each, &block, lift + high * 0.5);
    }
}

/// Plan (x, y) of the rectangle from `x0` to `x1` by `-hy` to `hy`, its corners at the
/// `x0` end cut by `rear` and at the `x1` end by `front` (0: square). Square at the
/// coarse level.
fn slab_plan(b: &MeshBuilder, x0: f32, x1: f32, hy: f32, rear: f32, front: f32) -> Vec<[f32; 2]> {
    let (rear, front) = if b.coarse() { (0.0, 0.0) } else { (rear, front) };
    let mut p = Vec::with_capacity(8);
    let mut corner = |c: [f32; 2], cut: f32, a: [f32; 2], d: [f32; 2]| {
        if cut > 0.0 {
            p.push([c[0] + a[0] * cut, c[1] + a[1] * cut]);
            p.push([c[0] + d[0] * cut, c[1] + d[1] * cut]);
        } else {
            p.push(c);
        }
    };
    corner([x1, -hy], front, [-1.0, 0.0], [0.0, 1.0]);
    corner([x1, hy], front, [0.0, -1.0], [-1.0, 0.0]);
    corner([x0, hy], rear, [1.0, 0.0], [0.0, -1.0]);
    corner([x0, -hy], rear, [0.0, 1.0], [1.0, 0.0]);
    p
}

fn slab(b: &mut MeshBuilder, plan: &[[f32; 2]], z0: f32, z1: f32) {
    b.loft_z(plan, &[Section::new(z0, 1.0), Section::new(z1, 1.0)]);
}

/// A projector head, its lens at the origin looking down -z, its body rising
/// `PROJECTOR_LEN * s` up +z to where it hangs from its mount.
fn projector(b: &mut MeshBuilder, s: f32) {
    // The lens is live: the one working part of the head.
    key_light(b);
    let lens = fine_rect(b, 5.0 * s, 5.0 * s, 1.8 * s);
    b.loft_z(&lens, &[Section::new(0.0, 0.7), Section::new(2.6 * s, 1.0)]);
    let body = cut_rect(b, 9.0 * s, 9.0 * s, 3.2 * s);
    pale(b);
    b.loft_z(
        &body,
        &[Section::new(2.2 * s, 0.6), Section::new(7.0 * s, 1.0), Section::new(15.0 * s, 1.0), Section::new(PROJECTOR_LEN * s, 0.62)],
    );
    if b.coarse() {
        return;
    }
    dark(b);
    b.loft_z(&body, &[Section::new(9.4 * s, 1.07), Section::new(12.6 * s, 1.07)]);
    light(b);
    b.loft_z(&body, &[Section::new(10.4 * s, 1.09), Section::new(11.6 * s, 1.09)]);
    if !b.fine() {
        return;
    }
    // Four petals splayed round the lens, a line of light down each.
    let petal = [[8.0 * s, 3.2 * s], [10.4 * s, 0.6 * s], [13.4 * s, 8.0 * s], [12.2 * s, 16.0 * s], [9.0 * s, 14.0 * s]];
    let edge = v3(3.0, 0.0, 7.4).normalize();
    let out = v3(edge.z, 0.0, -edge.x);
    b.radial(4, |b| {
        pale(b);
        b.extrude_y_chamfered(&petal, 2.2 * s, 0.7 * s);
        seam(b, v3(10.9 * s, 0.0, 1.8 * s) + out * 0.02, v3(13.0 * s, 0.0, 7.0 * s) + out * 0.02, out, 1.0 * s);
    });
    // The mount on top.
    dark(b);
    let neck = cut_rect(b, 3.6 * s, 3.6 * s, 1.0 * s);
    b.loft_z(&neck, &[Section::new(PROJECTOR_LEN * s - 0.5, 1.0), Section::new((PROJECTOR_LEN + 2.5) * s, 1.0)]);
}

/// Length of a projector head at scale 1, lens to mount.
const PROJECTOR_LEN: f32 = 19.0;

/// A shaft from `z0` to `z1` centred at (`x`, `y`), `hx` by `hy` half: a dark core
/// between pale corner piers, light up the middle of every face and pale bands across.
fn pier_tower(b: &mut MeshBuilder, x: f32, y: f32, hx: f32, hy: f32, z0: f32, z1: f32) {
    let at = |z: f32| Section::new(z, 1.0).shifted(x, y);
    if b.coarse() {
        pale(b);
        let core = cut_rect(b, hx, hy, 0.0);
        b.loft_z(&core, &[at(z0), at(z1)]);
        return;
    }
    dark(b);
    let core = cut_rect(b, hx - 1.5, hy - 1.5, 2.0);
    b.loft_z(&core, &[at(z0), at(z1)]);
    pale(b);
    let p = (hx.min(hy) * 0.26).max(4.0);
    let pier = fine_rect(b, p, p, p * 0.3);
    for (sx, sy) in [(1.0f32, 1.0f32), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
        let c = |z: f32| Section::new(z, 1.0).shifted(x + sx * (hx - p), y + sy * (hy - p));
        b.loft_z(&pier, &[c(z0 - 0.5), c(z1 + 0.5)]);
    }
    let faces = [(Vec3::X, Vec3::Y, hy, hx), (-Vec3::X, -Vec3::Y, hy, hx), (Vec3::Y, -Vec3::X, hx, hy), (-Vec3::Y, Vec3::X, hx, hy)];
    for (n, u, along, out) in faces {
        let f = Face::wall(v3(x, y, 0.0) + n * (out - 1.5), u, n);
        seam(b, f.at(0.0, z0 + 6.0), f.at(0.0, z1 - 6.0), n, (along * 0.12).clamp(1.2, 3.5));
        if b.fine() {
            pale(b);
            let w = along - 2.0 * p;
            let mut z = z0 + 28.0;
            while z < z1 - 16.0 {
                panel(b, &f.quad(-w, z, w, z + 5.0), n, 1.3, 0.4);
                z += 56.0;
            }
        }
    }
}

/// A ring of light `width` wide round (`x`, `y`) at radius `r` on the floor at `z`, in
/// `n` straight lengths.
fn floor_ring(b: &mut MeshBuilder, x: f32, y: f32, z: f32, r: f32, n: usize, width: f32, live: bool) {
    let p = |a: f32| v3(x + r * a.cos(), y + r * a.sin(), z);
    for i in 0..n {
        let (a0, a1) = (TAU * (i as f32 + 0.5) / n as f32, TAU * (i as f32 + 1.5) / n as f32);
        if live {
            live_line(b, p(a0), p(a1), width);
        } else {
            seam(b, p(a0), p(a1), Vec3::Z, width);
        }
    }
}

/// A live line of light `width` wide on the floor from `a` to `c`: a flat film, the
/// working marks of a print bay or a socket.
fn live_line(b: &mut MeshBuilder, a: Vec3, c: Vec3, width: f32) {
    let side = (c - a).cross(Vec3::Z).normalize() * width * 0.5;
    key_light(b);
    film(b, &[a - side, c - side, c + side, a + side], Vec3::Z, 0.12);
}

/// A bar from `a` to `c`, `hw` half across and `hd` half deep, without end caps: the
/// coarse level's members.
fn open_bar(b: &mut MeshBuilder, a: Vec3, c: Vec3, up: Vec3, hw: f32, hd: f32) {
    let run = Run::new(a, c, up);
    let ring = |t: f32| vec![run.at(t, -hw, -hd), run.at(t, hw, -hd), run.at(t, hw, hd), run.at(t, -hw, hd)];
    b.loft(&[ring(0.0), ring(run.len)], false, false);
}

// ---- Forge --------------------------------------------------------------------------
//
// A print hall 720 m across and 318 m high, facing +x. Four print bays open on its
// face, each 130 m wide and 150 m deep between piers, under a lintel that overhangs
// the bay mouths out to x = +40 from 150 m up. Each bay: a projector head hanging from
// its coffered ceiling, aimed at the stand on the lit floor where a unit is printed,
// and a dark back wall ribbed in pale with light up its middle. Behind, the hall's
// block, battered at the back like a dam with blade fins up it; on top a tier over
// the whole hall with a lantern over each bay, segments hovering along the pier lines
// between them, a second tier over the block and a crest hovering over a line of light.

/// Middle of each print bay across the hall (y); the bays open on the +x face.
pub(super) const FORGE_BAYS: [f32; 4] = [-255.0, -85.0, 85.0, 255.0];
/// Half the width of a bay.
pub(super) const FORGE_BAY_HALF: f32 = 65.0;
/// The bay's back wall (x): bays are open ground from here out.
pub(super) const FORGE_BAY_BACK: f32 = -150.0;
/// Where a printed unit stands (x, in its bay's middle).
pub(super) const FORGE_STAND: f32 = -40.0;
/// Where a bay's print beam leaves its projector head: (x, z), y the bay's.
pub(super) const FORGE_EMITTER: (f32, f32) = (-115.0, 110.0);
/// Nothing in a bay below this.
pub(super) const FORGE_CLEAR: f32 = 100.0;
const _: () = assert!(FORGE_EMITTER.1 > FORGE_CLEAR + 5.0);
const FORGE_PIERS: [f32; 5] = [-340.0, -170.0, 0.0, 170.0, 340.0];
const FORGE_HALF: f32 = 360.0;
const FORGE_REAR: f32 = -280.0;
/// The block's back face at its foot and at the eave: battered like a dam.
const FORGE_BODY_REAR: (f32, f32) = (-262.0, -238.0);
const FORGE_BODY_BASE: f32 = 24.0;
const LINTEL_Z: f32 = 150.0;
const LINTEL_FRONT: f32 = 40.0;
const FORGE_EAVE: f32 = 244.0;
const FORGE_TOP: f32 = 318.0;

/// The block's back face at height `z`.
fn forge_rear(z: f32) -> f32 {
    let (x0, x1) = FORGE_BODY_REAR;
    x0 + (x1 - x0) * (z - FORGE_BODY_BASE) / (FORGE_EAVE - FORGE_BODY_BASE)
}

fn forge(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        forge_coarse(b);
        return;
    }
    forge_block(b);
    for y in FORGE_PIERS {
        forge_pier(b, y);
    }
    forge_lintel(b);
    for y in FORGE_BAYS {
        forge_bay(b, y);
    }
    forge_roof(b);
}

fn forge_coarse(b: &mut MeshBuilder) {
    pale(b);
    let profile = [
        [FORGE_REAR, -20.0],
        [FORGE_BAY_BACK, -20.0],
        [FORGE_BAY_BACK, LINTEL_Z],
        [LINTEL_FRONT, LINTEL_Z],
        [LINTEL_FRONT, FORGE_EAVE],
        [0.0, 280.0],
        [FORGE_BODY_REAR.1, 300.0],
        [FORGE_REAR, 16.0],
    ];
    b.extrude_y(&profile, -FORGE_HALF, FORGE_HALF);
    for y in FORGE_PIERS {
        film(b, &[v3(0.0, y - 20.0, -5.0), v3(0.0, y + 20.0, -5.0), v3(0.0, y + 20.0, LINTEL_Z), v3(0.0, y - 20.0, LINTEL_Z)], Vec3::X, 0.0);
        for s in [-1.0f32, 1.0] {
            let y = y + s * 20.0;
            film(b, &[v3(FORGE_BAY_BACK, y, -5.0), v3(0.0, y, -5.0), v3(0.0, y, LINTEL_Z), v3(FORGE_BAY_BACK, y, LINTEL_Z)], Vec3::Y * s, 0.0);
        }
    }
}

/// The block behind the bays: a plinth, a dark course with light in it, the body
/// battered back to the eave, blade fins up its back, bands across it and its ends.
fn forge_block(b: &mut MeshBuilder) {
    pale(b);
    let plinth = slab_plan(b, FORGE_REAR, FORGE_BAY_BACK, FORGE_HALF, 12.0, 0.0);
    slab(b, &plinth, -80.0, 16.0);
    dark(b);
    let course = slab_plan(b, FORGE_BODY_REAR.0 - 4.0, FORGE_BAY_BACK, FORGE_HALF - 2.0, 10.0, 0.0);
    slab(b, &course, 15.5, FORGE_BODY_BASE + 0.5);
    seam(b, v3(FORGE_BODY_REAR.0 - 4.0, -340.0, 20.0), v3(FORGE_BODY_REAR.0 - 4.0, 340.0, 20.0), -Vec3::X, 2.4);
    pale(b);
    let ring = |b: &MeshBuilder, x0: f32, z: f32| {
        slab_plan(b, x0, FORGE_BAY_BACK, FORGE_HALF, 22.0, 0.0).iter().map(|p| v3(p[0], p[1], z)).collect::<Vec<_>>()
    };
    let rings = [ring(b, FORGE_BODY_REAR.0, FORGE_BODY_BASE), ring(b, FORGE_BODY_REAR.1, FORGE_EAVE)];
    b.loft(&rings, true, true);

    // Bands across the battered back.
    let lean = (FORGE_BODY_REAR.1 - FORGE_BODY_REAR.0) / (FORGE_EAVE - FORGE_BODY_BASE);
    let back = Face {
        o: v3(forge_rear(0.0), 0.0, 0.0),
        u: -Vec3::Y,
        v: v3(lean, 0.0, 1.0),
        n: v3(-1.0, 0.0, lean).normalize(),
    };
    windows(b, &back, (-336.0, 336.0), (150.0, 174.0), 0.0, 3.0);
    windows(b, &back, (-336.0, 336.0), (46.0, 60.0), 0.0, 2.2);
    if b.fine() {
        // Between the fins, a tall slot of light over the upper band.
        for i in 0..10 {
            let y = -288.0 + 64.0 * i as f32;
            seam(b, back.at(-y, 184.0), back.at(-y, 226.0), back.n, 2.4);
            seam(b, back.at(-y, 70.0), back.at(-y, 138.0), back.n, 1.4);
        }
    }

    // The ends: a tall dark field with light up it and pale ribs across it.
    b.mirror_y(|b| {
        let end = Face::wall(v3(0.0, FORGE_HALF, 0.0), -Vec3::X, Vec3::Y);
        dark(b);
        panel(b, &end.quad(162.0, 40.0, 226.0, 228.0), Vec3::Y, 0.6, 0.0);
        let on = end.lift(0.6);
        seam(b, on.at(194.0, 48.0), on.at(194.0, 220.0), Vec3::Y, 4.0);
        if b.fine() {
            pale(b);
            for s in [174.0f32, 214.0] {
                panel(b, &end.quad(s - 2.0, 40.0, s + 2.0, 228.0), Vec3::Y, 2.0, 0.6);
            }
            for s in [160.0f32, 228.0] {
                panel(b, &end.quad(s - 2.0, 36.0, s + 2.0, 232.0), Vec3::Y, 2.6, 0.6);
            }
            panel(b, &end.quad(158.0, 228.0, 230.0, 232.0), Vec3::Y, 2.6, 0.6);
            panel(b, &end.quad(158.0, 36.0, 230.0, 40.0), Vec3::Y, 2.6, 0.6);
        }
    });

    // Blade fins up the back, standing on the plinth and leaning into the batter.
    if b.mid() {
        let fin = [[-252.0, 14.0], [FORGE_REAR + 1.0, 14.0], [FORGE_REAR + 1.0, 44.0], [-255.0, 172.0], [-241.0, 238.0], [-236.0, 238.0]];
        for i in 0..11 {
            let y = -320.0 + 64.0 * i as f32;
            b.at(v3(0.0, y, 0.0), |b| {
                pale(b);
                b.extrude_y_chamfered(&fin, 7.0, 2.0);
                if b.fine() {
                    for s in [-7.0f32, 7.0] {
                        seam(b, v3(FORGE_REAR + 5.0, s, 40.0), v3(-258.0, s, 166.0), v3(0.0, s.signum(), 0.0), 1.8);
                        seam(b, v3(-254.0, s, 184.0), v3(-244.0, s, 226.0), v3(0.0, s.signum(), 0.0), 1.4);
                    }
                }
            });
        }
    }
}

/// One pier between bays, x from the block to 0, 40 m across: pale plinth, capital,
/// nose and back post framing a window each side where the dark core shows, pale ribs
/// up it and a line of light along it; a lit slot up the nose.
fn forge_pier(b: &mut MeshBuilder, y: f32) {
    let hw = 20.0;
    dark(b);
    b.cuboid(v3(-77.0, y, 35.0), v3(146.0, 2.0 * hw - 5.0, 230.0));
    pale(b);
    b.chamfered_box(v3(-75.0, y, -32.0), v3(150.0, 2.0 * hw, 96.0), 4.0);
    b.chamfered_box(v3(-75.0, y, 137.0), v3(150.0, 2.0 * hw, 26.0), 4.0);
    b.chamfered_box(v3(-12.0, y, 35.0), v3(24.0, 2.0 * hw, 230.0), 5.0);
    b.chamfered_box(v3(-141.0, y, 70.0), v3(18.0, 2.0 * hw, 108.0), 3.0);
    // The window each side.
    for s in [-1.0f32, 1.0] {
        let yf = y + s * (hw - 2.5);
        seam(b, v3(-130.0, yf, 70.0), v3(-26.0, yf, 70.0), Vec3::Y * s, 3.0);
        seam(b, v3(-130.0, y + s * hw, 9.0), v3(-26.0, y + s * hw, 9.0), Vec3::Y * s, 1.6);
        if b.fine() {
            seam(b, v3(-130.0, yf, 119.0), v3(-26.0, yf, 119.0), Vec3::Y * s, 1.4);
        }
    }
    if b.fine() {
        pale(b);
        for x in [-105.0f32, -78.0, -51.0] {
            b.chamfered_box(v3(x, y, 70.0), v3(4.0, 2.0 * hw - 2.0, 108.0), 1.0);
        }
    }
    // The nose: a dark slot with light up it, light across its joints.
    let nose = Face::wall(v3(0.0, y, 0.0), Vec3::Y, Vec3::X);
    dark(b);
    panel(b, &nose.quad(-6.0, 22.0, 6.0, 118.0), Vec3::X, 0.6, 0.0);
    seam(b, nose.lift(0.6).at(0.0, 28.0), nose.lift(0.6).at(0.0, 112.0), Vec3::X, 3.0);
    if b.fine() {
        for t in [17.0f32, 124.0] {
            seam(b, nose.at(-14.0, t), nose.at(14.0, t), Vec3::X, 1.6);
        }
        pale(b);
        panel(b, &nose.quad(-9.0, 130.0, 9.0, 146.0), Vec3::X, 1.6, 0.6);
    }
}

/// The lintel over the bay mouths: a lower course out to x = +40 with a band of
/// windows along its face, a dark course with light in it, an upper course set back,
/// a keystone over every bay and light under every mouth.
fn forge_lintel(b: &mut MeshBuilder) {
    let (z0, z1, z2, z3) = (LINTEL_Z, 200.0, 208.0, FORGE_EAVE);
    pale(b);
    let low = slab_plan(b, FORGE_BAY_BACK - 2.0, LINTEL_FRONT, FORGE_HALF, 0.0, 8.0);
    slab(b, &low, z0, z1);
    dark(b);
    let course = slab_plan(b, FORGE_BAY_BACK - 2.0, LINTEL_FRONT - 6.0, FORGE_HALF - 3.0, 0.0, 6.0);
    slab(b, &course, z1 - 0.5, z2 + 0.5);
    seam(b, v3(LINTEL_FRONT - 6.0, -350.0, 204.0), v3(LINTEL_FRONT - 6.0, 350.0, 204.0), Vec3::X, 3.0);
    pale(b);
    let high = slab_plan(b, FORGE_BAY_BACK - 2.0, LINTEL_FRONT - 12.0, FORGE_HALF, 0.0, 8.0);
    slab(b, &high, z2, z3);

    let front = Face::wall(v3(LINTEL_FRONT, 0.0, 0.0), Vec3::Y, Vec3::X);
    windows(b, &front, (-350.0, 350.0), (162.0, 188.0), 28.0, 3.0);
    b.mirror_y(|b| {
        let end = Face::wall(v3(0.0, FORGE_HALF, 0.0), -Vec3::X, Vec3::Y);
        windows(b, &end, (-36.0, 146.0), (162.0, 188.0), 45.0, 3.0);
        seam(b, end.at(-28.0, 204.0), end.at(146.0, 204.0), Vec3::Y, 2.4);
    });
    // Pilasters over the piers, light up them.
    let upper = Face::wall(v3(LINTEL_FRONT - 12.0, 0.0, 0.0), Vec3::Y, Vec3::X);
    for y in FORGE_PIERS {
        if b.fine() {
            pale(b);
            panel(b, &front.quad(y - 7.0, 152.0, y + 7.0, 198.0), Vec3::X, 3.4, 0.8);
        }
        seam(b, upper.at(y, 212.0), upper.at(y, 240.0), Vec3::X, 3.0);
    }
    for y in FORGE_BAYS {
        // A keystone hovering over a dark recess with light behind it.
        dark(b);
        panel(b, &upper.quad(y - 50.0, 213.0, y + 50.0, 239.0), Vec3::X, 0.6, 0.0);
        light(b);
        panel(b, &upper.quad(y - 20.0, 215.0, y + 20.0, 237.0), Vec3::X, 0.8, 0.0);
        pale(b);
        b.chamfered_box(v3(LINTEL_FRONT - 12.0 + 4.5, y, 226.0), v3(5.0, 34.0, 28.0), 3.0);
        if b.fine() {
            for s in [-1.0f32, 1.0] {
                b.chamfered_box(v3(LINTEL_FRONT - 12.0 + 3.0, y + s * 36.0, 226.0), v3(3.0, 12.0, 22.0), 1.5);
            }
        }
        // Light under the mouth's lip.
        light(b);
        let z = LINTEL_Z - 0.05;
        let (xa, xc) = (LINTEL_FRONT - 16.0, LINTEL_FRONT - 3.0);
        film(b, &[v3(xa, y - 60.0, z), v3(xc, y - 60.0, z), v3(xc, y + 60.0, z), v3(xa, y + 60.0, z)], -Vec3::Z, 0.0);
    }
}

/// One print bay: the back wall, the coffered ceiling, the projector head and the lit floor.
fn forge_bay(b: &mut MeshBuilder, y: f32) {
    let hy = FORGE_BAY_HALF;
    let back = Face::wall(v3(FORGE_BAY_BACK, y, 0.0), Vec3::Y, Vec3::X);
    dark(b);
    panel(b, &back.quad(-hy + 2.0, 2.0, hy - 2.0, 148.0), Vec3::X, 0.8, 0.0);
    let on = back.lift(0.8);
    seam(b, on.at(0.0, 14.0), on.at(0.0, 136.0), Vec3::X, 5.0);
    for t in [26.0f32, 128.0] {
        seam(b, on.at(-60.0, t), on.at(-14.0, t), Vec3::X, 2.2);
        seam(b, on.at(14.0, t), on.at(60.0, t), Vec3::X, 2.2);
    }
    if b.fine() {
        pale(b);
        for s in [24.0f32, 42.0, 58.0] {
            for s in [-s, s] {
                panel(b, &back.quad(s - 2.0, 6.0, s + 2.0, 146.0), Vec3::X, 3.0, 0.6);
            }
        }
        // A frame round the slot and a kerb along the foot.
        for s in [-8.0f32, 8.0] {
            panel(b, &back.quad(s - 1.6, 10.0, s + 1.6, 140.0), Vec3::X, 3.6, 0.6);
        }
        panel(b, &back.quad(-9.6, 140.0, 9.6, 144.0), Vec3::X, 3.6, 0.6);
        panel(b, &back.quad(-hy + 2.0, 0.0, hy - 2.0, 5.0), Vec3::X, 2.6, 0.6);
    }

    // The ceiling: dark, lines of light along it, pale ribs across.
    let ceiling = Face { o: v3(0.0, y, LINTEL_Z), u: Vec3::X, v: -Vec3::Y, n: -Vec3::Z };
    dark(b);
    panel(b, &ceiling.quad(FORGE_BAY_BACK + 2.0, -hy + 1.0, LINTEL_FRONT - 17.0, hy - 1.0), -Vec3::Z, 0.6, 0.0);
    let under = ceiling.lift(0.6);
    for t in [-40.0f32, 40.0] {
        seam(b, under.at(FORGE_BAY_BACK + 4.0, t), under.at(LINTEL_FRONT - 19.0, t), -Vec3::Z, 2.4);
    }
    if b.fine() {
        pale(b);
        for x in [-140.0f32, -100.0, -60.0, -20.0, 12.0] {
            b.chamfered_box(v3(x, y, LINTEL_Z - 4.0), v3(4.0, 2.0 * hy - 1.0, 8.0), 1.0);
        }
    }

    // The projector head, aimed at the stand, hanging from its carriage.
    let (ex, ez) = FORGE_EMITTER;
    let s = 1.4;
    let tilt = (ex - FORGE_STAND).atan2(ez);
    let top = v3(ex + PROJECTOR_LEN * s * tilt.sin(), y, ez + PROJECTOR_LEN * s * tilt.cos());
    b.with(Affine3A::from_translation(v3(ex, y, ez)) * Affine3A::from_rotation_y(tilt), |b| projector(b, s));
    dark(b);
    let mast = cut_rect(b, 4.5, 4.5, 1.2);
    b.loft_z(&mast, &[Section::new(top.z - 4.0, 1.0).shifted(top.x, y), Section::new(LINTEL_Z - 8.0, 1.0).shifted(top.x, y)]);
    pale(b);
    b.chamfered_box(v3(top.x, y, LINTEL_Z - 5.5), v3(26.0, 28.0, 11.0), 3.0);
    if b.fine() {
        light(b);
        b.chamfered_box(v3(top.x, y, LINTEL_Z - 11.3), v3(16.0, 18.0, 0.8), 2.0);
        // The rails it runs on.
        pale(b);
        for s in [-1.0f32, 1.0] {
            b.cuboid(v3(-75.0, y + s * 16.0, LINTEL_Z - 1.8), v3(140.0, 3.0, 3.6));
        }
    }

    // The floor: dark, a lit ring round the stand, lines out of the mouth.
    let plate_x = (FORGE_BAY_BACK + LINTEL_FRONT) * 0.5;
    dark(b);
    b.plate(v3(plate_x, y, 0.0), Vec2::new(LINTEL_FRONT - FORGE_BAY_BACK, 2.0 * hy - 2.0), 0.3, 0.2);
    let z = 0.3;
    floor_ring(b, FORGE_STAND, y, z, 32.0, 8, 2.4, true);
    if b.fine() {
        floor_ring(b, FORGE_STAND, y, z, 20.0, 8, 1.2, false);
        for i in 0..8 {
            let a = TAU * i as f32 / 8.0;
            let d = v3(a.cos(), a.sin(), 0.0);
            seam(b, v3(FORGE_STAND, y, z) + d * 35.0, v3(FORGE_STAND, y, z) + d * 43.0, Vec3::Z, 1.6);
        }
        live_line(b, v3(FORGE_BAY_BACK + 4.0, y, z), v3(FORGE_STAND - 46.0, y, z), 1.4);
        live_line(b, v3(FORGE_STAND - 4.0, y, z), v3(FORGE_STAND + 4.0, y, z), 8.0);
        // Pale kerbs down the bay's sides.
        pale(b);
        for s in [-1.0f32, 1.0] {
            let yk = y + s * (hy - 5.0);
            panel(b, &[v3(FORGE_BAY_BACK + 3.0, yk - 3.0, z), v3(LINTEL_FRONT - 2.0, yk - 3.0, z), v3(LINTEL_FRONT - 2.0, yk + 3.0, z), v3(FORGE_BAY_BACK + 3.0, yk + 3.0, z)], Vec3::Z, 0.16, 0.0);
        }
    }
    for s in [-1.0f32, 1.0] {
        seam(b, v3(-6.0, y + s * 46.0, z), v3(LINTEL_FRONT - 4.0, y + s * 46.0, z), Vec3::Z, 2.0);
    }
}

/// The roof: a dark course with light in it; a tier over the whole hall with a lantern
/// over each bay and segments hovering over light along the pier lines; a second tier
/// over the block and a crest of segments hovering over a line of light.
fn forge_roof(b: &mut MeshBuilder) {
    let (x0, x1) = (FORGE_BODY_REAR.1 + 2.0, 4.0);
    dark(b);
    let course = slab_plan(b, x0, x1, FORGE_HALF - 12.0, 10.0, 10.0);
    slab(b, &course, FORGE_EAVE - 0.5, 250.5);
    seam(b, v3(x1, -330.0, 247.0), v3(x1, 330.0, 247.0), Vec3::X, 2.4);
    seam(b, v3(x0, -330.0, 247.0), v3(x0, 330.0, 247.0), -Vec3::X, 2.4);
    let (tier_z0, tier_z1) = (250.0, 276.0);
    pale(b);
    let hx = (x1 - x0) * 0.5 - 2.0;
    let mid = (x0 + x1) * 0.5 - 2.0;
    let tier = cut_rect(b, hx, FORGE_HALF - 16.0, 16.0);
    b.loft_z(&tier, &[Section::new(tier_z0, 1.0).shifted(mid, 0.0), Section::scaled(tier_z1, 0.97, 0.985).shifted(mid, 0.0)]);
    let front = Face::wall(v3(x1 - 4.0, 0.0, 0.0), Vec3::Y, Vec3::X);
    windows(b, &front, (-320.0, 320.0), (256.0, 268.0), 40.0, 2.0);
    let top_front = mid + hx * 0.97;

    // A lantern over each bay: a dark course lit through, a pale block, light along its crown.
    for y in FORGE_BAYS {
        let (lx, lhx, lhy) = (-80.0, 62.0, 30.0);
        let at = |z: f32, s: f32| Section::new(z, s).shifted(lx, y);
        dark(b);
        let course = cut_rect(b, lhx, lhy, 6.0);
        b.loft_z(&course, &[at(tier_z1 - 0.5, 1.0), at(tier_z1 + 4.0, 1.0)]);
        light(b);
        b.loft_z(&course, &[at(tier_z1 + 1.0, 1.015), at(tier_z1 + 2.6, 1.015)]);
        pale(b);
        let lantern = cut_rect(b, lhx + 1.0, lhy + 1.0, 7.0);
        b.loft_z(&lantern, &[at(tier_z1 + 4.0, 1.0), at(tier_z1 + 11.0, 1.0), at(tier_z1 + 15.0, 0.9)]);
        seam(b, v3(lx - lhx * 0.8, y, tier_z1 + 15.0), v3(lx + lhx * 0.8, y, tier_z1 + 15.0), Vec3::Z, 3.0);
        if b.fine() {
            pale(b);
            for s in [-1.0f32, 1.0] {
                b.chamfered_box(v3(lx, y + s * 9.0, tier_z1 + 16.2), v3(2.0 * lhx * 0.72, 4.0, 3.0), 1.0);
            }
            let side = Face::wall(v3(0.0, y + lhy + 1.0, 0.0), -Vec3::X, Vec3::Y);
            seam(b, side.at(-lx - lhx + 8.0, tier_z1 + 8.0), side.at(-lx + lhx - 8.0, tier_z1 + 8.0), Vec3::Y, 1.6);
            let side = Face::wall(v3(0.0, y - lhy - 1.0, 0.0), Vec3::X, -Vec3::Y);
            seam(b, side.at(lx - lhx + 8.0, tier_z1 + 8.0), side.at(lx + lhx - 8.0, tier_z1 + 8.0), -Vec3::Y, 1.6);
        }
    }
    for y in FORGE_PIERS {
        let y = y.clamp(-FORGE_HALF + 40.0, FORGE_HALF - 40.0);
        if b.fine() {
            hover_row(b, v3(-146.0, y, tier_z1), v3(top_front - 8.0, y, tier_z1), 10.0, 5.0, 24.0, 5.0, 3.0);
        } else {
            seam(b, v3(-146.0, y, tier_z1), v3(top_front - 8.0, y, tier_z1), Vec3::Z, 3.0);
        }
    }
    // Along the eave's ledge in front of the roof, a row hovering over light.
    if b.fine() {
        hover_row(b, v3(16.0, -340.0, FORGE_EAVE), v3(16.0, 340.0, FORGE_EAVE), 12.0, 4.0, 22.0, 4.0, 2.0);
    }

    // The second tier over the block, and the crest.
    let (cx, hx, hy) = (-192.0, 42.0, 312.0);
    let at = |z: f32, s: f32| Section::new(z, s).shifted(cx, 0.0);
    dark(b);
    let course = cut_rect(b, hx + 2.0, hy + 4.0, 12.0);
    b.loft_z(&course, &[at(tier_z1 - 0.5, 1.0), at(281.5, 1.0)]);
    light(b);
    b.loft_z(&course, &[at(277.8, 1.008), at(279.2, 1.008)]);
    pale(b);
    let tier = cut_rect(b, hx, hy, 12.0);
    b.loft_z(&tier, &[at(281.0, 1.0), at(300.0, 0.95)]);
    let front = Face::wall(v3(cx + hx, 0.0, 0.0), Vec3::Y, Vec3::X);
    windows(b, &front, (-hy + 20.0, hy - 20.0), (285.0, 294.0), 0.0, 2.0);
    if b.mid() {
        hover_row(b, v3(cx, -290.0, 300.0), v3(cx, 290.0, 300.0), 32.0, 12.0, 48.0, 6.0, 6.0);
    } else {
        pale(b);
        b.chamfered_box(v3(cx, 0.0, 312.0), v3(32.0, 580.0, 12.0), 3.0);
    }
}

// ---- Cradle -------------------------------------------------------------------------
//
// A berth where a row of three Shapers forms, facing +x: three sockets on a dark floor,
// each a lit ring, under an arm cantilevered from the back wall that reaches out over
// it and hangs a projector head and two claws over its middle. The back wall is dark
// niches between pale ribs, one lit behind each socket; towers stand at either end,
// dark between pale piers, and a crown of segments hovers over the wall's cap.

/// Middle of each slot across the berth (y, at x = 0); 60 m lots.
pub(super) const CRADLE_SLOTS: [f32; 3] = [-72.0, 0.0, 72.0];
/// The clear box over the slots: half x, half y, and the height nothing comes under.
pub(super) const CRADLE_CLEAR: (f32, f32, f32) = (45.0, 110.0, 70.0);
/// The lens of each slot's projector head, straight over the slot's middle.
pub(super) const CRADLE_HEAD_Z: f32 = 86.0;
const _: () = assert!(CRADLE_HEAD_Z > CRADLE_CLEAR.2 + 10.0);
const CRADLE_WALL: (f32, f32, f32) = (-80.0, 16.0, 170.0);
const CRADLE_TOWER_Y: f32 = 150.0;
const CRADLE_TOP: f32 = 220.0;

fn cradle(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        cradle_coarse(b);
        return;
    }
    cradle_wall(b);
    b.mirror_y(|b| cradle_tower(b, CRADLE_TOWER_Y));
    cradle_floor(b);
    for y in CRADLE_SLOTS {
        cradle_arm(b, y);
    }
}

fn cradle_coarse(b: &mut MeshBuilder) {
    pale(b);
    let (x, hx, hy) = CRADLE_WALL;
    b.cuboid_open(v3(x, 0.0, 58.0), v3(2.0 * hx, 2.0 * hy, 276.0));
    for s in [-1.0f32, 1.0] {
        b.cuboid_open(v3(0.0, s * CRADLE_TOWER_Y, 68.0), v3(72.0, 52.0, 296.0));
    }
    for y in CRADLE_SLOTS {
        open_bar(b, v3(x, y, 150.0), v3(20.0, y, 116.0), Vec3::Z, 8.0, 12.0);
    }
}

fn cradle_wall(b: &mut MeshBuilder) {
    let (x, hx, hy) = CRADLE_WALL;
    let at = |z: f32, s: f32| Section::new(z, s).shifted(x, 0.0);
    pale(b);
    let plinth = cut_rect(b, hx, hy, 5.0);
    b.loft_z(&plinth, &[at(-80.0, 1.0), at(24.0, 1.0)]);
    dark(b);
    let course = cut_rect(b, hx - 1.0, hy - 2.0, 4.0);
    b.loft_z(&course, &[at(23.5, 1.0), at(30.5, 1.0)]);
    light(b);
    b.loft_z(&course, &[at(26.3, 1.012), at(27.7, 1.012)]);
    pale(b);
    let body = cut_rect(b, hx - 2.0, hy, 5.0);
    b.loft_z(&body, &[at(30.0, 1.0), at(176.0, 1.0)]);
    dark(b);
    b.loft_z(&course, &[at(175.5, 1.0), at(182.5, 1.0)]);
    light(b);
    b.loft_z(&course, &[at(178.3, 1.012), at(179.7, 1.012)]);
    pale(b);
    let cap = cut_rect(b, hx + 4.0, hy + 4.0, 7.0);
    b.loft_z(&cap, &[at(182.0, 1.0), at(190.0, 1.0), at(196.0, 0.94)]);

    // The front: a lit niche behind each slot between pale ribs, bands between.
    let face_x = |_z: f32| x + hx - 2.0;
    let front = Face::wall(v3(face_x(0.0), 0.0, 0.0), Vec3::Y, Vec3::X);
    for y in CRADLE_SLOTS {
        dark(b);
        panel(b, &front.quad(y - 26.0, 36.0, y + 26.0, 168.0), Vec3::X, 0.6, 0.0);
        let on = front.lift(0.6);
        seam(b, on.at(y, 42.0), on.at(y, 162.0), Vec3::X, 4.0);
        if b.fine() {
            for s in [-1.0f32, 1.0] {
                seam(b, on.at(y + s * 16.0, 60.0), on.at(y + s * 16.0, 150.0), Vec3::X, 1.4);
            }
            pale(b);
            panel(b, &front.quad(y - 26.0, 166.0, y + 26.0, 172.0), Vec3::X, 2.6, 0.6);
            panel(b, &front.quad(y - 26.0, 32.0, y + 26.0, 38.0), Vec3::X, 2.6, 0.6);
            for s in [-1.0f32, 1.0] {
                panel(b, &front.quad(y + s * 29.5 - 2.5, 30.0, y + s * 29.5 + 2.5, 176.0), Vec3::X, 3.2, 0.8);
            }
        }
    }
    for s in [-1.0f32, 1.0] {
        let (a, c) = if s < 0.0 { (-hy + 4.0, -104.0) } else { (104.0, hy - 4.0) };
        windows(b, &front, (a, c), (60.0, 150.0), 0.0, 2.4);
    }

    // Blades standing out of the wall between the niches, leaning back as they climb.
    if b.fine() {
        let blade = [[face_x(0.0) - 1.0, 28.0], [face_x(0.0) + 17.0, 28.0], [face_x(0.0) + 17.0, 40.0], [face_x(0.0) + 6.0, 150.0], [face_x(0.0) - 1.0, 174.0]];
        for y in [-108.0f32, -36.0, 36.0, 108.0] {
            b.at(v3(0.0, y, 0.0), |b| {
                pale(b);
                b.extrude_y_chamfered(&blade, 3.5, 1.2);
                for s in [-3.5f32, 3.5] {
                    seam(b, v3(face_x(0.0) + 14.0, s, 44.0), v3(face_x(0.0) + 5.5, s, 146.0), v3(0.0, s.signum(), 0.0), 1.2);
                }
            });
        }
    }

    // The back: bands with pale pilasters over them.
    let back = Face::wall(v3(x - hx + 2.0, 0.0, 0.0), -Vec3::Y, -Vec3::X);
    windows(b, &back, (-hy + 4.0, hy - 4.0), (120.0, 150.0), 0.0, 2.8);
    windows(b, &back, (-hy + 4.0, hy - 4.0), (48.0, 62.0), 0.0, 2.0);
    if b.fine() {
        pale(b);
        for y in [-136.0f32, -96.0, -36.0, 36.0, 96.0, 136.0] {
            panel(b, &back.quad(y - 4.0, 30.0, y + 4.0, 174.0), -Vec3::X, 2.0, 0.8);
        }
    }

    // The crown: a segment hovering over each slot on a line of light.
    if b.fine() {
        hover_row(b, v3(x, -108.0, 196.0), v3(x, 108.0, 196.0), 26.0, 16.0, 64.0, 12.0, 5.0);
    } else {
        pale(b);
        for y in CRADLE_SLOTS {
            b.chamfered_box(v3(x, y, 209.0), v3(26.0, 60.0, 16.0), 3.0);
        }
    }
}

/// An end tower at `y`: plinth, a dark course with light in it, a shaft dark between pale
/// piers, a neck of light, a head stage and a point hovering over it.
fn cradle_tower(b: &mut MeshBuilder, y: f32) {
    let at = |z: f32, s: f32| Section::new(z, s).shifted(0.0, y);
    pale(b);
    let plinth = cut_rect(b, 36.0, 26.0, 9.0);
    b.loft_z(&plinth, &[at(-80.0, 1.0), at(0.0, 1.0), at(22.0, 0.96)]);
    dark(b);
    let course = cut_rect(b, 33.0, 23.0, 8.0);
    b.loft_z(&course, &[at(21.5, 1.0), at(28.5, 1.0)]);
    light(b);
    b.loft_z(&course, &[at(24.3, 1.015), at(25.7, 1.015)]);
    pier_tower(b, 0.0, y, 30.0, 20.0, 28.0, 168.0);
    if b.fine() {
        // A dark band round the plinth with light in it.
        for (n, u, along, out) in [(Vec3::X, Vec3::Y, 26.0, 36.0), (-Vec3::X, -Vec3::Y, 26.0, 36.0), (Vec3::Y, -Vec3::X, 36.0, 26.0), (-Vec3::Y, Vec3::X, 36.0, 26.0)] {
            let f = Face::wall(v3(0.0, y, 0.0) + n * (out - 0.4), u, n);
            windows(b, &f, (-along + 10.0, along - 10.0), (6.0, 14.0), 0.0, 1.6);
        }
    }
    light(b);
    let neck = cut_rect(b, 20.0, 12.0, 4.0);
    b.loft_z(&neck, &[at(166.0, 1.0), at(174.0, 1.0)]);
    pale(b);
    let head = cut_rect(b, 33.0, 23.0, 8.0);
    b.loft_z(&head, &[at(174.0, 1.0), at(186.0, 1.0), at(196.0, 0.84)]);
    if b.fine() {
        // Light round the head stage, over its face toward the berth.
        let inner = Face::wall(v3(0.0, y - 23.0, 0.0), Vec3::X, -Vec3::Y);
        seam(b, inner.at(-26.0, 180.0), inner.at(26.0, 180.0), -Vec3::Y, 2.0);
        let outer = Face::wall(v3(0.0, y + 23.0, 0.0), -Vec3::X, Vec3::Y);
        seam(b, outer.at(-26.0, 180.0), outer.at(26.0, 180.0), Vec3::Y, 2.0);
    }
    light(b);
    let tip = cut_rect(b, 14.0, 9.0, 3.0);
    b.loft_z(&tip, &[at(198.0, 0.5), at(202.0, 1.0)]);
    pale(b);
    b.loft_z(&tip, &[at(202.0, 1.0), at(206.0, 1.0), at(CRADLE_TOP, 0.12)]);
}

/// The dark floor over the berth, a lit ring round each slot, lines to the wall.
fn cradle_floor(b: &mut MeshBuilder) {
    let (wx, whx, _) = CRADLE_WALL;
    let x0 = wx + whx - 1.0;
    let x1 = CRADLE_CLEAR.0 + 1.0;
    dark(b);
    b.plate(v3((x0 + x1) * 0.5, 0.0, 0.0), Vec2::new(x1 - x0, 2.0 * 118.0), 0.3, 0.2);
    let z = 0.3;
    for y in CRADLE_SLOTS {
        floor_ring(b, 0.0, y, z, 28.0, 8, 2.4, true);
        if b.fine() {
            floor_ring(b, 0.0, y, z, 15.0, 8, 1.2, false);
            for i in 0..8 {
                let a = TAU * (i as f32 + 0.5) / 8.0;
                let d = v3(a.cos(), a.sin(), 0.0);
                seam(b, v3(0.0, y, z) + d * 18.0, v3(0.0, y, z) + d * 25.0, Vec3::Z, 1.0);
            }
            live_line(b, v3(-4.0, y, z), v3(4.0, y, z), 8.0);
        }
        // The socket's pale rim, in eight pieces with gaps between.
        pale(b);
        for i in 0..8 {
            let a = TAU * i as f32 / 8.0;
            let half = TAU / 16.0 - 0.06;
            let p = |a: f32, r: f32| v3(r * a.cos(), y + r * a.sin(), z);
            panel(b, &[p(a - half, 31.0), p(a + half, 31.0), p(a + half, 36.0), p(a - half, 36.0)], Vec3::Z, 0.16, 0.0);
        }
        seam(b, v3(x0 + 2.0, y, z), v3(-37.0, y, z), Vec3::Z, 1.6);
    }
}

/// The arm over the slot at `y`: a girder rising out of the wall's shoulder to a lit
/// knuckle, then reaching down and out over the slot, braced by a strut under the
/// knuckle; a housing on it hangs a projector head straight down over the slot's
/// middle and two claws reaching down either side, and its end is a face of light.
fn cradle_arm(b: &mut MeshBuilder, y: f32) {
    let (wx, whx, _) = CRADLE_WALL;
    let face = wx + whx - 2.0;
    let (hw, hd) = (8.0, 12.0);
    let root = v3(face - 4.0, y, 150.0);
    let knee = v3(-30.0, y, 178.0);
    let tip = v3(20.0, y, 116.0);
    let rise = Run::new(root, knee, Vec3::Z);
    let reach = Run::new(knee, tip, Vec3::Z);
    girder(b, &rise, 0.0, rise.len + 3.0, hw, hd, 4.0, 22.0, 0.0);
    girder(b, &reach, -3.0, reach.len, hw, hd, 4.0, 22.0, 0.0);
    // The knuckle, square to the bend, lit round.
    let bend = (rise.dir + reach.dir).normalize();
    let knuckle = Run::new(knee - bend * 7.0, knee + bend * 7.0, Vec3::Z);
    dark(b);
    let collar = cut_rect(b, hw + 2.0, hd + 2.5, 3.0);
    knuckle.solid(b, 0.0, knuckle.len, &collar, 0.0);
    light(b);
    let ring = cut_rect(b, hw + 2.4, hd + 2.9, 3.2);
    knuckle.solid(b, knuckle.len * 0.5 - 1.0, knuckle.len * 0.5 + 1.0, &ring, 0.0);
    // The end: a housing with a face of light.
    pale(b);
    let housing = cut_rect(b, hw + 1.5, hd + 2.0, 3.0);
    reach.solid(b, reach.len - 12.0, reach.len + 4.0, &housing, 0.0);
    light(b);
    let end = reach.len + 4.0;
    let (w, d) = (hw - 2.0, hd - 3.0);
    film(b, &[reach.at(end, -w, -d), reach.at(end, w, -d), reach.at(end, w, d), reach.at(end, -w, d)], reach.dir, 0.1);
    // The shoulder it leaves the wall from.
    pale(b);
    b.chamfered_box(v3(face + 6.0, y, 150.0), v3(16.0, 30.0, 40.0), 4.0);
    if b.fine() {
        light(b);
        b.chamfered_box(v3(face + 14.2, y, 150.0), v3(0.6, 12.0, 24.0), 1.0);
    }
    // The strut under the knuckle.
    let strut = Run::new(v3(face - 2.0, y, 96.0), knee - v3(0.0, 0.0, hd + 1.0), Vec3::Z);
    girder(b, &strut, 0.0, strut.len, 5.0, 6.0, 2.0, 30.0, 0.0);
    // The housing over the slot's middle, and the head under it.
    let over = reach.at(reach.len * (-knee.x / (tip.x - knee.x)), 0.0, 0.0);
    pale(b);
    let drop = cut_rect(b, 11.0, 12.0, 3.5);
    b.loft_z(&drop, &[Section::new(106.0, 0.8).shifted(0.0, y), Section::new(112.0, 1.0).shifted(0.0, y), Section::new(over.z, 0.9).shifted(0.0, y)]);
    if b.fine() {
        light(b);
        b.loft_z(&drop, &[Section::new(117.0, 1.03).shifted(0.0, y), Section::new(118.6, 1.03).shifted(0.0, y)]);
    }
    let s = 1.1;
    b.at(v3(0.0, y, CRADLE_HEAD_Z), |b| projector(b, s));
    dark(b);
    let mast = cut_rect(b, 3.5, 3.5, 1.0);
    let top = CRADLE_HEAD_Z + PROJECTOR_LEN * s;
    b.loft_z(&mast, &[Section::new(top - 1.0, 1.0).shifted(0.0, y), Section::new(107.0, 1.0).shifted(0.0, y)]);
    // Two claws reaching down either side of the slot, lit at their tips.
    if b.fine() {
        for side in [-1.0f32, 1.0] {
            let claw = Run::new(v3(0.0, y + side * 9.0, 114.0), v3(-6.0, y + side * 28.0, 79.0), v3(1.0, 0.0, 0.0));
            pale(b);
            let bar = cut_rect(b, 2.4, 3.6, 1.0);
            claw.solid(b, 0.0, claw.len, &bar, 0.0);
            let tip = cut_rect(b, 2.8, 4.2, 1.2);
            light(b);
            claw.solid(b, claw.len - 2.0, claw.len + 2.5, &tip, 0.0);
            if b.fine() {
                seam(b, claw.at(3.0, 0.0, 3.6), claw.at(claw.len - 3.0, 0.0, 3.6), claw.up, 1.2);
            }
        }
    }
}

// ---- SeaGate ------------------------------------------------------------------------
//
// The harbour that prints ships: a channel 360 m wide between two moles, ships leaving
// along +x. The moles are battered from the seabed to a deck 40 m up, their quay faces
// banded dark with light at the waterline, halls and gatehouses on their decks and
// segments hovering along their outer lips. A gantry spans the channel over the three
// slips, a projector head over each; a lock gate closes the channel's back under a
// bridge between two gatehouses. Over the mouth an arch of two ribs, 420 m at its
// crown, a keystone hanging under it and a cluster hovering over it.

/// The slips across the channel (y), each at `SEAGATE_SLIP_X`.
pub(super) const SEAGATE_SLIPS: [f32; 3] = [-110.0, 0.0, 110.0];
pub(super) const SEAGATE_SLIP_X: f32 = -150.0;
/// Where a slip's print beam leaves its projector head on the gantry: (x, z), y the slip's.
pub(super) const SEAGATE_EMITTER: (f32, f32) = (-230.0, 140.0);
/// The channel kept clear: x from, x to, half width, and the height nothing comes under.
pub(super) const SEAGATE_CHANNEL: (f32, f32, f32, f32) = (-400.0, 200.0, 180.0, 60.0);
const _: () = assert!(SEAGATE_EMITTER.1 > SEAGATE_CHANNEL.3 + 60.0);
/// The arch over the mouth: its x, and the top of its crown.
pub(super) const SEAGATE_ARCH_X: f32 = 60.0;
pub(super) const SEAGATE_CROWN: f32 = 420.0;
/// The moles' deck.
pub(super) const SEAGATE_DECK: f32 = 40.0;
const SEAGATE_TOP: f32 = 520.0;
/// The quay face of each mole (|y|): its fittings stand out to the channel's edge.
const QUAY: f32 = 184.0;
const ARCH_RIBS: [f32; 2] = [38.0, 82.0];
const ARCH_FOOT_Y: f32 = 230.0;
/// The arch springs from pylons on the moles' ends at this height.
const ARCH_FOOT_Z: f32 = 150.0;
const ARCH_HD: f32 = 30.0;
const ARCH_HW: f32 = 13.0;
const GANTRY_Z: f32 = 182.0;

/// The arch's middle line over `y`: steep at the feet, broad over the crown.
fn arch_z(y: f32) -> f32 {
    let u = (y / ARCH_FOOT_Y).clamp(-1.0, 1.0);
    let rise = SEAGATE_CROWN - ARCH_HD - ARCH_FOOT_Z;
    ARCH_FOOT_Z + rise * (1.0 - u * u).max(0.0).powf(0.58)
}

fn arch_point(s: f32) -> Vec3 {
    let y = -ARCH_FOOT_Y * (s * std::f32::consts::PI).cos();
    v3(0.0, y, arch_z(y))
}

fn seagate(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        seagate_coarse(b);
        return;
    }
    b.mirror_y(seagate_mole);
    seagate_arch(b);
    seagate_gantry(b);
    seagate_lock(b);
}

fn seagate_coarse(b: &mut MeshBuilder) {
    pale(b);
    for s in [-1.0f32, 1.0] {
        b.cuboid_open(v3(-150.0, s * 232.0, -20.0), v3(500.0, 96.0, 120.0));
    }
    let x = SEAGATE_ARCH_X;
    let pts = [-ARCH_FOOT_Y, -150.0, 150.0, ARCH_FOOT_Y].map(|y| v3(x, y, arch_z(y)));
    for w in pts.windows(2) {
        open_bar(b, w[0], w[1], Vec3::Z, 28.0, ARCH_HD);
    }
    b.cuboid_open(v3(-412.0, 0.0, -5.0), v3(16.0, 380.0, 150.0));
}

/// The mole on the +y side (mirrored for the other).
fn seagate_mole(b: &mut MeshBuilder) {
    let deck = SEAGATE_DECK;
    let base = [[-400.0, QUAY], [86.0, QUAY], [100.0, QUAY + 14.0], [100.0, 266.0], [86.0, 280.0], [-400.0, 280.0]];
    let top = [[-400.0, QUAY], [82.0, QUAY], [96.0, QUAY + 14.0], [96.0, 258.0], [82.0, 272.0], [-400.0, 272.0]];
    let ring = |p: &[[f32; 2]; 6], z: f32| p.iter().map(|q| v3(q[0], q[1], z)).collect::<Vec<_>>();
    pale(b);
    b.loft(&[ring(&base, -80.0), ring(&top, deck)], true, true);

    // The quay face: a band of windows, light at the waterline, pilasters, a lip.
    let quay = Face::wall(v3(0.0, QUAY, 0.0), Vec3::X, -Vec3::Y);
    windows(b, &quay, (-396.0, 84.0), (10.0, 28.0), 25.0, 2.4);
    seam(b, quay.at(-396.0, 3.5), quay.at(84.0, 3.5), -Vec3::Y, 1.8);
    if b.fine() {
        pale(b);
        let mut x = -375.0;
        while x < 80.0 {
            panel(b, &quay.quad(x - 3.0, -20.0, x + 3.0, 37.0), -Vec3::Y, 3.0, 0.6);
            x += 50.0;
        }
    }
    pale(b);
    b.cuboid(v3(-153.0, QUAY + 2.75, deck - 0.25), v3(494.0, 10.5, 5.5));
    seam(b, v3(-396.0, QUAY - 2.5, deck - 1.0), v3(90.0, QUAY - 2.5, deck - 1.0), -Vec3::Y, 1.6);

    // The battered outer face and the mouth end.
    let lean = 8.0 / 120.0;
    let outer = Face { o: v3(0.0, 280.0 - 80.0 * lean, 0.0), u: Vec3::X, v: v3(0.0, -lean, 1.0), n: v3(0.0, 1.0, lean).normalize() };
    windows(b, &outer, (-396.0, 80.0), (8.0, 26.0), 25.0, 2.4);
    seam(b, outer.at(-396.0, 2.5), outer.at(80.0, 2.5), outer.n, 1.8);
    let lean = 4.0 / 120.0;
    let mouth = Face { o: v3(100.0 - 80.0 * lean, 0.0, 0.0), u: Vec3::Y, v: v3(-lean, 0.0, 1.0), n: v3(1.0, 0.0, lean).normalize() };
    windows(b, &mouth, (QUAY + 20.0, 252.0), (8.0, 26.0), 0.0, 2.4);

    // Lines of light along the deck, bollards along the lip, segments along the outer edge.
    seam(b, v3(-396.0, 197.0, deck), v3(78.0, 197.0, deck), Vec3::Z, 1.8);
    if b.fine() {
        pale(b);
        let mut x = -380.0;
        while x < 80.0 {
            b.chamfered_box(v3(x, 194.0, deck + 2.5), v3(4.0, 4.0, 5.0), 1.0);
            x += 40.0;
        }
    }
    if b.mid() {
        hover_row(b, v3(-392.0, 267.0, deck), v3(76.0, 267.0, deck), 6.0, 4.0, 30.0, 5.0, 2.5);
    }

    // A hall on the deck: body, a dark course with light in it, a roof tier, a crest.
    let (hx, hy) = (-105.0, 232.0);
    let at = |z: f32, s: f32| Section::new(z, s).shifted(hx, hy);
    pale(b);
    let body = cut_rect(b, 95.0, 34.0, 8.0);
    b.loft_z(&body, &[at(deck - 1.0, 1.0), at(74.0, 1.0)]);
    dark(b);
    let course = cut_rect(b, 93.0, 32.0, 7.0);
    b.loft_z(&course, &[at(73.5, 1.0), at(80.5, 1.0)]);
    light(b);
    b.loft_z(&course, &[at(76.4, 1.01), at(77.6, 1.01)]);
    pale(b);
    let roof = cut_rect(b, 90.0, 30.0, 7.0);
    b.loft_z(&roof, &[at(80.0, 1.0), at(92.0, 0.97)]);
    let side = Face::wall(v3(0.0, hy - 34.0, 0.0), Vec3::X, -Vec3::Y);
    windows(b, &side, (hx - 86.0, hx + 86.0), (48.0, 66.0), 16.0, 2.0);
    let side = Face::wall(v3(0.0, hy + 34.0, 0.0), -Vec3::X, Vec3::Y);
    windows(b, &side, (-hx - 86.0, -hx + 86.0), (48.0, 66.0), 16.0, 2.0);
    if b.mid() {
        hover_row(b, v3(hx - 78.0, hy, 92.0), v3(hx + 78.0, hy, 92.0), 12.0, 6.0, 26.0, 5.0, 4.0);
    }

    // A low block between the gatehouse and the gantry, lit slots in its face.
    let (bx, by) = (-291.0, 231.0);
    pale(b);
    let block = cut_rect(b, 27.0, 31.0, 6.0);
    b.loft_z(&block, &[Section::new(deck - 1.0, 1.0).shifted(bx, by), Section::new(64.0, 1.0).shifted(bx, by), Section::new(70.0, 0.9).shifted(bx, by)]);
    if b.fine() {
        let face = Face::wall(v3(0.0, by - 31.0, 0.0), Vec3::X, -Vec3::Y);
        let mut x = bx - 18.0;
        while x <= bx + 18.0 {
            seam(b, face.at(x, 46.0), face.at(x, 60.0), -Vec3::Y, 2.0);
            x += 12.0;
        }
    }

    // The arch's foot.
    seagate_arch_foot(b);
    // The gantry's leg.
    let (gx, _) = SEAGATE_EMITTER;
    pier_tower(b, gx, 230.0, 22.0, 22.0, deck - 1.0, GANTRY_Z + 12.0);
    pale(b);
    b.chamfered_box(v3(gx, 230.0, GANTRY_Z + 18.0), v3(54.0, 58.0, 12.0), 4.0);
    if b.fine() {
        light(b);
        b.chamfered_box(v3(gx, 230.0, GANTRY_Z + 24.6), v3(30.0, 34.0, 1.2), 3.0);
    }
    // The gatehouse.
    seagate_gatehouse(b);
}

/// One foot of the arch, on the +y mole: a pylon stepped in three tiers, each over a
/// dark course with light in it, blades leaning into each rib from outside.
fn seagate_arch_foot(b: &mut MeshBuilder) {
    let (x, y) = (SEAGATE_ARCH_X, ARCH_FOOT_Y);
    let at = |z: f32, s: f32| Section::new(z, s).shifted(x, y);
    let tiers = [(SEAGATE_DECK - 2.0, 84.0, 38.0, 40.0), (90.0, 124.0, 34.0, 36.0), (130.0, ARCH_FOOT_Z + 4.0, 31.0, 33.0)];
    for (i, &(z0, z1, hx, hy)) in tiers.iter().enumerate() {
        pale(b);
        let plan = cut_rect(b, hx, hy, 10.0);
        b.loft_z(&plan, &[at(z0, 1.0), at(z1 - 4.0, 1.0), at(z1, 0.95)]);
        if let Some(&(next, _, nx, ny)) = tiers.get(i + 1) {
            dark(b);
            let course = cut_rect(b, nx - 1.0, ny - 1.0, 9.0);
            b.loft_z(&course, &[at(z1 - 0.5, 1.0), at(next + 0.5, 1.0)]);
            light(b);
            b.loft_z(&course, &[at((z1 + next) * 0.5 - 0.8, 1.02), at((z1 + next) * 0.5 + 0.8, 1.02)]);
        }
        if !b.fine() {
            continue;
        }
        // A dark band round each tier's face with light in it.
        for (n, u, along, out) in [(Vec3::X, Vec3::Y, hy, hx), (-Vec3::X, -Vec3::Y, hy, hx), (Vec3::Y, -Vec3::X, hx, hy), (-Vec3::Y, Vec3::X, hx, hy)] {
            let f = Face::wall(v3(x, y, 0.0) + n * out, u, n);
            let m = (z0.max(SEAGATE_DECK) + z1 - 4.0) * 0.5;
            windows(b, &f, (-along + 12.0, along - 12.0), (m - 6.0, m + 6.0), 0.0, 1.8);
        }
    }
    if b.mid() {
        let blade = [[236.0, SEAGATE_DECK - 1.0], [274.0, SEAGATE_DECK - 1.0], [274.0, 62.0], [252.0, 196.0], [243.0, 206.0]];
        for rx in ARCH_RIBS {
            b.with(Affine3A::from_translation(v3(rx, 0.0, 0.0)) * Affine3A::from_rotation_z(FRAC_PI_2), |b| {
                // Local x is world y here, local y is world -x.
                pale(b);
                b.extrude_y_chamfered(&blade, 5.0, 1.6);
                if b.fine() {
                    for s in [-5.0f32, 5.0] {
                        seam(b, v3(268.0, s, 66.0), v3(251.0, s, 188.0), v3(0.0, s.signum(), 0.0), 1.8);
                    }
                }
            });
        }
    }
}

/// The gatehouse on the +y mole over the lock: a plinth, a tower dark between pale piers,
/// a neck of light, a cap and a point hovering over it.
fn seagate_gatehouse(b: &mut MeshBuilder) {
    let (x, y) = (-370.0, 232.0);
    let at = |z: f32, s: f32| Section::new(z, s).shifted(x, y);
    pale(b);
    let plinth = cut_rect(b, 34.0, 40.0, 8.0);
    b.loft_z(&plinth, &[at(SEAGATE_DECK - 2.0, 1.0), at(62.0, 1.0)]);
    dark(b);
    let course = cut_rect(b, 31.0, 37.0, 7.0);
    b.loft_z(&course, &[at(61.5, 1.0), at(68.5, 1.0)]);
    light(b);
    b.loft_z(&course, &[at(64.3, 1.012), at(65.7, 1.012)]);
    pier_tower(b, x, y, 28.0, 34.0, 68.0, 172.0);
    light(b);
    let neck = cut_rect(b, 16.0, 20.0, 5.0);
    b.loft_z(&neck, &[at(170.0, 1.0), at(178.0, 1.0)]);
    pale(b);
    let cap = cut_rect(b, 31.0, 37.0, 8.0);
    b.loft_z(&cap, &[at(178.0, 1.0), at(188.0, 1.0), at(198.0, 0.86)]);
    light(b);
    let tip = cut_rect(b, 12.0, 14.0, 4.0);
    b.loft_z(&tip, &[at(200.0, 0.5), at(203.5, 1.0)]);
    pale(b);
    b.loft_z(&tip, &[at(203.5, 1.0), at(207.0, 1.0), at(226.0, 0.15)]);
}

/// The arch: two ribs over the mouth, collared into voussoirs with light under their
/// soffits, ties between them, a keystone hanging under the crown and a cluster
/// hovering over it.
fn seagate_arch(b: &mut MeshBuilder) {
    let x = SEAGATE_ARCH_X;
    let segments = if b.fine() { 36 } else { 16 };
    let points: Vec<Vec3> = (0..=segments).map(|i| arch_point(i as f32 / segments as f32)).collect();
    let square = |i: usize| {
        let a = points[i.saturating_sub(1)];
        let c = points[(i + 1).min(segments)];
        let d = (c - a).normalize();
        (d, v3(0.0, -d.z, d.y))
    };
    for rx in ARCH_RIBS {
        let o = v3(rx, 0.0, 0.0);
        for w in points.windows(2) {
            let d = (w[1] - w[0]).normalize();
            let up = v3(0.0, -d.z, d.y);
            let run = Run::new(w[0] + o, w[1] + o, up);
            girder(b, &run, -3.0, run.len + 3.0, ARCH_HW, ARCH_HD, 10.0, 60.0, 0.0);
            // Light along the soffit, and along the extrados at the full level.
            seam(b, run.at(0.0, 0.0, -ARCH_HD - 0.05), run.at(run.len, 0.0, -ARCH_HD - 0.05), -run.up, 3.0);
            if b.fine() {
                seam(b, run.at(0.0, 0.0, ARCH_HD + 0.05), run.at(run.len, 0.0, ARCH_HD + 0.05), run.up, 1.6);
            }
        }
        // Collars round the rib every few segments: the voussoirs' joints.
        let every = if b.fine() { 3 } else { 4 };
        for i in (every..segments).step_by(every) {
            let (d, up) = square(i);
            let p = points[i] + o;
            let collar = Run::new(p - d * 4.0, p + d * 4.0, up);
            pale(b);
            let plan = cut_rect(b, ARCH_HW + 2.0, ARCH_HD + 2.5, 4.0);
            collar.solid(b, 0.0, collar.len, &plan, 0.0);
            if b.fine() {
                light(b);
                let ring = cut_rect(b, ARCH_HW + 2.3, ARCH_HD + 2.8, 4.2);
                collar.solid(b, collar.len * 0.5 - 0.7, collar.len * 0.5 + 0.7, &ring, 0.0);
            }
        }
    }
    // Ties between the ribs at the collars, none over the crown.
    dark(b);
    let tie = cut_rect(b, 6.0, 16.0, 2.0);
    let reach = (ARCH_RIBS[1] - ARCH_RIBS[0]) * 0.5 - ARCH_HW + 1.0;
    for s in [0.083f32, 0.167, 0.25, 0.333, 0.667, 0.75, 0.833, 0.917] {
        let p = arch_point(s);
        let run = Run::new(v3(x - reach, p.y, p.z), v3(x + reach, p.y, p.z), Vec3::Z);
        run.solid(b, 0.0, run.len, &tie, 0.0);
    }

    // The keystone hanging under the crown, between pale caps.
    let under = SEAGATE_CROWN - 2.0 * ARCH_HD;
    key_light(b);
    b.chamfered_box(v3(x, 0.0, under - 26.0), v3(18.0, 28.0, 26.0), 3.0);
    if b.mid() {
        pale(b);
        for s in [-1.0f32, 1.0] {
            b.chamfered_box(v3(x, s * 24.0, under - 24.0), v3(26.0, 14.0, 40.0), 3.0);
        }
        dark(b);
        let hanger = cut_rect(b, 3.0, 3.0, 0.8);
        b.loft_z(&hanger, &[Section::new(under - 13.5, 1.0).shifted(x, 0.0), Section::new(under + 1.0, 1.0).shifted(x, 0.0)]);
    }

    // The cluster hovering over the crown: a keystone wider at its top with light through
    // it, two voussoirs leaning out either side, a capstone over a line of light and a
    // lit point over that.
    let z0 = SEAGATE_CROWN + 14.0;
    pale(b);
    let key = cut_rect(b, 30.0, 24.0, 6.0);
    b.loft_z(&key, &[Section::scaled(z0, 0.8, 0.62).shifted(x, 0.0), Section::new(z0 + 44.0, 1.0).shifted(x, 0.0)]);
    light(b);
    for s in [-1.0f32, 1.0] {
        let xf = x + s * 30.3;
        film(b, &[v3(xf - s * 5.4, -5.0, z0 + 8.0), v3(xf - s * 5.4, 5.0, z0 + 8.0), v3(xf, 7.0, z0 + 38.0), v3(xf, -7.0, z0 + 38.0)], v3(s * 44.0, 0.0, -6.0).normalize(), 0.12);
    }
    for s in [-1.0f32, 1.0] {
        b.with(Affine3A::from_translation(v3(x, s * 50.0, z0 + 20.0)) * Affine3A::from_rotation_x(s * 0.3), |b| {
            pale(b);
            let v = cut_rect(b, 24.0, 12.0, 4.0);
            b.loft_z(&v, &[Section::scaled(-16.0, 0.8, 0.7), Section::new(16.0, 1.0)]);
            if b.mid() {
                light(b);
                b.chamfered_box(v3(0.0, -s * 13.0, 0.0), v3(14.0, 1.0, 20.0), 0.4);
            }
        });
    }
    light(b);
    b.chamfered_box(v3(x, 0.0, z0 + 48.0), v3(30.0, 26.0, 1.6), 3.0);
    pale(b);
    b.chamfered_box(v3(x, 0.0, z0 + 56.0), v3(40.0, 54.0, 10.0), 4.0);
    let point = cut_rect(b, 12.0, 16.0, 4.0);
    b.loft_z(&point, &[Section::new(z0 + 61.0, 1.0), Section::new(z0 + 64.0, 1.0), Section::new(SEAGATE_TOP - 6.0, 0.2)].map(|s| s.shifted(x, 0.0)));
    light(b);
    b.chamfered_box(v3(x, 0.0, SEAGATE_TOP - 2.5), v3(4.0, 5.0, 5.0), 1.0);
}

/// The gantry over the slips: twin girders from leg to leg, a carriage between them
/// over each slip hanging a projector head aimed at the slip's middle.
fn seagate_gantry(b: &mut MeshBuilder) {
    let (gx, ez) = SEAGATE_EMITTER;
    for dx in [-16.0f32, 16.0] {
        let run = Run::new(v3(gx + dx, -250.0, GANTRY_Z), v3(gx + dx, 250.0, GANTRY_Z), Vec3::Z);
        girder(b, &run, 0.0, run.len, 8.0, 18.0, 6.0, 44.0, 100.0);
    }
    let s = 1.3;
    let tilt = (gx - SEAGATE_SLIP_X).atan2(ez);
    for y in SEAGATE_SLIPS {
        let top = v3(gx + PROJECTOR_LEN * s * tilt.sin(), y, ez + PROJECTOR_LEN * s * tilt.cos());
        b.with(Affine3A::from_translation(v3(gx, y, ez)) * Affine3A::from_rotation_y(tilt), |b| projector(b, s));
        let cz = GANTRY_Z - 16.0;
        pale(b);
        b.chamfered_box(v3(gx - 4.0, y, cz), v3(46.0, 30.0, 8.0), 2.5);
        dark(b);
        let mast = cut_rect(b, 4.0, 4.0, 1.0);
        b.loft_z(&mast, &[Section::new(top.z - 1.0, 1.0).shifted(top.x, y), Section::new(cz - 3.5, 1.0).shifted(top.x, y)]);
        if b.fine() {
            light(b);
            b.chamfered_box(v3(gx - 4.0, y, cz - 4.2), v3(30.0, 20.0, 0.8), 2.0);
        }
        // A live blade over the slip on the gantry's front, facing the mouth.
        let xf = gx + 16.0 + 8.0;
        key_seam(b, v3(xf, y - 18.0, GANTRY_Z + 4.0), v3(xf, y + 18.0, GANTRY_Z + 4.0), Vec3::X, 3.0);
    }
}

/// The lock gate closing the channel's back: two leaves meeting on a line of light, a
/// dark field on each between pale ribs, a slot of light over each slip, a coping;
/// and the bridge over it between the gatehouses.
fn seagate_lock(b: &mut MeshBuilder) {
    let x1 = SEAGATE_CHANNEL.0 - 4.0;
    let x0 = x1 - 16.0;
    let (hy, top) = (QUAY + 6.0, 70.0);
    pale(b);
    b.block(v3(x0, -hy, -80.0), v3(x1, hy, top));
    b.chamfered_box(v3((x0 + x1) * 0.5 - 1.0, 0.0, top + 3.0), v3(x1 - x0 + 2.0, 2.0 * hy, 6.0), 1.5);
    let face = Face::wall(v3(x1, 0.0, 0.0), Vec3::Y, Vec3::X);
    for side in [-1.0f32, 1.0] {
        let (a, c) = if side < 0.0 { (-178.0, -4.0) } else { (4.0, 178.0) };
        windows(b, &face, (a, c), (10.0, 62.0), 0.0, 0.0);
    }
    let on = face.lift(0.5);
    seam(b, face.at(0.0, 2.0), face.at(0.0, 68.0), Vec3::X, 4.0);
    seam(b, on.at(-176.0, 36.0), on.at(176.0, 36.0), Vec3::X, 2.2);
    for y in SEAGATE_SLIPS {
        if y != 0.0 {
            seam(b, on.at(y, 14.0), on.at(y, 58.0), Vec3::X, 3.2);
        }
    }
    if b.fine() {
        pale(b);
        for y in [-150.0f32, -80.0, -40.0, 40.0, 80.0, 150.0] {
            panel(b, &face.quad(y - 2.0, 10.0, y + 2.0, 62.0), Vec3::X, 2.0, 0.6);
        }
    }
    let run = Run::new(v3(-411.0, -240.0, 108.0), v3(-411.0, 240.0, 108.0), Vec3::Z);
    girder(b, &run, 0.0, run.len, 9.0, 16.0, 5.0, 40.0, 0.0);
}

// ---- Rampart ------------------------------------------------------------------------
//
// 240 m of the plateau's face, facing +x down onto the lower ground: three courses, each
// 30 m high and 14 m behind the one below, battered, with pilasters up them, a dark
// recessed band along each with light in it and light along every nosing; a coping
// along the top with segments hovering over a line of light. Lengths tile along y
// 240 m apart: every run goes to the ends square and every rhythm keeps its pitch
// across the join.

pub(super) const RAMPART_HALF: f32 = 120.0;
pub(super) const RAMPART_TREAD: f32 = 14.0;
pub(super) const RAMPART_RISER: f32 = 30.0;
pub(super) const RAMPART_COURSES: usize = 3;
/// The upper ground's height and the line (x) it starts behind.
pub(super) const RAMPART_UPPER: (f32, f32) = (90.0, -60.0);
/// The coping's top, and how far back (x) it runs.
pub(super) const RAMPART_COPING: (f32, f32) = (96.0, -90.0);
const RAMPART_BATTER: f32 = 3.0;
/// How far the course between pilasters stands back from the contract face.
const RAMPART_RECESS: f32 = 2.5;
const RAMPART_PITCH: f32 = 30.0;
const RAMPART_MERLON_TOP: f32 = 106.0;

/// The pilasters' y in one length: every `RAMPART_PITCH`, half a pitch in from the ends.
fn rampart_pilasters() -> impl Iterator<Item = f32> {
    let n = (2.0 * RAMPART_HALF / RAMPART_PITCH) as usize;
    (0..n).map(|i| -RAMPART_HALF + RAMPART_PITCH * (i as f32 + 0.5))
}

fn rampart(b: &mut MeshBuilder, _tech: u8) {
    let half = RAMPART_HALF;
    let back = RAMPART_COPING.1;
    let (coping_top, _) = RAMPART_COPING;
    let top = RAMPART_UPPER.0;
    let nose = -RAMPART_TREAD * (RAMPART_COURSES - 1) as f32 - RAMPART_BATTER + 2.0;
    if b.coarse() {
        pale(b);
        let mut profile = vec![[back, -60.0], [0.0, -60.0]];
        for k in 0..RAMPART_COURSES {
            let (x0, z0) = (-RAMPART_TREAD * k as f32, RAMPART_RISER * k as f32);
            profile.push([x0, z0]);
            profile.push([x0 - RAMPART_BATTER, z0 + RAMPART_RISER]);
        }
        profile.push([nose, top]);
        profile.push([nose, coping_top]);
        profile.push([back, coping_top]);
        b.extrude_y(&profile, -half, half);
        light(b);
        for k in 0..RAMPART_COURSES {
            let (x, z) = (-RAMPART_TREAD * k as f32 - RAMPART_BATTER * 0.5, RAMPART_RISER * k as f32 + 15.0);
            film(b, &[v3(x, -half, z - 1.5), v3(x, half, z - 1.5), v3(x, half, z + 1.5), v3(x, -half, z + 1.5)], Vec3::X, 0.2);
        }
        return;
    }

    let rise = RAMPART_RISER;
    let n = v3(rise, 0.0, RAMPART_BATTER).normalize();
    for k in 0..RAMPART_COURSES {
        let (x0, z0, z1) = (-RAMPART_TREAD * k as f32, rise * k as f32, rise * (k + 1) as f32);
        // The course, set back from the contract face where the pilasters stand.
        let xb = x0 - RAMPART_RECESS;
        let bottom = if k == 0 { -60.0 } else { z0 - 1.0 };
        let profile = [[back, bottom], [xb, bottom], [xb, z0], [xb - RAMPART_BATTER, z1], [back, z1]];
        pale(b);
        b.extrude_y(&profile, -half, half);
        let face = Face { o: v3(xb, 0.0, z0), u: Vec3::Y, v: v3(-RAMPART_BATTER / rise, 0.0, 1.0), n };
        let at = |z: f32| z - z0;

        // The dark band, its rails, the light in it.
        dark(b);
        panel(b, &face.quad(-half, at(z0 + 10.0), half, at(z0 + 20.0)), n, 0.4, 0.0);
        seam(b, face.lift(0.4).at(-half, at(z0 + 15.0)), face.lift(0.4).at(half, at(z0 + 15.0)), n, 2.2);
        if b.fine() {
            pale(b);
            panel(b, &face.quad(-half, at(z0 + 20.0), half, at(z0 + 21.4)), n, 1.2, 0.0);
            panel(b, &face.quad(-half, at(z0 + 8.6), half, at(z0 + 10.0)), n, 1.2, 0.0);
            // Plates between the pilasters, above and below the band.
            let mut bays: Vec<(f32, f32, f32)> = vec![(-half, -half + RAMPART_PITCH * 0.5 - 4.0, 0.0)];
            for y in rampart_pilasters() {
                let end = (y + RAMPART_PITCH - 4.0).min(half);
                bays.push((y + 4.0, end, if end < half { 0.35 } else { 0.0 }));
            }
            for (y0, y1, bevel) in bays {
                if y1 - y0 < 1.0 {
                    continue;
                }
                let row0 = if k == 0 { 1.0 } else { 1.5 };
                panel(b, &face.quad(y0 + 0.4, at(z0 + row0), y1 - 0.4, at(z0 + 7.6)), n, 0.5, bevel);
                panel(b, &face.quad(y0 + 0.4, at(z0 + 22.6), y1 - 0.4, at(z1 - 1.2)), n, 0.5, bevel);
            }
        }

        // Pilasters, their fronts on the contract face, a slit of light up each.
        for y in rampart_pilasters() {
            pale(b);
            let plan = fine_rect(b, (RAMPART_RECESS + 1.5) * 0.5, 3.5, 0.9);
            let cx = |z: f32| x0 - RAMPART_BATTER * (z - z0) / rise - (RAMPART_RECESS + 1.5) * 0.5;
            let zb = if k == 0 { -2.0 } else { z0 };
            b.loft_z(&plan, &[Section::new(zb, 1.0).shifted(cx(z0), y), Section::new(z1 - 0.2, 1.0).shifted(cx(z1 - 0.2), y)]);
            if b.fine() {
                let front = |z: f32| v3(x0 - RAMPART_BATTER * (z - z0) / rise, y, z);
                seam(b, front(z0 + 3.0), front(z1 - 4.0), n, 1.0);
            }
        }

        // The nosing along the course's top, light along it.
        if k + 1 < RAMPART_COURSES {
            let xf = x0 - RAMPART_BATTER + 0.5;
            let lip = [[xf - 8.0, z1 - 0.5], [xf, z1 - 0.5], [xf, z1 + 1.4], [xf - 0.8, z1 + 2.2], [xf - 8.0, z1 + 2.2]];
            pale(b);
            b.extrude_y(&lip, -half, half);
            seam(b, v3(xf, -half, z1 + 0.5), v3(xf, half, z1 + 0.5), Vec3::X, 1.2);
            if b.fine() {
                seam(b, v3(xf - 4.5, -half, z1 + 2.2), v3(xf - 4.5, half, z1 + 2.2), Vec3::Z, 1.0);
            }
        }
    }

    // The coping, light along its nose, a dark inlay along its top with light in it and
    // segments hovering over that.
    let coping = [[back, top - 0.5], [nose - 2.5, top - 0.5], [nose - 2.5, top], [nose, top + 1.0], [nose, coping_top - 1.5], [nose - 1.5, coping_top], [back, coping_top]];
    pale(b);
    b.extrude_y(&coping, -half, half);
    seam(b, v3(nose, -half, top + 3.2), v3(nose, half, top + 3.2), Vec3::X, 1.6);
    let (ix0, ix1) = (-56.0, -34.0);
    dark(b);
    panel(b, &[v3(ix0, -half, coping_top), v3(ix1, -half, coping_top), v3(ix1, half, coping_top), v3(ix0, half, coping_top)], Vec3::Z, 0.2, 0.0);
    let mx = (ix0 + ix1) * 0.5;
    seam(b, v3(mx, -half, coping_top + 0.2), v3(mx, half, coping_top + 0.2), Vec3::Z, 2.4);
    if b.mid() {
        pale(b);
        for y in rampart_pilasters() {
            b.chamfered_box(v3(mx, y, RAMPART_MERLON_TOP - 4.0), v3(14.0, RAMPART_PITCH - 6.0, 8.0), 2.0);
        }
    }
    if b.fine() {
        seam(b, v3(ix0 - 8.0, -half, coping_top), v3(ix0 - 8.0, half, coping_top), Vec3::Z, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::super::{build_model, MeshLod};
    use super::*;

    /// Points on each triangle: its corners, edge middles and centroid.
    fn samples(mesh: &MeshLod) -> impl Iterator<Item = Vec3> + '_ {
        mesh.indices.chunks(3).flat_map(move |t| {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| Vec3::from(mesh.vertices[i as usize].pos));
            [a, b, c, (a + b) * 0.5, (b + c) * 0.5, (c + a) * 0.5, (a + b + c) / 3.0]
        })
    }

    fn inside(p: Vec3, lo: Vec3, hi: Vec3) -> bool {
        p.cmpgt(lo).all() && p.cmplt(hi).all()
    }

    /// Clear boxes per model: nothing may stand in them.
    fn clear_boxes(key: &str) -> Vec<(Vec3, Vec3)> {
        match key {
            "precursor_forge" => FORGE_BAYS
                .iter()
                .map(|&y| (v3(FORGE_BAY_BACK + 4.0, y - FORGE_BAY_HALF + 2.0, 0.6), v3(LINTEL_FRONT, y + FORGE_BAY_HALF - 2.0, FORGE_CLEAR)))
                .collect(),
            "precursor_cradle" => {
                let (hx, hy, h) = CRADLE_CLEAR;
                vec![(v3(-hx, -hy, 0.5), v3(hx, hy, h))]
            }
            "precursor_seagate" => {
                let (x0, x1, hy, h) = SEAGATE_CHANNEL;
                vec![(v3(x0 + 0.5, -hy + 0.5, -80.0), v3(x1, hy - 0.5, h))]
            }
            _ => Vec::new(),
        }
    }

    /// Where the model may stand below ground: its solid plans (centre x, y, half x, y).
    fn footings(key: &str) -> Vec<(f32, f32, f32, f32)> {
        match key {
            "precursor_forge" => {
                let mut v = vec![(-215.0, 0.0, 65.0, 360.0)];
                v.extend(FORGE_PIERS.iter().map(|&y| (-75.0, y, 75.0, 20.0)));
                v
            }
            "precursor_cradle" => vec![(-80.0, 0.0, 16.0, 170.0), (0.0, 150.0, 36.0, 26.0), (0.0, -150.0, 36.0, 26.0)],
            // The lock gate stands in the channel's end behind x = -400, outside the moles.
            "precursor_seagate" => vec![(-150.0, 230.0, 250.0, 50.0), (-150.0, -230.0, 250.0, 50.0), (-412.0, 0.0, 8.0, 190.0)],
            "precursor_rampart" => vec![(-45.0, 0.0, 45.0, 120.0)],
            _ => unreachable!(),
        }
    }

    #[test]
    fn facility_builds_within_budget() {
        for def in MODELS {
            let model = build_model(def.key).unwrap();
            let tris: Vec<usize> = model.lods.iter().map(|l| l.indices.len() / 3).collect();
            let low = model.lods[0].vertices.iter().map(|v| v.pos[2]).fold(f32::MAX, f32::min);
            let high = model.lods[0].vertices.iter().map(|v| v.pos[2]).fold(f32::MIN, f32::max);
            println!("{}: triangles {tris:?}, z {low:.0}..{high:.0}, bounds {:.0}", def.key, model.bounds_radius);
            assert!(tris[0] <= TRIANGLES, "{}: {} triangles", def.key, tris[0]);
            assert!(tris[1] as f32 <= tris[0] as f32 * 0.45 + 20.0, "{}: LOD1 not lighter", def.key);
            assert!(tris[2] < 60, "{}: coarse LOD has {}", def.key, tris[2]);
            assert!(low >= -80.0, "{}: below the footing", def.key);
            assert!((high - def.nominal[0].1).abs() < 8.0, "{}: top {high} vs height {}", def.key, def.nominal[0].1);
            for lod in &model.lods {
                for v in &lod.vertices {
                    assert!(v.pos.iter().chain(&v.normal).all(|c| c.is_finite()), "{}", def.key);
                }
            }
        }
    }

    #[test]
    fn facility_keeps_clear_zones_and_footings() {
        for def in MODELS {
            let model = build_model(def.key).unwrap();
            for (lod, mesh) in model.lods.iter().enumerate() {
                for (lo, hi) in clear_boxes(def.key) {
                    if let Some(p) = samples(mesh).find(|&p| inside(p, lo, hi)) {
                        panic!("{} lod{lod}: geometry at {p} in the clear zone {lo}..{hi}", def.key);
                    }
                }
                let plans = footings(def.key);
                for p in samples(mesh).filter(|p| p.z < -0.5) {
                    let within = plans.iter().any(|&(x, y, hx, hy)| (p.x - x).abs() <= hx + 0.6 && (p.y - y).abs() <= hy + 0.6);
                    assert!(within, "{} lod{lod}: footing at {p} outside the solid plans", def.key);
                }
            }
        }
    }

    #[test]
    fn facility_meshes_are_sound() {
        // The crate-wide mesh checks, run here for these models alone.
        for def in MODELS {
            let model = build_model(def.key).unwrap();
            for (lod, mesh) in model.lods.iter().enumerate() {
                for t in mesh.indices.chunks(3) {
                    let [a, b, c] = [t[0], t[1], t[2]].map(|i| Vec3::from(mesh.vertices[i as usize].pos));
                    let geometric = (b - a).cross(c - a);
                    assert!(geometric.length() * 0.5 > 1e-7, "{} lod{lod}: degenerate triangle at {a}", def.key);
                    for &i in t {
                        let v = mesh.vertices[i as usize];
                        let shading = Vec3::from(v.normal);
                        assert!((shading.length() - 1.0).abs() < 1e-4, "{} lod{lod}: unit normal", def.key);
                        assert!(geometric.normalize().dot(shading) > 0.5, "{} lod{lod}: winding disagrees at {a}", def.key);
                        assert!(v.uv.iter().all(|c| c.is_finite()), "{} lod{lod}: uv", def.key);
                    }
                    let m = mesh.vertices[t[0] as usize].material;
                    assert!(t.iter().all(|&i| mesh.vertices[i as usize].material == m), "{}: one material per triangle", def.key);
                }
                for v in &mesh.vertices {
                    assert!(Vec3::from(v.pos).length() <= model.bounds_radius + 1e-3, "{}: outside bounds", def.key);
                }
            }
        }
    }

    #[test]
    fn live_light_is_sparse() {
        use super::super::material;
        for def in MODELS {
            let model = build_model(def.key).unwrap();
            let mesh = &model.lods[0];
            let of = |m: u32| mesh.indices.chunks(3).filter(|t| mesh.vertices[t[0] as usize].material == m).count();
            let (live, dormant) = (of(material::GLOW_PRECURSOR), of(material::PRECURSOR_INLAY));
            println!("{}: live {live}, dormant {dormant}", def.key);
            assert!(live * 10 <= live + dormant, "{}: {live} live of {}", def.key, live + dormant);
        }
    }

    #[test]
    fn rampart_tiles_along_y() {
        let model = build_model("precursor_rampart").unwrap();
        for mesh in &model.lods {
            for v in &mesh.vertices {
                assert!(v.pos[1].abs() <= RAMPART_HALF + 1e-3, "past the end at {:?}", v.pos);
                assert!(v.pos[0] <= 1e-3 && v.pos[0] >= RAMPART_COPING.1 - 1e-3, "out of the plan at {:?}", v.pos);
            }
        }
    }

    #[test]
    fn emitters_hang_in_the_open() {
        // Each projector's lens is at its contract point: some light geometry there.
        let model = build_model("precursor_forge").unwrap();
        let (ex, ez) = FORGE_EMITTER;
        for y in FORGE_BAYS {
            let near = model.lods[0].vertices.iter().any(|v| Vec3::from(v.pos).distance(v3(ex, y, ez)) < 6.0);
            assert!(near, "no lens at bay {y}");
        }
        let model = build_model("precursor_seagate").unwrap();
        let (ex, ez) = SEAGATE_EMITTER;
        for y in SEAGATE_SLIPS {
            let near = model.lods[0].vertices.iter().any(|v| Vec3::from(v.pos).distance(v3(ex, y, ez)) < 6.0);
            assert!(near, "no lens at slip {y}");
        }
    }
}

/// Software previews: `FORGE_DUMP_DIR=... cargo test -p mc-render --lib forge_previews -- --ignored`.
/// Whole models from the RTS camera, and crops of the details (a bay, a slot, the arch).
#[cfg(test)]
#[test]
#[ignore]
fn forge_previews() {
    use super::MeshLod;
    let dir = std::path::PathBuf::from(std::env::var_os("FORGE_DUMP_DIR").expect("FORGE_DUMP_DIR"));
    std::fs::create_dir_all(&dir).unwrap();
    let crop = |mesh: &MeshLod, lo: Vec3, hi: Vec3| {
        let mut out = MeshLod::default();
        let mut map = std::collections::HashMap::new();
        for t in mesh.indices.chunks(3) {
            let c = t.iter().map(|&i| Vec3::from(mesh.vertices[i as usize].pos)).sum::<Vec3>() / 3.0;
            if !(c.cmpgt(lo).all() && c.cmplt(hi).all()) {
                continue;
            }
            for &i in t {
                let j = *map.entry(i).or_insert_with(|| {
                    out.vertices.push(mesh.vertices[i as usize]);
                    out.vertices.len() as u32 - 1
                });
                out.indices.push(j);
            }
        }
        out
    };
    for def in MODELS {
        let model = super::build_model(def.key).unwrap();
        for (lod, yaw) in [(0usize, -38.0f32), (0, 142.0), (0, 12.0), (1, -38.0), (2, -38.0)] {
            super::preview::render(&model.lods[lod], 1024, yaw)
                .write_ppm(&dir.join(format!("{}_{lod}_{}.ppm", def.key, yaw as i32)))
                .unwrap();
        }
        let crops: Vec<(&str, Vec3, Vec3, f32)> = match def.key {
            "precursor_forge" => vec![
                ("bay", v3(-200.0, -180.0, -10.0), v3(60.0, 10.0, 330.0), -30.0),
                ("bay_in", v3(-200.0, -160.0, -10.0), v3(60.0, -10.0, 152.0), 20.0),
            ],
            "precursor_cradle" => vec![("slot", v3(-110.0, -40.0, -10.0), v3(60.0, 110.0, 240.0), -30.0)],
            "precursor_seagate" => vec![
                ("arch", v3(0.0, -300.0, -10.0), v3(120.0, 300.0, 520.0), -20.0),
                ("slips", v3(-420.0, -300.0, -10.0), v3(-100.0, 300.0, 240.0), -30.0),
            ],
            _ => vec![("end", v3(-100.0, 30.0, -10.0), v3(10.0, 120.0, 110.0), -35.0)],
        };
        for (name, lo, hi, yaw) in crops {
            super::preview::render(&crop(&model.lods[0], lo, hi), 1024, yaw)
                .write_ppm(&dir.join(format!("{}_{name}.ppm", def.key)))
                .unwrap();
        }
    }
}
