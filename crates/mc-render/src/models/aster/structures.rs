//! Aster structures. Each sits on a ground foundation inside its build-grid
//! footprint (12 m cells) and opens or faces toward +x.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

use glam::Vec3;

use super::parts::*;
use crate::models::builder::{chamfered_rect, ngon, MeshBuilder, Section};
use crate::models::material::*;
use crate::models::part;

// ---- Forge: land factory -----------------------------------------------------

/// Authored heights per tech: 22, 26, 30 m.
pub fn factory_land(b: &mut MeshBuilder, tech: u8) {
    let tier = (tech - 1) as f32;
    let tower_top = 22.0 + 4.0 * tier;
    // Cross-section (y, z) of the two side halls flanking the build bay.
    let hall = [
        [25.0, 0.0],
        [44.0, 0.0],
        [44.0, 6.5],
        [40.5, 12.0],
        [30.0, 12.0],
        [25.0, 8.5],
    ];

    if b.coarse() {
        b.paint(PLATING);
        b.mirror_y(|b| {
            b.frustum_open(
                v3(-1.0, 34.5, 0.0),
                v2(84.0, 19.0),
                v2(80.0, 10.0),
                12.0,
                v2(0.0, 0.5),
            )
        });
        b.frustum_open(
            v3(-32.0, 0.0, 0.0),
            v2(24.0, 52.0),
            v2(14.0, 46.0),
            16.0,
            v2(-1.0, 0.0),
        );
        b.frustum_open(
            v3(-33.0, -11.0, 16.0),
            v2(10.0, 12.0),
            v2(7.0, 9.0),
            tower_top - 16.0,
            v2(0.0, 0.0),
        );
        b.paint(ACCENT);
        b.beam(
            v3(4.0, -31.0, 14.5),
            v3(4.0, 31.0, 14.5),
            v2(4.0, 2.5),
            v2(4.0, 2.5),
        );
        b.mirror_y(|b| team_panel(b, v3(24.0, 35.2, 12.0), v2(10.0, 9.0)));
        return;
    }

    // Build pad with its guide ring.
    b.paint(METAL);
    b.prism(v3(0.0, 0.0, 0.0), b.sides(16), 21.0, 20.4, 0.5);
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, 0.5), b.sides(16), 14.0, 13.6, 0.12);

    b.mirror_y(|b| {
        b.paint(PLATING);
        // Hall body with a tapered prow toward the exit.
        let ring = |x: f32, s: f32| {
            hall.iter()
                .map(|p| v3(x, 34.5 + (p[0] - 34.5) * s, p[1] * s))
                .collect::<Vec<_>>()
        };
        b.loft(
            &[
                ring(-43.0, 0.8),
                ring(-38.0, 1.0),
                ring(34.0, 1.0),
                ring(43.0, 0.55),
            ],
            true,
            true,
        );
        team_panel(b, v3(24.0, 35.2, 12.0), v2(10.0, 9.0));
        // Crane rail along the inner roof edge.
        b.paint(METAL);
        b.block(v3(-20.0, 30.2, 12.0), v3(32.0, 31.8, 12.9));
    });

    // Rear assembly block with the parts-feed door.
    b.paint(PLATING);
    b.extrude_y_chamfered(
        &[
            [-44.0, 0.0],
            [-20.0, 0.0],
            [-20.0, 10.0],
            [-24.5, 16.0],
            [-38.0, 16.0],
            [-44.0, 10.5],
        ],
        26.0,
        3.0,
    );
    b.paint(ACCENT);
    b.block(v3(-20.0, -12.0, 0.0), v3(-19.4, 12.0, 8.2));
    // Control tower, taller with tech.
    b.paint(PLATING);
    b.at(v3(-33.0, -11.0, 0.0), |b| {
        b.loft_z(
            &chamfered_rect(v2(5.0, 6.0), 1.6),
            &[
                Section::new(16.0, 1.0),
                Section::new(tower_top - 3.0, 0.9),
                Section::scaled(tower_top, 0.62, 0.7).shifted(-0.6, 0.0),
            ],
        );
        b.paint(GLASS);
        b.loft_z(
            &chamfered_rect(v2(5.0, 6.0), 1.6),
            &[
                Section::new(tower_top - 5.2, 0.97),
                Section::new(tower_top - 3.8, 0.95),
            ],
        );
    });

    // Gantry cranes over the bay: one per tech level.
    let gantry = |b: &mut MeshBuilder, x: f32, head_y: f32| {
        b.paint(ACCENT);
        b.mirror_y(|b| {
            b.beam(
                v3(x, 31.0, 12.9),
                v3(x, 31.0, 15.6),
                v2(2.4, 3.4),
                v2(1.8, 3.0),
            )
        });
        b.paint(PLATING);
        b.beam(
            v3(x, -32.0, 15.0),
            v3(x, 32.0, 15.0),
            v2(3.0, 2.4),
            v2(3.0, 2.4),
        );
        // Travelling build head.
        b.paint(ACCENT);
        b.chamfered_box(v3(x, head_y, 13.2), v3(4.4, 5.0, 1.6), 1.0);
        b.paint(GLOW);
        b.prism(v3(x, head_y, 10.9), 6, 0.5, 1.5, 1.5);
        if b.fine() {
            b.paint(GLOW);
            b.block(v3(x + 1.5, -24.0, 14.4), v3(x + 1.56, 24.0, 15.0));
            b.paint(METAL);
            b.beam(
                v3(x, head_y + 1.6, 12.4),
                v3(x + 2.6, head_y + 3.4, 9.6),
                v2(0.5, 0.5),
                v2(0.3, 0.3),
            );
            b.beam(
                v3(x, head_y - 1.6, 12.4),
                v3(x + 2.6, head_y - 3.4, 9.6),
                v2(0.5, 0.5),
                v2(0.3, 0.3),
            );
        }
    };
    gantry(b, 4.0, -5.0);
    if tech >= 2 {
        gantry(b, -10.0, 8.0);
    }
    if tech >= 3 {
        gantry(b, 18.0, 3.0);
    }

    // Exit apron.
    b.paint(PLATING);
    b.frustum(
        v3(41.0, 0.0, 0.8),
        v2(10.0, 44.0),
        v2(7.0, 40.0),
        0.5,
        v2(-1.0, 0.0),
    );

    // Tech modules: reactor annex at 2, sensor spire and hall stacks at 3.
    if tech >= 2 {
        b.paint(PLATING);
        b.at(v3(-32.0, 12.0, 0.0), |b| {
            b.loft_z(
                &ngon(8, 6.5),
                &[
                    Section::new(16.0, 1.0),
                    Section::new(19.0, 0.9),
                    Section::new(20.5, 0.55),
                ],
            )
        });
        b.paint(GLOW);
        b.prism(v3(-32.0, 12.0, 20.5), 8, 2.6, 2.0, 0.5);
    }
    if tech >= 3 {
        b.paint(ACCENT);
        b.prism(v3(-33.6, -11.0, tower_top), 6, 1.2, 0.5, 3.0);
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.at(v3(-30.0, 35.2, 0.0), |b| {
                b.loft_z(
                    &chamfered_rect(v2(5.0, 4.0), 1.2),
                    &[
                        Section::new(12.0, 1.0),
                        Section::new(15.0, 0.9),
                        Section::new(16.0, 0.6),
                    ],
                )
            });
            glow_strip(b, v3(-30.0, 35.2, 16.0), v2(4.0, 2.6), GLOW);
        });
    }

    if b.fine() {
        // Roof plating with dark seams, light bars along the bay walls, vents.
        b.mirror_y(|b| {
            for i in 0..4 {
                let x = -34.0 + 13.0 * i as f32;
                b.paint(PLATING);
                b.plate(v3(x + 5.5, 35.2, 12.0), v2(11.0, 9.4), 0.35, 0.2);
            }
            b.paint(GLOW);
            b.block(v3(-18.0, 24.7, 5.0), v3(30.0, 25.0, 5.7));
            for i in 0..tech as usize + 1 {
                vent(
                    b,
                    v3(-10.0 + 9.0 * i as f32, 35.2, 12.35),
                    v2(5.0, 4.0),
                    4,
                    GLOW,
                );
            }
            // Pad guide lights leading out of the bay.
            for i in 0..5 {
                glow_strip(
                    b,
                    v3(24.0 + 4.5 * i as f32, 17.0, if i < 4 { 0.8 } else { 1.3 }),
                    v2(2.2, 0.7),
                    GLOW,
                );
            }
        });
        b.paint(GLOW);
        b.block(v3(-19.4, -12.0, 8.2), v3(-19.2, 12.0, 8.8));
        b.radial(8, |b| glow_strip(b, v3(17.2, 0.0, 1.3), v2(1.2, 4.2), GLOW));
        // Rear block roof: vents, team flash, antenna farm.
        team_panel(b, v3(-31.0, 0.0, 16.0), v2(9.0, 8.0));
        b.mirror_y(|b| vent(b, v3(-31.0, 19.0, 16.0), v2(8.0, 5.0), 5, GLOW));
        antenna(b, v3(-36.0, -14.5, tower_top - 0.4), 5.0, 0.0);
        if tech >= 2 {
            antenna(b, v3(-30.5, -14.5, tower_top - 0.4), 3.5, 0.0);
        }
    }
}

