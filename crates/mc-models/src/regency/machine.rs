//! The Regency machine kit: the pieces their structures are built from, so every building
//! speaks the same mechanical language (docs/STYLE.md, "The Regency look").
//!
//! - Armour: thick dark plates (`kit::dark_plate`) with bevelled edges, laid in courses that
//!   overlap and sweep back. A spike is the swept trailing edge of a plate
//!   ([`swept`], [`Course`]), never a thorn stuck on.
//! - Machinery under and between the plates, in dark bronze (`kit::metal`): collars and
//!   drums ([`collar`]), shafts ([`shaft`]), rings ([`hoop`]) and cable runs
//!   (`kit::cable`). Teeth ([`teeth`]) go only on a ring that turns; no rams, no gears.
//! - What braces one part off another is a plated strut ([`strut`]).
//! - Light: a lit red slot ([`red_slot`]) where the machine looks out or runs hot, and
//!   construction violet only on what builds ([`fabricator`]).
//!
//! Plates are placed in a [`Frame`]: `u` runs along the plate toward its trailing edge
//! (the way it sweeps), `v` across it and `n` out of its face.

use std::f32::consts::TAU;

use glam::{Vec2, Vec3};
use mc_core::print_heads::{factory_heads, TUBE};

use crate::builder::MeshBuilder;
use crate::material::*;

use super::kit::{dark_plate, metal, seam};

/// A plate's local axes: `u` along it toward the trailing edge, `v` across, `n` out of
/// its face (`u`, `v`, `n` right-handed, like x, y, z).
#[derive(Clone, Copy, Debug)]
pub(super) struct Frame {
    pub(super) o: Vec3,
    pub(super) u: Vec3,
    pub(super) v: Vec3,
    pub(super) n: Vec3,
}

impl Frame {
    /// A frame at `o` running along `u`, its face turned as near `n` as it can.
    pub(super) fn new(o: Vec3, u: Vec3, n: Vec3) -> Self {
        let u = u.normalize();
        let n = (n - u * n.dot(u)).normalize();
        Self {
            o,
            u,
            v: n.cross(u),
            n,
        }
    }

    pub(super) fn at(&self, s: f32, t: f32, h: f32) -> Vec3 {
        self.o + self.u * s + self.v * t + self.n * h
    }
}

/// An armour plate of `outline` (points `[s, t]` in `f`), `thick` deep out of the face,
/// its top edges bevelled in at full detail.
pub(super) fn armour(b: &mut MeshBuilder, f: &Frame, outline: &[[f32; 2]], thick: f32) {
    let ring = |h: f32, inset: f32| -> Vec<Vec3> {
        let (min, max) = outline.iter().fold(
            (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)),
            |(lo, hi), &[s, t]| (lo.min(Vec2::new(s, t)), hi.max(Vec2::new(s, t))),
        );
        let (mid, half) = (
            (min + max) * 0.5,
            ((max - min) * 0.5).max(Vec2::splat(0.01)),
        );
        let k = (Vec2::ONE - Vec2::splat(inset) / half).max(Vec2::splat(0.5));
        outline
            .iter()
            .map(|&[s, t]| {
                let q = mid + (Vec2::new(s, t) - mid) * k;
                f.at(q.x, q.y, h)
            })
            .collect()
    };
    if b.fine() {
        let bevel = (thick * 0.45).min(0.4);
        b.loft(
            &[ring(0.0, 0.0), ring(thick - bevel, 0.0), ring(thick, bevel)],
            true,
            true,
        );
    } else {
        // Its underside lies against what it covers: not drawn below full detail.
        b.loft(&[ring(0.0, 0.0), ring(thick, 0.0)], false, true);
    }
}

/// The outline of a swept plate: a straight leading edge across `t` in `[-half, half]`
/// at `s = 0`, its sides running back to `shoulder` of the length, then drawn into a
/// point at `(len, tip * half)`: the plate's spike.
pub(super) fn swept(len: f32, half: f32, tip: f32, shoulder: f32) -> [[f32; 2]; 5] {
    [
        [0.0, -half],
        [0.0, half],
        [len * shoulder, half],
        [len, tip * half],
        [len * shoulder, -half],
    ]
}

