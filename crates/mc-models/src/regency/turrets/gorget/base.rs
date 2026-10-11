//! The Gorget's base: a hexagonal keep on a footing, buttressed, and the turret's deck.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::gpu_consts::surface_mark as mark;

use super::super::super::kit::{dark_plate, metal, seam, v3};
use super::super::super::machine::*;
use super::super::*;

/// A hexagonal keep on a footing, under a seam course: a keeled buttress off every other
/// face, lapped with two plates down its back; between them a recessed bay with an
/// intake low and lamp slots let into its plate; a dark race with a bronze bearing on top.
pub(super) fn keep(b: &mut MeshBuilder) {
    let fine = b.fine();
    let segs = if fine { 24 } else { 8 };
    dark_plate(b);
    b.prism(Vec3::ZERO, 6, 6.8, 6.5, 0.45);
    b.prism(Vec3::Z * 0.45, 6, 6.2, 5.3, 4.75);
    seam(b);
    b.prism(Vec3::Z * 5.2, 6, 5.3, 5.1, 0.3);
    dark_plate(b);
    b.prism(Vec3::Z * 5.5, 6, 5.0, 4.6, 0.25);
    for k in 0..3 {
        b.yawed(Vec3::ZERO, (120.0 * k as f32).to_radians(), |b| {
            buttress(b, 4.0, 3.2, 4.6, 10.6, 2.0, 0.9);
            dark_plate(b);
            armour(
                b,
                &Frame::new(v3(4.6, 0.0, 4.7), v3(1.0, 0.0, -0.6), v3(0.6, 0.0, 1.0)),
                &swept(6.2, 0.9, 0.0, 0.55),
                0.3,
            );
            armour(
                b,
                &Frame::new(v3(4.9, 0.0, 5.0), v3(1.0, 0.0, -0.6), v3(0.6, 0.0, 1.0)),
                &swept(3.0, 0.6, 0.0, 0.5),
                0.22,
            );
            if fine {
                // The buttress's toe shod in bronze, and a rib up its flank.
                metal(b);
                b.block(v3(9.8, -1.1, 0.0), v3(10.9, 1.1, 0.5));
                for y in [-1.25, 1.25] {
                    strut(b, v3(5.0, y, 0.5), v3(8.6, y * 0.8, 0.4), 0.14);
                }
            }
            if fine {
                face_battens(b);
            }
            buttress_marks(b);
        });
        // The bay between two buttresses, turned to face out.
        b.yawed(Vec3::ZERO, (60.0 + 120.0 * k as f32).to_radians(), |b| {
            dark_plate(b);
            // The bay's plate, set back into the face, an intake under it.
            // The face leans in as it rises (the keep narrows): the plate leans with it.
            armour(
                b,
                &Frame::new(v3(5.02, 0.0, 2.9), v3(-0.164, 0.0, 1.0), Vec3::X),
                &[[-2.2, -1.9], [-2.2, 1.9], [1.9, 1.5], [1.9, -1.5]],
                0.18,
            );
            if fine {
                metal(b);
                b.block(v3(5.15, -1.5, 0.6), v3(5.7, 1.5, 1.25));
                dark_plate(b);
                for y in [-1.0, -0.5, 0.0, 0.5, 1.0] {
                    b.block(v3(5.6, y - 0.08, 0.65), v3(5.85, y + 0.08, 1.2));
                }
                strut(b, v3(5.5, -2.2, 0.5), v3(5.0, -2.0, 4.7), 0.2);
                strut(b, v3(5.5, 2.2, 0.5), v3(5.0, 2.0, 4.7), 0.2);
                bay_fittings(b);
            }
            bay_marks(b);
        });
    }
    // The race the turret rides: a dark ring, a bronze bearing inside it.
    dark_plate(b);
    hoop(b, Vec3::Z * 5.85, 4.5, 0.6, 0.3, segs);
    metal(b);
    hoop(b, Vec3::Z * 5.9, 3.85, 0.7, 0.3, segs);
}

/// The face a bay's plate leans with, its outer surface `h` out (the bay's frame, +x out):
/// `s` up the face, `t` across it toward -y.
fn bay_face(h: f32) -> Frame {
    Frame::new(v3(5.02 + h, 0.0, 2.9), v3(-0.164, 0.0, 1.0), Vec3::X)
}

/// What stands on a bay's plate: a chevron applique high across it, a framed access cover
/// low on one side, a batten up the middle between them.
fn bay_fittings(b: &mut MeshBuilder) {
    dark_plate(b);
    let face = bay_face(0.18);
    armour(
        b,
        &face,
        &[
            [0.4, -1.25],
            [0.4, 1.25],
            [1.55, 1.0],
            [1.75, 0.0],
            [1.55, -1.0],
        ],
        0.1,
    );
    armour(
        b,
        &face,
        &[[-1.52, -1.2], [-1.52, -0.2], [-0.51, -0.2], [-0.51, -1.2]],
        0.06,
    );
    armour(
        b,
        &face,
        &[[-2.1, -0.09], [-2.1, 0.09], [0.35, 0.09], [0.35, -0.09]],
        0.12,
    );
}

