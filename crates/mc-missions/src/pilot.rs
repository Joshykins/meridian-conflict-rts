//! Deterministic authoring smoke pilot: ordinary Company orders only.
use mc_core::Fx;
use mc_sim::{Command, PlayerCommand, World};
#[derive(Default)]
pub struct Pilot {
    target: String,
}
impl Pilot {
    pub fn commands(&mut self, world: &World) -> Vec<PlayerCommand> {
        let Some(m) = world.state.mission.as_ref() else {
            return vec![];
        };
        if !m.controls || m.outcome.is_some() {
            return vec![];
        }
        let next = route(m);
        if self.target == next && !world.tick_count().is_multiple_of(200) {
            return vec![];
        }
        self.target = next.into();
        let Some(region) = m.definition.placed.regions.iter().find(|r| r.name == next) else {
            return vec![];
        };
        let mut at = (region.min + region.max) * Fx::HALF;
        if next == "ridge" && m.fired.contains_key("ridge") {
            if let Some(gun) = m
                .definition
                .placed
                .units
                .iter()
                .find(|u| u.group == "bridge_guns")
            {
                at = gun.at;
            }
        }
        let Some(company) = m.groups.get("company") else {
            return vec![];
        };
        let units = company
            .iter()
            .copied()
            .filter(|id| world.state.units.row(*id).is_some())
            .collect();
        vec![PlayerCommand {
            player: 0,
            command: Command::AttackMove {
                units,
                target: at,
                queue: false,
            },
        }]
    }
}
fn route(m: &mc_sim::mission::Mission) -> &'static str {
    for (event, region) in [
        ("roadblock_clear", "first_contact"),
        ("pdf_linkup", "junction"),
        ("bridge_clear", "ridge"),
        ("miners_freed", "camp"),
        ("betrayal", "square"),
        ("breakout", "breakout"),
        ("holdouts_rescued", "warehouse"),
        ("flank", "harbour"),
        ("base_destroyed", "spaceport"),
    ] {
        if !m.fired.contains_key(event) {
            return region;
        }
    }
    "spaceport"
}