/// A course of swept plates laid back along a frame's `u`, overlapping like feathers:
/// each plate is `len` long and starts `step` behind the last, pitched up a little so
/// its tail lifts off the head of the one behind it. The last runs `tail` further, out
/// past the course into a spike.
#[derive(Clone, Copy, Debug)]
pub(super) struct Course {
    pub(super) count: usize,
    pub(super) step: f32,
    pub(super) len: f32,
    /// Half the plates' width, and where across it their spikes point (-1 to 1).
    pub(super) half: f32,
    pub(super) tip: f32,
    pub(super) thick: f32,
    pub(super) tail: f32,
}

impl Course {
    /// Lays the course in `f` with the current brush, and gives each plate's own frame
    /// and length.
    pub(super) fn lay(&self, b: &mut MeshBuilder, f: &Frame) -> Vec<(Frame, f32)> {
        (0..self.count)
            .map(|k| {
                let long = if k + 1 == self.count {
                    self.len + self.tail
                } else {
                    self.len
                };
                let g = Frame::new(f.at(k as f32 * self.step, 0.0, 0.0), f.u + f.n * 0.08, f.n);
                armour(b, &g, &swept(long, self.half, self.tip, 0.3), self.thick);
                (g, long)
            })
            .collect()
    }
}

/// A plated strut from `a` to `c`, `r` its half-width: what braces one part of a
/// structure off another.
pub(super) fn strut(b: &mut MeshBuilder, a: Vec3, c: Vec3, r: f32) {
    dark_plate(b);
    b.beam(a, c, Vec2::splat(r * 2.0), Vec2::splat(r * 1.6));
}

/// A bronze collar or drum `width` long about `axis` at `at`.
pub(super) fn collar(b: &mut MeshBuilder, at: Vec3, axis: Vec3, r: f32, width: f32) {
    let half = axis.normalize() * (width * 0.5);
    metal(b);
    let sides = b.sides(10);
    b.cylinder_between(at - half, at + half, r, r, sides);
}

/// A plain bronze shaft from `a` to `c` of radius `r`.
pub(super) fn shaft(b: &mut MeshBuilder, a: Vec3, c: Vec3, r: f32) {
    metal(b);
    let sides = if b.fine() { 8 } else { 4 };
    b.cylinder_between(a, c, r, r, sides);
}

/// A flat ring about the z axis through `c`: `r` to its middle, `w` across, `h` deep,
/// `segs` facets round. Painted with the current brush.
pub(super) fn hoop(b: &mut MeshBuilder, c: Vec3, r: f32, w: f32, h: f32, segs: usize) {
    hoop_on(b, c, Vec3::Z, r, w, h, segs);
}

/// A hot red rim on the face of a tube that ends at `mouth` (+x), `r` the tube's radius
/// there. It stands a little proud of the tube's end cap and inside its flats, so neither
/// face fights the tube's in depth.
pub(super) fn mouth_rim(b: &mut MeshBuilder, mouth: Vec3, r: f32, sides: usize) {
    b.paint(GLOW_LASER);
    hoop_on(
        b,
        mouth + Vec3::X * 0.015,
        Vec3::X,
        r * 0.68,
        r * 0.3,
        0.05,
        sides,
    );
}

/// [`hoop`] about `axis` instead of z.
pub(super) fn hoop_on(
    b: &mut MeshBuilder,
    c: Vec3,
    axis: Vec3,
    r: f32,
    w: f32,
    h: f32,
    segs: usize,
) {
    let axis = axis.normalize();
    let e1 = axis.any_orthonormal_vector();
    let e2 = axis.cross(e1);
    let section = [
        Vec2::new(r - w * 0.5, -h * 0.5),
        Vec2::new(r + w * 0.5, -h * 0.5),
        Vec2::new(r + w * 0.5, h * 0.5),
        Vec2::new(r - w * 0.5, h * 0.5),
    ];
    let rings: Vec<Vec<Vec3>> = (0..=segs)
        .map(|i| {
            let a = TAU * (i % segs) as f32 / segs as f32;
            let d = e1 * a.cos() + e2 * a.sin();
            section.iter().map(|p| c + d * p.x + axis * p.y).collect()
        })
        .collect();
    b.loft(&rings, false, false);
}

