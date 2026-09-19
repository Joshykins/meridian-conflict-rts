//! The state tables. Together with the tick counter and the RNG these are the
//! whole game state: struct-of-arrays columns, one row per entity.

pub use crate::slots::Handle;
use crate::slots::{put, Slots};
use crate::{SimError, Table};
use mc_core::{Angle, Fx, FxVec2, FxVec3, StateHasher};
use mc_data::{BlueprintId, MAX_WEAPONS};
use serde::{Deserialize, Serialize};

pub type UnitId = Handle;
pub type WreckId = Handle;

pub const MAX_UNITS: usize = 8_192;
pub const MAX_PROJECTILES: usize = 16_384;
pub const MAX_WRECKS: usize = 16_384;
pub const MAX_STAINS: usize = 32_768;
pub const MAX_ORDERS: usize = 131_072;
pub const MAX_FLATTENS: usize = 32_768;

pub const NO_ORDER: u32 = u32::MAX;
pub const NO_FIELD: u32 = u32::MAX;

/// Unit flag bits.
pub mod flag {
    /// Still being built; does not produce, fight or move.
    pub const UNDER_CONSTRUCTION: u16 = 1 << 0;
    /// Being assembled inside a factory: no collision, not targetable.
    pub const IN_FACTORY: u16 = 1 << 1;
    /// A builder spent resources on its target this tick (drives build beams).
    pub const BUILDING: u16 = 1 << 2;
    /// Moved this tick.
    pub const MOVING: u16 = 1 << 3;
    /// Holds a reference on its flow field (`field` column).
    pub const HAS_FIELD: u16 = 1 << 4;
    /// This structure is the upgrade of the unit in `build_target` and replaces it when done.
    pub const UPGRADE: u16 = 1 << 5;
    /// Factory repeats its queue.
    pub const REPEAT: u16 = 1 << 6;
    /// The unit's order wants it to stand still this tick (in range, engaging, working).
    pub const HOLD: u16 = 1 << 7;
    /// Pulling mass out of a wreck this tick.
    pub const RECLAIMING: u16 = 1 << 8;

    /// Flags that describe one tick only; cleared when the next tick starts.
    pub const TRANSIENT: u16 = BUILDING | MOVING | HOLD | RECLAIMING;
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Units {
    pub slots: Slots,
    pub blueprint: Vec<BlueprintId>,
    pub owner: Vec<u8>,
    pub flags: Vec<u16>,
    pub pos: Vec<FxVec2>,
    pub z: Vec<Fx>,
    pub heading: Vec<Angle>,
    /// Signed speed along the heading, metres per second.
    pub speed: Vec<Fx>,
    /// Where the unit was at the end of the previous tick, for render interpolation.
    pub prev_pos: Vec<FxVec2>,
    pub prev_z: Vec<Fx>,
    pub prev_heading: Vec<Angle>,
    pub health: Vec<Fx>,
    /// Build-time units accumulated, up to the blueprint's `build_time`.
    pub build_progress: Vec<Fx>,
    /// Order queue: a linked list in the `Orders` pool.
    pub order_head: Vec<u32>,
    pub order_tail: Vec<u32>,
    /// Flow field the unit is following, `NO_FIELD` when none.
    pub field: Vec<u32>,
    /// Goal the field in `field` leads to.
    pub field_goal: Vec<FxVec2>,
    /// The unit's own destination: the order's target plus its formation offset.
    pub move_goal: Vec<FxVec2>,
    /// Ticks spent failing to get closer to `move_goal`.
    pub stuck_ticks: Vec<u16>,
    /// The thing this unit is constructing, assisting or upgrading from.
    pub build_target: Vec<UnitId>,
    pub rally: Vec<FxVec2>,
    pub weapon_cooldown: Vec<[u16; MAX_WEAPONS]>,
    pub weapon_salvo_left: Vec<[u8; MAX_WEAPONS]>,
    /// Turret yaw relative to the hull.
    pub weapon_yaw: Vec<[Angle; MAX_WEAPONS]>,
    pub weapon_target: Vec<[UnitId; MAX_WEAPONS]>,
}

pub struct UnitSpawn {
    pub blueprint: BlueprintId,
    pub owner: u8,
    pub pos: FxVec2,
    pub z: Fx,
    pub heading: Angle,
    pub health: Fx,
    pub flags: u16,
    pub build_progress: Fx,
}

impl Units {
    pub fn new() -> Units {
        Units {
            slots: Slots::new(MAX_UNITS),
            blueprint: Vec::new(),
            owner: Vec::new(),
            flags: Vec::new(),
            pos: Vec::new(),
            z: Vec::new(),
            heading: Vec::new(),
            speed: Vec::new(),
            prev_pos: Vec::new(),
            prev_z: Vec::new(),
            prev_heading: Vec::new(),
            health: Vec::new(),
            build_progress: Vec::new(),
            order_head: Vec::new(),
            order_tail: Vec::new(),
            field: Vec::new(),
            field_goal: Vec::new(),
            move_goal: Vec::new(),
            stuck_ticks: Vec::new(),
            build_target: Vec::new(),
            rally: Vec::new(),
            weapon_cooldown: Vec::new(),
            weapon_salvo_left: Vec::new(),
            weapon_yaw: Vec::new(),
            weapon_target: Vec::new(),
        }
    }

