//! Commands: the only way anything outside the simulation changes game state.
//!
//! A command carries no player id. The session attaches the issuing slot, and
//! the sim checks every referenced unit against it, so a client cannot order
//! units it does not own.

use crate::tables::{FireState, OrderKind, UnitId};
use mc_core::{Angle, Fx, FxVec2};
use mc_data::BlueprintId;
use serde::{Deserialize, Serialize};

/// Most units one command may address: every unit a side can field, so one
/// order moves a whole selection as one block.
pub const MAX_COMMAND_UNITS: usize = crate::tables::MAX_UNITS;
/// Most units one `DebugSpawn` puts down (a block of them); a bigger count is cut to it.
pub const MAX_DEBUG_SPAWN: u16 = 1024;
/// Largest encoded command [`Command::decode`] takes: a handle per unit, and
/// room for the rest (a patrol's points, a factory's queue).
pub const MAX_COMMAND_BYTES: u64 = 4 * MAX_COMMAND_UNITS as u64 + (4 << 10);
/// Most waypoints one `Patrol` takes; a longer route is refused (`SimEvent::CommandRefused`).
pub const MAX_PATROL_POINTS: usize = 32;
/// Narrowest and widest area a `Guard` may cover, metres.
pub const MIN_GUARD_RADIUS: Fx = Fx::from_int(40);
pub const MAX_GUARD_RADIUS: Fx = Fx::from_int(2400);
/// Widest circle a `ReclaimArea` may clear, metres.
pub const MAX_RECLAIM_RADIUS: Fx = Fx::from_int(2400);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    /// Explicit player formation settings; ordinary Move uses together/standard.
    FormationMove {
        units: Vec<UnitId>,
        target: FxVec2,
        queue: bool,
        attack_move: bool,
        /// Which way the block faces, as the player turned it with a right-drag;
        /// `None` faces the way it goes.
        facing: Option<Angle>,
        together: bool,
        spacing: u8,
        /// How wide the block stands for its depth: 0 square, up to
        /// `formations::SHAPES` wider, down to `-SHAPES` deeper.
        shape: i8,
    },
    Move {
        units: Vec<UnitId>,
        target: FxVec2,
        queue: bool,
    },
    AttackMove {
        units: Vec<UnitId>,
        target: FxVec2,
        queue: bool,
    },
    Attack {
        units: Vec<UnitId>,
        target: UnitId,
        queue: bool,
    },
    Stop {
        units: Vec<UnitId>,
    },
    /// `pos` is snapped to the build grid by the sim.
    Build {
        units: Vec<UnitId>,
        blueprint: BlueprintId,
        pos: FxVec2,
        heading: Angle,
        queue: bool,
    },
    /// Help build, repair, or feed a live shield on `target`. On a builder
    /// target: help whatever it builds. Stays until another order is given
    /// or the target is gone; with orders queued behind it, it gives way to
    /// them as soon as the target has no work to help with.
    Assist {
        units: Vec<UnitId>,
        target: UnitId,
        queue: bool,
    },
    ReclaimWreck {
        units: Vec<UnitId>,
        wreck: crate::tables::WreckId,
        queue: bool,
    },
    /// Take a live unit apart for mass: one of the player's own, or an enemy's. It loses
    /// health as it goes, and what is reclaimed to nothing leaves no blast and no wreck.
    ReclaimUnit {
        units: Vec<UnitId>,
        target: UnitId,
        queue: bool,
    },
    /// Mobile reclaimers head for `pos`, reclaiming the wrecks they pass; with a `radius`
    /// (up to `MAX_RECLAIM_RADIUS`), they clear every wreck within it of `pos` too.
    ReclaimArea {
        units: Vec<UnitId>,
        pos: FxVec2,
        radius: Fx,
        queue: bool,
    },
    /// Append `count` of `blueprint` to each factory's queue.
    Produce {
        factories: Vec<UnitId>,
        blueprint: BlueprintId,
        count: u8,
    },
    /// Drop the last queued `blueprint` from each factory.
    CancelProduce {
        factories: Vec<UnitId>,
        blueprint: BlueprintId,
    },
    SetRepeat {
        factories: Vec<UnitId>,
        repeat: bool,
    },
    SetRally {
        factories: Vec<UnitId>,
        pos: FxVec2,
    },
    /// Queues each unit's upgrade (`upgrades_to`) behind the orders it already has.
    Upgrade {
        units: Vec<UnitId>,
    },
    /// Takes the upgrade back out of each unit's queue; one already under way is scrapped.
    CancelUpgrade {
        units: Vec<UnitId>,
    },
    /// Queues fitting the module `kit` assembles (`Blueprints::kit`) behind each unit's
    /// orders. It has to fit what the unit will have by the time it comes up, after the
    /// refits already queued; a unit it does not fit is left alone.
    Refit {
        units: Vec<UnitId>,
        kit: BlueprintId,
    },
    /// Takes the refit to `kit` out of each unit's queue (scrapping it if it is under way),
    /// and any queued refit that no longer fits without it.
    CancelRefit {
        units: Vec<UnitId>,
        kit: BlueprintId,
    },
    /// Blows these units up. `timed`: after a countdown (`destruct::COUNTDOWN`) instead
    /// of at once, and if any of them is already counting down, every countdown among
    /// them is called off instead.
    SelfDestruct {
        units: Vec<UnitId>,
        timed: bool,
    },
    Resign,
    /// Moves the `kind` orders (`Move`, `AttackMove`, `Build`, `Patrol`, `AttackGround`,
    /// `Bombard`, `Guard` or `ReclaimArea`) these units hold at exactly `from` to `to`. A `Build` is snapped
    /// to the build grid and has to fit where it lands. A `Guard` round a unit is found near
    /// `from` and stops following it.
    RelocateOrder {
        units: Vec<UnitId>,
        kind: OrderKind,
        from: FxVec2,
        to: FxVec2,
    },
    /// Only honoured when the match was created with `cheats` on (test scenes, tools),
    /// like every `Debug*` command below. `flags` takes `flag::DEBUG` bits; `build` is
    /// how complete the unit is in thousandths, a construction site below 1000.
    DebugSpawn {
        owner: u8,
        blueprint: BlueprintId,
        pos: FxVec2,
        heading: Angle,
        count: u16,
        flags: u16,
        build: u16,
    },
    /// Takes `permille` thousandths of each unit's full health away (negative: gives
    /// it back), whoever owns it and whether or not it is invulnerable.
    DebugDamage {
        units: Vec<UnitId>,
        permille: i16,
    },
    /// Takes units off the map with no death, wreck or scorch mark.
    DebugRemove {
        units: Vec<UnitId>,
    },
    /// Sets and clears `flag::DEBUG` bits: hold fire, invulnerable.
    DebugSetFlags {
        units: Vec<UnitId>,
        set: u16,
        clear: u16,
    },
    /// Makes each unit a construction site that is `permille` thousandths built; 1000 completes it.
    DebugSetBuild {
        units: Vec<UnitId>,
        permille: u16,
    },
    /// Empties the map: units, wrecks, shots in the air and scorch marks.
    DebugClear,
    /// From now on the issuing slot's commands are applied as `player`'s.
    DebugControl {
        player: u8,
    },
    /// Building costs `player` nothing and never stalls.
    DebugFreeBuild {
        player: u8,
        on: bool,
    },
    // Later additions stay at the end, so older replays decode the same.
    /// Whether these units' weapons pick targets of their own.
    SetFireState {
        units: Vec<UnitId>,
        state: FireState,
    },
    /// Move into range of `pos` and fire every weapon that can hit the ground at it
    /// until given another order.
    AttackGround {
        units: Vec<UnitId>,
        pos: FxVec2,
        queue: bool,
    },
    /// `AttackGround`, each shot at a random point within `radius` of `pos`. Each unit
    /// takes at most the widest circle its guns allow (`Weapon::bombard_radius`).
    Bombard {
        units: Vec<UnitId>,
        pos: FxVec2,
        radius: Fx,
        queue: bool,
    },
    /// Loop through `points` (up to `MAX_PATROL_POINTS`), engaging on the way. With one
    /// point the loop runs between where the group stands now and that point.
    Patrol {
        units: Vec<UnitId>,
        points: Vec<FxVec2>,
        queue: bool,
    },
    /// Adds `point` to these units' patrol loops, right after their post at exactly `after`.
    PatrolInsert {
        units: Vec<UnitId>,
        after: FxVec2,
        point: FxVec2,
    },
    /// Takes the `kind` orders these units hold at exactly `pos` out of their queues
    /// (the same match as `RelocateOrder`). A structure already begun is left standing.
    CancelOrder {
        units: Vec<UnitId>,
        kind: OrderKind,
        pos: FxVec2,
    },
    /// Lays these units' moves, patrols and orbits out again, together or free and
    /// at `spacing` (0 compact, 1 standard, 2 wide) and `shape` (as `FormationMove`'s),
    /// keeping where they go.
    Reform {
        units: Vec<UnitId>,
        together: bool,
        spacing: u8,
        shape: i8,
    },
    /// Submarines among these units dive (`dive`) or surface. Others ignore it.
    SetDive {
        units: Vec<UnitId>,
        dive: bool,
    },
    /// Test range: sets `player`'s stores to these thousandths of what they can hold.
    DebugStock {
        player: u8,
        mass: Option<u16>,
        energy: Option<u16>,
    },
    /// Test range: `player`'s mines and generators make these thousandths of what they
    /// should (1000 is normal, 0 nothing), to stage a shortage or a glut.
    DebugIncome {
        player: u8,
        mass: u16,
        energy: u16,
    },
    /// Test range: stores `player` has on top of what its units hold, whole units.
    DebugStorage {
        player: u8,
        mass: u32,
        energy: u32,
    },
    /// Test range: `count` wrecks of `blueprint` in a block centred on `pos`, for reclaim.
    DebugWrecks {
        blueprint: BlueprintId,
        pos: FxVec2,
        count: u16,
    },
    /// Each factory takes `from`'s standing orders and rally point in place of its own,
    /// so what it makes leaves the way `from`'s products do.
    CopyFactoryOrders {
        factories: Vec<UnitId>,
        from: UnitId,
    },
    /// Pauses (`paused`) or resumes these units' work. A paused builder, factory or
    /// upgrading structure keeps its orders in order but spends nothing: a site it is on
    /// stays half built, and helpers of whatever it builds stop with it.
    SetPaused {
        units: Vec<UnitId>,
        paused: bool,
    },
    /// Hold `pos` (a group keeps its spread) and go after enemies that come within
    /// `radius` of it, coming back once they are gone. Aircraft circle it halfway out,
    /// a group in formation. With `target` a friendly unit, the area goes with it.
    Guard {
        units: Vec<UnitId>,
        pos: FxVec2,
        target: UnitId,
        radius: Fx,
        queue: bool,
    },
    /// Land units among these walk up the lift ship `carrier`'s ramp into its hold
    /// (`transport.rs`). One idle up in the sky comes down for them.
    Board {
        units: Vec<UnitId>,
        carrier: UnitId,
        queue: bool,
    },
    /// Capital ships among these fly to `pos` and set down on the nearest ground that
    /// takes them, the only way they ever land; lift ships lower the ramp, and with
    /// `unload` everything in the hold walks out.
    Land {
        units: Vec<UnitId>,
        pos: FxVec2,
        unload: bool,
        queue: bool,
    },
    /// Units among these riding in a lift ship's hold walk out of it, and nothing else
    /// does: the ship sets down where it is (or where it is already setting down) and
    /// lets them out in turn (`transport.rs`).
    Unload {
        units: Vec<UnitId>,
    },
    /// Capital ships among these that are down lift off (lift ships raising the ramp
    /// first) and climb back to cruise height where they are.
    TakeOff {
        units: Vec<UnitId>,
    },
    /// One warhead among these nuclear silos is given `pos`, anywhere on the map: from the
    /// silo with the most warheads not yet spoken for (ties: the nearest, then the lowest
    /// id), which fires its marks in turn. Nothing when none is free (`nukes.rs`).
    LaunchNuke {
        units: Vec<UnitId>,
        pos: FxVec2,
    },
    /// Strategic launchers among these assemble rounds by themselves (`on`), or only the
    /// ones queued by hand (`nukes.rs`).
    SetAutoBuild {
        units: Vec<UnitId>,
        on: bool,
    },
    /// `count` more rounds queued by hand on these launchers (fewer when negative).
    QueueRounds {
        units: Vec<UnitId>,
        count: i16,
    },
    /// A giant's storm called down on `pos` (the Behemoth's Strike button): `AttackGround`
    /// with its storm bore alone; its other guns keep to their own targets.
    Strike {
        units: Vec<UnitId>,
        pos: FxVec2,
        queue: bool,
    },
    /// How the side's economy pays for new mines and new power when it cannot pay
    /// for everything: first, with the rest, or last (`focus.rs`).
    SetFocus {
        focus: crate::focus::Focus,
    },
    /// Capital ships among these with a warp drive spool up, turn onto `pos` and jump
    /// there, or as far toward it as the drive reaches (`warp.rs`).
    Warp {
        units: Vec<UnitId>,
        pos: FxVec2,
        queue: bool,
    },
    /// Batch on: the factories become one batch with one set of standing orders. Each
    /// one's products form up outside it and wait until the batch is full, then all leave
    /// together on those orders (`batch.rs`). Off: each leaves its batch, and whoever it
    /// has waiting leaves at once.
    SetBatch {
        factories: Vec<UnitId>,
        batch: bool,
    },
    /// Each factory's batch leaves now, as many as have formed up.
    ReleaseBatch {
        factories: Vec<UnitId>,
    },
    /// The batches these factories are in leave once `size` units are formed up (1 to
    /// `batch::MAX_BATCH`).
    SetBatchSize {
        factories: Vec<UnitId>,
        size: u16,
    },
}

