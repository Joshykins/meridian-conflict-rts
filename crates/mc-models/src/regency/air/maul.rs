//! The Maul, the Regency's tech 3 strategic bomber: one Pinch-fusion Bomb carried in a
//! cradle under its belly, a small caged star (pinch fusion's prism, held in bronze gravity
//! rings) that it lets go over its mark. A broad jet in the Regency's language: a nose
//! blade, swept plates lapped over the wings in rows, their tips turned down, bronze
//! workings between them, red optics and red heat in four plasma jets.
//!
//! Authored at the blueprint's size (`regency_t3_strategic_bomber` in
//! `data/factions/regency/units/air_t3.ron`): model metres are unit metres, and `BOMB` is
//! the unit file's muzzle.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::library::ModelDef;
use crate::material::*;

use super::super::kit::{metal, seam, v3};
use super::super::machine::hoop_on;
use super::blade_jet::{
    blade, chevron, feathers, fin, half_width, hull, intake, lap_pair, nozzle, optics, pin_line,
    st, surface, wing, workings, Station,
};

pub(crate) const RADIUS: f32 = 16.0;
pub(crate) const HEIGHT: f32 = 3.6;

/// The bomb's middle as it hangs in its cradle: the unit file's muzzle.
pub(crate) const BOMB: Vec3 = Vec3::new(0.0, 0.0, 0.85);
/// The charge's radius.
const CHARGE: f32 = 0.55;

pub(crate) const MODELS: &[ModelDef] = &[ModelDef::new("regency_maul", RADIUS, HEIGHT, hammer)];

// ---- the hammer --------------------------------------------------------------------

/// A long heavy hull under a hammer of a nose blade, a swept wing set back, two jet
/// nacelles slung under its roots with the bomb cradled between them, down-turned tail
/// planes.
const HAMMER_HULL: [Station; 6] = [
    st(-11.0, 1.4, [2.0, 1.2], [2.6, 0.8], 3.0),
    st(-7.5, 1.0, [1.9, 1.7], [2.8, 1.1], 3.3),
    st(-2.0, 0.85, [1.85, 1.9], [2.9, 1.2], 3.4),
    st(4.0, 0.9, [1.9, 1.75], [2.85, 1.05], 3.2),
    st(8.5, 1.15, [2.0, 1.15], [2.6, 0.7], 2.75),
    st(11.0, 1.45, [2.05, 0.55], [2.3, 0.3], 2.3),
];
const HAMMER_WING: [[f32; 2]; 6] = [
    [3.6, 1.6],
    [-4.0, 9.6],
    [-6.8, 14.2],
    [-8.8, 14.2],
    [-8.2, 9.0],
    [-7.4, 1.6],
];
const HAMMER_FEATHERS: [&[[f32; 2]]; 3] = [
    &[
        [3.0, 1.9],
        [-1.2, 6.4],
        [-8.6, 6.6],
        [-6.4, 4.4],
        [-7.2, 2.0],
    ],
    &[
        [-1.0, 6.6],
        [-4.2, 10.2],
        [-9.4, 10.4],
        [-7.8, 8.8],
        [-7.4, 7.6],
    ],
    &[
        [-4.0, 10.4],
        [-6.4, 13.8],
        [-9.6, 14.8],
        [-8.4, 12.6],
        [-8.0, 11.6],
    ],
];
/// The nacelles: one hull each, under the wing roots.
const NACELLE: [Station; 4] = [
    st(-7.6, 0.55, [0.95, 0.75], [1.35, 0.5], 1.6),
    st(-4.0, 0.4, [0.95, 0.9], [1.45, 0.6], 1.75),
    st(1.4, 0.45, [0.95, 0.8], [1.4, 0.5], 1.65),
    st(3.4, 0.6, [1.0, 0.5], [1.3, 0.3], 1.4),
];
const NACELLE_Y: f32 = 3.6;

fn hammer(b: &mut MeshBuilder, _tech: u8) {
    let h = &HAMMER_HULL;
    hull(b, h, v3(11.8, 0.0, 2.0), &[0, 3]);
    // The hammer: a broad blade, and a short cross blade either side of its root.
    blade(b, v3(11.4, 0.0, 2.05), v3(16.2, 0.0, 1.95), 1.1, 0.4);
    if b.coarse() {
        b.mirror_y(|b| wing(b, &[[3.6, 1.6], [-7.8, 14.2], [-7.4, 1.6]], 1.85, 0.25));
        chevron(b, v3(-4.0, 0.0, surface(h, -4.0, 0.0) + 0.3), 2.6);
        cradle(b, 0.95, 2.3);
        return;
    }
    b.mirror_y(|b| {
        blade(b, v3(10.6, 0.9, 1.95), v3(11.6, 3.2, 1.85), 0.45, 0.24);
        wing(b, &HAMMER_WING, 1.85, 0.22);
        feathers(b, &HAMMER_FEATHERS, 2.07, 0.12, 0.08);
        fin(b, v3(-6.8, 14.2, 2.0), 2.0, 1.6, 0.35, 1.2, 0.55, 0.16);
        // The tail planes, turned down.
        fin(b, v3(-8.6, 1.9, 1.6), 2.2, 1.35, 0.7, 1.4, 0.5, 0.16);
        b.at(v3(0.0, NACELLE_Y, 0.0), |b| {
            hull(b, &NACELLE, v3(4.4, 0.0, 1.0), &[0, 2]);
        });
        nozzle(b, v3(-7.5, NACELLE_Y, 1.1), 0.62, 1.2);
        nozzle(b, v3(-10.9, 1.0, 2.05), 0.5, 0.9);
        if b.fine() {
            for (k, f) in HAMMER_FEATHERS.iter().enumerate() {
                let z = 2.07 + 0.08 * k as f32 + 0.135;
                pin_line(
                    b,
                    v3(f[0][0] - 0.2, f[0][1] + 0.15, z),
                    v3(f[1][0] - 0.2, f[1][1], z),
                );
            }
            workings(b, v3(2.6, 2.6, 2.2), v3(-7.0, 2.6, 2.2), 0.12, &[0.3, 0.65]);
            intake(
                b,
                v3(3.6, NACELLE_Y, 1.05),
                v3(1.8, NACELLE_Y, 1.05),
                glam::Vec2::new(0.9, 0.7),
            );
            // A strut from nacelle to the wing over it.
            metal(b);
            b.cylinder_between(
                v3(-1.0, NACELLE_Y, 1.7),
                v3(-1.0, NACELLE_Y, 1.9),
                0.3,
                0.3,
                8,
            );
        }
    });
    back(b, h);
    cradle(b, 0.95, 2.3);
    optics(b, 10.2, 0.75, 2.45, 1.2);
}

