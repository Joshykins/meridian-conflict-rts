//! Corona: tech 2 tactical missile defence, and the Corona II it upgrades to. A tripod of
//! raked legs stands round a dark capacitor slotted with the missile-defence red, and
//! swept arms carry two of the navy's laser heads (`pd_laser`); a tracking radar turns
//! on top. Tech 3 adds a second, higher set of arms with two more heads over a taller
//! capacitor, and the tech 2 tower shows them rising while it upgrades (`kit`). The heads
//! stand where the unit files' `anti_missile_mounts` say the beams leave: keep
//! `T2_HEADS`, `T3_HEADS` and the data in step.

use super::naval::pd_laser;
use super::parts::*;
use super::structures::kit;
use crate::builder::{ngon, MeshBuilder, Section};
use crate::material::*;
use crate::part;
use glam::Vec3;

/// The laser heads' centres: the tech 2 pair, then the pair tech 3 adds
/// (`anti_missile_mounts` in structures.ron).
const T2_HEADS: [Vec3; 2] = [Vec3::new(0.0, 2.8, 10.6), Vec3::new(0.0, -2.8, 10.6)];
const T3_HEADS: [Vec3; 2] = [Vec3::new(2.8, 0.0, 13.4), Vec3::new(-2.8, 0.0, 13.4)];
/// Laser head size (the navy's heads are 0.72).
const HEAD: f32 = 0.72;

pub(super) fn missile_defense(b: &mut MeshBuilder, tech: u8) {
    if b.coarse() {
        coarse(b, tech);
        return;
    }
    plinth(b, 3.4, 1.6, 0.8);
    team_panel(b, v3(-1.6, 0.0, 1.6), v2(0.9, 1.5));
    // The tripod.
    b.radial(3, |b| {
        b.paint(PLATING);
        b.beam(
            v3(4.4, 0.0, 0.3),
            v3(1.1, 0.0, 9.6),
            v2(1.0, 0.8),
            v2(0.6, 0.5),
        );
        b.paint(ACCENT);
        b.prism(v3(4.5, 0.0, 0.0), 6, 0.9, 0.8, 0.8);
        if b.fine() {
            b.paint(METAL);
            b.beam(
                v3(3.3, 0.0, 3.2),
                v3(-1.6, 2.9, 3.2),
                v2(0.2, 0.2),
                v2(0.2, 0.2),
            );
        }
    });
    // The capacitor: a dark drum with red slots, a white cap.
    b.paint(PLATING_DARK);
    b.prism(v3(0.0, 0.0, 1.6), b.sides(10), 1.3, 1.1, 8.0);
    if b.fine() {
        b.radial(5, |b| {
            b.paint(GLOW_LASER);
            b.cuboid(v3(1.22, 0.0, 5.4), v3(0.12, 0.35, 4.6));
        });
    } else {
        coolant(b, 5.4, 1.2);
    }
    b.paint(PLATING);
    b.prism(v3(0.0, 0.0, 9.6), b.sides(10), 1.4, 1.1, 0.8);
    // Swept arms out to the heads.
    for at in T2_HEADS {
        b.paint(PLATING);
        b.beam(
            v3(0.0, 0.0, 9.9),
            v3(at.x - 0.6, at.y, at.z - 0.9),
            v2(0.8, 0.7),
            v2(0.5, 0.5),
        );
        pd_laser(b, at, HEAD, Some(at.z - 0.9));
    }
    kit(b, tech, 3, 0.3, |b| {
        b.paint(PLATING_DARK);
        b.prism(v3(0.0, 0.0, 10.4), b.sides(10), 1.0, 0.8, 2.2);
        coolant(b, 11.4, 1.0);
        b.paint(PLATING);
        b.prism(v3(0.0, 0.0, 12.6), b.sides(10), 1.1, 0.9, 0.5);
        for at in T3_HEADS {
            b.paint(PLATING);
            b.beam(
                v3(0.0, 0.0, 12.8),
                v3(at.x, at.y * 0.8, at.z - 0.9),
                v2(0.7, 0.6),
                v2(0.45, 0.45),
            );
            pd_laser(b, at, HEAD, Some(at.z - 0.9));
        }
    });
    radar(b, if tech >= 3 { 13.1 } else { 10.4 }, 2.4);
}

