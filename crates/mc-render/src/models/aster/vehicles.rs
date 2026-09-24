//! Aster ground vehicles: wheeled, tracked and hover chassis.
//!
//! Shared anatomy: a dark recessed chassis between the running gear, a white
//! faceted shell over it, and (for armed units and the engineer) a `TURRET`
//! assembly yawing about the model's z axis, because the sim rotates muzzle
//! and emitter offsets about the unit origin.

use glam::Vec3;

use super::parts::*;
use crate::models::builder::{chamfered_rect, MeshBuilder, Section};
use crate::models::material::*;
use crate::models::{part, rig};

// ---- Mason: engineer -------------------------------------------------------
//
// Amphibious work crawler: treads on land, a float skirt on water, a graphite
// hull with white plates, and a two-bone construction arm. The boom is modeled
// level (pitch 0); the sim folds it up at rest, then unfolds and aims the
// forearm at whatever it builds.

/// Where the build beam leaves, as the tech-1 unit file has it.
const ENGINEER_EMITTER: Vec3 = Vec3::new(2.20, 0.0, 1.92);
const ENGINEER_ELBOW: Vec3 = Vec3::new(0.95, 0.0, 1.92);

pub fn engineer(b: &mut MeshBuilder, tech: u8) {
    let deck = tracked_chassis(
        b,
        &Chassis {
            rear: -2.35,
            front: 2.45,
            track: (1.22, 2.08, 0.92),
            split_tracks: false,
            deck: 1.42,
            dark: true,
            lit: tech >= 2,
        },
    );
    float_skirt(b, -2.4, 2.5, 2.12);
    let shoulder = v3(0.0, 0.0, deck.z);
    b.set_turret_pivot(shoulder);
    b.set_arm_boom(ENGINEER_ELBOW);

    if b.coarse() {
        team_panel(b, deck.at(0.22, 0.0), v2(1.1, 1.7));
        b.paint(METAL);
        b.decal(v3(-2.05, -1.05, 3.2), v2(0.18, 0.18));
        b.with_part(part::TURRET, |b| {
            b.with_limb(rig::ARM_BOOM, |b| {
                b.paint(PLATING_DARK);
                b.cuboid_open(
                    (shoulder + ENGINEER_ELBOW) * 0.5,
                    v3((ENGINEER_ELBOW.x - shoulder.x).max(0.4), 0.3, 0.3),
                );
            });
            b.with_limb(rig::ARM_TOOL, |b| {
                b.paint(PLATING);
                b.cuboid_open(
                    (ENGINEER_ELBOW + ENGINEER_EMITTER) * 0.5,
                    v3((ENGINEER_EMITTER.x - ENGINEER_ELBOW.x).max(0.4), 0.22, 0.22),
                );
            });
        });
        return;
    }

    // White armour over the graphite hull: glacis, deck, cab cheeks.
    b.paint(PLATING);
    on_slope(b, [2.45, 0.92], [1.55, deck.z], 0.55, |b| {
        b.plate(Vec3::ZERO, v2(0.95, 1.55), 0.08, 0.03);
    });
    b.plate(deck.at(0.35, 0.0), v2(1.15, 1.55), 0.07, 0.03);
    b.mirror_y(|b| {
        b.plate(v3(0.15, 1.05, deck.z - 0.22), v2(1.35, 0.42), 0.06, 0.03);
    });

    // Cab: dark frame, glass, white roof plate.
    b.paint(PLATING_DARK);
    b.frustum(
        deck.at(0.62, 0.0),
        v2(1.05, 1.42),
        v2(0.52, 0.98),
        0.52,
        v2(-0.18, 0.0),
    );
    b.paint(GLASS);
    b.frustum(
        deck.at(0.64, 0.0) + Vec3::Z * 0.26,
        v2(0.68, 1.05),
        v2(0.36, 0.76),
        0.3,
        v2(-0.12, 0.0),
    );
    b.paint(PLATING);
    b.plate(
        deck.at(0.48, 0.0) + Vec3::Z * 0.52,
        v2(0.72, 0.95),
        0.06,
        0.02,
    );
    team_panel(b, v3(-0.28, 0.0, deck.z), v2(0.8, 1.85));

    // Rear pack: a pair of resource vats. Higher suites add tankage.
    b.mirror_y(|b| {
        b.paint(METAL);
        b.cylinder_between(
            v3(-1.95, 0.55, 1.62),
            v3(-0.85, 0.55, 1.62),
            0.38,
            0.38,
            b.sides(8),
        );
        if b.fine() {
            b.paint(ACCENT);
            b.cylinder_between(v3(-1.65, 0.55, 1.62), v3(-1.52, 0.55, 1.62), 0.43, 0.43, 8);
        }
        b.paint(PLATING);
        b.plate(v3(-1.4, 0.55, 2.0), v2(0.7, 0.42), 0.05, 0.02);
    });

    if tech >= 2 {
        b.paint(PLATING_DARK);
        b.cylinder_between(
            v3(-2.12, 0.0, 1.48),
            v3(-2.12, 0.0, 2.22),
            0.36,
            0.36,
            b.sides(8),
        );
        b.paint(GLOW_AMBER);
        for z in [1.68, 1.95] {
            b.cylinder_between(
                v3(-2.12, 0.0, z),
                v3(-2.12, 0.0, z + 0.1),
                0.4,
                0.4,
                b.sides(8),
            );
        }
        b.paint(PLATING);
        b.plate(v3(-2.12, 0.0, 2.22), v2(0.55, 0.55), 0.05, 0.02);
        if b.fine() {
            b.mirror_y(|b| {
                b.paint(METAL);
                b.beam(
                    v3(-1.85, 0.55, 1.95),
                    v3(-2.05, 0.12, 2.05),
                    v2(0.08, 0.08),
                    v2(0.06, 0.06),
                );
            });
        }
    }
    if tech >= 3 {
        b.paint(PLATING);
        b.prism(v3(-0.55, 0.0, deck.z), 6, 0.36, 0.24, 0.95);
        b.paint(GLOW_AMBER);
        b.prism(v3(-0.55, 0.0, deck.z + 0.95), 6, 0.26, 0.08, 0.36);
        b.mirror_y(|b| {
            b.paint(PLATING_DARK);
            b.cylinder_between(
                v3(-2.05, -0.82, 1.48),
                v3(-2.05, -0.82, 2.18),
                0.3,
                0.3,
                b.sides(8),
            );
            b.paint(GLOW_AMBER);
            b.cylinder_between(
                v3(-2.05, -0.82, 1.78),
                v3(-2.05, -0.82, 1.9),
                0.34,
                0.34,
                b.sides(6),
            );
            b.paint(PLATING);
            b.plate(v3(-2.05, -0.82, 2.18), v2(0.42, 0.42), 0.05, 0.02);
        });
    }

    b.with_part(part::TURRET, |b| {
        // Shoulder ring the boom yaws and pitches on.
        b.paint(ACCENT);
        b.prism(shoulder, b.sides(8), 0.52, 0.52, 0.2);
        b.paint(PLATING_DARK);
        b.chamfered_box(shoulder + Vec3::Z * 0.32, v3(0.85, 0.95, 0.38), 0.18);
        b.paint(PLATING);
        b.plate(shoulder + Vec3::Z * 0.52, v2(0.7, 0.82), 0.05, 0.02);
        b.paint(METAL);
        b.cylinder_between(
            shoulder - Vec3::Y * 0.42,
            shoulder + Vec3::Y * 0.42,
            0.22,
            0.22,
            b.sides(8),
        );

        b.with_limb(rig::ARM_BOOM, |b| {
            // Upper boom: two graphite boxes with a wrist-pin at the elbow.
            b.paint(PLATING_DARK);
            b.beam(
                shoulder + v3(0.12, 0.0, 0.12),
                ENGINEER_ELBOW - Vec3::X * 0.12,
                v2(0.34, 0.4),
                v2(0.3, 0.34),
            );
            b.paint(METAL);
            b.cylinder_between(
                ENGINEER_ELBOW - Vec3::Y * 0.32,
                ENGINEER_ELBOW + Vec3::Y * 0.32,
                0.2,
                0.2,
                b.sides(8),
            );
            if b.fine() {
                b.paint(ACCENT);
                b.beam(
                    shoulder + v3(0.1, 0.16, -0.16),
                    ENGINEER_ELBOW + v3(-0.18, 0.14, -0.2),
                    v2(0.1, 0.1),
                    v2(0.08, 0.08),
                );
                b.paint(PLATING);
                b.plate(
                    v3(
                        (shoulder.x + ENGINEER_ELBOW.x) * 0.5,
                        0.0,
                        (shoulder.z + ENGINEER_ELBOW.z) * 0.5 + 0.2,
                    ),
                    v2(0.55, 0.28),
                    0.04,
                    0.02,
                );
            }
        });

        b.with_limb(rig::ARM_TOOL, |b| {
            let nozzle = ENGINEER_EMITTER - Vec3::X * 0.38;
            b.paint(PLATING_DARK);
            b.beam(
                ENGINEER_ELBOW + Vec3::X * 0.08,
                nozzle,
                v2(0.3, 0.34),
                v2(0.24, 0.26),
            );
            b.paint(PLATING);
            b.plate(
                v3(
                    (ENGINEER_ELBOW.x + nozzle.x) * 0.5,
                    0.0,
                    ENGINEER_ELBOW.z + 0.18,
                ),
                v2(0.85, 0.22),
                0.04,
                0.02,
            );
            b.paint(METAL);
            b.cylinder_between(nozzle - Vec3::X * 0.12, nozzle, 0.22, 0.18, b.sides(8));
            b.paint(GLOW_AMBER);
            b.cylinder_between(nozzle, ENGINEER_EMITTER, 0.14, 0.07, 6);
            b.cylinder_between(
                nozzle - Vec3::X * 0.04,
                nozzle + Vec3::X * 0.06,
                0.26,
                0.26,
                b.sides(8),
            );
            if b.fine() {
                glow_strip(
                    b,
                    v3(
                        (ENGINEER_ELBOW.x + nozzle.x) * 0.5,
                        0.0,
                        ENGINEER_ELBOW.z + 0.22,
                    ),
                    v2(0.9, 0.12),
                    GLOW_AMBER,
                );
                b.paint(METAL);
                for (dy, dz) in [(0.14, 0.1), (-0.14, 0.1), (0.0, -0.16)] {
                    b.beam(
                        nozzle + v3(-0.12, dy, dz),
                        ENGINEER_EMITTER + v3(-0.04, dy * 0.5, dz * 0.5),
                        v2(0.05, 0.05),
                        v2(0.03, 0.03),
                    );
                }
            }

            if tech >= 2 && b.fine() {
                b.paint(METAL);
                b.cylinder_between(
                    nozzle + Vec3::X * 0.1,
                    nozzle + Vec3::X * 0.26,
                    0.3,
                    0.3,
                    b.sides(8),
                );
                for (dy, dz) in [(0.24, 0.14), (-0.24, 0.14), (0.0, -0.26)] {
                    b.paint(METAL);
                    b.beam(
                        nozzle + v3(0.04, dy, dz),
                        ENGINEER_EMITTER + v3(-0.08, dy * 0.5, dz * 0.5),
                        v2(0.07, 0.07),
                        v2(0.04, 0.04),
                    );
                    b.paint(GLOW_AMBER);
                    b.beam(
                        ENGINEER_EMITTER + v3(-0.08, dy * 0.5, dz * 0.5),
                        ENGINEER_EMITTER + v3(0.0, dy * 0.42, dz * 0.42),
                        v2(0.05, 0.05),
                        v2(0.035, 0.035),
                    );
                }
                b.paint(PLATING);
                b.block(
                    v3(ENGINEER_ELBOW.x + 0.12, 0.28, ENGINEER_ELBOW.z - 0.14),
                    v3(ENGINEER_ELBOW.x + 0.78, 0.46, ENGINEER_ELBOW.z + 0.18),
                );
                b.paint(GLOW_AMBER);
                b.block(
                    v3(ENGINEER_ELBOW.x + 0.22, 0.46, ENGINEER_ELBOW.z - 0.02),
                    v3(ENGINEER_ELBOW.x + 0.68, 0.5, ENGINEER_ELBOW.z + 0.08),
                );
            }
            if tech >= 3 && b.fine() {
                for dy in [-0.36, 0.36] {
                    b.paint(METAL);
                    b.cylinder_between(
                        v3(ENGINEER_ELBOW.x + 0.28, dy, ENGINEER_ELBOW.z + 0.06),
                        v3(ENGINEER_EMITTER.x - 0.18, dy, ENGINEER_ELBOW.z + 0.06),
                        0.12,
                        0.09,
                        b.sides(6),
                    );
                    b.paint(GLOW_AMBER);
                    b.cylinder_between(
                        v3(ENGINEER_EMITTER.x - 0.18, dy, ENGINEER_ELBOW.z + 0.06),
                        v3(ENGINEER_EMITTER.x, dy, ENGINEER_ELBOW.z + 0.06),
                        0.08,
                        0.04,
                        6,
                    );
                }
                b.paint(PLATING);
                b.plate(
                    v3(
                        (ENGINEER_ELBOW.x + ENGINEER_EMITTER.x) * 0.5,
                        0.0,
                        ENGINEER_ELBOW.z + 0.32,
                    ),
                    v2(0.95, 0.28),
                    0.05,
                    0.02,
                );
                glow_strip(
                    b,
                    v3(
                        (ENGINEER_ELBOW.x + ENGINEER_EMITTER.x) * 0.5,
                        0.0,
                        ENGINEER_ELBOW.z + 0.38,
                    ),
                    v2(0.85, 0.14),
                    GLOW_AMBER,
                );
            }
        });
    });

    if b.fine() {
        if tech == 1 {
            whip(b, v3(-2.05, -1.05, deck.z), 1.78, 0.12);
        } else {
            antenna(b, v3(-2.05, -1.05, deck.z), 1.78, 0.12);
            antenna(b, v3(-2.05, 1.05, deck.z), 1.25, 0.12);
            b.mirror_y(|b| glow_strip(b, deck.at(0.48, 0.72), v2(1.15, 0.07), GLOW));
        }
        if tech >= 3 {
            b.mirror_y(|b| glow_strip(b, v3(-1.4, 0.55, 2.02), v2(0.85, 0.08), GLOW_AMBER));
        }
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.block(v3(-0.7, 1.35, deck.z), v3(0.28, 1.62, deck.z + 0.18));
            b.paint(PLATING);
            b.plate(v3(1.15, 0.85, deck.z), v2(0.65, 0.38), 0.05, 0.02);
        });
        vent(b, v3(-1.15, 0.0, deck.z), v2(0.62, 0.45), 3, ACCENT);
    } else {
        // Mid LOD has no whip: a bare mast keeps the silhouette up to the blueprint height.
        b.paint(METAL);
        b.cylinder_between(v3(-2.0, -1.0, deck.z), v3(-2.0, -1.0, 3.2), 0.055, 0.03, 4);
    }
}

