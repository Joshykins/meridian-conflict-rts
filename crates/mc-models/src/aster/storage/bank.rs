//! The Capacitor Bank, authored on a 3×3 lot (lot half-extent 18). Tech 1 is six
//! capacitor cells in two rows between armoured bookends, a bus bar down the middle,
//! open toward ±x. The higher tiers are built onto it in place, adding machinery;
//! the tech 1 bank never moves.
//! Tech 2 stands an armoured regulator tower at each open end, fed from the bus bar;
//! tech 3 adds a sloped battery rack down each flank and a box girder across the
//! tower heads.

use glam::Vec3;

use super::super::parts::*;
use super::super::structures::kit;
use super::{fill, status_lamp};
use crate::builder::{chamfered_rect, MeshBuilder, Section};
use crate::material::*;
use crate::pattern;

/// Authored (radius, height) per tier.
pub(in crate::aster) const SIZES: [(f32, f32); 3] = [(14.0, 10.0), (14.0, 14.0), (14.0, 19.0)];

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
                // The charge rises through the bands: the lower ones cell by cell
                // from -x, then the upper ones.
                let k = (col + 1) as f32;
                b.paint(GLOW);
                fill(b, (k + 0.5) / 6.0, |b| {
                    b.prism(v3(0.0, 0.0, 3.0), b.sides(8), 3.05, 3.05, 0.7)
                });
                fill(b, (k + 3.5) / 6.0, |b| {
                    b.prism(v3(0.0, 0.0, 5.6), b.sides(8), 3.05, 3.05, 0.7)
                });
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

/// A power bus from `a` to `bb`: a dark bar whose top carries the stored charge
/// (`pattern::FLUX`).
fn bus(b: &mut MeshBuilder, a: Vec3, bb: Vec3, size: glam::Vec2) {
    b.paint(ACCENT).pattern(pattern::FLUX);
    b.beam(a, bb, size, size);
}

/// Grows out along the lot. Tech 2 stands an armoured regulator tower at each open
/// end, fed from the bus bar, lit up its face and lamped on top; tech 3 adds a
/// sloped battery rack down each flank and a box girder across the tower heads that
/// carries the charge over the bank.
pub(in crate::aster) fn storage_energy(b: &mut MeshBuilder, tech: u8) {
    bank(b);
    lamps(b, tech);
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
                // The regulator head, and its gauge up the face.
                b.paint(PLATING_DARK);
                b.chamfered_box(v3(0.0, 0.0, 12.8), v3(3.4, 6.0, 1.2), 0.6);
                team_panel(b, v3(0.0, 0.0, 13.4), v2(2.4, 3.0));
                if b.fine() {
                    for (i, z) in [4.0, 6.0, 8.0].into_iter().enumerate() {
                        fill(b, (i as f32 + 0.5) / 3.0, |b| {
                            slit(b, v3(x * 2.52, 0.0, z), Vec3::X, 5.0, 0.3)
                        });
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
                    // Lit slits up the rack's slope, charging as the cells do.
                    let k = (i + 1) as f32;
                    for (y, z, at) in [(15.4, 4.0, k + 0.5), (14.4, 5.6, k + 3.5)] {
                        fill(b, at / 6.0, |b| {
                            slit(b, v3(i as f32 * 6.4, y, z), Vec3::Y, 4.4, 0.3)
                        });
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
                    fill(b, 0.95, |b| {
                        slit(b, v3(x * 1.52, 0.0, 16.0), Vec3::X, 3.2, 0.3)
                    });
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
            // A charge bar along the girder, filling from -x.
            b.mirror_y(|b| {
                for i in -3..=3 {
                    fill(b, ((i + 3) as f32 + 0.5) / 7.0, |b| {
                        slit(b, v3(i as f32 * 3.4, 1.82, 16.9), Vec3::Y, 1.6, 0.3)
                    });
                }
            });
        }
    });
}

/// The status lamps, on the tier's highest work: the bookends at tech 1, the
/// regulator heads at tech 2, the girder at tech 3. Far off, one at each end.
fn lamps(b: &mut MeshBuilder, tech: u8) {
    if b.coarse() {
        let at = match tech {
            1 => v3(7.0, 8.8, BOOK_TOP),
            2 => v3(TOWER_X, 0.0, 13.6),
            _ => v3(TOWER_X, 0.0, 18.6),
        };
        for x in [-1.0f32, 1.0] {
            status_lamp(b, v3(at.x * x, at.y, at.z), 0.9);
        }
        return;
    }
    let (at, r) = match tech {
        1 => (v3(9.8, 9.1, BOOK_TOP), 0.6),
        2 => (v3(TOWER_X, 2.3, 13.4), 0.55),
        _ => (v3(TOWER_X + 0.4, 1.0, 17.8), 0.55),
    };
    for x in [-1.0f32, 1.0] {
        b.mirror_y(|b| status_lamp(b, v3(at.x * x, at.y, at.z), r));
    }
}

/// Where the regulator towers stand, at each open end.
const TOWER_X: f32 = 14.6;

#[cfg(test)]
mod tests {
    use crate::build_model_scaled;

    /// Each tier adds machinery to the one below and keeps to the structure budgets.
    #[test]
    fn tiers_add_machinery_within_budget() {
        for key in ["storage_energy"] {
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
