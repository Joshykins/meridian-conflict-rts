//! The Sunspear's casemate: a faceted armoured box drawn in toward its roof, two courses
//! of swept plates down each flank, a locker forward on each; a ridged glacis under the
//! mantlet, a slot across each front corner, a bay in its back; and on the roof the
//! fusion in a keeled armoured hump, strapped and slotted, vent lids either side of it,
//! an optic block forward and the owner's colour.

use glam::Vec3;

use crate::builder::MeshBuilder;

use super::super::super::commander::form::{ring, sleeve, KEEL};
use super::super::super::kit::{dark_plate, metal, v3};
use super::super::super::machine::{Course, Frame};
use super::super::super::parts::{bay, locker, ridged};
use super::super::{plan_hull, team_patch};
use super::{slot, vents, DECK, ROOF};

/// Half the casemate's plan, front round to back.
const HALF: [[f32; 2]; 5] = [
    [11.0, 3.8],
    [7.0, 9.8],
    [-12.0, 9.8],
    [-16.0, 6.0],
    [-16.0, 0.0],
];
/// The plan's scale up the casemate: straight-sided to `SHOULDER`, then drawn in.
const LEVELS: [(f32, f32); 4] = [
    (DECK + 0.3, 0.96),
    (DECK + 1.2, 1.0),
    (SHOULDER, 0.98),
    (ROOF, 0.74),
];
const SHOULDER: f32 = 17.4;

/// The plan's scale at height `z`.
fn scale(z: f32) -> f32 {
    let i = LEVELS
        .iter()
        .rposition(|l| l.0 <= z)
        .unwrap_or(0)
        .min(LEVELS.len() - 2);
    let (a, c) = (LEVELS[i], LEVELS[i + 1]);
    a.1 + (c.1 - a.1) * ((z - a.0) / (c.0 - a.0)).clamp(0.0, 1.0)
}

pub(super) fn casemate(b: &mut MeshBuilder) {
    plan_hull(b, &HALF, &LEVELS);
    b.mirror_y(flank);
    front(b);
    roof(b);
    if b.fine() {
        let k = scale(12.5);
        bay(
            b,
            &Frame::new(v3(-16.0 * k - 0.02, -4.2, 12.5), Vec3::Y, -Vec3::X),
            8.4,
            1.5,
        );
    }
}

/// The +y flank: two courses of swept plates laid back along it, a locker forward.
fn flank(b: &mut MeshBuilder) {
    let rows: &[f32] = if b.fine() { &[10.6, 14.6] } else { &[14.6] };
    dark_plate(b);
    for &z in rows {
        Course {
            count: 4,
            step: 4.4,
            len: 5.4,
            half: 1.5,
            tip: 0.4,
            thick: 0.3,
            tail: 0.6,
        }
        .lay(
            b,
            &Frame::new(v3(6.2, 9.8 * scale(z) + 0.02, z), -Vec3::X, Vec3::Y),
        );
    }
    if b.fine() {
        // Ridged plates up the slope over the shoulder.
        let (k0, k1) = (scale(SHOULDER), scale(ROOF));
        let (y0, y1) = (9.8 * k0, 9.8 * k1);
        let (zm, ym) = ((SHOULDER + ROOF) * 0.5, (y0 + y1) * 0.5);
        let out = v3(0.0, ROOF - SHOULDER, y0 - y1).normalize();
        let up = v3(0.0, y1 - y0, ROOF - SHOULDER).length();
        for (x0, x1) in [(4.4f32, -1.6f32), (-2.4, -8.4)] {
            ridged(
                b,
                &Frame::new(v3(x0, ym + 0.02, zm), -Vec3::X, out),
                &[
                    [0.0, -up * 0.4],
                    [x0 - x1, -up * 0.4],
                    [x0 - x1 - 0.8, up * 0.36],
                    [0.6, up * 0.36],
                ],
                0.3,
                [(x0 - x1) * 0.5, 0.0],
                0.25,
            );
        }
        locker(
            b,
            &Frame::new(v3(1.6, 9.8 * scale(8.6) + 0.02, 8.6), Vec3::X, Vec3::Y),
            v3(3.4, 1.4, 0.6),
        );
    }
}