// ---- Kestrel: scout --------------------------------------------------------

pub fn scout(b: &mut MeshBuilder, _tech: u8) {
    b.set_turret_pivot(v3(0.0, 0.0, 1.25));
    let body = [
        [-2.1, 0.55],
        [1.7, 0.42],
        [2.3, 0.7],
        [0.7, 1.22],
        [-1.5, 1.3],
        [-2.2, 0.95],
    ];
    if b.coarse() {
        b.mirror_y(|b| {
            b.with_part(part::LOCOMOTION, |b| {
                b.paint(TREAD)
                    .cuboid_open(v3(0.0, 1.25, 0.5), v3(3.8, 0.5, 1.0))
            })
        });
        b.paint(PLATING);
        b.frustum_open(
            v3(0.0, 0.0, 0.45),
            v2(4.4, 1.9),
            v2(2.2, 1.3),
            0.85,
            v2(-0.5, 0.0),
        );
        team_panel(b, v3(-1.25, 0.0, 1.3), v2(0.55, 1.2));
        b.with_part(part::TURRET, |b| {
            b.paint(PLATING);
            b.frustum_open(
                v3(0.0, 0.0, 1.25),
                v2(1.2, 0.9),
                v2(0.7, 0.55),
                0.53,
                v2(-0.08, 0.0),
            );
            b.paint(METAL);
            b.beam(
                v3(0.4, 0.0, 1.6),
                v3(1.2, 0.0, 1.6),
                v2(0.2, 0.16),
                v2(0.16, 0.12),
            );
        });
        return;
    }
    b.mirror_y(|b| {
        wheel(b, v3(1.3, 1.25, 0.55), 0.55, 0.5);
        wheel(b, v3(-1.35, 1.25, 0.55), 0.55, 0.5);
    });
    b.paint(PLATING);
    b.extrude_y_chamfered(&body, 0.98, 0.3);
    b.paint(ACCENT);
    b.block(v3(-1.7, -1.05, 0.35), v3(1.7, 1.05, 0.75));
    team_panel(b, v3(-1.25, 0.0, 1.28), v2(0.55, 1.2));

    b.with_part(part::TURRET, |b| {
        b.paint(PLATING);
        b.loft_z(
            &turret_plan(1.3, 1.0),
            &[
                Section::new(1.25, 0.9),
                Section::new(1.45, 1.0),
                Section::scaled(1.78, 0.6, 0.6).shifted(-0.08, 0.0),
            ],
        );
        b.paint(METAL);
        b.beam(
            v3(0.4, 0.0, 1.6),
            v3(1.2, 0.0, 1.6),
            v2(0.2, 0.16),
            v2(0.16, 0.12),
        );
        b.paint(GLOW);
        b.cuboid(v3(1.2, 0.0, 1.6), v3(0.06, 0.1, 0.08));
        if b.fine() {
            b.block(v3(0.55, -0.04, 1.68), v3(1.05, 0.04, 1.71));
        }
    });
    if b.fine() {
        on_slope(b, [2.3, 0.7], [0.7, 1.22], 0.45, |b| {
            b.paint(GLASS);
            b.plate(Vec3::ZERO, v2(0.7, 1.1), 0.06, 0.03);
        });
        on_slope(b, [2.3, 0.7], [0.7, 1.22], 0.88, |b| {
            glow_strip(b, Vec3::ZERO, v2(0.12, 1.2), GLOW)
        });
        antenna(b, v3(-1.9, 0.6, 1.2), 1.0, 0.25);
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.block(v3(-0.5, 0.98, 0.8), v3(0.45, 1.5, 0.95));
        });
    }
}

