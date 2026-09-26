//! The Naga: ultra-mechanical war machines, dark armour over bronze workings, lit red
//! (`data/factions/naga`, docs/STYLE.md "The Naga look").
//!
//! No palette of their own reaches the renderer yet, so they are painted from the
//! materials every faction shares under `pattern::EMBER`, which the entity shader
//! recolours: `PLATING_DARK` for the armour plates (`kit::dark_plate`), `ACCENT` for the
//! darker seams between them, lit red (`kit::seam`), `METAL` for the dark bronze machinery
//! under them (`kit::metal`), `GLOW_LASER` for their optics and the heat in their weapons,
//! and `GLOW_VIOLET` for what builds. The pieces are in `kit` (shapes), `plating` (the
//! walkers' plates and joints) and `machine` (the structures' kit). A model authored at
//! its blueprint's size has its muzzles, pivots and emitters as the unit file's numbers;
//! one built at another size says so.

mod brood;
mod commander;
mod cyst;
mod defense;
mod eye;
mod hatchery;
mod heart;
mod kit;
mod machine;
mod plating;
mod scorpion;
mod taproot;
mod tender;
mod tidebrood;

use super::library::ModelDef;

/// The commander's full-detail triangle budget: one a player, the faction's hero, a walker
/// of many separate plates over its machinery.
#[cfg(test)]
pub(super) const COMMANDER_TRIANGLES: usize = 9000;

pub(super) const MODELS: &[ModelDef] = &[
    // The commander: a reverse-kneed walker, cannon arm and claw (`commander`).
    ModelDef::new("naga_commander", 10.4, 23.0, commander::commander),
    // The tech 3 battle scorpion: the old commander's scorpion, built bigger (`scorpion`).
    ModelDef::new(
        "naga_scorpion",
        scorpion::RADIUS,
        scorpion::HEIGHT,
        scorpion::scorpion,
    ),
    // The engineer: a six-legged walker, its fabricator on a boom over its back (`tender`).
    ModelDef::new("naga_tender", 3.8, 3.8, tender::tender),
    // Factories: the land press works (`brood`), the air launch frame (`hatchery`), the
    // floating dock (`tidebrood`). The land and air factories upgrade in place to tech 3,
    // the land one's lifted ring and the air one's crown standing taller.
    ModelDef::tiered(
        "naga_brood",
        [(46.0, 22.0), (46.0, 22.0), (46.0, 35.0)],
        brood::brood,
    ),
    ModelDef::tiered(
        "naga_hatchery",
        [(46.0, 30.0), (46.0, 30.0), (46.0, 38.0)],
        hatchery::hatchery,
    ),
    ModelDef::new("naga_tidebrood", 46.0, 20.0, tidebrood::tidebrood),
    // Economy: the sealed bore (`taproot`), the star core (`heart`), the vault and cells
    // (`cyst`).
    ModelDef::new("naga_taproot", 12.8, 11.0, taproot::taproot),
    ModelDef::new("naga_heart", 6.9, 7.5, heart::heart),
    ModelDef::new("naga_cyst", 12.9, 8.0, cyst::cyst),
    // Defence (`defense`): point defence, anti-air, wall.
    ModelDef::new("naga_barb", 5.5, 8.0, defense::barb),
    ModelDef::new("naga_spitter", 5.5, 8.5, defense::spitter),
    ModelDef::new("naga_thornwall", 6.0, 5.0, defense::thornwall),
    // Radar (`eye`).
    ModelDef::new("naga_eye", 7.0, 24.0, eye::eye),
];

/// Full-detail triangle budgets: the Naga are built from many separate parts, so each
/// model gets more than the library's default. `None` for a key that is not theirs.
#[cfg(test)]
pub(super) fn triangles(key: &str) -> Option<usize> {
    Some(match key {
        "naga_commander" => COMMANDER_TRIANGLES,
        "naga_scorpion" => 14000,
        // The land and air factories' tech 3, with their tech 2 kit and more.
        "naga_brood" | "naga_hatchery" => 15000,
        "naga_tidebrood" => 9000,
        "naga_taproot" | "naga_cyst" => 5000,
        "naga_heart" | "naga_barb" | "naga_spitter" | "naga_eye" => 4000,
        "naga_tender" => 3000,
        // Walls come by the dozen.
        "naga_thornwall" => 1500,
        _ => return None,
    })
}

/// One model's share of the library's checks (`models/tests.rs`), so a Naga model can be
/// tested on its own while its siblings are still being built: it fits its blueprint and
/// lot, wears team colour and dark plate at every level of detail, keeps to its budget, and its
/// turret reaches each muzzle.
#[cfg(test)]
pub(super) fn check(key: &str, radius: f32, height: f32, cells: Option<u32>, muzzles: &[[f32; 3]]) {
    check_at(key, 1, radius, height, cells, muzzles);
}

