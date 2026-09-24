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
/// The shares of its income a side can be given, thousandths: none, a shortage, normal, a glut.
pub const INCOME_STEPS: [u16; 8] = [0, 100, 250, 500, 1000, 2000, 5000, 20000];
/// Where `INCOME_STEPS` is normal.
pub const INCOME_NORMAL: usize = 4;

/// Stores a range side gets on top of its units', as multiples of a commander's
/// (`BASE_STORAGE`), so the stock controls work whatever the subject is.
pub const STORAGE_STEPS: [u32; 4] = [0, 1, 5, 25];
pub const BASE_STORAGE: [u32; 2] = [1000, 5000];
/// Where `STORAGE_STEPS` starts.
pub const STORAGE_DEFAULT: usize = 1;

/// `INCOME_STEPS[i]` as the panel shows it.
pub fn income_label(i: usize) -> String {
    let permille = INCOME_STEPS[i.min(INCOME_STEPS.len() - 1)];
    if permille > 1000 {
        format!("\u{d7}{}", permille / 1000)
    } else {
        format!("{}%", permille / 10)
    }
}

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
            Side::Blue => "Blue",
            Side::Red => "Red",
            Side::Dummy => "Dummy",
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

/// Which units the spawn stepper walks through. The subject still walks all of them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Roster {
    Any,
    Mobile,
    Structure,
}

impl Roster {
    pub const ALL: [Roster; 3] = [Roster::Any, Roster::Mobile, Roster::Structure];

    pub fn label(self) -> &'static str {
        match self {
            Roster::Any => "All",
            Roster::Mobile => "Mobile",
            Roster::Structure => "Struct",
        }
    }

    pub fn admits(self, bp: &UnitBlueprint) -> bool {
        match self {
            Roster::Any => true,
            Roster::Mobile => bp.is_mobile(),
            Roster::Structure => bp.is_structure(),
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
    /// A field of wrecks beside a reclaimer, for it (or its drones) to salvage.
    Salvage,
    /// The subject destroys itself.
    Destruct,
    /// A lift ship: a column of tanks behind the pad boards it, and it comes down for them.
    Lift,
}

impl Scenario {
    pub const ALL: [Scenario; 10] = [
        Scenario::UnderFire,
        Scenario::PointBlank,
        Scenario::Targets,
        Scenario::BuildIt,
        Scenario::AtWork,
        Scenario::Salvage,
        Scenario::Refit,
        Scenario::March,
        Scenario::Destruct,
        Scenario::Lift,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Scenario::UnderFire => "Attacked",
            Scenario::Targets => "Targets",
            Scenario::BuildIt => "Build It",
            Scenario::PointBlank => "Close in",
            Scenario::AtWork => "At Work",
            Scenario::Refit => "Upgrade",
            Scenario::March => "March",
            Scenario::Destruct => "Destruct",
            Scenario::Salvage => "Salvage",
            Scenario::Lift => "Lift",
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
            "salvage" => Scenario::Salvage,
            "lift" => Scenario::Lift,
            _ => return None,
        })
    }
}

/// What the range panel asks for. `game.rs` turns these into commands.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum RangeAction {
    /// Step the subject through the blueprints.
    Subject(i32),
    /// Pick a subject directly from the unit browser.
    PickSubject(BlueprintId),
    /// Step what the pointer places, without resetting the range.
    Spawn(i32),
    /// Pick a placement type without clearing the range.
    PickSpawn(BlueprintId),
    /// Restrict the spawn stepper to this set of units.
    Roster(Roster),
    Count(i32),
    Side(Side),
    /// Arm the pointer: the next click copies the current selection onto that ground.
    ArmSpawn,
    /// Arm the pointer: the next click places the current subject.
    ArmSubject,
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
    /// New weather for the range.
    Sky(RangeSky),
    /// Set a side's stores to thousandths of what they hold; `None` leaves one as it is.
    Stock {
        player: u8,
        mass: Option<u16>,
        energy: Option<u16>,
    },
    /// Step a side's income share (`resource` 0 materials, 1 energy) through `INCOME_STEPS`.
    Income { player: u8, resource: usize, step: i32 },
    /// Put a side's income share at `INCOME_STEPS[index]` outright.
    SetIncome { player: u8, resource: usize, index: usize },
    /// Step a side's extra stores through `STORAGE_STEPS`.
    Storage { player: u8, step: i32 },
    /// A field of wrecks beside the pad, for engineers to reclaim.
    Wrecks,
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
    /// Fit the module this kit assembles.
    Refit(BlueprintId),
    Move {
        pos: FxVec2,
    },
    Destruct,
    /// Board the lift ship of this blueprint (the subject): given to every unit of the
    /// builder's type at once, not only the first.
    Board(BlueprintId),
}

