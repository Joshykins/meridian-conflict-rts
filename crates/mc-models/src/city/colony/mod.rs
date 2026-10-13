//! The colony kit (map prop kinds 131..=141): a frontier colony's prefab town,
//! Vela's Port Calder, shipped down as panels and modules and bolted together.
//! Built on the city kit's tools and drawn by its shader (city.wgsl `PREFAB`,
//! `HAB`, `HAB_BAND`, `ACCENT`, `TRIM`), so it breaks, burns and comes down as
//! the city does.

use glam::{Vec2, Vec3};
use mc_map::PropKind;

use super::def;
use super::kit::*;
use crate::builder::MeshBuilder;
use crate::gpu_consts::city as pat;
use crate::library::ModelDef;

mod bridge;
mod depot;
mod dig;
mod fence;
mod hab;
mod hangar;
mod pad;
mod pit;
mod port;
mod stack;
mod tower;

pub(crate) const MODELS: &[ModelDef] = &[
    def("colony_hab", PropKind::ColonyHab, hab::hab),
    def("colony_hab_stack", PropKind::ColonyHabStack, stack::stack),
    def("colony_depot", PropKind::ColonyDepot, depot::depot),
    def("colony_tower", PropKind::ColonyTower, tower::tower),
    def("colony_pad", PropKind::ColonyPad, pad::pad),
    def(
        "colony_port_control",
        PropKind::ColonyPortControl,
        port::port,
    ),
    def("colony_hangar", PropKind::ColonyHangar, hangar::hangar),
    def("colony_dig", PropKind::ColonyDig, dig::dig),
    def("colony_bridge", PropKind::ColonyBridge, bridge::girder),
    def("colony_bridge~b", PropKind::ColonyBridge, bridge::haunched),
    def("colony_bridge~c", PropKind::ColonyBridge, bridge::through),
    def("colony_fence", PropKind::ColonyFence, fence::fence),
    def("colony_fence~b", PropKind::ColonyFence, fence::palisade),
    def("colony_gun_pit", PropKind::ColonyGunPit, pit::sandbags),
    def("colony_gun_pit~b", PropKind::ColonyGunPit, pit::blocks),
];

/// The outline of `r` at height `z` with its upright corners cut back `cut` metres in
/// `steps` facets (1 a chamfer, more a rounding), counter-clockwise from above, from
/// the corner at its least x and y.
fn cut_ring(r: Rect, cut: f32, steps: usize, z: f32) -> Vec<Vec3> {
    let cut = cut.min(r.size().min_element() * 0.5 - 0.01);
    if cut <= 0.0 {
        return r.ring(z);
    }
    let corners = [
        (Vec2::new(r.min.x, r.min.y), Vec2::new(1.0, 1.0), 180.0f32),
        (Vec2::new(r.max.x, r.min.y), Vec2::new(-1.0, 1.0), 270.0),
        (Vec2::new(r.max.x, r.max.y), Vec2::new(-1.0, -1.0), 0.0),
        (Vec2::new(r.min.x, r.max.y), Vec2::new(1.0, -1.0), 90.0),
    ];
    let mut out = Vec::with_capacity(4 * (steps + 1));
    for (at, inward, start) in corners {
        let c = at + inward * cut;
        for k in 0..=steps {
            let a = (start + 90.0 * k as f32 / steps as f32).to_radians();
            out.push((c + Vec2::new(a.cos(), a.sin()) * cut).extend(z));
        }
    }
    out
}

/// The walls of `r` from `z0` to `z1`, corners cut as [`cut_ring`]: each straight
/// side one face in its own pattern (`sides`: front, back, east, west, as
/// [`Side::ALL`]), the corner facets in `corner`.
#[expect(
    clippy::too_many_arguments,
    reason = "a module's walls are placed by its plan, its corners, its heights and their looks"
)]
fn cut_walls(
    b: &mut MeshBuilder,
    r: Rect,
    cut: f32,
    steps: usize,
    z0: f32,
    z1: f32,
    sides: [u32; 4],
    corner: u32,
) {
    let lo = cut_ring(r, cut, steps, z0);
    let hi = cut_ring(r, cut, steps, z1);
    ring_walls(b, &lo, &hi, steps, sides, corner);
}

/// Walls between two outlines of [`cut_ring`]'s shape and count, leaning where they
/// differ: each straight side in its own pattern, the corner facets in `corner`.
fn ring_walls(
    b: &mut MeshBuilder,
    lo: &[Vec3],
    hi: &[Vec3],
    steps: usize,
    sides: [u32; 4],
    corner: u32,
) {
    let n = lo.len();
    let per = if n == 4 { 1 } else { steps + 1 };
    for i in 0..n {
        let j = (i + 1) % n;
        let d = hi[j] - hi[i];
        if d.truncate().length() < 1e-3 {
            continue;
        }
        let out = Vec3::new(d.y, -d.x, 0.0);
        // The straight run after corner k: back (least y), east, front, west.
        let pattern = if i % per == per - 1 {
            match i / per {
                0 => sides[1],
                1 => sides[2],
                2 => sides[0],
                _ => sides[3],
            }
        } else {
            corner
        };
        paint(b, pattern);
        facing(b, vec![lo[i], lo[j], hi[j], hi[i]], out);
    }
}

