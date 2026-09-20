//! Whole-library checks: mesh validity, level-of-detail budgets, blueprint fit
//! and the conventions the renderer relies on (team colour, turret parts).

use glam::Vec3;

use super::library::build_lod;
use super::{
    all_model_keys, build_model, build_model_scaled, material, part, preview, prop_model_key, rig,
    MeshLod, Model, LOD_COUNT,
};

/// The blueprints in `data/factions/aster/units/*.ron` that matter to a model.
struct Blueprint {
    mesh: &'static str,
    radius: f32,
    height: f32,
    tech: u8,
    /// Build-grid footprint in 12 m cells (structures only).
    footprint: Option<(u32, u32)>,
    /// Weapon muzzle offsets (x forward, y left, z up).
    muzzles: &'static [[f32; 3]],
    /// False when the weapon is fixed to the hull (`turret_turn: 0`).
    turreted: bool,
}

const fn unit(
    mesh: &'static str,
    radius: f32,
    height: f32,
    tech: u8,
    muzzles: &'static [[f32; 3]],
) -> Blueprint {
    Blueprint {
        mesh,
        radius,
        height,
        tech,
        footprint: None,
        muzzles,
        turreted: true,
    }
}

const fn structure(
    mesh: &'static str,
    radius: f32,
    height: f32,
    tech: u8,
    cells: u32,
    muzzles: &'static [[f32; 3]],
) -> Blueprint {
    Blueprint {
        mesh,
        radius,
        height,
        tech,
        footprint: Some((cells, cells)),
        muzzles,
        turreted: true,
    }
}

const BLUEPRINTS: &[Blueprint] = &[
    unit("commander", 8.125, 18.75, 1, &[[7.75, -3.75, 12.125]]),
    unit("engineer", 3.6, 3.4, 1, &[]),
    unit("engineer", 4.2, 3.8, 2, &[]),
    unit("engineer", 4.8, 4.2, 3, &[]),
    unit("scout", 2.4, 1.8, 1, &[[1.2, 0.0, 1.6]]),
    unit("bot_light", 2.6, 5.2, 1, &[[1.4, 0.0, 4.2]]),
    unit("tank_light", 4.6, 3.4, 1, &[[5.2, 0.0, 2.82]]),
    unit("artillery_light", 4.2, 3.2, 1, &[[3.8, 0.0, 3.4]]),
    unit("tank_heavy", 6.2, 4.4, 2, &[[6.8, 0.0, 3.8]]),
    unit("hover_tank", 5.4, 3.2, 2, &[[4.6, 0.0, 2.8]]),
    Blueprint {
        turreted: false,
        ..unit("missile_launcher", 5.2, 4.0, 2, &[[-1.0, 0.0, 4.4]])
    },
    unit(
        "assault_bot",
        6.8,
        12.0,
        3,
        &[[3.4, -3.6, 9.0], [3.4, 3.6, 9.0]],
    ),
    unit("artillery_heavy", 7.0, 5.0, 3, &[[8.5, 0.0, 5.2]]),
    structure("factory_land", 46.0, 22.0, 1, 8, &[]),
    structure("factory_land", 46.0, 26.0, 2, 8, &[]),
    structure("factory_land", 46.0, 30.0, 3, 8, &[]),
    structure("extractor", 10.5, 6.75, 1, 2, &[]),
    structure("extractor", 10.5, 8.25, 2, 2, &[]),
    structure("extractor", 10.5, 9.75, 3, 2, &[]),
    structure("power", 10.5, 9.0, 1, 2, &[]),
    structure("power", 16.5, 13.5, 2, 3, &[]),
    structure("power", 22.5, 19.5, 3, 4, &[]),
    structure("storage_mass", 10.5, 6.0, 1, 2, &[]),
    structure("storage_energy", 10.5, 7.5, 1, 2, &[]),
    structure("turret", 5.25, 6.75, 1, 1, &[[4.5, 0.0, 5.55]]),
    structure("turret_heavy", 10.5, 9.75, 2, 2, &[[6.75, 0.0, 7.5]]),
    structure("artillery_static", 10.5, 9.0, 2, 2, &[[9.0, 0.0, 7.125]]),
    structure("radar", 4.5, 15.0, 1, 1, &[]),
    structure("radar", 4.5, 18.0, 2, 1, &[]),
    structure("radar", 4.5, 21.0, 3, 1, &[]),
    structure("reclaimer", 8.25, 18.0, 2, 2, &[[9.9, 0.0, 15.3]]),
    structure("wall", 6.0, 4.5, 1, 1, &[]),
];

