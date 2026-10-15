//! The commander's forearms: the gun arm on the right, the construction projector and the
//! engineering suites on the left.

use glam::Vec3;

use super::*;
use crate::aster::parts::*;
use crate::builder::MeshBuilder;
use crate::material::*;
use crate::{pattern, rig, TurretRail};

/// Right forearm. Bare, the Vulcan rotary gun sits on the forearm's axis. A main gun takes
/// that place when it goes on (the breech cannon, later rebuilt as the rail cannon) and the
/// Vulcan moves to a pod slung along the outside of the forearm. The main gun kicks back
/// when it fires; the Vulcan's barrels spin up before it fires.
pub(super) fn gun_arm(b: &mut MeshBuilder, gun_y: f32, arm_z: f32) {
    b.with_limb(rig::ARM_GUN, |b| {
        b.at(v3(0.0, gun_y, 0.0), |b| {
            b.paint(ACCENT);
            b.extrude_y_chamfered(
                &[
                    [-1.6, 8.95],
                    [1.6, 8.95],
                    [2.2, 9.35],
                    [2.2, 10.1],
                    [1.6, 10.5],
                    [-1.6, 10.5],
                ],
                0.8,
                0.25,
            );
            b.paint(PLATING);
            b.plate(v3(-0.7, 0.0, 10.5), v2(1.3, 1.05), 0.1, 0.04);
            b.plate(v3(1.15, 0.0, 10.5), v2(1.4, 1.05), 0.1, 0.04);
        });
        if b.fine() {
            b.paint(ACCENT);
            b.block(v3(-1.4, gun_y - 0.4, 10.6), v3(-0.1, gun_y + 0.4, 10.8));
        }

        // The Vulcan in the primary place: three barrels in a spinning clamp, a drum on the side.
        b.until("cannon", |b| {
            let axis = v3(2.2, gun_y, arm_z);
            b.paint(METAL);
            b.cylinder_between(axis, axis + Vec3::X * 0.7, 0.55, 0.5, b.sides(8));
            b.with_spin(axis, |b| {
                vulcan(b, axis + Vec3::X * 0.5, COMMANDER_MUZZLE, 0.26, 0.09)
            });
            b.paint(ACCENT);
            b.cylinder_between(
                v3(0.2, gun_y - 0.8, arm_z - 0.1),
                v3(0.2, gun_y - 1.25, arm_z - 0.1),
                0.62,
                0.62,
                b.sides(10),
            );
            if b.fine() {
                b.paint(PLATING);
                b.cylinder_between(
                    v3(0.2, gun_y - 1.25, arm_z - 0.1),
                    v3(0.2, gun_y - 1.3, arm_z - 0.1),
                    0.5,
                    0.5,
                    10,
                );
                b.paint(METAL);
                b.beam(
                    v3(0.6, gun_y - 0.95, arm_z - 0.1),
                    v3(2.1, gun_y - 0.35, arm_z - 0.05),
                    v2(0.16, 0.24),
                    v2(0.14, 0.2),
                );
            }
        });
        // Moved aside: the same gun, slung in a pod along the outside of the forearm.
        b.module("cannon", 0.1, |b| {
            b.paint(ACCENT);
            b.chamfered_box(
                v3(0.1, SLUNG_AXIS.y + 0.1, SLUNG_AXIS.z),
                v3(1.9, 0.75, 0.8),
                0.2,
            );
            b.paint(PLATING);
            b.block(
                v3(-0.6, SLUNG_AXIS.y - 0.28, SLUNG_AXIS.z + 0.35),
                v3(0.8, SLUNG_AXIS.y + 0.4, SLUNG_AXIS.z + 0.45),
            );
            b.paint(METAL);
            b.beam(
                v3(0.0, SLUNG_AXIS.y + 0.45, SLUNG_AXIS.z),
                v3(0.0, gun_y - 0.78, SLUNG_AXIS.z),
                v2(0.4, 0.3),
                v2(0.4, 0.3),
            );
            b.with_spin(SLUNG_AXIS, |b| {
                vulcan(b, SLUNG_AXIS + Vec3::X * 0.2, SLUNG_MUZZLE, 0.2, 0.07)
            });
        });

        // The breech cannon: a white breech housing on the forearm's nose, the tube in it.
        b.module("cannon", 0.3, |b| {
            b.until("railgun", |b| {
                let breech = v3(1.2, gun_y, arm_z);
                b.set_recoil(breech, COMMANDER_CANNON, 0.55);
                b.paint(PLATING);
                b.at(v3(0.0, gun_y, 0.0), |b| {
                    b.extrude_y_chamfered(
                        &[
                            [1.0, 9.05],
                            [2.5, 9.05],
                            [2.9, 9.4],
                            [2.9, 10.0],
                            [2.5, 10.35],
                            [1.0, 10.35],
                        ],
                        0.62,
                        0.16,
                    )
                });
                b.with_recoil(|b| cannon(b, breech, COMMANDER_CANNON, 0.28, Emitter::Unlit));
                if b.fine() {
                    b.paint(METAL);
                    b.mirror_y(|b| {
                        b.at(v3(0.0, gun_y, 0.0), |b| {
                            b.cylinder_between(
                                v3(1.4, 0.5, 10.25),
                                v3(3.1, 0.3, 10.05),
                                0.1,
                                0.1,
                                6,
                            )
                        })
                    });
                }
            });
        });

        // The rail cannon: a capacitor bank on the forearm feeding a plain rail barrel, the
        // same unlit hardware as the Paladin's (`rail_gun`): a round jacket, ring collars, a
        // rail spine down each side, a flared muzzle.
        b.module("railgun", 0.3, |b| {
            let breech = RAIL_BREECH;
            b.set_recoil(breech, COMMANDER_RAIL, 0.35);
            // Capacitor bank: black, white lid, a row of bare metal cans down each side.
            b.paint(ACCENT);
            b.at(v3(0.0, gun_y, 0.0), |b| {
                b.extrude_y_chamfered(
                    &[
                        [-1.5, 10.5],
                        [1.3, 10.5],
                        [1.7, 10.8],
                        [1.7, 11.25],
                        [-1.3, 11.25],
                        [-1.5, 11.0],
                    ],
                    0.72,
                    0.18,
                )
            });
            b.paint(PLATING);
            b.plate(v3(-0.1, gun_y, 11.25), v2(2.4, 1.1), 0.08, 0.03);
            if b.fine() {
                b.paint(METAL);
                for i in 0..4 {
                    let x = -1.1 + i as f32 * 0.62;
                    b.mirror_y(|b| {
                        b.at(v3(0.0, gun_y, 0.0), |b| {
                            b.block(v3(x, 0.72, 10.65), v3(x + 0.38, 0.75, 11.05))
                        })
                    });
                }
            }
            b.with_recoil(|b| {
                rail_gun(
                    b,
                    breech,
                    COMMANDER_RAIL,
                    RAIL_BARS,
                    RAIL_SLOT,
                    Emitter::Unlit,
                )
            });
        });
    });
}

