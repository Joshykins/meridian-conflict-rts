//! Convert map-authored slots into a local match roster.
pub(crate) fn roster(slots: &[mc_data::map_settings::MapPlayer]) -> Vec<mc_sim::PlayerSetup> {
    slots
        .iter()
        .map(|p| mc_sim::PlayerSetup {
            name: p.name.clone(),
            faction: p.faction.clone(),
            team: p.team,
            start: p.start,
            ai: Default::default(),
            controller: if p.computer {
                mc_sim::tables::Controller::Ai
            } else {
                mc_sim::tables::Controller::Human
            },
        })
        .collect()
}
