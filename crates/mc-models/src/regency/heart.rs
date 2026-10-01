//! The Regency power generator: a star core held in a gyroscope of gravity
//! rings. Three tiers, each its own structure (built new, not upgraded), each held a
//! different way so the three read apart from above and from the ground (docs/STYLE.md,
//! "The Regency suite"):
//!
//! - Tech 1, the cradle (2 x 2 lot): a low plated drum with the star over a bronze cup,
//!   three armoured talons curling up round it and over, two rings.
//! - Tech 2, the yoke (4 x 4): two plated towers on one long footing, the star hung between
//!   them on bronze trunnions, three rings, the first turning on the trunnions' line.
//! - Tech 3, the crown (8 x 8): six great talons arching in from planted feet to a heavy
//!   plated crown overhead, open so the star shows from above, tied by a plated belt and
//!   braced by buttresses; the star hung in the middle over a bronze collector, three
//!   rings and a toothed drive ring under it.
//!
//! The star itself is light, not a solid: the renderer draws it where the model says it is
//! (`Model::star_core`, renderer `star_core_fx.rs`), a ball of fusing plasma boiling and
//! crackling in the prism's pinks, while the plant runs. The mesh carries only a small
//! white-hot heart under it (`material::GLOW_PRISM`), for the portraits and far off.
//! Each ring tumbles about an axis of its own (`MeshBuilder::with_orbit`) round the star.
//! Nothing on it can go off: a breached cage lets the star fall in on itself and go out
//! (no Regency power generator has a death blast).

use std::f32::consts::{PI, TAU};

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;

use super::kit::{dark_plate, metal, seam, shell, v3};
use super::machine::*;

/// The star at `at`, `radius` across its face: recorded for the renderer to draw as light
/// (`Model::star_core`), the gyroscope's pivot, and a small white-hot heart in the mesh.
fn core(b: &mut MeshBuilder, at: Vec3, radius: f32) {
    b.set_spinner_pivot(at);
    b.set_star_core(at, radius);
    b.paint(GLOW_PRISM);
    let r = radius * 0.2;
    if b.coarse() {
        b.cylinder_between(at - Vec3::Z * r, at + Vec3::Z * r, r, r * 0.6, 3);
        return;
    }
    let sides = b.sides(10);
    b.spheroid(at, Vec3::splat(r), sides, if b.fine() { 6 } else { 4 });
}

/// A gravity ring round the core: a bronze band of radius `r` lying square to `normal`,
/// tumbling about `spin` (best a line across the ring, so it sweeps a shell round the
/// core) at `rate` rad/s, with `weights` plated blocks set round it.
#[derive(Clone, Copy)]
struct Ring {
    normal: Vec3,
    spin: Vec3,
    rate: f32,
    r: f32,
    band: f32,
    weights: usize,
}

fn ring(b: &mut MeshBuilder, at: Vec3, g: &Ring) {
    let Ring {
        normal,
        spin,
        rate,
        r,
        band,
        weights,
    } = *g;
    let fine = b.fine();
    let segs = if fine { 32 } else { 12 };
    let normal = normal.normalize();
    b.with_orbit(spin, rate, |b| {
        metal(b);
        hoop_on(b, at, normal, r, band, band * 0.7, segs);
        if weights == 0 {
            return;
        }
        let e1 = normal.any_orthonormal_vector();
        let e2 = normal.cross(e1);
        dark_plate(b);
        for k in 0..weights {
            let a = std::f32::consts::TAU * (k as f32 + 0.5) / weights as f32;
            let d = e1 * a.cos() + e2 * a.sin();
            let c = at + d * r;
            let along = normal.cross(d);
            b.beam(
                c - along * band * 1.1,
                c + along * band * 1.1,
                glam::Vec2::new(band * 1.5, band * 1.3),
                glam::Vec2::new(band * 1.5, band * 1.3),
            );
        }
    });
}