/// The front: a ridged glacis under the mantlet and a slot across each front corner.
fn front(b: &mut MeshBuilder) {
    let z = 12.0;
    b.mirror_y(|b| {
        let k = scale(z);
        let (a, c) = (
            v3(HALF[0][0] * k, HALF[0][1] * k, z),
            v3(HALF[1][0] * k, HALF[1][1] * k, z),
        );
        let along = c - a;
        let out = v3(along.y, -along.x, 0.0).normalize();
        slot(b, (a + c) * 0.5, out, along, 4.4, 0.4);
    });
    if b.fine() {
        // A ridged plate up each front corner's slope over the shoulder.
        b.mirror_y(|b| {
            let (k0, k1) = (scale(SHOULDER), scale(ROOF));
            let mid = |k: f32| {
                v3(
                    (HALF[0][0] + HALF[1][0]) * 0.5 * k,
                    (HALF[0][1] + HALF[1][1]) * 0.5 * k,
                    0.0,
                )
            };
            let (a, c) = (mid(k0) + Vec3::Z * SHOULDER, mid(k1) + Vec3::Z * ROOF);
            let edge = v3(HALF[1][0] - HALF[0][0], HALF[1][1] - HALF[0][1], 0.0).normalize();
            let rise = c - a;
            let out = edge.cross(rise).normalize();
            let out = if out.x < 0.0 { -out } else { out };
            dark_plate(b);
            ridged(
                b,
                &Frame::new((a + c) * 0.5 + out * 0.02, edge, out),
                &[[-2.6, -2.6], [2.6, -2.6], [2.0, 2.4], [-2.0, 2.4]],
                0.3,
                [0.0, 0.0],
                0.25,
            );
        });
        dark_plate(b);
        ridged(
            b,
            &Frame::new(v3(11.0 * scale(10.5) + 0.02, 0.0, 10.5), Vec3::Y, Vec3::X),
            &[[-3.4, -1.8], [3.4, -1.8], [2.8, 2.2], [-2.8, 2.2]],
            0.35,
            [0.0, 0.2],
            0.3,
        );
    }
}

/// The roof: the fusion hump, its straps and slots, vent lids either side, an optic
/// block forward and the owner's colour.
fn roof(b: &mut MeshBuilder) {
    // The hump's stations: x, half its width, its height.
    let hump: [(f32, f32, f32); 4] = [
        (-11.0, 3.0, 1.4),
        (-8.0, 3.8, 2.4),
        (-1.0, 3.6, 2.2),
        (2.2, 2.4, 1.0),
    ];
    let at = |x: f32, w: f32, h: f32| ring(v3(x, 0.0, ROOF + 0.8 * h), Vec3::Z, w, h);
    dark_plate(b);
    sleeve(b, &hump.map(|(x, w, h)| at(x, w, h)), &KEEL);
    if b.fine() {
        // Graphite straps over it.
        for x in [-7.0f32, -4.5, -2.0] {
            metal(b);
            let (w, h) = (3.8 - (x + 7.0).max(0.0) * 0.04 + 0.12, 2.35 + 0.12);
            sleeve(b, &[at(x, w, h), at(x + 0.5, w, h)], &KEEL);
        }
    }
    // A slot down each of its upper flanks.
    let (w, h) = (3.7, 2.3);
    let d = v3(0.0, -0.45 * w, 0.62 * h);
    let out = v3(0.0, -d.z, d.y).normalize() * -1.0;
    for side in [1.0f32, -1.0] {
        let p = v3(-4.5, side * 0.775 * w, ROOF + 0.8 * h + 0.41 * h);
        slot(
            b,
            p,
            v3(0.0, side * out.y.abs(), out.z.abs()),
            Vec3::X,
            7.0,
            0.4,
        );
    }
    b.mirror_y(|b| vents(b, [-10.0, -2.6], [4.4, 6.6], ROOF + 0.3, 3));
    if b.fine() {
        // The optic block, forward on the right, its red eye forward.
        dark_plate(b);
        b.block(v3(3.4, -6.0, ROOF), v3(5.8, -4.2, ROOF + 1.3));
        slot(b, v3(5.81, -5.1, ROOF + 0.75), Vec3::X, Vec3::Y, 1.3, 0.22);
    }
    team_patch(b, 3.0, 6.2, 1.6, ROOF + 0.01);
}
