//! `World`: state, inputs, derived structures, and the tick pipeline.

use crate::ai::AiState;
use crate::command::PlayerCommand;
use crate::fog::Fog;
use crate::mirror::SimEvent;
use crate::nav::Nav;
use crate::spatial::{kind, SpatialIndex, NO_OWNER};
use crate::tables::*;
use crate::{SimError, Table};
use mc_core::{player_bit, Angle, Fx, FxVec2, PlayerMask, Rng, MAX_PLAYERS};
use mc_data::{cat, BlueprintId, Blueprints, MoveLayer, UnitBlueprint};
use mc_jobs::Pool;
use mc_map::{Heightfield, MapFile, Prop};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerSetup {
    pub name: String,
    /// Faction key, e.g. "Aster".
    pub faction: String,
    #[serde(default)]
    pub ai: crate::AiConfig,
    pub team: u8,
    pub controller: Controller,
    /// Index into the map's start positions.
    pub start: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchConfig {
    pub seed: u64,
    pub players: Vec<PlayerSetup>,
    /// Enables `Command::DebugSpawn`. Test scenes and tools only.
    pub cheats: bool,
    /// With fog off everyone detects everything (test scenes, observers' sims still run fog).
    pub fog: bool,
    /// Spawn commanders at the start positions. Off for empty test scenes.
    pub spawn_commanders: bool,
}

/// A terrain edit in table form; mirrors `mc_map::FlattenRecord`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerrainEdit {
    pub min: (u16, u16),
    /// Inclusive.
    pub max: (u16, u16),
    pub sample: u16,
    /// The faction (`mc_data::FactionId`) of the structure the lot was levelled for:
    /// the renderer clads the slopes round it in that faction's foundation walls.
    pub faction: u8,
}

impl TerrainEdit {
    pub fn record(&self) -> mc_map::FlattenRecord {
        mc_map::FlattenRecord {
            min_x: self.min.0,
            min_y: self.min.1,
            max_x: self.max.0,
            max_y: self.max.1,
            sample: self.sample,
        }
    }
}

/// Everything a snapshot carries. If it is not reachable from here, it is not game state.
#[derive(Clone, Serialize, Deserialize)]
pub struct State {
    pub tick: u32,
    pub rng: Rng,
    pub cheats: bool,
    pub fog_enabled: bool,
    pub players: Vec<Player>,
    pub units: Units,
    pub orders: Orders,
    pub formation_serial: u64,
    pub formations: std::collections::BTreeMap<u64, crate::formations::Group>,
    pub projectiles: Projectiles,
    pub wrecks: Wrecks,
    pub aircraft_crashes: Vec<crate::aircraft_crash::AircraftCrash>,
    /// Dead ships on their way down to the seabed.
    #[serde(default)]
    pub sinking: Vec<crate::sinking::SinkingHull>,
    pub stains: Stains,
    /// Incendiary patches. Each bomb is its own fire; damage stacks where they overlap.
    #[serde(default)]
    pub fires: crate::tables::Fires,
    /// Giant bores' lightning storms, spreading from where they struck (`titan.rs`).
    #[serde(default)]
    pub storms: Vec<crate::titan::DischargeStorm>,
    /// Spent sabots in the air, thrown from a giant rail gun (`titan.rs`).
    #[serde(default)]
    pub sabots: Vec<crate::titan::FallingSabot>,
    /// Cone weapons' wakes rolling out over the ground (`wake.rs`).
    #[serde(default)]
    pub wakes: Vec<crate::wake::Wake>,
    /// Poured lots under structures. They stay after the building dies.
    #[serde(default)]
    pub pads: Pads,
    pub terrain_edits: Vec<TerrainEdit>,
    /// One bit per map prop: set once the prop has been destroyed.
    pub props_dead: Vec<u64>,
    /// City structures' health and fires (`city.rs`).
    #[serde(default)]
    pub city: crate::city::Structures,
    pub ai: Vec<AiState>,
    /// Commands the AI decided on last tick; applied with this tick's player commands.
    pub ai_pending: Vec<PlayerCommand>,
    /// Set when only one team is left.
    pub winner: Option<u8>,
    /// Core mines' feed, investment and ore share.
    #[serde(default)]
    pub mines: crate::mines::Mines,
    /// Survival mode's rounds, engine and nodes. None in any other match.
    #[serde(default)]
    pub survival: Option<crate::survival::Survival>,
    /// Finished hulls driving off their factory's pad while it prints the next (`orders.rs`).
    #[serde(default)]
    pub rollouts: std::collections::BTreeMap<UnitId, crate::orders::Rollout>,
    /// Factory batches, each one or more factories linked and the units they hold (`batch.rs`).
    #[serde(default)]
    pub batches: std::collections::BTreeMap<u32, crate::batch::Batch>,
    /// The number the next batch is given.
    #[serde(default)]
    pub next_batch: u32,
    /// Nuclear silos and interceptor arrays, warheads in flight, blasts running out (`nukes.rs`).
    #[serde(default)]
    pub strategic: crate::nukes::Strategic,
}

