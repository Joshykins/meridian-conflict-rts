//! The state tables. Together with the tick counter and the RNG these are the
//! whole game state: struct-of-arrays columns, one row per entity.

pub use crate::slots::Handle;
use crate::slots::{put, Slots};
use crate::{SimError, Table};
use mc_core::{Angle, Fx, FxVec2, FxVec3, StateHasher};
use mc_data::{BlueprintId, MAX_WEAPONS};

/// Pitch slots per unit: the gun arm, the build arm, then one for each weapon that is a
/// mount of its own (`Weapon::mount`), at `2 + w`.
pub const ARM_SLOTS: usize = 2 + MAX_WEAPONS;
use serde::{Deserialize, Serialize};

pub type UnitId = Handle;
pub type WreckId = Handle;

pub const MAX_UNITS: usize = 8_192;
pub const MAX_PROJECTILES: usize = 16_384;
pub const MAX_WRECKS: usize = 16_384;
pub const MAX_STAINS: usize = 32_768;
/// Incendiary patches. A Hellkite salvo is 24 bombs, and several bombers can be alight at once.
pub const MAX_FIRES: usize = 2_048;
/// Poured structure lots. They stay after the building dies.
pub const MAX_PADS: usize = 32_768;
pub const MAX_ORDERS: usize = 131_072;
pub const MAX_FLATTENS: usize = 32_768;

pub const NO_ORDER: u32 = u32::MAX;
pub const NO_FIELD: u32 = u32::MAX;

/// A unit's stance: whether its weapons pick targets of their own, and whether
/// it leaves its spot to go after them. An explicit `Attack`, `AttackGround` or
/// `Bombard` order fires (and moves) whatever the stance.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum FireState {
    /// Engages anything it can reach. An idle land or naval unit goes after an
    /// enemy it sees just out of range, on a leash, then walks back; idle
    /// aircraft launch at anything they see.
    #[default]
    FireAtWill,
    /// Never picks a target by itself.
    HoldFire,
    /// Shoots anything in range but never leaves its spot, or its route, to chase.
    HoldPosition,
}

