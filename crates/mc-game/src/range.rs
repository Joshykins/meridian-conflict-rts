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
    /// Told to go off to the left: it swings round hard as it gathers way.
    Turn,
    /// A field of wrecks for a reclaimer (or its drones) to salvage: beside it, or most of the
    /// way out for a long reach, with a mobile one passing by it.
    Salvage,
    /// The subject destroys itself.
    Destruct,
    /// A lift ship: a column of tanks behind the pad boards it, and it comes down for them.
    Lift,
    /// A ship with a warp drive charges and jumps 1.8 km down the range.
    Warp,
    /// `Warp` into the field of a red warp dampener: dragged, then thrown out hurt and stunned.
    WarpDampened,
}

impl Scenario {
    pub const ALL: [Scenario; 13] = [
        Scenario::UnderFire,
        Scenario::PointBlank,
        Scenario::Targets,
        Scenario::BuildIt,
        Scenario::AtWork,
        Scenario::Salvage,
        Scenario::Refit,
        Scenario::March,
        Scenario::Turn,
        Scenario::Destruct,
        Scenario::Lift,
        Scenario::Warp,
        Scenario::WarpDampened,
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
            Scenario::Turn => "Turn",
            Scenario::Destruct => "Destruct",
            Scenario::Salvage => "Salvage",
            Scenario::Lift => "Lift",
            Scenario::Warp => "Warp",
            Scenario::WarpDampened => "Dampened",
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
            "turn" => Scenario::Turn,
            "destruct" => Scenario::Destruct,
            "salvage" => Scenario::Salvage,
            "lift" => Scenario::Lift,
            "warp" => Scenario::Warp,
            "warp-dampened" => Scenario::WarpDampened,
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
    Income {
        player: u8,
        resource: usize,
        step: i32,
    },
    /// Put a side's income share at `INCOME_STEPS[index]` outright.
    SetIncome {
        player: u8,
        resource: usize,
        index: usize,
    },
    /// Step a side's extra stores through `STORAGE_STEPS`.
    Storage {
        player: u8,
        step: i32,
    },
    /// A field of wrecks beside the pad, for engineers to reclaim.
    Wrecks,
}

/// An order that has to wait for a unit the range just spawned to show up.
struct Pending {
    builder: BlueprintId,
    /// Units that were already there, so the new one can be told apart.
    known: HashSet<u32>,
    order: PendingOrder,
    /// Ticks still to wait once it is there: a unit set to destruct stands a moment
    /// first, so its end can be watched (and filmed: `--unit-shot --frames`).
    wait: u32,
}

/// Ticks a subject stands before it destructs.
pub(crate) const DESTRUCT_WAIT: u32 = 10;

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
    Warp {
        pos: FxVec2,
    },
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
        let riders: Vec<_> = units
            .iter()
            .filter(|(bp, _)| *bp == builder)
            .map(|(_, id)| *id)
            .collect();
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
            PendingOrder::Warp { pos } => vec![Command::Warp {
                units: who,
                pos,
                queue: false,
            }],
            PendingOrder::Board(_) => Vec::new(),
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
    /// Each side's income share, as indices into `INCOME_STEPS`: `[side][materials, energy]`.
    pub income: [[usize; 2]; 2],
    /// Each side's extra stores, as indices into `STORAGE_STEPS`.
    pub storage: [usize; 2],
    pending: Option<Pending>,
    /// The range's weather and whether a storm is parked over the pad; none
    /// until the game has read them from the settings.
    pub sky: Option<RangeSky>,
}

/// The range's own weather, kept in the settings between runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct RangeSky {
    pub choice: mc_data::weather::SkyChoice,
    /// A raging storm parked over the pad, to see rain and lightning at once.
    pub storm_overhead: bool,
}

