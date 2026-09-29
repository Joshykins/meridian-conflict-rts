//! Whole-library checks: mesh validity, level-of-detail budgets, blueprint fit
//! and the conventions the renderer relies on (team colour, turret parts).

use glam::Vec3;

use super::library::build_lod;
use super::{
    all_model_keys, build_model, build_model_scaled, material, part, preview, prop_model_key, rig,
    MeshLod, Model, LOD_COUNT,
};

mod skyguard;

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
        // The rail cannons. The shin torpedo tubes ride the legs, not the turret:
        // `paladin_shin_tubes_reach_their_muzzles`.
        &[[10.0, -7.2, 18.0], [10.0, 7.2, 18.0]],
    ),
    unit("artillery_heavy", 11.0, 8.0, 3, &[[6.666, 0.0, 8.599]]),
    // Long guns: the bore reaches well past the hull (`models_fit_their_blueprints`).
    unit("bore_tank", 8.2, 4.2, 3, &[[10.3, 0.0, 3.3]]),
    // The main turret's AEB-2; the sponson and flak houses are
    // `fulgur_houses_and_muzzles`.
    unit("assault_tank", 31.35, 24.75, 4, &[[37.95, 0.0, 18.48]]),
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
            [1.78, -0.27, 0.36],
            [1.78, 0.27, 0.36],
            [0.73, -0.27, 0.36],
            [0.73, 0.27, 0.36],
            [-0.33, -0.27, 0.36],
            [-0.33, 0.27, 0.36],
            [-1.38, -0.27, 0.36],
            [-1.38, 0.27, 0.36],
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
        &[
            [9.6, -0.7, -0.8],
            [9.6, 0.7, -0.8],
            [9.6, -0.7, -1.6],
            [9.6, 0.7, -1.6],
        ],
    ),
    structure("sonar", 6.0, 12.0, 1, 1, &[]),
    structure("sonar", 6.0, 15.0, 2, 1, &[]),
    structure("sonar", 6.0, 18.0, 3, 1, &[]),
    // The rest of the roster (docs/NAVY.md): guns on houses of their own (`rig::HOUSE_*`),
    // no `part::TURRET`, so `hull_unit`. Muzzles are weapon 0's, as authored on the model
    // (a `rear` weapon's muzzles are given to the sim mirrored; here they are as drawn).
    hull_unit("reclaim_boat", 8.0, 6.0, 1, &[]),
    // The Marlin and the Manta are drawn 1.2 times their authored size.
    hull_unit("destroyer", 26.4, 14.4, 2, &[[23.28, 0.0, 6.24]]),
    hull_unit(
        "aa_cruiser",
        26.4,
        16.8,
        2,
        &[
            [8.7, -2.7, 7.68],
            [8.7, -0.9, 7.68],
            [8.7, 0.9, 7.68],
            [8.7, 2.7, 7.68],
            [6.9, -2.7, 7.68],
            [6.9, -0.9, 7.68],
            [6.9, 0.9, 7.68],
            [6.9, 2.7, 7.68],
            [5.1, -2.7, 7.68],
            [5.1, -0.9, 7.68],
            [5.1, 0.9, 7.68],
            [5.1, 2.7, 7.68],
            [3.3, -2.7, 7.68],
            [3.3, -0.9, 7.68],
            [3.3, 0.9, 7.68],
            [3.3, 2.7, 7.68],
        ],
    ),
    hull_unit(
        "missile_ship",
        20.0,
        10.0,
        2,
        &[
            [2.0, -1.5, 6.0],
            [2.0, 1.5, 6.0],
            [0.0, -1.5, 6.0],
            [0.0, 1.5, 6.0],
            [-2.0, -1.5, 6.0],
            [-2.0, 1.5, 6.0],
            [-4.0, -1.5, 6.0],
            [-4.0, 1.5, 6.0],
        ],
    ),
    hull_unit(
        "submarine_hunter",
        14.0,
        4.2,
        2,
        &[
            [13.4, -0.9, -1.0],
            [13.4, 0.9, -1.0],
            [13.4, -0.9, -1.9],
            [13.4, 0.9, -1.9],
            [13.4, -0.9, -2.8],
            [13.4, 0.9, -2.8],
        ],
    ),
    hull_unit("shield_boat", 16.0, 12.0, 2, &[]),
    hull_unit(
        "battleship",
        72.0,
        38.0,
        3,
        &[[66.0, -3.4, 11.4], [66.0, 0.0, 11.4], [66.0, 3.4, 11.4]],
    ),
    hull_unit("rail_trimaran", 84.0, 34.0, 3, &[[86.0, 0.0, 24.0]]),
    hull_unit(
        "carrier",
        60.0,
        24.0,
        3,
        &[
            [-12.0, 8.0, 8.5],
            [-14.0, 8.0, 8.5],
            [-12.0, -8.0, 8.5],
            [-14.0, -8.0, 8.5],
        ],
    ),
    hull_unit(
        "submarine_strategic",
        30.0,
        5.0,
        3,
        &[
            [28.0, -1.0, -1.5],
            [28.0, 1.0, -1.5],
            [28.0, -2.2, -1.5],
            [28.0, 2.2, -1.5],
            [28.0, -1.0, -2.6],
            [28.0, 1.0, -2.6],
            [28.0, -2.2, -2.6],
            [28.0, 2.2, -2.6],
        ],
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
    structure("citadel", 20.0, 17.0, 3, 4, &[[34.0, 0.0, 13.0]]),
    structure("radar", 10.5, 35.0, 1, 2, &[]),
    structure("radar", 10.5, 42.0, 2, 2, &[]),
    structure("radar", 10.5, 49.0, 3, 2, &[]),
    structure("reclaim_tower", 16.9, 38.0, 1, 3, &[]),
    structure("reclaim_tower", 16.9, 38.0, 2, 3, &[]),
    structure("reclaim_tower", 16.9, 38.0, 3, 3, &[]),
    structure("shield", 13.9, 40.0, 2, 3, &[]),
    structure("shield", 13.9, 52.0, 3, 3, &[]),
    structure("wall", 6.0, 4.5, 1, 1, &[]),
    // Strategic weapons (strategic.ron).
    structure("nuke_silo", 42.5, 26.0, 4, 8, &[]),
    structure("nuke_defense", 18.75, 20.0, 3, 4, &[]),
    structure("culverin", 30.0, 27.0, 4, 6, &[[74.0, 0.0, 18.0]]),
    // The Naga (data/factions/naga/units): their engineer, scout and tech 1 structures.
    unit("naga_engineer", 3.6, 2.8, 1, &[]),
    unit("naga_scout", 3.8, 4.0, 1, &[[1.6, 0.0, 2.3]]),
    structure("naga_brood", 46.0, 22.0, 1, 8, &[]),
    structure("naga_brood", 46.0, 22.0, 2, 8, &[]),
    structure("naga_brood", 46.0, 35.0, 3, 8, &[]),
    structure("naga_hatchery", 46.0, 30.0, 1, 8, &[]),
    structure("naga_hatchery", 46.0, 30.0, 2, 8, &[]),
    structure("naga_hatchery", 46.0, 38.0, 3, 8, &[]),
    structure("naga_tidebrood", 46.0, 20.0, 1, 8, &[]),
    structure("naga_taproot", 12.8, 11.0, 1, 3, &[]),
    structure("naga_taproot", 12.8, 15.0, 2, 3, &[]),
    structure("naga_taproot", 12.8, 19.0, 3, 3, &[]),
    structure("naga_taproot", 12.8, 19.0, 4, 3, &[]),
    structure("naga_heart", 6.9, 7.5, 1, 2, &[]),
    structure("naga_heart_2", 18.75, 18.0, 2, 4, &[]),
    structure("naga_heart_3", 42.5, 35.0, 3, 8, &[]),
    structure("naga_cyst", 12.9, 8.0, 1, 3, &[]),
    structure("naga_barb", 5.5, 8.0, 1, 1, &[[5.2, 0.0, 6.8]]),
    structure("naga_spitter", 5.5, 8.5, 1, 1, &[[4.4, 0.0, 7.4]]),
    structure("naga_thornwall", 6.0, 5.0, 1, 1, &[]),
    structure("naga_eye", 7.0, 24.0, 1, 2, &[]),
    structure("naga_eye", 7.0, 28.0, 2, 2, &[]),
    structure("naga_eye", 7.0, 32.0, 3, 2, &[]),
];

/// Ships: the keel is below the waterline (model z = 0), and nothing is running gear.
const NAVAL_HULLS: &[&str] = &[
    "attack_boat",
    "frigate",
    "submarine",
    "reclaim_boat",
    "destroyer",
    "aa_cruiser",
    "missile_ship",
    "submarine_hunter",
    "shield_boat",
    "battleship",
    "carrier",
    "rail_trimaran",
    "submarine_strategic",
];
/// The capital ships: 120 m hulls with the triangle budget of a factory.
const CAPITAL_SHIPS: &[&str] = &["battleship", "carrier", "rail_trimaran"];
/// Tech 2 and 3 warships under 70 m: bigger than any land unit, few of them, a budget between.
const WARSHIP_TRIANGLES: usize = 3600;
const WARSHIPS: &[&str] = &[
    "destroyer",
    "aa_cruiser",
    "missile_ship",
    "shield_boat",
    "submarine_hunter",
    "submarine_strategic",
];

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
    "tree_palm",
    "tree_jungle",
    "rock_small",
    "rock_large",
    "building_small",
    "building_tower",
    "building_wide",
];

