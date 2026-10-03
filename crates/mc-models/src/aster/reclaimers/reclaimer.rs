//! The Reclaimer (every tier, drawn bigger at 2 and 3): a salvage hovercraft riding on two cushion pods
//! down its sides, working land and water alike, with a reclaim head (weapon 0) on a low A-frame over its tail. The head turns
//! full circle and pitches steeply down or up while the craft drives on; the haul drops
//! down a chute into the hoppers behind the cab.
//!
//! Authored at the tech 1 blueprint's scale (radius 4.6, height 3.4): model metres are unit
//! metres, and the higher tiers' head numbers are these times their scale.

use glam::Vec3;

use super::*;
use crate::builder::chamfered_rect;

/// The head's trunnion (the unit file's head `pivot`). Its mouth, the beam's emitter,
/// is 2.33 m ahead of it (`lance`: 2.22 x the head's scale).
pub(in crate::aster) const PIVOT: Vec3 = Vec3::new(-2.1, 0.0, 4.1);
/// The head's scale (1 for a 2 m head).
const HEAD: f32 = 1.05;
/// The deck: the top of the hull.
const DECK: f32 = 1.75;

pub(in crate::aster) fn reclaimer(b: &mut MeshBuilder, _tech: u8) {
    b.set_hover();
    if b.coarse() {
        coarse(b, (-3.8, 3.9, 2.35, 1.2), DECK, &[(PIVOT, DECK)]);
        return;
    }
    let deck = podded_hull(b);
    cab(b, deck.z);
    // Hoppers in a row behind the cab.
    hopper(b, v3(0.55, 0.0, deck.z), v2(1.6, 2.4), 0.6);
    hopper(b, v3(-0.9, 0.0, deck.z), v2(1.1, 2.4), 0.6);

    // A-frame: two short legs a side from the bed to a head block, fixed; the head turns
    // on top of it.
    let p = PIVOT;
    let apex = v3(p.x, 0.0, p.z - 0.85);
    b.paint(PLATING_DARK);
    b.mirror_y(|b| {
        b.beam(
            v3(p.x - 0.95, 1.2, deck.z),
            apex + v3(-0.2, 0.4, -0.1),
            v2(0.35, 0.35),
            v2(0.3, 0.3),
        );
        b.beam(
            v3(p.x + 0.95, 1.2, deck.z),
            apex + v3(0.2, 0.4, -0.1),
            v2(0.3, 0.3),
            v2(0.26, 0.26),
        );
    });
    b.paint(ACCENT);
    b.chamfered_box(apex - Vec3::Z * 0.1, v3(1.4, 1.4, 0.5), 0.2);
    // A glazed chute from the head block down into the rear hopper, the haul in it lit.
    chute(
        b,
        apex + v3(0.55, 0.0, -0.2),
        v3(-0.95, 0.0, deck.z + 0.62),
        0.36,
    );
    reclaim_head(b, 0, p, apex.z + 0.15, HEAD);
    if b.fine() {
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.cuboid(v3(3.65, 1.25, 1.35), v3(0.3, 0.3, 0.26));
            b.paint(GLASS);
            b.cuboid(v3(3.81, 1.25, 1.35), v3(0.04, 0.22, 0.18));
        });
        whip(b, v3(1.9, -1.1, deck.z + 0.9), 1.0, 0.2);
    }
}

/// The cab: a raked white box forward with a dark screen and the team plate on its roof.
fn cab(b: &mut MeshBuilder, deck: f32) {
    let (cab0, cab1) = (1.7, 3.6);
    b.paint(PLATING);
    b.frustum(
        v3((cab0 + cab1) * 0.5, 0.0, deck - 0.05),
        v2(cab1 - cab0, 3.0),
        v2(cab1 - cab0 - 0.9, 2.5),
        0.95,
        v2(-0.35, 0.0),
    );
    if b.fine() {
        on_slope(b, [cab1, deck], [cab1 - 0.45, deck + 0.9], 0.5, |b| {
            b.paint(GLASS);
            b.plate(Vec3::ZERO, v2(0.6, 2.2), 0.05, 0.02);
        });
        team_panel(b, v3(2.3, 0.0, deck + 0.9), v2(0.8, 2.0));
    }
}

/// Two cushion pods down the sides, each a long skirted float with a rub strip, and the
/// shell slung on a dark tub between them. Returns the deck.
fn podded_hull(b: &mut MeshBuilder) -> Roof {
    let pod = chamfered_rect(v2(3.95, 0.62), 0.5);
    b.mirror_y(|b| {
        b.at(v3(0.05, 1.75, 0.0), |b| {
            b.with_part(part::LOCOMOTION, |b| {
                b.paint(TREAD);
                b.loft_z(
                    &pod,
                    &[
                        Section::new(0.16, 1.06),
                        Section::new(0.42, 1.0),
                        Section::new(0.95, 0.82),
                    ],
                );
                b.paint(METAL);
                b.loft_z(&pod, &[Section::new(0.16, 1.03), Section::new(0.22, 1.03)]);
            });
            if b.fine() {
                b.paint(PLATING);
                b.plate(v3(0.2, 0.0, 0.95), v2(5.6, 0.7), 0.06, 0.03);
            }
        });
    });
    b.paint(ACCENT);
    b.block(v3(-3.2, -1.3, 0.5), v3(3.3, 1.3, 0.9));
    shell(b, -3.8, 3.9, 1.75, 0.8, DECK)
}