/// [`check`] for the model at tech `tech`.
#[cfg(test)]
pub(super) fn check_at(
    key: &str,
    tech: u8,
    radius: f32,
    height: f32,
    cells: Option<u32>,
    muzzles: &[[f32; 3]],
) {
    use super::{material, part, rig};
    let model = super::build_model_scaled(key, radius, height, tech).expect(key);
    let tris = |lod: usize| model.lods[lod].indices.len() / 3;
    let (full, mid, coarse) = (tris(0), tris(1), tris(2));
    let budget = triangles(key).unwrap_or(2600);
    assert!(
        full <= budget && full >= 250,
        "{key}: {full} triangles (budget {budget})"
    );
    assert!(
        mid as f32 <= full as f32 * 0.45 + 20.0 && coarse < 60,
        "{key}: {full}/{mid}/{coarse}"
    );
    for (lod, mesh) in model.lods.iter().enumerate() {
        let name = format!("{key} lod{lod}");
        let top = mesh
            .vertices
            .iter()
            .filter(|v| v.rig & rig::UPGRADE == 0)
            .map(|v| v.pos[2])
            .fold(0.0, f32::max);
        assert!(
            top <= height * 1.25 && top >= height * 0.8,
            "{name}: top {top} for height {height}"
        );
        assert!(
            mesh.vertices.iter().all(|v| v.pos[2] >= -1e-3),
            "{name}: below ground"
        );
        let barrel = muzzles.iter().map(|m| m[0].hypot(m[1])).fold(0.0, f32::max);
        let (x, y) = mesh.vertices.iter().fold((0.0f32, 0.0f32), |(x, y), v| {
            (x.max(v.pos[0].abs()), y.max(v.pos[1].abs()))
        });
        match cells {
            Some(c) => {
                let half = mc_map::BUILD_CELL_M as f32 * 0.5 * c as f32;
                assert!(
                    x <= half.max(barrel + 0.5) && y <= half,
                    "{name}: extent {x} x {y} outside lot {half}"
                );
                assert!(
                    x >= half * 0.55 && y >= half * 0.55,
                    "{name}: extent {x} x {y} too small for lot {half}"
                );
                // The Naga lot is an octagon: the square with its corners cut where
                // |x| + |y| passes 1.45 half (`ground.wgsl`). Nothing stands in the cut
                // corners; only a turning gun may swing over them.
                let corner = mesh
                    .vertices
                    .iter()
                    .filter(|v| v.part != part::TURRET)
                    .map(|v| v.pos[0].abs() + v.pos[1].abs())
                    .fold(0.0, f32::max);
                assert!(
                    corner <= half * 1.45,
                    "{name}: reaches {corner} into the lot's cut corners ({})",
                    half * 1.45
                );
            }
            None => {
                let reach = mesh
                    .vertices
                    .iter()
                    .map(|v| v.pos[0].hypot(v.pos[1]))
                    .fold(0.0, f32::max);
                assert!(
                    reach <= (radius * 1.3).max(barrel + 0.5) && reach >= radius * 0.75,
                    "{name}: reach {reach} for radius {radius}"
                );
            }
        }
        assert!(
            mesh.vertices
                .iter()
                .any(|v| v.material == material::TEAM && v.normal[2] > 0.5),
            "{name}: no upward team colour"
        );
        assert!(
            mesh.vertices
                .iter()
                .any(|v| v.material == material::PLATING_DARK),
            "{name}: no dark plate"
        );
        assert!(
            !mesh
                .vertices
                .iter()
                .any(|v| v.material == material::GLOW || v.material == material::GLOW_ORANGE),
            "{name}: ARC's blue or orange light"
        );
        for m in muzzles {
            let m = glam::Vec3::from(*m);
            let near = mesh
                .vertices
                .iter()
                .filter(|v| v.part == part::TURRET)
                .map(|v| glam::Vec3::from(v.pos).distance(m))
                .fold(f32::MAX, f32::min);
            assert!(near < 0.4, "{name}: turret {near} m from muzzle {m}");
            let past = mesh
                .vertices
                .iter()
                .filter(|v| v.part == part::TURRET)
                .map(|v| v.pos[0])
                .fold(f32::MIN, f32::max);
            assert!(
                past <= m.x + 0.5,
                "{name}: turret reaches {past}, past the muzzle {m}"
            );
        }
        if muzzles.is_empty() && cells.is_some() {
            assert!(
                !mesh.vertices.iter().any(|v| v.part == part::TURRET),
                "{name}: unarmed with a turret"
            );
        }
    }
}

/// A factory's violet fabricator tips are where the sim pours its nanite streams from
/// (`mc_sim::print_heads`), and it has one at every head.
#[cfg(test)]
pub(super) fn check_heads(key: &str, radius: f32, height: f32) {
    check_heads_at(key, 1, radius, height);
}

/// [`check_heads`] for the model at tech `tech`, with the heads that tech has fitted.
#[cfg(test)]
pub(super) fn check_heads_at(key: &str, tech: u8, radius: f32, height: f32) {
    use super::material;
    let factory = mc_sim::print_heads::factory_heads(key).expect(key);
    assert!(
        factory.heads.iter().any(|h| h.fitted(tech)),
        "{key}: no heads at tech {tech}"
    );
    let model = super::build_model_scaled(key, radius, height, tech).expect(key);
    for head in factory.heads.iter().filter(|h| h.fitted(tech)) {
        let tip = glam::Vec3::from(mc_sim::print_heads::nozzle(head, factory.aim));
        let near = model.lods[0]
            .vertices
            .iter()
            .filter(|v| v.material == material::GLOW_VIOLET)
            .map(|v| glam::Vec3::from(v.pos).distance(tip))
            .fold(f32::MAX, f32::min);
        assert!(
            near < 0.2,
            "{key} T{tech}: no violet within {near} m of the head at {:?}",
            head.mount
        );
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_entity_shader_reads_the_nanite_bit_the_mirror_sets() {
        // Until the shaders get generated constants, the one number is pinned here.
        let src = include_str!("../../../shaders/entity.wgsl");
        let line = format!("const UNIT_NANITE: u32 = {}u;", mc_sim::mirror::UNIT_NANITE);
        assert!(src.contains(&line), "entity.wgsl should say {line}");
        let src = include_str!("../../../shaders/ground.wgsl");
        let line = format!("const PAD_NANITE: u32 = {}u;", mc_sim::PAD_NANITE);
        assert!(src.contains(&line), "ground.wgsl should say {line}");
    }
}
