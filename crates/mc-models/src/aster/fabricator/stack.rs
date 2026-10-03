//! Design C (`fabricator~c`), the stack: a press of dark plates on a square footing, a
//! sheet of hot matter forming in each gap between them, four corner posts leaning in
//! to hold the press plate down on top; heat sinks on the corners.
//! - Tech 2: the stack to the press plate.
//! - Tech 3: a narrower stack pressed on top under a second press plate, the posts
//!   carried up to it, capacitor bastions on the grid sides (±x).

use std::f32::consts::FRAC_PI_4;

use glam::Vec3;

use super::super::parts::*;
use super::*;

/// The footing: half width, corner cut, top.
const FOOT: (f32, f32, f32) = (7.6, 2.0, 2.4);
/// A stack: its foot, half width at foot and head, how many plates, the press plate's
/// top. A plate and its sheet are `PITCH` deep.
struct Stack {
    z: f32,
    half: (f32, f32),
    plates: usize,
    press: f32,
}
const PITCH: f32 = 1.4;
const SHEET: f32 = 0.36;
const STACKS: [Stack; 2] = [
    Stack {
        z: FOOT.2,
        half: (7.0, 5.8),
        plates: 7,
        press: 13.8,
    },
    Stack {
        z: 13.8,
        half: (4.6, 3.8),
        plates: 4,
        press: 20.6,
    },
];
const CUT: f32 = 1.6;

pub(in crate::aster) fn build(b: &mut MeshBuilder, tech: u8) {
    plinth(b);
    corner_sinks(b, 10.0, 13.6, 2.4);
    footing(b);
    stack(b, &STACKS[0]);
    boss(b, STACKS[0].press, 1.8, 16.0);

    tech_3(b, tech, 0.2, |b| {
        b.radial(2, |b| bastion(b, v3(9.9, 0.0, DECK), 6.0, 1.8))
    });
    tech_3(b, tech, 0.55, |b| {
        stack(b, &STACKS[1]);
        boss(b, STACKS[1].press, 1.6, 22.0);
    });
}

/// The footing, hot vents on its top along the axes.
fn footing(b: &mut MeshBuilder) {
    let (h, c, top) = FOOT;
    if b.coarse() {
        return;
    }
    let plan = chamfered_rect(v2(h, h), c);
    b.paint(ACCENT);
    b.loft_z(
        &plan,
        &[Section::new(DECK, 1.0), Section::new(DECK + 0.5, 1.0)],
    );
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[Section::new(DECK + 0.5, 0.99), Section::new(top, 0.97)],
    );
}

/// A stack: plates and the hot sheets between them, a press plate on top, a post on each
/// corner leaning in with the taper.
fn stack(b: &mut MeshBuilder, s: &Stack) {
    let (h0, h1) = s.half;
    let body = s.z + PITCH * s.plates as f32;
    let half = |z: f32| h0 + (h1 - h0) * ((z - s.z) / (s.press - s.z)).clamp(0.0, 1.0);
    if b.coarse() {
        b.paint(ACCENT);
        b.frustum_open(
            v3(0.0, 0.0, if s.z < 3.0 { DECK } else { s.z }),
            Vec2::splat(h0 * 2.0),
            Vec2::splat(h1 * 2.0),
            s.press - if s.z < 3.0 { DECK } else { s.z },
            Vec2::ZERO,
        );
        team_panel(b, v3(0.0, 0.0, s.press), v2(2.0, 2.0));
        return;
    }
    let plan = chamfered_rect(Vec2::ONE, CUT / h0);
    // The sheets first, one solid behind all the plates: what shows between them glows.
    b.paint(GLOW_ORANGE).pattern(pattern::HEAT);
    b.loft_z(
        &plan,
        &[
            Section::new(s.z, half(s.z) - 0.35),
            Section::new(body, half(body) - 0.35),
        ],
    );
    for k in 0..s.plates {
        let z0 = s.z + PITCH * k as f32 + SHEET;
        let z1 = s.z + PITCH * (k + 1) as f32;
        b.paint(ACCENT);
        b.loft_z(
            &plan,
            &[
                Section::new(z0, half(z0) * 0.985),
                Section::new(z0 + 0.12, half(z0)),
                Section::new(z1 - 0.12, half(z1)),
                Section::new(z1, half(z1) * 0.985),
            ],
        );
    }
    // The press plate.
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[
            Section::new(body, half(body) + 0.3),
            Section::new(s.press - 0.3, half(s.press) + 0.3),
            Section::new(s.press, half(s.press)),
        ],
    );
    // The posts, on the corners.
    let corner = |z: f32| (half(z) * 2.0 - CUT) * std::f32::consts::FRAC_1_SQRT_2 + 0.75;
    b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
        b.radial(4, |b| {
            let (a, c) = (v3(corner(s.z), 0.0, s.z), v3(corner(s.press), 0.0, s.press));
            b.paint(PLATING);
            b.beam(a, c, v2(1.8, 1.2), v2(1.5, 1.0));
            b.paint(ACCENT);
            b.beam(
                c + Vec3::Z * 0.3,
                c - v3(1.2, 0.0, 0.0) + Vec3::Z * 0.3,
                v2(1.9, 0.8),
                v2(1.6, 0.8),
            );
            if b.fine() {
                for t in [0.3f32, 0.65] {
                    let p = a.lerp(c, t);
                    b.beam(
                        p - Vec3::Z * 0.3,
                        p + Vec3::Z * 0.3,
                        v2(2.1, 1.5),
                        v2(2.1, 1.5),
                    );
                }
            }
        })
    });
}

/// The press head on a press plate at `z`: a dark square block `half` across, a boss on
/// it up to `top`.
fn boss(b: &mut MeshBuilder, z: f32, half: f32, top: f32) {
    if b.coarse() {
        return;
    }
    let mid = z + (top - z) * 0.55;
    b.paint(ACCENT);
    b.frustum(
        v3(0.0, 0.0, z),
        Vec2::splat(half * 2.0),
        Vec2::splat(half * 1.6),
        mid - z,
        Vec2::ZERO,
    );
    b.paint(METAL);
    b.prism(
        v3(0.0, 0.0, mid),
        b.sides(8),
        half * 0.5,
        half * 0.3,
        top - mid - 0.25,
    );
    b.paint(GLOW_ORANGE);
    b.prism(
        v3(0.0, 0.0, top - 0.25),
        b.sides(8),
        half * 0.3,
        half * 0.2,
        0.25,
    );
    if b.fine() {
        b.mirror_y(|b| team_panel(b, v3(0.0, half + 1.4, z), v2(1.6, 0.8)));
    }
}
