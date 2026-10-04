//! The Regency: ultra-mechanical war machines, dark armour over bronze workings, lit red
//! (`data/factions/regency`, docs/STYLE.md "The Regency look").
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

pub(crate) mod air;
pub(crate) mod air_t2;
mod battle_tank;
mod bombard;
mod brood;
mod chassis;
mod commander;
mod condenser;
mod crucible;
mod cyst;
mod engineer;
mod eye;
mod fusion_guns;
mod guns;
pub(crate) mod gunships;
mod hatchery;
mod heart;
mod hover_tank;
mod kit;
mod lift;
mod machine;
mod mattock;
mod mobile_aa;
pub(crate) mod naval;
mod palisade;
mod plating;
mod raider;
mod reclaimer;
mod scorpion;
mod scout;
mod seeker_hover;
mod sonar;
pub(crate) mod space;
mod strategic;
mod strider;
mod taproot;
mod tidebrood;
mod torpedo;
mod turrets;
mod wake_tank;
mod ward;

use super::library::ModelDef;

/// The commander's full-detail triangle budget: one a player, the faction's hero, a walker
/// of many separate plates over its machinery.
#[cfg(test)]
pub(super) const COMMANDER_TRIANGLES: usize = 9000;

pub(super) const MODELS: &[ModelDef] = &[
    // The commander: a tall walker, fusion cannon forearm and taloned hand (`commander`).
    ModelDef::new("regency_commander", 10.4, 25.0, commander::commander),
    // The tech 4 battle scorpion: the old commander's scorpion, built bigger (`scorpion`).
    ModelDef::new(
        "regency_scorpion",
        scorpion::RADIUS,
        scorpion::HEIGHT,
        scorpion::scorpion,
    ),
    // The engineer: a craft on gravity lift, its fabricator arm on a turning housing, more
    // kit on it at each tier (`engineer`).
    ModelDef::new("regency_engineer", 3.6, 2.8, engineer::engineer),
    // The tech 2 bombardment walker: two tall legs, a launcher pod hinged on a ring on its
    // back, raised to lob its seekers (`bombard`).
    ModelDef::new(
        "regency_bombard",
        bombard::RADIUS,
        bombard::HEIGHT,
        bombard::bombard,
    ),
    // The land scout: a six-legged walker, a radar array on a mast over its back
    // (`scout`).
    ModelDef::new("regency_scout", 3.8, 4.0, scout::scout),
    // The tech 1 raider: a twin repeater on a four-legged walker (`raider`).
    ModelDef::new("regency_raider", 2.6, 4.0, raider::raider),
    // The tech 1 tank: a Plasmeric Repeater on a hull on gravity lift (`hover_tank`).
    ModelDef::new("regency_hover_tank", 4.6, 3.4, hover_tank::hover_tank),
    // The tech 1 mobile anti-air: a AA organ on a six-legged walker (`mobile_aa`).
    ModelDef::new("regency_mobile_aa", 4.0, 4.0, mobile_aa::mobile_aa),
    // The tech 2 battle tank: a lift hull, its turret's gun a Pinched-plasmeric Cannon
    // (`battle_tank`).
    ModelDef::new("regency_battle_tank", 6.2, 4.4, battle_tank::battle_tank),
    // The tech 2 mobile anti-air: a craft on lift, a Gravitic Seeker Battery on its back
    // (`seeker_hover`).
    ModelDef::new("regency_seeker_hover", 5.0, 5.6, seeker_hover::seeker_hover),
    // The tech 3 mobile fusion guns (`fusion_guns`): the anti-spaceship gun raised to the sky, and the
    // howitzer.
    ModelDef::new(
        "regency_skyspear",
        fusion_guns::SKYSPEAR_RADIUS,
        fusion_guns::SKYSPEAR_HEIGHT,
        fusion_guns::skyspear,
    ),
    ModelDef::new(
        "regency_fusion_howitzer",
        fusion_guns::HOWITZER_RADIUS,
        fusion_guns::HOWITZER_HEIGHT,
        fusion_guns::howitzer,
    ),
    // The Reclaimer, every tier: a hover hull with a nanite head on a turning house over its
    // stern, drawn bigger at tech 2 and 3 (`reclaimer`).
    ModelDef::new("regency_reclaimer", 4.6, 3.4, reclaimer::reclaimer),
    // The tech 1 artillery: a four-legged walker, a Plasmeric Mortar in the house on its
    // back (`mattock`).
    ModelDef::new("regency_mattock", 4.2, 3.2, mattock::mattock),
    // The tech 3 wake tank: a hover hull, a wide flat projector in front of its turret
    // (`wake_tank`).
    ModelDef::new("regency_wake_tank", 7.6, 4.6, wake_tank::wake_tank),
    // The tech 3 assault tripod: a keeled head high on three legs (`strider`).
    ModelDef::new("regency_strider", 12.0, 32.0, strider::strider),
    // Factories: the land press works (`brood`), the air launch frame (`hatchery`), the
    // one-sided quay (`tidebrood`). All three upgrade in place to tech 3, the land one's
    // lifted ring, the air one's crown and the quay's high boom standing taller.
    ModelDef::tiered(
        "regency_brood",
        [(46.0, 22.0), (46.0, 22.0), (46.0, 35.0)],
        brood::brood,
    ),
    ModelDef::tiered(
        "regency_hatchery",
        [(46.0, 30.0), (46.0, 30.0), (46.0, 38.0)],
        hatchery::hatchery,
    ),
    ModelDef::tiered(
        "regency_tidebrood",
        [(46.0, 20.0), (46.0, 24.0), (46.0, 32.0)],
        tidebrood::tidebrood,
    ),
    // Economy: the sealed bore (`taproot`), the star core (`heart`), the vault and cells
    // (`cyst`).
    ModelDef::tiered(
        "regency_taproot",
        [(12.8, 11.0), (12.8, 15.0), (12.8, 19.0)],
        taproot::taproot,
    )
    .with_tier_4(),
    ModelDef::new("regency_heart", 6.9, 7.5, heart::heart),
    ModelDef::new("regency_heart_2", 18.75, 18.0, heart::heart_2),
    ModelDef::new("regency_heart_3", 42.5, 35.0, heart::heart_3),
    // The Condenser, the material fabricator (`condenser`): tech 2, upgrading in place to
    // tech 3.
    ModelDef::tiered(
        "regency_fabricator",
        condenser::SIZES,
        condenser::spire::build,
    ),
    ModelDef::tiered(
        "regency_cyst",
        [(12.9, 8.0), (12.9, 12.0), (12.9, 16.0)],
        cyst::cyst,
    ),
    // Gun emplacements (`turrets`): point defence at three tiers, anti-air. The wall
    // (`palisade`).
    ModelDef::new("regency_barb", 5.5, 8.0, turrets::picket),
    ModelDef::new("regency_spitter", 5.5, 7.5, turrets::canopy),
    ModelDef::new("regency_airburst_repeater", 10.0, 9.0, turrets::gorget),
    ModelDef::new("regency_seeker_silo", 12.0, 14.0, turrets::belfry),
    ModelDef::new("regency_pinch_cannon", 10.5, 11.0, turrets::halberd),
    ModelDef::new("regency_fusion_cannon", 24.0, 24.0, turrets::sunspear),
    ModelDef::new("regency_palisade", 6.0, 5.4, palisade::palisade),
    // The shield generator (`ward`): tech 2, upgrading in place to tech 3.
    ModelDef::tiered(
        "regency_ward",
        [(13.9, 26.0), (13.9, 26.0), (13.9, 32.0)],
        ward::ward,
    ),
    // The reclaim tower (`crucible`): tech 1 to 3 in place, as tall at every tier.
    ModelDef::tiered(
        "regency_crucible",
        [(crucible::RADIUS, crucible::HEIGHT); 3],
        crucible::crucible,
    ),
    // Radar (`eye`).
    ModelDef::tiered(
        "regency_eye",
        [(7.0, 24.0), (7.0, 28.0), (7.0, 32.0)],
        eye::eye,
    ),
    // The Springald map gun: a three-rail Pinch-fusion gun laid high on a turntable, three
    // capacitor cells on its back (`turrets::springald`).
    ModelDef::new(
        "regency_springald",
        turrets::SPRINGALD_RADIUS,
        turrets::SPRINGALD_HEIGHT,
        turrets::springald,
    ),
    // At sea: the sonar's bell tripod (`sonar`), three tiers, and the torpedo launcher's
    // caisson (`torpedo`), two.
    ModelDef::tiered(
        "regency_sonar",
        [(6.0, 12.0), (6.0, 15.0), (6.0, 18.0)],
        sonar::sonar,
    ),
    ModelDef::tiered(
        "regency_torpedo",
        [(8.0, 6.0), (8.0, 8.0), (8.0, 8.0)],
        torpedo::torpedo,
    ),
    // Strategic launchers (`strategic`): the Mangonel silo and the Barbican array.
    ModelDef::new(
        "regency_nuke_silo",
        strategic::SILO_SIZE.0,
        strategic::SILO_SIZE.1,
        strategic::silo_vault,
    )
    .with_tier_4(),
    ModelDef::new(
        "regency_nuke_defense",
        strategic::ARRAY_SIZE.0,
        strategic::ARRAY_SIZE.1,
        strategic::array_mast,
    ),
];

