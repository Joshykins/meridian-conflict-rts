use glam::Vec3;

use super::{BODY_FRONT, BREECH_HINGE, DEPRESSION_DEG, MUZZLE, RAIL, RAIL_HH, TRUNNION};
use crate::{build_model_scaled, material, part, rig, MeshLod, Model};

/// The unit file's size (`aster_t3_point_defense`): radius, height, tech; 4x4 lot.
const SIZE: (f32, f32, u8) = (20.0, 28.0, 3);
const HALF_LOT: f32 = 24.0;

fn built() -> Model {
    build_model_scaled("citadel", SIZE.0, SIZE.1, SIZE.2).unwrap()
}

fn tris(mesh: &MeshLod) -> usize {
    mesh.indices.len() / 3
}

fn weapon() -> (Vec3, Vec3) {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let blueprints = mc_data::Blueprints::load(&data).unwrap();
    let bp = blueprints.unit(blueprints.id_of("aster_t3_point_defense").unwrap());
    let w = &bp.weapons[0];
    let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
    (v(w.pivot.expect("the Citadel's gun elevates")), v(w.muzzle))
}

/// The unit file's pivot and muzzle are the model's trunnion and barrel tip, and the
/// barrel reaches the muzzle at every level of detail.
#[test]
fn citadel_barrel_ends_at_the_muzzle_and_pitches_about_the_trunnion() {
    let (pivot, muzzle) = weapon();
    assert!(pivot.distance(TRUNNION) < 1e-2, "data pivot {pivot}");
    let tip = TRUNNION + Vec3::X * MUZZLE;
    assert!(muzzle.distance(tip) < 1e-2, "data muzzle {muzzle} vs {tip}");
    let model = built();
    assert_eq!(model.arm_pivot, Some(TRUNNION.to_array()));
    assert!(model.recoil.is_some());
    for (l, lod) in model.lods.iter().enumerate() {
        let barrel: Vec<Vec3> = lod
            .vertices
            .iter()
            .filter(|v| {
                v.part == part::TURRET
                    && v.rig & rig::LIMB_MASK == rig::ARM_GUN
                    && v.rig & rig::RECOIL != 0
            })
            .map(|v| Vec3::from(v.pos))
            .collect();
        let front = barrel.iter().map(|p| p.x).fold(f32::MIN, f32::max);
        assert!(
            (front - tip.x).abs() < 0.2,
            "lod{l}: barrel ends at {front}"
        );
        // Out of the gun body, the barrel is five times longer than its widest clamp.
        let wide = barrel
            .iter()
            .filter(|p| p.x > TRUNNION.x + BODY_FRONT)
            .map(|p| p.y.abs().max((p.z - TRUNNION.z).abs()))
            .fold(0.0, f32::max);
        assert!(
            MUZZLE - BODY_FRONT > 5.0 * 2.0 * wide,
            "lod{l}: {wide} m half-wide"
        );
    }
}

/// Stands in its 4x4 lot (only the barrel overhangs), to its height, bigger than the
/// Redoubt's 2x2 keep; wears team colour, and nothing on it glows.
#[test]
fn citadel_fits_its_lot_and_is_unlit_hardware() {
    let model = built();
    for (l, lod) in model.lods.iter().enumerate() {
        let fixed = lod.vertices.iter().filter(|v| v.part != part::TURRET);
        let (x, y) = fixed.fold((0.0f32, 0.0f32), |(x, y), v| {
            (x.max(v.pos[0].abs()), y.max(v.pos[1].abs()))
        });
        assert!(x <= HALF_LOT && y <= HALF_LOT, "lod{l}: base {x} x {y}");
        assert!(x >= 19.0 && y >= 19.0, "lod{l}: base {x} x {y}");
        let top = lod.vertices.iter().map(|v| v.pos[2]).fold(0.0, f32::max);
        assert!(
            top >= SIZE.1 * 0.8 && top <= SIZE.1 * 1.1,
            "lod{l}: top {top}"
        );
        assert!(
            lod.vertices.iter().any(|v| v.material == material::TEAM),
            "lod{l}: team colour"
        );
        let lit = lod
            .vertices
            .iter()
            .filter(|v| matches!(v.material, material::GLOW | material::GLOW_ORANGE))
            .count();
        assert_eq!(lit, 0, "lod{l}: lit");
        assert!(
            lod.vertices.iter().all(|v| v.pos[2] >= -1e-3),
            "lod{l}: below ground"
        );
    }
    let [full, mid, coarse] = [0, 1, 2].map(|l| tris(&model.lods[l]));
    println!("citadel: {full}/{mid}/{coarse}");
}