/// A level ring about z through `at` with teeth on its rim, turning about z: the cage's
/// drive ring (teeth only on a ring that turns, per the Regency kit).
fn drive_ring(b: &mut MeshBuilder, at: Vec3, r: f32, band: f32, rate: f32) {
    let fine = b.fine();
    b.with_orbit(Vec3::Z, rate, |b| {
        metal(b);
        hoop_on(
            b,
            at,
            Vec3::Z,
            r,
            band,
            band * 0.8,
            if fine { 40 } else { 14 },
        );
        if fine {
            teeth(
                b,
                at,
                r + band * 0.5,
                36,
                v3(band * 0.5, band * 0.35, band * 0.6),
            );
        }
    });
}

/// The direction at bearing `a` (radians) round z.
fn out(a: f32) -> Vec3 {
    v3(a.cos(), a.sin(), 0.0)
}

/// A talon standing out along +x, turned into place by the caller: an arched armour
/// plate following `path` (points in the x-z plane with their half widths), its arch
/// turned outward, and a bronze spine up its inside.
fn talon(b: &mut MeshBuilder, path: &[(f32, f32, f32)], thick: f32) {
    let fine = b.fine();
    let pts: Vec<(Vec3, f32, f32)> = path
        .iter()
        .map(|&(x, z, w)| (v3(x, 0.0, z), w, thick * (0.5 + w / path[0].2 * 0.5)))
        .collect();
    dark_plate(b);
    shell(b, &pts, Vec3::X);
    if !fine {
        return;
    }
    // The bronze spine on its inside from the first bend up, a hand's breadth in from the plate.
    let inside: Vec<Vec3> = path
        .windows(2)
        .skip(1)
        .flat_map(|w| {
            let (a, c) = (v3(w[0].0, 0.0, w[0].1), v3(w[1].0, 0.0, w[1].1));
            let d = (c - a).normalize();
            let n = v3(-d.z, 0.0, d.x);
            [a + n * thick * 0.9, c + n * thick * 0.9]
        })
        .collect();
    // Square-cut, not turned: round bronze reads as a threaded rod.
    let w = path[0].2 * 0.3;
    metal(b);
    for seg in inside.chunks(2) {
        b.beam(seg[0], seg[1], Vec2::splat(w), Vec2::splat(w * 0.8));
    }
}

/// The owner's colour on a flat top: a square `half` across at `at`.
fn team_tab(b: &mut MeshBuilder, at: Vec3, along: Vec3, half: f32) {
    let along = along.normalize() * half;
    let across = Vec3::Z.cross(along);
    b.paint(TEAM);
    b.face(&[
        at - along - across,
        at + along - across,
        at + along + across,
        at - along + across,
    ]);
}

// ---- tech 1: the cradle -------------------------------------------------------------

const T1_CORE: Vec3 = Vec3::new(0.0, 0.0, 4.0);

