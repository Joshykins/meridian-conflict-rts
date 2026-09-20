//! The test range: a real match with cheats on, one unit on a pad, and a panel
//! that does things to it. Everything here becomes `Command`s, the same way a
//! player's orders do, so the range shows exactly what a match would.
//!
//! Two sides: blue (slot 0) and red (slot 1). Whoever owns the unit you select
//! is the side you command, so the red tanks can be told to attack as easily
//! as the blue one.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::{cat, BlueprintId, Blueprints, UnitBlueprint};
use mc_sim::mirror::{UnitInstance, KIND_WRECK};
use mc_sim::tables::flag;
use mc_sim::{Command, Handle};
use std::collections::HashSet;

pub const BLUE: u8 = 0;
pub const RED: u8 = 1;
pub const DEFAULT_SUBJECT: &str = "aster_t1_tank";
/// How many the spawn count steps through.
pub const COUNTS: [u16; 5] = [1, 3, 5, 10, 25];
/// Camera distances of the three zoom keys: on the hull, the engagement, the strategic view.
pub const ZOOMS: [f32; 3] = [45.0, 260.0, 2200.0];

/// What a spawned unit is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Blue,
    Red,
    /// Red, holds its fire and cannot be hurt: something to shoot at.
    Dummy,
}

impl Side {
    pub const ALL: [Side; 3] = [Side::Blue, Side::Red, Side::Dummy];

    pub fn label(self) -> &'static str {
        match self {
            Side::Blue => "BLUE",
            Side::Red => "RED",
            Side::Dummy => "DUMMY",
        }
    }

    fn owner_flags(self) -> (u8, u16) {
        match self {
            Side::Blue => (BLUE, 0),
            Side::Red => (RED, 0),
            Side::Dummy => (RED, flag::PASSIVE | flag::INVULNERABLE),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scenario {
    /// Red units in range of the pad open fire on whatever stands there.
    UnderFire,
    /// Dummies at short, medium and long range for the subject to shoot.
    Targets,
    /// Whatever builds the subject is set to building it.
    BuildIt,
    /// One dummy almost at the subject's feet, to see its weapons point down at it.
    PointBlank,
    /// The subject builds the first thing it can, off to one side, so it has to turn to it.
    AtWork,
    /// The subject upgrades itself.
    Refit,
    /// The subject walks down the range.
    March,
    /// The subject destroys itself.
    Destruct,
}

impl Scenario {
    pub const ALL: [Scenario; 8] = [
        Scenario::UnderFire,
        Scenario::PointBlank,
        Scenario::Targets,
        Scenario::BuildIt,
        Scenario::AtWork,
        Scenario::Refit,
        Scenario::March,
        Scenario::Destruct,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Scenario::UnderFire => "ATTACKED",
            Scenario::Targets => "TARGETS",
            Scenario::BuildIt => "BUILD IT",
            Scenario::PointBlank => "CLOSE IN",
            Scenario::AtWork => "AT WORK",
            Scenario::Refit => "UPGRADE",
            Scenario::March => "MARCH",
            Scenario::Destruct => "DESTRUCT",
        }
    }

    pub fn parse(s: &str) -> Option<Scenario> {
        Some(match s {
            "under-fire" => Scenario::UnderFire,
            "targets" => Scenario::Targets,
            "build" => Scenario::BuildIt,
            "close" => Scenario::PointBlank,
            "work" => Scenario::AtWork,
            "upgrade" => Scenario::Refit,
            "march" => Scenario::March,
            "destruct" => Scenario::Destruct,
            _ => return None,
        })
    }
}

/// What the range panel asks for. `game.rs` turns these into commands.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum RangeAction {
    /// Step the subject through the blueprints.
    Subject(i32),
    Count(i32),
    Side(Side),
    /// Arm the pointer: the next click on the ground spawns there.
    ArmSpawn,
    /// Thousandths of full health to take away; negative gives it back.
    Damage(i16),
    Remove,
    /// Set (or clear) a `flag::DEBUG` bit on the units acted on.
    Flag(u16, bool),
    /// How complete the units acted on are, thousandths.
    Build(u16),
    Scenario(Scenario),
    Reset,
    Control(u8),
    FreeBuild(bool),
    Zoom(usize),
    /// Read `data/` again and restart the range.
    Reload,
}

