//! Undertow (`aster_t2_warp_damper`, mesh "warp_damper"): the tech 2 warp dampener,
//! authored at blueprint scale (metres, 4x4 lot, radius 18, height 46).
//!
//! A heavy anchored machine that grips space: it throws a 1600 m field that
//! snags a capital ship's jump. It keeps its field emitter at the top
//! centre, [`EMITTER`] (0.95 of the height), where the tether beam leaves, and stands
//! back from its lot's edge. Its lights (`GLOW`) and spinning gear (`part::SPINNER`)
//! go dark and still when the grid stalls, as the field does.
//!
//! [`spire`]: a tapering octagonal spire on a buttressed foot, stacked emitter collars
//! (the outer two turning) and a three-tine crown round the lens, guyed down to four
//! stakes driven into the ground (picked by the user over an anchor gyroscope and a claw
//! frame, 2026-09-29).

use std::f32::consts::{FRAC_PI_4, TAU};

use glam::{Vec2, Vec3};

use super::parts::*;
use crate::builder::{chamfered_rect, ngon, MeshBuilder, Section};
use crate::material::*;
use crate::{part, pattern};

/// The field emitter's centre, where the tether beam leaves (model space).
const EMITTER: Vec3 = Vec3::new(0.0, 0.0, 44.0);

/// A flat ring in the plane of `e1`, `e2` through `c`: `r` to its middle, `w` across,
/// `h` deep along the axis, `segs` facets round.
#[expect(
    clippy::too_many_arguments,
    reason = "a ring's frame and section are one call"
)]
fn hoop(b: &mut MeshBuilder, c: Vec3, e1: Vec3, e2: Vec3, r: f32, w: f32, h: f32, segs: usize) {
    let axis = e1.cross(e2);
    let section = [
        v2(r - w * 0.5, -h * 0.5),
        v2(r + w * 0.5, -h * 0.5),
        v2(r + w * 0.5, h * 0.5),
        v2(r - w * 0.5, h * 0.5),
    ];
    let rings: Vec<Vec<Vec3>> = (0..=segs)
        .map(|i| {
            let a = TAU * (i % segs) as f32 / segs as f32;
            let d = e1 * a.cos() + e2 * a.sin();
            section.iter().map(|p| c + d * p.x + axis * p.y).collect()
        })
        .collect();
    b.loft(&rings, false, false);
}

fn level_hoop(b: &mut MeshBuilder, z: f32, r: f32, w: f32, h: f32, segs: usize) {
    hoop(b, v3(0.0, 0.0, z), Vec3::X, Vec3::Y, r, w, h, segs);
}

fn pad(b: &mut MeshBuilder, plan: &[[f32; 2]], top: f32) {
    b.paint(ACCENT);
    b.loft_z(plan, &[Section::new(0.0, 1.0), Section::new(0.7, 0.985)]);
    b.paint(PLATING);
    b.loft_z(
        plan,
        &[
            Section::new(0.6, 0.95),
            Section::new(top - 0.8, 0.84),
            Section::new(top, 0.8),
        ],
    );
}

fn power_run(b: &mut MeshBuilder, from: Vec3, to: Vec3, width: f32) {
    b.paint(ACCENT).pattern(pattern::FLUX);
    b.beam(from, to, v2(width, 0.3), v2(width, 0.3));
}

const SPIRE: [(f32, f32); 5] = [
    (3.0, 6.4),
    (8.0, 5.8),
    (20.0, 4.7),
    (32.0, 3.6),
    (40.5, 2.6),
];
const COLLARS: [(f32, f32); 3] = [(13.0, 10.0), (22.5, 8.6), (31.5, 7.0)];
const STAKE_FOOT: [f32; 2] = [14.0, 1.8];
const STAKE_HEAD: [f32; 2] = [17.8, 9.5];

fn spire_radius(z: f32) -> f32 {
    let mut r = SPIRE[0].1;
    for w in SPIRE.windows(2) {
        if z >= w[0].0 && z <= w[1].0 {
            r = w[0].1 + (w[1].1 - w[0].1) * (z - w[0].0) / (w[1].0 - w[0].0);
        }
    }
    r * (TAU / 16.0).cos()
}

