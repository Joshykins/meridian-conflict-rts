//! The Aster reactors: fusion plants, authored at blueprint scale (metres), their
//! power leaving toward +x. Each tier is a different machine, and each is built from
//! the same working kit, more of it and better, so a base's plants read as one line:
//! - Tech 1, the fusion cell (2x2 lot, 10 m): a drum-shaped vessel with a belt of
//!   viewports onto the burning core, a coil carriage running round it on a rail, two
//!   coolant tanks and a pump behind, and a busbar out to a power coupler.
//! - Tech 2, the tokamak (4x4 lot, 22 m): the core is a ring now, a torus with a
//!   viewport belt round it, held in twelve D-shaped field coils round a central
//!   solenoid whose stirring head turns above it. Heat exchangers and pumps behind,
//!   capacitor banks on both flanks, two power couplers in front.
//! - Tech 3, the spherical tokamak (8x8 lot, 38 m, a factory's lot): a great
//!   containment sphere with a viewport belt at its waist, twelve coils and two
//!   poloidal rings round it, two neutral-beam injectors fired into it, four heat
//!   exchangers, capacitor halls, three power couplers, and a control block.
//!
//! What moves: the carriage or the stirring head turns (`part::SPINNER`), pumps and
//! injectors work on a stroke (`part::PUMP`), the viewports churn (`pattern::PLASMA`)
//! and the busbars carry pulses out to the couplers (`pattern::FLUX`). Nothing is
//! there for menace: every piece is plant that can be read off it.

use std::f32::consts::{FRAC_PI_2, TAU};

use glam::{Vec2, Vec3};

use super::parts::*;
use crate::models::builder::{chamfered_rect, MeshBuilder, Section};
use crate::models::material::*;
use crate::models::{part, pattern};

pub fn power(b: &mut MeshBuilder, tech: u8) {
    match tech {
        0 | 1 => cell(b),
        2 => tokamak(b),
        _ => sphere(b),
    }
}

// ---- Tech 1: the fusion cell ---------------------------------------------------

fn cell(b: &mut MeshBuilder) {
    const DECK: f32 = 0.9;
    b.set_spinner_pivot(v3(0.0, 0.0, DECK));
    if b.coarse() {
        b.paint(PLATING);
        b.cuboid_open(v3(0.0, 0.0, DECK * 0.5), v3(21.0, 21.0, DECK));
        b.prism(v3(0.0, 0.0, DECK), 4, 5.6, 3.6, 8.4);
        b.paint(ACCENT);
        b.cuboid_open(v3(8.9, 0.0, 1.45), v3(2.8, 3.0, 1.1));
        b.paint(GLOW);
        b.cuboid_open(v3(0.0, 0.0, 4.0), v3(8.2, 8.2, 2.2));
        team_panel(b, v3(-5.0, 7.0, DECK), v2(3.0, 3.0));
        return;
    }
    slab(b, 10.6, 2.4, 0.5, DECK);

    // The vessel: a collar, the belt of viewports, the shoulder and the injector.
    b.paint(PLATING);
    b.prism(v3(0.0, 0.0, DECK), round(b, 12), 4.7, 4.5, 1.3);
    b.paint(ACCENT).pattern(pattern::PLASMA);
    b.prism(v3(0.0, 0.0, DECK + 1.3), round(b, 12), 4.05, 4.05, 3.2);
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, DECK + 4.4), round(b, 12), 4.6, 4.6, 0.35);
    b.paint(PLATING);
    b.loft_z(
        &ngon_plan(round(b, 12), 1.0),
        &[
            Section::new(DECK + 4.75, 4.4),
            Section::new(DECK + 5.9, 4.1),
            Section::new(DECK + 7.1, 2.7),
            Section::new(DECK + 7.5, 1.5),
        ],
    );
    injector(b, v3(0.0, 0.0, DECK + 7.5), 1.0, 1.1);
    b.radial(2, |b| {
        on_slope(b, [4.1, DECK + 5.9], [2.7, DECK + 7.1], 0.5, |b| {
            team_panel(b, Vec3::ZERO, v2(1.3, 2.2))
        })
    });

    // The coil carriage: a rail round the collar, and three yokes that run round
    // on it, lit faces turned in on the core.
    if b.fine() {
        b.paint(ACCENT);
        // Clear of the collar's corners, so its inner wall never cuts through it.
        annulus(b, 16, 4.85, 6.1, DECK, DECK + 0.45);
    }
    b.with_part(part::SPINNER, |b| {
        b.paint(METAL);
        annulus(b, round(b, 16), 4.8, 6.0, DECK + 0.45, DECK + 0.8);
        b.radial(3, |b| yoke(b, 5.35, DECK + 0.8, DECK + 4.6, 0.9));
    });

    // Coolant: two tanks behind, the pump between them, pipes to the vessel.
    b.mirror_y(|b| {
        tank(b, v3(-7.4, 6.0, DECK), 1.9, 5.2);
        b.paint(METAL);
        pipe(b, &[v3(-5.6, 5.2, DECK + 3.8), v3(-4.2, 3.0, DECK + 3.8)], 0.42);
    });
    pump(b, v3(-8.6, 0.0, DECK), 1.0);
    b.paint(METAL);
    pipe(b, &[v3(-7.9, 0.0, DECK + 1.0), v3(-4.6, 0.0, DECK + 1.0)], 0.35);

    // Power out: a busbar across the deck to the coupler by the lot's edge.
    busbar(b, v3(4.7, 0.0, DECK), v3(7.4, 0.0, DECK), 1.1, 0.5);
    coupler(b, v3(8.9, 0.0, DECK), v3(2.8, 3.0, 1.1));
    if b.fine() {
        for (x, y) in [(7.6, 8.4), (-2.0, 9.2), (-2.0, -9.2), (7.6, -8.4)] {
            glow_strip(b, v3(x, y, DECK), v2(1.6, 0.3), GLOW);
        }
    }
}

