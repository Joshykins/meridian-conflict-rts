//! The Breakwater, a moored torpedo-defence float (tech 1, refitted to tech 2 where it
//! floats): interceptor launchers on turrets under the water (`aster_t1_torpedo_defense`).
//!
//! No hull under the launcher: a spar through the waterline stands on three outrigger
//! floats on arms (a three-pointed plan), with a work platform where the arms meet and
//! the training gear's drum on top. Under the water, a twin-tube launcher turns on the
//! spar on a gun house of its own (weapon 0, `MeshBuilder::with_house`), toward the
//! torpedo it fires at; the tracker head on the drum turns with it, so the float shows
//! from above where it is aiming. The tech 2 refit hangs a second launcher lower on the
//! spar (weapon 1), joins the floats with a triangular truss walk, and runs a lattice
//! mast with a fire-control director up out of the drum. The origin is the waterline;
//! the moorings and listening gear hang below it (the sonar buoy's floor rule). Tech 1
//! is unlit but for the red obstruction lamp every moored float carries.

use glam::{Vec2, Vec3};

use super::{beacon, buoy_ring};
use crate::aster::parts::*;
use crate::aster::structures::kit;
use crate::builder::{ngon, MeshBuilder, Section};
use crate::material::*;
use crate::pattern;

/// When in the refit the tech 2 pieces come on.
const REFIT_AT: f32 = 0.15;
/// Each launcher turret's pivot depth on the spar (weapons 0 and 1), and its muzzles:
/// `MUZZLE_X` forward of the spar, `MUZZLE_Y` either side (the unit file's).
const TURRET_Z: [f32; 2] = [-3.0, -5.5];
const MUZZLE_X: f32 = 3.2;
const MUZZLE_Y: f32 = 0.45;
/// How far the tubes kick back when they fire.
const TRAVEL: f32 = 0.2;
/// The floats' headings (degrees) and how far out they stand.
const FLOATS: [f32; 3] = [60.0, 180.0, 300.0];
const FLOAT_R: f32 = 4.6;
/// The work platform, and the drum's floor and roof.
const PLATFORM: f32 = 1.4;
const DRUM: [f32; 2] = [3.0, 3.75];
/// Tech 1's mast top, and tech 2's.
const MAST: [f32; 2] = [4.6, 7.0];
/// The spar's foot, under the lower turret, and the listening dome under it.
const SPAR_FOOT: f32 = -7.0;

/// A tube from `top` to its mouth at `mouth`: a dark trunk and a black collar whose end
/// cap is centred on the mouth.
fn tube(b: &mut MeshBuilder, top: Vec3, mouth: Vec3, radius: f32) {
    let dir = (mouth - top).normalize();
    let sides = b.sides(8);
    b.paint(PLATING_DARK);
    b.cylinder_between(top, mouth - dir * 0.14, radius, radius, sides);
    b.paint(ACCENT);
    b.cylinder_between(
        mouth - dir * 0.14,
        mouth,
        radius * 1.18,
        radius * 1.12,
        sides,
    );
}

/// Mooring chains out and down at the given headings (degrees).
fn moorings(b: &mut MeshBuilder, headings: &[f32], from: (f32, f32), to: (f32, f32)) {
    b.paint(ACCENT);
    for a in headings {
        let (s, c) = a.to_radians().sin_cos();
        b.beam(
            v3(from.0 * c, from.0 * s, from.1),
            v3(to.0 * c, to.0 * s, to.1),
            v2(0.12, 0.12),
            v2(0.12, 0.12),
        );
    }
}

/// The passive hydrophone that hears torpedoes coming: a dark dome under the float on a
/// stalk from `top`, with a clamp band at full detail.
fn listening_dome(b: &mut MeshBuilder, top: Vec3, centre: Vec3, radius: f32) {
    b.paint(PLATING_DARK).pattern(pattern::PILE);
    b.cylinder_between(top, centre, 0.3, 0.3, b.sides(8));
    b.paint(PLATING_DARK);
    let rings = if b.fine() { 4 } else { 2 };
    b.spheroid(
        centre,
        v3(radius, radius, radius * 0.85),
        if b.fine() { 12 } else { 6 },
        rings,
    );
    if b.fine() {
        b.paint(ACCENT);
        b.cylinder_between(
            centre - Vec3::Z * 0.12,
            centre + Vec3::Z * 0.12,
            radius * 1.03,
            radius * 1.03,
            12,
        );
    }
}

