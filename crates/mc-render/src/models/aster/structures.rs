//! Aster structures. Each sits on a ground foundation that traces its mesh
//! inside the build-grid footprint (12 m cells) and opens or faces toward +x.

use std::f32::consts::{FRAC_PI_4, FRAC_PI_6, FRAC_PI_8, TAU};

use glam::{Vec2, Vec3};

use super::parts::*;
use crate::models::builder::{chamfered_rect, ngon, MeshBuilder, Section};
use crate::models::material::*;
use crate::models::{part, rig};

/// Emits `f` if kit `tier` is fitted at `tech`. The next tier's kit is an
/// upgrade piece, rising at `at` of the way through the refit (full detail
/// only: a refit is watched from close by), and is omitted beyond that.
pub(super) fn kit(b: &mut MeshBuilder, tech: u8, tier: u8, at: f32, f: impl FnOnce(&mut MeshBuilder)) {
    if tier <= tech {
        f(b);
    } else if tier == tech + 1 && b.fine() {
        b.upgrade(at, f);
    }
}

// ---- Mass extractor ------------------------------------------------------------
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

// ---- Storage ---------------------------------------------------------------------

/// The Materials Vault, authored on a 3×3 lot (radius 16.5, lot half-extent 18) and
/// built up in place, tier on tier; the vats never move.
/// - Tech 1 (8 m): four squat vats round a manifold on an armoured slab.
/// - Tech 2 (13 m): a second stage on every vat, a loading tower in the middle,
///   gantries out to the vats and loaders on the four sides.
/// - Tech 3 (19 m): an armoured strongroom silo over the tower, corner pylons
///   feeding it, and banded armour round the vats.
pub fn storage_mass(b: &mut MeshBuilder, tech: u8) {
    if b.coarse() {
        storage_mass_coarse(b, tech);
        return;
    }
    // Slab: dark foot, white deck.
    b.paint(ACCENT);
    b.loft_z(
        &chamfered_rect(v2(VAULT_SLAB, VAULT_SLAB), 4.0),
        &[Section::new(0.0, 1.0), Section::new(0.7, 1.0)],
    );
    b.paint(PLATING);
    b.loft_z(
        &chamfered_rect(v2(VAULT_SLAB, VAULT_SLAB), 4.0),
        &[Section::new(0.7, 0.99), Section::new(VAULT_DECK, 0.96)],
    );
    b.radial(4, |b| team_panel(b, v3(15.2, 0.0, VAULT_DECK), v2(0.8, 12.0)));

    // Four vats on the diagonals, banded, with a lid and a light.
    b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
        b.radial(4, |b| {
            b.at(v3(VAULT_VAT_OUT, 0.0, 0.0), |b| {
                b.paint(PLATING);
                b.loft_z(
                    &ngon(b.sides(10), VAULT_VAT_R),
                    &[
                        Section::new(VAULT_DECK, 1.0),
                        Section::new(6.4, 1.0),
                        Section::new(7.6, 0.72),
                    ],
                );
                b.paint(ACCENT);
                b.prism(v3(0.0, 0.0, 2.8), b.sides(10), VAULT_VAT_R + 0.12, VAULT_VAT_R + 0.12, 0.9);
                b.paint(METAL);
                b.prism(v3(0.0, 0.0, 7.6), 8, 2.4, 2.0, 0.4);
                if b.fine() {
                    glow_strip(b, v3(0.0, 0.0, 8.0), v2(1.6, 0.5), GLOW);
                }
            });
        });
    });
    // The manifold between them, and the pipes out to each vat.
    b.paint(ACCENT);
    b.chamfered_box(v3(0.0, 0.0, 4.2), v3(7.6, 7.6, 5.4), 2.0);
    team_panel(b, v3(0.0, 0.0, 6.9), v2(3.6, 3.6));
    b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
        b.radial(4, |b| {
            b.paint(METAL);
            b.cylinder_between(v3(3.4, 0.0, 5.2), v3(VAULT_VAT_OUT - VAULT_VAT_R + 0.2, 0.0, 3.4), 0.7, 0.7, 6);
        });
    });

    // ---- Tech 2: stacked vats, a loading tower, gantries and loaders.
    kit(b, tech, 2, 0.1, |b| {
        b.radial(4, |b| {
            // A loader on each side: a hopper on legs with a chute to the slab.
            b.paint(ACCENT);
            b.chamfered_box(v3(12.6, 0.0, 3.4), v3(3.2, 6.0, 4.0), 0.6);
            b.paint(PLATING);
            b.plate(v3(12.6, 0.0, 5.4), v2(2.8, 5.4), 0.14, 0.05);
            if b.fine() {
                glow_strip(b, v3(14.22, 0.0, 2.2), v2(0.1, 3.0), GLOW);
            }
            b.paint(METAL);
            b.beam(v3(11.0, 0.0, 4.6), v3(3.8, 0.0, 6.6), v2(1.6, 0.8), v2(1.4, 0.7));
        });
    });
    kit(b, tech, 2, 0.35, |b| {
        b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
            b.radial(4, |b| {
                b.at(v3(VAULT_VAT_OUT, 0.0, 0.0), |b| {
                    b.paint(PLATING_DARK);
                    b.prism(v3(0.0, 0.0, 7.6), b.sides(10), 3.9, 3.9, 0.6);
                    b.paint(PLATING);
                    b.loft_z(
                        &ngon(b.sides(10), 3.7),
                        &[Section::new(8.2, 1.0), Section::new(10.6, 1.0), Section::new(11.4, 0.7)],
                    );
                    b.paint(METAL);
                    b.prism(v3(0.0, 0.0, 9.2), b.sides(10), 3.82, 3.82, 0.5);
                    if b.fine() {
                        b.paint(GLOW);
                        b.prism(v3(0.0, 0.0, 10.0), b.sides(10), 3.78, 3.78, 0.3);
                    }
                });
            });
        });
    });
    kit(b, tech, 2, 0.6, |b| {
        // The loading tower rises out of the manifold.
        b.paint(PLATING);
        b.loft_z(
            &chamfered_rect(v2(3.0, 3.0), 1.0),
            &[Section::new(6.9, 1.0), Section::new(11.8, 1.0), Section::new(12.6, 0.8)],
        );
        b.paint(ACCENT);
        b.chamfered_box(v3(0.0, 0.0, 9.4), v3(6.3, 6.3, 1.0), 0.3);
        team_panel(b, v3(0.0, 0.0, 12.6), v2(3.0, 3.0));
        if b.fine() {
            b.radial(4, |b| glow_strip(b, v3(3.02, 0.0, 10.4), v2(0.12, 3.6), GLOW));
        }
    });
    kit(b, tech, 2, 0.85, |b| {
        // Gantries from the tower out over the vat tops.
        b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
            b.radial(4, |b| {
                b.paint(ACCENT);
                b.beam(v3(3.0, 0.0, 11.0), v3(VAULT_VAT_OUT, 0.0, 11.2), v2(1.2, 0.8), v2(1.0, 0.7));
                b.paint(METAL);
                b.cylinder_between(v3(VAULT_VAT_OUT, 0.0, 11.2), v3(VAULT_VAT_OUT, 0.0, 12.4), 0.9, 0.7, 6);
            });
        });
    });

    // ---- Tech 3: the strongroom, corner pylons and banded armour.
    kit(b, tech, 3, 0.1, |b| {
        b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
            b.radial(4, |b| {
                b.at(v3(VAULT_VAT_OUT, 0.0, 0.0), |b| {
                    b.paint(ACCENT);
                    for z in [4.6, 8.4] {
                        b.prism(v3(0.0, 0.0, z), b.sides(10), VAULT_VAT_R + 0.3, VAULT_VAT_R + 0.3, 1.1);
                    }
                });
            });
        });
    });
    kit(b, tech, 3, 0.3, |b| {
        b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
            b.radial(4, |b| {
                // A pylon in each corner of the lot, lit at the top, fed into the silo.
                let at = v3(19.4, 0.0, 0.0);
                b.paint(PLATING_DARK);
                b.prism(at, 6, 1.8, 1.8, 1.2);
                b.paint(PLATING);
                b.prism(at + Vec3::Z * 1.2, 6, 1.2, 0.8, 13.0);
                b.paint(GLOW);
                b.prism(at + Vec3::Z * 14.2, 6, 0.9, 0.5, 1.4);
                b.paint(METAL);
                b.cylinder_between(at + Vec3::Z * 13.0, v3(5.4, 0.0, 15.4), 0.3, 0.3, 6);
            });
        });
    });
    kit(b, tech, 3, 0.55, |b| {
        // The strongroom: an armoured octagonal silo over the tower.
        b.paint(PLATING_DARK);
        b.prism(v3(0.0, 0.0, 12.6), 8, 6.2, 6.2, 0.8);
        b.paint(PLATING);
        b.loft_z(
            &ngon(8, 5.6),
            &[
                Section::new(13.4, 1.0),
                Section::new(16.2, 1.0),
                Section::new(17.6, 0.72),
                Section::new(18.4, 0.4),
            ],
        );
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, 14.6), 8, 5.75, 5.75, 0.9);
        team_panel(b, v3(0.0, 0.0, 18.4), v2(2.2, 2.2));
    });
    kit(b, tech, 3, 0.85, |b| {
        b.paint(GLOW);
        b.prism(v3(0.0, 0.0, 15.8), 8, 5.7, 5.7, 0.35);
        b.prism(v3(0.0, 0.0, 17.0), 8, 4.9, 4.7, 0.3);
        b.paint(METAL);
        b.prism(v3(0.0, 0.0, 18.4), 8, 1.4, 0.9, 0.6);
    });
}