impl PendingOrder {
    /// `Board`: every unit among `units` of `builder`'s type walks up the ramp of the
    /// first `carrier` among them. Empty for any other order, or if either is missing.
    fn board(
        &self,
        builder: BlueprintId,
        units: &[(BlueprintId, mc_sim::UnitId)],
    ) -> Option<Vec<Command>> {
        let PendingOrder::Board(carrier) = *self else {
            return None;
        };
        let ship = units.iter().find(|(bp, _)| *bp == carrier)?.1;
        let riders: Vec<_> = units.iter().filter(|(bp, _)| *bp == builder).map(|(_, id)| *id).collect();
        Some(vec![Command::Board {
            units: riders,
            carrier: ship,
            queue: false,
        }])
    }
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
            PendingOrder::Refit(kit) => vec![Command::Refit { units: who, kit }],
            PendingOrder::Move { pos } => vec![Command::Move {
                units: who,
                target: pos,
                queue: false,
            }],
            PendingOrder::Destruct => vec![Command::SelfDestruct { units: who }],
            PendingOrder::Board(_) => Vec::new(),
        }
    }
}

pub struct Range {
    /// Where the subject stands: the first start position's flat ground.
    pub pad: FxVec2,
    pub subject: BlueprintId,
    /// Same blueprint as the subject. The pointer places copies of the selection,
    /// so this only labels the ghost when nothing is selected.
    pub spawn: BlueprintId,
    pub roster: Roster,
    /// Index into `COUNTS`.
    pub count: usize,
    pub side: Side,
    pub free_build: bool,
    /// Each side's income share, as indices into `INCOME_STEPS`: `[side][materials, energy]`.
    pub income: [[usize; 2]; 2],
    /// Each side's extra stores, as indices into `STORAGE_STEPS`.
    pub storage: [usize; 2],
    pending: Option<Pending>,
    /// The range's weather and whether a storm is parked over the pad; none
    /// until the game has read them from the settings.
    pub sky: Option<RangeSky>,
    /// What the renderer was last given, so a change is applied once.
    pub sky_applied: Option<RangeSky>,
}

