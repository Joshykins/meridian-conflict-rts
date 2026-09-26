//! The Axis's Precursor kit (`mc_map::PropKind::Precursor{Citadel,Seaway}`, laid by
//! `mc-map/src/bake/archipelago.rs`): the citadel at the heart of the archipelago,
//! and the causeways that run along the seabed from it to every cay, so the whole
//! map reads as one installation drowned among the islands.
//!
//! The citadel is not a needle on a round base: two blades of unequal height lean
//! together off a stepped, off-centre plinth, a sloped buttress climbing the back of
//! the main blade and a lower glacis under its front, landing pads on the lowest
//! tier. Both blades end in chisel cuts sloping opposite ways; over the main one a
//! stair of slabs hovers on lit undersides, the one part that visibly works.
//!
//! Same kit and language as the megastructure (`precursor_mega.rs`): pale alloy over a
//! dark core, dormant channels of dark glass (`light`), live light only where it works
//! (`key_light`). Model space as for every prop: x along the heading, y left, z up,
//! the origin on the ground. Footings go to -80 m.

use glam::Vec3;

use super::builder::{MeshBuilder, Section};
use super::library::ModelDef;
use super::precursor::{
    cut_rect, dark, fine_rect, key_light, key_seam, light, pale, panel, seam, v3,
};

pub(super) const MODELS: &[ModelDef] = &[
    ModelDef::new("precursor_citadel", 460.0, CITADEL_TOP, citadel),
    ModelDef::new("precursor_seaway", 104.0, SEAWAY_TOP, seaway),
];

/// Full-detail triangle budget per model (held by the tests).
#[cfg(test)]
pub(super) const TRIANGLES: usize = 40_000;

// ---- Citadel ------------------------------------------------------------------------

pub(super) const CITADEL_TOP: f32 = 1_640.0;
/// The plinth's tiers: half extents, x shift, top. The lowest is the solid plan
/// (`mc-map` format.rs `solid_plan`).
const TIERS: [(f32, f32, f32, f32); 3] = [
    (330.0, 280.0, 0.0, 40.0),
    (262.0, 214.0, -24.0, 92.0),
    (196.0, 160.0, -40.0, 134.0),
];
const BASE: f32 = 134.0;

/// A blade: its plan at the base and at the top (half x, half y, centre x, centre y),
/// the top's height at its middle and the chisel's slope along x.
struct Blade {
    base: (f32, f32, f32, f32),
    top: (f32, f32, f32, f32),
    z: f32,
    chisel: f32,
}

const MAIN: Blade = Blade {
    base: (124.0, 88.0, -8.0, -34.0),
    top: (46.0, 32.0, 6.0, -44.0),
    z: 1_340.0,
    chisel: 1.1,
};
const SECOND: Blade = Blade {
    base: (74.0, 36.0, 24.0, 92.0),
    top: (26.0, 14.0, 16.0, 30.0),
    z: 1_560.0,
    chisel: -1.6,
};

impl Blade {
    /// The blade's plan at height `z` (ring points), chamfered at the corners.
    fn ring(&self, b: &MeshBuilder, t: f32, z_of: impl Fn(f32) -> f32) -> Vec<Vec3> {
        let lerp = |a: f32, c: f32| a + (c - a) * t;
        let (hx, hy, cx, cy) = (
            lerp(self.base.0, self.top.0),
            lerp(self.base.1, self.top.1),
            lerp(self.base.2, self.top.2),
            lerp(self.base.3, self.top.3),
        );
        cut_rect(b, hx, hy, hx.min(hy) * 0.22)
            .iter()
            .map(|p| v3(cx + p[0], cy + p[1], z_of(p[0])))
            .collect()
    }

    /// Where a face lies at `t` up the blade: its centre and half extents.
    fn at(&self, t: f32) -> (f32, f32, f32, f32) {
        let lerp = |a: f32, c: f32| a + (c - a) * t;
        (
            lerp(self.base.2, self.top.2),
            lerp(self.base.3, self.top.3),
            lerp(self.base.0, self.top.0),
            lerp(self.base.1, self.top.1),
        )
    }