// ---- Warden: light tank ----------------------------------------------------

/// The tech 1 line tank. Nothing on it is lit: a welded turret, a plain tube
/// gun and the clutter of a vehicle that lives in the field. Emitters and
/// sleek faceted shells belong to the higher tiers.
pub fn tank_light(b: &mut MeshBuilder, _tech: u8) {
    let deck = tracked_chassis(
        b,
        &Chassis {
            rear: -4.2,
            front: 4.3,
            track: (1.8, 3.15, 1.35),
            split_tracks: false,
            deck: 2.1,
            dark: false,
            lit: false,
        },
    );
    let (breech, muzzle) = (v3(1.5, 0.0, 2.82), v3(5.2, 0.0, 2.82));

    b.set_turret_pivot(v3(0.0, 0.0, 2.1));
    b.set_recoil(breech, muzzle, 0.7);
    b.with_part(part::TURRET, |b| {
        // Welded box turret: near-vertical sides, a sloped front, a stowage bustle behind.
        let (z0, z1) = (2.18, 3.22);
        let roof = Roof {
            rear: -1.55,
            front: 0.95,
            half_width: 1.02,
            z: z1,
        };
        b.paint(PLATING);
        if b.coarse() {
            b.frustum_open(
                v3(-0.25, 0.0, 2.1),
                v2(3.7, 2.9),
                v2(2.6, 2.1),
                z1 - 2.1,
                v2(-0.2, 0.0),
            );
        } else {
            let plan = chamfered_rect(v2(2.0, 1.56), 0.58);
            b.loft_z(
                &plan,
                &[
                    Section::new(z0, 0.94).shifted(-0.25, 0.0),
                    Section::new(z0 + 0.38, 1.0).shifted(-0.25, 0.0),
                    Section::new(z1, 0.72).shifted(-0.42, 0.0),
                ],
            );
        }
        b.with_recoil(|b| cannon(b, breech, muzzle, 0.17, Emitter::Unlit));
        team_panel(
            b,
            roof.at(0.2, 0.0),
            v2(roof.length() * 0.34, roof.half_width * 1.5),
        );
        if b.coarse() {
            return;
        }
        // Turret ring and the cast mantlet the gun swings in.
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, 2.06), 8, 1.55, 1.55, 0.14);
        b.chamfered_box(v3(1.62, 0.0, 2.82), v3(0.62, 1.5, 0.84), 0.2);
        if !b.fine() {
            return;
        }
        b.paint(METAL);
        b.cylinder_between(v3(1.9, 0.0, 2.82), v3(2.25, 0.0, 2.82), 0.34, 0.26, 8);
        // Gunner's sight beside the mantlet.
        b.paint(ACCENT);
        b.block(v3(1.2, -0.95, 3.0), v3(1.65, -0.6, 3.3));
        b.paint(GLASS);
        b.block(v3(1.65, -0.9, 3.06), v3(1.68, -0.65, 3.24));

        // Commander's cupola with vision blocks and a split hatch; loader's hatch beside it.
        let cupola = roof.at(0.42, 0.48);
        b.paint(PLATING);
        b.prism(cupola, 8, 0.52, 0.46, 0.24);
        b.paint(ACCENT);
        b.prism(cupola + Vec3::Z * 0.24, 8, 0.38, 0.34, 0.06);
        b.paint(GLASS);
        b.block(cupola + v3(0.4, -0.16, 0.06), cupola + v3(0.5, 0.16, 0.18));
        b.paint(ACCENT);
        b.plate(roof.at(0.5, -0.5), v2(0.7, 0.62), 0.06, 0.03);
        b.paint(METAL);
        b.block(
            roof.at(0.5, -0.5) + v3(-0.42, -0.1, 0.0),
            roof.at(0.5, -0.5) + v3(-0.35, 0.1, 0.1),
        );

        // Pintle machine gun on the cupola.
        let pintle = cupola + v3(0.0, 0.62, 0.0);
        b.paint(METAL);
        b.cylinder_between(pintle, pintle + Vec3::Z * 0.5, 0.05, 0.05, 4);
        b.cylinder_between(
            pintle + v3(-0.3, 0.0, 0.5),
            pintle + v3(0.95, 0.0, 0.56),
            0.05,
            0.035,
            4,
        );
        b.paint(ACCENT);
        b.block(pintle + v3(-0.35, -0.08, 0.42), pintle + v3(0.1, 0.08, 0.6));
        b.block(pintle + v3(-0.2, 0.08, 0.3), pintle + v3(0.1, 0.3, 0.52));

        // Smoke dischargers on the cheeks, stowage basket on the bustle.
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.block(v3(0.45, 1.36, 2.72), v3(1.15, 1.5, 2.84));
            b.paint(METAL);
            for i in 0..3 {
                let base = v3(0.55 + 0.24 * i as f32, 1.45, 2.82);
                b.cylinder_between(base, base + v3(0.2, 0.1, 0.26), 0.085, 0.085, 5);
            }
        });
        b.paint(ACCENT);
        b.block(v3(-2.75, -1.05, 2.4), v3(-2.1, 1.05, 2.46));
        b.mirror_y(|b| b.block(v3(-2.75, 0.99, 2.46), v3(-2.1, 1.05, 2.9)));
        b.block(v3(-2.75, -1.05, 2.84), v3(-2.69, 1.05, 2.9));
        // What the crew keeps in it: a tarp roll and two crates.
        b.paint(CONCRETE);
        b.cylinder_between(v3(-2.42, -0.9, 2.68), v3(-2.42, 0.05, 2.68), 0.2, 0.2, 6);
        b.paint(METAL);
        b.block(v3(-2.62, 0.2, 2.46), v3(-2.2, 0.9, 2.8));
        whip(b, roof.at(0.04, -0.72), 0.95, 0.22);
        // Lifting eyes on the roof corners.
        b.paint(METAL);
        for (u, v) in [(0.95, 0.7), (0.95, -0.7), (0.02, 0.0)] {
            b.cuboid(roof.at(u, v) + Vec3::Z * 0.05, v3(0.16, 0.06, 0.1));
        }
    });

    if b.mid() {
        // Engine deck: a dark louvred plate over the rear of the hull.
        b.paint(ACCENT);
        b.plate(
            deck.at(0.12, 0.0),
            v2(deck.length() * 0.22, deck.half_width * 1.7),
            0.06,
            0.03,
        );
    }
    if !b.fine() {
        return;
    }
    b.mirror_y(|b| {
        vent(
            b,
            deck.at(0.12, 0.48) + Vec3::Z * 0.06,
            v2(1.0, 0.62),
            4,
            METAL,
        )
    });
    // Driver's vision block at the top of the glacis, headlights and tow hooks below it.
    b.paint(ACCENT);
    b.block(
        deck.at(1.0, -0.3) + v3(-0.5, 0.0, 0.0),
        deck.at(1.0, 0.3) + v3(-0.1, 0.0, 0.16),
    );
    b.paint(GLASS);
    b.block(
        deck.at(1.0, -0.24) + v3(-0.1, 0.0, 0.03),
        deck.at(1.0, 0.24) + v3(-0.07, 0.0, 0.13),
    );
    b.mirror_y(|b| {
        b.paint(ACCENT);
        b.cuboid(v3(3.62, 1.2, 1.93), v3(0.34, 0.34, 0.3));
        b.paint(GLASS);
        b.cuboid(v3(3.8, 1.2, 1.93), v3(0.04, 0.24, 0.2));
        b.paint(METAL);
        b.block(v3(4.0, 0.8, 0.95), v3(4.42, 0.98, 1.2));
    });
    // Spare track links bolted to the glacis.
    on_slope(b, [4.3, 1.45], [3.1, 2.1], 0.52, |b| {
        b.paint(TREAD);
        for i in -1..=1 {
            let y = i as f32 * 0.6;
            b.block(v3(-0.26, y - 0.27, 0.0), v3(0.26, y + 0.27, 0.11));
        }
    });

    // Fenders: a stowage locker on the left, pioneer tools on the right, mudguards behind.
    b.paint(PLATING);
    b.block(v3(-2.7, 2.38, 1.35), v3(-0.7, 3.1, 1.78));
    b.paint(ACCENT);
    b.block(v3(-1.78, 2.36, 1.62), v3(-1.62, 3.12, 1.8));
    b.paint(METAL);
    b.cylinder_between(v3(-2.6, -2.62, 1.43), v3(-0.9, -2.62, 1.43), 0.05, 0.05, 4);
    b.block(v3(-0.9, -2.8, 1.36), v3(-0.45, -2.44, 1.42));
    b.cylinder_between(v3(-2.5, -2.92, 1.43), v3(-1.1, -2.92, 1.43), 0.06, 0.06, 4);
    b.mirror_y(|b| {
        b.paint(ACCENT);
        b.block(v3(-4.34, 1.82, 1.35), v3(-3.7, 3.15, 1.41));
        // Exhaust stacks out of the hull rear, each under a heat shield.
        b.paint(METAL);
        b.cylinder_between(v3(-3.35, 1.3, 1.78), v3(-4.3, 1.3, 1.62), 0.15, 0.17, 6);
        b.paint(ACCENT);
        b.block(v3(-4.1, 1.08, 1.8), v3(-3.45, 1.52, 1.86));
    });
    // Two fuel drums strapped across the tail.
    for y in [-1.0, 0.12] {
        b.paint(METAL);
        b.cylinder_between(v3(-4.22, y, 1.2), v3(-4.22, y + 0.88, 1.2), 0.3, 0.3, 8);
        b.paint(ACCENT);
        for k in [0.2, 0.68] {
            b.cylinder_between(
                v3(-4.22, y + k - 0.03, 1.2),
                v3(-4.22, y + k + 0.03, 1.2),
                0.325,
                0.325,
                8,
            );
        }
    }
}

