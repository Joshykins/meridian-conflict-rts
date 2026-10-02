//! The Wake, the Regency's tech 3 wake tank: a long, low armoured hull on four lift bells,
//! a low turret over its middle, and in front of the turret one wide, flat projector that
//! gathers its plasma across its mouth and lets it go as a fan (the cone the sim strikes,
//! `mc-sim` `wake.rs`). Its `muzzle` is the middle of the charge in front of the mouth.
//!
//! The hull is a pointed wedge, lofted in flat facets, two courses of plates swept back down
//! each flank into spikes past its stern, a glacis plate over the nose. The turret is a
//! plated block on a bronze race, red optics either side of its face, bronze capacitor
//! drums out of its back. The projector flares from the turret to its mouth, bronze pinch
//! collars round its throat, and holds the charge between two vanes splayed out along the
//! cone's edges, lit red inside.
//!
//! Finish (docs/STYLE.md "The Regency look"): dark plates, seam-dark joints, dark bronze on
//! the machinery, red optics and a red-lit mouth. No violet: it does not build.
//!
//! Rig: a hovercraft (`MeshBuilder::set_hover`): the shader heaves the hull over its lift
//! bells (`part::LOCOMOTION`), each marked for its red plasma (`add_lift`). The turret turns
//! about the unit's middle; the projector pitches about its trunnion (`rig::ARM_GUN`). The
//! numbers match `regency_t3_wake_tank` in `data/factions/regency/units/land_t3.ron`.

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::material::*;
use crate::{part, rig};

use super::commander::form::blade;
use super::kit::{dark_plate, metal, seam, v3};
use super::machine::{collar, hoop, red_slot};

/// The projector's trunnion and the middle of its charge: the unit file's weapon `pivot`
/// and `muzzle`.
const PIVOT: Vec3 = Vec3::new(1.4, 0.0, 3.4);
const MUZZLE: Vec3 = Vec3::new(8.2, 0.0, 3.4);
/// How far round the charge its plates stand: none closer than 0.4 of it.
#[cfg(test)]
const HOLD: f32 = 2.0;
/// Where the turret turns on the hull.
const DECK: f32 = 2.85;
/// The lift drums under the left side (x, y); the right's are their mirrors.
const LIFTS: [(f32, f32); 2] = [(4.2, 2.6), (-4.4, 2.9)];
/// Where each lift bell's mouth opens.
const MOUTH: f32 = 0.25;

/// The hull's plan: a pointed wedge, its stern cut in between two trailing points.
const HULL: [[f32; 2]; 10] = [
    [7.4, 0.0],
    [6.2, 2.2],
    [2.6, 4.0],
    [-4.6, 4.3],
    [-7.2, 3.0],
    [-6.4, 0.0],
    [-7.2, -3.0],
    [-4.6, -4.3],
    [2.6, -4.0],
    [6.2, -2.2],
];

/// The turret's plan: a blunt face, swept back to a point.
const TURRET: [[f32; 2]; 8] = [
    [3.2, 0.0],
    [2.4, 2.0],
    [-1.8, 2.6],
    [-4.2, 1.4],
    [-4.6, 0.0],
    [-4.2, -1.4],
    [-1.8, -2.6],
    [2.4, -2.0],
];

pub(super) fn wake_tank(b: &mut MeshBuilder, _tech: u8) {
    b.set_hover();
    b.set_turret_pivot(v3(0.0, 0.0, DECK));
    b.set_arm_pivot(PIVOT);
    b.set_recoil(PIVOT, MUZZLE, 0.5);
    b.set_dust_line(0.8);
    if b.coarse() {
        coarse(b);
        return;
    }
    hull(b);
    b.mirror_y(flank);
    lifts(b);
    b.with_part(part::TURRET, |b| {
        turret(b);
        b.with_limb(rig::ARM_GUN, projector);
    });
}

