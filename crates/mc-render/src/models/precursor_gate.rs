//! The Threshold's Precursor facility kit: the great land gate the one road into the
//! facility climbs through (`mc_map::PropKind::PrecursorGate`), and the sea platform
//! standing off its coast on four legs (`PrecursorPlatform`). Contracts (solid plans,
//! clear zones) live in mc-map `format.rs` `solid_plan()`; the numbers are exported
//! here so the tests hold the models to them.
//!
//! Same language as the megastructure (`precursor_mega.rs`): pale alloy over a dark
//! core, cold light in the seams. At this size what reads from kilometres is the
//! silhouette and the layering: stepped tiers, each hovering over the one below on a
//! neck of light; dark recessed bands down every girder split by pale ribs; shafts
//! of dark core between pale corner piers with a line of light up every face; and
//! segments floating apart over light. Close up the faces carry ribs, fins, nosings
//! with light along them, coffers and plates.
//!
//! Model space as for every prop: x along the heading, y left, z up, the origin on
//! the ground. Footings go to -80 m.

use glam::{Affine3A, Vec2, Vec3};

use super::builder::{MeshBuilder, Section};
use super::library::ModelDef;
use super::precursor::{cut_rect, dark, film, fine_rect, key_light, light, pale, panel, seam, v3};
use super::precursor_mega::{girder, Run};

pub(super) const MODELS: &[ModelDef] = &[
    ModelDef::new("precursor_platform", 262.0, PLATFORM_TOP, platform),
    ModelDef::new("precursor_gate", 550.0, GATE_TOP, gate),
    ModelDef::new("precursor_floor", 283.0, FLOOR_TOP, floor),
    ModelDef::new("precursor_viaduct", 101.0, VIADUCT_TOP, viaduct),
    ModelDef::new("precursor_pier", 24.0, PIER_TOP, pier),
];

/// Full-detail triangle budget per model (held by the tests).
#[cfg(test)]
pub(super) const TRIANGLES: usize = 60_000;

// ---- Shared pieces ------------------------------------------------------------------

fn mirror_x(b: &mut MeshBuilder, f: impl Fn(&mut MeshBuilder)) {
    f(b);
    b.with(Affine3A::from_scale(v3(-1.0, 1.0, 1.0)), |b| f(b));
}

/// A frame whose local x runs along world y and local y along world -x: an (x, z)
/// profile drawn here is a (y, z) profile extruded along the world's x.
fn across() -> Affine3A {
    Affine3A::from_cols(
        Vec3::Y.into(),
        (-Vec3::X).into(),
        Vec3::Z.into(),
        Vec3::ZERO.into(),
    )
}

/// A tier of shaft: a dark core between four pale corner piers, its plan `h0` half
/// size at `z0` drawing in to `h1` at `z1` about `c`. Up the middle of every face a
/// line of light, pale ribs across the dark every `bay` metres, and a pale nosing
/// over it with a line of light along it (to `z1 + 5`).
fn stage(b: &mut MeshBuilder, c: Vec2, h0: Vec2, h1: Vec2, z0: f32, z1: f32, bay: f32) {
    let k = h1 / h0;
    let at = |z: f32, s: Vec2| Section::scaled(z, s.x, s.y).shifted(c.x, c.y);
    let small = h0.min_element();
    dark(b);
    let core = cut_rect(b, h0.x, h0.y, small * 0.22);
    b.loft_z(&core, &[at(z0, Vec2::ONE), at(z1, k)]);

    // The corner piers, standing proud of the core.
    pale(b);
    let p0 = small * 0.2;
    let p1 = h1.min_element() * 0.2;
    let pier = fine_rect(b, p0, p0, p0 * 0.3);
    let (o0, o1) = (h0 - Vec2::splat(p0 - 1.6), h1 - Vec2::splat(p1 - 1.6));
    for (sx, sy) in [(1.0f32, 1.0f32), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
        let s = Vec2::new(sx, sy);
        b.loft_z(
            &pier,
            &[
                Section::new(z0 - 0.5, 1.0).shifted(c.x + s.x * o0.x, c.y + s.y * o0.y),
                Section::new(z1 + 0.5, p1 / p0).shifted(c.x + s.x * o1.x, c.y + s.y * o1.y),
            ],
        );
    }

    // The nosing, and the light along it.
    let cap = cut_rect(b, h1.x + 3.0, h1.y + 3.0, h1.min_element() * 0.22);
    let top = |z: f32, s: f32| Section::new(z, s).shifted(c.x, c.y);
    b.loft_z(
        &cap,
        &[top(z1, 1.0), top(z1 + 4.0, 1.0), top(z1 + 5.0, 0.985)],
    );
    if b.fine() {
        light(b);
        let band = cut_rect(b, h1.x + 3.4, h1.y + 3.4, h1.min_element() * 0.22);
        b.loft_z(&band, &[top(z1 + 1.4, 1.0), top(z1 + 2.8, 1.0)]);
    }

    // Each face: the line of light up it, the ribs across it.
    let t = |z: f32| (z - z0) / (z1 - z0);
    for (ax, sg) in [(0usize, 1.0f32), (0, -1.0), (1, 1.0), (1, -1.0)] {
        let half_n = |z: f32| h0[ax] + (h1[ax] - h0[ax]) * t(z);
        let half_a = |z: f32| h0[1 - ax] + (h1[1 - ax] - h0[1 - ax]) * t(z);
        let pier_at = |z: f32| p0 + (p1 - p0) * t(z);
        let pt = |u: f32, z: f32| {
            let mut q = [0.0f32; 2];
            q[ax] = sg * half_n(z);
            q[1 - ax] = u;
            v3(c.x + q[0], c.y + q[1], z)
        };
        let n = {
            let mut q = [0.0f32; 2];
            q[ax] = sg * (z1 - z0);
            v3(q[0], q[1], h0[ax] - h1[ax]).normalize()
        };
        seam(
            b,
            pt(0.0, z0 + 6.0),
            pt(0.0, z1 - 6.0),
            n,
            (small * 0.07).clamp(1.2, 4.0),
        );
        if b.fine() {
            pale(b);
            let ribs = ((z1 - z0) / bay).floor() as usize;
            let step = (z1 - z0) / (ribs as f32 + 1.0);
            for i in 1..=ribs {
                let (za, zb) = (z0 + step * i as f32 - 2.2, z0 + step * i as f32 + 2.2);
                let w = |z: f32| half_a(z) - pier_at(z) * 2.0 + 1.0;
                let quad = [pt(-w(za), za), pt(w(za), za), pt(w(zb), zb), pt(-w(zb), zb)];
                panel(b, &quad, n, 1.3, 0.5);
            }
            // Mullions down the bays between, so the dark reads as windows.
            let mw = (small * 0.04).clamp(1.0, 2.2) * 0.5;
            for j in 1..ribs {
                let (za, zb) = (z0 + step * j as f32 + 2.2, z0 + step * (j + 1) as f32 - 2.2);
                let w = |z: f32| half_a(z) - pier_at(z) * 2.0 + 1.0;
                for f in [-0.62f32, -0.3, 0.3, 0.62] {
                    let quad = [
                        pt(f * w(za) - mw, za),
                        pt(f * w(za) + mw, za),
                        pt(f * w(zb) + mw, zb),
                        pt(f * w(zb) - mw, zb),
                    ];
                    panel(b, &quad, n, 0.7, 0.0);
                }
            }
        }
    }
}

/// A slab of light `min`..`max`, or at the coarse level nothing.
fn glow(b: &mut MeshBuilder, min: Vec3, max: Vec3) {
    if !b.coarse() {
        light(b);
        b.block(min, max);
    }
}

