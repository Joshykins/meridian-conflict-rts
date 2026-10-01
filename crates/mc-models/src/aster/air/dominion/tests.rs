use super::*;
use crate::build_model_fitted;

const DESIGNS: [&str; 1] = ["space_dreadnought"];

/// A vertex on a gun house (it turns with its weapon, not with the hull).
fn on_house(v: &crate::MeshVertex) -> bool {
    let limb = v.rig & rig::LIMB_MASK;
    (rig::HOUSE_FIRST..rig::HOUSE_FIRST + 4).contains(&limb) || v.rig & rig::HOUSE_HIGH != 0
}

#[test]
fn every_design_keeps_its_budgets_rig_and_guns() {
    for key in DESIGNS {
        let model = build_model_fitted(key, 285.0, 110.0, 4, &[]).unwrap();
        let tris: Vec<_> = model.lods.iter().map(|l| l.indices.len() / 3).collect();
        println!("{key} triangles: {tris:?}");
        assert!(
            tris[0] <= 40000 && tris[1] as f32 <= tris[0] as f32 * 0.45 + 20.0 && tris[2] < 60,
            "{key}: {tris:?}"
        );
        // Casemates 0..=5, then the rifles 6 and 7, at the unit file's pivots.
        let want: Vec<_> = CASEMATES.iter().chain(RIFLES.iter()).copied().collect();
        assert_eq!(model.houses.len(), want.len(), "{key}");
        for (i, (house, pivot)) in model.houses.iter().zip(want).enumerate() {
            assert_eq!(house.pivot, pivot, "{key}");
            assert_eq!(house.weapon as usize, i, "{key}");
        }
        assert_eq!(model.cells.len(), 2, "{key}");
        let mesh = &model.lods[0];
        let span = |axis: usize| {
            mesh.vertices
                .iter()
                .map(|v| v.pos[axis])
                .fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)))
        };
        let (lo, hi) = span(0);
        assert!(
            (550.0..=580.0).contains(&(hi - lo)),
            "{key}: length {}",
            hi - lo
        );
        assert!(
            (hi - prow::BOW).abs() < 2.0,
            "{key}: the head's nose is foremost: {hi}"
        );
        let (ylo, yhi) = span(1);
        assert!(yhi <= 108.0 && ylo >= -108.0, "{key}: beam {ylo}..{yhi}");
        assert!(span(2).0 >= -0.01, "{key}");
        let top = span(2).1;
        assert!((120.0..=165.0).contains(&top), "{key}: top {top}");
        for part in [
            part::GEAR,
            part::GEAR_STRUT,
            part::GEAR_FOOT,
            part::GEAR_DOOR,
            part::DRIVE,
        ] {
            assert!(
                mesh.vertices.iter().any(|v| v.part == part),
                "{key}: no part {part}"
            );
        }
        for v in &mesh.vertices {
            if [part::GEAR, part::GEAR_STRUT, part::GEAR_FOOT].contains(&v.part) {
                let r = capital::stowed(&RIG, Vec3::from(v.pos), v.part, v.material);
                assert!(r.z >= KEEL + 0.2, "{key}: {:?} stows to {r}", v.part);
            }
        }
        assert!(model.shield_emitter.is_some(), "{key}");
        assert_eq!(crate::capital_rig(key), Some(RIG.gpu()), "{key}");
        assert_eq!(crate::lift_jets(key), &LIFT_JETS[..], "{key}");
        assert_eq!(crate::aircraft_exhausts(key), &NOZZLES[..], "{key}");
        assert!(crate::capital_lamps(key).is_some(), "{key}");
    }
}

/// Distance from `p` to the segment `a`..`b`.
fn to_segment(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let d = b - a;
    let t = ((p - a).dot(d) / d.length_squared()).clamp(0.0, 1.0);
    (a + d * t).distance(p)
}

/// Nothing on the hull stands in a casemate's barrels' sweep (from 10 degrees off the
/// nose round its beam to 10 off the stern, level and depressed) or over a rifle house's
/// (300 degrees round, level, clear of the deck it stands on).
#[test]
fn guns_sweep_clear_of_the_hull() {
    for key in DESIGNS {
        let model = build_model_fitted(key, 285.0, 110.0, 4, &[]).unwrap();
        let hull: Vec<Vec3> = model.lods[0]
            .vertices
            .iter()
            .filter(|v| !on_house(v))
            .map(|v| Vec3::from(v.pos))
            .collect();
        for p in CASEMATES {
            let p = Vec3::from(p);
            let s = p.y.signum();
            for step in 0..=32 {
                let yaw = (10.0 + step as f32 * 5.0).to_radians();
                for pitch in [0.0f32, -30.0, -60.0] {
                    let pitch = pitch.to_radians();
                    let dir = v3(
                        yaw.cos() * pitch.cos(),
                        s * yaw.sin() * pitch.cos(),
                        pitch.sin(),
                    );
                    for dz in [-CASEMATE_STACK, CASEMATE_STACK] {
                        let a = p + Vec3::Z * dz + dir * (DRUM_R + 1.0);
                        let b = p + Vec3::Z * dz + dir * CASEMATE_REACH;
                        for q in &hull {
                            assert!(
                                to_segment(*q, a, b) > CASEMATE_R * 1.5,
                                "{key}: casemate at {p} yawed {:.0} pitched {:.0} meets the hull at {q}",
                                yaw.to_degrees(),
                                pitch.to_degrees()
                            );
                        }
                    }
                }
            }
        }
        for (i, p) in RIFLES.iter().enumerate() {
            let p = Vec3::from(*p);
            // The forward house rests facing ahead, the aft one astern; each has a 60
            // degree dead zone behind it.
            let facing = if i == 0 { 0.0f32 } else { 180.0 };
            for q in &hull {
                let d = (*q - p).truncate();
                if !(12.0..RIFLE_REACH + 3.0).contains(&d.length()) {
                    continue;
                }
                let bearing = d.y.atan2(d.x).to_degrees() - facing;
                let off = (bearing + 540.0).rem_euclid(360.0) - 180.0;
                assert!(
                    off.abs() > 150.0 || q.z <= p.z - RIFLE_RAISE + 0.1,
                    "{key}: rifle house at {p} sweeps into the hull at {q}"
                );
            }
        }
    }
}

/// Software previews of each design from several sides (PPM), for a quick look.
#[test]
#[ignore]
fn dreadnought_previews() {
    let dir = std::path::PathBuf::from(
        std::env::var("MODEL_DUMP_DIR").unwrap_or("/tmp/dreadnought".into()),
    );
    std::fs::create_dir_all(&dir).unwrap();
    for key in DESIGNS {
        let model = build_model_fitted(key, 285.0, 110.0, 4, &[]).unwrap();
        for az in [-38.0f32, 90.0, 142.0] {
            crate::preview::render(&model.lods[0], 900, az)
                .write_ppm(&dir.join(format!("{key}_{}.ppm", az as i32)))
                .unwrap();
        }
    }
}
