//! `World`: state, inputs, derived structures, and the tick pipeline.

use crate::ai::AiState;
use crate::command::PlayerCommand;
use crate::fog::Fog;
use crate::mirror::SimEvent;
use crate::nav::Nav;
use crate::spatial::{kind, SpatialIndex};
use crate::tables::*;
use crate::{SimError, Table};
use mc_core::{Angle, Fx, FxVec2, Rng, StateHasher, MAX_PLAYERS};
use mc_data::{cat, BlueprintId, Blueprints, UnitBlueprint};
use mc_jobs::Pool;
use mc_map::{Heightfield, MapFile, Prop};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerSetup {
    pub name: String,
    /// Faction key, e.g. "Aster".
    pub faction: String,
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
    pub projectiles: Projectiles,
    pub wrecks: Wrecks,
    pub stains: Stains,
    /// Poured lots under structures. They stay after the building dies.
    #[serde(default)]
    pub pads: Pads,
    pub terrain_edits: Vec<TerrainEdit>,
    /// One bit per map prop: set once the prop has been destroyed.
    pub props_dead: Vec<u64>,
    pub ai: Vec<AiState>,
    /// Commands the AI decided on last tick; applied with this tick's player commands.
    pub ai_pending: Vec<PlayerCommand>,
    /// Set when only one team is left.
    pub winner: Option<u8>,
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
    pub deposits: Vec<FxVec2>,
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
    pub fog: Fog,
    pub nav: Nav,
    /// What happened this tick, for effects, audio and UI. Not state.
    pub events: Vec<SimEvent>,
    /// Shots that hit something this tick, for the mirror to draw to the end. Not state.
    pub spent: Vec<crate::mirror::SpentShot>,
    /// Muzzle positions of the shots fired this tick, so the mirror can tell a shot's first stretch. Not state.
    pub muzzles: Vec<mc_core::FxVec3>,
    /// Who drew mass out of what this tick, for the mirror's reclaim beams. Not state.
    pub reclaims: Vec<crate::reclaim::ReclaimWork>,
    pub timings: TickTimings,
    /// Index of the first terrain edit the renderer has not seen yet is tracked
    /// by the renderer; this is the count at the end of the last tick.
    pub(crate) scratch: Scratch,
}

/// Reusable per-tick buffers. Contents never survive a tick.
#[derive(Default)]
pub(crate) struct Scratch {
    pub build_jobs: Vec<crate::economy::BuildJob>,
    pub dead: Vec<usize>,
}

/// Radius props occupy in the spatial index.
const PROP_RADIUS: Fx = Fx::from_int(4);

/// Steepest an arm points up or down (35 degrees).
pub(crate) const ARM_PITCH_LIMIT: i16 = 6371;

