//! The Regency's tech 1 tank, the Sledge, the counterpart of ARC's Warden: a plated hull on
//! gravity lift, no legs, crossing water as it crosses land. It is low and wide, a swept
//! sponson either side laid in a course of plates like the Artificer's, four lift bells
//! under them, a glacis on its nose with red optics under its brow, and a low wedge turret
//! carrying a light Pinched-plasmeric Cannon (the Halberd's fork, small: a short bore and
//! two prongs either side of where the charge is gathered).
//!
//! Finish (docs/STYLE.md "The Regency look"): dark plate over dark bronze, red optics, red
//! in the bells' mouths and on the prongs' emitters. No violet: it does not build.
//!
//! Rig: a hovercraft (`set_hover`), every bell's mouth marked for its plasma (`add_lift`).
//! The turret turns about the unit's middle at `TURRET`; the cannon pitches about `PIVOT`
//! (`rig::ARM_GUN`) and gathers its charge at `CHARGE`, the unit file's `muzzle`
//! (`regency_t1_tank` in `data/factions/regency/units/land.ron`).

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::material::*;
use crate::{part, rig};

use super::chassis::{coarse_lift, hull, lift_bell, Hull};
use super::commander::form::blade;
use super::guns::pinch_cannon;
use super::kit::{dark_plate, metal, v3};
use super::machine::{armour, red_slot, Course, Frame};

/// Where the turret turns, the cannon's trunnion and the middle of its charge (the unit
/// file's `pivot` and `muzzle`).
pub(super) const TURRET: Vec3 = Vec3::new(0.0, 0.0, 1.9);
pub(super) const PIVOT: Vec3 = Vec3::new(0.9, 0.0, 2.5);
pub(super) const CHARGE: Vec3 = Vec3::new(4.9, 0.0, 2.5);
/// How far either side of the charge the prongs stand.
pub(super) const HOLD: f32 = 0.75;
/// The cannon's breech and its housing's half width.
const BREECH: f32 = 0.4;
const GUN_R: f32 = 0.42;

/// The low hull's plan, nose round the left to the tail and back up the right.
const LOW: [[f32; 2]; 11] = [
    [4.4, 0.0],
    [3.5, 1.2],
    [1.6, 2.0],
    [-1.8, 2.3],
    [-3.9, 2.6],
    [-3.4, 1.3],
    [-3.4, -1.3],
    [-3.9, -2.6],
    [-1.8, -2.3],
    [1.6, -2.0],
    [3.5, -1.2],
];

/// Far off: a six-sided plan.
const COARSE: [[f32; 2]; 6] = [
    [4.4, 0.0],
    [1.6, 2.1],
    [-3.8, 2.4],
    [-3.4, 0.0],
    [-3.8, -2.4],
    [1.6, -2.1],
];

fn rig(b: &mut MeshBuilder) {
    b.set_hover();
    b.set_turret_pivot(TURRET);
    b.set_arm_pivot(PIVOT);
    b.set_dust_line(0.6);
}

/// The low hull with swept sponsons, its bells and its turret.
pub(super) fn hover_tank(b: &mut MeshBuilder, _tech: u8) {
    rig(b);
    if b.coarse() {
        coarse(b);
        return;
    }
    hull(
        b,
        &Hull {
            plan: &LOW,
            z: [0.62, 0.85, 1.7],
            belly: 0.82,
            top: 0.8,
            back: 0.2,
        },
    );
    glacis(b, 1.45, 1.05);
    b.mirror_y(|b| {
        lift_bell(b, Vec2::new(2.1, 1.3), 0.52, 0.82);
        lift_bell(b, Vec2::new(-2.2, 1.6), 0.56, 0.82);
        // The sponson's course: plates lapped like feathers and swept back and out, the
        // last drawn out past the hull into the spike at its tail.
        dark_plate(b);
        let f = Frame::new(
            v3(2.7, 1.25, 1.62),
            v3(-1.0, 0.24, -0.06),
            v3(0.0, 0.55, 1.0),
        );
        course(b, &f, 3, 1.55, 2.2, 0.6, 0.9);
    });
    b.with_part(part::TURRET, |b| {
        turret(
            b,
            &[[1.3, 0.55], [0.2, 1.25], [-1.6, 1.15], [-2.3, 0.0]],
            (1.85, 2.8),
            0.68,
        );
        gun(b);
    });
}

