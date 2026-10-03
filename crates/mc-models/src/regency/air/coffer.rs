//! The Coffer, the Regency's tech 2 light transport: a box of a hold, open at the stern,
//! slung between lift bells. It sets the hold down flat on the ground and the cargo drives
//! in and out at the stern. Raised on a lot by the Exarch and the Artificers, as ARC's
//! Courier is by its builders. A long plated sponson either side of the hold, three lift
//! bells under each, joined to the hold's roof by swept yokes over bronze shafts (the
//! user's pick of two, 2026-10-02); a ridge of plates down the roof.
//!
//! Finish (docs/STYLE.md "The Regency look"): dark plates lapped over bronze, red optics
//! across the cab's brow, red under the bells. No jets, no rotors: the lift bells carry it
//! (`super::super::lift`). Authored at blueprint scale (radius 40, height 22): `HALF`,
//! `CLEAR`, `FRONT` and `HINGE` are `regency_t2_transport`'s `transport` numbers.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::library::ModelDef;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::lift::bell;
use super::super::machine::{red_slot, strut, Course, Frame};
use super::{chevron, flat};

const RADIUS: f32 = 40.0;
const HEIGHT: f32 = 22.0;

pub(crate) const MODELS: &[ModelDef] = &[ModelDef::new("regency_coffer", RADIUS, HEIGHT, sponsons)];

/// The hold inside: half its width (the unit file's `width` over two), its clear height
/// (`clearance`), and its front wall. Cargo stows toward the front (`hold`).
const HALF: f32 = 9.0;
const CLEAR: f32 = 13.0;
const FRONT: f32 = 18.0;
/// The ramp's hinge, where the hold opens at the stern (`ramp`'s first number).
const HINGE: f32 = -30.0;
/// The hold's walls and roof, how thick.
const WALL: f32 = 2.4;
const ROOF: f32 = CLEAR + 2.6;

/// What both slings share: the hold, open at the stern; the cab on its front with the
/// optics across its brow; the plates along its walls; the mark on its roof.
fn hold(b: &mut MeshBuilder) {
    if b.coarse() {
        dark_plate(b);
        b.block(
            v3(HINGE, -HALF - WALL, 0.0),
            v3(FRONT + WALL, HALF + WALL, ROOF),
        );
        flat(
            b,
            &[
                [FRONT + WALL, -HALF],
                [FRONT + 12.0, 0.0],
                [FRONT + WALL, HALF],
            ],
            ROOF - 3.0,
        );
        chevron(b, v3(4.0, 0.0, ROOF + 0.05), 9.0);
        return;
    }
    let outer = HALF + WALL;
    dark_plate(b);
    b.with_facets(|b| {
        b.mirror_y(|b| {
            // A wall, its top edge chamfered in to the roof.
            b.extrude_x(
                &[
                    [HALF, 0.3],
                    [outer, 0.3],
                    [outer, ROOF - 1.6],
                    [outer - 1.4, ROOF],
                    [HALF, ROOF],
                ],
                HINGE,
                FRONT,
            );
        });
        b.block(v3(HINGE, -HALF, CLEAR), v3(FRONT, HALF, ROOF));
        b.block(v3(FRONT, -outer, 0.3), v3(FRONT + WALL, outer, ROOF));
    });
    // The floor, and a sill across the stern where the cargo rolls out.
    seam(b);
    b.block(v3(HINGE, -HALF, 0.0), v3(FRONT, HALF, 0.6));
    metal(b);
    b.block(v3(HINGE - 3.5, -HALF, 0.0), v3(HINGE, HALF, 0.35));
    // A bronze frame round the stern's mouth.
    b.with_facets(|b| {
        b.mirror_y(|b| {
            b.block(
                v3(HINGE - 0.6, HALF - 0.3, 0.3),
                v3(HINGE + 0.6, outer + 0.3, ROOF),
            )
        });
        b.block(
            v3(HINGE - 0.6, -outer, CLEAR - 0.4),
            v3(HINGE + 0.6, outer, ROOF + 0.3),
        );
    });
    cab(b);
    // Plates lapped back along each wall over the bronze.
    b.mirror_y(|b| {
        dark_plate(b);
        let f = Frame::new(v3(FRONT - 1.0, outer, ROOF - 4.0), -Vec3::X, Vec3::Y);
        let (count, step) = if b.fine() { (5, 9.4) } else { (3, 15.0) };
        Course {
            count,
            step,
            len: 11.0,
            half: 2.6,
            tip: -0.6,
            thick: 0.6,
            tail: 2.0,
        }
        .lay(b, &f);
        if b.fine() {
            let f = Frame::new(v3(FRONT - 4.0, outer, 4.5), -Vec3::X, Vec3::Y);
            Course {
                count: 4,
                step: 11.0,
                len: 12.0,
                half: 1.8,
                tip: 0.6,
                thick: 0.5,
                tail: 0.0,
            }
            .lay(b, &f);
        }
    });
    chevron(b, v3(8.0, 0.0, ROOF + 0.02), 7.0);
}