// ---- Mass extractor ------------------------------------------------------------

/// Emits `f` if extractor kit `tier` is fitted at `tech`, as an upgrade piece
/// going up `at` of the way through the refit if it is the next one (full
/// detail only: a refit is watched from close by), and not at all beyond that.
fn kit(b: &mut MeshBuilder, tech: u8, tier: u8, at: f32, f: impl FnOnce(&mut MeshBuilder)) {
    if tier <= tech {
        f(b);
    } else if tier == tech + 1 && b.fine() {
        b.upgrade(at, f);
    }
}

/// A wellhead around an open bore: the deposit's cracks stay visible, and the
/// machine mines them with intake bells that sweep the fissures, not a drill.
/// The 2x2 lot is four build cells meeting at the deposit; each cell holds one
/// module, square to the grid, so the well at the vertex stays open.
/// Tech 1 is two vats, the pump and the probes. Tech 2 fills the last cell and
/// cages the well. Tech 3 adds condensers and a gantry that still leaves the
/// bore open. Authored heights: 9, 11, 13 m.
pub fn extractor(b: &mut MeshBuilder, tech: u8) {
    // Authored 16 m cell; scales with the 12 m build grid. Centre of each cell
    // of the 2x2, so the four modules sit on the grid rather than on a ring.
    const CELL: f32 = 8.0;
    // The probes orbit the bore; their bells hang just above the cracked earth.
    b.set_spinner_pivot(v3(0.0, 0.0, 2.4));

    if b.coarse() {
        let stack = 8.4 + 2.2 * (tech - 1) as f32;
        b.paint(PLATING);
        b.radial(2, |b| b.cuboid_open(v3(CELL, CELL, 1.5), v3(5.2, 5.2, 3.0)));
        b.cuboid_open(v3(-CELL, CELL, stack * 0.5), v3(3.6, 3.6, stack));
        team_panel(b, v3(-CELL, CELL, stack), v2(2.6, 2.6));
        b.with_part(part::SPINNER, |b| {
            b.paint(METAL);
            b.cuboid(v3(3.6, 0.0, 1.15), v3(7.4, 1.5, 0.7));
        });
        return;
    }

    // Square well curb: axis-aligned inner lip, open over the crack pit.
    b.radial(4, |b| {
        b.paint(PLATING);
        b.chamfered_box(v3(6.2, 0.0, 0.28), v3(2.4, 14.8, 0.56), 0.28);
    });
    if b.fine() {
        b.paint(ACCENT);
        b.radial(4, |b| {
            b.chamfered_box(v3(5.05, 0.0, 0.78), v3(0.45, 12.6, 0.2), 0.08)
        });
        b.radial(4, |b| {
            b.paint(PLATING);
            b.plate(v3(4.7, 0.0, 0.92), v2(1.2, 3.4), 0.1, 0.05);
        });
    }

    // One square foot in each cell, team colour on top so the lot reads from above.
    b.radial(4, |b| {
        b.paint(PLATING);
        b.chamfered_box(v3(CELL, CELL, 0.7), v3(5.6, 5.6, 1.4), 0.4);
        team_panel(b, v3(CELL, CELL, 1.4), v2(3.4, 3.4));
        if b.fine() {
            b.paint(METAL);
            b.beam(
                v3(CELL - 2.6, CELL - 2.6, 1.4),
                v3(4.6, 4.6, 2.8),
                v2(0.7, 0.65),
                v2(0.42, 0.4),
            );
        }
    });

    // Field vat in a cell, piped into the well along the diagonal.
    let vat = |b: &mut MeshBuilder, at: Vec3, h: f32| {
        b.at(at, |b| {
            b.paint(PLATING);
            b.loft_z(
                &ngon(b.sides(8), 2.2),
                &[
                    Section::new(0.0, 1.0),
                    Section::new(h - 0.5, 1.0),
                    Section::new(h, 0.7),
                ],
            );
            b.paint(ACCENT);
            b.prism(v3(0.0, 0.0, h * 0.4), b.sides(8), 2.32, 2.32, 0.35);
            team_panel(b, v3(0.0, 0.0, h), v2(1.5, 1.5));
            if b.fine() {
                b.paint(METAL);
                b.prism(v3(0.0, 0.0, h - 0.12), 8, 1.15, 0.95, 0.28);
                vent(
                    b,
                    v3(0.0, 0.0, h),
                    v2(1.3, 1.3),
                    3,
                    if tech == 1 { ACCENT } else { GLOW },
                );
            }
        });
        b.paint(METAL);
        let inward = v3(at.x.signum(), at.y.signum(), 0.0);
        b.beam(
            at + v3(0.0, 0.0, 1.7) - inward * 2.2,
            inward * 5.4 + v3(0.0, 0.0, 1.35),
            v2(0.55, 0.42),
            v2(0.4, 0.32),
        );
    };
    // A pair on one diagonal. The pump house takes a third cell; tech 2 fills the last.
    b.radial(2, |b| vat(b, v3(CELL, CELL, 0.0), 5.6));
    kit(b, tech, 2, 0.15, |b| vat(b, v3(CELL, -CELL, 0.0), 6.2));

    // Pump house: the field machine that draws from the probes.
    b.at(v3(-CELL, CELL, 0.0), |b| {
        b.paint(PLATING);
        b.loft_z(
            &ngon(b.sides(8), 2.15),
            &[
                Section::new(0.0, 1.0),
                Section::new(6.4, 1.0),
                Section::new(7.15, 0.62),
            ],
        );
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, 3.4), b.sides(8), 2.28, 2.28, 0.38);
        team_panel(b, v3(0.0, 0.0, 7.15), v2(1.45, 1.45));
        // Working light on the roof: the one thing lit on a field pump.
        b.paint(GLOW);
        b.prism(v3(0.0, 0.0, 7.15), 6, 0.42, 0.28, 1.85);
        if b.fine() {
            b.paint(METAL);
            b.prism(v3(0.0, 0.0, 6.95), 8, 1.1, 0.9, 0.28);
            vent(b, v3(0.0, 0.0, 7.15), v2(1.2, 1.2), 3, ACCENT);
        }
    });

    // Intake probes: three pipes from the well lip, in and down, bells hanging
    // over the fissures. They sweep the well; they do not bore it.
    b.with_part(part::SPINNER, |b| {
        b.radial(3, |b| {
            b.paint(METAL);
            b.beam(
                v3(6.4, 0.0, 3.15),
                v3(2.7, 0.0, 0.72),
                v2(0.58, 0.5),
                v2(0.36, 0.32),
            );
            b.paint(ACCENT);
            b.prism(v3(2.5, 0.0, 0.16), b.sides(6), 0.95, 0.58, 0.42);
            if b.fine() {
                b.paint(METAL);
                b.cylinder_between(v3(6.55, 0.0, 3.35), v3(6.55, 0.0, 1.15), 0.22, 0.18, 6);
                b.prism(v3(2.5, 0.0, 0.52), 6, 0.42, 0.32, 0.28);
            }
            if tech >= 2 {
                b.paint(GLOW);
                b.prism(v3(2.5, 0.0, 0.16), 6, 1.02, 0.7, 0.1);
            }
            if tech >= 3 {
                b.paint(GLOW);
                b.prism(v3(2.5, 0.0, 0.78), 6, 0.22, 0.12, 0.28);
            }
        });
    });

    // Tech 2 cage: posts at the lot corners, rails along the build-grid edges.
    kit(b, tech, 2, 0.4, |b| {
        b.radial(4, |b| {
            b.paint(METAL);
            b.beam(
                v3(15.2, 15.2, 0.12),
                v3(15.2, 15.2, 10.15),
                v2(0.5, 0.5),
                v2(0.4, 0.4),
            );
            b.beam(
                v3(15.2, 15.2, 10.05),
                v3(15.2, -15.2, 10.05),
                v2(0.42, 0.5),
                v2(0.42, 0.5),
            );
        });
    });
    if tech >= 2 {
        b.paint(GLOW);
        b.radial(4, |b| {
            if b.fine() {
                glow_strip(b, v3(15.2, 0.0, 10.35), v2(0.28, 6.4), GLOW);
            } else {
                b.decal(v3(15.2, 0.0, 10.35), v2(0.28, 6.4));
            }
        });
        if b.fine() {
            antenna(b, v3(15.2, 15.2, 10.15), 0.85, 0.0);
        }
    }

    // Tech 3 foundry: condensers on the four cells and a gantry that spans them
    // without roofing the bore.
    kit(b, tech, 3, 0.2, |b| {
        b.radial(4, |b| {
            b.paint(PLATING);
            b.chamfered_box(v3(CELL, CELL, 6.1), v3(3.2, 3.2, 12.2), 0.4);
            b.paint(METAL);
            b.beam(
                v3(CELL, CELL, 12.15),
                v3(CELL, -CELL, 12.15),
                v2(0.42, 0.5),
                v2(0.42, 0.5),
            );
            if b.fine() {
                b.paint(ACCENT);
                b.prism(v3(CELL, CELL, 2.0), 8, 1.68, 1.68, 0.28);
            }
        });
    });
    if tech >= 3 {
        b.paint(GLOW);
        b.radial(4, |b| {
            b.prism(v3(CELL, CELL, 12.15), 8, 0.7, 0.42, 0.85);
            if b.fine() {
                b.cylinder_between(
                    v3(CELL - 1.8, CELL - 1.8, 7.4),
                    v3(4.8, 4.8, 3.2),
                    0.14,
                    0.14,
                    6,
                );
            }
        });
        if b.fine() {
            antenna(b, v3(-CELL, CELL, 7.15), 2.7, 0.0);
        }
    }
}