/// A course of `count` plates laid in `f`, each `len` long and `half` either side, the last
/// running `tail` further into a spike.
fn course(b: &mut MeshBuilder, f: &Frame, count: usize, step: f32, len: f32, half: f32, tail: f32) {
    let (count, step) = if b.fine() {
        (count, step)
    } else {
        (2, step * (count - 1) as f32)
    };
    Course {
        count,
        step,
        len,
        half,
        tip: -1.0,
        thick: 0.14,
        tail,
    }
    .lay(b, f);
}

/// The glacis on the low hull's nose: a thick plate from the nose up toward the turret,
/// red optics under its brow.
fn glacis(b: &mut MeshBuilder, z: f32, half: f32) {
    dark_plate(b);
    armour(
        b,
        &Frame::new(v3(4.2, 0.0, z - 0.3), v3(-1.0, 0.0, 0.3), v3(0.3, 0.0, 1.0)),
        &[
            [0.0, -0.3],
            [0.0, 0.3],
            [1.4, half],
            [2.2, half * 0.8],
            [2.2, -half * 0.8],
            [1.4, -half],
        ],
        0.16,
    );
    b.mirror_y(|b| {
        red_slot(
            b,
            v3(3.95, 0.5, 1.1),
            v3(0.7, 0.7, 0.0),
            Vec3::X,
            0.35,
            0.08,
        );
        if b.fine() {
            red_slot(
                b,
                v3(3.55, 0.85, 1.12),
                v3(0.5, 0.86, 0.0),
                Vec3::X,
                0.22,
                0.06,
            );
        }
    });
}

/// A turret: a bronze ring at the turret's pivot, a faceted housing lofted from `half` its
/// plan (x, y >= 0, nose to tail) between `z` heights, drawn in to `top` at its roof, the
/// owner's colour on its roof, a sensor blade swept back and up off it, red optics either
/// side of the gun.
fn turret(b: &mut MeshBuilder, half: &[[f32; 2]], z: (f32, f32), top: f32) {
    let plan: Vec<[f32; 2]> = half
        .iter()
        .copied()
        .chain(
            half.iter()
                .rev()
                .filter(|p| p[1] > 0.0)
                .map(|&[x, y]| [x, -y]),
        )
        .collect();
    metal(b);
    b.prism(v3(0.0, 0.0, z.0 - 0.1), b.sides(12), 1.0, 0.95, 0.2);
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &plan,
            &[
                Section::new(z.0 + 0.05, 1.0),
                Section::new(z.0 + (z.1 - z.0) * 0.55, 1.0),
                Section::new(z.1, top).shifted(-0.2, 0.0),
            ],
        )
    });
    let tail = half.last().map_or(-2.0, |p| p[0]) * top - 0.2;
    b.paint(TEAM);
    b.face(&[
        v3(-0.2, -0.45, z.1 + 0.005),
        v3(-0.2, 0.45, z.1 + 0.005),
        v3(tail * 0.6, 0.35, z.1 + 0.005),
        v3(tail * 0.6, -0.35, z.1 + 0.005),
    ]);
    dark_plate(b);
    blade(
        b,
        v3(tail * 0.72, 0.0, z.1),
        v3(-1.0, 0.0, 0.55),
        Vec3::Y,
        0.95,
        0.18,
        0.9,
        0.12,
    );
    let front = half[0][0] * top;
    b.mirror_y(|b| {
        red_slot(
            b,
            v3(front - 0.12, 0.55, z.1 - 0.25),
            v3(0.7, 0.7, 0.2),
            Vec3::Y,
            0.18,
            0.08,
        );
    });
}

