//! The render mirror: what the simulation publishes for presentation.
//!
//! Built once at the end of a tick on the sim side. Each record carries the
//! previous and the current tick's transform, so the GPU interpolates on its
//! own and the render thread never touches an entity. Floats are fine here:
//! nothing in this module is read back by the simulation.
#![expect(
    clippy::float_arithmetic,
    clippy::disallowed_types,
    clippy::disallowed_methods,
    reason = "presentation: builds the RenderFrame (instances, beams, welds, events) the renderer, HUD \
              and audio read; nothing here is written into State"
)]

use crate::reclaim::{BEAM_NANITE, BEAM_NANITE_SITE};
use crate::tables::UnitId;
use crate::World;
use bytemuck::{Pod, Zeroable};
use mc_core::{Fx, FxVec3, TICKS_PER_SECOND};
use mc_data::{BlueprintId, Trajectory, WeaponColor};
use std::collections::HashMap;

mod batch;
mod destruct;
mod fog;
mod units;
mod walls;
mod warp;
mod wrecks;

pub use batch::{BatchView, UNIT_BATCH};
pub use destruct::DestructView;
pub use fog::VisionDisc;
pub use walls::{join_walls, WALL_JOINS};
pub use warp::{UNIT_IN_WARP, UNIT_WARP_DAMPED};
pub use wrecks::WRECK_EXTRA_INSTANCES;

/// Wave origins drawn at once. A crowd on one site is clustered before it lands here.
pub const MAX_CONSTRUCTION_WELDS: usize = 2048;
/// Distinct origins kept on one hull; more builders merge into the nearest of these.
pub const MAX_WELDS_PER_SITE: usize = 12;
/// How long work light keeps running out after the beam that raised it goes off.
const WELD_LINGER_SECONDS: f32 = 2.5;
/// An origin that moved less than this (metres) is the same weld as last tick.
const WELD_STICK: f32 = 10.0;

/// One print origin lighting a construction site. `fade` is 1 while a beam is
/// on it, then runs down so the waves from that side do not pop off.
#[derive(Clone, Copy, Debug, Default)]
pub struct ConstructionWeld {
    pub site: u32,
    pub local: [f32; 3],
    pub fade: f32,
}

/// Why a command was refused (`SimEvent::CommandRefused`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Refusal {
    /// A patrol route may have at most `command::MAX_PATROL_POINTS` points.
    PatrolTooLong,
    /// A factory keeps at most `standing::MAX_STANDING` orders for its products.
    StandingOrdersFull,
    /// A factory's queue holds at most `orders::MAX_FACTORY_QUEUE` orders.
    FactoryQueueFull,
    /// A core mine stands only on a mine point, and none was near enough.
    NotOnMinePoint,
    /// The mine point already has a mine on it, finished or being built.
    MinePointTaken,
}

impl Refusal {
    pub fn message(self) -> String {
        match self {
            Refusal::PatrolTooLong => {
                format!(
                    "A patrol takes at most {} points",
                    crate::command::MAX_PATROL_POINTS
                )
            }
            Refusal::StandingOrdersFull => format!(
                "A factory keeps at most {} orders for its units",
                crate::standing::MAX_STANDING
            ),
            Refusal::FactoryQueueFull => format!(
                "A factory queue holds at most {} units",
                crate::orders::MAX_FACTORY_QUEUE
            ),
            Refusal::NotOnMinePoint => "A mine goes only on a mine point".to_string(),
            Refusal::MinePointTaken => "That mine point already has a mine".to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum SimEvent {
    UnitCompleted {
        unit: UnitId,
        owner: u8,
    },
    UnitDied {
        pos: FxVec3,
        blueprint: BlueprintId,
        owner: u8,
        airborne: bool,
        /// Built: false for a build site (or a unit still in its factory's frame), which
        /// goes up as a frame does, not as the finished thing.
        complete: bool,
    },
    /// Side `by` is credited with killing an `owner` unit of `blueprint` (`veterancy::settle_kill`).
    /// The `UnitDied` follows when the unit is reaped. `weapon_of`: the killing unit's
    /// blueprint, when it was a unit. Read by the battle report, never by the sim.
    UnitKilled {
        blueprint: BlueprintId,
        owner: u8,
        by: u8,
        weapon_of: Option<BlueprintId>,
        /// Built: false for a build site or a frame still in its factory.
        complete: bool,
    },
    /// The second detonation, at the end of a dead aircraft's fall.
    AircraftCrashed {
        pos: FxVec3,
        blueprint: BlueprintId,
    },
    /// A builder's clearing field over the lot at `pos`, `half` metres each way
    /// from it: `wave` 0 when it goes up, then each wave as it runs out from the
    /// middle, the last (`trees::CLEAR_WAVES`) reaching the edge.
    LotClearing {
        pos: mc_core::FxVec2,
        half: Fx,
        wave: u8,
    },
    /// A clearing field centred on `center` took map prop `prop`, a tree.
    TreeVaporized {
        prop: u32,
        center: mc_core::FxVec2,
    },
    /// A big walker knocked over map prop `prop`, a tree: it walked from `from` by `motion` this tick.
    TreeTrampled {
        prop: u32,
        from: mc_core::FxVec2,
        motion: mc_core::FxVec2,
    },
    /// The last of a unit, or of a wreck, went up a reclaim beam. Instead of `UnitDied`: nothing blows up.
    Reclaimed {
        pos: FxVec3,
        blueprint: BlueprintId,
        wreck: bool,
    },
    /// A weapon with a target began charging for its next salvo. During a reload
    /// that is `charge_ticks` before it is due; the first shot, and a shot after
    /// sitting idle, charge once the tube is on.
    WeaponCharging {
        /// The unit charging, so what is drawn can follow it while it charges.
        unit: crate::tables::UnitId,
        pos: FxVec3,
        owner: u8,
        blueprint: BlueprintId,
        weapon: u8,
    },
    /// `vel` is metres per tick. `blueprint` and `weapon` name the weapon that fired.
    ShotFired {
        pos: FxVec3,
        vel: FxVec3,
        /// The firing unit's own travel this tick (metres). Units are drawn gliding
        /// from last tick's place to this one's, so as the shot is seen to leave, the
        /// gun is drawn this far short of `pos`.
        travel: FxVec3,
        color: WeaponColor,
        owner: u8,
        blueprint: BlueprintId,
        weapon: u8,
    },
    /// A cold-launched missile has pitched onto its target and its motor starts here.
    MissileIgnited {
        pos: FxVec3,
        vel: FxVec3,
        blueprint: BlueprintId,
        weapon: u8,
    },
    /// A cluster shot (`Weapon::cluster`) broke into `count` sub-shots at `pos`, flying on
    /// at `vel` (metres per tick) before they part (`cluster.rs`).
    ClusterSplit {
        pos: FxVec3,
        vel: FxVec3,
        count: u8,
        owner: u8,
        blueprint: BlueprintId,
        weapon: u8,
    },
    /// An anti-missile laser from `from` to a hostile missile at `to`.
    /// `killed` is the tick the casing fails. `blueprint` is the defender's: its faction
    /// says how the kill is drawn (`mc_data::AntiMissileLook`).
    MissileLased {
        from: FxVec3,
        to: FxVec3,
        killed: bool,
        blueprint: BlueprintId,
    },
    /// An interceptor torpedo met the torpedo it was fired at, at `pos` under the water:
    /// both burst there.
    TorpedoIntercepted {
        pos: FxVec3,
    },
    /// A missile left a dived hull at `pos` (under the water): the boil on the surface, and
    /// the launch that paints the boat.
    DivedLaunch {
        pos: FxVec3,
        blueprint: BlueprintId,
        weapon: u8,
    },
    /// `after` is how far into the tick the shot arrived, zero to one, so the
    /// burst can wait for the shell that is drawn flying in. `on_unit`: it
    /// struck a unit rather than the ground. `on_shield`: it struck a dome.
    Impact {
        pos: FxVec3,
        /// Struck hull's measured displacement in this tick, including climb.
        /// Presentation only: delayed fragments can lead its rendered motion.
        /// Ground and shield interceptions have no freely moving target.
        target_motion: FxVec3,
        splash: mc_core::Fx,
        color: WeaponColor,
        after: mc_core::Fx,
        on_unit: bool,
        on_shield: bool,
        blueprint: BlueprintId,
        weapon: u8,
    },
    /// A shield bubble ran out of hit points and shattered.
    ShieldBroken {
        pos: FxVec3,
        radius: mc_core::Fx,
        owner: u8,
    },
    /// The terrain edit table grew (or was replaced by a snapshot).
    TerrainEdited,
    BuildRejected {
        player: u8,
    },
    /// A command ran into a limit and was refused, or refused in part (see `Refusal`).
    /// Limits are never applied silently: the player is told.
    CommandRefused {
        player: u8,
        reason: Refusal,
    },
    PlayerDefeated {
        player: u8,
    },
    MatchOver {
        winner_team: u8,
    },
    /// A sinking hull came to rest on the seabed at `pos` (it is a wreck from now on).
    ShipSettled {
        pos: FxVec3,
        blueprint: BlueprintId,
    },
    /// Survival: the engine began printing round `round`.
    RoundPrinting {
        round: u16,
    },
    /// Survival: round `round` left the engine, `units` strong.
    RoundLaunched {
        round: u16,
        units: u16,
    },
    /// Survival: the engine's ray is raising a node at map node site `site`.
    NodeRaising {
        site: u8,
        pos: mc_core::FxVec2,
        product: BlueprintId,
    },
    /// Survival: the node at `site` is up and printing `product`.
    NodeOnline {
        site: u8,
        pos: mc_core::FxVec2,
        product: BlueprintId,
    },
    /// Survival: the node at `site` was destroyed (`raised`: after it came online);
    /// `wreck`: mass its wreck holds for the defenders to reclaim (0 if it never rose).
    NodeDestroyed {
        site: u8,
        pos: mc_core::FxVec2,
        product: BlueprintId,
        wreck: u32,
        raised: bool,
    },
    /// Survival: the defenders held out through the last round.
    SurvivalWon {
        rounds: u16,
    },
    /// An Argon Electric Bore's tracer landed at `to` and the charge ran down its channel
    /// from `from` (`bore.rs`): the bolt, and `width` either side of it seared.
    /// Comes right after the tracer's `Impact`.
    BoreDischarge {
        from: FxVec3,
        to: FxVec3,
        width: mc_core::Fx,
        /// How far into the tick the tracer landed, zero to one (as `Impact`).
        after: mc_core::Fx,
        owner: u8,
        blueprint: BlueprintId,
        weapon: u8,
    },
    /// A giant bore (`Bore::storm`) started its charge on `target`: it fires in `ticks`
    /// and its storm will spread `radius` metres from where it lands. For the strike
    /// warning the HUD shows (both sides see it where they can see the ground).
    StormCharging {
        unit: crate::tables::UnitId,
        muzzle: FxVec3,
        target: FxVec3,
        /// The unit it is on, when it is on one rather than on the ground: the mark
        /// moves with it.
        on: Option<crate::tables::UnitId>,
        radius: mc_core::Fx,
        ticks: u16,
        owner: u8,
        blueprint: BlueprintId,
        weapon: u8,
    },
    /// A giant bore changed its mark part way through its charge: it now lands on `target`
    /// (`None`: it has nothing to fire at) in `left` ticks. Carries what `StormCharging`
    /// does, so a listener that missed the start can pick the charge up here.
    StormRetargeted {
        unit: crate::tables::UnitId,
        target: Option<FxVec3>,
        on: Option<crate::tables::UnitId>,
        left: u16,
        radius: mc_core::Fx,
        ticks: u16,
        owner: u8,
        blueprint: BlueprintId,
        weapon: u8,
    },
    /// A giant bore's storm at `pos` died before its time: the machine feeding it is gone.
    StormCollapsed {
        pos: FxVec3,
    },
    /// A giant gun threw out a spent sabot from `from` at `vel` (m/s): the sim flies it
    /// (`titan::FallingSabot`, drawn as a tumbling falling wreck); this is for the kick of
    /// the ejector and the vapour it trails.
    SabotThrown {
        from: FxVec3,
        vel: FxVec3,
        blueprint: BlueprintId,
        weapon: u8,
    },
    /// A giant gun's spent sabot (`Weapon::sabot`) came down at `pos` and burst; its
    /// scrap lies there as a wreck. Thrown by weapon `weapon` of `blueprint`.
    SabotLanded {
        pos: FxVec3,
        blueprint: BlueprintId,
        weapon: u8,
    },
    /// A charged shell (`Weapon::discharge`) landed at `to`, coming in from `from`: the
    /// last stretch of its flight, down which its charge strikes. Presentation only;
    /// comes right after the shell's `Impact`.
    ShellDischarge {
        from: FxVec3,
        to: FxVec3,
        /// How far into the tick the shell landed, zero to one (as `Impact`).
        after: mc_core::Fx,
        blueprint: BlueprintId,
        weapon: u8,
    },
    /// A silo's blast doors start to open for a launch (`nukes.rs`).
    SiloOpening {
        pos: FxVec3,
        owner: u8,
    },
    /// A nuclear warhead lit in its silo, bound for `to` (its burst point). Everyone is told.
    NuclearLaunch {
        from: FxVec3,
        to: FxVec3,
        owner: u8,
        serial: u32,
        /// How its missiles and blast are drawn and heard (presentation only).
        look: mc_data::strategic::StrategicLook,
    },
    /// An interceptor out of its cell at `from`.
    InterceptorLaunch {
        from: FxVec3,
        owner: u8,
        serial: u32,
        /// How its missiles and blast are drawn and heard (presentation only).
        look: mc_data::strategic::StrategicLook,
    },
    /// An interceptor burst at `pos`: `killed` a warhead, or burnt out with nothing to hit.
    WarheadIntercepted {
        pos: FxVec3,
        owner: u8,
        killed: bool,
        /// The warhead's look when `killed` (its containment breaking), else the interceptor's.
        look: mc_data::strategic::StrategicLook,
    },
    /// A nuclear blast: `radius` is how far it does damage; its front runs out over
    /// seconds after this. `commander`: a commander's reactor, not a warhead.
    NuclearDetonation {
        pos: FxVec3,
        radius: mc_core::Fx,
        owner: u8,
        commander: bool,
        look: mc_data::strategic::StrategicLook,
    },
    /// A launcher finished assembling a round (`warhead`: a silo's; else an interceptor).
    RoundReady {
        unit: UnitId,
        pos: FxVec3,
        owner: u8,
        warhead: bool,
    },
    /// A capital ship began spooling its drive at `from` for a jump to `to`; it jumps in
    /// `ticks` at the soonest, once its nose is on the mark (`warp.rs`).
    WarpSpooling {
        unit: UnitId,
        from: FxVec3,
        to: mc_core::FxVec2,
        ticks: u16,
        blueprint: BlueprintId,
        owner: u8,
    },
    /// It went into warp at `from`, bound for `to`, and comes out in `ticks`: `dampened`
    /// when an enemy dampener's field already covers `to`.
    WarpJumped {
        unit: UnitId,
        from: FxVec3,
        to: FxVec3,
        ticks: u16,
        dampened: bool,
        blueprint: BlueprintId,
        owner: u8,
    },
    /// A dampener raised part way through a jump snagged it: it drags on, torn.
    WarpSnagged {
        unit: UnitId,
        at: mc_core::FxVec2,
    },
    /// It came out of warp at `at`; `dampened`: thrown out hurt and stunned.
    WarpArrived {
        unit: UnitId,
        at: FxVec3,
        dampened: bool,
        blueprint: BlueprintId,
        owner: u8,
    },
    /// An EMP stunned `unit` for `ticks`.
    Stunned {
        unit: UnitId,
        ticks: u16,
    },
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct UnitInstance {
    pub prev_pos: [f32; 3],
    pub prev_heading: f32,
    pub pos: [f32; 3],
    pub heading: f32,
    pub blueprint: u32,
    /// `owner | flags << 8`, plus the `KIND_*` bits.
    pub owner_flags: u32,
    pub health: f32,
    /// Zero to one; one when complete.
    pub build: f32,
    /// Turret yaw of the first weapon relative to the hull, radians. A settled
    /// wreck: the yaw it died at, where its thrown turret starts from.
    pub turret_yaw: f32,
    pub radius: f32,
    pub unit_id: u32,
    /// Props: scale in thousandths. Units: kills in the low 14 bits, the fire
    /// state in 14..16 (`UNIT_FIRE_STATE_SHIFT`), rank in 16..23, `UNIT_BURNING`,
    /// progress toward the next rank (0..=255) in 24..32.
    /// Wrecks: WRECK_FALLING while still airborne, zero after impact.
    pub packed: u32,
    /// Ground covered in metres (wraps at 4096), then what this tick and the
    /// tick before added to it: the vertex shader times a walker's stride by it.
    /// A spent casing in the air: seconds since it was thrown, last tick and this.
    /// A wreck left in the last `WRECK_SETTLED` seconds: seconds since it was left, last
    /// tick and this, then 1. Zero on one long settled, which is drawn as it lies.
    pub gait: [f32; 3],
    /// Zero, or how far along the unit's refit is: above zero from its first tick, one when done.
    pub upgrade: f32,
    /// Pitch of the gun arm last tick and this, then of the build arm: radians, up positive.
    /// A prop's pose instead (the renderer's): a wire span's pitch and raise, a fallen
    /// tree's pitch and sink, and a tree's stretch in height in the last.
    pub arm_pitch: [f32; 4],
    /// `turret_yaw` last tick, so the turret glides between ticks like the hull does.
    pub prev_turret_yaw: f32,
    /// Local-space point a live beam is printing from. Zero when nobody is
    /// working and lingering waves have run out. The hull fill does not use it.
    pub weld: [f32; 3],
    /// How far the barrel is kicked back: 1 the instant it fires, 0 at rest.
    pub recoil: f32,
    /// `recoil` last tick, so the slide glides between ticks like the turret does.
    pub prev_recoil: f32,
    /// Range into `RenderFrame::welds`: every origin still lighting this site.
    pub weld_first: u32,
    pub weld_count: u32,
    /// How far a siege gun is planted: 0 packed, 1 ready to fire.
    pub deploy: f32,
    /// `deploy` last tick, so the outriggers glide between ticks.
    pub prev_deploy: f32,
    /// Previous/current aircraft roll in radians (zero for ground units).
    pub _pad2: [f32; 2],
    /// While a refit is under way, the look bits of the loadout it is fitting
    /// (`Blueprints::look`): the pieces that are going up. Zero otherwise.
    pub refit_modules: u32,
    /// Three more state words, their bits named by the `UNIT_*` constants: 0 dive and
    /// deck state and the pause mark, 1 gun-house index, nanite and replicating marks,
    /// 2 a launcher's rounds (`nukes::LAUNCHER_*`), a cell launcher's loaded cells
    /// (`launch_cells::loaded_cells`), a mounted gun's aim or a wall
    /// section's neighbours ([`WALL_JOINS`]), a stored unit's carrier; on a spent
    /// casing in the air, one more than the index of the walker that threw it in
    /// `RenderFrame::units` (zero when it is not drawn).
    pub status: [u32; 3],
    /// A weapon on a turret of its own (`Weapon::mount`): its yaw off the torso last tick
    /// and this, then its pitch last tick and this (radians).
    pub mount: [f32; 4],
    /// A rotary gun's barrels, turned last tick and this (radians, unwrapped between the
    /// two); then the mounted weapon's kick-back, last tick and this (1 the instant it fires).
    pub spin_recoil: [f32; 4],
    /// A warp and an EMP (`crate::warp`): its warp drive, last tick and this (in a jump
    /// how far into its streak, 0 whole to 1 a streak of light; spooling, minus how full
    /// its charge is, 0 to -1), then how stunned it is, last tick and this (0 to 1,
    /// falling to 0 over the stun's last seconds). A settled wreck: zero, zero, then the
    /// share of the hull reclaim has taken (0 to 1) and seconds since reclaim last worked it.
    /// The renderer's copy of a wreck whose last mass went sends 1 + how far it has gone
    /// in the share (`mc_render` wreck_finish.rs).
    pub fx: [f32; 4],
    /// A spacecraft's stern drives (`CapitalRig::drives`): how far their nozzles are swung,
    /// last tick and this (radians, toward the side the nose turns to). The mirror leaves
    /// it zero; the renderer eases it toward the hull's turn in the copy it draws
    /// (`capital_fx.rs` `drive_swing`), so the nozzles glide instead of jumping each tick.
    pub drive_swing: [f32; 2],
    pub _pad3: [f32; 2],
}

const _: () = assert!(std::mem::size_of::<UnitInstance>() == 224);

impl UnitInstance {
    /// A settled wreck's second and later sections, and every section's inside: more of
    /// a wreck already listed. Anything counting wrecks (salvage totals, smoke) skips them.
    pub fn is_wreck_extra(&self) -> bool {
        self.owner_flags & KIND_WRECK != 0
            && self.packed == 0
            && self.refit_modules & WRECK_POSED != 0
            && (self.refit_modules & WRECK_INNER != 0
                || (self.refit_modules >> WRECK_SECTION_SHIFT) & 0xF != 0)
    }

    /// A settled wreck: the share of a fresh wreck's mass still in it. Its `health` leaves
    /// out what reclaim took, since reclaim does not wear the hull away (`fx`).
    pub fn wreck_left(&self) -> f32 {
        self.health * (1.0 - self.fx[2])
    }

    pub fn pack_veterancy(kills: u32, level: u8, progress: f32) -> u32 {
        let kills = kills.min(UNIT_KILLS_MASK);
        let progress = (progress.clamp(0.0, 1.0) * 255.0).round() as u32;
        kills | (level as u32) << 16 | progress << 24
    }

    pub fn kill_count(&self) -> u32 {
        self.packed & UNIT_KILLS_MASK
    }

    /// Whether the unit's weapons pick targets of their own. `FireAtWill` for anything not a unit.
    /// A submarine ordered down: diving, or dived.
    pub fn dive_goal(&self) -> bool {
        self.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST) == 0
            && self.status[0] & UNIT_DIVE_GOAL != 0
    }

    /// A capital ship standing on the ground, its gear all the way out (`UNIT_GEAR_SHIFT`).
    pub fn set_down(&self) -> bool {
        self.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST) == 0
            && (self.status[0] >> UNIT_GEAR_SHIFT) & 0xFF == 0xFF
    }

    /// Stored in a lift ship's hold (`UNIT_STORED`).
    pub fn stored(&self) -> bool {
        self.status[0] & UNIT_STORED != 0
    }

    /// The lift ship whose hold it rides in (`UNIT_STORED`), by unit id.
    pub fn carrier(&self) -> Option<u32> {
        self.stored().then_some(self.status[2])
    }

    /// A factory with batch on (`UNIT_BATCH`): its products form up and leave together.
    pub fn batching(&self) -> bool {
        self.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST) == 0
            && self.status[0] & UNIT_BATCH != 0
    }

    /// Work paused by its player: it keeps its queue but builds nothing.
    pub fn paused(&self) -> bool {
        self.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST) == 0
            && self.status[0] & UNIT_PAUSED != 0
    }