#[derive(Clone, Debug, Default)]
pub struct TickTimings {
    pub phases: Vec<(&'static str, u64)>,
    pub total_ns: u64,
}

/// Immutable map inputs the sim keeps around.
pub struct MapData {
    pub name: String,
    pub content_id: u64,
    /// Ore fields core mines draw on.
    pub ore: Vec<mc_map::OreRegion>,
    pub starts: Vec<FxVec2>,
    pub props: Vec<Prop>,
}

pub struct World {
    pub blueprints: Arc<Blueprints>,
    pub pool: Arc<Pool>,
    pub map: MapData,
    pub terrain: Heightfield,
    pub state: State,
    /// Units, wrecks and stains; rebuilt every tick after movement.
    pub index: SpatialIndex,
    /// Map props; built once.
    pub prop_index: SpatialIndex,
    /// Where the city's structures stand, and the buckets that find them (`city.rs`); built once.
    pub city_shapes: crate::city::CityShapes,
    pub fog: Fog,
    pub nav: Nav,
    /// What happened this tick, for effects, audio and UI. Not state.
    pub events: Vec<SimEvent>,
    /// Shots that hit something this tick, for the mirror to draw to the end. Not state.
    pub spent: Vec<crate::mirror::SpentShot>,
    /// Rounds of a stream gun still in the air after the shot carrying them landed. Not state.
    pub streams: Vec<crate::mirror::StreamTail>,
    /// Muzzle positions of the shots fired this tick, so the mirror can tell a shot's first stretch. Not state.
    pub muzzles: Vec<mc_core::FxVec3>,
    /// Who drew mass out of what this tick, for the mirror's reclaim beams. Not state.
    pub reclaims: Vec<crate::reclaim::ReclaimWork>,
    /// Each unit's resource flows this tick, by row. Not state.
    pub flows: Vec<crate::economy::UnitFlow>,
    /// Which buildings save which this tick (`adjacency.rs`). Not state.
    pub adjacency: crate::adjacency::Links,
    pub timings: TickTimings,
    /// Everything `mc_core::perf` recorded during the last tick, with the
    /// phases as `sim.<phase>` spans and table sizes as counters. Not state.
    pub perf: mc_core::perf::Frame,
    /// Path statistics at the end of the last tick, to report per-tick deltas.
    perf_nav: mc_path::NavStats,
    /// The map's ore fields, rasterised for the mines to count.
    pub ore: crate::mines::OreGrid,
    /// Index of the first terrain edit the renderer has not seen yet is tracked
    /// by the renderer; this is the count at the end of the last tick.
    pub(crate) scratch: Scratch,
}

/// Reusable per-tick buffers. Contents never survive a tick.
#[derive(Default)]
pub(crate) struct Scratch {
    pub build_jobs: Vec<crate::economy::BuildJob>,
    pub dead: Vec<usize>,
    /// Rows whose blueprint carries a shield, in row order, listed with the
    /// index and again after the tick's commands: blast and shot checks look
    /// only at these. It can still hold a row that has died since.
    pub shielded: Vec<usize>,
}

/// Radius props occupy in the spatial index.
const PROP_RADIUS: Fx = Fx::from_int(4);

/// Steepest an arm points up or down (35 degrees).
pub(crate) const ARM_PITCH_LIMIT: i16 = 6371;
/// Steepest a howitzer elevates (80 degrees).
pub(crate) const BALLISTIC_PITCH_LIMIT: i16 = 14564;

/// The pitch that points from `height` at something `rise` higher, `run` metres away, within what an arm can do.
pub(crate) fn pitch_to(run: Fx, rise: Fx) -> Angle {
    pitch_to_limit(run, rise, ARM_PITCH_LIMIT)
}

pub(crate) fn pitch_to_limit(run: Fx, rise: Fx, limit: i16) -> Angle {
    let pitch = Angle::ZERO.delta_to(FxVec2::new(run.max(Fx::ONE), rise).angle());
    Angle(pitch.clamp(-limit, limit) as u16)
}

/// `point` (a muzzle, an emitter; model space) carried round `pivot` by an arm pitched up by `pitch`.
pub(crate) fn pitched(
    point: mc_core::FxVec3,
    pivot: Option<mc_core::FxVec3>,
    pitch: Angle,
) -> mc_core::FxVec3 {
    let Some(pivot) = pivot else { return point };
    let arm = FxVec2::new(point.x - pivot.x, point.z - pivot.z).rotate(pitch);
    mc_core::FxVec3::new(pivot.x + arm.x, point.y, pivot.z + arm.y)
}

/// `offset` from a ground unit's origin (hull frame turned to `heading`, z up) tipped with
/// the hull as it leans on the ground under it. Matches the entity shader's lean, which
/// takes the terrain normal across the unit's radius.
pub(crate) fn leaned(
    terrain: &mc_map::Heightfield,
    pos: FxVec2,
    radius: Fx,
    heading: Angle,
    offset: mc_core::FxVec3,
) -> mc_core::FxVec3 {
    use mc_core::FxVec3;
    let step = radius.max(Fx::from_int(4));
    let h = |dx: Fx, dy: Fx| terrain.height_at(pos + FxVec2::new(dx, dy));
    let hx = h(step, Fx::ZERO) - h(-step, Fx::ZERO);
    let hy = h(Fx::ZERO, step) - h(Fx::ZERO, -step);
    if hx == Fx::ZERO && hy == Fx::ZERO {
        return offset;
    }
    let cross = |a: FxVec3, b: FxVec3| {
        FxVec3::new(
            a.y * b.z - a.z * b.y,
            a.z * b.x - a.x * b.z,
            a.x * b.y - a.y * b.x,
        )
    };
    let up = FxVec3::new(-hx, -hy, step * 2).normalize();
    let ahead = FxVec2::from_angle(heading);
    let left = cross(up, ahead.extend(Fx::ZERO)).normalize();
    let fwd = cross(left, up);
    let along = offset.xy().dot(ahead);
    let across = offset.xy().dot(FxVec2::new(-ahead.y, ahead.x));
    fwd * along + left * across + up * offset.z
}

/// Where a build beam leaves: the forearm pitched about the elbow, then the
/// boom about the shoulder when the arm is two-bone.
pub(crate) fn pose_build_arm(arm: mc_data::BuildArm, boom: Angle, tool: Angle) -> mc_core::FxVec3 {
    let at = pitched(arm.emitter, arm.pivot, tool);
    pitched(at, arm.shoulder, boom)
}

impl World {
    pub fn new(
        map_file: &MapFile,
        blueprints: Arc<Blueprints>,
        pool: Arc<Pool>,
        config: &MatchConfig,
    ) -> Result<World, SimError> {
        let terrain = Heightfield::load(map_file).map_err(|e| SimError::Setup(e.to_string()))?;
        let map = MapData {
            name: map_file.info().name.clone(),
            content_id: map_file.content_id(),
            ore: map_file.ore_regions().to_vec(),
            starts: map_file.start_positions().to_vec(),
            props: map_file.props().to_vec(),
        };
        let mut world = Self::with_terrain(terrain, map, blueprints, pool, config)?;
        world.lay_map_wrecks(map_file.wrecks())?;
        Ok(world)
    }

    /// The wreckage the map starts with, weathered down to what its record
    /// says is left. A key this build does not know is left out.
    pub fn lay_map_wrecks(&mut self, wrecks: &[mc_map::MapWreck]) -> Result<(), SimError> {
        for w in wrecks {
            let Some(id) = self.blueprints.id_of(&w.blueprint) else {
                continue;
            };
            let bp = self.blueprints.unit(id);
            let full = bp.cost_mass * bp.wreck_fraction;
            let mass = full * Fx::from_int(w.mass_milli.min(1000) as i32) / Fx::from_int(1000);
            if mass <= Fx::ZERO {
                continue;
            }
            if self.state.wrecks.slots.live() >= MAX_WRECKS {
                return Err(SimError::Setup(format!(
                    "the map starts with more than {MAX_WRECKS} wrecks"
                )));
            }
            let pos = self.clamp_to_map(w.pos);
            let row = self.state.wrecks.spawn(
                id,
                pos,
                self.terrain.height_at(pos),
                w.heading,
                mass,
                0,
            )?;
            let wrecks = &mut self.state.wrecks;
            wrecks.mass_max[row] = full;
            wrecks.bank[row] = w.bank;
            wrecks.prev_bank[row] = w.bank;
            wrecks.from_map[row] = true;
        }
        Ok(())
    }

