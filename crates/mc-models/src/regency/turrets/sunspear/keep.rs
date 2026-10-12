//! The Sunspear's keep: a low chamfered square on a plinth, its walls battered in toward
//! the deck, each face panelled in two ridged plates either side of a slot with conduits
//! running up beside it, a keeled buttress out from each cut corner under a ridged cap,
//! and the graphite race the casemate turns on.

use glam::Vec3;

use crate::builder::{MeshBuilder, Section};

use super::super::super::kit::{cable, dark_plate, metal, seam, v3};
use super::super::super::machine::Frame;
use super::super::super::parts::ridged;
use super::super::buttress;
use super::{slot, DECK};

/// The keep's plan: a square 16 m to a side's middle, its corners cut.
const PLAN: [[f32; 2]; 8] = [
    [16.0, -11.0],
    [16.0, 11.0],
    [11.0, 16.0],
    [-11.0, 16.0],
    [-16.0, 11.0],
    [-16.0, -11.0],
    [-11.0, -16.0],
    [11.0, -16.0],
];
/// The plinth's top, and where the battered wall starts over its seam.
const PLINTH: f32 = 1.4;
const WALL: f32 = 1.7;
/// The wall's scale of the plan at its foot and at the deck.
const FOOT: f32 = 0.98;
const TOP: f32 = 0.86;

pub(super) fn keep(b: &mut MeshBuilder) {
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &PLAN,
            &[Section::new(0.0, 1.08), Section::new(PLINTH, 1.05)],
        );
    });
    seam(b);
    b.loft_z(&PLAN, &[Section::new(PLINTH, 1.0), Section::new(WALL, 1.0)]);
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(&PLAN, &[Section::new(WALL, FOOT), Section::new(DECK, TOP)]);
    });
    for k in 0..4 {
        b.yawed(Vec3::ZERO, (90.0 * k as f32).to_radians(), face);
        b.yawed(Vec3::ZERO, (45.0 + 90.0 * k as f32).to_radians(), corner);
    }
    metal(b);
    let sides = b.sides(20);
    b.prism(Vec3::Z * (DECK - 0.05), sides, 12.6, 12.2, 0.5);
}

/// The wall's face toward +x at height `z`: how far out it is.
fn wall_x(z: f32) -> f32 {
    16.0 * (FOOT + (TOP - FOOT) * (z - WALL) / (DECK - WALL))
}

/// The +x face: a slot across its middle, a ridged plate either side, and a conduit up
/// each side of the slot from the plinth to the deck.
fn face(b: &mut MeshBuilder) {
    let z = (WALL + DECK) * 0.5;
    let rise = v3(wall_x(DECK) - wall_x(WALL), 0.0, DECK - WALL).normalize();
    let out = v3(rise.z, 0.0, -rise.x);
    let mid = v3(wall_x(z), 0.0, z);
    slot(b, mid, out, Vec3::Y, 5.0, 0.4);
    if !b.fine() {
        return;
    }
    let f = Frame::new(mid, Vec3::Y, out);
    dark_plate(b);
    for side in [-1.0f32, 1.0] {
        let (s0, s1) = (side * 3.6, side * 10.4);
        let (lo, hi) = (s0.min(s1), s0.max(s1));
        ridged(
            b,
            &f,
            &[
                [lo, -1.9],
                [hi, -1.9],
                [hi - side.max(0.0) * 0.8, 1.9],
                [lo + (-side).max(0.0) * 0.8, 1.9],
            ],
            0.3,
            [(lo + hi) * 0.5, 0.0],
            0.25,
        );
        metal(b);
        let y = side * 3.0;
        cable(
            b,
            &[
                v3(17.5, y, 0.25),
                v3(17.0, y, PLINTH + 0.25),
                v3(wall_x(WALL) + 0.3, y, WALL + 0.2),
                v3(wall_x(DECK) + 0.3, y, DECK - 0.1),
            ],
            0.28,
        );
    }
}

/// The +x corner's buttress, a ridged cap plate down its back.
fn corner(b: &mut MeshBuilder) {
    let (x0, h0, x1, h1) = (15.0, 6.2, 23.4, 1.6);
    buttress(b, x0, 6.0, h0, x1, 3.0, h1);
    if b.fine() {
        let down = v3(x1 - x0, 0.0, h1 - h0);
        let f = Frame::new(v3(x0 + 0.2, 0.0, h0 - 0.05), down, Vec3::Z);
        dark_plate(b);
        ridged(
            b,
            &f,
            &[
                [0.0, -1.4],
                [down.length() - 0.4, -0.7],
                [down.length() - 0.4, 0.7],
                [0.0, 1.4],
            ],
            0.25,
            [2.4, 0.0],
            0.3,
        );
    }
}