pub(super) fn heart(b: &mut MeshBuilder, _tech: u8) {
    let fine = b.fine();
    let sides = b.sides(10);
    // The drum: a low plated plinth, a dark seam, a lip.
    dark_plate(b);
    b.prism(Vec3::ZERO, sides, 3.9, 3.4, 1.0);
    if b.coarse() {
        coarse_t1(b);
        return;
    }
    seam(b);
    b.prism(v3(0.0, 0.0, 1.0), sides, 3.1, 3.1, 0.25);
    dark_plate(b);
    b.prism(v3(0.0, 0.0, 1.25), sides, 3.2, 2.6, 0.45);
    // The bronze cup the core sits over, lit from inside.
    metal(b);
    b.cylinder_between(v3(0.0, 0.0, 1.7), v3(0.0, 0.0, 2.6), 0.9, 1.6, sides);
    core(b, T1_CORE, 1.35);
    for (k, &(normal, spin, rate, r)) in [
        (v3(0.0, 0.7, 1.0), Vec3::X, 0.9, 1.95),
        (v3(0.7, 0.0, 1.0), v3(0.0, 1.0, 0.4), -1.3, 2.45),
    ]
    .iter()
    .enumerate()
    {
        ring(
            b,
            T1_CORE,
            &Ring {
                normal,
                spin,
                rate,
                r,
                band: 0.26,
                weights: if k == 0 { 2 } else { 4 },
            },
        );
    }
    // Three talons curling up round the core and over it, feet on the lot.
    for k in 0..3 {
        let a = TAU * (k as f32 + 0.5) / 3.0;
        b.yawed(Vec3::ZERO, a, |b| {
            talon(
                b,
                &[
                    (7.6, 0.5, 1.5),
                    (5.0, 1.8, 1.4),
                    (3.9, 4.0, 1.1),
                    (3.3, 6.2, 0.8),
                    (2.0, 7.3, 0.5),
                ],
                0.9,
            );
            // The talon's tip carries the owner's colour.
            team_tab(b, v3(2.6, 0.0, 7.55), Vec3::X, 0.45);
            if fine {
                red_slot(b, v3(3.25, 0.0, 1.0), Vec3::X, Vec3::Y, 1.2, 0.16);
            }
        });
    }
}

/// Far off: a fin for each talon with the owner's colour on its tip, and the core.
fn coarse_t1(b: &mut MeshBuilder) {
    for k in 0..3 {
        let d = out(TAU * (k as f32 + 0.5) / 3.0);
        let side = Vec3::Z.cross(d);
        dark_plate(b);
        b.face(&[d * 7.8, d * 3.0 + v3(0.0, 0.0, 7.4), d * 3.0 + side]);
        b.face(&[d * 3.0 - side, d * 3.0 + v3(0.0, 0.0, 7.4), d * 7.8]);
        team_tab(b, d * 2.4 + v3(0.0, 0.0, 7.45), d, 0.5);
    }
    core(b, T1_CORE, 1.35);
}

// ---- tech 2: the yoke ---------------------------------------------------------------

const T2_CORE: Vec3 = Vec3::new(0.0, 0.0, 10.5);
/// Where each tower's inner face stands, along x.
const T2_ARM: f32 = 7.6;

pub(super) fn heart_2(b: &mut MeshBuilder, _tech: u8) {
    let fine = b.fine();
    // The footing: a long plated sill under both towers, a seam, a deck.
    dark_plate(b);
    b.frustum(
        Vec3::ZERO,
        Vec2::new(30.0, 13.0),
        Vec2::new(27.0, 10.5),
        2.2,
        Vec2::ZERO,
    );
    if b.coarse() {
        for a in [0.0, PI] {
            b.yawed(Vec3::ZERO, a, |b| {
                dark_plate(b);
                b.frustum(
                    v3(10.5, 0.0, 2.2),
                    Vec2::new(6.0, 8.0),
                    Vec2::new(4.0, 5.0),
                    15.0,
                    Vec2::new(-1.0, 0.0),
                );
                team_tab(b, v3(9.5, 0.0, 17.25), Vec3::X, 1.2);
            });
        }
        b.mirror_y(|b| {
            dark_plate(b);
            b.face(&[v3(-3.0, 4.0, 2.2), v3(0.0, 15.0, 0.3), v3(3.0, 4.0, 2.2)]);
        });
        core(b, T2_CORE, 3.1);
        return;
    }
    seam(b);
    b.frustum(
        v3(0.0, 0.0, 2.2),
        Vec2::new(26.0, 9.6),
        Vec2::new(26.0, 9.6),
        0.3,
        Vec2::ZERO,
    );
    // The collector under the core: a bronze dish on a drum.
    let sides = b.sides(16);
    metal(b);
    b.cylinder_between(v3(0.0, 0.0, 2.4), v3(0.0, 0.0, 3.6), 2.0, 3.4, sides);
    seam(b);
    b.cylinder_between(v3(0.0, 0.0, 3.6), v3(0.0, 0.0, 3.8), 3.4, 3.0, sides);
    core(b, T2_CORE, 3.1);
    // The first ring turns on the trunnions' line; the others tumble their own ways.
    let rings = [
        (Vec3::Y, Vec3::X, 0.55, 4.2, 4),
        (v3(1.0, 0.0, 0.6), v3(0.0, 1.0, 0.0), -0.85, 5.2, 2),
        (v3(0.0, 0.6, 1.0), v3(1.0, 1.0, 0.0), 1.15, 6.1, 6),
    ];
    for (normal, spin, rate, r, weights) in rings {
        ring(
            b,
            T2_CORE,
            &Ring {
                normal,
                spin,
                rate,
                r,
                band: 0.55,
                weights,
            },
        );
    }
    for a in [0.0, PI] {
        b.yawed(Vec3::ZERO, a, |b| tower(b, fine));
    }
    // Across the yoke, plated wings lapped out from the sill's flanks along the lot.
    b.mirror_y(|b| {
        let f = Frame::new(v3(0.0, 4.6, 2.1), v3(0.0, 1.0, -0.22), Vec3::Z);
        dark_plate(b);
        Course {
            count: 3,
            step: 2.6,
            len: 3.6,
            half: 3.2,
            tip: 0.0,
            thick: 0.55,
            tail: 3.0,
        }
        .lay(b, &f);
        if fine {
            red_slot(b, v3(0.0, 6.0, 2.62), Vec3::Z, Vec3::X, 2.6, 0.22);
        }
    });
}