// ---- shared ---------------------------------------------------------------------

/// Plates lapped down the back, the bronze spine between them, the owner's chevron.
fn back(b: &mut MeshBuilder, h: &[Station]) {
    let (tail, front) = (h[0].x, h[h.len() - 2].x);
    let length = front - tail;
    for k in 0..3 {
        let x0 = front - length * (0.08 + 0.3 * k as f32);
        let x1 = x0 - length * 0.32;
        let (w0, w1) = (half_width(h, x0), half_width(h, x1));
        lap_pair(
            b,
            h,
            &[
                [x0, 0.15, 0.06],
                [x0 - 0.3, w0 * 0.7, 0.08],
                [x1 - 1.4, w1 * 1.0 + 0.4, 0.3],
                [x1 + 0.4, w1 * 0.55, 0.25],
                [x1 + 0.15, 0.15, 0.22],
            ],
            0.14,
        );
    }
    if b.fine() {
        let at = |x: f32| v3(x, 0.0, surface(h, x, 0.0) + 0.08);
        workings(
            b,
            at(front - length * 0.05),
            at(tail + length * 0.08),
            0.16,
            &[0.3, 0.62],
        );
    }
    let x = tail + length * 0.3;
    chevron(b, v3(x, 0.0, surface(h, x, 0.0) + 0.42), 2.4);
}

/// The bomb in its cradle under the belly: bronze arms from the keel at `keel` down to two
/// gravity rings, the bomb hung in them, a small caged star, white at its heart with the
/// prism over it. `span` is how far fore and aft the cradle's arms reach.
fn cradle(b: &mut MeshBuilder, keel: f32, span: f32) {
    if b.coarse() {
        b.paint(GLOW_PRISM);
        b.spheroid(BOMB, Vec3::splat(CHARGE), 4, 2);
        return;
    }
    // The arms: two pairs of bronze struts from the belly to the rings.
    metal(b);
    let sides = b.sides(8);
    for x in [-span * 0.5, span * 0.5] {
        for y in [-0.45f32, 0.45] {
            b.cylinder_between(
                v3(x * 1.2, y * 1.4, keel + 0.25),
                v3(x * 0.55, y, BOMB.z + 0.1),
                0.09,
                0.07,
                sides,
            );
        }
    }
    // The gravity rings, one fore and one aft of the charge, and a third round it.
    let segs = if b.fine() { 16 } else { 8 };
    for x in [-0.55f32, 0.55] {
        metal(b);
        hoop_on(
            b,
            BOMB + v3(x, 0.0, 0.0),
            Vec3::X,
            CHARGE * 1.1,
            0.12,
            0.12,
            segs,
        );
    }
    seam(b);
    hoop_on(b, BOMB, Vec3::Z, CHARGE * 1.25, 0.08, 0.1, segs);
    let (s, r) = if b.fine() { (14, 7) } else { (8, 4) };
    b.paint(GLOW_PRISM);
    b.spheroid(BOMB, Vec3::new(CHARGE * 1.15, CHARGE, CHARGE), s, r);
    if b.fine() {
        b.paint(GLOW_LASER);
        for x in [-1.0f32, 1.0] {
            b.cylinder_between(
                BOMB + v3(x * CHARGE * 1.15, 0.0, 0.0),
                BOMB + v3(x * (CHARGE * 1.15 + 0.22), 0.0, 0.0),
                0.12,
                0.05,
                6,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_the_librarys_checks() {
        super::super::super::check("regency_maul", RADIUS, HEIGHT, None, &[]);
    }

    #[test]
    fn the_bomb_hangs_at_the_muzzle() {
        for key in ["regency_maul"] {
            let model = crate::build_model(key).unwrap();
            for lod in &model.lods {
                let prism: Vec<Vec3> = lod
                    .vertices
                    .iter()
                    .filter(|v| v.material == GLOW_PRISM)
                    .map(|v| Vec3::from(v.pos))
                    .collect();
                let mid = prism.iter().copied().sum::<Vec3>() / prism.len() as f32;
                assert!(mid.distance(BOMB) < 0.05, "{key}: the charge at {mid}");
            }
        }
    }
}