/// An order that has to wait for a unit the range just spawned to show up.
struct Pending {
    builder: BlueprintId,
    /// Units that were already there, so the new one can be told apart.
    known: HashSet<u32>,
    order: PendingOrder,
}

/// A scenario's commands, and the order it still owes to the builder it spawned.
type Staged = (Vec<Command>, Option<(BlueprintId, PendingOrder)>);

enum PendingOrder {
    Produce {
        blueprint: BlueprintId,
        count: u8,
        rally: FxVec2,
    },
    Build {
        blueprint: BlueprintId,
        pos: FxVec2,
    },
    Upgrade,
    Move {
        pos: FxVec2,
    },
    Destruct,
}

impl PendingOrder {
    fn commands(&self, who: Vec<mc_sim::UnitId>) -> Vec<Command> {
        match *self {
            PendingOrder::Produce {
                blueprint,
                count,
                rally,
            } => vec![
                Command::SetRally {
                    factories: who.clone(),
                    pos: rally,
                },
                Command::Produce {
                    factories: who,
                    blueprint,
                    count,
                },
            ],
            PendingOrder::Build { blueprint, pos } => vec![Command::Build {
                units: who,
                blueprint,
                pos,
                heading: Angle::from_degrees(270),
                queue: false,
            }],
            PendingOrder::Upgrade => vec![Command::Upgrade { units: who }],
            PendingOrder::Move { pos } => vec![Command::Move {
                units: who,
                target: pos,
                queue: false,
            }],
            PendingOrder::Destruct => vec![Command::SelfDestruct { units: who }],
        }
    }
}

pub struct Range {
    /// Where the subject stands: the first start position's flat ground.
    pub pad: FxVec2,
    pub subject: BlueprintId,
    /// Index into `COUNTS`.
    pub count: usize,
    pub side: Side,
    pub free_build: bool,
    pending: Option<Pending>,
}

/// The units a panel action applies to, and what to call them.
pub struct Acted {
    pub ids: Vec<u32>,
    pub label: String,
}

impl Range {
    pub fn new(pad: FxVec2, subject: BlueprintId) -> Range {
        Range {
            pad,
            subject,
            count: 0,
            side: Side::Blue,
            free_build: true,
            pending: None,
        }
    }

    pub fn count(&self) -> u16 {
        COUNTS[self.count]
    }

    /// The selection, or with nothing selected every unit of the subject's type on the side being commanded.
    pub fn acted(
        &self,
        selection: &[u32],
        units: &[UnitInstance],
        blueprints: &Blueprints,
        side: u8,
    ) -> Acted {
        if !selection.is_empty() {
            return Acted {
                ids: selection.to_vec(),
                label: format!("SELECTION  \u{b7}  {}", selection.len()),
            };
        }
        let ids: Vec<u32> = units
            .iter()
            .filter(|u| {
                u.owner_flags & (KIND_WRECK | 0xFF) == side as u32
                    && u.blueprint == self.subject.0 as u32
            })
            .map(|u| u.unit_id)
            .collect();
        let name = blueprints.unit(self.subject).name.to_uppercase();
        Acted {
            label: format!(
                "EVERY {} {name}  \u{b7}  {}",
                if side == BLUE { "BLUE" } else { "RED" },
                ids.len()
            ),
            ids,
        }
    }

    /// The commands that put the range back the way it opens.
    pub fn reset(&mut self, blueprints: &Blueprints) -> Vec<Command> {
        self.pending = None;
        let mut out = vec![Command::DebugClear, Command::DebugControl { player: BLUE }];
        out.extend(
            opening(
                blueprints,
                self.pad,
                self.subject,
                self.count(),
                self.free_build,
                None,
            )
            .0,
        );
        out
    }

    pub fn spawn_at(&self, pos: FxVec2) -> Command {
        let (owner, flags) = self.side.owner_flags();
        // Blue faces the range; everything else faces the pad.
        let heading = if self.side == Side::Blue || pos == self.pad {
            Angle::ZERO
        } else {
            (self.pad - pos).angle()
        };
        Command::DebugSpawn {
            owner,
            blueprint: self.subject,
            pos,
            heading,
            count: self.count(),
            flags,
            build: 1000,
        }
    }