/// Full-detail triangle budgets: the Regency's models are built from many separate parts, so each
/// model gets more than the library's default. `None` for a key that is not theirs.
#[cfg(test)]
pub(super) fn triangles(key: &str) -> Option<usize> {
    if let Some(budget) = gunships::triangles(key) {
        return Some(budget);
    }
    // A design variant (`mesh~name`) has its mesh's budget.
    Some(match key.split('~').next().unwrap_or(key) {
        "regency_commander" => COMMANDER_TRIANGLES,
        "regency_scorpion" => 14000,
        // The land and air factories' tech 3, with their tech 2 kit and more.
        "regency_brood" | "regency_hatchery" => 15000,
        "regency_tidebrood" => 9000,
        // The tech 1 hulls; the tech 2 warships have a capital ship's detail, lighter.
        "regency_attack_boat" => 2400,
        // The tech 1 air force: jets by the dozen, a transport the size of a frigate.
        "regency_flechette" => 2200,
        "regency_quarrel" | "regency_petard" => 3200,
        "regency_coffer" | "regency_ark" | "regency_space_frigate" => 6000,
        "regency_space_cruiser" | "regency_space_destroyer" => 9000,
        "regency_submarine" => 2800,
        "regency_frigate" => 4500,
        "regency_destroyer" | "regency_cruiser" => 7000,
        // Tech 3: the capital ships, and a big submarine.
        "regency_battleship" | "regency_carrier" => 14000,
        // The tech 3 jets: hull, blade, wings and the plates lapped over them.
        "regency_partisan" | "regency_augur" => 4000,
        "regency_maul" => 6000,
        "regency_assault_submarine" => 8000,
        // Three tiers, and the next one's pieces waiting on each.
        "regency_cyst" => 6000,
        // One 2 x 2 machine, its tech 3 plate and field gear waiting on it.
        "regency_fabricator" => 9500,
        // Four tiers, and the next one's pieces waiting on each.
        "regency_taproot" => 8500,
        "regency_heart" | "regency_barb" | "regency_spitter" => 4000,
        "regency_heart_2" => 6000,
        "regency_heart_3" => 9000,
        "regency_eye" => 7000,
        "regency_crucible" => crucible::TRIANGLES,
        // Three tiers of bells over a raft, the plummet under it.
        "regency_sonar" => 5000,
        // Two launcher houses in a caisson of plated walls.
        "regency_torpedo" => 6000,
        "regency_scout" => 3000,
        // The tech 1 line: a few hundred of each in a battle.
        "regency_raider" => 2600,
        "regency_hover_tank" | "regency_mobile_aa" => 3200,
        "regency_battle_tank" => 5000,
        "regency_seeker_hover" => 4500,
        "regency_reclaimer" | "regency_mattock" => 3400,
        // Three long walking legs and a plated head with two cannons.
        "regency_strider" => 7000,
        "regency_wake_tank" => 5000,
        "regency_bombard" => 4500,
        // Tech 3's kit: the ram, skirts, fin ring and two more lifts.
        "regency_engineer" => 3600,
        // Walls come by the dozen.
        "regency_palisade" => 1500,
        "regency_pinch_cannon" | "regency_airburst_repeater" => 5000,
        "regency_fusion_cannon" => 7500,
        // Eight cells, each a lid, a rim and a seeker, round a turning array.
        "regency_seeker_silo" => 6000,
        // Its tech 3 kit waiting on it.
        "regency_ward" => 6000,
        // A skirted tracked body, a turret, the gun and its caged core.
        "regency_skyspear" | "regency_fusion_howitzer" => 8000,
        // Strategic: an 8 x 8 launch complex, and a 4 x 4 array; a few a match.
        "regency_nuke_silo" => 9000,
        "regency_nuke_defense" => 6000,
        // A 6 x 6 map gun: plinth and pylons, a long turret, its plant, a three-rail gun.
        "regency_springald" => 12000,
        _ => return None,
    })
}

