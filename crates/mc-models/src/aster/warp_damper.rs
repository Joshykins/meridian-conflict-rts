//! Undertow (`aster_t2_warp_damper`, mesh "warp_damper"): the tech 2 warp dampener,
//! authored at blueprint scale (metres, 4x4 lot, radius 18, height 46).
//!
//! A heavy anchored machine that grips space: it throws a 1600 m field that
//! snags a capital ship's jump. Every design keeps its field emitter at the top
//! centre, [`EMITTER`] (0.95 of the height), where the tether beam leaves, and stands
//! back from its lot's edge. Its lights (`GLOW`) and spinning gear (`part::SPINNER`)
//! go dark and still when the grid stalls, as the field does.
//!
//! Variants on trial (`warp_damper~a`, `~b`, `~c`):
//! - [`gyre`]: four anchor legs clawing up round a fixed equator ring, holding a
//!   crown over a floating core that two gimbal rings spin round.
//! - [`spire`]: a tapering spire with stacked emitter collars, the outer two
//!   turning, guyed down to four stakes driven into the ground.
//! - [`talon`]: three broad claws rising from the pad and curling in to grip the
//!   emitter node, a field needle between them with spinning halos on it.
//!

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

/// A strut swept along `path` in the xz plane, (thickness in xz, width along y) per
/// point; `lift` pushes it off the centre line along the outward normal.
fn strut(b: &mut MeshBuilder, path: &[[f32; 2]], size: &[[f32; 2]], lift: f32) {
    let n = path.len();
    let rings: Vec<Vec<Vec3>> = (0..n)
        .map(|i| {
            let p = Vec2::from(path[i]);
            let (a, c) = (path[i.saturating_sub(1)], path[(i + 1).min(n - 1)]);
            let t = (Vec2::from(c) - Vec2::from(a)).normalize();
            let out = v2(t.y, -t.x);
            let o = p + out * lift;
            let (th, w) = (size[i][0] * 0.5, size[i][1] * 0.5);
            let at = |s: f32, u: f32| {
                let q = o + out * s;
                v3(q.x, u, q.y)
            };
            vec![at(-th, -w), at(th, -w), at(th, w), at(-th, w)]
        })
        .collect();
    b.loft(&rings, true, true);
}