/// The pitch that points from `height` at something `rise` higher, `run` metres away, within what an arm can do.
pub(crate) fn pitch_to(run: Fx, rise: Fx) -> Angle {
    let pitch = Angle::ZERO.delta_to(FxVec2::new(run.max(Fx::ONE), rise).angle());
    Angle(pitch.clamp(-ARM_PITCH_LIMIT, ARM_PITCH_LIMIT) as u16)
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
            deposits: map_file.mass_deposits().to_vec(),
            starts: map_file.start_positions().to_vec(),
            props: map_file.props().to_vec(),
        };
        Self::with_terrain(terrain, map, blueprints, pool, config)
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
                reclaimed_mass: Fx::ZERO,
                units_built: 0,
                units_lost: 0,
                units_killed: 0,
                acts_as: players.len() as u8,
                free_build: false,
            });
        }

        let mut prop_index = SpatialIndex::new(size);
        for (i, p) in map.props.iter().enumerate() {
            prop_index.insert(kind::PROP, i, p.pos, PROP_RADIUS);
        }
        prop_index.build();

        let mut nav = Nav::new(&terrain, pool.clone())?;
        block_buildings(&mut nav, &map.props, size);
        let state = State {
            tick: 0,
            rng: Rng::new(config.seed),
            cheats: config.cheats,
            fog_enabled: config.fog,
            ai: players.iter().map(|_| AiState::default()).collect(),
            players,
            units: Units::new(),
            orders: Orders::new(),
            projectiles: Projectiles::default(),
            wrecks: Wrecks::new(),
            stains: Stains::default(),
            pads: Pads::default(),
            terrain_edits: Vec::new(),
            props_dead: vec![0; map.props.len().div_ceil(64)],
            ai_pending: Vec::new(),
            winner: None,
        };
        let mut world = World {
            blueprints,
            pool,
            map,
            index: SpatialIndex::new(size),
            prop_index,
            fog: Fog::new(size),
            nav,
            terrain,
            state,
            events: Vec::new(),
            spent: Vec::new(),
            muzzles: Vec::new(),
            reclaims: Vec::new(),
            timings: TickTimings::default(),
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
    pub fn team_mask(&self, player: u8) -> u8 {
        let team = self.state.players[player as usize].team;
        let mut mask = 0;
        for (i, p) in self.state.players.iter().enumerate() {
            if p.team == team {
                mask |= 1 << i;
            }
        }
        mask
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
        if bp.is_structure() {
            let (min, max) = footprint_cells(&bp, pos);
            let record = self.terrain.flatten_rect(min, max, None);
            if self.state.terrain_edits.len() >= MAX_FLATTENS {
                return Err(SimError::TableFull(Table::Flattens));
            }
            self.state.terrain_edits.push(TerrainEdit {
                min: (record.min_x, record.min_y),
                max: (record.max_x, record.max_y),
                sample: record.sample,
            });
            self.nav.block_cells(min, max);
            self.events.push(SimEvent::TerrainEdited);
        }
        let z = self.terrain.height_at(pos);
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
        if complete {
            self.remember_structure_pad(&bp, pos, owner)?;
        }
        Ok(row)
    }

    /// Records the poured lot under a structure. Rebuilds on the same centre
    /// share it; walls have none. Extractors get a well pad so the deposit's
    /// cracks stay visible in the middle of the 2x2.
    pub(crate) fn remember_structure_pad(
        &mut self,
        bp: &UnitBlueprint,
        pos: FxVec2,
        owner: u8,
    ) -> Result<(), SimError> {
        if !bp.is_structure() || bp.has(cat::WALL) {
            return Ok(());
        }
        let cells = bp.footprint.0.max(bp.footprint.1) as i32;
        let radius = Fx::from_int(cells * mc_map::BUILD_CELL_M / 2);
        let seed = (pos.x.round_int() as u32).wrapping_mul(7)
            ^ (pos.y.round_int() as u32).wrapping_mul(13);
        let packed = pack_structure_pad(owner, 255, seed, false, bp.needs_deposit);
        self.state.pads.upsert(pos, radius, packed)?;
        Ok(())
    }

    /// Advances the simulation by one tick. `commands` must already be in the
    /// canonical order the session delivers (by player slot, then issue order).
    pub fn tick(&mut self, commands: &[PlayerCommand]) -> Result<u64, SimError> {
        let start = Instant::now();
        let mut last = start;
        self.timings.phases.clear();
        self.events.clear();
        self.spent.clear();
        self.muzzles.clear();
        self.reclaims.clear();
        let mut phase = |timings: &mut TickTimings, name: &'static str| {
            let now = Instant::now();
            timings.phases.push((name, (now - last).as_nanos() as u64));
            last = now;
        };

        self.state.tick += 1;
        let units = &mut self.state.units;
        units.prev_pos.clone_from(&units.pos);
        units.prev_z.clone_from(&units.z);
        units.prev_heading.clone_from(&units.heading);
        units.prev_weapon_yaw.clone_from(&units.weapon_yaw);
        units.prev_arm_pitch.clone_from(&units.arm_pitch);
        for f in &mut units.flags {
            *f &= !flag::TRANSIENT;
        }
        let projectiles = &mut self.state.projectiles;
        projectiles.prev_pos.clone_from(&projectiles.pos);

        self.nav.begin_tick(self.state.tick)?;
        phase(&mut self.timings, "paths");

        let ai_commands = std::mem::take(&mut self.state.ai_pending);
        for c in ai_commands.iter().chain(commands) {
            self.apply_command(c)?;
        }
        phase(&mut self.timings, "commands");

        self.run_orders()?;
        phase(&mut self.timings, "orders");

        self.run_economy()?;
        phase(&mut self.timings, "economy");

        self.run_regen();
        phase(&mut self.timings, "regen");

        self.run_movement()?;
        phase(&mut self.timings, "movement");

        self.rebuild_index();
        phase(&mut self.timings, "index");

        self.run_targeting();
        phase(&mut self.timings, "targeting");

        self.run_weapons()?;
        phase(&mut self.timings, "weapons");

        self.run_projectiles()?;
        phase(&mut self.timings, "projectiles");

        self.reap_dead()?;
        phase(&mut self.timings, "deaths");

        self.update_fog();
        phase(&mut self.timings, "fog");

        self.run_ai()?;
        phase(&mut self.timings, "ai");

        self.check_victory();
        let hash = self.hash();
        phase(&mut self.timings, "hash");
        self.timings.total_ns = start.elapsed().as_nanos() as u64;
        Ok(hash)
    }

    pub(crate) fn rebuild_index(&mut self) {
        let s = &self.state;
        self.index.clear();
        for row in s.units.slots.iter() {
            if !s.units.has_flag(row, flag::IN_FACTORY) {
                let radius = self.blueprints.unit(s.units.blueprint[row]).radius;
                self.index.insert(kind::UNIT, row, s.units.pos[row], radius);
            }
        }
        for row in s.wrecks.slots.iter() {
            let radius = self.blueprints.unit(s.wrecks.blueprint[row]).radius;
            self.index
                .insert(kind::WRECK, row, s.wrecks.pos[row], radius);
        }
        for row in 0..s.stains.len() {
            self.index
                .insert(kind::STAIN, row, s.stains.pos[row], s.stains.radius[row]);
        }
        self.index.build();
    }

    /// True when a unit entry from the index still describes a live unit. The
    /// index is rebuilt mid-tick, so early phases can see rows that have since
    /// died or been reused.
    #[inline]
    pub(crate) fn unit_entry_is_current(&self, e: &crate::spatial::Entry) -> bool {
        let row = e.row as usize;
        self.state.units.slots.is_alive(row) && self.state.units.pos[row] == e.pos
    }

    pub(crate) fn update_fog(&mut self) {
        self.fog.begin();
        let s = &self.state;
        let masks: Vec<u8> = (0..s.players.len())
            .map(|p| self.team_mask(p as u8))
            .collect();
        for row in s.units.slots.iter() {
            if s.units.has_flag(row, flag::IN_FACTORY) {
                continue;
            }
            let bp = self.blueprints.unit(s.units.blueprint[row]);
            // Construction sites see a little so their owner can watch them.
            let (vision, radar) = if s.units.is_active(row) {
                (bp.vision, bp.radar)
            } else {
                (bp.radius * 2, Fx::ZERO)
            };
            self.fog.reveal(
                s.units.pos[row],
                vision,
                radar,
                masks[s.units.owner[row] as usize],
            );
        }
        for row in s.units.slots.iter() {
            if s.units.has_flag(row, flag::IN_FACTORY) {
                continue;
            }
            let seen = self.fog.visible_mask(s.units.pos[row]);
            if seen != 0 {
                self.fog.identify(row, s.units.id(row).generation(), seen);
            }
        }
    }

    /// Can `player` shoot at / see the unit in `row`?
    #[inline]
    pub fn detects(&self, player: u8, row: usize) -> bool {
        !self.state.fog_enabled
            || self.state.units.owner[row] == player
            || self.fog.is_detected(self.state.units.pos[row], 1 << player)
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

    /// Hash of the whole game state. Equal hashes on every machine, every tick, or it is a desync.
    pub fn hash(&self) -> u64 {
        let s = &self.state;
        let mut h = StateHasher::new();
        h.write_u64(s.tick as u64);
        h.write_u64(s.rng.state());
        h.write_u64(s.winner.map_or(u64::MAX, |w| w as u64));
        for p in &s.players {
            p.hash(&mut h);
        }
        s.units.hash(&mut h);
        s.orders.hash(&mut h);
        s.projectiles.hash(&mut h);
        s.wrecks.hash(&mut h);
        s.stains.hash(&mut h);
        s.pads.hash(&mut h);
        h.write_u64(s.terrain_edits.len() as u64);
        if let Some(e) = s.terrain_edits.last() {
            h.write_u64(e.min.0 as u64 | (e.min.1 as u64) << 32);
            h.write_u64(e.max.0 as u64 | (e.max.1 as u64) << 32);
            h.write_u64(e.sample as u64);
        }
        h.write_u64s(&s.props_dead);
        for ai in &s.ai {
            ai.hash(&mut h);
        }
        h.write_u64(s.ai_pending.len() as u64);
        self.nav.hash(&mut h);
        h.finish()
    }

    /// Serialises the game state for late join, reconnect and save games.
    /// Call between ticks. May wait for flow-field builds in flight to finish.
    pub fn snapshot(&mut self) -> Vec<u8> {
        let nav = self.nav.snapshot();
        bincode::serialize(&(&self.state, &nav)).expect("state always serialises")
    }

    /// Replaces this world's state with a snapshot taken on another machine
    /// running the same map and blueprints. The restored world steps exactly
    /// as the source does from the next tick on.
    ///
    /// `base_terrain` is the map's terrain as baked, before any edits.
    pub fn restore(&mut self, base_terrain: Heightfield, bytes: &[u8]) -> Result<(), SimError> {
        let (state, nav): (State, crate::nav::NavSnapshot) =
            bincode::deserialize(bytes).map_err(|e| SimError::Snapshot(e.to_string()))?;
        if state.players.len() != self.state.players.len()
            || state.props_dead.len() != self.state.props_dead.len()
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
        self.rebuild_index();
        self.update_fog();
        self.events.push(SimEvent::TerrainEdited);
        Ok(())
    }

    pub fn is_prop_alive(&self, prop: usize) -> bool {
        self.state.props_dead[prop / 64] & (1 << (prop % 64)) == 0
    }
}