/// The reduced level's largest share of the full level, where it is not the library's 0.45.
/// The tech 3 land and air factories are mostly armour plates, which keep their sides at
/// the reduced level, and have no gears or rams for the full level to spend on. The Picket
/// and the Halberd are the same: plated legs, buttresses and gun shrouds, little else; so
/// are the tech 2 and 3 power generators.
#[cfg(test)]
pub(super) fn reduced_share(key: &str) -> Option<f32> {
    if let Some(share) = gunships::reduced_share(key) {
        return Some(share);
    }
    match key.split('~').next().unwrap_or(key) {
        "regency_brood" | "regency_hatchery" => Some(0.57),
        // The quay's tech 3: plated booms, pylons and crabs that keep their sides reduced.
        "regency_tidebrood" => Some(0.52),
        "regency_barb"
        | "regency_pinch_cannon"
        | "regency_airburst_repeater"
        | "regency_seeker_silo" => Some(0.52),
        // Walls and launcher drums: plated solids that keep their sides reduced.
        "regency_torpedo" => Some(0.52),
        // Towers and talons: plates that keep their sides when reduced.
        "regency_heart_2" | "regency_heart_3" => Some(0.5),
        // A lofted vessel and plates that keep their sides when reduced.
        "regency_fabricator" => Some(0.5),
        // The tech 1 line: faceted plates and lift bells that keep their shape when reduced.
        "regency_hover_tank" | "regency_raider" | "regency_mobile_aa" => Some(0.5),
        // Faceted plates and skirts that keep their sides when reduced.
        "regency_skyspear" | "regency_fusion_howitzer" => Some(0.52),
        // Plated hulls, heads and legs: faceted solids that keep their sides when reduced.
        "regency_strider" | "regency_wake_tank" => Some(0.5),
        // Bunkers, pylons and casemates of faceted plate that keep their sides reduced.
        "regency_nuke_silo" | "regency_nuke_defense" => Some(0.58),
        _ => None,
    }
}

