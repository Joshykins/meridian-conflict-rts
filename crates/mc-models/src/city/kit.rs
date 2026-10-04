//! The city kit's tools: boxes of walls and roofs on a plan, cornices and
//! parapets, gabled and hipped roofs, openings, balconies laid to the facade's
//! grid, and rooftop plant. Every face is `CONCRETE` with a `gpu_consts::city`
//! pattern: city.wgsl draws the wall, its windows and the roofs from it.

use glam::{Vec2, Vec3};
use mc_map::city::{structure, Structure};
use mc_map::PropKind;

use crate::builder::{hash_unit, MeshBuilder};
use crate::gpu_consts::city as pat;
use crate::material::CONCRETE;

pub(super) fn v3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

/// A city kind's numbers (`mc_map::city`).
pub(super) fn the(kind: PropKind) -> Structure {
    structure(kind).expect("a city kind")
}

/// Part `i` of a city kind's plan, and its top, at scale 1.
pub(super) fn part(kind: PropKind, i: usize) -> (Rect, f32) {
    let s = the(kind);
    let (cx, cy, hx, hy) = s.plan[i];
    (
        Rect::centred(cx as f32, cy as f32, hx as f32, hy as f32),
        s.tops[i] as f32,
    )
}

/// How the shader lays a facade's cells across `len` metres at `pitch`: the count and
/// each cell's size (gpu_consts `city`: `n = max(1, round(len / pitch))`).
pub(super) fn grid(len: f32, pitch: f32) -> (usize, f32) {
    let n = (len / pitch).round().max(1.0);
    (n as usize, len / n)
}

/// The middles of the cells across `len` metres from `start`, at `pitch`.
pub(super) fn cells(start: f32, len: f32, pitch: f32) -> impl Iterator<Item = f32> {
    let (n, step) = grid(len, pitch);
    (0..n).map(move |i| start + (i as f32 + 0.5) * step)
}

/// An upright rectangle in plan.
#[derive(Clone, Copy, Debug)]
pub(super) struct Rect {
    pub min: Vec2,
    pub max: Vec2,
}

impl Rect {
    pub(super) fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect {
            min: Vec2::new(x0.min(x1), y0.min(y1)),
            max: Vec2::new(x0.max(x1), y0.max(y1)),
        }
    }

    pub(super) fn centred(cx: f32, cy: f32, hx: f32, hy: f32) -> Rect {
        Rect::new(cx - hx, cy - hy, cx + hx, cy + hy)
    }

    pub(super) fn grow(self, d: f32) -> Rect {
        self.grow_xy(d, d)
    }

    pub(super) fn grow_xy(self, dx: f32, dy: f32) -> Rect {
        Rect::new(
            self.min.x - dx,
            self.min.y - dy,
            self.max.x + dx,
            self.max.y + dy,
        )
    }

    pub(super) fn size(self) -> Vec2 {
        self.max - self.min
    }

    pub(super) fn centre(self) -> Vec2 {
        (self.min + self.max) * 0.5
    }

    /// Its corners at height `z`, counter-clockwise from above.
    pub(super) fn ring(self, z: f32) -> Vec<Vec3> {
        vec![
            v3(self.min.x, self.min.y, z),
            v3(self.max.x, self.min.y, z),
            v3(self.max.x, self.max.y, z),
            v3(self.min.x, self.max.y, z),
        ]
    }
}

/// A wall of a box: which way it faces.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Side {
    /// +y: a building's street front (the bake turns rows so).
    Front,
    Back,
    /// +x and -x.
    East,
    West,
}

impl Side {
    pub(super) const ALL: [Side; 4] = [Side::Front, Side::Back, Side::East, Side::West];

    pub(super) fn out(self) -> Vec3 {
        match self {
            Side::Front => Vec3::Y,
            Side::Back => Vec3::NEG_Y,
            Side::East => Vec3::X,
            Side::West => Vec3::NEG_X,
        }
    }

    /// The wall's run along it (from, to) and its plane, on `r`.
    pub(super) fn run(self, r: Rect) -> (f32, f32, f32) {
        match self {
            Side::Front => (r.min.x, r.max.x, r.max.y),
            Side::Back => (r.min.x, r.max.x, r.min.y),
            Side::East => (r.min.y, r.max.y, r.max.x),
            Side::West => (r.min.y, r.max.y, r.min.x),
        }
    }