const PROP_KEYS: &[&str] = &[
    "tree_conifer",
    "tree_broadleaf",
    "tree_dead",
    "rock_small",
    "rock_large",
    "building_small",
    "building_tower",
    "building_wide",
];

fn built(bp: &Blueprint) -> Model {
    build_model_scaled(bp.mesh, bp.radius, bp.height, bp.tech)
        .unwrap_or_else(|| panic!("{} builds", bp.mesh))
}

fn every_model() -> Vec<Model> {
    let mut models: Vec<Model> = all_model_keys()
        .into_iter()
        .map(|key| build_model(key).expect(key))
        .collect();
    models.extend(BLUEPRINTS.iter().filter(|bp| bp.tech > 1).map(built));
    models
}

fn triangles(mesh: &MeshLod) -> usize {
    mesh.indices.len() / 3
}

/// Closest point to `p` on triangle `abc` (Ericson, Real-Time Collision Detection 5.1.5).
fn closest_point_on_triangle(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

fn position(mesh: &MeshLod, index: u32) -> Vec3 {
    Vec3::from(mesh.vertices[index as usize].pos)
}

#[test]
fn every_key_builds() {
    let keys = all_model_keys();
    for bp in BLUEPRINTS {
        assert!(keys.contains(&bp.mesh), "{} is listed", bp.mesh);
    }
    for key in PROP_KEYS {
        assert!(keys.contains(key), "{key} is listed");
    }
    for key in &keys {
        let model = build_model(key).unwrap_or_else(|| panic!("{key} builds"));
        assert_eq!(model.key, *key);
        assert_eq!(
            keys.iter().filter(|k| k == &key).count(),
            1,
            "{key} is listed once"
        );
    }
    assert!(build_model("no_such_model").is_none());
    assert!(build_model_scaled("no_such_model", 1.0, 1.0, 1).is_none());
}

#[test]
fn prop_kinds_map_to_models() {
    // mc_map::PropKind raw values.
    for raw in [0u16, 1, 2, 3, 16, 17, 32, 33, 34, 35] {
        let key = prop_model_key(raw);
        assert!(build_model(key).is_some(), "prop kind {raw} -> {key}");
        let family = match raw {
            0..=15 => "tree_",
            16..=31 => "rock_",
            _ => "building_",
        };
        assert!(key.starts_with(family), "prop kind {raw} -> {key}");
    }
    assert_eq!(prop_model_key(1), "tree_conifer");
    assert_eq!(prop_model_key(35), "building_tower");
    assert!(build_model(prop_model_key(999)).is_some());
}

#[test]
fn meshes_are_valid() {
    for model in every_model() {
        for (lod, mesh) in model.lods.iter().enumerate() {
            let name = format!("{} lod{lod}", model.key);
            assert!(
                !mesh.indices.is_empty() && mesh.indices.len().is_multiple_of(3),
                "{name}: indices"
            );
            assert!(
                mesh.indices
                    .iter()
                    .all(|&i| (i as usize) < mesh.vertices.len()),
                "{name}: index range"
            );
            for v in &mesh.vertices {
                assert!(
                    v.pos
                        .iter()
                        .chain(&v.normal)
                        .chain(&v.uv)
                        .all(|c| c.is_finite()),
                    "{name}: finite"
                );
                assert!(
                    (Vec3::from(v.normal).length() - 1.0).abs() < 1e-4,
                    "{name}: unit normal"
                );
                assert!(
                    v.material <= material::PLATING_DARK && v.part <= part::LOCOMOTION,
                    "{name}: ids"
                );
                // Units stand on the ground; props are rooted a little into it for slopes.
                let is_prop = ["tree_", "rock_", "building_"]
                    .iter()
                    .any(|family| model.key.starts_with(family));
                let floor = if is_prop { -3.0 } else { -1e-3 };
                assert!(v.pos[2] >= floor, "{name}: below ground");
            }
            for t in mesh.indices.chunks(3) {
                let [a, b, c] = [
                    position(mesh, t[0]),
                    position(mesh, t[1]),
                    position(mesh, t[2]),
                ];
                let geometric = (b - a).cross(c - a);
                assert!(
                    geometric.length() * 0.5 > 1e-7,
                    "{name}: degenerate triangle at {a}"
                );
                // Front faces are counter-clockwise: the winding normal agrees with the shading normal.
                for &i in t {
                    let shading = Vec3::from(mesh.vertices[i as usize].normal);
                    assert!(
                        geometric.normalize().dot(shading) > 0.999,
                        "{name}: winding disagrees with normal at {a}"
                    );
                }
                let [v0, v1, v2] = [t[0], t[1], t[2]].map(|i| mesh.vertices[i as usize]);
                assert!(
                    v0.material == v1.material && v1.material == v2.material,
                    "{name}: one material per triangle"
                );
                assert!(
                    v0.part == v1.part && v1.part == v2.part,
                    "{name}: one part per triangle"
                );
            }
        }
    }
}

/// Every closed solid has positive signed volume: its faces point outward.
#[test]
fn solids_face_outward() {
    for key in all_model_keys() {
        for tech in 1..=3 {
            for lod in 0..LOD_COUNT {
                let builder = build_lod(key, lod, tech);
                let mesh = builder.mesh();
                for range in builder.solids() {
                    let volume: f32 = mesh.indices[range.clone()]
                        .chunks(3)
                        .map(|t| {
                            position(mesh, t[0])
                                .dot(position(mesh, t[1]).cross(position(mesh, t[2])))
                        })
                        .sum();
                    assert!(
                        volume > 0.0,
                        "{key} tech{tech} lod{lod}: inside-out solid at {}",
                        position(mesh, mesh.indices[range.start])
                    );
                }
            }
        }
    }
}

#[test]
fn lods_reduce_and_respect_budgets() {
    for model in every_model() {
        let [full, reduced, coarse] = [0, 1, 2].map(|lod| triangles(&model.lods[lod]));
        assert!(
            full >= reduced && reduced >= coarse,
            "{}: {full} >= {reduced} >= {coarse}",
            model.key
        );
        assert!(
            coarse < 60,
            "{}: coarse LOD has {coarse} triangles",
            model.key
        );
        assert!(full <= 2600, "{}: full LOD has {full} triangles", model.key);
        assert!(
            reduced as f32 <= full as f32 * 0.45 + 20.0,
            "{}: reduced LOD {reduced} of {full}",
            model.key
        );
    }
    for bp in BLUEPRINTS {
        let full = triangles(&built(bp).lods[0]);
        assert!(
            full >= 250,
            "{}: only {full} triangles at full detail",
            bp.mesh
        );
    }
}

#[test]
fn bounds_hold_every_lod() {
    for model in every_model() {
        assert!(
            model.bounds_radius.is_finite() && model.bounds_radius > 0.5,
            "{}",
            model.key
        );
        for mesh in &model.lods {
            for v in &mesh.vertices {
                assert!(
                    Vec3::from(v.pos).length() <= model.bounds_radius + 1e-3,
                    "{}",
                    model.key
                );
            }
        }
        // A turret vertex stays inside at the worst yaw: swung directly away from the origin.
        let pivot = Vec3::from(model.turret_pivot);
        for v in model.lods[0]
            .vertices
            .iter()
            .filter(|v| v.part == part::TURRET)
        {
            let p = Vec3::from(v.pos);
            let reach = pivot.truncate().length() + (p - pivot).truncate().length();
            assert!(
                reach.hypot(p.z) <= model.bounds_radius + 1e-3,
                "{}",
                model.key
            );
        }
    }
}

#[test]
fn models_fit_their_blueprints() {
    for bp in BLUEPRINTS {
        let model = built(bp);
        for (lod, mesh) in model.lods.iter().enumerate() {
            let name = format!("{} tech{} lod{lod}", bp.mesh, bp.tech);
            let top = mesh.vertices.iter().map(|v| v.pos[2]).fold(0.0, f32::max);
            assert!(
                top <= bp.height * 1.25,
                "{name}: top {top} over height {}",
                bp.height
            );
            assert!(
                top >= bp.height * 0.8,
                "{name}: top {top} well short of height {}",
                bp.height
            );
            let reach = mesh
                .vertices
                .iter()
                .map(|v| v.pos[0].hypot(v.pos[1]))
                .fold(0.0, f32::max);
            match bp.footprint {
                Some((cx, cy)) => {
                    let half = mc_map::BUILD_CELL_M as f32 * 0.5;
                    let (hx, hy) = (cx as f32 * half, cy as f32 * half);
                    let (x, y) = mesh.vertices.iter().fold((0.0f32, 0.0f32), |(x, y), v| {
                        (x.max(v.pos[0].abs()), y.max(v.pos[1].abs()))
                    });
                    // Slewing gun barrels may overhang the lot; nothing else may.
                    let barrel = bp
                        .muzzles
                        .iter()
                        .map(|m| m[0].hypot(m[1]))
                        .fold(0.0, f32::max);
                    assert!(
                        x <= hx.max(barrel + 0.5) && y <= hy,
                        "{name}: extent {x} x {y} outside footprint {hx} x {hy}"
                    );
                    assert!(
                        x >= hx * 0.6 && y >= hy * 0.55,
                        "{name}: extent {x} x {y} too small for footprint"
                    );
                }
                None => {
                    assert!(
                        reach <= bp.radius * 1.3,
                        "{name}: reach {reach} over radius {}",
                        bp.radius
                    );
                    assert!(
                        reach >= bp.radius * 0.75,
                        "{name}: reach {reach} too small for radius {}",
                        bp.radius
                    );
                }
            }
        }
    }
}

#[test]
fn units_wear_team_colour_at_every_lod() {
    for bp in BLUEPRINTS {
        let model = built(bp);
        for (lod, mesh) in model.lods.iter().enumerate() {
            // It has to read from above: some team-coloured face must point mostly upward.
            let seen_from_above = mesh
                .vertices
                .iter()
                .any(|v| v.material == material::TEAM && v.normal[2] > 0.5);
            assert!(
                seen_from_above,
                "{} lod{lod}: no upward team colour",
                bp.mesh
            );
            assert!(
                mesh.vertices
                    .iter()
                    .any(|v| v.material == material::PLATING),
                "{} lod{lod}: no plating",
                bp.mesh
            );
        }
    }
}

#[test]
fn weapons_are_turrets_ending_at_the_muzzle() {
    for bp in BLUEPRINTS.iter().filter(|bp| !bp.muzzles.is_empty()) {
        let model = built(bp);
        let weapon_part = if bp.turreted {
            part::TURRET
        } else {
            part::HULL
        };
        for (lod, mesh) in model.lods.iter().enumerate() {
            let has_turret = mesh.vertices.iter().any(|v| v.part == part::TURRET);
            assert_eq!(has_turret, bp.turreted, "{} lod{lod}: turret part", bp.mesh);
            for muzzle in bp.muzzles {
                let muzzle_point = Vec3::from(*muzzle);
                let nearest = mesh
                    .indices
                    .chunks(3)
                    .filter(|t| mesh.vertices[t[0] as usize].part == weapon_part)
                    .map(|t| {
                        closest_point_on_triangle(
                            muzzle_point,
                            position(mesh, t[0]),
                            position(mesh, t[1]),
                            position(mesh, t[2]),
                        )
                        .distance(muzzle_point)
                    })
                    .fold(f32::MAX, f32::min);
                assert!(
                    nearest < 0.4,
                    "{} lod{lod}: barrel ends {nearest} m from muzzle {muzzle:?}",
                    bp.mesh
                );
                // Nothing of the weapon pokes far beyond the muzzle either.
                if bp.turreted {
                    let overshoot = mesh
                        .vertices
                        .iter()
                        .filter(|v| v.part == part::TURRET)
                        .map(|v| v.pos[0] - muzzle[0])
                        .fold(f32::MIN, f32::max);
                    assert!(
                        overshoot < 0.5,
                        "{} lod{lod}: weapon overshoots muzzle by {overshoot}",
                        bp.mesh
                    );
                }
            }
        }
        if bp.turreted {
            // The sim rotates muzzle offsets about the unit origin, so the turret must too.
            assert!(
                Vec3::from(model.turret_pivot).truncate().length() < 1e-4,
                "{}: turret axis off the origin",
                bp.mesh
            );
        }
    }
    for bp in BLUEPRINTS.iter().filter(|bp| bp.muzzles.is_empty()) {
        assert!(
            built(bp)
                .lods
                .iter()
                .all(|m| m.vertices.iter().all(|v| v.part != part::TURRET)),
            "{}: unarmed but has a turret",
            bp.mesh
        );
    }
}

#[test]
fn commander_sole_is_recorded_for_ground_marks() {
    let legs = build_model("commander")
        .unwrap()
        .legs
        .expect("commander walks");
    assert!(
        legs.foot[2] > 1.0,
        "sole wide enough to stamp: {:?}",
        legs.foot
    );
    assert!(
        legs.foot[1] - legs.foot[0] > 2.0,
        "sole long enough to stamp: {:?}",
        legs.foot
    );
}

#[test]
fn spinners_and_locomotion_are_tagged() {
    let has = |key: &str, part: u32| {
        build_model(key)
            .unwrap()
            .lods
            .iter()
            .all(|m| m.vertices.iter().any(|v| v.part == part))
    };
    for key in ["extractor", "radar"] {
        assert!(has(key, part::SPINNER), "{key} spins");
        assert!(
            build_model(key).unwrap().spinner_pivot[2] > 1.0,
            "{key} spinner pivot"
        );
    }
    for bp in BLUEPRINTS.iter().filter(|bp| bp.footprint.is_none()) {
        assert!(
            has(bp.mesh, part::LOCOMOTION),
            "{} has running gear",
            bp.mesh
        );
    }
    for bp in BLUEPRINTS.iter().filter(|bp| bp.footprint.is_some()) {
        assert!(
            !has(bp.mesh, part::LOCOMOTION),
            "{} is a structure",
            bp.mesh
        );
    }
}

#[test]
fn orange_weapons_glow_orange() {
    let orange = [
        "bot_light",
        "artillery_light",
        "missile_launcher",
        "artillery_static",
        "reclaimer",
    ];
    for bp in BLUEPRINTS.iter().filter(|bp| !bp.muzzles.is_empty()) {
        let mesh = &built(bp).lods[0];
        let count = |m: u32| mesh.vertices.iter().filter(|v| v.material == m).count();
        // The plain tech 1 tank carries no emitters at all: a dark bore, nothing lit.
        if bp.mesh == "tank_light" {
            assert_eq!(
                count(material::GLOW_ORANGE) + count(material::GLOW),
                0,
                "{}: lit",
                bp.mesh
            );
            continue;
        }
        assert_eq!(
            count(material::GLOW_ORANGE) > 0,
            orange.contains(&bp.mesh),
            "{}",
            bp.mesh
        );
        if !orange.contains(&bp.mesh) {
            assert!(count(material::GLOW) > 0, "{}: blue emitters", bp.mesh);
        }
    }
}

#[test]
fn extractor_next_tier_is_upgrade_pieces() {
    let has = |tech: u8| {
        build_model_scaled("extractor", 14.0, 9.0, tech)
            .unwrap()
            .lods[0]
            .vertices
            .iter()
            .any(|v| v.rig & rig::UPGRADE != 0)
    };
    assert!(has(1), "T1 carries the T2 kit as upgrade pieces");
    assert!(has(2), "T2 carries the T3 kit as upgrade pieces");
    assert!(!has(3), "T3 is finished");
}

#[test]
fn higher_tech_adds_highlights() {
    let glow = |key: &str, tech: u8| {
        let model = build_model_scaled(key, 10.0, 10.0, tech).unwrap();
        model.lods[0]
            .indices
            .chunks(3)
            .filter(|t| model.lods[0].vertices[t[0] as usize].material == material::GLOW)
            .count()
    };
    for key in ["engineer", "factory_land", "extractor", "power", "radar"] {
        let [t1, t2, t3] = [1, 2, 3].map(|tech| glow(key, tech));
        assert!(
            t1 > 0 && t1 < t2 && t2 < t3,
            "{key}: glow triangles {t1}, {t2}, {t3}"
        );
    }
}

#[test]
fn scaling_fits_radius_and_height() {
    let base = build_model("tank_light").unwrap();
    let big = build_model_scaled("tank_light", 9.2, 5.1, 1).unwrap();
    for (a, b) in base.lods[0].vertices.iter().zip(&big.lods[0].vertices) {
        assert!(
            (a.pos[0] * 2.0 - b.pos[0]).abs() < 1e-4 && (a.pos[2] * 1.5 - b.pos[2]).abs() < 1e-4,
            "{:?} against {:?}",
            a.pos,
            b.pos
        );
    }
    assert!((big.turret_pivot[2] - base.turret_pivot[2] * 1.5).abs() < 1e-4);
    assert!(big.bounds_radius > base.bounds_radius * 1.5);
}

/// Writes every model's LOD0 as OBJ (+ shared MTL), a triangle-count table and
/// RTS-camera preview images to `target/model-dump/`:
/// `cargo test -p mc-render -- --ignored dump_models`
#[test]
#[ignore = "writes inspection files to target/model-dump"]
fn dump_models() {
    let dir = std::env::var_os("MODEL_DUMP_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
            manifest
                .ancestors()
                .find(|p| p.join("Cargo.lock").exists())
                .unwrap_or(manifest)
                .join("target/model-dump")
        });
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("aster.mtl"), preview::mtl_text()).unwrap();

    let mut table = String::from("model                    lod0   lod1   lod2  bounds\n");
    let mut dump = |name: &str, model: &Model| {
        std::fs::write(
            dir.join(format!("{name}.obj")),
            preview::obj_text(model, 0, "aster.mtl"),
        )
        .unwrap();
        preview::render(&model.lods[0], 512, -38.0)
            .write_ppm(&dir.join(format!("{name}.ppm")))
            .unwrap();
        preview::render(&model.lods[0], 512, 142.0)
            .write_ppm(&dir.join(format!("{name}_rear.ppm")))
            .unwrap();
        for lod in 1..LOD_COUNT {
            preview::render(&model.lods[lod], 256, -38.0)
                .write_ppm(&dir.join(format!("{name}_lod{lod}.ppm")))
                .unwrap();
        }
        let [a, b, c] = [0, 1, 2].map(|lod| triangles(&model.lods[lod]));
        table.push_str(&format!(
            "{name:<22} {a:>6} {b:>6} {c:>6} {:>7.1}\n",
            model.bounds_radius
        ));
    };
    for key in all_model_keys() {
        dump(key, &build_model(key).unwrap());
    }
    for bp in BLUEPRINTS.iter().filter(|bp| {
        bp.tech > 1
            && BLUEPRINTS
                .iter()
                .any(|other| other.mesh == bp.mesh && other.tech < bp.tech)
    }) {
        dump(&format!("{}_t{}", bp.mesh, bp.tech), &built(bp));
    }
    std::fs::write(dir.join("triangles.txt"), &table).unwrap();
    println!("{table}\nwrote {}", dir.display());
}
