//! The Capacitor Bank, authored on a 3×3 lot (lot half-extent 18). Tech 1 is six
//! capacitor cells in two rows between armoured bookends, a bus bar down the middle,
//! open toward ±x. The higher tiers are built onto it in place, adding machinery;
//! the tech 1 bank never moves.
//!
//! Design round: `storage_energy~a`, `~b` and `~c` are three alternatives for the
//! tech 2 and tech 3 additions, drawn at the authored heights of [`SIZES`].

use glam::Vec3;

use super::parts::*;
use super::structures::kit;
use crate::builder::{chamfered_rect, MeshBuilder, Section};
use crate::material::*;
use crate::pattern;

/// Authored (radius, height) per tier of the tiered designs.
pub(super) const SIZES: [(f32, f32); 3] = [(14.0, 10.0), (14.0, 14.0), (14.0, 19.0)];

/// The bank as it stands today, every tier the tech 1 bank.
pub(super) fn storage_energy(b: &mut MeshBuilder, _tech: u8) {
    bank(b);
}

/// Where the cells stand: three along x, one row each side of the bus bar.
const CELL_X: f32 = 6.4;
const CELL_Y: f32 = 3.4;
const CELL_R: f32 = 2.9;
/// The bookends' inner and outer faces, and their top.
const BOOK_IN: f32 = 7.4;
const BOOK_OUT: f32 = 11.8;
const BOOK_TOP: f32 = 9.6;

