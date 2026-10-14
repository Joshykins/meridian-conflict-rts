use super::*;
use std::collections::BTreeSet;
impl Definition {
    pub fn validate(
        &self,
        blueprints: &mc_data::Blueprints,
        players: usize,
        size: mc_core::FxVec2,
        props: usize,
    ) -> Result<(), SimError> {
        self.validate_inner(blueprints, players, size, props)
            .map_err(SimError::Setup)
    }

    fn validate_inner(
        &self,
        blueprints: &mc_data::Blueprints,
        players: usize,
        size: mc_core::FxVec2,
        props: usize,
    ) -> Result<(), String> {
        if self.key.is_empty()
            || self.key.len() > 128
            || self.events.is_empty()
            || self.events.len() > 512
        {
            return Err("invalid mission definition size".into());
        }
        let info = mc_map::MapInfo {
            name: "mission validation".into(),
            tiles_w: (size.x.floor_int() / mc_map::TILE_SIZE_M) as u32,
            tiles_h: (size.y.floor_int() / mc_map::TILE_SIZE_M) as u32,
            min_z: Fx::ZERO,
            z_step: Fx::ONE,
            water_level: Fx::ZERO,
        };
        self.placed
            .validate(&info, props)
            .map_err(|e| e.to_string())?;
        for unit in &self.placed.units {
            if unit.owner as usize >= players || blueprints.id_of(&unit.blueprint).is_none() {
                return Err(format!("invalid mission unit {}", unit.name));
            }
        }
        self.validate_program(players)
    }

    pub(super) fn validate_program(&self, players: usize) -> Result<(), String> {
        if self.key.is_empty()
            || self.key.len() > 128
            || self.events.is_empty()
            || self.events.len() > 512
        {
            return Err("invalid mission definition size".into());
        }
        let names: BTreeSet<_> = self.events.iter().map(|e| e.name.as_str()).collect();
        if names.len() != self.events.len() {
            return Err("duplicate mission event".into());
        }
        let group = |name: &str| self.placed.units.iter().any(|u| u.group == name);
        let mut total = 0;
        for event in &self.events {
            if event.name.is_empty() || event.name.len() > 128 {
                return Err("invalid event name".into());
            }
            self.condition(&event.when, &names, 0)?;
            total += event.actions.len();
            if total > 4096 {
                return Err("too many mission actions".into());
            }
            for action in &event.actions {
                let valid = match action {
                    Action::Spawn(name) | Action::Despawn(name) => group(name),
                    Action::Load {
                        group: name,
                        carrier,
                    } => group(name) && self.placed.units.iter().any(|u| u.name == *carrier),
                    Action::Land { unit, point, .. } => {
                        self.placed.units.iter().any(|u| u.name == *unit)
                            && self.placed.points.iter().any(|p| p.name == *point)
                    }
                    Action::Ai { player, config } => {
                        (*player as usize) < players
                            && config.is_none_or(|c| {
                                c.domain_weights.iter().all(|w| *w <= 200)
                                    && (250..=10_000).contains(&c.income)
                                    && (250..=10_000).contains(&c.build)
                            })
                    }
                    Action::Arrive(unit) | Action::Protect { unit, .. } | Action::TakeOff(unit) => {
                        self.placed.units.iter().any(|u| u.name == *unit)
                    }
                    Action::Move {
                        group: name, point, ..
                    } => group(name) && self.placed.points.iter().any(|p| p.name == *point),
                    Action::Team { player, .. } => (*player as usize) < players,
                    Action::Say { line, ticks } => {
                        !line.is_empty() && line.len() <= 128 && (1..=36000).contains(ticks)
                    }
                    Action::Objective { line, .. } => !line.is_empty() && line.len() <= 128,
                    Action::Camera { name, ticks } => {
                        self.placed.cameras.iter().any(|c| c.name == *name) && *ticks <= 36000
                    }
                    _ => true,
                };
                if !valid {
                    return Err(format!("invalid action in {}", event.name));
                }
            }
        }
        Ok(())
    }

    fn condition(
        &self,
        c: &Condition,
        events: &BTreeSet<&str>,
        depth: usize,
    ) -> Result<(), String> {
        if depth > 8 {
            return Err("mission conditions nested too deeply".into());
        }
        let unit = |name: &str| self.placed.units.iter().any(|u| u.name == name);
        let valid = match c {
            Condition::Elapsed(_) | Condition::DialogueIdle => true,
            Condition::After { event, .. } => events.contains(event.as_str()),
            Condition::Inside { unit: name, region } => {
                unit(name) && self.placed.regions.iter().any(|r| r.name == *region)
            }
            Condition::DeadUnit(name) => unit(name),
            Condition::HealthBelow {
                unit: name,
                percent,
            } => unit(name) && *percent <= 100,
            Condition::DeadGroup(name) | Condition::Unloaded(name) => {
                self.placed.units.iter().any(|u| u.group == *name)
            }
            Condition::All(cs) | Condition::Any(cs) => {
                if cs.is_empty() || cs.len() > 32 {
                    return Err("invalid condition list".into());
                }
                for c in cs {
                    self.condition(c, events, depth + 1)?;
                }
                true
            }
        };
        if valid {
            Ok(())
        } else {
            Err("unknown mission condition reference".into())
        }
    }
}

impl Mission {
    pub(crate) fn validate(&self, players: usize) -> Result<(), String> {
        self.definition.validate_program(players)?;
        if self.fired.len() > 512
            || self.units.len() > 16384
            || self.groups.len() > 16384
            || self.objectives.len() > 4096
            || self.dialogue.len() > 4096
            || self.protected.len() > 16384
        {
            return Err("mission state exceeds its limits".into());
        }
        if self
            .fired
            .keys()
            .any(|name| !self.definition.events.iter().any(|e| e.name == *name))
        {
            return Err("unknown fired mission event".into());
        }
        if self
            .units
            .keys()
            .any(|name| !self.definition.placed.units.iter().any(|u| u.name == *name))
            || self.groups.keys().any(|name| {
                !self
                    .definition
                    .placed
                    .units
                    .iter()
                    .any(|u| u.group == *name)
            })
            || self
                .protected
                .iter()
                .any(|id| !self.units.values().any(|known| known == id))
            || self
                .groups
                .values()
                .flatten()
                .any(|id| !self.units.values().any(|known| known == id))
        {
            return Err("unknown mission runtime identity".into());
        }
        if self
            .dialogue
            .iter()
            .any(|line| line.id.len() > 128 || line.start >= line.end)
        {
            return Err("invalid mission dialogue".into());
        }
        Ok(())
    }
}