    pub fn spawn(&mut self, s: UnitSpawn) -> Result<usize, SimError> {
        let row = self.slots.alloc().ok_or(SimError::TableFull(Table::Units))?;
        put(&mut self.blueprint, row, s.blueprint);
        put(&mut self.owner, row, s.owner);
        put(&mut self.flags, row, s.flags);
        put(&mut self.pos, row, s.pos);
        put(&mut self.z, row, s.z);
        put(&mut self.heading, row, s.heading);
        put(&mut self.speed, row, Fx::ZERO);
        put(&mut self.prev_pos, row, s.pos);
        put(&mut self.prev_z, row, s.z);
        put(&mut self.prev_heading, row, s.heading);
        put(&mut self.health, row, s.health);
        put(&mut self.build_progress, row, s.build_progress);
        put(&mut self.order_head, row, NO_ORDER);
        put(&mut self.order_tail, row, NO_ORDER);
        put(&mut self.field, row, NO_FIELD);
        put(&mut self.field_goal, row, s.pos);
        put(&mut self.move_goal, row, s.pos);
        put(&mut self.stuck_ticks, row, 0);
        put(&mut self.build_target, row, Handle::NONE);
        put(&mut self.rally, row, s.pos);
        put(&mut self.weapon_cooldown, row, [0; MAX_WEAPONS]);
        put(&mut self.weapon_salvo_left, row, [0; MAX_WEAPONS]);
        put(&mut self.weapon_yaw, row, [Angle::ZERO; MAX_WEAPONS]);
        put(&mut self.weapon_target, row, [Handle::NONE; MAX_WEAPONS]);
        Ok(row)
    }

    #[inline]
    pub fn id(&self, row: usize) -> UnitId {
        self.slots.handle(row)
    }

    #[inline]
    pub fn row(&self, id: UnitId) -> Option<usize> {
        self.slots.resolve(id)
    }

    #[inline]
    pub fn has_flag(&self, row: usize, f: u16) -> bool {
        self.flags[row] & f != 0
    }

    /// Complete and out of the factory: takes part in the economy, combat and movement.
    #[inline]
    pub fn is_active(&self, row: usize) -> bool {
        self.flags[row] & (flag::UNDER_CONSTRUCTION | flag::IN_FACTORY) == 0
    }

