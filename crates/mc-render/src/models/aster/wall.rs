//! Rampart: a wall section that joins the sections beside it.
//!
//! A section is one 12 m cell. Each quarter of it, from the section's centre out to
//! one corner of its lot, carries every piece that quarter could need, and the entity
//! shader draws the one its neighbours call for (`gpu_consts::wall`, `models::wall`):
//! - `CAP`: a quarter of a pillar, where the wall ends or turns outward;
//! - `RUN_A`, `RUN_B`: the wall running on to the next section on one side;
//! - `JOIN`: the inside of a turn;
//! - `FULL`: a quarter filled in where a block of sections stands, raised to the
//!   pillars' height, so a block is one thick rampart with a ledge round its foot.
//!
//! A lone section is a square pillar; a line is a battered wall with a half pillar at
//! each end and a pillar's corner outside every turn.
//!
//! Every piece is authored in the quarter between +x and +y and turned round for the
//! other three. A piece draws only its outside faces: where it meets the next quarter
//! or the next section, that one's own piece carries on.

use glam::{Affine3A, Vec3};

use super::parts::*;
use crate::gpu_consts::wall;
use crate::models::builder::MeshBuilder;
use crate::models::material::*;
use crate::models::part;

/// Half the lot: every piece runs from the section's centre out to its edge.
const EDGE: f32 = 6.0;
/// A step up from the wall's top to a block's: from a little under it, so no seam shows.
const STEP_FOOT: f32 = 3.9;
/// Cut off a pillar's outer corner.
const CHAMFER: f32 = 1.5;
/// Buttress ribs down the wall's face, a rib every 3 m along the wall.
const RIBS: [f32; 2] = [1.5, 4.5];

/// A side profile, bottom to top: how far the face stands out from the wall's middle
/// (`[w, z]`), and the paint of each band from one point to the next. The top is flat
/// at the last point.
struct Profile {
    points: &'static [[f32; 2]],
    paint: &'static [u32],
}

impl Profile {
    fn top(&self) -> [f32; 2] {
        self.points[self.points.len() - 1]
    }
}

/// The wall: a dark plinth, a battered white face, a dark coping over it.
fn run(b: &MeshBuilder) -> Profile {
    if b.fine() {
        Profile {
            points: &[
                [3.1, 0.0],
                [3.1, 0.9],
                [2.85, 1.1],
                [2.05, 3.7],
                [2.35, 3.8],
                [2.35, 4.3],
            ],
            paint: &[ACCENT, ACCENT, PLATING, ACCENT, ACCENT],
        }
    } else if b.mid() {
        Profile {
            points: &[[3.1, 0.0], [3.1, 0.9], [2.1, 3.8], [2.1, 4.3]],
            paint: &[ACCENT, PLATING, ACCENT],
        }
    } else {
        Profile {
            points: &[[3.1, 0.0], [2.2, 4.3]],
            paint: &[PLATING],
        }
    }
}

/// The pillars: the wall's profile, broader and a metre taller.
fn pillar(b: &MeshBuilder) -> Profile {
    if b.fine() {
        Profile {
            points: &[
                [5.0, 0.0],
                [5.0, 0.9],
                [4.75, 1.1],
                [4.15, 4.8],
                [4.45, 4.9],
                [4.45, 5.4],
            ],
            paint: &[ACCENT, ACCENT, PLATING, ACCENT, ACCENT],
        }
    } else if b.mid() {
        Profile {
            points: &[[5.0, 0.0], [5.0, 0.9], [4.2, 4.9], [4.2, 5.4]],
            paint: &[ACCENT, PLATING, ACCENT],
        }
    } else {
        Profile {
            points: &[[5.0, 0.0], [4.3, 5.4]],
            paint: &[PLATING],
        }
    }
}