/// Thin battens up the keep's face either side of a buttress (in its frame), at the face's
/// corners, leaning with it.
fn face_battens(b: &mut MeshBuilder) {
    dark_plate(b);
    for y in [-2.7, 2.7] {
        armour(
            b,
            &Frame::new(v3(5.02, y, 2.6), v3(-0.164, 0.0, 1.0), Vec3::X),
            &[[-2.0, -0.12], [-2.0, 0.12], [2.0, 0.1], [2.0, -0.1]],
            0.12,
        );
    }
}

/// What a bay's plate carries, drawn by its surface (in the bay's frame, +x out; the plate
/// leans in as it rises, so each box spans its depth there): a bolt ring round the applique,
/// the access cover's seam and handle, a lamp slot down either edge, and bolts along the
/// footing under it. Features, not grooves or ribs.
fn bay_marks(b: &mut MeshBuilder) {
    b.mark(v3(4.9, -1.3, 3.3), v3(5.45, 1.3, 4.6), mark::PANEL, 0.0);
    b.mark(v3(5.1, 0.2, 1.4), v3(5.7, 1.2, 2.4), mark::ACCESS, 0.0);
    for y in [-1.45f32, 1.45] {
        b.mark(
            v3(4.95, y - 0.1, 1.4),
            v3(5.6, y + 0.1, 3.6),
            mark::LAMP,
            0.0,
        );
    }
    b.mark(v3(5.9, -2.0, 0.15), v3(6.8, 2.0, 0.42), mark::BOLTS, 0.0);
}

/// What a buttress carries (in its frame, running out along +x): a lapped edge down each
/// flank, bolts down the plate on its back, an access cover on one flank by the keep, and a
/// vented panel on the keep's face either side of it, inside the battens.
fn buttress_marks(b: &mut MeshBuilder) {
    for (y0, y1) in [(1.75, 2.5), (-2.5, -1.75)] {
        b.mark(v3(4.6, y0, 2.6), v3(5.3, y1, 4.0), mark::VENT, 0.0);
    }
    for (y0, y1) in [(1.0, 1.75), (-1.75, -1.0)] {
        b.mark(v3(5.5, y0, 0.6), v3(8.5, y1, 1.5), mark::LAP, 0.0);
    }
    b.mark(v3(5.2, -0.5, 1.9), v3(9.0, 0.5, 4.6), mark::BOLTS, 0.0);
    b.mark(v3(4.6, 1.2, 1.3), v3(5.9, 1.9, 2.7), mark::ACCESS, 0.0);
}

/// The turret's deck: a stepped plinth, a wall either side of the gun with a plate swept
/// back down its outside, and a bustle behind carrying the owner's colour as a mark.
pub(super) fn deck(b: &mut MeshBuilder) {
    let fine = b.fine();
    step(b, 6, 4.0, 3.6, 5.7, 0.45);
    step(b, 6, 3.6, 3.3, 6.15, 0.3);
    b.mirror_y(|b| {
        dark_plate(b);
        b.block(v3(-2.8, 2.2, 6.2), v3(1.6, 2.75, 8.9));
        armour(
            b,
            &Frame::new(v3(1.8, 2.75, 8.2), -Vec3::X, Vec3::Y),
            &[[0.0, -1.0], [0.0, 0.9], [3.6, 0.8], [5.4, 0.2], [3.6, -1.2]],
            0.28,
        );
        armour(
            b,
            &Frame::new(v3(1.2, 3.0, 7.0), -Vec3::X, Vec3::Y),
            &[
                [0.0, -0.7],
                [0.0, 0.5],
                [3.0, 0.4],
                [4.2, -0.2],
                [2.8, -0.8],
            ],
            0.2,
        );
        if fine {
            metal(b);
            collar(b, v3(0.0, 2.2, 7.0), Vec3::Y, 0.75, 0.2);
        }
    });
    if fine {
        // A lamp line along each wall's lower plate.
        for y in [-1.0f32, 1.0] {
            let (lo, hi) = ((y * 3.15).min(y * 3.27), (y * 3.15).max(y * 3.27));
            b.mark(v3(-2.0, lo, 6.75), v3(0.8, hi, 6.95), mark::LAMP, 0.0);
        }
    }
    // The bustle: the gun's plant, a keeled block behind the walls.
    dark_plate(b);
    hull_x(
        b,
        &[
            [-4.6, 2.6, 1.4, 7.2],
            [-3.6, 3.6, 1.9, 7.3],
            [-2.4, 3.8, 2.0, 7.3],
        ],
        &CHAMFERED,
    );
    b.mark(v3(-4.0, -0.9, 8.2), v3(-2.6, 0.9, 8.35), mark::TEAM, 0.0);
    if fine {
        b.mark(v3(-4.75, -1.0, 6.7), v3(-4.45, 1.0, 7.7), mark::GRILLE, 0.0);
    }
}