/// One of the yoke's towers, at +x (mirrored for the other): a plated pier leaning in
/// over the core, its head's plates swept out and back into horns, the trunnion and
/// pinch emitter on its inner face, the lit slots and the owner's colour.
fn tower(b: &mut MeshBuilder, fine: bool) {
    let x = T2_ARM;
    dark_plate(b);
    b.frustum(
        v3(x + 3.2, 0.0, 2.5),
        Vec2::new(6.4, 9.0),
        Vec2::new(4.2, 5.6),
        14.0,
        Vec2::new(-0.9, 0.0),
    );
    // The trunnion: a bronze drum through the inner face on the core's line, and the
    // pinch emitter from it at the core, its tip lit red.
    collar(b, v3(x + 0.2, 0.0, T2_CORE.z), Vec3::X, 1.6, 1.4);
    metal(b);
    b.cylinder_between(
        v3(x - 0.5, 0.0, T2_CORE.z),
        v3(T2_CORE.x + 6.7, 0.0, T2_CORE.z),
        0.55,
        0.4,
        8,
    );
    // Swept plates off the head, out and back: the yoke's horns.
    let f = Frame::new(
        v3(x + 1.2, 0.0, 16.4),
        v3(1.0, 0.0, 0.35),
        v3(-0.35, 0.0, 1.0),
    );
    dark_plate(b);
    Course {
        count: if fine { 3 } else { 2 },
        step: 1.9,
        len: 3.4,
        half: 2.4,
        tip: 0.0,
        thick: 0.55,
        tail: 1.6,
    }
    .lay(b, &f);
    team_tab(b, v3(x + 2.0, 0.0, 16.55), Vec3::X, 1.1);
    // Plates lapped down each flank, outward from the footing.
    b.mirror_y(|b| {
        let f = Frame::new(
            v3(x + 3.0, 3.4, 12.0),
            v3(0.15, 0.25, -1.0),
            v3(0.0, 1.0, 0.1),
        );
        dark_plate(b);
        Course {
            count: if fine { 3 } else { 2 },
            step: 3.0,
            len: 3.8,
            half: 1.9,
            tip: 0.4,
            thick: 0.5,
            tail: 1.0,
        }
        .lay(b, &f);
    });
    // The sill's end: plates swept out along the lot.
    let f = Frame::new(v3(x + 6.0, 0.0, 2.3), v3(1.0, 0.0, -0.18), Vec3::Z);
    dark_plate(b);
    Course {
        count: 2,
        step: 2.2,
        len: 3.4,
        half: 3.4,
        tip: 0.0,
        thick: 0.6,
        tail: 2.4,
    }
    .lay(b, &f);
    if fine {
        red_slot(b, v3(x - 0.05, 0.0, 5.0), -Vec3::X, Vec3::Z, 3.0, 0.24);
        red_slot(b, v3(x - 0.05, 0.0, 15.0), -Vec3::X, Vec3::Y, 2.4, 0.24);
        // A bronze stay from the deck up the inner face to the trunnion.
        stay(b, v3(x - 0.4, 2.6, 2.5), v3(x - 0.2, 1.2, T2_CORE.z - 1.4));
        stay(
            b,
            v3(x - 0.4, -2.6, 2.5),
            v3(x - 0.2, -1.2, T2_CORE.z - 1.4),
        );
    }
}