    /// The point `along` the wall of `r`, `off` out from it, at height `z`.
    pub(super) fn at(self, r: Rect, along: f32, off: f32, z: f32) -> Vec3 {
        let (_, _, plane) = self.run(r);
        match self {
            Side::Front | Side::Back => v3(along, plane + self.out().y * off, z),
            Side::East | Side::West => v3(plane + self.out().x * off, along, z),
        }
    }

    /// A box standing out from the wall of `r`: `along` a0..a1, from `off0` to `off1`
    /// out, z0..z1.
    pub(super) fn block(
        self,
        r: Rect,
        a0: f32,
        a1: f32,
        off0: f32,
        off1: f32,
        z0: f32,
        z1: f32,
    ) -> (Vec3, Vec3) {
        let p = self.at(r, a0, off0, z0);
        let q = self.at(r, a1, off1, z1);
        (p.min(q), p.max(q))
    }
}

/// Paints what follows: city concrete with `pattern`.
pub(super) fn paint(b: &mut MeshBuilder, pattern: u32) {
    b.paint(CONCRETE).pattern(pattern);
}

/// A one-sided polygon turned to face `out`.
pub(super) fn facing(b: &mut MeshBuilder, mut points: Vec<Vec3>, out: Vec3) {
    let n = (points[1] - points[0]).cross(points[2] - points[0]);
    if n.dot(out) < 0.0 {
        points.reverse();
    }
    b.face(&points);
}

/// The four walls of `r` from `z0` to `z1`, each one face.
pub(super) fn walls(b: &mut MeshBuilder, r: Rect, z0: f32, z1: f32, pattern: u32) {
    paint(b, pattern);
    b.loft(&[r.ring(z0), r.ring(z1)], false, false);
}

/// The walls of `r` from `z0` to `z1` on the given sides only.
pub(super) fn walls_on(
    b: &mut MeshBuilder,
    r: Rect,
    z0: f32,
    z1: f32,
    sides: &[Side],
    pattern: u32,
) {
    paint(b, pattern);
    for &side in sides {
        let (a0, a1, _) = side.run(r);
        panel(b, r, side, a0, a1, z0, z1, 0.0);
    }
}

/// A flat roof or floor over `r` at `z`, facing up.
pub(super) fn deck(b: &mut MeshBuilder, r: Rect, z: f32, pattern: u32) {
    paint(b, pattern);
    b.face(&r.ring(z));
}

/// Walls and a flat top.
pub(super) fn solid(b: &mut MeshBuilder, r: Rect, z0: f32, z1: f32, wall: u32, top: u32) {
    walls(b, r, z0, z1, wall);
    deck(b, r, z1, top);
}

/// A closed box in the current paint, `pattern`.
pub(super) fn boxed(b: &mut MeshBuilder, min: Vec3, max: Vec3, pattern: u32) {
    paint(b, pattern);
    b.block(min, max);
}

/// A quad on wall `side` of `r`: `a0..a1` along it, `z0..z1`, `off` proud of it.
#[expect(
    clippy::too_many_arguments,
    reason = "a wall piece is placed by its run, its heights and its look"
)]
pub(super) fn panel(
    b: &mut MeshBuilder,
    r: Rect,
    side: Side,
    a0: f32,
    a1: f32,
    z0: f32,
    z1: f32,
    off: f32,
) {
    let pts = vec![
        side.at(r, a0, off, z0),
        side.at(r, a1, off, z0),
        side.at(r, a1, off, z1),
        side.at(r, a0, off, z1),
    ];
    facing(b, pts, side.out());
}

/// The base course: the walls taken 2 m into the ground (a building on a slope never
/// floats) up to `top`, standing a little proud.
pub(super) fn plinth(b: &mut MeshBuilder, r: Rect, top: f32, pattern: u32) {
    walls(b, r.grow(0.08), -2.0, top, pattern);
    ledge(b, r, r.grow(0.08), top, true, pattern);
}

