//! The reclaim head all three tower designs carry: gun house 0, a turntable and two
//! cheeks turning on the tower's slewing ring, and between the cheeks the projector,
//! which pitches about its trunnion steeply up and down. Its intake bell ends at the
//! emitter ([`super::EMIT_X`]).

use glam::{Vec2, Vec3};

use super::super::parts::*;
use super::{tier, EMIT_X, HEAD_SCALE, PIVOT_Z};
use crate::builder::MeshBuilder;
use crate::material::*;
use crate::pattern;

/// How the projector is dressed: the design each tower goes with.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Style {
    /// A long slender barrel in collars, an open yoke (the lattice derrick).
    Boom,
    /// A heavy faceted armoured body, short and wide (the spire).
    Pod,
    /// Four jaw plates round the intake bell, a clamshell (the four-legged tower).
    Jaws,
}

/// The fixed bearing on the tower top, then the head in its house.
pub(super) fn head(b: &mut MeshBuilder, tech: u8, style: Style) {
    let t = tier(tech);
    let (pz, s, emit) = (PIVOT_Z[t], HEAD_SCALE[t], EMIT_X[t]);
    let ring = super::ring_top(tech);
    let pivot = v3(0.0, 0.0, pz);

    // The slewing ring's fixed race, on the tower.
    if !b.coarse() {
        b.paint(ACCENT);
        b.prism(
            v3(0.0, 0.0, ring - 1.6),
            b.sides(12),
            3.1 * s,
            3.1 * s,
            0.75,
        );
    }

    b.with_house(0, pivot, 0.0, |b| {
        if b.coarse() {
            b.paint(PLATING);
            b.cuboid(
                v3(-0.6 * s, 0.0, (ring + pz) * 0.5),
                v3(5.0 * s, 5.0 * s, pz - ring + 1.8 * s),
            );
            b.with_recoil(|b| {
                b.paint(PLATING_DARK);
                b.cuboid(v3(emit * 0.5, 0.0, pz), v3(emit, 2.0 * s, 2.0 * s));
            });
            return;
        }
        yoke(b, ring, pz, s, style);
        b.with_recoil(|b| projector(b, pz, s, emit, style, tech));
    });
}

/// The part of the head that only turns: turntable, the arms carrying the trunnion, the
/// drive house behind.
fn yoke(b: &mut MeshBuilder, ring: f32, pz: f32, s: f32, style: Style) {
    b.paint(PLATING_DARK);
    b.prism(
        v3(0.0, 0.0, ring - 0.85),
        b.sides(12),
        3.5 * s,
        3.4 * s,
        0.85,
    );
    b.paint(PLATING);
    b.prism(v3(0.0, 0.0, ring), b.sides(12), 3.3 * s, 3.0 * s, 0.3);
    if b.fine() {
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, ring - 0.95), 12, 3.62 * s, 3.62 * s, 0.25);
    }

    let (inner, outer) = (2.2 * s, 2.8 * s);
    let arm = (inner + outer) * 0.5;
    b.mirror_y(|b| {
        if style == Style::Pod {
            // Armoured cheeks: slabs rising either side, a lightening cut through them.
            b.paint(PLATING);
            b.extrude_y(
                &[
                    [-2.6 * s, ring],
                    [2.3 * s, ring],
                    [1.4 * s, pz + 0.4 * s],
                    [0.6 * s, pz + 1.6 * s],
                    [-1.2 * s, pz + 1.6 * s],
                    [-2.3 * s, pz - 0.2 * s],
                ],
                inner,
                outer,
            );
            b.paint(ACCENT);
            b.extrude_y(
                &[
                    [-1.4 * s, ring + 0.9 * s],
                    [1.2 * s, ring + 0.9 * s],
                    [0.5 * s, pz - 1.8 * s],
                    [-s, pz - 1.8 * s],
                ],
                outer - 0.02,
                outer + 0.06,
            );
        } else {
            // An A-frame each side: two raked legs from the turntable to the bearing,
            // tied by a web low down, so the projector is seen between them.
            b.paint(PLATING);
            let foot = if style == Style::Boom { 2.3 } else { 2.6 };
            for x in [-foot * s, foot * s] {
                b.beam(
                    v3(x, arm, ring + 0.2),
                    v3(x * 0.18, arm, pz - 0.2 * s),
                    Vec2::new(0.62 * s, 0.9 * s),
                    Vec2::new(0.55 * s, 0.7 * s),
                );
            }
            b.paint(PLATING_DARK);
            b.extrude_y(
                &[
                    [-foot * s, ring + 0.1],
                    [foot * s, ring + 0.1],
                    [foot * s * 0.72, ring + 1.3 * s],
                    [-foot * s * 0.72, ring + 1.3 * s],
                ],
                arm - 0.2 * s,
                arm + 0.2 * s,
            );
            // The bearing block the legs meet in.
            b.paint(PLATING);
            b.chamfered_box(
                v3(0.0, arm, pz),
                v3(1.9 * s, outer - inner, 2.0 * s),
                0.45 * s,
            );
        }
        // The trunnion bearing cap on the outside.
        b.paint(METAL);
        b.cylinder_between(
            v3(0.0, outer - 0.1, pz),
            v3(0.0, outer + 0.4 * s, pz),
            0.85 * s,
            0.7 * s,
            b.sides(10),
        );
        if b.fine() {
            b.paint(ACCENT);
            b.cylinder_between(
                v3(0.0, outer + 0.35 * s, pz),
                v3(0.0, outer + 0.55 * s, pz),
                0.42 * s,
                0.42 * s,
                8,
            );
            // The elevation ram: from the turntable up to a lug under the trunnion.
            b.paint(METAL);
            b.cylinder_between(
                v3(-2.0 * s, inner - 0.3 * s, ring + 0.3),
                v3(-0.9 * s, inner - 0.3 * s, pz - 1.3 * s),
                0.26 * s,
                0.2 * s,
                6,
            );
        }
    });

    // The drive house on the turntable's back edge, low so the counterweight clears it.
    b.paint(PLATING_DARK);
    b.chamfered_box(
        v3(-2.55 * s, 0.0, ring + 0.65 * s),
        v3(1.5 * s, 3.4 * s, 1.3 * s),
        0.3 * s,
    );
    b.paint(PLATING);
    b.plate(
        v3(-2.55 * s, 0.0, ring + 1.3 * s),
        Vec2::new(1.2 * s, 3.0 * s),
        0.1,
        0.04,
    );
    team_panel(
        b,
        v3(-2.55 * s, 0.0, ring + 1.4 * s),
        Vec2::new(0.8 * s, 1.8 * s),
    );
    if b.fine() {
        // The feed from the head drops through the ring's middle into the tower.
        b.paint(ACCENT).pattern(pattern::MASS_FLOW);
        b.cylinder_between(
            v3(-0.6 * s, 0.0, pz - 1.4 * s),
            v3(-0.2 * s, 0.0, ring - 0.4),
            0.45 * s,
            0.45 * s,
            8,
        );
        whip(b, v3(-2.9 * s, 1.3 * s, ring + 1.3 * s), 3.0 * s, 0.08);
    }
}