/// Half-width of the vault's slab, and the top of its deck.
const VAULT_SLAB: f32 = 16.6;
const VAULT_DECK: f32 = 1.6;
/// How far out on the diagonal the vats stand, and their radius.
const VAULT_VAT_OUT: f32 = 9.4;
const VAULT_VAT_R: f32 = 4.4;

/// A block per tier: the slab and vats, the tower, the strongroom.
fn storage_mass_coarse(b: &mut MeshBuilder, tech: u8) {
    b.paint(PLATING);
    b.cuboid_open(v3(0.0, 0.0, VAULT_DECK * 0.5), v3(VAULT_SLAB * 2.0, VAULT_SLAB * 2.0, VAULT_DECK));
    b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
        b.radial(4, |b| {
            let top = if tech >= 2 { 11.4 } else { 7.6 };
            b.frustum_open(v3(VAULT_VAT_OUT, 0.0, VAULT_DECK), v2(8.4, 8.4), v2(6.0, 6.0), top - VAULT_DECK, Vec2::ZERO);
        });
    });
    b.paint(ACCENT);
    let top = match tech {
        1 => 6.9,
        2 => 12.6,
        _ => 18.4,
    };
    b.frustum_open(v3(0.0, 0.0, VAULT_DECK), v2(7.6, 7.6), v2(6.0, 6.0), top - VAULT_DECK, Vec2::ZERO);
    team_panel(b, v3(0.0, 0.0, top), v2(3.0, 3.0));
    if tech >= 3 {
        b.paint(PLATING);
        b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
            b.radial(4, |b| b.cuboid_open(v3(19.4, 0.0, 7.5), v3(2.0, 2.0, 15.0)))
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
/// `dark` paints the body black metal and drops the blue emitters — T1 kit.
fn pedestal(b: &mut MeshBuilder, radius: f32, height: f32, dark: bool) {
    b.paint(if dark { ACCENT } else { PLATING });
    if b.coarse() {
        b.frustum_open(
            v3(0.0, 0.0, 0.0),
            v2(radius * 1.85, radius * 1.85),
            v2(radius * 1.1, radius * 1.1),
            height,
            v2(0.0, 0.0),
        );
        if dark {
            b.paint(PLATING);
            b.plate(
                v3(0.0, 0.0, height - 0.08),
                v2(radius * 0.55, radius * 0.55),
                0.16,
                0.04,
            );
        }
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
    if dark {
        b.paint(PLATING);
        b.mirror_y(|b| {
            b.plate(
                v3(0.15, radius * 0.22, height - 0.4),
                v2(radius * 0.72, radius * 0.24),
                0.12,
                0.04,
            )
        });
    }
    b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
        b.radial(4, |b| {
            b.paint(if dark { ACCENT } else { PLATING });
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
            // White face plate out on the slope, team flash nearer the keep
            // so the armour still reads as white, not team colour.
            on_slope(
                b,
                [radius * 1.22, 1.6],
                [radius * 0.6, height * 0.8],
                0.58,
                |b| {
                    if dark {
                        b.paint(PLATING);
                        b.plate(Vec3::ZERO, v2(radius * 0.5, radius * 0.3), 0.1, 0.03);
                    }
                },
            );
            on_slope(
                b,
                [radius * 1.22, 1.6],
                [radius * 0.6, height * 0.8],
                0.28,
                |b| team_panel(b, Vec3::ZERO, v2(radius * 0.28, radius * 0.16)),
            );
        });
    });
    if b.fine() {
        if dark {
            b.paint(ACCENT);
            b.mirror_y(|b| b.block(v3(-radius * 1.05, 1.4, 0.3), v3(-radius * 0.7, 2.5, 1.8)));
        } else {
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
}

/// Sentinel: tech 1 point defence. Black bunker, white welded-box turret,
/// a conventional cannon (steel tube, white sleeve, dark bore) — no rails,
/// no glass, no emitters.
pub fn turret(b: &mut MeshBuilder, _tech: u8) {
    pedestal(b, 5.4, 5.2, true);
    b.set_turret_pivot(v3(0.0, 0.0, 5.2));
    b.with_part(part::TURRET, |b| {
        let (z0, z1) = (5.25, 8.55);
        let roof = Roof {
            rear: -2.7,
            front: 1.85,
            half_width: 1.85,
            z: z1,
        };
        b.paint(PLATING);
        if b.coarse() {
            b.frustum_open(
                v3(-0.25, 0.0, 5.2),
                v2(6.2, 5.0),
                v2(5.2, 4.2),
                z1 - 5.2,
                v2(-0.2, 0.0),
            );
        } else {
            // Near-vertical sides: a welded box, not a sloped rail house.
            let plan = chamfered_rect(v2(2.85, 2.3), 0.45);
            b.loft_z(
                &plan,
                &[
                    Section::new(z0, 0.96).shifted(-0.2, 0.0),
                    Section::new(z0 + 1.2, 1.0).shifted(-0.2, 0.0),
                    Section::new(z1, 0.88).shifted(-0.3, 0.0),
                ],
            );
        }
        // The bunker buttresses end near x=6.6, so the muzzle has to clear
        // them by a lot or the gun reads as a stub. A steel tube, not a
        // panel-mapped black one: seams wrapping a cylinder look like dirt.
        let (breech, muzzle) = (v3(1.1, 0.0, 7.4), v3(10.0, 0.0, 7.4));
        cannon(b, breech, muzzle, 0.32, Emitter::Unlit);
        if b.coarse() {
            team_panel(
                b,
                roof.at(0.18, 0.0),
                v2(roof.length() * 0.32, roof.half_width * 1.2),
            );
            return;
        }
        // Raised roof and cheek plates so the armour reads from above.
        b.paint(PLATING);
        b.plate(
            roof.at(0.52, 0.0),
            v2(roof.length() * 0.52, roof.half_width * 1.5),
            0.12,
            0.04,
        );
        b.mirror_y(|b| {
            b.plate(
                roof.at(0.42, 0.62),
                v2(roof.length() * 0.32, roof.half_width * 0.34),
                0.09,
                0.03,
            )
        });
        b.mirror_y(|b| b.block(v3(-1.5, 2.22, 5.7), v3(0.9, 2.42, 7.5)));
        team_panel(
            b,
            roof.at(0.16, 0.0),
            v2(roof.length() * 0.28, roof.half_width * 1.2),
        );
        // Turret ring and a thick steel mantlet.
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, 5.12), 8, 2.5, 2.5, 0.2);
        b.chamfered_box(v3(2.2, 0.0, 7.4), v3(1.05, 2.15, 1.55), 0.18);
        if !b.fine() {
            return;
        }
        b.paint(ACCENT);
        b.cylinder_between(v3(2.55, 0.0, 7.4), v3(3.15, 0.0, 7.4), 0.5, 0.4, 8);
        // Gunner's sight: a slit in a steel box, not a glass blister.
        b.block(v3(1.55, -1.7, 7.7), v3(2.25, -1.05, 8.25));
        b.block(v3(2.25, -1.6, 7.84), v3(2.3, -1.15, 8.12));
        // Hatch and a bare whip — no lit tip.
        let hatch = roof.at(0.4, 0.42);
        b.paint(PLATING);
        b.prism(hatch, 8, 0.52, 0.44, 0.22);
        b.paint(ACCENT);
        b.prism(hatch + Vec3::Z * 0.22, 8, 0.36, 0.3, 0.06);
        whip(b, roof.at(-0.02, 0.58), 1.2, 0.1);
        // Rear bustle: ammo bins, not capacitors.
        b.paint(ACCENT);
        b.block(v3(-3.05, -1.5, 5.4), v3(-1.95, 1.5, 7.7));
        b.mirror_y(|b| {
            b.block(v3(-3.1, 0.3, 5.75), v3(-3.0, 1.15, 7.35));
            b.block(v3(-2.7, 1.5, 5.65), v3(-2.15, 1.65, 7.2));
        });
    });
}

/// Bevelled armour from a convex outline in the XY plane, sitting at `z`.
fn armour(b: &mut MeshBuilder, outline: &[[f32; 2]], z: f32, thickness: f32, bevel: f32) {
    let ring = |pts: &[[f32; 2]], z: f32| pts.iter().map(|p| v3(p[0], p[1], z)).collect::<Vec<_>>();
    if b.fine() && bevel > 0.0 {
        b.loft(
            &[
                ring(outline, z),
                ring(outline, z + thickness - bevel),
                ring(&inset(outline, bevel), z + thickness),
            ],
            false,
            true,
        );
    } else {
        b.loft(
            &[ring(outline, z), ring(outline, z + thickness)],
            false,
            true,
        );
    }
}

/// Inset a CCW convex ring by `d` metres.
fn inset(pts: &[[f32; 2]], d: f32) -> Vec<[f32; 2]> {
    let n = pts.len();
    (0..n)
        .map(|i| {
            let prev = Vec2::from_array(pts[(i + n - 1) % n]);
            let cur = Vec2::from_array(pts[i]);
            let next = Vec2::from_array(pts[(i + 1) % n]);
            let e1 = (cur - prev).normalize_or_zero();
            let e2 = (next - cur).normalize_or_zero();
            let n1 = Vec2::new(-e1.y, e1.x);
            let n2 = Vec2::new(-e2.y, e2.x);
            let bis = (n1 + n2).normalize_or_zero();
            let miter = d / bis.dot(n1).max(0.35);
            let p = cur + bis * miter;
            [p.x, p.y]
        })
        .collect()
}