    fn build(&self, b: &mut MeshBuilder) {
        // The core: dark between pale piers at full detail, pale when far.
        let rise = self.z - BASE;
        let rings: Vec<Vec<Vec3>> = [0.0f32, 0.5, 1.0]
            .iter()
            .map(|&t| {
                let z = BASE + rise * t;
                if t < 1.0 {
                    self.ring(b, t, |_| z)
                } else {
                    let chisel = self.chisel;
                    self.ring(b, t, move |x| z + x * chisel)
                }
            })
            .collect();
        let mut rings = rings;
        rings.insert(0, self.ring(b, 0.0, |_| BASE - 30.0));
        if b.coarse() {
            pale(b);
        } else {
            dark(b);
        }
        b.loft(&rings, false, true);
        if b.coarse() {
            return;
        }
        // Pale cladding down the long faces, broken by dark bands every 110 m, a
        // dormant channel down the middle of each; pale corner piers.
        pale(b);
        let steps = ((rise - 60.0) / 110.0).floor() as usize;
        for k in 0..steps {
            let (t0, t1) = (
                (k as f32 * 110.0 + 14.0) / rise,
                (k as f32 * 110.0 + 96.0) / rise,
            );
            let (z0, z1) = (BASE + rise * t0, BASE + rise * t1);
            for side in [-1.0f32, 1.0] {
                let (c0x, c0y, h0x, h0y) = self.at(t0);
                let (c1x, c1y, h1x, h1y) = self.at(t1);
                // The ±y faces (the blade's broad sides).
                let out = v3(0.0, side, 0.0);
                let quad = [
                    v3(c0x - h0x * 0.8, c0y + side * h0y, z0),
                    v3(c0x + h0x * 0.8, c0y + side * h0y, z0),
                    v3(c1x + h1x * 0.8, c1y + side * h1y, z1),
                    v3(c1x - h1x * 0.8, c1y + side * h1y, z1),
                ];
                panel(b, &quad, out, 1.8, 0.6);
                if b.fine() {
                    // A row of dark slits across the plate.
                    dark(b);
                    let n = ((h0x * 1.6) / 22.0).floor().max(2.0) as usize;
                    for i in 0..n {
                        let u = (i as f32 + 0.5) / n as f32 * 1.4 - 0.7;
                        let (za, zb) = (z0 + (z1 - z0) * 0.58, z0 + (z1 - z0) * 0.8);
                        let ya = c0y + side * (h0y + 1.8) + (c1y - c0y + side * (h1y - h0y)) * 0.58;
                        let yb = c0y + side * (h0y + 1.8) + (c1y - c0y + side * (h1y - h0y)) * 0.8;
                        let (xa, xb) = (c0x + (c1x - c0x) * 0.58, c0x + (c1x - c0x) * 0.8);
                        let (wa, wb) = (h0x + (h1x - h0x) * 0.58, h0x + (h1x - h0x) * 0.8);
                        let slit = [
                            v3(xa + u * wa - 4.0, ya, za),
                            v3(xa + u * wa + 4.0, ya, za),
                            v3(xb + u * wb + 4.0, yb, zb),
                            v3(xb + u * wb - 4.0, yb, zb),
                        ];
                        panel(b, &slit, out, 0.35, 0.0);
                    }
                    pale(b);
                }
                // The ±x faces (its narrow ends).
                let out = v3(side, 0.0, 0.0);
                let quad = [
                    v3(c0x + side * h0x, c0y - h0y * 0.7, z0),
                    v3(c0x + side * h0x, c0y + h0y * 0.7, z0),
                    v3(c1x + side * h1x, c1y + h1y * 0.7, z1),
                    v3(c1x + side * h1x, c1y - h1y * 0.7, z1),
                ];
                panel(b, &quad, out, 1.6, 0.5);
            }
        }
        for side in [-1.0f32, 1.0] {
            let (c0x, c0y, _, h0y) = self.at(0.02);
            let (c1x, c1y, _, h1y) = self.at(0.94);
            let (z0, z1) = (BASE + rise * 0.02, BASE + rise * 0.94);
            seam(
                b,
                v3(c0x, c0y + side * (h0y + 1.9), z0),
                v3(c1x, c1y + side * (h1y + 1.9), z1),
                v3(0.0, side, 0.0),
                (h0y * 0.07).max(3.0),
            );
        }
        if b.mid() {
            // Ribs up the broad faces, following the taper.
            pale(b);
            let rib = fine_rect(b, 2.6, 2.2, 0.8);
            for side in [-1.0f32, 1.0] {
                for u in [-0.5f32, 0.0, 0.5] {
                    let (c0x, c0y, h0x, h0y) = self.at(0.0);
                    let (c1x, c1y, h1x, h1y) = self.at(0.95);
                    b.loft_z(
                        &rib,
                        &[
                            Section::new(BASE, 1.0)
                                .shifted(c0x + u * h0x, c0y + side * (h0y + 3.2)),
                            Section::new(BASE + rise * 0.95, 1.0)
                                .shifted(c1x + u * h1x, c1y + side * (h1y + 3.2)),
                        ],
                    );
                }
            }
        }
        if b.fine() {
            pale(b);
            let pier = fine_rect(b, 9.0, 9.0, 2.5);
            for (sx, sy) in [(1.0f32, 1.0f32), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
                let (c0x, c0y, h0x, h0y) = self.at(0.0);
                let (c1x, c1y, h1x, h1y) = self.at(0.97);
                b.loft_z(
                    &pier,
                    &[
                        Section::new(BASE - 2.0, 1.0)
                            .shifted(c0x + sx * (h0x - 12.0), c0y + sy * (h0y - 12.0)),
                        Section::new(BASE + rise * 0.97, 0.7)
                            .shifted(c1x + sx * (h1x - 8.0), c1y + sy * (h1y - 8.0)),
                    ],
                );
            }
        }
    }
}