impl Command {
    pub fn encode(&self) -> Vec<u8> {
        bincode::serialize(self).expect("commands always serialize")
    }

    /// `None` for malformed input. A peer can send anything; never panic on it.
    pub fn decode(bytes: &[u8]) -> Option<Command> {
        use bincode::Options;
        // Bounded so a hostile length prefix cannot force a huge allocation.
        let cmd: Command = bincode::options()
            .with_fixint_encoding()
            .allow_trailing_bytes()
            .with_limit(MAX_COMMAND_BYTES)
            .deserialize(bytes)
            .ok()?;
        let units = match &cmd {
            Command::FormationMove { units, .. }
            | Command::Move { units, .. }
            | Command::AttackMove { units, .. }
            | Command::Attack { units, .. }
            | Command::Stop { units }
            | Command::Build { units, .. }
            | Command::Assist { units, .. }
            | Command::ReclaimWreck { units, .. }
            | Command::ReclaimUnit { units, .. }
            | Command::ReclaimArea { units, .. }
            | Command::Upgrade { units }
            | Command::CancelUpgrade { units }
            | Command::Refit { units, .. }
            | Command::CancelRefit { units, .. }
            | Command::SelfDestruct { units, .. }
            | Command::RelocateOrder { units, .. }
            | Command::DebugDamage { units, .. }
            | Command::DebugRemove { units }
            | Command::DebugSetFlags { units, .. }
            | Command::DebugSetBuild { units, .. }
            | Command::SetFireState { units, .. }
            | Command::AttackGround { units, .. }
            | Command::Strike { units, .. }
            | Command::Bombard { units, .. }
            | Command::Patrol { units, .. }
            | Command::PatrolInsert { units, .. }
            | Command::CancelOrder { units, .. }
            | Command::Reform { units, .. }
            | Command::SetDive { units, .. }
            | Command::SetPaused { units, .. }
            | Command::Guard { units, .. }
            | Command::Board { units, .. }
            | Command::Land { units, .. }
            | Command::Unload { units }
            | Command::TakeOff { units }
            | Command::LaunchNuke { units, .. }
            | Command::SetAutoBuild { units, .. }
            | Command::QueueRounds { units, .. }
            | Command::Warp { units, .. } => units.len(),

            Command::Produce { factories, .. }
            | Command::CancelProduce { factories, .. }
            | Command::SetRepeat { factories, .. }
            | Command::SetRally { factories, .. }
            | Command::CopyFactoryOrders { factories, .. }
            | Command::SetBatch { factories, .. }
            | Command::ReleaseBatch { factories }
            | Command::SetBatchSize { factories, .. } => factories.len(),
            Command::Resign
            | Command::DebugSpawn { .. }
            | Command::DebugClear
            | Command::DebugControl { .. }
            | Command::DebugFreeBuild { .. }
            | Command::DebugStock { .. }
            | Command::DebugIncome { .. }
            | Command::DebugStorage { .. }
            | Command::DebugWrecks { .. }
            | Command::SetFocus { .. } => 0,
        };
        (units <= MAX_COMMAND_UNITS).then_some(cmd)
    }
}

