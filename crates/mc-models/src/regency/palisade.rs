//! Palisade: the Regency's wall section, joining the sections beside it as the ARC
//! Rampart does (`crate::wall`, `gpu_consts::wall`), in Regency armour:
//! - `CAP`: a quarter of an octagonal tower, a glacis plate on each face, a corner
//!   plate lapped up past the top into a spike, a red slot in the bronze waist under it;
//! - `RUN_A`, `RUN_B`: a battered wall of dark plate on a bronze waist, its face
//!   hung with plates lapped down into spikes over the waist, a bronze ram in the gap
//!   between them, a ribbed bronze rail along the crest;
//! - `JOIN`: the inside of a turn, the wall mitred;
//! - `FULL`: a quarter of a block raised to the towers' height, decked in plate.
//!
//! A lone section is an armoured octagon with a spike at each corner; a line is a
//! plated wall with a half tower at each end and a tower's corner outside every turn.

use glam::Vec3;

use super::kit::{dark_plate, metal, v3};
use super::machine::{armour, piston, red_slot, ribbed, Course, Frame};
use crate::builder::MeshBuilder;
use crate::gpu_consts::wall;
use crate::material::*;
use crate::pattern;
use crate::wall::{diagonal, face_out, pieces, sides, Profile, EDGE};

/// A step up from the wall's top to a block's: from a little under it, so no seam shows.
const STEP_FOOT: f32 = 4.0;
/// Cut off a tower's outer corner: from above it is an octagon.
const CHAMFER: f32 = 1.8;
/// Plates hung on the wall's face, by the middle of each along the wall, and half
/// their width. A ram stands in the gap between them.
const HUNG: [f32; 2] = [1.5, 4.5];
const HUNG_HALF: f32 = 1.3;
const RAM_AT: f32 = 3.0;
/// The crest rail: how far out from the middle, and its height.
const RAIL: (f32, f32) = (0.85, 4.5);
const THICK: f32 = 0.25;

/// The wall: a dark footing, a bronze waist, dark armour battered up to a narrow crest.
fn run(b: &MeshBuilder) -> Profile {
    if b.fine() {
        Profile {
            points: &[
                [3.0, 0.0],
                [3.0, 0.8],
                [2.55, 0.85],
                [2.55, 1.6],
                [2.85, 1.7],
                [1.7, 4.1],
                [1.4, 4.4],
            ],
            paint: &[
                ACCENT,
                ACCENT,
                METAL,
                PLATING_DARK,
                PLATING_DARK,
                PLATING_DARK,
            ],
            pattern: pattern::EMBER,
        }
    } else if b.mid() {
        Profile {
            points: &[[3.0, 0.0], [3.0, 0.8], [2.6, 1.0], [1.7, 4.1], [1.4, 4.4]],
            paint: &[ACCENT, METAL, PLATING_DARK, PLATING_DARK],
            pattern: pattern::EMBER,
        }
    } else {
        Profile {
            points: &[[3.0, 0.0], [1.5, 4.4]],
            paint: &[PLATING_DARK],
            pattern: pattern::EMBER,
        }
    }
}

/// The towers: the wall's profile, broader and 1.4 m taller.
fn tower(b: &MeshBuilder) -> Profile {
    if b.fine() {
        Profile {
            points: &[
                [4.6, 0.0],
                [4.6, 0.9],
                [4.1, 0.95],
                [4.1, 1.8],
                [4.4, 1.9],
                [3.3, 5.3],
                [2.9, 5.8],
            ],
            paint: &[
                ACCENT,
                ACCENT,
                METAL,
                PLATING_DARK,
                PLATING_DARK,
                PLATING_DARK,
            ],
            pattern: pattern::EMBER,
        }
    } else if b.mid() {
        Profile {
            points: &[[4.6, 0.0], [4.6, 0.9], [4.2, 1.1], [3.3, 5.3], [2.9, 5.8]],
            paint: &[ACCENT, METAL, PLATING_DARK, PLATING_DARK],
            pattern: pattern::EMBER,
        }
    } else {
        Profile {
            points: &[[4.6, 0.0], [3.0, 5.8]],
            paint: &[PLATING_DARK],
            pattern: pattern::EMBER,
        }
    }
}