/// Bastion keep: black frame, four pointed outworks. White armour follows
/// those faces — trapezoids on the keep, a pointed shell on each outwork —
/// black only as a rim. T2 blue conduits run the cardinal faces.
fn bastion_keep(b: &mut MeshBuilder) {
    let (radius, height) = (10.6, 6.6);
    if b.coarse() {
        b.paint(ACCENT);
        b.frustum_open(
            v3(0.0, 0.0, 0.0),
            v2(radius * 1.9, radius * 1.9),
            v2(radius * 1.05, radius * 1.05),
            height,
            v2(0.0, 0.0),
        );
        b.paint(PLATING);
        b.plate(
            v3(0.0, 0.0, height - 0.1),
            v2(radius * 0.72, radius * 0.72),
            0.18,
            0.04,
        );
        return;
    }
    // Low black octagonal plinth, then a steep keep — a fort, not a grey cone.
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, 0.0), 8, radius * 1.06, radius * 1.0, 0.4);
    b.loft_z(
        &ngon(8, radius),
        &[
            Section::new(0.38, 1.0),
            Section::new(height * 0.28, 0.96),
            Section::new(height * 0.58, 0.82),
            Section::new(height - 0.62, 0.7),
        ],
    );
    // Parapet and the race the turret sits in.
    b.prism(
        v3(0.0, 0.0, height - 0.62),
        8,
        radius * 0.64,
        radius * 0.58,
        0.5,
    );
    b.prism(
        v3(0.0, 0.0, height - 0.18),
        8,
        radius * 0.5,
        radius * 0.48,
        0.22,
    );
    // One trapezoid per cardinal face, matching the keep's taper. The conduit
    // sits proud on the plate, not in a seam between two tiles.
    let apothem = radius * FRAC_PI_8.cos();
    b.radial(4, |b| {
        b.paint(PLATING);
        on_slope(
            b,
            [apothem, 0.52],
            [apothem * 0.7, height - 0.72],
            0.5,
            |b| {
                let ring = |dx: f32, y_hi: f32, y_lo: f32, z: f32| {
                    vec![
                        v3(-dx, -y_hi, z),
                        v3(dx, -y_lo, z),
                        v3(dx, y_lo, z),
                        v3(-dx, y_hi, z),
                    ]
                };
                let (dx, y_hi, y_lo, z1, bevel) = (2.70, 2.75, 3.92, 0.14, 0.04);
                if b.fine() {
                    b.loft(
                        &[
                            ring(dx, y_hi, y_lo, 0.0),
                            ring(dx, y_hi, y_lo, z1 - bevel),
                            ring(dx - bevel, y_hi - bevel, y_lo - bevel, z1),
                        ],
                        false,
                        true,
                    );
                } else {
                    b.loft(
                        &[ring(dx, y_hi, y_lo, 0.0), ring(dx, y_hi, y_lo, z1)],
                        false,
                        true,
                    );
                }
            },
        );
    });
    // Pointed bastions: one shell per outwork, the same profile as the frame,
    // inset so it does not overhang the chamfer.
    b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
        b.radial(4, |b| {
            b.paint(ACCENT);
            b.extrude_y_chamfered(
                &[
                    [radius * 0.52, 0.0],
                    [radius * 1.3, 0.0],
                    [radius * 1.3, 1.15],
                    [radius * 0.98, 2.7],
                    [radius * 0.58, height * 0.78],
                ],
                radius * 0.24,
                radius * 0.06,
            );
            b.paint(PLATING);
            b.extrude_y(
                &[
                    [radius * 1.30, 0.18],
                    [radius * 1.30, 1.15],
                    [radius * 0.98, 2.70],
                    [radius * 0.64, height * 0.75],
                    [radius * 0.61, height * 0.78 + 0.06],
                    [radius * 0.98 + 0.05, 2.70 + 0.12],
                    [radius * 1.30 + 0.06, 1.15 + 0.12],
                    [radius * 1.30 + 0.13, 0.18],
                ],
                -radius * 0.175,
                radius * 0.175,
            );
            on_slope(
                b,
                [radius * 0.98, 2.7],
                [radius * 0.58, height * 0.78],
                0.42,
                |b| team_panel(b, v3(-0.15, 0.0, 0.14), v2(radius * 0.18, radius * 0.10)),
            );
        });
    });
    if b.fine() {
        // Conduits proud on the cardinal plates.
        b.radial(4, |b| {
            b.paint(GLOW);
            b.block(
                v3(radius * 0.86, -0.08, 0.7),
                v3(radius * 0.94, 0.08, height * 0.52),
            );
            b.paint(ACCENT);
            b.block(
                v3(radius * 0.84, -0.14, 0.55),
                v3(radius * 0.86, 0.14, height * 0.54),
            );
        });
    }
}

/// Bastion: tech 2 triple arc battery. Dark keep, a faceted rotating
/// house with three casemates, white plates on the black frame.
pub fn turret_heavy(b: &mut MeshBuilder, _tech: u8) {
    bastion_keep(b);
    b.set_turret_pivot(v3(0.0, 0.0, 6.6));
    let barrels = [-2.0, 0.0, 2.0];
    let (breech, muzzle, gun_z) = (4.0, 13.2, 10.0);
    b.with_part(part::TURRET, |b| {
        let roof = Roof {
            rear: -4.15,
            front: 1.55,
            half_width: 3.35,
            z: 11.25,
        };
        if b.coarse() {
            b.paint(ACCENT);
            b.frustum_open(
                v3(-0.5, 0.0, 6.65),
                v2(10.8, 10.0),
                v2(6.6, 6.0),
                4.6,
                v2(-0.5, 0.0),
            );
            // One wide tube that covers all three mouths at this distance.
            rail_gun(
                b,
                v3(breech, 0.0, gun_z),
                v3(muzzle, 0.0, gun_z),
                v2(1.6, 0.55),
                0.45,
                Emitter::Blue,
            );
            team_panel(
                b,
                roof.at(0.18, 0.0),
                v2(roof.length() * 0.3, roof.half_width * 1.4),
            );
            return;
        }
        // Battery house: blunt gun face, wide cheeks, clipped bustle —
        // a rotating casemate, not the tank turret plan.
        b.paint(ACCENT);
        let plan = [
            [4.45, -3.55],
            [4.45, 3.55],
            [1.7, 5.65],
            [-2.55, 5.45],
            [-5.7, 2.95],
            [-5.7, -2.95],
            [-2.55, -5.45],
            [1.7, -5.65],
        ];
        b.loft_z(
            &plan,
            &[
                Section::new(6.65, 0.84),
                Section::new(8.15, 1.0),
                Section::scaled(11.25, 0.68, 0.6).shifted(-0.62, 0.0),
            ],
        );
        b.prism(v3(0.0, 0.0, 6.52), 8, 4.7, 4.7, 0.24);
        // Short collars the rails fire out of — the shroud has to show, or
        // the battery reads as a casemate, not the Trebuchet's barrel.
        for y in barrels {
            b.at(v3(0.0, y, 0.0), |b| {
                b.paint(ACCENT);
                b.extrude_y_chamfered(
                    &[[3.55, 9.15], [5.15, 9.35], [5.15, 10.7], [3.75, 10.85]],
                    0.52,
                    0.10,
                );
            });
            // Same rail family as the Trebuchet, scaled down: not a siege piece.
            rail_gun(
                b,
                v3(breech, y, gun_z),
                v3(muzzle, y, gun_z),
                v2(0.30, 0.58),
                0.26,
                Emitter::Blue,
            );
            // Proud collar so the shroud reads from the RTS camera.
            b.paint(PLATING);
            b.chamfered_box(v3(5.55, y, gun_z), v3(2.15, 0.82, 0.88), 0.12);
        }
        // Light mantlet tying the three collars, a thin brow — not a visor.
        b.paint(ACCENT);
        b.chamfered_box(v3(3.85, 0.0, gun_z), v3(1.05, 5.15, 1.65), 0.20);
        b.extrude_y_chamfered(
            &[[2.45, 10.55], [4.85, 10.42], [4.55, 11.15], [2.25, 11.28]],
            4.25,
            0.18,
        );
        // Roof: one plate following the house plan, not a grid of squares.
        b.paint(PLATING);
        armour(
            b,
            &inset(
                &[
                    [2.40, -2.13],
                    [2.40, 2.13],
                    [0.54, 3.39],
                    [-2.35, 3.27],
                    [-4.50, 1.77],
                    [-4.50, -1.77],
                    [-2.35, -3.27],
                    [0.54, -3.39],
                ],
                0.12,
            ),
            11.25,
            0.13,
            0.04,
        );
        // Brow: one trapezoid on the visor, inset from the chamfer.
        b.extrude_y(
            &[[2.28, 11.22], [4.42, 11.10], [4.50, 11.22], [2.20, 11.34]],
            -3.85,
            3.85,
        );
        team_panel(
            b,
            roof.at(0.2, 0.0),
            v2(roof.length() * 0.26, roof.half_width * 0.45),
        );
        // Cheek sponsons: dark banks, one lid following the top.
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.extrude_y(
                &[[-4.35, 7.2], [2.05, 7.2], [1.1, 10.45], [-3.55, 10.45]],
                4.5,
                5.55,
            );
            b.paint(PLATING);
            armour(
                b,
                &[[-3.45, 4.58], [1.00, 4.58], [0.65, 5.46], [-3.15, 5.46]],
                10.45,
                0.11,
                0.03,
            );
            if b.fine() {
                b.paint(GLOW);
                b.extrude_y(
                    &[[-3.35, 8.05], [0.95, 8.05], [0.65, 9.3], [-2.95, 9.3]],
                    5.55,
                    5.62,
                );
            }
        });
        if b.fine() {
            // Rangefinder bar across the brow.
            b.paint(ACCENT);
            b.beam(
                v3(1.85, -3.55, 11.08),
                v3(1.85, 3.55, 11.08),
                v2(0.32, 0.22),
                v2(0.32, 0.22),
            );
            // Cupola offset on the rear deck, glass slit. No square lid.
            let cupola = roof.at(0.16, -0.38);
            b.paint(ACCENT);
            b.prism(cupola, 8, 1.12, 0.98, 0.58);
            b.paint(GLASS);
            b.block(
                cupola + v3(0.72, -0.38, 0.18),
                cupola + v3(0.98, 0.38, 0.46),
            );
            b.mirror_y(|b| {
                glow_strip(b, roof.at(0.68, 0.88), v2(roof.length() * 0.42, 0.16), GLOW)
            });
            b.mirror_y(|b| vent(b, roof.at(0.46, 0.22), v2(1.35, 0.72), 3, GLOW));
            antenna(b, roof.at(-0.06, 0.55), 2.15, 0.12);
            antenna(b, roof.at(-0.04, -0.62), 1.55, 0.1);
            // Rear bustle: stacked dark banks, clipped under the roof.
            b.paint(ACCENT);
            b.block(v3(-6.05, -1.55, 7.1), v3(-3.85, 1.55, 10.35));
            b.block(v3(-6.35, -1.15, 7.45), v3(-6.05, 1.15, 10.05));
            b.paint(GLOW);
            b.mirror_y(|b| b.block(v3(-6.38, 0.45, 7.7), v3(-6.32, 1.4, 9.85)));
        }
    });
}

