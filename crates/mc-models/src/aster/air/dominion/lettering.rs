//! Painted stencil lettering for a hull's flank: block capitals cut into separate pieces
//! with hard 45 degree corners, each piece a flat painted face laid a hand off a flat
//! wall. Text reads from bow to stern on the port side and stern to bow on starboard, so
//! it reads left to right from either side.

use super::*;

/// A glyph: its advance width in cells (6 cells tall), and its convex pieces as
/// counter-clockwise outlines (u along the text, v up).
type Glyph = (f32, &'static [&'static [[f32; 2]]]);

const A: Glyph = (
    4.0,
    &[
        &[[0.0, 0.0], [1.0, 0.0], [1.0, 4.2], [0.0, 4.2]],
        &[[3.0, 0.0], [4.0, 0.0], [4.0, 4.2], [3.0, 4.2]],
        &[[1.2, 2.2], [2.8, 2.2], [2.8, 3.2], [1.2, 3.2]],
        &[[0.0, 4.4], [4.0, 4.4], [3.0, 6.0], [1.0, 6.0]],
    ],
);
const C: Glyph = (
    4.0,
    &[
        &[[0.0, 1.0], [1.0, 0.0], [1.0, 6.0], [0.0, 5.0]],
        &[[1.2, 5.0], [4.0, 5.0], [4.0, 6.0], [1.2, 6.0]],
        &[[1.2, 0.0], [4.0, 0.0], [4.0, 1.0], [1.2, 1.0]],
    ],
);
const D: Glyph = (
    4.0,
    &[
        &[[0.0, 0.0], [1.0, 0.0], [1.0, 6.0], [0.0, 6.0]],
        &[[1.2, 5.0], [2.8, 5.0], [2.8, 6.0], [1.2, 6.0]],
        &[[1.2, 0.0], [2.8, 0.0], [2.8, 1.0], [1.2, 1.0]],
        &[[3.0, 0.0], [4.0, 1.0], [4.0, 5.0], [3.0, 6.0]],
    ],
);
const I: Glyph = (1.0, &[&[[0.0, 0.0], [1.0, 0.0], [1.0, 6.0], [0.0, 6.0]]]);
const M: Glyph = (
    5.0,
    &[
        &[[0.0, 0.0], [1.0, 0.0], [1.0, 6.0], [0.0, 6.0]],
        &[[4.0, 0.0], [5.0, 0.0], [5.0, 6.0], [4.0, 6.0]],
        &[[1.2, 4.2], [2.4, 2.6], [2.4, 4.0], [1.2, 5.6]],
        &[[2.6, 2.6], [3.8, 4.2], [3.8, 5.6], [2.6, 4.0]],
    ],
);
const N: Glyph = (
    4.0,
    &[
        &[[0.0, 0.0], [1.0, 0.0], [1.0, 6.0], [0.0, 6.0]],
        &[[3.0, 0.0], [4.0, 0.0], [4.0, 6.0], [3.0, 6.0]],
        &[[1.2, 4.4], [2.8, 0.0], [2.8, 1.6], [1.2, 6.0]],
    ],
);
const O: Glyph = (
    4.0,
    &[
        &[[0.0, 1.0], [1.0, 0.0], [1.0, 6.0], [0.0, 5.0]],
        &[[3.0, 0.0], [4.0, 1.0], [4.0, 5.0], [3.0, 6.0]],
        &[[1.2, 5.0], [2.8, 5.0], [2.8, 6.0], [1.2, 6.0]],
        &[[1.2, 0.0], [2.8, 0.0], [2.8, 1.0], [1.2, 1.0]],
    ],
);
const R: Glyph = (
    4.0,
    &[
        &[[0.0, 0.0], [1.0, 0.0], [1.0, 6.0], [0.0, 6.0]],
        &[[1.2, 5.0], [3.0, 5.0], [3.0, 6.0], [1.2, 6.0]],
        &[[1.2, 2.4], [3.0, 2.4], [3.0, 3.4], [1.2, 3.4]],
        &[[3.2, 2.4], [4.0, 2.4], [4.0, 5.2], [3.2, 6.0]],
        &[[3.0, 0.0], [4.0, 0.0], [2.6, 2.2], [1.6, 2.2]],
    ],
);

/// The gap between letters, and a space's advance, in cells.
const KERN: f32 = 1.2;
const SPACE: f32 = 3.0;

fn glyph(c: char) -> Option<Glyph> {
    Some(match c {
        'A' => A,
        'C' => C,
        'D' => D,
        'I' => I,
        'M' => M,
        'N' => N,
        'O' => O,
        'R' => R,
        _ => return None,
    })
}

/// The length of `text` `height` tall, in metres.
pub(super) fn length(text: &str, height: f32) -> f32 {
    let cells: f32 = text
        .chars()
        .map(|c| glyph(c).map_or(SPACE, |g| g.0 + KERN))
        .sum();
    (cells - KERN).max(0.0) * height / 6.0
}

/// `text` painted `height` tall on both flanks, centred on x `centre` with its foot at
/// `foot`, on a flat wall whose face stands at |y| `wall` at height `wall_z` and leans in
/// `lean` metres per metre up. Letters with no glyph are spaces.
pub(super) fn paint_flanks(
    b: &mut MeshBuilder,
    text: &str,
    centre: f32,
    [wall, wall_z, lean]: [f32; 3],
    foot: f32,
    height: f32,
) {
    let cell = height / 6.0;
    let half = length(text, height) * 0.5;
    let y = wall - (foot - wall_z) * lean + 0.06;
    // Port: u runs aft, v up the wall, out to +y. Starboard: u runs forward, out to -y.
    let frame = |u: Vec3, s: f32, at: Vec3| {
        let v = v3(0.0, -s * lean, 1.0).normalize();
        Affine3A::from_cols(u.into(), v.into(), u.cross(v).into(), at.into())
    };
    let port = frame(-Vec3::X, 1.0, v3(centre + half, y, foot));
    let starboard = frame(Vec3::X, -1.0, v3(centre - half, -y, foot));
    for frame in [port, starboard] {
        b.with(frame, |b| {
            let mut u = 0.0;
            for c in text.chars() {
                let Some((advance, pieces)) = glyph(c) else {
                    u += SPACE * cell;
                    continue;
                };
                for piece in pieces {
                    let outline: Vec<Vec3> = piece
                        .iter()
                        .map(|p| v3(u + p[0] * cell, p[1] * cell, 0.0))
                        .collect();
                    b.face(&outline);
                }
                u += (advance + KERN) * cell;
            }
        });
    }
}
