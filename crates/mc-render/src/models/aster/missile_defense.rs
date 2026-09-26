//! Corona: tech 2 tactical missile defence. A squat plinth, a dark column and a
//! yoke carrying two of the navy's missile-defence laser heads (`pd_laser`, the
//! steady red that marks missile defence on every hull), with a tracking radar
//! turning between them. The heads stand where the unit file's
//! `anti_missile_mounts` say the beams leave: keep `LASERS` and the data in step.

use super::naval::pd_laser;
use super::parts::*;
use crate::models::builder::{ngon, MeshBuilder, Section};
use crate::models::material::*;
use crate::models::part;
use glam::Vec3;

/// The laser heads' centres (`anti_missile_mounts` in structures.ron).
const LASERS: [Vec3; 2] = [Vec3::new(0.0, 2.8, 10.6), Vec3::new(0.0, -2.8, 10.6)];
/// Top of the column, where the yoke sits.
const YOKE: f32 = 9.4;
/// The tracking radar turns about the column's axis from here.
const RADAR: f32 = 10.0;

pub(super) fn missile_defense(b: &mut MeshBuilder, _tech: u8) {
    b.set_spinner_pivot(v3(0.0, 0.0, RADAR));
    if b.coarse() {
        // Plinth, column, one bar standing in for yoke and heads, the heads' red
        // lenses on top and the radar.
        b.paint(PLATING);
        b.frustum_open(v3(0.0, 0.0, 0.0), v2(8.4, 8.4), v2(4.6, 4.6), 3.4, v2(0.0, 0.0));
        b.paint(PLATING_DARK);
        b.cuboid_open(v3(0.0, 0.0, 6.4), v3(1.7, 1.7, 6.0));
        b.paint(PLATING);
        b.cuboid_open(v3(0.0, 0.0, 10.2), v3(1.3, 7.0, 1.6));
        b.paint(GLOW_LASER);
        for at in LASERS {
            b.decal(v3(at.x, at.y, 11.02), v2(1.2, 1.2));
        }
        b.with_part(part::SPINNER, |b| {
            b.paint(PLATING);
            b.cuboid_open(v3(0.3, 0.0, 12.2), v3(0.3, 2.8, 1.5));
        });
        return;
    }

    // A dark pad, then a white octagonal plinth pinching in to the column.
    b.paint(ACCENT);
    b.loft_z(&ngon(8, 5.4), &[Section::new(0.0, 1.0), Section::new(0.4, 0.97)]);
    b.paint(PLATING);
    b.loft_z(
        &ngon(8, 4.2),
        &[
            Section::new(0.3, 1.0),
            Section::new(2.2, 0.92),
            Section::new(3.4, 0.62),
        ],
    );
    team_panel(b, v3(-1.75, 0.0, 3.4), v2(0.9, 1.5));
    if b.fine() {
        // Access hatch on the plinth's face and a cable run up the column.
        b.paint(ACCENT);
        b.cuboid(v3(3.55, 0.0, 1.3), v3(0.3, 1.5, 1.8));
        b.paint(METAL);
        b.cylinder_between(v3(0.75, -0.55, 3.3), v3(0.62, -0.5, YOKE - 0.3), 0.12, 0.12, 6);
    }

    // The column and a collar where the yoke clamps on.
    b.paint(PLATING_DARK);
    b.prism(v3(0.0, 0.0, 3.3), b.sides(8), 1.0, 0.8, YOKE - 3.3);
    b.paint(PLATING);
    b.prism(v3(0.0, 0.0, YOKE - 0.45), b.sides(8), 1.3, 1.2, 0.9);

    // The yoke: one arm out to each head, braced from the column below.
    b.paint(PLATING);
    b.beam(v3(0.0, -2.9, YOKE), v3(0.0, 2.9, YOKE), v2(0.8, 0.6), v2(0.8, 0.6));
    if b.fine() {
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.beam(v3(0.0, 0.75, YOKE - 2.6), v3(0.0, 2.55, YOKE - 0.2), v2(0.32, 0.32), v2(0.26, 0.26));
        });
    }
    for at in LASERS {
        pd_laser(b, at, 0.72, Some(YOKE + 0.3));
    }

    // Tracking radar: a tilted panel on a short mast with a feed horn and a
    // counterweight, turning between the heads without reaching them.
    b.with_part(part::SPINNER, |b| {
        b.paint(METAL);
        b.prism(v3(0.0, 0.0, YOKE + 0.4), b.sides(6), 0.34, 0.28, 2.0);
        b.pitched(v3(0.35, 0.0, 12.2), 0.3, |b| {
            b.paint(PLATING);
            b.cuboid(Vec3::ZERO, v3(0.22, 2.8, 1.5));
            if b.fine() {
                b.paint(ACCENT);
                b.cuboid(v3(-0.2, 0.0, 0.0), v3(0.2, 2.2, 1.0));
                b.paint(METAL);
                b.cylinder_between(v3(0.1, 0.0, 0.0), v3(1.1, 0.0, -0.05), 0.09, 0.07, 6);
                b.cuboid(v3(1.15, 0.0, -0.05), v3(0.2, 0.3, 0.3));
            }
        });
        if b.fine() {
            b.paint(ACCENT);
            b.cuboid(v3(-0.7, 0.0, 11.9), v3(0.8, 0.7, 0.55));
        }
    });
}