fn taper(n: usize, from: [f32; 2], to: [f32; 2]) -> Vec<[f32; 2]> {
    (0..n)
        .map(|i| {
            let f = i as f32 / (n - 1).max(1) as f32;
            [
                from[0] + (to[0] - from[0]) * f,
                from[1] + (to[1] - from[1]) * f,
            ]
        })
        .collect()
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

fn bank(b: &mut MeshBuilder, at: Vec3) {
    b.paint(ACCENT);
    b.prism(at, b.sides(6), 1.5, 1.3, 2.8);
    b.paint(GLOW);
    b.prism(at + Vec3::Z * 2.8, b.sides(6), 0.85, 0.5, 0.45);
}

fn power_run(b: &mut MeshBuilder, from: Vec3, to: Vec3, width: f32) {
    b.paint(ACCENT).pattern(pattern::FLUX);
    b.beam(from, to, v2(width, 0.3), v2(width, 0.3));
}

// ---- (a) Gyre ----
const GYRE_C: f32 = 26.0;
const GYRE_FRAME: f32 = 15.6;
const GYRE_SPIN: f32 = 12.0;
const GYRE_CROWN: f32 = 38.9;
const GYRE_BASE: f32 = 12.9;
const GYRE_LEG: [[f32; 2]; 8] = [
    [14.2, 6.0],
    [15.6, 13.0],
    [16.5, 19.5],
    [16.7, 26.0],
    [15.9, 31.5],
    [13.6, 36.0],
    [9.9, 39.6],
    [4.6, 41.6],
];

pub(super) fn gyre(b: &mut MeshBuilder, _tech: u8) {
    b.set_spinner_pivot(v3(0.0, 0.0, GYRE_C));
    if b.coarse() {
        gyre_coarse(b);
        return;
    }
    let fine = b.fine();
    let segs = if fine { 32 } else { 14 };
    pad(b, &ngon(8, 18.4), 3.0);
    b.radial(4, |b| {
        b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
            b.paint(PLATING);
            b.extrude_y(
                &[
                    [10.2, 0.6],
                    [17.4, 0.6],
                    [17.4, 2.6],
                    [15.4, 7.6],
                    [10.2, 7.6],
                ],
                -3.3,
                3.3,
            );
            b.paint(ACCENT);
            b.block(v3(10.0, -3.5, 7.4), v3(15.6, 3.5, 8.2));
            team_panel(b, v3(12.4, 0.0, 8.2), v2(3.2, 4.4));
            b.paint(PLATING);
            let path: &[[f32; 2]] = if fine {
                &GYRE_LEG
            } else {
                &[
                    GYRE_LEG[0],
                    GYRE_LEG[2],
                    GYRE_LEG[4],
                    GYRE_LEG[6],
                    GYRE_LEG[7],
                ]
            };
            strut(b, path, &taper(path.len(), [3.6, 4.4], [1.8, 2.2]), 0.0);
            b.paint(ACCENT).pattern(pattern::FLUX);
            strut(
                b,
                &path[..path.len() - 1],
                &taper(path.len() - 1, [0.3, 1.4], [0.3, 0.8]),
                1.75,
            );
            b.paint(ACCENT);
            b.chamfered_box(v3(16.2, 0.0, GYRE_C), v3(3.8, 5.0, 5.2), 0.9);
            if fine {
                b.paint(GLOW);
                b.cuboid(v3(18.15, 0.0, GYRE_C), v3(0.2, 2.4, 3.0));
                b.paint(ACCENT);
                for (i, s) in [(5usize, 2.3f32), (6, 2.0)] {
                    let [x, z] = GYRE_LEG[i];
                    b.chamfered_box(v3(x, 0.0, z), v3(s, 2.6, s), 0.5);
                }
            }
        });
    });
    b.radial(4, |b| {
        bank(b, v3(12.6, 0.0, 3.0));
        if fine {
            power_run(b, v3(11.2, 0.0, 3.15), v3(6.0, 0.0, 3.15), 1.3);
        }
    });
    b.paint(PLATING);
    b.loft_z(
        &ngon(b.sides(8), 7.6),
        &[
            Section::new(2.9, 1.0),
            Section::new(6.5, 0.9),
            Section::new(9.6, 0.62),
        ],
    );
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, 9.5), b.sides(8), 4.2, 3.4, 2.2);
    b.paint(GLOW);
    b.prism(v3(0.0, 0.0, 11.7), b.sides(8), 3.0, 2.2, GYRE_BASE - 11.7);
    b.paint(PLATING);
    level_hoop(b, GYRE_C, GYRE_FRAME, 1.8, 2.4, segs);
    if fine {
        b.paint(ACCENT);
        level_hoop(b, GYRE_C - 1.5, GYRE_FRAME - 0.3, 1.2, 0.6, segs);
        b.paint(GLOW);
        level_hoop(b, GYRE_C, GYRE_FRAME - 0.95, 0.1, 0.8, segs);
    }
    // The core: a held singularity, glowing through a fixed armoured cage.
    b.paint(GLOW);
    b.spheroid(
        v3(0.0, 0.0, GYRE_C),
        v3(3.4, 3.4, 5.4),
        b.sides(8),
        if fine { 4 } else { 2 },
    );
    b.paint(PLATING_DARK);
    let core = v3(0.0, 0.0, GYRE_C);
    let d = Vec3::new(FRAC_PI_4.cos(), FRAC_PI_4.sin(), 0.0);
    for e1 in [d, v3(-d.y, d.x, 0.0)] {
        hoop(b, core, e1, Vec3::Z * 1.45, 3.9, 0.9, 1.3, segs / 2);
    }
    b.prism(v3(0.0, 0.0, GYRE_C - 6.2), b.sides(8), 2.4, 1.6, 1.4);
    b.prism(v3(0.0, 0.0, GYRE_C + 4.8), b.sides(8), 1.6, 2.4, 1.4);
    b.with_part(part::SPINNER, |b| {
        let c = v3(0.0, 0.0, GYRE_C);
        for (e1, e2) in [(Vec3::X, Vec3::Z), (Vec3::Y, Vec3::Z)] {
            b.paint(PLATING_DARK);
            hoop(b, c, e1, e2, GYRE_SPIN, 1.4, 1.2, segs);
            if fine {
                b.paint(GLOW);
                hoop(b, c, e1, e2, GYRE_SPIN - 0.8, 0.2, 0.5, segs);
            }
        }
        if fine {
            b.radial(4, |b| {
                b.paint(METAL);
                b.cuboid(v3(GYRE_SPIN + 0.4, 0.0, GYRE_C), v3(1.4, 1.8, 2.4));
                b.paint(GLOW);
                b.cuboid(v3(GYRE_SPIN + 1.15, 0.0, GYRE_C), v3(0.2, 1.0, 1.4));
            });
        }
        b.paint(METAL);
        b.prism(v3(0.0, 0.0, GYRE_BASE), b.sides(8), 1.8, 1.5, 1.8);
        b.prism(v3(0.0, 0.0, GYRE_CROWN - 1.9), b.sides(8), 1.5, 1.8, 1.9);
    });
    b.paint(PLATING);
    b.loft_z(
        &ngon(b.sides(8), 5.4),
        &[
            Section::new(GYRE_CROWN, 0.72),
            Section::new(GYRE_CROWN + 1.2, 1.0),
            Section::new(42.0, 0.9),
        ],
    );
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, 41.9), b.sides(8), 3.4, 2.6, 1.1);
    emitter(b, 42.9);
}