// ---- Reactor: power generator --------------------------------------------------

/// Authored at the tech 1 size (radius 14, height 12); higher tiers scale up
/// and gain cowls, capacitor pods and a brighter core.
pub fn power(b: &mut MeshBuilder, tech: u8) {
    let cowls = [4, 6, 8][(tech - 1) as usize];
    if b.coarse() {
        b.radial(4, |b| {
            b.paint(PLATING);
            b.extrude_y(&[[3.0, 0.0], [12.4, 0.0], [3.0, 10.6]], -2.6, 2.6);
        });
        b.paint(GLOW);
        b.cuboid_open(v3(0.0, 0.0, 6.5), v3(5.0, 5.0, 11.0));
        team_panel(b, v3(9.5, 9.5, 0.0), v2(5.0, 5.0));
        return;
    }
    // Plasma core in its cage.
    b.paint(GLOW);
    b.prism(v3(0.0, 0.0, 0.0), b.sides(8), 2.7, 2.7, 10.6);
    b.paint(ACCENT);
    for z in [1.0, 4.4, 7.8] {
        b.prism(v3(0.0, 0.0, z), b.sides(8), 3.7, 3.7, 0.9);
    }
    b.paint(PLATING);
    b.prism(v3(0.0, 0.0, 10.2), b.sides(8), 3.9, 2.4, 1.5);
    b.paint(GLOW);
    b.prism(v3(0.0, 0.0, 11.7), 6, 1.3, 1.0, 0.3);

    // Containment cowls: sloped fins ringing the core, gaps showing the glow.
    let half_width = if cowls == 4 { 2.6 } else { 1.7 };
    b.radial(cowls, |b| {
        b.paint(PLATING);
        b.extrude_y_chamfered(
            &[
                [3.6, 0.0],
                [12.4, 0.0],
                [12.4, 3.2],
                [8.0, 9.2],
                [3.6, 10.6],
            ],
            half_width,
            0.5,
        );
        if b.fine() {
            on_slope(b, [12.4, 3.2], [8.0, 9.2], 0.5, |b| {
                glow_strip(b, Vec3::ZERO, v2(4.6, 0.4), GLOW)
            });
        }
    });
    b.radial(2, |b| {
        on_slope(b, [8.0, 9.2], [3.6, 10.6], 0.5, |b| {
            team_panel(b, Vec3::ZERO, v2(3.2, half_width * 1.5))
        })
    });

    // Capacitor pods between the cowls from tech 2.
    if tech >= 2 {
        b.yawed(Vec3::ZERO, std::f32::consts::PI / cowls as f32, |b| {
            b.radial(cowls, |b| {
                b.paint(ACCENT);
                b.prism(v3(9.6, 0.0, 0.0), b.sides(6), 1.5, 1.3, 4.2);
                b.paint(GLOW);
                if b.fine() {
                    b.prism(
                        v3(9.6, 0.0, 4.2),
                        6,
                        1.0,
                        0.8,
                        0.4 + 0.5 * (tech - 2) as f32,
                    );
                } else {
                    b.decal(v3(9.6, 0.0, 4.22), v2(1.5, 1.5));
                }
            });
        });
    }
    if tech >= 3 {
        b.paint(PLATING);
        b.prism(v3(0.0, 0.0, 11.7), 8, 1.9, 0.5, 1.6);
        b.paint(GLOW);
        b.prism(v3(0.0, 0.0, 5.3), b.sides(8), 4.4, 4.4, 0.5);
        if b.fine() {
            b.prism(v3(0.0, 0.0, 8.7), 8, 4.2, 4.2, 0.4);
            b.radial(cowls, |b| {
                glow_strip(b, v3(12.9, 0.0, 0.0), v2(0.5, 2.6), GLOW)
            });
        }
    }
    if b.fine() && tech <= 2 {
        b.radial(4, |b| vent(b, v3(8.7, 8.7, 0.0), v2(2.4, 2.4), 3, GLOW));
    }
}