/// A block's raised middle, where it steps up from the wall round it: sheer, with a
/// bronze band at the top. `w` is unused: the step stands on the quarter's edge.
fn step(b: &MeshBuilder) -> Profile {
    if b.coarse() {
        Profile {
            points: &[[0.0, STEP_FOOT], [0.0, 5.8]],
            paint: &[PLATING_DARK],
            pattern: pattern::EMBER,
        }
    } else {
        Profile {
            points: &[[0.0, STEP_FOOT], [0.0, 5.45], [0.0, 5.8]],
            paint: &[PLATING_DARK, METAL],
            pattern: pattern::EMBER,
        }
    }
}

pub(super) fn palisade(b: &mut MeshBuilder, _tech: u8) {
    pieces(b, |b, case| match case {
        wall::CAP => cap(b),
        wall::RUN_A => run_a(b),
        wall::RUN_B => b.with(diagonal(), run_a),
        wall::JOIN => join(b),
        _ => full(b),
    });
}

/// The wall running out along +x to the lot's edge, its face toward +y.
fn run_a(b: &mut MeshBuilder) {
    let profile = run(b);
    sides(
        b,
        &profile,
        |w| vec![v3(EDGE, w, 0.0), v3(0.0, w, 0.0)],
        Vec3::Y,
    );
    let [w, top] = profile.top();
    deck(
        b,
        vec![
            v3(0.0, 0.0, top),
            v3(EDGE, 0.0, top),
            v3(EDGE, w, top),
            v3(0.0, w, top),
        ],
    );
    team_strip(b, v3(0.0, 0.0, top), v3(EDGE, 0.25, top));
    for x in HUNG {
        hung_plate(b, x);
    }
    if b.fine() {
        ram(b, RAM_AT);
        metal(b);
        ribbed(
            b,
            v3(0.0, RAIL.0, RAIL.1),
            v3(EDGE, RAIL.0, RAIL.1),
            0.16,
            2,
        );
    }
}

/// The inside of a turn: the wall along +x and along +y, meeting in a mitre.
fn join(b: &mut MeshBuilder) {
    let profile = run(b);
    sides(
        b,
        &profile,
        |w| vec![v3(EDGE, w, 0.0), v3(w, w, 0.0), v3(w, EDGE, 0.0)],
        Vec3::new(1.0, 1.0, 0.0),
    );
    let [w, top] = profile.top();
    deck(
        b,
        vec![
            v3(0.0, 0.0, top),
            v3(EDGE, 0.0, top),
            v3(EDGE, w, top),
            v3(0.0, w, top),
        ],
    );
    deck(
        b,
        vec![
            v3(0.0, w, top),
            v3(w, w, top),
            v3(w, EDGE, top),
            v3(0.0, EDGE, top),
        ],
    );
    team_strip(b, v3(0.0, 0.0, top), v3(EDGE, 0.25, top));
    team_strip(b, v3(0.0, 0.25, top), v3(0.25, EDGE, top));
    // The plates nearer the turn would cross the mitre; the outer ones hang clear.
    hung_plate(b, HUNG[1]);
    b.with(diagonal(), |b| hung_plate(b, HUNG[1]));
    if b.fine() {
        ram(b, RAM_AT);
        b.with(diagonal(), |b| ram(b, RAM_AT));
        let (r, z) = RAIL;
        metal(b);
        ribbed(b, v3(EDGE, r, z), v3(r, r, z), 0.16, 1);
        ribbed(b, v3(r, r, z), v3(r, EDGE, z), 0.16, 1);
    }
}