/// What pitches: the core drum on the trunnion, the barrel, the intake bell ending at
/// the emitter, and the feed lines that carry what it takes back into the core.
fn projector(b: &mut MeshBuilder, pz: f32, s: f32, emit: f32, style: Style, tech: u8) {
    let axis = |x: f32| v3(x, 0.0, pz);
    let sides = b.sides(12);
    let half = 2.15 * s;

    // The core: a drum across the trunnion, capped dark, a plated saddle over it.
    b.paint(PLATING);
    b.cylinder_between(
        v3(0.0, -half, pz),
        v3(0.0, half, pz),
        1.45 * s,
        1.45 * s,
        sides,
    );
    b.paint(ACCENT);
    for y in [-half, half] {
        b.cylinder_between(
            v3(0.0, y * 0.97, pz),
            v3(0.0, y * 1.02, pz),
            1.55 * s,
            1.55 * s,
            sides,
        );
    }
    let bell_start = emit - 2.6 * s;
    let neck = match style {
        Style::Boom => 2.0 * s,
        Style::Pod => 3.2 * s,
        Style::Jaws => 2.6 * s,
    };
    b.paint(if style == Style::Pod {
        PLATING
    } else {
        PLATING_DARK
    });
    b.with_bevel(0.12 * s, |b| {
        b.chamfered_box(
            v3((neck - 3.0 * s) * 0.5, 0.0, pz + 0.25 * s),
            v3(neck + 3.0 * s, 2.7 * s, 2.2 * s),
            0.5 * s,
        );
    });
    // The counterweight hopper behind: what the head takes is seen held in it.
    b.paint(PLATING_DARK);
    b.chamfered_box(
        v3(-3.1 * s, 0.0, pz + 0.1 * s),
        v3(1.4 * s, 3.0 * s, 2.0 * s),
        0.3 * s,
    );
    if b.mid() {
        b.mirror_y(|b| {
            b.paint(ACCENT).pattern(pattern::MASS_FLOW);
            b.block(
                v3(-3.6 * s, 1.48 * s, pz - 0.5 * s),
                v3(-2.6 * s, 1.56 * s, pz + 0.7 * s),
            );
        });
    }

    // The barrel, by style.
    match style {
        Style::Boom => {
            b.paint(PLATING);
            b.cylinder_between(axis(neck), axis(bell_start), 0.72 * s, 0.6 * s, sides);
            if b.mid() {
                b.paint(ACCENT);
                for i in 0..3 {
                    let x = neck + 0.5 * s + (bell_start - neck - 1.2 * s) * i as f32 / 2.0;
                    b.cylinder_between(axis(x), axis(x + 0.45 * s), 0.9 * s, 0.9 * s, sides);
                }
            }
        }
        Style::Pod => {
            b.paint(PLATING_DARK);
            b.cylinder_between(axis(neck), axis(bell_start), 1.05 * s, 0.9 * s, sides);
            b.paint(ACCENT);
            b.cylinder_between(axis(neck), axis(neck + 0.5 * s), 1.3 * s, 1.3 * s, sides);
        }
        Style::Jaws => {
            b.paint(PLATING_DARK);
            b.cylinder_between(axis(neck), axis(bell_start), 0.95 * s, 0.85 * s, sides);
            if b.fine() {
                b.paint(METAL);
                let n = 5;
                for i in 0..n {
                    let x = neck + 0.4 * s + 0.42 * s * i as f32;
                    b.cylinder_between(axis(x), axis(x + 0.2 * s), 1.08 * s, 1.08 * s, sides);
                }
            }
        }
    }
    // Feed lines along the top of the barrel, from the bell back into the core.
    if b.fine() {
        b.mirror_y(|b| {
            b.paint(ACCENT).pattern(pattern::MASS_FLOW);
            b.cylinder_between(
                v3(bell_start + 0.3 * s, 0.55 * s, pz + 0.95 * s),
                v3(neck - 0.4 * s, 0.7 * s, pz + 1.2 * s),
                0.26 * s,
                0.26 * s,
                b.sides(6),
            );
        });
    }

    // The intake bell: a hollow horn, flared wide and turned in at the lip, so its mouth
    // is a dark throat with the emitter glowing at the bottom, not a lid.
    let mouth = if style == Style::Pod { 2.2 } else { 1.9 } * s;
    let ring = |x: f32, r: f32| -> Vec<Vec3> {
        (0..sides)
            .map(|i| {
                let a = std::f32::consts::TAU * (i as f32 + 0.5) / sides as f32;
                v3(x, r * a.cos(), pz + r * a.sin())
            })
            .collect()
    };
    b.paint(PLATING);
    b.loft(
        &[
            ring(bell_start, 0.9 * s),
            ring(emit - 0.3 * s, mouth),
            ring(emit, mouth + 0.1 * s),
            ring(emit, mouth - 0.18 * s),
            ring(emit - 1.9 * s, 0.55 * s),
        ],
        true,
        true,
    );
    // A glowing band round the outside of the lip: a closed ring, open through the middle.
    b.paint(GLOW_MATERIALS);
    let (x0, x1) = (emit - 0.3 * s, emit - 0.1 * s);
    let (outer, inner) = (mouth + 0.1 * s, mouth - 0.02 * s);
    b.loft(
        &[
            ring(x0, outer),
            ring(x1, outer + 0.06 * s),
            ring(x1, inner),
            ring(x0, inner),
            ring(x0, outer),
        ],
        false,
        false,
    );
    b.cylinder_between(
        axis(emit - 1.95 * s),
        axis(emit - 1.75 * s),
        0.5 * s,
        0.5 * s,
        b.sides(8),
    );
    if b.fine() && tech >= 2 {
        // Focusing coils round the bell's throat.
        b.paint(METAL);
        for k in 0..tech - 1 {
            let x = bell_start - 0.1 + 0.55 * s * f32::from(k);
            b.cylinder_between(axis(x), axis(x + 0.3 * s), 1.12 * s, 1.12 * s, sides);
        }
    }

    // Tines round the mouth: the claw that reads as reclaim from afar.
    let (tines, root, tip, w) = match style {
        Style::Jaws => (4, 1.3 * s, 2.35 * s, Vec2::new(1.5 * s, 0.24 * s)),
        Style::Pod => (3, 1.6 * s, 2.1 * s, Vec2::new(0.4 * s, 0.34 * s)),
        Style::Boom => (3, 1.3 * s, 1.95 * s, Vec2::new(0.36 * s, 0.3 * s)),
    };
    let reach = if style == Style::Jaws { -0.4 * s } else { 0.0 };
    for i in 0..tines {
        let a = std::f32::consts::TAU * (i as f32 + 0.5) / tines as f32;
        let dir = Vec3::new(0.0, a.cos(), a.sin());
        b.paint(if style == Style::Jaws {
            PLATING
        } else {
            ACCENT
        });
        b.beam(
            axis(bell_start + 0.4 * s) + dir * root,
            axis(emit + reach) + dir * tip,
            w,
            w * 0.7,
        );
    }
}
