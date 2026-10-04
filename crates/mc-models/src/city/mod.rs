//! The city kit (map prop kinds 96..=120): an ARC colonial city's houses, blocks,
//! towers, public buildings and its wall, each built to its numbers in
//! `mc_map::city` (solid parts, tops), its street front to local +y.
//!
//! Detail is mostly the shader's (city.wgsl): walls, windows, shopfronts, curtain
//! walls and roofs are drawn on plain faces by their `gpu_consts::city` pattern, so
//! a block is a few dozen triangles at a distance and every pane can break on its
//! own. Geometry carries what a silhouette and a close look need: roofs, cornices,
//! parapets, balconies, bays, porticoes, plant, masts.

use mc_map::PropKind;

use crate::builder::MeshBuilder;
use crate::library::ModelDef;

mod blocks;
mod civic;
mod kit;
mod outskirts;
mod towers;
mod wall;

/// Collision radius and height a city kind's model is authored at: the reach of its
/// plan and its highest top, at scale 1.
const fn size(kind: PropKind) -> (f32, f32) {
    let Some(s) = mc_map::city::structure(kind) else {
        return (1.0, 1.0);
    };
    let mut reach = 6.0f32;
    let mut top = 3.0f32;
    let mut i = 0;
    while i < s.plan.len() {
        let (cx, cy, hx, hy) = s.plan[i];
        let x = (cx.abs() + hx) as f32;
        let y = (cy.abs() + hy) as f32;
        let r = if x > y { x } else { y };
        if r > reach {
            reach = r;
        }
        if s.tops[i] as f32 > top {
            top = s.tops[i] as f32;
        }
        i += 1;
    }
    (reach, top)
}

const fn def(key: &'static str, kind: PropKind, build: fn(&mut MeshBuilder, u8)) -> ModelDef {
    let (r, h) = size(kind);
    ModelDef::new(key, r, h, build).with_far()
}

pub(super) const MODELS: &[ModelDef] = &[
    def("city_house", PropKind::CityHouse, outskirts::house),
    def(
        "city_rowhouses",
        PropKind::CityRowhouses,
        outskirts::rowhouses,
    ),
    def("city_shops", PropKind::CityShops, outskirts::shops),
    def(
        "city_farmstead",
        PropKind::CityFarmstead,
        outskirts::farmstead,
    ),
    def(
        "city_warehouse",
        PropKind::CityWarehouse,
        outskirts::warehouse,
    ),
    def("city_factory", PropKind::CityFactory, outskirts::factory),
    def(
        "city_tank_farm",
        PropKind::CityTankFarm,
        outskirts::tank_farm,
    ),
    def("city_tenement", PropKind::CityTenement, blocks::tenement),
    def("city_courtyard", PropKind::CityCourtyard, blocks::courtyard),
    def(
        "city_apartments",
        PropKind::CityApartments,
        blocks::apartments,
    ),
    def("city_office", PropKind::CityOffice, blocks::office),
    def("city_highrise", PropKind::CityHighrise, towers::highrise),
    def(
        "city_skyscraper",
        PropKind::CitySkyscraper,
        towers::skyscraper,
    ),
    def("city_spire", PropKind::CitySpire, towers::spire),
    def("city_slab", PropKind::CitySlab, towers::slab),
    def("city_civic", PropKind::CityCivic, civic::civic),
    def("city_station", PropKind::CityStation, civic::station),
    def("city_garage", PropKind::CityGarage, blocks::garage),
    def("city_mall", PropKind::CityMall, blocks::mall),
    def("city_church", PropKind::CityChurch, civic::church),
    def("city_ruin", PropKind::CityRuin, blocks::ruin),
    def("city_wall", PropKind::CityWall, wall::wall),
    def("city_wall_tower", PropKind::CityWallTower, wall::tower),
    def("city_gate", PropKind::CityGate, wall::gate),
    def("city_rubble", PropKind::CityRubble, blocks::rubble),
];

/// Full-detail triangle budgets above the default: the towers, a few dozen on the
/// map and seen from everywhere, and the big public buildings, one or two each.
#[cfg(test)]
pub(super) fn triangles(key: &str) -> Option<usize> {
    match key {
        "city_highrise" | "city_skyscraper" | "city_spire" | "city_slab" => Some(6500),
        "city_civic" | "city_station" | "city_church" | "city_courtyard" => Some(4500),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