// ---- Gate ---------------------------------------------------------------------------
//
// The great land gate: the road runs along +x between two colossal legs 600 m apart.
// Each leg stands on a battered plinth ribbed with fins, and climbs in two tiers of
// dark shaft between pale corner piers, the upper hovering over the lower on a neck of
// light, buttress blades against its ends. On the legs, over lines of light, ride the
// lintel's end blocks; between them a centre span floats over a gap of light, all one
// girder with a dark windowed band split by ribs down both faces, coffered beneath.
// Horns sweep up and out from the end blocks to the crown at 640 m; over the lintel
// three stepped tiers hover one above the other; over those a keystone split by a
// blade of light floats with shards and blocks about it to 800 m.

/// Centre y of each leg, and its solid plan's half size (x, y).
pub(super) const GATE_LEG_Y: f32 = 300.0;
pub(super) const GATE_LEG_HALF: (f32, f32) = (70.0, 60.0);
/// Half the opening's width: clear for |y| under this.
pub(super) const GATE_OPENING: f32 = 240.0;
/// The lintel's underside: the opening is clear to here.
pub(super) const GATE_CLEAR: f32 = 380.0;
/// The crown (the horns' tips and the top tier), and the keystone cluster's top.
pub(super) const GATE_CROWN: f32 = 640.0;
pub(super) const GATE_TOP: f32 = 800.0;
// The legs' inner faces stand at the opening's edge, the end blocks overhang it
// only above the clear height.
const _: () = assert!(GATE_LEG_Y - GATE_LEG_HALF.1 >= GATE_OPENING && LINTEL.0 >= GATE_CLEAR);

const LINTEL: (f32, f32) = (384.0, 500.0);
const LINTEL_HW: f32 = 76.0;
/// The centre span's half length, and where the end blocks start and stop.
const SPAN_HALF: f32 = 198.0;
const END_BLOCK: (f32, f32) = (206.0, 408.0);

fn gate(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        gate_coarse(b);
        return;
    }
    b.mirror_y(gate_leg);
    gate_lintel(b);
    gate_crown(b);
    gate_keystone(b);
}

fn gate_coarse(b: &mut MeshBuilder) {
    pale(b);
    let (lx, ly) = GATE_LEG_HALF;
    for s in [-1.0f32, 1.0] {
        let y = s * GATE_LEG_Y;
        let ring = |z: f32, hx: f32, hy: f32, dy: f32| {
            vec![
                v3(hx, y + dy - hy, z),
                v3(hx, y + dy + hy, z),
                v3(-hx, y + dy + hy, z),
                v3(-hx, y + dy - hy, z),
            ]
        };
        b.loft(
            &[
                ring(-80.0, lx, ly, 0.0),
                ring(LINTEL.0, 44.0, 36.0, s * 4.0),
            ],
            false,
            true,
        );
    }
    let (y0, y1) = (END_BLOCK.1, LINTEL.1);
    b.block(v3(-LINTEL_HW, -y0, LINTEL.0), v3(LINTEL_HW, y0, y1));
    b.frustum_open(
        v3(0.0, 0.0, LINTEL.1),
        Vec2::new(128.0, 900.0),
        Vec2::new(80.0, 252.0),
        GATE_CROWN - LINTEL.1,
        Vec2::ZERO,
    );
    b.cuboid_open(v3(0.0, 0.0, 730.0), v3(60.0, 100.0, 140.0));
}

/// One leg, on the +y side.
fn gate_leg(b: &mut MeshBuilder) {
    let (lx, ly) = GATE_LEG_HALF;
    let cy = GATE_LEG_Y;
    let at = |z: f32, s: f32| Section::new(z, s).shifted(0.0, cy);

    // The plinth: fins stand out of it to the edge of the plan.
    pale(b);
    let plan = cut_rect(b, lx - 5.0, ly - 5.0, 16.0);
    b.loft_z(&plan, &[at(-80.0, 1.0), at(34.0, 1.0)]);
    dark(b);
    let course = cut_rect(b, lx - 8.0, ly - 8.0, 14.0);
    b.loft_z(&course, &[at(33.5, 1.0), at(50.5, 1.0)]);
    light(b);
    let ring = cut_rect(b, lx - 7.4, ly - 7.4, 14.0);
    b.loft_z(&ring, &[at(40.8, 1.0), at(43.2, 1.0)]);
    pale(b);
    let cap = cut_rect(b, lx - 2.5, ly - 2.5, 17.0);
    b.loft_z(&cap, &[at(50.0, 0.99), at(56.0, 1.0), at(60.0, 0.965)]);
    if b.fine() {
        light(b);
        let nosing = cut_rect(b, lx - 2.1, ly - 2.1, 17.0);
        b.loft_z(&nosing, &[at(53.0, 1.0), at(54.6, 1.0)]);
    }
    if b.fine() {
        // Fins: on the ends at three places, on the outer and inner faces at three.
        let fin = |b: &mut MeshBuilder, c: Vec3, size: Vec3, out: Vec3| {
            pale(b);
            b.chamfered_box(c, size, 1.2);
            let face = c + out * (out.abs().dot(size) * 0.5 + 0.02);
            seam(b, face - Vec3::Z * 22.0, face + Vec3::Z * 22.0, out, 1.8);
        };
        for x in [-1.0f32, 1.0] {
            for y in [cy - 36.0, cy, cy + 36.0] {
                fin(
                    b,
                    v3(x * (lx - 3.7), y, 7.0),
                    v3(7.0, 9.0, 54.0),
                    v3(x, 0.0, 0.0),
                );
            }
        }
        for (y, out) in [(cy + ly - 3.7, 1.0f32), (cy - ly + 3.7, -1.0)] {
            for x in [-42.0f32, 0.0, 42.0] {
                fin(b, v3(x, y, 7.0), v3(9.0, 7.0, 54.0), v3(0.0, out, 0.0));
            }
        }
        // Plates in two courses on the plinth's faces, between the fins.
        pale(b);
        for (za, zb) in [(-8.0f32, 12.0f32), (15.0, 31.0)] {
            for x in [-1.0f32, 1.0] {
                for (ya, yb) in [(cy - 31.0, cy - 5.5), (cy + 5.5, cy + 31.0)] {
                    let f = |y: f32, z: f32| v3(x * (lx - 5.0), y, z);
                    panel(
                        b,
                        &[f(ya, za), f(yb, za), f(yb, zb), f(ya, zb)],
                        v3(x, 0.0, 0.0),
                        1.0,
                        0.6,
                    );
                }
            }
            for y in [-1.0f32, 1.0] {
                for (xa, xb) in [(-37.5f32, -4.5f32), (4.5, 37.5)] {
                    let f = |x: f32, z: f32| v3(x, cy + y * (ly - 5.0), z);
                    panel(
                        b,
                        &[f(xa, za), f(xb, za), f(xb, zb), f(xa, zb)],
                        v3(0.0, y, 0.0),
                        1.0,
                        0.6,
                    );
                }
            }
        }
    }

    // The lower tier, the neck of light, the upper tier.
    stage(
        b,
        Vec2::new(0.0, cy + 2.0),
        Vec2::new(60.0, 52.0),
        Vec2::new(54.0, 46.0),
        59.5,
        228.0,
        24.0,
    );
    light(b);
    let neck = cut_rect(b, 40.0, 32.0, 9.0);
    b.loft_z(
        &neck,
        &[
            Section::new(232.5, 1.0).shifted(0.0, cy + 3.0),
            Section::new(243.5, 1.0).shifted(0.0, cy + 3.0),
        ],
    );
    stage(
        b,
        Vec2::new(0.0, cy + 4.0),
        Vec2::new(48.0, 40.0),
        Vec2::new(42.0, 34.0),
        243.0,
        366.0,
        22.0,
    );
    // The neck the end block rides on.
    light(b);
    let neck = cut_rect(b, 30.0, 24.0, 7.0);
    b.loft_z(
        &neck,
        &[
            Section::new(370.5, 1.0).shifted(0.0, cy + 4.0),
            Section::new(LINTEL.0 + 0.5, 1.0).shifted(0.0, cy + 4.0),
        ],
    );

    // Buttress blades against the ends of the lower tier, standing on the plinth.
    mirror_x(b, |b| {
        let blade: [[f32; 2]; 5] = [
            [52.0, 59.0],
            [67.0, 59.0],
            [67.0, 72.0],
            [58.0, 214.0],
            [52.0, 226.0],
        ];
        for y in [cy - 28.0, cy + 30.0] {
            b.with(Affine3A::from_translation(v3(0.0, y, 0.0)), |b| {
                pale(b);
                b.extrude_y_chamfered(&blade, 5.0, 1.4);
                if b.fine() {
                    for s in [-5.0f32, 5.0] {
                        seam(
                            b,
                            v3(63.0, s, 78.0),
                            v3(56.5, s, 200.0),
                            v3(0.0, s.signum(), 0.0),
                            1.4,
                        );
                    }
                }
            });
        }
    });

    // The wing: blocks hovering in a stack off the lower tier's outer face, each
    // over a line of light and stepping in as it climbs, under the end block.
    let wings: [(f32, f32, f32); 4] = [
        (74.0, 146.0, 424.0),
        (154.0, 222.0, 414.0),
        (230.0, 300.0, 404.0),
        (308.0, 372.0, 394.0),
    ];
    for (k, &(z0, z1, outer)) in wings.iter().enumerate() {
        let inner = 366.0;
        let hx = 46.0 - 3.0 * k as f32;
        pale(b);
        b.chamfered_box(
            v3(0.0, (inner + outer) * 0.5, (z0 + z1) * 0.5),
            v3(2.0 * hx, outer - inner, z1 - z0),
            8.0,
        );
        glow(
            b,
            v3(-hx + 12.0, inner + 6.0, z0 - 8.0),
            v3(hx - 12.0, outer - 8.0, z0 + 0.5),
        );
        if b.mid() {
            // A dark band across its outer face with light in it.
            dark(b);
            let (za, zb) = (z0 + 16.0, z1 - 16.0);
            let quad = [
                v3(-hx + 10.0, outer, za),
                v3(hx - 10.0, outer, za),
                v3(hx - 10.0, outer, zb),
                v3(-hx + 10.0, outer, zb),
            ];
            panel(b, &quad, Vec3::Y, 0.8, 0.0);
            seam(
                b,
                v3(-hx + 16.0, outer + 0.8, (za + zb) * 0.5),
                v3(hx - 16.0, outer + 0.8, (za + zb) * 0.5),
                Vec3::Y,
                2.6,
            );
        }
        if b.fine() {
            // Ribs across the band, light along the ends.
            pale(b);
            for x in [-hx * 0.5, 0.0, hx * 0.5] {
                b.chamfered_box(
                    v3(x, outer + 0.6, (z0 + z1) * 0.5),
                    v3(4.0, 2.4, z1 - z0 - 30.0),
                    0.6,
                );
            }
            for x in [-hx, hx] {
                seam(
                    b,
                    v3(x, inner + 10.0, z0 + 8.0),
                    v3(x, inner + 10.0, z1 - 8.0),
                    v3(x.signum(), 0.0, 0.0),
                    2.0,
                );
            }
        }
    }
    // Light down the gap between the wing and the leg.
    glow(b, v3(-3.0, 358.5, 80.0), v3(3.0, 360.0, 366.0));
}