/// City buildings are solid. Their lots are blocked on the build grid, once,
/// before the match starts.
fn block_buildings(nav: &mut Nav, props: &[Prop], map_size: FxVec2) {
    for p in props {
        if let Some((min, max)) = building_cells(p, map_size) {
            nav.block_cells(min, max);
        }
    }
}

fn building_cells(p: &Prop, map_size: FxVec2) -> Option<((u32, u32), (u32, u32))> {
    if !p.kind.is_building() {
        return None;
    }
    let grid = mc_map::BUILD_CELL_M;
    let half = 20 * p.scale_milli as i32 / 1000 + 2;
    let (x, y) = (p.pos.x.round_int(), p.pos.y.round_int());
    let lo = |v: i32| ((v - half).div_euclid(grid) * grid).max(0);
    let hi = |v: i32, limit: i32| (((v + half + grid - 1).div_euclid(grid)) * grid).min(limit);
    let (x0, y0) = (lo(x), lo(y));
    let (x1, y1) = (hi(x, map_size.x.floor_int()), hi(y, map_size.y.floor_int()));
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let cell = mc_map::CELL_SIZE_M;
    Some((
        ((x0 / cell) as u32, (y0 / cell) as u32),
        ((x1 / cell - 1) as u32, (y1 / cell - 1) as u32),
    ))
}