/// One look on every side of a module.
fn all(pattern: u32) -> [u32; 4] {
    [pattern; 4]
}

/// A closed slab over `r` from `z0` to `z1`, corners cut as [`cut_ring`].
fn cut_slab(b: &mut MeshBuilder, r: Rect, cut: f32, steps: usize, z0: f32, z1: f32, pattern: u32) {
    paint(b, pattern);
    b.loft(
        &[cut_ring(r, cut, steps, z0), cut_ring(r, cut, steps, z1)],
        true,
        true,
    );
}

/// A level ring between two outlines of the same count, facing up or down.
fn ring_ledge(b: &mut MeshBuilder, inner: &[Vec3], outer: &[Vec3], up: bool, pattern: u32) {
    paint(b, pattern);
    let out = if up { Vec3::Z } else { Vec3::NEG_Z };
    let n = inner.len();
    for k in 0..n {
        let j = (k + 1) % n;
        facing(b, vec![outer[k], outer[j], inner[j], inner[k]], out);
    }
}

/// A recessed band round a module: its walls drawn in `inset` from `z0` to `z1` in
/// `pattern`, with the ledges of the walls above and below it.
#[expect(
    clippy::too_many_arguments,
    reason = "a band is placed by its module's plan and corners, its heights and its look"
)]
fn recess(
    b: &mut MeshBuilder,
    r: Rect,
    cut: f32,
    steps: usize,
    z0: f32,
    z1: f32,
    inset: f32,
    pattern: u32,
) {
    let inner = r.grow(-inset);
    let icut = (cut - inset * 0.4).max(0.0);
    cut_walls(b, inner, icut, steps, z0, z1, all(pattern), pattern);
    if b.fine() {
        let o0 = cut_ring(r, cut, steps, z0);
        let i0 = cut_ring(inner, icut, steps, z0);
        ring_ledge(b, &i0, &o0, true, pattern);
        let o1 = cut_ring(r, cut, steps, z1);
        let i1 = cut_ring(inner, icut, steps, z1);
        ring_ledge(b, &i1, &o1, false, pattern);
    }
}

/// A battered skirt: the ground course of a heavy building, sloping in from `out`
/// past `r` at the ground to `r` at `z`, taken 2 m into the ground.
fn skirt(b: &mut MeshBuilder, r: Rect, cut: f32, out: f32, z: f32, pattern: u32) {
    paint(b, pattern);
    let wide = r.grow(out);
    b.loft(
        &[
            cut_ring(wide, cut + out * 0.4, 1, -2.0),
            cut_ring(wide, cut + out * 0.4, 1, 0.0),
            cut_ring(r, cut, 1, z),
        ],
        false,
        false,
    );
    ring_ledge(
        b,
        &cut_ring(r.grow(-0.4), cut, 1, z),
        &cut_ring(r, cut, 1, z),
        true,
        pattern,
    );
}

/// A horizontal tank on two saddles, its axis along x from `a` (its middle at the
/// height of its axis) `len` long.
fn tank(b: &mut MeshBuilder, at: Vec3, len: f32, r: f32, pattern: u32) {
    paint(b, pattern);
    let sides = b.sides(14);
    b.cylinder_between(
        at - Vec3::X * len * 0.5,
        at + Vec3::X * len * 0.5,
        r,
        r,
        sides,
    );
    if b.fine() {
        for s in [-0.3f32, 0.3] {
            let x = at.x + s * len;
            boxed(
                b,
                v3(x - 0.2, at.y - r * 0.8, at.z - r - 0.6),
                v3(x + 0.2, at.y + r * 0.8, at.z - r * 0.5),
                pat::TRIM,
            );
        }
    }
}

/// A lattice-less comms mast: a tapering pole from `at` up `h`, cross-arms and a dish
/// at full detail.
fn comms_mast(b: &mut MeshBuilder, at: Vec3, h: f32) {
    paint(b, pat::STEEL);
    b.beam(at, at + Vec3::Z * h, Vec2::splat(0.35), Vec2::splat(0.14));
    if !b.fine() {
        return;
    }
    for k in 0..3 {
        let z = at.z + h * (0.45 + 0.18 * k as f32);
        let w = 1.6 - 0.4 * k as f32;
        b.beam(
            v3(at.x - w, at.y, z),
            v3(at.x + w, at.y, z),
            Vec2::splat(0.08),
            Vec2::splat(0.08),
        );
    }
    paint(b, pat::WHITE_STEEL);
    b.cylinder_between(
        at + v3(0.15, 0.0, h * 0.35),
        at + v3(0.7, 0.4, h * 0.35 + 0.2),
        0.9,
        0.15,
        10,
    );
}