/// A command with the slot that issued it, as delivered by the session for one tick.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerCommand {
    pub player: u8,
    pub command: Command,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slots::Handle;

    #[test]
    fn round_trip() {
        let cmd = Command::Move {
            units: vec![Handle::new(3, 1), Handle::new(9, 0)],
            target: FxVec2::from_ints(100, -5),
            queue: true,
        };
        assert_eq!(Command::decode(&cmd.encode()), Some(cmd));
    }

    #[test]
    fn player_orders_round_trip() {
        let units = vec![Handle::new(3, 1)];
        let at = FxVec2::from_ints(100, -5);
        for cmd in [
            Command::SetFireState {
                units: units.clone(),
                state: FireState::HoldFire,
            },
            Command::AttackGround {
                units: units.clone(),
                pos: at,
                queue: false,
            },
            Command::Bombard {
                units: units.clone(),
                pos: at,
                radius: Fx::from_int(40),
                queue: true,
            },
            Command::Patrol {
                units: units.clone(),
                points: vec![at, FxVec2::ZERO],
                queue: false,
            },
            Command::PatrolInsert {
                units: units.clone(),
                after: at,
                point: FxVec2::ZERO,
            },
            Command::CancelOrder {
                units: units.clone(),
                kind: OrderKind::Patrol,
                pos: at,
            },
        ] {
            assert_eq!(Command::decode(&cmd.encode()), Some(cmd));
        }
        // A whole army in one order decodes; one more unit than a side can have does not.
        let army = Command::FormationMove {
            units: vec![Handle::new(1, 0); MAX_COMMAND_UNITS],
            target: at,
            queue: false,
            attack_move: true,
            facing: None,
            together: true,
            spacing: 1,
            shape: -3,
        };
        assert_eq!(Command::decode(&army.encode()), Some(army));
        let crowd = Command::Patrol {
            units: vec![Handle::new(1, 0); MAX_COMMAND_UNITS + 1],
            points: vec![at],
            queue: false,
        };
        assert_eq!(Command::decode(&crowd.encode()), None);
    }

    #[test]
    fn garbage_is_rejected() {
        assert_eq!(Command::decode(&[0xFF; 3]), None);
        assert_eq!(Command::decode(&[]), None);
        // A Move whose length prefix claims far more units than the limit.
        let mut bytes = vec![0, 0, 0, 0];
        bytes.extend_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(Command::decode(&bytes), None);
    }
}
