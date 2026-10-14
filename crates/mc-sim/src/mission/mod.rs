//! Deterministic mission events. Conditions, pending dialogue and fired events
//! are ordinary snapshot state; presentation never controls a simulation wait.
mod actions;
mod decode;
mod validate;
use crate::{Command, SimError, UnitId, World};
use mc_core::{Fx, StateHasher};
use mc_map::placed::Placed;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Definition {
    pub key: String,
    pub placed: Placed,
    pub events: Vec<Event>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub name: String,
    pub when: Condition,
    pub actions: Vec<Action>,
}

#[derive(Clone, Debug, Serialize)]
#[repr(u32)]
pub enum Condition {
    Elapsed(u32) = 0,
    After {
        event: String,
        ticks: u32,
    } = 1,
    Inside {
        unit: String,
        region: String,
    } = 2,
    DeadGroup(String) = 3,
    Unloaded(String) = 4,
    DeadUnit(String) = 5,
    HealthBelow {
        unit: String,
        percent: u8,
    } = 6,
    All(Vec<Condition>) = 7,
    Any(Vec<Condition>) = 8,
    /// All queued comms have finished in simulation time.
    DialogueIdle = 9,
    AliveGroup(String) = 10,
    Objective {
        line: String,
        status: Objective,
    } = 11,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[repr(u32)]
pub enum Action {
    Spawn(String) = 0,
    Load {
        group: String,
        carrier: String,
    } = 1,
    Land {
        unit: String,
        point: String,
        unload: bool,
    } = 2,
    TakeOff(String) = 3,
    Despawn(String) = 4,
    Controls(bool) = 5,
    Move {
        group: String,
        point: String,
        attack: bool,
    } = 6,
    Team {
        player: u8,
        team: u8,
    } = 7,
    Say {
        line: String,
        ticks: u32,
    } = 8,
    Objective {
        line: String,
        status: Objective,
    } = 9,
    Camera {
        name: String,
        ticks: u32,
    } = 10,
    Checkpoint(u16) = 11,
    Win = 12,
    Lose = 13,
    Arrive(String) = 14,
    Protect {
        unit: String,
        enabled: bool,
    } = 15,
    Ai {
        player: u8,
        config: Option<crate::AiConfig>,
    } = 16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum Objective {
    Active = 0,
    Complete = 1,
    Failed = 2,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Line {
    pub id: String,
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mission {
    pub definition: Definition,
    pub began: u32,
    pub fired: BTreeMap<String, u32>,
    pub units: BTreeMap<String, UnitId>,
    pub groups: BTreeMap<String, Vec<UnitId>>,
    pub objectives: BTreeMap<String, Objective>,
    pub dialogue: Vec<Line>,
    pub camera: Option<(String, u32, u32)>,
    pub beat: u16,
    pub checkpoint: u32,
    pub outcome: Option<bool>,
    pub controls: bool,
    pub protected: std::collections::BTreeSet<UnitId>,
}

#[derive(Clone, Debug, Default)]
pub struct Status {
    pub key: String,
    pub beat: u16,
    pub checkpoint: u32,
    pub line: Option<String>,
    pub objectives: Vec<(String, Objective)>,
    pub outcome: Option<bool>,
    pub controls: bool,
    pub camera: Option<(mc_map::placed::Camera, u32, u32)>,
}

impl Mission {
    pub(crate) fn hash(&self, h: &mut StateHasher) {
        // All fields are bounded integer/string data, ordered maps and vectors.
        // Hash the definition as well: different scripts must desync immediately.
        h.write_u8s(&bincode::serialize(self).expect("mission state serialises"));
    }

    fn ready(&self, condition: &Condition, world: &World) -> bool {
        let tick = world.state.tick;
        match condition {
            Condition::DialogueIdle => self.dialogue.is_empty(),
            Condition::Objective { line, status } => self.objectives.get(line) == Some(status),
            Condition::AliveGroup(name) => self.groups.get(name).is_some_and(|ids| {
                ids.iter().any(|id| {
                    world
                        .state
                        .units
                        .row(*id)
                        .is_some_and(|row| world.state.units.health[row] > Fx::ZERO)
                })
            }),
            Condition::Elapsed(n) => tick.saturating_sub(self.began) >= *n,
            Condition::After { event, ticks } => self
                .fired
                .get(event)
                .is_some_and(|at| tick.saturating_sub(*at) >= *ticks),
            Condition::Inside { unit, region } => self
                .units
                .get(unit)
                .and_then(|id| world.state.units.row(*id))
                .zip(
                    self.definition
                        .placed
                        .regions
                        .iter()
                        .find(|r| r.name == *region),
                )
                .is_some_and(|(row, r)| r.contains(world.state.units.pos[row])),
            Condition::DeadGroup(name) => self.groups.get(name).is_some_and(|ids| {
                !ids.is_empty() && ids.iter().all(|id| world.state.units.row(*id).is_none())
            }),
            Condition::Unloaded(name) => self.groups.get(name).is_some_and(|ids| {
                !ids.is_empty()
                    && ids.iter().all(|id| {
                        world
                            .state
                            .units
                            .row(*id)
                            .is_none_or(|r| world.state.units.hangar[r] == UnitId::NONE)
                    })
            }),
            Condition::DeadUnit(name) => self
                .units
                .get(name)
                .is_some_and(|id| world.state.units.row(*id).is_none()),
            Condition::HealthBelow { unit, percent } => self
                .units
                .get(unit)
                .and_then(|id| world.state.units.row(*id))
                .is_some_and(|row| {
                    world.state.units.health[row] * Fx::from_int(100)
                        < world.bp(row).health * Fx::from_int(*percent as i32)
                }),
            Condition::All(conditions) => conditions.iter().all(|c| self.ready(c, world)),
            Condition::Any(conditions) => conditions.iter().any(|c| self.ready(c, world)),
        }
    }

    fn live_row(&self, name: &str, world: &World) -> Result<usize, SimError> {
        self.units
            .get(name)
            .and_then(|id| world.state.units.row(*id))
            .ok_or_else(|| SimError::Setup(format!("Mission unit {name} is not alive")))
    }

    fn load_cargo(
        &mut self,
        group: &str,
        carrier: &str,
        world: &mut World,
    ) -> Result<(), SimError> {
        let ship = self.live_row(carrier, world)?;
        let transport = world
            .transport(ship)
            .ok_or_else(|| SimError::Setup("Mission carrier is not a transport".into()))?;
        let ids = self
            .groups
            .get(group)
            .cloned()
            .ok_or_else(|| SimError::Setup(format!("Group {group} is not spawned")))?;
        let mut space = world.cargo_used(ship);
        for id in &ids {
            let row = world
                .state
                .units
                .row(*id)
                .ok_or_else(|| SimError::Setup("Mission cargo is missing".into()))?;
            if world.state.units.owner[row] != world.state.units.owner[ship]
                || !world.cargo_fits(row, ship)
            {
                return Err(SimError::Setup(
                    "Mission cargo cannot board this carrier".into(),
                ));
            }
            space = space.saturating_add(world.bp(row).cargo_room().unwrap_or(u16::MAX));
        }
        if space > transport.capacity {
            return Err(SimError::Setup(
                "Mission cargo exceeds the hold capacity".into(),
            ));
        }
        for id in ids {
            let row = world.state.units.row(id).expect("validated cargo");
            world.stow(row, ship)?;
        }

        Ok(())
    }

    fn act(&mut self, action: &Action, world: &mut World) -> Result<(), SimError> {
        let tick = world.state.tick;
        match action {
            Action::Arrive(unit) => self.arrive(unit, world)?,
            Action::Protect { unit, enabled } => {
                let row = self.live_row(unit, world)?;
                let id = world.state.units.id(row);
                if *enabled {
                    self.protected.insert(id);
                } else {
                    self.protected.remove(&id);
                }
            }
            Action::Ai { player, config } => {
                let seat = *player as usize;
                world.state.players[seat].controller = if config.is_some() {
                    crate::tables::Controller::Ai
                } else {
                    crate::tables::Controller::Human
                };
                if let Some(config) = config {
                    world.state.ai[seat].config = *config;
                    world.state.players[seat].income_permille = [config.income; 2];
                    world.state.players[seat].build_permille = config.build;
                }
            }
            Action::Spawn(group) => self.spawn(group, world)?,
            Action::Load { group, carrier } => self.load_cargo(group, carrier, world)?,
            Action::Land {
                unit,
                point,
                unload,
            } => {
                let row = self.live_row(unit, world)?;
                let pos = self
                    .definition
                    .placed
                    .points
                    .iter()
                    .find(|p| p.name == *point)
                    .ok_or_else(|| SimError::Setup(format!("Missing point {point}")))?
                    .at;
                world.apply_as(
                    world.state.units.owner[row],
                    &Command::Land {
                        units: vec![world.state.units.id(row)],
                        pos,
                        unload: *unload,
                        queue: false,
                    },
                )?;
            }
            Action::TakeOff(unit) => {
                let row = self.live_row(unit, world)?;
                world.apply_as(
                    world.state.units.owner[row],
                    &Command::TakeOff {
                        units: vec![world.state.units.id(row)],
                    },
                )?;
            }
            Action::Despawn(group) => {
                let ids = self.groups.get(group).cloned().unwrap_or_default();
                for id in ids {
                    if let Some(row) = world.state.units.row(id) {
                        world.remove_unit_row(row, false)?;
                    }
                }
            }
            Action::Controls(enabled) => self.controls = *enabled,
            Action::Move {
                group,
                point,
                attack,
            } => self.move_group(group, point, *attack, world)?,
            Action::Team { player, team } => {
                world.state.players[*player as usize].team = *team;
                crate::tables::derive_allies(&mut world.state.players);
            }
            Action::Say { line, ticks } => {
                let start = self.dialogue.last().map_or(tick, |l| l.end.max(tick));
                self.dialogue.push(Line {
                    id: line.clone(),
                    start,
                    end: start.saturating_add(*ticks),
                });
            }
            Action::Objective { line, status } => {
                self.objectives.insert(line.clone(), *status);
            }
            Action::Camera { name, ticks } => {
                self.camera = Some((name.clone(), tick, tick.saturating_add(*ticks)))
            }
            Action::Checkpoint(beat) => {
                self.beat = *beat;
                self.checkpoint = tick;
            }
            Action::Win | Action::Lose => {
                let won = matches!(action, Action::Win);
                self.outcome = Some(won);
                let team = if won {
                    world.state.players[0].team
                } else {
                    world.state.players[1].team
                };
                world.state.winner = Some(team);
                world
                    .events
                    .push(crate::mirror::SimEvent::MatchOver { winner_team: team });
            }
        }
        Ok(())
    }
}

impl World {
    pub fn begin_mission(&mut self, definition: Definition) -> Result<(), SimError> {
        definition.validate(
            &self.blueprints,
            self.state.players.len(),
            self.terrain.size_metres(),
            self.map.props.len(),
        )?;
        self.state.mission = Some(Mission {
            definition,
            began: self.state.tick,
            fired: Default::default(),
            units: Default::default(),
            groups: Default::default(),
            objectives: Default::default(),
            dialogue: vec![],
            camera: None,
            beat: 0,
            checkpoint: 0,
            outcome: None,
            controls: true,
            protected: Default::default(),
        });
        self.run_mission()
    }

    pub(crate) fn run_mission(&mut self) -> Result<(), SimError> {
        let Some(mut mission) = self.state.mission.take() else {
            return Ok(());
        };
        let result = (|| {
            if mission.outcome.is_some() {
                return Ok(());
            }
            mission.dialogue.retain(|line| line.end > self.state.tick);
            for index in 0..mission.definition.events.len() {
                let event = &mission.definition.events[index];
                if !mission.fired.contains_key(&event.name) && mission.ready(&event.when, self) {
                    let event = event.clone();
                    mission.fired.insert(event.name, self.state.tick);
                    for action in &event.actions {
                        mission.act(action, self)?;
                    }
                    if mission.outcome.is_some() {
                        break;
                    }
                }
            }
            Ok(())
        })();
        self.state.mission = Some(mission);
        result
    }

    pub fn mission_status(&self) -> Option<Status> {
        let m = self.state.mission.as_ref()?;
        Some(Status {
            key: m.definition.key.clone(),
            beat: m.beat,
            checkpoint: m.checkpoint,
            line: m
                .dialogue
                .iter()
                .find(|l| l.start <= self.state.tick && l.end > self.state.tick)
                .map(|l| l.id.clone()),
            objectives: m.objectives.iter().map(|(k, v)| (k.clone(), *v)).collect(),
            outcome: m.outcome,
            controls: m.controls,
            camera: m
                .camera
                .as_ref()
                .filter(|(_, _, end)| *end > self.state.tick)
                .and_then(|(name, start, end)| {
                    m.definition
                        .placed
                        .cameras
                        .iter()
                        .find(|c| c.name == *name)
                        .map(|c| (c.clone(), *start, *end))
                }),
        })
    }
}

#[cfg(test)]
mod tests;
