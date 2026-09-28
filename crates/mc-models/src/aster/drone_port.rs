//! The commander's salvage drone port: a rack on its back that two drones ride
//! and launch from (the `drone_port` refit, `data/factions/aster/units/command.ron`).
//! The drones are units of their own, sat on the pads by the sim (`drone_sockets`,
//! which are these pad tops times the model's 1.6 scale).

use glam::Vec3;

use super::parts::*;
use crate::builder::MeshBuilder;
use crate::material::*;

/// A pad's centre on the +y side, and the height of its top, where a drone sits.
const PAD: Vec3 = Vec3::new(-3.3, 1.25, 13.6);
const PAD_RADIUS: f32 = 0.95;

/// The rack, drawn on the torso (`TURRET`) so it turns with it.
pub(super) fn commander_drone_port(b: &mut MeshBuilder) {
    if b.coarse() {
        b.module("drone_port", 0.3, |b| {
            b.paint(ACCENT);
            b.block(v3(-3.0, -0.6, 10.6), v3(-2.0, 0.6, 13.3));
            b.paint(METAL);
            b.block(
                v3(PAD.x - PAD_RADIUS, -PAD.y - PAD_RADIUS, PAD.z - 0.3),
                v3(PAD.x + PAD_RADIUS, PAD.y + PAD_RADIUS, PAD.z),
            );
        });
        return;
    }
    b.module("drone_port", 0.0, |b| {
        // The spine: a black housing up the middle of the back, a white lid on it.
        b.paint(ACCENT);
        b.chamfered_box(v3(-2.6, 0.0, 12.0), v3(1.1, 1.3, 2.8), 0.2);
        b.paint(PLATING);
        b.plate(v3(-2.6, 0.0, 13.4), v2(0.8, 0.9), 0.1, 0.04);
    });
    b.module("drone_port", 0.35, |b| {
        b.mirror_y(|b| {
            // A strut out from the spine to each pad, and the pad on it.
            b.paint(METAL);
            b.beam(
                v3(-2.7, 0.4, 12.9),
                v3(PAD.x, PAD.y, PAD.z - 0.35),
                v2(0.35, 0.3),
                v2(0.3, 0.25),
            );
            b.paint(ACCENT);
            b.cylinder_between(
                v3(PAD.x, PAD.y, PAD.z - 0.3),
                PAD,
                PAD_RADIUS,
                PAD_RADIUS,
                b.sides(12),
            );
        });
    });
    b.module("drone_port", 0.7, |b| {
        if b.fine() {
            // An amber band lit round each pad's rim, its caps hidden inside the pad.
            b.paint(GLOW_AMBER);
            b.mirror_y(|b| {
                b.cylinder_between(
                    v3(PAD.x, PAD.y, PAD.z - 0.2),
                    v3(PAD.x, PAD.y, PAD.z - 0.08),
                    PAD_RADIUS + 0.04,
                    PAD_RADIUS + 0.04,
                    b.sides(12),
                )
            });
        }
    });
}
