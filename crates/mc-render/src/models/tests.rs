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

const fn hull_unit(
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
        turreted: false,
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
    unit("commander", 10.4, 24.0, 1, &[[8.8, -4.8, 15.52]]),
    unit("engineer", 3.6, 3.4, 1, &[]),
    unit("engineer", 4.2, 3.8, 2, &[]),
    unit("engineer", 4.8, 4.2, 3, &[]),
    unit("scout", 2.4, 1.8, 1, &[[1.2, 0.0, 1.6]]),
    unit("bot_light", 2.6, 5.2, 1, &[[1.4, 0.0, 4.2]]),
    unit("tank_light", 4.6, 3.4, 1, &[[5.2, 0.0, 2.82]]),
    unit("artillery_light", 4.2, 3.2, 1, &[[3.8, 0.0, 3.4]]),
    unit(
        "tank_heavy",
        6.2,
        4.4,
        2,
        &[[8.0, -0.6, 3.8], [8.0, 0.6, 3.8]],
    ),
    unit(
        "hover_tank",
        5.4,
        3.2,
        2,
        &[[4.15, -0.22, 2.30], [4.15, 0.22, 2.30]],
    ),
    unit(
        "missile_launcher",
        5.2,
        4.0,
        2,
        &[
            [2.49, -1.12, 2.32],
            [2.49, 0.0, 2.32],
            [2.49, 1.12, 2.32],
            [2.49, -1.12, 2.88],
            [2.49, 0.0, 2.88],
            [2.49, 1.12, 2.88],
        ],
    ),
    unit(
        "assault_bot",
        13.6,
        24.0,
        3,
        // The arc projectors. The shin torpedo tubes ride the legs, not the turret:
        // `paladin_shin_tubes_reach_their_muzzles`.
        &[[6.8, -7.2, 18.0], [6.8, 7.2, 18.0]],
    ),
    unit("artillery_heavy", 10.5, 7.5, 3, &[[12.75, 0.0, 7.8]]),
    // Long guns: the bore reaches well past the hull (`models_fit_their_blueprints`).
    unit("bore_tank", 8.2, 4.8, 3, &[[11.5, 0.0, 3.6]]),
    // The main turret's AEB-2; the sponson and flak houses are
    // `fulgur_houses_and_muzzles`.
    unit("assault_tank", 31.35, 24.75, 4, &[[36.3, 4.125, 20.295]]),
    unit(
        "interceptor",
        3.6,
        1.8,
        1,
        &[[3.55, -0.22, 1.12], [3.55, 0.22, 1.12]],
    ),
    hull_unit(
        "bomber",
        5.6,
        2.6,
        1,
        &[
            [1.4, -1.05, 0.42],
            [1.4, -0.35, 0.42],
            [1.4, 0.35, 0.42],
            [1.4, 1.05, 0.42],
            [-0.2, -1.05, 0.42],
            [-0.2, -0.35, 0.42],
            [-0.2, 0.35, 0.42],
            [-0.2, 1.05, 0.42],
        ],
    ),
    // Naval: a hull's origin is its waterline. The frigate's AA mount is on the hull
    // (`frigate_aa_mount_turns_on_its_own`).
    unit("attack_boat", 6.0, 4.0, 1, &[[4.6, 0.0, 3.1]]),
    unit("frigate", 15.0, 10.0, 1, &[[13.4, 0.0, 4.1]]),
    hull_unit(
        "submarine",
        10.0,
        3.6,
        1,
        &[[9.6, -0.7, -0.8], [9.6, 0.7, -0.8], [9.6, -0.7, -1.6], [9.6, 0.7, -1.6]],
    ),
    structure("sonar", 6.0, 12.0, 1, 1, &[]),
    structure("sonar", 6.0, 15.0, 2, 1, &[]),
    structure("sonar", 6.0, 18.0, 3, 1, &[]),
    // The rest of the roster (docs/NAVY.md): guns on houses of their own (`rig::HOUSE_*`),
    // no `part::TURRET`, so `hull_unit`. Muzzles are weapon 0's, as authored on the model
    // (a `rear` weapon's muzzles are given to the sim mirrored; here they are as drawn).
    hull_unit("salvage_boat", 8.0, 6.0, 1, &[]),
    hull_unit("destroyer", 22.0, 12.0, 2, &[[20.5, -0.5, 5.2], [20.5, 0.5, 5.2]]),
    hull_unit("aa_cruiser", 22.0, 14.0, 2, &[[6.0, -2.0, 6.4], [6.0, 2.0, 6.4], [4.0, -2.0, 6.4], [4.0, 2.0, 6.4]]),
    hull_unit(
        "missile_ship",
        20.0,
        10.0,
        2,
        &[[2.0, -1.5, 6.0], [2.0, 1.5, 6.0], [0.0, -1.5, 6.0], [0.0, 1.5, 6.0], [-2.0, -1.5, 6.0], [-2.0, 1.5, 6.0], [-4.0, -1.5, 6.0], [-4.0, 1.5, 6.0]],
    ),
    hull_unit(
        "submarine_hunter",
        14.0,
        4.2,
        2,
        &[[13.4, -0.9, -1.0], [13.4, 0.9, -1.0], [13.4, -0.9, -1.9], [13.4, 0.9, -1.9], [13.4, -0.9, -2.8], [13.4, 0.9, -2.8]],
    ),
    hull_unit("shield_boat", 16.0, 12.0, 2, &[]),
    hull_unit("battleship", 58.0, 26.0, 3, &[[48.0, -2.2, 12.5], [48.0, 0.0, 12.5], [48.0, 2.2, 12.5]]),
    hull_unit("carrier", 60.0, 24.0, 3, &[[-12.0, 8.0, 8.5], [-14.0, 8.0, 8.5], [-12.0, -8.0, 8.5], [-14.0, -8.0, 8.5]]),
    hull_unit(
        "submarine_strategic",
        30.0,
        5.0,
        3,
        &[[28.0, -1.0, -1.5], [28.0, 1.0, -1.5], [28.0, -2.2, -1.5], [28.0, 2.2, -1.5], [28.0, -1.0, -2.6], [28.0, 1.0, -2.6], [28.0, -2.2, -2.6], [28.0, 2.2, -2.6]],
    ),
    structure("factory_land", 46.0, 28.0, 1, 8, &[]),
    structure("factory_land", 46.0, 34.0, 2, 8, &[]),
    structure("factory_land", 46.0, 42.0, 3, 8, &[]),
    structure("factory_air", 46.0, 26.0, 1, 8, &[]),
    structure("factory_air", 46.0, 34.0, 2, 8, &[]),
    structure("factory_air", 46.0, 42.0, 3, 8, &[]),
    structure("factory_naval", 46.0, 26.0, 1, 8, &[]),
    structure("factory_naval", 46.0, 34.0, 2, 8, &[]),
    structure("factory_naval", 46.0, 42.0, 3, 8, &[]),
    structure("extractor", 10.5, 6.75, 1, 2, &[]),
    structure("extractor", 10.5, 8.25, 2, 2, &[]),
    structure("extractor", 10.5, 9.75, 3, 2, &[]),
    structure("core_mine", 12.8, 12.6, 1, 3, &[]),
    structure("core_mine", 12.8, 20.8, 2, 3, &[]),
    structure("core_mine", 12.8, 22.3, 3, 3, &[]),
    structure("core_mine", 12.8, 22.3, 4, 3, &[]),
    structure("power", 6.9, 7.0, 1, 2, &[]),
    structure("power", 18.75, 18.0, 2, 4, &[]),
    structure("power", 42.5, 35.0, 3, 8, &[]),
    structure("storage_mass", 12.9, 6.3, 1, 3, &[]),
    structure("storage_mass", 12.9, 10.2, 2, 3, &[]),
    structure("storage_mass", 12.9, 14.9, 3, 3, &[]),
    structure("storage_energy", 14.2, 10.2, 1, 3, &[]),
    structure("turret", 5.25, 6.75, 1, 1, &[[7.5, 0.0, 5.55]]),
    structure(
        "turret_heavy",
        10.5,
        9.75,
        2,
        2,
        &[[9.9, -1.5, 7.5], [9.9, 0.0, 7.5], [9.9, 1.5, 7.5]],
    ),
    structure("artillery_static", 10.5, 9.0, 2, 2, &[[10.2, 0.0, 8.7]]),
    structure("radar", 10.5, 35.0, 1, 2, &[]),
    structure("radar", 10.5, 42.0, 2, 2, &[]),
    structure("radar", 10.5, 49.0, 3, 2, &[]),
    structure("reclaimer", 8.25, 9.0, 2, 2, &[[12.0, 0.0, 6.45]]),
    structure("reclaimer", 8.25, 10.5, 3, 2, &[[12.0, 0.0, 6.45]]),
    structure("shield", 13.9, 40.0, 2, 3, &[]),
    structure("shield", 13.9, 52.0, 3, 3, &[]),
    structure("wall", 6.0, 4.5, 1, 1, &[]),
];

/// Ships: the keel is below the waterline (model z = 0), and nothing is running gear.
const NAVAL_HULLS: &[&str] = &[
    "attack_boat",
    "frigate",
    "submarine",
    "salvage_boat",
    "destroyer",
    "aa_cruiser",
    "missile_ship",
    "submarine_hunter",
    "shield_boat",
    "battleship",
    "carrier",
    "submarine_strategic",
];
/// The capital ships: 120 m hulls with the triangle budget of a factory.
const CAPITAL_SHIPS: &[&str] = &["battleship", "carrier"];
/// Tech 2 and 3 warships under 70 m: bigger than any land unit, few of them, a budget between.
const WARSHIP_TRIANGLES: usize = 3600;
const WARSHIPS: &[&str] = &["destroyer", "aa_cruiser", "missile_ship", "shield_boat", "submarine_hunter", "submarine_strategic"];

