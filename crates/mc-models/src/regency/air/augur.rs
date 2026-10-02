//! The Augur, the Regency's tech 3 spy plane: unarmed, it flies far above the cloud deck
//! and faster than anything sent up after it, looking down with a wide eye and radar. A
//! high-altitude reconnaissance jet (the SR-71, the U-2) in the Regency's language: a long
//! nose blade, swept plates lapped down its body, fins turned down, bronze workings between
//! the plates, a long band of red optics where it looks, and red heat in its plasma jets.
//!
//! Authored at the blueprint's size (`regency_t3_spy_plane` in
//! `data/factions/regency/units/air_t3.ron`).

use glam::Vec2;

use crate::builder::MeshBuilder;
use crate::library::ModelDef;
use crate::material::*;

use super::super::kit::{seam, v3};
use super::blade_jet::{
    blade, chevron, feathers, fin, half_width, hull, lap_pair, nozzle, optics, pin_line, st,
    surface, wing, workings, Station,
};

pub(crate) const RADIUS: f32 = 12.0;
pub(crate) const HEIGHT: f32 = 2.8;

pub(crate) const MODELS: &[ModelDef] = &[ModelDef::new("regency_augur", RADIUS, HEIGHT, glider)];

// ---- the glider --------------------------------------------------------------------

/// A slim long body on great straight wings, their tips turned down; a long sensor spine
/// down its back; one jet.
const GLIDER_BODY: [Station; 6] = [
    st(-9.6, 1.1, [1.3, 0.6], [1.6, 0.4], 1.8),
    st(-6.0, 0.85, [1.25, 0.9], [1.75, 0.55], 2.15),
    st(-1.0, 0.75, [1.2, 1.0], [1.8, 0.6], 2.3),
    st(3.0, 0.8, [1.2, 0.85], [1.7, 0.5], 2.1),
    st(6.5, 0.95, [1.25, 0.6], [1.55, 0.32], 1.8),
    st(8.4, 1.1, [1.3, 0.32], [1.42, 0.16], 1.55),
];
const GLIDER_WING: [[f32; 2]; 5] = [
    [1.4, 0.9],
    [-0.6, 12.6],
    [-1.6, 13.2],
    [-2.6, 12.6],
    [-2.6, 0.9],
];
const GLIDER_FEATHERS: [&[[f32; 2]]; 3] = [
    &[
        [1.2, 1.0],
        [0.6, 4.6],
        [-3.4, 4.6],
        [-2.4, 2.6],
        [-2.8, 1.1],
    ],
    &[
        [0.5, 4.8],
        [-0.1, 8.6],
        [-3.2, 8.8],
        [-2.4, 6.6],
        [-2.8, 5.0],
    ],
    &[
        [-0.2, 8.8],
        [-0.7, 12.4],
        [-3.2, 12.8],
        [-2.4, 10.8],
        [-2.8, 9.0],
    ],
];

fn glider(b: &mut MeshBuilder, _tech: u8) {
    let h = &GLIDER_BODY;
    hull(b, h, v3(9.2, 0.0, 1.35), &[0, 3]);
    blade(b, v3(8.8, 0.0, 1.38), v3(12.6, 0.0, 1.3), 0.45, 0.2);
    if b.coarse() {
        b.mirror_y(|b| wing(b, &[[1.4, 0.9], [-1.6, 13.2], [-2.6, 0.9]], 1.6, 0.16));
        chevron(b, v3(-5.0, 0.0, surface(h, -5.0, 0.0) + 0.2), 1.4);
        return;
    }
    b.mirror_y(|b| {
        wing(b, &GLIDER_WING, 1.6, 0.14);
        feathers(b, &GLIDER_FEATHERS, 1.74, 0.08, 0.05);
        fin(b, v3(-0.6, 12.6, 1.62), 1.6, 0.9, 0.35, 0.7, 0.55, 0.12);
        // The tail planes, turned down.
        fin(b, v3(-7.4, 0.85, 1.5), 2.0, 1.2, 0.75, 1.3, 0.5, 0.12);
        if b.fine() {
            for (k, f) in GLIDER_FEATHERS.iter().enumerate() {
                let z = 1.74 + 0.05 * k as f32 + 0.095;
                pin_line(
                    b,
                    v3(f[0][0] - 0.15, f[0][1] + 0.1, z),
                    v3(f[1][0] - 0.15, f[1][1], z),
                );
            }
            workings(
                b,
                v3(0.8, 1.2, 1.85),
                v3(0.0, 12.0, 1.85),
                0.07,
                &[0.3, 0.6],
            );
        }
    });
    nozzle(b, v3(-9.5, 0.0, 1.45), 0.55, 0.9);
    back(b, h);
    // The sensor spine: a long keeled housing down the back, a red eye band along each side.
    let (x0, x1) = (4.6, -4.4);
    let z0 = surface(h, x0, 0.0);
    seam(b);
    b.with_facets(|b| {
        b.beam(
            v3(x1, 0.0, surface(h, x1, 0.0) + 0.25),
            v3(x0, 0.0, z0 + 0.25),
            Vec2::new(0.75, 0.55),
            Vec2::new(0.6, 0.5),
        )
    });
    if b.fine() {
        b.paint(GLOW_LASER);
        b.mirror_y(|b| {
            b.beam(
                v3(x1 + 0.6, 0.39, surface(h, x1, 0.0) + 0.32),
                v3(x0 - 0.4, 0.32, z0 + 0.32),
                Vec2::new(0.04, 0.1),
                Vec2::new(0.04, 0.1),
            )
        });
    }
    eye_band(b, h, 6.4, 3.0);
    optics(b, 7.6, 0.33, 1.5, 0.6);
}

