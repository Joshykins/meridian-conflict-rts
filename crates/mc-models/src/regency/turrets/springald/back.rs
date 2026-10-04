//! What the Springald's turret carries on its back behind the trunnion: the fusion plant
//! that feeds its gun, three upright capacitor cells, one for each knot of its triune
//! charge, its light let out only in thin lit seams (no open star).

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;

use super::super::super::kit::{dark_plate, metal, v3};
use super::super::super::machine::{armour, hoop_on, swept, Frame};
use super::super::bar_through;
use super::super::sunspear::coil_light;
use super::{LINE, ROOF};

/// The cells across the turret's back, each a plated hexagonal column under a swept cap, a lit seam up its back
/// lighting in turn through the charge; a plated spine ties them.
pub(super) fn draw(b: &mut MeshBuilder) {
    let (x, top) = (-14.0, ROOF + 9.0);
    dark_plate(b);
    b.block(v3(x - 3.6, -6.2, ROOF - 0.1), v3(x + 3.6, 6.2, ROOF + 1.2));
    for (y, stage) in [(-4.2f32, 2), (0.0, 4), (4.2, 6)] {
        let c = v3(x, y, ROOF + 1.2);
        dark_plate(b);
        b.prism(c, 6, 2.1, 1.8, top - c.z);
        metal(b);
        hoop_on(b, c + Vec3::Z * 1.0, Vec3::Z, 2.2, 0.35, 0.6, b.sides(12));
        coil_light(b, stage);
        b.block(
            v3(x - 2.06, y - 0.12, c.z + 2.0),
            v3(x - 1.9, y + 0.12, top - 1.0),
        );
        if b.fine() {
            dark_plate(b);
            armour(
                b,
                &Frame::new(v3(x + 1.6, y, top + 0.05), v3(-1.0, 0.0, -0.15), Vec3::Z),
                &swept(5.4, 1.6, 0.0, 0.5),
                0.35,
            );
        }
    }
    dark_plate(b);
    bar_through(
        b,
        &[
            (v3(x - 4.5, 0.0, ROOF + 1.2), Vec2::new(1.6, 2.4)),
            (v3(x - 3.0, 0.0, top - 2.0), Vec2::new(1.4, 2.0)),
        ],
        Vec3::Y,
    );
    conduits(b, v3(x + 2.0, 4.2, ROOF + 4.0));
}

/// Bronze conduits from `from` (either side) forward to the trunnion cheeks.
fn conduits(b: &mut MeshBuilder, from: Vec3) {
    if !b.fine() {
        return;
    }
    b.mirror_y(|b| {
        metal(b);
        super::super::super::kit::cable(
            b,
            &[
                from,
                from + v3(2.0, 0.6, -0.4),
                v3(-6.4, 5.6, LINE.pivot.z - 1.5),
            ],
            0.45,
        );
    });
}