    /// How far a submarine is under: 0 surfaced, 1 dived.
    pub fn dive(&self) -> f32 {
        if self.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST) != 0 {
            return 0.0;
        }
        (self.status[0] & UNIT_DIVE_MASK) as f32 / UNIT_DIVE_MASK as f32
    }

    pub fn fire_state(&self) -> crate::tables::FireState {
        if self.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST) != 0 {
            return crate::tables::FireState::FireAtWill;
        }
        crate::tables::FireState::from_bits(self.packed >> UNIT_FIRE_STATE_SHIFT)
    }

    pub fn veterancy_level(&self) -> u8 {
        ((self.packed >> 16) & 0x7F) as u8
    }

    /// Zero to one toward the next rank. Zero when already at the top.
    pub fn veterancy_progress(&self) -> f32 {
        ((self.packed >> 24) & 0xFF) as f32 / 255.0
    }
}

/// Instant kick when the tube fires, then a cubic ease back home in about
/// a second and a half. Spawn stagger sits on half a reload, well past this.
fn barrel_recoil(cooldown: u16, reload: u16) -> f32 {
    if reload == 0 || cooldown == 0 {
        return 0.0;
    }
    let since = reload.saturating_sub(cooldown);
    if since >= 16 {
        return 0.0;
    }
    let t = since as f32 / 16.0;
    let u = 1.0 - t;
    u * u * u
}

/// A casing lying where it came down (`FallingSabot::lying`), drawn as `casing` (its
/// radius and height): flat at its resting place and heading, sinking into the ground over
/// its last `SABOT_SINK_TICKS`.
#[expect(
    clippy::float_arithmetic,
    reason = "presentation: the render mirror's pose for a lying casing"
)]
fn lying_casing(
    sabot: &crate::titan::FallingSabot,
    casing: BlueprintId,
    [radius, height]: [f32; 2],
    i: usize,
) -> UnitInstance {
    use crate::titan::{SABOT_LIE_TICKS, SABOT_SINK_TICKS};
    let sink = |lying: u16| {
        let left = SABOT_LIE_TICKS.saturating_sub(lying) as f32 / SABOT_SINK_TICKS as f32;
        height * 1.2 * (1.0 - left.min(1.0))
    };
    let rest = sabot.rest.to_f32();
    let at = |lying: u16| [rest[0], rest[1], rest[2] - sink(lying)];
    let heading = sabot.rest_yaw.to_radians_f32();
    UnitInstance {
        prev_pos: at(sabot.lying - 1),
        pos: at(sabot.lying),
        prev_heading: heading,
        heading,
        blueprint: casing.0 as u32,
        owner_flags: KIND_WRECK,
        health: 1.0,
        build: 1.0,
        radius,
        unit_id: 0x5AB0_0000 | (sabot.seed & 0xFFFF) ^ i as u32,
        deploy: 1.0,
        prev_deploy: 1.0,
        ..UnitInstance::zeroed()
    }
}

/// This tick's kick and last tick's, so the shader can interpolate. A shot
/// this tick starts from rest (`prev` 0) even though cooldown just jumped
/// to `reload`.
fn barrel_recoil_pair(cooldown: u16, reload: u16) -> (f32, f32) {
    let now = barrel_recoil(cooldown, reload);
    let prev = if cooldown == reload || cooldown == 0 {
        0.0
    } else {
        barrel_recoil(cooldown + 1, reload)
    };
    (now, prev)
}

/// A held beam's brace (`Weapon::beam`), this tick and last, in place of a kick: how far
/// its projector has run up (`Weapon::spin_ticks`, `Units::spin`), zero to one, so the gun
/// and whatever carries it settle onto the shot and ease off after it. A beam that does not
/// run up is braced while it fires.
fn beam_brace(weapon: &mc_data::Weapon, spin: [u16; 4], cooldown: u16) -> (f32, f32) {
    if weapon.spin_ticks == 0 {
        let on = if cooldown > 0 { 1.0 } else { 0.0 };
        return (on, on);
    }
    let level = |n: u16| n.min(weapon.spin_ticks) as f32 / weapon.spin_ticks as f32;
    (level(spin[0]), level(spin[3]))
}

/// `owner_flags` bit: the record is a wreck. `health` holds the share of mass left.
pub const KIND_WRECK: u32 = 1 << 31;
/// _pad on a KIND_WRECK instance: intact falling hull, not settled salvage.
pub const WRECK_FALLING: u32 = 1;
/// _pad on a KIND_WRECK instance: a ship's hull going down through the water to the
/// seabed. `arm_pitch` = [prev pitch, pitch, 0, 0], `_pad2` = [prev roll, roll], and
/// `health` how far it has gone down (0 at the surface, 1 on the bottom).
pub const WRECK_SINKING: u32 = 2;
/// Seconds a wreck's `gait` counts its age for: long after its thrown turret has landed.
const WRECK_SETTLED: f32 = 30.0;
/// `refit_modules` of a settled wreck (`mirror/wrecks.rs`), as `mc_models::gpu_consts::wreck`
/// spells it (a test holds them equal): the low four bits how it came down
/// (`tables::Landing`), then which section of the hull this is and how many it broke
/// into. `arm_pitch` is then its pitch (last tick and this) and the stretch of the hull
/// it keeps, and `_pad2` its roll. Zero on a spent casing, which lies as it is.
pub const WRECK_POSED: u32 = 1 << 13;
pub const WRECK_LANDING_MASK: u32 = 0xF;
pub const WRECK_SECTION_SHIFT: u32 = 4;
pub const WRECK_COUNT_SHIFT: u32 = 8;
/// A section's second instance: the hull's inside, seen through its torn ends.
pub const WRECK_INNER: u32 = 1 << 12;
/// Units' `status[0]`, low byte: how far a submarine has dived, 0 surfaced to 255 down.
pub const UNIT_DIVE_MASK: u32 = 0xFF;
/// Units' `status[0]`: the submarine is ordered down (diving or dived), else up.
pub const UNIT_DIVE_GOAL: u32 = 1 << 8;
/// Units' `status[0]`: the player paused this unit's work (`Command::SetPaused`).
pub const UNIT_PAUSED: u32 = 1 << 9;
pub const UNIT_BURNING: u32 = 1 << 23;
/// Units' `status[0]`: a unit stored in a lift ship's hold. Listed for its own side's
/// interface (the hold, selecting and ordering it); never drawn. It is `IN_FACTORY` too,
/// and `status[2]` is the ship's unit id ([`UnitInstance::carrier`]).
pub const UNIT_STORED: u32 = 1 << 11;
/// Units' `status[0]` bits 16..24: a lift ship's landing gear, 0 stowed to 255 out.
pub const UNIT_GEAR_SHIFT: u32 = 16;
/// Units' `status[0]`: a land unit standing on a lift ship's deck or ramp
/// (`transport::Deck::up`). `status[2]` then holds its up vector's x and y, each an
/// i16 over 32767 (low half x), for the shader to lean it with the ramp, not the ground.
pub const UNIT_ON_DECK: u32 = 1 << 24;
/// Units' `status[0]`: a drone docked on an aircraft (`World::drone_riding`), drawn in the
/// frame its carrier is drawn in. `status[2]` is one more than the carrier's index in
/// `RenderFrame::units`. `mc_models::gpu_consts::dock::RIDING`; a test holds them equal.
pub const UNIT_RIDING: u32 = 1 << 10;
/// Units' `status[1]`: the unit is being printed by a replicator (Survival). Its
/// construction fill is drawn in replication violet instead of construction amber.
pub const UNIT_REPLICATING: u32 = 1 << 0;
/// Units' `status[1]`: a construction site of a faction that builds with nanites
/// (`mc_data::Construction::Nanite`), or one of theirs being refitted: a black swarm condenses
/// into it instead of it being printed.
pub const UNIT_NANITE: u32 = 1 << 1;
/// Units' `_pad`: kills shown, at most this many.
pub const UNIT_KILLS_MASK: u32 = 0x3FFF;
/// Units' `_pad`: where the two bits of `FireState` sit. Read with `UnitInstance::fire_state`.
pub const UNIT_FIRE_STATE_SHIFT: u32 = 14;
/// `owner_flags` bit: a map prop (set by the renderer for its static records).
pub const KIND_PROP: u32 = 1 << 30;
/// `owner_flags` bit: a placement preview added by the UI, not a real unit.
pub const KIND_GHOST: u32 = 1 << 29;
/// `owner_flags` bit: the unit has no orders. Above the sim's sixteen flag bits.
pub const STATE_IDLE: u32 = 1 << 24;
/// Detected by radar only: draw the strategic icon, never the model.
pub const STATE_RADAR: u32 = 1 << 25;
/// Radar contact that vision has never identified: a grey blip, not the real icon.
pub const STATE_UNIDENTIFIED: u32 = 1 << 26;
/// Powered kit (a radar dish, a shield crystal) is dark: the economy cannot pay its upkeep.
pub const STATE_UNPOWERED: u32 = 1 << 27;
/// A shattered dome is filling while down. The crystal pulses; the wreath turns slowly.
pub const STATE_CHARGING: u32 = 1 << 28;