// ---- Tech 2: the tokamak --------------------------------------------------------

fn tokamak(b: &mut MeshBuilder) {
    const DECK: f32 = 1.4;
    // Torus: its ring radius, tube radius, how much taller than round, and its middle.
    const RING: f32 = 9.6;
    const TUBE: f32 = 4.4;
    const TALL: f32 = 1.2;
    const MID: f32 = DECK + 6.2;
    b.set_spinner_pivot(v3(0.0, 0.0, 16.5));
    if b.coarse() {
        b.paint(PLATING);
        b.cuboid_open(v3(0.0, 0.0, DECK * 0.5), v3(45.0, 45.0, DECK));
        b.prism(v3(0.0, 0.0, DECK), 4, 20.0, 14.0, 11.0);
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, DECK), 4, 5.0, 3.5, 19.5);
        b.cuboid_open(v3(-17.0, 0.0, 3.2), v3(6.0, 30.0, 3.6));
        b.paint(GLOW);
        b.cuboid_open(v3(0.0, 0.0, MID), v3(20.4, 20.4, 2.6));
        team_panel(b, v3(13.5, 17.0, DECK), v2(4.0, 3.0));
        return;
    }
    slab(b, 22.8, 4.0, 0.7, DECK);

    // The torus, and the belt of viewports round its outer equator.
    b.paint(PLATING);
    torus(b, v3(0.0, 0.0, MID), RING, TUBE, TALL, round(b, 24), round(b, 10));
    b.paint(ACCENT).pattern(pattern::PLASMA);
    band(b, round(b, 24), RING + TUBE + 0.12, MID - 1.3, MID + 1.3);
    // Plinth ring under it, the torus's cradle.
    if b.fine() {
        b.paint(ACCENT);
        annulus(b, 24, RING - 2.4, RING + 2.4, DECK, MID - TUBE * TALL + 0.6);
    }

    // Field coils: twelve Ds round the ring, their inner legs in the solenoid.
    let d = coil_d(5.2, RING + TUBE + 1.0, DECK + 0.2, MID + TUBE * TALL + 0.9, MID);
    let coils = if b.fine() { 12 } else { 6 };
    b.radial(coils, |b| {
        b.paint(PLATING);
        sweep(b, &d, v2(1.1, 0.9), true);
    });
    if b.fine() {
        b.yawed(Vec3::ZERO, TAU / 24.0, |b| {
            b.radial(12, |b| {
                b.paint(ACCENT);
                b.cuboid(v3(RING + TUBE + 1.4, 0.0, MID), v3(0.5, 0.7, 1.6));
            })
        });
    }

    // The central solenoid, and its stirring head turning above the ring.
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, DECK), round(b, 12), 4.8, 4.6, 13.6);
    b.paint(PLATING);
    for z in [DECK + 2.0, DECK + 12.0] {
        b.prism(v3(0.0, 0.0, z), round(b, 12), 5.2, 5.2, 1.2);
    }
    b.paint(PLATING);
    b.loft_z(
        &ngon_plan(round(b, 12), 1.0),
        &[Section::new(15.0, 5.0), Section::new(15.6, 5.0), Section::new(16.5, 3.8)],
    );
    b.with_part(part::SPINNER, |b| {
        b.paint(ACCENT);
        annulus(b, round(b, 16), 2.6, 4.4, 16.5, 17.5);
        b.radial(4, |b| pod(b, v3(4.2, 0.0, 17.0), 0.7));
    });
    injector(b, v3(0.0, 0.0, 16.5), 1.5, 3.2);
    b.mirror_y(|b| team_panel(b, v3(13.5, 17.0, DECK), v2(4.0, 3.0)));

    // Behind: two heat exchangers on saddles, each with its pump, piped to the ring.
    b.mirror_y(|b| {
        exchanger(b, v3(-20.0, 8.5, DECK), v3(-20.0, 19.0, DECK), 2.2);
        pump(b, v3(-14.8, 17.5, DECK), 1.4);
        b.paint(METAL);
        pipe(
            b,
            &[v3(-20.0, 8.0, DECK + 3.6), v3(-15.5, 6.0, DECK + 3.6), v3(-12.2, 4.4, MID - 1.5)],
            0.7,
        );
        pipe(b, &[v3(-14.8, 16.2, DECK + 1.2), v3(-14.8, 11.0, DECK + 1.2), v3(-11.0, 9.0, DECK + 1.2)], 0.5);
    });

    // Flanks: a capacitor bank on each side.
    b.mirror_y(|b| {
        b.paint(ACCENT);
        b.chamfered_box(v3(1.0, 19.0, DECK + 1.0), v3(17.0, 4.6, 2.0), 0.8);
        for i in 0..4 {
            capacitor(b, v3(-5.4 + 4.2 * i as f32, 19.0, DECK + 2.0), 1.2, 3.4);
        }
        busbar(b, v3(1.0, 16.4, DECK), v3(1.0, 11.6, DECK), 1.0, 0.45);
    });

    // In front: the busbars out to two couplers, where the power goes into the ground.
    b.mirror_y(|b| {
        busbar(b, v3(RING + 2.4, 4.6, DECK), v3(16.4, 7.0, DECK), 1.3, 0.55);
        coupler(b, v3(18.4, 7.0, DECK), v3(4.0, 4.4, 1.6));
    });
    if b.fine() {
        for (x, y) in [(-8.0, 21.8), (-8.0, -21.8), (12.0, 21.8), (12.0, -21.8)] {
            glow_strip(b, v3(x, y, DECK), v2(2.6, 0.4), GLOW);
        }
    }
}