fn cells_overlap(a: ((u32, u32), (u32, u32)), b: ((u32, u32), (u32, u32))) -> bool {
    a.0 .0 <= b.1 .0 && b.0 .0 <= a.1 .0 && a.0 .1 <= b.1 .1 && b.0 .1 <= a.1 .1
}

/// Pad `packed` bit: an extractor well, one poured slab per cell of the 2x2
/// with the crack pit left open. The pad shader reads this at bit 3.
pub const PAD_WELL: u32 = 1 << 3;

/// Packing the pad shader reads: owner in 0..3, well flag at bit 3, build in
/// 8..16, seed in 16..24, ghost at bit 24.
pub fn pack_structure_pad(owner: u8, build: u8, seed: u32, ghost: bool, well: bool) -> u32 {
    (owner as u32 & 7)
        | if well { PAD_WELL } else { 0 }
        | (build as u32) << 8
        | (seed & 0xFF) << 16
        | u32::from(ghost) << 24
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

/// Inclusive path-cell rectangle covered by a structure centred at `pos`.
/// Rounded outward so a 12 m lot always covers the 8 m cells it overlaps.
pub fn footprint_cells(bp: &UnitBlueprint, pos: FxVec2) -> ((u32, u32), (u32, u32)) {
    path_cells_of(bp.footprint, pos)
}

fn path_cells_of(footprint: (u8, u8), pos: FxVec2) -> ((u32, u32), (u32, u32)) {
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
/// lots share the 8 m cells their edges round into; those cells block
/// pathing but must not block placement.
fn place_cells_of(footprint: (u8, u8), pos: FxVec2) -> ((u32, u32), (u32, u32)) {
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
    /// allows, not overlapping another structure, and on a free deposit when
    /// the blueprint needs one.
    pub fn can_place(&self, bp: &UnitBlueprint, pos: FxVec2) -> bool {
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
        // Neighbouring 2x2s share the overhang cells, and those must stay
        // blocked for pathing without forbidding the next building.
        let path = footprint_cells(bp, pos);
        if !self.nav.passable_terrain(path.0, path.1) {
            return false;
        }
        let place = place_cells_of(bp.footprint, pos);
        if !self.nav.no_blockers(place.0, place.1) {
            return false;
        }
        if bp.needs_deposit {
            let on_deposit = self
                .map
                .deposits
                .iter()
                .any(|d| snap_to_build_grid(bp, *d) == pos);
            if !on_deposit {
                return false;
            }
        }
        true
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
                let (min, max) = footprint_cells(bp, e.pos);
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

    /// Drops this lot's path blockers, then puts back any still needed by a
    /// neighbour or a city. Adjacent 12 m lots share the 8 m overhang cells.
    pub(crate) fn release_lot(&mut self, except: usize, min: (u32, u32), max: (u32, u32)) {
        self.nav.unblock_cells(min, max);
        let released = (min, max);
        let restore = {
            let units = &self.state.units;
            let mut restore = Vec::new();
            for row in units.slots.iter() {
                if row == except {
                    continue;
                }
                let bp = self.blueprints.unit(units.blueprint[row]);
                if !bp.is_structure() {
                    continue;
                }
                let other = footprint_cells(bp, units.pos[row]);
                if cells_overlap(released, other) {
                    restore.push(other);
                }
            }
            let map_size = self.terrain.size_metres();
            for p in &self.map.props {
                if let Some(cells) = building_cells(p, map_size) {
                    if cells_overlap(released, cells) {
                        restore.push(cells);
                    }
                }
            }
            restore
        };
        for (min, max) in restore {
            self.nav.block_cells(min, max);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neighbouring_2x2_lots_share_path_cells_not_place_cells() {
        let a = FxVec2::from_ints(24, 24);
        let b = FxVec2::from_ints(48, 24);
        let fp = (2, 2);
        assert!(
            cells_overlap(path_cells_of(fp, a), path_cells_of(fp, b)),
            "pathing must cover the 4 m overhang both lots round into"
        );
        assert!(
            !cells_overlap(place_cells_of(fp, a), place_cells_of(fp, b)),
            "placement must let them sit edge to edge"
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
}