/// A square-cut bronze stay from `a` to `c`: round bronze reads as a threaded rod.
fn stay(b: &mut MeshBuilder, a: Vec3, c: Vec3) {
    metal(b);
    b.beam(a, c, Vec2::splat(0.5), Vec2::splat(0.4));
}

// ---- tech 3: the crown --------------------------------------------------------------

const T3_CORE: Vec3 = Vec3::new(0.0, 0.0, 21.0);
const T3_TALONS: usize = 6;
/// The crown ring overhead: its radius and height.
const CROWN_R: f32 = 9.5;
const CROWN_Z: f32 = 33.0;

pub(super) fn heart_3(b: &mut MeshBuilder, _tech: u8) {
    let fine = b.fine();
    let bearing = |k: usize| TAU * (k as f32 + 0.5) / T3_TALONS as f32;
    // The footing: a wide plated platform, a seam, a deck.
    let sides = b.sides(T3_TALONS * 2);
    dark_plate(b);
    b.prism(Vec3::ZERO, T3_TALONS, 24.0, 21.0, 2.5);
    if b.coarse() {
        for k in 0..T3_TALONS {
            let d = out(bearing(k));
            let side = Vec3::Z.cross(d) * 2.5;
            dark_plate(b);
            b.face(&[
                d * 40.0,
                d * CROWN_R + v3(0.0, 0.0, CROWN_Z),
                d * 30.0 + side,
            ]);
            b.face(&[
                d * 30.0 - side,
                d * CROWN_R + v3(0.0, 0.0, CROWN_Z),
                d * 40.0,
            ]);
            team_tab(b, d * CROWN_R + v3(0.0, 0.0, CROWN_Z + 1.1), d, 1.4);
        }
        core(b, T3_CORE, 8.0);
        return;
    }
    seam(b);
    b.prism(v3(0.0, 0.0, 2.5), T3_TALONS, 19.5, 19.5, 0.5);
    dark_plate(b);
    b.prism(v3(0.0, 0.0, 3.0), T3_TALONS, 19.0, 16.5, 1.2);
    // The collector under the core: a bronze bowl on a stepped drum, a lit seam.
    metal(b);
    b.cylinder_between(v3(0.0, 0.0, 4.2), v3(0.0, 0.0, 5.4), 4.5, 7.5, sides);
    seam(b);
    b.cylinder_between(v3(0.0, 0.0, 5.4), v3(0.0, 0.0, 5.8), 7.5, 6.8, sides);
    core(b, T3_CORE, 8.0);
    for (normal, spin, rate, r, weights) in [
        (v3(0.0, 0.5, 1.0), Vec3::X, 0.45, 10.0, 4),
        (v3(1.0, 0.0, 0.5), v3(0.0, 1.0, 0.3), -0.7, 11.8, 6),
        (v3(-0.6, 0.6, 1.0), v3(1.0, 1.0, 0.0), 0.95, 13.5, 8),
    ] {
        ring(
            b,
            T3_CORE,
            &Ring {
                normal,
                spin,
                rate,
                r,
                band: 1.2,
                weights,
            },
        );
    }
    // The drive ring, level under the gyroscope, its teeth turning.
    drive_ring(b, v3(0.0, 0.0, 8.5), 17.0, 1.3, 0.25);
    for k in 0..T3_TALONS {
        b.yawed(Vec3::ZERO, bearing(k), |b| {
            talon(b, &TALON, 3.8);
            foot(b, fine);
            // A buttress from the talon's shoulder down onto the platform's rim.
            strut(b, v3(23.0, 0.0, 21.5), v3(18.5, 0.0, 3.6), 1.3);
            // The pinch emitter off the talon's inside at the core.
            let from = v3(25.6, 0.0, T3_CORE.z);
            collar(b, from, -Vec3::X, 1.7, 1.6);
            metal(b);
            b.cylinder_between(from, v3(16.6, 0.0, T3_CORE.z), 0.7, 0.5, 8);
            b.paint(GLOW_LASER);
            b.cylinder_between(
                v3(16.6, 0.0, T3_CORE.z),
                v3(16.1, 0.0, T3_CORE.z),
                0.55,
                0.2,
                6,
            );
            // A plated strut from the deck to the drive ring's bearing.
            strut(b, v3(19.0, 0.0, 3.0), v3(17.0, 0.0, 7.7), 0.8);
            if fine {
                red_slot(b, v3(19.05, 0.0, 1.4), Vec3::X, Vec3::Y, 4.0, 0.3);
            }
        });
    }
    belt(b, fine);
    crown(b, fine);
}

