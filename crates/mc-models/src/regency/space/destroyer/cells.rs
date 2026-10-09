//! The Gravitic Seeker cells: two blocks of eight hatched cells either side of the
//! spine, port block first, one weapon over all sixteen (`space.ron` weapon 7, its
//! `muzzles` in `FIRE` order, port then starboard). Each block is a faceted armoured
//! box stood on the hull, a red-lit coaming round its deck; each lid is hinged on its
//! outboard edge and swings open over the seeker waiting under it.

use glam::Vec2;
#[cfg(test)]
use glam::Vec3;

use crate::builder::{chamfered_rect, CellGrid, MeshBuilder, Section};
use crate::material::GLOW_LASER;
use crate::part;

use super::super::super::kit::{dark_plate, metal, seam, v3};
use super::super::body::Body;
use super::super::dress::pale_plate;

/// The cells' spacing and each lid's half-width.
const PITCH: f32 = 7.0;
const HALF: f32 = 2.9;
/// The order the cells fire in, (along, across): spread over the block.
const FIRE: [(u8, u8); 8] = [
    (0, 0),
    (2, 1),
    (1, 0),
    (3, 1),
    (0, 1),
    (2, 0),
    (1, 1),
    (3, 0),
];

/// The two blocks: their middle along the hull and out from the spine.
#[derive(Clone, Copy)]
pub(super) struct Cells {
    pub(super) x: f32,
    pub(super) y: f32,
}

impl Cells {
    fn half(&self) -> Vec2 {
        Vec2::new(PITCH * 2.0 + 2.2, PITCH + 2.2)
    }

    /// The block's middle and its deck's height, port side `1.0`, starboard `-1.0`. The
    /// deck clears the hull at the block's inner edge, where it stands highest.
    fn deck(&self, body: &Body) -> f32 {
        let h = self.half();
        [self.x - h.x, self.x, self.x + h.x]
            .iter()
            .map(|&x| body.point(x, body.u_up(x, self.y - h.y)).z)
            .fold(f32::MIN, f32::max)
            + 3.0
    }

    /// Where the seekers leave from, port block then starboard, each in firing order.
    #[cfg(test)]
    pub(super) fn muzzles(&self, body: &Body) -> Vec<Vec3> {
        let deck = self.deck(body);
        [1.0f32, -1.0]
            .iter()
            .flat_map(|&side| {
                let c = Vec2::new(self.x, self.y * side);
                FIRE.iter().map(move |&(i, j)| {
                    let at = |k: u8, n: f32| (k as f32 - (n - 1.0) * 0.5) * PITCH;
                    (c + Vec2::new(at(i, 4.0), at(j, 2.0))).extend(deck - 3.0)
                })
            })
            .collect()
    }
}

pub(super) fn blocks(b: &mut MeshBuilder, body: &Body, cells: Cells) {
    let deck = cells.deck(body);
    let h = cells.half();
    // The box runs down into the hull at its outer edge.
    let foot = [cells.x - h.x, cells.x, cells.x + h.x]
        .iter()
        .map(|&x| body.point(x, body.u_up(x, cells.y + h.y)).z)
        .fold(f32::MAX, f32::min)
        - 3.0;
    for side in [1.0f32, -1.0] {
        let c = Vec2::new(cells.x, cells.y * side);
        let plan = chamfered_rect(h, 3.0);
        b.at(c.extend(0.0), |b| {
            dark_plate(b);
            b.with_facets(|b| {
                b.loft_z(
                    &plan,
                    &[
                        Section::new(foot, 1.12),
                        Section::new(deck - 3.0, 1.06),
                        Section::new(deck - 0.8, 1.0),
                    ],
                )
            });
            seam(b);
            b.loft_z(
                &plan,
                &[Section::new(deck - 0.8, 1.0), Section::new(deck, 0.97)],
            );
            if !b.coarse() {
                b.paint(GLOW_LASER);
                b.loft_z(
                    &plan,
                    &[
                        Section::new(deck - 2.2, 1.065),
                        Section::new(deck - 1.7, 1.065),
                    ],
                );
            }
        });
        if b.coarse() {
            continue;
        }
        let grid = CellGrid {
            centre: c,
            deck,
            pitch: PITCH,
            half: HALF,
            nx: 4,
            ny: 2,
            hinge_y: true,
        };
        for m in b.cell_block(grid, &FIRE) {
            let out = (m.y - c.y).signum();
            let hinge = m.y + out * HALF;
            b.with_part(part::CELL_HATCH, |b| {
                pale_plate(b);
                b.with_facets(|b| {
                    b.loft_z(
                        &chamfered_rect(Vec2::splat(HALF), 0.9),
                        &[
                            Section::new(deck, 1.0).shifted(m.x, m.y),
                            Section::new(deck + 0.6, 0.86).shifted(m.x, m.y),
                        ],
                    )
                });
                if b.fine() {
                    metal(b);
                    for dx in [-1.6, 1.6] {
                        b.cylinder_between(
                            v3(m.x + dx - 0.6, hinge, deck + 0.2),
                            v3(m.x + dx + 0.6, hinge, deck + 0.2),
                            0.35,
                            0.35,
                            4,
                        );
                    }
                }
            });
            b.with_part(part::CELL_ROUND, |b| {
                let top = deck - 0.5;
                let foot = if b.fine() { top - 12.0 } else { top - 4.0 };
                metal(b);
                b.cylinder_between(
                    v3(m.x, m.y, foot),
                    v3(m.x, m.y, top - 2.0),
                    1.1,
                    1.1,
                    b.sides(6),
                );
                b.paint(GLOW_LASER);
                b.cylinder_between(
                    v3(m.x, m.y, top - 2.0),
                    v3(m.x, m.y, top),
                    1.1,
                    0.25,
                    b.sides(6),
                );
            });
        }
    }
}
