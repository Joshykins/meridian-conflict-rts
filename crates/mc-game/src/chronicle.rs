//! The match's record for the battle report (`ui/report`): every side's economy and
//! forces sampled every two seconds, where its armies, bases and commander stood every
//! five, and every death, kill, finished unit, defeat and warhead as it happened.
//!
//! Kept on the sim thread, which sees the whole match whatever the fog. It holds raw
//! logs only; what the report shows is worked out from them when it opens
//! (`ui::report::analysis`), so a replay seeking backwards just cuts the logs back.

use glam::Vec2;
use mc_data::{cat, BlueprintId};
use mc_sim::tables::flag;
use mc_sim::{SimEvent, World};

/// Ticks between economy samples.
pub const SAMPLE_TICKS: u32 = 20;
/// Ticks between snapshots of where everything stands.
pub const FRAME_TICKS: u32 = 50;
/// Cells a side of the map is split into for the snapshots.
pub const GRID: usize = 64;

#[derive(Clone, Debug, Default)]
pub struct SideInfo {
    pub name: String,
    pub team: u8,
    pub faction: u8,
    pub ai: bool,
}

/// One side's economy and forces at a sample.
#[derive(Clone, Copy, Debug, Default)]
pub struct SideSample {
    /// Materials and energy a second: made (mines and generators), reclaimed, spent.
    pub mass_income: f32,
    pub reclaim_income: f32,
    pub energy_income: f32,
    pub mass_spent: f32,
    pub energy_spent: f32,
    /// Materials in store.
    pub mass: f32,
    /// Share of the asked-for spending that was paid, zero to one.
    pub efficiency: f32,
    /// Mobile fighters (not engineers or the commander), and their worth in materials.
    pub army: u32,
    pub army_value: f32,
    pub engineers: u32,
    /// Finished structures, and their worth.
    pub structures: u32,
    pub structure_value: f32,
    pub mines: u32,
    pub factories: u32,
}

#[derive(Clone, Debug)]
pub struct Sample {
    pub tick: u32,
    pub sides: Vec<SideSample>,
}

/// Where one side stood at a snapshot: cells of `GRID` (row-major, y down the map).
#[derive(Clone, Debug, Default)]
pub struct SideFrame {
    /// Each occupied cell and the worth of the army in it.
    pub army: Vec<(u16, f32)>,
    /// Each cell with a finished structure, and how many.
    pub bases: Vec<(u16, u16)>,
    pub commander: Option<Vec2>,
}

#[derive(Clone, Debug)]
pub struct Frame {
    pub tick: u32,
    pub sides: Vec<SideFrame>,
}