/// The range's own weather, kept in the settings between runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct RangeSky {
    pub choice: mc_data::weather::SkyChoice,
    /// A raging storm parked over the pad, to see rain and lightning at once.
    pub storm_overhead: bool,
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
            spawn: subject,
            roster: Roster::Any,
            count: 0,
            side: Side::Blue,
            free_build: true,
            income: [[INCOME_NORMAL; 2]; 2],
            storage: [STORAGE_DEFAULT; 2],
            pending: None,
            sky: None,
            sky_applied: None,
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
                label: format!("Selection  \u{b7}  {}", selection.len()),
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
        let name = blueprints.unit(self.subject).name.clone();
        Acted {
            label: format!(
                "Every {} {name}  \u{b7}  {}",
                if side == BLUE { "Blue" } else { "Red" },
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
        out.extend([BLUE, RED].map(|player| self.income_command(player)));
        out.extend([BLUE, RED].map(|player| self.storage_command(player)));
        out
    }

    /// What `player`'s extra stores are set to, as a command.
    pub fn storage_command(&self, player: u8) -> Command {
        storage_command(player, self.storage[player.min(1) as usize])
    }

    /// What `player`'s income share is set to, as a command.
    pub fn income_command(&self, player: u8) -> Command {
        let [mass, energy] = self.income[player.min(1) as usize];
        Command::DebugIncome {
            player,
            mass: INCOME_STEPS[mass],
            energy: INCOME_STEPS[energy],
        }
    }

    /// A block of wrecks south of the pad, of the subject if it leaves one, else of the medium tank.
    pub fn wrecks(&self, blueprints: &Blueprints) -> Option<Command> {
        let leaves = |id: BlueprintId| {
            let bp = blueprints.unit(id);
            bp.cost_mass * bp.wreck_fraction > Fx::ZERO
        };
        let blueprint = Some(self.subject)
            .filter(|&id| leaves(id))
            .or_else(|| blueprints.id_of(DEFAULT_SUBJECT).filter(|&id| leaves(id)))?;
        Some(Command::DebugWrecks {
            blueprint,
            pos: self.pad + FxVec2::from_ints(30, -80),
            count: self.count().max(6),
        })
    }

    /// Copies of the selected units, laid out as they stand, centred on `pos`.
    /// One selected unit is repeated `count` times on that point.
    pub fn duplicate_at(
        &self,
        pos: FxVec2,
        units: &[UnitInstance],
        selection: &[u32],
        blueprints: &Blueprints,
    ) -> Vec<Command> {
        let picked: Vec<&UnitInstance> = selection
            .iter()
            .filter_map(|id| units.iter().find(|u| u.unit_id == *id))
            .filter(|u| u.owner_flags & KIND_WRECK == 0)
            .collect();
        if picked.is_empty() {
            return Vec::new();
        }
        let (owner, flags) = self.side.owner_flags();
        if picked.len() == 1 {
            let blueprint = BlueprintId(picked[0].blueprint as u16);
            return vec![Command::DebugSpawn {
                owner,
                blueprint,
                pos,
                heading: self.heading_of(blueprints, blueprint, pos),
                count: self.count(),
                flags,
                build: (picked[0].build.clamp(0.0, 1.0) * 1000.0).round() as u16,
            }];
        }
        let n = picked.len() as f32;
        let cx = picked.iter().map(|u| u.pos[0]).sum::<f32>() / n;
        let cy = picked.iter().map(|u| u.pos[1]).sum::<f32>() / n;
        picked
            .into_iter()
            .map(|u| {
                let at =
                    pos + FxVec2::new(Fx::from_f32(u.pos[0] - cx), Fx::from_f32(u.pos[1] - cy));
                let blueprint = BlueprintId(u.blueprint as u16);
                Command::DebugSpawn {
                    owner,
                    blueprint,
                    pos: at,
                    heading: self.heading_of(blueprints, blueprint, at),
                    count: 1,
                    flags,
                    build: (u.build.clamp(0.0, 1.0) * 1000.0).round() as u16,
                }
            })
            .collect()
    }

    /// Structures share the facing a player build uses. Mobile units face the pad
    /// when spawned off it for the red side, and along +X otherwise.
    pub fn heading_of(&self, blueprints: &Blueprints, blueprint: BlueprintId, pos: FxVec2) -> Angle {
        if blueprints.unit(blueprint).is_structure() {
            Angle::from_degrees(270)
        } else {
            self.heading_at(pos)
        }
    }

    fn heading_at(&self, pos: FxVec2) -> Angle {
        if self.side == Side::Blue || pos == self.pad {
            Angle::ZERO
        } else {
            (self.pad - pos).angle()
        }
    }

    pub fn spawn_at(&self, pos: FxVec2, blueprints: &Blueprints) -> Command {
        let (owner, flags) = self.side.owner_flags();
        let heading = self.heading_of(blueprints, self.subject, pos);
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
        let blue: Vec<(BlueprintId, mc_sim::UnitId)> = units
            .iter()
            .filter(|u| u.owner_flags & (KIND_WRECK | 0xFF) == BLUE as u32 && u.build >= 1.0)
            .map(|u| (BlueprintId(u.blueprint as u16), Handle(u.unit_id)))
            .collect();
        let out = p
            .order
            .board(p.builder, &blue)
            .unwrap_or_else(|| p.order.commands(vec![Handle(unit.unit_id)]));
        self.pending = None;
        // Building is blue's business whichever side was being steered.
        std::iter::once(Command::DebugControl { player: BLUE })
            .chain(out)
            .collect()
    }
}