/// Gun pit on a plus-shaped lot: magazine wings, a low berm the
/// tube fires over, a black race the mount sits in. The octagon is only
/// the well — the lot is a magazine, not a casemate.
fn howitzer_bunker(b: &mut MeshBuilder) {
    let height = 4.2;
    if b.coarse() {
        b.paint(ACCENT);
        b.frustum_open(
            v3(0.15, 0.0, 0.0),
            v2(18.6, 11.4),
            v2(11.8, 7.7),
            height,
            v2(-0.4, 0.0),
        );
        b.paint(PLATING);
        b.mirror_y(|b| b.cuboid_open(v3(0.15, 7.2, 1.2), v3(6.4, 3.5, 2.4)));
        return;
    }

    // Plus-shaped pad: a spine toward the face and a crossbar for the
    // magazines, so the lot is not a regular octagon.
    b.paint(ACCENT);
    b.loft_z(
        &chamfered_rect(v2(9.4, 5.2), 0.9),
        &[Section::new(0.0, 1.0), Section::new(0.32, 1.0)],
    );
    b.loft_z(
        &chamfered_rect(v2(4.7, 9.4), 0.9),
        &[Section::new(0.0, 1.0), Section::new(0.32, 1.0)],
    );
    b.loft_z(
        &chamfered_rect(v2(6.4, 5.2), 1.5),
        &[
            Section::new(0.30, 1.0),
            Section::new(3.2, 0.96),
            Section::new(height - 0.38, 0.9),
        ],
    );
    // Race the turret sits in — the only octagon, and a well, not the body.
    b.prism(v3(0.0, 0.0, height - 0.45), 8, 4.2, 4.2, 0.45);

    // Blast berm: a low face the tube fires over, not a casemate glacis.
    b.extrude_y_chamfered(
        &[[3.7, 0.0], [8.9, 0.0], [8.9, 0.95], [6.6, 2.2], [4.0, 2.85]],
        3.7,
        0.32,
    );
    on_slope(b, [8.9, 0.95], [6.6, 2.2], 0.55, |b| {
        b.paint(PLATING);
        b.plate(Vec3::ZERO, v2(2.0, 1.85), 0.12, 0.04);
        team_panel(b, v3(0.0, 0.0, 0.12), v2(1.15, 1.0));
    });

    // Magazine wings: ready ammo beside the pit, lockers not casemates.
    b.mirror_y(|b| {
        let at = v3(0.15, 7.2, 0.0);
        b.paint(ACCENT);
        if b.fine() {
            b.chamfered_box(at + Vec3::Z * 1.2, v3(6.4, 3.5, 2.4), 0.42);
        } else {
            b.cuboid(at + Vec3::Z * 1.2, v3(6.4, 3.5, 2.4));
        }
        b.paint(PLATING);
        b.plate(at + Vec3::Z * 2.42, v2(4.7, 2.35), 0.12, 0.04);
        b.block(v3(-2.85, 8.85, 0.4), v3(3.2, 8.98, 2.2));
        team_panel(b, at + Vec3::Z * 2.56, v2(2.2, 1.15));
    });

    // Rear ready-ammo locker, set back over the lot.
    b.paint(ACCENT);
    if b.fine() {
        b.chamfered_box(v3(-6.4, 0.0, 1.4), v3(4.5, 5.4, 2.8), 0.48);
    } else {
        b.cuboid(v3(-6.4, 0.0, 1.4), v3(4.5, 5.4, 2.8));
    }
    b.paint(PLATING);
    b.plate(v3(-6.4, 0.0, 2.82), v2(3.2, 3.7), 0.12, 0.04);
    b.block(v3(-8.67, -2.45, 0.35), v3(-8.54, 2.45, 2.6));
    b.mirror_y(|b| b.block(v3(-8.5, 2.6, 0.35), v3(-4.35, 2.74, 2.6)));

    // Narrow walkway plates around the race — not a white lid.
    b.paint(PLATING);
    b.mirror_y(|b| {
        b.plate(v3(1.35, 2.65, height - 0.36), v2(2.7, 1.35), 0.10, 0.03);
        b.plate(v3(-2.2, 2.65, height - 0.36), v2(2.2, 1.35), 0.10, 0.03);
    });

    if b.fine() {
        // Hatch lids and dark vents on the magazines — slats, not emitters.
        b.mirror_y(|b| {
            vent(b, v3(-1.5, 7.2, 2.56), v2(1.85, 1.1), 3, ACCENT);
            b.paint(ACCENT);
            b.plate(v3(1.75, 7.2, 2.54), v2(1.5, 1.0), 0.08, 0.03);
            b.block(v3(-1.2, 5.38, 0.5), v3(1.5, 5.5, 1.95));
            for i in 0..3 {
                let x = -0.85 + i as f32 * 0.95;
                b.block(v3(x, 5.5, 0.62), v3(x + 0.58, 5.68, 1.65));
            }
        });
        // Rear hatches, a ladder, a whip, a hoist that stays over the locker.
        b.paint(ACCENT);
        b.mirror_y(|b| {
            b.plate(v3(-5.9, 1.2, 2.92), v2(1.6, 1.1), 0.08, 0.03);
        });
        vent(b, v3(-6.9, 0.0, 2.92), v2(1.85, 1.2), 4, ACCENT);
        b.block(v3(-8.75, -0.32, 0.35), v3(-8.6, 0.32, 2.65));
        for i in 0..4 {
            let z = 0.55 + i as f32 * 0.48;
            b.block(v3(-8.82, -0.27, z), v3(-8.52, 0.27, z + 0.09));
        }
        b.beam(
            v3(-7.75, -1.7, 2.96),
            v3(-7.75, 1.7, 2.96),
            v2(0.28, 0.2),
            v2(0.28, 0.2),
        );
        b.chamfered_box(v3(-5.2, 0.0, 3.02), v3(1.25, 0.8, 0.38), 0.09);
        whip(b, v3(-7.55, 1.65, 2.92), 1.7, 0.12);
        // Blast trough down the berm.
        b.block(v3(5.4, -0.38, 1.05), v3(8.7, 0.38, 1.2));
    }
}

/// Onager: tech 2 static howitzer. An open gun on a magazine bunker —
/// poured lot, wing magazines, a berm the tube fires over. An arch
/// rises from the race; the A-frame sits on the crown and the tube
/// pitches about a trunnion there. Same barrel family as the rail
/// (shroud, taper, jacket) but a closed tube with a dark bore. No glow.
pub fn artillery_static(b: &mut MeshBuilder, _tech: u8) {
    howitzer_bunker(b);
    // Authored at 14 / 12, fitted to the 10.5 / 9 lot (×0.75). Pivot and
    // muzzle here must stay on that ratio so the sim and the mesh agree.
    let (breech, muzzle) = (v3(-2.4, 0.0, 7.2), v3(13.6, 0.0, 11.6));
    let elevation = (muzzle - breech).z.atan2((muzzle - breech).x);
    let pivot = v3(-0.4, 0.0, 7.8);
    let deck_z = 4.2;
    let half_width = 1.95;
    b.set_turret_pivot(v3(0.0, 0.0, deck_z));
    b.set_arm_pivot(pivot);
    b.set_recoil(breech, muzzle, 2.6);
    b.with_part(part::TURRET, |b| {
        b.with_limb(rig::ARM_GUN, |b| {
            b.with_recoil(|b| howitzer(b, breech, muzzle, 0.78));
            if !b.coarse() {
                siege_counterweight(b, v3(-4.35, 0.0, 7.15), v3(2.35, 2.4, 2.0));
                b.paint(ACCENT);
                b.mirror_y(|b| {
                    b.beam(
                        v3(-1.15, 0.68, 7.65),
                        v3(-3.35, 0.82, 7.05),
                        v2(0.20, 0.18),
                        v2(0.26, 0.22),
                    );
                });
            }
        });
        if b.coarse() {
            b.paint(PLATING);
            b.frustum_open(
                v3(-0.35, 0.0, deck_z),
                v2(5.6, 2.4),
                v2(1.15, 0.7),
                pivot.z - deck_z,
                v2(-0.05, 0.0),
            );
            team_panel(b, v3(pivot.x, 0.0, pivot.z + 0.35), v2(1.4, 0.7));
            return;
        }
        // Turntable the arch stands on — yaws with the gun.
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, deck_z), b.sides(8), 2.15, 2.08, 0.14);
        b.paint(PLATING);
        b.prism(v3(0.0, 0.0, deck_z + 0.14), b.sides(8), 2.02, 1.96, 0.12);
        team_panel(b, v3(0.0, 0.0, deck_z + 0.26), v2(1.45, 1.45));
        // Wide circular arch; the A-frame is a smaller saddle on the crown.
        siege_arch(b, deck_z + 0.26, pivot, half_width, 3.0, -3.8, 1.22);
        let crown = pivot.z;
        siege_a_frame(
            b,
            crown - 0.02,
            pivot,
            half_width * 0.72,
            pivot.x + 0.85,
            pivot.x - 0.85,
            pivot.x,
            crown + 0.38,
            0.92,
        );
        if b.fine() {
            // Cradle and recuperators pitch with the tube; only the tube slides.
            b.with_limb(rig::ARM_GUN, |b| {
                b.pitched(breech, elevation, |b| {
                    b.paint(ACCENT);
                    b.mirror_y(|b| {
                        b.cylinder_between(v3(0.4, 1.08, 0.74), v3(6.4, 1.08, 0.74), 0.20, 0.20, 6);
                    });
                    b.block(v3(-0.45, -1.2, -0.6), v3(1.9, 1.2, 0.6));
                });
                b.with_recoil(|b| {
                    b.pitched(breech, elevation, |b| {
                        b.paint(ACCENT);
                        b.mirror_y(|b| {
                            b.cylinder_between(
                                v3(-0.3, 1.08, 0.74),
                                v3(0.55, 1.08, 0.74),
                                0.12,
                                0.12,
                                6,
                            )
                        });
                    });
                });
            });
        }
    });
}