fn emitter(b: &mut MeshBuilder, z: f32) {
    b.paint(METAL);
    b.prism(v3(0.0, 0.0, z), b.sides(8), 2.4, 1.9, 0.7);
    b.paint(GLOW);
    b.spheroid(
        EMITTER,
        v3(1.5, 1.5, 1.6),
        b.sides(8),
        if b.fine() { 3 } else { 2 },
    );
    b.radial(4, |b| {
        b.paint(PLATING);
        b.beam(
            v3(2.3, 0.0, z + 0.2),
            v3(1.5, 0.0, 46.4),
            v2(1.0, 0.5),
            v2(0.5, 0.3),
        );
    });
}

fn gyre_coarse(b: &mut MeshBuilder) {
    b.paint(ACCENT);
    b.decal(v3(0.0, 0.0, 0.2), v2(34.0, 34.0));
    team_panel(b, v3(0.0, 0.0, 0.3), v2(8.0, 8.0));
    b.radial(4, |b| {
        b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
            b.paint(PLATING);
            coarse_limb(b, &[GYRE_LEG[0], GYRE_LEG[7]], [2.2, 1.0]);
        });
    });
    b.with_part(part::SPINNER, |b| {
        b.paint(PLATING_DARK);
        b.cuboid_open(v3(0.0, 0.0, GYRE_C), v3(0.8, 24.0, 24.0));
    });
    b.paint(GLOW);
    b.cuboid_open(v3(0.0, 0.0, GYRE_C), v3(6.0, 6.0, 10.0));
}

// ---- (b) Spire ----
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

// ---- (c) Talon ----
const CLAW: [[f32; 2]; 7] = [
    [15.0, 2.4],
    [16.2, 10.0],
    [15.8, 19.5],
    [13.6, 28.5],
    [9.8, 35.8],
    [5.6, 40.6],
    [2.9, 42.9],
];
const HALOS: [(f32, f32); 2] = [(20.0, 9.0), (29.5, 6.6)];