    /// Starts a scenario. `Err` says why it cannot be staged for this subject.
    pub fn stage(
        &mut self,
        scenario: Scenario,
        blueprints: &Blueprints,
        units: &[UnitInstance],
    ) -> Result<Vec<Command>, &'static str> {
        let (mut commands, pending) = stage(scenario, blueprints, self.pad, self.subject)?;
        self.pending = pending.map(|(builder, order)| {
            // Staged before: the builder that is already standing there takes the order again.
            let standing = units.iter().any(|u| {
                u.owner_flags & (KIND_WRECK | 0xFF) == BLUE as u32
                    && u.blueprint == builder.0 as u32
                    && u.build >= 1.0
            });
            if standing {
                commands.clear();
            }
            let known = if standing {
                HashSet::new()
            } else {
                units.iter().map(|u| u.unit_id).collect()
            };
            Pending {
                builder,
                known,
                order,
            }
        });
        Ok(commands)
    }

    /// Once the builder a scenario spawned is on the map, the order it was spawned for.
    pub fn resolve_pending(&mut self, units: &[UnitInstance]) -> Vec<Command> {
        let Some(p) = &self.pending else {
            return Vec::new();
        };
        let found = units.iter().find(|u| {
            u.owner_flags & KIND_WRECK == 0
                && u.owner_flags & 0xFF == BLUE as u32
                && u.blueprint == p.builder.0 as u32
                && u.build >= 1.0
                && !p.known.contains(&u.unit_id)
        });
        let Some(unit) = found else { return Vec::new() };
        let out = p.order.commands(vec![Handle(unit.unit_id)]);
        self.pending = None;
        // Building is blue's business whichever side was being steered.
        std::iter::once(Command::DebugControl { player: BLUE })
            .chain(out)
            .collect()
    }
}

/// The subject on its pad, and optionally a scenario around it: the first
/// tick of a range. The second value is the order a scenario still owes.
fn opening(
    blueprints: &Blueprints,
    pad: FxVec2,
    subject: BlueprintId,
    count: u16,
    free_build: bool,
    scenario: Option<Scenario>,
) -> Staged {
    let mut out = vec![
        Command::DebugFreeBuild {
            player: BLUE,
            on: free_build,
        },
        Command::DebugFreeBuild {
            player: RED,
            on: free_build,
        },
    ];
    out.push(Command::DebugSpawn {
        owner: BLUE,
        blueprint: subject,
        pos: pad,
        heading: Angle::ZERO,
        count,
        flags: 0,
        build: 1000,
    });
    let mut owed = None;
    if let Some(Ok((commands, pending))) = scenario.map(|s| stage(s, blueprints, pad, subject)) {
        out.extend(commands);
        owed = pending;
    }
    (out, owed)
}

/// First-tick commands for the `range` scene (windowed and headless alike).
pub fn opening_commands(
    blueprints: &Blueprints,
    pad: FxVec2,
    subject: BlueprintId,
    scenario: Option<Scenario>,
) -> Vec<Command> {
    opening(blueprints, pad, subject, 1, true, scenario).0
}

/// Second-tick commands for a headless range: the order its scenario owes, given to
/// the first complete blue unit of the builder's type.
pub fn owed_commands(
    blueprints: &Blueprints,
    pad: FxVec2,
    subject: BlueprintId,
    scenario: Option<Scenario>,
    blue_units: &[(BlueprintId, mc_sim::UnitId)],
) -> Vec<Command> {
    let Some((builder, order)) = opening(blueprints, pad, subject, 1, true, scenario).1 else {
        return Vec::new();
    };
    let Some((_, id)) = blue_units.iter().find(|(bp, _)| *bp == builder) else {
        return Vec::new();
    };
    order.commands(vec![*id])
}

