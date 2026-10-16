//! The Nimbus (`aster_t4_storm_bomber`, mesh "storm_bomber"): the unit file's pivots,
//! muzzles and laser mounts are the model's, its four twin AA mounts turn about their
//! pivots with both barrels reaching their ends, and the seed leaves from the
//! dispenser's mouth at the very tail.

use glam::Vec3;

use super::nearest_where;
use crate::aster::air::nimbus::{GAP, LASERS, REACH, SEED, TURRETS};
use crate::{build_model, rig, MeshVertex};

/// A mount's two barrel ends, the mount facing the nose: left, then right.
fn barrels(pivot: Vec3) -> [Vec3; 2] {
    [1.0, -1.0].map(|side| pivot + v(REACH, GAP * side))
}

fn v(x: f32, y: f32) -> Vec3 {
    Vec3::new(x, y, 0.0)
}

/// The unit file is the model at the unit's size: the seeder's muzzle and pivot are
/// the dispenser's mouth, each twin AA mount turns about its pivot and fires from both
/// barrel ends, and the missile-defence beams leave the laser heads.
#[test]
fn nimbus_unit_file_matches_the_model() {
    let data =
        std::path::Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../../data");
    let bps = mc_data::Blueprints::load(&data).unwrap();
    let bp = bps.unit(bps.id_of("aster_t4_storm_bomber").unwrap());
    assert_eq!(bp.visual.mesh, "storm_bomber");
    let (radius, height) = crate::authored_size("storm_bomber").unwrap();
    assert!(
        (bp.radius.to_f32() - radius).abs() < 1e-3 && (bp.height.to_f32() - height).abs() < 1e-3,
        "the model is authored at the unit's size"
    );
    let at = |p: mc_core::FxVec3| Vec3::from(p.to_f32());
    let seeder = &bp.weapons[0];
    assert!(
        at(seeder.muzzle).distance(SEED) < 0.02
            && seeder.pivot.is_some_and(|p| at(p).distance(SEED) < 0.02),
        "the seeder's muzzle and pivot are the dispenser's mouth {}",
        SEED
    );
    assert_eq!(bp.weapons.len(), 5);
    for (k, pivot) in TURRETS.iter().enumerate() {
        let w = &bp.weapons[k + 1];
        assert!(w.mount, "AA mount {k} turns on its own");
        assert!(
            w.pivot.is_some_and(|p| at(p).distance(*pivot) < 0.02),
            "AA mount {k} turns about {pivot}"
        );
        let ends = barrels(*pivot);
        assert!(
            at(w.muzzle).distance((ends[0] + ends[1]) * 0.5) < 0.02,
            "AA mount {k}'s muzzle is not between its barrels' ends"
        );
        assert_eq!(w.muzzles.len(), 2, "AA mount {k} fires from both barrels");
        for (m, end) in w.muzzles.iter().zip(ends) {
            assert!(
                at(*m).distance(end) < 0.02,
                "AA mount {k}: a barrel ends at {end}"
            );
        }
    }
    assert_eq!(bp.anti_missile_mounts.len(), LASERS.len());
    for (m, laser) in bp.anti_missile_mounts.iter().zip(LASERS) {
        assert!(at(*m).distance(laser) < 0.02, "a laser head at {laser}");
    }
}

/// Four mounts for weapons 1 to 4 at their pivots, both barrels of each pitching and
/// reaching their ends at the full and mid levels; a laser head at each mount; the seed
/// leaves from the dispenser's mouth, the aftmost thing on the aircraft.
#[test]
fn nimbus_mounts_lasers_and_dispenser() {
    let model = build_model("storm_bomber").expect("the Nimbus builds");
    assert_eq!(model.houses.len(), 4, "four AA mounts");
    for (k, pivot) in TURRETS.iter().enumerate() {
        let slot = model
            .houses
            .iter()
            .position(|h| h.gun as usize == k + 1 && Vec3::from(h.pivot).distance(*pivot) < 1e-3)
            .unwrap_or_else(|| panic!("a mount for weapon {} at {pivot}", k + 1));
        let gun = move |v: &MeshVertex| {
            (v.rig & rig::LIMB_MASK) == rig::HOUSE_FIRST + (slot as u32 & 3)
                && v.rig & rig::RECOIL != 0
        };
        for (lod, mesh) in model.lods[..2].iter().enumerate() {
            for end in barrels(*pivot) {
                let near = nearest_where(mesh, end, gun);
                assert!(
                    near < 0.3,
                    "lod{lod}: mount {} has no barrel ending at {end} ({near} m)",
                    k + 1
                );
            }
        }
    }
    let mesh = &model.lods[0];
    for laser in LASERS {
        let near = nearest_where(mesh, laser, |_| true);
        assert!(near < 1.0, "no laser head at {laser} ({near} m)");
    }
    let near = nearest_where(mesh, SEED, |_| true);
    assert!(near < 1.0, "nothing at the dispenser's mouth ({near} m)");
    let aft = mesh
        .vertices
        .iter()
        .map(|v| v.pos[0])
        .fold(f32::MAX, f32::min);
    assert!(
        aft > SEED.x - 0.2,
        "something trails {} m behind the dispenser",
        SEED.x - aft
    );
}
