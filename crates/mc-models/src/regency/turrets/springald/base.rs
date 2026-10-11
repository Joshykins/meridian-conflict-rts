//! What the Springald's turret turns on, up to the deck (`DECK`): a round plated drum and
//! four outriggers splayed to the lot's corners, each a plated arm stepping down from a
//! joint drum to a spade dug into the ground, feathered plates down its back.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::pattern;

use super::super::super::kit::{blade, dark_plate, seam, v3};
use super::super::super::machine::{Course, Frame};
use super::super::super::parts::{drum, inlay, ridged};
use super::super::buttress;
use super::DECK;

/// A ridged plate laid on a sloped facet at `c`, `out` its face, `down` the slope.
fn facet_plate(b: &mut MeshBuilder, c: Vec3, down: Vec3, out: Vec3, len: f32, half: f32) {
    dark_plate(b);
    ridged(
        b,
        &Frame::new(c, down, out),
        &[
            [0.0, -half],
            [len, -half * 1.05],
            [len, half * 1.05],
            [0.0, half],
        ],
        0.3,
        [len * 0.4, 0.0],
        0.25,
    );
}

/// A round plated drum, four outriggers splayed to the lot's corners.
pub(super) fn draw(b: &mut MeshBuilder) {
    let fine = b.fine();
    let sides = b.sides(16);
    dark_plate(b);
    b.prism(Vec3::ZERO, sides, 18.0, 17.2, 2.0);
    seam(b);
    b.prism(Vec3::Z * 2.0, sides, 16.8, 16.8, 0.6);
    dark_plate(b);
    b.prism(Vec3::Z * 2.6, sides, 16.6, 15.8, DECK - 2.6);
    if b.mid() {
        // Ridged plates round the drum's foot between the outriggers.
        for k in 0..8 {
            b.yawed(Vec3::ZERO, (22.5 + 45.0 * k as f32).to_radians(), |b| {
                facet_plate(
                    b,
                    v3(17.9, 0.0, 1.9),
                    v3(0.3, 0.0, -1.0),
                    v3(1.0, 0.0, 0.3),
                    1.8,
                    3.0,
                );
                if fine {
                    inlay(
                        b,
                        &[
                            v3(16.5, -2.6, 5.0),
                            v3(16.5, 2.6, 5.0),
                            v3(16.42, 2.6, 5.45),
                            v3(16.42, -2.6, 5.45),
                        ],
                        v3(1.0, 0.0, 0.1),
                        pattern::EMBER_SLOT,
                    );
                }
            });
        }
    }
    for k in 0..4 {
        b.yawed(Vec3::ZERO, (45.0 + 90.0 * k as f32).to_radians(), |b| {
            outrigger(b)
        });
    }
}

/// One outrigger along +x from the drum: a plated arm stepping down from a joint drum at
/// the drum's shoulder to a spade dug into the ground, feathered plates down its back and
/// a slot in each flank.
fn outrigger(b: &mut MeshBuilder) {
    let fine = b.fine();
    let reach = 30.4;
    buttress(b, 13.0, 4.6, 7.4, reach - 2.0, 3.0, 2.4);
    seam(b);
    b.beam(
        v3(reach - 3.0, 0.0, 0.5),
        v3(reach, 0.0, 0.3),
        Vec2::new(4.4, 1.0),
        Vec2::new(3.6, 0.6),
    );
    if !b.mid() {
        return;
    }
    drum(b, v3(15.4, 0.0, 6.0), Vec3::Y, 1.8, 5.4);
    // The spade: three blades driven into the ground at its end.
    dark_plate(b);
    for y in [-1.3f32, 0.0, 1.3] {
        blade(
            b,
            v3(reach - 1.6, y, 1.4),
            v3(reach + 0.6, y * 1.2, 0.0),
            0.7,
            Vec3::Y,
        );
    }
    dark_plate(b);
    Course {
        count: if fine { 4 } else { 2 },
        step: 3.1,
        len: 3.8,
        half: 1.3,
        tip: 0.0,
        thick: 0.3,
        tail: 1.2,
    }
    .lay(
        b,
        &Frame::new(v3(16.8, 0.0, 7.2), v3(1.0, 0.0, -0.36), v3(0.36, 0.0, 1.0)),
    );
    if fine {
        b.mirror_y(|b| {
            inlay(
                b,
                &[
                    v3(17.0, 2.08, 3.6),
                    v3(25.0, 1.72, 2.0),
                    v3(25.0, 1.72, 2.4),
                    v3(17.0, 2.08, 4.1),
                ],
                Vec3::Y,
                pattern::EMBER_SLOT,
            );
        });
    }
}