/// The tech 1 bank: two rows of three cells between armoured bookends.
fn bank(b: &mut MeshBuilder) {
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
    for col in -1..=1 {
        b.mirror_y(|b| {
            b.at(v3(col as f32 * CELL_X, CELL_Y, 0.0), |b| {
                b.paint(ACCENT);
                b.prism(v3(0.0, 0.0, 0.0), b.sides(8), CELL_R, CELL_R, 8.0);
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
                [BOOK_IN, 0.0],
                [BOOK_OUT, 0.0],
                [BOOK_OUT, 4.0],
                [10.0, BOOK_TOP],
                [8.2, BOOK_TOP],
                [BOOK_IN, 8.0],
            ],
            -11.0,
            11.0,
        );
        if b.fine() {
            for i in 0..3 {
                vent(
                    b,
                    v3(-6.4 + 6.4 * i as f32, 9.1, BOOK_TOP),
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

/// A capacitor cell standing at `base`: a dark octagonal can, a lit band and a
/// plated cap, `h` tall to the cap's top.
fn cell(b: &mut MeshBuilder, base: Vec3, r: f32, h: f32) {
    let sides = b.sides(8);
    b.paint(ACCENT);
    b.prism(base, sides, r, r, h - 0.8);
    if b.fine() {
        b.paint(GLOW);
        b.prism(base + Vec3::Z * (h * 0.45), sides, r + 0.12, r + 0.12, 0.6);
    }
    b.paint(PLATING);
    b.prism(base + Vec3::Z * (h - 0.8), sides, r + 0.15, r * 0.7, 0.8);
}

/// A lit slit standing proud of a wall that faces along `out` (a unit axis):
/// `size` is its (along-wall, height) and it stands at `center`.
fn slit(b: &mut MeshBuilder, center: Vec3, out: Vec3, along: f32, height: f32) {
    b.paint(GLOW);
    let size = if out.x.abs() > 0.5 {
        v3(0.12, along, height)
    } else {
        v3(along, 0.12, height)
    };
    b.cuboid(center, size);
}

/// A working lamp on a short dark post, `at` its foot.
fn lamp(b: &mut MeshBuilder, at: Vec3) {
    b.paint(ACCENT);
    b.prism(at, 6, 0.35, 0.3, 0.9);
    b.paint(GLOW_LAMP);
    b.prism(at + Vec3::Z * 0.9, 6, 0.42, 0.3, 0.4);
}

/// A power bus from `a` to `bb`: a dark bar whose top carries the stored charge
/// (`pattern::FLUX`).
fn bus(b: &mut MeshBuilder, a: Vec3, bb: Vec3, size: glam::Vec2) {
    b.paint(ACCENT).pattern(pattern::FLUX);
    b.beam(a, bb, size, size);
}

// ---- Variant A: the stacked bank ------------------------------------------------
/// Grows upward, a storey a tier. Tech 2 lays girders across the bookends, stands a
/// second row of cells on them and raises armoured cheeks on the bookends; tech 3
/// closes the top in an armoured accumulator housing, lit along its flanks, with
/// buttresses bracing the cheeks and a lamp at each end.
pub(super) fn storage_energy_a(b: &mut MeshBuilder, tech: u8) {
    bank(b);
    if b.coarse() {
        if tech >= 2 {
            b.paint(PLATING);
            b.cuboid_open(v3(0.0, 0.0, 11.7), v3(19.0, 19.0, 4.2));
        }
        if tech >= 3 {
            b.paint(PLATING_DARK);
            b.frustum_open(
                v3(0.0, 0.0, 13.8),
                v2(18.0, 6.0),
                v2(14.0, 3.0),
                4.2,
                glam::Vec2::ZERO,
            );
        }
        return;
    }
    // ---- Tech 2: girders, the second storey and the cheeks.
    kit(b, tech, 2, 0.1, |b| {
        b.paint(ACCENT);
        for x in [-9.6, -3.2, 3.2, 9.6] {
            b.block(v3(x - 0.7, -10.2, BOOK_TOP), v3(x + 0.7, 10.2, 10.6));
        }
        b.mirror_y(|b| {
            b.paint(PLATING_DARK);
            b.block(v3(-10.4, 7.6, BOOK_TOP), v3(10.4, 10.4, 10.4));
        });
    });
    kit(b, tech, 2, 0.4, |b| {
        for col in -1..=1 {
            b.mirror_y(|b| cell(b, v3(col as f32 * CELL_X, CELL_Y, 10.6), 2.5, 3.2));
        }
        bus(b, v3(-10.0, 0.0, 12.9), v3(10.0, 0.0, 12.9), v2(1.4, 0.9));
        team_panel(b, v3(0.0, 0.0, 13.35), v2(6.0, 1.0));
    });
    kit(b, tech, 2, 0.7, |b| {
        b.mirror_y(|b| {
            // An armoured cheek on each bookend, raked in over the upper cells.
            b.paint(PLATING);
            b.extrude_x(
                &[
                    [7.8, 10.4],
                    [10.4, 10.4],
                    [10.4, 11.6],
                    [9.0, 14.0],
                    [7.8, 14.0],
                ],
                -10.4,
                10.4,
            );
            if b.fine() {
                for i in -1..=1 {
                    slit(b, v3(i as f32 * 6.4, 10.49, 11.0), Vec3::Y, 4.0, 0.25);
                }
                b.paint(ACCENT);
                b.block(v3(-10.6, 7.7, 13.6), v3(10.6, 9.1, 14.0));
            }
        });
    });
    // ---- Tech 3: the accumulator housing and buttresses.
    kit(b, tech, 3, 0.15, |b| {
        b.mirror_y(|b| {
            for x in [-8.0, 0.0, 8.0] {
                b.paint(PLATING_DARK);
                b.beam(
                    v3(x, 12.4, 0.0),
                    v3(x, 10.0, 12.6),
                    v2(1.6, 1.4),
                    v2(1.2, 1.0),
                );
            }
        });
    });
    kit(b, tech, 3, 0.45, |b| {
        b.paint(PLATING);
        b.loft_z(
            &chamfered_rect(v2(9.4, 6.4), 2.0),
            &[
                Section::new(14.0, 1.0),
                Section::new(15.6, 1.0),
                Section::scaled(17.4, 0.9, 0.62),
                Section::scaled(18.2, 0.8, 0.4),
            ],
        );
        b.paint(ACCENT);
        b.block(v3(-9.6, -6.6, 15.0), v3(9.6, 6.6, 15.5));
        team_panel(b, v3(0.0, 0.0, 18.2), v2(10.0, 2.0));
    });
    kit(b, tech, 3, 0.8, |b| {
        if b.fine() {
            b.mirror_y(|b| {
                for i in -2..=2 {
                    slit(b, v3(i as f32 * 3.4, 6.45, 14.5), Vec3::Y, 2.2, 0.3);
                }
            });
            for x in [-1.0f32, 1.0] {
                slit(b, v3(x * 9.45, 0.0, 14.5), Vec3::X, 7.0, 0.3);
            }
        }
        for x in [-6.4f32, 6.4] {
            lamp(b, v3(x, 0.0, 18.2));
        }
    });
}

// ---- Variant B: regulator towers and flank banks ----------------------------------
/// Grows out along the lot. Tech 2 stands an armoured regulator tower at each open
/// end, fed from the bus bar, lit up its face and lamped on top; tech 3 adds a
/// sloped battery rack down each flank and a box girder across the tower heads that
/// carries the charge over the bank.
pub(super) fn storage_energy_b(b: &mut MeshBuilder, tech: u8) {
    bank(b);
    if b.coarse() {
        if tech >= 2 {
            b.paint(PLATING);
            for x in [-1.0f32, 1.0] {
                b.frustum_open(
                    v3(x * TOWER_X, 0.0, 0.0),
                    v2(5.0, 9.0),
                    v2(4.0, 7.0),
                    if tech >= 3 { 18.6 } else { 13.6 },
                    glam::Vec2::ZERO,
                );
            }
        }
        return;
    }
    // ---- Tech 2: a regulator tower at each end.
    kit(b, tech, 2, 0.1, |b| {
        for x in [-1.0f32, 1.0] {
            b.at(v3(x * TOWER_X, 0.0, 0.0), |b| {
                b.paint(ACCENT);
                b.chamfered_box(v3(0.0, 0.0, 1.0), v3(5.4, 9.4, 2.0), 1.0);
                b.paint(PLATING);
                b.loft_z(
                    &chamfered_rect(v2(2.5, 4.5), 1.2),
                    &[
                        Section::new(2.0, 1.0),
                        Section::new(9.0, 1.0),
                        Section::new(11.6, 0.8),
                        Section::new(12.2, 0.72),
                    ],
                );
            });
        }
    });
    kit(b, tech, 2, 0.45, |b| {
        for x in [-1.0f32, 1.0] {
            b.at(v3(x * TOWER_X, 0.0, 0.0), |b| {
                // The regulator head and its lamp.
                b.paint(PLATING_DARK);
                b.chamfered_box(v3(0.0, 0.0, 12.8), v3(3.4, 6.0, 1.2), 0.6);
                team_panel(b, v3(0.0, 0.0, 13.4), v2(2.4, 3.0));
                if b.fine() {
                    lamp(b, v3(0.0, 2.6, 12.2));
                    lamp(b, v3(0.0, -2.6, 12.2));
                    for z in [4.0, 6.0, 8.0] {
                        slit(b, v3(x * 2.52, 0.0, z), Vec3::X, 5.0, 0.3);
                    }
                }
            });
            // The feed from the bus bar into the tower.
            bus(
                b,
                v3(x * 9.6, 0.0, 8.8),
                v3(x * (TOWER_X - 2.2), 0.0, 10.4),
                v2(1.4, 1.0),
            );
        }
    });
    // ---- Tech 3: flank racks and the girder over the bank.
    kit(b, tech, 3, 0.1, |b| {
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.extrude_x(
                &[
                    [BOOK_OUT, 0.0],
                    [16.4, 0.0],
                    [16.4, 2.4],
                    [13.2, 7.6],
                    [BOOK_OUT, 7.6],
                ],
                -9.8,
                9.8,
            );
            b.paint(ACCENT);
            b.block(v3(-10.2, 16.2, 0.0), v3(10.2, 16.8, 1.2));
            if b.fine() {
                for i in -1..=1 {
                    // Lit slits up the rack's slope.
                    for (y, z) in [(15.4, 4.0), (14.4, 5.6)] {
                        slit(b, v3(i as f32 * 6.4, y, z), Vec3::Y, 4.4, 0.3);
                    }
                }
            }
        });
    });
    kit(b, tech, 3, 0.45, |b| {
        for x in [-1.0f32, 1.0] {
            b.at(v3(x * TOWER_X, 0.0, 0.0), |b| {
                b.paint(PLATING);
                b.loft_z(
                    &chamfered_rect(v2(1.5, 2.6), 0.8),
                    &[Section::new(13.4, 1.0), Section::new(17.6, 1.0)],
                );
                if b.fine() {
                    slit(b, v3(x * 1.52, 0.0, 16.0), Vec3::X, 3.2, 0.3);
                }
            });
        }
    });
    kit(b, tech, 3, 0.75, |b| {
        b.paint(PLATING_DARK);
        b.block(v3(-TOWER_X - 1.6, -1.8, 16.0), v3(TOWER_X + 1.6, 1.8, 17.8));
        bus(
            b,
            v3(-TOWER_X + 1.6, 0.0, 18.1),
            v3(TOWER_X - 1.6, 0.0, 18.1),
            v2(2.0, 0.6),
        );
        team_panel(b, v3(0.0, 0.0, 18.4), v2(4.0, 1.4));
        if b.fine() {
            b.mirror_y(|b| {
                for i in -3..=3 {
                    slit(b, v3(i as f32 * 3.4, 1.82, 16.9), Vec3::Y, 1.6, 0.3);
                }
            });
            for x in [-1.0f32, 1.0] {
                lamp(b, v3(x * TOWER_X, 0.0, 17.8));
            }
        }
    });
}

/// Where variant B's regulator towers stand, at each open end.
const TOWER_X: f32 = 14.6;

// ---- Variant C: pressure drums -----------------------------------------------------
/// Grows in vessels. Tech 2 cradles a long armoured drum over each row of cells,
/// banded and lit, with a terminal head at each end dropping its cable to the lot;
/// tech 3 raises a single heavy drum on saddles above both, its bands lit, a lamp at
/// each end.
pub(super) fn storage_energy_c(b: &mut MeshBuilder, tech: u8) {
    bank(b);
    if b.coarse() {
        if tech >= 2 {
            b.paint(PLATING);
            b.cuboid_open(v3(0.0, 0.0, 11.6), v3(20.0, 11.6, 4.4));
        }
        if tech >= 3 {
            b.paint(PLATING_DARK);
            b.cuboid_open(v3(0.0, 0.0, 16.3), v3(22.0, 5.6, 5.4));
        }
        return;
    }
    let round = b.sides(10);
    // ---- Tech 2: a drum over each row.
    kit(b, tech, 2, 0.1, |b| {
        b.mirror_y(|b| {
            for x in [-6.4, 0.0, 6.4] {
                // A saddle on the cell cap.
                b.paint(ACCENT);
                b.block(
                    v3(x - 0.9, CELL_Y - 2.0, 9.0),
                    v3(x + 0.9, CELL_Y + 2.0, 10.4),
                );
            }
        });
    });
    kit(b, tech, 2, 0.35, |b| {
        b.mirror_y(|b| {
            let axis = |x: f32| v3(x, CELL_Y, DRUM2_Z);
            b.paint(PLATING);
            // Into the terminal heads at each end.
            b.cylinder_between(axis(-10.6), axis(10.6), DRUM2_R, DRUM2_R, round);
            b.paint(ACCENT);
            for x in [-6.4f32, 0.0, 6.4] {
                b.cylinder_between(
                    axis(x - 0.5),
                    axis(x + 0.5),
                    DRUM2_R + 0.18,
                    DRUM2_R + 0.18,
                    round,
                );
            }
            if b.fine() {
                b.paint(GLOW);
                for x in [-3.2f32, 3.2] {
                    b.cylinder_between(
                        axis(x - 0.25),
                        axis(x + 0.25),
                        DRUM2_R + 0.1,
                        DRUM2_R + 0.1,
                        round,
                    );
                }
            }
        });
    });
    kit(b, tech, 2, 0.7, |b| {
        // A terminal head at each end, its cable down to the lot.
        for x in [-1.0f32, 1.0] {
            b.paint(PLATING_DARK);
            b.chamfered_box(v3(x * 11.6, 0.0, DRUM2_Z), v3(2.2, 9.4, 2.4), 0.6);
            bus(
                b,
                v3(x * 11.6, 0.0, DRUM2_Z - 1.2),
                v3(x * 13.4, 0.0, 0.7),
                v2(1.6, 1.2),
            );
            b.paint(ACCENT);
            b.chamfered_box(v3(x * 13.6, 0.0, 0.7), v3(2.4, 3.0, 1.4), 0.4);
            team_panel(b, v3(x * 11.6, 0.0, DRUM2_Z + 1.2), v2(1.6, 4.0));
            if b.fine() {
                slit(b, v3(x * 12.72, 0.0, DRUM2_Z), Vec3::X, 7.0, 0.3);
            }
        }
    });
    // ---- Tech 3: the heavy drum on saddles above both.
    kit(b, tech, 3, 0.15, |b| {
        // Saddles bedded on the two drums below, cradling the heavy one.
        for x in [-6.4f32, 6.4] {
            b.paint(PLATING_DARK);
            b.extrude_x(
                &[
                    [-4.8, 12.9],
                    [4.8, 12.9],
                    [4.8, 14.0],
                    [3.6, 14.6],
                    [-3.6, 14.6],
                    [-4.8, 14.0],
                ],
                x - 1.1,
                x + 1.1,
            );
        }
    });
    kit(b, tech, 3, 0.45, |b| {
        let axis = |x: f32| v3(x, 0.0, DRUM3_Z);
        b.paint(PLATING);
        b.cylinder_between(axis(-9.4), axis(9.4), DRUM3_R, DRUM3_R, round);
        for x in [-1.0f32, 1.0] {
            b.cylinder_between(axis(x * 9.4), axis(x * 11.0), DRUM3_R, DRUM3_R * 0.6, round);
        }
        b.paint(ACCENT);
        for x in [-6.4f32, 6.4] {
            b.cylinder_between(
                axis(x - 0.7),
                axis(x + 0.7),
                DRUM3_R + 0.2,
                DRUM3_R + 0.2,
                round,
            );
        }
        team_panel(b, v3(0.0, 0.0, DRUM3_Z + DRUM3_R - 0.1), v2(6.0, 1.2));
    });
    kit(b, tech, 3, 0.8, |b| {
        if b.fine() {
            let axis = |x: f32| v3(x, 0.0, DRUM3_Z);
            b.paint(GLOW);
            for x in [-3.2f32, 0.0, 3.2] {
                b.cylinder_between(
                    axis(x - 0.3),
                    axis(x + 0.3),
                    DRUM3_R + 0.1,
                    DRUM3_R + 0.1,
                    round,
                );
            }
            for x in [-1.0f32, 1.0] {
                lamp(b, v3(x * 6.4, 0.0, DRUM3_Z + DRUM3_R));
            }
        }
    });
}

/// Variant C's drums: the two over the rows, and the heavy one above them.
const DRUM2_Z: f32 = 11.8;
const DRUM2_R: f32 = 2.2;
const DRUM3_Z: f32 = 16.0;
const DRUM3_R: f32 = 2.9;

#[cfg(test)]
mod tests {
    use crate::build_model_scaled;

    /// Each tier adds machinery to the one below and keeps to the structure budgets.
    #[test]
    fn tiers_add_machinery_within_budget() {
        for key in ["storage_energy~a", "storage_energy~b", "storage_energy~c"] {
            let mut last = 0;
            for (tech, height) in [(1, 10.2), (2, 14.3), (3, 19.4)] {
                let model = build_model_scaled(key, 14.2, height, tech).unwrap();
                let [full, mid, coarse] = [0, 1, 2].map(|l| model.lods[l].indices.len() / 3);
                println!("{key} T{tech}: {full}/{mid}/{coarse}");
                assert!(full <= 2600 && coarse < 60, "{key} T{tech}");
                assert!(
                    mid as f32 <= full as f32 * 0.45 + 20.0,
                    "{key} T{tech} reduced"
                );
                assert!(mid > last, "{key} T{tech} adds machinery");
                last = mid;
            }
        }
    }
}