/// A tech 3 talon's line from its foot to the crown (x, z) and half width there.
const TALON: [(f32, f32, f32); 6] = [
    (40.0, 2.2, 6.0),
    (33.0, 6.5, 5.4),
    (27.5, 15.0, 4.6),
    (22.0, 24.0, 3.8),
    (15.5, 30.5, 3.0),
    (CROWN_R + 0.5, CROWN_Z, 2.4),
];
/// Where the belt ties the talons: its height, and how far out the talons are there.
const BELT_Z: f32 = 13.0;
const BELT_R: f32 = 28.4;

/// A talon's foot, along +x: a plated block planted where it meets the ground, plates
/// lapped out from it over the lot.
fn foot(b: &mut MeshBuilder, fine: bool) {
    dark_plate(b);
    b.frustum(
        v3(39.0, 0.0, 0.0),
        Vec2::new(9.0, 11.0),
        Vec2::new(6.5, 8.0),
        4.2,
        Vec2::new(-0.8, 0.0),
    );
    seam(b);
    b.frustum(
        v3(38.7, 0.0, 4.2),
        Vec2::new(6.2, 7.6),
        Vec2::new(5.6, 7.0),
        0.5,
        Vec2::ZERO,
    );
    let f = Frame::new(v3(42.5, 0.0, 1.6), v3(1.0, 0.0, -0.22), Vec3::Z);
    dark_plate(b);
    Course {
        count: if fine { 2 } else { 1 },
        step: 1.6,
        len: 2.6,
        half: 4.2,
        tip: 0.0,
        thick: 0.8,
        tail: 1.2,
    }
    .lay(b, &f);
    if fine {
        red_slot(b, v3(43.55, 0.0, 2.2), Vec3::X, Vec3::Y, 5.0, 0.3);
    }
}