// ---- Storage ---------------------------------------------------------------------

pub fn storage_mass(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        b.paint(PLATING);
        b.frustum_open(
            v3(0.0, 0.0, 0.0),
            v2(23.0, 23.0),
            v2(17.0, 17.0),
            7.0,
            v2(0.0, 0.0),
        );
        team_panel(b, v3(0.0, 0.0, 7.0), v2(6.0, 6.0));
        return;
    }
    // Four squat vats around a manifold.
    b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
        b.radial(4, |b| {
            b.paint(PLATING);
            b.at(v3(8.6, 0.0, 0.0), |b| {
                b.loft_z(
                    &ngon(b.sides(8), 5.4),
                    &[
                        Section::new(0.0, 1.0),
                        Section::new(5.2, 1.0),
                        Section::new(6.8, 0.6),
                    ],
                );
                if b.fine() {
                    b.paint(METAL);
                    b.prism(v3(0.0, 0.0, 6.8), 8, 2.2, 1.8, 0.5);
                }
                b.paint(ACCENT);
                b.prism(v3(0.0, 0.0, 2.6), b.sides(8), 5.55, 5.55, 0.8);
                if b.fine() {
                    glow_strip(b, v3(0.0, 0.0, 7.3), v2(1.6, 0.5), GLOW);
                }
            });
        });
    });
    b.paint(ACCENT);
    b.chamfered_box(v3(0.0, 0.0, 3.6), v3(7.0, 7.0, 5.2), 2.0);
    team_panel(b, v3(0.0, 0.0, 6.2), v2(3.4, 3.4));
    if b.fine() {
        b.radial(4, |b| {
            b.paint(METAL);
            b.cylinder_between(v3(3.0, 0.0, 4.6), v3(8.0, 0.0, 2.4), 0.6, 0.6, 6);
            glow_strip(b, v3(12.0, 0.0, 0.0), v2(1.2, 4.0), GLOW);
        });
    }
}

