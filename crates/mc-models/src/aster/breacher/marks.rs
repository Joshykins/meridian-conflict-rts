//! The Breacher's surface marks (docs/STANDARD.md, "Texture detail and marks"): the
//! fittings a real machine would carry, drawn by the surface, and the hull number. The
//! arms' and guns' own marks are laid where they are built (`body::arm`, `gun::gun`,
//! `pods`), in their frames.

use super::super::parts::*;
use crate::builder::MeshBuilder;
use crate::gpu_consts::surface_mark as mark;

/// The torso's and the legs' marks.
pub(super) fn marks(b: &mut MeshBuilder) {
    // Louvres over the hump's tail, where the launchers' bay breathes.
    b.mark(
        v3(-16.8, -6.0, 38.5),
        v3(-14.8, 6.0, 43.0),
        mark::LOUVRES,
        0.35,
    );
    // An access cover on the belly's front.
    b.mark(v3(9.6, -2.4, 27.6), v3(10.9, 2.4, 29.4), mark::ACCESS, 0.0);
    // Hazard stripes round the waist ring the torso turns on.
    b.mark(v3(-8.5, -8.0, 24.2), v3(6.5, 8.0, 25.8), mark::HAZARD, 0.0);
    // A weld across the brow's sloped face.
    b.mark(v3(14.2, -8.0, 41.5), v3(14.8, 8.0, 42.1), mark::SEAM, 0.0);
    b.mirror_y(|b| {
        // Applique on the pectorals, which take the hits.
        b.mark(v3(11.0, 6.0, 32.2), v3(13.2, 10.6, 34.4), mark::PANEL, 0.0);
        // The hull number on the upper flank, lettering under it.
        b.mark(
            v3(-1.5, 12.8, 40.0),
            v3(4.5, 16.0, 42.0),
            mark::STENCIL,
            47.0,
        );
        b.mark(v3(-1.5, 12.8, 38.6), v3(4.5, 16.0, 39.4), mark::LABEL, 2.0);
        // A warning on the hump's flank, by the launchers' blast.
        b.mark(
            v3(-12.5, 10.0, 40.0),
            v3(-9.5, 12.8, 42.5),
            mark::WARNING,
            0.0,
        );
        // The legs: the owner's colour on the thigh's top plate, a bolt row along the
        // foot's outer side, hazard stripes on its toe.
        b.mark(v3(2.9, 9.0, 21.6), v3(5.1, 11.5, 23.0), mark::TEAM, 0.0);
        b.mark(v3(-6.0, 16.4, 1.1), v3(7.0, 17.0, 1.6), mark::BOLTS, 1.2);
        b.mark(v3(8.3, 8.9, 0.9), v3(9.2, 16.1, 1.9), mark::HAZARD, 0.0);
    });
}