/// The cab on the hold's front: a wedge raked forward to a prow blade, a brow across its
/// top with the optics in a row under it.
fn cab(b: &mut MeshBuilder) {
    let x0 = FRONT + WALL;
    dark_plate(b);
    b.with_facets(|b| {
        b.loft(
            &[
                vec![
                    v3(x0, -10.5, 1.5),
                    v3(x0, 10.5, 1.5),
                    v3(x0, 10.0, ROOF - 0.5),
                    v3(x0, -10.0, ROOF - 0.5),
                ],
                vec![
                    v3(x0 + 7.0, -7.0, 3.0),
                    v3(x0 + 7.0, 7.0, 3.0),
                    v3(x0 + 5.0, 7.5, ROOF - 2.5),
                    v3(x0 + 5.0, -7.5, ROOF - 2.5),
                ],
                vec![
                    v3(x0 + 13.0, -1.5, 5.0),
                    v3(x0 + 13.0, 1.5, 5.0),
                    v3(x0 + 11.0, 1.5, 9.0),
                    v3(x0 + 11.0, -1.5, 9.0),
                ],
            ],
            true,
            true,
        )
    });
    if b.fine() {
        // The brow, jutting over the optics.
        b.with_facets(|b| {
            b.loft(
                &[
                    vec![
                        v3(x0 - 1.0, -11.0, ROOF - 1.0),
                        v3(x0 - 1.0, 11.0, ROOF - 1.0),
                        v3(x0 - 1.0, 11.0, ROOF + 0.6),
                        v3(x0 - 1.0, -11.0, ROOF + 0.6),
                    ],
                    vec![
                        v3(x0 + 6.5, -6.5, ROOF - 2.4),
                        v3(x0 + 6.5, 6.5, ROOF - 2.4),
                        v3(x0 + 6.0, 6.0, ROOF - 1.6),
                        v3(x0 + 6.0, -6.0, ROOF - 1.6),
                    ],
                ],
                true,
                true,
            )
        });
    }
    for y in [-4.5f32, -1.5, 1.5, 4.5] {
        red_slot(
            b,
            v3(x0 + 5.4, y, ROOF - 3.6),
            v3(0.9, 0.0, 0.45),
            Vec3::Y,
            2.0,
            0.5,
        );
    }
}

/// A sponson either side, its bells under it, yoked to the roof.
const SPONSON_Y: f32 = 21.0;

fn sponsons(b: &mut MeshBuilder, _tech: u8) {
    hold(b);
    b.mirror_y(|b| {
        let y = SPONSON_Y;
        dark_plate(b);
        if b.coarse() {
            b.block(v3(-26.0, y - 4.5, 1.0), v3(24.0, y + 4.5, 9.0));
            return;
        }
        // The sponson: a long faceted body, pointed fore and aft.
        b.with_facets(|b| {
            b.loft(
                &[
                    vec![
                        v3(-30.0, y, 4.0),
                        v3(-30.0, y + 0.1, 5.0),
                        v3(-30.0, y, 6.0),
                        v3(-30.0, y - 0.1, 5.0),
                    ],
                    vec![
                        v3(-20.0, y - 4.5, 2.0),
                        v3(-20.0, y + 4.5, 2.0),
                        v3(-20.0, y + 3.5, 9.5),
                        v3(-20.0, y - 3.5, 9.5),
                    ],
                    vec![
                        v3(16.0, y - 4.5, 2.0),
                        v3(16.0, y + 4.5, 2.0),
                        v3(16.0, y + 3.5, 9.5),
                        v3(16.0, y - 3.5, 9.5),
                    ],
                    vec![
                        v3(30.0, y, 4.5),
                        v3(30.0, y + 0.6, 5.5),
                        v3(30.0, y, 7.0),
                        v3(30.0, y - 0.6, 5.5),
                    ],
                ],
                true,
                true,
            )
        });
        let f = Frame::new(v3(18.0, y, 9.5), -Vec3::X, Vec3::Z);
        let (count, step) = if b.fine() { (5, 9.0) } else { (2, 18.0) };
        Course {
            count,
            step,
            len: 10.5,
            half: 3.6,
            tip: 0.0,
            thick: 0.6,
            tail: 3.0,
        }
        .lay(b, &f);
        for x in [-18.0f32, 0.0, 16.0] {
            bell(b, v3(x, y, 0.3), 3.2, 1.7);
            // A yoke from the roof's edge out and down to the sponson's back, bronze
            // shafts under its plate.
            let (a, c) = (
                v3(x + 2.0, HALF + WALL - 1.0, ROOF - 1.0),
                v3(x - 2.0, y - 1.0, 9.6),
            );
            strut(b, a, c, 1.3);
            if b.fine() {
                metal(b);
                b.cylinder_between(a - Vec3::Z * 2.2, c - Vec3::Z * 1.4, 0.55, 0.55, 8);
                b.cylinder_between(a - v3(2.0, 0.0, 2.2), c - v3(2.0, 0.0, 1.4), 0.4, 0.4, 8);
            }
        }
    });
    keel_ridge(b, ROOF, 5.0);
}