/// Far off: the wedge, the turret a block with the team colour on it, the projector a
/// flat bar that still pitches.
fn coarse(b: &mut MeshBuilder) {
    dark_plate(b);
    let wedge = [
        [7.4, 0.0],
        [2.6, 4.2],
        [-7.2, 3.0],
        [-7.2, -3.0],
        [2.6, -4.2],
    ];
    b.loft_z(&wedge, &[Section::new(0.9, 0.9), Section::new(2.8, 0.75)]);
    b.with_part(part::LOCOMOTION, |b| {
        seam(b);
        b.face(&[v3(5.0, 0.0, 0.5), v3(-5.5, -3.0, 0.5), v3(-5.5, 3.0, 0.5)]);
    });
    b.with_part(part::TURRET, |b| {
        dark_plate(b);
        let block = [[3.2, 0.0], [-1.8, 2.6], [-4.6, 0.0], [-1.8, -2.6]];
        b.loft_z(
            &block,
            &[
                Section::new(DECK, 1.0),
                Section::new(4.5, 0.7).shifted(-0.5, 0.0),
            ],
        );
        b.paint(TEAM);
        b.face(&[
            v3(0.4, 0.0, 4.52),
            v3(-2.0, 0.8, 4.52),
            v3(-2.0, -0.8, 4.52),
        ]);
        b.with_limb(rig::ARM_GUN, |b| {
            dark_plate(b);
            b.beam(
                PIVOT,
                v3(7.4, 0.0, PIVOT.z),
                Vec2::new(2.0, 1.4),
                Vec2::new(2.4, 1.6),
            );
        });
    });
}

/// The hull: a flat-faceted wedge over a seam-dark belly, a glacis plate over the nose and
/// a red slot along each side of it, a bronze spine showing between the turret race and
/// the stern.
fn hull(b: &mut MeshBuilder) {
    seam(b);
    b.loft_z(&HULL, &[Section::new(0.75, 0.82), Section::new(1.0, 0.86)]);
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &HULL,
            &[
                Section::new(0.95, 0.88),
                Section::new(1.6, 1.0),
                Section::new(2.45, 0.9),
                Section::new(DECK, 0.62).shifted(-0.7, 0.0),
            ],
        );
    });
    // The glacis: a thick plate over the nose, its point back toward the turret.
    blade(
        b,
        v3(6.9, 0.0, 1.85),
        v3(-1.0, 0.0, 0.28),
        v3(0.3, 0.0, 1.0),
        4.4,
        1.6,
        0.0,
        0.35,
    );
    b.mirror_y(|b| {
        red_slot(
            b,
            v3(5.0, 2.62, 1.55),
            v3(0.45, 1.0, 0.0),
            v3(-1.0, 0.45, 0.0),
            1.6,
            0.14,
        );
    });
    // The spine behind the race: bronze workings between the stern's plates.
    if b.fine() {
        metal(b);
        let sides = b.sides(8);
        b.cylinder_between(v3(-3.4, 0.0, 2.75), v3(-6.0, 0.0, 2.2), 0.42, 0.34, sides);
        b.mirror_y(|b| {
            b.cylinder_between(v3(-3.2, 0.9, 2.6), v3(-5.8, 1.3, 2.0), 0.22, 0.2, 6);
        });
    }
    b.paint(TEAM);
    b.face(&[v3(-4.4, 0.0, 2.88), v3(-5.6, 0.7, 2.6), v3(-5.6, -0.7, 2.6)]);
}

/// The left flank's plates: two courses swept back, the second running out past the stern
/// into a spike.
fn flank(b: &mut MeshBuilder) {
    dark_plate(b);
    blade(
        b,
        v3(4.8, 3.3, 1.95),
        v3(-1.0, 0.1, -0.06),
        v3(0.0, 1.0, 0.75),
        5.6,
        0.85,
        0.2,
        0.34,
    );
    blade(
        b,
        v3(0.4, 3.9, 1.85),
        v3(-1.0, 0.08, -0.04),
        v3(0.0, 1.0, 0.7),
        7.6,
        0.9,
        0.4,
        0.36,
    );
}

