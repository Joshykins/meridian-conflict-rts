//! Rocket racks fixed on a model (`fixed_rack`): the unit file's rockets leave the way
//! the model's tubes point, so its `rake` is the tubes' angle above level, and a rack
//! that rides a torso does not pitch.

use glam::Vec3;

use crate::aster::breacher::POD;
use crate::aster::naval::missile_ship;
use crate::aster::titan;

fn blueprints() -> mc_data::Blueprints {
    let data =
        std::path::Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../../data");
    mc_data::Blueprints::load(&data).unwrap()
}

/// Each fixed rack, its rocket weapon, and the angle above level its model's tubes
/// point (radians).
fn racks() -> [(&'static str, usize, f32); 3] {
    [
        ("aster_t4_breacher", 0, POD.1),
        ("aster_t5_titan", 0, titan::POD_PITCH),
        (
            "aster_t2_missile_ship",
            0,
            std::f32::consts::FRAC_PI_2 - missile_ship::CANT,
        ),
    ]
}

#[test]
fn fixed_racks_launch_along_their_tubes() {
    let bps = blueprints();
    for (key, w, tubes) in racks() {
        let weapon = &bps.unit(bps.id_of(key).unwrap()).weapons[w];
        assert!(
            weapon.fixed_rack,
            "{key}: its rack does not pitch in the model"
        );
        let rake = weapon.rake.to_radians_f32();
        assert!(
            (rake - tubes).abs() < 0.005,
            "{key}: rockets leave at {rake} rad, its tubes point {tubes}"
        );
    }
}

/// The Behemoth's rockets leave its pods' cell mouths (the unit file at its size).
#[test]
fn titan_rockets_leave_the_pod_mouths() {
    let bps = blueprints();
    let bp = bps.unit(bps.id_of("aster_t5_titan").unwrap());
    let (radius, height) = crate::authored_size("titan").unwrap();
    let grown = Vec3::new(
        bp.radius.to_f32() / radius,
        bp.radius.to_f32() / radius,
        bp.height.to_f32() / height,
    );
    let muzzles = &bp.weapons[0].muzzles;
    assert_eq!(muzzles.len(), 2 * titan::POD_MOUTHS.len());
    for (x, y, z) in titan::POD_MOUTHS {
        for side in [1.0, -1.0] {
            let want = Vec3::new(x, y * side, z + titan::RAISE) * grown;
            assert!(
                muzzles
                    .iter()
                    .any(|m| Vec3::from(m.to_f32()).distance(want) < 0.1),
                "no muzzle at the pod mouth {want}"
            );
        }
    }
}