// ---- Tech 3: the spherical tokamak -----------------------------------------------

fn sphere(b: &mut MeshBuilder) {
    const DECK: f32 = 1.8;
    const PLINTH: f32 = 3.0;
    const R: f32 = 14.5;
    const MID: f32 = 17.5;
    b.set_spinner_pivot(v3(0.0, 0.0, 33.0));
    if b.coarse() {
        b.paint(PLATING);
        b.cuboid_open(v3(0.0, 0.0, DECK * 0.5), v3(93.0, 93.0, DECK));
        b.prism(v3(0.0, 0.0, DECK), 4, 30.0, 16.0, 29.0);
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, 30.0), 4, 5.0, 2.5, 8.0);
        b.cuboid_open(v3(-36.0, 0.0, 4.0), v3(10.0, 70.0, 5.0));
        b.paint(GLOW);
        b.cuboid_open(v3(0.0, 0.0, MID), v3(29.6, 29.6, 5.0));
        team_panel(b, v3(33.0, -37.0, DECK), v2(9.0, 6.0));
        return;
    }
    slab(b, 46.2, 7.0, 0.9, DECK);
    // The plinth the machine stands on.
    b.paint(ACCENT);
    b.loft_z(
        &chamfered_rect(v2(24.0, 24.0), 7.0),
        &[Section::new(DECK, 1.0), Section::new(PLINTH, 0.98)],
    );

    // The sphere, the viewport belt at its waist, and its foot.
    b.paint(PLATING);
    b.loft_z(
        &ngon_plan(round(b, 24), 1.0),
        &sphere_sections(MID, R, 0.92, PLINTH, 0.64 * R, round(b, 12)),
    );
    b.paint(ACCENT).pattern(pattern::PLASMA);
    band(b, round(b, 24), R + 0.15, MID - 2.6, MID + 2.6);
    b.paint(ACCENT);
    for z in [MID - 2.9, MID + 2.6] {
        annulus(b, round(b, 16), R - 0.5, R + 0.55, z, z + 0.3);
    }

    // Field coils and the two poloidal rings outside them.
    let top = MID + R * 0.92 + 1.4;
    let d = coil_d(4.2, R + 2.0, PLINTH, top, MID);
    let coils = if b.fine() { 12 } else { 6 };
    b.radial(coils, |b| {
        b.paint(PLATING);
        sweep(b, &d, v2(1.6, 1.3), true);
        if b.fine() {
            b.paint(ACCENT);
            b.cuboid(v3(R + 2.0, 0.0, MID), v3(1.0, 2.0, 3.0));
        }
    });
    b.paint(ACCENT);
    let rings: &[(f32, f32)] = if b.fine() { &[(PLINTH + 4.0, 16.4), (MID + 9.5, 14.2)] } else { &[(MID + 9.5, 14.2)] };
    for &(z, r) in rings {
        annulus(b, round(b, 20), r + 0.4, r + 2.0, z, z + 1.3);
    }

    // The central column out of the top, its stirring head, and the injector.
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, MID + R * 0.8), round(b, 12), 3.6, 3.4, 32.2 - (MID + R * 0.8));
    b.with_part(part::SPINNER, |b| {
        b.paint(PLATING);
        annulus(b, round(b, 16), 3.2, 5.6, 32.2, 33.8);
        b.radial(6, |b| pod(b, v3(5.4, 0.0, 33.0), 0.9));
    });
    injector(b, v3(0.0, 0.0, 32.2), 1.9, 5.2);

    // Two neutral-beam injectors fired tangentially into the waist.
    b.mirror_y(|b| {
        let (from, to) = (v3(10.0, 31.0, DECK + 8.0), v3(4.0, R + 1.0, MID - 1.0));
        b.paint(ACCENT);
        b.beam(from, to, v2(3.2, 3.4), v2(2.2, 2.4));
        b.paint(PLATING);
        b.cuboid_open(from - Vec3::Z * 4.0, v3(6.0, 7.0, 8.0));
        b.paint(GLOW);
        b.beam(from + v3(0.0, 0.0, 1.8), to + v3(0.0, 0.0, 1.3), v2(0.35, 0.2), v2(0.35, 0.2));
    });

    // Behind: four heat exchangers with their pumps, piped to the sphere.
    for y in [-30.0, -11.0, 11.0, 30.0] {
        let s = if y < 0.0 { -1.0f32 } else { 1.0 };
        exchanger(b, v3(-44.0, y, DECK), v3(-30.0, y, DECK), 3.2);
        pump(b, v3(-26.5, y, DECK), 1.9);
        b.paint(METAL);
        let into = (v2(-10.4, s * (4.0 + y.abs() * 0.2)).normalize() * R * 0.9).extend(MID - 5.0);
        pipe(b, &[v3(-30.0, y, DECK + 3.6), v3(-24.0, y * 0.8, DECK + 5.4), into], 0.95);
    }

    // Flanks: capacitor halls, their busbars to the plinth.
    b.mirror_y(|b| {
        b.paint(ACCENT);
        b.chamfered_box(v3(-4.0, 39.0, DECK + 2.0), v3(30.0, 8.0, 4.0), 1.2);
        b.paint(PLATING);
        b.plate(v3(-4.0, 39.0, DECK + 4.0), v2(28.4, 6.6), 0.3, 0.15);
        for i in 0..6 {
            capacitor(b, v3(-16.0 + 4.8 * i as f32, 39.0, DECK + 4.3), 1.5, 4.0);
        }
        busbar(b, v3(-4.0, 34.8, DECK), v3(-4.0, 24.4, DECK), 1.4, 0.6);
    });

    // In front: three busbars to three couplers.
    for y in [-15.0, 0.0, 15.0] {
        busbar(b, v3(9.0, y * 0.3, PLINTH), v3(24.0, y, PLINTH), 1.6, 0.6);
        busbar(b, v3(24.0, y, DECK), v3(32.4, y, DECK), 1.6, 0.6);
        coupler(b, v3(35.0, y, DECK), v3(5.6, 6.0, 2.2));
    }

    // The control block in the front corner, its windows toward the machine.
    b.paint(PLATING);
    b.chamfered_box(v3(33.0, -37.0, DECK + 4.0), v3(16.0, 11.0, 8.0), 1.2);
    b.paint(GLASS);
    b.cuboid(v3(33.0, -31.4, DECK + 5.4), v3(13.0, 0.3, 1.6));
    team_panel(b, v3(33.0, -37.0, DECK + 8.0), v2(9.0, 6.0));
    if b.fine() {
        antenna(b, v3(38.0, -40.0, DECK + 8.0), 5.0, 0.0);
        for (x, y) in [(-16.0f32, 45.2f32), (16.0, 45.2), (-16.0, -45.2), (16.0, -45.2), (45.2, 30.0), (45.2, -30.0)] {
            let size = if y.abs() > 45.0 { v2(4.0, 0.5) } else { v2(0.5, 4.0) };
            glow_strip(b, v3(x, y, DECK), size, GLOW);
        }
    }
}