// ---- Watchtower: radar --------------------------------------------------------------

/// One AESA fin hugging the shaft: dark frame, white face, scan slots.
/// The plate follows the taper so the silhouette stays a spire.
fn scan_fin(b: &mut MeshBuilder, z0: f32, z1: f32, r0: f32, r1: f32, width: f32, lit: bool) {
    b.paint(METAL);
    b.beam(
        v3(r0 * 0.42, 0.0, (z0 + z1) * 0.5),
        v3(r0 - 0.06, 0.0, (z0 + z1) * 0.5),
        v2(0.14, 0.14),
        v2(0.1, 0.1),
    );
    b.paint(ACCENT);
    b.beam(
        v3(r0, 0.0, z0 + 0.04),
        v3(r1, 0.0, z1 - 0.04),
        v2(width, 0.18),
        v2(width * 0.68, 0.13),
    );
    b.paint(PLATING);
    b.beam(
        v3(r0 + 0.1, 0.0, z0 + 0.16),
        v3(r1 + 0.08, 0.0, z1 - 0.16),
        v2(width * 0.82, 0.08),
        v2(width * 0.56, 0.06),
    );
    if b.fine() {
        let slots = if z1 - z0 > 5.0 { 6 } else { 4 };
        b.paint(if lit { GLOW } else { METAL });
        for i in 0..slots {
            let t = (i as f32 + 0.5) / slots as f32;
            let z = z0 + (z1 - z0) * t;
            let r = r0 + (r1 - r0) * t;
            let w = width * (1.0 - 0.3 * t);
            b.cuboid(v3(r + 0.14, 0.0, z), v3(0.05, w * 0.62, (z1 - z0) * 0.055));
        }
        b.paint(PLATING);
        b.plate(v3(r1, 0.0, z1 - 0.02), v2(0.24, width * 0.58), 0.07, 0.02);
    }
}

/// Three fins around the mast. Each tier keeps a real segment, not a
/// shrinking ring.
fn scan_wreath(b: &mut MeshBuilder, z0: f32, z1: f32, r0: f32, r1: f32, yaw: f32, lit: bool) {
    let width = ((r0 + r1) * 0.46).clamp(0.9, 1.6);
    b.yawed(Vec3::ZERO, yaw, |b| {
        b.radial(3, |b| scan_fin(b, z0, z1, r0, r1, width, lit));
        scan_collar(b, z0, r0 * 0.55);
        scan_collar(b, z1 - 0.2, r1 * 0.7);
    });
}

/// A metal band on the shaft, not a deck around it.
fn scan_collar(b: &mut MeshBuilder, z: f32, radius: f32) {
    b.paint(METAL);
    b.prism(v3(0.0, 0.0, z), b.sides(8), radius, radius * 0.88, 0.22);
}

/// Red obstruction lamp on a short stalk.
fn beacon(b: &mut MeshBuilder, z: f32) {
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, z), 6, 0.13, 0.09, 0.32);
    b.paint(GLOW_RED);
    if b.fine() {
        b.spheroid(v3(0.0, 0.0, z + 0.46), v3(0.2, 0.2, 0.2), 6, 2);
    } else {
        b.cuboid(v3(0.0, 0.0, z + 0.46), Vec3::splat(0.36));
    }
}

/// Authored heights per tech: 20, 24, 28 m. A tapering watchtower spire.
/// The needle is the building; each tier bolts a full-size scan wreath
/// further up — a stack of segments, not a shrinking tip.
pub fn radar(b: &mut MeshBuilder, tech: u8) {
    const SPIRE: f32 = 19.2;
    // (z0, z1, r0, r1) — later tiers step in only a little.
    const T1: (f32, f32, f32, f32) = (7.2, 13.6, 2.10, 1.35);
    const T2: (f32, f32, f32, f32) = (14.2, 20.2, 1.90, 1.25);
    const T3: (f32, f32, f32, f32) = (20.6, 26.4, 1.70, 1.15);
    b.set_spinner_pivot(v3(0.0, 0.0, 12.6));
    if b.coarse() {
        b.paint(PLATING);
        let stack = SPIRE + 4.4 * (tech - 1) as f32;
        b.frustum_open(
            v3(0.0, 0.0, 0.0),
            v2(8.4, 8.4),
            v2(0.7, 0.7),
            stack,
            v2(0.0, 0.0),
        );
        team_panel(b, v3(0.0, 0.0, 2.8), v2(4.4, 4.4));
        b.with_part(part::SPINNER, |b| {
            b.paint(PLATING);
            let stages: &[(f32, f32, f32)] = match tech {
                1 => &[(10.4, 6.2, 1.9)],
                2 => &[(10.4, 6.2, 1.9), (17.2, 5.8, 1.7)],
                _ => &[(10.4, 6.2, 1.9), (17.2, 5.8, 1.7), (23.4, 5.6, 1.5)],
            };
            for (i, &(z, h, r)) in stages.iter().enumerate() {
                b.yawed(Vec3::ZERO, i as f32 * 2.1, |b| {
                    b.cuboid_open(v3(r, 0.0, z), v3(0.28, r * 0.9, h));
                });
            }
        });
        return;
    }

    // Dark pad and a white spire that pinches to a needle — never rebuilt.
    b.paint(ACCENT);
    b.loft_z(
        &ngon(b.sides(8), 5.5),
        &[Section::new(0.0, 1.0), Section::new(0.42, 0.96)],
    );
    b.paint(PLATING);
    b.loft_z(
        &ngon(b.sides(8), 3.55),
        &[
            Section::new(0.0, 1.0),
            Section::new(2.4, 0.88),
            Section::new(5.6, 0.48),
            Section::new(9.2, 0.38),
            Section::new(14.8, 0.26),
            Section::new(SPIRE, 0.075),
        ],
    );
    b.paint(PLATING);
    b.prism(v3(0.0, 0.0, 6.2), b.sides(8), 2.0, 1.85, 0.2);
    team_panel(b, v3(0.0, 0.0, 2.72), v2(3.6, 3.6));
    b.radial(3, |b| {
        b.paint(ACCENT);
        b.beam(
            v3(5.55, 0.0, 0.55),
            v3(1.55, 0.0, 6.25),
            v2(0.55, 0.55),
            v2(0.32, 0.32),
        );
    });

    kit(b, tech, 2, 0.18, |b| {
        b.paint(PLATING);
        b.prism(v3(0.0, 0.0, 15.15), b.sides(8), 1.15, 1.0, 0.2);
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.prism(v3(0.0, 5.7, 0.0), b.sides(8), 1.15, 1.0, 3.4);
            if b.fine() {
                b.paint(METAL);
                b.prism(v3(0.0, 5.7, 3.4), 6, 0.72, 0.55, 0.45);
                b.paint(GLOW);
                b.prism(v3(0.0, 5.7, 2.2), 8, 1.22, 1.22, 0.12);
                antenna(b, v3(0.35, 5.7, 3.85), 2.6, 0.08);
            }
        });
    });
    kit(b, tech, 3, 0.2, |b| {
        b.paint(PLATING);
        b.prism(v3(0.0, 0.0, 20.95), b.sides(8), 1.0, 0.88, 0.2);
        b.paint(PLATING);
        b.prism(v3(5.35, 0.0, 0.0), b.sides(8), 1.05, 0.88, 3.8);
        if b.fine() {
            b.paint(METAL);
            b.prism(v3(5.35, 0.0, 3.8), 6, 0.6, 0.42, 0.4);
            b.paint(GLOW);
            b.prism(v3(5.35, 0.0, 2.4), 8, 1.12, 1.12, 0.12);
            antenna(b, v3(5.35, 0.7, 4.2), 3.2, 0.12);
            antenna(b, v3(5.35, -0.7, 4.2), 2.5, 0.18);
        }
    });

    b.with_part(part::SPINNER, |b| {
        let (z0, z1, r0, r1) = T1;
        scan_wreath(b, z0, z1, r0, r1, 0.22, false);
        if tech < 2 {
            beacon(b, 19.15);
        }

        kit(b, tech, 2, 0.42, |b| {
            let (z0, z1, r0, r1) = T2;
            scan_wreath(b, z0, z1, r0, r1, 2.25, true);
            b.paint(METAL);
            b.prism(v3(0.0, 0.0, SPIRE), b.sides(6), 0.48, 0.32, 4.4);
            if tech < 3 {
                beacon(b, 23.75);
            }
        });
        kit(b, tech, 3, 0.42, |b| {
            let (z0, z1, r0, r1) = T3;
            scan_wreath(b, z0, z1, r0, r1, 4.28, true);
            b.paint(METAL);
            b.prism(v3(0.0, 0.0, 23.5), b.sides(6), 0.36, 0.22, 4.3);
            beacon(b, 27.85);
        });
    });

    if b.fine() {
        b.paint(GLOW);
        for (z, r) in [(3.9, 1.88), (5.25, 1.32), (6.5, 1.02)] {
            b.prism(v3(0.0, 0.0, z), 8, r, r, 0.16);
        }
        if tech >= 2 {
            b.prism(v3(0.0, 0.0, 15.15), 8, 0.85, 0.85, 0.14);
        }
        if tech >= 3 {
            b.prism(v3(0.0, 0.0, 4.15), 8, 2.15, 2.15, 0.16);
            b.prism(v3(0.0, 0.0, 20.85), 8, 0.78, 0.78, 0.1);
        }
        b.paint(GLASS);
        b.loft_z(
            &ngon(8, 3.55),
            &[Section::new(2.15, 0.94), Section::new(2.55, 0.91)],
        );
    }
}