/// A quarter of an octagonal tower. Its inner faces are drawn too: beside a wall
/// running on, the tower stands out past the wall and above it.
fn cap(b: &mut MeshBuilder) {
    let profile = tower(b);
    let c = if b.coarse() { 0.0 } else { CHAMFER };
    let outline = |w: f32| {
        if c > 0.0 {
            vec![
                v3(w, 0.0, 0.0),
                v3(w, w - c, 0.0),
                v3(w - c, w, 0.0),
                v3(0.0, w, 0.0),
            ]
        } else {
            vec![v3(w, 0.0, 0.0), v3(w, w, 0.0), v3(0.0, w, 0.0)]
        }
    };
    sides(b, &profile, outline, Vec3::new(1.0, 1.0, 0.0));
    // The faces toward the section's middle, each a band at a time.
    for (band, pair) in profile.points.windows(2).enumerate() {
        let ([w0, z0], [w1, z1]) = (pair[0], pair[1]);
        b.paint(profile.paint[band]).pattern(profile.pattern);
        for (along, out) in [(Vec3::Y, -Vec3::X), (Vec3::X, -Vec3::Y)] {
            let at = |w: f32, z: f32| along * w + Vec3::Z * z;
            face_out(
                b,
                vec![at(0.0, z0), at(w0, z0), at(w1, z1), at(0.0, z1)],
                out,
            );
        }
    }
    let [w, top] = profile.top();
    let mut plan = outline(w);
    plan.insert(0, Vec3::ZERO);
    deck(b, plan.iter().map(|p| *p + Vec3::Z * top).collect());
    b.paint(TEAM);
    face_out(
        b,
        vec![
            v3(0.0, 0.0, top + 0.03),
            v3(1.0, 0.0, top + 0.03),
            v3(1.0, 1.0, top + 0.03),
            v3(0.0, 1.0, top + 0.03),
        ],
        Vec3::Z,
    );
    if b.coarse() {
        return;
    }
    // A glacis plate on each outer face, from beside the middle to the chamfer.
    glacis(b);
    b.with(diagonal(), glacis);
    // The corner: a plate up the chamfer, lapped on past the top into a spike, and a
    // red slot in the waist under it.
    let d = Vec3::new(1.0, 1.0, 0.0).normalize();
    // The chamfer's distance out from the middle, at the foot of the armour and at its
    // top: the plate lies on the face between.
    let out = |w: f32| (2.0 * w - CHAMFER) * std::f32::consts::FRAC_1_SQRT_2;
    let (foot, head) = (d * out(4.4) + Vec3::Z * 1.9, d * out(3.3) + Vec3::Z * 5.3);
    let up = head - foot;
    dark_plate(b);
    Course {
        count: 1,
        step: 0.0,
        len: up.length(),
        half: 0.8,
        tip: 0.0,
        thick: THICK,
        tail: if b.fine() { 1.0 } else { 0.6 },
    }
    .lay(b, &Frame::new(foot, up, d * up.z - Vec3::Z * up.dot(d)));
    if b.fine() {
        let across = Vec3::new(-1.0, 1.0, 0.0);
        red_slot(b, d * out(4.1) + Vec3::Z * 1.4, d, across, 1.0, 0.22);
    }
}

/// A filled quarter of a block, at the towers' height. Its sides toward the next
/// quarters are the step up from the wall; toward the next sections it is filled too.
fn full(b: &mut MeshBuilder) {
    let profile = step(b);
    for (band, pair) in profile.points.windows(2).enumerate() {
        let (z0, z1) = (pair[0][1], pair[1][1]);
        b.paint(profile.paint[band]).pattern(profile.pattern);
        for (along, out) in [(Vec3::Y, -Vec3::X), (Vec3::X, -Vec3::Y)] {
            let at = |s: f32, z: f32| along * s + Vec3::Z * z;
            face_out(
                b,
                vec![at(0.0, z0), at(EDGE, z0), at(EDGE, z1), at(0.0, z1)],
                out,
            );
        }
    }
    let top = profile.top()[1];
    deck(
        b,
        vec![
            v3(0.0, 0.0, top),
            v3(EDGE, 0.0, top),
            v3(EDGE, EDGE, top),
            v3(0.0, EDGE, top),
        ],
    );
    if b.coarse() {
        return;
    }
    // Deck plates, one a quarter, swept toward the block's corner: a block is
    // shingled in armour from above.
    let d = Vec3::new(1.0, 1.0, 0.0).normalize();
    dark_plate(b);
    armour(
        b,
        &Frame::new(v3(1.5, 1.5, top), d, Vec3::Z),
        &[[0.0, -1.2], [0.0, 1.2], [3.6, 1.2], [5.0, 0.0], [3.6, -1.2]],
        0.12,
    );
}

/// The flat top of a piece, dark plate.
fn deck(b: &mut MeshBuilder, points: Vec<Vec3>) {
    dark_plate(b);
    face_out(b, points, Vec3::Z);
}