// ---- Ballista: light artillery ---------------------------------------------

pub fn artillery_light(b: &mut MeshBuilder, _tech: u8) {
    let deck = tracked_chassis(
        b,
        &Chassis {
            rear: -3.8,
            front: 3.8,
            track: (1.65, 2.85, 1.2),
            split_tracks: false,
            deck: 1.7,
            dark: false,
            lit: true,
        },
    );

    // Open gun mount: trunnion cheeks, splinter shield, long elevated tube.
    let (breech, muzzle) = (v3(-1.3, 0.0, 1.97), v3(3.8, 0.0, 3.4));
    b.set_turret_pivot(v3(0.0, 0.0, 1.7));
    b.set_arm_pivot(v3(-0.75, 0.0, 2.25));
    b.with_part(part::TURRET, |b| {
        b.with_limb(rig::ARM_GUN, |b| {
            cannon(b, breech, muzzle, 0.22, Emitter::Orange)
        });
        b.paint(PLATING);
        if b.coarse() {
            b.frustum_open(
                v3(-0.7, 0.0, 1.7),
                v2(2.6, 2.4),
                v2(1.6, 1.7),
                1.0,
                v2(-0.2, 0.0),
            );
            team_panel(b, v3(-0.9, 0.0, 2.7), v2(1.6, 0.6));
            return;
        }
        b.mirror_y(|b| {
            b.extrude_y(
                &[
                    [-1.9, 1.7],
                    [0.7, 1.7],
                    [0.3, 2.75],
                    [-1.2, 2.9],
                    [-1.9, 2.4],
                ],
                0.55,
                0.85,
            )
        });
        b.pitched(v3(0.35, 0.0, 1.75), -1.15, |b| {
            b.mirror_y(|b| b.block(v3(-1.35, 0.3, 0.0), v3(0.0, 1.25, 0.12)))
        });
        b.paint(ACCENT);
        b.prism(v3(-0.4, 0.0, 1.62), 8, 1.4, 1.4, 0.16);
        b.block(v3(-2.2, -0.5, 1.75), v3(-0.9, 0.5, 2.45));
        b.cylinder_between(v3(-0.75, -0.9, 2.25), v3(-0.75, 0.9, 2.25), 0.22, 0.22, 6);
        team_panel(b, v3(-1.55, 0.0, 2.45), v2(0.9, 0.8));
        if b.fine() {
            // Recoil cylinders along the tube, breech glow behind.
            b.with_limb(rig::ARM_GUN, |b| {
                b.pitched(
                    breech,
                    (muzzle - breech).z.atan2((muzzle - breech).x),
                    |b| {
                        b.paint(METAL);
                        b.mirror_y(|b| {
                            b.cylinder_between(
                                v3(0.3, 0.3, 0.26),
                                v3(2.0, 0.3, 0.26),
                                0.09,
                                0.09,
                                6,
                            )
                        });
                    },
                );
                b.paint(GLOW_ORANGE);
                b.block(v3(-2.23, -0.35, 2.0), v3(-2.2, 0.35, 2.2));
            });
        }
    });

    if b.fine() {
        // Trail spades folded against the tail, deck vents, ammunition lockers.
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.beam(
                v3(-3.4, 1.0, 1.3),
                v3(-4.3, 1.3, 0.7),
                v2(0.3, 0.3),
                v2(0.3, 0.3),
            );
            b.block(v3(-4.55, 0.95, 0.3), v3(-4.3, 1.65, 0.95));
            b.plate(deck.at(0.1, 0.62), v2(1.0, 0.8), 0.25, 0.08);
        });
        b.mirror_y(|b| vent(b, deck.at(0.8, 0.55), v2(0.9, 0.6), 3, GLOW));
    }
}