fn gate_lintel(b: &mut MeshBuilder) {
    let (z0, z1) = LINTEL;
    let zc = (z0 + z1) * 0.5;
    let hd = (z1 - z0) * 0.5;
    let run = Run::new(v3(0.0, 0.0, zc), v3(0.0, 1.0, zc), Vec3::Z);
    // The centre span, in four segments, and the end blocks.
    girder(
        b, &run, -SPAN_HALF, SPAN_HALF, LINTEL_HW, hd, 18.0, 30.0, 99.0,
    );
    for s in [-1.0f32, 1.0] {
        let (a, c) = if s > 0.0 {
            (END_BLOCK.0, END_BLOCK.1)
        } else {
            (-END_BLOCK.1, -END_BLOCK.0)
        };
        girder(b, &run, a, c, LINTEL_HW, hd, 18.0, 30.0, 0.0);
    }
    b.mirror_y(|b| {
        // The gap of light between span and end block.
        glow(
            b,
            v3(-LINTEL_HW + 14.0, SPAN_HALF + 0.5, z0 + 18.0),
            v3(LINTEL_HW - 14.0, END_BLOCK.0 - 0.5, z1 - 18.0),
        );
        // The end block's outer face: a face of light in a pale frame.
        light(b);
        let y = END_BLOCK.1;
        let face = [
            v3(-LINTEL_HW + 16.0, y, z0 + 16.0),
            v3(LINTEL_HW - 16.0, y, z0 + 16.0),
            v3(LINTEL_HW - 16.0, y, z1 - 16.0),
            v3(-LINTEL_HW + 16.0, y, z1 - 16.0),
        ];
        film(b, &face, Vec3::Y, 0.1);
        if b.fine() {
            dark(b);
            let inner = [
                v3(-24.0, y, z0 + 34.0),
                v3(24.0, y, z0 + 34.0),
                v3(24.0, y, z1 - 34.0),
                v3(-24.0, y, z1 - 34.0),
            ];
            panel(b, &inner, Vec3::Y, 1.2, 0.6);
        }
        // The end block's inner face, over the opening: light down it.
        seam(
            b,
            v3(0.0, END_BLOCK.0, z0 + 12.0),
            v3(0.0, END_BLOCK.0, z1 - 12.0),
            -Vec3::Y,
            4.0,
        );
    });

    // Beneath: the lit bar hanging under the span, ribs across the soffit with coffers
    // of dark between them, lines of light along the coffers.
    glow(
        b,
        v3(-12.0, -SPAN_HALF + 12.0, GATE_CLEAR + 4.0),
        v3(12.0, SPAN_HALF - 12.0, GATE_CLEAR + 9.0),
    );
    if b.mid() {
        pale(b);
        let mut y = -END_BLOCK.1 + 22.0;
        let step = 2.0 * (END_BLOCK.1 - 22.0) / 18.0;
        for _ in 0..=18 {
            b.chamfered_box(
                v3(0.0, y, GATE_CLEAR + 2.6),
                v3(2.0 * LINTEL_HW - 12.0, 4.0, 5.0),
                1.0,
            );
            y += step;
        }
        if b.fine() {
            for x in [-44.0f32, 44.0] {
                seam(
                    b,
                    v3(x, -END_BLOCK.1 + 16.0, z0 - 0.02),
                    v3(x, END_BLOCK.1 - 16.0, z0 - 0.02),
                    -Vec3::Z,
                    3.0,
                );
            }
        }
    }

    // The horns, sweeping up and out from the end blocks: three girders stepping
    // down in section, each hovering off the last over a joint of light.
    b.mirror_y(|b| {
        let path = HORN.map(|(y, z)| v3(0.0, y, z));
        let sections = [
            (40.0f32, 30.0f32, 9.0f32),
            (34.0, 24.0, 7.0),
            (28.0, 17.0, 5.0),
        ];
        for i in 0..3 {
            let run = Run::new(path[i], path[i + 1], Vec3::Z);
            let (hw, hd, band) = sections[i];
            let t0 = if i == 0 { -36.0 } else { 4.0 };
            girder(b, &run, t0, run.len - 4.0, hw, hd, band, 22.0, 0.0);
            if i > 0 {
                // The joint: a block of light square to the bend.
                let before = Run::new(path[i - 1], path[i], Vec3::Z);
                let joint = Run::new(
                    path[i] - (before.dir + run.dir) * 5.0,
                    path[i] + (before.dir + run.dir) * 5.0,
                    Vec3::Z,
                );
                if !b.coarse() {
                    light(b);
                    let plan = cut_rect(b, hw - 6.0, hd - 5.0, 3.0);
                    joint.solid(b, 0.0, joint.len, &plan, 0.0);
                }
            }
            if i == 2 {
                // A face of light at the tip, live.
                key_light(b);
                let t = run.len - 4.0;
                let face = [
                    run.at(t, -hw + 5.0, -hd + 4.0),
                    run.at(t, hw - 5.0, -hd + 4.0),
                    run.at(t, hw - 5.0, hd - 4.0),
                    run.at(t, -hw + 5.0, hd - 4.0),
                ];
                film(b, &face, run.dir, 0.1);
                // A shard hovering past it, lit beneath.
                if b.mid() {
                    pale(b);
                    let shard = cut_rect(b, 16.0, 12.0, 3.0);
                    let tip = Run::new(
                        run.at(run.len + 10.0, 0.0, 6.0),
                        run.at(run.len + 60.0, 0.0, 16.0),
                        Vec3::Z,
                    );
                    tip.solid(b, 0.0, tip.len, &shard, 0.0);
                    if b.fine() {
                        seam(
                            b,
                            tip.at(4.0, 0.0, -12.1),
                            tip.at(tip.len - 4.0, 0.0, -12.1),
                            -tip.up,
                            3.0,
                        );
                    }
                }
            }
        }
    });
}

