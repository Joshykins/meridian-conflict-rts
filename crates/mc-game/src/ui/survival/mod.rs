//! Survival's pieces for the set-up screen (`super::setup`, which skirmish
//! shares): the theatres, the siege chart (`siege`) with the Progenitor's
//! fronts flowing toward the defenders' zones, and the engagement rules and
//! round forecast (`rules`).

mod marks;
pub mod rules;
pub mod siege;

use super::maps::{self, MapCard};
use mc_data::survival::SurvivalLayout;
use mc_map::MapFile;
use std::sync::Arc;

pub use marks::engine_mark;

pub struct Theatre {
    pub stem: String,
    pub map: Arc<MapFile>,
    pub layout: SurvivalLayout,
    pub look: mc_data::weather::MapLook,
}

/// The map at `path` as a theatre and as the browser shows it, when its
/// settings (`config`, which has a survival block) give a usable layout.
pub(super) fn theatre_of(
    path: &std::path::Path,
    config: mc_data::weather::MapConfig,
) -> Option<(Theatre, MapCard)> {
    let card = maps::open_card(path, &config)?;
    let layout = crate::survival::layout_in(&card.map, config)?;
    Some((
        Theatre {
            stem: card.stem.clone(),
            map: card.map.clone(),
            layout,
            look: card.look.clone(),
        },
        card,
    ))
}