fn storage_command(player: u8, step: usize) -> Command {
    let k = STORAGE_STEPS[step.min(STORAGE_STEPS.len() - 1)];
    Command::DebugStorage {
        player,
        mass: BASE_STORAGE[0] * k,
        energy: BASE_STORAGE[1] * k,
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
    out.extend([BLUE, RED].map(|player| storage_command(player, STORAGE_DEFAULT)));
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
    if let Some(board) = order.board(builder, blue_units) {
        return board;
    }
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
        .or_else(|| {
            blueprints
                .units
                .iter()
                .filter(|bp| blueprints.is_listed(bp.id))
                .filter(can)
                .min_by_key(|bp| bp.tech)
        })
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
            let attacker = attacker_for(blueprints, bp).ok_or("Nothing can shoot at this")?;
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
            let weapon = bp.weapons.first().ok_or("This Unit Is Unarmed")?;
            let dummy = blueprints
                .units
                .iter()
                .filter(|d| {
                    d.is_mobile()
                        && !d.has(cat::COMMANDER)
                        && d.categories & weapon.target_mask != 0
                })
                .min_by_key(|d| (d.key != DEFAULT_SUBJECT, d.tech))
                .ok_or("Nothing it can shoot at")?;
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
            let weapon = bp.weapons.first().ok_or("This Unit Is Unarmed")?;
            let dummy = blueprints
                .units
                .iter()
                .filter(|d| {
                    d.is_mobile()
                        && !d.has(cat::COMMANDER)
                        && d.categories & weapon.target_mask != 0
                })
                .min_by_key(|d| (d.key != DEFAULT_SUBJECT, d.tech))
                .ok_or("Nothing it can shoot at")?;
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
                .ok_or("Nothing Builds This")?;
            let north = pad + FxVec2::from_ints(0, 160);
            let order = if bp.built_on_site() {
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
                .ok_or("This Unit Builds Nothing")?;
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
        // A unit with refit slots fits its first module; any other takes its next tier.
        Scenario::Refit => match blueprints.refit_set(bp.id) {
            Some(set) => set
                .slots
                .iter()
                .flat_map(|s| &s.modules)
                .find(|m| blueprints.refit_result(bp.id, m.kit).is_ok())
                .map(|m| (Vec::new(), Some((subject, PendingOrder::Refit(m.kit)))))
                .ok_or("Nothing more fits this unit"),
            None => bp
                .upgrades_to
                .map(|_| (Vec::new(), Some((subject, PendingOrder::Upgrade))))
                .ok_or("This unit has no upgrade"),
        },
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
            .ok_or("This unit does not move"),
        Scenario::Destruct => Ok((Vec::new(), Some((subject, PendingOrder::Destruct)))),
        Scenario::Lift => {
            // A column of tanks behind the pad (the ship's stern, as it faces east), told
            // to board: the ship comes down out of the clouds for them.
            if bp.transport.is_none() {
                return Err("This Unit Carries Nothing");
            }
            let tank = blueprints.id_of(DEFAULT_SUBJECT).ok_or("No Tank To Carry")?;
            let mut spawns = vec![Command::DebugSpawn {
                owner: BLUE,
                blueprint: tank,
                pos: pad - FxVec2::from_ints(150, 0),
                heading: Angle::ZERO,
                count: 6,
                flags: 0,
                build: 1000,
            }];
            // `MERIDIAN_LIFT_FOES=1` (headless checks): enemy tanks to either side and ahead,
            // inside the ship's gun reach once it is down, so all four guns open up.
            if std::env::var_os("MERIDIAN_LIFT_FOES").is_some() {
                for (x, y) in [(260, 0), (-40, 200), (-40, -200), (-320, 60)] {
                    spawns.push(Command::DebugSpawn {
                        owner: RED,
                        blueprint: tank,
                        pos: pad + FxVec2::from_ints(x, y),
                        heading: Angle::ZERO,
                        count: 1,
                        flags: 0,
                        build: 1000,
                    });
                }
            }
            Ok((spawns, Some((tank, PendingOrder::Board(subject)))))
        }
        Scenario::Salvage => {
            // Wrecks of the medium tank a short way east, inside a carrier's drone reach
            // and a builder's walk, for a reclaimer to get to work on.
            if bp.reclaimer.is_none() && bp.drone.is_none() && bp.builder.is_none() {
                return Err("This Unit Does Not Reclaim");
            }
            let wreck = blueprints
                .id_of(DEFAULT_SUBJECT)
                .filter(|&id| {
                    let w = blueprints.unit(id);
                    w.cost_mass * w.wreck_fraction > Fx::ZERO
                })
                .ok_or("Nothing Leaves A Wreck")?;
            Ok((
                vec![Command::DebugWrecks {
                    blueprint: wreck,
                    pos: pad + FxVec2::from_ints(48, 0),
                    count: 6,
                }],
                None,
            ))
        }
    }
}

/// The blueprint `step` places along from `from`, wrapping.
pub fn step_subject(blueprints: &Blueprints, from: BlueprintId, step: i32) -> BlueprintId {
    step_in(blueprints, from, step, Roster::Any)
}

/// `from` stepped through the units `roster` admits. A `from` that is not in
/// the set lands on the first of it when `step` is 0, then walks from there.
pub fn step_in(
    blueprints: &Blueprints,
    from: BlueprintId,
    step: i32,
    roster: Roster,
) -> BlueprintId {
    let ids: Vec<BlueprintId> = blueprints
        .units
        .iter()
        .filter(|u| roster.admits(u))
        .map(|u| u.id)
        .collect();
    if ids.is_empty() {
        return from;
    }
    let at = ids.iter().position(|&id| id == from).unwrap_or(0);
    let n = ids.len() as i32;
    ids[(at as i32 + step).rem_euclid(n.max(1)) as usize]
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
        for bp in b.units.iter().filter(|bp| b.is_listed(bp.id)) {
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
            [Scenario::AtWork, Scenario::Salvage, Scenario::Refit, Scenario::Lift],
            "the default subject supports everything a tank can do"
        );
        let commander = b.unit_by_key("aster_commander").unwrap().id;
        let cannot: Vec<Scenario> = Scenario::ALL
            .into_iter()
            .filter(|s| stage(*s, &b, pad, commander).is_err())
            .collect();
        assert_eq!(
            cannot,
            [Scenario::BuildIt, Scenario::Lift],
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

    #[test]
    fn spawn_places_the_chosen_type_not_the_subject() {
        let b = blueprints();
        let tank = b.id_of(DEFAULT_SUBJECT).unwrap();
        let shield = b.id_of("aster_t2_shield").unwrap();
        let mut range = Range::new(FxVec2::from_ints(2000, 2000), tank);
        range.spawn = shield;
        let Command::DebugSpawn { blueprint, .. } = range.spawn_at(range.pad, &b) else {
            panic!()
        };
        assert_eq!(blueprint, shield);
        assert_eq!(range.subject, tank);
    }

    #[test]
    fn stepping_stays_inside_the_roster() {
        let b = blueprints();
        let tank = b.id_of(DEFAULT_SUBJECT).unwrap();
        let mut id = tank;
        for _ in 0..b.units.len() + 2 {
            id = step_in(&b, id, 1, Roster::Structure);
            assert!(
                b.unit(id).is_structure(),
                "{} left the structure roster",
                b.unit(id).key
            );
        }
        let first = step_in(&b, tank, 0, Roster::Structure);
        assert_eq!(
            first,
            b.units.iter().find(|u| u.is_structure()).unwrap().id,
            "a mobile unit snaps to the first structure"
        );
        assert_eq!(
            step_in(&b, tank, 0, Roster::Any),
            tank,
            "a unit already in the set stays put"
        );
    }
}