pub(super) fn spire(b: &mut MeshBuilder, _tech: u8) {
    b.set_spinner_pivot(v3(0.0, 0.0, COLLARS[0].0));
    if b.coarse() {
        spire_coarse(b);
        return;
    }
    let fine = b.fine();
    let segs = if fine { 24 } else { 12 };
    pad(b, &chamfered_rect(v2(17.4, 17.4), 6.0), 3.2);
    b.paint(PLATING);
    let sections: Vec<Section> = SPIRE
        .iter()
        .map(|&(z, r)| Section::new(z, r / SPIRE[0].1))
        .collect();
    b.loft_z(&ngon(8, SPIRE[0].1), &sections);
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, 40.4), 8, 2.9, 2.5, 1.2);
    // Four buttresses brace the spire's foot into the pad.
    b.radial(4, |b| {
        b.paint(PLATING);
        b.extrude_y(
            &[
                [5.0, 3.0],
                [13.4, 3.0],
                [13.4, 5.4],
                [6.6, 11.5],
                [4.6, 11.5],
            ],
            -1.3,
            1.3,
        );
        b.paint(ACCENT);
        b.block(v3(9.6, -1.5, 3.0), v3(13.8, 1.5, 6.0));
        if fine {
            b.paint(GLOW);
            b.cuboid(v3(13.85, 0.0, 4.5), v3(0.12, 1.4, 0.6));
        }
    });
    if fine {
        b.radial(4, |b| {
            b.paint(ACCENT).pattern(pattern::FLUX);
            let at = |z: f32| v3(spire_radius(z) + 0.1, 0.0, z);
            b.beam(at(4.0), at(39.0), v2(1.2, 0.3), v2(0.7, 0.3));
        });
    }
    for (i, &(z, r)) in COLLARS.iter().enumerate() {
        let core = spire_radius(z);
        b.paint(ACCENT);
        b.prism(
            v3(0.0, 0.0, z - 1.0),
            b.sides(8),
            core + 0.8,
            core + 0.6,
            2.0,
        );
        let turns = i != 1;
        let collar = |b: &mut MeshBuilder| {
            b.paint(PLATING);
            level_hoop(b, z, r, 1.8, 1.4, segs);
            b.paint(METAL);
            level_hoop(b, z, core + 1.1, 0.6, 1.6, segs / 2);
            b.radial(if fine { 6 } else { 3 }, |b| {
                b.paint(PLATING_DARK);
                b.beam(
                    v3(core + 1.2, 0.0, z),
                    v3(r - 0.8, 0.0, z),
                    v2(0.8, 0.8),
                    v2(0.8, 0.5),
                );
            });
            if fine {
                b.paint(GLOW);
                level_hoop(b, z + 0.72, r, 0.35, 0.1, segs);
                let rake = if turns { 0.5 } else { -0.5 };
                b.radial(8, |b| {
                    b.yawed(v3(r + 0.6, 0.0, z), rake, |b| {
                        b.paint(PLATING);
                        b.extrude_y_chamfered(
                            &[[-0.3, -1.6], [1.6, -0.6], [1.6, 0.6], [-0.3, 1.6]],
                            0.3,
                            0.1,
                        );
                        b.paint(GLOW);
                        b.cuboid(v3(1.62, 0.0, 0.0), v3(0.1, 0.4, 1.0));
                    });
                });
            }
        };
        if turns {
            b.with_part(part::SPINNER, collar);
        } else {
            collar(b);
        }
    }
    b.paint(METAL);
    b.prism(v3(0.0, 0.0, 41.5), b.sides(8), 2.2, 1.7, 0.8);
    b.paint(GLOW);
    b.spheroid(
        EMITTER,
        v3(1.5, 1.5, 1.8),
        b.sides(8),
        if fine { 3 } else { 2 },
    );
    b.radial(3, |b| {
        b.paint(PLATING);
        b.beam(
            v3(2.3, 0.0, 40.6),
            v3(2.9, 0.0, 43.5),
            v2(1.3, 0.9),
            v2(1.1, 0.8),
        );
        b.beam(
            v3(2.9, 0.0, 43.3),
            v3(1.4, 0.0, 46.6),
            v2(1.1, 0.8),
            v2(0.4, 0.4),
        );
        if fine {
            b.paint(GLOW);
            b.cuboid(v3(2.6, 0.0, 44.4), v3(0.2, 0.5, 1.4));
        }
    });
    b.radial(4, |b| {
        b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
            let foot = v3(STAKE_FOOT[0], 0.0, STAKE_FOOT[1]);
            let head = v3(STAKE_HEAD[0], 0.0, STAKE_HEAD[1]);
            b.paint(ACCENT);
            b.chamfered_box(v3(14.6, 0.0, 2.2), v3(5.4, 5.4, 4.4), 1.2);
            b.paint(PLATING);
            b.cylinder_between(foot, head, 2.0, 1.4, b.sides(6));
            b.paint(METAL);
            b.cylinder_between(head, head + v3(0.35, 0.0, 0.9), 1.7, 1.0, b.sides(6));
            team_panel(b, v3(13.0, 0.0, 4.4), v2(2.6, 3.2));
            b.paint(METAL);
            let guy = v3(spire_radius(10.0) + 0.3, 0.0, 10.0);
            b.cylinder_between(head, guy, 0.55, 0.55, b.sides(6));
            if fine {
                b.paint(GLOW);
                let d = (head - foot).normalize();
                b.cylinder_between(head - d * 2.6, head - d * 2.0, 1.62, 1.58, 6);
                b.paint(ACCENT);
                b.chamfered_box(guy + v3(0.3, 0.0, 0.0), v3(1.2, 1.6, 1.6), 0.3);
                power_run(b, v3(12.0, 0.0, 3.35), v3(6.6, 0.0, 3.35), 1.2);
            }
        });
    });
}

fn spire_coarse(b: &mut MeshBuilder) {
    b.paint(ACCENT);
    b.decal(v3(0.0, 0.0, 0.2), v2(34.0, 34.0));
    team_panel(b, v3(0.0, 0.0, 0.3), v2(20.0, 20.0));
    b.paint(PLATING);
    b.frustum_open(
        v3(0.0, 0.0, 0.0),
        v2(11.0, 11.0),
        v2(4.0, 4.0),
        41.0,
        Vec2::ZERO,
    );
    b.with_part(part::SPINNER, |b| {
        b.paint(PLATING);
        b.cuboid_open(v3(0.0, 0.0, COLLARS[0].0), v3(20.0, 20.0, 1.4));
        b.cuboid_open(v3(0.0, 0.0, COLLARS[2].0), v3(14.0, 14.0, 1.4));
    });
    b.paint(GLOW);
    b.cuboid_open(v3(0.0, 0.0, 44.0), v3(3.0, 3.0, 4.6));
}