/// The Breakwater: a spar on three outrigger floats with a twin-tube launcher turret
/// under the water (tech 2 hangs a second turret below it, joins the floats with a truss
/// walk, and raises a lattice director mast).
pub(in crate::aster) fn build(b: &mut MeshBuilder, tech: u8) {
    if b.coarse() {
        coarse(b, tech);
        return;
    }
    column(b);
    for a in FLOATS {
        b.yawed(Vec3::ZERO, a.to_radians(), outrigger);
    }
    b.with_house(0, v3(0.0, 0.0, TURRET_Z[0]), TRAVEL, |b| {
        launcher(b, TURRET_Z[0]);
        tracker(b);
    });
    b.paint(PLATING);
    b.prism(
        v3(0.0, 0.0, DRUM[1]),
        b.sides(6),
        0.18,
        0.13,
        MAST[0] - DRUM[1],
    );
    if b.fine() {
        whip(b, v3(-0.9, -0.9, DRUM[1]), 1.3, 0.05);
    }
    if tech < 2 {
        beacon(b, v3(0.0, 0.0, MAST[0]));
    }
    listening_dome(
        b,
        v3(0.0, 0.0, SPAR_FOOT),
        v3(0.0, 0.0, SPAR_FOOT - 1.0),
        0.9,
    );
    moorings(b, &FLOATS, (FLOAT_R + 0.4, -0.9), (6.8, -8.0));
    kit(b, tech, 2, REFIT_AT, refit);
}

fn coarse(b: &mut MeshBuilder, tech: u8) {
    b.paint(PLATING);
    b.prism(v3(0.0, 0.0, -1.5), 4, 1.8, 1.8, DRUM[1] + 1.5);
    for a in FLOATS {
        let (s, c) = a.to_radians().sin_cos();
        b.cuboid_open(v3(FLOAT_R * c, FLOAT_R * s, -0.1), v3(2.0, 2.0, 2.2));
    }
    team_panel(b, v3(0.0, 0.0, DRUM[1]), v2(1.6, 1.6));
    let top = if tech >= 2 { MAST[1] } else { MAST[0] + 0.5 };
    b.frustum_open(
        v3(0.0, 0.0, DRUM[1]),
        v2(0.5, 0.5),
        v2(0.2, 0.2),
        top - DRUM[1],
        Vec2::ZERO,
    );
    // Each launcher as one flat plate facing up, from the spar out to its muzzles.
    let turrets = if tech >= 2 {
        &TURRET_Z[..]
    } else {
        &TURRET_Z[..1]
    };
    for (weapon, &z) in turrets.iter().enumerate() {
        b.with_house(weapon, v3(0.0, 0.0, z), TRAVEL, |b| {
            b.paint(PLATING_DARK);
            b.face(&[
                v3(0.0, -0.7, z),
                v3(MUZZLE_X, -0.7, z),
                v3(MUZZLE_X, 0.7, z),
                v3(0.0, 0.7, z),
            ]);
        });
    }
}

/// The spar through the waterline, the work platform round it, the drum on top, and the
/// training motor on the platform with its shaft down the spar.
fn column(b: &mut MeshBuilder) {
    let sides = if b.fine() { 12 } else { 6 };
    b.paint(PLATING).pattern(pattern::PILE);
    b.prism(
        v3(0.0, 0.0, SPAR_FOOT),
        b.sides(8),
        0.72,
        0.72,
        DRUM[0] - SPAR_FOOT,
    );
    // The platform: a black frame ring with a walkway on it.
    b.paint(ACCENT);
    b.loft_z(
        &ngon(sides, 2.1),
        &[
            Section::new(PLATFORM - 0.25, 0.9),
            Section::new(PLATFORM - 0.05, 1.0),
            Section::new(PLATFORM + 0.05, 1.0),
        ],
    );
    b.paint(PLATING).pattern(pattern::WALKWAY);
    b.loft_z(
        &ngon(sides, 1.98),
        &[
            Section::new(PLATFORM, 1.0),
            Section::new(PLATFORM + 0.1, 1.0),
        ],
    );
    // The drum: a flared base ring, the drum, a slewing ring on its roof.
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, DRUM[0] - 0.15), sides, 1.55, 1.8, 0.15);
    b.paint(PLATING);
    b.prism(v3(0.0, 0.0, DRUM[0]), sides, 1.8, 1.7, DRUM[1] - DRUM[0]);
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, DRUM[1] - 0.12), sides, 1.73, 1.73, 0.08);
    b.prism(v3(0.0, 0.0, DRUM[1]), sides, 0.95, 0.95, 0.06);
    for a in [135.0f32, 315.0] {
        let (s, c) = a.to_radians().sin_cos();
        team_panel(b, v3(1.25 * c, 1.25 * s, DRUM[1]), v2(0.55, 0.55));
    }
    // The training motor on the platform, its shaft run down the spar into the sea.
    b.paint(PLATING);
    b.chamfered_box(v3(0.0, 1.35, PLATFORM + 0.5), v3(0.8, 0.6, 0.8), 0.15);
    b.paint(METAL);
    b.cylinder_between(
        v3(0.0, 0.95, PLATFORM + 0.4),
        v3(0.0, 0.95, TURRET_Z[0] + 0.7),
        0.13,
        0.13,
        b.sides(6),
    );
    if b.fine() {
        b.paint(ACCENT);
        b.block(
            v3(0.42, 1.15, PLATFORM + 0.2),
            v3(0.46, 1.55, PLATFORM + 0.75),
        );
        // A guard rail round the platform.
        b.paint(METAL);
        for k in 0..6 {
            let a = (30.0 + 60.0 * k as f32).to_radians();
            b.cylinder_between(
                v3(1.95 * a.cos(), 1.95 * a.sin(), PLATFORM + 0.1),
                v3(1.95 * a.cos(), 1.95 * a.sin(), PLATFORM + 0.95),
                0.04,
                0.04,
                4,
            );
        }
        b.loft(
            &[
                buoy_ring(12, PLATFORM + 0.9, 1.97),
                buoy_ring(12, PLATFORM + 0.98, 1.97),
                buoy_ring(12, PLATFORM + 0.98, 1.91),
                buoy_ring(12, PLATFORM + 0.9, 1.91),
                buoy_ring(12, PLATFORM + 0.9, 1.97),
            ],
            false,
            false,
        );
    }
}