/// Most units `write_orders` lists when asked for a whole side's queues.
pub const MAX_LISTED_UNITS: usize = 1024;
/// Most orders listed per unit in `UnitOrders`; a longer queue is cut short for display.
pub const MAX_LISTED_ORDERS: usize = 96;

/// One entry of a unit's order queue, for the interface.
#[derive(Clone, Copy, Debug)]
pub struct QueuedOrder {
    /// The command group the order was given to; zero for a unit ordered on its own.
    pub formation: u64,
    pub offset: [f32; 2],
    pub moving_slot: Option<[f32; 2]>,
    pub formation_phase: u8,
    pub kind: crate::tables::OrderKind,
    /// Where the order takes the unit: its position, or its target's.
    pub pos: [f32; 2],
    /// The order's own position, exactly: what `Command::RelocateOrder` takes as `from`.
    pub at: mc_core::FxVec2,
    /// What `Build`, `Produce` and `Upgrade` make.
    pub blueprint: BlueprintId,
    /// `Bombard`: how far from `at` shots may fall; `Guard`: the area watched (aircraft
    /// circle it halfway out); `ReclaimArea`: the circle cleared, metres. Zero for every
    /// other kind.
    pub radius: f32,
}

/// The order queue of one unit the interface asked about, front first.
#[derive(Clone, Debug, Default)]
pub struct UnitOrders {
    pub unit_id: u32,
    pub orders: Vec<QueuedOrder>,
    /// A factory's standing orders: what each unit it rolls out is told to do, in order.
    pub standing: Vec<QueuedOrder>,
    /// Zero to one: how far along the thing this unit is building is. Zero when it builds nothing.
    pub progress: f32,
    /// Seconds of game time before that thing is finished at last tick's pace, every
    /// builder on it counted; `None` when it builds nothing or the work stands still.
    pub eta: Option<f32>,
    /// The unit id of that thing (itself while it goes up with builders on it), so a
    /// site's own tag can read the time left off the builders at it.
    pub building: Option<u32>,
    /// Per second, last tick. Made: what it produced (generator or mine output, materials
    /// reclaimed). Wanted: what its building, repairs and upkeep asked for at the full rate.
    /// Used: what it was given of that; less than wanted while its side stalls.
    pub mass_made: f32,
    pub energy_made: f32,
    pub mass_wanted: f32,
    pub energy_wanted: f32,
    pub mass_used: f32,
    pub energy_used: f32,
    /// Its side's share of demand met last tick, zero to one: below one is a stall.
    pub efficiency: f32,
    /// Mass it has reclaimed since it was made (a carrier's includes its drones').
    pub reclaimed: f32,
    /// The highest tier its side has reached ([`World::side_tech`]): mines upgrade no further.
    pub side_tech: u8,
    /// A finished core mine's output.
    pub mine: Option<MineView>,
    /// Where the target its guns are laid on stands, its middle, while the ground hides
    /// it and none of them has anything it can see (`line_of_fire.rs`).
    pub hidden_target: Option<[f32; 3]>,
    /// A lift ship: what is in its hold.
    pub cargo: Option<CargoView>,
    /// Seconds before its warp drive may spool again (`Warp::cooldown_ticks`); zero when
    /// ready or without a drive.
    pub warp_recharge: f32,
    /// Seconds before its stun wears off (`crate::warp`); zero when not stunned.
    pub stunned: f32,
    /// A factory with batch on: its muster block and who stands in it.
    pub batch: Option<BatchView>,
}

/// A lift ship as the interface shows it: its hold (`transport.rs`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CargoView {
    /// Room in the hold, and how much of it is taken.
    pub capacity: u16,
    pub used: u16,
    /// What rides in it, in the order it would walk out.
    pub stored: Vec<CargoUnit>,
    /// Units on their way up its ramp.
    pub boarding: u16,
    /// Down, with the ramp open: units can walk on and off.
    pub ramp_down: bool,
    /// Letting the hold out.
    pub unloading: bool,
    /// What the ship is doing, for the hold panel's status line.
    pub phase: LiftPhase,
    /// Units its unload orders in hand will still let out.
    pub to_unload: u16,
}

/// What a lift ship is doing, as its hold panel reports it (`World::cargo_view`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LiftPhase {
    /// Up at cruise height, or flying about there.
    #[default]
    InFlight,
    /// Setting down: flying in to its site, gliding or settling.
    Descending,
    /// Down, the ramp on its way open.
    RampOpening,
    /// Down with the ramp open: units can board.
    Ready,
    /// Down with the ramp open, letting units out.
    Unloading,
    /// Down, raising the ramp to lift off.
    RampClosing,
    /// Lifting off and climbing back to cruise height.
    TakingOff,
}

/// One unit in a lift ship's hold.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CargoUnit {
    pub unit_id: u32,
    pub blueprint: BlueprintId,
    /// Zero to one of its full health.
    pub health: f32,
    /// Room it takes.
    pub room: u8,
}

/// A core mine as the interface shows it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MineView {
    /// Materials per second now (less in an energy stall), and at full power.
    pub rate: f32,
    pub full: f32,
    /// Seconds it has been digging.
    pub age: f32,
}

impl UnitOrders {
    /// The queue, then a factory's standing orders: everything its route is drawn from.
    pub fn route(&self) -> impl DoubleEndedIterator<Item = &QueuedOrder> {
        self.orders.iter().chain(&self.standing)
    }
}

/// A structure a builder has been ordered to build and has not begun.
#[derive(Clone, Copy, Debug)]
pub struct PlannedBuild {
    pub unit_id: u32,
    pub blueprint: BlueprintId,
    pub pos: [f32; 2],
    /// The site exactly: what `Command::RelocateOrder` takes as `from`.
    pub at: mc_core::FxVec2,
    pub heading: f32,
}

/// Set in `ProjectileInstance::color` for a missile. Not for a Gravitic Seeker, which has
/// no motor or body to draw: it is its plasma charge (`plasma_look`).
pub const PROJECTILE_MISSILE: u32 = 1 << 8;
/// `plasma_look` of a Regency seeker: a Gravitic Seeker, or any other plasma `missile`.
/// `mc_models::gpu_consts::plasma_look::GRAVITIC_SEEKER`
/// is the shader's copy; a test holds them equal.
pub const PLASMA_LOOK_GRAVITIC_SEEKER: u32 = 5;
/// Set in `ProjectileInstance::color` for a torpedo running under the water: no
/// tracer, flame or casing in the air; a dark body, bubbles and a wake instead.
pub const PROJECTILE_TORPEDO: u32 = 1 << 4;
/// Set in `ProjectileInstance::color` for a guided missile that runs in low over the
/// ground and water (`Weapon::skim`): a low, bright exhaust and a spray line on the sea
/// under it. Like `PROJECTILE_TORPEDO` this sits in the low byte: the colour is bits 0..4.
pub const PROJECTILE_SKIM: u32 = 1 << 5;
/// Set in `ProjectileInstance::color` for a guided missile that climbs to an apogee and
/// falls on its mark (`Weapon::apogee`): a boost column going up, the motor out and a
/// re-entry glow coming down. In the low byte, like `PROJECTILE_SKIM`.
pub const PROJECTILE_APOGEE: u32 = 1 << 6;
/// Set in `ProjectileInstance::color` for an ARC rail slug (`Weapon::rail`): a long
/// white-hot streak, and the renderer lays a cooling hot channel and vapour along its path.
pub const PROJECTILE_RAIL: u32 = 1 << 7;
/// Missile casing only: no motor glow or trail during cold launch.
pub const PROJECTILE_COLD: u32 = 1 << 15;
/// Set in `ProjectileInstance::color` for an energy slug that leaves a trail.
pub const PROJECTILE_TRAIL: u32 = 1 << 11;
/// Set for a conventional projectile that leaves a white smoke wake.
pub const PROJECTILE_SMOKE: u32 = 1 << 12;
/// An unpowered dropped bomb: an opaque black casing with no emissive tracer.
pub const PROJECTILE_BOMB: u32 = 1 << 13;
/// A fading hitscan beam: drawn whole from `prev_pos` to `pos`. `size` is width
/// in metres, `wake` the start time, `plasma` the life in seconds.
pub const PROJECTILE_FADE_BEAM: u32 = 1 << 14;
/// Set in `ProjectileInstance::color` for a construction beam: drawn whole from
/// `prev_pos` (the emitter) to `pos` (where the beam meets the work). `size` is the work's radius.
pub const PROJECTILE_BEAM: u32 = 1 << 10;
/// Low byte of `ProjectileInstance::color` for a construction beam (0 blue, 1 orange are weapons).
pub const COLOR_BUILD: u32 = 2;
/// Set in `ProjectileInstance::color` for a shot fired this tick: `prev_pos` is
/// the muzzle, and its trace must not reach back behind it.
pub const PROJECTILE_FRESH: u32 = 1 << 9;
/// Bits 16..24 of `ProjectileInstance::color`: the shot ends during this tick,
/// this many 255ths of the way through it, at `pos`. Zero for a shot still in flight.
pub const PROJECTILE_ENDS_SHIFT: u32 = 16;

/// Bits 24..32 of `ProjectileInstance::color`: the shot leaves the muzzle during this
/// tick, this many 255ths of the way through it. `prev_pos` is where it would have been
/// at the start of the tick; it is not drawn before then. Zero: it was already out.
pub const PROJECTILE_STARTS_SHIFT: u32 = 24;

/// Ticks between the rounds a stream gun's shot is seen as (`Weapon::rounds`): they
/// fill the time to the next shot, or the gun's own `round_span` (a jet pinched out).
pub fn round_gap(weapon: &mc_data::Weapon) -> f32 {
    let interval = if weapon.round_span > 0.0 {
        weapon.round_span
    } else if weapon.salvo > 1 && weapon.salvo_delay_ticks > 0 {
        weapon.salvo_delay_ticks.max(1) as f32
    } else {
        weapon.reload_ticks.max(1) as f32
    };
    interval / weapon.rounds.max(1) as f32
}

/// Ticks of flight over which a round eases from the gun's drawn place back onto its line.
const LAUNCH_EASE: f32 = 3.0;

/// How far off its line a round is drawn `flight` ticks after it left the gun, for a
/// gun drawn `shift` metres off the line as it fired: a unit is drawn a tick behind
/// the sim, and a stream's later rounds leave from wherever the gun has got to by
/// then. Held through the first tick, so the round leaves the muzzle on a straight
/// line, then eased out, so it still lands where its shot does.
pub fn launch_shift(shift: [f32; 3], flight: f32) -> [f32; 3] {
    let f = (1.0 - (flight - 1.0) / LAUNCH_EASE).clamp(0.0, 1.0);
    shift.map(|s| s * f)
}

/// Where round `k` of a stream gun's shot is drawn leaving from, off the shot's own
/// muzzle, for a gun travelling `travel` metres a tick: a tick behind (the unit is drawn
/// gliding up to its place), then on as far as the gun got in the rounds' gap.
pub fn round_shift(travel: [f32; 3], k: u8, gap: f32) -> [f32; 3] {
    travel.map(|t| t * (k as f32 * gap - 1.0))
}

/// The rounds of a stream gun's shot still in the air after it landed. Round `k`
/// left the muzzle `k` gaps after the shot, on a line of its own, and lands as far
/// along it as the shot did. Cosmetic. Not state.
#[derive(Clone, Copy, Debug)]
pub struct StreamTail {
    /// Where the shot left the muzzle, and its travel per tick.
    pub origin: [f32; 3],
    pub vel: [f32; 3],
    /// Ticks since the shot was fired, at the end of this tick.
    pub age: f32,
    /// Ticks of flight at which the shot landed.
    pub lands: f32,
    /// Ticks after the shot that its last round left.
    pub last: f32,
    /// The shot struck something, so its rounds burst where they land; one that ran
    /// out of range does not.
    pub hit: bool,
    /// The firing unit's travel per tick as the shot left (`World::unit_travel`).
    pub travel: [f32; 3],
    pub blueprint: BlueprintId,
    pub weapon: u8,
}

impl StreamTail {
    /// For shot `i`, moved this tick, that ended `lands` ticks into its flight.
    pub(crate) fn of(
        p: &crate::tables::Projectiles,
        i: usize,
        weapon: &mc_data::Weapon,
        lands: f32,
        hit: bool,
        travel: [f32; 3],
    ) -> Self {
        let age = p.age[i] as f32;
        let vel = p.vel[i].to_f32();
        let prev = p.prev_pos[i].to_f32();
        let origin = std::array::from_fn(|a| prev[a] - vel[a] * (age - 1.0));
        StreamTail {
            origin,
            vel,
            age,
            lands,
            last: (weapon.rounds.max(1) - 1) as f32 * round_gap(weapon),
            hit,
            travel,
            blueprint: p.blueprint[i],
            weapon: p.weapon[i],
        }
    }
}