/// The lowest-tech armed mobile unit that can shoot at `target`, the medium tank if it can.
fn attacker_for<'a>(
    blueprints: &'a Blueprints,
    target: &UnitBlueprint,
) -> Option<&'a UnitBlueprint> {
    let can = |bp: &&UnitBlueprint| {
        bp.is_mobile()
            && !bp.has(cat::COMMANDER)
            && bp
                .weapons
                .iter()
                .any(|w| w.target_mask & target.categories != 0)
    };
    let tank = blueprints
        .id_of(DEFAULT_SUBJECT)
        .map(|id| blueprints.unit(id));
    tank.filter(&can)
        .or_else(|| blueprints.units.iter().filter(can).min_by_key(|bp| bp.tech))
}

fn stage(
    scenario: Scenario,
    blueprints: &Blueprints,
    pad: FxVec2,
    subject: BlueprintId,
) -> Result<Staged, &'static str> {
    let bp = blueprints.unit(subject);
    let east = |metres: Fx, across: i32| pad + FxVec2::new(metres, Fx::from_int(across));
    match scenario {
        Scenario::UnderFire => {
            let attacker = attacker_for(blueprints, bp).ok_or("NOTHING CAN SHOOT AT THIS")?;
            let weapon = attacker
                .weapons
                .iter()
                .find(|w| w.target_mask & bp.categories != 0)
                .expect("chosen for it");
            // Well inside their range, so they open fire where they stand.
            let distance = weapon.range_min
                + (weapon.range_max - weapon.range_min) * Fx::ratio(7, 10)
                + bp.radius;
            Ok((
                vec![Command::DebugSpawn {
                    owner: RED,
                    blueprint: attacker.id,
                    pos: east(distance, 0),
                    heading: Angle::HALF_TURN,
                    count: 2,
                    flags: 0,
                    build: 1000,
                }],
                None,
            ))
        }
        Scenario::Targets => {
            let weapon = bp.weapons.first().ok_or("THIS UNIT IS UNARMED")?;
            let dummy = blueprints
                .units
                .iter()
                .filter(|d| {
                    d.is_mobile()
                        && !d.has(cat::COMMANDER)
                        && d.categories & weapon.target_mask != 0
                })
                .min_by_key(|d| (d.key != DEFAULT_SUBJECT, d.tech))
                .ok_or("NOTHING IT CAN SHOOT AT")?;
            let span = weapon.range_max - weapon.range_min;
            let spots = [
                (Fx::ratio(35, 100), -28),
                (Fx::ratio(65, 100), 0),
                (Fx::ratio(92, 100), 28),
            ];
            let commands = spots
                .iter()
                .map(|(share, across)| Command::DebugSpawn {
                    owner: RED,
                    blueprint: dummy.id,
                    pos: east(weapon.range_min + span * *share, *across),
                    heading: Angle::HALF_TURN,
                    count: 1,
                    flags: flag::PASSIVE | flag::INVULNERABLE,
                    build: 1000,
                })
                .collect();
            Ok((commands, None))
        }
        Scenario::PointBlank => {
            let weapon = bp.weapons.first().ok_or("THIS UNIT IS UNARMED")?;
            let dummy = blueprints
                .units
                .iter()
                .filter(|d| {
                    d.is_mobile()
                        && !d.has(cat::COMMANDER)
                        && d.categories & weapon.target_mask != 0
                })
                .min_by_key(|d| (d.key != DEFAULT_SUBJECT, d.tech))
                .ok_or("NOTHING IT CAN SHOOT AT")?;
            let distance = (weapon.range_min + bp.radius + dummy.radius + Fx::from_int(12))
                .min(weapon.range_max);
            Ok((
                vec![Command::DebugSpawn {
                    owner: RED,
                    blueprint: dummy.id,
                    pos: east(distance, 14),
                    heading: Angle::HALF_TURN,
                    count: 1,
                    flags: flag::PASSIVE | flag::INVULNERABLE,
                    build: 1000,
                }],
                None,
            ))
        }
        Scenario::BuildIt => {
            let builds = |b: &&UnitBlueprint| {
                b.builder
                    .as_ref()
                    .is_some_and(|k| k.builds.contains(&subject))
                    && !b.has(cat::COMMANDER)
            };
            let builder = blueprints
                .units
                .iter()
                .filter(builds)
                .min_by_key(|b| b.tech)
                .ok_or("NOTHING BUILDS THIS")?;
            let north = pad + FxVec2::from_ints(0, 160);
            let order = if bp.is_structure() {
                PendingOrder::Build {
                    blueprint: subject,
                    pos: north,
                }
            } else {
                PendingOrder::Produce {
                    blueprint: subject,
                    count: 3,
                    rally: pad + FxVec2::from_ints(90, 90),
                }
            };
            let at = if builder.is_structure() {
                north
            } else {
                pad + FxVec2::from_ints(-30, 110)
            };
            Ok((
                vec![Command::DebugSpawn {
                    owner: BLUE,
                    blueprint: builder.id,
                    pos: at,
                    heading: Angle::from_degrees(270),
                    count: 1,
                    flags: 0,
                    build: 1000,
                }],
                Some((builder.id, order)),
            ))
        }
        // The rest are orders to the subject itself, which is already standing on the pad.
        Scenario::AtWork => {
            let first = bp
                .builder
                .as_ref()
                .filter(|_| bp.is_mobile())
                .and_then(|b| b.builds.first())
                .ok_or("THIS UNIT BUILDS NOTHING")?;
            Ok((
                Vec::new(),
                Some((
                    subject,
                    PendingOrder::Build {
                        blueprint: *first,
                        pos: pad + FxVec2::from_ints(36, 44),
                    },
                )),
            ))
        }
        Scenario::Refit => bp
            .upgrades_to
            .map(|_| (Vec::new(), Some((subject, PendingOrder::Upgrade))))
            .ok_or("THIS UNIT HAS NO UPGRADE"),
        Scenario::March => bp
            .motion
            .map(|_| {
                (
                    Vec::new(),
                    Some((
                        subject,
                        PendingOrder::Move {
                            pos: east(Fx::from_int(260), 0),
                        },
                    )),
                )
            })
            .ok_or("THIS UNIT DOES NOT MOVE"),
        Scenario::Destruct => Ok((Vec::new(), Some((subject, PendingOrder::Destruct)))),
    }
}