/// The dark pad and an octagonal plinth pinching in to `neck` at `top`.
fn plinth(b: &mut MeshBuilder, r: f32, top: f32, neck: f32) {
    b.paint(ACCENT);
    b.loft_z(
        &ngon(8, 5.4),
        &[Section::new(0.0, 1.0), Section::new(0.4, 0.97)],
    );
    b.paint(PLATING);
    b.loft_z(
        &ngon(8, r),
        &[
            Section::new(0.3, 1.0),
            Section::new(top * 0.6, 0.94),
            Section::new(top, neck),
        ],
    );
}

/// A band of the missile-defence red round the column at `z`: coolant lit by the lasers.
fn coolant(b: &mut MeshBuilder, z: f32, r: f32) {
    if !b.fine() {
        return;
    }
    b.paint(GLOW_LASER);
    b.prism(v3(0.0, 0.0, z), b.sides(8), r, r, 0.22);
}

/// The tracking radar: a panel on a short mast at `z`, turning.
fn radar(b: &mut MeshBuilder, z: f32, span: f32) {
    b.set_spinner_pivot(v3(0.0, 0.0, z));
    b.with_part(part::SPINNER, |b| {
        b.paint(METAL);
        b.prism(v3(0.0, 0.0, z), b.sides(6), 0.34, 0.28, 1.2);
        b.pitched(v3(0.35, 0.0, z + 1.6), 0.35, |b| {
            b.paint(PLATING);
            b.cuboid(Vec3::ZERO, v3(0.22, span, span * 0.55));
            if b.fine() {
                b.paint(ACCENT);
                b.cuboid(v3(-0.2, 0.0, 0.0), v3(0.2, span * 0.8, span * 0.36));
                b.paint(METAL);
                b.cylinder_between(v3(0.1, 0.0, 0.0), v3(1.1, 0.0, -0.05), 0.09, 0.07, 6);
            }
        });
    });
}

/// The far LOD, shared: plinth, column, the heads' red on top of a bar.
fn coarse(b: &mut MeshBuilder, tech: u8) {
    b.paint(PLATING);
    b.frustum_open(
        v3(0.0, 0.0, 0.0),
        v2(8.4, 8.4),
        v2(4.6, 4.6),
        3.4,
        v2(0.0, 0.0),
    );
    b.paint(PLATING_DARK);
    b.cuboid_open(v3(0.0, 0.0, 6.4), v3(1.7, 1.7, 6.0));
    b.paint(PLATING);
    b.cuboid_open(v3(0.0, 0.0, 10.2), v3(1.3, 7.0, 1.6));
    b.paint(GLOW_LASER);
    for at in T2_HEADS {
        b.decal(v3(at.x, at.y, 11.02), v2(1.2, 1.2));
    }
    if tech >= 3 {
        b.paint(PLATING);
        b.cuboid_open(v3(0.0, 0.0, 13.0), v3(7.0, 1.3, 1.6));
        b.paint(GLOW_LASER);
        for at in T3_HEADS {
            b.decal(v3(at.x, at.y, 13.82), v2(1.2, 1.2));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{build_model_fitted, part};

    #[test]
    fn the_corona_turns_its_radar_stays_in_its_lot_and_stands_taller_at_tech_3() {
        let built = |tech| {
            let model = build_model_fitted("missile_defense", 6.0, 13.0, tech, &[]).unwrap();
            let lod = &model.lods[0];
            assert!(lod.vertices.iter().any(|v| v.part == part::SPINNER));
            for v in &lod.vertices {
                assert!(v.pos[0].abs() < 6.0 && v.pos[1].abs() < 6.0, "{:?}", v.pos);
            }
            // What stands built, not counting an upgrade's rising pieces.
            lod.vertices
                .iter()
                .filter(|v| v.rig & crate::rig::UPGRADE == 0)
                .count()
        };
        let (two, three) = (built(2), built(3));
        assert!(three > two, "{two} {three}");
    }
}