fn citadel(b: &mut MeshBuilder, _tech: u8) {
    // The plinth: three tiers, each set back toward the back, battered a little.
    let mut below = -80.0;
    for (i, &(hx, hy, sx, top)) in TIERS.iter().enumerate() {
        pale(b);
        let plan = cut_rect(b, hx, hy, 38.0 - 8.0 * i as f32);
        b.loft_z(
            &plan,
            &[
                Section::new(below, 1.0).shifted(sx, 0.0),
                Section::new(top, 0.975).shifted(sx, 0.0),
            ],
        );
        if !b.coarse() {
            // A dark band round each tier's face, a dormant channel along its nosing.
            dark(b);
            let band = cut_rect(b, hx * 0.99 + 1.2, hy * 0.99 + 1.2, 38.0 - 8.0 * i as f32);
            let (z0, z1) = (
                top - (top - below.max(0.0)) * 0.62,
                top - (top - below.max(0.0)) * 0.3,
            );
            b.loft_z(
                &band,
                &[
                    Section::new(z0, 1.0).shifted(sx, 0.0),
                    Section::new(z1, 0.995).shifted(sx, 0.0),
                ],
            );
            light(b);
            let lip = cut_rect(b, hx * 0.975 - 3.0, hy * 0.975 - 3.0, 36.0 - 8.0 * i as f32);
            b.loft_z(
                &lip,
                &[
                    Section::new(top, 1.0).shifted(sx, 0.0),
                    Section::new(top + 0.4, 1.0).shifted(sx, 0.0),
                ],
            );
        }
        below = top - 2.0;
    }

    // Stairs cut up the front of the plinth: pale treads stepping up to the glacis.
    if b.mid() {
        pale(b);
        for k in 0..6 {
            let x = TIERS[0].0 - 18.0 - k as f32 * 20.0;
            let z = 8.0 + k as f32 * 20.0;
            if z > BASE {
                break;
            }
            b.chamfered_box(v3(x, 0.0, z * 0.5), v3(22.0, 80.0 - k as f32 * 6.0, z), 1.5);
        }
    }

    // Fins round the lowest tier's walls, every 44 m.
    if b.mid() {
        pale(b);
        let (hx, hy, sx, top) = TIERS[0];
        let mut u = -hx + 60.0;
        while u < hx - 50.0 {
            for side in [-1.0f32, 1.0] {
                b.chamfered_box(
                    v3(sx + u, side * (hy + 2.0), top * 0.5 - 2.0),
                    v3(7.0, 8.0, top + 2.0),
                    1.2,
                );
            }
            u += 44.0;
        }
        let mut u = -hy + 60.0;
        while u < hy - 50.0 {
            b.chamfered_box(
                v3(sx - hx - 2.0, u, top * 0.5 - 2.0),
                v3(8.0, 7.0, top + 2.0),
                1.2,
            );
            u += 44.0;
        }
    }

    // Halls on the upper tiers round the blades' feet: low blocks set unevenly,
    // each with a dark band and a dormant channel along its roof.
    for &(x, y, hx, hy, h) in &[
        (140.0f32, 150.0f32, 46.0f32, 34.0f32, 58.0f32),
        (-40.0, 170.0, 70.0, 26.0, 40.0),
        (-190.0, -190.0, 60.0, 40.0, 70.0),
        (40.0, -178.0, 38.0, 30.0, 34.0),
        (-250.0, 120.0, 30.0, 50.0, 52.0),
    ] {
        let z0 = if x.abs() < TIERS[1].0 - 20.0 && y.abs() < TIERS[1].1 - 20.0 {
            TIERS[1].3
        } else {
            TIERS[0].3
        };
        pale(b);
        let plan = cut_rect(b, hx, hy, 8.0);
        b.loft_z(
            &plan,
            &[
                Section::new(z0 - 4.0, 1.0).shifted(x, y),
                Section::new(z0 + h, 0.92).shifted(x, y),
            ],
        );
        if !b.coarse() {
            dark(b);
            let band = cut_rect(b, hx + 1.0, hy + 1.0, 8.0);
            b.loft_z(
                &band,
                &[
                    Section::new(z0 + h * 0.45, 0.985).shifted(x, y),
                    Section::new(z0 + h * 0.7, 0.97).shifted(x, y),
                ],
            );
            seam(
                b,
                v3(x - hx * 0.7, y, z0 + h + 0.2),
                v3(x + hx * 0.7, y, z0 + h + 0.2),
                Vec3::Z,
                2.4,
            );
        }
    }

    MAIN.build(b);
    SECOND.build(b);

    // The buttress climbing the back of the main blade: a sloped mass stepped in
    // courses, dark bands between them.
    pale(b);
    let butt = cut_rect(b, 1.0, 1.0, 0.18);
    let course = |z: f32| -> Section {
        let t = ((z - BASE) / (780.0 - BASE)).clamp(0.0, 1.0);
        let (hx, hy, cx) = (
            150.0 + (22.0 - 150.0) * t,
            150.0 + (66.0 - 150.0) * t,
            -206.0 + (-118.0 + 206.0) * t,
        );
        Section::scaled(z, hx, hy).shifted(cx, -30.0)
    };
    b.loft_z(
        &butt,
        &[
            Section {
                z: BASE - 20.0,
                ..course(BASE)
            },
            course(BASE),
            course(780.0),
        ],
    );
    if !b.coarse() {
        dark(b);
        let mut z = BASE + 60.0;
        while z < 740.0 {
            let (lo, hi) = (course(z), course(z + 9.0));
            let grow = |s: Section| Section {
                scale: s.scale + glam::Vec2::splat(1.5),
                ..s
            };
            b.loft_z(&butt, &[grow(lo), grow(hi)]);
            z += 80.0;
        }
        // A dormant channel up the buttress's back.
        let (a, c) = (course(BASE + 10.0), course(760.0));
        let back = |s: &Section| v3(s.shift.x - s.scale.x - 0.3, s.shift.y, s.z);
        let out = (back(&a) - back(&c)).cross(Vec3::Y).normalize();
        seam(b, back(&a), back(&c), -out, 6.0);
    }

    // The glacis under the main blade's front: lower, steeper.
    pale(b);
    let glacis = |z: f32| -> Section {
        let t = ((z - BASE) / (360.0 - BASE)).clamp(0.0, 1.0);
        let (hx, hy, cx) = (
            96.0 + (12.0 - 96.0) * t,
            118.0 + (70.0 - 118.0) * t,
            178.0 + (118.0 - 178.0) * t,
        );
        Section::scaled(z, hx, hy).shifted(cx, -30.0)
    };
    b.loft_z(
        &butt,
        &[
            Section {
                z: BASE - 20.0,
                ..glacis(BASE)
            },
            glacis(BASE),
            glacis(360.0),
        ],
    );
    if !b.coarse() {
        // The portal: a tall dark opening in the glacis's front, lit dormant round it.
        let front = |z: f32| {
            let g = glacis(z);
            g.shift.x + g.scale.x
        };
        let (z0, z1) = (BASE + 2.0, BASE + 150.0);
        let slope = v3(front(z1) - front(z0), 0.0, z1 - z0).normalize();
        let out = v3(slope.z, 0.0, -slope.x);
        dark(b);
        let quad = [
            v3(front(z0), -64.0, z0),
            v3(front(z0), 4.0, z0),
            v3(front(z1), -14.0, z1),
            v3(front(z1), -46.0, z1),
        ];
        panel(b, &quad, out, 0.4, 0.0);
        for (a, c) in [(quad[0], quad[3]), (quad[1], quad[2])] {
            seam(b, a + out * 0.5, c + out * 0.5, out, 3.0);
        }
    }

    // Landing pads on the lowest tier's front corners, their edges lit dormant.
    for side in [-1.0f32, 1.0] {
        let at = v3(TIERS[0].0 - 96.0, side * (TIERS[0].1 - 90.0), TIERS[0].3);
        pale(b);
        b.chamfered_box(at + v3(0.0, 0.0, 3.0), v3(128.0, 128.0, 6.0), 18.0);
        if !b.coarse() {
            dark(b);
            b.chamfered_box(at + v3(0.0, 0.0, 6.2), v3(96.0, 96.0, 0.6), 14.0);
            light(b);
            for (a, c) in [
                (v3(-40.0, -40.0, 6.8), v3(40.0, -40.0, 6.8)),
                (v3(40.0, -40.0, 6.8), v3(40.0, 40.0, 6.8)),
                (v3(40.0, 40.0, 6.8), v3(-40.0, 40.0, 6.8)),
                (v3(-40.0, 40.0, 6.8), v3(-40.0, -40.0, 6.8)),
            ] {
                seam(b, at + a, at + c, Vec3::Z, 2.2);
            }
        }
    }

    // Over the main blade's chisel, a stair of slabs hovering on lit undersides.
    let (cx, cy, _, _) = MAIN.at(1.0);
    for k in 0..4 {
        let x = cx - 30.0 + k as f32 * 24.0;
        let z = MAIN.z + (x - 0.0) * MAIN.chisel + 46.0 + k as f32 * 28.0;
        let size = v3(30.0, 58.0 - k as f32 * 8.0, 9.0);
        pale(b);
        b.chamfered_box(v3(x, cy, z), size, 2.0);
        if !b.coarse() {
            key_light(b);
            b.chamfered_box(v3(x, cy, z - 5.5), v3(size.x * 0.6, size.y * 0.7, 1.4), 0.4);
        }
    }
    // The lens between the blades, where they lean closest: the one burning seam.
    if !b.coarse() {
        let (ax, ay, _, ahy) = MAIN.at(0.8);
        let (bx, by, _, bhy) = SECOND.at(0.8);
        let z = BASE + (MAIN.z - BASE) * 0.8;
        let (y0, y1) = (ay + ahy, by - bhy);
        key_seam(
            b,
            v3((ax + bx) * 0.5, y0 + 0.5, z - 160.0),
            v3((ax + bx) * 0.5, (y0 + y1) * 0.5, z + 40.0),
            v3(1.0, 0.0, 0.0),
            5.0,
        );
    }
}