/// A shot that hit something this tick. It has already left the projectile
/// table, but its last stretch still has to be drawn. Not state.
#[derive(Clone, Copy, Debug)]
pub struct SpentShot {
    pub cold: bool,
    /// A cluster's sub-shot (`Projectiles::sub`): drawn smaller than the whole shot.
    pub sub: bool,
    pub from: FxVec3,
    pub to: FxVec3,
    /// Share of the tick the last stretch took, zero to one.
    pub after: mc_core::Fx,
    /// Where `from` is drawn off by, to leave from the gun as it is drawn (`launch_shift`).
    pub lead: [f32; 3],
    pub blueprint: BlueprintId,
    pub weapon: u8,
    /// Whose it was: a torpedo looks as its side's faction's do (`torpedo_look`).
    pub owner: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct ProjectileInstance {
    pub prev_pos: [f32; 3],
    /// Tracer colour (0 blue, 1 orange), plus `PROJECTILE_MISSILE` for a missile.
    pub color: u32,
    pub pos: [f32; 3],
    pub size: f32,
    /// Seconds the energy wake lasts. Zero: the usual hang, or none. A torpedo's is
    /// its look instead (`mc_data::TorpedoLook` as a number, `torpedo_look`).
    pub wake: f32,
    /// Metres of blue plasma around the traveling slug. Zero: none.
    pub plasma: f32,
    /// One: a small-calibre tracer, drawn deep orange (a stream gun's rounds); up to two, redder;
    /// plus twice a Regency plasma shot's look (`plasma_look`). Then how far a cruise missile's
    /// wings are out, 0 to 1 (`cruise_wings`), or below zero, a missile turning over on its
    /// thrusters (`wings_or_turn`).
    pub _pad: [f32; 2],
    /// Nose this tick, xyz. Zero: the body follows travel (`pos - prev_pos`).
    /// Then a missile's body across in metres (`Weapon::caliber`); zero: from `size`. Any
    /// other shot's tail-length multiplier instead (`Weapon::streak`, `aim_w`).
    pub aim: [f32; 4],
    /// Nose last tick. A cold body blends from this to `aim` across the frame.
    pub prev_aim: [f32; 4],
}

const _: () = assert!(std::mem::size_of::<ProjectileInstance>() == 80);

/// How a Regency plasma shot is drawn in flight (sprites.wgsl, `ProjectileInstance::_pad[0]`
/// above 2): 1 a Pinched-plasmeric stream slug, 2 a Pinch-fusion slug strobing with fusion
/// bursts, 3 a gravitic charge thrown curving onto its mark (`Weapon::curve`), 4 a
/// Plasmeric bolt (a fat glowing teardrop, as a plasma repeater throws), 5 a seeker, any
/// plasma `missile` (`plasma_seeker`, `PLASMA_LOOK_GRAVITIC_SEEKER`): its contained charge
/// in its lens, violet, so every Regency missile reads as one a missile defence can take.
/// Zero for anything else.
pub fn plasma_look(weapon: &mc_data::Weapon) -> u32 {
    use mc_data::PlasmaGrade;
    match weapon.plasma_shot() {
        _ if plasma_seeker(weapon) => PLASMA_LOOK_GRAVITIC_SEEKER,
        Some(_) if weapon.curve.0 > 0 => 3,
        Some(PlasmaGrade::Pinched) => 1,
        Some(PlasmaGrade::PinchFusion) => 2,
        Some(PlasmaGrade::Plasmeric) => 4,
        Some(PlasmaGrade::Gravitic) | None => 0,
    }
}

/// How far a cruise missile's wings have unfolded, 0 to 1: a sea skimmer out of a
/// vertical-launch cell flies up it with them folded and opens them as it turns over
/// (`naval_arms::POP_BOOST`). Every other missile: none.
fn cruise_wings(weapon: &mc_data::Weapon, age: u16) -> f32 {
    if !weapon.missile || weapon.skim <= Fx::ZERO || !weapon.vertical_launch {
        return 0.0;
    }
    (age.saturating_sub(crate::naval_arms::POP_BOOST) as f32 / 5.0).min(1.0)
}

/// How a cluster's sub-shot is drawn against its whole shot.
fn sub_scale(sub: bool) -> f32 {
    if sub {
        0.55
    } else {
        1.0
    }
}

/// `ProjectileInstance::_pad[1]`: a missile's wings (`cruise_wings`), or, for one coasting
/// after its booster while its thrusters turn it over (`Weapon::boost_ticks`), minus how far
/// through the turn it is (just under zero to minus one).
fn wings_or_turn(weapon: &mc_data::Weapon, age: u16) -> f32 {
    if weapon.boost_ticks == 0 || !weapon.motor_out(age) {
        return cruise_wings(weapon, age);
    }
    let span = weapon
        .cold_launch_ticks
        .saturating_sub(weapon.boost_ticks)
        .max(1);
    -((age - weapon.boost_ticks) as f32 / span as f32).clamp(0.01, 1.0)
}

/// A Regency seeker: a plasma `missile` of any grade, a charge held in gravity
/// containment and steered onto its mark. It has no body and no motor, so it is not drawn
/// as a missile (`PROJECTILE_MISSILE`) but as the charge it is (`plasma_look`).
fn plasma_seeker(weapon: &mc_data::Weapon) -> bool {
    weapon.missile && weapon.plasma_grade.is_some()
}

/// A shot's `aim.w`: a missile's body across (`Weapon::caliber`), any other shot's
/// tail-length multiplier (`Weapon::streak`).
fn aim_w(weapon: &mc_data::Weapon) -> f32 {
    if !plasma_seeker(weapon) && weapon.missile {
        weapon.caliber
    } else {
        weapon.streak
    }
}

fn nose_pad(cold: bool, aim: FxVec3, caliber: f32) -> [f32; 4] {
    if !cold {
        return [0.0, 0.0, 0.0, caliber];
    }
    let a = aim.to_f32();
    [a[0], a[1], a[2], caliber]
}

/// A projected dome, for the shield pass. Written while it is visible
/// (opening, up, or closing), and while a shattered generator is filling so
/// the selection panel can show charge.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct ShieldInstance {
    pub pos: [f32; 3],
    pub radius: f32,
    pub prev_open: f32,
    pub open: f32,
    /// Hit-point share, zero to one.
    pub health: f32,
    /// `owner | team << 8 | tech << 16 | collapsing << 24 | hull << 25 | stalling << 26 |
    /// veil << 27 | look << 28` (`SHIELD_VEIL`, `SHIELD_LOOK_SHIFT`).
    /// Collapsing is a shattered dome filling while it peels — not low health.
    /// Stalling is a dry grid: the projector goes dark with the bubble.
    pub packed: u32,
    pub unit_id: u32,
    /// Emitter height above `pos.z`: inside the crystal, so the launch beam
    /// is born in the needle instead of sitting on the tip. Zero on a hull
    /// field: there is no projector.
    pub projector: f32,
    /// Unit height, metres. The hull wrap uses this as the vertical axis.
    pub height: f32,
    /// `radius` last tick, for render interpolation while an upgraded dome swells
    /// out. Zero: the same as `radius`.
    pub prev_radius: f32,
}

const _: () = assert!(std::mem::size_of::<ShieldInstance>() == 48);

/// `ShieldInstance::packed` bit: a veil, the Replication Engine's unbreakable dome
/// (the unit is `flag::INVULNERABLE`). Drawn apart from every other shield: its own
/// lattice, hits that slide off, never a break, and it fuses with nothing.
pub const SHIELD_VEIL: u32 = 1 << 27;

/// `ShieldInstance::packed`: the field's look (`mc_data::ShieldLook` of the unit's faction)
/// sits in the two bits from here (`mc_models::gpu_consts::shield_look`).
pub const SHIELD_LOOK_SHIFT: u32 = 28;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct StainInstance {
    pub pos: [f32; 2],
    pub radius: f32,
    /// `strength | seed << 8`.
    pub strength_seed: u32,
}

/// An incendiary patch. `elapsed` and `duration` are seconds.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct FireInstance {
    pub pos: [f32; 3],
    pub radius: f32,
    pub elapsed: f32,
    pub duration: f32,
}

/// A builder at work this tick, for the game's building sounds.
#[derive(Clone, Copy, Debug)]
pub struct BuildSource {
    pub unit: u32,
    /// Where its work is heard from.
    pub at: [f32; 3],
    /// The builder's faction (`mc_data::FactionId`): whose building sounds it makes.
    pub faction: u8,
}

/// A shot in flight whose weapon has a sound while it flies (`WeaponSounds::flight`,
/// a missile's motor), for the game's audio.
#[derive(Clone, Copy, Debug)]
pub struct FlightSource {
    pub pos: [f32; 3],
    pub blueprint: u16,
    pub weapon: u8,
}

/// One tick's worth of presentation data.
#[derive(Clone, Default)]
pub struct RenderFrame {
    pub tick: u32,
    /// Units first, then wrecks (flagged `KIND_WRECK`).
    pub units: Vec<UnitInstance>,
    pub projectiles: Vec<ProjectileInstance>,
    /// The shots in `projectiles` that are heard as they fly.
    pub flights: Vec<FlightSource>,
    /// Reclaim and repair beams at work this tick (`BeamInstance::kind`).
    pub beams: Vec<crate::reclaim::BeamInstance>,
    /// The unit each of `beams` comes from, so the game can tell a beam starting from one carrying on.
    pub beam_sources: Vec<u32>,
    /// Survival: how awake the Precursor facility is, 0.15..1; zero in any other match.
    pub precursor_activity: f32,
    /// Mobile builders whose construction beam is on this tick, and where it meets the work.
    pub build_sources: Vec<BuildSource>,
    /// Wave origins on construction sites, packed; a unit's `weld_first` /
    /// `weld_count` index this. Includes origins whose beam has just gone off.
    pub welds: Vec<ConstructionWeld>,
    pub shields: Vec<ShieldInstance>,
    pub stains: Vec<StainInstance>,
    pub fires: Vec<FireInstance>,
    /// Poured structure lots, including those whose building is already gone.
    pub pads: Vec<StainInstance>,
    pub events: Vec<SimEvent>,
    /// Fog for the viewer, two bytes per 64 m cell: visible now, explored. Empty when fog is off.
    pub fog: Vec<u8>,
    pub fog_dims: (u32, u32),
    /// Whose eyes `fog` is: the viewer's side as a player mask, 0 with fog off.
    pub fog_mask: u32,
    /// The vision discs that side sees through this tick, to draw round.
    pub vision: Vec<VisionDisc>,
    /// One bit per map prop, set when destroyed.
    pub props_dead: Vec<u32>,
    /// The whole terrain edit table, in order (copied again only when it changed).
    pub terrain_edits: Vec<mc_map::FlattenRecord>,
    /// The faction each of `terrain_edits` was levelled for (`TerrainEdit::faction`).
    pub terrain_edit_factions: Vec<u8>,
    /// Every weapon's pose for units with gun houses of their own (`Weapon::mount`):
    /// `UnitInstance::status[1]` bits 8.. hold the index here plus one.
    pub houses: Vec<HousePose>,
    /// Strategic missiles in flight, every side's: warheads and interceptors (`nukes.rs`).
    pub strategic: Vec<StrategicInstance>,
    /// Every warhead in flight with the whole path it flies, for the interface.
    pub warhead_tracks: Vec<crate::nukes::WarheadTrack>,
    /// Launches ordered and not yet away, the viewer's side only (everyone's with no viewer).
    pub planned_launches: Vec<crate::nukes::PlannedLaunch>,
    /// Capital ships in a jump that the viewer may see (`crate::warp`).
    pub warps: Vec<WarpView>,
    /// Timed self-destructs counting down that the viewer may see (`crate::destruct`).
    pub destructs: Vec<DestructView>,
    /// Warp dampeners the viewer knows of, and their fields.
    pub dampers: Vec<DamperView>,
    /// Each side's stores as the storage structures' lamps last judged them.
    pub stores: crate::store_lights::StoreWatch,
}

/// Each unit's instance by its id (wrecks, props and ghosts left out), in id order for
/// `drawn_at`.
fn drawn_units(units: &[UnitInstance]) -> Vec<(u32, u32)> {
    let mut drawn: Vec<(u32, u32)> = units
        .iter()
        .enumerate()
        .filter(|(_, u)| u.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST) == 0)
        .map(|(i, u)| (u.unit_id, i as u32))
        .collect();
    drawn.sort_unstable();
    drawn
}

/// Where unit `id`'s instance is, from `drawn_units` (its first, should it have two).
fn drawn_at(drawn: &[(u32, u32)], id: u32) -> Option<u32> {
    let k = drawn.partition_point(|d| d.0 < id);
    drawn.get(k).filter(|d| d.0 == id).map(|d| d.1)
}

/// A capital ship in a jump (`crate::warp`), for the effects, the sound and the interface.
#[derive(Clone, Copy, Debug)]
pub struct WarpView {
    pub unit_id: u32,
    pub owner: u8,
    pub blueprint: BlueprintId,
    pub phase: crate::tables::WarpPhase,
    /// Ticks into this phase, and how many it lasts. A spool can run past its length
    /// while the nose comes round onto the mark; a transit snagged part way grows.
    pub ticks: u16,
    pub length: u16,
    /// Where it jumps from and comes out, at cruise height over each.
    pub from: [f32; 3],
    pub to: [f32; 3],
    /// Which way the jump runs, radians (as `UnitInstance::heading`).
    pub bearing: f32,
    /// The hull's radius, metres.
    pub radius: f32,
    /// A live enemy dampener has the jump: slow, torn, and it comes out hurt and stunned.
    pub dampened: bool,
    /// Spooling: how far the drive is charged, 0 to 1 (1 from the jump on).
    pub charge: f32,
    /// Spooling: its nose is on the mark. Until it is, the charge stops short of the
    /// last third (`crate::warp`).
    pub aligned: bool,
    /// Energy this jump's whole charge takes: its drive's price a kilometre times how far
    /// it goes (`mc_data::Warp::charge`), fixed when it began to spool.
    pub energy: f32,
    /// Spooling: energy the grid is asked for each second while it charges at full power
    /// (0 otherwise).
    pub draw: f32,
}

/// A warp dampener and its field.
#[derive(Clone, Copy, Debug)]
pub struct DamperView {
    pub unit_id: u32,
    pub owner: u8,
    pub pos: [f32; 3],
    /// How far its field reaches, metres.
    pub radius: f32,
    /// Finished and powered: its field is up.
    pub live: bool,
}

/// A warhead or an interceptor in flight. Seen by everyone, fog or no fog.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug, Default)]
pub struct StrategicInstance {
    pub prev_pos: [f32; 3],
    /// `STRATEGIC_WARHEAD` or `STRATEGIC_INTERCEPTOR`.
    pub kind: u32,
    pub pos: [f32; 3],
    pub owner: u32,
    /// A warhead's burst point; an interceptor's quarry's position.
    pub mark: [f32; 3],
    /// Seconds since launch.
    pub age: f32,
    pub serial: u32,
    /// A warhead's seconds left to the burst, if nothing stops it; zero for an interceptor.
    pub eta: f32,
    /// A warhead: 1 while still on its boost out of the tube.
    pub boost: f32,
    pub quarry: u32,
    /// How big the missile is drawn against the Sunfall's (`Strategic::missile_scale`).
    pub scale: f32,
    /// `StrategicLook` as its number: how the missile and its trail are drawn.
    pub look: u32,
}

const _: () = assert!(std::mem::size_of::<StrategicInstance>() == 72);

pub const STRATEGIC_WARHEAD: u32 = 0;
pub const STRATEGIC_INTERCEPTOR: u32 = 1;

/// The pose of each weapon on a unit whose guns turn on houses of their own, for the
/// entity shader's `rig::HOUSE` limbs: per weapon its yaw off the hull last tick and
/// this, then its pitch last tick and this (radians); then each weapon's kick-back
/// last tick and this (1 the instant it fires), two per weapon.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug, Default)]
pub struct HousePose {
    pub pose: [[f32; 4]; mc_data::MAX_HOUSES],
    pub kick: [f32; 2 * mc_data::MAX_HOUSES],
}

const _: () = assert!(std::mem::size_of::<HousePose>() == 192);

/// `UnitInstance::status[1]`: bits 8.. hold the unit's index into `RenderFrame::houses` plus one.
pub const UNIT_HOUSE_SHIFT: u32 = 8;
/// `UnitInstance::status[1]` bits 2..5: one more than the weapon of a twin gun on the arm
/// (`twin_arm_gun`), zero for none. Its tube kicks on its own shots, read from the unit's
/// `HousePose`; the main gun's kick stays in `recoil`.
pub const UNIT_TWIN_SHIFT: u32 = 2;
pub const UNIT_TWIN_MASK: u32 = 0x7;
/// `UnitInstance::status[1]`: the twin gun is on the right (-y) of the arm, the main gun left.
pub const UNIT_TWIN_RIGHT: u32 = 1 << 5;