// ---- Bulwark: heavy tank ---------------------------------------------------

pub fn tank_heavy(b: &mut MeshBuilder, _tech: u8) {
    let deck = tracked_chassis(
        b,
        &Chassis {
            rear: -5.8,
            front: 5.9,
            track: (2.7, 4.45, 1.75),
            split_tracks: true,
            deck: 2.75,
            dark: false,
            lit: true,
        },
    );
    if b.mid() {
        // Raised engine deck.
        b.paint(PLATING);
        b.frustum(
            deck.at(0.1, 0.0),
            v2(2.2, deck.half_width * 1.8),
            v2(1.6, deck.half_width * 1.5),
            0.4,
            v2(-0.1, 0.0),
        );
        b.paint(ACCENT);
        b.plate(
            deck.at(0.1, 0.0) + Vec3::Z * 0.4,
            v2(1.4, deck.half_width * 1.3),
            0.05,
            0.02,
        );
    }

    let (breech, muzzle) = (v3(2.5, 0.0, 3.8), v3(8.0, 0.0, 3.8));
    b.set_turret_pivot(v3(0.0, 0.0, 2.75));
    b.set_recoil(breech, muzzle, 1.15);
    b.with_part(part::TURRET, |b| {
        b.paint(PLATING);
        let roof = turret_shell(b, 6.2, 5.0, 2.8, 4.45);
        team_panel(
            b,
            roof.at(0.16, 0.0),
            v2(roof.length() * 0.3, roof.half_width * 1.7),
        );
        if b.coarse() {
            b.with_recoil(|b| {
                rail_gun(b, breech, muzzle, v2(0.7, 0.45), 0.5, Emitter::Blue);
            });
            return;
        }
        b.mirror_y(|b| {
            b.with_recoil(|b| {
                rail_gun(
                    b,
                    v3(2.5, 0.6, 3.8),
                    v3(8.0, 0.6, 3.8),
                    v2(0.2, 0.4),
                    0.2,
                    Emitter::Blue,
                );
            });
        });
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, 2.65), 8, 2.3, 2.3, 0.2);
        b.block(v3(2.2, -1.5, 3.3), v3(2.9, 1.5, 4.25));
        // Cheek armour modules.
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.extrude_y(
                &[[-2.2, 3.05], [1.0, 3.05], [0.6, 4.0], [-1.9, 4.0]],
                2.3,
                2.65,
            );
        });
        if b.fine() {
            b.mirror_y(|b| glow_strip(b, roof.at(0.66, 0.86), v2(roof.length() * 0.6, 0.12), GLOW));
            b.mirror_y(|b| {
                b.paint(GLOW);
                b.block(v3(-1.6, 2.65, 3.4), v3(0.2, 2.69, 3.6));
            });
            b.paint(ACCENT);
            b.prism(roof.at(0.55, 0.0), 8, 0.5, 0.42, 0.14);
            b.paint(GLASS);
            b.mirror_y(|b| {
                b.frustum(
                    roof.at(0.88, 0.5),
                    v2(0.6, 0.5),
                    v2(0.35, 0.35),
                    0.3,
                    v2(-0.05, 0.0),
                )
            });
            antenna(b, roof.at(-0.1, -0.5), 0.9, 0.3);
            antenna(b, roof.at(-0.1, 0.5), 0.6, 0.3);
        }
    });

    if b.fine() {
        b.mirror_y(|b| vent(b, deck.at(0.1, 0.4) + Vec3::Z * 0.45, v2(1.2, 0.8), 4, GLOW));
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.plate(deck.at(0.93, 0.6), v2(0.8, 0.9), 0.08, 0.03);
        });
    }
}

// ---- Skimmer: hover tank ---------------------------------------------------
//
// Tech 2 field hovercraft: a rubber skirt and a graphite tub with white plates
// bolted on, not a faceted energy sled. The gun is a machine gun. One plenum
// light under the lip is the earned emitter — the rest is workshop kit.

