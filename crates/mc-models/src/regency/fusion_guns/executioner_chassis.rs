//! Three separate armoured track sponsons: broad flanks and a long central prow.
use super::super::kit::v3;
use crate::builder::{MeshBuilder, Section};
use crate::material::{METAL, PLATING_DARK, TEAM, TREAD};
use crate::{part, pattern};
use glam::Vec3;

pub(super) fn metal(b: &mut MeshBuilder) {
    b.paint(METAL).pattern(pattern::REG_SENTRY);
}

pub(super) fn plate(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::REG_SENTRY);
}

pub(super) fn body(b: &mut MeshBuilder) {
    b.set_treads(5.0, 2.3, -10.6);
    b.set_dust_line(0.65);
    if b.coarse() {
        for (y, nose, tail, half) in [
            (0.0, 9.8, -6.8, 1.6),
            (-5.0, 6.3, -10.6, 1.65),
            (5.0, 6.3, -10.6, 1.65),
        ] {
            plate(b);
            b.loft(
                &[
                    vec![
                        v3(tail, y - half, 0.5),
                        v3(tail, y + half, 0.5),
                        v3(tail, y, 3.7),
                    ],
                    vec![
                        v3(nose, y - half, 0.5),
                        v3(nose, y + half, 0.5),
                        v3(nose, y, 2.2),
                    ],
                ],
                true,
                true,
            );
            b.with_part(part::LOCOMOTION, |b| {
                b.paint(TREAD).pattern(pattern::REG_SENTRY);
                b.face(&[
                    v3(nose + 0.01, y - half * 0.8, 0.0),
                    v3(nose + 0.01, y + half * 0.8, 0.0),
                    v3(nose + 0.01, y + half * 0.8, 0.9),
                    v3(nose + 0.01, y - half * 0.8, 0.9),
                ]);
            });
        }
        return;
    }
    for (y, nose, tail, half) in [
        (-5.0, 6.3, -10.6, 1.15),
        (0.0, 9.8, -6.8, 1.2),
        (5.0, 6.3, -10.6, 1.15),
    ] {
        track(b, y, nose, tail, half);
    }
    plate(b);
    b.loft_z(
        &[
            [4.2, 3.7],
            [-7.9, 3.7],
            [-9.0, 0.0],
            [-7.9, -3.7],
            [4.2, -3.7],
        ],
        &[Section::new(1.85, 1.0), Section::new(3.65, 0.8)],
    );
    // The pointed middle track cover projects well beyond the shorter side pods.
    wedge(b, 9.35, -4.7, 0.0, 1.65, 2.0, 1.75, true);
    b.mirror_y(|b| {
        wedge(b, 6.0, -10.5, 5.0, 1.62, 2.0, 1.8, true);
        // A second inset shoulder ends in the raised rear knife of the reference.
        wedge(b, 1.4, -10.7, 4.3, 1.12, 3.05, 1.5, false);
        wedge(b, 5.1, -5.2, 2.35, 0.7, 2.5, 1.1, false);
        // The reference's blue slash stays team-coloured for army readability.
        b.paint(TEAM).pattern(pattern::REG_SENTRY);
        let a = v3(-9.127, 5.207, 3.92);
        let c = v3(-10.7, 4.588, 4.55);
        let d = v3(-4.892, 4.154, 4.55);
        let normal = (c - a).cross(d - a).normalize();
        let at = |u, v| a * (1.0 - u - v) + c * u + d * v + normal * 0.018;
        b.face(&[at(0.15, 0.2), at(0.3, 0.2), at(0.22, 0.66), at(0.1, 0.55)]);
    });
}

/// A solid, bevelled arrowhead with an uneven ridge. Every plate is physically
/// thick; its triangular facets catch light independently of the surface texture.
pub(super) fn wedge(
    b: &mut MeshBuilder,
    nose: f32,
    tail: f32,
    y: f32,
    half: f32,
    z: f32,
    rise: f32,
    lamp: bool,
) {
    let len = nose - tail;
    let outline = [
        [nose, y - half * 0.62],
        [nose, y + half * 0.62],
        [nose - len * 0.35, y + half],
        [tail + len * 0.13, y + half * 0.88],
        [tail, y + half * 0.28],
        [tail + len * 0.08, y - half * 0.78],
        [nose - len * 0.38, y - half],
    ];
    let lower: Vec<Vec3> = outline.iter().map(|&[x, y]| v3(x, y, z)).collect();
    let upper: Vec<Vec3> = outline
        .iter()
        .enumerate()
        .map(|(k, &[x, yy])| {
            let h = match k {
                0 | 1 => 0.12,
                2 | 6 => 0.72,
                4 => 1.0,
                _ => 0.58,
            };
            v3(x, y + (yy - y) * 0.92, z + rise * h)
        })
        .collect();
    plate(b);
    b.loft(&[lower, upper.clone()], true, false);
    let ridge = v3(nose - len * 0.52, y - half * 0.13, z + rise);
    for k in 0..upper.len() {
        plate(b);
        // The nose plate owns its red slit; no luminous mesh is added above it.
        if lamp && k == 0 {
            // Only the short physical tip panel carries a running light.
            let a = upper[0].lerp(ridge, 0.28);
            let c = upper[1].lerp(ridge, 0.28);
            b.pattern(pattern::REG_SENTRY_SLOT);
            b.face(&[upper[0], upper[1], c, a]);
            plate(b);
            b.face(&[a, c, ridge]);
        } else {
            b.face(&[upper[k], upper[(k + 1) % upper.len()], ridge]);
        }
    }
}

fn track(b: &mut MeshBuilder, y: f32, nose: f32, tail: f32, half: f32) {
    b.with_part(part::LOCOMOTION, |b| {
        b.paint(TREAD).pattern(pattern::REG_SENTRY);
        let outline = [
            [tail + 0.7, 0.0],
            [nose - 0.65, 0.0],
            [nose, 0.9],
            [nose - 0.45, 1.8],
            [tail + 0.7, 1.8],
            [tail, 0.85],
        ];
        b.extrude_y(&outline, y - half, y + half);
        if b.fine() {
            // Flat shoes, with separated cleats across their two visible front
            // slopes. The continuous belt retains the moving tread shader.
            metal(b);
            for (x, z) in [
                (nose - 0.46, 0.22),
                (nose - 0.23, 0.55),
                (nose - 0.03, 0.88),
                (nose - 0.17, 1.22),
                (nose - 0.34, 1.54),
            ] {
                for side in [-1.0, 1.0] {
                    b.beam(
                        v3(x - 0.11, y + side * half * 0.48, z),
                        v3(x + 0.05, y + side * half * 0.48, z),
                        glam::Vec2::new(half * 0.82, 0.19),
                        glam::Vec2::new(half * 0.82, 0.19),
                    );
                }
            }
            for i in 0..6 {
                let x = tail + 1.1 + (nose - tail - 2.2) * i as f32 / 5.0;
                b.cylinder_between(
                    v3(x, y - half - 0.02, 0.86),
                    v3(x, y + half + 0.02, 0.86),
                    0.65,
                    0.65,
                    8,
                );
            }
        }
    });
}