/// One launcher turret, in its house's frame facing +x about the spar at pivot depth
/// `z`: a collar turning round the spar, a trunnion yoke, the twin tubes out to the
/// muzzles (they kick back on firing), and the reload magazine behind as the
/// counterweight.
fn launcher(b: &mut MeshBuilder, z: f32) {
    let sides = if b.fine() { 12 } else { 6 };
    b.paint(PLATING);
    b.prism(v3(0.0, 0.0, z - 0.53), sides, 1.1, 1.0, 1.06);
    if b.fine() {
        // The turntable under the collar, and the yoke: two cheeks and a trunnion pin.
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, z - 0.65), sides, 1.05, 1.05, 0.12);
        b.paint(PLATING);
        for y in [0.72, -0.72] {
            b.block(v3(0.6, y - 0.12, z - 0.45), v3(1.55, y + 0.12, z + 0.45));
        }
        b.paint(METAL);
        b.cylinder_between(v3(1.2, -0.86, z), v3(1.2, 0.86, z), 0.16, 0.16, 8);
        // The magazine behind the collar.
        b.paint(PLATING_DARK);
        b.chamfered_box(v3(-1.55, 0.0, z), v3(1.1, 1.3, 0.9), 0.2);
    } else {
        b.paint(PLATING_DARK);
        b.block(v3(-2.1, -0.65, z - 0.45), v3(-1.0, 0.65, z + 0.45));
    }
    b.with_recoil(|b| {
        b.paint(PLATING);
        b.block(v3(0.9, -0.62, z - 0.34), v3(1.9, 0.62, z + 0.34));
        for y in [MUZZLE_Y, -MUZZLE_Y] {
            tube(b, v3(0.9, y, z), v3(MUZZLE_X, y, z), 0.26);
        }
        if b.fine() {
            b.paint(ACCENT);
            b.beam(
                v3(2.5, -0.8, z),
                v3(2.5, 0.8, z),
                v2(0.22, 0.66),
                v2(0.22, 0.66),
            );
        }
    });
}

/// The tracker head on the drum's roof: it turns with weapon 0's launcher (it is drawn in
/// that house), its sensor face toward where the launcher points.
fn tracker(b: &mut MeshBuilder) {
    let z = DRUM[1] + 0.06;
    b.paint(PLATING);
    b.chamfered_box(v3(0.15, 0.0, z + 0.22), v3(1.1, 0.72, 0.44), 0.14);
    b.paint(ACCENT);
    b.block(v3(0.7, -0.26, z + 0.08), v3(0.8, 0.26, z + 0.38));
    if b.fine() {
        b.paint(METAL);
        b.block(v3(-0.4, -0.1, z + 0.44), v3(-0.1, 0.1, z + 0.52));
    }
}