/// A second gun on the first weapon's elbow on the other side of it from the arm's main
/// gun (`main`), so each arm kicks on its own shots: its weapon and whether it is on the
/// right (-y). The heaviest such gun; none for a gun on the centreline or past the weapons
/// a `HousePose` holds.
fn twin_arm_gun(weapons: &[mc_data::Weapon], main: usize) -> Option<(usize, bool)> {
    let arm = weapons.first()?.pivot?;
    let side = weapons.get(main)?.muzzle.y.signum();
    if side == 0 {
        return None;
    }
    (0..weapons
        .len()
        .min(mc_data::MAX_HOUSES)
        .min(UNIT_TWIN_MASK as usize))
        .filter(|&w| {
            let g = &weapons[w];
            w != main && g.pivot == Some(arm) && !g.mount && g.muzzle.y.signum() == -side
        })
        .max_by_key(|&w| (weapons[w].damage, std::cmp::Reverse(w)))
        .map(|w| (w, side > 0))
}

impl World {
    /// How far unit `id` moved this tick, metres; nothing for one that is gone. A jump
    /// (a spawn, a teleport) is not travel.
    pub(crate) fn unit_travel(&self, id: UnitId) -> [f32; 3] {
        let u = &self.state.units;
        let Some(row) = u.row(id) else {
            return [0.0; 3];
        };
        let step = (u.pos[row] - u.prev_pos[row])
            .extend(u.z[row] - u.prev_z[row])
            .to_f32();
        if step.iter().map(|v| v * v).sum::<f32>() > 30.0 * 30.0 {
            return [0.0; 3];
        }
        step
    }

    /// Ticks from packed to fully out of what `units.deploy` counts on this unit:
    /// a siege gun's spade, a builder's folding gear, or a cell launcher's hatches. Zero:
    /// none of them.
    pub(crate) fn deploy_span(&self, row: usize) -> u16 {
        let bp = self.bp(row);
        if let Some(s) = bp.strategic.as_ref() {
            // A strategic launcher's blast doors (`nukes.rs`).
            return match s.kind {
                mc_data::strategic::StrategicKind::Nuke => crate::nukes::SILO_DOOR_TICKS,
                mc_data::strategic::StrategicKind::Interceptor => crate::nukes::ARRAY_DOOR_TICKS,
            };
        }
        if let Some(hatch) = crate::launch_cells::hatch_ticks(bp) {
            return hatch;
        }
        match bp.motion.map_or(0, |m| m.deploy_ticks) {
            0 => bp.builder.as_ref().map_or(0, |b| b.unfold_ticks),
            n => n,
        }
    }

    /// Where the beams of a builder's extra emitters leave (`Builder::emitters`), once its
    /// folding gear is out. They turn with the turret.
    pub(crate) fn builder_extra_emitters(&self, row: usize) -> Vec<FxVec3> {
        let s = &self.state;
        let bp = self.bp(row);
        let Some(b) = bp.builder.as_ref() else {
            return Vec::new();
        };
        if b.emitters.is_empty() || s.units.deploy[row] < b.unfold_ticks {
            return Vec::new();
        }
        let facing = s.units.heading[row] + s.units.weapon_yaw[row][0];
        // Out, the gear points at the work the way the arm does.
        let pitch = s.units.arm_pitch[row][1];
        b.emitters
            .iter()
            .map(|&e| {
                let e = crate::world::pitched(e, b.hinge, pitch);
                (s.units.pos[row]
                    + bp.turret_point(mc_core::FxVec2::new(e.x, e.y), s.units.heading[row], facing))
                .extend(s.units.z[row] + e.z)
            })
            .collect()
    }

    /// How a torpedo of `owner`'s looks running, as `ProjectileInstance::wake` carries it:
    /// the weapon's own look, else an interceptor's sprint, else its side's faction's.
    fn torpedo_look(&self, weapon: &mc_data::Weapon, owner: u8) -> f32 {
        let look = weapon.torpedo_look.unwrap_or_else(|| {
            if weapon.intercepts {
                mc_data::TorpedoLook::Sprint
            } else {
                self.state
                    .players
                    .get(owner as usize)
                    .and_then(|p| self.blueprints.factions.get(p.faction as usize))
                    .map_or(mc_data::TorpedoLook::Bubbles, |f| f.torpedo_look)
            }
        });
        f32::from(look as u8)
    }

    /// How unit `row`'s shield field is drawn: its faction's look.
    fn shield_look(&self, row: usize) -> mc_data::ShieldLook {
        self.blueprints
            .factions
            .get(self.bp(row).faction.0 as usize)
            .map_or(mc_data::ShieldLook::Honeycomb, |f| f.shield_look)
    }

    /// Where a construction beam leaves this builder.
    /// Whether `faction` builds with nanites (`mc_data::Construction::Nanite`).
    fn faction_uses_nanites(&self, faction: mc_data::FactionId) -> bool {
        self.blueprints
            .factions
            .get(faction.0 as usize)
            .is_some_and(|f| f.construction == mc_data::Construction::Nanite)
    }

    /// Whether builder `row` is of a faction that builds with nanites: it feeds its work
    /// with a nanite stream, not a print beam (and takes things apart with one).
    pub(crate) fn uses_nanites(&self, row: usize) -> bool {
        self.faction_uses_nanites(self.bp(row).faction)
    }

    fn build_source(&self, row: usize, at: [f32; 3]) -> BuildSource {
        BuildSource {
            unit: self.state.units.id(row).0,
            at,
            faction: self.bp(row).faction.0,
        }
    }

    pub(crate) fn builder_emitter(&self, row: usize) -> FxVec3 {
        let bp = self.bp(row);
        let s = &self.state;
        match bp.builder.as_ref().and_then(|b| b.arm) {
            Some(arm) => {
                let facing = s.units.heading[row] + s.units.weapon_yaw[row][0];
                let at = crate::world::pose_build_arm(
                    arm,
                    s.units.arm_pitch[row][0],
                    s.units.arm_pitch[row][1],
                );
                (s.units.pos[row]
                    + bp.turret_point(
                        mc_core::FxVec2::new(at.x, at.y),
                        s.units.heading[row],
                        facing,
                    ))
                .extend(s.units.z[row] + at.z)
            }
            None => s.units.pos[row].extend(s.units.z[row] + bp.height),
        }
    }

    /// Fabricator tips on a factory, in world space: the heads its tier has
    /// fitted, from the table the factory meshes stand them on.
    fn factory_print_heads(&self, row: usize) -> Vec<FxVec3> {
        let bp = self.bp(row);
        let Some(factory) = crate::print_heads::factory_heads(&bp.visual.mesh) else {
            return Vec::new();
        };
        let heading = self.state.units.heading[row];
        let pos = self.state.units.pos[row];
        let z = self.state.units.z[row];
        factory
            .heads
            .iter()
            .filter(|h| h.fitted(bp.tech))
            .map(|h| {
                let [nx, ny, nz] = crate::print_heads::nozzle(h, factory.aim);
                (pos + mc_core::FxVec2::new(Fx::from_f32(nx), Fx::from_f32(ny)).rotate(heading))
                    .extend(z + Fx::from_f32(nz))
            })
            .collect()
    }

