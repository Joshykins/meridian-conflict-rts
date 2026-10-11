//! The station's surface marks (`gpu_consts::surface_mark`): its zones (armour on the
//! foot, machinery on the head and at the mast's head, a walkway round the plinth) and
//! the fittings a real post carries: bolt rows under the foot's band, applique on its
//! shoulders, louvres and access covers on its walls, panels and a grille on the mast,
//! grilles, a hatch, louvres and a vent on the head, and the works' own.

use std::f32::consts::{FRAC_PI_2, PI};

use glam::Vec3;

use super::super::parts::*;
use super::{BASE, DECK, ELBOW, FOOT, FOOT_TOP, FOOT_WALL, PLINTH, ROOF, STEP};
use crate::builder::MeshBuilder;
use crate::gpu_consts::surface_mark as mark;

/// The pitch of a grille's, a louvre's or a vent's holes: the factories'.
const PITCH: f32 = 0.22;

pub(super) fn marks(b: &mut MeshBuilder) {
    let wall = FOOT + 0.3;
    // Zones.
    b.mark(
        v3(-wall, -wall, BASE),
        v3(wall, wall, FOOT_TOP),
        mark::ARMOUR,
        0.0,
    );
    b.mark(
        v3(-PLINTH - 0.2, -PLINTH - 0.2, BASE - 0.1),
        v3(PLINTH + 0.2, PLINTH + 0.2, BASE + 0.12),
        mark::WALKWAY,
        0.0,
    );
    b.mark(
        v3(-3.4, -3.4, DECK),
        v3(3.4, 3.4, ROOF + 0.2),
        mark::MACHINERY,
        0.0,
    );
    b.mark(
        v3(-3.0, -3.0, DECK - 2.6),
        v3(3.0, 3.0, DECK),
        mark::MACHINERY,
        0.0,
    );
    // The foot: a bolt row under the band and applique on the shoulder of every face;
    // louvres and an access cover on every wall.
    for q in 0..4 {
        b.yawed(Vec3::ZERO, q as f32 * FRAC_PI_2, |b| {
            b.mark(
                v3(FOOT - 0.2, -4.2, FOOT_WALL - 0.6),
                v3(FOOT + 0.2, 4.2, FOOT_WALL - 0.4),
                mark::BOLTS,
                0.0,
            );
            b.mark(v3(5.0, -2.2, 5.2), v3(6.7, 2.2, 6.6), mark::PANEL, 0.0);
            b.mark(
                v3(FOOT - 0.2, -4.3, BASE + 0.3),
                v3(FOOT + 0.2, -2.0, 3.9),
                mark::LOUVRES,
                PITCH,
            );
            b.mark(
                v3(FOOT - 0.2, 2.0, BASE + 0.1),
                v3(FOOT + 0.2, 3.6, 3.9),
                mark::ACCESS,
                0.0,
            );
        });
    }
    // The plinth: bolt rows along its back and flanks.
    for a in [FRAC_PI_2, PI, -FRAC_PI_2] {
        b.yawed(Vec3::ZERO, a, |b| {
            b.mark(
                v3(PLINTH - 0.2, -5.6, BASE - 0.55),
                v3(PLINTH + 0.2, 5.6, BASE - 0.35),
                mark::BOLTS,
                0.0,
            );
        });
    }
    // The mast: applique low, a grille band above the braces.
    b.mark(v3(-3.2, -3.2, 8.6), v3(3.2, 3.2, 10.6), mark::PANEL, 0.0);
    b.mark(
        v3(-3.0, -3.0, 12.9),
        v3(3.0, 3.0, 13.8),
        mark::GRILLE,
        PITCH,
    );
    // The head: a grille between the cheeks, a hatch aft, louvres on the flanks, a vent
    // on the back, an access cover on the front, a bolt row down the forearm's back.
    b.mark(
        v3(0.4, -0.8, ROOF - 0.1),
        v3(2.4, 0.8, ROOF + 0.1),
        mark::GRILLE,
        PITCH,
    );
    b.mark(
        v3(-2.7, 0.2, ROOF - 0.1),
        v3(-1.8, 1.1, ROOF + 0.1),
        mark::HATCH,
        0.0,
    );
    b.mirror_y(|b| {
        b.mark(
            v3(-1.6, 2.85, 22.6),
            v3(1.2, 3.15, 23.4),
            mark::LOUVRES,
            PITCH,
        );
    });
    b.mark(
        v3(-3.15, -1.2, 22.6),
        v3(-2.85, 1.2, 23.5),
        mark::VENT,
        PITCH,
    );
    b.mark(
        v3(2.85, -2.3, 22.5),
        v3(3.15, -1.1, 23.6),
        mark::ACCESS,
        0.0,
    );
    b.mark(
        v3(ELBOW.x + 1.0, -0.3, ELBOW.z + 0.8),
        v3(ELBOW.x + 6.5, 0.3, ELBOW.z + 1.0),
        mark::BOLTS,
        0.0,
    );
    // The magazines: a bolt row along each lid's edge.
    for a in [FRAC_PI_2, -FRAC_PI_2, PI] {
        b.yawed(Vec3::ZERO, a, |b| {
            b.mark(
                v3(13.3, -2.3, STEP + 2.6),
                v3(13.7, 2.3, STEP + 2.8),
                mark::BOLTS,
                0.0,
            );
        });
    }
}
