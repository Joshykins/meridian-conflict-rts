//! The Regent's guns as the unit file has them are the model's: each weapon group's mounts
//! on the model's houses, in gun-slot order, the seekers in the spines' cells, the Rondels'
//! lenses where the anti-missile mounts are.

use std::path::Path;

use glam::Vec3;
use mc_core::{Angle, FxVec3};
use mc_data::Blueprints;

use super::guns::{guns, Kind, GROUPS, RONDEL_LENS};
use super::spear::{mounts, seekers, HULL, LIFT};
use crate::build_model;

const KEY: &str = "regency_t5_super_destroyer";

fn at(p: FxVec3) -> Vec3 {
    Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32())
}

/// Each group's mounts are the model's guns of its kind, in the model's gun-slot order:
/// the same pivots to the quarter metre, the same rest, the muzzles where the model's
/// bores are; and each of the model's houses is bound to the gun slot the data gives it.
#[test]
fn the_unit_files_guns_are_the_models() {
    let bp = Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    let unit = bp.unit(bp.id_of(KEY).unwrap());
    let (port, centre) = mounts(&HULL);
    let guns = guns(&port, &centre);
    let lift = Vec3::Z * LIFT;
    let mut slot = 0;
    for (k, kind) in GROUPS.iter().enumerate() {
        let weapon = &unit.weapons[k];
        let model: Vec<_> = guns.iter().filter(|g| g.kind == *kind).collect();
        assert_eq!(weapon.mounts.len(), model.len(), "{kind:?}: mounts");
        assert_eq!(unit.gun_slot(k), slot, "{kind:?}: first gun slot");
        for (m, g) in model.iter().enumerate() {
            let gun = weapon.gun(m);
            let pivot = g.house() + lift;
            assert_eq!(at(weapon.mounts[m].pivot), pivot, "{kind:?} {m}: pivot");
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
                assert!(h.distance(*w) < 0.01, "{kind:?} {m}: muzzle {h} not {w}");
            }
        }
        slot += model.len();
    }
    let model = build_model(&unit.visual.mesh).unwrap();
    assert_eq!(model.houses.len(), guns.len(), "houses");
    for (s, (house, g)) in model.houses.iter().zip(&guns).enumerate() {
        assert_eq!(house.gun as usize, s, "house {s}: gun slot");
        assert_eq!(
            Vec3::from(house.pivot),
            g.house() + lift,
            "house {s}: pivot"
        );
    }
    // The seekers stand in the spines' cells, port spine first, in firing order.
    let seeker = &unit.weapons[GROUPS.len()];
    let want: Vec<Vec3> = seekers().muzzles().iter().map(|&m| m + lift).collect();
    let have: Vec<Vec3> = seeker.muzzles.iter().map(|&p| at(p)).collect();
    assert_eq!(have, want, "seeker muzzles");
    assert_eq!(model.cells.len(), 2, "two spines of cells");
    // The Rondels' lenses are the anti-missile mounts, port then starboard.
    let lenses: Vec<Vec3> = [1.0f32, -1.0]
        .iter()
        .flat_map(|&side| {
            port.iter()
                .filter(|m| m.kind == Kind::Rondel)
                .map(move |m| (m.pivot + lift + Vec3::Z * RONDEL_LENS) * Vec3::new(1.0, side, 1.0))
        })
        .collect();
    let mounts: Vec<Vec3> = unit.anti_missile_mounts.iter().map(|&p| at(p)).collect();
    assert_eq!(mounts, lenses, "anti-missile mounts");
    assert_eq!(unit.anti_missile_lasers as usize, lenses.len());
}
