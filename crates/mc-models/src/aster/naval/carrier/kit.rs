//! Pieces the carrier is fitted with: the SAM cells, the fleet radar, round pads and
//! domed hatches.

use super::super::*;
use crate::builder::CellGrid;
use glam::Vec2;

/// Weapon 0's cells: pitch, a hatch's half width, the order each block fires in (`i`
/// along x, `j` along y), and half a block's plan with the coaming round it.
const CELL_PITCH: f32 = 2.8;
const CELL_HALF: f32 = 1.15;
const CELL_FIRE: [(u8, u8); 6] = [(0, 0), (2, 1), (1, 0), (0, 1), (2, 0), (1, 1)];
const BLOCK_HALF: Vec2 = Vec2::new(3.0 * CELL_PITCH * 0.5 + 0.7, CELL_PITCH + 0.7);

/// A round pad on a deck at `at`: a low disc with a raised rim, a dark ring inside it
/// and a boss in the middle.
pub(super) fn pad(b: &mut MeshBuilder, at: Vec3, radius: f32) {
    let n = b.sides(12);
    b.paint(PLATING).pattern(pattern::PLAIN);
    b.prism(at, n, radius, radius * 0.96, 0.3);
    if !b.mid() {
        return;
    }
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    b.prism(at + Vec3::Z * 0.3, n, radius * 0.8, radius * 0.8, 0.04);
    b.paint(PLATING).pattern(pattern::PLAIN);
    b.prism(at + Vec3::Z * 0.3, n, radius * 0.4, radius * 0.34, 0.16);
}

/// A block of weapon 0's cells let flush into a deck at `deck`: a dark coaming round
/// six hatches, each hinged on its outer edge along y over the missile in its cell.
pub(super) fn cell_battery(b: &mut MeshBuilder, c: Vec2, deck: f32) {
    if b.coarse() {
        // Far off the coaming alone; the cells are still declared for the shader.
        let grid = CellGrid {
            centre: c,
            deck,
            pitch: CELL_PITCH,
            half: CELL_HALF,
            nx: 3,
            ny: 2,
            hinge_y: true,
        };
        b.cell_block(grid, &CELL_FIRE);
        return;
    }
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.plate(c.extend(deck - 0.02), BLOCK_HALF * 2.0, 0.06, 0.03);
    let grid = CellGrid {
        centre: c,
        deck,
        pitch: CELL_PITCH,
        half: CELL_HALF,
        nx: 3,
        ny: 2,
        hinge_y: true,
    };
    for m in b.cell_block(grid, &CELL_FIRE) {
        let side = (m.y - c.y).signum();
        let hinge = m.y + side * CELL_HALF;
        b.with_part(part::CELL_HATCH, |b| {
            b.paint(PLATING).pattern(pattern::PLAIN);
            b.cuboid(
                v3(m.x, m.y, deck + 0.12),
                v3(CELL_HALF * 2.0, CELL_HALF * 2.0, 0.24),
            );
            if b.fine() {
                b.paint(METAL);
                for dx in [-0.65, 0.65] {
                    b.cylinder_between(
                        v3(m.x + dx - 0.22, hinge, deck + 0.08),
                        v3(m.x + dx + 0.22, hinge, deck + 0.08),
                        0.15,
                        0.15,
                        4,
                    );
                }
            }
        });
        if !b.fine() {
            // Further off the shut hatches hide the rounds.
            continue;
        }
        b.with_part(part::CELL_ROUND, |b| {
            let top = deck - 0.25;
            let foot = top - 5.6;
            b.paint(PLATING);
            b.cylinder_between(
                v3(m.x, m.y, foot),
                v3(m.x, m.y, top - 1.0),
                0.45,
                0.45,
                b.sides(6),
            );
            b.paint(PLATING_DARK);
            b.cylinder_between(
                v3(m.x, m.y, top - 1.0),
                v3(m.x, m.y, top),
                0.45,
                0.06,
                b.sides(6),
            );
        });
    }
}
/// The fleet radar: a faceted mast from `foot` and the long double-faced array turning
/// on its head at `pivot_z`.
pub(super) fn fleet_radar(b: &mut MeshBuilder, foot: Vec3, pivot_z: f32) {
    if b.coarse() {
        return;
    }
    b.paint(PLATING).pattern(pattern::GENERIC);
    b.loft_z(
        &chamfered_rect(v2(2.4, 1.9), 0.5),
        &[
            Section::new(foot.z, 1.0).shifted(foot.x, foot.y),
            Section::scaled(pivot_z - 1.2, 0.5, 0.55).shifted(foot.x, foot.y),
            Section::scaled(pivot_z, 0.3, 0.35).shifted(foot.x, foot.y),
        ],
    );
    let pivot = v3(foot.x, foot.y, pivot_z);
    b.set_spinner_pivot(pivot);
    b.with_part(part::SPINNER, |b| {
        b.paint(ACCENT);
        b.prism(pivot, b.sides(8), 0.8, 0.7, 0.6);
        b.paint(PLATING);
        b.beam(
            pivot + v3(0.0, -5.5, 1.7),
            pivot + v3(0.0, 5.5, 1.7),
            v2(0.7, 2.4),
            v2(0.7, 2.4),
        );
        b.paint(PLATING_DARK);
        for s in [-1.0, 1.0] {
            b.beam(
                pivot + v3(s * 0.38, -5.1, 1.7),
                pivot + v3(s * 0.38, 5.1, 1.7),
                v2(0.06, 2.0),
                v2(0.06, 2.0),
            );
        }
    });
}

