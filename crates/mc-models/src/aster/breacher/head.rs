//! The Breacher's head, sunk low under the brow: not the Behemoth's hood and orange visor
//! band, but a low pointed head with one thin sensor line.

use glam::Vec3;

use super::super::parts::*;
use crate::builder::{MeshBuilder, Section};
use crate::material::*;
use crate::pattern;

/// Where the head turns: low, ahead of the chest, under the brow.
const NECK: Vec3 = Vec3::new(11.0, 0.0, 33.5);

/// The neck's collar, fixed to the chest.
fn collar(b: &mut MeshBuilder) {
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.prism(
        v3(NECK.x - 1.5, 0.0, NECK.z - 3.0),
        b.sides(10),
        3.2,
        2.8,
        2.0,
    );
}

/// A low head drawn out to a blunt prow: a dark lower half, a light plated crown with fins
/// swept back from it, one thin sensor line between them wrapped round the prow. It looks
/// about while the unit idles (`with_head`).
pub(super) fn head(b: &mut MeshBuilder) {
    collar(b);
    let z = NECK.z;
    b.with_head(NECK, |b| {
        let fine = b.fine();
        let plan = [
            [-3.4, -3.6],
            [1.6, -4.0],
            [4.8, -1.6],
            [5.4, 0.0],
            [4.8, 1.6],
            [1.6, 4.0],
            [-3.4, 3.6],
        ];
        b.at(v3(NECK.x + 0.6, 0.0, 0.0), |b| {
            b.paint(PLATING_DARK);
            b.loft_z(
                &plan,
                &[
                    Section::scaled(z - 1.8, 0.9, 0.86),
                    Section::new(z - 0.2, 1.0),
                ],
            );
            b.paint(PLATING);
            b.loft_z(
                &plan,
                &[
                    Section::new(z + 0.2, 1.0),
                    Section::scaled(z + 1.4, 0.96, 0.94).shifted(-0.4, 0.0),
                    Section::scaled(z + 2.2, 0.7, 0.6).shifted(-1.2, 0.0),
                ],
            );
            // The sensor line between them, wrapped round the prow.
            b.paint(GLOW).pattern(pattern::PLAIN);
            b.loft_z(
                &plan,
                &[
                    Section::scaled(z - 0.2, 1.01, 1.01),
                    Section::scaled(z + 0.2, 1.01, 1.01),
                ],
            );
            if fine {
                // Fins swept back from the crown.
                b.paint(PLATING);
                b.mirror_y(|b| {
                    b.extrude_y(
                        &[
                            [-1.0, z + 1.4],
                            [2.6, z + 1.6],
                            [-3.8, z + 3.2],
                            [-4.4, z + 2.6],
                        ],
                        2.4,
                        2.9,
                    );
                });
            }
        });
    });
}