pub fn hover_tank(b: &mut MeshBuilder, _tech: u8) {
    b.set_hover();
    let (breech, muzzle) = (v3(1.15, 0.0, 2.30), v3(4.15, 0.0, 2.30));

    let skirt = chamfered_rect(v2(4.5, 3.25), 1.1);
    b.with_part(part::LOCOMOTION, |b| {
        b.paint(TREAD);
        if b.coarse() {
            b.frustum_open(
                v3(0.0, 0.0, 0.16),
                v2(8.2, 6.2),
                v2(9.4, 7.2),
                0.48,
                v2(0.0, 0.0),
            );
        } else {
            // Flared bag sitting off the ground so the air gap reads.
            b.loft_z(
                &skirt,
                &[
                    Section::new(0.16, 1.08),
                    Section::new(0.38, 1.0),
                    Section::new(0.58, 0.86),
                ],
            );
            // Cushion ring on the lip — visible looking down, not buried in the bag.
            b.paint(GLOW);
            b.loft_z(
                &skirt,
                &[Section::new(0.16, 1.04), Section::new(0.22, 1.04)],
            );
            if b.fine() {
                b.mirror_y(|b| {
                    b.paint(TREAD);
                    b.block(v3(-3.5, 3.18, 0.18), v3(3.3, 3.52, 0.48));
                });
            }
        }
    });

    b.paint(ACCENT);
    if b.coarse() {
        b.frustum_open(
            v3(0.0, 0.0, 0.78),
            v2(7.6, 5.0),
            v2(5.8, 3.6),
            0.9,
            v2(-0.12, 0.0),
        );
    } else {
        // Graphite tub rides above the bag so the waist is the hover.
        let tub = chamfered_rect(v2(3.85, 2.25), 0.48);
        b.loft_z(
            &tub,
            &[
                Section::new(0.72, 0.95),
                Section::new(1.05, 1.0),
                Section::new(1.50, 0.92),
            ],
        );
        // Cushion collar in the waist — the hover, readable from the RTS camera.
        b.paint(GLOW);
        b.loft_z(&tub, &[Section::new(0.68, 1.04), Section::new(0.76, 1.04)]);
        b.paint(PLATING);
        // Deck plates leave a dark rim; this is bolted armour, not a shell.
        b.plate(v3(0.15, 0.0, 1.50), v2(5.4, 3.35), 0.09, 0.04);
        on_slope(b, [3.85, 1.05], [2.2, 1.50], 0.55, |b| {
            b.plate(Vec3::ZERO, v2(1.55, 2.9), 0.08, 0.03);
        });
        b.mirror_y(|b| {
            b.plate(v3(0.2, 1.95, 1.50), v2(4.6, 0.55), 0.07, 0.03);
        });
    }
    team_panel(b, v3(-1.85, 0.0, 1.59), v2(0.7, 1.45));

    b.set_turret_pivot(v3(0.0, 0.0, 1.66));
    b.with_part(part::TURRET, |b| {
        b.paint(PLATING);
        let (z0, z1) = (1.70, 2.72);
        let roof = Roof {
            rear: -1.28,
            front: 0.98,
            half_width: 0.92,
            z: z1,
        };
        if b.coarse() {
            b.frustum_open(
                v3(-0.15, 0.0, 1.66),
                v2(2.7, 2.2),
                v2(1.9, 1.5),
                z1 - 1.66,
                v2(-0.15, 0.0),
            );
            machine_gun(b, breech, muzzle, 0.1);
            team_panel(b, v3(-0.2, 0.0, z1), v2(0.7, 1.3));
            return;
        }
        // Welded box turret: near-vertical sides, a sloped front, a short bustle.
        let plan = chamfered_rect(v2(1.22, 1.05), 0.32);
        b.loft_z(
            &plan,
            &[
                Section::new(z0, 0.94).shifted(-0.12, 0.0),
                Section::new(z0 + 0.32, 1.0).shifted(-0.12, 0.0),
                Section::new(z1, 0.78).shifted(-0.28, 0.0),
            ],
        );
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, 1.64), 8, 1.35, 1.35, 0.12);
        b.chamfered_box(v3(1.18, 0.0, 2.30), v3(0.55, 1.15, 0.62), 0.14);
        // Twin tubes through the mantlet.
        b.mirror_y(|b| machine_gun(b, breech + Vec3::Y * 0.22, muzzle + Vec3::Y * 0.22, 0.08));
        team_panel(
            b,
            roof.at(0.28, 0.0),
            v2(roof.length() * 0.4, roof.half_width * 1.45),
        );
        if !b.fine() {
            return;
        }
        b.paint(METAL);
        b.cylinder_between(v3(1.35, 0.0, 2.30), v3(1.58, 0.0, 2.30), 0.22, 0.18, 6);
        // Ammo can on the left cheek, feed into the receiver.
        b.paint(ACCENT);
        b.block(v3(-0.05, 0.92, 1.92), v3(0.95, 1.36, 2.38));
        b.paint(METAL);
        b.block(v3(0.72, 0.55, 2.18), v3(1.12, 0.96, 2.32));
        // Gunner's sight and a split hatch.
        b.paint(ACCENT);
        b.block(v3(0.55, -0.88, 2.42), v3(0.95, -0.55, 2.66));
        b.paint(GLASS);
        b.block(v3(0.95, -0.82, 2.46), v3(0.98, -0.6, 2.62));
        b.paint(PLATING);
        b.prism(roof.at(0.42, 0.38), 8, 0.38, 0.34, 0.16);
        b.paint(ACCENT);
        b.prism(roof.at(0.42, 0.38) + Vec3::Z * 0.16, 8, 0.28, 0.24, 0.05);
        b.plate(roof.at(0.48, -0.42), v2(0.55, 0.5), 0.05, 0.02);
        whip(b, roof.at(0.08, -0.62), 0.72, 0.2);
        b.paint(METAL);
        for (u, v) in [(0.92, 0.62), (0.92, -0.62)] {
            b.cuboid(roof.at(u, v) + Vec3::Z * 0.04, v3(0.14, 0.05, 0.08));
        }
    });

    if !b.fine() {
        return;
    }
    // Recessed engine deck and dark lift intakes — fans, not glowing nacelles.
    b.paint(ACCENT);
    b.plate(v3(-2.85, 0.0, 1.59), v2(1.7, 2.4), 0.05, 0.02);
    b.mirror_y(|b| vent(b, v3(-2.85, 0.85, 1.64), v2(1.1, 0.62), 3, METAL));
    // Driver's hatch on the glacis, exhaust and a crate on the tail.
    b.paint(PLATING);
    b.prism(v3(2.15, 0.55, 1.59), 8, 0.32, 0.28, 0.1);
    b.paint(ACCENT);
    b.plate(v3(2.15, 0.55, 1.69), v2(0.28, 0.24), 0.04, 0.015);
    b.mirror_y(|b| {
        b.paint(METAL);
        b.cylinder_between(v3(-3.55, 1.05, 1.28), v3(-4.05, 1.05, 1.14), 0.12, 0.14, 6);
        b.paint(ACCENT);
        b.block(v3(-3.95, 0.88, 1.28), v3(-3.4, 1.22, 1.34));
    });
    b.paint(METAL);
    b.block(v3(-3.55, -0.5, 1.59), v3(-2.9, 0.5, 1.98));
    b.mirror_y(|b| {
        b.paint(ACCENT);
        b.cuboid(v3(3.35, 1.35, 1.22), v3(0.22, 0.26, 0.18));
        b.paint(GLASS);
        b.cuboid(v3(3.46, 1.35, 1.22), v3(0.04, 0.18, 0.12));
    });
}

// ---- Javelin: missile launcher ---------------------------------------------