    /// The point on `target` a beam from `from` prints at: where it meets the
    /// work, in the world and in the work's model space.
    fn weld_on(&self, target: usize, from: [f32; 3]) -> ([f32; 3], [f32; 3]) {
        let s = &self.state;
        let bp = self.bp(target);
        let heading = s.units.heading[target].to_radians_f32();
        let (sin, cos) = heading.sin_cos();
        let origin = s.units.pos[target].extend(s.units.z[target]).to_f32();
        let (r, h) = (bp.radius.to_f32(), bp.height.to_f32());
        let center = [origin[0], origin[1], origin[2] + h * 0.5];
        let dir = [
            center[0] - from[0],
            center[1] - from[1],
            center[2] - from[2],
        ];
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2])
            .sqrt()
            .max(0.001);
        let inset = r * 0.82;
        let hit = [
            center[0] - dir[0] / len * inset,
            center[1] - dir[1] / len * inset,
            (center[2] - dir[2] / len * inset).clamp(origin[2] + h * 0.15, origin[2] + h * 0.8),
        ];
        let off = [hit[0] - origin[0], hit[1] - origin[1], hit[2] - origin[2]];
        (
            hit,
            [
                off[0] * cos + off[1] * sin,
                -off[0] * sin + off[1] * cos,
                off[2],
            ],
        )
    }

    /// Work a mobile builder's beam can print on: an open site, an upgrade
    /// assembling inside the unit it replaces, or the factory this builder is
    /// feeding. A hull in the bay stays hidden, so the beam hits the foundry.
    fn print_target(&self, row: usize) -> Option<usize> {
        use crate::tables::flag;
        let t = self.state.units.row(self.state.units.build_target[row])?;
        if self.state.units.has_flag(t, flag::UPGRADE)
            || !self.state.units.has_flag(t, flag::IN_FACTORY)
        {
            return Some(t);
        }
        self.factory_printing(t)
    }

    /// The factory whose current product is `product`.
    fn factory_printing(&self, product: usize) -> Option<usize> {
        let id = self.state.units.id(product);
        self.state.units.slots.iter().find(|&row| {
            self.bp(row).has(mc_data::cat::FACTORY) && self.state.units.build_target[row] == id
        })
    }

    /// The hidden successor a structure is assembling in place, when there is one.
    /// Factories, extractors, intel towers and shield generators are refitted
    /// like a mobile unit: the next kit is built onto them.
    fn structure_upgrade(&self, row: usize) -> Option<usize> {
        use crate::tables::flag;
        if self.upgrades_in_place(row) {
            return None;
        }
        self.state
            .units
            .row(self.state.units.build_target[row])
            .filter(|&t| {
                self.state.units.has_flag(t, flag::UPGRADE)
                    && self.state.units.has_flag(t, flag::UNDER_CONSTRUCTION)
            })
    }

    /// Each open site's print origins: one per mobile builder after nearby
    /// ones are clustered, so thirty engineers around a factory still read as
    /// a ring of waves rather than a wall of light. A structure upgrading
    /// itself, with nobody on it, prints from the pad.
    fn construction_welds(&self) -> HashMap<u32, Vec<[f32; 3]>> {
        use crate::tables::flag;
        let s = &self.state;
        let mut points: HashMap<u32, Vec<[f32; 3]>> = HashMap::new();
        let mut radius: HashMap<u32, f32> = HashMap::new();
        for row in s.units.slots.iter() {
            if s.units.flags[row] & flag::BUILDING == 0
                || s.units.flags[row] & flag::REPAIRING != 0
                || !self.bp(row).is_mobile()
            {
                continue;
            }
            let Some(t) = self.print_target(row) else {
                continue;
            };
            let from = self.builder_emitter(row).to_f32();
            let (_, local) = self.weld_on(t, from);
            let id = s.units.id(t).0;
            points.entry(id).or_default().push(local);
            for extra in self.builder_extra_emitters(row) {
                let (_, local) = self.weld_on(t, extra.to_f32());
                points.entry(id).or_default().push(local);
            }
            radius
                .entry(id)
                .or_insert_with(|| self.bp(t).radius.to_f32());
        }
        for row in s.units.slots.iter() {
            if s.units.flags[row] & flag::BUILDING == 0 || self.bp(row).is_mobile() {
                continue;
            }
            let Some(t) = s.units.row(s.units.build_target[row]) else {
                continue;
            };
            let id = s.units.id(t).0;
            if points.contains_key(&id) {
                continue;
            }
            if self.bp(row).has(mc_data::cat::FACTORY) && !s.units.has_flag(t, flag::UPGRADE) {
                for head in self.factory_print_heads(row) {
                    let (_, local) = self.weld_on(t, head.to_f32());
                    points.entry(id).or_default().push(local);
                }
                radius
                    .entry(id)
                    .or_insert_with(|| self.bp(t).radius.to_f32());
                continue;
            }
            let h = self.bp(t).height.to_f32();
            points.insert(id, vec![[0.0, 0.0, h * 0.2]]);
            radius.insert(id, self.bp(t).radius.to_f32());
        }
        points
            .into_iter()
            .map(|(id, locals)| {
                let r = radius.get(&id).copied().unwrap_or(8.0);
                (id, cluster_print_origins(&locals, r))
            })
            .collect()
    }

    /// Powered kit is dark: a radar dish, or a shield crystal, while the grid
    /// cannot pay, and anything the player has powered down (paused). Radar
    /// keeps the same rule it always had (including a tower
    /// that is still going up). A dome only goes dark once it is finished; a
    /// hull wrap's stall shows on its bubble alone, not on the unit's glow.
    fn kit_unpowered(&self, row: usize) -> bool {
        let bp = self.bp(row);
        if bp.radar > Fx::ZERO && self.live_radar(row) <= Fx::ZERO {
            return true;
        }
        if self.powered_down(row) {
            return true;
        }
        bp.shield.is_some_and(|s| s.is_dome())
            && self.state.units.is_active(row)
            && self.shield_off(row)
    }

    /// A shattered dome filling while down. A stall pauses the fill, so the
    /// crystal goes dark (`kit_unpowered`) instead of pulsing.
    fn kit_charging(&self, row: usize) -> bool {
        self.bp(row).shield.is_some_and(|s| s.is_dome())
            && self.state.units.is_active(row)
            && self.state.units.shield_recharge[row] > 0
            && !self.shield_off(row)
    }

    /// Fills `frame` with this tick's mirror, reusing its allocations. Units the
    /// viewer cannot detect are left out, so fog is enforced before the GPU.
    /// `viewer = None` shows everything (observers, replays, tools).
    pub fn write_render_frame(&self, viewer: Option<u8>, frame: &mut RenderFrame) {
        let s = &self.state;
        let prev_tick = frame.tick;
        let prev_welds = std::mem::take(&mut frame.welds);
        frame.tick = s.tick;
        frame.units.clear();
        frame.houses.clear();
        let live = self.construction_welds();
        let decay = if s.tick > prev_tick {
            (s.tick - prev_tick) as f32 / (TICKS_PER_SECOND as f32 * WELD_LINGER_SECONDS)
        } else {
            0.0
        };
        let weld_range = pack_construction_welds(
            &mut frame.welds,
            fade_construction_welds(prev_welds, &live, decay),
        );
        let decks = self.lift_decks();
        let pass = units::UnitPass {
            viewer,
            decks: &decks,
            weld_range: &weld_range,
            welds: &frame.welds,
        };
        let (mut instances, mut houses) = (Vec::new(), Vec::new());
        self.unit_instances(&pass, &mut instances, &mut houses);
        frame.units.append(&mut instances);
        frame.houses.append(&mut houses);

        // A drone docked on an aircraft rides it as it is drawn, heave, sway and lean
        // included (`entity.wgsl` `riding_frame`): it names its carrier's instance.
        let riders: Vec<(usize, u32)> = frame
            .units
            .iter()
            .enumerate()
            .filter(|(_, u)| u.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST) == 0)
            .filter_map(|(i, u)| {
                let row = s.units.row(crate::Handle(u.unit_id))?;
                Some((i, s.units.id(self.drone_riding(row)?).0))
            })
            .collect();
        if !riders.is_empty() {
            let drawn = drawn_units(&frame.units);
            for (i, carrier) in riders {
                if let Some(at) = drawn_at(&drawn, carrier) {
                    frame.units[i].status[0] |= UNIT_RIDING;
                    frame.units[i].status[2] = at + 1;
                }
            }
        }
        // Storage structures show their side's store (`store_lights`).
        self.write_store_lights(viewer, frame);

        frame.projectiles.clear();
        frame.flights.clear();
        let look = |blueprint: BlueprintId, weapon: u8| {
            let unit = self.blueprints.unit(blueprint);
            let weapon = &unit.weapons[weapon as usize];
            let bomb = unit
                .motion
                .is_some_and(|m| m.layer == mc_data::MoveLayer::Air)
                && weapon.trajectory == Trajectory::Ballistic
                && !weapon.missile;
            let trail = !weapon.missile
                && weapon.color == WeaponColor::Blue
                && (weapon.trail || weapon.trajectory == Trajectory::Ballistic);
            // A flak shell always leaves a thin smoke wake: it is slow enough to be followed.
            let smoke = !weapon.missile
                && weapon.color == WeaponColor::Orange
                && (weapon.trail || weapon.flak);
            let power = weapon.damage.to_f32().sqrt();
            // A flak shell is a small hot round; its splash is the burst, not the tracer.
            let size = if weapon.flak {
                (0.45 + power * 0.03) * weapon.tracer
            } else {
                (0.3 + power * 0.045 + weapon.splash.to_f32() * 0.07) * weapon.tracer
            };
            let plasma = if weapon.plasma > 0.0 {
                (1.35 + power * 0.11 + weapon.splash.to_f32() * 0.04) * weapon.plasma
            } else {
                0.0
            };
            // A Regency seeker is drawn as its charge, not as a missile (`plasma_look`).
            let missile = weapon.missile && !plasma_seeker(weapon);
            (
                weapon.color as u32
                    | if missile { PROJECTILE_MISSILE } else { 0 }
                    | if trail { PROJECTILE_TRAIL } else { 0 }
                    | if smoke { PROJECTILE_SMOKE } else { 0 }
                    | if bomb { PROJECTILE_BOMB } else { 0 }
                    | if weapon.torpedo {
                        PROJECTILE_TORPEDO
                    } else {
                        0
                    }
                    | if missile && weapon.skim > Fx::ZERO {
                        PROJECTILE_SKIM
                    } else {
                        0
                    }
                    | if missile && weapon.apogee > Fx::ZERO {
                        PROJECTILE_APOGEE
                    } else {
                        0
                    }
                    | if weapon.rail { PROJECTILE_RAIL } else { 0 },
                if trail { size * 1.55 } else { size },
                if trail || smoke || weapon.missile {
                    weapon.wake
                } else {
                    0.0
                },
                plasma,
                // A stream gun's rounds are small and many: drawn a deep tracer orange, not
                // the white-hot of a shell. Above one, it leans on to red (`Weapon::red`).
                // A Regency plasma shot (`plasma_grade`) is drawn that way too, whatever its size,
                // and plus twice its look (`plasma_look`).
                if (weapon.rounds > 1 || weapon.plasma_shot().is_some() || weapon.gravitic_seeker())
                    && weapon.color == WeaponColor::Orange
                {
                    1.0 + weapon.red + 2.0 * plasma_look(weapon) as f32
                } else {
                    0.0
                },
            )
        };
        let muzzles: std::collections::HashSet<[i64; 3]> =
            self.muzzles.iter().map(|m| [m.x.0, m.y.0, m.z.0]).collect();
        let fresh = |from: FxVec3| {
            if muzzles.contains(&[from.x.0, from.y.0, from.z.0]) {
                PROJECTILE_FRESH
            } else {
                0
            }
        };
        let hitscan = |blueprint: BlueprintId, weapon: u8| {
            self.blueprints.unit(blueprint).weapons[weapon as usize].hitscan
        };
        for i in 0..s.projectiles.len() {
            // A hitscan weapon is a fading beam the renderer draws from the fire and
            // impact events, not a traveling slug.
            if hitscan(s.projectiles.blueprint[i], s.projectiles.weapon[i]) {
                continue;
            }
            let (color, size, wake, plasma, hot) =
                look(s.projectiles.blueprint[i], s.projectiles.weapon[i]);
            let size = size * sub_scale(s.projectiles.sub[i] > 0);
            let weapon = &self.blueprints.unit(s.projectiles.blueprint[i]).weapons
                [s.projectiles.weapon[i] as usize];
            let wake = if weapon.torpedo {
                self.torpedo_look(weapon, s.projectiles.owner[i])
            } else {
                wake
            };
            // Age has advanced after this step.
            let color = color
                | if weapon.motor_out(s.projectiles.age[i]) {
                    PROJECTILE_COLD
                } else {
                    0
                };
            let from = s.projectiles.prev_pos[i];
            let cold_body = color & PROJECTILE_COLD != 0;
            // Out of the gun where it is drawn, a tick behind the sim's muzzle.
            let back = self.unit_travel(s.projectiles.source[i]).map(|t| -t);
            let age = s.projectiles.age[i] as f32;
            let off = |at: FxVec3, flight: f32| {
                let (at, d) = (at.to_f32(), launch_shift(back, flight));
                std::array::from_fn(|a| at[a] + d[a])
            };
            let pos = off(s.projectiles.pos[i], age);
            if weapon.sounds.flight.is_some() {
                frame.flights.push(FlightSource {
                    pos,
                    blueprint: s.projectiles.blueprint[i].0,
                    weapon: s.projectiles.weapon[i],
                });
            }
            frame.projectiles.push(ProjectileInstance {
                prev_pos: off(from, age - 1.0),
                color: color | fresh(from),
                pos,
                size,
                wake,
                plasma,
                _pad: [hot, wings_or_turn(weapon, s.projectiles.age[i])],
                aim: nose_pad(cold_body, s.projectiles.aim[i], aim_w(weapon)),
                prev_aim: nose_pad(cold_body, s.projectiles.prev_aim[i], aim_w(weapon)),
            });
        }
        // Shots that landed this tick fly their last stretch, so a shell is seen
        // all the way in, and one fired at point-blank range is seen at all.
        for shot in &self.spent {
            if hitscan(shot.blueprint, shot.weapon) {
                continue;
            }
            let (color, size, wake, plasma, hot) = look(shot.blueprint, shot.weapon);
            let size = size * sub_scale(shot.sub);
            let shot_weapon = &self.blueprints.unit(shot.blueprint).weapons[shot.weapon as usize];
            let wake = if shot_weapon.torpedo {
                self.torpedo_look(shot_weapon, shot.owner)
            } else {
                wake
            };
            let ends = ((shot.after.to_f32() * 255.0) as u32).clamp(1, 255);
            let from = shot.from.to_f32();
            let caliber =
                aim_w(&self.blueprints.unit(shot.blueprint).weapons[shot.weapon as usize]);
            frame.projectiles.push(ProjectileInstance {
                prev_pos: std::array::from_fn(|a| from[a] + shot.lead[a]),
                color: color
                    | fresh(shot.from)
                    | ends << PROJECTILE_ENDS_SHIFT
                    | if shot.cold { PROJECTILE_COLD } else { 0 },
                pos: shot.to.to_f32(),
                size,
                wake,
                plasma,
                _pad: [
                    hot,
                    cruise_wings(
                        &self.blueprints.unit(shot.blueprint).weapons[shot.weapon as usize],
                        u16::MAX,
                    ),
                ],
                aim: [0.0, 0.0, 0.0, caliber],
                prev_aim: [0.0, 0.0, 0.0, caliber],
            });
        }

        // A stream gun's shot is seen as several rounds (`Weapon::rounds`). The sim's
        // own leads; the rest follow it out of the muzzle, a gap apart, each a little
        // off its line; those still in the air when it lands fly on as far again.
        let stream = |origin: [f32; 3],
                      vel: [f32; 3],
                      age: f32,
                      lands: f32,
                      hit: bool,
                      travel: [f32; 3],
                      blueprint: BlueprintId,
                      weapon: u8,
                      out: &mut Vec<ProjectileInstance>| {
            let w = &self.blueprints.unit(blueprint).weapons[weapon as usize];
            let (color, size, wake, plasma, hot) = look(blueprint, weapon);
            let gap = round_gap(w);
            let speed = (vel[0] * vel[0] + vel[1] * vel[1] + vel[2] * vel[2]).sqrt();
            if speed <= 0.0 {
                return;
            }
            let fwd = vel.map(|v| v / speed);
            // Across the line, level and then upward.
            let flat = (fwd[0] * fwd[0] + fwd[1] * fwd[1]).sqrt().max(1e-4);
            let side = [-fwd[1] / flat, fwd[0] / flat, 0.0];
            let up = [
                side[1] * fwd[2] - side[2] * fwd[1],
                side[2] * fwd[0] - side[0] * fwd[2],
                side[0] * fwd[1] - side[1] * fwd[0],
            ];
            let cone = (w.spread.max(40) as f32 * std::f32::consts::TAU / 65536.0).tan() * speed;
            let seed = origin[0].to_bits()
                ^ origin[1].to_bits().rotate_left(11)
                ^ origin[2].to_bits().rotate_left(22);
            for k in 1..w.rounds {
                let end = age - k as f32 * gap;
                let start = end - 1.0;
                if end <= 0.0 {
                    break;
                }
                if start >= lands {
                    continue;
                }
                let hash = |salt: u32| {
                    let mut x = seed ^ (k as u32).wrapping_mul(0x9E37_79B9) ^ salt;
                    x ^= x >> 16;
                    x = x.wrapping_mul(0x7FEB_352D);
                    x ^= x >> 15;
                    x = x.wrapping_mul(0x846C_A68B);
                    x ^= x >> 16;
                    x as f32 / u32::MAX as f32 * 2.0 - 1.0
                };
                // Evenly over a disc across the line, as the sim's own round is.
                let (r, turn) = (
                    ((hash(0x51) + 1.0) * 0.5).sqrt(),
                    hash(0xA7) * std::f32::consts::PI,
                );
                let (a, b) = (r * turn.cos(), r * turn.sin());
                let v: [f32; 3] =
                    std::array::from_fn(|i| vel[i] + (side[i] * a + up[i] * b) * cone);
                // Out of the gun where it has got to by the time this round leaves.
                let shift = round_shift(travel, k, gap);
                let at = |t: f32| -> [f32; 3] {
                    let d = launch_shift(shift, t.max(0.0));
                    std::array::from_fn(|i| origin[i] + v[i] * t + d[i])
                };
                let starts = if start < 0.0 {
                    ((-start * 255.0) as u32).clamp(1, 255) << PROJECTILE_STARTS_SHIFT
                        | PROJECTILE_FRESH
                } else {
                    0
                };
                let ends = if end > lands {
                    ((lands - start) * 255.0).round().clamp(1.0, 255.0) as u32
                } else {
                    0
                };
                // A jet pinched out (`Weapon::round_span`) thins to its tail as the pinch
                // lets go.
                let size = if w.round_span > 0.0 {
                    size * (1.0 - 0.45 * k as f32 / w.rounds as f32)
                } else {
                    size
                };
                out.push(ProjectileInstance {
                    prev_pos: at(start),
                    color: color | starts | ends << PROJECTILE_ENDS_SHIFT,
                    pos: at(end.min(lands)),
                    size,
                    wake,
                    plasma,
                    // `_pad[1]`: a round landing on what its shot hit, to burst there.
                    _pad: [hot, if hit && ends > 0 { 1.0 } else { 0.0 }],
                    aim: [0.0; 4],
                    prev_aim: [0.0; 4],
                });
            }
        };
        for i in 0..s.projectiles.len() {
            let (blueprint, weapon) = (s.projectiles.blueprint[i], s.projectiles.weapon[i]);
            if self.blueprints.unit(blueprint).weapons[weapon as usize].rounds < 2 {
                continue;
            }
            let age = s.projectiles.age[i] as f32;
            let vel = s.projectiles.vel[i].to_f32();
            let pos = s.projectiles.pos[i].to_f32();
            let origin = std::array::from_fn(|a| pos[a] - vel[a] * age);
            stream(
                origin,
                vel,
                age,
                f32::INFINITY,
                false,
                self.unit_travel(s.projectiles.source[i]),
                blueprint,
                weapon,
                &mut frame.projectiles,
            );
        }
        for t in &self.streams {
            stream(
                t.origin,
                t.vel,
                t.age,
                t.lands,
                t.hit,
                t.travel,
                t.blueprint,
                t.weapon,
                &mut frame.projectiles,
            );
        }

        // Construction beams: from each mobile builder at work to the weld it prints from,
        // including engineers helping an upgrade, or a factory printing in its bay.
        use crate::tables::flag;
        frame.build_sources.clear();
        let mut nanite_beams: Vec<(u32, BeamInstance)> = Vec::new();
        // The Regency sites those streams feed, by unit id.
        let mut nanite_sites: Vec<u32> = Vec::new();
        for row in s.units.slots.iter() {
            let flags = s.units.flags[row];
            if flags & flag::BUILDING == 0
                || flags & flag::REPAIRING != 0
                || !self.bp(row).is_mobile()
            {
                continue;
            }
            if viewer.is_some_and(|v| {
                self.are_enemies(v, s.units.owner[row]) && !self.detects_for_team(v, row)
            }) {
                continue;
            }
            // A refit of its own is assembled on it, not beamed onto it by its own arm
            // (it is still heard, below).
            let refitting = s.orders.front(&s.units, row).is_some_and(|o| {
                o.kind == crate::tables::OrderKind::Upgrade && self.upgrades_in_place(row)
            });
            if refitting {
                continue;
            }
            let Some(t) = self.print_target(row) else {
                continue;
            };
            let from = self.builder_emitter(row);
            let to = self.weld_on(t, from.to_f32()).0;
            frame.build_sources.push(self.build_source(row, to));
            if self.uses_nanites(row) {
                // A Regency site is fed by one nanite stream from the builder's emitter.
                nanite_sites.push(s.units.id(t).0);
                nanite_beams.push((
                    s.units.id(row).0,
                    BeamInstance {
                        from: from.to_f32(),
                        kind: BEAM_NANITE,
                        to_prev: to,
                        radius: self.bp(t).radius.to_f32(),
                        to,
                        height: 0.0,
                    },
                ));
                continue;
            }
            for from in std::iter::once(from).chain(self.builder_extra_emitters(row)) {
                frame.projectiles.push(ProjectileInstance {
                    prev_pos: from.to_f32(),
                    color: COLOR_BUILD | PROJECTILE_BEAM,
                    pos: self.weld_on(t, from.to_f32()).0,
                    size: self.bp(t).radius.to_f32(),
                    wake: 0.0,
                    plasma: 0.0,
                    _pad: [0.0; 2],
                    aim: [0.0; 4],
                    prev_aim: [0.0; 4],
                });
            }
        }
        // Factory print guns: one amber thread per nozzle, never during a refit.
        for row in s.units.slots.iter() {
            if s.units.flags[row] & flag::BUILDING == 0 || !self.bp(row).has(mc_data::cat::FACTORY)
            {
                continue;
            }
            if viewer.is_some_and(|v| {
                self.are_enemies(v, s.units.owner[row]) && !self.detects_for_team(v, row)
            }) {
                continue;
            }
            let Some(t) = s.units.row(s.units.build_target[row]) else {
                continue;
            };
            if !s.units.has_flag(t, flag::UNDER_CONSTRUCTION) || s.units.has_flag(t, flag::UPGRADE)
            {
                continue;
            }
            let heads = self.factory_print_heads(row);
            if let Some(first) = heads.first() {
                let to = self.weld_on(t, first.to_f32()).0;
                frame.build_sources.push(self.build_source(row, to));
            } else if self.uses_nanites(row) {
                // A Regency factory without fabricator heads is heard from the hull it builds.
                let at = s.units.pos[t].extend(s.units.z[t]).to_f32();
                frame.build_sources.push(self.build_source(row, at));
            }
            if self.uses_nanites(row) {
                // A Regency factory pours a nanite stream from each fabricator head. Each needs
                // a source of its own (the renderer times a beam by its source).
                if !heads.is_empty() {
                    nanite_sites.push(s.units.id(t).0);
                }
                for (k, head) in heads.iter().enumerate() {
                    let to = self.weld_on(t, head.to_f32()).0;
                    nanite_beams.push((
                        s.units.id(row).0 ^ ((k as u32) << 27),
                        BeamInstance {
                            from: head.to_f32(),
                            kind: BEAM_NANITE,
                            to_prev: to,
                            radius: self.bp(t).radius.to_f32(),
                            to,
                            height: 0.0,
                        },
                    ));
                }
                continue;
            }
            for head in heads {
                let to = self.weld_on(t, head.to_f32()).0;
                frame.projectiles.push(ProjectileInstance {
                    prev_pos: head.to_f32(),
                    color: COLOR_BUILD | PROJECTILE_BEAM,
                    pos: to,
                    size: self.bp(t).radius.to_f32(),
                    wake: 0.0,
                    plasma: 0.0,
                    _pad: [0.0; 2],
                    aim: [0.0; 4],
                    prev_aim: [0.0; 4],
                });
            }
        }

        // Upgrades a unit or structure works on itself: no beam, but the same
        // construction hum, heard from the hull being refitted.
        for row in s.units.slots.iter() {
            let flags = s.units.flags[row];
            if flags & flag::BUILDING == 0 || flags & flag::REPAIRING != 0 {
                continue;
            }
            if s.orders.front(&s.units, row).map(|o| o.kind)
                != Some(crate::tables::OrderKind::Upgrade)
            {
                continue;
            }
            if viewer.is_some_and(|v| {
                self.are_enemies(v, s.units.owner[row]) && !self.detects_for_team(v, row)
            }) {
                continue;
            }
            let at = s.units.pos[row].extend(s.units.z[row]).to_f32();
            let h = self.bp(row).height.to_f32();
            frame
                .build_sources
                .push(self.build_source(row, [at[0], at[1], at[2] + h * 0.5]));
        }

        self.write_reclaim_beams(viewer, &mut frame.beams, &mut frame.beam_sources);
        self.write_repair_beams(viewer, &mut frame.beams, &mut frame.beam_sources);
        self.write_survival_beams(viewer, &mut frame.beams, &mut frame.beam_sources);
        for (source, beam) in nanite_beams {
            frame.beam_sources.push(source);
            frame.beams.push(beam);
        }
        // Sites of a faction that builds with nanites condense out of a swarm, and its
        // refits go up the same way.
        for u in frame.units.iter_mut() {
            let working = u.owner_flags & ((crate::tables::flag::UNDER_CONSTRUCTION as u32) << 8)
                != 0
                || (u.upgrade > 0.0 && u.upgrade < 1.0);
            if u.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST) == 0
                && working
                && self
                    .blueprints
                    .units
                    .get(u.blueprint as usize)
                    .is_some_and(|b| self.faction_uses_nanites(b.faction))
            {
                u.status[1] |= UNIT_NANITE;
            }
        }
        // Round each Regency site at work, its rings and rising filaments: one record a site,
        // timed by the site's id with bits 27..31 set (no stream's source has them all). A
        // Regency refit gets the same, fed or not: it goes up the way a new hull does.
        nanite_sites.sort_unstable();
        nanite_sites.dedup();
        for u in frame.units.iter() {
            let printing = (crate::tables::flag::UNDER_CONSTRUCTION as u32) << 8;
            let fed =
                u.owner_flags & printing != 0 && nanite_sites.binary_search(&u.unit_id).is_ok();
            let refit = u.upgrade > 0.0 && u.upgrade < 1.0 && u.status[1] & UNIT_NANITE != 0;
            if !fed && !refit {
                continue;
            }
            let Some(bp) = self.blueprints.units.get(u.blueprint as usize) else {
                continue;
            };
            let height = bp.height.to_f32();
            // Where the hull has condensed to (`nanite_grow` in entity.wgsl); a refit's
            // work climbs it as the refit goes.
            let done = if fed { u.build / 0.8 } else { u.upgrade };
            let front = u.pos[2] + height * done.clamp(0.0, 1.0);
            let to = [u.pos[0], u.pos[1], front];
            frame.beam_sources.push(u.unit_id | 0xF800_0000);
            frame.beams.push(BeamInstance {
                from: u.pos,
                kind: BEAM_NANITE_SITE,
                to_prev: to,
                radius: bp.radius.to_f32(),
                to,
                height,
            });
        }
        frame.precursor_activity = self.survival_activity();

        // Units a replicator is printing fill in its violet, not construction amber.
        let printing = self.survival_printing();
        if !printing.is_empty() {
            for u in frame.units.iter_mut() {
                if u.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST) == 0
                    && printing.binary_search(&u.unit_id).is_ok()
                {
                    u.status[1] |= 1;
                }
            }
        }

        self.push_wrecks(viewer, &mut frame.units);

        // Spent casings: tumbling in the air, then lying where they came down and sinking
        // away. One in the air names the walker that threw it: the shader carries it with
        // that walker's drawn stride at first, so it leaves the port the gun is drawn at,
        // not the sim's.
        let drawn = if s.sabots.is_empty() {
            Vec::new()
        } else {
            drawn_units(&frame.units)
        };
        for (i, sabot) in s.sabots.iter().enumerate() {
            let from = drawn_at(&drawn, sabot.source.0).map_or(0, |at| at + 1);
            let Some(wreck) = self
                .blueprints
                .unit(sabot.blueprint)
                .weapons
                .get(sabot.weapon as usize)
                .and_then(|w| w.sabot)
                .map(|s| s.casing)
            else {
                continue;
            };
            let here = if sabot.lying > 0 {
                sabot.rest
            } else {
                sabot.at()
            };
            // Debris, not a unit: shown anywhere explored, like wrecks.
            if let (Some(v), true) = (viewer, s.fog_enabled) {
                if !self.fog.is_explored(here.xy(), self.team_mask(v)) {
                    continue;
                }
            }
            if sabot.lying > 0 {
                let bp = self.blueprints.unit(wreck);
                frame.units.push(lying_casing(
                    sabot,
                    wreck,
                    [bp.radius.to_f32(), bp.height.to_f32()],
                    i,
                ));
                continue;
            }
            let (pos, prev) = (sabot.at().to_f32(), sabot.before().to_f32());
            let (y0, p0, r0) = sabot.tumble(sabot.age.saturating_sub(1));
            let (y1, p1, r1) = sabot.tumble(sabot.age);
            frame.units.push(UnitInstance {
                prev_pos: prev,
                pos,
                prev_heading: y0,
                heading: y1,
                blueprint: wreck.0 as u32,
                owner_flags: KIND_WRECK,
                health: 1.0,
                build: 1.0,
                radius: self.blueprints.unit(wreck).radius.to_f32(),
                unit_id: 0x5AB0_0000 | (sabot.seed & 0xFFFF) ^ i as u32,
                packed: WRECK_FALLING,
                gait: [
                    sabot.age.saturating_sub(1) as f32 / TICKS_PER_SECOND as f32,
                    sabot.age as f32 / TICKS_PER_SECOND as f32,
                    0.0,
                ],
                status: [0, 0, from],
                arm_pitch: [p0, p1, 0.0, 0.0],
                _pad2: [r0, r1],
                deploy: 1.0,
                prev_deploy: 1.0,
                ..UnitInstance::zeroed()
            });
        }

        self.push_crashes(viewer, &mut frame.units);
        self.push_sinking(viewer, &mut frame.units);

        // Wall sections side by side are drawn as one wall.
        join_walls(&self.blueprints, &mut frame.units, &[]);

        frame.shields.clear();
        for row in s.units.slots.iter() {
            let Some(spec) = self.bp(row).shield else {
                continue;
            };
            if s.units.has_flag(row, crate::tables::flag::UPGRADE) {
                continue;
            }
            let open = s.units.shield_open[row];
            let prev = s.units.prev_shield_open[row];
            // A recovering generator stays in the list so the selection panel
            // can show charge filling while the dome is still down.
            if open == 0
                && prev == 0
                && s.units.shield_hp[row] <= Fx::ZERO
                && s.units.shield_recharge[row] == 0
            {
                continue;
            }
            if viewer.is_some_and(|v| {
                self.are_enemies(v, s.units.owner[row]) && !self.detects_for_team(v, row)
            }) {
                continue;
            }
            let owner = s.units.owner[row];
            let team = self.state.players[owner as usize].team;
            let kind = spec.kind as u32;
            let max = spec.health.max(Fx::ONE);
            // An upgraded dome holds its size while the refit is built, then swells
            // out to the new radius once it is done (`shield_grow`).
            let grow = s.units.shield_grow[row];
            let radius = self.dome_radius_at(row, grow).to_f32();
            let prev_radius = if grow == 0 {
                radius
            } else {
                self.dome_radius_at(row, grow + 1).to_f32()
            };
            frame.shields.push(ShieldInstance {
                pos: s.units.pos[row].extend(s.units.z[row]).to_f32(),
                radius,
                prev_open: prev as f32 / 255.0,
                open: open as f32 / 255.0,
                health: (s.units.shield_hp[row] / max).to_f32(),
                packed: owner as u32
                    | (team as u32) << 8
                    | (self.bp(row).tech as u32) << 16
                    | u32::from(s.units.shield_recharge[row] > 0) << 24
                    | kind << 25
                    | u32::from(self.shield_off(row)) << 26
                    | if s.units.has_flag(row, crate::tables::flag::INVULNERABLE) {
                        SHIELD_VEIL
                    } else {
                        0
                    }
                    | (self.shield_look(row) as u32) << SHIELD_LOOK_SHIFT,
                unit_id: s.units.id(row).0,
                projector: if spec.is_hull() {
                    0.0
                } else {
                    mc_data::SHIELD_PROJECTOR_HEIGHT
                },
                height: self.bp(row).height.to_f32() + mc_data::HULL_SHIELD_PAD as f32,
                prev_radius,
            });
        }

        frame.fires.clear();
        for i in 0..s.fires.len() {
            let span = s.fires.span[i].max(1) as f32 * 0.1;
            let left = s.fires.ticks[i] as f32 * 0.1;
            frame.fires.push(FireInstance {
                pos: s.fires.pos[i].extend(s.fires.z[i]).to_f32(),
                radius: s.fires.radius[i].to_f32(),
                elapsed: (span - left).max(0.0),
                duration: span,
            });
        }

        frame.stains.clear();
        for i in 0..s.stains.len() {
            frame.stains.push(StainInstance {
                pos: s.stains.pos[i].to_f32(),
                radius: s.stains.radius[i].to_f32(),
                strength_seed: s.stains.strength[i] as u32 | (s.stains.seed[i] as u32) << 8,
            });
        }

        frame.pads.clear();
        for i in 0..s.pads.len() {
            frame.pads.push(StainInstance {
                pos: s.pads.pos[i].to_f32(),
                radius: s.pads.radius[i].to_f32(),
                strength_seed: s.pads.packed[i],
            });
        }

        frame.events.clear();
        frame.events.extend(self.events.iter().cloned());
        self.write_warp_events(viewer, &mut frame.events);
        self.write_strategic(&mut frame.strategic);
        self.write_warhead_plans(
            viewer,
            &mut frame.warhead_tracks,
            &mut frame.planned_launches,
        );
        self.write_warps(viewer, &mut frame.warps);
        self.write_destructs(viewer, &mut frame.destructs);
        self.write_dampers(viewer, &mut frame.dampers);

        self.write_fog(viewer, frame);

        frame.props_dead.clear();
        frame.props_dead.extend(
            s.props_dead
                .iter()
                .flat_map(|w| [*w as u32, (*w >> 32) as u32]),
        );
        // The table only grows (a restore or a seek back replaces it, and goes back in
        // time): a frame that already holds it as it stands keeps its copy.
        let edits = &s.terrain_edits;
        let held = frame.terrain_edits.len() == edits.len()
            && prev_tick <= s.tick
            && frame.terrain_edits.first().copied() == edits.first().map(|e| e.record())
            && frame.terrain_edits.last().copied() == edits.last().map(|e| e.record());
        if !held {
            frame.terrain_edits.clear();
            frame.terrain_edits.extend(edits.iter().map(|e| e.record()));
            frame.terrain_edit_factions.clear();
            frame
                .terrain_edit_factions
                .extend(edits.iter().map(|e| e.faction));
        }
    }

    /// The order queues of `watch` (unit ids), for the interface. Ids that no
    /// longer resolve, and units `viewer` does not own, are left out.
    /// The order queues of the units in `watch` and, with `everyone` naming a player, of
    /// every mobile unit of theirs that has orders. `viewer` limits both to one owner.
    pub fn write_orders(
        &self,
        viewer: Option<u8>,
        watch: &[u32],
        everyone: Option<u8>,
        out: &mut Vec<UnitOrders>,
    ) {
        use crate::tables::OrderKind;
        let s = &self.state;
        out.clear();
        // Each side's tier, worked out once when first needed (it walks every unit).
        let mut side_tech = [0u8; mc_core::MAX_PLAYERS];
        let watched = watch
            .iter()
            .filter_map(|&id| s.units.row(crate::Handle(id)));
        // Sorted, for the test below against every unit of the side.
        let mut watching = watch.to_vec();
        watching.sort_unstable();
        let rest = s.units.slots.iter().filter(|&row| {
            everyone == Some(s.units.owner[row])
                // Builders too, standing or not: a factory's product shows its progress unselected.
                && ((s.units.is_active(row)
                    && (self.bp(row).is_mobile() || self.bp(row).builder.is_some())
                    && s.orders.front(&s.units, row).is_some())
                    || !s.units.standing[row].is_empty()
                    // A batching factory, idle or not: its batch is drawn on the ground.
                    || self.batching(row))
                && watching.binary_search(&s.units.id(row).0).is_err()
        });
        for row in watched.chain(rest.take(MAX_LISTED_UNITS)) {
            let id = s.units.id(row).0;
            if viewer.is_some_and(|v| v != s.units.owner[row]) {
                continue;
            }
            let orders: Vec<QueuedOrder> = s
                .orders
                .iter(&s.units, row)
                .take(MAX_LISTED_ORDERS)
                .map(|o| {
                    let target = match o.kind {
                        OrderKind::Attack
                        | OrderKind::Assist
                        | OrderKind::ReclaimUnit
                        | OrderKind::Guard => s.units.row(o.target).map(|r| s.units.pos[r]),
                        OrderKind::Reclaim => {
                            s.wrecks.slots.resolve(o.target).map(|r| s.wrecks.pos[r])
                        }
                        OrderKind::Produce | OrderKind::Upgrade => Some(s.units.pos[row]),
                        _ => None,
                    };
                    QueuedOrder {
                        formation: o.formation,
                        offset: o.offset.to_f32(),
                        moving_slot: s
                            .formations
                            .get(&o.formation)
                            .filter(|g| g.phase != 0)
                            .map(|g| (g.anchor + o.offset.rotate(g.heading - o.heading)).to_f32()),
                        formation_phase: s.formations.get(&o.formation).map_or(0, |g| g.phase),
                        kind: o.kind,
                        pos: target.unwrap_or(o.pos).to_f32(),
                        at: o.pos,
                        blueprint: o.blueprint,
                        radius: o.radius.to_f32(),
                    }
                })
                .collect();
            let target = s.units.row(s.units.build_target[row]);
            let progress = target.map_or(0.0, |t| {
                (s.units.build_progress[t] / self.bp(t).build_time).to_f32()
            });
            let flow = self.flows.get(row).copied().unwrap_or_default();
            let rate = |v: Fx| (v * TICKS_PER_SECOND as i32).to_f32();
            // What it builds, or itself while it goes up with builders on it.
            let building = target.or_else(|| (flow.built > Fx::ZERO).then_some(row));
            let eta = building.and_then(|t| {
                let pace = rate(self.flows.get(t).map_or(Fx::ZERO, |f| f.built));
                let left = (self.bp(t).build_time - s.units.build_progress[t]).to_f32();
                (pace > 0.0 && left > 0.0).then(|| left / pace)
            });
            let standing = self
                .standing_view(row)
                .into_iter()
                .map(|v| QueuedOrder {
                    formation: 0,
                    offset: [0.0; 2],
                    moving_slot: None,
                    formation_phase: 0,
                    kind: v.kind,
                    pos: v.pos.to_f32(),
                    at: v.at,
                    blueprint: BlueprintId(0),
                    radius: v.radius.to_f32(),
                })
                .collect();
            out.push(UnitOrders {
                unit_id: id,
                orders,
                standing,
                progress,
                eta,
                building: building.map(|t| s.units.id(t).0),
                mass_made: rate(flow.made[0]),
                energy_made: rate(flow.made[1]),
                mass_wanted: rate(flow.wanted[0]),
                energy_wanted: rate(flow.wanted[1]),
                mass_used: rate(flow.used[0]),
                energy_used: rate(flow.used[1]),
                efficiency: s.players[s.units.owner[row] as usize].efficiency.to_f32(),
                reclaimed: s.units.reclaimed[row].to_f32(),
                side_tech: {
                    let owner = s.units.owner[row];
                    let t = &mut side_tech[owner as usize];
                    if *t == 0 {
                        *t = self.side_tech(owner);
                    }
                    *t
                },
                mine: self.mine_view(row),
                hidden_target: self.hidden_target(row).map(|t| {
                    let p = s.units.pos[t].to_f32();
                    [p[0], p[1], (s.units.z[t] + self.bp(t).height / 2).to_f32()]
                }),
                cargo: self.cargo_view(row),
                warp_recharge: self.warp_recharge_seconds(row),
                stunned: self.stun_seconds(row),
                batch: self.batch_view(row),
            });
        }
    }

    fn mine_view(&self, row: usize) -> Option<MineView> {
        let spec = self.bp(row).mine?;
        let m = self.state.mines.by_unit.get(&self.state.units.id(row))?;
        let owner = self.state.units.owner[row] as usize;
        let powered = self
            .state
            .players
            .get(owner)
            .map_or(Fx::ONE, |p| p.mine_power);
        Some(MineView {
            rate: (spec.rate * crate::mines::mine_power(powered)).to_f32(),
            full: spec.rate.to_f32(),
            age: m.age as f32 / TICKS_PER_SECOND as f32,
        })
    }

    /// Every structure `player`'s builders have planned, begun ones left out: they are on the map.
    pub fn write_plans(&self, player: u8, out: &mut Vec<PlannedBuild>) {
        let units = &self.state.units;
        out.clear();
        out.extend(self.planned_sites(player).map(|(row, o)| PlannedBuild {
            unit_id: units.id(row).0,
            blueprint: o.blueprint,
            pos: o.pos.to_f32(),
            at: o.pos,
            heading: o.heading.to_radians_f32(),
        }));
    }

    /// Detection shared across the viewer's team.
    fn detects_for_team(&self, viewer: u8, row: usize) -> bool {
        !self.state.fog_enabled || self.detected_by(row, self.team_mask(viewer))
    }

    /// How the viewer should draw this enemy: radar-only, and whether vision
    /// has ever named it. Allies and observers get nothing extra.
    fn contact_flags(&self, viewer: Option<u8>, row: usize) -> u32 {
        let Some(v) = viewer else {
            return 0;
        };
        if !self.state.fog_enabled {
            return 0;
        }
        if !self.are_enemies(v, self.state.units.owner[row]) {
            return 0;
        }
        let mask = self.team_mask(v);
        if self.seen_by(row, mask) {
            return 0;
        }
        let mut flags = STATE_RADAR;
        if !self
            .fog
            .is_identified(row, self.state.units.id(row).generation(), mask)
        {
            flags |= STATE_UNIDENTIFIED;
        }
        flags
    }
}