// ---- the kit ---------------------------------------------------------------------

/// Sides for a round shape with `n` at full detail. The plants are big and many: at the
/// middle distance they drop to half, not the usual three quarters.
fn round(b: &MeshBuilder, n: usize) -> usize {
    if b.fine() {
        n
    } else if b.mid() {
        (n / 2).max(6)
    } else {
        4
    }
}

/// Unit n-gon plan, for lofts scaled by their sections.
fn ngon_plan(sides: usize, radius: f32) -> Vec<[f32; 2]> {
    crate::models::builder::ngon(sides, radius)
}

/// The lot's slab: a dark foot and a white deck, `half` metres out.
fn slab(b: &mut MeshBuilder, half: f32, chamfer: f32, foot: f32, deck: f32) {
    let plan = chamfered_rect(v2(half, half), chamfer);
    b.paint(ACCENT);
    b.loft_z(&plan, &[Section::new(0.0, 1.0), Section::new(foot, 1.0)]);
    b.paint(PLATING);
    b.loft_z(&plan, &[Section::new(foot, 0.99), Section::new(deck, 0.97)]);
}

/// Sections of a sphere of radius `r` about height `mid`, cut flat at `top` of the
/// way up and standing on a foot `foot_r` wide at `base`.
fn sphere_sections(mid: f32, r: f32, top: f32, base: f32, foot_r: f32, rings: usize) -> Vec<Section> {
    // The pedestal rises straight to where the sphere is as wide as it, then the sphere.
    let low = -(foot_r / r).clamp(0.0, 1.0).acos();
    let low = low.max(((base - mid) / r).clamp(-1.0, 0.0).asin());
    let high = top.asin();
    let mut out = vec![Section::new(base, foot_r)];
    for i in 0..=rings {
        let a = low + (high - low) * i as f32 / rings as f32;
        out.push(Section::new(mid + r * a.sin(), (r * a.cos()).max(foot_r * 0.3)));
    }
    out
}

