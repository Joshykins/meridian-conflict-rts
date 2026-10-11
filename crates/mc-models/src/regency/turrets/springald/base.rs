//! What the Springald's turret turns on, up to the deck (`DECK`), the owner's colour round
//! the race: three alternatives to pick from.
//!
//! - [`plinth`]: a stepped octagonal plinth, ridged skirt plates on its facets, ringed by
//!   eight plated buttresses with lit slits.
//! - [`outriggers`]: a round plated drum and four outriggers splayed to the lot's corners,
//!   each a plated arm on a joint drum, feathered plates down its back, its spade dug into
//!   the ground.
//! - [`bastion`]: a dodecagonal drum in two tiers, feathered plates lapped down its skirt
//!   like scales, machinery bays let into its upper tier.

use std::f32::consts::TAU;

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::TEAM;
use crate::pattern;

use super::super::super::kit::{blade, dark_plate, metal, seam, v3};
use super::super::super::machine::{armour, hoop, swept, Course, Frame};
use super::super::super::parts::{bay, drum, inlay, ridged};
use super::super::{buttress, slit, step};
use super::{DECK, RACE};

/// The owner's colour round the race, on the deck.
fn race_ring(b: &mut MeshBuilder) {
    let segs = b.sides(32);
    b.paint(TEAM);
    hoop(b, Vec3::Z * (DECK + 0.02), RACE + 1.4, 2.0, 0.05, segs);
}

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

/// A: the stepped octagonal plinth.
pub(super) fn plinth(b: &mut MeshBuilder) {
    let fine = b.fine();
    step(b, 8, 23.6, 22.4, 0.0, 3.0);
    seam(b);
    b.prism(Vec3::Z * 3.0, 8, 21.0, 21.0, 0.5);
    step(b, 8, 19.6, 17.6, 3.5, DECK - 3.5);
    for k in 0..8 {
        // The flats face 22.5 degrees off each corner.
        b.yawed(Vec3::ZERO, (45.0 * k as f32).to_radians(), |b| {
            if b.mid() {
                // Skirt plates on the lower step's facet, and slits in the upper's.
                let r = 23.6 * (TAU / 16.0).cos();
                facet_plate(
                    b,
                    v3(r - 0.6, 0.0, 2.7),
                    v3(0.4, 0.0, -1.0),
                    v3(1.0, 0.0, 0.4),
                    2.6,
                    5.2,
                );
                if fine {
                    let r = 18.6 * (TAU / 16.0).cos();
                    inlay(
                        b,
                        &[
                            v3(r + 0.12, -4.0, 5.6),
                            v3(r + 0.12, 4.0, 5.6),
                            v3(r - 0.02, 4.0, 6.0),
                            v3(r - 0.02, -4.0, 6.0),
                        ],
                        v3(1.0, 0.0, 0.3),
                        pattern::EMBER_SLOT,
                    );
                }
            }
        });
        b.yawed(Vec3::ZERO, (22.5 + 45.0 * k as f32).to_radians(), |b| {
            buttress(b, 16.8, 3.4, 10.5, 23.0, 2.6, 3.0);
            if !b.mid() {
                return;
            }
            dark_plate(b);
            ridged(
                b,
                &Frame::new(v3(17.4, 0.0, 10.4), v3(1.0, 0.0, -1.2), v3(1.2, 0.0, 1.0)),
                &[[0.0, -1.5], [7.4, -1.1], [8.4, 0.0], [7.4, 1.1], [0.0, 1.5]],
                0.35,
                [3.0, 0.0],
                0.3,
            );
            slit(b, v3(22.4, 0.0, 2.2), v3(1.0, 0.0, 0.2), Vec3::Z, 2.4, 0.5);
            if fine {
                metal(b);
                b.cylinder_between(v3(19.0, -1.75, 5.0), v3(19.0, 1.75, 5.0), 0.5, 0.5, 8);
            }
        });
    }
    race_ring(b);
}

/// B: a round plated drum, four outriggers splayed to the lot's corners.
pub(super) fn outriggers(b: &mut MeshBuilder) {
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
    race_ring(b);
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

/// C: a dodecagonal drum in two tiers, its skirt feathered in plates like scales.
pub(super) fn bastion(b: &mut MeshBuilder) {
    step(b, 12, 23.4, 21.6, 0.0, 3.6);
    seam(b);
    b.prism(Vec3::Z * 3.6, 12, 20.6, 20.6, 0.5);
    step(b, 12, 20.0, 18.4, 4.1, DECK - 4.1);
    for k in 0..12 {
        b.yawed(Vec3::ZERO, (30.0 * k as f32 + 15.0).to_radians(), |b| {
            let flat = 23.4 * (TAU / 24.0).cos();
            if b.mid() {
                // Two plates lapped down each facet of the skirt, staggered.
                dark_plate(b);
                Course {
                    count: 2,
                    step: 1.2,
                    len: 1.8,
                    half: 2.4,
                    tip: if k % 2 == 0 { 0.5 } else { -0.5 },
                    thick: 0.3,
                    tail: 0.5,
                }
                .lay(
                    b,
                    &Frame::new(
                        v3(flat - 1.6, 0.0, 3.9),
                        v3(0.45, 0.0, -1.0),
                        v3(1.0, 0.0, 0.45),
                    ),
                );
            }
            // Every other upper facet a bay over the machinery, the rest a lit slit.
            if b.fine() {
                let r = 19.2 * (TAU / 24.0).cos();
                if k % 2 == 0 {
                    bay(
                        b,
                        &Frame::new(v3(r + 0.1, -2.4, 6.1), Vec3::Y, v3(1.0, 0.0, 0.2)),
                        4.8,
                        0.55,
                    );
                } else {
                    slit(
                        b,
                        v3(r + 0.15, 0.0, 6.1),
                        v3(1.0, 0.0, 0.2),
                        Vec3::Y,
                        3.6,
                        0.5,
                    );
                }
            }
        });
    }
    if b.mid() {
        // Swept plates lapped over the deck's rim, pointing round it.
        for k in 0..6 {
            b.yawed(Vec3::ZERO, (60.0 * k as f32).to_radians(), |b| {
                dark_plate(b);
                armour(
                    b,
                    &Frame::new(v3(19.0, -2.6, DECK + 0.02), Vec3::Y, Vec3::Z),
                    &swept(5.6, 0.7, 0.0, 0.6),
                    0.3,
                );
            });
        }
    }
    race_ring(b);
}