/// `count` teeth standing out from a ring of radius `r` about the z axis through `c`,
/// each `size` (out, along, up). Painted with the current brush.
pub(super) fn teeth(b: &mut MeshBuilder, c: Vec3, r: f32, count: usize, size: Vec3) {
    for i in 0..count {
        let a = TAU * (i as f32 + 0.5) / count as f32;
        let d = Vec3::new(a.cos(), a.sin(), 0.0);
        b.beam(
            c + d * r,
            c + d * (r + size.x),
            Vec2::new(size.y, size.z),
            Vec2::new(size.y * 0.6, size.z),
        );
    }
}

/// A lit red slot on a face: `len` along `along`, `h` across it, standing just proud of
/// the face whose outward normal is `out`.
pub(super) fn red_slot(b: &mut MeshBuilder, at: Vec3, out: Vec3, along: Vec3, len: f32, h: f32) {
    let out = out.normalize();
    let along = along.normalize();
    let across = out.cross(along) * (h * 0.5);
    let half = along * (len * 0.5);
    b.paint(GLOW_LASER);
    b.loft(
        &[
            vec![
                at - half - across,
                at + half - across,
                at + half + across,
                at - half + across,
            ],
            [
                at - half - across,
                at + half - across,
                at + half + across,
                at - half + across,
            ]
            .iter()
            .map(|&p| p + out * 0.12)
            .collect(),
        ],
        false,
        true,
    );
}

/// A fabricator head, the Regency's construction emitter: a bronze trunnion at `mount`, a
/// plated housing reaching toward `aim`, a bronze nozzle through a seam-dark collar and
/// a violet lens whose tip is `TUBE * s` from the mount, where the sim pours the nanite
/// stream from (`mc_core::print_heads::nozzle`). Its violet runs hot while the building
/// builds.
pub(super) fn fabricator(b: &mut MeshBuilder, mount: Vec3, aim: Vec3, s: f32) {
    let d = (aim - mount).normalize();
    let along = |k: f32| mount + d * (k * s);
    let tip = along(TUBE);
    if b.coarse() {
        b.paint(GLOW_VIOLET);
        b.cylinder_between(mount, tip, 0.5 * s, 0.3 * s, 4);
        return;
    }
    let sides = b.sides(8);
    if b.fine() {
        metal(b);
        let across = d.cross(Vec3::Z).try_normalize().unwrap_or(Vec3::Y);
        b.cylinder_between(
            mount - across * (0.75 * s),
            mount + across * (0.75 * s),
            0.42 * s,
            0.42 * s,
            6,
        );
    }
    dark_plate(b);
    b.cylinder_between(along(-0.5), along(1.8), 0.62 * s, 0.52 * s, sides);
    seam(b);
    b.cylinder_between(along(1.8), along(2.2), 0.5 * s, 0.5 * s, sides);
    metal(b);
    b.cylinder_between(along(2.2), along(3.0), 0.3 * s, 0.26 * s, sides);
    b.paint(GLOW_VIOLET);
    b.cylinder_between(along(3.0), tip, 0.34 * s, 0.12 * s, sides);
}

/// The fabricator heads that tier `tier`'s kit fits on the factory drawn with `mesh`
/// (`mc_core::print_heads`): their mounts, their scale, and the point they aim at, so
/// mesh and stream cannot drift apart.
pub(super) fn tier_heads(mesh: &str, tier: u8) -> impl Iterator<Item = (Vec3, f32, Vec3)> {
    let factory = factory_heads(mesh).expect("a Regency factory mesh");
    let aim = Vec3::from(factory.aim);
    factory
        .heads
        .iter()
        .filter(move |h| h.tier == tier)
        .map(move |h| (Vec3::from(h.mount), h.scale, aim))
}

/// Tier `tier`'s pieces of a structure drawn at tech `tech`: drawn when it has that
/// tier, carried as upgrade pieces (hidden until the refit, going up `at` of the way
/// through it) when it is the next one, and left off otherwise.
pub(super) fn tier(
    b: &mut MeshBuilder,
    tech: u8,
    tier: u8,
    at: f32,
    f: impl FnOnce(&mut MeshBuilder),
) {
    if tier <= tech {
        f(b);
    } else if tier == tech + 1 && b.fine() {
        b.upgrade(at, f);
    }
}
