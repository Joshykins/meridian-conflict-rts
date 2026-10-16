//! The ship's guns as the unit file has them are the model's: the lance from the core's
//! lower lens, each weapon group's mounts on the model's houses in gun-slot order, the
//! Rondels' lenses where the anti-missile mounts are.

use std::path::Path;

use glam::Vec3;
use mc_core::{Angle, FxVec3};
use mc_data::Blueprints;

use super::guns::{guns, GROUPS, RONDEL_LENS};
use super::{mounts, A, CORE, HULL};
use crate::{build_model, material};

const KEY: &str = "regency_t4_space_destroyer";

fn at(p: FxVec3) -> Vec3 {
    Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32())
}

fn show(p: Vec3) -> String {
    format!("({:.2}, {:.2}, {:.2})", p.x, p.y, p.z)
}

#[test]
fn the_unit_files_guns_are_the_models() {
    let bp = Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    let unit = bp.unit(bp.id_of(KEY).unwrap());
    let model = build_model(&unit.visual.mesh).unwrap();
    // The lance is laid from the core's lower lens at the ship's middle: no turret, nothing
    // turns or pitches (a pivot would pitch the hull: `capital_ship` in entity.wgsl).
    let lance = &unit.weapons[0];
    let lens = CORE.lens();
    assert!(lance.pivot.is_none() && lance.mounts.is_empty());
    assert!(
        at(lance.muzzle).distance(lens) < 0.01,
        "lance muzzle {}",
        show(lens)
    );
    let face: Vec<Vec3> = model.lods[0]
        .vertices
        .iter()
        .filter(|v| v.material == material::GLOW_PRISM && (v.pos[2] - lens.z).abs() < 0.05)
        .map(|v| Vec3::from(v.pos))
        .collect();
    let middle = face.iter().sum::<Vec3>() / face.len().max(1) as f32;
    assert!(
        face.len() >= 8 && middle.distance(lens) < 0.1,
        "no lens at the core's muzzle: {} points about {middle}",
        face.len()
    );
    // Each group's mounts are the model's guns of its kind, in gun-slot order: the same
    // pivots to the quarter metre, the same rest, the muzzles where the model's bores are.
    let (port, rondels) = mounts(&A, &HULL);
    let guns = guns(&port);
    let mut slot = 1;
    for (k, kind) in GROUPS.iter().enumerate() {
        let weapon = &unit.weapons[1 + k];
        let model: Vec<_> = guns.iter().filter(|g| g.kind == *kind).collect();
        assert_eq!(weapon.mounts.len(), model.len(), "{kind:?}: mounts");
        assert_eq!(unit.gun_slot(1 + k), slot, "{kind:?}: first gun slot");
        for (m, g) in model.iter().enumerate() {
            let gun = weapon.gun(m);
            let pivot = g.house();
            assert_eq!(
                at(weapon.mounts[m].pivot),
                pivot,
                "{kind:?} {m}: pivot {}",
                show(pivot)
            );
            let facing = Angle::from_degrees(g.yaw.to_degrees().round() as i32);
            assert_eq!(gun.facing, facing, "{kind:?} {m}: facing");
            let want: Vec<Vec3> = kind.muzzles().iter().map(|&o| pivot + o).collect();
            let have: Vec<Vec3> = if gun.muzzles.is_empty() {
                vec![at(gun.muzzle)]
            } else {
                gun.muzzles.iter().map(|&p| at(p)).collect()
            };
            assert_eq!(have.len(), want.len(), "{kind:?} {m}: muzzles");
            for (h, w) in have.iter().zip(&want) {
                assert!(
                    h.distance(*w) < 0.01,
                    "{kind:?} {m}: muzzle {} not {}",
                    show(*h),
                    show(*w)
                );
            }
        }
        slot += model.len();
    }
    assert_eq!(model.houses.len(), guns.len(), "houses");
    for (s, (house, g)) in model.houses.iter().zip(&guns).enumerate() {
        assert_eq!(house.gun as usize, 1 + s, "house {s}: gun slot");
        assert_eq!(Vec3::from(house.pivot), g.house(), "house {s}: pivot");
    }
    // The Rondels' lenses are the anti-missile mounts, port then starboard.
    let lenses: Vec<Vec3> = [1.0f32, -1.0]
        .iter()
        .flat_map(|&side| {
            rondels
                .iter()
                .map(move |&p| (p + Vec3::Z * RONDEL_LENS) * Vec3::new(1.0, side, 1.0))
        })
        .collect();
    let have: Vec<Vec3> = unit.anti_missile_mounts.iter().map(|&p| at(p)).collect();
    assert_eq!(have, lenses, "anti-missile mounts");
    assert_eq!(unit.anti_missile_lasers as usize, lenses.len());
}

#[test]
fn zz_print_mounts() {
    let (port, rondels) = mounts(&A, &HULL);
    for g in guns(&port) {
        let o: Vec<String> = g.kind.muzzles().iter().map(|&m| show(m)).collect();
        println!(
            "{:?} pivot {} yaw {} muzzles {}",
            g.kind,
            show(g.house()),
            g.yaw.to_degrees(),
            o.join(" ")
        );
    }
    for r in rondels {
        println!("rondel {}", show(r + Vec3::Z * RONDEL_LENS));
    }
    println!("lens {}", show(CORE.lens()));
}