pub fn missile_launcher(b: &mut MeshBuilder, _tech: u8) {
    let deck = tracked_chassis(
        b,
        &Chassis {
            rear: -4.5,
            front: 4.6,
            track: (1.95, 3.25, 1.32),
            split_tracks: false,
            deck: 1.85,
            dark: false,
            lit: true,
        },
    );
    // Turntable on the hull origin — the sim yaws the turret about that axis.
    // The rack pitches from a hinge on the ring, so it stays seated when it turns.
    let (pivot, length, thickness, half_width) = (v3(-1.4, 0.0, 2.0), 3.8, 1.2, 1.9);
    b.set_turret_pivot(v3(0.0, 0.0, deck.z));
    b.set_arm_pivot(pivot);
    if b.coarse() {
        b.with_part(part::TURRET, |b| {
            b.with_limb(rig::ARM_GUN, |b| {
                b.pitched(pivot, 0.0, |b| {
                    b.cuboid(
                        v3(length * 0.5, 0.0, thickness * 0.5),
                        v3(length, half_width * 2.0, thickness),
                    );
                    team_panel(b, v3(length * 0.3, 0.0, thickness), v2(0.8, 3.0));
                });
            });
        });
        return;
    }
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, deck.z), 8, 2.2, 2.2, 0.1);

    b.with_part(part::TURRET, |b| {
        b.paint(PLATING);
        b.prism(v3(0.0, 0.0, deck.z + 0.08), 8, 2.05, 2.05, 0.2);
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.block(v3(-1.75, 1.32, deck.z + 0.12), v3(-0.85, 1.82, 2.28));
            b.paint(METAL);
            b.cylinder_between(
                v3(0.55, 1.5, deck.z + 0.16),
                v3(-1.0, 1.55, 2.12),
                0.11,
                0.09,
                6,
            );
        });
        if b.fine() {
            b.paint(ACCENT);
            b.prism(v3(0.15, 0.0, deck.z + 0.26), 6, 0.55, 0.48, 0.2);
            b.paint(GLASS);
            b.spheroid(v3(0.15, 0.0, 2.22), v3(0.22, 0.22, 0.12), 4, 2);
            b.mirror_y(|b| glow_strip(b, v3(0.0, 1.55, 2.28), v2(0.7, 0.08), GLOW));
            antenna(b, v3(-0.35, -1.35, 2.28), 0.95, 0.12);
            antenna(b, v3(-0.55, 1.2, 2.28), 0.65, 0.16);
        }
        b.with_limb(rig::ARM_GUN, |b| {
            b.pitched(pivot, 0.0, |b| {
                b.paint(PLATING);
                b.extrude_y_chamfered(
                    &[
                        [0.0, 0.0],
                        [length, 0.0],
                        [length, thickness],
                        [0.0, thickness],
                    ],
                    half_width,
                    0.16,
                );
                // Fire-control box on the magazine, then the six cells.
                b.paint(ACCENT);
                b.block(
                    v3(length * 0.12, -1.15, thickness),
                    v3(length * 0.42, 1.15, thickness + 0.28),
                );
                team_panel(b, v3(length * 0.27, 0.0, thickness + 0.28), v2(0.55, 2.0));
                for row in 0..2 {
                    for col in 0..3 {
                        let (y, z) = ((col as f32 - 1.0) * 1.12, 0.32 + row as f32 * 0.56);
                        let sides = b.sides(6);
                        b.paint(METAL);
                        b.cylinder_between(v3(0.2, y, z), v3(length, y, z), 0.23, 0.23, sides);
                        b.paint(ACCENT);
                        b.cylinder_between(
                            v3(length - 0.06, y, z),
                            v3(length + 0.05, y, z),
                            0.28,
                            0.28,
                            sides,
                        );
                        b.paint(GLOW_ORANGE);
                        b.cylinder_between(
                            v3(length + 0.05, y, z),
                            v3(length + 0.09, y, z),
                            0.15,
                            0.11,
                            sides,
                        );
                    }
                }
                if b.fine() {
                    b.paint(ACCENT);
                    for col in 0..3 {
                        let y = (col as f32 - 1.0) * 1.12;
                        b.block(
                            v3(length * 0.5, y - 0.03, thickness),
                            v3(length - 0.08, y + 0.03, thickness + 0.05),
                        );
                    }
                    // Exhaust grille at the breech.
                    b.block(v3(-0.06, -1.45, 0.18), v3(0.0, 1.45, 1.02));
                    b.paint(GLOW_ORANGE);
                    b.block(v3(-0.08, -0.7, 0.4), v3(-0.06, 0.7, 0.8));
                    glow_strip(
                        b,
                        v3(length * 0.27, 0.0, thickness + 0.28),
                        v2(0.7, 0.1),
                        GLOW,
                    );
                }
            });
        });
    });
    // Side electronics pods over the track sponsons.
    b.mirror_y(|b| {
        b.paint(PLATING);
        b.block(v3(-2.2, 2.05, 1.35), v3(1.4, 2.42, 2.15));
        if b.fine() {
            vent(b, v3(-0.4, 2.24, 2.15), v2(1.3, 0.48), 3, GLOW);
        }
    });
}

// ---- Trebuchet: heavy artillery --------------------------------------------
//
// A carriage, not a tank. Split tracks hang off a narrow lozenge spine; a
// tall A-frame holds the rail; a hanging mass sits behind the trunnion.
// From the RTS camera it is a triangle and a throwing arm, not a plated
// rectangle.