    pub fn with_terrain(
        terrain: Heightfield,
        map: MapData,
        blueprints: Arc<Blueprints>,
        pool: Arc<Pool>,
        config: &MatchConfig,
    ) -> Result<World, SimError> {
        if config.players.is_empty() || config.players.len() > MAX_PLAYERS {
            return Err(SimError::Setup(format!(
                "a match has 1 to {MAX_PLAYERS} players"
            )));
        }
        let size = terrain.size_metres();
        let mut players = Vec::new();
        for p in &config.players {
            let faction = blueprints
                .faction_by_key(&p.faction)
                .ok_or_else(|| SimError::Setup(format!("unknown faction {}", p.faction)))?;
            let start = if config.spawn_commanders {
                *map.starts.get(p.start as usize).ok_or_else(|| {
                    SimError::Setup(format!("map has no start position {}", p.start))
                })?
            } else {
                map.starts
                    .get(p.start as usize)
                    .copied()
                    .unwrap_or(size * Fx::HALF)
            };
            players.push(Player {
                name: p.name.clone(),
                faction: faction.id.0,
                team: p.team,
                allies: 0,
                controller: p.controller,
                start,
                defeated: false,
                commander: Handle::NONE,
                mass: Fx::ZERO,
                energy: Fx::ZERO,
                mass_capacity: Fx::ZERO,
                energy_capacity: Fx::ZERO,
                mass_income: Fx::ZERO,
                energy_income: Fx::ZERO,
                mass_demand: Fx::ZERO,
                energy_demand: Fx::ZERO,
                efficiency: Fx::ONE,
                mass_spent: Fx::ZERO,
                energy_spent: Fx::ZERO,
                upkeep_efficiency: Fx::ONE,
                build_speed: Fx::ONE,
                mine_power: Fx::ONE,
                mine_lost: Fx::ZERO,
                focus: crate::focus::Focus::default(),
                tier_speed: [None; 3],
                reclaimed_mass: Fx::ZERO,
                reclaim_income: Fx::ZERO,
                reclaimed_counted: Fx::ZERO,
                units_built: 0,
                units_lost: 0,
                units_killed: 0,
                acts_as: players.len() as u8,
                free_build: false,
                income_permille: [1000, 1000],
                bonus_storage: [Fx::ZERO; 2],
            });
        }

        crate::tables::derive_allies(&mut players);
        let mut prop_index = SpatialIndex::new(size);
        for (i, p) in map.props.iter().enumerate() {
            prop_index.insert(kind::PROP, NO_OWNER, i, p.pos, PROP_RADIUS);
        }
        prop_index.build();

        let mut nav = Nav::new(&terrain, pool.clone())?;
        crate::lots::block_buildings(&mut nav, &map.props, size);
        let (city, city_shapes) = crate::city::build(&map.props, &terrain);
        let state = State {
            tick: 0,
            rng: Rng::new(config.seed),
            cheats: config.cheats,
            fog_enabled: config.fog,
            ai: config.players.iter().map(|p| AiState::new(p.ai)).collect(),
            players,
            units: Units::new(),
            orders: Orders::new(),
            formation_serial: 0,
            formations: std::collections::BTreeMap::new(),
            projectiles: Projectiles::default(),
            wrecks: Wrecks::new(),
            aircraft_crashes: Vec::new(),
            sinking: Vec::new(),
            stains: Stains::default(),
            fires: crate::tables::Fires::default(),
            storms: Vec::new(),
            sabots: Vec::new(),
            wakes: Vec::new(),
            pads: Pads::default(),
            terrain_edits: Vec::new(),
            props_dead: vec![0; map.props.len().div_ceil(64)],
            city,
            ai_pending: Vec::new(),
            winner: None,
            mines: Default::default(),
            survival: None,
            rollouts: Default::default(),
            batches: Default::default(),
            next_batch: 0,
            strategic: Default::default(),
        };
        let water = terrain.water_level();
        let ore = crate::mines::OreGrid::new(&map.ore, size, |p| terrain.height_at(p) > water);
        let mut world = World {
            blueprints,
            pool,
            map,
            index: SpatialIndex::new(size),
            prop_index,
            city_shapes,
            fog: Fog::new(size),
            nav,
            ore,
            terrain,
            state,
            events: Vec::new(),
            spent: Vec::new(),
            streams: Vec::new(),
            muzzles: Vec::new(),
            reclaims: Vec::new(),
            flows: Vec::new(),
            adjacency: Default::default(),
            timings: TickTimings::default(),
            perf: mc_core::perf::Frame::default(),
            perf_nav: mc_path::NavStats::default(),
            scratch: Scratch::default(),
        };

        if config.spawn_commanders {
            for p in 0..world.state.players.len() {
                let faction = world.state.players[p].faction as usize;
                let bp = world.blueprints.factions[faction].commander;
                let start = world.state.players[p].start;
                // Commanders face the middle of the map.
                let heading = (size * Fx::HALF - start).angle();
                let row = world.spawn_unit(bp, p as u8, start, heading, true)?;
                world.state.players[p].commander = world.state.units.id(row);
                let acu = world.blueprints.unit(bp).economy;
                world.state.players[p].mass = acu.mass_storage;
                world.state.players[p].energy = acu.energy_storage;
            }
        }
        world.rebuild_index();
        world.update_fog();
        Ok(world)
    }

    #[inline]
    pub fn tick_count(&self) -> u32 {
        self.state.tick
    }

    #[inline]
    pub fn bp(&self, row: usize) -> &UnitBlueprint {
        self.blueprints.unit(self.state.units.blueprint[row])
    }

    /// Bit mask of the players allied with `player`, including itself.
    #[inline]
    pub fn team_mask(&self, player: u8) -> PlayerMask {
        self.state.players[player as usize].allies
    }

    #[inline]
    pub fn are_enemies(&self, a: u8, b: u8) -> bool {
        self.state.players[a as usize].team != self.state.players[b as usize].team
    }