/// The blueprint `step` places along from `from`, wrapping.
pub fn step_subject(blueprints: &Blueprints, from: BlueprintId, step: i32) -> BlueprintId {
    let n = blueprints.units.len() as i32;
    BlueprintId((from.0 as i32 + step).rem_euclid(n.max(1)) as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blueprints() -> Blueprints {
        Blueprints::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"))
            .unwrap()
    }

    /// Every scenario either stages or says why not, for every unit there is.
    #[test]
    fn every_unit_can_be_the_subject() {
        let b = blueprints();
        let pad = FxVec2::from_ints(2000, 2000);
        for bp in &b.units {
            for s in Scenario::ALL {
                match stage(s, &b, pad, bp.id) {
                    Ok((commands, owed)) => assert!(
                        !commands.is_empty() || owed.is_some(),
                        "{} / {s:?} staged nothing",
                        bp.key
                    ),
                    Err(why) => assert!(!why.is_empty()),
                }
            }
        }
        let tank = b.id_of(DEFAULT_SUBJECT).unwrap();
        let cannot: Vec<Scenario> = Scenario::ALL
            .into_iter()
            .filter(|s| stage(*s, &b, pad, tank).is_err())
            .collect();
        assert_eq!(
            cannot,
            [Scenario::AtWork, Scenario::Refit],
            "the default subject supports everything a tank can do"
        );
        let commander = b.unit_by_key("aster_commander").unwrap().id;
        let cannot: Vec<Scenario> = Scenario::ALL
            .into_iter()
            .filter(|s| stage(*s, &b, pad, commander).is_err())
            .collect();
        assert_eq!(
            cannot,
            [Scenario::BuildIt],
            "nothing builds a commander; it does everything else"
        );
    }

    #[test]
    fn attackers_and_targets_stand_inside_weapon_range() {
        let b = blueprints();
        let pad = FxVec2::from_ints(2000, 2000);
        let tank = b.unit(b.id_of(DEFAULT_SUBJECT).unwrap());
        let (commands, _) = stage(Scenario::Targets, &b, pad, tank.id).unwrap();
        for c in commands {
            let Command::DebugSpawn { pos, .. } = c else {
                panic!()
            };
            assert!(pos.distance(pad) < tank.weapons[0].range_max);
        }
    }
}