/// The lift: four bronze collars under the hull, a lift bell in each, a dark core ringed in
/// red where the lift leaves it, marked for its plasma.
fn lifts(b: &mut MeshBuilder) {
    let drum = |b: &mut MeshBuilder, x: f32, y: f32, r: f32| {
        b.add_lift(v3(x, y, MOUTH), r * 0.7);
        metal(b);
        let sides = b.sides(12);
        b.prism(v3(x, y, 0.62), sides, r, r * 0.95, 0.36);
        b.with_part(part::LOCOMOTION, |b| {
            seam(b);
            b.prism(v3(x, y, 0.34), sides, r * 0.86, r * 0.84, 0.5);
            b.prism(v3(x, y, MOUTH), sides, r * 0.55, r * 0.7, 0.09);
            if b.fine() {
                b.paint(GLOW_LASER);
                hoop(b, v3(x, y, 0.36), r * 0.7, 0.1, 0.06, 14);
            }
        });
    };
    b.mirror_y(|b| {
        for (x, y) in LIFTS {
            drum(b, x, y, 1.05);
        }
    });
}

/// The turret: a bronze race, a plated block swept back to a point, red optics either side
/// of its face, bronze capacitor drums out of its back, the team colour on top, and the
/// projector's trunnion cheeks.
fn turret(b: &mut MeshBuilder) {
    metal(b);
    let sides = b.sides(16);
    b.prism(v3(0.0, 0.0, DECK - 0.05), sides, 2.75, 2.7, 0.3);
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &TURRET,
            &[
                Section::new(DECK + 0.2, 1.0),
                Section::new(3.95, 0.94),
                Section::new(4.6, 0.62).shifted(-0.6, 0.0),
            ],
        );
    });
    b.mirror_y(|b| {
        red_slot(
            b,
            v3(2.75, 1.05, 3.75),
            v3(1.0, 0.45, 0.0),
            v3(0.45, -1.0, 0.0),
            0.7,
            0.16,
        );
        // A cheek plate either side of the trunnion.
        dark_plate(b);
        b.extrude_y(
            &[
                [PIVOT.x - 1.4, DECK + 0.25],
                [PIVOT.x + 1.6, DECK + 0.25],
                [PIVOT.x + 1.1, PIVOT.z + 0.9],
                [PIVOT.x - 0.9, PIVOT.z + 1.15],
            ],
            1.55,
            1.85,
        );
        if b.fine() {
            // Capacitor drums out of the turret's back.
            metal(b);
            b.cylinder_between(v3(-3.9, 0.85, 3.5), v3(-5.3, 0.85, 3.4), 0.38, 0.34, 10);
            seam(b);
            b.cylinder_between(v3(-5.3, 0.85, 3.4), v3(-5.45, 0.85, 3.39), 0.3, 0.24, 10);
        }
    });
    b.paint(TEAM);
    b.face(&[
        v3(0.5, 0.0, 4.62),
        v3(-2.2, 0.85, 4.62),
        v3(-2.2, -0.85, 4.62),
    ]);
}