fn dist2(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    dx * dx + dy * dy + dz * dz
}

/// Nearby print hits become one origin. Merge distance grows with the hull so
/// thirty engineers around a factory become a ring, not one blob and not thirty
/// overlapping fronts.
fn cluster_print_origins(points: &[[f32; 3]], radius: f32) -> Vec<[f32; 3]> {
    if points.is_empty() {
        return Vec::new();
    }
    if points.len() == 1 {
        return vec![points[0]];
    }
    let ring = (radius * 0.82 * 2.0 * std::f32::consts::PI).max(1.0);
    let want = points.len().clamp(1, MAX_WELDS_PER_SITE);
    // Distance to join a cluster is half the arc we want between origins, so
    // a full ring of builders becomes about `want` fronts, not half that.
    let merge = (ring / (2.0 * want as f32)).max(4.0);
    let merge2 = merge * merge;
    let mut clusters: Vec<([f32; 3], u32)> = Vec::new();
    for &p in points {
        let mut best = None;
        let mut best_d = merge2;
        for (i, (sum, n)) in clusters.iter().enumerate() {
            let inv = 1.0 / *n as f32;
            let mean = [sum[0] * inv, sum[1] * inv, sum[2] * inv];
            let d = dist2(mean, p);
            if d < best_d {
                best_d = d;
                best = Some(i);
            }
        }
        match best {
            Some(i) => {
                clusters[i].0[0] += p[0];
                clusters[i].0[1] += p[1];
                clusters[i].0[2] += p[2];
                clusters[i].1 += 1;
            }
            None => clusters.push((p, 1)),
        }
    }
    let mut out: Vec<[f32; 3]> = clusters
        .iter()
        .map(|(sum, n)| {
            let inv = 1.0 / *n as f32;
            [sum[0] * inv, sum[1] * inv, sum[2] * inv]
        })
        .collect();
    while out.len() > MAX_WELDS_PER_SITE {
        let mut pair = (0, 1);
        let mut best = f32::MAX;
        for i in 0..out.len() {
            for j in i + 1..out.len() {
                let d = dist2(out[i], out[j]);
                if d < best {
                    best = d;
                    pair = (i, j);
                }
            }
        }
        let (i, j) = pair;
        let merged = [
            (out[i][0] + out[j][0]) * 0.5,
            (out[i][1] + out[j][1]) * 0.5,
            (out[i][2] + out[j][2]) * 0.5,
        ];
        out.remove(j);
        out[i] = merged;
    }
    out
}