/// The flat ring between `inner` and `outer` at `z`, facing up or down.
pub(super) fn ledge(b: &mut MeshBuilder, inner: Rect, outer: Rect, z: f32, up: bool, pattern: u32) {
    paint(b, pattern);
    let i = inner.ring(z);
    let o = outer.ring(z);
    let out = if up { Vec3::Z } else { Vec3::NEG_Z };
    for k in 0..4 {
        let j = (k + 1) % 4;
        facing(b, vec![o[k], o[j], i[j], i[k]], out);
    }
}

/// A cornice: a band `h` deep standing `out` proud of `r`'s walls, its top at `z`.
pub(super) fn cornice(b: &mut MeshBuilder, r: Rect, z: f32, h: f32, out: f32, pattern: u32) {
    let o = r.grow(out);
    walls(b, o, z - h, z, pattern);
    // Its top and underside only read up close.
    if b.fine() {
        ledge(b, r, o, z - h, false, pattern);
        ledge(b, r, o, z, true, pattern);
    }
}

/// A parapet round the roof of `r`: walls `thick` thick from `z` to `z + h`, its
/// outer face flush with the wall below. Full detail; below it, a plain band.
pub(super) fn parapet(b: &mut MeshBuilder, r: Rect, z: f32, h: f32, thick: f32, wall: u32) {
    walls(b, r, z, z + h, wall);
    let inner = r.grow(-thick);
    if b.mid() {
        paint(b, wall);
        // The inner faces, looking in over the roof.
        b.loft(&[inner.ring(z + h), inner.ring(z)], false, false);
        ledge(b, inner, r, z + h, true, pat::CONCRETE);
    } else {
        ledge(b, inner, r, z + h, true, pat::CONCRETE);
    }
}

/// A gabled roof over `r`, its ridge along x (or along y when `along_y`), from the
/// eaves at `eaves` to the ridge at `ridge`, overhanging `over`: a slab 0.25 m thick
/// with the gable walls under its ends in `gable`.
#[expect(
    clippy::too_many_arguments,
    reason = "a wall piece is placed by its run, its heights and its look"
)]
pub(super) fn gable_roof(
    b: &mut MeshBuilder,
    r: Rect,
    eaves: f32,
    ridge: f32,
    over: f32,
    along_y: bool,
    roof: u32,
    gable: u32,
) {
    let (lo, hi, mid, half) = if along_y {
        (r.min.y, r.max.y, r.centre().x, r.size().x * 0.5)
    } else {
        (r.min.x, r.max.x, r.centre().y, r.size().y * 0.5)
    };
    let w = half + over;
    let slope = (ridge - eaves) / half;
    let drop = over * slope;
    let t = 0.25;
    let profile = [
        [mid - w, eaves - drop - t],
        [mid - w, eaves - drop],
        [mid, ridge],
        [mid + w, eaves - drop],
        [mid + w, eaves - drop - t],
        [mid, ridge - t * 1.3],
    ];
    paint(b, roof);
    let end_over = over * 0.8;
    // Flat slopes: a roof's planes meet at its ridge and eaves, never curve.
    b.with_facets(|b| {
        if along_y {
            // extrude_y takes (x, z).
            b.extrude_y(&profile, lo - end_over, hi + end_over);
        } else {
            b.extrude_x(&profile, lo - end_over, hi + end_over);
        }
    });
    // The gables under it, flush with the walls.
    paint(b, gable);
    for (end, out) in [(lo, -1.0f32), (hi, 1.0)] {
        let tri = if along_y {
            vec![
                v3(mid - half, end, eaves),
                v3(mid + half, end, eaves),
                v3(mid, end, ridge - t),
            ]
        } else {
            vec![
                v3(end, mid - half, eaves),
                v3(end, mid + half, eaves),
                v3(end, mid, ridge - t),
            ]
        };
        let normal = if along_y {
            Vec3::Y * out
        } else {
            Vec3::X * out
        };
        facing(b, tri, normal);
    }
}

/// A mansard: steep slopes from the walls of `r` at `z` up to `z + h`, drawn in by
/// `inset`, a flat roof on top.
pub(super) fn mansard(
    b: &mut MeshBuilder,
    r: Rect,
    z: f32,
    h: f32,
    inset: f32,
    roof: u32,
    top: u32,
) {
    paint(b, roof);
    let inner = r.grow(-inset);
    b.loft(&[r.ring(z), inner.ring(z + h)], false, false);
    deck(b, inner, z + h, top);
}