    /// Spawns a unit on the ground at `pos`. Structures also flatten the
    /// terrain under them and block their footprint for pathing.
    pub fn spawn_unit(
        &mut self,
        blueprint: BlueprintId,
        owner: u8,
        pos: FxVec2,
        heading: Angle,
        complete: bool,
    ) -> Result<usize, SimError> {
        let bp = self.blueprints.unit(blueprint).clone();
        // An experimental's site takes its lot like a structure until it is finished.
        if bp.is_structure() || (bp.is_site_built_unit() && !complete) {
            let lot = path_cells_of(bp.footprint, pos);
            let record = self.terrain.flatten_rect(lot.0, lot.1, None);
            if self.state.terrain_edits.len() >= MAX_FLATTENS {
                return Err(SimError::TableFull(Table::Flattens));
            }
            self.state.terrain_edits.push(TerrainEdit {
                min: (record.min_x, record.min_y),
                max: (record.max_x, record.max_y),
                sample: record.sample,
                faction: bp.faction.0,
            });
            self.occupy_lot(&bp, pos, heading);
            self.events.push(SimEvent::TerrainEdited);
        }
        let ground = self.terrain.height_at(pos);
        let surface = match bp.motion.map(|m| m.layer) {
            Some(MoveLayer::Hover) | Some(MoveLayer::Naval) | Some(MoveLayer::Air) => {
                ground.max(self.terrain.water_level())
            }
            // A seabed installation stands on the bottom, under the water.
            _ if bp.seabed => ground,
            _ if bp.water_build => ground.max(self.terrain.water_level()),
            _ => ground,
        };
        let z = match bp.motion {
            Some(m) if m.layer == MoveLayer::Air && complete => surface + m.altitude,
            _ => surface,
        };
        let row = self.state.units.spawn(UnitSpawn {
            blueprint,
            owner,
            pos,
            z,
            heading,
            health: if complete { bp.health } else { bp.health / 10 },
            flags: if complete {
                0
            } else {
                flag::UNDER_CONSTRUCTION
            },
            build_progress: if complete { bp.build_time } else { Fx::ZERO },
        })?;
        // Twin barrels of the same gun start half a reload apart so they take turns.
        // Cooldown counts down at the start of the tick, so half plus one puts
        // the later barrel midway between the earlier one's shots.
        // A submarine goes down as soon as it is out and about.
        self.state.units.dive_goal[row] = bp.dive.is_some();
        for (w, weapon) in bp.weapons.iter().enumerate() {
            if bp.weapons[..w].iter().any(|o| o.name == weapon.name) {
                self.state.units.weapon_cooldown[row][w] = weapon.reload_ticks / 2 + 1;
            }
            // A mount that faces off the nose (aft, or to a side) starts turned that way, as it rests.
            if weapon.mount && weapon.facing != Angle::ZERO {
                self.state.units.weapon_yaw[row][w] = weapon.facing;
                self.state.units.prev_weapon_yaw[row][w] = weapon.facing;
            }
        }
        if let Some(arm) = bp.builder.as_ref().and_then(|b| b.arm) {
            if arm.shoulder.is_some() {
                self.state.units.arm_pitch[row][0] = arm.rest;
                self.state.units.prev_arm_pitch[row][0] = arm.rest;
            } else {
                self.state.units.arm_pitch[row][1] = arm.rest;
                self.state.units.prev_arm_pitch[row][1] = arm.rest;
            }
        }
        if complete {
            self.remember_structure_pad(&bp, pos, owner)?;
            self.arm_shield(row, true);
        }
        Ok(row)
    }

    /// Records the poured lot under a structure. Rebuilds on the same centre
    /// share it; walls have none. The renderer looks up the mesh plan by
    /// blueprint so the slab still traces the hull after the building is gone.
    pub(crate) fn remember_structure_pad(
        &mut self,
        bp: &UnitBlueprint,
        pos: FxVec2,
        owner: u8,
    ) -> Result<(), SimError> {
        if !bp.poured_lot() {
            return Ok(());
        }
        let cells = bp.footprint.0.max(bp.footprint.1) as i32;
        let radius = Fx::from_int(cells * mc_map::BUILD_CELL_M / 2);
        let nanite = self
            .blueprints
            .factions
            .get(bp.faction.0 as usize)
            .is_some_and(|f| f.construction == mc_data::Construction::Nanite);
        let packed =
            pack_structure_pad(owner, 255, bp.id.0, false) | if nanite { PAD_NANITE } else { 0 };
        self.state.pads.upsert(pos, radius, packed)?;
        Ok(())
    }