/// The cannon on its trunnion, pitching (`rig::ARM_GUN`): a bronze trunnion across the
/// turret's front and the light Pinched-plasmeric Cannon out to its charge.
fn gun(b: &mut MeshBuilder) {
    b.with_limb(rig::ARM_GUN, |b| {
        metal(b);
        b.cylinder_between(
            PIVOT - Vec3::Y * 0.6,
            PIVOT + Vec3::Y * 0.6,
            0.3,
            0.3,
            b.sides(10),
        );
        pinch_cannon(b, v3(BREECH, 0.0, PIVOT.z), CHARGE, GUN_R, HOLD);
    });
}

/// Far off: a six-sided slab with the lift under it, a wedge of a turret with
/// the owner's colour on it, the bore as a bar and the two prongs as thin bars either
/// side of the charge, still pitching.
fn coarse(b: &mut MeshBuilder) {
    dark_plate(b);
    b.loft_z(
        &COARSE,
        &[
            Section::new(0.6, 1.0),
            Section::new(1.6, 0.85).shifted(-0.2, 0.0),
        ],
    );
    coarse_lift(
        b,
        &[
            Vec2::new(3.0, 0.0),
            Vec2::new(-3.0, 1.8),
            Vec2::new(-3.0, -1.8),
        ],
        0.5,
    );
    b.with_part(part::TURRET, |b| {
        dark_plate(b);
        b.frustum_open(
            v3(-0.4, 0.0, 1.6),
            Vec2::new(3.4, 2.2),
            Vec2::new(2.4, 1.6),
            1.3,
            Vec2::new(-0.2, 0.0),
        );
        b.paint(TEAM);
        b.face(&[
            v3(0.2, -0.5, 2.91),
            v3(0.2, 0.5, 2.91),
            v3(-1.2, 0.5, 2.91),
            v3(-1.2, -0.5, 2.91),
        ]);
        b.with_limb(rig::ARM_GUN, |b| {
            dark_plate(b);
            let mouth = BREECH + (CHARGE.x - BREECH) * 0.74;
            b.cylinder_between(
                v3(1.0, 0.0, CHARGE.z),
                v3(mouth, 0.0, CHARGE.z),
                0.4,
                0.25,
                3,
            );
            for y in [-HOLD, HOLD] {
                b.cylinder_between(
                    v3(2.4, y, CHARGE.z),
                    v3(CHARGE.x + 0.25, y, CHARGE.z),
                    0.2,
                    0.12,
                    3,
                );
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_model;

    const KEYS: [&str; 1] = ["regency_hover_tank"];

    #[test]
    fn fits_the_librarys_checks() {
        for key in KEYS {
            super::super::check_charge(key, 4.6, 3.4, None, &[CHARGE.to_array()], HOLD);
        }
    }

    #[test]
    fn hovers_on_its_bells_with_no_legs() {
        for key in KEYS {
            let model = build_model(key).unwrap();
            assert!(model.hover, "{key} floats on its lift");
            assert!(model.legs.is_none() && model.treads.is_none(), "{key}");
            assert!(model.lifts.len() >= 3, "{key}");
            for lod in &model.lods {
                let low = lod
                    .vertices
                    .iter()
                    .map(|v| v.pos[2])
                    .fold(f32::MAX, f32::min);
                assert!(low > 0.15, "{key} hangs clear of the ground: lowest {low}");
                assert!(lod.vertices.iter().all(|v| v.material != GLOW_VIOLET));
            }
        }
    }

    #[test]
    fn the_unit_files_gun_is_the_models() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&dir).unwrap();
        let bp = blueprints.unit(blueprints.id_of("regency_t1_tank").unwrap());
        assert_eq!(bp.visual.mesh, "regency_hover_tank");
        let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
        let w = &bp.weapons[0];
        assert!(v(w.muzzle).distance(CHARGE) < 1e-3);
        assert!(v(w.pivot.unwrap()).distance(PIVOT) < 1e-3);
        assert!(bp.turret_at.is_none(), "the turret turns about the middle");
        assert_eq!(bp.motion.map(|m| m.layer), Some(mc_data::MoveLayer::Hover));
        let model = build_model("regency_hover_tank").unwrap();
        assert_eq!(Vec3::from(model.turret_pivot), TURRET);
        assert_eq!(model.arm_pivot, Some(PIVOT.to_array()));
    }
}