/// A closed ring of flat facets round the z axis with no caps: a belt laid over a drum.
fn band(b: &mut MeshBuilder, sides: usize, radius: f32, z0: f32, z1: f32) {
    let ring = |z: f32| -> Vec<Vec3> {
        (0..sides)
            .map(|i| {
                let a = (i as f32 + 0.5) * TAU / sides as f32;
                v3(a.cos() * radius, a.sin() * radius, z)
            })
            .collect()
    };
    b.loft(&[ring(z0), ring(z1)], false, false);
}

/// A flat ring round the z axis, `r0` to `r1` out and `z0` to `z1` up.
fn annulus(b: &mut MeshBuilder, sides: usize, r0: f32, r1: f32, z0: f32, z1: f32) {
    let section = [(r0, z0), (r1, z0), (r1, z1), (r0, z1)];
    let rings: Vec<Vec<Vec3>> = (0..=sides)
        .map(|i| {
            let a = i as f32 * TAU / sides as f32;
            let (c, s) = (a.cos(), a.sin());
            section.iter().map(|&(r, z)| v3(c * r, s * r, z)).collect()
        })
        .collect();
    b.loft(&rings, false, false);
}

/// A torus round the z axis at `center`, its tube `tall` times taller than it is wide.
fn torus(b: &mut MeshBuilder, center: Vec3, ring: f32, tube: f32, tall: f32, around: usize, across: usize) {
    let rings: Vec<Vec<Vec3>> = (0..=around)
        .map(|i| {
            let a = i as f32 * TAU / around as f32;
            let (c, s) = (a.cos(), a.sin());
            (0..across)
                .map(|j| {
                    let t = (j as f32 + 0.5) * TAU / across as f32;
                    let r = ring + tube * t.cos();
                    center + v3(c * r, s * r, tube * tall * t.sin())
                })
                .collect()
        })
        .collect();
    b.loft(&rings, false, false);
}