// ---- Scavenger: reclaim plant ------------------------------------------------------

/// A squat reclaim plant: black bunker, grey plant kit, white plates
/// and an Aster turret set back over the lot. The processor sits in a
/// mantle with feed pipes around it — plant, not a point-defence lance.
/// Tech 2: the lights it has earned are the orange of reclaim, not a
/// rail's blue. Tech 3 (Scavenger II) is the same plant built up: collector
/// pylons in the corners, an armour skirt, a second processor drum on the
/// turret and a focusing fork that carries the beam out further.
pub fn reclaimer(b: &mut MeshBuilder, tech: u8) {
    let (radius, deck) = (10.6, 5.0);
    let (breech, muzzle) = (v3(-4.0, 0.0, 8.4), v3(16.0, 0.0, 8.6));
    b.set_turret_pivot(v3(0.0, 0.0, deck));
    if b.coarse() {
        b.paint(ACCENT);
        b.frustum_open(
            v3(0.0, 0.0, 0.0),
            v2(radius * 1.85, radius * 1.85),
            v2(radius * 1.15, radius * 1.15),
            deck,
            v2(0.0, 0.0),
        );
        b.paint(PLATING);
        b.plate(
            v3(0.0, 0.0, deck - 0.12),
            v2(radius * 0.95, radius * 0.95),
            0.16,
            0.04,
        );
        b.with_part(part::TURRET, |b| {
            b.paint(PLATING);
            b.cuboid(v3(-3.2, 0.0, 7.7), v3(8.4, 6.4, 4.0));
            reclaim_gun(b, breech, muzzle, 0.62);
            team_panel(b, v3(-3.4, 0.0, 9.85), v2(3.6, 4.0));
            if tech >= 3 {
                b.paint(METAL);
                b.cuboid(v3(-3.0, 0.0, 11.6), v3(6.0, 3.0, 3.0));
                b.paint(GLOW_ORANGE);
                b.cuboid(v3(13.6, 0.0, 8.6), v3(3.4, 2.4, 1.2));
            }
        });
        if tech >= 3 {
            b.paint(PLATING);
            b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
                b.radial(4, |b| b.cuboid_open(v3(RECLAIM_PYLON, 0.0, 6.8), v3(1.6, 1.6, 13.6)))
            });
        }
        return;
    }

    // Black bunker, grey ring, white walkway plates, four industrial feet.
    b.paint(ACCENT);
    b.loft_z(
        &ngon(b.sides(8), radius),
        &[
            Section::new(0.0, 1.0),
            Section::new(deck * 0.38, 0.94),
            Section::new(deck - 0.5, 0.66),
        ],
    );
    b.paint(PLATING_DARK);
    b.prism(
        v3(0.0, 0.0, deck - 0.5),
        8,
        radius * 0.58,
        radius * 0.58,
        0.5,
    );
    b.paint(PLATING);
    b.mirror_y(|b| {
        b.plate(
            v3(2.8, radius * 0.28, deck - 0.38),
            v2(4.8, radius * 0.24),
            0.14,
            0.05,
        );
        b.plate(
            v3(-3.6, radius * 0.28, deck - 0.38),
            v2(4.2, radius * 0.24),
            0.14,
            0.05,
        );
    });
    // Cardinal faces: a white plate on the taper, black in the seams.
    let apothem = radius * FRAC_PI_8.cos();
    b.radial(4, |b| {
        on_slope(
            b,
            [apothem * 0.94, deck * 0.38],
            [apothem * 0.66, deck - 0.5],
            0.5,
            |b| {
                b.paint(PLATING);
                b.plate(Vec3::ZERO, v2(radius * 0.42, radius * 0.28), 0.13, 0.04);
            },
        );
    });
    // Cable trays along the deck, dark so the plates stay the highlight.
    b.paint(ACCENT);
    b.mirror_y(|b| {
        b.block(v3(-4.8, 3.6, deck - 0.28), v3(4.4, 4.05, deck + 0.18));
        b.paint(METAL);
        b.block(v3(-4.6, 3.68, deck + 0.18), v3(4.2, 3.97, deck + 0.28));
    });
    b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
        b.radial(4, |b| {
            b.paint(ACCENT);
            b.extrude_y_chamfered(
                &[
                    [radius * 0.6, 0.0],
                    [radius * 1.22, 0.0],
                    [radius * 1.22, 1.6],
                    [radius * 0.6, deck * 0.82],
                ],
                radius * 0.2,
                radius * 0.05,
            );
            on_slope(
                b,
                [radius * 1.22, 1.6],
                [radius * 0.6, deck * 0.82],
                0.55,
                |b| {
                    b.paint(PLATING);
                    b.plate(Vec3::ZERO, v2(radius * 0.48, radius * 0.32), 0.14, 0.05);
                    team_panel(b, v3(0.0, 0.0, 0.14), v2(radius * 0.26, radius * 0.14));
                },
            );
        });
    });

    // Mass hoppers on the rear flanks: black bins, grey bands, white lids.
    b.mirror_y(|b| {
        let at = v3(-7.2, 7.0, 0.0);
        b.paint(ACCENT);
        if b.fine() {
            b.at(at, |b| {
                b.loft_z(
                    &ngon(8, 2.7),
                    &[
                        Section::new(0.0, 1.0),
                        Section::new(2.6, 1.0),
                        Section::new(3.4, 0.62),
                    ],
                );
                b.paint(METAL);
                b.prism(v3(0.0, 0.0, 1.4), 8, 2.84, 2.84, 0.38);
                b.paint(PLATING_DARK);
                b.prism(v3(0.0, 0.0, 2.55), 8, 2.84, 2.84, 0.26);
                b.paint(METAL);
                b.prism(v3(0.0, 0.0, 3.4), 8, 1.35, 1.05, 0.28);
            });
        } else {
            b.paint(METAL);
            b.cuboid(at + Vec3::Z * 1.7, v3(5.2, 5.2, 3.4));
        }
        b.paint(PLATING);
        b.plate(at + Vec3::Z * 3.55, v2(2.8, 2.8), 0.12, 0.05);
        b.block(
            v3(at.x - 1.7, at.y + 2.58, 0.45),
            v3(at.x + 1.7, at.y + 2.74, 3.15),
        );
    });

    // Charge banks sit on the deck, not a glowing shaft.
    b.mirror_y(|b| {
        b.paint(METAL);
        b.chamfered_box(v3(2.2, 5.6, deck + 0.55), v3(3.6, 2.2, 1.1), 0.22);
        b.paint(ACCENT);
        b.block(v3(0.8, 6.45, deck + 0.15), v3(3.6, 6.7, deck + 0.95));
        b.paint(PLATING);
        b.plate(v3(2.2, 5.6, deck + 1.10), v2(3.2, 1.9), 0.1, 0.03);
        if b.fine() {
            vent(b, v3(2.2, 5.6, deck + 1.20), v2(2.4, 1.2), 3, GLOW_ORANGE);
        }
    });

    // Tech 3: an armour skirt round the bunker, and collector pylons on the diagonals.
    kit(b, tech, 3, 0.15, |b| {
        b.paint(PLATING_DARK);
        b.loft_z(
            &ngon(8, radius + 1.0),
            &[Section::new(0.0, 1.0), Section::new(1.2, 1.0), Section::new(1.7, 0.93)],
        );
        if b.fine() {
            b.radial(4, |b| glow_strip(b, v3(radius + 0.4, 0.0, 1.7), v2(0.5, 3.2), GLOW_ORANGE));
        }
    });
    kit(b, tech, 3, 0.4, |b| {
        b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
            b.radial(4, |b| {
                let at = v3(RECLAIM_PYLON, 0.0, 0.0);
                b.paint(PLATING_DARK);
                b.prism(at, 6, 1.5, 1.3, 1.7);
                b.paint(METAL);
                b.prism(at + Vec3::Z * 1.7, 6, 0.75, 0.5, 10.6);
                b.paint(PLATING);
                b.prism(at + Vec3::Z * 3.4, 6, 1.05, 1.05, 1.2);
                b.paint(GLOW_ORANGE);
                for z in [6.6, 9.0] {
                    b.prism(at + Vec3::Z * z, 8, 1.1, 1.1, 0.35);
                }
                b.prism(at + Vec3::Z * 12.3, 6, 0.8, 0.3, 1.3);
                // A feed line from the foot into the bunker.
                b.paint(ACCENT);
                b.cylinder_between(at + Vec3::Z * 1.0, v3(radius * 0.7, 0.0, deck - 0.6), 0.32, 0.32, 6);
            });
        });
    });

    b.with_part(part::TURRET, |b| {
        // Tech 3: a second processor drum along the roof, and the focusing fork.
        kit(b, tech, 3, 0.65, |b| {
            b.paint(METAL);
            b.cylinder_between(v3(-6.4, 0.0, 11.7), v3(0.6, 0.0, 11.7), 1.55, 1.55, b.sides(10));
            b.paint(ACCENT);
            for x in [-5.4, -2.9, -0.4] {
                b.cylinder_between(v3(x, 0.0, 11.7), v3(x + 0.7, 0.0, 11.7), 1.7, 1.7, b.sides(10));
            }
            b.paint(PLATING);
            b.plate(v3(-2.9, 0.0, 13.25), v2(4.0, 1.4), 0.12, 0.04);
            if b.fine() {
                glow_strip(b, v3(-4.15, 0.0, 13.25), v2(0.9, 0.9), GLOW_ORANGE);
            }
            b.paint(ACCENT);
            b.cylinder_between(v3(0.6, 0.0, 11.7), v3(2.2, 0.0, 10.4), 0.5, 0.4, 6);
        });
        kit(b, tech, 3, 0.85, |b| {
            b.paint(GLOW_ORANGE);
            for x in [11.5, 12.6] {
                b.cylinder_between(v3(x, 0.0, 8.6), v3(x + 0.45, 0.0, 8.6), 1.25, 1.25, b.sides(10));
            }
            b.mirror_y(|b| {
                b.paint(PLATING);
                b.beam(v3(13.4, 1.05, 8.6), v3(RECLAIM_TIP, 0.6, 8.6), v2(0.5, 0.9), v2(0.3, 0.6));
                b.paint(GLOW_ORANGE);
                b.cuboid(v3(RECLAIM_TIP - 0.3, 0.6, 8.6), v3(0.6, 0.3, 0.5));
            });
        });
        // Head sits aft of the pivot so the processor is rooted in the bunker.
        b.at(v3(-3.0, 0.0, 0.0), |b| {
            b.paint(ACCENT);
            let roof = turret_shell(b, 10.2, 7.8, deck + 0.05, 10.2);
            b.paint(ACCENT);
            b.block(v3(-5.2, -2.05, 5.15), v3(-3.3, 2.05, 9.55));
            // Short mast so the mid LOD still fills the authored height.
            b.prism(roof.at(-0.04, 0.0) + Vec3::Z * 0.02, 6, 0.28, 0.18, 1.35);
            b.paint(PLATING);
            b.mirror_y(|b| {
                b.plate(
                    roof.at(0.36, 0.5),
                    v2(roof.length() * 0.44, roof.half_width * 0.46),
                    0.12,
                    0.04,
                )
            });
            team_panel(
                b,
                roof.at(0.14, 0.0),
                v2(roof.length() * 0.28, roof.half_width * 1.4),
            );
            b.mirror_y(|b| {
                b.paint(PLATING_DARK);
                b.extrude_y(
                    &[[-3.6, 5.75], [1.8, 5.75], [1.2, 8.85], [-3.0, 8.7]],
                    3.35,
                    4.2,
                );
                b.paint(PLATING);
                b.block(v3(-3.3, 4.18, 6.05), v3(1.4, 4.34, 8.55));
            });
            if b.fine() {
                b.mirror_y(|b| {
                    glow_strip(
                        b,
                        roof.at(0.58, 0.88),
                        v2(roof.length() * 0.4, 0.14),
                        GLOW_ORANGE,
                    )
                });
                b.mirror_y(|b| vent(b, roof.at(0.48, 0.38), v2(1.1, 0.7), 3, GLOW_ORANGE));
                b.mirror_y(|b| {
                    b.paint(GLOW_ORANGE);
                    b.extrude_y(
                        &[[-2.6, 6.45], [1.0, 6.45], [0.65, 8.05], [-2.25, 7.9]],
                        4.2,
                        4.26,
                    );
                });
                b.paint(ACCENT);
                b.block(v3(3.15, -0.85, 8.85), v3(3.85, 0.85, 9.65));
                b.block(v3(3.85, -0.42, 9.05), v3(3.95, 0.42, 9.45));
                whip(b, roof.at(-0.06, 0.5), 2.6, 0.1);
                whip(b, roof.at(-0.06, -0.42), 2.0, 0.16);
            }
        });
        reclaim_gun(b, breech, muzzle, 0.62);
        // Mantle and feed trunks: the barrel is a processor, not a bare tube.
        b.paint(METAL);
        b.chamfered_box(v3(1.15, 0.0, 8.5), v3(3.4, 5.1, 3.8), 0.34);
        b.paint(PLATING);
        b.plate(v3(1.15, 0.0, 10.38), v2(2.8, 4.2), 0.12, 0.04);
        b.paint(METAL);
        b.cylinder_between(
            v3(2.55, -2.6, 8.5),
            v3(2.55, 2.6, 8.5),
            0.62,
            0.62,
            b.sides(8),
        );
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.cylinder_between(v3(-2.4, 3.8, 7.3), v3(0.8, 1.7, 8.55), 0.46, 0.34, 6);
            b.paint(METAL);
            b.cylinder_between(v3(-1.2, 3.5, 6.5), v3(2.2, 1.8, 8.1), 0.24, 0.18, 6);
        });
        if b.fine() {
            b.paint(PLATING_DARK);
            b.chamfered_box(v3(-0.5, 0.0, 10.72), v3(2.2, 1.6, 0.52), 0.12);
        }
    });
    if b.fine() {
        // Conduits from the hoppers into the bunker, a rear ladder, wall kit.
        b.mirror_y(|b| {
            b.paint(METAL);
            b.cylinder_between(v3(-5.6, 7.0, 2.8), v3(-2.8, 4.2, 4.2), 0.22, 0.22, 6);
            b.cylinder_between(v3(-2.8, 4.2, 4.2), v3(-0.6, 2.0, 5.05), 0.22, 0.18, 6);
            b.paint(ACCENT);
            b.block(v3(-8.6, -0.55, 0.3), v3(-8.35, 0.55, 4.7));
            for i in 0..6 {
                let z = 0.55 + i as f32 * 0.68;
                b.block(v3(-8.7, -0.42, z), v3(-8.25, 0.42, z + 0.12));
            }
            b.paint(ACCENT);
            b.block(v3(-8.5, 2.6, 0.3), v3(-6.4, 4.5, 1.9));
            b.paint(ACCENT);
            b.block(v3(-8.55, 2.85, 0.55), v3(-8.42, 3.35, 1.65));
            b.block(v3(-8.55, 3.7, 0.55), v3(-8.42, 4.2, 1.65));
        });
        b.mirror_y(|b| {
            b.paint(GLOW_ORANGE);
            b.block(v3(-0.2, 7.96, 1.5), v3(2.6, 8.08, 3.5));
        });
        glow_strip(b, v3(12.2, 0.0, 0.0), v2(1.1, 3.6), GLOW_ORANGE);
        glow_strip(b, v3(-12.2, 0.0, 0.0), v2(1.1, 3.6), GLOW_ORANGE);
        glow_strip(b, v3(0.0, 12.2, 0.0), v2(3.6, 1.1), GLOW_ORANGE);
        glow_strip(b, v3(0.0, -12.2, 0.0), v2(3.6, 1.1), GLOW_ORANGE);
    }
}