/// Guns on a ship turn about their `pivot` in the unit file, not the hull's origin.
fn naval_gun_pivot(mesh: &str) -> Option<[f32; 2]> {
    match mesh {
        "attack_boat" => Some([3.3, 0.0]),
        "frigate" => Some([8.8, 0.0]),
        _ => None,
    }
}

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
                    v.material <= material::GLOW_VIOLET && (v.part <= part::CRADLE || (part::RAM..=part::GEAR_DOOR).contains(&v.part)),
                    "{name}: ids"
                );
                // Units stand on the ground; props are rooted a little into it for slopes.
                let is_prop = ["tree_", "rock_", "building_"]
                    .iter()
                    .any(|family| model.key.starts_with(family));
                // The naval yard stands in water on piles driven into the seabed.
                let floor = if is_prop {
                    -3.0
                } else if model.key == "factory_naval" {
                    -91.0
                } else if CAPITAL_SHIPS.contains(&model.key.as_str()) || model.key == "submarine_strategic" {
                    // A capital ship's keel, or a big submarine's hull, runs deep.
                    -12.0
                } else if NAVAL_HULLS.contains(&model.key.as_str()) {
                    // Hulls float: the keel is under the waterline.
                    -4.5
                } else if model.key == "sonar" {
                    // The sonar buoy's hydrophone arrays hang under it.
                    -15.0
                } else if model.key == "core_mine" {
                    // The pit, the bore and the pipe down it (`Model::pit`), and the stilts
                    // an offshore one stands on.
                    -170.0
                } else {
                    -1e-3
                };
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