/// A strip of team colour along the crest, from corner `a` to corner `c` (on the deck).
fn team_strip(b: &mut MeshBuilder, a: Vec3, c: Vec3) {
    let z = a.z + 0.03;
    b.paint(TEAM);
    face_out(
        b,
        vec![
            v3(a.x, a.y, z),
            v3(c.x, a.y, z),
            v3(c.x, c.y, z),
            v3(a.x, c.y, z),
        ],
        Vec3::Z,
    );
}

/// Where the wall's armoured face is at height `z`: (y, z), from the waist's ledge
/// (2.85, 1.7) up to (1.7, 4.1) (`run`).
fn face(z: f32) -> Vec3 {
    let w = 2.85 + (1.7 - 2.85) * (z - 1.7) / (4.1 - 1.7);
    v3(0.0, w, z)
}

/// A plate hung on the wall's face at `x` along it, from under the crest down over
/// the waist into a spike.
fn hung_plate(b: &mut MeshBuilder, x: f32) {
    if b.coarse() {
        return;
    }
    let (head, foot) = (face(3.95), face(1.7));
    let down = foot - head;
    let normal = Vec3::new(0.0, -down.z, down.y);
    dark_plate(b);
    let plate = Frame::new(head + Vec3::X * x, down, normal);
    let tail = if b.fine() { 0.55 } else { 0.35 };
    Course {
        count: 1,
        step: 0.0,
        len: down.length() - 0.15,
        half: HUNG_HALF,
        tip: 0.0,
        thick: THICK,
        tail,
    }
    .lay(b, &plate);
}

/// A bronze ram up the wall's face at `x`, in the gap between two hung plates.
fn ram(b: &mut MeshBuilder, x: f32) {
    let lift = Vec3::Y * 0.2;
    piston(
        b,
        v3(x, 2.55, 0.9) + lift,
        face(3.9) + Vec3::X * x + lift,
        0.2,
        false,
    );
}

/// A glacis plate on the tower's face at +x, beside the middle up to the chamfer.
fn glacis(b: &mut MeshBuilder) {
    let (foot, head) = (v3(4.4, 0.0, 1.9), v3(3.3, 0.0, 5.3));
    let up = head - foot;
    let normal = Vec3::new(up.z, 0.0, -up.x);
    let f = Frame::new(foot, up, normal);
    // Across the face (`v`) runs toward -y here: the plate keeps 0.3 m off the middle
    // and off the chamfer, which comes in as the face does.
    let len = up.length();
    dark_plate(b);
    armour(
        b,
        &f,
        &[
            [0.0, -0.3],
            [len, -0.3],
            [len, -(3.3 - CHAMFER - 0.3)],
            [0.0, -(4.4 - CHAMFER - 0.3)],
        ],
        THICK,
    );
}

#[cfg(test)]
mod tests {
    use crate::build_model;
    use crate::gpu_consts::wall;
    use crate::part;

    /// Every quarter carries all five pieces, and they meet as the Rampart's do: the
    /// towers stand over the wall and are broader, a block is tower high, and a run
    /// reaches the lot's edge.
    #[test]
    fn pieces_meet() {
        let model = build_model("regency_palisade").unwrap();
        let mesh = &model.lods[0];
        for piece in part::WALL_FIRST..part::WALL_FIRST + part::WALL_COUNT {
            assert!(
                mesh.vertices.iter().any(|v| v.part == piece),
                "piece {piece} has a mesh"
            );
        }
        let max = |piece: u32, axis: usize| {
            mesh.vertices
                .iter()
                .filter(|v| v.part == part::WALL_FIRST + piece)
                .map(|v| v.pos[axis])
                .fold(0.0f32, f32::max)
        };
        assert!(max(wall::CAP, 2) > max(wall::RUN_A, 2) + 1.0);
        assert!((max(wall::FULL, 2) - 5.8).abs() < 0.2);
        assert!(max(wall::CAP, 1) > max(wall::RUN_A, 1) + 1.0);
        assert!((max(wall::RUN_A, 0) - 6.0).abs() < 1e-3);
        assert!((max(wall::FULL, 0) - 6.0).abs() < 1e-3);
    }
}