/// The breech door hangs on the gun from its hinge: at full and middle detail its verts
/// carry `rig::BREECH` on the gun's limb and never the barrel's kick, and the model's
/// hinge is the door's, where the unit file throws the cartridge out.
#[test]
fn citadel_breech_door_rides_the_gun_and_swings_up() {
    let model = built();
    let [x, y, z, open] = model.breech.expect("the Citadel has a breech door");
    let hinge = TRUNNION + BREECH_HINGE;
    assert!(
        Vec3::new(x, y, z).distance(hinge) < 1e-3,
        "hinge {x} {y} {z}"
    );
    assert!(open < -1.0, "the door swings up and back: {open}");
    for (l, lod) in model.lods.iter().take(2).enumerate() {
        let door: Vec<_> = lod
            .vertices
            .iter()
            .filter(|v| v.rig & rig::BREECH != 0)
            .collect();
        assert!(!door.is_empty(), "lod{l}: no door");
        for v in door {
            assert_eq!(v.part, part::TURRET);
            assert_eq!(
                v.rig & rig::LIMB_MASK,
                rig::ARM_GUN,
                "lod{l}: door off the gun"
            );
            assert_eq!(
                v.rig & (rig::RECOIL | rig::UPGRADE),
                0,
                "lod{l}: door kicks"
            );
            // It hangs below its hinge, over the mouth on the back face.
            assert!(v.pos[2] <= hinge.z + 0.3 && v.pos[0] < TRUNNION.x + BODY_FRONT);
        }
    }
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let blueprints = mc_data::Blueprints::load(&data).unwrap();
    let bp = blueprints.unit(blueprints.id_of("aster_t3_point_defense").unwrap());
    let sabot = bp.weapons[0]
        .sabot
        .expect("the Citadel throws its cartridges");
    let port = Vec3::new(
        sabot.port.x.to_f32(),
        sabot.port.y.to_f32(),
        sabot.port.z.to_f32(),
    );
    assert!(
        (port.x - (TRUNNION.x + super::BODY_BACK)).abs() < 1.5
            && port.y.abs() < super::BREECH_MOUTH.x
            && (port.z - TRUNNION.z).abs() < super::BREECH_MOUTH.y,
        "the unit file's port {port} is not in the breech mouth"
    );
}

/// The spent cartridge: small, sound, and lying on the ground.
#[test]
fn citadel_cartridge_is_a_sound_little_mesh() {
    let model = build_model_scaled("citadel_casing", 2.2, 1.3, 3).unwrap();
    let [full, mid, coarse] = [0, 1, 2].map(|l| tris(&model.lods[l]));
    println!("citadel_casing: {full}/{mid}/{coarse}");
    assert!(full <= 400 && coarse < 20, "{full}/{mid}/{coarse}");
    for lod in &model.lods {
        let top = lod.vertices.iter().map(|v| v.pos[2]).fold(0.0, f32::max);
        assert!(lod.vertices.iter().all(|v| v.pos[2] >= -1e-3) && top < 1.5);
    }
}

/// Sound meshes: no degenerate triangles, winding agreeing with the normals, one
/// material and part per triangle.
#[test]
fn citadel_is_a_sound_mesh() {
    for (l, mesh) in built().lods.iter().enumerate() {
        for t in mesh.indices.chunks(3) {
            let v = [0, 1, 2].map(|k| mesh.vertices[t[k] as usize]);
            let p = v.map(|v| Vec3::from(v.pos));
            let n = (p[1] - p[0]).cross(p[2] - p[0]);
            assert!(n.length() * 0.5 > 1e-7, "lod{l}: degenerate at {}", p[0]);
            for v in &v {
                assert!(
                    n.normalize().dot(Vec3::from(v.normal)) > 0.5,
                    "lod{l}: winding at {}",
                    p[0]
                );
            }
            assert!(v[0].material == v[1].material && v[1].material == v[2].material);
            assert!(v[0].part == v[1].part && v[1].part == v[2].part);
        }
    }
}