    pub fn hash(&self, h: &mut StateHasher) {
        self.slots.hash(h);
        for row in self.slots.iter() {
            h.write_u64(self.blueprint[row].0 as u64 | (self.owner[row] as u64) << 16 | (self.flags[row] as u64) << 24 | (self.heading[row].0 as u64) << 40);
            h.write_i64(self.pos[row].x.0);
            h.write_i64(self.pos[row].y.0);
            h.write_i64(self.z[row].0);
            h.write_i64(self.speed[row].0);
            h.write_i64(self.health[row].0);
            h.write_i64(self.build_progress[row].0);
            h.write_u64(self.order_head[row] as u64 | (self.order_tail[row] as u64) << 32);
            h.write_u64(self.field[row] as u64 | (self.stuck_ticks[row] as u64) << 32);
            h.write_i64(self.field_goal[row].x.0);
            h.write_i64(self.field_goal[row].y.0);
            h.write_i64(self.move_goal[row].x.0);
            h.write_i64(self.move_goal[row].y.0);
            h.write_u64(self.build_target[row].0 as u64);
            h.write_i64(self.rally[row].x.0);
            h.write_i64(self.rally[row].y.0);
            for w in 0..MAX_WEAPONS {
                h.write_u64(
                    self.weapon_cooldown[row][w] as u64
                        | (self.weapon_salvo_left[row][w] as u64) << 16
                        | (self.weapon_yaw[row][w].0 as u64) << 24
                        | (self.weapon_target[row][w].0 as u64) << 40,
                );
            }
        }
    }
}

impl Default for Units {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[repr(u8)]
pub enum OrderKind {
    Move,
    /// Move, engaging anything met on the way.
    AttackMove,
    Attack,
    /// Construct `blueprint` at `pos`.
    Build,
    /// Help a builder or finish/repair a unit.
    Assist,
    Reclaim,
    /// Factory queue entry: produce one `blueprint`.
    Produce,
    /// Structure: build `blueprint` in place of this unit.
    Upgrade,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Order {
    pub kind: OrderKind,
    pub pos: FxVec2,
    /// Unit or wreck, depending on `kind`.
    pub target: Handle,
    pub blueprint: BlueprintId,
    pub heading: Angle,
    /// Offset from `pos` this unit keeps, so a group arrives in formation.
    pub offset: FxVec2,
}

/// Pool of order nodes. Each unit's queue is a singly linked list through `next`.
#[derive(Clone, Serialize, Deserialize)]
pub struct Orders {
    pub order: Vec<Order>,
    pub next: Vec<u32>,
    free: Vec<u32>,
}

impl Orders {
    pub fn new() -> Orders {
        Orders { order: Vec::new(), next: Vec::new(), free: Vec::new() }
    }

    pub fn live(&self) -> usize {
        self.order.len() - self.free.len()
    }

    fn alloc(&mut self, order: Order) -> Result<u32, SimError> {
        if let Some(i) = self.free.pop() {
            self.order[i as usize] = order;
            self.next[i as usize] = NO_ORDER;
            return Ok(i);
        }
        if self.order.len() >= MAX_ORDERS {
            return Err(SimError::TableFull(Table::Orders));
        }
        self.order.push(order);
        self.next.push(NO_ORDER);
        Ok(self.order.len() as u32 - 1)
    }

    pub fn push_back(&mut self, units: &mut Units, row: usize, order: Order) -> Result<(), SimError> {
        let node = self.alloc(order)?;
        match units.order_tail[row] {
            NO_ORDER => units.order_head[row] = node,
            tail => self.next[tail as usize] = node,
        }
        units.order_tail[row] = node;
        Ok(())
    }

    pub fn push_front(&mut self, units: &mut Units, row: usize, order: Order) -> Result<(), SimError> {
        let node = self.alloc(order)?;
        self.next[node as usize] = units.order_head[row];
        if units.order_head[row] == NO_ORDER {
            units.order_tail[row] = node;
        }
        units.order_head[row] = node;
        Ok(())
    }

    pub fn front<'a>(&'a self, units: &Units, row: usize) -> Option<&'a Order> {
        match units.order_head[row] {
            NO_ORDER => None,
            head => Some(&self.order[head as usize]),
        }
    }

