//! Three independent track runs beneath a split, angular assault hull.
use super::super::kit::{dark_plate, metal, v3};
use super::super::machine::red_slot;
use crate::builder::{MeshBuilder, Section};
use crate::material::{TEAM, TREAD};
use crate::part;
use glam::Vec3;

pub(super) fn body(b: &mut MeshBuilder) {
    b.set_treads(5.0, 2.3, -10.6);
    b.set_dust_line(1.0);
    // The centre belt projects through the split nose; all three remain visible at
    // every LOD. They share the tread shader's forward animation.
    for (y, nose, tail, half) in [
        (-5.0, 6.3, -10.6, 1.15),
        (0.0, 9.8, -6.8, 1.2),
        (5.0, 6.3, -10.6, 1.15),
    ] {
        track(b, y, nose, tail, half);
    }
    if b.coarse() {
        dark_plate(b);
        b.loft_z(
            &[[5.0, 4.3], [-7.4, 4.3], [-7.4, -4.3], [5.0, -4.3]],
            &[Section::new(1.8, 1.0), Section::new(3.65, 0.8)],
        );
        return;
    }
    dark_plate(b);
    let outline = [
        [6.7, 3.1],
        [2.6, 4.8],
        [-7.2, 4.4],
        [-8.0, 0.0],
        [-7.2, -4.4],
        [2.6, -4.8],
        [6.7, -3.1],
        [4.7, 0.0],
    ];
    b.with_facets(|b| {
        b.loft_z(
            &outline,
            &[
                Section::new(2.0, 1.0),
                Section::new(3.35, 0.92),
                Section::new(3.65, 0.72).shifted(-0.6, 0.0),
            ],
        )
    });
    if !b.coarse() {
        for sign in [-1.0, 1.0] {
            // Long, straight track guards: one sharp wedge per side, no rounded skirt.
            dark_plate(b);
            b.extrude_y(
                &[[6.2, 2.1], [4.8, 3.3], [-8.6, 3.05], [-10.1, 2.0]],
                sign * 3.8,
                sign * 6.3,
            );
            red_slot(
                b,
                v3(4.8, sign * 6.31, 2.65),
                Vec3::Y * sign,
                Vec3::X,
                1.1,
                0.11,
            );
        }
    }
    b.paint(TEAM);
    b.face(&[v3(-5.0, 0.0, 3.68), v3(-6.5, 0.7, 3.4), v3(-6.5, -0.7, 3.4)]);
}

fn track(b: &mut MeshBuilder, y: f32, nose: f32, tail: f32, half: f32) {
    b.with_part(part::LOCOMOTION, |b| {
        b.paint(TREAD);
        if b.coarse() {
            b.face(&[
                v3(tail, y - half, 1.0),
                v3(nose, y - half, 1.0),
                v3(nose, y + half, 1.0),
                v3(tail, y + half, 1.0),
            ]);
            b.face(&[
                v3(tail, y - half, 0.0),
                v3(nose, y - half, 1.0),
                v3(tail, y - half, 1.0),
            ]);
            return;
        }
        let outline = [
            [tail + 0.7, 0.0],
            [nose - 0.9, 0.0],
            [nose, 1.1],
            [nose - 0.7, 2.2],
            [tail + 0.7, 2.2],
            [tail, 1.0],
        ];
        b.extrude_y(&outline, y - half, y + half);
        if b.fine() {
            metal(b);
            for i in 0..5 {
                let x = tail + 1.0 + (nose - tail - 2.0) * i as f32 / 4.0;
                for sign in [-1.0, 1.0] {
                    b.cylinder_between(
                        v3(x, y + sign * (half + 0.01), 1.0),
                        v3(x, y + sign * (half + 0.07), 1.0),
                        0.72,
                        0.65,
                        6,
                    );
                }
            }
        }
    });
}