/// How far out on the diagonal the tech 3 Scavenger's pylons stand, and where its fork
/// ends: short of the muzzle, which is the lot's edge.
const RECLAIM_PYLON: f32 = 14.4;
const RECLAIM_TIP: f32 = 15.8;

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

// ---- Aegis: shield generator -------------------------------------------------

/// Authored at the tech 2 size (radius 16.5, height 40) on a 3×3 lot. A hex
/// generator pad — dark plinth, white terrace, capacitor banks — and a white
/// hexagonal column around a spinning crystal. The projector wreath orbits
/// the shaft; a crown of emitter petals aims the field up. Tech 3 bolts on a
/// second wreath, pad pods, and an upper drum — the T2 hull is never stretched.
pub fn shield(b: &mut MeshBuilder, tech: u8) {
    // Regular hex, circumradius under the 3×3 lot (18 m half-extent).
    const PAD_R: f32 = 15.4;
    const SHOULDER: f32 = 5.4;
    const SHAFT: f32 = 31.2;
    const TIP: f32 = 40.0;
    const SPIN: f32 = 18.6;
    b.set_spinner_pivot(v3(0.0, 0.0, SPIN));
    if b.coarse() {
        b.paint(ACCENT);
        b.frustum_open(
            v3(0.0, 0.0, 0.0),
            v2(30.0, 30.0),
            v2(18.0, 18.0),
            5.2,
            v2(0.0, 0.0),
        );
        b.paint(PLATING);
        b.frustum_open(
            v3(0.0, 0.0, 5.0),
            v2(11.2, 11.2),
            v2(7.6, 7.6),
            34.8,
            v2(0.0, 0.0),
        );
        team_panel(b, v3(0.0, 0.0, 5.1), v2(6.4, 6.4));
        b.with_part(part::SPINNER, |b| {
            b.paint(GLOW);
            b.cuboid(v3(0.0, 0.0, 18.4), v3(3.6, 3.6, 18.0));
            b.paint(METAL);
            b.cuboid(v3(0.0, 0.0, SPIN), v3(14.4, 14.4, 0.8));
        });
        if tech >= 3 {
            b.paint(PLATING);
            b.frustum_open(
                v3(0.0, 0.0, 39.6),
                v2(5.4, 5.4),
                v2(3.8, 3.8),
                11.6,
                v2(0.0, 0.0),
            );
        }
        return;
    }

    let pad = ngon(6, PAD_R);
    let apothem = PAD_R * FRAC_PI_6.cos();

    // Dark hex plinth, then a white terrace that steps in — a shoulder, not
    // a slope into a needle. The rim between them holds the banks.
    b.paint(ACCENT);
    b.loft_z(&pad, &[Section::new(0.0, 1.0), Section::new(1.4, 0.96)]);
    b.paint(PLATING);
    b.loft_z(
        &pad,
        &[
            Section::new(1.35, 0.88),
            Section::new(2.8, 0.74),
            Section::new(4.2, 0.62),
            Section::new(SHOULDER, 0.52),
        ],
    );
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, SHOULDER - 0.18), 6, 8.4, 8.0, 0.36);
    team_panel(b, v3(0.0, -7.2, SHOULDER + 0.18), v2(2.4, 1.3));
    // White plates on the dark plinth wall — contrast, not white-on-white.
    b.yawed(Vec3::ZERO, TAU / 6.0, |b| {
        b.radial(3, |b| {
            on_slope(b, [apothem, 0.15], [apothem * 0.96, 1.3], 0.5, |b| {
                b.paint(PLATING);
                b.cuboid(v3(0.0, 0.0, 0.1), v3(1.0, 6.2, 0.2));
            });
        });
    });

    // Capacitor banks on the flats, sitting on the dark rim.
    b.radial(6, |b| {
        if !b.fine() {
            b.paint(ACCENT);
            b.cuboid_open(v3(11.6, 0.0, 1.5), v3(3.0, 3.0, 3.0));
            return;
        }
        b.paint(ACCENT);
        b.prism(v3(11.6, 0.0, 0.0), 6, 1.55, 1.35, 3.0);
        b.paint(GLOW);
        b.prism(v3(11.6, 0.0, 3.0), 6, 0.85, 0.55, 0.42);
    });

    // Three flying buttresses from every other bank into the lower shaft.
    b.radial(3, |b| {
        b.paint(ACCENT);
        b.beam(
            v3(10.4, 0.0, 2.6),
            v3(4.8, 0.0, 13.2),
            v2(1.05, 0.75),
            v2(0.5, 0.4),
        );
        b.paint(PLATING);
        b.beam(
            v3(9.8, 0.0, 3.4),
            v3(5.0, 0.0, 12.2),
            v2(0.58, 0.2),
            v2(0.3, 0.16),
        );
    });

    // White hexagonal column. It holds its width so the crystal reads as a
    // core, not as the leftover of a cone.
    b.paint(PLATING);
    b.loft_z(
        &ngon(6, 5.4),
        &[
            Section::new(SHOULDER, 1.0),
            Section::new(12.0, 0.97),
            Section::new(20.0, 0.94),
            Section::new(SHAFT, 0.90),
        ],
    );
    b.paint(ACCENT);
    b.loft_z(
        &ngon(6, 4.55),
        &[
            Section::new(SHOULDER + 0.15, 1.0),
            Section::new(14.0, 0.96),
            Section::new(SHAFT - 0.4, 0.90),
        ],
    );

    // Faceted crystal: a column that spins inside the cage.
    b.with_part(part::SPINNER, |b| {
        b.paint(GLOW);
        b.prism(
            v3(0.0, 0.0, SHOULDER + 0.5),
            6,
            2.35,
            1.95,
            SHAFT - SHOULDER - 2.4,
        );
        b.paint(GLOW);
        b.prism(v3(0.0, 0.0, SHAFT - 0.6), 6, 1.55, 1.15, 2.4);
    });

    // Crown: a collar and a throat, then petals that aim the field up.
    b.paint(METAL);
    b.prism(v3(0.0, 0.0, SHAFT - 0.35), 6, 5.05, 5.2, 0.55);
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, SHAFT + 0.15), 6, 3.5, 2.7, 1.7);
    b.paint(PLATING);
    b.prism(v3(0.0, 0.0, SHAFT + 1.7), 6, 2.85, 2.35, 2.4);
    b.paint(GLOW);
    b.prism(v3(0.0, 0.0, SHAFT + 1.9), 6, 1.45, 1.05, 3.4);
    b.paint(METAL);
    b.prism(v3(0.0, 0.0, TIP - 1.35), 6, 2.55, 2.2, 0.4);

    if b.fine() {
        // Vertical cage ribs and slit windows onto the crystal.
        b.radial(6, |b| {
            b.paint(ACCENT);
            b.beam(
                v3(4.85, 0.0, SHOULDER + 1.0),
                v3(4.35, 0.0, SHAFT - 0.5),
                v2(0.3, 0.22),
                v2(0.2, 0.14),
            );
            b.paint(GLASS);
            b.prism(v3(4.05, 0.0, 11.2), 4, 0.52, 0.44, 5.4);
            b.prism(v3(3.85, 0.0, 20.4), 4, 0.44, 0.38, 4.6);
        });
        b.radial(6, |b| {
            b.paint(PLATING);
            b.pitched(v3(3.15, 0.0, SHAFT + 2.4), 1.05, |b| {
                b.extrude_y_chamfered(
                    &[[-0.4, 0.0], [1.55, 0.0], [1.2, 0.95], [-0.22, 1.15]],
                    0.4,
                    0.12,
                );
                glow_strip(b, v3(0.45, 0.0, 1.05), v2(1.35, 0.14), GLOW);
            });
        });
        antenna(b, v3(-1.4, 1.1, SHAFT + 3.4), 2.0, 0.12);
    }

    b.with_part(part::SPINNER, |b| {
        projector_wreath(b, SPIN, 6.8, 1.0);
        kit(b, tech, 3, 0.42, |b| {
            b.yawed(Vec3::ZERO, FRAC_PI_6, |b| {
                // A second ring of pods, not a second full wreath — T3 has to
                // stay inside the mesh budget.
                b.paint(METAL);
                b.prism(v3(0.0, 0.0, 26.2), 6, 6.5, 6.1, 0.28);
                b.radial(6, |b| {
                    b.paint(ACCENT);
                    b.prism(v3(6.3, 0.0, 25.85), 6, 0.58, 0.42, 0.7);
                    b.paint(GLOW);
                    b.prism(v3(6.3, 0.0, 26.5), 6, 0.32, 0.2, 0.24);
                });
            });
        });
    });

    kit(b, tech, 3, 0.22, |b| {
        if b.fine() {
            b.yawed(Vec3::ZERO, FRAC_PI_6, |b| {
                b.radial(6, |b| {
                    b.paint(ACCENT);
                    b.prism(v3(12.8, 0.0, 0.0), 6, 1.45, 1.2, 3.6);
                    b.paint(GLOW);
                    b.prism(v3(12.8, 0.0, 3.6), 6, 0.85, 0.55, 0.55);
                });
            });
        }
        // An upper drum bolted onto the T2 crown — still a column, not a spike.
        b.paint(PLATING);
        b.loft_z(
            &ngon(6, 2.35),
            &[
                Section::new(TIP - 0.35, 1.0),
                Section::new(TIP + 6.2, 0.88),
                Section::new(TIP + 10.8, 0.74),
            ],
        );
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, TIP + 4.8), 6, 2.15, 2.05, 0.45);
        b.paint(GLOW);
        b.prism(v3(0.0, 0.0, TIP + 10.2), 6, 1.15, 0.72, 1.6);
        if b.fine() {
            antenna(b, v3(0.8, -0.6, TIP + 10.4), 1.5, 0.14);
            antenna(b, v3(-0.5, 0.9, TIP + 10.4), 1.2, 0.08);
        }
    });
}

