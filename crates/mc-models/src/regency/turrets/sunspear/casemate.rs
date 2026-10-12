//! The Sunspear's casemate: a low faceted wedge, an arrowhead in plan (a narrow nose,
//! broad shoulders, a tail drawn to a point), its sides drawn in steeply over the
//! shoulder and its top pulled back so the front runs up as a glacis. Either side of the
//! gun a tall cheek stands over the roof, lapped in a plate swept back into a spike. Down
//! each flank a course of swept plates runs back past the tail into fins, ridged plates
//! over it on the slope, a locker forward; a ridged glacis under the mantlet and a slot
//! across each front corner. On the roof the fusion lies in a low keeled spine, strapped
//! and slotted, vent lids either side, the owner's colour forward of it.

use glam::Vec3;

use crate::builder::{MeshBuilder, Section};
use crate::material::TEAM;

use super::super::super::commander::form::{ring, sleeve, KEEL};
use super::super::super::kit::{dark_plate, metal, v3};
use super::super::super::machine::{armour, swept, Course, Frame};
use super::super::super::parts::{locker, ridged};
use super::super::team_patch;
use super::{slot, vents, DECK, ROOF};

/// Half the casemate's plan, nose round to tail.
const HALF: [[f32; 2]; 7] = [
    [12.0, 2.8],
    [9.0, 6.6],
    [2.0, 10.4],
    [-8.0, 10.4],
    [-13.0, 8.0],
    [-17.0, 4.2],
    [-18.5, 0.0],
];
/// The plan up the casemate: height, scale, and how far back it is pulled.
const LEVELS: [(f32, f32, f32); 5] = [
    (DECK + 0.2, 0.94, 0.0),
    (DECK + 1.0, 1.0, 0.0),
    (SHOULDER, 0.97, 0.0),
    (14.4, 0.8, -0.8),
    (ROOF, 0.62, -2.0),
];
/// Where the sides turn in.
const SHOULDER: f32 = 11.2;
/// The cheeks either side of the gun: their inner faces, and their thickness.
const CHEEK: f32 = 3.9;
const CHEEK_THICK: f32 = 1.5;

/// The plan's scale and pull at height `z`.
fn level(z: f32) -> (f32, f32) {
    let i = LEVELS
        .iter()
        .rposition(|l| l.0 <= z)
        .unwrap_or(0)
        .min(LEVELS.len() - 2);
    let (a, c) = (LEVELS[i], LEVELS[i + 1]);
    let t = ((z - a.0) / (c.0 - a.0)).clamp(0.0, 1.0);
    (a.1 + (c.1 - a.1) * t, a.2 + (c.2 - a.2) * t)
}

/// The plan point `p` at height `z`.
fn at(p: [f32; 2], z: f32) -> Vec3 {
    let (k, dx) = level(z);
    v3(p[0] * k + dx, p[1] * k, z)
}

pub(super) fn casemate(b: &mut MeshBuilder) {
    let outline: Vec<[f32; 2]> = HALF
        .iter()
        .copied()
        .chain(
            HALF.iter()
                .rev()
                .filter(|p| p[1] > 0.0)
                .map(|&[x, y]| [x, -y]),
        )
        .collect();
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &outline,
            &LEVELS.map(|(z, k, dx)| Section::new(z, k).shifted(dx, 0.0)),
        );
    });
    b.mirror_y(cheek);
    b.mirror_y(flank);
    front(b);
    roof(b);
}

/// The +y cheek beside the gun: a tall faceted slab standing over the roof, a plate down
/// its outside swept back into a spike with a slot let into it, the owner's colour along
/// its top.
fn cheek(b: &mut MeshBuilder) {
    let (top0, top1) = ([9.4, 17.2], [3.0, 17.6]);
    dark_plate(b);
    b.with_facets(|b| {
        b.extrude_y(
            &[
                [-3.0, DECK + 3.0],
                [11.6, DECK + 3.4],
                [12.6, 13.0],
                top0,
                top1,
                [-3.2, 15.4],
            ],
            CHEEK,
            CHEEK + CHEEK_THICK,
        );
    });
    let y = CHEEK + CHEEK_THICK;
    let f = Frame::new(v3(12.0, y, 14.0), v3(-1.0, 0.0, 0.14), Vec3::Y);
    armour(b, &f, &swept(17.0, 1.7, -0.3, 0.5), 0.35);
    slot(b, f.at(6.0, 0.0, 0.36), Vec3::Y, f.u, 7.0, 0.32);
    b.paint(TEAM);
    let lift = |p: [f32; 2], y: f32| v3(p[0], y, p[1] + 0.01);
    b.face(&[
        lift(top1, CHEEK + 0.2),
        lift(top0, CHEEK + 0.2),
        lift(top0, y - 0.2),
        lift(top1, y - 0.2),
    ]);
}