/// The horn's path over each end block, (y, z): it leaves the end block and climbs
/// out to the crown.
const HORN: [(f32, f32); 4] = [
    (350.0, 508.0),
    (416.0, 552.0),
    (470.0, 590.0),
    (512.0, 620.0),
];

/// Tiers over the lintel: (bottom, top, half length along y, half across x).
const CROWN_TIERS: [(f32, f32, f32, f32); 3] = [
    (504.5, 546.0, 332.0, 56.0),
    (554.0, 592.0, 226.0, 44.0),
    (600.0, GATE_CROWN - 2.0, 126.0, 34.0),
];

fn gate_crown(b: &mut MeshBuilder) {
    let mut below = LINTEL.1;
    for (i, &(z0, z1, hl, hw)) in CROWN_TIERS.iter().enumerate() {
        // The neck of light it hovers on.
        glow(
            b,
            v3(-hw + 12.0, -hl + 30.0, below - 0.5),
            v3(hw - 12.0, hl - 30.0, z0 + 0.5),
        );
        let zc = (z0 + z1) * 0.5;
        let run = Run::new(v3(0.0, 0.0, zc), v3(0.0, 1.0, zc), Vec3::Z);
        let segment = if i < 2 { hl * 2.0 / 5.0 } else { 0.0 };
        girder(b, &run, -hl, hl, hw, (z1 - z0) * 0.5, 6.5, 26.0, segment);
        // A line of light along the top, and faces of light at the ends.
        seam(
            b,
            v3(0.0, -hl + 10.0, z1),
            v3(0.0, hl - 10.0, z1),
            Vec3::Z,
            3.0 + i as f32,
        );
        if b.fine() {
            b.mirror_y(|b| {
                light(b);
                let face = [
                    v3(-hw + 10.0, hl, z0 + 8.0),
                    v3(hw - 10.0, hl, z0 + 8.0),
                    v3(hw - 10.0, hl, z1 - 8.0),
                    v3(-hw + 10.0, hl, z1 - 8.0),
                ];
                film(b, &face, Vec3::Y, 0.1);
            });
            // Light along both nosings of the tread.
            for x in [-1.0f32, 1.0] {
                seam(
                    b,
                    v3(x * (hw - 7.0), -hl + 14.0, z1),
                    v3(x * (hw - 7.0), hl - 14.0, z1),
                    Vec3::Z,
                    1.8,
                );
            }
        }
        below = z1;
    }
}

fn gate_keystone(b: &mut MeshBuilder) {
    // The bar of light it floats over.
    glow(b, v3(-20.0, -90.0, 645.0), v3(20.0, 90.0, 650.0));
    // The keystone: two halves hovering apart over a blade of light.
    b.mirror_y(|b| {
        let half: [[f32; 2]; 4] = [[4.0, 662.0], [78.0, 724.0], [78.0, 780.0], [4.0, 798.0]];
        b.with(across(), |b| {
            pale(b);
            b.extrude_y_chamfered(&half, 36.0, 3.5);
        });
        for x in [-36.0f32, 36.0] {
            let n = v3(x.signum(), 0.0, 0.0);
            dark(b);
            let band = [
                v3(x, 16.0, 690.0),
                v3(x, 66.0, 736.0),
                v3(x, 66.0, 770.0),
                v3(x, 16.0, 784.0),
            ];
            panel(b, &band, n, 0.6, 0.0);
            seam(
                b,
                v3(x + x.signum() * 0.6, 40.0, 718.0),
                v3(x + x.signum() * 0.6, 40.0, 774.0),
                n,
                3.0,
            );
            if b.fine() {
                pale(b);
                for z in [724.0f32, 752.0] {
                    b.chamfered_box(v3(x, 41.0, z), v3(2.4, 50.0, 4.0), 0.6);
                }
            }
        }
        // Shards leaning out either side, lit beneath.
        let shard: [[f32; 2]; 4] = [
            [96.0, 678.0],
            [140.0, 690.0],
            [170.0, 792.0],
            [126.0, 770.0],
        ];
        b.with(across(), |b| {
            pale(b);
            b.extrude_y_chamfered(&shard, 20.0, 2.5);
        });
        glow(b, v3(-16.0, 104.0, 666.0), v3(16.0, 142.0, 670.0));
        // Blocks further out, hovering over light.
        if b.mid() {
            pale(b);
            b.chamfered_box(v3(0.0, 204.0, 674.0), v3(34.0, 40.0, 26.0), 3.0);
            b.chamfered_box(v3(0.0, 262.0, 660.0), v3(26.0, 30.0, 16.0), 2.5);
            if b.fine() {
                light(b);
                b.block(v3(-11.0, 190.0, 652.0), v3(11.0, 218.0, 654.5));
                b.block(v3(-8.0, 252.0, 644.0), v3(8.0, 272.0, 646.0));
            }
        }
    });
    // The blade of light between the halves: the gate's one live light.
    if !b.coarse() {
        key_light(b);
        b.block(v3(-24.0, -3.2, 670.0), v3(24.0, 3.2, 794.0));
    }
}

// ---- Platform -----------------------------------------------------------------------
//
// A sea platform on four legs, ships passing under its deck between them. Each leg a
// footing to the seabed, a dark course with light in it at the water, and a shaft of
// dark core between pale piers flaring into a capital under the deck. The deck a
// girder slab: pale flanges over a dark band split by ribs with light along it,
// coffered and lit beneath. On it a stepped hall in three tiers hovering on necks of
// light, an open frame over the back legs with a keystone of light in it, pylons over
// the front legs, and a row of blocks hovering along each edge.

/// Leg centres (±x, ±y), and a leg's solid half size.
pub(super) const PLATFORM_LEG: (f32, f32) = (160.0, 90.0);
pub(super) const PLATFORM_LEG_HALF: f32 = 28.0;
/// The deck: its underside and top, and its half size.
pub(super) const PLATFORM_DECK: (f32, f32) = (110.0, 140.0);
pub(super) const PLATFORM_DECK_HALF: (f32, f32) = (220.0, 130.0);
/// Clear between the legs under this.
pub(super) const PLATFORM_CLEAR: f32 = 100.0;
pub(super) const PLATFORM_TOP: f32 = 260.0;

const LEG_CORNERS: [(f32, f32); 4] = [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)];

fn platform(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        platform_coarse(b);
        return;
    }
    for (sx, sy) in LEG_CORNERS {
        platform_leg(b, Vec2::new(sx * PLATFORM_LEG.0, sy * PLATFORM_LEG.1));
    }
    platform_deck(b);
    platform_works(b);
}