/// One outrigger float along the frame's +x: its arm from the platform, the float, and
/// the brace under the water to the spar's foot.
fn outrigger(b: &mut MeshBuilder) {
    let r = FLOAT_R;
    b.paint(PLATING);
    b.beam(
        v3(1.9, 0.0, PLATFORM),
        v3(r - 0.7, 0.0, 1.05),
        v2(0.5, 0.42),
        v2(0.4, 0.36),
    );
    b.paint(PLATING_DARK);
    b.beam(
        v3(r - 0.5, 0.0, -0.9),
        v3(0.6, 0.0, -2.2),
        v2(0.3, 0.3),
        v2(0.3, 0.3),
    );
    let sides = if b.fine() { 12 } else { 6 };
    let ring = |z: f32, radius: f32| -> Vec<Vec3> {
        ngon(sides, radius)
            .into_iter()
            .map(|[x, y]| v3(r + x, y, z))
            .collect()
    };
    b.paint(PLATING).pattern(pattern::HULL);
    b.loft(
        &[
            ring(-1.3, 0.55),
            ring(-0.85, 1.05),
            ring(0.75, 1.1),
            ring(1.0, 0.95),
        ],
        true,
        true,
    );
    b.paint(ACCENT);
    b.loft_z(
        &ngon(sides, 0.98),
        &[
            Section::new(0.94, 1.0).shifted(r, 0.0),
            Section::new(1.08, 1.0).shifted(r, 0.0),
        ],
    );
    team_panel(b, v3(r, 0.0, 1.08), v2(0.95, 0.95));
    if b.fine() {
        b.paint(TREAD);
        b.loft_z(
            &ngon(sides, 1.17),
            &[
                Section::new(0.1, 1.0).shifted(r, 0.0),
                Section::new(0.35, 1.0).shifted(r, 0.0),
            ],
        );
    }
}

/// Tech 2: the second launcher turret under the first, the truss walk between the
/// floats, a lattice mast with the director, and a lowered array off each float.
fn refit(b: &mut MeshBuilder) {
    b.with_house(1, v3(0.0, 0.0, TURRET_Z[1]), TRAVEL, |b| {
        launcher(b, TURRET_Z[1])
    });
    let float_at = |a: f32, r: f32, z: f32| {
        let (s, c) = a.to_radians().sin_cos();
        v3(r * c, r * s, z)
    };
    for k in 0..3 {
        let (a0, a1) = (FLOATS[k], FLOATS[(k + 1) % 3]);
        b.paint(PLATING);
        b.beam(
            float_at(a0, FLOAT_R - 0.6, 1.05),
            float_at(a1, FLOAT_R - 0.6, 1.05),
            v2(0.7, 0.3),
            v2(0.7, 0.3),
        );
        if b.fine() {
            b.paint(METAL);
            b.beam(
                float_at(a0, FLOAT_R - 0.6, 1.8),
                float_at(a1, FLOAT_R - 0.6, 1.8),
                v2(0.07, 0.07),
                v2(0.07, 0.07),
            );
        }
        // A davit on each float lowering an array deep.
        let f = float_at(a0, FLOAT_R + 0.9, 0.0);
        b.paint(PLATING);
        b.beam(
            float_at(a0, FLOAT_R, 1.08),
            f + Vec3::Z * 2.1,
            v2(0.16, 0.16),
            v2(0.12, 0.12),
        );
        b.paint(ACCENT);
        b.cylinder_between(f + Vec3::Z * 2.0, f - Vec3::Z * 10.3, 0.035, 0.035, 4);
        b.paint(PLATING_DARK);
        b.cylinder_between(
            f - Vec3::Z * 10.3,
            f - Vec3::Z * 12.6,
            0.16,
            0.16,
            b.sides(6),
        );
    }
    // The lattice mast out of the drum, the director on it, the mast run up higher.
    let top = 5.4;
    let foot = |k: usize| {
        let a = (90.0 + 120.0 * k as f32).to_radians();
        v3(1.1 * a.cos(), 1.1 * a.sin(), DRUM[1])
    };
    b.paint(PLATING);
    for k in 0..3 {
        b.beam(foot(k), v3(0.0, 0.0, top), v2(0.16, 0.16), v2(0.12, 0.12));
    }
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, top - 0.1), b.sides(8), 0.75, 0.75, 0.16);
    b.paint(PLATING);
    b.chamfered_box(v3(0.1, 0.0, top + 0.5), v3(1.3, 1.0, 0.85), 0.2);
    b.paint(ACCENT);
    b.block(v3(0.72, -0.35, top + 0.25), v3(0.78, 0.35, top + 0.75));
    if b.fine() {
        b.paint(GLASS);
        b.block(v3(-0.2, -0.52, top + 0.6), v3(0.5, 0.52, top + 0.78));
    }
    b.paint(PLATING);
    b.prism(
        v3(0.0, 0.0, MAST[0]),
        b.sides(6),
        0.13,
        0.09,
        MAST[1] - MAST[0],
    );
    b.paint(METAL);
    b.beam(
        v3(0.0, -0.75, 6.5),
        v3(0.0, 0.75, 6.5),
        v2(0.08, 0.08),
        v2(0.08, 0.08),
    );
    beacon(b, v3(0.0, 0.0, MAST[1]));
}