/// A domed hatch or, from 1.2 m across, a sensor blister on a deck at `at`: a dark ring
/// and a low faceted dome.
pub(super) fn dome(b: &mut MeshBuilder, at: Vec3, radius: f32) {
    // A big sensor blister stays in view further off than a hatch.
    let blister = radius >= 1.2;
    if !b.fine() && !(blister && b.mid()) {
        return;
    }
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.prism(at, 8, radius, radius, 0.12);
    b.paint(if blister { PLATING } else { PLATING_DARK })
        .pattern(pattern::PLAIN);
    b.loft_z(
        &ngon(8, radius * 0.8),
        &[
            Section::new(at.z + 0.12, 1.0).shifted(at.x, at.y),
            Section::new(at.z + 0.42, 0.8).shifted(at.x, at.y),
            Section::new(at.z + 0.6, 0.4).shifted(at.x, at.y),
        ],
    );
}

/// A raised housing on a deck at `at`: a chamfered block drawn in as it rises, a dark
/// slot across its face.
pub(super) fn housing(b: &mut MeshBuilder, at: Vec3, plan: Vec2, height: f32) {
    b.paint(PLATING).pattern(pattern::GENERIC);
    b.loft_z(
        &chamfered_rect(plan * 0.5, plan.min_element() * 0.2),
        &[
            Section::new(at.z, 1.0).shifted(at.x, at.y),
            Section::scaled(at.z + height, 0.86, 0.8).shifted(at.x, at.y),
        ],
    );
    if b.fine() {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.cuboid(
            v3(at.x + plan.x * 0.47, at.y, at.z + height * 0.55),
            v3(0.12, plan.y * 0.5, height * 0.2),
        );
    }
}

/// A bolted access cover on a deck at `at`: a dark rebate under a bevelled lid with a
/// recessed grip.
pub(super) fn cover(b: &mut MeshBuilder, at: Vec3, plan: Vec2) {
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.plate(at, plan + Vec2::splat(0.3), 0.05, 0.01);
    b.paint(PLATING).pattern(pattern::PLAIN);
    b.plate(at + Vec3::Z * 0.05, plan, 0.12, 0.05);
    if b.fine() {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.cuboid(
            at + v3(plan.x * 0.3, 0.0, 0.18),
            v3(0.16, plan.y * 0.4, 0.04),
        );
    }
}

/// A recessed service bay at `at`: a dark rim round a sunken panel cut with circuit runs.
pub(super) fn bay(b: &mut MeshBuilder, at: Vec3, plan: Vec2) {
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.plate(at, plan, 0.04, 0.01);
    if !b.fine() {
        return;
    }
    b.paint(PLATING_DARK).pattern(pattern::CIRCUIT);
    b.plate(at + Vec3::Z * 0.04, plan - Vec2::splat(0.5), 0.02, 0.005);
}