    /// Advances the simulation by one tick. `commands` must already be in the
    /// canonical order the session delivers (by player slot, then issue order).
    /// Returns the state hash by section (`state_hash.rs`); `tick` folds them.
    pub fn tick_sections(
        &mut self,
        commands: &[PlayerCommand],
    ) -> Result<crate::state_hash::SectionHashes, SimError> {
        #[expect(
            clippy::disallowed_types,
            clippy::disallowed_methods,
            reason = "presentation: wall-clock phase timings for perf reports; never read by the tick"
        )]
        let start = std::time::Instant::now();
        let perf_scope = mc_core::perf::Scope::begin();
        let mut last = start;
        self.timings.phases.clear();
        self.events.clear();
        self.spent.clear();
        #[expect(
            clippy::float_arithmetic,
            reason = "presentation: ages the render mirror's stream tails; World::streams is not State"
        )]
        {
            for tail in &mut self.streams {
                tail.age += 1.0;
            }
            self.streams.retain(|t| t.age - 1.0 - t.last < t.lands);
        }
        self.muzzles.clear();
        self.reclaims.clear();
        self.flows.clear();
        // Each phase's own counters, so a report can say which phase made the
        // spatial queries or line-of-fire rays.
        let mut phase_scope = mc_core::perf::Scope::begin();
        let mut phase_counts: Vec<(&'static str, mc_core::perf::Frame)> = Vec::new();
        let mut phase = |timings: &mut TickTimings, name: &'static str| {
            #[expect(
                clippy::disallowed_types,
                clippy::disallowed_methods,
                reason = "presentation: wall-clock phase timings for perf reports; never read by the tick"
            )]
            let now = std::time::Instant::now();
            timings.phases.push((name, (now - last).as_nanos() as u64));
            last = now;
            let done = std::mem::replace(&mut phase_scope, mc_core::perf::Scope::begin());
            phase_counts.push((name, done.end()));
        };

        self.state.tick += 1;
        let units = &mut self.state.units;
        units.prev_pos.clone_from(&units.pos);
        units.prev_z.clone_from(&units.z);
        units.prev_heading.clone_from(&units.heading);
        units.prev_bank.clone_from(&units.bank);
        units.prev_weapon_yaw.clone_from(&units.weapon_yaw);
        units.prev_arm_pitch.clone_from(&units.arm_pitch);
        units.prev_shield_open.clone_from(&units.shield_open);
        units.prev_deploy.clone_from(&units.deploy);
        for f in &mut units.flags {
            *f &= !flag::TRANSIENT;
        }
        let projectiles = &mut self.state.projectiles;
        projectiles.prev_pos.clone_from(&projectiles.pos);
        projectiles.prev_aim.clone_from(&projectiles.aim);

        self.nav.begin_tick(self.state.tick)?;
        phase(&mut self.timings, "paths");

        // Before the commands, so a countdown armed this tick runs its whole length.
        self.run_destructs();
        let ai_commands = std::mem::take(&mut self.state.ai_pending);
        for c in ai_commands.iter().chain(commands) {
            self.apply_command(c)?;
        }
        self.list_shielded();
        phase(&mut self.timings, "commands");

        {
            let _t = mc_core::perf_span!("fn.run_air_support");
            self.run_air_support()?;
        }
        {
            let _t = mc_core::perf_span!("fn.run_orders");
            self.run_orders()?;
        }
        {
            let _t = mc_core::perf_span!("fn.lose_orphaned_cargo");
            self.lose_orphaned_cargo()?;
        }
        {
            let _t = mc_core::perf_span!("fn.run_warps");
            self.run_warps()?;
        }
        {
            let _t = mc_core::perf_span!("fn.run_transports");
            self.run_transports();
        }
        phase(&mut self.timings, "orders");

        {
            let _t = mc_core::perf_span!("fn.run_deploy");
            self.run_deploy();
        }
        phase(&mut self.timings, "deploy");

        {
            let _t = mc_core::perf_span!("fn.run_mines");
            self.run_mines();
        }
        {
            let _t = mc_core::perf_span!("fn.run_economy");
            self.run_economy()?;
        }
        phase(&mut self.timings, "economy");

        {
            let _t = mc_core::perf_span!("fn.run_shields");
            self.run_shields();
        }
        phase(&mut self.timings, "shields");

        {
            let _t = mc_core::perf_span!("fn.run_regen");
            self.run_regen();
        }
        {
            let _t = mc_core::perf_span!("fn.run_repair_fields");
            self.run_repair_fields();
        }
        phase(&mut self.timings, "regen");

        {
            let _t = mc_core::perf_span!("fn.run_dive");
            self.run_dive();
        }
        {
            let _t = mc_core::perf_span!("fn.run_movement");
            self.run_movement()?;
        }
        {
            let _t = mc_core::perf_span!("fn.seat_drones");
            self.seat_drones();
        }
        {
            let _t = mc_core::perf_span!("fn.run_trampling");
            self.run_trampling();
        }
        phase(&mut self.timings, "movement");

        {
            let _t = mc_core::perf_span!("fn.rebuild_index");
            self.rebuild_index();
        }
        phase(&mut self.timings, "index");

        {
            let _t = mc_core::perf_span!("fn.run_targeting");
            self.run_targeting();
        }
        phase(&mut self.timings, "targeting");

        {
            let _t = mc_core::perf_span!("fn.run_torpedo_defence");
            self.run_torpedo_defence()?;
        }
        {
            let _t = mc_core::perf_span!("fn.run_weapons");
            self.run_weapons()?;
        }
        {
            let _t = mc_core::perf_span!("fn.run_giants");
            self.run_giants()?;
        }
        if !self.state.wakes.is_empty() {
            let _t = mc_core::perf_span!("fn.run_wakes");
            self.run_wakes();
        }
        phase(&mut self.timings, "weapons");

        {
            let _t = mc_core::perf_span!("fn.run_projectiles");
            self.run_projectiles()?;
        }
        {
            let _t = mc_core::perf_span!("fn.run_strategic");
            self.run_strategic()?;
        }
        {
            let _t = mc_core::perf_span!("fn.run_city_fires");
            self.run_city_fires();
        }
        phase(&mut self.timings, "projectiles");

        {
            let _t = mc_core::perf_span!("fn.run_aircraft_crashes");
            self.run_aircraft_crashes()?;
        }
        {
            let _t = mc_core::perf_span!("fn.run_sinking");
            self.run_sinking()?;
        }
        {
            let _t = mc_core::perf_span!("fn.reap_dead");
            self.reap_dead()?;
        }
        phase(&mut self.timings, "deaths");

        {
            let _t = mc_core::perf_span!("fn.update_fog");
            self.update_fog();
        }
        phase(&mut self.timings, "fog");

        {
            let _t = mc_core::perf_span!("fn.run_ai");
            self.run_ai()?;
        }
        phase(&mut self.timings, "ai");

        {
            let _t = mc_core::perf_span!("fn.run_survival");
            self.run_survival()?;
        }
        phase(&mut self.timings, "survival");

        {
            let _t = mc_core::perf_span!("fn.check_victory");
            self.check_victory();
        }
        {
            // Again for the next tick's orders: the deaths, wrecks and scorch marks
            // since the rebuild above are in it, as they are in a world restored from a
            // snapshot taken now (`restore` builds it from the state alone).
            let _t = mc_core::perf_span!("fn.rebuild_index");
            self.rebuild_index();
        }
        phase(&mut self.timings, "index_after");
        let sections = self.hash_sections();
        phase(&mut self.timings, "hash");
        self.timings.total_ns = start.elapsed().as_nanos() as u64;
        self.perf = perf_scope.end();
        self.record_perf(&phase_counts);
        Ok(sections)
    }

    /// Adds the phase times, table sizes and path work of the tick just run to `perf`.
    fn record_perf(&mut self, phase_counts: &[(&'static str, mc_core::perf::Frame)]) {
        use mc_core::perf::intern;
        let f = &mut self.perf;
        for (phase, counts) in phase_counts {
            for e in counts.entries.iter().filter(|e| e.ns.is_none()) {
                f.push(intern(&format!("{phase}>{}", e.name)), e.n, None);
            }
        }
        f.push("sim.tick", 1, Some(self.timings.total_ns));
        for &(name, ns) in &self.timings.phases {
            f.push(intern(&format!("sim.{name}")), 1, Some(ns));
        }
        let s = &self.state;
        f.push("size.units", s.units.slots.live() as u64, None);
        f.push("size.projectiles", s.projectiles.len() as u64, None);
        f.push("size.wrecks", s.wrecks.slots.live() as u64, None);
        f.push("size.index_entries", self.index.len() as u64, None);
        f.push(
            "size.index_max_radius_m",
            self.index.max_radius().ceil_int().max(0) as u64,
            None,
        );
        let n = self.nav.stats();
        let was = std::mem::replace(&mut self.perf_nav, n);
        for (name, now, before) in [
            ("nav.builds", n.builds_scheduled, was.builds_scheduled),
            ("nav.repairs", n.repairs_scheduled, was.repairs_scheduled),
            ("nav.extends", n.extends_scheduled, was.extends_scheduled),
            ("nav.search_nodes", n.search_nodes, was.search_nodes),
            ("nav.tiles_built", n.tiles_built, was.tiles_built),
            ("nav.late_joins", n.late_joins, was.late_joins),
        ] {
            if now > before {
                f.push(name, now - before, None);
            }
        }
        f.push("size.nav_fields", n.live_fields as u64, None);
    }

    pub(crate) fn rebuild_index(&mut self) {
        let s = &self.state;
        self.index.clear();
        for row in s.units.slots.iter() {
            if !s.units.has_flag(row, flag::IN_FACTORY) {
                let bp = self.blueprints.unit(s.units.blueprint[row]);
                let air = bp.motion.is_some_and(|m| m.layer == MoveLayer::Air);
                let kinds = if air {
                    kind::UNIT | kind::AIRCRAFT
                } else {
                    kind::UNIT
                };
                self.index
                    .insert(kinds, s.units.owner[row], row, s.units.pos[row], bp.radius);
            }
        }
        for row in s.wrecks.slots.iter() {
            let radius = self.blueprints.unit(s.wrecks.blueprint[row]).radius;
            self.index
                .insert(kind::WRECK, NO_OWNER, row, s.wrecks.pos[row], radius);
        }
        for row in 0..s.stains.len() {
            self.index.insert(
                kind::STAIN,
                NO_OWNER,
                row,
                s.stains.pos[row],
                s.stains.radius[row],
            );
        }
        self.index.build_on(&self.pool);
        self.list_shielded();
    }

    /// Refreshes `scratch.shielded`.
    pub(crate) fn list_shielded(&mut self) {
        let units = &self.state.units;
        self.scratch.shielded.clear();
        self.scratch.shielded.extend(
            units
                .slots
                .iter()
                .filter(|&row| self.blueprints.unit(units.blueprint[row]).shield.is_some()),
        );
    }

    /// True when a unit entry from the index still describes a live unit. The
    /// index is rebuilt mid-tick, so early phases can see rows that have since
    /// died or been reused.
    #[inline]
    pub(crate) fn unit_entry_is_current(&self, e: &crate::spatial::Entry) -> bool {
        let row = e.row as usize;
        self.state.units.slots.is_alive(row) && self.state.units.pos[row] == e.pos
    }

    /// Radar range this unit paints right now. A tower that draws energy is
    /// dark while the grid cannot pay or it is paused; a scout's dish keeps working.
    pub(crate) fn live_radar(&self, row: usize) -> Fx {
        let bp = self.bp(row);
        if !self.state.units.is_active(row) || self.powered_down(row) {
            return Fx::ZERO;
        }
        if bp.economy.energy_upkeep > Fx::ZERO && self.energy_stalling(self.state.units.owner[row])
        {
            Fx::ZERO
        } else {
            bp.radar
        }
    }

    pub(crate) fn update_fog(&mut self) {
        self.fog.begin();
        let rows: Vec<usize> = self.state.units.slots.iter().collect();
        // Every unit's discs, once per fog cell: the hundred tanks of a cell
        // with one sight and one team stamp the same discs, and a stamp is an OR.
        let mut stamps: Vec<_> = rows
            .iter()
            .filter(|&&row| !self.state.units.has_flag(row, flag::IN_FACTORY))
            .map(|&row| {
                let bp = self.blueprints.unit(self.state.units.blueprint[row]);
                // Construction sites see a little so their owner can watch them.
                // An intel tower that draws energy is dark while the grid cannot pay.
                let (vision, radar) = if self.state.units.is_active(row) {
                    (bp.vision, self.live_radar(row))
                } else {
                    (bp.radius * 2, Fx::ZERO)
                };
                let sight = (vision, radar, self.live_sonar(row));
                let mask = self.team_mask(self.state.units.owner[row]);
                (Fog::cell_of(self.state.units.pos[row]), sight, mask, row)
            })
            .collect();
        // The renderer draws every unit's own disc, round and following it.
        for &(_, (vision, _, _), mask, row) in &stamps {
            self.fog
                .note_sight(Some(row), self.state.units.pos[row], vision, mask);
        }
        // The whole tuple orders them, so the sort is total; one of each.
        stamps.sort_unstable();
        stamps.dedup_by_key(|&mut (cell, sight, mask, _)| (cell, sight, mask));
        for (_, (vision, radar, sonar), mask, row) in stamps {
            let pos = self.state.units.pos[row];
            self.fog.stamp_discs(pos, vision, radar, mask);
            self.fog.reveal_sonar(pos, sonar, mask);
        }
        self.survival_reveal();
        for &row in &rows {
            if self.state.units.has_flag(row, flag::IN_FACTORY) {
                continue;
            }
            // Nobody makes out a hull under water; sonar only knows it is there.
            let seen = self.fog.visible_mask(self.state.units.pos[row]);
            if seen != 0 && !self.submerged(row) {
                self.fog
                    .identify(row, self.state.units.id(row).generation(), seen);
            }
        }
    }

    /// Can `player` shoot at / see the unit in `row`?
    #[inline]
    pub fn detects(&self, player: u8, row: usize) -> bool {
        !self.state.fog_enabled
            || !self.are_enemies(player, self.state.units.owner[row])
            || self.detected_by(row, player_bit(player))
    }

    fn check_victory(&mut self) {
        if self.state.winner.is_some() || self.state.players.len() < 2 {
            return;
        }
        let mut alive_team = None;
        for p in &self.state.players {
            if p.defeated {
                continue;
            }
            match alive_team {
                None => alive_team = Some(p.team),
                Some(t) if t != p.team => return,
                _ => {}
            }
        }
        if let Some(team) = alive_team {
            self.state.winner = Some(team);
            self.events.push(SimEvent::MatchOver { winner_team: team });
        }
    }

    /// Serialises the game state for late join, reconnect and save games.
    /// Call between ticks. May wait for flow-field builds in flight to finish.
    pub fn snapshot(&mut self) -> Vec<u8> {
        let nav = self.nav.snapshot();
        bincode::serialize(&(&self.state, &nav, &self.fog)).expect("state always serialises")
    }

    /// Replaces this world's state with a snapshot taken on another machine
    /// running the same map and blueprints. The restored world steps exactly
    /// as the source does from the next tick on.
    ///
    /// `base_terrain` is the map's terrain as baked, before any edits.
    pub fn restore(&mut self, base_terrain: Heightfield, bytes: &[u8]) -> Result<(), SimError> {
        let (state, nav, fog): (State, crate::nav::NavSnapshot, crate::fog::Fog) =
            crate::decode_untrusted(bytes, crate::MAX_SNAPSHOT_BYTES)
                .map_err(SimError::Snapshot)?;
        state.validate().map_err(SimError::Snapshot)?;
        state
            .validate_ids(self.blueprints.units.len())
            .map_err(SimError::Snapshot)?;
        if state.players.len() != self.state.players.len()
            || state.props_dead.len() != self.state.props_dead.len()
            || state.city.prop != self.state.city.prop
            || state.city.max != self.state.city.max
        {
            return Err(SimError::Snapshot(
                "snapshot is from a different match".into(),
            ));
        }
        // Pathing restores over unedited terrain classes; blockers come from its own state.
        self.nav = Nav::restore(&base_terrain, self.pool.clone(), &nav)?;
        self.terrain = base_terrain;
        for e in &state.terrain_edits {
            self.terrain.apply_flatten(&e.record());
        }
        self.state = state;
        crate::tables::derive_allies(&mut self.state.players);
        self.rebuild_lots();
        self.rebuild_index();
        // Not rebuilt: what each side has seen, and the vision the next tick reads, are the
        // source's own.
        self.fog.replace_with(fog).map_err(SimError::Snapshot)?;
        self.events.push(SimEvent::TerrainEdited);
        Ok(())
    }

    pub fn is_prop_alive(&self, prop: usize) -> bool {
        self.state.props_dead[prop / 64] & (1 << (prop % 64)) == 0
    }
}

/// Pad `packed`: the owner's slot, bits 0..5 (room for `mc_core::MAX_PLAYERS`).
pub const PAD_OWNER_MASK: u32 = 0x1F;
/// Pad `packed` bit: a plan not yet started, drawn as a ghost. Bit 6.
pub const PAD_GHOST: u32 = 1 << 6;
/// Pad `packed` bit: the lot of a faction that builds with nanites
/// (`mc_data::Construction::Nanite`): laid in dark machined plate, not paved. Bit 7.
pub const PAD_NANITE: u32 = 1 << 7;

const _: () = assert!(MAX_PLAYERS <= PAD_OWNER_MASK as usize + 1);

/// Packing the pad shader reads (`mc_models::gpu_consts::pad`; a test holds the
/// two equal): owner in 0..5, ghost at bit 6, nanite at bit 7, build in 8..16,
/// blueprint index in 16..32. The slab is the mesh plan for that blueprint,
/// not a hashed lot.
pub fn pack_structure_pad(owner: u8, build: u8, blueprint: u16, ghost: bool) -> u32 {
    (owner as u32 & PAD_OWNER_MASK)
        | if ghost { PAD_GHOST } else { 0 }
        | (build as u32) << 8
        | (blueprint as u32) << 16
}

/// Snaps a structure centre to the build grid: odd footprints centre on a
/// cell, even footprints on a cell corner.
pub fn snap_to_build_grid(bp: &UnitBlueprint, pos: FxVec2) -> FxVec2 {
    let cell = Fx::from_int(mc_map::BUILD_CELL_M);
    let snap = |v: Fx, cells: u8| {
        if cells % 2 == 1 {
            Fx::from_int((v / cell).floor_int()) * cell + cell / 2
        } else {
            Fx::from_int((v / cell).round_int()) * cell
        }
    };
    FxVec2::new(snap(pos.x, bp.footprint.0), snap(pos.y, bp.footprint.1))
}

fn lot_extents(footprint: (u8, u8), pos: FxVec2) -> (i32, i32, i32, i32) {
    let half_w = footprint.0 as i32 * mc_map::BUILD_CELL_M / 2;
    let half_h = footprint.1 as i32 * mc_map::BUILD_CELL_M / 2;
    let cx = pos.x.round_int();
    let cy = pos.y.round_int();
    (cx - half_w, cy - half_h, cx + half_w, cy + half_h)
}

/// Inclusive lot rectangle: a deposit on the edge still counts as covered.
pub fn lot_covers_point(footprint: (u8, u8), pos: FxVec2, point: FxVec2) -> bool {
    let (x0, y0, x1, y1) = lot_extents(footprint, pos);
    let px = point.x.round_int();
    let py = point.y.round_int();
    px >= x0 && px <= x1 && py >= y0 && py <= y1
}

/// Inclusive path-cell rectangle a structure blocks. Hulls sit inside the
/// build-grid lot, so only cells whose centres lie inside the lot are
/// impassable — units can squeeze along the pad edge.
pub fn footprint_cells(bp: &UnitBlueprint, pos: FxVec2) -> ((u32, u32), (u32, u32)) {
    place_cells_of(bp.footprint, pos)
}

/// Metres of a path cell, across, a hull may cover and still leave the cell
/// walkable. Units brush past a building's walls; a lane between two lots
/// stays open once each hull stands this far back from the lot edge.
const HULL_GRACE_M: i32 = 3;

/// Inclusive path-cell rectangles a structure's hull keeps units off: the lot
/// interior cells the hull (turned to `heading`) covers by more than
/// [`HULL_GRACE_M`] across. The rest of the lot is apron units walk on.
/// Walls fill their lot.
pub fn hull_cells(
    bp: &UnitBlueprint,
    pos: FxVec2,
    heading: Angle,
) -> Vec<((u32, u32), (u32, u32))> {
    let (min, max) = place_cells_of(bp.footprint, pos);
    if bp.has(cat::WALL) {
        return vec![(min, max)];
    }
    // A seabed installation stands deep under the keels: ships sail over it.
    if bp.seabed {
        return Vec::new();
    }
    let cell = mc_map::CELL_SIZE_M;
    let grow = Fx::from_int(cell / 2 - HULL_GRACE_M);
    let (hx, hy) = (bp.hull.0 + grow, bp.hull.1 + grow);
    let inside = |x: u32, y: u32| {
        let c = FxVec2::from_ints(x as i32 * cell + cell / 2, y as i32 * cell + cell / 2);
        let local = (c - pos).rotate(-heading);
        local.x.abs() < hx && local.y.abs() < hy
    };
    let mut out: Vec<((u32, u32), (u32, u32))> = Vec::new();
    for y in min.1..=max.1 {
        let mut x = min.0;
        while x <= max.0 {
            if !inside(x, y) {
                x += 1;
                continue;
            }
            let x0 = x;
            while x < max.0 && inside(x + 1, y) {
                x += 1;
            }
            // Grow the rectangle from the row below when it spans the same cells.
            match out
                .iter_mut()
                .find(|r| r.0 .0 == x0 && r.1 .0 == x && r.1 .1 + 1 == y)
            {
                Some(r) => r.1 .1 = y,
                None => out.push(((x0, y), (x, y))),
            }
            x += 1;
        }
    }
    if out.is_empty() {
        // A hull too small to cover any cell still stands on the one under its middle.
        let c = (
            (pos.x.floor_int() / cell).max(0) as u32,
            (pos.y.floor_int() / cell).max(0) as u32,
        );
        out.push((c, c));
    }
    out
}

/// Path cells the lot overlaps when rounded out to 8 m. Flattening and the
/// terrain check use this so a 12 m pad still sits on ground that covers it.
pub(crate) fn path_cells_of(footprint: (u8, u8), pos: FxVec2) -> ((u32, u32), (u32, u32)) {
    let cell = mc_map::CELL_SIZE_M;
    let (x0, y0, x1, y1) = lot_extents(footprint, pos);
    let to_cell = |lo: i32, hi: i32| {
        let a = lo.max(0) / cell;
        let b = ((hi + cell - 1) / cell - 1).max(a);
        (a as u32, b as u32)
    };
    let (xa, xb) = to_cell(x0, x1);
    let (ya, yb) = to_cell(y0, y1);
    ((xa, ya), (xb, yb))
}

/// Path cells whose centres lie strictly inside the lot. Neighbouring 12 m
/// lots share the 8 m cells their edges round into; those stay walkable and
/// must not block placement.
pub(crate) fn place_cells_of(footprint: (u8, u8), pos: FxVec2) -> ((u32, u32), (u32, u32)) {
    let cell = mc_map::CELL_SIZE_M;
    let (x0, y0, x1, y1) = lot_extents(footprint, pos);
    let to_cell = |lo: i32, hi: i32| {
        let first = (lo - cell / 2).div_euclid(cell) + 1;
        let last = (hi - cell / 2 - 1).div_euclid(cell);
        let a = first.max(0);
        let b = last.max(a);
        (a as u32, b as u32)
    };
    let (xa, xb) = to_cell(x0, x1);
    let (ya, yb) = to_cell(y0, y1);
    ((xa, ya), (xb, yb))
}

impl World {
    /// Placement rules for a structure: inside the map, on ground its layer
    /// allows, not overlapping another structure. A core mine also keeps out
    /// of every other mine's reach, anyone's, finished or begun (`mine_in_the_way`).
    pub fn can_place(&self, bp: &UnitBlueprint, pos: FxVec2) -> bool {
        self.lot_fits(bp, pos)
            && bp
                .mine
                .is_none_or(|m| self.mine_in_the_way(pos, m.reach, None).is_none())
    }

    /// `can_place` without the mines' reach: the lot alone.
    pub(crate) fn lot_fits(&self, bp: &UnitBlueprint, pos: FxVec2) -> bool {
        let size = self.terrain.size_metres();
        let half = FxVec2::from_ints(
            bp.footprint.0 as i32 * mc_map::BUILD_CELL_M / 2,
            bp.footprint.1 as i32 * mc_map::BUILD_CELL_M / 2,
        );
        if pos.x - half.x < Fx::ZERO
            || pos.y - half.y < Fx::ZERO
            || pos.x + half.x > size.x
            || pos.y + half.y > size.y
        {
            return false;
        }
        // Terrain on the rounded-out path rect; occupancy on the lot interior.
        // Neighbouring 2x2s share the overhang cells, and those stay walkable
        // so units can squeeze through without forbidding the next building.
        let path = path_cells_of(bp.footprint, pos);
        let water = self.terrain.water_level();
        if bp.seabed
            && !(path.0 .1..=path.1 .1).all(|y| {
                (path.0 .0..=path.1 .0).all(|x| crate::placement::abyss(&self.terrain, water, x, y))
            })
        {
            return false;
        }
        if if bp.water_only() {
            !self.nav.passable_naval_terrain(path.0, path.1)
        } else if bp.water_build {
            !self.nav.passable_floating_terrain(path.0, path.1)
        } else {
            !self.nav.passable_terrain(path.0, path.1)
        } {
            return false;
        }
        let place = place_cells_of(bp.footprint, pos);
        self.nav.no_blockers(place.0, place.1) && self.nav.lots_free(place.0, place.1)
    }

    /// A structure of category `categories` whose footprint contains `pos`, if any.
    pub fn structure_at(&self, pos: FxVec2, categories: u32) -> Option<usize> {
        let mut found = None;
        self.index.query(pos, Fx::ONE, kind::UNIT, |e| {
            let row = e.row as usize;
            if !self.unit_entry_is_current(e) {
                return true;
            }
            let bp = self.bp(row);
            if bp.is_structure() && bp.has(categories) {
                let (min, max) = path_cells_of(bp.footprint, e.pos);
                let cx = (pos.x.floor_int() / mc_map::CELL_SIZE_M) as u32;
                let cy = (pos.y.floor_int() / mc_map::CELL_SIZE_M) as u32;
                if cx >= min.0 && cx <= max.0 && cy >= min.1 && cy <= max.1 {
                    found = Some(row);
                    return false;
                }
            }
            true
        });
        found
    }
}

#[cfg(test)]
#[expect(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    reason = "test assertions measure in floats"
)]
mod tests {
    use super::*;
    use crate::lots::cells_overlap;