/// Projector wreath: a metal ring of emitter pods around the spire. Tagged
/// `SPINNER` by the caller so the ring orbits without the cage turning with it.
fn projector_wreath(b: &mut MeshBuilder, z: f32, ring: f32, scale: f32) {
    b.paint(METAL);
    b.prism(
        v3(0.0, 0.0, z - 0.22 * scale),
        6,
        ring,
        ring - 0.45 * scale,
        0.32 * scale,
    );
    b.paint(ACCENT);
    b.prism(
        v3(0.0, 0.0, z - 0.55 * scale),
        6,
        ring * 0.22,
        ring * 0.18,
        0.7 * scale,
    );
    if !b.fine() {
        b.radial(6, |b| {
            b.paint(ACCENT);
            b.cuboid(
                v3(ring - 0.15 * scale, 0.0, z),
                v3(1.4 * scale, 1.4 * scale, 1.1 * scale),
            );
        });
        return;
    }
    b.radial(6, |b| {
        b.paint(ACCENT);
        b.prism(
            v3(ring - 0.15 * scale, 0.0, z - 0.55 * scale),
            6,
            0.72 * scale,
            0.52 * scale,
            0.85 * scale,
        );
        b.paint(GLOW);
        b.prism(
            v3(ring - 0.15 * scale, 0.0, z + 0.22 * scale),
            6,
            0.38 * scale,
            0.22 * scale,
            0.28 * scale,
        );
        b.paint(PLATING);
        b.pitched(v3(ring * 0.92, 0.0, z + 0.15 * scale), 0.55, |b| {
            b.extrude_y_chamfered(
                &[
                    [-0.9 * scale, 0.0],
                    [2.4 * scale, 0.0],
                    [2.0 * scale, 1.15 * scale],
                    [-0.5 * scale, 1.55 * scale],
                ],
                0.42 * scale,
                0.14 * scale,
            );
            glow_strip(
                b,
                v3(0.55 * scale, 0.0, 1.25 * scale),
                v2(2.1 * scale, 0.16 * scale),
                GLOW,
            );
        });
    });
}