// ---- Seaway -------------------------------------------------------------------------
//
// A 200 m length of causeway lying on the seabed, centred on its origin along x: a
// broad low deck on a footing, a dark channel down its crown with a line of live light
// in it (what reads through the water from the strategic camera), fins down both
// flanks. Laid end to end they run from island to island; the map scales each to the
// water over it, so on a bank it lies flat and in a channel it stands tall.

pub(super) const SEAWAY_TOP: f32 = 14.0;
const SEAWAY_HALF: f32 = 98.0;
const SEAWAY_HW: f32 = 30.0;

fn seaway(b: &mut MeshBuilder, _tech: u8) {
    pale(b);
    let deck = cut_rect(b, SEAWAY_HALF, SEAWAY_HW, 5.0);
    b.loft_z(
        &deck,
        &[
            Section::new(-8.0, 1.12),
            Section::new(0.0, 1.1),
            Section::new(4.0, 1.0),
            Section::new(10.0, 0.9),
        ],
    );
    if !b.coarse() {
        dark(b);
        let channel = [
            v3(-SEAWAY_HALF + 8.0, -12.0, 10.02),
            v3(SEAWAY_HALF - 8.0, -12.0, 10.02),
            v3(SEAWAY_HALF - 8.0, 12.0, 10.02),
            v3(-SEAWAY_HALF + 8.0, 12.0, 10.02),
        ];
        panel(b, &channel, Vec3::Z, 0.2, 0.0);
        key_seam(
            b,
            v3(-SEAWAY_HALF + 10.0, 0.0, 10.3),
            v3(SEAWAY_HALF - 10.0, 0.0, 10.3),
            Vec3::Z,
            6.0,
        );
        for y in [-9.0f32, 9.0] {
            seam(
                b,
                v3(-SEAWAY_HALF + 10.0, y, 10.3),
                v3(SEAWAY_HALF - 10.0, y, 10.3),
                Vec3::Z,
                1.6,
            );
        }
    }
    if b.mid() {
        pale(b);
        let mut x = -SEAWAY_HALF + 20.0;
        while x < SEAWAY_HALF - 10.0 {
            for side in [-1.0f32, 1.0] {
                b.chamfered_box(
                    v3(x, side * (SEAWAY_HW + 2.0), 6.0),
                    v3(8.0, 6.0, SEAWAY_TOP - 2.0),
                    1.2,
                );
            }
            x += 40.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::build_model;
    use super::*;

    #[test]
    fn citadel_kit_builds_within_budget() {
        for def in MODELS {
            let model = build_model(def.key).unwrap();
            let tris: Vec<usize> = model.lods.iter().map(|l| l.indices.len() / 3).collect();
            let high = model.lods[0]
                .vertices
                .iter()
                .map(|v| v.pos[2])
                .fold(f32::MIN, f32::max);
            let low = model.lods[0]
                .vertices
                .iter()
                .map(|v| v.pos[2])
                .fold(f32::MAX, f32::min);
            println!(
                "{}: triangles {tris:?}, z {low:.0}..{high:.0}, bounds {:.0}",
                def.key, model.bounds_radius
            );
            assert!(tris[0] <= TRIANGLES, "{}: {} triangles", def.key, tris[0]);
            assert!(tris.last() < tris.first(), "{}: LODs not lighter", def.key);
            assert!(low >= -80.5, "{}: below the footing", def.key);
            for lod in &model.lods {
                for v in &lod.vertices {
                    assert!(
                        v.pos.iter().chain(&v.normal).all(|c| c.is_finite()),
                        "{}",
                        def.key
                    );
                }
            }
        }
    }
}

/// Software previews: `CITADEL_DUMP_DIR=... cargo test -p mc-render --lib citadel_previews -- --ignored`.
#[cfg(test)]
#[test]
#[ignore]
fn citadel_previews() {
    let dir =
        std::path::PathBuf::from(std::env::var_os("CITADEL_DUMP_DIR").expect("CITADEL_DUMP_DIR"));
    std::fs::create_dir_all(&dir).unwrap();
    for def in MODELS {
        let model = super::build_model(def.key).unwrap();
        for yaw in [-38.0f32, 142.0] {
            super::preview::render(&model.lods[0], 768, yaw)
                .write_ppm(&dir.join(format!("{}_{}.ppm", def.key, yaw as i32)))
                .unwrap();
        }
    }
}