    pub fn pop_front(&mut self, units: &mut Units, row: usize) -> Option<Order> {
        let head = units.order_head[row];
        if head == NO_ORDER {
            return None;
        }
        let next = self.next[head as usize];
        units.order_head[row] = next;
        if next == NO_ORDER {
            units.order_tail[row] = NO_ORDER;
        }
        self.free.push(head);
        Some(self.order[head as usize])
    }

    pub fn clear(&mut self, units: &mut Units, row: usize) {
        while self.pop_front(units, row).is_some() {}
    }

    /// Queue contents front to back. Bounded by the pool size.
    pub fn iter<'a>(&'a self, units: &Units, row: usize) -> impl Iterator<Item = &'a Order> + 'a {
        let mut node = units.order_head[row];
        std::iter::from_fn(move || {
            (node != NO_ORDER).then(|| {
                let o = &self.order[node as usize];
                node = self.next[node as usize];
                o
            })
        })
    }

    pub fn hash(&self, h: &mut StateHasher) {
        h.write_u32s(&self.next);
        h.write_u32s(&self.free);
        for o in &self.order {
            h.write_u64(o.kind as u64 | (o.blueprint.0 as u64) << 8 | (o.heading.0 as u64) << 24);
            h.write_u64(o.target.0 as u64);
            for v in [o.pos.x, o.pos.y, o.offset.x, o.offset.y] {
                h.write_i64(v.0);
            }
        }
    }
}

impl Default for Orders {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Controller {
    Human,
    Ai,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Player {
    pub name: String,
    pub faction: u8,
    /// Players with the same team are allied and share vision.
    pub team: u8,
    pub controller: Controller,
    pub start: FxVec2,
    pub defeated: bool,
    pub commander: UnitId,
    pub mass: Fx,
    pub energy: Fx,
    pub mass_capacity: Fx,
    pub energy_capacity: Fx,
    /// Last tick's flows in units per second, for the UI and the AI.
    pub mass_income: Fx,
    pub energy_income: Fx,
    pub mass_demand: Fx,
    pub energy_demand: Fx,
    /// Share of requested spending that was met last tick, zero to one.
    pub efficiency: Fx,
    pub reclaimed_mass: Fx,
    pub units_built: u32,
    pub units_lost: u32,
    pub units_killed: u32,
}

impl Player {
    pub fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.faction as u64 | (self.team as u64) << 8 | (self.defeated as u64) << 16 | (self.controller as u64) << 17);
        h.write_u64(self.commander.0 as u64);
        for v in [self.mass, self.energy, self.mass_capacity, self.energy_capacity, self.efficiency, self.reclaimed_mass] {
            h.write_i64(v.0);
        }
        h.write_u64(self.units_built as u64 | (self.units_lost as u64) << 32);
        h.write_u64(self.units_killed as u64);
    }
}

/// Projectiles are never referenced, so the table is dense and removal swaps.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Projectiles {
    pub pos: Vec<FxVec3>,
    pub prev_pos: Vec<FxVec3>,
    /// Metres per tick.
    pub vel: Vec<FxVec3>,
    pub owner: Vec<u8>,
    pub source: Vec<UnitId>,
    /// Blueprint and weapon slot the shot came from; damage and splash are looked up there.
    pub blueprint: Vec<BlueprintId>,
    pub weapon: Vec<u8>,
    pub ticks_left: Vec<u16>,
}