/// Dormers on a mansard or a roof face of `r` on `side`: one per cell of the facade
/// grid `pitch` wide, `w` wide and `h` tall, their fronts `back` in from the wall at
/// `z`, a window drawn on each by `facade`.
#[expect(
    clippy::too_many_arguments,
    reason = "a wall piece is placed by its run, its heights and its look"
)]
pub(super) fn dormers(
    b: &mut MeshBuilder,
    r: Rect,
    side: Side,
    pitch: f32,
    w: f32,
    h: f32,
    back: f32,
    z: f32,
    facade: u32,
    roof: u32,
) {
    let (a0, a1, _) = side.run(r);
    for c in cells(a0, a1 - a0, pitch) {
        let (min, max) = side.block(r, c - w * 0.5, c + w * 0.5, -back - 2.0, -back, z, z + h);
        walls(b, Rect::new(min.x, min.y, max.x, max.y), z, z + h, facade);
        let rr = Rect::new(min.x, min.y, max.x, max.y);
        let along_y = matches!(side, Side::Front | Side::Back);
        gable_roof(b, rr, z + h, z + h + w * 0.45, 0.15, along_y, roof, roof);
    }
}

/// A dark doorway on wall `side` of `r`, `w` wide and `h` tall at `along`, with a
/// stone or steel surround and, when `canopy`, a slab over it.
#[expect(
    clippy::too_many_arguments,
    reason = "a wall piece is placed by its run, its heights and its look"
)]
pub(super) fn door(
    b: &mut MeshBuilder,
    r: Rect,
    side: Side,
    along: f32,
    w: f32,
    h: f32,
    canopy: bool,
    surround: u32,
) {
    paint(b, pat::SHADOW);
    panel(b, r, side, along - w * 0.5, along + w * 0.5, 0.0, h, 0.03);
    if !b.fine() {
        return;
    }
    let f = 0.18;
    for (a0, a1, z0, z1) in [
        (along - w * 0.5 - f, along - w * 0.5, 0.0, h + f),
        (along + w * 0.5, along + w * 0.5 + f, 0.0, h + f),
        (along - w * 0.5, along + w * 0.5, h, h + f),
    ] {
        let (min, max) = side.block(r, a0, a1, 0.0, 0.1, z0, z1);
        boxed(b, min, max, surround);
    }
    if canopy {
        let (min, max) = side.block(
            r,
            along - w * 0.5 - 0.6,
            along + w * 0.5 + 0.6,
            0.0,
            1.4,
            h + 0.3,
            h + 0.5,
        );
        boxed(b, min, max, pat::STEEL);
    }
}

/// Balconies on wall `side` of `r`: a slab and a solid or railed front at every storey
/// from `z0` (each `storey` up, `floors` of them) under each cell of the facade grid
/// `bay` wide, `w` of the cell wide and `deep` out. Lined up with the windows the
/// shader draws on the same grid.
#[expect(
    clippy::too_many_arguments,
    reason = "a wall piece is placed by its run, its heights and its look"
)]
pub(super) fn balconies(
    b: &mut MeshBuilder,
    r: Rect,
    side: Side,
    bay: f32,
    every: usize,
    z0: f32,
    storey: f32,
    floors: usize,
    w: f32,
    deep: f32,
    front: u32,
) {
    let (a0, a1, _) = side.run(r);
    let (_, step) = grid(a1 - a0, bay);
    for (i, c) in cells(a0, a1 - a0, bay).enumerate() {
        if i % every != every / 2 % every.max(1) {
            continue;
        }
        for f in 0..floors {
            let z = z0 + f as f32 * storey;
            let half = step * w * 0.5;
            let (min, max) = side.block(r, c - half, c + half, 0.0, deep, z - 0.18, z);
            boxed(b, min, max, pat::CONCRETE);
            if b.fine() {
                let (min, max) = side.block(r, c - half, c + half, deep - 0.08, deep, z, z + 1.05);
                boxed(b, min, max, front);
            }
        }
    }
}

