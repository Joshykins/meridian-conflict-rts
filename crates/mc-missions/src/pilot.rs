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
        let next = route(&m.fired);
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
fn route(fired: &std::collections::BTreeMap<String, u32>) -> &'static str {
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
        if event == "holdouts_rescued" && fired.contains_key("holdouts_lost") {
            continue;
        }
        if !fired.contains_key(event) {
            return region;
        }
    }
    "spaceport"
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_lost_optional_force_does_not_strand_the_pilot_at_the_warehouse() {
        let mut fired = [
            "roadblock_clear",
            "pdf_linkup",
            "bridge_clear",
            "miners_freed",
            "betrayal",
            "breakout",
        ]
        .into_iter()
        .map(|n| (n.into(), 1))
        .collect();
        assert_eq!(route(&fired), "warehouse");
        fired.insert("holdouts_lost".into(), 2);
        assert_eq!(route(&fired), "harbour");
        fired.insert("flank".into(), 3);
        assert_eq!(route(&fired), "spaceport");
    }
}