pub fn storage_energy(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        b.paint(ACCENT);
        b.cuboid_open(v3(0.0, 0.0, 4.6), v3(19.0, 14.0, 7.2));
        b.paint(PLATING);
        b.mirror_y(|b| {
            b.frustum_open(
                v3(0.0, 9.6, 0.0),
                v2(22.0, 4.4),
                v2(16.0, 2.4),
                9.6,
                v2(0.0, -0.8),
            )
        });
        team_panel(b, v3(0.0, 0.0, 8.25), v2(19.0, 3.0));
        return;
    }
    // Capacitor cells: two rows of three between armoured bookends.
    for col in -1..=1 {
        b.mirror_y(|b| {
            b.at(v3(col as f32 * 6.4, 3.4, 0.0), |b| {
                b.paint(ACCENT);
                b.prism(v3(0.0, 0.0, 0.0), b.sides(8), 2.9, 2.9, 8.0);
                b.paint(GLOW);
                b.prism(v3(0.0, 0.0, 3.0), b.sides(8), 3.05, 3.05, 0.7);
                b.prism(v3(0.0, 0.0, 5.6), b.sides(8), 3.05, 3.05, 0.7);
                b.paint(PLATING);
                b.prism(v3(0.0, 0.0, 8.0), b.sides(8), 3.1, 2.0, 1.0);
            });
        });
    }
    b.mirror_y(|b| {
        b.paint(PLATING);
        b.extrude_x(
            &[
                [7.4, 0.0],
                [11.8, 0.0],
                [11.8, 4.0],
                [10.0, 9.6],
                [8.2, 9.6],
                [7.4, 8.0],
            ],
            -11.0,
            11.0,
        );
        if b.fine() {
            for i in 0..3 {
                vent(
                    b,
                    v3(-6.4 + 6.4 * i as f32, 9.1, 9.6),
                    v2(3.6, 1.2),
                    4,
                    GLOW,
                );
            }
        }
    });
    // Bus bar down the middle.
    b.paint(METAL);
    b.block(v3(-10.0, -0.9, 8.4), v3(10.0, 0.9, 9.2));
    team_panel(b, v3(0.0, 0.0, 9.2), v2(14.0, 1.4));
    if b.fine() {
        b.mirror_y(|b| glow_strip(b, v3(12.4, 4.0, 0.0), v2(0.8, 5.0), GLOW));
        b.mirror_y(|b| glow_strip(b, v3(-12.4, 4.0, 0.0), v2(0.8, 5.0), GLOW));
    }
}

// ---- Defences ------------------------------------------------------------------------

/// Tapered bunker pedestal with corner buttresses, top at `height`.
fn pedestal(b: &mut MeshBuilder, radius: f32, height: f32) {
    b.paint(PLATING);
    if b.coarse() {
        b.frustum_open(
            v3(0.0, 0.0, 0.0),
            v2(radius * 1.85, radius * 1.85),
            v2(radius * 1.1, radius * 1.1),
            height,
            v2(0.0, 0.0),
        );
        return;
    }
    b.loft_z(
        &ngon(8, radius),
        &[
            Section::new(0.0, 1.0),
            Section::new(height * 0.35, 0.94),
            Section::new(height - 0.5, 0.64),
        ],
    );
    b.paint(ACCENT);
    b.prism(
        v3(0.0, 0.0, height - 0.5),
        8,
        radius * 0.56,
        radius * 0.56,
        0.5,
    );
    b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
        b.radial(4, |b| {
            b.paint(PLATING);
            b.extrude_y_chamfered(
                &[
                    [radius * 0.6, 0.0],
                    [radius * 1.22, 0.0],
                    [radius * 1.22, 1.6],
                    [radius * 0.6, height * 0.8],
                ],
                radius * 0.2,
                radius * 0.05,
            );
            on_slope(
                b,
                [radius * 1.22, 1.6],
                [radius * 0.6, height * 0.8],
                0.6,
                |b| team_panel(b, Vec3::ZERO, v2(radius * 0.4, radius * 0.22)),
            );
        });
    });
    if b.fine() {
        b.radial(4, |b| {
            glow_strip(
                b,
                v3(radius * 1.02, 0.0, 0.0),
                v2(radius * 0.12, radius * 0.5),
                GLOW,
            )
        });
    }
}

/// Sentinel: tech 1 point defence.
pub fn turret(b: &mut MeshBuilder, _tech: u8) {
    pedestal(b, 5.4, 5.2);
    b.set_turret_pivot(v3(0.0, 0.0, 5.2));
    b.with_part(part::TURRET, |b| {
        b.paint(PLATING);
        let roof = turret_shell(b, 7.4, 6.0, 5.25, 8.8);
        rail_gun(
            b,
            v3(2.9, 0.0, 7.4),
            v3(6.0, 0.0, 7.4),
            v2(0.36, 0.7),
            0.36,
            Emitter::Blue,
        );
        if b.mid() {
            b.paint(ACCENT);
            b.block(v3(2.7, -1.0, 6.6), v3(3.4, 1.0, 8.2));
        }
        team_panel(
            b,
            roof.at(0.2, 0.0),
            v2(roof.length() * 0.36, roof.half_width * 1.7),
        );
        if b.fine() {
            b.mirror_y(|b| glow_strip(b, roof.at(0.7, 0.85), v2(roof.length() * 0.5, 0.14), GLOW));
            b.paint(GLASS);
            b.frustum(
                roof.at(0.8, -0.35),
                v2(0.7, 0.6),
                v2(0.4, 0.4),
                0.35,
                v2(-0.05, 0.0),
            );
            b.paint(ACCENT);
            b.prism(roof.at(0.62, 0.35), 8, 0.45, 0.38, 0.15);
            antenna(b, roof.at(-0.1, 0.5), 1.6, 0.1);
        }
    });
}