/// A ridge of plates lapped back down the roof's middle, `rise` high, over a bronze
/// conduit: what stands highest on the hold.
fn keel_ridge(b: &mut MeshBuilder, z: f32, rise: f32) {
    if b.coarse() {
        dark_plate(b);
        b.block(v3(-24.0, -0.8, z), v3(16.0, 0.8, z + rise));
        return;
    }
    metal(b);
    b.cylinder_between(
        v3(FRONT, 0.0, z + 0.8),
        v3(HINGE + 2.0, 0.0, z + 0.8),
        1.0,
        1.0,
        b.sides(8),
    );
    dark_plate(b);
    let count = if b.fine() { 4 } else { 2 };
    for k in 0..count {
        let x = FRONT - 2.0 - k as f32 * 11.0;
        b.with_facets(|b| {
            b.extrude_y(
                &[
                    [x, z],
                    [x - 3.0, z + rise],
                    [x - 12.0, z + rise * 0.7],
                    [x - 9.0, z],
                ],
                -0.5,
                0.5,
            )
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build_model, rig};

    #[test]
    fn fits_the_regency_checks() {
        for key in MODELS.iter().map(|m| m.key) {
            crate::regency::check(key, RADIUS, HEIGHT, None, &[]);
            let model = build_model(key).unwrap();
            assert!(model.lifts.len() >= 4, "{key}: its lift bells");
            for (lod, mesh) in model.lods.iter().enumerate() {
                // It stands on its lot: 7 by 5 cells.
                let (x, y) = mesh.vertices.iter().fold((0.0f32, 0.0f32), |(x, y), v| {
                    (x.max(v.pos[0].abs()), y.max(v.pos[1].abs()))
                });
                assert!(x <= 42.0 && y <= 30.0, "{key} lod{lod}: {x} x {y}");
                if lod == 2 {
                    continue;
                }
                // The hold is clear from the floor to its roof, wall to wall, from the
                // stern's mouth to its front, so cargo drives in and stands in it.
                let inside = mesh.vertices.iter().find(|v| {
                    v.rig & rig::UPGRADE == 0
                        && v.pos[1].abs() < HALF - 0.05
                        && (0.65..CLEAR - 0.05).contains(&v.pos[2])
                        && (HINGE + 0.7..FRONT - 0.05).contains(&v.pos[0])
                });
                assert!(
                    inside.is_none(),
                    "{key} lod{lod}: {:?} in the hold",
                    inside.map(|v| v.pos)
                );
            }
        }
    }

    #[test]
    fn the_unit_files_hold_is_the_models() {
        let (b, id) = super::super::tests::blueprint("regency_t2_transport");
        let bp = b.unit(id);
        assert_eq!(bp.visual.mesh, "regency_coffer");
        assert!(
            (bp.radius.to_f32() - RADIUS).abs() < 1e-3
                && (bp.height.to_f32() - HEIGHT).abs() < 1e-3
        );
        assert_eq!(bp.footprint, (7, 5));
        assert!(bp.warp.is_none(), "the Regency do not jump");
        let t = bp.transport.expect("it carries");
        assert!((t.width.to_f32() - HALF * 2.0).abs() < 1e-3);
        assert!((t.clearance.to_f32() - CLEAR).abs() < 1e-3);
        assert!((t.hinge.to_f32() - HINGE).abs() < 1e-3);
        assert!(t.lip.to_f32() < HINGE && t.floor.to_f32() == 0.0);
        let hold = t.hold.x.to_f32();
        assert!(hold > HINGE && hold < FRONT, "cargo stows inside the hold");
    }
}