/// A field coil's D in the xz plane: a straight inner leg at `inner`, bellying out to
/// `outer` at height `mid`, from `bottom` to `top`.
fn coil_d(inner: f32, outer: f32, bottom: f32, top: f32, mid: f32) -> Vec<Vec3> {
    let mut path = vec![v3(inner, 0.0, bottom), v3(inner, 0.0, top)];
    let n = 7;
    for i in 1..n {
        // Round from the top of the inner leg, out through the waist, back to the bottom.
        let a = FRAC_PI_2 - std::f32::consts::PI * i as f32 / n as f32;
        let (half_up, half_down) = (top - mid, mid - bottom);
        let z = if a >= 0.0 { mid + half_up * a.sin() } else { mid + half_down * a.sin() };
        let x = inner + (outer - inner) * a.cos().powf(0.7);
        path.push(v3(x, 0.0, z));
    }
    path
}

/// A bar of rectangular section (`size`: across in y, deep in the path's plane) swept
/// along `path`, closed back on itself if `closed`.
fn sweep(b: &mut MeshBuilder, path: &[Vec3], size: Vec2, closed: bool) {
    let n = path.len();
    let at = |i: usize| path[(i + n) % n];
    let rings: Vec<Vec<Vec3>> = (0..n + usize::from(closed))
        .map(|k| {
            let i = k % n;
            let (prev, next) = if closed {
                (at(i + n - 1), at(i + 1))
            } else {
                (path[i.saturating_sub(1)], path[(i + 1).min(n - 1)])
            };
            let t = (next - prev).normalize_or(Vec3::Z);
            let side = Vec3::Y;
            let up = side.cross(t).normalize_or(Vec3::X);
            let (s, u) = (side * size.x * 0.5, up * size.y * 0.5);
            let p = path[i];
            vec![p - s - u, p + s - u, p + s + u, p - s + u]
        })
        .collect();
    b.loft(&rings, !closed, !closed);
}

/// A pipe run through `points`, with a flange at each joint.
fn pipe(b: &mut MeshBuilder, points: &[Vec3], radius: f32) {
    let sides = round(b, 8);
    for pair in points.windows(2) {
        b.cylinder_between(pair[0], pair[1], radius, radius, sides);
    }
    if b.fine() {
        for &p in &points[1..points.len() - 1] {
            b.spheroid(p, Vec3::splat(radius * 1.25), 6, 2);
        }
    }
}

/// The fuel injector on a vessel's crown: a neck, a head, a lit port. `s` scales it.
fn injector(b: &mut MeshBuilder, base: Vec3, s: f32, height: f32) {
    b.paint(METAL);
    b.prism(base, round(b, 8), 0.75 * s, 0.6 * s, height * 0.6);
    b.paint(ACCENT);
    b.prism(base + Vec3::Z * height * 0.6, round(b, 8), 1.15 * s, 0.95 * s, height * 0.3);
    b.paint(GLOW);
    b.prism(base + Vec3::Z * height * 0.9, 6, 0.5 * s, 0.35 * s, height * 0.1);
}