#[derive(Clone, Copy, Debug)]
pub struct Death {
    pub tick: u32,
    pub pos: Vec2,
    pub owner: u8,
    pub blueprint: BlueprintId,
    pub complete: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Kill {
    pub tick: u32,
    pub by: u8,
    pub victim: u8,
    pub blueprint: BlueprintId,
    pub weapon_of: Option<BlueprintId>,
    pub complete: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Built {
    pub tick: u32,
    pub owner: u8,
    pub blueprint: BlueprintId,
}

/// A nuclear warhead's blast (a commander's reactor is a `Death`, not one of these).
#[derive(Clone, Copy, Debug)]
pub struct Blast {
    pub tick: u32,
    pub pos: Vec2,
    pub owner: u8,
}

#[derive(Clone, Debug, Default)]
pub struct Chronicle {
    pub sides: Vec<SideInfo>,
    /// The map's extent in metres.
    pub size: Vec2,
    pub samples: Vec<Sample>,
    pub frames: Vec<Frame>,
    pub deaths: Vec<Death>,
    pub kills: Vec<Kill>,
    pub built: Vec<Built>,
    pub defeats: Vec<(u32, u8)>,
    pub blasts: Vec<Blast>,
    /// The tick the match was decided, and the winning team. Nothing after it is kept.
    pub ended: Option<(u32, u8)>,
    /// The last tick recorded.
    pub tick: u32,
}

impl Chronicle {
    pub fn new(size: Vec2) -> Chronicle {
        Chronicle {
            size: size.max(Vec2::ONE),
            ..Default::default()
        }
    }

    /// Takes in one tick: its events, and a sample or snapshot when one is due. Call it
    /// after every tick, a replay's rushed ones included.
    pub fn record(&mut self, world: &World) {
        let tick = world.state.tick;
        if tick < self.tick {
            self.rewind(tick);
        }
        if self.ended.is_some() {
            return;
        }
        if self.sides.is_empty() {
            self.sides = world
                .state
                .players
                .iter()
                .map(|p| SideInfo {
                    name: p.name.clone(),
                    team: p.team,
                    faction: p.faction,
                    ai: p.controller == mc_sim::tables::Controller::Ai,
                })
                .collect();
        }
        let fresh = tick != self.tick || self.samples.is_empty();
        self.tick = tick;
        self.take_events(world, tick);
        if (fresh && tick.is_multiple_of(SAMPLE_TICKS)) || self.ended.is_some() {
            self.sample(world, tick);
        }
        if (fresh && tick.is_multiple_of(FRAME_TICKS)) || self.ended.is_some() {
            self.frame(world, tick);
        }
    }

    /// The match as far as `tick`: a replay went back to it.
    fn rewind(&mut self, tick: u32) {
        self.samples.retain(|s| s.tick <= tick);
        self.frames.retain(|f| f.tick <= tick);
        self.deaths.retain(|d| d.tick <= tick);
        self.kills.retain(|k| k.tick <= tick);
        self.built.retain(|b| b.tick <= tick);
        self.defeats.retain(|d| d.0 <= tick);
        self.blasts.retain(|b| b.tick <= tick);
        if self.ended.is_some_and(|e| e.0 > tick) {
            self.ended = None;
        }
        self.tick = tick;
    }

    fn take_events(&mut self, world: &World, tick: u32) {
        let units = &world.state.units;
        let xy = |p: mc_core::FxVec3| Vec2::new(p.x.to_f32(), p.y.to_f32());
        for event in &world.events {
            match *event {
                SimEvent::UnitDied {
                    pos,
                    blueprint,
                    owner,
                    complete,
                    ..
                } => self.deaths.push(Death {
                    tick,
                    pos: xy(pos),
                    owner,
                    blueprint,
                    complete,
                }),
                SimEvent::UnitKilled {
                    blueprint,
                    owner,
                    by,
                    weapon_of,
                    complete,
                } if (by as usize) < self.sides.len() => self.kills.push(Kill {
                    tick,
                    by,
                    victim: owner,
                    blueprint,
                    weapon_of,
                    complete,
                }),
                SimEvent::UnitCompleted { unit, owner } => {
                    if let Some(row) = units.row(unit) {
                        self.built.push(Built {
                            tick,
                            owner,
                            blueprint: units.blueprint[row],
                        });
                    }
                }
                SimEvent::PlayerDefeated { player } => self.defeats.push((tick, player)),
                SimEvent::NuclearDetonation {
                    pos,
                    owner,
                    commander: false,
                    ..
                } => self.blasts.push(Blast {
                    tick,
                    pos: xy(pos),
                    owner,
                }),
                SimEvent::MatchOver { winner_team } => self.ended = Some((tick, winner_team)),
                _ => {}
            }
        }
    }

    fn sample(&mut self, world: &World, tick: u32) {
        let f = |v: mc_core::Fx| v.to_f32();
        let mut sides: Vec<SideSample> = world
            .state
            .players
            .iter()
            .map(|p| SideSample {
                mass_income: f(p.mass_income),
                reclaim_income: f(p.reclaim_income),
                energy_income: f(p.energy_income),
                mass_spent: f(p.mass_spent),
                energy_spent: f(p.energy_spent),
                mass: f(p.mass),
                efficiency: f(p.efficiency),
                ..Default::default()
            })
            .collect();
        let units = &world.state.units;
        for row in units.slots.iter() {
            let Some(s) = sides.get_mut(units.owner[row] as usize) else {
                continue;
            };
            if units.flags[row] & (flag::UNDER_CONSTRUCTION | flag::IN_FACTORY | flag::UPGRADE) != 0
            {
                continue;
            }
            let bp = world.bp(row);
            if !bp.is_mobile() {
                s.structures += 1;
                s.structure_value += bp.cost_mass.to_f32();
                s.mines += bp.has(cat::EXTRACTOR) as u32;
                s.factories += bp.has(cat::FACTORY) as u32;
            } else if bp.has(cat::ENGINEER) {
                s.engineers += 1;
            } else if !bp.has(cat::COMMANDER) && !bp.is_salvager() {
                s.army += 1;
                s.army_value += bp.cost_mass.to_f32();
            }
        }
        if self.samples.last().is_some_and(|s| s.tick == tick) {
            self.samples.pop();
        }
        self.samples.push(Sample { tick, sides });
    }

    fn frame(&mut self, world: &World, tick: u32) {
        let mut sides = vec![SideFrame::default(); self.sides.len()];
        let mut army = vec![0.0f32; GRID * GRID];
        let mut bases = vec![0u16; GRID * GRID];
        let units = &world.state.units;
        for (i, side) in sides.iter_mut().enumerate() {
            army.fill(0.0);
            bases.fill(0);
            for row in units.slots.iter().filter(|&r| units.owner[r] as usize == i) {
                if units.flags[row] & (flag::UNDER_CONSTRUCTION | flag::IN_FACTORY) != 0 {
                    continue;
                }
                let bp = world.bp(row);
                let at = Vec2::from(units.pos[row].to_f32());
                if bp.has(cat::COMMANDER) {
                    side.commander = Some(at);
                } else if !bp.is_mobile() {
                    let c = self.cell(at);
                    bases[c] = bases[c].saturating_add(1);
                } else if !bp.has(cat::ENGINEER) && !bp.is_salvager() {
                    army[self.cell(at)] += bp.cost_mass.to_f32();
                }
            }
            side.army = (0..GRID * GRID)
                .filter(|&c| army[c] > 0.0)
                .map(|c| (c as u16, army[c]))
                .collect();
            side.bases = (0..GRID * GRID)
                .filter(|&c| bases[c] > 0)
                .map(|c| (c as u16, bases[c]))
                .collect();
        }
        if self.frames.last().is_some_and(|f| f.tick == tick) {
            self.frames.pop();
        }
        self.frames.push(Frame { tick, sides });
    }

    /// The `GRID` cell a point of the map lies in.
    fn cell(&self, at: Vec2) -> usize {
        let k = (at / self.size * GRID as f32).clamp(Vec2::ZERO, Vec2::splat(GRID as f32 - 1.0));
        k.y as usize * GRID + k.x as usize
    }
}

/// The middle of `GRID` cell `c` of a map `size` metres across.
pub fn cell_centre(size: Vec2, c: u16) -> Vec2 {
    let (x, y) = (c as usize % GRID, c as usize / GRID);
    (Vec2::new(x as f32, y as f32) + 0.5) / GRID as f32 * size
}