fn fade_construction_welds(
    prev: Vec<ConstructionWeld>,
    live: &HashMap<u32, Vec<[f32; 3]>>,
    decay: f32,
) -> Vec<ConstructionWeld> {
    let stick2 = WELD_STICK * WELD_STICK;
    let mut used: HashMap<u32, Vec<bool>> = live
        .iter()
        .map(|(site, points)| (*site, vec![false; points.len()]))
        .collect();
    let mut out = Vec::new();
    for old in prev {
        let mut matched = None;
        if let Some(points) = live.get(&old.site) {
            let flags = used.get_mut(&old.site).unwrap();
            let mut best_d = stick2;
            for (i, p) in points.iter().enumerate() {
                if flags[i] {
                    continue;
                }
                let d = dist2(old.local, *p);
                if d < best_d {
                    best_d = d;
                    matched = Some(i);
                }
            }
            if let Some(i) = matched {
                flags[i] = true;
                out.push(ConstructionWeld {
                    site: old.site,
                    local: points[i],
                    fade: 1.0,
                });
                continue;
            }
        }
        let fade = old.fade - decay;
        if fade > 0.02 {
            out.push(ConstructionWeld {
                site: old.site,
                local: old.local,
                fade,
            });
        }
    }
    for (site, points) in live {
        let flags = used.get(site).map(|v| v.as_slice()).unwrap_or(&[]);
        for (i, p) in points.iter().enumerate() {
            if flags.get(i).copied().unwrap_or(false) {
                continue;
            }
            out.push(ConstructionWeld {
                site: *site,
                local: *p,
                fade: 1.0,
            });
        }
    }
    out
}

fn pack_construction_welds(
    out: &mut Vec<ConstructionWeld>,
    welds: Vec<ConstructionWeld>,
) -> HashMap<u32, (u32, u32)> {
    out.clear();
    let mut by_site: HashMap<u32, Vec<ConstructionWeld>> = HashMap::new();
    for w in welds {
        by_site.entry(w.site).or_default().push(w);
    }
    let mut sites: Vec<u32> = by_site.keys().copied().collect();
    sites.sort_unstable();
    let mut ranges = HashMap::new();
    for site in sites {
        if out.len() >= MAX_CONSTRUCTION_WELDS {
            break;
        }
        let list = by_site.get_mut(&site).unwrap();
        list.sort_by(|a, b| {
            a.local[1]
                .atan2(a.local[0])
                .total_cmp(&b.local[1].atan2(b.local[0]))
        });
        let first = out.len() as u32;
        for w in list.iter().copied().take(MAX_WELDS_PER_SITE) {
            if out.len() >= MAX_CONSTRUCTION_WELDS {
                break;
            }
            out.push(w);
        }
        ranges.insert(site, (first, out.len() as u32 - first));
    }
    ranges
}

fn weld_on_unit(
    ranges: &HashMap<u32, (u32, u32)>,
    welds: &[ConstructionWeld],
    id: u32,
) -> Option<([f32; 3], u32, u32)> {
    let &(first, count) = ranges.get(&id)?;
    if count == 0 {
        return None;
    }
    let slice = &welds[first as usize..first as usize + count as usize];
    let local = slice
        .iter()
        .find(|w| w.fade >= 0.99)
        .or_else(|| slice.first())
        .map(|w| w.local)
        .unwrap_or([0.0; 3]);
    Some((local, first, count))
}

// Presentation records of other systems (re-exported from `reclaim`, `transport`,
// `survival` and `nukes`). They live here so this module's `expect` also covers
// what their derives generate beside them.

/// A beam between a unit and its work, for the renderer. The far end glides
/// from `to_prev` to `to` over the tick, as the unit it is on does.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct BeamInstance {
    /// The emitter.
    pub from: [f32; 3],
    pub kind: u32,
    /// Foot of the target a tick ago.
    pub to_prev: [f32; 3],
    /// Size of the target: what is torn off it comes from all over this.
    pub radius: f32,
    pub to: [f32; 3],
    pub height: f32,
}

/// Presentation of a lift ship that is down (`World::lift_decks`): enough to raise what
/// walks its ramp and hold onto the deck, since the sim keeps land units on the ground.
#[derive(Clone, Copy, Debug)]
pub struct Deck {
    pub pos: [f32; 2],
    /// Its heading as a unit vector.
    pub dir: [f32; 2],
    pub ground: f32,
    pub hinge: f32,
    pub lip: f32,
    pub front: f32,
    pub half_width: f32,
    pub floor: f32,
    /// How far the ramp is down, zero to one.
    pub open: f32,
}

impl Deck {
    /// Metres over the ground of the deck at `p`, where a unit there stands; zero off it.
    pub fn lift(&self, p: [f32; 2]) -> f32 {
        let d = [p[0] - self.pos[0], p[1] - self.pos[1]];
        let x = d[0] * self.dir[0] + d[1] * self.dir[1];
        let y = -d[0] * self.dir[1] + d[1] * self.dir[0];
        if y.abs() > self.half_width || x < self.lip || x > self.front {
            return 0.0;
        }
        if x >= self.hinge {
            return self.floor;
        }
        // Down the ramp: it lies from the hinge to the lip once open, and swings up closed.
        let along = (x - self.lip) / (self.hinge - self.lip);
        self.floor * along * self.open
    }

    /// Which way is up for a unit standing on the deck at `p` (world, unit length): tilted
    /// with the ramp's slope on the ramp, easing in over its foot and its top so a unit
    /// does not snap onto it; straight up on the hold floor. `None` off the deck.
    pub fn up(&self, p: [f32; 2]) -> Option<[f32; 3]> {
        let d = [p[0] - self.pos[0], p[1] - self.pos[1]];
        let x = d[0] * self.dir[0] + d[1] * self.dir[1];
        let y = -d[0] * self.dir[1] + d[1] * self.dir[0];
        if y.abs() > self.half_width || x < self.lip || x > self.front {
            return None;
        }
        let run = (self.hinge - self.lip).max(1.0);
        let ease = |e: f32| {
            let e = e.clamp(0.0, 1.0);
            e * e * (3.0 - 2.0 * e)
        };
        let on_ramp = ease((x - self.lip) / 6.0) * (1.0 - ease((x - self.hinge + 6.0) / 6.0));
        let slope = self.floor * self.open / run * on_ramp;
        let n = (slope * slope + 1.0).sqrt();
        Some([-slope * self.dir[0] / n, -slope * self.dir[1] / n, 1.0 / n])
    }
}

/// A Shaper node as the survival HUD shows it (`survival::SurvivalStatus::nodes`).
#[derive(Clone, Copy, Debug)]
pub struct NodeStatus {
    pub site: u8,
    pub pos: [f32; 2],
    pub product: BlueprintId,
    /// 0..1 while the ray raises it; 1 online.
    pub raised: f32,
    pub printed: u32,
}

/// A launch ordered and not yet away, for its own side and observers only.
#[derive(Clone, Copy, Debug)]
pub struct PlannedLaunch {
    /// The silo that will fire it.
    pub silo: u32,
    pub owner: u8,
    /// Its number among the launches ordered (`Strategic::orders`): lower goes first.
    pub order: u32,
    /// The path the warhead will fly.
    pub path: crate::nukes::WarheadPath,
    /// The silo's doors are opening for it now.
    pub opening: bool,
    /// Seconds until it bursts, if nothing stops it.
    pub eta: f32,
}

#[cfg(test)]
mod recoil_tests {
    use super::barrel_recoil;

    #[test]
    fn kick_is_instant_and_ignores_spawn_stagger() {
        assert_eq!(barrel_recoil(160, 160), 1.0);
        assert_eq!(barrel_recoil(0, 160), 0.0);
        // Newly built guns sit on half a reload; that must not look like recoil.
        assert_eq!(barrel_recoil(80, 160), 0.0);
        let mid = barrel_recoil(156, 160);
        assert!(mid > 0.2 && mid < 0.9, "{mid}");
        let (now, prev) = super::barrel_recoil_pair(160, 160);
        assert_eq!((now, prev), (1.0, 0.0));
        let (now, prev) = super::barrel_recoil_pair(159, 160);
        assert!(prev > now && prev > 0.8, "{prev} -> {now}");
    }
}

#[cfg(test)]
mod weld_tests {
    use super::{cluster_print_origins, MAX_WELDS_PER_SITE};

    #[test]
    fn opposite_builders_keep_their_own_origin() {
        let clustered = cluster_print_origins(&[[8.0, 0.0, 2.0], [-8.0, 0.0, 2.0]], 10.5);
        assert_eq!(clustered.len(), 2);
    }

    #[test]
    fn a_crowd_around_a_factory_becomes_a_ring() {
        let r = 46.0;
        let points: Vec<[f32; 3]> = (0..30)
            .map(|i| {
                let a = i as f32 / 30.0 * std::f32::consts::TAU;
                [a.cos() * r * 0.82, a.sin() * r * 0.82, 8.0]
            })
            .collect();
        let clustered = cluster_print_origins(&points, r);
        assert!(
            clustered.len() >= 6 && clustered.len() <= MAX_WELDS_PER_SITE,
            "{} origins from 30 builders",
            clustered.len()
        );
        let min_x = clustered.iter().map(|p| p[0]).fold(f32::MAX, f32::min);
        let max_x = clustered.iter().map(|p| p[0]).fold(f32::MIN, f32::max);
        assert!(
            min_x < -10.0 && max_x > 10.0,
            "the ring still has two sides ({min_x} .. {max_x})"
        );
    }
}