/// A coil yoke on the carriage: a C of dark frame round the core, its lit face turned in.
fn yoke(b: &mut MeshBuilder, r: f32, z0: f32, z1: f32, w: f32) {
    b.paint(PLATING);
    b.cuboid_open(v3(r + 0.1, 0.0, (z0 + z1) * 0.5), v3(0.7, w, z1 - z0));
    if b.fine() {
        b.paint(ACCENT);
        // Its top stands proud of the frame's, or the two fight.
        b.cuboid(v3(r - 0.3, 0.0, z1 - 0.25), v3(1.0, w * 1.1, 0.6));
    }
    b.paint(GLOW);
    b.cuboid(v3(r - 0.35, 0.0, (z0 + z1) * 0.5 + 0.2), v3(0.2, w * 0.55, (z1 - z0) * 0.55));
}

/// A field pod on a stirring head: a dark housing with a lit face turned out and down.
fn pod(b: &mut MeshBuilder, at: Vec3, s: f32) {
    b.paint(ACCENT);
    if b.fine() {
        b.chamfered_box(at, v3(1.4 * s, 1.4 * s, 1.2 * s), 0.3 * s);
    } else {
        b.cuboid(at, v3(1.4 * s, 1.4 * s, 1.2 * s));
    }
    b.paint(GLOW);
    b.cuboid(at + v3(0.72 * s, 0.0, -0.1 * s), v3(0.16 * s, 0.8 * s, 0.6 * s));
}

/// A coolant tank: a banded drum with a domed head.
fn tank(b: &mut MeshBuilder, base: Vec3, r: f32, h: f32) {
    let sides = round(b, 12);
    b.paint(PLATING);
    let head = if b.fine() { 4 } else { 3 };
    b.loft_z(
        &ngon_plan(sides, 1.0),
        &[
            Section::new(base.z, r).shifted(base.x, base.y),
            Section::new(base.z + h, r).shifted(base.x, base.y),
            Section::new(base.z + h + r * 0.35, r * 0.7).shifted(base.x, base.y),
            Section::new(base.z + h + r * 0.5, r * 0.25).shifted(base.x, base.y),
        ][..head],
    );
    if b.fine() {
        b.paint(ACCENT);
        for z in [0.3, 0.62] {
            b.prism(base + Vec3::Z * h * z, sides, r + 0.08, r + 0.08, h * 0.08);
        }
    }
}

/// A heat exchanger: a long drum on saddles from `a` to `bb` (both on the deck), with
/// dark end bells.
fn exchanger(b: &mut MeshBuilder, a: Vec3, bb: Vec3, r: f32) {
    let lift = Vec3::Z * (r + 0.6);
    let sides = round(b, 10);
    b.paint(PLATING);
    b.cylinder_between(a + lift, bb + lift, r, r, sides);
    b.paint(ACCENT);
    let along = (bb - a).normalize_or(Vec3::X);
    if b.fine() {
        for (p, dir) in [(a, along), (bb, -along)] {
            b.cylinder_between(p + lift - dir * 0.3, p + lift + dir * 0.9, r * 1.08, r * 1.08, sides);
        }
    }
    for k in [0.25, 0.75] {
        if !b.fine() {
            // At a distance the drum's own foot is enough.
            break;
        }
        let p = a.lerp(bb, k);
        let across = v3(-along.y, along.x, 0.0);
        b.beam(p, p + Vec3::Z * (r + 0.4), v2(r * 1.6, 0.8), v2(r * 1.6, 0.8));
        if b.fine() {
            b.paint(METAL);
            b.cylinder_between(p + lift + across * r * 0.2, p + lift + Vec3::Z * (r + 0.5), 0.25, 0.25, 6);
            b.paint(ACCENT);
        }
    }
}