/// Factories and core mines are the largest models by far (96 and 84 m lots) and
/// there are few of them.
const FACTORY_TRIANGLES: usize = 6000;
const CORE_MINE_TRIANGLES: usize = 9000;

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
        let budget = if model.key == "core_mine" {
            // The pit it digs is real geometry, down to the deep core's shaft.
            CORE_MINE_TRIANGLES
        } else if model.key == "replication_engine" {
            // One 240 m landmark per match (Survival).
            super::replicator::ENGINE_TRIANGLES
        } else if model.key.starts_with("factory_") || model.key == "power" || model.key == "airbase" {
            // The tech 3 reactor stands on a factory's lot; the airbase's shaft is real geometry.
            FACTORY_TRIANGLES
        } else if model.key == "lift_ship" {
            // 300 m capital hull: full ventral bay, four drive bells, four gun houses, articulated gear.
            14000
        } else if model.key == "light_transport" {
            // 115 m spacecraft: walk-through bay, two drive bells, lift jets, dorsal mast.
            9000
        } else if CAPITAL_SHIPS.contains(&model.key.as_str()) {
            FACTORY_TRIANGLES
        } else if WARSHIPS.contains(&model.key.as_str()) {
            WARSHIP_TRIANGLES
        } else {
            2600
        };
        // A core mine draws either its stilts or its pit, never both.
        let drawn = if model.key == "core_mine" {
            let of = |kind| {
                let mesh = &model.lods[0];
                mesh.indices.chunks(3).filter(|t| mesh.vertices[t[0] as usize].part == kind).count()
            };
            full - of(part::AFLOAT).min(of(part::ASHORE))
        } else {
            full
        };
        assert!(drawn <= budget, "{}: full LOD draws {drawn} triangles", model.key);
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
        // Down a pit, and under the sea on stilts, is only ever seen through what the bounds hold.
        let hidden_below = |v: &super::MeshVertex| {
            model.pit.is_some_and(|pit| {
                v.pos[2] < pit.open
                    && (v.part == part::AFLOAT || Vec3::from(v.pos).truncate().length() <= pit.radius)
            })
        };
        for mesh in &model.lods {
            for v in mesh.vertices.iter().filter(|v| !hidden_below(v)) {
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
            // The next tier's refit pieces stand to that tier's height, not this one's.
            let top = mesh
                .vertices
                .iter()
                .filter(|v| v.rig & rig::UPGRADE == 0)
                .map(|v| v.pos[2])
                .fold(0.0, f32::max);
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
                    // Economy structures stand back from the lot edge: the apron is walkable.
                    assert!(
                        x >= hx * 0.55 && y >= hy * 0.55,
                        "{name}: extent {x} x {y} too small for footprint"
                    );
                }
                None => {
                    // A long gun may reach past the hull (the Arbalest's and the Fulgur's
                    // bores); nothing else may.
                    let barrel = bp
                        .muzzles
                        .iter()
                        .map(|m| m[0].hypot(m[1]))
                        .fold(0.0, f32::max);
                    let hull_reach = mesh
                        .vertices
                        .iter()
                        .filter(|v| v.part != part::TURRET)
                        .map(|v| v.pos[0].hypot(v.pos[1]))
                        .fold(0.0, f32::max);
                    assert!(
                        hull_reach <= bp.radius * 1.3 && reach <= (bp.radius * 1.3).max(barrel + 0.5),
                        "{name}: reach {reach} (hull {hull_reach}) over radius {}",
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
        if let Some(pivot) = naval_gun_pivot(bp.mesh) {
            let at = Vec3::from(model.turret_pivot);
            assert!(
                (at.x - pivot[0]).abs() < 1e-3 && (at.y - pivot[1]).abs() < 1e-3,
                "{}: turret axis {at} off the gun's pivot",
                bp.mesh
            );
        } else if bp.turreted {
            // The sim rotates muzzle offsets about the unit origin, so the turret must too.
            assert!(
                Vec3::from(model.turret_pivot).truncate().length() < 1e-4,
                "{}: turret axis off the origin",
                bp.mesh
            );
        }
    }
    for key in [
        "artillery_light",
        "artillery_heavy",
        "artillery_static",
        "missile_launcher",
    ] {
        let model = build_model(key).unwrap();
        assert!(model.arm_pivot.is_some(), "{key}: howitzer has no trunnion");
        assert!(
            model.lods[0]
                .vertices
                .iter()
                .any(|v| v.part == part::TURRET && (v.rig & rig::LIMB_MASK) == rig::ARM_GUN),
            "{key}: barrel is not a pitching gun arm"
        );
    }
    for key in ["artillery_static", "artillery_heavy"] {
        let model = build_model(key).unwrap();
        assert!(model.recoil.is_some(), "{key} barrel has no recoil travel");
        let travel = model.recoil.unwrap()[3];
        assert!(travel > 1.0, "{key} recoil travel {travel}");
        let slides = model.lods[0]
            .vertices
            .iter()
            .filter(|v| v.rig & rig::RECOIL != 0)
            .collect::<Vec<_>>();
        assert!(!slides.is_empty(), "{key} barrel is not tagged to recoil");
        assert!(
            slides
                .iter()
                .all(|v| (v.rig & rig::LIMB_MASK) == rig::ARM_GUN),
            "{key}: recoiling verts must ride the gun arm"
        );
    }
    let onager = build_model("artillery_static").unwrap();
    let count = |m| {
        onager.lods[0]
            .vertices
            .iter()
            .filter(|v| v.material == m)
            .count()
    };
    assert!(
        count(material::PLATING) > 80,
        "onager needs white plating: {}",
        count(material::PLATING)
    );
    assert!(
        count(material::ACCENT) > 80,
        "onager needs a black frame: {}",
        count(material::ACCENT)
    );
    assert_eq!(
        count(material::GLOW_ORANGE) + count(material::GLOW),
        0,
        "onager has no emitters"
    );
    assert_eq!(
        count(material::METAL),
        0,
        "onager metal is black, not gunmetal"
    );
    let jacket = onager.lods[0]
        .vertices
        .iter()
        .filter(|v| v.rig & rig::RECOIL != 0 && v.material == material::PLATING)
        .count();
    assert!(jacket > 20, "onager barrel needs a white jacket: {jacket}");
    let sentinel = build_model("turret").unwrap();
    let scount = |m| {
        sentinel.lods[0]
            .vertices
            .iter()
            .filter(|v| v.material == m)
            .count()
    };
    assert_eq!(
        scount(material::GLOW_ORANGE) + scount(material::GLOW),
        0,
        "sentinel has no emitters"
    );
    assert!(
        scount(material::PLATING) > 80,
        "sentinel needs white plating: {}",
        scount(material::PLATING)
    );
    assert!(
        scount(material::METAL) > 0,
        "sentinel barrel is a clean steel tube, not panelled black"
    );
    for bp in BLUEPRINTS
        .iter()
        .filter(|bp| bp.muzzles.is_empty() && bp.mesh != "engineer")
    {
        assert!(
            built(bp)
                .lods
                .iter()
                .all(|m| m.vertices.iter().all(|v| v.part != part::TURRET)),
            "{}: unarmed but has a turret",
            bp.mesh
        );
    }
    let mason = build_model("engineer").unwrap();
    assert!(mason.arm_pivot.is_some(), "engineer has no build-arm elbow");
    assert!(mason.arm_boom, "engineer boom is not two-bone");
    assert!(
        mason.treads.is_some(),
        "engineer crawls; it should stamp tracks on land"
    );
    assert!(
        Vec3::from(mason.turret_pivot).truncate().length() < 1e-4,
        "engineer: turret axis off the origin"
    );
    assert!(
        mason.lods.iter().all(|m| m
            .vertices
            .iter()
            .any(|v| v.part == part::TURRET && (v.rig & rig::LIMB_MASK) == rig::ARM_BOOM)),
        "engineer: boom is not a pitching upper arm"
    );
    assert!(
        mason.lods.iter().all(|m| m
            .vertices
            .iter()
            .any(|v| v.part == part::TURRET && (v.rig & rig::LIMB_MASK) == rig::ARM_TOOL)),
        "engineer: projector is not a pitching build arm"
    );
    assert!(
        mason
            .lods
            .iter()
            .any(|m| m.vertices.iter().any(|v| v.rig & rig::FLOAT != 0)),
        "engineer: no float skirt"
    );
}

#[test]
fn siege_guns_share_a_carrier() {
    for key in ["artillery_static", "artillery_heavy", "turret_heavy"] {
        let model = build_model(key).unwrap();
        let [full, reduced, coarse] = [0, 1, 2].map(|lod| triangles(&model.lods[lod]));
        assert!(full <= 2600, "{key} full LOD has {full} triangles");
        assert!(coarse < 60, "{key} coarse LOD has {coarse} triangles");
        assert!(
            reduced as f32 <= full as f32 * 0.45 + 20.0,
            "{key} reduced LOD {reduced} of {full}"
        );
    }
    let onager = build_model("artillery_static").unwrap();
    let count = |m| {
        onager.lods[0]
            .vertices
            .iter()
            .filter(|v| v.material == m)
            .count()
    };
    assert_eq!(
        count(material::GLOW_ORANGE) + count(material::GLOW),
        0,
        "onager has no emitters"
    );
    assert_eq!(
        count(material::METAL),
        0,
        "onager metal is black, not gunmetal"
    );
    let jacket = onager.lods[0]
        .vertices
        .iter()
        .filter(|v| v.rig & rig::RECOIL != 0 && v.material == material::PLATING)
        .count();
    assert!(jacket > 20, "onager barrel needs a white jacket: {jacket}");
    assert!(
        onager.arm_pivot.is_some() && onager.recoil.is_some(),
        "onager keeps a pitching, recoiling tube"
    );
    for bp in BLUEPRINTS.iter().filter(|bp| {
        matches!(
            bp.mesh,
            "artillery_static" | "artillery_heavy" | "turret_heavy"
        )
    }) {
        let model = built(bp);
        let top = model.lods[0]
            .vertices
            .iter()
            .map(|v| v.pos[2])
            .fold(0.0, f32::max);
        assert!(
            top <= bp.height * 1.25 && top >= bp.height * 0.8,
            "{} top {top} against height {}",
            bp.mesh,
            bp.height
        );
        for muzzle in bp.muzzles {
            let muzzle_point = Vec3::from(*muzzle);
            let nearest = model.lods[0]
                .indices
                .chunks(3)
                .filter(|t| model.lods[0].vertices[t[0] as usize].part == part::TURRET)
                .map(|t| {
                    closest_point_on_triangle(
                        muzzle_point,
                        position(&model.lods[0], t[0]),
                        position(&model.lods[0], t[1]),
                        position(&model.lods[0], t[2]),
                    )
                    .distance(muzzle_point)
                })
                .fold(f32::MAX, f32::min);
            assert!(
                nearest < 0.4,
                "{} barrel ends {nearest} m from muzzle {muzzle:?}",
                bp.mesh
            );
        }
    }
}

#[test]
fn trebuchet_outriggers_plant() {
    let model = build_model("artillery_heavy").unwrap();
    let deploy: Vec<_> = model.lods[0]
        .vertices
        .iter()
        .filter(|v| v.rig & rig::DEPLOY != 0)
        .collect();
    assert!(
        !deploy.is_empty(),
        "the Trebuchet's outriggers should plant"
    );
    let reach_y = deploy.iter().map(|v| v.pos[1].abs()).fold(0.0f32, f32::max);
    let reach_back = deploy.iter().map(|v| v.pos[0]).fold(f32::MAX, f32::min);
    assert!(
        reach_y > 7.5,
        "outriggers should reach past the tracks, got {reach_y}"
    );
    assert!(
        reach_back < -7.5,
        "the recoil spade should hang well behind the hull, got {reach_back}"
    );
    let track_y = model.lods[0]
        .vertices
        .iter()
        .filter(|v| v.part == part::LOCOMOTION)
        .map(|v| v.pos[1].abs())
        .fold(0.0f32, f32::max);
    assert!(
        track_y > 4.4 && track_y < 5.0,
        "treads should hug the carriage, got {track_y}"
    );
    let reach = model.lods[0]
        .vertices
        .iter()
        .map(|v| v.pos[0].hypot(v.pos[1]))
        .fold(0.0f32, f32::max);
    assert!(
        reach <= 7.0 * 1.3,
        "artillery_heavy reach {reach} over radius 7"
    );
    let full = triangles(&model.lods[0]);
    assert!(
        full <= 2600,
        "artillery_heavy full LOD has {full} triangles"
    );
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
fn commander_armour_is_black_and_white() {
    let model = build_model("commander").unwrap();
    for (lod, mesh) in model.lods.iter().enumerate() {
        let count = |m| mesh.vertices.iter().filter(|v| v.material == m).count();
        assert!(count(material::PLATING) > 0, "lod{lod}: no white plates");
        assert!(
            count(material::ACCENT) > 0,
            "lod{lod}: no black between the plates"
        );
        assert_eq!(
            count(material::PLATING_DARK),
            0,
            "lod{lod}: graphite is not the armour"
        );
    }
}

#[test]
fn paladin_takes_long_strides() {
    let bp = BLUEPRINTS
        .iter()
        .find(|bp| bp.mesh == "assault_bot")
        .expect("paladin");
    let legs = built(bp).legs.expect("paladin walks");
    assert!(
        legs.stride >= 16.0,
        "paladin shuffles: stride {}",
        legs.stride
    );
    assert!(legs.lift > 2.0, "feet come up: lift {}", legs.lift);
    assert!(
        legs.foot[2] > 2.0,
        "sole wide enough to stamp: {:?}",
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
    for key in ["extractor", "radar", "shield"] {
        assert!(has(key, part::SPINNER), "{key} spins");
        assert!(
            build_model(key).unwrap().spinner_pivot[2] > 1.0,
            "{key} spinner pivot"
        );
    }
    for bp in BLUEPRINTS
        .iter()
        .filter(|bp| {
            bp.footprint.is_none()
                && !["interceptor", "bomber"].contains(&bp.mesh)
                && !NAVAL_HULLS.contains(&bp.mesh)
        })
    {
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
fn hover_tank_rides_a_cushion() {
    let model = build_model("hover_tank").unwrap();
    assert!(model.hover, "skimmer is a hovercraft");
    let skirt_low = model.lods[0]
        .vertices
        .iter()
        .filter(|v| v.part == part::LOCOMOTION)
        .map(|v| v.pos[2])
        .fold(f32::MAX, f32::min);
    assert!(skirt_low > 0.08, "skirt sits off the ground: {skirt_low}");
}

#[test]
fn orange_weapons_glow_orange() {
    let orange = [
        "commander",
        "bot_light",
        "artillery_light",
        "missile_launcher",
        "reclaimer",
        "hover_tank",
        // Missile cells carry orange seams; the carrier's flak and rotary gun are orange too.
        "missile_ship",
        "carrier",
        "battleship",
    ];
    // Conventional guns with a dark bore: no emitters on the mesh.
    let unlit = [
        "attack_boat",
        "frigate",
        "submarine",
        "salvage_boat",
        "tank_light",
        "turret",
        "artillery_static",
        "interceptor",
        "bomber",
    ];
    for bp in BLUEPRINTS.iter().filter(|bp| !bp.muzzles.is_empty()) {
        let mesh = &built(bp).lods[0];
        let count = |m: u32| mesh.vertices.iter().filter(|v| v.material == m).count();
        if unlit.contains(&bp.mesh) {
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
fn shield_next_tier_is_upgrade_pieces() {
    let has = |tech: u8| {
        build_model_scaled("shield", 16.5, 40.0, tech).unwrap().lods[0]
            .vertices
            .iter()
            .any(|v| v.rig & rig::UPGRADE != 0)
    };
    assert!(!has(1), "T1 is not a shield");
    assert!(has(2), "T2 carries the T3 wreath as upgrade pieces");
    assert!(!has(3), "T3 is finished");
}

#[test]
fn shield_t3_bolts_on_without_stretching_the_hull() {
    let hull_z = |tech: u8, height: f32, lo: f32, hi: f32| {
        build_model_scaled("shield", 16.5, height, tech)
            .unwrap()
            .lods[0]
            .vertices
            .iter()
            .filter(|v| v.rig & rig::UPGRADE == 0)
            .map(|v| v.pos[2])
            .filter(|&z| z >= lo && z < hi)
            .fold(0.0f32, f32::max)
    };
    let t2_collar = hull_z(2, 40.0, 4.5, 8.0);
    let t3_collar = hull_z(3, 52.0, 4.5, 8.0);
    assert!(
        (t2_collar - t3_collar).abs() < 0.2,
        "T3 stretched the pad: T2 {t2_collar} T3 {t3_collar}"
    );
    let t2_top = hull_z(2, 40.0, 0.0, 100.0);
    let t3_top = hull_z(3, 52.0, 0.0, 100.0);
    assert!(
        t3_top > t2_top + 5.0,
        "T3 should add a needle, not scale the T2 tip: T2 {t2_top} T3 {t3_top}"
    );
}

#[test]
fn shield_shaft_stays_a_column() {
    let model = build_model_scaled("shield", 16.5, 40.0, 2).unwrap();
    let plating_r = |lo: f32, hi: f32| {
        model.lods[0]
            .vertices
            .iter()
            .filter(|v| v.part != part::SPINNER && v.material == material::PLATING)
            .filter(|v| v.pos[2] >= lo && v.pos[2] < hi)
            .map(|v| v.pos[0].hypot(v.pos[1]))
            .fold(0.0f32, f32::max)
    };
    let mid = plating_r(18.0, 22.0);
    let high = plating_r(28.0, 32.0);
    assert!(mid > 4.4, "shaft pinched to a needle at mid-height: {mid}");
    assert!(
        high > 4.0,
        "shaft pinched to a needle under the crown: {high}"
    );
    assert!(
        high / mid > 0.72,
        "shaft tapers like a cone: mid {mid} high {high}"
    );
}

#[test]
fn radar_next_tier_is_upgrade_pieces() {
    let has = |tech: u8| {
        build_model_scaled("radar", 6.0, 20.0, tech).unwrap().lods[0]
            .vertices
            .iter()
            .any(|v| v.rig & rig::UPGRADE != 0)
    };
    assert!(has(1), "T1 carries the T2 wreath as upgrade pieces");
    assert!(has(2), "T2 carries the T3 wreath as upgrade pieces");
    assert!(!has(3), "T3 is finished");
}

#[test]
fn factory_next_tier_is_upgrade_pieces() {
    let has = |key: &str, tech: u8, h: f32| {
        build_model_scaled(key, 46.0, h, tech).unwrap().lods[0]
            .vertices
            .iter()
            .any(|v| v.rig & rig::UPGRADE != 0)
    };
    for (key, t1) in [("factory_land", 28.0), ("factory_air", 26.0), ("factory_naval", 26.0)] {
        assert!(
            has(key, 1, t1),
            "{key} T1 carries the T2 suite as upgrade pieces"
        );
        assert!(
            has(key, 2, 34.0),
            "{key} T2 carries the T3 suite as upgrade pieces"
        );
        assert!(!has(key, 3, 42.0), "{key} T3 is finished");
    }
}

#[test]
fn factories_print_from_the_sides() {
    for key in ["factory_land", "factory_air"] {
        let model = build_model(key).unwrap();
        assert!(
            model
                .lods
                .iter()
                .all(|m| m.vertices.iter().all(|v| v.part != part::SPINNER)),
            "{key} has no spinning print head"
        );
        assert!(
            model.lods[0]
                .vertices
                .iter()
                .any(|v| v.rig & rig::LIFT != 0),
            "{key} has a lift deck"
        );
        let pad_amber = model.lods[0]
            .vertices
            .iter()
            .filter(|v| {
                v.material == material::GLOW_AMBER
                    && v.pos[2] < 1.6
                    && v.pos[0].hypot(v.pos[1]) < 16.0
            })
            .count();
        assert_eq!(pad_amber, 0, "{key}: amber disc on the pad");
        let gun_amber = model.lods[0]
            .vertices
            .iter()
            .filter(|v| v.material == material::GLOW_AMBER && v.pos[2] > 3.0)
            .count();
        assert!(gun_amber > 0, "{key}: no amber on the print guns");
    }
}

#[test]
fn naval_yard_is_one_sided_on_piles() {
    for (tech, h) in [(1, 26.0), (2, 34.0), (3, 42.0)] {
        let model = build_model_scaled("factory_naval", 46.0, h, tech).unwrap();
        for (lod, mesh) in model.lods.iter().enumerate() {
            // The berth is open water: a hull wider than the yard floats clear of it.
            let reach = mesh.vertices.iter().map(|v| v.pos[1]).fold(f32::MIN, f32::max);
            assert!(reach < -4.0, "T{tech} lod{lod}: yard reaches y {reach} into the berth");
            // It stands on piles down past any seabed.
            let foot = mesh.vertices.iter().map(|v| v.pos[2]).fold(f32::MAX, f32::min);
            assert!(foot < -60.0, "T{tech} lod{lod}: piles stop at {foot}");
        }
        let amber = model.lods[0]
            .vertices
            .iter()
            .filter(|v| v.material == material::GLOW_AMBER && v.pos[2] > 3.0)
            .count();
        assert!(amber > 0, "T{tech}: no fabricator tips");
    }
}

/// Every fabricator's amber tip is where the sim draws the build beam from.
#[test]
fn fabricator_tips_are_the_print_heads() {
    for key in ["factory_land", "factory_air", "factory_naval"] {
        let factory = mc_sim::print_heads::factory_heads(key).unwrap();
        let model = build_model_scaled(key, 46.0, 42.0, 3).unwrap();
        for head in factory.heads {
            let tip = Vec3::from(mc_sim::print_heads::nozzle(head, factory.aim));
            let near = model.lods[0]
                .vertices
                .iter()
                .filter(|v| v.material == material::GLOW_AMBER)
                .map(|v| Vec3::from(v.pos).distance(tip))
                .fold(f32::MAX, f32::min);
            assert!(near < 0.2, "{key}: no amber within {near} m of the head at {:?}", head.mount);
        }
    }
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
    for key in ["factory_land", "factory_air", "factory_naval", "extractor", "power", "radar"] {
        let [t1, t2, t3] = [1, 2, 3].map(|tech| glow(key, tech));
        assert!(
            t1 > 0 && t1 < t2 && t2 < t3,
            "{key}: glow triangles {t1}, {t2}, {t3}"
        );
    }
    let amber = |tech: u8| {
        let model = build_model_scaled("engineer", 10.0, 10.0, tech).unwrap();
        model.lods[0]
            .indices
            .chunks(3)
            .filter(|t| model.lods[0].vertices[t[0] as usize].material == material::GLOW_AMBER)
            .count()
    };
    let [t1, t2, t3] = [1, 2, 3].map(amber);
    assert!(
        t1 > 0 && t1 < t2 && t2 < t3,
        "engineer: amber triangles {t1}, {t2}, {t3}"
    );
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
    for tech in [2, 3] {
        let (r, h) = if tech == 2 { (5.5, 5.5) } else { (7.0, 7.0) };
        dump(
            &format!("mobile_aa_t{tech}"),
            &build_model_scaled("mobile_aa", r, h, tech).unwrap(),
        );
    }
    std::fs::write(dir.join("triangles.txt"), &table).unwrap();
    println!("{table}\nwrote {}", dir.display());
}

#[test]
fn aircraft_have_swept_wings_nozzle_origins_and_bounded_lods() {
    for key in ["interceptor", "bomber"] {
        let model = build_model(key).unwrap();
        let counts = [0, 1, 2].map(|i| triangles(&model.lods[i]));
        assert!(
            counts[2] < 60 && counts[1] as f32 <= counts[0] as f32 * 0.45 + 20.0,
            "{key}: {counts:?}"
        );
        let ports = super::aircraft_exhausts(key);
        assert_eq!(ports.len(), 2);
        for p in ports {
            let point = Vec3::from(*p);
            let mesh = &model.lods[0];
            let distance = mesh
                .indices
                .chunks(3)
                .map(|t| {
                    closest_point_on_triangle(
                        point,
                        position(mesh, t[0]),
                        position(mesh, t[1]),
                        position(mesh, t[2]),
                    )
                    .distance(point)
                })
                .fold(f32::MAX, f32::min);
            assert!(
                distance < 0.12,
                "{key}: nozzle detached from hull by {distance}"
            );
        }
        assert!(ports[0][0] < 0.0);
        assert_eq!(ports[0][1], -ports[1][1]);
    }
}

#[test]
fn vtol_pods_carry_their_nozzles_and_the_hold_fits_the_flock() {
    for key in ["gunship", "reclaim_carrier"] {
        let model = build_model(key).unwrap();
        let pivots = super::vtol_nacelles(key).unwrap();
        let ports = super::aircraft_exhausts(key);
        assert_eq!(ports.len(), 4, "{key}: four engines");
        for (level, lod) in model.lods.iter().enumerate() {
            for (i, part) in [part::VTOL_FRONT, part::VTOL_REAR].into_iter().enumerate() {
                let pod: Vec<Vec3> = lod
                    .vertices
                    .iter()
                    .filter(|v| v.part == part)
                    .map(|v| Vec3::from(v.pos))
                    .collect();
                assert!(!pod.is_empty(), "{key} LOD{level}: pod {i} is missing");
                // Everything on a pod stays near its pivot, so the tilt reads as a tilt.
                for p in pod {
                    let pivot = Vec3::new(pivots[i][0], pivots[i][1] * p.y.signum(), pivots[i][2]);
                    assert!(p.distance(pivot) < 3.4, "{key} LOD{level}: pod {i} vertex {p} is far from its pivot");
                }
            }
        }
        let mesh = &model.lods[0];
        for port in ports {
            let p = Vec3::from(*port);
            let near = mesh
                .indices
                .chunks(3)
                .filter(|t| mesh.vertices[t[0] as usize].part == part::VTOL_FRONT
                    || mesh.vertices[t[0] as usize].part == part::VTOL_REAR)
                .map(|t| {
                    closest_point_on_triangle(p, position(mesh, t[0]), position(mesh, t[1]), position(mesh, t[2]))
                        .distance(p)
                })
                .fold(f32::MAX, f32::min);
            assert!(near < 0.15, "{key}: nozzle {p} is {near} m from its pod");
        }
    }
    // The hold: doors and cradles at the levels that draw them, and a drone in a
    // cradle clear of the hold's sides.
    let model = build_model("reclaim_carrier").unwrap();
    for (level, lod) in model.lods.iter().take(2).enumerate() {
        assert!(lod.vertices.iter().any(|v| v.part == part::HOLD_DOOR), "LOD{level}: no hold doors");
    }
    assert!(model.lods[0].vertices.iter().any(|v| v.part == part::CRADLE), "no cradles");
    let (cradles, ceiling) = super::carrier_cradles();
    let drone = build_model("reclaim_drone").unwrap();
    let half_width = drone.lods[0].vertices.iter().map(|v| v.pos[1].abs()).fold(0.0, f32::max);
    let top = drone.lods[0].vertices.iter().map(|v| v.pos[2]).fold(0.0, f32::max);
    for c in cradles {
        assert!(c[1].abs() + half_width < super::aster::air::osprey::HOLD_HALF_WIDTH, "a docked drone touches the hold's side");
    }
    // Stowed 0.7 m up (`drone_socket`), a drone's top is under the ceiling.
    assert!(0.7 + top <= ceiling + 0.15, "a docked drone {top} tall sticks through the ceiling at {ceiling}");
}

#[test]
fn hellkite_barrels_are_seated_in_their_guns() {
    let model = build_model("fire_bomber").unwrap();
    let mesh = &model.lods[0];
    let guns = [
        (Vec3::new(2.0, 0.0, 4.8), Vec3::X),
        (Vec3::new(1.4, 3.0, 2.8), Vec3::X),
        (Vec3::new(1.4, -3.0, 2.8), Vec3::X),
        (Vec3::new(-10.0, 0.0, 2.8), -Vec3::X),
    ];
    for (muzzle, bore) in guns {
        let nearest = mesh
            .indices
            .chunks(3)
            .map(|t| {
                closest_point_on_triangle(
                    muzzle,
                    position(mesh, t[0]),
                    position(mesh, t[1]),
                    position(mesh, t[2]),
                )
                .distance(muzzle)
            })
            .fold(f32::MAX, f32::min);
        assert!(
            nearest < 0.2,
            "muzzle {muzzle} is {nearest} m from its barrel"
        );
        let socket = muzzle - bore * 0.7;
        let seated = mesh.vertices.iter().any(|v| {
            let rel = Vec3::from(v.pos) - socket;
            let along = rel.dot(bore);
            let radial = (rel - bore * along).length();
            along.abs() < 0.45 && (0.3..0.7).contains(&radial)
        });
        assert!(seated, "barrel at {muzzle} has no gun around it");
    }
}

#[test]
fn complete_air_roster_models_meet_lod_budgets() {
    let keys = ["air_scout","rotor_gunship","support_air","reclaim_carrier","reclaim_drone","gunship",
        "fire_bomber","torpedo_bomber","interceptor_t2","superiority","strategic_bomber","assault_air","aa_gun","aa_array","aa_sam","aa_shatter","mobile_aa","factory_air"];
    for key in keys {
        for tech in 1..=3 {
            let (r,h)=if key=="factory_air" {(46.0, match tech {1=>26.0,2=>34.0,_=>42.0})}
                else if key=="mobile_aa" {match tech {1=>(4.0,4.5),2=>(5.5,5.5),_=>(7.0,7.0)}}
                else {(10.0,10.0)};
            let model=build_model_scaled(key,r,h,tech).unwrap();
            let [full,mid,coarse]=[0,1,2].map(|i|triangles(&model.lods[i]));
            let budget = if key == "factory_air" { FACTORY_TRIANGLES } else { 2600 };
            assert!(coarse<60 && full<=budget && mid as f32 <= full as f32 * 0.45+20.0,"{key} T{tech}: {full}/{mid}/{coarse}");
        }
    }
}

#[test]
fn shatter_has_connected_bearing_black_breech_and_recoil_at_every_lod() {
    let model = build_model("aa_shatter").unwrap();
    assert!(model.recoil.unwrap()[3] >= 1.0);
    for mesh in &model.lods {
        let hull_top = mesh.vertices.iter()
            .filter(|v| v.part == part::HULL && v.material != material::TEAM && v.pos[0].abs() < 3.1 && v.pos[1].abs() < 3.1)
            .map(|v| v.pos[2]).fold(f32::NEG_INFINITY, f32::max);
        let mount_bottom = mesh.vertices.iter()
            .filter(|v| v.part == part::TURRET && v.rig & rig::LIMB_MASK == 0)
            .map(|v| v.pos[2]).fold(f32::INFINITY, f32::min);
        assert!(mount_bottom <= hull_top, "turret floats above the bearing");
        assert!(mesh.vertices.iter().any(|v| v.material == material::ACCENT && v.rig & rig::RECOIL != 0));
        assert!(mesh.vertices.iter().any(|v| v.part == part::TURRET && v.rig & rig::RECOIL == 0));
    }
}

#[test]
fn skyguard_lods_keep_the_fixed_silo() {
    let model = build_model("aa_sam").unwrap();
    for (level, lod) in model.lods.iter().enumerate() {
        assert!(
            lod.vertices.iter().all(|v| v.part == part::HULL),
            "LOD{level} launch mast must not yaw like a turret"
        );
        let high = lod.vertices.iter().filter(|v| v.pos[2] >= 11.0);
        assert!(
            high.clone().any(|v| v.material == material::ACCENT),
            "LOD{level} needs the launch mouth"
        );
        assert!(
            high.clone().all(|v| v.pos[0].hypot(v.pos[1]) <= 2.5),
            "LOD{level} grows a turret above the mast"
        );
        assert!(
            lod.vertices
                .iter()
                .any(|v| v.pos[0] < -6.5 && (2.0..6.5).contains(&v.pos[2])),
            "LOD{level} is missing the side cabinet"
        );
        assert!(
            lod.vertices.iter().any(|v| v.pos[0].hypot(v.pos[1]) > 9.0),
            "LOD{level} pad is too small"
        );
    }
    let [full, mid, coarse] = [0, 1, 2].map(|lod| triangles(&model.lods[lod]));
    assert!(
        full >= mid && mid >= coarse && coarse < 60 && mid as f32 <= full as f32 * 0.45 + 20.0,
        "aa_sam lod triangles {full}/{mid}/{coarse}"
    );
}

#[test]
fn tempest_lods_keep_sixteen_fixed_cell_mouths() {
    let model = build_model("aa_array").unwrap();
    for (level, lod) in model.lods.iter().enumerate() {
        let mouths: Vec<_> = lod.indices.chunks_exact(3).filter(|t| {
            t.iter().all(|&i| {
                let v = &lod.vertices[i as usize];
                v.material == material::ACCENT && (v.pos[2] - 6.75).abs() < 0.01
            })
        }).collect();
        assert_eq!(mouths.len(), 32, "LOD{level} must show all 16 cell mouths");
        assert!(lod.vertices.iter().all(|v| v.part == part::HULL), "fixed launch cells must not track turret yaw");
    }
}

#[test]
fn sunder_has_supported_recoil_turret_and_tracks_at_every_lod() {
    let model = build_model_scaled("mobile_aa", 7.0, 7.0, 3).unwrap();
    assert_eq!(model.turret_pivot, [0.0, 0.0, 5.5]);
    assert_eq!(model.arm_pivot.unwrap(), [0.0, 0.0, 5.5]);
    assert!((model.recoil.unwrap()[3] - 0.85).abs() < 0.001);
    for mesh in &model.lods {
        assert!(mesh.vertices.iter().all(|v| v.pos[2] <= 7.0));
        assert!(mesh.vertices.iter().any(|v| v.part == part::LOCOMOTION && v.material == material::TREAD));
        assert!(mesh.vertices.iter().any(|v| v.material == material::ACCENT && v.rig & rig::RECOIL != 0));
        let hull_top = mesh.indices.chunks_exact(3)
            .filter(|t| mesh.vertices[t[0] as usize].part == part::HULL)
            .filter_map(|t| {
                let a = position(mesh, t[0]);
                let ab = position(mesh, t[1]) - a;
                let ac = position(mesh, t[2]) - a;
                let area = ab.truncate().perp_dot(ac.truncate());
                if area.abs() < 0.0001 { return None; }
                let u = (-a.truncate()).perp_dot(ac.truncate()) / area;
                let v = ab.truncate().perp_dot(-a.truncate()) / area;
                (u >= -0.001 && v >= -0.001 && u + v <= 1.001)
                    .then_some(a.z + ab.z * u + ac.z * v)
            }).fold(f32::NEG_INFINITY, f32::max);
        let mount_bottom = mesh.vertices.iter()
            .filter(|v| v.part == part::TURRET && v.rig & rig::LIMB_MASK == 0)
            .map(|v| v.pos[2]).fold(f32::INFINITY, f32::min);
        assert!(mount_bottom <= hull_top, "mobile turret floats over its deck");
        let muzzle = Vec3::new(8.0, 0.0, 5.5);
        let distance = mesh.indices.chunks_exact(3)
            .filter(|t| mesh.vertices[t[0] as usize].rig & rig::RECOIL != 0)
            .map(|t| {
                closest_point_on_triangle(
                    muzzle,
                    position(mesh, t[0]),
                    position(mesh, t[1]),
                    position(mesh, t[2]),
                ).distance(muzzle)
            }).fold(f32::MAX, f32::min);
        assert!(distance < 0.08, "muzzle is detached from the recoil barrel: {distance}");
    }
}

#[test]
fn tree_canopies_are_cutout_sprays_with_bounded_lods() {
    for key in ["tree_broadleaf", "tree_conifer", "tree_pine"] {
        let model = build_model(key).unwrap();
        let counts = model.lods.each_ref().map(|m| m.indices.len() / 3);
        // Forests carry hundreds of thousands of trees: the reduced level is a
        // hundred-odd triangles and the coarse one a handful of cards.
        assert!(counts[0] <= 1000 && counts[1] <= 200 && counts[2] <= 32, "{key}: {counts:?}");
        assert!(counts[1] as f32 <= counts[0] as f32 * 0.45 + 20.0, "{key}: {counts:?}");
        for mesh in &model.lods {
            let leaves: Vec<_> = mesh.vertices.iter().filter(|v| v.material == material::FOLIAGE).collect();
            assert!(!leaves.is_empty(), "{key}: missing foliage");
            // Cards show a region of a cutout atlas, never a solid canopy surface.
            assert!(leaves.iter().all(|v| v.uv.iter().all(|u| (0.0..=1.0).contains(u))), "{key}: card outside its atlas");
            assert!(leaves.chunks_exact(4).all(|card| card[0].uv != card[2].uv), "{key}: card without an atlas region");
            // Each leaf knows its crown: an outward normal and how deep in the crown it sits.
            for v in &leaves {
                let crown = Vec3::new(v.face[0], v.face[1], v.face[2]);
                assert!((crown.length() - 1.0).abs() < 1e-3 && (0.0..=1.0).contains(&v.face[3]), "{key}: {:?}", v.face);
            }
            assert!(mesh.vertices.iter().any(|v| v.material == material::BARK), "{key}: missing branches");
            for face in mesh.indices.chunks_exact(3) {
                let [a, b, c] = [face[0], face[1], face[2]].map(|i| &mesh.vertices[i as usize]);
                let n = (Vec3::from(b.pos) - Vec3::from(a.pos)).cross(Vec3::from(c.pos) - Vec3::from(a.pos));
                assert!(n.length() > 0.00001);
                assert!(n.normalize().dot(Vec3::from(a.normal)) > 0.999);
            }
        }
    }
}

/// Every refit module a unit file lists has pieces on its model: the shader shows
/// them when the module is fitted, so a module without any would change nothing.
#[test]
fn every_refit_module_has_pieces_on_the_model() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let bps = mc_data::Blueprints::load(&data).unwrap();
    for set in &bps.refits {
        let bp = bps.unit(set.base);
        let keys: Vec<&str> = set.slots.iter().flat_map(|s| &s.modules).map(|m| m.key.as_str()).collect();
        let model = super::build_model_fitted(&bp.visual.mesh, bp.radius.to_f32(), bp.height.to_f32(), bp.tech, &keys)
            .expect("the unit has a model");
        let bare = build_model(&bp.visual.mesh).unwrap();
        for (lod, mesh) in model.lods.iter().enumerate() {
            for (i, key) in keys.iter().enumerate() {
                let tag = (i as u32 + 1) << rig::MODULE_SHIFT;
                assert!(
                    mesh.vertices.iter().any(|v| v.rig & rig::MODULE_MASK == tag),
                    "{} lod{lod}: module {key} has no pieces",
                    bp.key
                );
            }
            // Untagged, the model is the bare unit.
            let always = mesh.vertices.iter().filter(|v| v.rig & rig::MODULE_MASK == 0).count();
            assert_eq!(always, bare.lods[lod].vertices.len(), "{} lod{lod}", bp.key);
        }
    }
}

#[test]
fn warden_cannon_recoils_at_every_lod() {
    let model = build_model("tank_light").unwrap();
    let travel = model.recoil.expect("warden gun has no recoil travel")[3];
    assert!(travel > 0.3 && travel < 1.5, "warden recoil travel {travel}");
    for mesh in &model.lods {
        assert!(
            mesh.vertices
                .iter()
                .any(|v| v.part == part::TURRET && v.rig & rig::RECOIL != 0),
            "warden barrel is not tagged to recoil"
        );
        // Only the tube slides: the hull and tracks stay put.
        assert!(mesh
            .vertices
            .iter()
            .filter(|v| v.rig & rig::RECOIL != 0)
            .all(|v| v.part == part::TURRET));
    }
}


#[test]
fn frigate_aa_mount_turns_on_its_own() {
    let model = build_model("frigate").unwrap();
    let mount = model.mount.expect("frigate: the AA gun has a trunnion");
    assert!(
        (Vec3::new(mount[0], mount[1], mount[2]) - Vec3::new(-6.4, 0.0, 8.1)).length() < 1e-3,
        "frigate: AA trunnion at {mount:?}"
    );
    for (lod, mesh) in model.lods.iter().enumerate().take(2) {
        let mounted: Vec<_> = mesh
            .vertices
            .iter()
            .filter(|v| (v.rig & rig::LIMB_MASK) == rig::MOUNT)
            .collect();
        // On the hull, not the deck gun's turret: it turns by itself.
        assert!(
            !mounted.is_empty() && mounted.iter().all(|v| v.part == part::HULL),
            "frigate lod{lod}: the AA mount rides the hull"
        );
        for muzzle in [[-4.2, -0.35, 8.1], [-4.2, 0.35, 8.1]] {
            let m = Vec3::from(muzzle);
            let nearest = mesh
                .indices
                .chunks(3)
                .filter(|t| (mesh.vertices[t[0] as usize].rig & rig::LIMB_MASK) == rig::MOUNT)
                .map(|t| {
                    closest_point_on_triangle(m, position(mesh, t[0]), position(mesh, t[1]), position(mesh, t[2]))
                        .distance(m)
                })
                .fold(f32::MAX, f32::min);
            assert!(nearest < 0.3, "frigate lod{lod}: AA barrel ends {nearest} m from {muzzle:?}");
        }
    }
}

#[test]
fn ships_float_and_radars_turn() {
    for key in ["attack_boat", "frigate", "submarine"] {
        let model = build_model(key).unwrap();
        for mesh in &model.lods {
            // Nothing the water shader would lift as running gear.
            assert!(mesh.vertices.iter().all(|v| v.part != part::LOCOMOTION), "{key}: running gear");
            // The hull goes down into the water.
            let keel = mesh.vertices.iter().map(|v| v.pos[2]).fold(f32::MAX, f32::min);
            assert!(keel < -0.4, "{key}: keel at {keel}");
        }
    }
    for key in ["attack_boat", "frigate"] {
        let model = build_model(key).unwrap();
        assert!(
            model.lods.iter().take(2).all(|m| m.vertices.iter().any(|v| v.part == part::SPINNER)),
            "{key}: radar spins"
        );
        assert!(model.spinner_pivot[2] > 3.0, "{key}: radar on the mast");
    }
    // A tech 1 warship is unlit; the sonar buoy has only its red lamp.
    for key in ["attack_boat", "frigate", "submarine", "sonar"] {
        let lit = build_model(key).unwrap().lods[0]
            .vertices
            .iter()
            // The next tier's refit pieces (the buoy's blue deck strips) are not fitted yet.
            .filter(|v| v.rig & rig::UPGRADE == 0)
            .filter(|v| matches!(v.material, material::GLOW | material::GLOW_ORANGE | material::GLOW_AMBER))
            .count();
        assert_eq!(lit, 0, "{key}: lit at tech 1");
    }
}

#[test]
fn sonar_next_tier_is_upgrade_pieces() {
    let has = |tech: u8, h: f32| {
        build_model_scaled("sonar", 6.0, h, tech).unwrap().lods[0]
            .vertices
            .iter()
            .any(|v| v.rig & rig::UPGRADE != 0)
    };
    assert!(has(1, 12.0), "T1 carries the T2 float ring and array as upgrade pieces");
    assert!(has(2, 15.0), "T2 carries the T3 transducer and mast as upgrade pieces");
    assert!(!has(3, 18.0), "T3 is finished");
}

/// The Leviathan: four houses bound to its four weapons at the unit file's pivots, every
/// weapon's muzzles reached by a barrel at every level, and the capital-ship budgets.
#[test]
fn battleship_houses_and_muzzles() {
    let bp = BLUEPRINTS.iter().find(|bp| bp.mesh == "battleship").unwrap();
    let model = built(bp);
    let want: [(u8, [f32; 3]); 4] = [
        (0, [30.0, 0.0, 12.0]),
        (1, [14.0, 0.0, 15.5]),
        (2, [-30.0, 0.0, 12.0]),
        (3, [-4.0, 0.0, 22.0]),
    ];
    assert_eq!(model.houses.len(), 4, "battleship: four gun houses");
    for (weapon, pivot) in want {
        let house = model.houses.iter().find(|h| h.weapon == weapon).expect("house per weapon");
        assert!(Vec3::from(house.pivot).distance(Vec3::from(pivot)) < 1e-3, "battleship: house {weapon} pivot {:?}", house.pivot);
    }
    // Muzzles of every weapon (the aft battery's as authored, facing forward).
    let muzzles: [(&str, &[[f32; 3]]); 4] = [
        ("fore", &[[48.0, -2.2, 12.5], [48.0, 0.0, 12.5], [48.0, 2.2, 12.5]]),
        ("second", &[[32.0, -2.2, 16.0], [32.0, 0.0, 16.0], [32.0, 2.2, 16.0]]),
        ("aft", &[[-12.0, -2.2, 12.5], [-12.0, 0.0, 12.5], [-12.0, 2.2, 12.5]]),
        ("aa", &[[-2.2, -0.35, 22.2], [-2.2, 0.35, 22.2]]),
    ];
    for (lod, mesh) in model.lods.iter().enumerate() {
        for (name, list) in muzzles {
            // The coarse level keeps only the forward batteries' barrels.
            if lod == 2 && (name == "aft" || name == "aa") {
                continue;
            }
            for muzzle in list {
                let p = Vec3::from(*muzzle);
                let nearest = mesh
                    .indices
                    .chunks(3)
                    .map(|t| closest_point_on_triangle(p, position(mesh, t[0]), position(mesh, t[1]), position(mesh, t[2])).distance(p))
                    .fold(f32::MAX, f32::min);
                assert!(nearest < 0.4, "battleship lod{lod}: {name} barrel ends {nearest} m from {muzzle:?}");
            }
        }
        let team_up = mesh.vertices.iter().any(|v| v.material == material::TEAM && v.normal[2] > 0.5);
        assert!(team_up, "battleship lod{lod}: no team colour seen from above");
        let floor = mesh.vertices.iter().map(|v| v.pos[2]).fold(f32::MAX, f32::min);
        assert!(floor >= -12.0, "battleship lod{lod}: keel at {floor}");
    }
    let [full, mid, coarse] = [0, 1, 2].map(|lod| triangles(&model.lods[lod]));
    assert!(full >= 250 && full <= 6000, "battleship: {full} triangles");
    assert!(mid as f32 <= full as f32 * 0.45 + 20.0, "battleship: mid {mid} of {full}");
    assert!(coarse < 60, "battleship: coarse {coarse}");
    let top = model.lods[0].vertices.iter().map(|v| v.pos[2]).fold(f32::MIN, f32::max);
    assert!((0.8 * 26.0..=1.25 * 26.0).contains(&top), "battleship: top {top}");
    let reach = model.lods[0].vertices.iter().map(|v| (v.pos[0].powi(2) + v.pos[1].powi(2)).sqrt()).fold(f32::MIN, f32::max);
    assert!((0.75 * 58.0..=1.3 * 58.0).contains(&reach), "battleship: reach {reach}");
    println!("battleship triangles {full}/{mid}/{coarse}, top {top:.1}, reach {reach:.1}");
}

/// The Moray and the Kraken: chined, angular submarines whose bow tube doors carry
/// weapon 0's muzzles at every LOD, the Moray's deck gun on a house of its own, and
/// both lit blue (tech 2 and 3) with nothing orange on them.
#[test]
fn moray_and_kraken_hulls() {
    for bp in BLUEPRINTS.iter().filter(|bp| bp.mesh == "submarine_hunter" || bp.mesh == "submarine_strategic") {
        let key = bp.mesh;
        let model = built(bp);
        let floor = if key == "submarine_strategic" { -12.0 } else { -4.5 };
        for (lod, mesh) in model.lods.iter().enumerate() {
            let name = format!("{key} lod{lod}");
            assert!(mesh.vertices.iter().all(|v| v.part != part::TURRET && v.part != part::LOCOMOTION), "{name}: turret or running gear");
            for muzzle in bp.muzzles {
                let p = Vec3::from(*muzzle);
                let nearest = mesh
                    .indices
                    .chunks(3)
                    .filter(|t| mesh.vertices[t[0] as usize].part == part::HULL)
                    .map(|t| closest_point_on_triangle(p, position(mesh, t[0]), position(mesh, t[1]), position(mesh, t[2])).distance(p))
                    .fold(f32::MAX, f32::min);
                assert!(nearest < 0.4, "{name}: tube door {nearest} m from muzzle {muzzle:?}");
            }
            assert!(mesh.vertices.iter().any(|v| v.material == material::TEAM && v.normal[2] > 0.5), "{name}: no team colour from above");
            assert!(mesh.vertices.iter().any(|v| v.material == material::PLATING), "{name}: no plating");
            let top = mesh.vertices.iter().map(|v| v.pos[2]).fold(f32::MIN, f32::max);
            assert!((0.8 * bp.height..=1.25 * bp.height).contains(&top), "{name}: top {top} for height {}", bp.height);
            let reach = mesh.vertices.iter().map(|v| v.pos[0].hypot(v.pos[1])).fold(f32::MIN, f32::max);
            assert!((0.75 * bp.radius..=1.3 * bp.radius).contains(&reach), "{name}: reach {reach} for radius {}", bp.radius);
            let keel = mesh.vertices.iter().map(|v| v.pos[2]).fold(f32::MAX, f32::min);
            assert!(keel >= floor && keel < -0.4, "{name}: keel at {keel}");
            for t in mesh.indices.chunks(3) {
                let [a, b, c] = [position(mesh, t[0]), position(mesh, t[1]), position(mesh, t[2])];
                let geometric = (b - a).cross(c - a);
                assert!(geometric.length() * 0.5 > 1e-7, "{name}: degenerate triangle at {a}");
                for &i in t {
                    let v = mesh.vertices[i as usize];
                    assert!((Vec3::from(v.normal).length() - 1.0).abs() < 1e-4, "{name}: unit normal");
                    assert!(geometric.normalize().dot(Vec3::from(v.normal)) > 0.999, "{name}: winding disagrees with normal at {a}");
                }
            }
        }
        let [full, mid, coarse] = [0, 1, 2].map(|lod| triangles(&model.lods[lod]));
        assert!((250..=WARSHIP_TRIANGLES).contains(&full), "{key}: {full} triangles");
        assert!(mid as f32 <= full as f32 * 0.45 + 20.0, "{key}: mid {mid} of {full}");
        assert!(coarse < 60, "{key}: coarse {coarse}");
        let count = |m: u32| model.lods[0].vertices.iter().filter(|v| v.material == m).count();
        assert!(count(material::GLOW) > 0, "{key}: no blue emitters");
        assert_eq!(count(material::GLOW_ORANGE), 0, "{key}: orange emitters");
        for lod in 0..LOD_COUNT {
            let builder = build_lod(key, lod, bp.tech);
            let mesh = builder.mesh();
            for range in builder.solids() {
                let volume: f32 = mesh.indices[range.clone()]
                    .chunks(3)
                    .map(|t| position(mesh, t[0]).dot(position(mesh, t[1]).cross(position(mesh, t[2]))))
                    .sum();
                assert!(volume > 0.0, "{key} lod{lod}: inside-out solid at {}", position(mesh, mesh.indices[range.start]));
            }
        }
        let houses = model.houses.clone();
        if key == "submarine_hunter" {
            assert_eq!(houses.len(), 1, "moray: one gun house");
            assert_eq!(houses[0].weapon, 1, "moray: the deck gun is weapon 1");
            assert!(Vec3::from(houses[0].pivot).distance(Vec3::new(5.5, 0.0, 2.2)) < 1e-3, "moray: house pivot {:?}", houses[0].pivot);
        } else {
            assert!(houses.is_empty(), "kraken: fixed launchers only");
        }
        println!("{key} triangles {full}/{mid}/{coarse}");
    }
}

/// The Atoll: houses bound to weapons 1 and 2 at the unit file's pivots and a spinning
/// close-in gun, the SAM cells' muzzles on the deck at every level, plasma glow, and the
/// capital-ship budgets.
#[test]
fn carrier_houses_and_muzzles() {
    let bp = BLUEPRINTS.iter().find(|bp| bp.mesh == "carrier").unwrap();
    let model = built(bp);
    assert_eq!(model.houses.len(), 2, "carrier: two gun houses");
    for (weapon, pivot) in [(1u8, [20.0, 10.0, 8.6]), (2u8, [20.0, -10.0, 8.6])] {
        let house = model.houses.iter().find(|h| h.weapon == weapon).expect("house per weapon");
        assert!(Vec3::from(house.pivot).distance(Vec3::from(pivot)) < 1e-3, "carrier: house {weapon} pivot {:?}", house.pivot);
    }
    assert!(model.lods[0].vertices.iter().any(|v| v.rig & rig::SPIN != 0), "carrier: rotary gun spins");
    assert!(model.lods[0].vertices.iter().any(|v| v.part == part::SPINNER), "carrier: radar turns");
    assert!(model.spinner_pivot[2] > 3.0, "carrier: radar on the mast");
    let muzzles: [(&str, &[[f32; 3]]); 3] = [
        ("sam", &[[-12.0, 8.0, 8.5], [-14.0, 8.0, 8.5], [-12.0, -8.0, 8.5], [-14.0, -8.0, 8.5]]),
        ("flak", &[[22.4, 9.6, 8.8], [22.4, 10.4, 8.8]]),
        ("ciws", &[[22.0, -10.0, 8.8]]),
    ];
    for (lod, mesh) in model.lods.iter().enumerate() {
        for (name, list) in muzzles {
            // The coarse level keeps only the SAM cells.
            if lod == 2 && name != "sam" {
                continue;
            }
            for muzzle in list {
                let p = Vec3::from(*muzzle);
                let nearest = mesh
                    .indices
                    .chunks(3)
                    .map(|t| closest_point_on_triangle(p, position(mesh, t[0]), position(mesh, t[1]), position(mesh, t[2])).distance(p))
                    .fold(f32::MAX, f32::min);
                assert!(nearest < 0.4, "carrier lod{lod}: {name} barrel ends {nearest} m from {muzzle:?}");
            }
        }
        let team_up = mesh.vertices.iter().any(|v| v.material == material::TEAM && v.normal[2] > 0.5);
        assert!(team_up, "carrier lod{lod}: no team colour seen from above");
        assert!(mesh.vertices.iter().any(|v| v.material == material::PLATING), "carrier lod{lod}: no plating");
        assert!(mesh.vertices.iter().all(|v| v.part != part::LOCOMOTION), "carrier lod{lod}: running gear");
        let floor = mesh.vertices.iter().map(|v| v.pos[2]).fold(f32::MAX, f32::min);
        assert!((-12.0..-0.4).contains(&floor), "carrier lod{lod}: keel at {floor}");
    }
    assert!(model.lods[0].vertices.iter().any(|v| v.material == material::GLOW), "carrier: plasma glow");
    let [full, mid, coarse] = [0, 1, 2].map(|lod| triangles(&model.lods[lod]));
    assert!(full >= 250 && full <= 6000, "carrier: {full} triangles");
    assert!(mid as f32 <= full as f32 * 0.45 + 20.0, "carrier: mid {mid} of {full}");
    assert!(coarse < 60, "carrier: coarse {coarse}");
    let top = model.lods[0].vertices.iter().map(|v| v.pos[2]).fold(f32::MIN, f32::max);
    assert!((0.8 * 24.0..=1.25 * 24.0).contains(&top), "carrier: top {top}");
    let reach = model.lods[0].vertices.iter().map(|v| v.pos[0].hypot(v.pos[1])).fold(f32::MIN, f32::max);
    assert!((0.75 * 60.0..=1.3 * 60.0).contains(&reach), "carrier: reach {reach}");
    println!("carrier triangles {full}/{mid}/{coarse}, top {top:.1}, reach {reach:.1}");
}

/// The Marlin and the Manta: every gun house at its data pivot, every weapon's
/// muzzles reached at every LOD they are drawn at (fixed launchers and the light
/// mounts drop out of the coarse level), lit blue with nothing orange, team colour
/// seen from above at every level, and inside the budgets and the fit.
#[test]
fn marlin_and_manta_hulls() {
    type Weapons = &'static [(&'static str, &'static [[f32; 3]], bool)];
    let ships: [(&str, f32, f32, &[(u8, [f32; 3])], Weapons); 2] = [
        (
            "destroyer",
            22.0,
            12.0,
            &[(0, [12.0, 0.0, 5.2]), (2, [-9.0, 0.0, 9.0])],
            &[
                ("rail", &[[20.5, -0.5, 5.2], [20.5, 0.5, 5.2]], true),
                ("tubes", &[[16.0, -0.8, -1.2], [16.0, 0.8, -1.2], [16.0, -0.8, -2.0], [16.0, 0.8, -2.0]], false),
                ("pd", &[[-7.0, -0.3, 9.2], [-7.0, 0.3, 9.2]], false),
                ("interceptors", &[[-14.0, -1.4, -1.0], [-14.0, 1.4, -1.0]], false),
            ],
        ),
        (
            "aa_cruiser",
            22.0,
            14.0,
            &[(1, [-4.0, 0.0, 9.6]), (2, [14.5, 0.0, 5.4])],
            &[
                ("cells", &[[6.0, -2.0, 6.4], [6.0, 2.0, 6.4], [4.0, -2.0, 6.4], [4.0, 2.0, 6.4]], true),
                ("flak", &[[-1.6, -0.4, 9.8], [-1.6, 0.4, 9.8]], false),
                ("deck gun", &[[19.5, 0.0, 5.4]], false),
            ],
        ),
    ];
    for (key, radius, height, houses, weapons) in ships {
        let bp = BLUEPRINTS.iter().find(|bp| bp.mesh == key).unwrap();
        let model = built(bp);
        assert_eq!(model.houses.len(), houses.len(), "{key}: gun houses");
        for (weapon, pivot) in houses {
            let house = model.houses.iter().find(|h| h.weapon == *weapon).expect("house per weapon");
            assert!(Vec3::from(house.pivot).distance(Vec3::from(*pivot)) < 1e-3, "{key}: house {weapon} pivot {:?}", house.pivot);
        }
        for (lod, mesh) in model.lods.iter().enumerate() {
            for (name, list, at_coarse) in weapons {
                if lod == 2 && !at_coarse {
                    continue;
                }
                for muzzle in *list {
                    let p = Vec3::from(*muzzle);
                    let nearest = mesh
                        .indices
                        .chunks(3)
                        .filter(|t| mesh.vertices[t[0] as usize].part == part::HULL)
                        .map(|t| closest_point_on_triangle(p, position(mesh, t[0]), position(mesh, t[1]), position(mesh, t[2])).distance(p))
                        .fold(f32::MAX, f32::min);
                    assert!(nearest < 0.4, "{key} lod{lod}: {name} barrel ends {nearest} m from {muzzle:?}");
                }
            }
            let team_up = mesh.vertices.iter().any(|v| v.material == material::TEAM && v.normal[2] > 0.5);
            assert!(team_up, "{key} lod{lod}: no team colour seen from above");
            assert!(mesh.vertices.iter().any(|v| v.material == material::PLATING), "{key} lod{lod}: no plating");
            let floor = mesh.vertices.iter().map(|v| v.pos[2]).fold(f32::MAX, f32::min);
            assert!(floor >= -4.5, "{key} lod{lod}: keel at {floor}");
            assert!(!mesh.vertices.iter().any(|v| v.part == part::TURRET), "{key} lod{lod}: turret part");
        }
        let full_mesh = &model.lods[0];
        assert!(!full_mesh.vertices.iter().any(|v| v.material == material::GLOW_ORANGE), "{key}: orange");
        assert!(full_mesh.vertices.iter().any(|v| v.material == material::GLOW), "{key}: nothing lit blue");
        assert!(full_mesh.vertices.iter().any(|v| v.part == part::SPINNER), "{key}: no radar spinner");
        let [full, mid, coarse] = [0, 1, 2].map(|lod| triangles(&model.lods[lod]));
        assert!(full >= 250 && full <= WARSHIP_TRIANGLES, "{key}: {full} triangles");
        assert!(mid as f32 <= full as f32 * 0.45 + 20.0, "{key}: mid {mid} of {full}");
        assert!(coarse < 60, "{key}: coarse {coarse}");
        let top = full_mesh.vertices.iter().map(|v| v.pos[2]).fold(f32::MIN, f32::max);
        assert!((0.8 * height..=1.25 * height).contains(&top), "{key}: top {top}");
        let reach = full_mesh.vertices.iter().map(|v| (v.pos[0].powi(2) + v.pos[1].powi(2)).sqrt()).fold(f32::MIN, f32::max);
        assert!((0.75 * radius..=1.3 * radius).contains(&reach), "{key}: reach {reach}");
        println!("{key} triangles {full}/{mid}/{coarse}, top {top:.1}, reach {reach:.1}");
    }
}

/// Nearest distance from `p` to the triangles of `mesh` whose first vertex passes `keep`.
fn nearest_where(mesh: &MeshLod, p: Vec3, keep: impl Fn(&super::MeshVertex) -> bool) -> f32 {
    mesh.indices
        .chunks(3)
        .filter(|t| keep(&mesh.vertices[t[0] as usize]))
        .map(|t| {
            closest_point_on_triangle(p, position(mesh, t[0]), position(mesh, t[1]), position(mesh, t[2]))
                .distance(p)
        })
        .fold(f32::MAX, f32::min)
}

/// The Paladin's shin torpedo tubes: a mouth at each of the unit file's muzzles with the
/// legs at rest, on the shin bone so the pod strides with the leg, and unlit orange.
#[test]
fn paladin_shin_tubes_reach_their_muzzles() {
    let bp = BLUEPRINTS.iter().find(|bp| bp.mesh == "assault_bot").unwrap();
    let model = built(bp);
    // The coarse level's legs are one block each: no pods.
    for (lod, mesh) in model.lods.iter().enumerate().take(2) {
        for muzzle in [[2.2, -4.6, 3.0], [2.2, 4.6, 3.0]] {
            let nearest = nearest_where(mesh, Vec3::from(muzzle), |v| {
                v.part == part::LOCOMOTION && (v.rig & rig::LIMB_MASK) == rig::SHIN
            });
            assert!(nearest < 0.4, "paladin lod{lod}: shin tube mouth {nearest} m from {muzzle:?}");
        }
    }
}

/// The Fulgur: a main turret on the origin, three gun houses bound to weapons 1, 2 and 3
/// at the unit file's pivots with a barrel at each muzzle, the houses standing on the hull,
/// and the flak house low enough for the main barrel to pass over it.
#[test]
fn fulgur_houses_and_muzzles() {
    let bp = BLUEPRINTS.iter().find(|bp| bp.mesh == "assault_tank").unwrap();
    let model = built(bp);
    let want: [(u8, [f32; 3], [f32; 3]); 3] = [
        (1, [14.85, 17.325, 11.88], [25.575, 17.325, 12.87]),
        (2, [14.85, -17.325, 11.88], [25.575, -17.325, 12.87]),
        (3, [-18.15, 0.0, 15.51], [-9.075, 0.0, 16.83]),
    ];
    assert_eq!(model.houses.len(), 3, "fulgur: three gun houses");
    for (weapon, pivot, muzzle) in want {
        let (slot, house) = model
            .houses
            .iter()
            .enumerate()
            .find(|(_, h)| h.weapon == weapon)
            .expect("a house per weapon");
        assert!(
            Vec3::from(house.pivot).distance(Vec3::from(pivot)) < 1e-3,
            "fulgur: house {weapon} pivot {:?}",
            house.pivot
        );
        let limb = rig::HOUSE_FIRST + slot as u32;
        let of_house = |v: &super::MeshVertex| (v.rig & rig::LIMB_MASK) == limb;
        // The coarse level draws no houses.
        for (lod, mesh) in model.lods.iter().enumerate().take(2) {
            let nearest = nearest_where(mesh, Vec3::from(muzzle), |v| of_house(v) && v.rig & rig::RECOIL != 0);
            assert!(nearest < 0.4, "fulgur lod{lod}: house {weapon} barrel ends {nearest} m from {muzzle:?}");
            // It stands on the hull: its foot is at or under the pivot, not hanging above the deck.
            let foot = mesh.vertices.iter().filter(|v| of_house(v)).map(|v| v.pos[2]).fold(f32::MAX, f32::min);
            assert!(foot <= pivot[2], "fulgur lod{lod}: house {weapon} floats, foot at {foot}");
            assert!(
                mesh.vertices.iter().filter(|v| of_house(v)).all(|v| v.part == part::HULL),
                "fulgur lod{lod}: house {weapon} rides the hull, not the turret"
            );
        }
    }
    let mesh = &model.lods[0];
    let flak_slot = model.houses.iter().position(|h| h.weapon == 3).unwrap() as u32;
    let flak_top = mesh
        .vertices
        .iter()
        .filter(|v| (v.rig & rig::LIMB_MASK) == rig::HOUSE_FIRST + flak_slot)
        .map(|v| v.pos[2])
        .fold(f32::MIN, f32::max);
    // The main barrel: the turret's recoiling part out past the shroud.
    let barrel_bottom = mesh
        .vertices
        .iter()
        .filter(|v| v.part == part::TURRET && v.rig & rig::RECOIL != 0 && v.pos[0] > 7.0)
        .map(|v| v.pos[2])
        .fold(f32::MAX, f32::min);
    assert!(
        flak_top < barrel_bottom,
        "fulgur: the main barrel (bottom {barrel_bottom}) must pass over the flak house (top {flak_top})"
    );
    let [full, mid, coarse] = [0, 1, 2].map(|lod| triangles(&model.lods[lod]));
    println!("fulgur triangles {full}/{mid}/{coarse}, flak top {flak_top:.2}, barrel bottom {barrel_bottom:.2}");
}

/// The Arbalest plants two spades behind the tail to fire, folded up by the shader about
/// the Trebuchet's rear hinge (x −5.2 at a 1.88 m deck), and its bore recoils.
#[test]
fn arbalest_spades_plant() {
    let model = build_model("bore_tank").unwrap();
    assert!((model.turret_pivot[2] - 1.88).abs() < 1e-3, "deck under the deploy hinges");
    let deploy: Vec<_> = model.lods[0].vertices.iter().filter(|v| v.rig & rig::DEPLOY != 0).collect();
    assert!(!deploy.is_empty(), "the spades plant");
    // All of it behind the rear hinge, none out where the side outriggers fold.
    assert!(deploy.iter().all(|v| v.pos[0] < -5.25 && v.pos[1].abs() < 3.8), "spades fold about the rear hinge");
    let reach_back = deploy.iter().map(|v| v.pos[0]).fold(f32::MAX, f32::min);
    assert!(reach_back < -7.0, "spades reach back to {reach_back}");
    let travel = model.recoil.expect("the bore recoils")[3];
    assert!(travel > 0.2 && travel < 1.0, "bore recoil travel {travel}");
}

/// Validate the changed electric-bore pair independently of unrelated roster models.
#[test]
fn electric_bore_pair_geometry_and_lod_budgets() {
    for key in ["bore_tank", "assault_tank"] {
        let bp = BLUEPRINTS.iter().find(|bp| bp.mesh == key).unwrap();
        let model = built(bp);
        assert!(model.arm_pivot.is_some(), "{key}: missing barrel trunnion");
        for mesh in &model.lods {
            assert!(mesh.vertices.iter().filter(|v| v.part == part::TURRET && v.rig & rig::RECOIL != 0)
                .all(|v| v.rig & rig::LIMB_MASK == rig::ARM_GUN), "{key}: barrel cannot elevate");
        }
        let [full, mid, coarse] = [0, 1, 2].map(|lod| triangles(&model.lods[lod]));
        assert!(full <= 2600 && coarse < 60 && mid as f32 <= full as f32 * 0.45 + 20.0,
            "{key}: {full}/{mid}/{coarse}");
        for (lod, mesh) in model.lods.iter().enumerate() {
            assert!(mesh.vertices.iter().all(|v| Vec3::from(v.pos).is_finite()));
            let muzzle = Vec3::from(bp.muzzles[0]);
            assert!(nearest_where(mesh, muzzle, |v| v.part == part::TURRET && v.rig & rig::RECOIL != 0) < 0.5,
                "{key} lod{lod}: emitter misses the weapon origin");
        }
        println!("{key}: {full}/{mid}/{coarse} triangles");
    }
}