/// Where the rail cannon's charge crawls (renderer heavy_rail_fx.rs), in unit metres
/// along the bore from the elbow: over the bare rails from the capacitor bank's front
/// (`rail_gun`'s power block ends at 0.3 of the rails) out to the prongs.
pub(crate) const RAIL: TurretRail = {
    const UNIT: f32 = 1.6;
    let length = COMMANDER_RAIL.x - RAIL_BREECH.x;
    let from = RAIL_BREECH.x + length * 0.33;
    let step = (COMMANDER_RAIL.x - from) / 6.0;
    let first = (from + step * 0.5 - GUN_ELBOW.x) * UNIT;
    let s = step * UNIT;
    let slot = if RAIL_SLOT > RAIL_BARS.x * 1.5 {
        RAIL_SLOT
    } else {
        RAIL_BARS.x * 1.5
    };
    TurretRail {
        breech: (RAIL_BREECH.x - GUN_ELBOW.x) * UNIT,
        muzzle: (COMMANDER_RAIL.x - GUN_ELBOW.x) * UNIT,
        rail_y: (slot + RAIL_BARS.x) * 0.5 * UNIT,
        rail_top: (RAIL_BREECH.z + RAIL_BARS.y * 0.5 - GUN_ELBOW.z) * UNIT,
        arcs: [
            first,
            first + s,
            first + 2.0 * s,
            first + 3.0 * s,
            first + 4.0 * s,
            first + 5.0 * s,
        ],
        arc_half: step * 0.5 * UNIT,
    }
};

