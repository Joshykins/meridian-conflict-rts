//! The Breacher (`aster_t4_breacher`, mesh "breacher"): the unit file's muzzles are the
//! model's, both arms are houses about the torso's axis that reach their muzzles at every
//! level, and the model's spin axis is the right gun's bore, the left gun its mirror.

use glam::Vec3;

use super::nearest_where;
use crate::aster::breacher::{muzzle, pods, GUN, PIVOT, POD};
use crate::{build_model, part, rig, MeshVertex};

fn blueprint() -> mc_data::UnitBlueprint {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let bps = mc_data::Blueprints::load(&data).unwrap();
    bps.unit(bps.id_of("aster_t4_breacher").unwrap()).clone()
}

/// The unit file is authored 1:1 with the model: its pods' muzzles are the pod mouths, its
/// arms' muzzles the gun muzzles, all three weapons turn about the shoulder.
#[test]
fn breacher_unit_file_matches_the_model() {
    let bp = blueprint();
    let to = |p: mc_core::FxVec3| Vec3::from(p.to_f32());
    let pods_w = &bp.weapons[0];
    let mouths = pods::mouths(POD.0, POD.1);
    assert_eq!(pods_w.muzzles.len(), 2 * mouths.len());
    for m in &mouths {
        for side in [1.0, -1.0] {
            let want = Vec3::new(m.x, m.y * side, m.z);
            assert!(
                pods_w.muzzles.iter().any(|&p| to(p).distance(want) < 0.02),
                "no muzzle in the unit file at the pod mouth {want}"
            );
        }
    }
    for (w, side) in [(1, -1.0), (2, 1.0)] {
        let want = muzzle() * Vec3::new(1.0, side, 1.0);
        assert!(
            to(bp.weapons[w].muzzle).distance(want) < 0.02,
            "weapon {w}'s muzzle is not the model's {want}"
        );
        assert_eq!(
            bp.weapons[w].rotary as usize,
            w - 1,
            "weapon {w}'s rotary slot"
        );
    }
    for w in &bp.weapons {
        assert!(
            w.pivot.is_some_and(|p| to(p).distance(PIVOT) < 0.02),
            "{} turns about the shoulder",
            w.name
        );
    }
}

/// Both arms are houses on the torso's axis, every vertex of them pitching with the gun,
/// each reaching its muzzle at every level of detail; the model spins about the right
/// arm's bore.
#[test]
fn breacher_arms_reach_their_muzzles() {
    let model = build_model("breacher").expect("breacher builds");
    let slot_of = |weapon: u8| {
        model
            .houses
            .iter()
            .position(|h| h.weapon == weapon && Vec3::from(h.pivot).distance(PIVOT) < 0.003)
            .unwrap_or_else(|| panic!("breacher: a house for weapon {weapon}"))
    };
    let in_house = |slot: usize| {
        move |v: &MeshVertex| {
            (v.rig & rig::LIMB_MASK) == rig::HOUSE_FIRST + (slot as u32 & 3)
                && ((v.rig & rig::HOUSE_HIGH) != 0) == (slot >= 4)
                && v.part != part::TURRET
        }
    };
    let spin = model.spins.first().expect("breacher: the clusters spin").2;
    assert!(
        (spin[1] + GUN.y).abs() < 1e-3 && (spin[2] - GUN.z).abs() < 1e-3,
        "breacher: the model spins about the right arm's bore, not {spin:?}"
    );
    for (lod, mesh) in model.lods.iter().enumerate() {
        for (weapon, side) in [(1u8, -1.0), (2, 1.0)] {
            let slot = slot_of(weapon);
            let muzzle = muzzle() * Vec3::new(1.0, side, 1.0);
            let near = nearest_where(mesh, muzzle, in_house(slot));
            assert!(
                near < 0.5,
                "breacher lod{lod}: arm {weapon} ends {near} m from its muzzle"
            );
            assert!(
                mesh.vertices
                    .iter()
                    .filter(|v| in_house(slot)(v))
                    .all(|v| v.rig & rig::RECOIL != 0),
                "breacher lod{lod}: the whole arm {weapon} pitches"
            );
        }
    }
}