/// The projector, in the turret's frame (it pitches about `PIVOT`): a plated throat
/// flaring from the trunnion to a wide mouth, bronze pinch collars round it, a plate over
/// its top, and the plates that hold the charge in front of the mouth, lit red inside.
fn projector(b: &mut MeshBuilder) {
    let z = PIVOT.z;
    // The throat: from a narrow breech at the trunnion out to the mouth, wider than deep.
    let rect = |x: f32, hw: f32, hh: f32| {
        vec![
            v3(x, -hw, z - hh),
            v3(x, hw, z - hh),
            v3(x, hw * 0.86, z + hh),
            v3(x, -hw * 0.86, z + hh),
        ]
    };
    dark_plate(b);
    b.with_facets(|b| {
        b.loft(
            &[
                rect(PIVOT.x - 0.9, 1.0, 0.7),
                rect(3.4, 1.25, 0.72),
                rect(6.4, 2.0, 0.55),
                rect(6.85, 2.1, 0.5),
            ],
            true,
            true,
        );
    });
    // The trunnion, bronze, through both cheeks.
    collar(b, PIVOT, Vec3::Y, 0.55, 3.7);
    // Pinch collars round the throat.
    if b.fine() {
        for (x, hw, hh) in [(3.0, 1.32, 0.8), (4.6, 1.62, 0.72)] {
            metal(b);
            b.loft(
                &[rect(x - 0.18, hw, hh), rect(x + 0.18, hw, hh)],
                true,
                true,
            );
        }
    }
    // The plate over its top, its point back over the trunnion.
    dark_plate(b);
    blade(
        b,
        v3(6.5, 0.0, z + 0.62),
        v3(-1.0, 0.0, 0.08),
        Vec3::Z,
        4.6,
        1.7,
        0.0,
        0.3,
    );
    // The mouth: a red-lit face across the end of the throat.
    b.paint(GLOW_LASER);
    b.face(&[
        v3(6.87, -1.8, z - 0.4),
        v3(6.87, 1.8, z - 0.4),
        v3(6.87, 1.55, z + 0.4),
        v3(6.87, -1.55, z + 0.4),
    ]);
    // Two vanes either side of the charge, splayed out along the cone's edges.
    b.mirror_y(|b| vane(b, z));
}

/// The left vane of the mouth: a plate standing on edge from the mouth's corner,
/// splayed out along the cone's edge, its inner face lit red.
fn vane(b: &mut MeshBuilder, z: f32) {
    let a = 20f32.to_radians();
    let out = v3(a.cos(), a.sin(), 0.0);
    let root = v3(6.0, 1.0, z);
    let tip = root + out * 2.15;
    let thick = v3(-a.sin(), a.cos(), 0.0) * 0.3;
    let ring = |o: Vec3| {
        vec![
            root + o + Vec3::Z * -0.8,
            tip + o + Vec3::Z * -0.55,
            tip + out * 0.25 + o,
            tip + o + Vec3::Z * 0.55,
            root + o + Vec3::Z * 0.8,
        ]
    };
    dark_plate(b);
    b.loft(&[ring(Vec3::ZERO), ring(thick)], true, true);
    b.paint(GLOW_LASER);
    let inner = -thick.normalize() * 0.02;
    b.face(&[
        root + out * 0.4 + inner + Vec3::Z * -0.55,
        root + out * 0.4 + inner + Vec3::Z * 0.55,
        tip - out * 0.2 + inner + Vec3::Z * 0.4,
        tip - out * 0.2 + inner + Vec3::Z * -0.4,
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_model;

    #[test]
    fn fits_the_librarys_checks() {
        super::super::check_charge(
            "regency_wake_tank",
            7.6,
            4.6,
            None,
            &[MUZZLE.to_array()],
            HOLD,
        );
    }

    #[test]
    fn the_unit_files_projector_is_the_models() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&dir).unwrap();
        let bp = blueprints.unit(blueprints.id_of("regency_t3_wake_tank").unwrap());
        let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
        assert_eq!(bp.visual.mesh, "regency_wake_tank");
        let w = &bp.weapons[0];
        assert!(w.cone.is_some(), "the projector fires a cone");
        assert!(v(w.muzzle).distance(MUZZLE) < 1e-3);
        assert!(v(w.pivot.unwrap()).distance(PIVOT) < 1e-3);
        assert!(bp.turret_at.is_none(), "the turret turns about the middle");
        assert_eq!(
            bp.motion.map(|m| m.layer),
            Some(mc_data::MoveLayer::Hover),
            "it rides its lift"
        );

        let model = build_model("regency_wake_tank").unwrap();
        assert!(model.hover);
        assert_eq!(model.lifts.len(), 4);
        assert_eq!(Vec3::from(model.arm_pivot.unwrap()), PIVOT);
        for lod in &model.lods {
            assert!(lod.vertices.iter().all(|v| v.material != GLOW_VIOLET));
        }
    }
}