/// Three barrels round an axis in a clamp: the Vulcan, wherever it is carried.
fn vulcan(b: &mut MeshBuilder, from: Vec3, to: Vec3, ring: f32, barrel: f32) {
    if b.fine() {
        for i in 0..3 {
            let a = i as f32 * std::f32::consts::TAU / 3.0 + 0.5;
            let off = v3(0.0, a.cos() * ring, a.sin() * ring);
            machine_gun(b, from + off, to + off * 0.8, barrel);
        }
        // The clamp: hazard-banded, so the spin reads.
        b.paint(PLATING).pattern(pattern::HAZARD);
        let mid = from.lerp(to, 0.55);
        b.cylinder_between(mid, mid + Vec3::X * 0.22, ring * 1.8, ring * 1.8, 8);
    } else {
        // From further off the three barrels read as one.
        b.paint(METAL);
        b.cylinder_between(from, to, ring * 1.2, ring, 6);
        b.paint(GLOW_ORANGE);
        b.cylinder_between(
            to - Vec3::X * 0.05,
            to + Vec3::X * 0.02,
            ring * 0.55,
            ring * 0.45,
            6,
        );
    }
}

/// Left forearm: the construction projector, and the engineering suites. Each suite is
/// built into the arm (and Suite III down the leg), never onto the back. What builds is
/// amber; the suites' conduit housings run with light while the commander builds.
pub(super) fn tool_arm(b: &mut MeshBuilder, tool_y: f32, arm_z: f32) {
    b.with_limb(rig::ARM_TOOL, |b| {
        let nozzle = COMMANDER_EMITTER - Vec3::X * 0.5;
        b.at(v3(0.0, tool_y, 0.0), |b| {
            b.paint(ACCENT);
            b.extrude_y_chamfered(
                &[
                    [-1.4, 9.0],
                    [1.4, 9.0],
                    [2.0, 9.35],
                    [2.0, 10.05],
                    [1.4, 10.4],
                    [-1.4, 10.4],
                ],
                0.75,
                0.25,
            );
            b.paint(PLATING);
            b.plate(v3(-0.6, 0.0, 10.4), v2(1.2, 0.95), 0.1, 0.04);
            b.plate(v3(1.0, 0.0, 10.4), v2(1.3, 0.95), 0.1, 0.04);
        });
        // Suite III's lance barrel takes the projector's place.
        b.until("eng_3", |b| {
            b.paint(METAL);
            b.cylinder_between(v3(1.8, tool_y, arm_z), nozzle, 0.45, 0.36, b.sides(8));
            b.paint(GLOW_AMBER);
            b.cylinder_between(nozzle, COMMANDER_EMITTER, 0.3, 0.14, 6);
        });
        b.paint(GLOW_AMBER);
        b.cylinder_between(
            v3(2.3, tool_y, arm_z),
            v3(2.5, tool_y, arm_z),
            0.55,
            0.55,
            b.sides(8),
        );
        b.paint(ACCENT).pattern(pattern::CONDUIT);
        b.plate(v3(-0.2, tool_y, 10.4), v2(2.0, 0.26), 0.05, 0.02);
        if b.fine() {
            // Prongs around the construction emitter.
            b.until("eng_3", |b| {
                b.paint(METAL);
                for (dy, dz) in [(0.34, 0.22), (-0.34, 0.22), (0.0, -0.4)] {
                    b.beam(
                        nozzle + v3(-0.5, dy, dz),
                        COMMANDER_EMITTER + v3(-0.05, dy * 0.6, dz * 0.6),
                        v2(0.12, 0.12),
                        v2(0.08, 0.08),
                    );
                }
            });
        }

        // Suite II. Conduit housings along both flanks of the forearm, white-capped, running
        // with light as it builds; a fabricator head of a focusing collar and three long
        // prongs round the projector; a wrist drum under it.
        b.module("eng_2", 0.0, |b| {
            b.mirror_y(|b| {
                b.at(v3(0.0, tool_y, 0.0), |b| {
                    b.paint(ACCENT).pattern(pattern::CONDUIT);
                    b.at(v3(0.0, 0.92, 0.0), |b| {
                        b.extrude_y_chamfered(
                            &[
                                [-1.2, 9.1],
                                [1.3, 9.1],
                                [1.7, 9.45],
                                [1.7, 10.0],
                                [1.3, 10.3],
                                [-1.2, 10.3],
                            ],
                            0.2,
                            0.08,
                        )
                    });
                    b.paint(PLATING);
                    b.block(v3(-1.25, 0.78, 10.3), v3(1.35, 1.14, 10.42));
                    b.block(v3(1.3, 0.78, 9.15), v3(1.75, 1.14, 10.3));
                });
            });
        });
        // The fabricator head twists back and forth while it builds. Suite III takes its place.
        b.module("eng_2", 0.3, |b| {
            b.until("eng_3", |b| {
                b.with_work(rig::WORK_TWIST, |b| {
                    b.paint(PLATING);
                    b.cylinder_between(
                        v3(2.95, tool_y, arm_z),
                        v3(3.4, tool_y, arm_z),
                        0.72,
                        0.66,
                        b.sides(10),
                    );
                    b.paint(GLOW_AMBER);
                    b.cylinder_between(
                        v3(3.4, tool_y, arm_z),
                        v3(3.46, tool_y, arm_z),
                        0.6,
                        0.6,
                        b.sides(10),
                    );
                    for (dy, dz) in [(0.55, 0.32), (-0.55, 0.32), (0.0, -0.64)] {
                        b.paint(METAL);
                        b.beam(
                            v3(3.05, tool_y + dy, arm_z + dz),
                            COMMANDER_EMITTER + v3(-0.12, dy * 0.55, dz * 0.55),
                            v2(0.18, 0.18),
                            v2(0.11, 0.11),
                        );
                        b.paint(GLOW_AMBER);
                        b.beam(
                            COMMANDER_EMITTER + v3(-0.12, dy * 0.55, dz * 0.55),
                            COMMANDER_EMITTER + v3(0.0, dy * 0.5, dz * 0.5),
                            v2(0.12, 0.12),
                            v2(0.08, 0.08),
                        );
                    }
                })
            })
        });
        b.module("eng_2", 0.55, |b| {
            b.paint(ACCENT);
            b.cylinder_between(
                v3(0.6, tool_y - 0.6, 8.75),
                v3(0.6, tool_y + 0.6, 8.75),
                0.44,
                0.44,
                b.sides(10),
            );
            b.paint(PLATING);
            b.cylinder_between(
                v3(0.6, tool_y - 0.66, 8.75),
                v3(0.6, tool_y - 0.6, 8.75),
                0.36,
                0.36,
                b.sides(10),
            );
            b.paint(GLOW_AMBER);
            for dy in [-0.3, 0.3] {
                b.cylinder_between(
                    v3(0.6, tool_y + dy - 0.05, 8.75),
                    v3(0.6, tool_y + dy + 0.05, 8.75),
                    0.48,
                    0.48,
                    b.sides(10),
                );
            }
        });

        // Suite III. Long and lean: a narrow white spine down the top of the forearm, and a
        // lance past the projector, carried in a slim barrel between two fine rails. While it
        // builds the lance runs out, the focusing rings on the barrel twist back and forth,
        // and the prongs round the lance's tip open and close.
        b.module("eng_3", 0.0, |b| {
            b.paint(PLATING);
            b.at(v3(0.0, tool_y, 0.0), |b| {
                b.extrude_y_chamfered(
                    &[
                        [-1.2, 10.4],
                        [2.6, 10.4],
                        [3.4, 10.2],
                        [3.3, 10.5],
                        [2.4, 10.72],
                        [-1.2, 10.72],
                    ],
                    0.4,
                    0.12,
                )
            });
            b.paint(ACCENT).pattern(pattern::CONDUIT);
            b.block(
                v3(-0.9, tool_y - 0.12, 10.72),
                v3(2.3, tool_y + 0.12, 10.78),
            );
        });
        b.module("eng_3", 0.3, |b| {
            let (y, z) = (tool_y, arm_z);
            // The barrel: from the wrist to past the old projector, tapering.
            b.paint(ACCENT);
            b.cylinder_between(v3(2.2, y, z), v3(5.3, y, z), 0.36, 0.28, b.sides(10));
            b.paint(PLATING);
            b.beam(
                v3(2.3, y, z + 0.34),
                v3(4.9, y, z + 0.27),
                v2(0.34, 0.08),
                v2(0.24, 0.07),
            );
            // Rails either side, lit at the ends.
            for dy in [-0.44, 0.44] {
                b.paint(METAL);
                b.cylinder_between(
                    v3(2.0, y + dy, z - 0.05),
                    v3(5.6, y + dy * 0.8, z - 0.05),
                    0.09,
                    0.07,
                    6,
                );
                b.paint(GLOW_AMBER);
                b.cylinder_between(
                    v3(5.6, y + dy * 0.8, z - 0.05),
                    v3(5.85, y + dy * 0.78, z - 0.05),
                    0.07,
                    0.03,
                    6,
                );
            }
            // Focusing rings, each with three lit studs, twisting on the barrel.
            b.with_work(rig::WORK_TWIST, |b| {
                for (i, x) in [3.2f32, 3.85, 4.5].into_iter().enumerate() {
                    let r = 0.5 - i as f32 * 0.05;
                    b.paint(PLATING);
                    b.cylinder_between(v3(x, y, z), v3(x + 0.16, y, z), r, r, b.sides(12));
                    b.paint(GLOW_AMBER);
                    for k in 0..3 {
                        let a = (k as f32 + i as f32 * 0.5) * std::f32::consts::TAU / 3.0;
                        let c = v3(x + 0.08, y + a.cos() * r, z + a.sin() * r);
                        b.block(c - v3(0.07, 0.07, 0.07), c + v3(0.07, 0.07, 0.07));
                    }
                }
            });
            // The lance: a slender rod out of the barrel, amber at the point.
            b.with_work(rig::WORK_EXTEND, |b| {
                b.paint(METAL);
                b.cylinder_between(
                    v3(4.4, y, z),
                    v3(LANCE_TIP - 0.45, y, z),
                    0.19,
                    0.15,
                    b.sides(8),
                );
                b.paint(ACCENT);
                b.cylinder_between(v3(5.45, y, z), v3(5.6, y, z), 0.24, 0.24, b.sides(8));
                b.paint(GLOW_AMBER);
                b.cylinder_between(
                    v3(LANCE_TIP - 0.45, y, z),
                    v3(LANCE_TIP, y, z),
                    0.15,
                    0.04,
                    6,
                );
            });
            // Prongs round the barrel's mouth.
            b.with_work(rig::WORK_BREATHE, |b| {
                for k in 0..3 {
                    let a = (k as f32 + 0.25) * std::f32::consts::TAU / 3.0;
                    let (cy, cz) = (a.cos(), a.sin());
                    b.paint(METAL);
                    b.beam(
                        v3(4.95, y + cy * 0.3, z + cz * 0.3),
                        v3(5.95, y + cy * 0.2, z + cz * 0.2),
                        v2(0.1, 0.1),
                        v2(0.06, 0.06),
                    );
                    b.paint(GLOW_AMBER);
                    b.beam(
                        v3(5.95, y + cy * 0.2, z + cz * 0.2),
                        v3(6.1, y + cy * 0.18, z + cz * 0.18),
                        v2(0.06, 0.06),
                        v2(0.04, 0.04),
                    );
                }
            });
        });
    });
    // Suite III: an armoured sleeve and a lit feed line down the upper arm.
    b.module("eng_3", 0.55, |b| {
        b.paint(ACCENT).pattern(pattern::CONDUIT);
        b.beam(
            v3(0.05, tool_y + 0.5, 11.7),
            v3(-0.25, tool_y + 0.48, 10.3),
            v2(0.95, 0.26),
            v2(0.85, 0.24),
        );
        b.paint(PLATING);
        b.beam(
            v3(0.05, tool_y + 0.66, 11.75),
            v3(-0.1, tool_y + 0.64, 11.0),
            v2(1.0, 0.08),
            v2(0.95, 0.08),
        );
    });
}