/// One model's share of the library's checks (`models/tests.rs`), so a Regency model can be
/// tested on its own while its siblings are still being built: it fits its blueprint and
/// lot, wears team colour and dark plate at every level of detail, keeps to its budget, and its
/// turret reaches each muzzle.
#[cfg(test)]
pub(super) fn check(key: &str, radius: f32, height: f32, cells: Option<u32>, muzzles: &[[f32; 3]]) {
    check_at(key, 1, radius, height, cells, muzzles);
}

/// [`check`] for the model drawn at `tech`, for a structure with tiers. What a pit holds
/// may go below the ground, inside its opening (`models::Pit`).
#[cfg(test)]
pub(super) fn check_at(
    key: &str,
    tech: u8,
    radius: f32,
    height: f32,
    cells: Option<u32>,
    muzzles: &[[f32; 3]],
) {
    check_shots(key, tech, radius, height, cells, muzzles, None);
}

/// [`check`] for a gun that gathers its charge in front of its bore (the pinch guns): each
/// `charges` point is held between projectors, so the turret frames it, on opposite sides
/// within `hold` of it, but leaves the middle clear for the charge.
#[cfg(test)]
pub(super) fn check_charge(
    key: &str,
    radius: f32,
    height: f32,
    cells: Option<u32>,
    charges: &[[f32; 3]],
    hold: f32,
) {
    check_shots(key, 1, radius, height, cells, charges, Some(hold));
}