/// The +y flank: a course of swept plates laid back along it, running past the tail into
/// fins; ridged plates on the slope over the shoulder; a locker forward.
fn flank(b: &mut MeshBuilder) {
    let z = 8.8;
    dark_plate(b);
    Course {
        count: if b.fine() { 4 } else { 2 },
        step: 4.6,
        len: 5.6,
        half: 1.5,
        tip: 0.5,
        thick: 0.3,
        tail: 3.4,
    }
    .lay(
        b,
        &Frame::new(v3(3.4, 10.4 * level(z).0 + 0.02, z), -Vec3::X, Vec3::Y),
    );
    if !b.fine() {
        return;
    }
    // The slope over the shoulder: its middle and its outward face.
    let (lo, hi) = (SHOULDER, 14.4);
    let (p0, p1) = (at([0.0, 10.4], lo), at([0.0, 10.4], hi));
    let out = v3(0.0, hi - lo, p0.y - p1.y).normalize();
    let up = (p1 - p0).with_x(0.0).length();
    let mid = (p0 + p1) * 0.5;
    dark_plate(b);
    for (x0, x1) in [(1.4f32, -3.4f32), (-4.2, -9.0)] {
        ridged(
            b,
            &Frame::new(v3(x0, mid.y + 0.02, mid.z), -Vec3::X, out),
            &[
                [0.0, -up * 0.42],
                [x0 - x1, -up * 0.42],
                [x0 - x1 - 0.7, up * 0.38],
                [0.5, up * 0.38],
            ],
            0.25,
            [(x0 - x1) * 0.5, 0.0],
            0.2,
        );
    }
    locker(
        b,
        &Frame::new(v3(5.0, 10.4 * level(6.9).0 - 0.6, 6.9), Vec3::X, Vec3::Y),
        v3(3.0, 1.1, 0.5),
    );
}

/// The front: a ridged glacis under the mantlet and a slot across each front corner.
fn front(b: &mut MeshBuilder) {
    b.mirror_y(|b| {
        let z = 9.0;
        let (a, c) = (at(HALF[0], z), at(HALF[1], z));
        let along = c - a;
        let out = v3(along.y, -along.x, 0.0).normalize();
        slot(b, (a + c) * 0.5, out, along, 3.4, 0.36);
    });
    if b.fine() {
        dark_plate(b);
        ridged(
            b,
            &Frame::new(at([12.0, 0.0], 8.8) + Vec3::X * 0.02, Vec3::Y, Vec3::X),
            &[[-2.5, -1.5], [2.5, -1.5], [2.1, 1.5], [-2.1, 1.5]],
            0.3,
            [0.0, 0.0],
            0.25,
        );
    }
}

/// The roof: the fusion spine, its straps and slots, vent lids either side and the
/// owner's colour forward of it.
fn roof(b: &mut MeshBuilder) {
    // The spine's stations: x, half its width, its height.
    let spine = [
        (-15.0, 1.2, 0.7),
        (-11.0, 2.0, 1.3),
        (-3.0, 2.0, 1.3),
        (0.2, 1.3, 0.6),
    ];
    let ring_at = |x: f32, w: f32, h: f32| ring(v3(x, 0.0, ROOF + 0.8 * h), Vec3::Z, w, h);
    dark_plate(b);
    sleeve(b, &spine.map(|(x, w, h)| ring_at(x, w, h)), &KEEL);
    if b.fine() {
        for x in [-9.5f32, -7.0, -4.5] {
            metal(b);
            sleeve(
                b,
                &[ring_at(x, 2.1, 1.38), ring_at(x + 0.45, 2.1, 1.38)],
                &KEEL,
            );
        }
    }
    // A slot down each of its upper flanks.
    let (w, h) = (2.0, 1.3);
    let out = v3(0.0, 0.62 * h, 0.45 * w).normalize();
    for side in [1.0f32, -1.0] {
        let p = v3(-7.0, side * 0.775 * w, ROOF + 0.8 * h + 0.41 * h);
        slot(b, p, v3(0.0, side * out.y, out.z), Vec3::X, 6.0, 0.3);
    }
    b.mirror_y(|b| vents(b, [-10.4, -3.2], [2.3, 3.6], ROOF + 0.25, 3));
    team_patch(b, 0.8, 3.6, 1.2, ROOF + 0.01);
}
