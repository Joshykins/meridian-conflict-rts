//! Recessed bifurcated siege head: broad angular cheeks frame a protected pinch chamber.
use super::super::commander::form::{blade, ring, sleeve, OCT};
use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::hoop_on;
use crate::material::GLOW_LASER;

fn coil_light(b: &mut MeshBuilder, stage: u32) {
    b.paint(GLOW_LASER).pattern(pattern::COIL + stage);
}
use super::LANCE_PIVOT;
use crate::builder::MeshBuilder;
use crate::gpu_consts::charge_gear::{EXTEND, SPREAD};
use crate::{pattern, rig};
use glam::Vec3;

#[cfg(test)]
pub(super) const MOUTH: f32 = 22.0;
pub(super) const GEAR: f32 = 3.5;

pub(super) fn lance(b: &mut MeshBuilder) {
    b.set_charge_gear(LANCE_PIVOT, GEAR);
    b.with_limb(rig::ARM_GUN, |b| {
        b.at(LANCE_PIVOT, |b| {
            dark_plate(b);
            sleeve(
                b,
                &[
                    ring(v3(-5.0, 0.0, 0.0), Vec3::Z, 6.0, 4.5),
                    ring(v3(0.0, 0.0, 0.0), Vec3::Z, 7.0, 5.0),
                    ring(v3(5.0, 0.0, 0.0), Vec3::Z, 5.5, 4.5),
                ],
                &OCT,
            );
            core(b);
            b.mirror_y(|b| b.with_charge_gear(SPREAD, cheek));
        })
    });
}

fn cheek(b: &mut MeshBuilder) {
    // The sloping brow, hard chine and undercut jaw form a helmet rather than a barrel.
    let cross = |x: f32, w: f32, h: f32| {
        vec![
            v3(x, 1.9, h * 0.85),
            v3(x, (w * 0.6).max(2.3), h),
            v3(x, w, h * 0.25),
            v3(x, w * 0.88, -h * 0.65),
            v3(x, 1.9, -h * 0.75),
        ]
    };
    dark_plate(b);
    b.with_facets(|b| {
        b.loft(
            &[
                cross(1.0, 6.5, 4.8),
                cross(7.0, 7.3, 4.5),
                cross(14.0, 5.4, 3.7),
                cross(20.0, 3.2, 2.6),
            ],
            true,
            true,
        )
    });
    // A single swept brow plate, with narrow nested edge trim and an inset heat gallery.
    dark_plate(b);
    blade(
        b,
        v3(3.0, 3.2, 4.5),
        v3(1.0, -0.06, -0.10),
        Vec3::Z,
        15.0,
        2.2,
        -0.25,
        0.55,
    );
    if b.fine() {
        metal(b);
        blade(
            b,
            v3(3.4, 3.2, 5.0),
            v3(1.0, -0.06, -0.10),
            Vec3::Z,
            9.0,
            0.22,
            0.0,
            0.08,
        );
        for i in 0..6 {
            let x = 5.0 + i as f32 * 1.5;
            seam(b);
            blade(
                b,
                v3(x, 5.8 - i as f32 * 0.20, 1.0),
                v3(0.5, 0.0, -1.0),
                Vec3::Y,
                1.8,
                0.25,
                0.0,
                0.16,
            );
        }
        // Charge is a surface treatment on each cheek's inside wall.
        for i in 0..6u32 {
            dark_plate(b);
            b.pattern(pattern::COIL + i);
            b.face(&[
                v3(3.0 + i as f32 * 2.6, 1.89, -1.5),
                v3(5.0 + i as f32 * 2.6, 1.89, -1.5),
                v3(5.0 + i as f32 * 2.6, 1.89, 1.5),
                v3(3.0 + i as f32 * 2.6, 1.89, 1.5),
            ]);
        }
    }
}

fn core(b: &mut MeshBuilder) {
    metal(b);
    b.cylinder_between(
        v3(3.0, 0.0, 0.0),
        v3(21.2, 0.0, 0.0),
        1.55,
        1.3,
        b.sides(12),
    );
    let count = if b.fine() { 6 } else { 3 };
    for i in 0..count {
        let x = 4.5 + i as f32 * if b.fine() { 2.6 } else { 5.2 };
        dark_plate(b);
        hoop_on(b, v3(x, 0.0, 0.0), Vec3::X, 1.7, 0.6, 0.7, b.sides(12));
        coil_light(b, i as u32);
        b.face(&[
            v3(x + 0.71, -0.8, 1.15),
            v3(x + 0.71, 0.8, 1.15),
            v3(x + 0.71, 0.8, 1.5),
            v3(x + 0.71, -0.8, 1.5),
        ]);
    }
    b.with_charge_gear(EXTEND, |b| {
        metal(b);
        b.cylinder_between(v3(11.0, 0.0, 0.0), v3(16.0, 0.0, 0.0), 0.7, 0.45, 6);
    });
    dark_plate(b);
    hoop_on(b, v3(20.5, 0.0, 0.0), Vec3::X, 1.55, 0.45, 0.8, b.sides(12));
    b.with_charge_gear(0, |b| {
        coil_light(b, 6);
        b.face(&[
            v3(21.31, -0.55, -0.55),
            v3(21.31, 0.55, -0.55),
            v3(21.31, 0.55, 0.55),
            v3(21.31, -0.55, 0.55),
        ]);
    });
}