// ---- shared ------------------------------------------------------------------------

/// Plates lapped down the back, the bronze spine between them, the owner's chevron.
fn back(b: &mut MeshBuilder, h: &[Station]) {
    let (tail, front) = (h[0].x, h[h.len() - 2].x);
    let length = front - tail;
    for k in 0..3 {
        let x0 = front - length * (0.12 + 0.28 * k as f32);
        let x1 = x0 - length * 0.3;
        let (w0, w1) = (half_width(h, x0), half_width(h, x1));
        lap_pair(
            b,
            h,
            &[
                [x0, 0.1, 0.04],
                [x0 - 0.2, w0 * 0.7, 0.05],
                [x1 - 1.0, w1 * 1.0 + 0.2, 0.2],
                [x1 + 0.3, w1 * 0.5, 0.17],
                [x1 + 0.1, 0.1, 0.15],
            ],
            0.1,
        );
    }
    if b.fine() {
        let at = |x: f32| v3(x, 0.0, surface(h, x, 0.0) + 0.05);
        workings(
            b,
            at(front - length * 0.1),
            at(tail + length * 0.08),
            0.11,
            &[0.36, 0.66],
        );
    }
    let x = tail + length * 0.25;
    chevron(b, v3(x, 0.0, surface(h, x, 0.0) + 0.3), 1.5);
}

/// Where it looks: a band of red optics down each side of the nose from `x` back `len`
/// under the chine, eight to a side up close, and a long lens slit under the keel.
fn eye_band(b: &mut MeshBuilder, h: &[Station], x: f32, len: f32) {
    if !b.fine() {
        return;
    }
    let count = 8;
    b.paint(GLOW_LASER);
    b.mirror_y(|b| {
        for k in 0..count {
            let at = x - len * k as f32 / (count - 1) as f32;
            let y = half_width(h, at) * 0.92;
            let z = surface(h, at, y) - 0.12;
            b.beam(
                v3(at + 0.12, y, z),
                v3(at - 0.12, y + 0.02, z),
                Vec2::new(0.06, 0.08),
                Vec2::new(0.06, 0.08),
            );
        }
    });
    let keel = |at: f32| {
        let (mut lo, mut hi) = (h[0], h[1]);
        for w in h.windows(2) {
            if at <= w[1].x {
                lo = w[0];
                hi = w[1];
                break;
            }
        }
        let t = ((at - lo.x) / (hi.x - lo.x)).clamp(0.0, 1.0);
        lo.keel + (hi.keel - lo.keel) * t
    };
    b.beam(
        v3(x, 0.0, keel(x) - 0.02),
        v3(x - len, 0.0, keel(x - len) - 0.02),
        Vec2::new(0.18, 0.06),
        Vec2::new(0.18, 0.06),
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn fits_the_librarys_checks() {
        super::super::super::check("regency_augur", super::RADIUS, super::HEIGHT, None, &[]);
    }
}
