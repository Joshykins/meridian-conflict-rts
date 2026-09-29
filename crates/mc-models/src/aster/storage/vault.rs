//! The Materials Vault (mesh `storage_mass`): four vats round a manifold, built up in
//! place tier on tier.

use std::f32::consts::FRAC_PI_4;

use glam::{Vec2, Vec3};

use super::super::parts::*;
use super::super::structures::kit;
use super::{fill, status_lamp};
use crate::builder::{chamfered_rect, ngon, MeshBuilder, Section};
use crate::material::*;

/// The Materials Vault, authored on a 3×3 lot (radius 16.5, lot half-extent 18) and
/// built up in place, tier on tier; the vats never move.
/// - Tech 1 (8 m): four squat vats round a manifold on an armoured slab.
/// - Tech 2 (13 m): a second stage on every vat, a loading tower in the middle,
///   gantries out to the vats and loaders on the four sides.
/// - Tech 3 (19 m): an armoured strongroom silo over the tower, corner pylons
///   feeding it, and banded armour round the vats.
pub(in crate::aster) fn storage_mass(b: &mut MeshBuilder, tech: u8) {
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
    b.radial(4, |b| {
        team_panel(b, v3(15.2, 0.0, VAULT_DECK), v2(0.8, 12.0))
    });

    // Four vats on the diagonals, banded, with a lid and a light.
    b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
        b.radial(4, |b| {
            b.at(v3(VAULT_VAT_OUT, 0.0, 0.0), |b| {
                b.paint(PLATING);
                b.loft_z(
                    &ngon(b.sides(VAT_SIDES), VAULT_VAT_R),
                    &[
                        Section::new(VAULT_DECK, 1.0),
                        Section::new(6.4, 1.0),
                        Section::new(7.6, 0.72),
                    ],
                );
                b.paint(ACCENT);
                band(b, 2.8, VAULT_VAT_R + 0.12, 0.9);
                b.paint(METAL);
                b.prism(v3(0.0, 0.0, 7.6), 8, 2.4, 2.0, 0.4);
                if b.fine() {
                    glow_strip(b, v3(0.0, 0.0, 8.0), v2(1.6, 0.5), GLOW);
                }
                // The level gauge: lit rings up the vat, the materials rising in them.
                for (j, z) in [2.1, 4.1, 5.95].into_iter().enumerate() {
                    fill(b, level(j, tech), |b| {
                        gauge_ring(b, z, VAULT_VAT_R + 0.08, 0.35)
                    });
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
            b.cylinder_between(
                v3(3.4, 0.0, 5.2),
                v3(VAULT_VAT_OUT - VAULT_VAT_R + 0.2, 0.0, 3.4),
                0.7,
                0.7,
                6,
            );
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
            b.beam(
                v3(11.0, 0.0, 4.6),
                v3(3.8, 0.0, 6.6),
                v2(1.6, 0.8),
                v2(1.4, 0.7),
            );
        });
    });
    kit(b, tech, 2, 0.35, |b| {
        b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
            b.radial(4, |b| {
                b.at(v3(VAULT_VAT_OUT, 0.0, 0.0), |b| {
                    b.paint(PLATING_DARK);
                    b.prism(v3(0.0, 0.0, 7.6), b.sides(VAT_SIDES), 3.9, 3.9, 0.6);
                    b.paint(PLATING);
                    b.loft_z(
                        &ngon(b.sides(VAT_SIDES), 3.7),
                        &[
                            Section::new(8.2, 1.0),
                            Section::new(10.6, 1.0),
                            Section::new(11.4, 0.7),
                        ],
                    );
                    b.paint(METAL);
                    band(b, 9.2, 3.82, 0.5);
                    fill(b, level(3, tech), |b| gauge_ring(b, 10.0, 3.78, 0.3));
                });
            });
        });
    });
    kit(b, tech, 2, 0.6, |b| {
        // The loading tower rises out of the manifold.
        b.paint(PLATING);
        b.loft_z(
            &chamfered_rect(v2(3.0, 3.0), 1.0),
            &[
                Section::new(6.9, 1.0),
                Section::new(11.8, 1.0),
                Section::new(12.6, 0.8),
            ],
        );
        b.paint(ACCENT);
        b.chamfered_box(v3(0.0, 0.0, 9.4), v3(6.3, 6.3, 1.0), 0.3);
        team_panel(b, v3(0.0, 0.0, 12.6), v2(3.0, 3.0));
        if b.fine() {
            b.radial(4, |b| {
                glow_strip(b, v3(3.02, 0.0, 10.4), v2(0.12, 3.6), GLOW)
            });
        }
    });
    kit(b, tech, 2, 0.85, |b| {
        // Gantries from the tower out over the vat tops.
        b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
            b.radial(4, |b| {
                b.paint(ACCENT);
                b.beam(
                    v3(3.0, 0.0, 11.0),
                    v3(VAULT_VAT_OUT, 0.0, 11.2),
                    v2(1.2, 0.8),
                    v2(1.0, 0.7),
                );
                b.paint(METAL);
                b.cylinder_between(
                    v3(VAULT_VAT_OUT, 0.0, 11.2),
                    v3(VAULT_VAT_OUT, 0.0, 12.4),
                    0.9,
                    0.7,
                    6,
                );
            });
        });
    });

    // ---- Tech 3: the strongroom, corner pylons and banded armour.
    kit(b, tech, 3, 0.1, |b| {
        b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
            b.radial(4, |b| {
                b.at(v3(VAULT_VAT_OUT, 0.0, 0.0), |b| {
                    b.paint(ACCENT);
                    // Relief, left to the full level.
                    if b.fine() {
                        for z in [4.6, 8.4] {
                            band(b, z, VAULT_VAT_R + 0.3, 1.1);
                        }
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
                if tech >= 3 {
                    status_lamp(b, at + Vec3::Z * 14.2, 0.75);
                } else {
                    b.paint(PLATING_DARK);
                    b.prism(at + Vec3::Z * 14.2, 6, 0.9, 0.5, 1.0);
                }
                if b.fine() {
                    b.paint(METAL);
                    b.cylinder_between(at + Vec3::Z * 13.0, v3(5.4, 0.0, 15.4), 0.3, 0.3, 6);
                }
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
        fill(b, level(4, tech), |b| {
            b.prism(v3(0.0, 0.0, 15.8), 8, 5.7, 5.7, 0.35)
        });
        fill(b, level(5, tech), |b| {
            b.prism(v3(0.0, 0.0, 17.0), 8, 4.9, 4.7, 0.3)
        });
        b.paint(METAL);
        b.prism(v3(0.0, 0.0, 18.4), 8, 1.4, 0.9, 0.6);
    });
    // The status lamps: on the manifold at tech 1, the loading tower's head at tech 2,
    // and the corner pylons' heads (above) at tech 3.
    let lamp = match tech {
        1 => Some((v3(2.9, 0.0, 6.9), 0.5)),
        2 => Some((v3(1.95, 0.0, 12.6), 0.38)),
        _ => None,
    };
    if let Some((at, r)) = lamp {
        b.radial(4, |b| status_lamp(b, at, r));
    }
}

/// Sides of a vat and its stage.
const VAT_SIDES: usize = 8;

/// The fill level at which the vault's gauge ring `j` lights (0 to 2 up each vat, 3 the
/// vat's second stage, 4 and 5 round the strongroom), at `tech`: the rings a tier has
/// share the store between them, bottom to top.
fn level(j: usize, tech: u8) -> f32 {
    let rings = match tech {
        1 => 3.0,
        2 => 4.0,
        _ => 6.0,
    };
    (j as f32 + 0.5) / rings
}

/// A lit gauge ring round a vat, `h` tall from `z`.
fn gauge_ring(b: &mut MeshBuilder, z: f32, r: f32, h: f32) {
    b.paint(GLOW);
    band(b, z, r, h);
}

/// A band round a vat in the current paint, `h` tall from `z`: open top and bottom,
/// since the vat fills it.
fn band(b: &mut MeshBuilder, z: f32, r: f32, h: f32) {
    let ring = |z: f32| -> Vec<Vec3> {
        ngon(b.sides(VAT_SIDES), r)
            .into_iter()
            .map(|p| v3(p[0], p[1], z))
            .collect()
    };
    let (lo, hi) = (ring(z), ring(z + h));
    b.loft(&[lo, hi], false, false);
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
    b.cuboid_open(
        v3(0.0, 0.0, VAULT_DECK * 0.5),
        v3(VAULT_SLAB * 2.0, VAULT_SLAB * 2.0, VAULT_DECK),
    );
    b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
        b.radial(4, |b| {
            let top = if tech >= 2 { 11.4 } else { 7.6 };
            b.frustum_open(
                v3(VAULT_VAT_OUT, 0.0, VAULT_DECK),
                v2(8.4, 8.4),
                v2(6.0, 6.0),
                top - VAULT_DECK,
                Vec2::ZERO,
            );
        });
    });
    b.paint(ACCENT);
    let top = match tech {
        1 => 6.9,
        2 => 12.6,
        _ => 18.4,
    };
    b.frustum_open(
        v3(0.0, 0.0, VAULT_DECK),
        v2(7.6, 7.6),
        v2(6.0, 6.0),
        top - VAULT_DECK,
        Vec2::ZERO,
    );
    team_panel(b, v3(0.0, 0.0, top), v2(3.0, 3.0));
    if tech >= 3 {
        b.paint(PLATING);
        b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
            b.radial(4, |b| b.cuboid_open(v3(19.4, 0.0, 7.5), v3(2.0, 2.0, 15.0)))
        });
    }
}