fn platform_coarse(b: &mut MeshBuilder) {
    pale(b);
    let h = PLATFORM_LEG_HALF;
    for (sx, sy) in LEG_CORNERS {
        let c = v3(sx * PLATFORM_LEG.0, sy * PLATFORM_LEG.1, 0.0);
        let ring = |z: f32, r: f32| {
            vec![
                c + v3(r, -r, z),
                c + v3(r, r, z),
                c + v3(-r, r, z),
                c + v3(-r, -r, z),
            ]
        };
        b.loft(
            &[ring(-80.0, h), ring(PLATFORM_DECK.0 + 1.0, h - 6.0)],
            false,
            false,
        );
    }
    let (dx, dy) = PLATFORM_DECK_HALF;
    b.block(v3(-dx, -dy, PLATFORM_DECK.0), v3(dx, dy, PLATFORM_DECK.1));
    b.frustum_open(
        v3(10.0, 0.0, PLATFORM_DECK.1),
        Vec2::new(200.0, 140.0),
        Vec2::new(80.0, 60.0),
        PLATFORM_TOP - PLATFORM_DECK.1,
        Vec2::ZERO,
    );
}

fn platform_leg(b: &mut MeshBuilder, c: Vec2) {
    let h = PLATFORM_LEG_HALF;
    let at = |z: f32, s: f32| Section::new(z, s).shifted(c.x, c.y);
    pale(b);
    let foot = cut_rect(b, h, h, 8.0);
    b.loft_z(&foot, &[at(-80.0, 1.0), at(-8.0, 1.0), at(-4.0, 0.97)]);
    // The course at the water, light in it.
    dark(b);
    let course = cut_rect(b, h - 2.5, h - 2.5, 7.0);
    b.loft_z(&course, &[at(-4.5, 1.0), at(8.5, 1.0)]);
    light(b);
    let ring = cut_rect(b, h - 2.0, h - 2.0, 7.0);
    b.loft_z(&ring, &[at(3.0, 1.0), at(5.0, 1.0)]);
    pale(b);
    let collar = cut_rect(b, h - 0.5, h - 0.5, 8.0);
    b.loft_z(&collar, &[at(8.0, 1.0), at(13.0, 1.0), at(15.0, 0.96)]);
    // The shaft.
    stage(b, c, Vec2::splat(25.0), Vec2::splat(21.0), 14.5, 92.0, 16.0);
    // The capital, flaring under the deck (over the clear height).
    pale(b);
    let cap = cut_rect(b, 24.0, 24.0, 7.0);
    b.loft_z(
        &cap,
        &[
            at(96.5, 1.0),
            at(PLATFORM_CLEAR + 2.0, 1.1),
            at(PLATFORM_DECK.0 + 1.0, 1.45),
        ],
    );
    glow(
        b,
        v3(c.x - 18.0, c.y - 18.0, 96.8),
        v3(c.x + 18.0, c.y + 18.0, 97.8),
    );
}

fn platform_deck(b: &mut MeshBuilder) {
    let (dx, dy) = PLATFORM_DECK_HALF;
    let (z0, z1) = PLATFORM_DECK;
    let ch = 26.0;
    dark(b);
    let core = cut_rect(b, dx - 5.0, dy - 5.0, ch);
    b.loft_z(
        &core,
        &[Section::new(z0 + 4.0, 1.0), Section::new(z1 - 4.0, 1.0)],
    );
    pale(b);
    let flange = cut_rect(b, dx, dy, ch + 2.0);
    let ring = |z: f32, d: f32| Section::scaled(z, (dx - d) / dx, (dy - d) / dy);
    b.loft_z(
        &flange,
        &[ring(z0, 2.0), ring(z0 + 2.0, 0.0), ring(z0 + 5.5, 0.0)],
    );
    b.loft_z(
        &flange,
        &[ring(z1 - 6.5, 0.0), ring(z1 - 1.5, 0.0), ring(z1, 1.2)],
    );
    // The line of light along the band.
    light(b);
    let band = cut_rect(b, dx - 4.4, dy - 4.4, ch);
    b.loft_z(&band, &[Section::new(123.4, 1.0), Section::new(125.6, 1.0)]);
    // Ribs across the band.
    if b.fine() {
        pale(b);
        for s in [-1.0f32, 1.0] {
            let mut x = -dx + 36.0;
            while x < dx - 30.0 {
                b.chamfered_box(v3(x, s * (dy - 3.0), 124.5), v3(4.0, 4.4, 19.0), 0.8);
                x += 24.0;
            }
            let mut y = -dy + 32.0;
            while y < dy - 30.0 {
                b.chamfered_box(v3(s * (dx - 3.0), y, 124.5), v3(4.4, 4.0, 19.0), 0.8);
                y += 24.0;
            }
        }
    }
    // Lines of light round the deck, in from its edge.
    let e = 9.0;
    let z = z1;
    let corners = [
        v3(dx - e - 12.0, -dy + e, z),
        v3(dx - e, -dy + e + 12.0, z),
        v3(dx - e, dy - e - 12.0, z),
        v3(dx - e - 12.0, dy - e, z),
    ];
    for sx in [1.0f32, -1.0] {
        let m = |p: Vec3| v3(p.x * sx, p.y, p.z);
        seam(b, m(corners[0]), m(corners[1]), Vec3::Z, 2.2);
        seam(
            b,
            m(corners[1] + v3(0.0, 1.0, 0.0)),
            m(corners[2] - v3(0.0, 1.0, 0.0)),
            Vec3::Z,
            2.2,
        );
        seam(b, m(corners[2]), m(corners[3]), Vec3::Z, 2.2);
    }
    for sy in [1.0f32, -1.0] {
        seam(
            b,
            v3(-dx + e + 13.0, sy * (dy - e), z),
            v3(dx - e - 13.0, sy * (dy - e), z),
            Vec3::Z,
            2.2,
        );
    }
    if b.fine() {
        // Dark joints between the deck's plates.
        dark(b);
        let mut x = -dx + 40.0;
        while x < dx - 20.0 {
            film(
                b,
                &[
                    v3(x - 0.7, -dy + 18.0, z),
                    v3(x + 0.7, -dy + 18.0, z),
                    v3(x + 0.7, dy - 18.0, z),
                    v3(x - 0.7, dy - 18.0, z),
                ],
                Vec3::Z,
                0.04,
            );
            x += 36.0;
        }
        for y in [-66.0f32, -22.0, 22.0, 66.0] {
            film(
                b,
                &[
                    v3(-dx + 18.0, y - 0.7, z),
                    v3(dx - 18.0, y - 0.7, z),
                    v3(dx - 18.0, y + 0.7, z),
                    v3(-dx + 18.0, y + 0.7, z),
                ],
                Vec3::Z,
                0.04,
            );
        }
        // Dark plates laid in the deck either side of the hall, a line of light down each.
        for sy in [1.0f32, -1.0] {
            {
                let (x0, x1) = (-128.0f32, -72.0f32);
                dark(b);
                let y0 = sy * 40.0;
                let y1 = sy * 100.0;
                let quad = [v3(x0, y0, z), v3(x1, y0, z), v3(x1, y1, z), v3(x0, y1, z)];
                panel(b, &quad, Vec3::Z, 0.5, 0.0);
                seam(
                    b,
                    v3(x0 + 6.0, sy * 70.0, z + 0.5),
                    v3(x1 - 6.0, sy * 70.0, z + 0.5),
                    Vec3::Z,
                    1.8,
                );
            }
        }
    }
    // Beneath: a dark soffit with lines of light, and ribs across it.
    if b.mid() {
        dark(b);
        let under = [
            v3(-dx + 12.0, -dy + 12.0, z0),
            v3(dx - 12.0, -dy + 12.0, z0),
            v3(dx - 12.0, dy - 12.0, z0),
            v3(-dx + 12.0, dy - 12.0, z0),
        ];
        film(b, &under, -Vec3::Z, 0.05);
        for y in [-40.0f32, 0.0, 40.0] {
            seam(
                b,
                v3(-dx + 30.0, y, z0 - 0.1),
                v3(dx - 30.0, y, z0 - 0.1),
                -Vec3::Z,
                2.6,
            );
        }
        if b.fine() {
            pale(b);
            let mut x = -dx + 40.0;
            while x < dx - 30.0 {
                if (x.abs() - PLATFORM_LEG.0).abs() > 34.0 {
                    b.chamfered_box(v3(x, 0.0, z0 - 2.0), v3(4.0, 2.0 * dy - 40.0, 4.2), 0.8);
                }
                x += 30.0;
            }
        }
    }
}