/// Bastion: tech 2 triple arc battery.
pub fn turret_heavy(b: &mut MeshBuilder, _tech: u8) {
    pedestal(b, 10.6, 6.6);
    b.set_turret_pivot(v3(0.0, 0.0, 6.6));
    b.with_part(part::TURRET, |b| {
        b.paint(PLATING);
        let roof = turret_shell(b, 11.0, 9.6, 6.65, 11.8);
        if b.coarse() {
            rail_gun(
                b,
                v3(4.0, 0.0, 10.0),
                v3(9.0, 0.0, 10.0),
                v2(2.0, 0.7),
                0.6,
                Emitter::Blue,
            );
            team_panel(
                b,
                roof.at(0.2, 0.0),
                v2(roof.length() * 0.36, roof.half_width * 1.7),
            );
            return;
        }
        for y in [-1.8, 0.0, 1.8] {
            rail_gun(
                b,
                v3(4.2, y, 10.0),
                v3(9.0, y, 10.0),
                v2(0.34, 0.7),
                0.34,
                Emitter::Blue,
            );
        }
        b.paint(ACCENT);
        b.block(v3(3.9, -3.0, 9.1), v3(4.7, 3.0, 10.9));
        team_panel(
            b,
            roof.at(0.14, 0.0),
            v2(roof.length() * 0.26, roof.half_width * 1.7),
        );
        // Cheek capacitor housings.
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.extrude_y(
                &[[-4.0, 7.4], [2.2, 7.4], [1.4, 10.2], [-3.2, 10.2]],
                4.5,
                5.2,
            );
            if b.fine() {
                b.paint(GLOW);
                b.extrude_y(
                    &[[-3.0, 8.2], [1.2, 8.2], [0.9, 9.4], [-2.6, 9.4]],
                    5.2,
                    5.26,
                );
            }
        });
        if b.fine() {
            b.mirror_y(|b| glow_strip(b, roof.at(0.66, 0.88), v2(roof.length() * 0.6, 0.22), GLOW));
            b.mirror_y(|b| vent(b, roof.at(0.5, 0.45), v2(1.4, 1.0), 3, GLOW));
            b.paint(GLASS);
            b.frustum(
                roof.at(0.92, 0.0),
                v2(1.0, 1.6),
                v2(0.6, 1.0),
                0.5,
                v2(-0.1, 0.0),
            );
            antenna(b, roof.at(-0.1, -0.5), 2.4, 0.1);
            antenna(b, roof.at(-0.1, 0.5), 1.8, 0.1);
        }
    });
}

/// Onager: tech 2 static artillery with a conventional howitzer.
pub fn artillery_static(b: &mut MeshBuilder, _tech: u8) {
    pedestal(b, 10.6, 4.6);
    let (breech, muzzle) = (v3(-2.0, 0.0, 5.7), v3(12.0, 0.0, 9.5));
    b.set_turret_pivot(v3(0.0, 0.0, 4.6));
    b.with_part(part::TURRET, |b| {
        cannon(b, breech, muzzle, 0.55, Emitter::Orange);
        b.paint(PLATING);
        if b.coarse() {
            b.frustum_open(
                v3(-1.5, 0.0, 4.6),
                v2(9.0, 8.0),
                v2(5.0, 5.0),
                4.2,
                v2(-0.8, 0.0),
            );
            team_panel(b, v3(-2.3, 0.0, 8.8), v2(5.0, 5.0));
            return;
        }
        // Gun house: two trunnion cheeks and a sloped rear casemate.
        b.mirror_y(|b| {
            b.extrude_y(
                &[
                    [-5.0, 4.65],
                    [3.4, 4.65],
                    [2.2, 7.6],
                    [-0.6, 8.9],
                    [-4.4, 8.2],
                ],
                1.5,
                3.3,
            )
        });
        b.extrude_y_chamfered(
            &[
                [-6.2, 4.65],
                [-2.6, 4.65],
                [-2.6, 7.4],
                [-3.4, 8.4],
                [-5.4, 8.0],
                [-6.2, 6.4],
            ],
            3.0,
            0.6,
        );
        b.paint(ACCENT);
        b.prism(v3(-0.8, 0.0, 4.5), 8, 5.4, 5.4, 0.2);
        b.cylinder_between(
            v3(-0.4, -3.6, 6.6),
            v3(-0.4, 3.6, 6.6),
            0.9,
            0.9,
            b.sides(8),
        );
        on_slope(b, [-0.6, 8.9], [-4.4, 8.2], 0.5, |b| {
            b.mirror_y(|b| team_panel(b, v3(0.0, 2.4, 0.0), v2(2.4, 1.2)))
        });
        if b.fine() {
            b.pitched(
                breech,
                (muzzle - breech).z.atan2((muzzle - breech).x),
                |b| {
                    b.paint(METAL);
                    b.mirror_y(|b| {
                        b.cylinder_between(v3(1.0, 0.85, 0.7), v3(6.2, 0.85, 0.7), 0.26, 0.26, 6)
                    });
                    b.paint(ACCENT);
                    b.block(v3(0.6, -1.2, 0.35), v3(1.6, 1.2, 1.05));
                },
            );
            on_slope(b, [3.4, 4.65], [2.2, 7.6], 0.5, |b| {
                b.mirror_y(|b| glow_strip(b, v3(0.0, 2.4, 0.0), v2(1.6, 0.3), GLOW_ORANGE))
            });
            b.mirror_y(|b| vent(b, v3(-4.4, 1.5, 8.2), v2(1.2, 1.4), 3, GLOW_ORANGE));
            antenna(b, v3(-5.6, -2.2, 7.6), 2.6, 0.1);
        }
    });
}

// ---- Watchtower: radar --------------------------------------------------------------