/// A circulating pump: a housing on the deck and its piston working above it (`part::PUMP`).
fn pump(b: &mut MeshBuilder, base: Vec3, s: f32) {
    b.paint(ACCENT);
    b.chamfered_box(base + Vec3::Z * 0.8 * s, v3(2.2 * s, 2.2 * s, 1.6 * s), 0.4 * s);
    b.paint(METAL);
    b.prism(base + Vec3::Z * 1.6 * s, round(b, 8), 0.7 * s, 0.7 * s, 0.25 * s);
    b.with_part(part::PUMP, |b| {
        b.paint(METAL);
        b.prism(base + Vec3::Z * 1.6 * s, round(b, 8), 0.35 * s, 0.35 * s, 1.9 * s);
        b.paint(PLATING);
        b.chamfered_box(base + Vec3::Z * 3.7 * s, v3(1.1 * s, 1.1 * s, 0.6 * s), 0.2 * s);
    });
    if b.fine() {
        glow_strip(b, base + v3(1.12 * s, 0.0, 0.4 * s), v2(0.08, 1.2 * s), GLOW);
    }
}

/// A busbar along the deck from `a` to `bb`: its top carries the plant's output
/// (`pattern::FLUX`) toward `bb`.
fn busbar(b: &mut MeshBuilder, a: Vec3, bb: Vec3, width: f32, height: f32) {
    b.paint(ACCENT).pattern(pattern::FLUX);
    b.beam(a + Vec3::Z * height * 0.5, bb + Vec3::Z * height * 0.5, v2(width, height), v2(width, height));
}

/// A power coupler: a low dark housing the busbar runs into along x, its top carrying
/// the output on (`pattern::FLUX`), and a lit slot at its outer end where the line
/// goes into the ground.
fn coupler(b: &mut MeshBuilder, base: Vec3, size: Vec3) {
    b.paint(ACCENT);
    b.chamfered_box(base + Vec3::Z * size.z * 0.5, size, size.x.min(size.y) * 0.2);
    b.paint(ACCENT).pattern(pattern::FLUX);
    b.plate(base + Vec3::Z * size.z, v2(size.x * 0.86, size.y * 0.6), 0.12, 0.06);
    glow_strip(b, base + v3(size.x * 0.5 + 0.05, 0.0, 0.0), v2(0.25, size.y * 0.5), GLOW);
}

/// A capacitor can: a dark drum with a lit crown and a terminal.
fn capacitor(b: &mut MeshBuilder, base: Vec3, r: f32, h: f32) {
    b.paint(ACCENT);
    b.prism(base, round(b, 6), r, r, h);
    b.paint(GLOW);
    if b.fine() {
        b.prism(base + Vec3::Z * h, 6, r * 0.85, r * 0.6, r * 0.3);
    } else {
        // High enough off the lid to hold apart at the middle distance.
        b.decal(base + Vec3::Z * (h + 0.08), Vec2::splat(r * 1.2));
    }
    if b.fine() {
        b.paint(METAL);
        b.prism(base + Vec3::Z * (h + r * 0.3), 6, r * 0.2, r * 0.15, r * 0.5);
    }
}

#[cfg(test)]
mod tests {
    use crate::models::{build_model_scaled, material, part};

    const SIZES: [(f32, f32); 3] = [(10.5, 10.0), (22.5, 22.0), (46.0, 38.0)];

    #[test]
    fn reactors_grow_by_tier_within_budget() {
        let mut last = (0, 0);
        for (i, &(r, h)) in SIZES.iter().enumerate() {
            let tech = i as u8 + 1;
            let model = build_model_scaled("power", r, h, tech).unwrap();
            let [full, mid, coarse] = [0, 1, 2].map(|l| model.lods[l].indices.len() / 3);
            println!("power T{tech}: {full}/{mid}/{coarse}");
            assert!(full <= 6000 && coarse < 60, "T{tech}: {full}/{mid}/{coarse}");
            assert!(mid as f32 <= full as f32 * 0.45 + 20.0, "T{tech}: reduced {mid} of {full}");
            let glow = model.lods[0].vertices.iter().filter(|v| v.material == material::GLOW).count();
            assert!(full > last.0 && glow > last.1, "T{tech} adds machinery: {full} tris, {glow} glow");
            last = (full, glow);
            for lod in &model.lods[..2] {
                assert!(lod.vertices.iter().any(|v| v.part == part::SPINNER), "T{tech} turns");
                assert!(lod.vertices.iter().any(|v| v.part == part::PUMP), "T{tech} pumps");
            }
        }
    }
}