/// Railings along a line from `a` to `b` at the height of their foot: a top rail and
/// posts every 1.5 m. Full detail.
fn rail(b: &mut MeshBuilder, a: Vec3, c: Vec3) {
    if !b.fine() {
        return;
    }
    paint(b, pat::TRIM);
    let h = Vec3::Z * 1.05;
    b.beam(a + h, c + h, Vec2::splat(0.06), Vec2::splat(0.06));
    let n = ((c - a).length() / 1.5).ceil().max(1.0) as usize;
    for k in 0..=n {
        let p = a.lerp(c, k as f32 / n as f32);
        b.beam(p, p + h, Vec2::splat(0.05), Vec2::splat(0.05));
    }
}

/// A sloped hood over a window or a door on wall `side` of `r`: `a0..a1` along it, its
/// foot at `z`, standing `deep` out. Full detail.
#[expect(
    clippy::too_many_arguments,
    reason = "a hood is placed by its wall, its run, its height and its depth"
)]
fn hood(
    b: &mut MeshBuilder,
    r: Rect,
    side: Side,
    a0: f32,
    a1: f32,
    z: f32,
    deep: f32,
    pattern: u32,
) {
    if !b.fine() {
        return;
    }
    let (_, _, plane) = side.run(r);
    let o = match side {
        Side::Front | Side::East => deep,
        Side::Back | Side::West => -deep,
    };
    let profile = [
        [plane, z],
        [plane + o, z],
        [plane + o, z + 0.12],
        [plane, z + 0.12 + deep * 0.5],
    ];
    paint(b, pattern);
    b.with_facets(|b| match side {
        Side::Front | Side::Back => b.extrude_x(&profile, a0, a1),
        Side::East | Side::West => b.extrude_y(&profile, a0, a1),
    });
}

/// A glazed cab, its glass canted out from `lo` at `z0` to `hi` at `z1`, under a heavy
/// brim and a flat roof; the roof's height.
fn canted_cab(b: &mut MeshBuilder, lo: Rect, z0: f32, hi: Rect, z1: f32, cut: f32) -> f32 {
    ring_walls(
        b,
        &cut_ring(lo, cut, 1, z0),
        &cut_ring(hi, cut + 0.6, 1, z1),
        1,
        all(pat::HAB_BAND),
        pat::TRIM,
    );
    let brim = hi.grow(0.4);
    let lid = hi.grow(-2.0);
    let roof = z1 + 1.8;
    paint(b, pat::PREFAB);
    b.loft(
        &[
            cut_ring(hi, cut + 0.6, 1, z1),
            cut_ring(brim, cut + 0.8, 1, z1 + 0.2),
            cut_ring(brim, cut + 0.8, 1, z1 + 0.9),
            cut_ring(lid, cut, 1, roof),
        ],
        false,
        false,
    );
    deck(b, lid, roof, pat::ROOF_FLAT);
    roof
}

/// A lamp mast from `at` up `h`: a pole and a lamp head lit by the surface. Full
/// detail.
fn lamp_mast(b: &mut MeshBuilder, at: Vec3, h: f32) {
    if !b.fine() {
        return;
    }
    paint(b, pat::STEEL);
    b.beam(at, at + Vec3::Z * h, Vec2::splat(0.3), Vec2::splat(0.18));
    boxed(
        b,
        at + v3(-0.6, -0.35, h - 0.5),
        at + v3(0.6, 0.35, h),
        pat::LED,
    );
}

/// A heap of spoil round `at`, `r` across and `h` high, its outline rough.
fn spoil(b: &mut MeshBuilder, at: Vec3, r: Vec2, h: f32, seed: u32) {
    let n = if b.fine() { 14 } else { 7 };
    let ring = |k: f32, z: f32| -> Vec<Vec3> {
        (0..n)
            .map(|i| {
                let a = i as f32 / n as f32 * std::f32::consts::TAU;
                let wob = 0.85 + 0.3 * crate::builder::hash_unit(seed, i as u32);
                at + v3(a.cos() * r.x * k * wob, a.sin() * r.y * k * wob, z)
            })
            .collect()
    };
    paint(b, pat::RUBBLE);
    b.loft(
        &[
            ring(1.0, -1.0),
            ring(0.75, h * 0.45),
            ring(0.35, h * 0.85),
            ring(0.08, h),
        ],
        false,
        true,
    );
}