impl Projectiles {
    pub fn len(&self) -> usize {
        self.pos.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pos.is_empty()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn spawn(&mut self, pos: FxVec3, vel: FxVec3, owner: u8, source: UnitId, blueprint: BlueprintId, weapon: u8, ticks: u16) -> Result<(), SimError> {
        if self.len() >= MAX_PROJECTILES {
            return Err(SimError::TableFull(Table::Projectiles));
        }
        self.pos.push(pos);
        self.prev_pos.push(pos);
        self.vel.push(vel);
        self.owner.push(owner);
        self.source.push(source);
        self.blueprint.push(blueprint);
        self.weapon.push(weapon);
        self.ticks_left.push(ticks);
        Ok(())
    }

    pub fn swap_remove(&mut self, row: usize) {
        self.pos.swap_remove(row);
        self.prev_pos.swap_remove(row);
        self.vel.swap_remove(row);
        self.owner.swap_remove(row);
        self.source.swap_remove(row);
        self.blueprint.swap_remove(row);
        self.weapon.swap_remove(row);
        self.ticks_left.swap_remove(row);
    }

    pub fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.len() as u64);
        for i in 0..self.len() {
            for v in [self.pos[i], self.vel[i]] {
                h.write_i64(v.x.0);
                h.write_i64(v.y.0);
                h.write_i64(v.z.0);
            }
            h.write_u64(self.owner[i] as u64 | (self.weapon[i] as u64) << 8 | (self.blueprint[i].0 as u64) << 16 | (self.ticks_left[i] as u64) << 32);
            h.write_u64(self.source[i].0 as u64);
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Wrecks {
    pub slots: Slots,
    pub blueprint: Vec<BlueprintId>,
    pub pos: Vec<FxVec2>,
    pub z: Vec<Fx>,
    pub heading: Vec<Angle>,
    pub mass: Vec<Fx>,
    pub mass_max: Vec<Fx>,
}

impl Wrecks {
    pub fn new() -> Wrecks {
        Wrecks { slots: Slots::new(MAX_WRECKS), blueprint: Vec::new(), pos: Vec::new(), z: Vec::new(), heading: Vec::new(), mass: Vec::new(), mass_max: Vec::new() }
    }

    pub fn spawn(&mut self, blueprint: BlueprintId, pos: FxVec2, z: Fx, heading: Angle, mass: Fx) -> Result<usize, SimError> {
        let row = self.slots.alloc().ok_or(SimError::TableFull(Table::Wrecks))?;
        put(&mut self.blueprint, row, blueprint);
        put(&mut self.pos, row, pos);
        put(&mut self.z, row, z);
        put(&mut self.heading, row, heading);
        put(&mut self.mass, row, mass);
        put(&mut self.mass_max, row, mass);
        Ok(row)
    }

    pub fn hash(&self, h: &mut StateHasher) {
        self.slots.hash(h);
        for row in self.slots.iter() {
            h.write_u64(self.blueprint[row].0 as u64 | (self.heading[row].0 as u64) << 16);
            h.write_i64(self.pos[row].x.0);
            h.write_i64(self.pos[row].y.0);
            h.write_i64(self.mass[row].0);
        }
    }
}

impl Default for Wrecks {
    fn default() -> Self {
        Self::new()
    }
}

/// Scorch marks and craters. They are decals: the ground itself is untouched.
/// Append-only; overlapping impacts deepen an existing stain instead of adding one.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Stains {
    pub pos: Vec<FxVec2>,
    pub radius: Vec<Fx>,
    /// 1..=255, how dark the mark is.
    pub strength: Vec<u8>,
    pub seed: Vec<u16>,
}

impl Stains {
    pub fn len(&self) -> usize {
        self.pos.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pos.is_empty()
    }

    pub fn push(&mut self, pos: FxVec2, radius: Fx, strength: u8, seed: u16) -> Result<usize, SimError> {
        if self.len() >= MAX_STAINS {
            return Err(SimError::TableFull(Table::Stains));
        }
        self.pos.push(pos);
        self.radius.push(radius);
        self.strength.push(strength);
        self.seed.push(seed);
        Ok(self.len() - 1)
    }

    pub fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.len() as u64);
        for i in 0..self.len() {
            h.write_i64(self.pos[i].x.0);
            h.write_i64(self.pos[i].y.0);
            h.write_i64(self.radius[i].0);
            h.write_u64(self.strength[i] as u64 | (self.seed[i] as u64) << 8);
        }
    }
}