#[cfg(test)]
fn check_shots(
    key: &str,
    tech: u8,
    radius: f32,
    height: f32,
    cells: Option<u32>,
    muzzles: &[[f32; 3]],
    hold: Option<f32>,
) {
    use super::{material, part, rig};
    let model = super::build_model_scaled(key, radius, height, tech).expect(key);
    let down_the_pit = |v: &super::MeshVertex| {
        model.pit.is_some_and(|pit| {
            v.pos[2] < pit.open && glam::Vec2::new(v.pos[0], v.pos[1]).length() <= pit.radius
        })
    };
    let tris = |lod: usize| model.lods[lod].indices.len() / 3;
    let (full, mid, coarse) = (tris(0), tris(1), tris(2));
    let budget = triangles(key).unwrap_or(2600);
    assert!(
        full <= budget && full >= 250,
        "{key}: {full} triangles (budget {budget})"
    );
    assert!(
        mid as f32 <= full as f32 * reduced_share(key).unwrap_or(0.45) + 20.0 && coarse < 60,
        "{key}: {full}/{mid}/{coarse}"
    );
    for (lod, mesh) in model.lods.iter().enumerate() {
        let name = format!("{key} tech{tech} lod{lod}");
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
            mesh.vertices
                .iter()
                .all(|v| v.pos[2] >= -1e-3 || down_the_pit(v) || part::afloat_only(v.part)),
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
            let turret = || {
                mesh.vertices
                    .iter()
                    .filter(|v| v.part == part::TURRET)
                    .map(|v| glam::Vec3::from(v.pos) - m)
            };
            let near = turret().map(glam::Vec3::length).fold(f32::MAX, f32::min);
            match hold {
                None => assert!(near < 0.4, "{name}: turret {near} m from muzzle {m}"),
                Some(hold) => {
                    assert!(
                        near > hold * 0.4,
                        "{name}: projectors {near} m into the charge at {m}"
                    );
                    let round: Vec<glam::Vec3> = turret()
                        .filter(|d| d.length() < hold)
                        .map(|d| d.with_x(0.0))
                        .collect();
                    assert!(
                        round.iter().any(|a| round.iter().any(|c| a.dot(*c) < 0.0)),
                        "{name}: nothing holds the charge at {m} from both sides"
                    );
                }
            }
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
/// (`mc_core::print_heads`), and the model at tech `tech` has one at every head that tech
/// has fitted. A design variant (`mesh~name`) has its mesh's heads.
#[cfg(test)]
pub(super) fn check_heads_at(key: &str, tech: u8, radius: f32, height: f32) {
    use super::material;
    let design = key.split('~').next().unwrap_or(key);
    let factory = mc_core::print_heads::factory_heads(design).expect(key);
    assert!(
        factory.heads.iter().any(|h| h.fitted(tech)),
        "{key}: no heads at tech {tech}"
    );
    let model = super::build_model_scaled(key, radius, height, tech).expect(key);
    for head in factory.heads.iter().filter(|h| h.fitted(tech)) {
        let tip = glam::Vec3::from(mc_core::print_heads::nozzle(head, factory.aim));
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
        let src = include_str!("../../../mc-render/shaders/entity.wgsl");
        let line = format!("const UNIT_NANITE: u32 = {}u;", mc_sim::mirror::UNIT_NANITE);
        assert!(src.contains(&line), "entity.wgsl should say {line}");
    }

    #[test]
    fn the_shield_look_bits_are_the_mirrors_and_the_datas() {
        use crate::gpu_consts::shield_look;
        assert_eq!(shield_look::SHIFT, mc_sim::mirror::SHIELD_LOOK_SHIFT);
        assert_eq!(
            shield_look::HONEYCOMB,
            mc_data::ShieldLook::Honeycomb as u32
        );
        assert_eq!(shield_look::PRISM, mc_data::ShieldLook::Prism as u32);
        assert!(mc_data::ShieldLook::Prism as u32 <= shield_look::MASK);
    }

    #[test]
    fn the_nuke_look_numbers_are_the_datas() {
        use crate::gpu_consts::nuke_look;
        use mc_data::strategic::StrategicLook;
        assert_eq!(nuke_look::FISSION, StrategicLook::Fission as u32);
        assert_eq!(nuke_look::PLASMA, StrategicLook::Plasma as u32);
    }

    #[test]
    fn the_beam_shader_knows_the_sims_nanite_beam_kinds() {
        use crate::gpu_consts::beam;
        assert_eq!(beam::NANITE, mc_sim::reclaim::BEAM_NANITE);
        assert_eq!(beam::NANITE_SITE, mc_sim::reclaim::BEAM_NANITE_SITE);
        assert_eq!(beam::NANITE_RECLAIM, mc_sim::reclaim::BEAM_NANITE_RECLAIM);
    }
}