impl RangeSky {
    /// Puts this weather over the range on `map`.
    pub fn show(&self, renderer: &mut mc_render::Renderer, map: &mc_map::MapFile) {
        let config = crate::setup::map_config(map);
        renderer.set_weather(self.choice.weather(&config));
        renderer.set_hour(self.choice.hour(&config));
        let pad = crate::setup::range_pad(map).to_f32();
        renderer.park_storm(self.storm_overhead.then(|| glam::Vec2::from(pad)));
    }
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
            income: [[INCOME_NORMAL; 2]; 2],
            storage: [STORAGE_DEFAULT; 2],
            pending: None,
            sky: None,
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
    pub fn heading_of(
        &self,
        blueprints: &Blueprints,
        blueprint: BlueprintId,
        pos: FxVec2,
    ) -> Angle {
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
            let wait = if matches!(order, PendingOrder::Destruct) {
                DESTRUCT_WAIT
            } else {
                0
            };
            Pending {
                builder,
                known,
                order,
                wait,
            }
        });
        Ok(commands)
    }

    /// Once the builder a scenario spawned is on the map, the order it was spawned for.
    pub fn resolve_pending(&mut self, units: &[UnitInstance]) -> Vec<Command> {
        let Some(p) = &mut self.pending else {
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
        if p.wait > 0 {
            p.wait -= 1;
            return Vec::new();
        }
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
    if matches!(order, PendingOrder::Destruct) {
        // Given later, after `DESTRUCT_WAIT` (`setup::late_orders`).
        return Vec::new();
    }
    if let Some(board) = order.board(builder, blue_units) {
        return board;
    }
    let Some((_, id)) = blue_units.iter().find(|(bp, _)| *bp == builder) else {
        return Vec::new();
    };
    order.commands(vec![*id])
}

/// The lowest-tech armed mobile unit that can shoot at `target`, the medium tank if it can.
/// A missile defence is shot at with missiles, so it has something to burn.
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
    if target.anti_missile > Fx::ZERO {
        let missiles = |bp: &&UnitBlueprint| {
            can(bp)
                && bp
                    .motion
                    .is_some_and(|m| m.layer == mc_data::MoveLayer::Land)
                && bp
                    .weapons
                    .iter()
                    .any(|w| w.missile && w.target_mask & target.categories != 0)
        };
        let launcher = blueprints
            .units
            .iter()
            .filter(|bp| blueprints.is_listed(bp.id))
            .filter(missiles)
            .min_by_key(|bp| bp.tech);
        if launcher.is_some() {
            return launcher;
        }
    }
    let tank = blueprints
        .id_of(DEFAULT_SUBJECT)
        .map(|id| blueprints.unit(id));
    tank.filter(&can).or_else(|| {
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
                // A unit to shoot at, or a building for a gun that only shells buildings
                // (not a mine: one stands only on ore).
                .filter(|d| {
                    !d.has(cat::COMMANDER)
                        && !d.has(cat::EXTRACTOR)
                        && d.categories & weapon.target_mask != 0
                })
                .min_by_key(|d| {
                    // Of buildings, the smallest: the likeliest to find room to stand.
                    let small = d.is_structure().then_some(d.radius);
                    (!d.is_mobile(), d.key != DEFAULT_SUBJECT, d.tech, small)
                })
                .ok_or("Nothing it can shoot at")?;
            // A map gun's targets stand within 2 km past its dead zone: on the map, and
            // near enough to see the shells land.
            let span = match weapon.range_max - weapon.range_min {
                span if span > Fx::from_int(4000) => Fx::from_int(2000),
                span => span,
            };
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
                // A unit to shoot at, or a building for a gun that only shells buildings
                // (not a mine: one stands only on ore).
                .filter(|d| {
                    !d.has(cat::COMMANDER)
                        && !d.has(cat::EXTRACTOR)
                        && d.categories & weapon.target_mask != 0
                })
                .min_by_key(|d| {
                    // Of buildings, the smallest: the likeliest to find room to stand.
                    let small = d.is_structure().then_some(d.radius);
                    (!d.is_mobile(), d.key != DEFAULT_SUBJECT, d.tech, small)
                })
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
                            // Far enough for a giant to take a few strides too.
                            pos: east(Fx::from_int(260).max(bp.radius * 8), 0),
                        },
                    )),
                )
            })
            .ok_or("This unit does not move"),
        // Off to the left and a little ahead: it swings hard round while it gathers way.
        Scenario::Turn => bp
            .motion
            .map(|_| {
                let far = Fx::from_int(260).max(bp.radius * 8);
                (
                    Vec::new(),
                    Some((
                        subject,
                        PendingOrder::Move {
                            pos: pad + FxVec2::new(far / 3, far),
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
            let tank = blueprints
                .id_of(DEFAULT_SUBJECT)
                .ok_or("No Tank To Carry")?;
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
        Scenario::Warp | Scenario::WarpDampened => {
            // Stores enough for the drive's charge, then the jump; dampened, a red Undertow
            // on free power stands near the pad (land on every map's pad), its field over
            // where the ship comes out.
            let drive = bp.warp.ok_or("This Unit Has No Warp Drive")?;
            let exit = east(Fx::from_int(1800), 0);
            let mut spawns = vec![
                Command::DebugStorage {
                    player: BLUE,
                    mass: 0,
                    energy: (drive.energy * 2).ceil_int().max(0) as u32,
                },
                Command::DebugStock {
                    player: BLUE,
                    mass: None,
                    energy: Some(1000),
                },
            ];
            if scenario == Scenario::WarpDampened {
                let damper = blueprints
                    .units
                    .iter()
                    .find(|d| d.warp_damper.is_some() && blueprints.is_listed(d.id))
                    .ok_or("No Warp Dampener")?;
                spawns.push(Command::DebugFreeBuild {
                    player: RED,
                    on: true,
                });
                spawns.push(Command::DebugSpawn {
                    owner: RED,
                    blueprint: damper.id,
                    pos: east(Fx::from_int(600), 300),
                    heading: Angle::from_degrees(270),
                    count: 1,
                    flags: flag::PASSIVE | flag::INVULNERABLE,
                    build: 1000,
                });
            }
            Ok((spawns, Some((subject, PendingOrder::Warp { pos: exit }))))
        }
        Scenario::Salvage => {
            // Wrecks of the medium tank a short way east, inside a carrier's drone reach
            // and a builder's walk, for a reclaimer to get to work on. A reclaimer with a
            // long reach finds the wrecks of the heaviest land units most of the way out
            // (a tank's it would clear in a second), and one that moves drives (or flies)
            // north past them with its beams on.
            if bp.reclaimer.is_none() && bp.drone.is_none() && bp.builder.is_none() {
                return Err("This Unit Does Not Reclaim");
            }
            let reach = bp.reclaimer.map_or(Fx::ZERO, |r| r.range);
            let long = reach > Fx::from_int(160);
            let out = if long {
                reach * Fx::ratio(3, 4)
            } else {
                Fx::from_int(48)
            };
            let pass = (long && bp.motion.is_some()).then(|| {
                (
                    subject,
                    PendingOrder::Move {
                        pos: pad + FxVec2::new(Fx::ZERO, reach),
                    },
                )
            });
            let worth = |w: &UnitBlueprint| w.cost_mass * w.wreck_fraction;
            let heaviest = || {
                blueprints
                    .units
                    .iter()
                    .filter(|w| blueprints.is_listed(w.id) && w.is_mobile())
                    .filter(|w| {
                        w.motion
                            .is_some_and(|m| m.layer == mc_data::MoveLayer::Land)
                    })
                    .max_by_key(|w| (worth(w), w.id.0))
                    .map(|w| w.id)
            };
            let wreck = if long {
                heaviest()
            } else {
                blueprints.id_of(DEFAULT_SUBJECT)
            }
            .filter(|&id| worth(blueprints.unit(id)) > Fx::ZERO)
            .ok_or("Nothing Leaves A Wreck")?;
            Ok((
                vec![Command::DebugWrecks {
                    blueprint: wreck,
                    pos: east(out, 0),
                    count: if long { 3 } else { 6 },
                }],
                pass,
            ))
        }
    }
}

/// Every unit asked for, spawned in rows to look at: `Range::line_up`.
pub struct LineUp {
    pub commands: Vec<Command>,
    /// Middle of the block on land, and how far across it runs, metres.
    pub centre: FxVec2,
    pub span: f32,
    /// Ships and sea structures, moored at open sea apart from the rest.
    pub at_sea: usize,
    /// Ships and sea structures left out: no open sea near the pad.
    pub stranded: usize,
}

/// How wide a line-up row runs before it wraps, metres.
const LINE_UP_WIDTH: f32 = 900.0;

/// Half the ground a unit takes in a line-up, metres: its lot, or its hull.
fn line_up_half(bp: &UnitBlueprint) -> f32 {
    if bp.footprint.0 > 0 {
        bp.footprint.0.max(bp.footprint.1) as f32 * mc_map::BUILD_CELL_M as f32 * 0.5
    } else {
        bp.radius.to_f32()
    }
}

/// Lays `units` out in rows from the origin: one band per list, a new row at each tech.
/// Returns each unit's centre and the block's size.
fn line_up_block(bands: &[Vec<&UnitBlueprint>]) -> (Vec<(BlueprintId, [f32; 2])>, [f32; 2]) {
    let mut out = Vec::new();
    let (mut y, mut width) = (0.0f32, 0.0f32);
    for band in bands.iter().filter(|b| !b.is_empty()) {
        let (mut x, mut row_h, mut tech) = (0.0f32, 0.0f32, band[0].tech);
        for bp in band {
            // Lots snap to the build grid, so structures keep a cell clear either side.
            let gap = if bp.is_structure() {
                2.0 * mc_map::BUILD_CELL_M as f32
            } else {
                12.0
            };
            let cell = 2.0 * line_up_half(bp) + gap;
            if x > 0.0 && (bp.tech != tech || x + cell > LINE_UP_WIDTH) {
                y += row_h;
                x = 0.0;
                row_h = 0.0;
            }
            tech = bp.tech;
            out.push((bp.id, [x + cell * 0.5, y + cell * 0.5]));
            x += cell;
            row_h = row_h.max(cell);
            width = width.max(x);
        }
        y += row_h + 40.0;
    }
    (out, [width, (y - 40.0).max(0.0)])
}

impl Range {
    /// Clears the range and lines up one of each of `units` for Blue: land units, then
    /// aircraft, then structures, in rows by tech, centred on the pad and kept inside
    /// `bounds`. Ships and sea structures moor at `sea` when there is one.
    pub fn line_up(
        &mut self,
        blueprints: &Blueprints,
        units: &[BlueprintId],
        sea: Option<FxVec2>,
        bounds: FxVec2,
    ) -> LineUp {
        self.pending = None;
        let mut picked: Vec<&UnitBlueprint> = units.iter().map(|&id| blueprints.unit(id)).collect();
        picked.sort_by(|a, b| (a.tech, &a.name, &a.key).cmp(&(b.tech, &b.name, &b.key)));
        let layer = |bp: &UnitBlueprint| bp.motion.map(|m| m.layer);
        let wet =
            |bp: &UnitBlueprint| bp.water_only() || layer(bp) == Some(mc_data::MoveLayer::Naval);
        let band = |f: &dyn Fn(&UnitBlueprint) -> bool| -> Vec<&UnitBlueprint> {
            picked.iter().copied().filter(|bp| f(bp)).collect()
        };
        let land = [
            band(&|bp| bp.is_mobile() && !wet(bp) && layer(bp) != Some(mc_data::MoveLayer::Air)),
            band(&|bp| layer(bp) == Some(mc_data::MoveLayer::Air)),
            band(&|bp| !bp.is_mobile() && !wet(bp)),
        ];
        let sea_units = [
            band(&|bp| wet(bp) && bp.is_mobile()),
            band(&|bp| wet(bp) && !bp.is_mobile()),
        ];
        let wet_count = sea_units.iter().map(Vec::len).sum::<usize>();

        let (w, h) = (bounds.x.to_f32(), bounds.y.to_f32());
        // The block's top-left corner, so it is centred on `at` and inside the map.
        let place = |at: FxVec2, size: [f32; 2]| {
            let [cx, cy] = at.to_f32();
            let fit =
                |c: f32, s: f32, edge: f32| (c - s * 0.5).clamp(30.0, (edge - s - 30.0).max(30.0));
            [fit(cx, size[0], w), fit(cy, size[1], h)]
        };
        let mut commands = vec![Command::DebugClear, Command::DebugControl { player: BLUE }];
        // Rows run down the screen (north is up), so the first band is the top one.
        let mut spawn = |cells: Vec<(BlueprintId, [f32; 2])>, corner: [f32; 2], size: [f32; 2]| {
            for (blueprint, [x, y]) in cells {
                let structure = blueprints.unit(blueprint).is_structure();
                commands.push(Command::DebugSpawn {
                    owner: BLUE,
                    blueprint,
                    pos: FxVec2::new(
                        Fx::from_f32(corner[0] + x),
                        Fx::from_f32(corner[1] + size[1] - y),
                    ),
                    heading: Angle::from_degrees(if structure { 270 } else { 0 }),
                    count: 1,
                    flags: 0,
                    build: 1000,
                });
            }
        };
        let (cells, size) = line_up_block(&land);
        let corner = place(self.pad, size);
        spawn(cells, corner, size);
        let at_sea = match sea {
            Some(sea) if wet_count > 0 => {
                let (cells, size) = line_up_block(&sea_units);
                spawn(cells, place(sea, size), size);
                wet_count
            }
            _ => 0,
        };
        LineUp {
            commands,
            centre: FxVec2::new(
                Fx::from_f32(corner[0] + size[0] * 0.5),
                Fx::from_f32(corner[1] + size[1] * 0.5),
            ),
            span: size[0].max(size[1]),
            at_sea,
            stranded: wet_count - at_sea,
        }
    }
}

/// The blueprint `step` places along from `from`, wrapping. A `from` that is
/// not a unit lands on the first one when `step` is 0, then walks from there.
pub fn step_subject(blueprints: &Blueprints, from: BlueprintId, step: i32) -> BlueprintId {
    let ids: Vec<BlueprintId> = blueprints.units.iter().map(|u| u.id).collect();
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
            [
                Scenario::AtWork,
                Scenario::Salvage,
                Scenario::Refit,
                Scenario::Lift,
                Scenario::Warp,
                Scenario::WarpDampened
            ],
            "the default subject supports everything a tank can do"
        );
        let commander = b.unit_by_key("aster_commander").unwrap().id;
        let cannot: Vec<Scenario> = Scenario::ALL
            .into_iter()
            .filter(|s| stage(*s, &b, pad, commander).is_err())
            .collect();
        assert_eq!(
            cannot,
            [
                Scenario::BuildIt,
                Scenario::Lift,
                Scenario::Warp,
                Scenario::WarpDampened
            ],
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
    fn a_map_gun_gets_targets_a_few_kilometres_out() {
        let b = blueprints();
        let pad = FxVec2::from_ints(2000, 2000);
        let gun = b.unit(b.id_of("aster_t4_artillery").unwrap());
        let (commands, _) = stage(Scenario::Targets, &b, pad, gun.id).unwrap();
        assert!(!commands.is_empty());
        for c in commands {
            let Command::DebugSpawn { pos, blueprint, .. } = c else {
                panic!()
            };
            assert!(b.unit(blueprint).categories & gun.weapons[0].target_mask != 0);
            let gap = pos.distance(pad);
            assert!(gap > gun.weapons[0].range_min && gap < Fx::from_int(4000));
        }
    }

    #[test]
    fn spawn_subject_places_the_subject() {
        let b = blueprints();
        let shield = b.id_of("aster_t2_shield").unwrap();
        assert_ne!(shield, b.id_of(DEFAULT_SUBJECT).unwrap());
        let range = Range::new(FxVec2::from_ints(2000, 2000), shield);
        let Command::DebugSpawn { blueprint, .. } = range.spawn_at(range.pad, &b) else {
            panic!()
        };
        assert_eq!(blueprint, shield);
    }

    #[test]
    fn line_up_places_each_unit_once_inside_the_map() {
        let b = blueprints();
        let tank = b.id_of(DEFAULT_SUBJECT).unwrap();
        let mut range = Range::new(FxVec2::from_ints(100, 100), tank);
        let all: Vec<BlueprintId> = b
            .units
            .iter()
            .filter(|u| b.is_listed(u.id))
            .map(|u| u.id)
            .collect();
        let bounds = FxVec2::from_ints(4096, 4096);
        let wet = |id: BlueprintId| {
            let u = b.unit(id);
            u.water_only()
                || u.motion
                    .is_some_and(|m| m.layer == mc_data::MoveLayer::Naval)
        };
        let shown = |l: &LineUp| -> Vec<(BlueprintId, [f32; 2])> {
            l.commands
                .iter()
                .filter_map(|c| match c {
                    Command::DebugSpawn {
                        blueprint,
                        pos,
                        count: 1,
                        owner: BLUE,
                        ..
                    } => Some((*blueprint, pos.to_f32())),
                    Command::DebugSpawn { .. } => panic!("one Blue unit each"),
                    _ => None,
                })
                .collect()
        };
        let dry = range.line_up(&b, &all, None, bounds);
        assert!(matches!(dry.commands[0], Command::DebugClear));
        let placed = shown(&dry);
        assert_eq!(dry.stranded, all.iter().filter(|&&id| wet(id)).count());
        assert_eq!(placed.len() + dry.stranded, all.len());
        // Near the map's corner, the block is pushed back inside it.
        assert!(placed
            .iter()
            .all(|(_, [x, y])| *x > 0.0 && *y > 0.0 && *x < 4096.0 && *y < 4096.0));
        for (i, (_, p)) in placed.iter().enumerate() {
            for (_, q) in &placed[i + 1..] {
                assert!(
                    (p[0] - q[0]).hypot(p[1] - q[1]) > 4.0,
                    "two units on one spot"
                );
            }
        }
        let wet_sea = range.line_up(&b, &all, Some(FxVec2::from_ints(3000, 3000)), bounds);
        assert_eq!(wet_sea.stranded, 0);
        assert_eq!(shown(&wet_sea).len(), all.len());
    }
}