fn platform_works(b: &mut MeshBuilder) {
    let deck = PLATFORM_DECK.1;
    // The hall: a plinth, then three tiers each hovering over the last on light.
    let hc = Vec2::new(10.0, 0.0);
    pale(b);
    let plinth = cut_rect(b, 112.0, 78.0, 22.0);
    let at = |z: f32, s: f32| Section::new(z, s).shifted(hc.x, hc.y);
    b.loft_z(
        &plinth,
        &[
            at(deck - 1.0, 1.0),
            at(deck + 7.0, 1.0),
            at(deck + 10.0, 0.975),
        ],
    );
    if b.fine() {
        light(b);
        let nosing = cut_rect(b, 112.4, 78.4, 22.0);
        b.loft_z(&nosing, &[at(deck + 3.0, 1.0), at(deck + 4.6, 1.0)]);
    }
    stage(
        b,
        hc,
        Vec2::new(96.0, 64.0),
        Vec2::new(86.0, 56.0),
        deck + 9.5,
        194.0,
        14.0,
    );
    light(b);
    let neck = cut_rect(b, 60.0, 38.0, 12.0);
    b.loft_z(&neck, &[at(198.5, 1.0), at(205.5, 1.0)]);
    stage(
        b,
        hc,
        Vec2::new(64.0, 44.0),
        Vec2::new(56.0, 38.0),
        205.0,
        232.0,
        12.0,
    );
    light(b);
    let neck = cut_rect(b, 32.0, 22.0, 7.0);
    b.loft_z(&neck, &[at(236.5, 1.0), at(242.5, 1.0)]);
    pale(b);
    let top = cut_rect(b, 38.0, 28.0, 9.0);
    b.loft_z(
        &top,
        &[at(242.0, 1.0), at(252.0, 1.0), at(PLATFORM_TOP - 0.5, 0.7)],
    );
    seam(
        b,
        v3(hc.x - 20.0, 0.0, PLATFORM_TOP - 0.5),
        v3(hc.x + 20.0, 0.0, PLATFORM_TOP - 0.5),
        Vec3::Z,
        4.0,
    );

    // The frame over the back legs: two uprights, a lintel, a keystone of light in it.
    let fx = -PLATFORM_LEG.0;
    let fy = PLATFORM_LEG.1;
    for s in [-1.0f32, 1.0] {
        let c = Vec2::new(fx, s * fy);
        pale(b);
        let foot = cut_rect(b, 24.0, 21.0, 6.0);
        b.loft_z(
            &foot,
            &[
                Section::new(deck - 1.0, 1.0).shifted(c.x, c.y),
                Section::new(deck + 6.0, 1.0).shifted(c.x, c.y),
                Section::new(deck + 8.0, 0.95).shifted(c.x, c.y),
            ],
        );
        stage(
            b,
            c,
            Vec2::new(20.0, 17.0),
            Vec2::new(17.0, 14.0),
            deck + 7.5,
            231.0,
            15.0,
        );
    }
    let lintel = Run::new(v3(fx, 0.0, 248.0), v3(fx, 1.0, 248.0), Vec3::Z);
    girder(
        b,
        &lintel,
        -fy - 30.0,
        fy + 30.0,
        24.0,
        12.0,
        4.0,
        22.0,
        0.0,
    );
    light(b);
    b.chamfered_box(v3(fx, 0.0, 196.0), v3(12.0, 34.0, 24.0), 3.0);
    if b.mid() {
        pale(b);
        for s in [-1.0f32, 1.0] {
            b.chamfered_box(v3(fx, s * 28.0, 196.0), v3(18.0, 12.0, 40.0), 3.0);
        }
    }

    // Pylons over the front legs, a point hovering over each on light.
    for s in [-1.0f32, 1.0] {
        let c = v3(PLATFORM_LEG.0, s * PLATFORM_LEG.1, 0.0);
        let at = |z: f32, k: f32| Section::new(z, k).shifted(c.x, c.y);
        pale(b);
        let base = cut_rect(b, 20.0, 20.0, 6.0);
        b.loft_z(
            &base,
            &[
                at(deck - 1.0, 1.0),
                at(deck + 8.0, 1.0),
                at(deck + 10.0, 0.9),
            ],
        );
        dark(b);
        let shaft = cut_rect(b, 13.0, 13.0, 4.0);
        b.loft_z(&shaft, &[at(deck + 9.0, 1.0), at(214.0, 0.62)]);
        if b.mid() {
            pale(b);
            let pier = fine_rect(b, 3.2, 3.2, 1.0);
            for (px, py) in LEG_CORNERS {
                b.loft_z(
                    &pier,
                    &[
                        Section::new(deck + 9.0, 1.0).shifted(c.x + px * 11.4, c.y + py * 11.4),
                        Section::new(214.0, 0.62).shifted(c.x + px * 7.1, c.y + py * 7.1),
                    ],
                );
            }
            let out = |ax: f32, ay: f32| {
                v3(
                    ax * (214.0 - deck - 9.0),
                    ay * (214.0 - deck - 9.0),
                    13.0 - 8.06,
                )
                .normalize()
            };
            for (ax, ay) in [(1.0f32, 0.0f32), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                let p = |z: f32, r: f32| c + v3(ax * r, ay * r, z);
                seam(b, p(deck + 14.0, 12.8), p(206.0, 8.4), out(ax, ay), 1.8);
            }
        }
        let point = cut_rect(b, 9.0, 9.0, 3.0);
        key_light(b);
        b.loft_z(&point, &[at(214.5, 0.6), at(221.0, 1.0)]);
        pale(b);
        b.loft_z(&point, &[at(221.0, 1.0), at(226.0, 1.0), at(248.0, 0.0)]);
    }

    // A ring hovering over the fore deck: eight segments, light under each and a lens
    // of light in its eye.
    let rc = v3(170.0, 0.0, deck);
    let (r0, r1) = (28.0f32, 46.0f32);
    for k in 0..8 {
        let a = k as f32 / 8.0 * std::f32::consts::TAU;
        b.with(
            Affine3A::from_translation(rc) * Affine3A::from_rotation_z(a),
            |b| {
                let half = (std::f32::consts::PI / 8.0 - 0.035) * r0;
                pale(b);
                b.chamfered_box(
                    v3((r0 + r1) * 0.5, 0.0, 16.0),
                    v3(r1 - r0, half * 2.0, 7.0),
                    2.0,
                );
                if b.fine() {
                    light(b);
                    let under = [
                        v3(r0 + 3.0, -half * 0.8, 0.0),
                        v3(r1 - 3.0, -half * 0.8, 0.0),
                        v3(r1 - 3.0, half * 0.8, 0.0),
                        v3(r0 + 3.0, half * 0.8, 0.0),
                    ];
                    film(b, &under, Vec3::Z, 0.06);
                }
            },
        );
    }
    if b.mid() {
        key_light(b);
        let lens = cut_rect(b, 16.0, 16.0, 6.0);
        b.loft_z(
            &lens,
            &[
                Section::new(deck - 0.5, 1.0).shifted(rc.x, rc.y),
                Section::new(deck + 1.5, 1.0).shifted(rc.x, rc.y),
                Section::new(deck + 3.0, 0.6).shifted(rc.x, rc.y),
            ],
        );
    }

    // Blocks hovering in a row along each long edge, light under each.
    if b.fine() {
        let (_, dy) = PLATFORM_DECK_HALF;
        for s in [-1.0f32, 1.0] {
            let mut x = -104.0;
            while x < 140.0 {
                pale(b);
                b.chamfered_box(v3(x, s * (dy - 14.0), deck + 12.0), v3(26.0, 9.0, 9.0), 1.6);
                light(b);
                let under = [
                    v3(x - 10.0, s * (dy - 17.0), deck),
                    v3(x + 10.0, s * (dy - 17.0), deck),
                    v3(x + 10.0, s * (dy - 11.0), deck),
                    v3(x - 10.0, s * (dy - 11.0), deck),
                ];
                film(b, &under, Vec3::Z, 0.06);
                x += 40.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{build_model, MeshLod};
    use super::*;

    fn position(mesh: &MeshLod, i: u32) -> Vec3 {
        Vec3::from(mesh.vertices[i as usize].pos)
    }

    /// Points spread over every triangle of `mesh`: its corners and an interior grid.
    fn samples(mesh: &MeshLod) -> Vec<Vec3> {
        let mut out = Vec::new();
        for t in mesh.indices.chunks(3) {
            let [a, b, c] = [
                position(mesh, t[0]),
                position(mesh, t[1]),
                position(mesh, t[2]),
            ];
            let n = 6;
            for i in 0..=n {
                for j in 0..=n - i {
                    let (u, v) = (i as f32 / n as f32, j as f32 / n as f32);
                    out.push(a + (b - a) * u + (c - a) * v);
                }
            }
        }
        out
    }

    fn in_rect(p: Vec3, (cx, cy, hx, hy): (f32, f32, f32, f32)) -> bool {
        (p.x - cx).abs() <= hx + 0.05 && (p.y - cy).abs() <= hy + 0.05
    }

    #[test]
    fn gate_and_platform_build_within_budget() {
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
            println!(
                "{}: triangles {tris:?}, z {low:.0}..{high:.0}, bounds {:.0}",
                def.key, model.bounds_radius
            );
            assert!(tris[0] <= TRIANGLES, "{}: {} triangles", def.key, tris[0]);
            assert!(tris[1] < tris[0], "{}: LOD1 not lighter", def.key);
            assert!(
                tris[1] as f32 <= tris[0] as f32 * 0.45 + 20.0,
                "{}: LOD1 {} of {}",
                def.key,
                tris[1],
                tris[0]
            );
            assert!(tris[2] < 60, "{}: coarse {}", def.key, tris[2]);
            assert!(low >= -80.0, "{}: below the footing", def.key);
            assert!(
                high <= def.nominal[0].1 + 1e-3,
                "{}: over its height",
                def.key
            );
            for lod in &model.lods {
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
                for t in lod.indices.chunks(3) {
                    let [a, b, c] = [
                        position(lod, t[0]),
                        position(lod, t[1]),
                        position(lod, t[2]),
                    ];
                    let n = (b - a).cross(c - a);
                    assert!(
                        n.length() * 0.5 > 1e-7,
                        "{}: degenerate triangle at {a}",
                        def.key
                    );
                    for &i in t {
                        let shading = Vec3::from(lod.vertices[i as usize].normal);
                        assert!(
                            n.normalize().dot(shading) > 0.5,
                            "{}: winding at {a}",
                            def.key
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn gate_opening_is_clear_and_legs_stand_in_their_plans() {
        let model = build_model("precursor_gate").unwrap();
        let (lx, ly) = GATE_LEG_HALF;
        let legs = [(0.0, GATE_LEG_Y, lx, ly), (0.0, -GATE_LEG_Y, lx, ly)];
        for (lod, mesh) in model.lods.iter().enumerate() {
            for p in samples(mesh) {
                assert!(
                    !(p.y.abs() < GATE_OPENING - 0.05 && p.z < GATE_CLEAR - 0.05),
                    "lod{lod}: in the opening at {p}"
                );
                assert!(
                    p.z >= 60.0 || legs.iter().any(|&r| in_rect(p, r)),
                    "lod{lod}: outside the legs at {p}"
                );
            }
        }
    }

    #[test]
    fn platform_is_clear_under_the_deck() {
        let model = build_model("precursor_platform").unwrap();
        let h = PLATFORM_LEG_HALF;
        let (x, y) = PLATFORM_LEG;
        let legs = [(x, y, h, h), (-x, y, h, h), (-x, -y, h, h), (x, -y, h, h)];
        for (lod, mesh) in model.lods.iter().enumerate() {
            for p in samples(mesh) {
                assert!(
                    p.z >= PLATFORM_CLEAR - 0.05 || legs.iter().any(|&r| in_rect(p, r)),
                    "lod{lod}: under the deck at {p}"
                );
            }
        }
    }
}

/// Software previews, whole and cropped close: `GATE_DUMP_DIR=... cargo test -p mc-render
/// --lib gate_previews -- --ignored`.
#[cfg(test)]
#[test]
#[ignore]
fn gate_previews() {
    use super::MeshLod;
    let dir = std::path::PathBuf::from(std::env::var_os("GATE_DUMP_DIR").expect("GATE_DUMP_DIR"));
    std::fs::create_dir_all(&dir).unwrap();
    // The triangles of `mesh` whose middle is inside `min`..`max`.
    let crop = |mesh: &MeshLod, min: Vec3, max: Vec3| {
        let mut out = MeshLod::default();
        for t in mesh.indices.chunks(3) {
            let mid = t
                .iter()
                .map(|&i| Vec3::from(mesh.vertices[i as usize].pos))
                .sum::<Vec3>()
                / 3.0;
            if mid.cmpge(min).all() && mid.cmple(max).all() {
                for &i in t {
                    out.indices.push(out.vertices.len() as u32);
                    out.vertices.push(mesh.vertices[i as usize]);
                }
            }
        }
        out
    };
    for def in MODELS {
        let model = super::build_model(def.key).unwrap();
        for (lod, yaw) in [
            (0usize, -38.0f32),
            (0, 0.0),
            (0, 142.0),
            (1, -38.0),
            (2, -38.0),
        ] {
            super::preview::render(&model.lods[lod], 768, yaw)
                .write_ppm(&dir.join(format!("{}_{lod}_{}.ppm", def.key, yaw as i32)))
                .unwrap();
        }
        let close: &[(&str, Vec3, Vec3)] = if def.key == "precursor_gate" {
            &[
                ("leg", v3(-200.0, 150.0, -100.0), v3(200.0, 500.0, 390.0)),
                ("crown", v3(-200.0, -100.0, 470.0), v3(200.0, 700.0, 900.0)),
            ]
        } else {
            &[
                ("leg", v3(80.0, 20.0, -100.0), v3(260.0, 200.0, 106.0)),
                ("works", v3(-260.0, -200.0, 139.0), v3(260.0, 200.0, 300.0)),
            ]
        };
        for (name, min, max) in close {
            let part = crop(&model.lods[0], *min, *max);
            super::preview::render(&part, 768, -38.0)
                .write_ppm(&dir.join(format!("{}_close_{name}.ppm", def.key)))
                .unwrap();
        }
    }
}

// ---- Floor ----------------------------------------------------------------------------
//
// The facility's paving: a 400 m square of it, lying flush on level ground and walked
// over (nothing solid). Pale alloy showing in thin seams between sixteen dark plates,
// a low boss on each, and lines of light down the middle both ways, so the plateau
// carries a grid of light every 400 m and the pale machine stands out on a dark floor.
// The map lays them edge to edge.

/// Half the side of one length of floor, and the height of its plates' faces: under the
/// cradles' and forges' floor plates (0.3 m), which lie over it.
pub(super) const FLOOR_HALF: f32 = 200.0;
pub(super) const FLOOR_TOP: f32 = 0.25;

fn floor(b: &mut MeshBuilder, _tech: u8) {
    let h = FLOOR_HALF;
    if b.coarse() {
        dark(b);
        b.chamfered_box(v3(0.0, 0.0, -1.12), v3(2.0 * h, 2.0 * h, 2.6), 0.5);
        return;
    }
    if !b.fine() {
        // Past a few hundred metres the seams are under a pixel: plain plate and its light.
        dark(b);
        b.chamfered_box(v3(0.0, 0.0, -1.12), v3(2.0 * h, 2.0 * h, 2.6), 0.5);
        light(b);
        b.chamfered_box(v3(0.0, 0.0, -0.1), v3(2.0 * h - 2.0, 0.9, 0.7), 0.2);
        b.chamfered_box(v3(0.0, 0.0, -0.1), v3(0.9, 2.0 * h - 2.0, 0.7), 0.2);
        return;
    }
    // The seams: dark alloy too, only a little lighter than the plates, so the floor
    // reads as worn metal and not a drawn grid.
    dark(b);
    b.chamfered_box(v3(0.0, 0.0, -1.22), v3(2.0 * h, 2.0 * h, 2.6), 0.5);
    let cell = h / 2.0;
    for i in 0..4 {
        for j in 0..4 {
            let c = v3(
                -h + cell * (i as f32 + 0.5),
                -h + cell * (j as f32 + 0.5),
                0.0,
            );
            dark(b);
            b.chamfered_box(
                c + v3(0.0, 0.0, -0.52),
                v3(cell - 3.0, cell - 3.0, 1.4),
                2.0,
            );
            // A boss raised on the plate, its bevel catching the light.
            b.chamfered_box(
                c + v3(0.0, 0.0, 0.2),
                v3(cell - 30.0, cell - 30.0, 0.08),
                6.0,
            );
        }
    }
    light(b);
    b.chamfered_box(v3(0.0, 0.0, -0.1), v3(2.0 * h - 2.0, 0.9, 0.7), 0.2);
    b.chamfered_box(v3(0.0, 0.0, -0.1), v3(0.9, 2.0 * h - 2.0, 0.7), 0.2);
}

// ---- Viaduct and pier -----------------------------------------------------------------
//
// What ties the facility into one machine: elevated decks running from building to
// building on piers, the way the booms and spans tie the megastructure. A viaduct is
// 200 m of deck along +x from its origin (the map lays them end to end, a pier under
// every joint it can take one); a pier is the column under a joint with a hub on top
// that the decks run into. Deck 48-64 m up: over the units, under the cradles' arms.

pub(super) const VIADUCT_LEN: f32 = 200.0;
pub(super) const VIADUCT_DECK: (f32, f32) = (48.0, 64.0);
pub(super) const VIADUCT_TOP: f32 = 72.0;
pub(super) const PIER_TOP: f32 = 84.0;

fn viaduct(b: &mut MeshBuilder, _tech: u8) {
    let (z0, z1) = VIADUCT_DECK;
    let mid = (z0 + z1) * 0.5;
    let run = Run::new(v3(0.0, 0.0, mid), v3(VIADUCT_LEN, 0.0, mid), Vec3::Z);
    if b.coarse() {
        pale(b);
        b.chamfered_box(
            v3(VIADUCT_LEN * 0.5, 0.0, mid),
            v3(VIADUCT_LEN, 30.0, z1 - z0),
            1.0,
        );
        return;
    }
    // The deck: a girder, its flanks' dark band ribbed and lit.
    girder(
        b,
        &run,
        0.0,
        VIADUCT_LEN,
        15.0,
        (z1 - z0) * 0.5,
        3.0,
        25.0,
        50.0,
    );
    // A line of light along the soffit.
    seam(
        b,
        v3(4.0, 0.0, z0 - 0.3),
        v3(VIADUCT_LEN - 4.0, 0.0, z0 - 0.3),
        -Vec3::Z,
        3.0,
    );
    // Parapets: pale rails in lengths hovering over a line of light down each edge.
    for side in [-1.0f32, 1.0] {
        light(b);
        b.chamfered_box(
            v3(VIADUCT_LEN * 0.5, side * 12.5, z1 + 0.6),
            v3(VIADUCT_LEN - 4.0, 1.2, 1.2),
            0.3,
        );
        if b.fine() {
            pale(b);
            for k in 0..8 {
                let x = 12.5 + 25.0 * k as f32;
                b.chamfered_box(v3(x, side * 12.5, z1 + 4.5), v3(21.0, 3.0, 4.0), 0.8);
            }
        }
    }
    if b.fine() {
        // A spine of plates down the walk, lit between.
        dark(b);
        b.chamfered_box(
            v3(VIADUCT_LEN * 0.5, 0.0, z1 + 0.25),
            v3(VIADUCT_LEN - 6.0, 10.0, 0.5),
            0.5,
        );
        light(b);
        b.chamfered_box(
            v3(VIADUCT_LEN * 0.5, 0.0, z1 + 0.55),
            v3(VIADUCT_LEN - 10.0, 0.8, 0.3),
            0.1,
        );
    }
}

fn pier(b: &mut MeshBuilder, _tech: u8) {
    let (z0, z1) = VIADUCT_DECK;
    if b.coarse() {
        pale(b);
        b.chamfered_box(
            v3(0.0, 0.0, (z1 + 4.0 - 80.0) * 0.5),
            v3(32.0, 32.0, z1 + 84.0),
            4.0,
        );
        return;
    }
    // The column: a battered pale shaft, dark core showing down each face with light in it.
    pale(b);
    let foot = cut_rect(b, 16.0, 16.0, 4.0);
    b.loft_z(
        &foot,
        &[
            Section::new(-80.0, 1.0),
            Section::new(6.0, 1.0),
            Section::new(z0 - 6.0, 0.8),
        ],
    );
    dark(b);
    b.chamfered_box(
        v3(0.0, 0.0, (z0 + 8.0) * 0.5),
        v3(28.0, 6.0, z0 - 12.0),
        1.0,
    );
    b.chamfered_box(
        v3(0.0, 0.0, (z0 + 8.0) * 0.5),
        v3(6.0, 28.0, z0 - 12.0),
        1.0,
    );
    for (dir, side) in [
        (Vec3::X, Vec3::Y),
        (-Vec3::X, Vec3::Y),
        (Vec3::Y, Vec3::X),
        (-Vec3::Y, Vec3::X),
    ] {
        let out = dir * 14.1;
        if b.fine() {
            seam(
                b,
                out + v3(0.0, 0.0, 10.0),
                out + v3(0.0, 0.0, z0 - 10.0),
                dir,
                1.4,
            );
            // Fins either side of the slot.
            pale(b);
            for s in [-1.0f32, 1.0] {
                b.chamfered_box(
                    dir * 13.0 + side * s * 5.5 + v3(0.0, 0.0, 20.0),
                    (dir.abs() * 4.0 + side.abs() * 2.0) + v3(0.0, 0.0, 26.0),
                    0.5,
                );
            }
        }
    }
    // The hub the decks run into, a lit collar under it, a cap hovering over light.
    light(b);
    b.chamfered_box(v3(0.0, 0.0, z0 - 4.0), v3(30.0, 30.0, 2.0), 4.0);
    pale(b);
    b.chamfered_box(
        v3(0.0, 0.0, (z0 - 3.0 + z1 + 6.0) * 0.5),
        v3(40.0, 40.0, z1 + 9.0 - z0),
        6.0,
    );
    if b.fine() {
        dark(b);
        b.chamfered_box(v3(0.0, 0.0, (z0 + z1) * 0.5), v3(41.0, 41.0, 5.0), 6.0);
    }
    light(b);
    b.chamfered_box(v3(0.0, 0.0, z1 + 7.0), v3(16.0, 16.0, 1.6), 3.0);
    pale(b);
    b.chamfered_box(v3(0.0, 0.0, z1 + 13.5), v3(22.0, 22.0, 8.0), 3.0);
    if b.fine() {
        b.chamfered_box(v3(0.0, 0.0, PIER_TOP - 2.0), v3(10.0, 10.0, 4.0), 1.5);
        light(b);
        b.chamfered_box(v3(0.0, 0.0, z1 + 18.2), v3(6.0, 6.0, 1.4), 1.0);
    }
}