    #[test]
    fn a_muzzle_leans_with_the_hull_on_a_slope() {
        use mc_core::FxVec3;
        // Ground rising one metre per metre toward +x: a 45 degree climb.
        let (w, h) = (64u32, 64u32);
        let size = mc_map::Heightfield::flat(w, h, Fx::ZERO).size_metres();
        let per_cell = (size.x / Fx::from_int(w as i32)).to_f32();
        let samples = (0..=h)
            .flat_map(|_| (0..=w).map(move |x| (x as f32 * per_cell * 4.0) as u16))
            .collect();
        let terrain = mc_map::Heightfield::from_samples(
            w,
            h,
            samples,
            Fx::ZERO,
            Fx::ratio(1, 4),
            Fx::from_int(-100),
        );
        let pos = size * Fx::ratio(1, 2);
        let muzzle = FxVec3::new(Fx::from_int(4), Fx::ZERO, Fx::from_int(3));
        let flat = mc_map::Heightfield::flat(w, h, Fx::ZERO);
        assert_eq!(
            leaned(&flat, pos, Fx::from_int(3), Angle::ZERO, muzzle),
            muzzle
        );

        // Facing uphill, a barrel ahead of the hull rides up and is drawn back.
        let up = leaned(&terrain, pos, Fx::from_int(3), Angle::ZERO, muzzle);
        let half = std::f32::consts::FRAC_1_SQRT_2;
        assert!((up.x.to_f32() - (4.0 - 3.0) * half).abs() < 0.05, "{up:?}");
        assert!((up.z.to_f32() - (4.0 + 3.0) * half).abs() < 0.05, "{up:?}");
        assert!(up.y.to_f32().abs() < 0.01);
        // Its length is kept.
        assert!((up.length().to_f32() - 5.0).abs() < 0.05);
    }