/// Authored heights per tech: 20, 24, 28 m. The mast grows and the spinner
/// picks up extra dishes; emitters are earned at each tier.
pub fn radar(b: &mut MeshBuilder, tech: u8) {
    let lift = 4.0 * (tech - 1) as f32;
    let hub = 15.2 + lift;
    b.set_spinner_pivot(v3(0.0, 0.0, hub));
    b.paint(PLATING);
    if b.coarse() {
        b.frustum_open(
            v3(0.0, 0.0, 0.0),
            v2(6.4, 6.4),
            v2(1.6, 1.6),
            hub,
            v2(0.0, 0.0),
        );
        team_panel(b, v3(3.6, 0.0, 0.0), v2(3.0, 9.0));
        b.with_part(part::SPINNER, |b| {
            b.paint(PLATING);
            b.pitched(v3(0.0, 0.0, hub + 2.2), 0.35, |b| {
                b.cuboid(v3(0.6, 0.0, 0.0), v3(0.6, 9.0, 4.2))
            });
            if tech >= 2 {
                b.yawed(v3(0.0, 0.0, hub + 2.2), FRAC_PI_2, |b| {
                    b.pitched(Vec3::ZERO, 0.35, |b| {
                        b.cuboid(v3(0.6, 0.0, 0.0), v3(0.6, 7.0, 3.2))
                    })
                });
            }
        });
        return;
    }
    // Mast: tapered octagonal tower on a bunker base, braced by three legs.
    b.loft_z(
        &ngon(b.sides(8), 3.4),
        &[
            Section::new(0.0, 1.0),
            Section::new(3.2, 0.8),
            Section::new(4.2, 0.45),
            Section::new(hub - 0.8, 0.28),
        ],
    );
    if b.fine() {
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, hub - 0.8), 8, 1.4, 1.4, 0.8);
    }
    b.radial(3, |b| {
        b.paint(ACCENT);
        b.beam(
            v3(5.4, 0.0, 0.75),
            v3(0.9, 0.0, 9.5),
            v2(0.7, 0.7),
            v2(0.45, 0.45),
        );
    });
    team_panel(b, v3(-5.4, 0.0, 0.0), v2(1.6, 5.0));

    // Side sensor pods from tech 2: more reach, more kit on the bunker.
    if tech >= 2 {
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.chamfered_box(v3(0.0, 5.6, 1.6), v3(2.4, 1.8, 2.2), 0.35);
            if b.fine() {
                b.paint(GLOW);
                b.block(v3(-0.8, 6.45, 1.1), v3(0.8, 6.55, 2.2));
                antenna(b, v3(0.6, 5.6, 2.7), 2.4 + lift * 0.15, 0.1);
            }
        });
    }
    if tech >= 3 {
        b.paint(PLATING);
        b.chamfered_box(v3(5.2, 0.0, 1.8), v3(1.6, 2.6, 2.6), 0.35);
        if b.fine() {
            b.paint(GLOW);
            b.block(v3(5.95, -0.9, 1.2), v3(6.05, 0.9, 2.5));
            antenna(b, v3(5.2, 0.8, 3.1), 3.0, 0.15);
            antenna(b, v3(5.2, -0.8, 3.1), 2.4, 0.2);
        }
    }

    let dish = |b: &mut MeshBuilder, width: f32, lit: bool| {
        b.pitched(v3(0.0, 0.0, hub + 2.4), 0.35, |b| {
            let panel = |b: &mut MeshBuilder, width: f32| {
                b.paint(PLATING);
                b.extrude_x(
                    &[
                        [-width, -1.9],
                        [width, -1.9],
                        [width, 1.5],
                        [width - 0.8, 2.1],
                        [-width + 0.8, 2.1],
                        [-width, 1.5],
                    ],
                    0.45,
                    0.7,
                );
                if b.fine() {
                    b.paint(ACCENT);
                    b.block(v3(0.2, -width * 0.8, -1.5), v3(0.45, width * 0.8, 1.6));
                }
            };
            panel(b, width);
            b.mirror_y(|b| {
                b.yawed(v3(0.35, width, 0.0), -0.5, |b| {
                    b.at(v3(-0.35, width - 0.2, 0.0), |b| panel(b, width - 0.2))
                })
            });
            b.paint(ACCENT);
            b.beam(
                v3(0.6, 0.0, -1.6),
                v3(3.2, 0.0, 0.1),
                v2(0.22, 0.22),
                v2(0.16, 0.16),
            );
            b.paint(if lit { GLOW } else { ACCENT });
            if b.fine() {
                b.spheroid(v3(3.25, 0.0, 0.15), v3(0.4, 0.4, 0.4), 6, 2);
            } else {
                b.cuboid(v3(3.25, 0.0, 0.15), Vec3::splat(0.6));
            }
            if b.fine() && lit {
                b.paint(GLOW);
                b.block(v3(0.7, -1.4, 1.2), v3(0.74, 1.4, 1.4));
                b.block(v3(0.7, -1.4, -1.5), v3(0.74, 1.4, -1.3));
            }
        });
        b.paint(ACCENT);
        b.beam(
            v3(0.0, 0.0, hub + 1.0),
            v3(0.25, 0.0, hub + 2.4),
            v2(0.8, 0.6),
            v2(0.6, 0.4),
        );
    };

    b.with_part(part::SPINNER, |b| {
        b.paint(METAL);
        b.prism(v3(0.0, 0.0, hub), b.sides(8), 1.0, 0.8, 1.2);
        // Faceted dish: three panels, the outer two swept forward, all tilted skyward.
        dish(b, 1.7, true);
        if tech >= 2 {
            // Crossed array: a second, slightly smaller dish at a right angle.
            b.yawed(v3(0.0, 0.0, 0.0), FRAC_PI_2, |b| dish(b, 1.4, tech >= 3));
        }
        if tech >= 3 {
            b.paint(GLOW);
            b.prism(v3(0.0, 0.0, hub + 3.6), 8, 0.55, 0.35, 0.8);
        }
        if b.fine() {
            antenna(b, v3(-0.3, 0.0, hub + 1.2), 3.2, 0.0);
            if tech >= 2 {
                antenna(b, v3(-0.2, 0.6, hub + 1.2), 2.6, 0.12);
            }
            if tech >= 3 {
                antenna(b, v3(-0.2, -0.6, hub + 1.2), 2.2, 0.18);
            }
        }
    });
    if b.fine() {
        b.paint(GLOW);
        let rings = 3 + (tech - 1) as usize * 2;
        for i in 0..rings {
            let z = 6.0 + 3.0 * i as f32;
            if z > hub - 1.2 {
                break;
            }
            let r = 3.4 * (0.45 - 0.17 * (z - 4.2) / (hub - 5.0)) + 0.06;
            b.prism(v3(0.0, 0.0, z), 8, r, r, 0.25);
        }
        b.paint(GLASS);
        b.loft_z(
            &ngon(8, 3.4),
            &[Section::new(2.2, 0.9), Section::new(2.7, 0.865)],
        );
    }
}

// ---- Scavenger: reclaim tower ------------------------------------------------------