/// A block's raised middle, where it steps up from the wall round it: sheer, with the
/// coping's dark band at the top. `w` is unused: the step stands on the quarter's edge.
fn step(b: &MeshBuilder) -> Profile {
    if b.coarse() {
        Profile {
            points: &[[0.0, STEP_FOOT], [0.0, 5.4]],
            paint: &[PLATING],
        }
    } else {
        Profile {
            points: &[[0.0, STEP_FOOT], [0.0, 4.9], [0.0, 5.4]],
            paint: &[PLATING, ACCENT],
        }
    }
}

pub(super) fn wall(b: &mut MeshBuilder, _tech: u8) {
    for q in 0..4 {
        b.with(quarter(q), |b| {
            for case in 0..wall::CASES {
                b.with_part(part::WALL_FIRST + q * wall::CASES + case, |b| match case {
                    wall::CAP => cap(b),
                    wall::RUN_A => run_a(b),
                    wall::RUN_B => b.with(diagonal(), run_a),
                    wall::JOIN => join(b),
                    _ => full(b),
                });
            }
        });
    }
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
    b.paint(PLATING);
    face_out(
        b,
        vec![
            v3(0.0, 0.0, top),
            v3(EDGE, 0.0, top),
            v3(EDGE, w, top),
            v3(0.0, w, top),
        ],
        Vec3::Z,
    );
    // A team stripe down the middle of the top, half of it this side.
    team_panel(b, v3(EDGE * 0.5, 0.2, top), v2(EDGE, 0.4));
    if b.fine() {
        for x in RIBS {
            rib(b, x);
        }
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
    b.paint(PLATING);
    face_out(
        b,
        vec![
            v3(0.0, 0.0, top),
            v3(EDGE, 0.0, top),
            v3(EDGE, w, top),
            v3(0.0, w, top),
        ],
        Vec3::Z,
    );
    face_out(
        b,
        vec![
            v3(0.0, w, top),
            v3(w, w, top),
            v3(w, EDGE, top),
            v3(0.0, EDGE, top),
        ],
        Vec3::Z,
    );
    team_panel(b, v3(EDGE * 0.5, 0.2, top), v2(EDGE, 0.4));
    team_panel(b, v3(0.2, (EDGE + 0.4) * 0.5, top), v2(0.4, EDGE - 0.4));
    if b.fine() {
        rib(b, RIBS[1]);
        b.with(diagonal(), |b| rib(b, RIBS[1]));
    }
}

/// A quarter of a square pillar, its outer corner cut. Its inner faces are drawn too:
/// beside a wall running on, the pillar stands out past the wall and above it.
fn cap(b: &mut MeshBuilder) {
    let profile = pillar(b);
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
        b.paint(profile.paint[band]);
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
    b.paint(PLATING);
    let mut deck = outline(w);
    deck.insert(0, Vec3::ZERO);
    face_out(
        b,
        deck.iter().map(|p| *p + Vec3::Z * top).collect(),
        Vec3::Z,
    );
    team_panel(b, v3(2.1, 2.1, top), v2(2.2, 2.2));
    if b.fine() {
        // An armour panel on each outer face, following the batter.
        armour(b);
        b.with(diagonal(), armour);
    }
}

/// A filled quarter of a block, at the pillars' height. Its sides toward the next
/// quarters are the step up from the wall; toward the next sections it is filled too.
fn full(b: &mut MeshBuilder) {
    let profile = step(b);
    for (band, pair) in profile.points.windows(2).enumerate() {
        let (z0, z1) = (pair[0][1], pair[1][1]);
        b.paint(profile.paint[band]);
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
    // Dark under the deck plates; far off, where there are none, the deck is plain.
    b.paint(if b.coarse() { PLATING } else { PLATING_DARK });
    face_out(
        b,
        vec![
            v3(0.0, 0.0, top),
            v3(EDGE, 0.0, top),
            v3(EDGE, EDGE, top),
            v3(0.0, EDGE, top),
        ],
        Vec3::Z,
    );
    // Deck plates, one a quarter, raised a little so the seams between them show.
    let tile = v3(EDGE * 0.5, EDGE * 0.5, top);
    b.paint(PLATING);
    if b.fine() {
        b.plate(tile, v2(EDGE - 1.0, EDGE - 1.0), 0.08, 0.04);
    } else if b.mid() {
        b.decal(tile + Vec3::Z * 0.04, v2(EDGE - 1.0, EDGE - 1.0));
    }
}

/// A quarter turn `q` times counter-clockwise, exactly: the pieces meet on the lot's edge.
fn quarter(q: u32) -> Affine3A {
    let (c, s) = [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)][q as usize % 4];
    Affine3A::from_cols(
        Vec3::new(c, s, 0.0).into(),
        Vec3::new(-s, c, 0.0).into(),
        Vec3::Z.into(),
        Vec3::ZERO.into(),
    )
}

/// Mirror across the quarter's diagonal: +x becomes +y and +y becomes +x.
fn diagonal() -> Affine3A {
    Affine3A::from_cols(
        Vec3::Y.into(),
        Vec3::X.into(),
        Vec3::Z.into(),
        Vec3::ZERO.into(),
    )
}

/// The outside faces of a piece: band by band up `profile`, along the plan `outline`
/// the profile's `w` gives. `out` points roughly away from the piece in plan.
fn sides(b: &mut MeshBuilder, profile: &Profile, outline: impl Fn(f32) -> Vec<Vec3>, out: Vec3) {
    for (band, pair) in profile.points.windows(2).enumerate() {
        let ([w0, z0], [w1, z1]) = (pair[0], pair[1]);
        let (low, high) = (outline(w0), outline(w1));
        b.paint(profile.paint[band]);
        for i in 0..low.len() - 1 {
            // This edge's own outward direction in plan, tilted by the band's slope: up
            // for a ledge, down for the underside of the coping.
            let edge = low[i + 1] - low[i];
            let mut side = Vec3::new(edge.y, -edge.x, 0.0).normalize_or_zero();
            if side.dot(out) < 0.0 {
                side = -side;
            }
            let normal = side * (z1 - z0) + Vec3::Z * (w0 - w1);
            face_out(
                b,
                vec![
                    low[i] + Vec3::Z * z0,
                    low[i + 1] + Vec3::Z * z0,
                    high[i + 1] + Vec3::Z * z1,
                    high[i] + Vec3::Z * z1,
                ],
                normal,
            );
        }
    }
}

/// Emits `points` as one flat face, wound so its front faces `out`.
fn face_out(b: &mut MeshBuilder, mut points: Vec<Vec3>, out: Vec3) {
    let mut normal = Vec3::ZERO;
    for (i, p) in points.iter().enumerate() {
        let q = points[(i + 1) % points.len()];
        normal += p.cross(q);
    }
    if normal.dot(out) < 0.0 {
        points.reverse();
    }
    b.face(&points);
}

/// A buttress rib `x` along the wall, up the battered face on +y.
fn rib(b: &mut MeshBuilder, x: f32) {
    let ring = |inner: f32, z: f32| {
        vec![
            v3(x - 0.3, inner, z),
            v3(x + 0.3, inner, z),
            v3(x + 0.3, inner + 0.3, z),
            v3(x - 0.3, inner + 0.3, z),
        ]
    };
    b.paint(PLATING);
    b.loft(&[ring(2.9, 0.9), ring(2.0, 3.7)], false, true);
}

/// An armour panel on the pillar's face at +x, from beside the middle to the chamfer.
fn armour(b: &mut MeshBuilder) {
    // The pillar's battered band: (4.75, 1.1) up to (4.15, 4.8).
    let face = |z: f32| 4.75 + (4.15 - 4.75) * (z - 1.1) / (4.8 - 1.1);
    // It stops short of the chamfer, which comes in as the face does.
    let ring = |z: f32| {
        let w = face(z);
        let end = w - CHAMFER - 0.35;
        vec![
            v3(w - 0.05, 0.4, z),
            v3(w + 0.15, 0.4, z),
            v3(w + 0.15, end, z),
            v3(w - 0.05, end, z),
        ]
    };
    b.paint(PLATING);
    b.loft(&[ring(1.5), ring(4.4)], true, true);
}