/// Rooftop plant on the roof `r` at `z`: air handlers, vents, a stair and lift
/// house, a water tank on legs when `tank`. Full detail; the lift house at mid.
pub(super) fn plant(b: &mut MeshBuilder, r: Rect, z: f32, seed: u32, units: u32, tank: bool) {
    plant_with(b, r, z, seed, units, tank, true);
}

/// [`plant`], with or without the lift and stair house (a low building has none).
pub(super) fn plant_with(
    b: &mut MeshBuilder,
    r: Rect,
    z: f32,
    seed: u32,
    units: u32,
    tank: bool,
    lift_house: bool,
) {
    let c = r.centre();
    let s = r.size();
    // The lift and stair house.
    let lift = Rect::centred(
        c.x - s.x * 0.18,
        c.y + s.y * 0.08,
        (s.x * 0.12).clamp(2.0, 5.0),
        (s.y * 0.14).clamp(1.8, 4.0),
    );
    if lift_house {
        solid(b, lift, z, z + 3.2, pat::CONCRETE, pat::ROOF_FLAT);
    }
    if !b.fine() {
        return;
    }
    for i in 0..units {
        let p = c + Vec2::new(hash_unit(seed, i) - 0.5, hash_unit(seed, 10 + i) - 0.5) * s * 0.6;
        let size = v3(
            2.0 + hash_unit(seed, 20 + i) * 2.0,
            1.4 + hash_unit(seed, 30 + i),
            1.3,
        );
        if lift_house
            && Rect::centred(p.x, p.y, size.x * 0.5 + 0.5, size.y * 0.5 + 0.5)
                .min
                .x
                < lift.max.x
            && Rect::centred(p.x, p.y, size.x * 0.5 + 0.5, size.y * 0.5 + 0.5)
                .max
                .x
                > lift.min.x
            && (p.y - lift.centre().y).abs() < lift.size().y * 0.5 + size.y * 0.5 + 0.5
        {
            continue;
        }
        let min = v3(p.x - size.x * 0.5, p.y - size.y * 0.5, z);
        boxed(b, min, min + size, pat::STEEL);
        // Its fan on top.
        paint(b, pat::SHADOW);
        b.prism(
            v3(p.x, p.y, z + size.z),
            8,
            size.y * 0.32,
            size.y * 0.32,
            0.12,
        );
    }
    // Vent stacks.
    paint(b, pat::STEEL);
    for i in 0..3 {
        let p =
            c + Vec2::new(hash_unit(seed, 40 + i) - 0.5, hash_unit(seed, 50 + i) - 0.5) * s * 0.75;
        b.prism(p.extend(z), 6, 0.25, 0.25, 1.4 + hash_unit(seed, 60 + i));
    }
    if tank {
        // A timber-and-steel water tank on a frame, as on older blocks.
        let at = Vec2::new(c.x + s.x * 0.22, c.y - s.y * 0.15);
        for (dx, dy) in [(-1.2, -1.2), (1.2, -1.2), (1.2, 1.2), (-1.2, 1.2)] {
            boxed(
                b,
                v3(at.x + dx - 0.12, at.y + dy - 0.12, z),
                v3(at.x + dx + 0.12, at.y + dy + 0.12, z + 2.4),
                pat::STEEL,
            );
        }
        paint(b, pat::TIMBER);
        b.prism(at.extend(z + 2.4), 12, 1.9, 1.9, 2.8);
        paint(b, pat::ROOF_METAL);
        b.prism(at.extend(z + 5.2), 12, 2.0, 0.2, 0.9);
    }
}

/// A rainwater pipe down a wall of `r` from `z` to the ground at `along`.
pub(super) fn downpipe(b: &mut MeshBuilder, r: Rect, side: Side, along: f32, z: f32) {
    if !b.fine() {
        return;
    }
    let (min, max) = side.block(r, along - 0.07, along + 0.07, 0.02, 0.16, 0.0, z);
    boxed(b, min, max, pat::STEEL);
}

/// A mast or aerial: a thin square bar from `at` up `h`.
pub(super) fn mast(b: &mut MeshBuilder, at: Vec3, h: f32, w: f32) {
    paint(b, pat::STEEL);
    b.beam(at, at + Vec3::Z * h, Vec2::splat(w), Vec2::splat(w * 0.4));
}