/// A gun tower: bunker, shaft and an Aster turret that turns a reclaim lance
/// onto its work and charges before the beam comes on. Tech 2: the lights it
/// has earned are the orange of reclaim, not a rail's blue.
pub fn reclaimer(b: &mut MeshBuilder, _tech: u8) {
    let ring = 17.2;
    let (breech, muzzle) = (v3(1.6, 0.0, 19.2), v3(13.2, 0.0, 20.4));
    b.set_turret_pivot(v3(0.0, 0.0, ring));
    b.paint(PLATING);
    if b.coarse() {
        b.frustum_open(
            v3(0.0, 0.0, 0.0),
            v2(22.0, 22.0),
            v2(8.0, 8.0),
            ring,
            v2(0.0, 0.0),
        );
        b.with_part(part::TURRET, |b| {
            b.paint(PLATING);
            b.cuboid(v3(-0.6, 0.0, 18.8), v3(8.8, 7.0, 3.4));
            reclaim_gun(b, breech, muzzle, 0.52);
            team_panel(b, v3(-1.8, 0.0, 20.5), v2(4.6, 5.0));
        });
        return;
    }

    // Bunker filling the lot, four cardinal feet, a thick shaft to the gun deck.
    b.loft_z(
        &ngon(b.sides(8), 11.0),
        &[
            Section::new(0.0, 1.0),
            Section::new(2.6, 0.94),
            Section::new(7.2, 0.68),
        ],
    );
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, 7.2), 8, 7.2, 7.2, 0.45);
    b.radial(4, |b| {
        b.paint(PLATING);
        b.extrude_y_chamfered(
            &[[6.4, 0.0], [13.2, 0.0], [13.2, 1.8], [6.8, 6.4]],
            2.2,
            0.45,
        );
        on_slope(b, [13.2, 1.8], [6.8, 6.4], 0.55, |b| {
            team_panel(b, Vec3::ZERO, v2(3.6, 2.0))
        });
    });
    b.paint(PLATING);
    b.loft_z(
        &ngon(b.sides(8), 4.8),
        &[
            Section::new(7.4, 1.0),
            Section::new(11.2, 0.92),
            Section::new(ring - 0.55, 0.78),
        ],
    );
    b.paint(METAL);
    b.prism(v3(0.0, 0.0, ring - 0.55), b.sides(8), 3.9, 3.9, 0.55);
    // Charge banks on the shaft: this tower stores up before it fires.
    b.mirror_y(|b| {
        b.paint(PLATING);
        b.extrude_y(
            &[[-2.6, 1.2], [3.2, 1.2], [2.6, 13.6], [-2.0, 12.4]],
            5.4,
            7.2,
        );
    });

    b.with_part(part::TURRET, |b| {
        b.paint(PLATING);
        let roof = turret_shell(b, 10.4, 8.0, ring + 0.05, 20.7);
        reclaim_gun(b, breech, muzzle, 0.52);
        b.paint(ACCENT);
        b.block(v3(2.2, -1.5, 18.4), v3(3.4, 1.5, 20.2));
        team_panel(
            b,
            roof.at(0.18, 0.0),
            v2(roof.length() * 0.34, roof.half_width * 1.7),
        );
        // Cheek capacitors on the head, same job as the banks on the shaft.
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.extrude_y(
                &[[-3.8, 17.8], [2.0, 17.8], [1.4, 20.2], [-3.2, 20.0]],
                3.5,
                4.4,
            );
        });
        if b.fine() {
            b.mirror_y(|b| {
                glow_strip(
                    b,
                    roof.at(0.62, 0.88),
                    v2(roof.length() * 0.5, 0.16),
                    GLOW_ORANGE,
                )
            });
            b.mirror_y(|b| vent(b, roof.at(0.48, 0.42), v2(1.3, 0.9), 3, GLOW_ORANGE));
            b.mirror_y(|b| {
                b.paint(GLOW_ORANGE);
                b.extrude_y(
                    &[[-2.8, 18.4], [1.2, 18.4], [0.8, 19.6], [-2.4, 19.4]],
                    4.4,
                    4.46,
                );
            });
            b.paint(GLASS);
            b.frustum(
                roof.at(0.88, 0.0),
                v2(0.9, 1.4),
                v2(0.5, 0.85),
                0.42,
                v2(-0.08, 0.0),
            );
            antenna(b, roof.at(-0.08, 0.55), 2.2, 0.12);
            antenna(b, roof.at(-0.08, -0.45), 1.7, 0.18);
        }
    });
    if b.fine() {
        b.paint(GLOW_ORANGE);
        for z in [8.8, 11.6, 14.4] {
            let t = (z - 7.4) / (ring - 8.0);
            let r = 4.8 * (0.96 - 0.2 * t);
            b.prism(v3(0.0, 0.0, z), 8, r, r, 0.22);
        }
        b.mirror_y(|b| {
            b.paint(GLOW_ORANGE);
            b.extrude_y(
                &[[-1.4, 3.2], [2.2, 3.2], [1.8, 12.2], [-1.0, 11.2]],
                7.2,
                7.28,
            );
        });
        b.paint(GLASS);
        b.loft_z(
            &ngon(8, 11.0),
            &[Section::new(3.0, 0.9), Section::new(3.6, 0.88)],
        );
        glow_strip(b, v3(12.4, 0.0, 0.0), v2(1.2, 4.2), GLOW_ORANGE);
        glow_strip(b, v3(-12.4, 0.0, 0.0), v2(1.2, 4.2), GLOW_ORANGE);
        glow_strip(b, v3(0.0, 12.4, 0.0), v2(4.2, 1.2), GLOW_ORANGE);
        glow_strip(b, v3(0.0, -12.4, 0.0), v2(4.2, 1.2), GLOW_ORANGE);
    }
}

// ---- Rampart: wall segment -----------------------------------------------------------

pub fn wall(b: &mut MeshBuilder, _tech: u8) {
    let plan = chamfered_rect(v2(7.7, 7.7), 2.4);
    if b.coarse() {
        b.paint(PLATING);
        b.frustum_open(
            v3(0.0, 0.0, 0.0),
            v2(15.4, 15.4),
            v2(10.0, 10.0),
            6.0,
            v2(0.0, 0.0),
        );
        team_panel(b, v3(0.0, 0.0, 6.0), v2(5.0, 5.0));
        return;
    }
    b.paint(ACCENT);
    b.loft_z(&plan, &[Section::new(0.0, 1.0), Section::new(1.4, 1.0)]);
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[
            Section::new(1.4, 0.97),
            Section::new(2.4, 0.97),
            Section::new(5.3, 0.72),
            Section::new(6.0, 0.62),
        ],
    );
    team_panel(
        b,
        v3(0.0, 0.0, if b.fine() { 6.2 } else { 6.0 }),
        v2(4.4, 4.4),
    );
    if b.fine() {
        // Armour panels on the four sloped faces, marker lights on the corners.
        b.radial(4, |b| {
            on_slope(b, [7.47, 2.4], [5.54, 5.3], 0.5, |b| {
                b.paint(PLATING);
                b.plate(Vec3::ZERO, v2(2.4, 7.0), 0.22, 0.12);
                glow_strip(b, v3(-0.2, 0.0, 0.22), v2(0.3, 4.0), GLOW);
            });
        });
        b.paint(ACCENT);
        b.chamfered_box(v3(0.0, 0.0, 6.1), v3(7.4, 7.4, 0.2), 1.7);
    }
}