pub fn artillery_heavy(b: &mut MeshBuilder, _tech: u8) {
    let (rear, front) = (-5.15, 4.4);
    let (inner, outer, track_h) = (1.92, 4.72, 1.50);
    let deck_z = 1.88;
    let mid = (rear + front) * 0.5;
    b.set_treads((inner + outer) * 0.5, outer - inner, rear);

    if !b.coarse() {
        b.mirror_y(|b| {
            track(b, rear - 0.08, mid - 0.28, inner, outer, track_h);
            track(b, mid + 0.28, front - 0.04, inner, outer, track_h);
            // Team flash on the front bogie, same read as the other tracked hulls.
            let fender = v2((front - mid) * 0.42, (outer - inner) * 0.92);
            let at = v3(front - 0.55 * (front - mid), (inner + outer) * 0.5, track_h);
            b.paint(TEAM);
            if b.fine() {
                b.plate(at, fender, 0.1, 0.05);
            } else {
                b.decal(at + Vec3::Z * 0.06, fender);
            }
        });
    }

    // Breech sits on the trunnion, not a long tail behind it: when the tube
    // pitches up that tail would swing through the ring and the hull.
    let (breech, muzzle) = (v3(-1.05, 0.0, 4.32), v3(8.5, 0.0, 5.2));
    let pivot = v3(-0.8, 0.0, 4.40);
    let elevation = (muzzle - breech).z.atan2((muzzle - breech).x);
    b.set_turret_pivot(v3(0.0, 0.0, deck_z));
    b.set_arm_pivot(pivot);
    b.set_recoil(breech, muzzle, 2.2);
    // Bearing on the hull, turntable on the turret: the A-frame stands on
    // the ring, so a yaw is a race turning, not feet sliding on the deck.
    let ring_top = deck_z + 0.40;

    if b.coarse() {
        b.mirror_y(|b| {
            b.with_part(part::LOCOMOTION, |b| {
                b.paint(TREAD);
                b.cuboid_open(
                    v3(-0.2, (inner + outer) * 0.5, track_h * 0.5),
                    v3(8.8, outer - inner, track_h),
                );
            });
        });
        b.with_part(part::TURRET, |b| {
            b.with_limb(rig::ARM_GUN, |b| {
                b.with_recoil(|b| rail_gun(b, breech, muzzle, v2(0.4, 0.7), 0.36, Emitter::Blue));
            });
            b.paint(ACCENT);
            b.prism(v3(0.0, 0.0, deck_z), 4, 2.0, 1.9, 0.22);
            b.paint(PLATING);
            b.frustum_open(
                v3(-0.5, 0.0, ring_top),
                v2(3.4, 2.0),
                v2(0.65, 0.5),
                3.95,
                v2(-0.35, 0.0),
            );
            team_panel(b, v3(-1.0, 0.0, 5.85), v2(1.3, 0.5));
        });
        return;
    }

    // Graphite spine: a pointed lozenge, treads tucked against its sides.
    let plan = hull_plan(rear + 0.1, front - 0.2, 1.82, 1.05);
    b.paint(ACCENT);
    b.loft_z(
        &plan,
        &[
            Section::new(0.72, 0.92),
            Section::new(1.28, 1.0),
            Section::scaled(deck_z, 0.78, 0.68).shifted(-0.18, 0.0),
        ],
    );
    b.paint(PLATING);
    b.plate(v3(0.35, 0.0, deck_z), v2(3.6, 1.85), 0.07, 0.03);
    b.plate(v3(-2.7, 0.0, deck_z), v2(2.3, 1.65), 0.07, 0.03);
    team_panel(b, v3(-0.15, 0.0, deck_z), v2(1.15, 1.9));
    // Short sponsons: the bogies sit on the hull, not at the end of a beam.
    b.mirror_y(|b| {
        b.paint(ACCENT);
        b.block(v3(-3.4, 1.55, 0.95), v3(2.9, 2.02, 1.72));
        b.paint(PLATING);
        let pad = v2(1.55, (outer - inner) * 0.48);
        b.plate(
            v3(1.55, (inner + outer) * 0.5, track_h + 0.02),
            pad,
            0.06,
            0.02,
        );
        b.plate(
            v3(-2.15, (inner + outer) * 0.5, track_h + 0.02),
            pad,
            0.06,
            0.02,
        );
    });
    // Tail plate the recoil ramp hinges on — hull, not deploy, so it stays put.
    b.paint(ACCENT);
    b.extrude_y(
        &[[-4.55, 0.78], [-5.2, 0.72], [-5.2, 1.72], [-4.45, 1.88]],
        -1.55,
        1.55,
    );
    // Hull race: stays put while the turret turns on it.
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, deck_z), b.sides(8), 2.08, 2.02, 0.12);
    b.paint(PLATING);
    b.prism(v3(0.0, 0.0, deck_z + 0.12), b.sides(8), 1.96, 1.90, 0.10);

    b.with_part(part::TURRET, |b| {
        b.with_limb(rig::ARM_GUN, |b| {
            b.with_recoil(|b| rail_gun(b, breech, muzzle, v2(0.4, 0.7), 0.36, Emitter::Blue));
            // Compact mass just behind the trunnion, high enough that a steep
            // loft still clears the ring.
            siege_counterweight(b, v3(-1.55, 0.0, 3.85), v3(1.05, 1.35, 1.00));
            b.paint(METAL);
            b.mirror_y(|b| {
                b.beam(
                    v3(-0.95, 0.42, 4.28),
                    v3(-1.45, 0.52, 4.25),
                    v2(0.14, 0.12),
                    v2(0.18, 0.14),
                );
            });
        });
        // Turntable the arches stand on — yaws with the gun.
        b.paint(PLATING);
        b.prism(v3(0.0, 0.0, deck_z + 0.22), b.sides(8), 1.84, 1.78, 0.18);
        team_panel(b, v3(0.0, 0.0, ring_top), v2(1.35, 1.35));
        siege_a_frame(b, ring_top, pivot, 1.32, 1.22, -1.48, -0.55, 6.05, 1.0);
        if b.fine() {
            // Capacitor banks along the rails: the tech-3 light show.
            b.with_limb(rig::ARM_GUN, |b| {
                b.with_recoil(|b| {
                    b.pitched(breech, elevation, |b| {
                        b.mirror_y(|b| {
                            for i in 0..4 {
                                let x = 3.6 + 1.45 * i as f32;
                                b.paint(ACCENT);
                                b.block(v3(x, 0.56, -0.24), v3(x + 0.85, 0.86, 0.24));
                                b.paint(GLOW);
                                b.block(v3(x + 0.14, 0.86, -0.12), v3(x + 0.71, 0.90, 0.12));
                            }
                        });
                    });
                });
            });
            b.mirror_y(|b| {
                glow_strip(b, v3(-0.55, 1.15, 6.16), v2(0.5, 0.10), GLOW);
            });
            // On the left rear strut, below the peak cap.
            antenna(b, v3(-1.22, -1.48, 3.85), 0.78, 0.12);
        }
    });

    // Rear recoil spade and side outriggers: authored planted, folded by the shader.
    b.with_deploy(|b| {
        b.paint(ACCENT);
        b.extrude_y(
            &[
                [-5.2, 1.72],
                [-6.55, 0.85],
                [-8.15, 0.70],
                [-8.15, 0.08],
                [-7.35, 0.08],
                [-5.95, 1.08],
                [-5.2, 0.72],
            ],
            -1.55,
            1.55,
        );
        b.mirror_y(|b| {
            // Shoulder on the outer bogie, then a long boom out and aft to a foot pad.
            b.paint(ACCENT);
            b.block(v3(-1.4, 4.48, 0.98), v3(0.6, 4.95, 1.82));
            b.paint(PLATING);
            b.beam(
                v3(-0.3, 4.72, 1.38),
                v3(-1.8, 8.05, 0.48),
                v2(0.68, 0.54),
                v2(0.72, 0.32),
            );
            b.paint(ACCENT);
            b.beam(
                v3(-1.4, 4.72, 1.18),
                v3(-2.1, 7.85, 0.52),
                v2(0.32, 0.24),
                v2(0.34, 0.18),
            );
            b.plate(v3(-1.85, 8.05, 0.16), v2(1.15, 0.95), 0.22, 0.05);
            b.paint(TEAM);
            b.plate(v3(-1.85, 8.05, 0.32), v2(0.7, 0.55), 0.04, 0.02);
        });
        if b.fine() {
            b.mirror_y(|b| {
                b.paint(METAL);
                b.cylinder_between(v3(-0.55, 4.80, 1.42), v3(-1.55, 7.90, 0.50), 0.09, 0.07, 5);
                b.paint(ACCENT);
                b.block(v3(-2.25, 7.80, 0.26), v3(-1.45, 8.30, 0.50));
            });
            b.paint(METAL);
            b.mirror_y(|b| {
                b.cylinder_between(v3(-5.35, 0.7, 1.45), v3(-7.45, 1.25, 0.50), 0.10, 0.08, 5);
            });
        }
    });
    if b.fine() {
        vent(b, v3(1.7, 0.0, deck_z), v2(1.2, 0.7), 3, GLOW);
        b.mirror_y(|b| glow_strip(b, v3(-2.2, 0.72, deck_z), v2(0.8, 0.11), GLOW));
        b.paint(GLOW);
        b.plate(v3(2.85, 0.0, deck_z), v2(0.08, 1.05), 0.04, 0.02);
    }
}