impl FireState {
    /// The inverse of `state as u8`; anything unknown reads as `FireAtWill`.
    pub fn from_bits(bits: u32) -> FireState {
        match bits & 3 {
            1 => FireState::HoldFire,
            2 => FireState::HoldPosition,
            _ => FireState::FireAtWill,
        }
    }
}

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
    /// Hidden successor being assembled: most structures replace the parent; a
    /// commander, factory, extractor, intel tower or shield generator keeps the
    /// same unit and takes this blueprint.
    pub const UPGRADE: u16 = 1 << 5;
    /// Factory repeats its queue.
    pub const REPEAT: u16 = 1 << 6;
    /// The unit's order wants it to stand still this tick (in range, engaging, working).
    pub const HOLD: u16 = 1 << 7;
    /// Pulling mass out of a wreck this tick.
    pub const RECLAIMING: u16 = 1 << 8;
    /// Test range: never picks a target or fires.
    pub const PASSIVE: u16 = 1 << 9;
    /// Test range: weapons do it no harm.
    pub const INVULNERABLE: u16 = 1 << 10;
    /// The turret is turned to the unit's work this tick (building, reclaiming,
    /// upgrading itself), so its weapons hold fire: a build arm and a gun arm
    /// share one torso, and it does one job at a time.
    pub const WORKING: u16 = 1 << 11;
    /// A reclaim beam took the last of it: it is gone without a blast, a wreck or a scorch mark.
    pub const RECLAIMED: u16 = 1 << 12;
    /// Lost health this tick (weapons or a reclaim beam). Regen waits.
    pub const HURT: u16 = 1 << 13;
    /// Mending a finished unit's hull this tick (drives the repair beam).
    pub const REPAIRING: u16 = 1 << 14;
    /// Air is committed to a fly-through (bombing run or similar) toward `move_goal`.
    pub const AIR_RUN: u16 = 1 << 15;

    /// Flags a debug command may set or clear.
    pub const DEBUG: u16 = PASSIVE | INVULNERABLE;

    /// Flags that describe one tick only; cleared when the next tick starts.
    pub const TRANSIENT: u16 = BUILDING | MOVING | HOLD | RECLAIMING | WORKING | HURT | REPAIRING;
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
    /// Signed visual roll, binary angle steps; eased toward the actual turn.
    pub bank: Vec<i16>,
    /// Last detected aircraft target position; used to finish a return through fog.
    pub air_aim: Vec<FxVec2>,
    /// Persistent displacement per tick for damped VTOL and vertical motion.
    pub air_velocity: Vec<FxVec3>,
    /// Time spent unable to bring a fighter's nose onto its opponent.
    pub air_turn_ticks: Vec<u16>,
    /// Brief extension to regain separation after a stalled pursuit turn.
    pub air_break_ticks: Vec<u16>,
    pub drone_parent: Vec<UnitId>,
    pub drone_progress: Vec<Fx>,
    pub intercept_cooldown: Vec<u16>,
    pub burn_ticks: Vec<u16>,
    pub burn_owner: Vec<u8>,
    pub burn_source: Vec<UnitId>,
    pub prev_bank: Vec<i16>,
    /// Signed speed along the heading, metres per second.
    pub speed: Vec<Fx>,
    /// Where the unit was at the end of the previous tick, for render interpolation.
    pub prev_pos: Vec<FxVec2>,
    pub prev_z: Vec<Fx>,
    pub prev_heading: Vec<Angle>,
    pub health: Vec<Fx>,
    /// Combat kills this unit has taken (last hit on an enemy).
    pub kills: Vec<u32>,
    /// Combat rank, 0..=5.
    pub veterancy: Vec<u8>,
    /// Kill-equivalents held toward the next rank.
    pub veterancy_progress: Vec<Fx>,
    /// Damage this unit has taken, by who dealt it, until it dies.
    pub damage: Vec<Vec<(UnitId, Fx)>>,
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
    /// A factory's standing orders, as the commands they came in as: each unit it
    /// rolls out is given them (`standing.rs`). Empty for everything else.
    pub standing: Vec<Vec<crate::command::Command>>,
    pub weapon_cooldown: Vec<[u16; MAX_WEAPONS]>,
    pub weapon_salvo_left: Vec<[u8; MAX_WEAPONS]>,
    /// Turret yaw relative to the hull.
    pub weapon_yaw: Vec<[Angle; MAX_WEAPONS]>,
    pub weapon_target: Vec<[UnitId; MAX_WEAPONS]>,
    /// Bit `w` set: the ground stands between weapon `w` and its target, so it holds its
    /// fire (`line_of_fire.rs`). Always clear for a gun that lobs, homes or has no target.
    pub shot_blocked: Vec<u8>,
    /// `weapon_yaw` at the end of the previous tick, for render interpolation.
    pub prev_weapon_yaw: Vec<[Angle; MAX_WEAPONS]>,
    /// How far the gun arm (first weapon) and the build arm are pitched up (down: negative)
    /// at what they point at. A build arm's rest pitch folds it away when idle.
    /// It moves the muzzle and the emitter, so it is state. Slot `2 + w` is the pitch of
    /// weapon `w`'s own turret when it is a mount (`Weapon::mount`).
    pub arm_pitch: Vec<[Angle; ARM_SLOTS]>,
    pub prev_arm_pitch: Vec<[Angle; ARM_SLOTS]>,
    /// A rotary gun (`Weapon::spin_ticks`, the first one on the unit): how far spun up,
    /// in ticks, and the barrels' turn (angle steps, wrapping).
    pub spin: Vec<[u16; 2]>,
    /// A sweeping gun (`Weapon::sweep`) is mid-stream: it has fired and its barrels are
    /// still at speed, so it keeps firing down the barrel as it swings to its next mark.
    pub streaming: Vec<bool>,
    /// Ground covered since the unit was made, in 1/256 m, wrapping; turning on
    /// the spot counts too. Only the presentation reads it: it times a walker's stride.
    pub gait: Vec<u32>,
    /// What the last tick added to `gait`, and the tick before.
    pub gait_step: Vec<[u16; 2]>,
    /// Ticks a reclaimer turret has been locked on, toward `Reclaimer::charge_ticks`.
    pub reclaim_charge: Vec<u16>,
    /// Current hit points of a projected shield bubble. Zero when the unit has none.
    pub shield_hp: Vec<Fx>,
    /// How far the dome is open, 0..=255. Blocking starts near full.
    pub shield_open: Vec<u8>,
    /// `shield_open` last tick, for render interpolation.
    pub prev_shield_open: Vec<u8>,
    /// After a break: `1` while the bubble fills and stays down. Zero once it
    /// may rise. Engineers cannot hurry that fill.
    pub shield_recharge: Vec<u16>,
    /// Ticks planted toward `Motion::deploy_ticks`. Zero is packed.
    pub deploy: Vec<u16>,
    pub prev_deploy: Vec<u16>,
    pub fire_state: Vec<FireState>,
    /// How far a submarine has dived: 0 surfaced, 255 fully down (`naval.rs`).
    pub dive: Vec<u8>,
    /// The submarine is ordered down. New submarines are.
    pub dive_goal: Vec<bool>,
    /// Ticks a dived hull stays on enemy radar and vision after a missile launch
    /// gave it away (`naval_arms.rs`). Zero: only sonar finds it under water.
    pub revealed: Vec<u16>,
    /// The player paused this unit's work (`Command::SetPaused`): it keeps its queue
    /// but spends nothing on building, assisting, producing, upgrading or repairing.
    pub paused: Vec<bool>,
    /// An aircraft stored below an airbase: that base (`airbase.rs`). A land unit in a
    /// lift ship's hold: that ship (`transport.rs`). `NONE` otherwise. A stored unit is
    /// also `IN_FACTORY`, so nothing can see, hit or order it.
    pub hangar: Vec<UnitId>,
    /// Airbase: ticks until its tunnels can fire again. Stored aircraft: nonzero when
    /// called out (`SORTIE_*`). Aircraft just out of a tunnel: ticks left before it may
    /// go home by itself. Aircraft running down a launch tunnel: `airbase::SORTIE_RUN`
    /// with the ticks left before the mouth.
    pub sortie: Vec<u16>,
    /// An airbase's guard area: its middle and radius, zero radius for none.
    pub guard: Vec<(FxVec2, Fx)>,
    /// An airbase: aircraft with nothing to do may come home to it by themselves.
    pub auto_land: Vec<bool>,
    /// `Bombard`: the point each weapon is laying on, chosen after its last salvo.
    pub ground_aim: Vec<[FxVec2; MAX_WEAPONS]>,
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
            bank: Vec::new(),
            air_aim: Vec::new(),
            air_velocity: Vec::new(),
            air_turn_ticks: Vec::new(),
            air_break_ticks: Vec::new(),
            drone_parent: Vec::new(),
            drone_progress: Vec::new(),
            intercept_cooldown: Vec::new(),
            burn_ticks: Vec::new(),
            burn_owner: Vec::new(),
            burn_source: Vec::new(),
            prev_bank: Vec::new(),
            speed: Vec::new(),
            prev_pos: Vec::new(),
            prev_z: Vec::new(),
            prev_heading: Vec::new(),
            health: Vec::new(),
            kills: Vec::new(),
            veterancy: Vec::new(),
            veterancy_progress: Vec::new(),
            damage: Vec::new(),
            build_progress: Vec::new(),
            order_head: Vec::new(),
            order_tail: Vec::new(),
            field: Vec::new(),
            field_goal: Vec::new(),
            move_goal: Vec::new(),
            stuck_ticks: Vec::new(),
            build_target: Vec::new(),
            rally: Vec::new(),
            standing: Vec::new(),
            weapon_cooldown: Vec::new(),
            weapon_salvo_left: Vec::new(),
            weapon_yaw: Vec::new(),
            weapon_target: Vec::new(),
            shot_blocked: Vec::new(),
            prev_weapon_yaw: Vec::new(),
            arm_pitch: Vec::new(),
            prev_arm_pitch: Vec::new(),
            spin: Vec::new(),
            streaming: Vec::new(),
            gait: Vec::new(),
            gait_step: Vec::new(),
            reclaim_charge: Vec::new(),
            shield_hp: Vec::new(),
            shield_open: Vec::new(),
            prev_shield_open: Vec::new(),
            shield_recharge: Vec::new(),
            deploy: Vec::new(),
            prev_deploy: Vec::new(),
            fire_state: Vec::new(),
            dive: Vec::new(),
            dive_goal: Vec::new(),
            revealed: Vec::new(),
            paused: Vec::new(),
            hangar: Vec::new(),
            sortie: Vec::new(),
            guard: Vec::new(),
            auto_land: Vec::new(),
            ground_aim: Vec::new(),
        }
    }

    pub fn spawn(&mut self, s: UnitSpawn) -> Result<usize, SimError> {
        let row = self
            .slots
            .alloc()
            .ok_or(SimError::TableFull(Table::Units))?;
        put(&mut self.blueprint, row, s.blueprint);
        put(&mut self.owner, row, s.owner);
        put(&mut self.flags, row, s.flags);
        put(&mut self.pos, row, s.pos);
        put(&mut self.z, row, s.z);
        put(&mut self.heading, row, s.heading);
        put(&mut self.bank, row, 0);
        put(&mut self.air_aim, row, s.pos);
        put(&mut self.air_velocity, row, FxVec3::ZERO);
        put(&mut self.air_turn_ticks, row, 0);
        put(&mut self.air_break_ticks, row, 0);
        put(&mut self.drone_parent, row, Handle::NONE);
        put(&mut self.drone_progress, row, Fx::ZERO);
        put(&mut self.intercept_cooldown, row, 0);
        put(&mut self.burn_ticks, row, 0);
        put(&mut self.burn_owner, row, 0);
        put(&mut self.burn_source, row, Handle::NONE);
        put(&mut self.prev_bank, row, 0);
        put(&mut self.speed, row, Fx::ZERO);
        put(&mut self.prev_pos, row, s.pos);
        put(&mut self.prev_z, row, s.z);
        put(&mut self.prev_heading, row, s.heading);
        put(&mut self.health, row, s.health);
        put(&mut self.kills, row, 0);
        put(&mut self.veterancy, row, 0);
        put(&mut self.veterancy_progress, row, Fx::ZERO);
        put(&mut self.damage, row, Vec::new());
        put(&mut self.build_progress, row, s.build_progress);
        put(&mut self.order_head, row, NO_ORDER);
        put(&mut self.order_tail, row, NO_ORDER);
        put(&mut self.field, row, NO_FIELD);
        put(&mut self.field_goal, row, s.pos);
        put(&mut self.move_goal, row, s.pos);
        put(&mut self.stuck_ticks, row, 0);
        put(&mut self.build_target, row, Handle::NONE);
        put(&mut self.rally, row, s.pos);
        put(&mut self.standing, row, Vec::new());
        put(&mut self.weapon_cooldown, row, [0; MAX_WEAPONS]);
        put(&mut self.weapon_salvo_left, row, [0; MAX_WEAPONS]);
        put(&mut self.weapon_yaw, row, [Angle::ZERO; MAX_WEAPONS]);
        put(&mut self.weapon_target, row, [Handle::NONE; MAX_WEAPONS]);
        put(&mut self.shot_blocked, row, 0);
        put(&mut self.prev_weapon_yaw, row, [Angle::ZERO; MAX_WEAPONS]);
        put(&mut self.arm_pitch, row, [Angle::ZERO; ARM_SLOTS]);
        put(&mut self.prev_arm_pitch, row, [Angle::ZERO; ARM_SLOTS]);
        put(&mut self.spin, row, [0; 2]);
        put(&mut self.streaming, row, false);
        put(&mut self.gait, row, 0);
        put(&mut self.gait_step, row, [0; 2]);
        put(&mut self.reclaim_charge, row, 0);
        put(&mut self.shield_hp, row, Fx::ZERO);
        put(&mut self.shield_open, row, 0);
        put(&mut self.prev_shield_open, row, 0);
        put(&mut self.shield_recharge, row, 0);
        put(&mut self.deploy, row, 0);
        put(&mut self.prev_deploy, row, 0);
        put(&mut self.fire_state, row, FireState::FireAtWill);
        put(&mut self.dive, row, 0);
        put(&mut self.dive_goal, row, false);
        put(&mut self.revealed, row, 0);
        put(&mut self.paused, row, false);
        put(&mut self.hangar, row, Handle::NONE);
        put(&mut self.sortie, row, 0);
        put(&mut self.guard, row, (s.pos, Fx::ZERO));
        put(&mut self.auto_land, row, true);
        // Far from any map: the first bombardment picks a point.
        put(
            &mut self.ground_aim,
            row,
            [FxVec2::from_ints(-1_000_000, -1_000_000); MAX_WEAPONS],
        );
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
            h.write_u64(
                self.blueprint[row].0 as u64
                    | (self.owner[row] as u64) << 16
                    | (self.flags[row] as u64) << 24
                    | (self.heading[row].0 as u64) << 40,
            );
            h.write_i64(self.pos[row].x.0);
            h.write_i64(self.pos[row].y.0);
            h.write_i64(self.z[row].0);
            h.write_i64(self.speed[row].0);
            h.write_i64(self.bank[row] as i64);
            h.write_i64(self.air_velocity[row].x.0);
            h.write_i64(self.air_velocity[row].y.0);
            h.write_i64(self.air_velocity[row].z.0);
            h.write_i64(self.air_aim[row].x.0);
            h.write_i64(self.air_aim[row].y.0);
            h.write_u32(self.air_turn_ticks[row] as u32);
            h.write_u32(self.air_break_ticks[row] as u32);
            h.write_u32(self.drone_parent[row].0);
            h.write_i64(self.drone_progress[row].0);
            h.write_u32(self.intercept_cooldown[row] as u32);
            h.write_u32(self.burn_ticks[row] as u32);
            h.write_u32(self.burn_owner[row] as u32);
            h.write_u32(self.burn_source[row].0);
            h.write_i64(self.health[row].0);
            h.write_u64(self.kills[row] as u64 | (self.veterancy[row] as u64) << 32);
            h.write_i64(self.veterancy_progress[row].0);
            h.write_u64(self.damage[row].len() as u64);
            for (id, dmg) in &self.damage[row] {
                h.write_u64(id.0 as u64);
                h.write_i64(dmg.0);
            }
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
            h.write_u64(self.standing[row].len() as u64);
            for c in &self.standing[row] {
                h.write_u8s(&c.encode());
            }
            h.write_u64(
                self.arm_pitch[row][0].0 as u64
                    | (self.arm_pitch[row][1].0 as u64) << 16
                    | (self.reclaim_charge[row] as u64) << 32
                    | (self.arm_pitch[row][2].0 as u64) << 48,
            );
            h.write_u64(
                (3..ARM_SLOTS).fold(0u64, |acc, s| acc | (self.arm_pitch[row][s].0 as u64) << ((s - 3) * 16)),
            );
            h.write_u64(
                self.spin[row][0] as u64
                    | (self.spin[row][1] as u64) << 16
                    | (self.streaming[row] as u64) << 32
                    | (self.shot_blocked[row] as u64) << 40,
            );
            h.write_i64(self.shield_hp[row].0);
            h.write_u64(self.shield_open[row] as u64 | (self.shield_recharge[row] as u64) << 8);
            h.write_u64(self.deploy[row] as u64 | (self.revealed[row] as u64) << 16);
            h.write_u64(
                self.fire_state[row] as u64
                    | (self.dive[row] as u64) << 8
                    | (self.dive_goal[row] as u64) << 16
                    | (self.paused[row] as u64) << 24,
            );
            h.write_u64(
                self.hangar[row].0 as u64
                    | (self.sortie[row] as u64) << 32
                    | (self.auto_land[row] as u64) << 48,
            );
            h.write_i64(self.guard[row].0.x.0);
            h.write_i64(self.guard[row].0.y.0);
            h.write_i64(self.guard[row].1 .0);
            for p in self.ground_aim[row] {
                h.write_i64(p.x.0);
                h.write_i64(p.y.0);
            }
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
    /// Help a builder, finish/repair a unit, or feed a shield. Stays until
    /// another order is given or the target is gone.
    Assist,
    Reclaim,
    /// Factory queue entry: produce one `blueprint`.
    Produce,
    /// Build `blueprint` in place of this unit. A mobile unit, factory,
    /// extractor, intel tower or shield generator is refitted and stays the
    /// same unit; other structures are replaced by their successor.
    Upgrade,
    /// Take a live unit apart: the builder's own side's, or an enemy's.
    ReclaimUnit,
    /// Circle a point or a friendly unit until cancelled.
    Orbit,
    /// Move into range of `pos` and shell the ground there until given another order.
    AttackGround,
    /// `AttackGround`, each shot at a random point within `radius` of `pos`.
    Bombard,
    /// Attack-move to `pos`; on arrival the order goes to the back of the queue,
    /// so a unit with several loops through them for good.
    Patrol,
    /// Hold a spot (`pos` plus `offset`) and go after enemies that come within
    /// `radius` of `pos`, then come back. An airbase with one launches its
    /// aircraft at them; aircraft it sent out carry the base in `target` and go
    /// home when the area is clear.
    Guard,
    /// Fly to the airbase in `target` and go down its hatch.
    Dock,
    /// A land unit walks up the ramp of the lift ship in `target` into its hold.
    Board,
    /// A lift ship sets down at `pos` and lowers its ramp.
    Land,
    /// `Land`, then everything in the hold walks out; done once it is empty. With a
    /// `target`, only that unit walks out (`Command::Unload`).
    Unload,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Order {
    /// Stable command-group identity; zero means independent movement.
    pub formation: u64,
    pub kind: OrderKind,
    pub pos: FxVec2,
    /// Unit or wreck, depending on `kind`.
    pub target: Handle,
    pub blueprint: BlueprintId,
    pub heading: Angle,
    /// Offset from `pos` this unit keeps, so a group arrives in formation.
    pub offset: FxVec2,
    /// `Bombard`: how far from `pos` shots may fall. `Orbit`: the circle asked for,
    /// zero for the unit's own. `Board`: one after reaching the stern approach lane.
    /// Zero otherwise.
    pub radius: Fx,
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
        Orders {
            order: Vec::new(),
            next: Vec::new(),
            free: Vec::new(),
        }
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

    pub fn push_back(
        &mut self,
        units: &mut Units,
        row: usize,
        order: Order,
    ) -> Result<(), SimError> {
        let node = self.alloc(order)?;
        match units.order_tail[row] {
            NO_ORDER => units.order_head[row] = node,
            tail => self.next[tail as usize] = node,
        }
        units.order_tail[row] = node;
        Ok(())
    }

    pub fn push_front(
        &mut self,
        units: &mut Units,
        row: usize,
        order: Order,
    ) -> Result<(), SimError> {
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

    /// Pool indices of a unit's queue, front to back: `order[node]` can be edited in place.
    pub fn nodes<'a>(&'a self, units: &Units, row: usize) -> impl Iterator<Item = u32> + 'a {
        let mut node = units.order_head[row];
        std::iter::from_fn(move || {
            (node != NO_ORDER).then(|| {
                let at = node;
                node = self.next[node as usize];
                at
            })
        })
    }

    pub fn hash(&self, h: &mut StateHasher) {
        h.write_u32s(&self.next);
        h.write_u32s(&self.free);
        for o in &self.order {
            h.write_u64(o.formation);
            h.write_u64(o.kind as u64 | (o.blueprint.0 as u64) << 8 | (o.heading.0 as u64) << 24);
            h.write_u64(o.target.0 as u64);
            for v in [o.pos.x, o.pos.y, o.offset.x, o.offset.y, o.radius] {
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
    /// Share of upkeep (paid before any building) that was met last tick. Shields
    /// go down when this falls short, not when only construction is starved.
    #[serde(default = "fx_one")]
    pub upkeep_efficiency: Fx,
    pub reclaimed_mass: Fx,
    /// Materials a second reclaimed over the last tick, for the UI and the AI; not in
    /// `mass_income`, which is what the mines and generators make.
    #[serde(default)]
    pub reclaim_income: Fx,
    /// `reclaimed_mass` when `reclaim_income` was last worked out.
    #[serde(default)]
    pub reclaimed_counted: Fx,
    pub units_built: u32,
    pub units_lost: u32,
    pub units_killed: u32,
    /// Test range: the slot whose units this slot's commands are applied to (normally itself).
    pub acts_as: u8,
    /// Test range: building costs this player nothing and never stalls.
    pub free_build: bool,
    /// Test range: what share of their mass and energy income this player gets, thousandths.
    #[serde(default = "full_income")]
    pub income_permille: [u16; 2],
    /// Test range: mass and energy storage on top of what this player's units hold.
    #[serde(default)]
    pub bonus_storage: [Fx; 2],
}

fn full_income() -> [u16; 2] {
    [1000, 1000]
}

fn fx_one() -> Fx {
    Fx::ONE
}

impl Player {
    pub fn hash(&self, h: &mut StateHasher) {
        h.write_u64(
            self.faction as u64
                | (self.team as u64) << 8
                | (self.defeated as u64) << 16
                | (self.controller as u64) << 17,
        );
        h.write_u64(self.commander.0 as u64);
        for v in [
            self.mass,
            self.energy,
            self.mass_capacity,
            self.energy_capacity,
            self.efficiency,
            self.upkeep_efficiency,
            self.reclaimed_mass,
        ] {
            h.write_i64(v.0);
        }
        h.write_u64(self.units_built as u64 | (self.units_lost as u64) << 32);
        h.write_u64(
            self.units_killed as u64 | (self.acts_as as u64) << 32 | (self.free_build as u64) << 40,
        );
        h.write_u64(self.income_permille[0] as u64 | (self.income_permille[1] as u64) << 16);
        h.write_i64(self.bonus_storage[0].0);
        h.write_i64(self.bonus_storage[1].0);
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
    pub target: Vec<UnitId>,
    pub age: Vec<u16>,
    /// Nose direction. Cold missiles pitch this onto the intercept while the
    /// body stays on the vertical lob; ignition commits it into `vel`.
    pub aim: Vec<FxVec3>,
    /// `aim` at the start of the tick, so the body can ease onto the new nose.
    pub prev_aim: Vec<FxVec3>,
    /// Casing left for an intercept laser. Zero until a missile is first burned,
    /// which reads as a full casing.
    pub hp: Vec<Fx>,
    /// This shot's number, from `next_serial`: never zero, never reused in a match.
    pub serial: Vec<u32>,
    /// An interceptor torpedo (`Weapon::intercepts`): the `serial` of the torpedo it
    /// runs at. Zero for every other shot.
    pub quarry: Vec<u32>,
    /// A guided missile's aim point at launch; what a high arc or a sea skimmer fired
    /// at the ground flies to (`naval_arms.rs`).
    pub mark: Vec<FxVec3>,
    /// Where the shot was fired from.
    pub origin: Vec<FxVec2>,
    /// The last `serial` handed out.
    pub next_serial: u32,
}

impl Projectiles {
    pub fn len(&self) -> usize {
        self.pos.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pos.is_empty()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        &mut self,
        pos: FxVec3,
        vel: FxVec3,
        owner: u8,
        source: UnitId,
        blueprint: BlueprintId,
        weapon: u8,
        ticks: u16,
    ) -> Result<(), SimError> {
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
        self.target.push(Handle::NONE);
        self.age.push(0);
        let nose = vel.normalize();
        let nose = if nose.length_sq() > Fx::ZERO {
            nose
        } else {
            FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::ONE)
        };
        self.aim.push(nose);
        self.prev_aim.push(nose);
        self.hp.push(Fx::ZERO);
        self.next_serial = self.next_serial.wrapping_add(1).max(1);
        self.serial.push(self.next_serial);
        self.quarry.push(0);
        self.mark.push(pos);
        self.origin.push(pos.xy());
        Ok(())
    }

    pub fn clear(&mut self) {
        let next_serial = self.next_serial;
        *self = Projectiles::default();
        self.next_serial = next_serial;
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
        self.target.swap_remove(row);
        self.age.swap_remove(row);
        self.aim.swap_remove(row);
        self.prev_aim.swap_remove(row);
        self.hp.swap_remove(row);
        self.serial.swap_remove(row);
        self.quarry.swap_remove(row);
        self.mark.swap_remove(row);
        self.origin.swap_remove(row);
    }

    pub fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.len() as u64 | (self.next_serial as u64) << 32);
        for i in 0..self.len() {
            h.write_u64(self.serial[i] as u64 | (self.quarry[i] as u64) << 32);
            for v in [self.mark[i], self.origin[i].extend(Fx::ZERO)] {
                h.write_i64(v.x.0);
                h.write_i64(v.y.0);
                h.write_i64(v.z.0);
            }
            for v in [self.pos[i], self.vel[i]] {
                h.write_i64(v.x.0);
                h.write_i64(v.y.0);
                h.write_i64(v.z.0);
            }
            h.write_u64(
                self.owner[i] as u64
                    | (self.weapon[i] as u64) << 8
                    | (self.blueprint[i].0 as u64) << 16
                    | (self.ticks_left[i] as u64) << 32,
            );
            h.write_u64(self.source[i].0 as u64);
            h.write_u32(self.target[i].0);
            h.write_u32(self.age[i] as u32);
            h.write_i64(self.aim[i].x.0);
            h.write_i64(self.aim[i].y.0);
            h.write_i64(self.aim[i].z.0);
            h.write_i64(self.hp[i].0);
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
    /// Signed roll the hull lies at, binary angle steps: a sunk ship's list on the seabed.
    pub bank: Vec<i16>,
    pub prev_bank: Vec<i16>,
    pub mass: Vec<Fx>,
    pub mass_max: Vec<Fx>,
}

impl Wrecks {
    pub fn new() -> Wrecks {
        Wrecks {
            slots: Slots::new(MAX_WRECKS),
            blueprint: Vec::new(),
            pos: Vec::new(),
            z: Vec::new(),
            heading: Vec::new(),
            bank: Vec::new(),
            prev_bank: Vec::new(),
            mass: Vec::new(),
            mass_max: Vec::new(),
        }
    }

    pub fn spawn(
        &mut self,
        blueprint: BlueprintId,
        pos: FxVec2,
        z: Fx,
        heading: Angle,
        mass: Fx,
    ) -> Result<usize, SimError> {
        let row = self
            .slots
            .alloc()
            .ok_or(SimError::TableFull(Table::Wrecks))?;
        put(&mut self.blueprint, row, blueprint);
        put(&mut self.pos, row, pos);
        put(&mut self.z, row, z);
        put(&mut self.heading, row, heading);
        put(&mut self.bank, row, 0);
        put(&mut self.prev_bank, row, 0);
        put(&mut self.mass, row, mass);
        put(&mut self.mass_max, row, mass);
        Ok(row)
    }

    pub fn hash(&self, h: &mut StateHasher) {
        self.slots.hash(h);
        for row in self.slots.iter() {
            h.write_u64(
                self.blueprint[row].0 as u64
                    | (self.heading[row].0 as u64) << 16
                    | (self.bank[row] as u16 as u64) << 32,
            );
            h.write_i64(self.pos[row].x.0);
            h.write_i64(self.pos[row].y.0);
            h.write_i64(self.z[row].0);
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

    pub fn clear(&mut self) {
        *self = Stains::default();
    }

    pub fn push(
        &mut self,
        pos: FxVec2,
        radius: Fx,
        strength: u8,
        seed: u16,
    ) -> Result<usize, SimError> {
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

/// One bomb, one patch. Overlapping patches each deal their own damage.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Fires {
    pub pos: Vec<FxVec2>,
    pub z: Vec<Fx>,
    pub radius: Vec<Fx>,
    /// Ticks of damage still to come.
    pub ticks: Vec<u16>,
    /// Ticks the patch was lit for. The renderer fades across this.
    pub span: Vec<u16>,
    pub owner: Vec<u8>,
    pub source: Vec<UnitId>,
    pub target_mask: Vec<u32>,
}

impl Fires {
    pub fn len(&self) -> usize {
        self.pos.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pos.is_empty()
    }

    pub fn clear(&mut self) {
        *self = Fires::default();
    }

    pub fn push(
        &mut self,
        pos: FxVec2,
        z: Fx,
        radius: Fx,
        ticks: u16,
        owner: u8,
        source: UnitId,
        target_mask: u32,
    ) -> Result<usize, SimError> {
        if self.len() >= MAX_FIRES {
            return Err(SimError::TableFull(Table::Fires));
        }
        self.pos.push(pos);
        self.z.push(z);
        self.radius.push(radius);
        self.ticks.push(ticks);
        self.span.push(ticks);
        self.owner.push(owner);
        self.source.push(source);
        self.target_mask.push(target_mask);
        Ok(self.len() - 1)
    }

    pub fn swap_remove(&mut self, i: usize) {
        self.pos.swap_remove(i);
        self.z.swap_remove(i);
        self.radius.swap_remove(i);
        self.ticks.swap_remove(i);
        self.span.swap_remove(i);
        self.owner.swap_remove(i);
        self.source.swap_remove(i);
        self.target_mask.swap_remove(i);
    }

    pub fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.len() as u64);
        for i in 0..self.len() {
            h.write_i64(self.pos[i].x.0);
            h.write_i64(self.pos[i].y.0);
            h.write_i64(self.z[i].0);
            h.write_i64(self.radius[i].0);
            h.write_u64(self.ticks[i] as u64 | (self.span[i] as u64) << 16);
            h.write_u64(self.owner[i] as u64 | (self.target_mask[i] as u64) << 8);
            h.write_u32(self.source[i].0);
        }
    }
}

/// Structure foundations. A poured lot stays after the building is gone;
/// overlapping a lot that is already there is a no-op (rebuilds share it).
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Pads {
    pub pos: Vec<FxVec2>,
    pub radius: Vec<Fx>,
    /// Same packing the renderer already uses for a pad: owner, well flag,
    /// ghost, build, blueprint. See [`crate::world::pack_structure_pad`].
    pub packed: Vec<u32>,
}

impl Pads {
    pub fn len(&self) -> usize {
        self.pos.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pos.is_empty()
    }

    pub fn clear(&mut self) {
        *self = Pads::default();
    }

    pub fn index_at(&self, pos: FxVec2) -> Option<usize> {
        self.pos.iter().position(|&p| p == pos)
    }

    pub fn upsert(&mut self, pos: FxVec2, radius: Fx, packed: u32) -> Result<usize, SimError> {
        if let Some(i) = self.index_at(pos) {
            return Ok(i);
        }
        if self.len() >= MAX_PADS {
            return Err(SimError::TableFull(Table::Pads));
        }
        self.pos.push(pos);
        self.radius.push(radius);
        self.packed.push(packed);
        Ok(self.len() - 1)
    }

    pub fn remove_at(&mut self, pos: FxVec2) {
        if let Some(i) = self.index_at(pos) {
            self.pos.swap_remove(i);
            self.radius.swap_remove(i);
            self.packed.swap_remove(i);
        }
    }

    pub fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.len() as u64);
        for i in 0..self.len() {
            h.write_i64(self.pos[i].x.0);
            h.write_i64(self.pos[i].y.0);
            h.write_i64(self.radius[i].0);
            h.write_u64(self.packed[i] as u64);
        }
    }
}