/// The belt: heavy plated beams from talon to talon at `BELT_Z`, so the six stand as one
/// frame, a lit seam along each.
fn belt(b: &mut MeshBuilder, fine: bool) {
    let at =
        |k: usize| out(TAU * (k as f32 + 0.5) / T3_TALONS as f32) * BELT_R + v3(0.0, 0.0, BELT_Z);
    for k in 0..T3_TALONS {
        let (a, c) = (at(k), at(k + 1));
        dark_plate(b);
        b.beam(a, c, Vec2::new(2.8, 2.6), Vec2::new(2.8, 2.6));
        if fine {
            let mid = (a + c) * 0.5;
            let inward = -mid.truncate().extend(0.0).normalize();
            red_slot(b, mid + inward * 1.42, inward, c - a, 9.0, 0.3);
        }
    }
}

/// The crown overhead: a plated ring joining the talons' tips, open in the middle, plates
/// swept out over each tip and the owner's colour on top.
fn crown(b: &mut MeshBuilder, fine: bool) {
    let n = T3_TALONS * 2;
    let at = |k: usize| out(TAU * k as f32 / n as f32) * CROWN_R + v3(0.0, 0.0, CROWN_Z);
    dark_plate(b);
    for k in 0..n {
        b.beam(at(k), at(k + 1), Vec2::new(3.2, 2.4), Vec2::new(3.2, 2.4));
    }
    // A second, wider ring a storey down, braced up to the first at every talon.
    let low = |k: usize| {
        out(TAU * (k as f32 + 0.5) / T3_TALONS as f32) * 14.6 + v3(0.0, 0.0, CROWN_Z - 4.2)
    };
    for k in 0..T3_TALONS {
        b.beam(low(k), low(k + 1), Vec2::new(2.4, 2.0), Vec2::new(2.4, 2.0));
    }
    seam(b);
    hoop(
        b,
        v3(0.0, 0.0, CROWN_Z - 0.9),
        CROWN_R - 1.3,
        0.8,
        0.5,
        if fine { 36 } else { 12 },
    );
    for k in 0..T3_TALONS {
        let a = TAU * (k as f32 + 0.5) / T3_TALONS as f32;
        b.yawed(Vec3::ZERO, a, |b| {
            let f = Frame::new(
                v3(CROWN_R - 1.0, 0.0, CROWN_Z + 0.8),
                v3(-1.0, 0.0, 0.12),
                Vec3::Z,
            );
            dark_plate(b);
            Course {
                count: if fine { 2 } else { 1 },
                step: 1.4,
                len: 2.6,
                half: 1.6,
                tip: 0.0,
                thick: 0.6,
                tail: 1.2,
            }
            .lay(b, &f);
            team_tab(b, v3(CROWN_R + 0.6, 0.0, CROWN_Z + 0.76), Vec3::X, 1.1);
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::{build_model_scaled, part};

    /// Each tier's mesh at its unit file's size and lot.
    const TIERS: [(&str, f32, f32, u32); 3] = [
        ("regency_heart", 6.9, 7.5, 2),
        ("regency_heart_2", 18.75, 18.0, 4),
        ("regency_heart_3", 42.5, 35.0, 8),
    ];

    #[test]
    fn heart() {
        for (key, r, h, cells) in TIERS {
            super::super::check(key, r, h, Some(cells), &[]);
            let model = build_model_scaled(key, r, h, 1).unwrap();
            assert!(
                model.lods[0]
                    .vertices
                    .iter()
                    .any(|v| v.part & part::ORBIT_MASK == part::ORBIT),
                "{key}: no rings turning"
            );
        }
    }

    /// The star is where the rings turn, and a higher tier holds a bigger one.
    #[test]
    fn a_higher_tier_holds_a_bigger_star() {
        let sizes = TIERS.map(|(key, r, h, _)| {
            let model = build_model_scaled(key, r, h, 1).unwrap();
            let star = model.star_core.expect(key);
            assert_eq!(&star[..3], &model.spinner_pivot[..], "{key}");
            star[3]
        });
        assert!(sizes[0] < sizes[1] && sizes[1] < sizes[2], "{sizes:?}");
    }
}