/// Previews at rest and elevated: `MODEL_DUMP_DIR=... cargo test -p mc-models --lib
/// -- --ignored citadel_previews`.
#[test]
#[ignore = "writes preview images"]
fn citadel_previews() {
    let dir = std::path::PathBuf::from(std::env::var_os("MODEL_DUMP_DIR").expect("MODEL_DUMP_DIR"));
    std::fs::create_dir_all(&dir).unwrap();
    let model = built();
    for (l, lod) in model.lods.iter().enumerate() {
        for pitch in [0.0f32, 0.2] {
            let mut posed = lod.clone();
            for v in &mut posed.vertices {
                if v.rig & rig::LIMB_MASK == rig::ARM_GUN {
                    let r = Vec3::from(v.pos) - TRUNNION;
                    let (s, c) = pitch.sin_cos();
                    v.pos = (TRUNNION + Vec3::new(r.x * c - r.z * s, r.y, r.x * s + r.z * c))
                        .to_array();
                }
            }
            let res = if l == 0 { 900 } else { 300 };
            for az in [-38.0f32, 52.0, 142.0] {
                if l > 0 && az != -38.0 {
                    continue;
                }
                crate::preview::render(&posed, res, az)
                    .write_ppm(&dir.join(format!(
                        "citadel_l{l}_p{}_{}.ppm",
                        (pitch * 100.0) as i32,
                        az as i32
                    )))
                    .unwrap();
            }
        }
    }
}

/// Laid [`DEPRESSION_DEG`] below level, the gun (body, pods, rails) stays clear of
/// everything that does not pitch with it: the carriage, the cheeks' feet, the
/// turntable and the tower. Checked against a height field of the rest, sampled
/// over its triangles on a half-metre grid.
#[test]
fn citadel_gun_lays_down_clear_of_its_tower() {
    let model = built();
    let lod = &model.lods[0];
    let gun =
        |v: &crate::MeshVertex| v.part == part::TURRET && v.rig & rig::LIMB_MASK == rig::ARM_GUN;
    let (s, c) = (-DEPRESSION_DEG.to_radians()).sin_cos();
    let laid = |p: Vec3| {
        let r = p - TRUNNION;
        TRUNNION + Vec3::new(r.x * c - r.z * s, r.y, r.x * s + r.z * c)
    };
    // Points over every triangle, about ten to the metre, the gun's laid down.
    let mut fixed = std::collections::BTreeMap::<(i32, i32), f32>::new();
    let mut laid_gun = Vec::new();
    for t in lod.indices.chunks(3) {
        let v = [0, 1, 2].map(|k| &lod.vertices[t[k] as usize]);
        let on_gun = gun(v[0]);
        let p = v.map(|v| {
            let p = Vec3::from(v.pos);
            if on_gun {
                laid(p)
            } else {
                p
            }
        });
        let n = ((p[1] - p[0]).length().max((p[2] - p[0]).length()) * 10.0).ceil() as usize;
        for i in 0..=n {
            for j in 0..=n - i {
                let (a, b) = (i as f32 / n as f32, j as f32 / n as f32);
                let q = p[0] + (p[1] - p[0]) * a + (p[2] - p[0]) * b;
                if on_gun {
                    laid_gun.push(q);
                } else {
                    let key = ((q.x * 10.0).floor() as i32, (q.y * 10.0).floor() as i32);
                    let h = fixed.entry(key).or_insert(f32::MIN);
                    *h = h.max(q.z);
                }
            }
        }
    }
    let mut worst = (f32::MAX, Vec3::ZERO);
    for q in laid_gun {
        let key = ((q.x * 10.0).floor() as i32, (q.y * 10.0).floor() as i32);
        if let Some(h) = fixed.get(&key) {
            if q.z - h < worst.0 {
                worst = (q.z - h, q);
            }
        }
    }
    assert!(
        worst.0 > 0.0,
        "laid {DEPRESSION_DEG} deg down the gun sinks {} m at {}",
        worst.0,
        worst.1
    );
}

/// The charge's arcs crawl on the bare rails, breech forward, and the gun they run
/// along is the one the unit file fires from.
#[test]
fn citadel_charge_arcs_run_along_the_bare_rails() {
    let (pivot, muzzle) = weapon();
    assert!((pivot.x + RAIL.muzzle - muzzle.x).abs() < 0.01);
    assert!(RAIL.breech < 0.0 && (RAIL.rail_top - RAIL_HH).abs() < 1e-6);
    let mut last = BODY_FRONT - TRUNNION.x;
    for x in RAIL.arcs {
        assert!(x > last && x < RAIL.muzzle, "arc stretch at {x}");
        last = x;
    }
}