pub(super) fn talon(b: &mut MeshBuilder, _tech: u8) {
    b.set_spinner_pivot(v3(0.0, 0.0, HALOS[0].0));
    if b.coarse() {
        talon_coarse(b);
        return;
    }
    let fine = b.fine();
    let segs = if fine { 28 } else { 12 };
    pad(b, &ngon(6, 18.6), 2.6);
    b.radial(3, |b| {
        b.paint(PLATING);
        b.extrude_y(
            &[
                [10.8, 0.6],
                [17.8, 0.6],
                [17.8, 2.4],
                [16.0, 6.4],
                [11.4, 6.4],
            ],
            -3.6,
            3.6,
        );
        team_panel(b, v3(13.0, 0.0, 6.4), v2(2.8, 4.6));
        let path: &[[f32; 2]] = if fine {
            &CLAW
        } else {
            &[CLAW[0], CLAW[2], CLAW[4], CLAW[6]]
        };
        b.paint(PLATING);
        strut(b, path, &taper(path.len(), [3.8, 5.2], [1.3, 1.8]), 0.0);
        b.paint(ACCENT).pattern(pattern::FLUX);
        strut(
            b,
            &path[..path.len() - 1],
            &taper(path.len() - 1, [0.6, 1.8], [0.5, 0.9]),
            -1.75,
        );
        if fine {
            for i in [2usize, 4] {
                let [x, z] = CLAW[i];
                b.paint(ACCENT);
                b.chamfered_box(v3(x, 0.0, z), v3(3.0, 4.6, 2.2), 0.6);
                b.paint(GLOW);
                b.cuboid(v3(x - 1.55, 0.0, z), v3(0.2, 2.4, 0.9));
            }
            power_run(b, v3(10.6, 0.0, 2.75), v3(7.2, 0.0, 2.75), 1.4);
        }
    });
    b.yawed(Vec3::ZERO, TAU / 6.0, |b| {
        b.radial(3, |b| bank(b, v3(12.2, 0.0, 2.6)))
    });
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, 2.5), b.sides(12), 7.2, 6.6, 3.0);
    b.paint(GLOW);
    level_hoop(b, 5.5, 5.6, 1.0, 0.3, segs);
    b.paint(GLASS);
    b.prism(v3(0.0, 0.0, 5.3), b.sides(12), 4.8, 4.6, 0.3);
    b.paint(METAL);
    b.prism(v3(0.0, 0.0, 5.5), b.sides(8), 2.6, 1.7, 33.5);
    b.with_part(part::SPINNER, |b| {
        for &(z, r) in &HALOS {
            b.paint(PLATING);
            level_hoop(b, z, r, 2.2, 1.6, segs);
            b.paint(ACCENT);
            b.prism(v3(0.0, 0.0, z - 1.0), b.sides(8), 3.0, 3.0, 2.0);
            b.radial(3, |b| {
                b.paint(PLATING_DARK);
                b.beam(
                    v3(2.8, 0.0, z),
                    v3(r - 0.6, 0.0, z),
                    v2(0.8, 0.9),
                    v2(0.6, 0.5),
                );
            });
            if fine {
                b.paint(GLOW);
                level_hoop(b, z + 0.82, r, 0.5, 0.1, segs);
                b.radial(6, |b| {
                    b.paint(METAL);
                    b.cuboid(v3(r + 0.9, 0.0, z), v3(0.8, 1.2, 1.8));
                    b.paint(GLOW);
                    b.cuboid(v3(r + 1.35, 0.0, z), v3(0.12, 0.7, 1.1));
                });
            }
        }
    });
    // The armoured emitter node the claw tips close on.
    b.paint(PLATING);
    b.loft_z(
        &ngon(b.sides(8), 4.4),
        &[
            Section::new(38.4, 0.45),
            Section::new(40.2, 1.0),
            Section::new(42.4, 0.9),
            Section::new(43.0, 0.72),
        ],
    );
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, 42.9), b.sides(8), 3.1, 2.7, 0.5);
    if fine {
        b.paint(GLOW);
        level_hoop(b, 40.2, 4.45, 0.2, 0.5, segs);
    }
    b.paint(GLOW);
    b.spheroid(
        EMITTER + Vec3::Z * 0.6,
        v3(1.9, 1.9, 2.0),
        b.sides(8),
        if fine { 3 } else { 2 },
    );
}

fn talon_coarse(b: &mut MeshBuilder) {
    team_panel(b, v3(0.0, 0.0, 0.3), v2(10.0, 10.0));
    b.radial(3, |b| {
        b.paint(PLATING);
        coarse_limb(b, &[CLAW[0], CLAW[3], CLAW[6]], [3.0, 1.0]);
    });
    b.with_part(part::SPINNER, |b| {
        b.paint(PLATING);
        b.cuboid_open(v3(0.0, 0.0, HALOS[0].0), v3(18.0, 18.0, 1.2));
    });
    b.paint(GLOW);
    b.cuboid_open(v3(0.0, 0.0, 42.0), v3(4.0, 4.0, 4.4));
}

/// A limb bent through `path` (xz) at far detail: a triangular section, its apex
/// outward, `half` its half-width at the root and the tip.
fn coarse_limb(b: &mut MeshBuilder, path: &[[f32; 2]], half: [f32; 2]) {
    let n = path.len();
    let rings: Vec<Vec<Vec3>> = path
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let w = half[0] + (half[1] - half[0]) * i as f32 / (n - 1) as f32;
            vec![
                v3(p[0] - w, -w, p[1]),
                v3(p[0] + w, 0.0, p[1]),
                v3(p[0] - w, w, p[1]),
            ]
        })
        .collect();
    b.loft(&rings, false, false);
}