    #[test]
    fn neighbouring_2x2_lots_share_overhang_not_block_cells() {
        let a = FxVec2::from_ints(24, 24);
        let b = FxVec2::from_ints(48, 24);
        let fp = (2, 2);
        assert!(
            cells_overlap(path_cells_of(fp, a), path_cells_of(fp, b)),
            "flatten still covers the 4 m overhang both lots round into"
        );
        assert!(
            !cells_overlap(place_cells_of(fp, a), place_cells_of(fp, b)),
            "pathing and placement leave the overhang walkable"
        );
    }

    #[test]
    fn a_2x2_blocks_less_than_its_lot() {
        let pos = FxVec2::from_ints(24, 24);
        let fp = (2, 2);
        let lot = path_cells_of(fp, pos);
        let block = place_cells_of(fp, pos);
        assert!(
            block.0 .0 > lot.0 .0 && block.1 .0 < lot.1 .0,
            "exclusion sits inside the build-grid lot, not on its overhang"
        );
    }

    #[test]
    fn a_1x1_against_a_2x2_does_not_share_place_cells() {
        let power = FxVec2::from_ints(24, 24);
        let wall = FxVec2::from_ints(42, 24);
        assert!(!cells_overlap(
            place_cells_of((2, 2), power),
            place_cells_of((1, 1), wall),
        ));
    }

    #[test]
    fn a_lot_covers_a_deposit_on_its_edge() {
        let pad = FxVec2::from_ints(24, 24);
        assert!(lot_covers_point((2, 2), pad, pad));
        assert!(lot_covers_point((2, 2), FxVec2::from_ints(36, 24), pad));
        assert!(!lot_covers_point((2, 2), FxVec2::from_ints(48, 24), pad));
        assert!(lot_covers_point((1, 1), FxVec2::from_ints(30, 30), pad));
    }

    #[test]
    fn pad_packing_names_the_blueprint() {
        let p = pack_structure_pad(31, 200, 42, true);
        assert_eq!(p & PAD_OWNER_MASK, 31);
        assert_ne!(p & PAD_GHOST, 0, "ghost");
        assert_eq!(p & PAD_NANITE, 0);
        assert_eq!((p >> 8) & 0xFF, 200);
        assert_eq!(p >> 16, 42);
    }
}