/// The mesh a design variant (`mesh~name`) is drawn for: its rules are that mesh's.
fn base_key(key: &str) -> &str {
    key.split('~').next().unwrap_or(key)
}

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

/// Triangles drawn. A joining wall draws one piece a quarter: the most any neighbours
/// have it draw.
pub(super) fn triangles(mesh: &MeshLod) -> usize {
    let walls = part::WALL_FIRST..part::WALL_FIRST + part::WALL_COUNT;
    if !mesh.vertices.iter().any(|v| walls.contains(&v.part)) {
        return mesh.indices.len() / 3;
    }
    (0..256)
        .map(|joins| {
            mesh.indices
                .chunks(3)
                .filter(|t| super::wall::shown(mesh.vertices[t[0] as usize].part, joins))
                .count()
        })
        .max()
        .unwrap_or(0)
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
    assert_eq!(prop_model_key(4), "tree_palm");
    assert_eq!(prop_model_key(5), "tree_jungle");
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
                    v.material <= material::LAST
                        && (v.part <= part::CRADLE
                            || (part::RAM..=part::SILO_ROUND).contains(&v.part)
                            || (part::WALL_FIRST..part::WALL_FIRST + part::WALL_COUNT)
                                .contains(&v.part)
                            || v.part == part::LAUNCHER_HOIST
                            || v.part == part::CELL_HATCH
                            || v.part == part::CELL_ROUND),
                    "{name}: ids"
                );
                // Units stand on the ground; props are rooted a little into it for slopes.
                let is_prop = ["tree_", "rock_", "building_", "precursor_", "landmark_"]
                    .iter()
                    .any(|family| model.key.starts_with(family));
                // The naval yard stands in water on piles driven into the seabed.
                let floor = if v.rig & rig::DEPLOY != 0 && v.rig & rig::STAKE_SPIKE != 0 {
                    // A planted ground stake's point, driven into the ground.
                    -1.0
                } else if model.key == "landmark_dam" {
                    // The dam stands on the gorge's floor, far under its crest road.
                    super::dam::FLOOR
                } else if model.key == "landmark_span" {
                    // A power line's span is built about its pivot, raised to it in the air.
                    super::dam_works::SPAN_FLOOR
                } else if model.key == "precursor_lining" {
                    // The lining cases a shaft wall, its full depth below the rim.
                    -super::precursor_polar::LINING_DEPTH - 10.0
                } else if model.key.starts_with("precursor_") {
                    // Precursor artifacts run deep: half-buried rings and shards, footings
                    // sunk so they stand on a slope without showing their underside.
                    -80.0
                } else if is_prop {
                    -3.0
                } else if model.key == "factory_naval" {
                    -91.0
                } else if CAPITAL_SHIPS.contains(&model.key.as_str())
                    || model.key == "submarine_strategic"
                {
                    // A capital ship's keel, or a big submarine's hull, runs deep.
                    -12.0
                } else if NAVAL_HULLS.contains(&base_key(&model.key)) {
                    // Hulls float: the keel is under the waterline.
                    -4.5
                } else if model.key == "sonar" {
                    // The sonar buoy's hydrophone arrays hang under it.
                    -15.0
                } else if model.key == "nuke_silo" {
                    // The launch tube, dug in below its mouth (`Model::pit`).
                    -18.0
                } else if model.key == "airbase" {
                    // The parked Roost's shaft, down to the lift 21 m under the deck.
                    -22.0
                } else if model.key == "naga_taproot" {
                    // The bore the beam cuts, down to the deep core's floor (`Model::pit`).
                    -121.0
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
                        geometric.normalize().dot(shading) > 0.5,
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
/// The Leviathan, the navy's hero: more than a factory's budget for its layered detail,
/// and nine Arc Cannons (the Trebuchet's howitzer tube, about 700 triangles each).
const BATTLESHIP_TRIANGLES: usize = 15000;
const CORE_MINE_TRIANGLES: usize = 9000;
/// The tech 4 assault tank runs on four open track units (road wheels, toothed
/// sprockets, return rollers seen through the side) and carries two bolt rifles and the
/// capacitor-fed AEB-2, and finned heat sinks.
const ASSAULT_TANK_TRIANGLES: usize = 9200;

/// Models already over the rules below when they were last checked (2026-09-28),
/// held where they are so they cannot grow further: its coarse level's triangles,
/// its reduced level's share of the full one, and its full level's triangles
/// (`None` keeps the rule). Each wants lighter levels, made and judged on a shot
/// sheet; its row goes once it meets the rules.
const OVER_BUDGET: &[Over] = &[
    ("storage_mass", Some(102), Some(0.7), Some(2882)),
    ("sonar", Some(176), None, None),
    ("reclaimer", Some(122), Some(0.54), Some(3604)),
    ("airbase", None, Some(0.53), None),
    ("precursor_bastion", Some(222), None, None),
    ("precursor_boom", Some(228), None, None),
    ("precursor_vault", Some(404), None, None),
    ("precursor_axis", Some(206), None, None),
    ("precursor_terrace", Some(112), None, None),
    ("precursor_citadel", Some(356), Some(0.61), None),
    ("precursor_seaway", None, Some(1.0), None),
];
type Over = (&'static str, Option<usize>, Option<f32>, Option<usize>);

#[test]
fn lods_reduce_and_respect_budgets() {
    for model in every_model() {
        let [full, reduced, coarse] = [0, 1, 2].map(|lod| triangles(&model.lods[lod]));
        assert!(
            full >= reduced && reduced >= coarse,
            "{}: {full} >= {reduced} >= {coarse}",
            model.key
        );
        let over = OVER_BUDGET.iter().find(|o| o.0 == model.key);
        let coarse_cap = if model.key == "nuke_silo" {
            // The open silo's rim and tube mouth (its own test in `strategic.rs`).
            80
        } else {
            over.and_then(|o| o.1).map_or(60, |cap| cap + 1)
        };
        assert!(
            coarse < coarse_cap,
            "{}: coarse LOD has {coarse} triangles",
            model.key
        );
        let budget = if let Some(cap) = over.and_then(|o| o.3) {
            cap
        } else if model.key == "core_mine" {
            // The pit it digs is real geometry, down to the deep core's shaft.
            CORE_MINE_TRIANGLES
        } else if super::precursor_forge::MODELS
            .iter()
            .any(|d| d.key == model.key)
        {
            // The Threshold's facility kit: one map, a few of each, kilometres high.
            super::precursor_forge::TRIANGLES
        } else if super::precursor_sky::MODELS
            .iter()
            .any(|d| d.key == model.key)
        {
            super::precursor_sky::TRIANGLES
        } else if super::precursor_gate::MODELS
            .iter()
            .any(|d| d.key == model.key)
        {
            super::precursor_gate::TRIANGLES
        } else if super::precursor_mega::MODELS
            .iter()
            .chain(super::precursor_tower::MODELS)
            .any(|d| d.key == model.key)
        {
            // A map's one machine: a few pieces hundreds of metres high.
            super::precursor_mega::TRIANGLES
        } else if super::precursor_polar::MODELS
            .iter()
            .any(|d| d.key == model.key)
        {
            // The Axis's crown and the pieces made with it (`precursor_polar`'s own test).
            super::precursor_polar::TRIANGLES
        } else if super::precursor_citadel::MODELS
            .iter()
            .any(|d| d.key == model.key)
        {
            super::precursor_citadel::TRIANGLES
        } else if model.key == "landmark_dam" {
            // The canyon map's 400 m arch dam, one a map.
            super::dam::TRIANGLES
        } else if let Some(budget) = super::dam_works::triangles(&model.key) {
            // The works round it: its switchyard, its line's towers, its town.
            budget
        } else if model.key == "replication_engine" {
            // One 240 m landmark per match (Survival).
            super::replicator::ENGINE_TRIANGLES
        } else if model.key.starts_with("factory_")
            || model.key == "power"
            || model.key == "airbase"
            || model.key == "nuke_silo"
            || model.key == "nuke_defense"
            || model.key == "anti_ship_rail"
            || model.key == "culverin"
        {
            // The tech 3 reactor stands on a factory's lot; the airbase's shaft is real geometry.
            FACTORY_TRIANGLES
        } else if model.key == "lift_ship" {
            // 300 m capital hull: full ventral bay, four drive bells, four gun houses, articulated gear.
            14000
        } else if model.key == "space_frigate" {
            // 325 m tech 3 heavy frigate in sections: trenched spine with the rail's collars,
            // bridge and search radar, four rail houses, two nacelles of two deep drives, legs.
            17000
        } else if model.key == "titan" {
            // The tech 5 Behemoth: a 400 m walker, one a match; two long rigged legs, six
            // rails in a rotary cluster, the AEB-3, rocket pods, two flak turrets, and the
            // deck gear that tells its size.
            30000
        } else if model.key == "citadel" {
            // The tech 3 rail keep: a 4x4 lot, corner towers, a casemate and a 54 m rail.
            4200
        } else if model.key == "assault_bot" {
            // The tech 3 Paladin: two rigged legs with shin tubes, and a bolt rifle carried
            // in each hooded shoulder, its plasma cell down the flank.
            3400
        } else if model.key == "assault_tank" {
            ASSAULT_TANK_TRIANGLES
        } else if model.key.starts_with("reclaim_tower") {
            // A 3x3 installation, a 38 m tower with the plant round its foot.
            super::aster::reclaim_tower::TRIANGLES
        } else if let Some(budget) = super::naga::triangles(&model.key) {
            budget
        } else if model.key == "light_transport" {
            // 115 m spacecraft: walk-through bay, two drive bells, lift jets, dorsal mast.
            9000
        } else if model.key == "battleship" {
            // 142 m hero hull: layered sides, a stepped pagoda, three triple Arc Cannon houses.
            BATTLESHIP_TRIANGLES
        } else if CAPITAL_SHIPS.contains(&model.key.as_str()) {
            FACTORY_TRIANGLES
        } else if WARSHIPS.contains(&base_key(&model.key)) {
            // A design variant (`mesh~name`) has its hull's budget.
            WARSHIP_TRIANGLES
        } else {
            2600
        };
        // A core mine draws either its stilts or its pit, never both.
        let drawn = if model.key == "core_mine" {
            let of = |kind| {
                let mesh = &model.lods[0];
                mesh.indices
                    .chunks(3)
                    .filter(|t| mesh.vertices[t[0] as usize].part == kind)
                    .count()
            };
            full - of(part::AFLOAT).min(of(part::ASHORE))
        } else {
            full
        };
        assert!(
            drawn <= budget,
            "{}: full LOD draws {drawn} triangles",
            model.key
        );
        let share = over.and_then(|o| o.2).unwrap_or(0.45);
        assert!(
            reduced as f32 <= full as f32 * share + 20.0,
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
                    && (v.part == part::AFLOAT
                        || Vec3::from(v.pos).truncate().length() <= pit.radius)
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
                        hull_reach <= bp.radius * 1.3
                            && reach <= (bp.radius * 1.3).max(barrel + 0.5),
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
            // The Naga wear black hide (`PLATING_DARK`) where ARC wears its plating.
            assert!(
                mesh.vertices.iter().any(
                    |v| v.material == material::PLATING || v.material == material::PLATING_DARK
                ),
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
        // Engineers' build arms are their turrets.
        .filter(|bp| bp.muzzles.is_empty() && !["engineer", "naga_engineer"].contains(&bp.mesh))
    {
        assert!(
            built(bp)
                .lods
                .iter()
                .all(|m| m.vertices.iter().all(|v| v.part != part::TURRET
                    // A reclaim head's house (`reclaimers::cradle_turret`) is tagged as
                    // one: it is a tool on a house of its own, not a gun turret.
                    || (rig::HOUSE_FIRST..rig::HOUSE_FIRST + rig::HOUSE_COUNT)
                        .contains(&(v.rig & rig::LIMB_MASK)))),
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

/// The Trebuchet and the Arbalest plant a stake on each corner, fired from a tube into
/// the ground (`gpu_consts::stake`): the spikes are authored driven in past the ground,
/// down the tube's line from its hinge.
#[test]
fn siege_stakes_plant() {
    use crate::stakes::{stake_scale, STAKES};
    let mut orders: Vec<usize> = STAKES.iter().map(|s| s.order()).collect();
    orders.sort_unstable();
    assert_eq!(orders, [0, 1, 2, 3], "each stake has its own turn");
    for key in ["artillery_heavy", "bore_tank"] {
        let model = build_model(key).unwrap();
        assert_eq!(stake_scale(&model), Some(1.0), "{key} plants stakes");
        assert!(
            (model.turret_pivot[2] - 1.88).abs() < 1e-3,
            "{key}: the deck under the stakes' hinges"
        );
        let verts = &model.lods[0].vertices;
        assert!(
            verts
                .iter()
                .filter(|v| v.rig & rig::DEPLOY != 0)
                .all(|v| v.rig & rig::STAKE != 0),
            "all {key} plants is its stakes"
        );
        for s in STAKES {
            assert!(s.strikes() < 0.95, "{s:?} strikes after the deploy is over");
            let (hinge, down) = (s.hinge(), s.down());
            let spike: Vec<_> = verts
                .iter()
                .filter(|v| v.rig & rig::STAKE_SPIKE != 0)
                .filter(|v| (v.pos[0] > 0.0) == s.front && (v.pos[1] > 0.0) == s.left)
                .collect();
            assert!(!spike.is_empty(), "{key}: no spike on {s:?}");
            let tip = spike.iter().map(|v| v.pos[2]).fold(f32::MAX, f32::min);
            assert!(
                tip < -0.4,
                "{key} {s:?}: the spike stops at {tip}, not driven into the ground"
            );
            for v in &spike {
                let q = Vec3::from(v.pos) - hinge;
                let off = (q - down * q.dot(down)).length();
                assert!(
                    off < 0.4,
                    "{key} {s:?}: spike vertex {off} m off the tube's line"
                );
            }
            assert!(s.strike_point().z.abs() < 1e-4);
        }
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
    for bp in BLUEPRINTS.iter().filter(|bp| {
        bp.footprint.is_none()
            && !["interceptor", "bomber"].contains(&bp.mesh)
            && !NAVAL_HULLS.contains(&bp.mesh)
    }) {
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
        "hover_tank",
        // Missile cells carry orange seams; the carrier's flak and rotary gun are orange too.
        "missile_ship",
        "carrier",
    ];
    // Whether a gun carries light is its own design's call; only the orange is ruled:
    // it is on the units above and nowhere else.
    for bp in BLUEPRINTS.iter().filter(|bp| !bp.muzzles.is_empty()) {
        let mesh = &built(bp).lods[0];
        let orange_lit = mesh
            .vertices
            .iter()
            .any(|v| v.material == material::GLOW_ORANGE);
        assert_eq!(orange_lit, orange.contains(&bp.mesh), "{}", bp.mesh);
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
    for (key, t1) in [
        ("factory_land", 28.0),
        ("factory_air", 26.0),
        ("factory_naval", 26.0),
    ] {
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
            let reach = mesh
                .vertices
                .iter()
                .map(|v| v.pos[1])
                .fold(f32::MIN, f32::max);
            assert!(
                reach < -4.0,
                "T{tech} lod{lod}: yard reaches y {reach} into the berth"
            );
            // It stands on piles down past any seabed.
            let foot = mesh
                .vertices
                .iter()
                .map(|v| v.pos[2])
                .fold(f32::MAX, f32::min);
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
        let factory = mc_core::print_heads::factory_heads(key).unwrap();
        let model = build_model_scaled(key, 46.0, 42.0, 3).unwrap();
        for head in factory.heads {
            let tip = Vec3::from(mc_core::print_heads::nozzle(head, factory.aim));
            let near = model.lods[0]
                .vertices
                .iter()
                .filter(|v| v.material == material::GLOW_AMBER)
                .map(|v| Vec3::from(v.pos).distance(tip))
                .fold(f32::MAX, f32::min);
            assert!(
                near < 0.2,
                "{key}: no amber within {near} m of the head at {:?}",
                head.mount
            );
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
    for key in [
        "factory_land",
        "factory_air",
        "factory_naval",
        "extractor",
        "power",
        "radar",
    ] {
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
/// `cargo test -p mc-models -- --ignored dump_models`
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
    dump(
        "mobile_aa_t2",
        &build_model_scaled("mobile_aa", 5.5, 5.5, 2).unwrap(),
    );
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
        let vtol = model.vtol.expect("VTOL pods");
        // Each pod's nozzle, lying along the hull (the rest pose).
        let ports: Vec<[f32; 3]> = vtol
            .pods()
            .flat_map(|(p, _)| [1.0, -1.0].map(|s| [p[0] - vtol.nozzle[0], p[1] * s, p[2]]))
            .collect();
        for (level, lod) in model.lods.iter().enumerate() {
            for (pivot, front) in vtol.pods() {
                let part = if front {
                    part::VTOL_FRONT
                } else {
                    part::VTOL_REAR
                };
                let pod: Vec<Vec3> = lod
                    .vertices
                    .iter()
                    .filter(|v| v.part == part)
                    .map(|v| Vec3::from(v.pos))
                    .collect();
                assert!(!pod.is_empty(), "{key} LOD{level}: pod {part} is missing");
                // Everything on a pod stays near its pivot, so the tilt reads as a tilt.
                for p in pod {
                    let pivot = Vec3::new(pivot[0], pivot[1] * p.y.signum(), pivot[2]);
                    assert!(
                        p.distance(pivot) < 3.4,
                        "{key} LOD{level}: pod {part} vertex {p} is far from its pivot"
                    );
                }
            }
        }
        let mesh = &model.lods[0];
        for port in ports {
            let p = Vec3::from(port);
            let near = mesh
                .indices
                .chunks(3)
                .filter(|t| {
                    mesh.vertices[t[0] as usize].part == part::VTOL_FRONT
                        || mesh.vertices[t[0] as usize].part == part::VTOL_REAR
                })
                .map(|t| {
                    closest_point_on_triangle(
                        p,
                        position(mesh, t[0]),
                        position(mesh, t[1]),
                        position(mesh, t[2]),
                    )
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
        assert!(
            lod.vertices.iter().any(|v| v.part == part::HOLD_DOOR),
            "LOD{level}: no hold doors"
        );
    }
    assert!(
        model.lods[0]
            .vertices
            .iter()
            .any(|v| v.part == part::CRADLE),
        "no cradles"
    );
    let (cradles, ceiling) = super::carrier_cradles();
    let drone = build_model("reclaim_drone").unwrap();
    let half_width = drone.lods[0]
        .vertices
        .iter()
        .map(|v| v.pos[1].abs())
        .fold(0.0, f32::max);
    let top = drone.lods[0]
        .vertices
        .iter()
        .map(|v| v.pos[2])
        .fold(0.0, f32::max);
    for c in cradles {
        assert!(
            c[1].abs() + half_width < super::aster::air::osprey::HOLD_HALF_WIDTH,
            "a docked drone touches the hold's side"
        );
    }
    // Stowed 0.7 m up (`drone_socket`), a drone's top is under the ceiling.
    assert!(
        0.7 + top <= ceiling + 0.15,
        "a docked drone {top} tall sticks through the ceiling at {ceiling}"
    );
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
    let keys = [
        "air_scout",
        "rotor_gunship",
        "support_air",
        "reclaim_carrier",
        "reclaim_drone",
        "gunship",
        "fire_bomber",
        "torpedo_bomber",
        "interceptor_t2",
        "superiority",
        "strategic_bomber",
        "assault_air",
        "aa_gun",
        "flak_battery",
        "aa_sam",
        "mobile_aa",
        "factory_air",
    ];
    for key in keys {
        for tech in 1..=3 {
            let (r, h) = if key == "factory_air" {
                (
                    46.0,
                    match tech {
                        1 => 26.0,
                        2 => 34.0,
                        _ => 42.0,
                    },
                )
            } else if key == "mobile_aa" {
                if tech == 1 {
                    (4.0, 4.5)
                } else {
                    (5.5, 5.5)
                }
            } else {
                (10.0, 10.0)
            };
            let model = build_model_scaled(key, r, h, tech).unwrap();
            let [full, mid, coarse] = [0, 1, 2].map(|i| triangles(&model.lods[i]));
            let budget = if key == "factory_air" {
                FACTORY_TRIANGLES
            } else {
                2600
            };
            assert!(
                coarse < 60 && full <= budget && mid as f32 <= full as f32 * 0.45 + 20.0,
                "{key} T{tech}: {full}/{mid}/{coarse}"
            );
        }
    }
}

#[test]
fn tree_canopies_are_cutout_sprays_with_bounded_lods() {
    for key in [
        "tree_broadleaf",
        "tree_conifer",
        "tree_pine",
        "tree_palm",
        "tree_jungle",
    ] {
        let model = build_model(key).unwrap();
        let counts = model.lods.each_ref().map(|m| m.indices.len() / 3);
        // Forests carry hundreds of thousands of trees: the reduced level is a
        // hundred-odd triangles and the coarse one a handful of cards.
        assert!(
            counts[0] <= 1000 && counts[1] <= 200 && counts[2] <= 32,
            "{key}: {counts:?}"
        );
        assert!(
            counts[1] as f32 <= counts[0] as f32 * 0.45 + 20.0,
            "{key}: {counts:?}"
        );
        for mesh in &model.lods {
            let leaves: Vec<_> = mesh
                .vertices
                .iter()
                .filter(|v| v.material == material::FOLIAGE)
                .collect();
            assert!(!leaves.is_empty(), "{key}: missing foliage");
            // Cards show a region of a cutout atlas, never a solid canopy surface.
            assert!(
                leaves
                    .iter()
                    .all(|v| v.uv.iter().all(|u| (0.0..=1.0).contains(u))),
                "{key}: card outside its atlas"
            );
            assert!(
                leaves
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|card| card[0].uv != card[2].uv),
                "{key}: card without an atlas region"
            );
            // Each leaf knows its crown: an outward normal and how deep in the crown it sits.
            for v in &leaves {
                let crown = Vec3::new(v.face[0], v.face[1], v.face[2]);
                assert!(
                    (crown.length() - 1.0).abs() < 1e-3 && (0.0..=1.0).contains(&v.face[3]),
                    "{key}: {:?}",
                    v.face
                );
            }
            assert!(
                mesh.vertices.iter().any(|v| v.material == material::BARK),
                "{key}: missing branches"
            );
            for face in mesh.indices.as_chunks::<3>().0 {
                let [a, b, c] = [face[0], face[1], face[2]].map(|i| &mesh.vertices[i as usize]);
                let n = (Vec3::from(b.pos) - Vec3::from(a.pos))
                    .cross(Vec3::from(c.pos) - Vec3::from(a.pos));
                assert!(n.length() > 0.00001);
                assert!(n.normalize().dot(Vec3::from(a.normal)) > 0.5);
            }
        }
    }
}

/// The tropical trees show the tropical leaf atlas and pale bark; the temperate
/// ones keep their own atlases.
#[test]
fn tropical_trees_pick_the_tropical_atlas_and_pale_bark() {
    use super::pattern;
    let pattern_of = |v: &super::MeshVertex| v.surface & 0xFF;
    for key in ["tree_palm", "tree_jungle"] {
        for mesh in &build_model(key).unwrap().lods {
            let mut leaves = mesh
                .vertices
                .iter()
                .filter(|v| v.material == material::FOLIAGE);
            assert!(
                leaves.all(|v| pattern_of(v) == pattern::PLAIN),
                "{key}: temperate leaves"
            );
            let mut bark = mesh
                .vertices
                .iter()
                .filter(|v| v.material == material::BARK);
            // Pale bark (SHUTTER), ringed (DECK), and brown coconuts (GENERIC).
            assert!(
                bark.all(|v| matches!(
                    pattern_of(v),
                    pattern::SHUTTER | pattern::DECK | pattern::GENERIC
                )),
                "{key}: bark pattern"
            );
        }
    }
    for key in ["tree_broadleaf", "tree_conifer", "tree_pine"] {
        for mesh in &build_model(key).unwrap().lods {
            assert!(
                mesh.vertices
                    .iter()
                    .filter(|v| v.material == material::FOLIAGE)
                    .all(|v| pattern_of(v) == pattern::NONE),
                "{key}: leaf atlas changed"
            );
        }
    }
    // Tall enough to read over a temperate wood: a palm ~15 m, the jungle tree ~22 m.
    for (key, lo, hi) in [("tree_palm", 12.0, 18.0), ("tree_jungle", 18.0, 26.0)] {
        let top = build_model(key).unwrap().lods[0]
            .vertices
            .iter()
            .map(|v| v.pos[2])
            .fold(f32::MIN, f32::max);
        assert!((lo..=hi).contains(&top), "{key}: top at {top}");
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
        let keys: Vec<&str> = set
            .slots
            .iter()
            .flat_map(|s| &s.modules)
            .map(|m| m.key.as_str())
            .collect();
        let model = super::build_model_fitted(
            &bp.visual.mesh,
            bp.radius.to_f32(),
            bp.height.to_f32(),
            bp.tech,
            &keys,
        )
        .expect("the unit has a model");
        let bare = build_model(&bp.visual.mesh).unwrap();
        for (lod, mesh) in model.lods.iter().enumerate() {
            for (i, key) in keys.iter().enumerate() {
                let tag = (i as u32 + 1) << rig::MODULE_SHIFT;
                assert!(
                    mesh.vertices
                        .iter()
                        .any(|v| v.rig & rig::MODULE_MASK == tag),
                    "{} lod{lod}: module {key} has no pieces",
                    bp.key
                );
            }
            // Untagged, the model is the bare unit.
            let always = mesh
                .vertices
                .iter()
                .filter(|v| v.rig & rig::MODULE_MASK == 0)
                .count();
            assert_eq!(always, bare.lods[lod].vertices.len(), "{} lod{lod}", bp.key);
        }
    }
}

#[test]
fn warden_cannon_recoils_at_every_lod() {
    let model = build_model("tank_light").unwrap();
    let travel = model.recoil.expect("warden gun has no recoil travel")[3];
    assert!(
        travel > 0.3 && travel < 1.5,
        "warden recoil travel {travel}"
    );
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
                    closest_point_on_triangle(
                        m,
                        position(mesh, t[0]),
                        position(mesh, t[1]),
                        position(mesh, t[2]),
                    )
                    .distance(m)
                })
                .fold(f32::MAX, f32::min);
            assert!(
                nearest < 0.3,
                "frigate lod{lod}: AA barrel ends {nearest} m from {muzzle:?}"
            );
        }
    }
}

#[test]
fn ships_float_and_radars_turn() {
    for key in ["attack_boat", "frigate", "submarine"] {
        let model = build_model(key).unwrap();
        for mesh in &model.lods {
            // Nothing the water shader would lift as running gear.
            assert!(
                mesh.vertices.iter().all(|v| v.part != part::LOCOMOTION),
                "{key}: running gear"
            );
            // The hull goes down into the water.
            let keel = mesh
                .vertices
                .iter()
                .map(|v| v.pos[2])
                .fold(f32::MAX, f32::min);
            assert!(keel < -0.4, "{key}: keel at {keel}");
        }
    }
    for key in ["attack_boat", "frigate"] {
        let model = build_model(key).unwrap();
        assert!(
            model
                .lods
                .iter()
                .take(2)
                .all(|m| m.vertices.iter().any(|v| v.part == part::SPINNER)),
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
            .filter(|v| {
                matches!(
                    v.material,
                    material::GLOW | material::GLOW_ORANGE | material::GLOW_AMBER
                )
            })
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
    assert!(
        has(1, 12.0),
        "T1 carries the T2 float ring and array as upgrade pieces"
    );
    assert!(
        has(2, 15.0),
        "T2 carries the T3 transducer and mast as upgrade pieces"
    );
    assert!(!has(3, 18.0), "T3 is finished");
}

/// The Leviathan: four houses bound to its four weapons at the unit file's pivots, every
/// weapon's muzzles reached by a barrel at every level, and the capital-ship budgets.
#[test]
fn battleship_houses_and_muzzles() {
    let bp = BLUEPRINTS
        .iter()
        .find(|bp| bp.mesh == "battleship")
        .unwrap();
    let model = built(bp);
    let want: [(u8, [f32; 3]); 8] = [
        (0, [44.0, 0.0, 11.0]),
        (1, [24.0, 0.0, 14.6]),
        (2, [-40.0, 0.0, 8.8]),
        (3, [-14.0, 0.0, 18.0]),
        (4, [8.0, 10.5, 9.0]),
        (5, [-20.0, 10.5, 9.0]),
        (6, [8.0, -10.5, 9.0]),
        (7, [-20.0, -10.5, 9.0]),
    ];
    assert_eq!(model.houses.len(), 8, "battleship: eight gun houses");
    for (weapon, pivot) in want {
        let house = model
            .houses
            .iter()
            .find(|h| h.weapon == weapon)
            .expect("house per weapon");
        assert!(
            Vec3::from(house.pivot).distance(Vec3::from(pivot)) < 1e-3,
            "battleship: house {weapon} pivot {:?}",
            house.pivot
        );
    }
    // Muzzles of every weapon (the aft battery's as authored, facing forward).
    // The secondaries' as authored too: facing the nose, though they rest trained outboard.
    let muzzles: [(&str, &[[f32; 3]]); 8] = [
        (
            "fore",
            &[[66.0, -3.4, 11.4], [66.0, 0.0, 11.4], [66.0, 3.4, 11.4]],
        ),
        (
            "second",
            &[[46.0, -3.4, 15.0], [46.0, 0.0, 15.0], [46.0, 3.4, 15.0]],
        ),
        (
            "aft",
            &[[-18.0, -3.4, 9.2], [-18.0, 0.0, 9.2], [-18.0, 3.4, 9.2]],
        ),
        ("aa", &[[-11.4, -0.4, 18.2], [-11.4, 0.4, 18.2]]),
        (
            "port fore secondary",
            &[[15.0, 9.9, 9.3], [15.0, 11.1, 9.3]],
        ),
        (
            "port aft secondary",
            &[[-13.0, 9.9, 9.3], [-13.0, 11.1, 9.3]],
        ),
        (
            "starboard fore secondary",
            &[[15.0, -9.9, 9.3], [15.0, -11.1, 9.3]],
        ),
        (
            "starboard aft secondary",
            &[[-13.0, -9.9, 9.3], [-13.0, -11.1, 9.3]],
        ),
    ];
    for (lod, mesh) in model.lods.iter().enumerate() {
        for (name, list) in muzzles {
            // The coarse level keeps only the forward batteries' barrels.
            if lod == 2 && !(name == "fore" || name == "second") {
                continue;
            }
            for muzzle in list {
                let p = Vec3::from(*muzzle);
                let nearest = mesh
                    .indices
                    .chunks(3)
                    .map(|t| {
                        closest_point_on_triangle(
                            p,
                            position(mesh, t[0]),
                            position(mesh, t[1]),
                            position(mesh, t[2]),
                        )
                        .distance(p)
                    })
                    .fold(f32::MAX, f32::min);
                assert!(
                    nearest < 0.4,
                    "battleship lod{lod}: {name} barrel ends {nearest} m from {muzzle:?}"
                );
            }
        }
        let team_up = mesh
            .vertices
            .iter()
            .any(|v| v.material == material::TEAM && v.normal[2] > 0.5);
        assert!(
            team_up,
            "battleship lod{lod}: no team colour seen from above"
        );
        let floor = mesh
            .vertices
            .iter()
            .map(|v| v.pos[2])
            .fold(f32::MAX, f32::min);
        assert!(floor >= -12.0, "battleship lod{lod}: keel at {floor}");
    }
    let [full, mid, coarse] = [0, 1, 2].map(|lod| triangles(&model.lods[lod]));
    assert!(
        (250..=BATTLESHIP_TRIANGLES).contains(&full),
        "battleship: {full} triangles"
    );
    assert!(
        mid as f32 <= full as f32 * 0.45 + 20.0,
        "battleship: mid {mid} of {full}"
    );
    assert!(coarse < 60, "battleship: coarse {coarse}");
    let top = model.lods[0]
        .vertices
        .iter()
        .map(|v| v.pos[2])
        .fold(f32::MIN, f32::max);
    assert!(
        (0.8 * 38.0..=1.25 * 38.0).contains(&top),
        "battleship: top {top}"
    );
    let reach = model.lods[0]
        .vertices
        .iter()
        .map(|v| (v.pos[0].powi(2) + v.pos[1].powi(2)).sqrt())
        .fold(f32::MIN, f32::max);
    assert!(
        (0.75 * 72.0..=1.3 * 72.0).contains(&reach),
        "battleship: reach {reach}"
    );
    println!("battleship triangles {full}/{mid}/{coarse}, top {top:.1}, reach {reach:.1}");
}

/// The Moray and the Kraken: chined, angular submarines whose bow tube doors carry
/// weapon 0's muzzles at every LOD, the Moray's deck gun on a house of its own, and
/// both lit blue (tech 2 and 3) with nothing orange on them.
#[test]
fn moray_and_kraken_hulls() {
    for bp in BLUEPRINTS
        .iter()
        .filter(|bp| bp.mesh == "submarine_hunter" || bp.mesh == "submarine_strategic")
    {
        let key = bp.mesh;
        let model = built(bp);
        let floor = if key == "submarine_strategic" {
            -12.0
        } else {
            -4.5
        };
        for (lod, mesh) in model.lods.iter().enumerate() {
            let name = format!("{key} lod{lod}");
            assert!(
                mesh.vertices
                    .iter()
                    .all(|v| v.part != part::TURRET && v.part != part::LOCOMOTION),
                "{name}: turret or running gear"
            );
            for muzzle in bp.muzzles {
                let p = Vec3::from(*muzzle);
                let nearest = mesh
                    .indices
                    .chunks(3)
                    .filter(|t| mesh.vertices[t[0] as usize].part == part::HULL)
                    .map(|t| {
                        closest_point_on_triangle(
                            p,
                            position(mesh, t[0]),
                            position(mesh, t[1]),
                            position(mesh, t[2]),
                        )
                        .distance(p)
                    })
                    .fold(f32::MAX, f32::min);
                assert!(
                    nearest < 0.4,
                    "{name}: tube door {nearest} m from muzzle {muzzle:?}"
                );
            }
            assert!(
                mesh.vertices
                    .iter()
                    .any(|v| v.material == material::TEAM && v.normal[2] > 0.5),
                "{name}: no team colour from above"
            );
            assert!(
                mesh.vertices
                    .iter()
                    .any(|v| v.material == material::PLATING),
                "{name}: no plating"
            );
            let top = mesh
                .vertices
                .iter()
                .map(|v| v.pos[2])
                .fold(f32::MIN, f32::max);
            assert!(
                (0.8 * bp.height..=1.25 * bp.height).contains(&top),
                "{name}: top {top} for height {}",
                bp.height
            );
            let reach = mesh
                .vertices
                .iter()
                .map(|v| v.pos[0].hypot(v.pos[1]))
                .fold(f32::MIN, f32::max);
            assert!(
                (0.75 * bp.radius..=1.3 * bp.radius).contains(&reach),
                "{name}: reach {reach} for radius {}",
                bp.radius
            );
            let keel = mesh
                .vertices
                .iter()
                .map(|v| v.pos[2])
                .fold(f32::MAX, f32::min);
            assert!(keel >= floor && keel < -0.4, "{name}: keel at {keel}");
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
                for &i in t {
                    let v = mesh.vertices[i as usize];
                    assert!(
                        (Vec3::from(v.normal).length() - 1.0).abs() < 1e-4,
                        "{name}: unit normal"
                    );
                    assert!(
                        geometric.normalize().dot(Vec3::from(v.normal)) > 0.5,
                        "{name}: winding disagrees with normal at {a}"
                    );
                }
            }
        }
        let [full, mid, coarse] = [0, 1, 2].map(|lod| triangles(&model.lods[lod]));
        assert!(
            (250..=WARSHIP_TRIANGLES).contains(&full),
            "{key}: {full} triangles"
        );
        assert!(
            mid as f32 <= full as f32 * 0.45 + 20.0,
            "{key}: mid {mid} of {full}"
        );
        assert!(coarse < 60, "{key}: coarse {coarse}");
        let count = |m: u32| {
            model.lods[0]
                .vertices
                .iter()
                .filter(|v| v.material == m)
                .count()
        };
        assert!(count(material::GLOW) > 0, "{key}: no blue emitters");
        assert_eq!(count(material::GLOW_ORANGE), 0, "{key}: orange emitters");
        for lod in 0..LOD_COUNT {
            let builder = build_lod(key, lod, bp.tech);
            let mesh = builder.mesh();
            for range in builder.solids() {
                let volume: f32 = mesh.indices[range.clone()]
                    .chunks(3)
                    .map(|t| {
                        position(mesh, t[0]).dot(position(mesh, t[1]).cross(position(mesh, t[2])))
                    })
                    .sum();
                assert!(
                    volume > 0.0,
                    "{key} lod{lod}: inside-out solid at {}",
                    position(mesh, mesh.indices[range.start])
                );
            }
        }
        let houses = model.houses.clone();
        if key == "submarine_hunter" {
            assert_eq!(houses.len(), 1, "moray: one gun house");
            assert_eq!(houses[0].weapon, 1, "moray: the deck gun is weapon 1");
            assert!(
                Vec3::from(houses[0].pivot).distance(Vec3::new(5.5, 0.0, 2.2)) < 1e-3,
                "moray: house pivot {:?}",
                houses[0].pivot
            );
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
        let house = model
            .houses
            .iter()
            .find(|h| h.weapon == weapon)
            .expect("house per weapon");
        assert!(
            Vec3::from(house.pivot).distance(Vec3::from(pivot)) < 1e-3,
            "carrier: house {weapon} pivot {:?}",
            house.pivot
        );
    }
    assert!(
        model.lods[0]
            .vertices
            .iter()
            .any(|v| v.rig & rig::SPIN != 0),
        "carrier: rotary gun spins"
    );
    assert!(
        model.lods[0]
            .vertices
            .iter()
            .any(|v| v.part == part::SPINNER),
        "carrier: radar turns"
    );
    assert!(model.spinner_pivot[2] > 3.0, "carrier: radar on the mast");
    let muzzles: [(&str, &[[f32; 3]]); 3] = [
        (
            "sam",
            &[
                [-12.0, 8.0, 8.5],
                [-14.0, 8.0, 8.5],
                [-12.0, -8.0, 8.5],
                [-14.0, -8.0, 8.5],
            ],
        ),
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
                    .map(|t| {
                        closest_point_on_triangle(
                            p,
                            position(mesh, t[0]),
                            position(mesh, t[1]),
                            position(mesh, t[2]),
                        )
                        .distance(p)
                    })
                    .fold(f32::MAX, f32::min);
                assert!(
                    nearest < 0.4,
                    "carrier lod{lod}: {name} barrel ends {nearest} m from {muzzle:?}"
                );
            }
        }
        let team_up = mesh
            .vertices
            .iter()
            .any(|v| v.material == material::TEAM && v.normal[2] > 0.5);
        assert!(team_up, "carrier lod{lod}: no team colour seen from above");
        assert!(
            mesh.vertices
                .iter()
                .any(|v| v.material == material::PLATING),
            "carrier lod{lod}: no plating"
        );
        assert!(
            mesh.vertices.iter().all(|v| v.part != part::LOCOMOTION),
            "carrier lod{lod}: running gear"
        );
        let floor = mesh
            .vertices
            .iter()
            .map(|v| v.pos[2])
            .fold(f32::MAX, f32::min);
        assert!(
            (-12.0..-0.4).contains(&floor),
            "carrier lod{lod}: keel at {floor}"
        );
    }
    assert!(
        model.lods[0]
            .vertices
            .iter()
            .any(|v| v.material == material::GLOW),
        "carrier: plasma glow"
    );
    let [full, mid, coarse] = [0, 1, 2].map(|lod| triangles(&model.lods[lod]));
    assert!((250..=6000).contains(&full), "carrier: {full} triangles");
    assert!(
        mid as f32 <= full as f32 * 0.45 + 20.0,
        "carrier: mid {mid} of {full}"
    );
    assert!(coarse < 60, "carrier: coarse {coarse}");
    let top = model.lods[0]
        .vertices
        .iter()
        .map(|v| v.pos[2])
        .fold(f32::MIN, f32::max);
    assert!(
        (0.8 * 24.0..=1.25 * 24.0).contains(&top),
        "carrier: top {top}"
    );
    let reach = model.lods[0]
        .vertices
        .iter()
        .map(|v| v.pos[0].hypot(v.pos[1]))
        .fold(f32::MIN, f32::max);
    assert!(
        (0.75 * 60.0..=1.3 * 60.0).contains(&reach),
        "carrier: reach {reach}"
    );
    println!("carrier triangles {full}/{mid}/{coarse}, top {top:.1}, reach {reach:.1}");
}

/// The Marlin and the Manta: every gun house at its data pivot (both drawn 1.2 times
/// the size they are authored at, so the points below are scaled), every weapon's
/// muzzles reached at every LOD they are drawn at (fixed launchers and the light
/// mounts drop out of the coarse level), lit blue with nothing orange, team colour
/// seen from above at every level, and inside the budgets and the fit.
#[test]
fn marlin_and_manta_hulls() {
    type Weapons = &'static [(&'static str, &'static [[f32; 3]], bool)];
    type Houses = &'static [(u8, [f32; 3])];
    let ships: [(&str, f32, f32, Houses, Weapons); 2] = [
        (
            "destroyer",
            22.0,
            12.0,
            &[(0, [12.0, 0.0, 5.2]), (2, [-9.0, 0.0, 9.0])],
            &[
                ("bolt rifle", &[[19.4, 0.0, 5.2]], true),
                (
                    "tubes",
                    &[
                        [16.0, -0.8, -1.2],
                        [16.0, 0.8, -1.2],
                        [16.0, -0.8, -2.0],
                        [16.0, 0.8, -2.0],
                    ],
                    false,
                ),
                ("pd", &[[-7.0, -0.3, 9.2], [-7.0, 0.3, 9.2]], false),
                (
                    "interceptors",
                    &[[-14.0, -1.4, -1.0], [-14.0, 1.4, -1.0]],
                    false,
                ),
            ],
        ),
        (
            "aa_cruiser",
            22.0,
            14.0,
            &[(1, [14.5, 0.0, 5.4])],
            &[
                (
                    "cells",
                    &[
                        [7.25, -2.25, 6.4],
                        [7.25, -0.75, 6.4],
                        [7.25, 0.75, 6.4],
                        [7.25, 2.25, 6.4],
                        [5.75, -2.25, 6.4],
                        [5.75, -0.75, 6.4],
                        [5.75, 0.75, 6.4],
                        [5.75, 2.25, 6.4],
                        [4.25, -2.25, 6.4],
                        [4.25, -0.75, 6.4],
                        [4.25, 0.75, 6.4],
                        [4.25, 2.25, 6.4],
                        [2.75, -2.25, 6.4],
                        [2.75, -0.75, 6.4],
                        [2.75, 0.75, 6.4],
                        [2.75, 2.25, 6.4],
                    ],
                    true,
                ),
                ("deck gun", &[[19.5, 0.0, 5.4]], false),
            ],
        ),
    ];
    for (key, radius, height, houses, weapons) in ships {
        let bp = BLUEPRINTS.iter().find(|bp| bp.mesh == key).unwrap();
        let model = built(bp);
        let scale = bp.radius / radius;
        assert!(
            (bp.height / height - scale).abs() < 1e-3,
            "{key}: drawn out of proportion"
        );
        let (radius, height) = (bp.radius, bp.height);
        assert_eq!(model.houses.len(), houses.len(), "{key}: gun houses");
        for (weapon, pivot) in houses {
            let house = model
                .houses
                .iter()
                .find(|h| h.weapon == *weapon)
                .expect("house per weapon");
            assert!(
                Vec3::from(house.pivot).distance(Vec3::from(*pivot) * scale) < 1e-3,
                "{key}: house {weapon} pivot {:?}",
                house.pivot
            );
        }
        for (lod, mesh) in model.lods.iter().enumerate() {
            for (name, list, at_coarse) in weapons {
                if lod == 2 && !at_coarse {
                    continue;
                }
                for muzzle in *list {
                    let p = Vec3::from(*muzzle) * scale;
                    let nearest = mesh
                        .indices
                        .chunks(3)
                        .filter(|t| mesh.vertices[t[0] as usize].part == part::HULL)
                        .map(|t| {
                            closest_point_on_triangle(
                                p,
                                position(mesh, t[0]),
                                position(mesh, t[1]),
                                position(mesh, t[2]),
                            )
                            .distance(p)
                        })
                        .fold(f32::MAX, f32::min);
                    assert!(
                        nearest < 0.4,
                        "{key} lod{lod}: {name} barrel ends {nearest} m from {muzzle:?}"
                    );
                }
            }
            let team_up = mesh
                .vertices
                .iter()
                .any(|v| v.material == material::TEAM && v.normal[2] > 0.5);
            assert!(team_up, "{key} lod{lod}: no team colour seen from above");
            assert!(
                mesh.vertices
                    .iter()
                    .any(|v| v.material == material::PLATING),
                "{key} lod{lod}: no plating"
            );
            let floor = mesh
                .vertices
                .iter()
                .map(|v| v.pos[2])
                .fold(f32::MAX, f32::min);
            assert!(floor >= -4.5 * scale, "{key} lod{lod}: keel at {floor}");
            assert!(
                !mesh.vertices.iter().any(|v| v.part == part::TURRET),
                "{key} lod{lod}: turret part"
            );
        }
        let full_mesh = &model.lods[0];
        assert!(
            !full_mesh
                .vertices
                .iter()
                .any(|v| v.material == material::GLOW_ORANGE),
            "{key}: orange"
        );
        assert!(
            full_mesh
                .vertices
                .iter()
                .any(|v| v.material == material::GLOW),
            "{key}: nothing lit blue"
        );
        assert!(
            full_mesh.vertices.iter().any(|v| v.part == part::SPINNER),
            "{key}: no radar spinner"
        );
        let [full, mid, coarse] = [0, 1, 2].map(|lod| triangles(&model.lods[lod]));
        assert!(
            (250..=WARSHIP_TRIANGLES).contains(&full),
            "{key}: {full} triangles"
        );
        assert!(
            mid as f32 <= full as f32 * 0.45 + 20.0,
            "{key}: mid {mid} of {full}"
        );
        assert!(coarse < 60, "{key}: coarse {coarse}");
        let top = full_mesh
            .vertices
            .iter()
            .map(|v| v.pos[2])
            .fold(f32::MIN, f32::max);
        assert!(
            (0.8 * height..=1.25 * height).contains(&top),
            "{key}: top {top}"
        );
        let reach = full_mesh
            .vertices
            .iter()
            .map(|v| (v.pos[0].powi(2) + v.pos[1].powi(2)).sqrt())
            .fold(f32::MIN, f32::max);
        assert!(
            (0.75 * radius..=1.3 * radius).contains(&reach),
            "{key}: reach {reach}"
        );
        println!("{key} triangles {full}/{mid}/{coarse}, top {top:.1}, reach {reach:.1}");
    }
}

/// Nearest distance from `p` to the triangles of `mesh` whose first vertex passes `keep`.
fn nearest_where(mesh: &MeshLod, p: Vec3, keep: impl Fn(&super::MeshVertex) -> bool) -> f32 {
    mesh.indices
        .chunks(3)
        .filter(|t| keep(&mesh.vertices[t[0] as usize]))
        .map(|t| {
            closest_point_on_triangle(
                p,
                position(mesh, t[0]),
                position(mesh, t[1]),
                position(mesh, t[2]),
            )
            .distance(p)
        })
        .fold(f32::MAX, f32::min)
}

/// The Paladin's shin torpedo tubes: a mouth at each of the unit file's muzzles with the
/// legs at rest, on the shin bone so the pod strides with the leg, and unlit orange.
#[test]
fn paladin_shin_tubes_reach_their_muzzles() {
    let bp = BLUEPRINTS
        .iter()
        .find(|bp| bp.mesh == "assault_bot")
        .unwrap();
    let model = built(bp);
    // The coarse level's legs are one block each: no pods.
    for (lod, mesh) in model.lods.iter().enumerate().take(2) {
        for muzzle in [[2.2, -4.6, 3.0], [2.2, 4.6, 3.0]] {
            let nearest = nearest_where(mesh, Vec3::from(muzzle), |v| {
                v.part == part::LOCOMOTION && (v.rig & rig::LIMB_MASK) == rig::SHIN
            });
            assert!(
                nearest < 0.4,
                "paladin lod{lod}: shin tube mouth {nearest} m from {muzzle:?}"
            );
        }
    }
}

/// The Fulgur: a main turret on the origin, three gun houses bound to weapons 1, 2 and 3
/// at the unit file's pivots with a barrel at each muzzle, the houses standing on the hull,
/// and the flak house low enough for the main barrel to pass over it.
#[test]
fn fulgur_houses_and_muzzles() {
    let bp = BLUEPRINTS
        .iter()
        .find(|bp| bp.mesh == "assault_tank")
        .unwrap();
    let model = built(bp);
    let want: [(u8, [f32; 3], [f32; 3]); 3] = [
        (1, [14.85, 17.325, 11.88], [28.875, 17.325, 14.025]),
        (2, [14.85, -17.325, 11.88], [28.875, -17.325, 14.025]),
        (3, [-23.925, 0.0, 11.7975], [-15.7575, 0.0, 11.7975]),
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
            let nearest = nearest_where(mesh, Vec3::from(muzzle), |v| {
                of_house(v) && v.rig & rig::RECOIL != 0
            });
            assert!(
                nearest < 0.4,
                "fulgur lod{lod}: house {weapon} barrel ends {nearest} m from {muzzle:?}"
            );
            // It stands on the hull: its foot is at or under the pivot, not hanging above the deck.
            let foot = mesh
                .vertices
                .iter()
                .filter(|v| of_house(v))
                .map(|v| v.pos[2])
                .fold(f32::MAX, f32::min);
            assert!(
                foot <= pivot[2],
                "fulgur lod{lod}: house {weapon} floats, foot at {foot}"
            );
            assert!(
                mesh.vertices
                    .iter()
                    .filter(|v| of_house(v))
                    .all(|v| v.part == part::HULL),
                "fulgur lod{lod}: house {weapon} rides the hull, not the turret"
            );
        }
    }
    let mesh = &model.lods[0];
    // The AA gun rests raised to the sky; the house under it must clear the main barrel.
    let aa_slot = model.houses.iter().position(|h| h.weapon == 3).unwrap() as u32;
    let aa_top = mesh
        .vertices
        .iter()
        .filter(|v| {
            (v.rig & rig::LIMB_MASK) == rig::HOUSE_FIRST + aa_slot && v.rig & rig::RECOIL == 0
        })
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
        aa_top < barrel_bottom,
        "fulgur: the main barrel (bottom {barrel_bottom}) must pass over the AA house (top {aa_top})"
    );
    let [full, mid, coarse] = [0, 1, 2].map(|lod| triangles(&model.lods[lod]));
    println!("fulgur triangles {full}/{mid}/{coarse}, AA house top {aa_top:.2}, barrel bottom {barrel_bottom:.2}");
}

/// The Arbalest's bore recoils into its housing.
#[test]
fn arbalest_bore_recoils() {
    let model = build_model("bore_tank").unwrap();
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
            assert!(
                mesh.vertices
                    .iter()
                    .filter(|v| v.part == part::TURRET && v.rig & rig::RECOIL != 0)
                    .all(|v| v.rig & rig::LIMB_MASK == rig::ARM_GUN),
                "{key}: barrel cannot elevate"
            );
        }
        let [full, mid, coarse] = [0, 1, 2].map(|lod| triangles(&model.lods[lod]));
        let budget = if key == "assault_tank" {
            ASSAULT_TANK_TRIANGLES
        } else {
            2600
        };
        assert!(
            full <= budget && coarse < 60 && mid as f32 <= full as f32 * 0.45 + 20.0,
            "{key}: {full}/{mid}/{coarse}"
        );
        for (lod, mesh) in model.lods.iter().enumerate() {
            assert!(mesh.vertices.iter().all(|v| Vec3::from(v.pos).is_finite()));
            let muzzle = Vec3::from(bp.muzzles[0]);
            assert!(
                nearest_where(mesh, muzzle, |v| v.part == part::TURRET
                    && v.rig & rig::RECOIL != 0)
                    < 0.5,
                "{key} lod{lod}: emitter misses the weapon origin"
            );
        }
        println!("{key}: {full}/{mid}/{coarse} triangles");
    }
}

/// The unit file throws the Tempest's casings from the model's port: `sabot.port` is
/// `titan::EJECT` at the size the Behemoth is built (its gun's muzzle over the model's).
#[test]
fn titan_casings_leave_by_the_models_port() {
    use super::aster::titan;
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let bps = mc_data::Blueprints::load(&data).unwrap();
    let bp = bps.unit(bps.id_of("aster_t5_titan").unwrap());
    let gun = bp.weapons.iter().find(|w| w.sabot.is_some()).unwrap();
    let scale = gun.muzzle.x.to_f32() / titan::GATLING_MUZZLE_AT.x;
    let port = gun.sabot.unwrap().port.to_f32();
    let want = titan::EJECT_AT * scale;
    assert!(
        Vec3::from(port).distance(want) < 0.5,
        "the unit file's port {port:?} is not the model's {want:?}"
    );
}

/// The Behemoth: every weapon's muzzle is reached by its own barrel at every level of
/// detail it is drawn at, its arms are houses on the torso axis that turn with the torso,
/// the gatling's cluster spins, it walks with a head and a shield projector, and its
/// triangles stay in budget.
#[test]
fn titan_houses_muzzles_and_rig() {
    use super::aster::titan;
    let model = build_model("titan").expect("titan builds");
    let slot_of = |weapon: u8, pivot: Vec3| {
        model
            .houses
            .iter()
            .position(|h| h.weapon == weapon && Vec3::from(h.pivot).distance(pivot) < 0.003)
            .unwrap_or_else(|| panic!("titan: a house for weapon {weapon} at {pivot}"))
    };
    let limb = |slot: usize| rig::HOUSE_FIRST + (slot as u32 & 3);
    let in_house = |slot: usize| {
        move |v: &super::MeshVertex| {
            (v.rig & rig::LIMB_MASK) == limb(slot)
                && ((v.rig & rig::HOUSE_HIGH) != 0) == (slot >= 4)
                && v.part != part::TURRET
        }
    };
    assert!(model.houses.len() <= rig::HOUSE_COUNT as usize);
    let up = |p: Vec3| p + Vec3::Z * titan::RAISE;
    let gatling = slot_of(1, titan::SHOULDER_AT);
    let arm = slot_of(2, titan::SHOULDER_AT);
    let pivot = Vec3::from(model.houses[gatling].pivot);
    assert!(
        pivot.truncate().length() < 1e-4,
        "titan: the arms turn about the torso's axis"
    );
    let spin = model
        .spins
        .first()
        .expect("titan: the rail cluster spins")
        .2;
    assert!(
        (spin[1] - titan::GATLING_MUZZLE_AT.y).abs() < 1e-3
            && (spin[2] - titan::GATLING_MUZZLE_AT.z).abs() < 1e-3
            && (titan::BORE_MUZZLE_AT.y + spin[1]).abs() < 1e-3
            && (titan::BORE_MUZZLE_AT.z - spin[2]).abs() < 1e-3,
        "titan: the cluster turns about the gatling's bore, the AEB's is its mirror"
    );
    for (lod, mesh) in model.lods.iter().enumerate() {
        assert!(mesh
            .vertices
            .iter()
            .all(|v| Vec3::from(v.pos).is_finite() && Vec3::from(v.normal).is_finite()));
        // The arm guns at every level, each in its house.
        let near = nearest_where(mesh, titan::GATLING_MUZZLE_AT, in_house(gatling));
        assert!(
            near < 0.4,
            "titan lod{lod}: gatling ends {near} m from its muzzle"
        );
        let bore = if lod < 2 {
            slot_of(2, titan::SHOULDER_AT + Vec3::Z * 0.01)
        } else {
            arm
        };
        let near = nearest_where(mesh, titan::BORE_MUZZLE_AT, in_house(bore));
        assert!(
            near < 0.4,
            "titan lod{lod}: bore ends {near} m from its muzzle"
        );
        assert!(
            mesh.vertices
                .iter()
                .filter(|v| in_house(gatling)(v) || in_house(arm)(v))
                .all(|v| v.rig & rig::RECOIL != 0),
            "titan lod{lod}: the whole arm pitches"
        );
        // The rocket pods ride the torso.
        for (x, y, z) in titan::POD_MOUTHS {
            for side in [1.0, -1.0] {
                let mouth = up(Vec3::new(x, y * side, z));
                let near = nearest_where(mesh, mouth, |v| v.part == part::TURRET);
                assert!(near < 0.4, "titan lod{lod}: pod mouth {mouth} {near} m off");
            }
        }
        // The flak turrets: drawn at the two finer levels, gun houses riding the torso.
        if lod < 2 {
            for (i, &p) in titan::FLAK.iter().enumerate() {
                let slot = slot_of(3 + i as u8, up(p));
                let on_torso = |v: &super::MeshVertex| {
                    (v.rig & rig::LIMB_MASK) == limb(slot)
                        && ((v.rig & rig::HOUSE_HIGH) != 0) == (slot >= 4)
                        && v.part == part::TURRET
                };
                for s in [-1.0f32, 1.0] {
                    let muzzle = up(p) + Vec3::new(titan::FLAK_REACH, s * titan::FLAK_GAP, 0.0);
                    let near = nearest_where(mesh, muzzle, on_torso);
                    assert!(
                        near < 0.5,
                        "titan lod{lod}: flak {i} barrel ends {near} m from its muzzle"
                    );
                }
            }
        }
        // Legs walk on their bones.
        if lod < 2 {
            for bone in [rig::THIGH, rig::SHIN, rig::FOOT] {
                assert!(
                    mesh.vertices
                        .iter()
                        .any(|v| v.part == part::LOCOMOTION && (v.rig & rig::LIMB_MASK) == bone),
                    "titan lod{lod}: no bone {bone}"
                );
            }
            assert!(
                mesh.vertices.iter().any(|v| v.rig & rig::SPIN != 0),
                "titan lod{lod}: nothing spins"
            );
        }
    }
    let legs = model.legs.expect("titan walks");
    assert_eq!(legs.stride, titan::STRIDE);
    assert!((legs.hip[2] - titan::HIP.z).abs() < 1e-3);
    assert!(model.neck.is_some(), "titan: a head that looks about");
    assert!(model.shield_emitter.is_some(), "titan: a shield projector");
    let [full, mid, coarse] = [0, 1, 2].map(|lod| triangles(&model.lods[lod]));
    assert!(
        full <= 30000 && coarse < 60 && mid as f32 <= full as f32 * 0.45 + 20.0,
        "titan: {full}/{mid}/{coarse}"
    );
    println!(
        "titan: {full}/{mid}/{coarse} triangles, {} houses",
        model.houses.len()
    );
    let sabot = build_model("titan_sabot").expect("sabot builds");
    // The same checks as `meshes_are_valid`, for these two alone.
    for m in [&model, &sabot] {
        for (lod, mesh) in m.lods.iter().enumerate() {
            for v in &mesh.vertices {
                assert!(
                    (Vec3::from(v.normal).length() - 1.0).abs() < 1e-4,
                    "{} lod{lod}: unit normal",
                    m.key
                );
                assert!(v.pos[2] >= -1e-3, "{} lod{lod}: below ground", m.key);
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
                    "{} lod{lod}: degenerate triangle at {a}",
                    m.key
                );
                for &i in t {
                    let shading = Vec3::from(mesh.vertices[i as usize].normal);
                    assert!(
                        geometric.normalize().dot(shading) > 0.5,
                        "{} lod{lod}: winding at {a}",
                        m.key
                    );
                }
            }
        }
    }
    let [full, mid, coarse] = [0, 1, 2].map(|lod| triangles(&sabot.lods[lod]));
    assert!(
        full <= 600 && coarse < 60 && mid <= full,
        "titan sabot: {full}/{mid}/{coarse}"
    );
    assert!(sabot
        .lods
        .iter()
        .all(|m| m.vertices.iter().all(|v| Vec3::from(v.pos).is_finite())));
    println!("titan sabot: {full}/{mid}/{coarse} triangles");
}

/// The reclaim boat's head pitches from straight down (the seabed) to steeply up (a
/// cliff-top shore) and turns all the way round, so nothing that does not pitch with it
/// may stand in its sweep: the hull and tower anywhere it could turn to, and its own
/// yoke across the head's width (bar the trunnion axle itself).
#[test]
fn reclaim_boat_head_sweep_is_clear() {
    use super::aster::{
        RECLAIM_BOAT_PIVOT as PIVOT, RECLAIM_BOAT_REACH as REACH, RECLAIM_BOAT_SWEEP as SWEEP,
    };
    let key = "reclaim_boat";
    let model = build_model(key).unwrap();
    assert_eq!(model.houses.len(), 1, "{key}: one head");
    assert_eq!(model.houses[0].weapon, 0, "{key}: head on weapon 0");
    assert!(
        (Vec3::from(model.houses[0].pivot) - PIVOT).length() < 1e-3,
        "{key}: head pivot"
    );
    let mesh = &model.lods[0];
    let house = |v: &super::MeshVertex| v.rig & rig::LIMB_MASK == rig::HOUSE_FIRST;
    let pitches = |v: &super::MeshVertex| house(v) && v.rig & rig::RECOIL != 0;
    let radius = |q: Vec3| (q.x - PIVOT.x).hypot(q.z - PIVOT.z);
    let sweep = mesh
        .vertices
        .iter()
        .filter(|v| pitches(v))
        .map(|v| radius(Vec3::from(v.pos)))
        .fold(0.0, f32::max);
    assert!(
        sweep <= SWEEP,
        "{key}: head reaches {sweep} from its trunnion"
    );
    assert!(sweep >= REACH, "{key}: head short of its mouth ({sweep})");
    for v in mesh.vertices.iter().filter(|v| !pitches(v)) {
        let q = Vec3::from(v.pos);
        if house(v) {
            assert!(
                q.y.abs() > 0.5 || radius(q) < 0.35 || radius(q) > sweep,
                "{key}: yoke at {q:?} in the head's sweep"
            );
        } else {
            assert!(
                q.z < PIVOT.z - sweep || (q.x - PIVOT.x).hypot(q.y) > sweep,
                "{key}: hull at {q:?} in the head's sweep"
            );
        }
    }
    // The emitter the unit file names sits in the lit intake, at the mouth.
    let mouth = PIVOT + Vec3::X * REACH;
    let (lo, hi) = mesh
        .vertices
        .iter()
        .filter(|v| pitches(v) && v.material == material::GLOW_MATERIALS)
        .map(|v| Vec3::from(v.pos))
        .filter(|q| q.x > mouth.x - 0.4)
        .fold((Vec3::MAX, Vec3::MIN), |(lo, hi), q| (lo.min(q), hi.max(q)));
    assert!(
        mouth.cmpge(lo - Vec3::splat(0.12)).all() && mouth.cmple(hi + Vec3::splat(0.12)).all(),
        "{key}: emitter {mouth:?} outside the lit intake {lo:?}..{hi:?}"
    );
}
