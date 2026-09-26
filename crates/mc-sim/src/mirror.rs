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

use crate::reclaim::BEAM_GROW;
use crate::tables::UnitId;
use crate::World;
use bytemuck::{Pod, Zeroable};
use mc_core::{Fx, FxVec3, TICKS_PER_SECOND};
use mc_data::{BlueprintId, Trajectory, WeaponColor};
use std::collections::HashMap;

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
    /// An anti-missile laser from `from` to a hostile missile at `to`.
    /// `killed` is the tick the casing fails.
    MissileLased {
        from: FxVec3,
        to: FxVec3,
        killed: bool,
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
    },
    /// An interceptor out of its cell at `from`.
    InterceptorLaunch {
        from: FxVec3,
        owner: u8,
        serial: u32,
    },
    /// An interceptor burst at `pos`: `killed` a warhead, or burnt out with nothing to hit.
    WarheadIntercepted {
        pos: FxVec3,
        owner: u8,
        killed: bool,
    },
    /// A nuclear blast: `radius` is how far it does damage; its front runs out over
    /// seconds after this. `commander`: a commander's reactor, not a warhead.
    NuclearDetonation {
        pos: FxVec3,
        radius: mc_core::Fx,
        owner: u8,
        commander: bool,
    },
    /// A launcher finished assembling a round (`warhead`: a silo's; else an interceptor).
    RoundReady {
        unit: UnitId,
        pos: FxVec3,
        owner: u8,
        warhead: bool,
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
    /// Turret yaw of the first weapon relative to the hull, radians.
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
    pub gait: [f32; 3],
    /// Zero, or how far along the unit's refit is: above zero from its first tick, one when done.
    pub upgrade: f32,
    /// Pitch of the gun arm last tick and this, then of the build arm: radians, up positive.
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
    /// deck state and the pause mark, 1 gun-house index, grown and replicating marks,
    /// 2 a launcher's rounds (`nukes::LAUNCHER_*`) or a mounted gun's aim; on a spent
    /// casing in the air, one more than the index of the walker that threw it in
    /// `RenderFrame::units` (zero when it is not drawn).
    pub status: [u32; 3],
    /// A weapon on a turret of its own (`Weapon::mount`): its yaw off the torso last tick
    /// and this, then its pitch last tick and this (radians).
    pub mount: [f32; 4],
    /// A rotary gun's barrels, turned last tick and this (radians, unwrapped between the
    /// two); then the mounted weapon's kick-back, last tick and this (1 the instant it fires).
    pub spin_recoil: [f32; 4],
}

const _: () = assert!(std::mem::size_of::<UnitInstance>() == 192);

impl UnitInstance {
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

    /// Stored in a lift ship's hold (`UNIT_STORED`).
    pub fn stored(&self) -> bool {
        self.status[0] & UNIT_STORED != 0
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

/// `owner_flags` bit: the record is a wreck. `health` holds the share of mass left.
pub const KIND_WRECK: u32 = 1 << 31;
/// _pad on a KIND_WRECK instance: intact falling hull, not settled salvage.
pub const WRECK_FALLING: u32 = 1;
/// _pad on a KIND_WRECK instance: a ship's hull going down through the water to the
/// seabed. `arm_pitch` = [prev pitch, pitch, 0, 0], `_pad2` = [prev roll, roll], and
/// `health` how far it has gone down (0 at the surface, 1 on the bottom).
pub const WRECK_SINKING: u32 = 2;
/// Units' `status[0]`, low byte: how far a submarine has dived, 0 surfaced to 255 down.
pub const UNIT_DIVE_MASK: u32 = 0xFF;
/// Units' `status[0]`: the submarine is ordered down (diving or dived), else up.
pub const UNIT_DIVE_GOAL: u32 = 1 << 8;
/// Units' `status[0]`: the player paused this unit's work (`Command::SetPaused`).
pub const UNIT_PAUSED: u32 = 1 << 9;
pub const UNIT_BURNING: u32 = 1 << 23;
/// Units' `status[0]`: a unit stored in a lift ship's hold. Listed for its own side's
/// interface (the hold, selecting and ordering it); never drawn. It is `IN_FACTORY` too.
pub const UNIT_STORED: u32 = 1 << 11;
/// Units' `status[0]` bits 16..24: a lift ship's landing gear, 0 stowed to 255 out.
pub const UNIT_GEAR_SHIFT: u32 = 16;
/// Units' `status[0]`: a land unit standing on a lift ship's deck or ramp
/// (`transport::Deck::up`). `status[2]` then holds its up vector's x and y, each an
/// i16 over 32767 (low half x), for the shader to lean it with the ramp, not the ground.
pub const UNIT_ON_DECK: u32 = 1 << 24;
/// Units' `status[1]`: the unit is being printed by a replicator (Survival). Its
/// construction fill is drawn in replication violet instead of construction amber.
pub const UNIT_REPLICATING: u32 = 1 << 0;
/// Units' `status[1]`: a construction site of a faction that grows its buildings
/// (`mc_data::Construction::Grow`): it rises out of a molten pool instead of being printed.
pub const UNIT_GROWN: u32 = 1 << 1;
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
    /// `Bombard`: how far from `at` shots may fall; `Orbit`: the circle flown, metres.
    /// Zero for every other kind.
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
    /// The highest tier its side has reached ([`World::side_tech`]): mines upgrade no further.
    pub side_tech: u8,
    /// A finished core mine's territory and output.
    pub mine: Option<MineView>,
    /// A core mine's ore fields: what each is worth and when its drift arrives.
    pub mine_veins: Vec<MineVein>,
    /// Where the target its guns are laid on stands, its middle, while the ground hides
    /// it and none of them has anything it can see (`line_of_fire.rs`).
    pub hidden_target: Option<[f32; 3]>,
    /// A lift ship: what is in its hold.
    pub cargo: Option<CargoView>,
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

/// One ore field a core mine works, as the interface shows it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MineVein {
    /// The field, in map order.
    pub field: u16,
    /// Hectares of its ore in the mine's territory.
    pub ore: f32,
    /// Materials per second it adds once reached.
    pub rate: f32,
    /// Seconds from the mine's finishing to the drift arriving, and seconds left.
    pub dig: f32,
    pub eta: f32,
}

/// A core mine as the interface shows it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MineView {
    /// Its territory: land and ore it has, and would have alone.
    pub land: crate::mines::Share,
    /// Zero to one: what its territory gives over what its whole circle would.
    pub share: f32,
    /// Materials per second now, and once every drift is dug.
    pub rate: f32,
    pub full: f32,
    /// Seconds it has been digging.
    pub age: f32,
    /// Metres out from it the land it works reaches by now.
    pub spread: f32,
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

/// Set in `ProjectileInstance::color` for a missile.
pub const PROJECTILE_MISSILE: u32 = 1 << 8;
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
/// fill the time to the next shot.
pub fn round_gap(weapon: &mc_data::Weapon) -> f32 {
    let interval = if weapon.salvo > 1 && weapon.salvo_delay_ticks > 0 {
        weapon.salvo_delay_ticks as u16
    } else {
        weapon.reload_ticks
    };
    interval.max(1) as f32 / weapon.rounds.max(1) as f32
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
    pub from: FxVec3,
    pub to: FxVec3,
    /// Share of the tick the last stretch took, zero to one.
    pub after: mc_core::Fx,
    /// Where `from` is drawn off by, to leave from the gun as it is drawn (`launch_shift`).
    pub lead: [f32; 3],
    pub blueprint: BlueprintId,
    pub weapon: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct ProjectileInstance {
    pub prev_pos: [f32; 3],
    /// Tracer colour (0 blue, 1 orange), plus `PROJECTILE_MISSILE` for a missile.
    pub color: u32,
    pub pos: [f32; 3],
    pub size: f32,
    /// Seconds the energy wake lasts. Zero: the usual hang, or none.
    pub wake: f32,
    /// Metres of blue plasma around the traveling slug. Zero: none.
    pub plasma: f32,
    /// One: a small-calibre tracer, drawn deep orange (a stream gun's rounds); up to two, redder.
    /// Then how far a cruise missile's wings are out, 0 to 1 (`cruise_wings`).
    pub _pad: [f32; 2],
    /// Nose this tick, xyz. Zero: the body follows travel (`pos - prev_pos`).
    pub aim: [f32; 4],
    /// Nose last tick. A cold body blends from this to `aim` across the frame.
    pub prev_aim: [f32; 4],
}

const _: () = assert!(std::mem::size_of::<ProjectileInstance>() == 80);

/// How far a cruise missile's wings have unfolded, 0 to 1: a sea skimmer out of a
/// vertical-launch cell flies up it with them folded and opens them as it turns over
/// (`naval_arms::POP_BOOST`). Every other missile: none.
fn cruise_wings(weapon: &mc_data::Weapon, age: u16) -> f32 {
    if !weapon.missile || weapon.skim <= Fx::ZERO || !weapon.vertical_launch {
        return 0.0;
    }
    (age.saturating_sub(crate::naval_arms::POP_BOOST) as f32 / 5.0).min(1.0)
}

fn nose_pad(cold: bool, aim: FxVec3) -> [f32; 4] {
    if !cold {
        return [0.0; 4];
    }
    let a = aim.to_f32();
    [a[0], a[1], a[2], 0.0]
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
    /// `owner | team << 8 | tech << 16 | collapsing << 24 | hull << 25 | stalling << 26`.
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

/// One tick's worth of presentation data.
#[derive(Clone, Default)]
pub struct RenderFrame {
    pub tick: u32,
    /// Units first, then wrecks (flagged `KIND_WRECK`).
    pub units: Vec<UnitInstance>,
    pub projectiles: Vec<ProjectileInstance>,
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
    /// One bit per map prop, set when destroyed.
    pub props_dead: Vec<u32>,
    /// The whole terrain edit table, in order.
    pub terrain_edits: Vec<mc_map::FlattenRecord>,
    /// Every weapon's pose for units with gun houses of their own (`Weapon::mount`):
    /// `UnitInstance::status[1]` bits 8.. hold the index here plus one.
    pub houses: Vec<HousePose>,
    /// Strategic missiles in flight, every side's: warheads and interceptors (`nukes.rs`).
    pub strategic: Vec<StrategicInstance>,
    /// Every warhead in flight with the whole path it flies, for the interface.
    pub warhead_tracks: Vec<crate::nukes::WarheadTrack>,
    /// Launches ordered and not yet away, the viewer's side only (everyone's with no viewer).
    pub planned_launches: Vec<crate::nukes::PlannedLaunch>,
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
}

const _: () = assert!(std::mem::size_of::<StrategicInstance>() == 64);

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
    /// a siege gun's spade, or a builder's folding gear. Zero: neither.
    pub(crate) fn deploy_span(&self, row: usize) -> u16 {
        let bp = self.bp(row);
        if let Some(s) = bp.strategic.as_ref() {
            // A strategic launcher's blast doors (`nukes.rs`).
            return match s.kind {
                mc_data::strategic::StrategicKind::Nuke => crate::nukes::SILO_DOOR_TICKS,
                mc_data::strategic::StrategicKind::Interceptor => crate::nukes::ARRAY_DOOR_TICKS,
            };
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

    /// Where a construction beam leaves this builder.
    /// Whether `faction` grows its buildings (`mc_data::Construction::Grow`).
    fn faction_grows(&self, faction: mc_data::FactionId) -> bool {
        self.blueprints
            .factions
            .get(faction.0 as usize)
            .is_some_and(|f| f.construction == mc_data::Construction::Grow)
    }

    /// Whether builder `row` is of a faction that grows its buildings: it feeds its work
    /// with a tendril, not a beam.
    fn grows(&self, row: usize) -> bool {
        self.faction_grows(self.bp(row).faction)
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
            .filter(|h| h.tier <= bp.tech)
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
    /// cannot pay. Radar keeps the same rule it always had (including a tower
    /// that is still going up). A dome only goes dark once it is finished; a
    /// hull wrap's stall shows on its bubble alone, not on the unit's glow.
    fn kit_unpowered(&self, row: usize) -> bool {
        let bp = self.bp(row);
        if bp.radar > Fx::ZERO && self.live_radar(row) <= Fx::ZERO {
            return true;
        }
        bp.shield.is_some_and(|s| s.is_dome())
            && self.state.units.is_active(row)
            && self.shields_unpowered(self.state.units.owner[row])
    }

    /// A shattered dome filling while down. A stall pauses the fill, so the
    /// crystal goes dark (`kit_unpowered`) instead of pulsing.
    fn kit_charging(&self, row: usize) -> bool {
        self.bp(row).shield.is_some_and(|s| s.is_dome())
            && self.state.units.is_active(row)
            && self.state.units.shield_recharge[row] > 0
            && !self.shields_unpowered(self.state.units.owner[row])
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
        // Signed: a little below level is a little below zero, not nearly a full turn.
        let pitch = |a: mc_core::Angle| {
            mc_core::Angle::ZERO.delta_to(a) as f32 * (std::f32::consts::TAU / 65536.0)
        };
        // What walks up a lift ship's ramp stands on it, not on the ground under it.
        let decks = self.lift_decks();
        let on_deck = |row: usize, p: mc_core::FxVec2| -> f32 {
            if decks.is_empty() || self.is_air(row) {
                return 0.0;
            }
            let p = [p.x.to_f32(), p.y.to_f32()];
            decks.iter().map(|d| d.lift(p)).fold(0.0, f32::max)
        };
        let deck_up = |row: usize| -> Option<u32> {
            if decks.is_empty() || self.is_air(row) {
                return None;
            }
            let p = s.units.pos[row];
            let up = decks
                .iter()
                .find_map(|d| d.up([p.x.to_f32(), p.y.to_f32()]))?;
            let q = |v: f32| ((v * 32767.0).round().clamp(-32767.0, 32767.0) as i16 as u16) as u32;
            Some(q(up[0]) | q(up[1]) << 16)
        };
        for row in s.units.slots.iter() {
            if let Some(v) = viewer {
                if self.are_enemies(v, s.units.owner[row]) && !self.detects_for_team(v, row) {
                    continue;
                }
            }
            let bp = self.bp(row);
            // A refit is shown on the unit being refitted, not as a second unit inside it.
            if s.units.has_flag(row, crate::tables::flag::UPGRADE) {
                continue;
            }
            // In a lift ship's hold: its own side still lists it (`UNIT_STORED`) so the
            // hold can show it and it can be picked and ordered, but nothing draws it.
            let stored = s.units.hangar[row] != crate::Handle::NONE;
            if stored && viewer.is_some_and(|v| self.are_enemies(v, s.units.owner[row])) {
                continue;
            }
            let site = self.structure_upgrade(row);
            let refit = s.orders.front(&s.units, row).filter(|o| {
                o.kind == crate::tables::OrderKind::Upgrade && self.upgrades_in_place(row)
            });
            let upgrade = site
                .or(refit.and_then(|_| s.units.row(s.units.build_target[row])))
                .map_or(0.0, |t| {
                    (s.units.build_progress[t] / self.bp(t).build_time)
                        .to_f32()
                        .clamp(0.002, 1.0)
                });
            let step = s.units.gait_step[row];
            let mut flags = s.units.flags[row];
            let contact = self.contact_flags(viewer, row);
            if contact & STATE_RADAR != 0 && flags & crate::tables::flag::IN_FACTORY != 0 {
                continue;
            }
            let (build, weld_id) = match site {
                Some(t) => {
                    // The structure rebuilds in place: same fill-and-wave as a fresh site.
                    flags |= crate::tables::flag::UNDER_CONSTRUCTION;
                    (
                        (s.units.build_progress[t] / self.bp(t).build_time).to_f32(),
                        s.units.id(t).0,
                    )
                }
                None => (
                    (s.units.build_progress[row] / bp.build_time).to_f32(),
                    s.units.id(row).0,
                ),
            };
            // The tube that kicks is the arm's main gun: the heaviest on the first weapon's elbow.
            let arm = bp.weapons.first().and_then(|w| w.pivot);
            let main = (0..bp.weapons.len())
                .filter(|&w| {
                    w == 0 || (arm.is_some() && bp.weapons[w].pivot == arm && !bp.weapons[w].mount)
                })
                .max_by_key(|&w| (bp.weapons[w].damage, std::cmp::Reverse(w)))
                .unwrap_or(0);
            let (recoil, prev_recoil) = barrel_recoil_pair(
                s.units.weapon_cooldown[row][main],
                bp.weapons.get(main).map(|w| w.reload_ticks).unwrap_or(0),
            );
            let mounted = bp.weapons.iter().position(|w| w.mount);
            let (mount_kick, prev_mount_kick) = mounted.map_or((0.0, 0.0), |w| {
                barrel_recoil_pair(s.units.weapon_cooldown[row][w], bp.weapons[w].reload_ticks)
            });
            // Guns on houses of their own (`rig::HOUSE`): every weapon's pose, in a side list.
            let house = mounted.map(|_| {
                let mut hp = HousePose::default();
                for (w, weapon) in bp.weapons.iter().enumerate().take(mc_data::MAX_HOUSES) {
                    let slot = crate::combat::pitch_slot(weapon, w);
                    hp.pose[w] = [
                        s.units.prev_weapon_yaw[row][w].to_radians_f32(),
                        s.units.weapon_yaw[row][w].to_radians_f32(),
                        pitch(s.units.prev_arm_pitch[row][slot]),
                        pitch(s.units.arm_pitch[row][slot]),
                    ];
                    let (now, prev) =
                        barrel_recoil_pair(s.units.weapon_cooldown[row][w], weapon.reload_ticks);
                    hp.kick[2 * w] = prev;
                    hp.kick[2 * w + 1] = now;
                }
                frame.houses.push(hp);
                frame.houses.len() - 1
            });
            let spin = bp
                .weapons
                .iter()
                .find(|w| w.spin_ticks > 0)
                .map_or([0.0; 2], |_| {
                    let [_, turn, step] = s.units.spin[row];
                    let step = step as f32;
                    let now = turn as f32 * (std::f32::consts::TAU / 65536.0);
                    [now - step * (std::f32::consts::TAU / 65536.0), now]
                });
            let (weld, weld_first, weld_count) = weld_on_unit(&weld_range, &frame.welds, weld_id)
                .or_else(|| weld_on_unit(&weld_range, &frame.welds, s.units.id(row).0))
                .unwrap_or(([0.0; 3], 0, 0));
            frame.units.push(UnitInstance {
                prev_pos: {
                    let mut p = s.units.prev_pos[row].extend(s.units.prev_z[row]).to_f32();
                    p[2] += on_deck(row, s.units.prev_pos[row]);
                    p
                },
                prev_heading: s.units.prev_heading[row].to_radians_f32(),
                pos: {
                    let mut p = s.units.pos[row].extend(s.units.z[row]).to_f32();
                    p[2] += on_deck(row, s.units.pos[row]);
                    p
                },
                heading: s.units.heading[row].to_radians_f32(),
                blueprint: s.units.blueprint[row].0 as u32,
                owner_flags: s.units.owner[row] as u32
                    | (flags as u32) << 8
                    | if s.units.order_head[row] == crate::tables::NO_ORDER {
                        STATE_IDLE
                    } else {
                        0
                    }
                    | if self.kit_unpowered(row) {
                        STATE_UNPOWERED
                    } else {
                        0
                    }
                    | if self.kit_charging(row) {
                        STATE_CHARGING
                    } else {
                        0
                    }
                    | contact,
                health: (s.units.health[row]
                    / crate::veterancy_health(bp.health, s.units.veterancy[row]).max(Fx::ONE))
                .to_f32(),
                build,
                turret_yaw: s.units.weapon_yaw[row][0].to_radians_f32(),
                radius: bp.radius.to_f32(),
                unit_id: s.units.id(row).0,
                packed: {
                    let level = s.units.veterancy[row];
                    let need = crate::veterancy_need(level);
                    let share = if level >= crate::VETERANCY_MAX {
                        0.0
                    } else {
                        (s.units.veterancy_progress[row] / need).to_f32()
                    };
                    UnitInstance::pack_veterancy(s.units.kills[row], level, share)
                        | (s.units.fire_state[row] as u32) << UNIT_FIRE_STATE_SHIFT
                        | if s.units.burn_ticks[row] > 0 {
                            UNIT_BURNING
                        } else {
                            0
                        }
                },
                // A core mine's gait is its hammer's beat instead: blows struck, and the share of one
                // this tick added (`mines::hammer_gait`).
                gait: match s
                    .mines
                    .by_unit
                    .get(&s.units.id(row))
                    .filter(|_| bp.mine.is_some())
                {
                    Some(m) if bp.mine.is_some_and(|m| m.hammer) => {
                        crate::mines::hammer_gait(m.age, bp.tech)
                    }
                    // A mine that strikes nothing has no beat.
                    Some(_) => [0.0; 3],
                    None => [
                        (s.units.gait[row] & 0xF_FFFF) as f32 / 256.0,
                        step[0] as f32 / 256.0,
                        step[1] as f32 / 256.0,
                    ],
                },
                upgrade,
                arm_pitch: [
                    pitch(s.units.prev_arm_pitch[row][0]),
                    pitch(s.units.arm_pitch[row][0]),
                    pitch(s.units.prev_arm_pitch[row][1]),
                    pitch(s.units.arm_pitch[row][1]),
                ],
                prev_turret_yaw: s.units.prev_weapon_yaw[row][0].to_radians_f32(),
                weld,
                recoil,
                prev_recoil,
                weld_first,
                weld_count,
                // A siege gun's spade, or a builder's folding gear (`Builder::unfold_ticks`).
                deploy: {
                    let need = self.deploy_span(row);
                    if need == 0 {
                        0.0
                    } else {
                        s.units.deploy[row] as f32 / need as f32
                    }
                },
                prev_deploy: {
                    let need = self.deploy_span(row);
                    if need == 0 {
                        0.0
                    } else {
                        s.units.prev_deploy[row] as f32 / need as f32
                    }
                },
                _pad2: [
                    s.units.prev_bank[row] as f32 * (std::f32::consts::TAU / 65536.0),
                    s.units.bank[row] as f32 * (std::f32::consts::TAU / 65536.0),
                ],
                refit_modules: refit
                    .and_then(|o| self.blueprints.refit_result(bp.id, o.blueprint).ok())
                    .map_or(0, |to| self.blueprints.look(to)),
                status: [
                    s.units.dive[row] as u32
                        | if s.units.dive_goal[row] {
                            UNIT_DIVE_GOAL
                        } else {
                            0
                        }
                        | if s.units.paused[row] { UNIT_PAUSED } else { 0 }
                        | if stored { UNIT_STORED } else { 0 }
                        | self.lift_gear(row) << UNIT_GEAR_SHIFT
                        | if deck_up(row).is_some() {
                            UNIT_ON_DECK
                        } else {
                            0
                        },
                    house.map_or(0, |i| (i as u32 + 1) << UNIT_HOUSE_SHIFT),
                    deck_up(row).unwrap_or_else(|| self.launcher_pad(row)),
                ],
                mount: mounted.map_or([0.0; 4], |w| {
                    let off = |yaw: &[mc_core::Angle; mc_data::MAX_WEAPONS]| pitch(yaw[w] - yaw[0]);
                    [
                        off(&s.units.prev_weapon_yaw[row]),
                        off(&s.units.weapon_yaw[row]),
                        pitch(s.units.prev_arm_pitch[row][2 + w]),
                        pitch(s.units.arm_pitch[row][2 + w]),
                    ]
                }),
                spin_recoil: [spin[0], spin[1], prev_mount_kick, mount_kick],
            });
        }

        frame.projectiles.clear();
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
            let smoke = !weapon.missile && weapon.color == WeaponColor::Orange && weapon.trail;
            let power = weapon.damage.to_f32().sqrt();
            let size = (0.3 + power * 0.045 + weapon.splash.to_f32() * 0.07) * weapon.tracer;
            let plasma = if weapon.plasma > 0.0 {
                (1.35 + power * 0.11 + weapon.splash.to_f32() * 0.04) * weapon.plasma
            } else {
                0.0
            };
            (
                weapon.color as u32
                    | if weapon.missile {
                        PROJECTILE_MISSILE
                    } else {
                        0
                    }
                    | if trail { PROJECTILE_TRAIL } else { 0 }
                    | if smoke { PROJECTILE_SMOKE } else { 0 }
                    | if bomb { PROJECTILE_BOMB } else { 0 }
                    | if weapon.torpedo {
                        PROJECTILE_TORPEDO
                    } else {
                        0
                    }
                    | if weapon.missile && weapon.skim > Fx::ZERO {
                        PROJECTILE_SKIM
                    } else {
                        0
                    }
                    | if weapon.missile && weapon.apogee > Fx::ZERO {
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
                if weapon.rounds > 1 && weapon.color == WeaponColor::Orange {
                    1.0 + weapon.red
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
        let shatter = |blueprint: BlueprintId, weapon: u8| {
            let weapon = &self.blueprints.unit(blueprint).weapons[weapon as usize];
            weapon.hitscan || weapon.sounds.fire.as_deref() == Some("aster_shatter")
        };
        for i in 0..s.projectiles.len() {
            // Shatter guns and hitscan weapons are a fading beam the renderer draws
            // from the fire and impact events, not a traveling slug.
            if shatter(s.projectiles.blueprint[i], s.projectiles.weapon[i]) {
                continue;
            }
            let (color, size, wake, plasma, hot) =
                look(s.projectiles.blueprint[i], s.projectiles.weapon[i]);
            let weapon = &self.blueprints.unit(s.projectiles.blueprint[i]).weapons
                [s.projectiles.weapon[i] as usize];
            // Age has advanced after this step.
            let color = color
                | if weapon.cold_launch_ticks > 0
                    && s.projectiles.age[i] <= weapon.cold_launch_ticks
                {
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
            frame.projectiles.push(ProjectileInstance {
                prev_pos: off(from, age - 1.0),
                color: color | fresh(from),
                pos: off(s.projectiles.pos[i], age),
                size,
                wake,
                plasma,
                _pad: [hot, cruise_wings(weapon, s.projectiles.age[i])],
                aim: nose_pad(cold_body, s.projectiles.aim[i]),
                prev_aim: nose_pad(cold_body, s.projectiles.prev_aim[i]),
            });
        }
        // Shots that landed this tick fly their last stretch, so a shell is seen
        // all the way in, and one fired at point-blank range is seen at all.
        for shot in &self.spent {
            if shatter(shot.blueprint, shot.weapon) {
                continue;
            }
            let (color, size, wake, plasma, hot) = look(shot.blueprint, shot.weapon);
            let ends = ((shot.after.to_f32() * 255.0) as u32).clamp(1, 255);
            let from = shot.from.to_f32();
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
                aim: [0.0; 4],
                prev_aim: [0.0; 4],
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
        let mut grow_beams: Vec<(u32, BeamInstance)> = Vec::new();
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
            if self.grows(row) {
                // A grown site is fed by one writhing tendril from the builder's emitter.
                grow_beams.push((
                    s.units.id(row).0,
                    BeamInstance {
                        from: from.to_f32(),
                        kind: BEAM_GROW,
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
            } else if self.grows(row) {
                // A grown factory has no print guns: its young grow in the pit, heard there.
                let at = s.units.pos[t].extend(s.units.z[t]).to_f32();
                frame.build_sources.push(self.build_source(row, at));
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
        for (source, beam) in grow_beams {
            frame.beam_sources.push(source);
            frame.beams.push(beam);
        }
        // Sites of a faction that grows its buildings rise out of a molten pool.
        for u in frame.units.iter_mut() {
            if u.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST) == 0
                && u.owner_flags & ((crate::tables::flag::UNDER_CONSTRUCTION as u32) << 8) != 0
                && self
                    .blueprints
                    .units
                    .get(u.blueprint as usize)
                    .is_some_and(|b| self.faction_grows(b.faction))
            {
                u.status[1] |= UNIT_GROWN;
            }
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

        // Settled wrecks are salvage to plan around: they show under the fog
        // anywhere the viewer's team has explored, not only in sight, and the
        // map's own wreckage shows from the start, like the map.
        for row in s.wrecks.slots.iter() {
            if let (Some(v), true) = (
                viewer,
                s.fog_enabled && s.wrecks.from_map.get(row) != Some(&true),
            ) {
                if !self.fog.is_explored(s.wrecks.pos[row], self.team_mask(v)) {
                    continue;
                }
            }
            let pos = s.wrecks.pos[row].extend(s.wrecks.z[row]).to_f32();
            let heading = s.wrecks.heading[row].to_radians_f32();
            let bp = self.blueprints.unit(s.wrecks.blueprint[row]);
            frame.units.push(UnitInstance {
                prev_pos: pos,
                prev_heading: heading,
                pos,
                heading,
                blueprint: bp.id.0 as u32,
                owner_flags: KIND_WRECK,
                health: (s.wrecks.mass[row] / s.wrecks.mass_max[row]).to_f32(),
                build: 1.0,
                turret_yaw: 0.0,
                radius: bp.radius.to_f32(),
                unit_id: s.wrecks.slots.handle(row).0,
                packed: 0,
                gait: [0.0; 3],
                upgrade: 0.0,
                arm_pitch: [0.0; 4],
                prev_turret_yaw: 0.0,
                weld: [0.0; 3],
                recoil: 0.0,
                prev_recoil: 0.0,
                weld_first: 0,
                weld_count: 0,
                deploy: 1.0,
                prev_deploy: 1.0,
                // A sunk ship lies on the seabed with the list it went down with.
                _pad2: {
                    let bank = s.wrecks.bank[row] as f32 * (std::f32::consts::TAU / 65536.0);
                    [bank, bank]
                },
                refit_modules: 0,
                status: [0; 3],
                mount: [0.0; 4],
                spin_recoil: [0.0; 4],
            });
        }

        // Spent casings: tumbling in the air, then lying where they came down and sinking
        // away. One in the air names the walker that threw it: the shader carries it with
        // that walker's drawn stride at first, so it leaves the port the gun is drawn at,
        // not the sim's.
        let mut thrower: Option<(u32, u32)> = None;
        for (i, sabot) in s.sabots.iter().enumerate() {
            let from = match thrower {
                Some((id, at)) if id == sabot.source.0 => at,
                _ => {
                    let at = frame
                        .units
                        .iter()
                        .position(|u| {
                            u.unit_id == sabot.source.0
                                && u.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST) == 0
                        })
                        .map_or(0, |at| at as u32 + 1);
                    thrower = Some((sabot.source.0, at));
                    at
                }
            };
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
            if let (Some(v), true) = (viewer, s.fog_enabled) {
                if !self.fog.is_visible(here.xy(), self.team_mask(v)) {
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

        for crash in &s.aircraft_crashes {
            if let (Some(v), true) = (viewer, s.fog_enabled) {
                if !self.fog.is_visible(crash.pos.xy(), self.team_mask(v)) {
                    continue;
                }
            }
            let bp = self.blueprints.unit(crash.blueprint);
            let spin = crash.spin() as f32;
            let bank = crash.bank as f32 * std::f32::consts::TAU / 65536.0;
            let heading = crash.heading.to_radians_f32();
            let tps = TICKS_PER_SECOND as f32;
            // Tumbling as it falls: yaw, pitch and roll after `ticks`, `above` metres over the seabed.
            // Once in the sea the tumble stops where it was; over two seconds the hull turns to
            // hang nose down, and it comes level again over its last few metres to the bottom.
            let pose = |ticks: u16, above: f32| {
                let air = ticks.min(if crash.splashed != 0 {
                    crash.splashed
                } else {
                    u16::MAX
                });
                let t = air as f32 / tps;
                let heft = crate::aircraft_crash::heft(bp.radius).to_f32();
                let (yaw, pitch, roll) = if heft < 1.0 {
                    // A heavy hull leans into its fall and rolls a little, easing
                    // toward a limit rather than tumbling over.
                    let ease =
                        |rate: f32, most: f32| most * (1.0 - (-t * rate * heft / most).exp());
                    (
                        heading + spin * t * 0.3 * heft,
                        -ease(0.75, 0.15 + heft * 1.5),
                        bank + spin * ease(2.4, 0.2 + heft * 2.0),
                    )
                } else {
                    (heading + spin * t * 0.3, -t * 0.75, bank + spin * t * 2.4)
                };
                if crash.splashed == 0 {
                    return (yaw, pitch, roll);
                }
                let under = ticks.saturating_sub(crash.splashed) as f32 / tps;
                let turn = {
                    let u = (under / 2.0).clamp(0.0, 1.0);
                    u * u * (3.0 - 2.0 * u)
                };
                let level = {
                    let u = ((6.0 - above) / 5.0).clamp(0.0, 1.0);
                    u * u * (3.0 - 2.0 * u)
                };
                let tau = std::f32::consts::TAU;
                let nose = (pitch / tau).round() * tau - 0.55 * (1.0 - level);
                let flat = (roll / tau).round() * tau;
                (
                    yaw,
                    pitch + (nose - pitch) * turn,
                    roll + (flat - roll) * turn,
                )
            };
            let floor = crash.floor.to_f32();
            let (prev_yaw, prev_pitch, prev_roll) = pose(
                crash.age.saturating_sub(1),
                crash.prev_pos.z.to_f32() - floor,
            );
            let (yaw, pitch, roll) = pose(crash.age, crash.pos.z.to_f32() - floor);
            frame.units.push(UnitInstance {
                prev_pos: crash.prev_pos.to_f32(),
                pos: crash.pos.to_f32(),
                prev_heading: prev_yaw,
                heading: yaw,
                blueprint: bp.id.0 as u32,
                owner_flags: KIND_WRECK,
                health: 1.0,
                build: 1.0,
                radius: bp.radius.to_f32(),
                unit_id: crash.unit_id,
                packed: WRECK_FALLING,
                // These otherwise unused wreck fields carry the tumble pitch.
                arm_pitch: [prev_pitch, pitch, 0.0, 0.0],
                _pad2: [prev_roll, roll],
                deploy: 1.0,
                prev_deploy: 1.0,
                ..UnitInstance::zeroed()
            });
        }

        for hull in &s.sinking {
            if let (Some(v), true) = (viewer, s.fog_enabled) {
                if !self.fog.is_visible(hull.pos, self.team_mask(v)) {
                    continue;
                }
            }
            let bp = self.blueprints.unit(hull.blueprint);
            let angle = |a: i16| a as f32 * (std::f32::consts::TAU / 65536.0);
            let heading = hull.heading.to_radians_f32();
            frame.units.push(UnitInstance {
                prev_pos: hull.pos.extend(hull.prev_z).to_f32(),
                pos: hull.pos.extend(hull.z).to_f32(),
                prev_heading: heading,
                heading,
                blueprint: bp.id.0 as u32,
                owner_flags: KIND_WRECK,
                health: hull.progress().to_f32(),
                build: 1.0,
                radius: bp.radius.to_f32(),
                unit_id: hull.unit_id,
                packed: WRECK_SINKING,
                arm_pitch: [angle(hull.prev_pitch), angle(hull.pitch), 0.0, 0.0],
                _pad2: [angle(hull.prev_roll), angle(hull.roll)],
                deploy: 1.0,
                prev_deploy: 1.0,
                ..UnitInstance::zeroed()
            });
        }

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
                    | u32::from(self.shields_unpowered(owner)) << 26
                    | if s.units.has_flag(row, crate::tables::flag::INVULNERABLE) {
                        SHIELD_VEIL
                    } else {
                        0
                    },
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
        self.write_strategic(&mut frame.strategic);
        self.write_warhead_plans(
            viewer,
            &mut frame.warhead_tracks,
            &mut frame.planned_launches,
        );

        frame.fog.clear();
        frame.fog_dims = self.fog.dims();
        if let (Some(v), true) = (viewer, s.fog_enabled) {
            let mask = self.team_mask(v);
            let (visible, explored) = (self.fog.visible_cells(), self.fog.explored_cells());
            frame.fog.reserve(visible.len() * 2);
            for i in 0..visible.len() {
                // Radar paints blips, not the ground: only vision lights a cell.
                frame.fog.push(if visible[i] & mask != 0 { 255 } else { 0 });
                frame
                    .fog
                    .push(if explored[i] & mask != 0 { 255 } else { 0 });
            }
        }

        frame.props_dead.clear();
        frame.props_dead.extend(
            s.props_dead
                .iter()
                .flat_map(|w| [*w as u32, (*w >> 32) as u32]),
        );
        frame.terrain_edits.clear();
        frame
            .terrain_edits
            .extend(s.terrain_edits.iter().map(|e| e.record()));
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
        let rest = s.units.slots.iter().filter(|&row| {
            everyone == Some(s.units.owner[row])
                && ((s.units.is_active(row)
                    && self.bp(row).is_mobile()
                    && s.orders.front(&s.units, row).is_some())
                    || !s.units.standing[row].is_empty())
                && !watch.contains(&s.units.id(row).0)
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
                        | OrderKind::Orbit => s.units.row(o.target).map(|r| s.units.pos[r]),
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
                        radius: if o.kind == OrderKind::Orbit && o.radius <= Fx::ZERO {
                            self.bp(row).orbit_radius.to_f32()
                        } else {
                            o.radius.to_f32()
                        },
                    }
                })
                .collect();
            let progress = s.units.row(s.units.build_target[row]).map_or(0.0, |t| {
                (s.units.build_progress[t] / self.bp(t).build_time).to_f32()
            });
            let flow = self.flows.get(row).copied().unwrap_or_default();
            let rate = |v: Fx| (v * TICKS_PER_SECOND as i32).to_f32();
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
                mass_made: rate(flow.made[0]),
                energy_made: rate(flow.made[1]),
                mass_wanted: rate(flow.wanted[0]),
                energy_wanted: rate(flow.wanted[1]),
                mass_used: rate(flow.used[0]),
                energy_used: rate(flow.used[1]),
                efficiency: s.players[s.units.owner[row] as usize].efficiency.to_f32(),
                side_tech: {
                    let owner = s.units.owner[row];
                    let t = &mut side_tech[owner as usize];
                    if *t == 0 {
                        *t = self.side_tech(owner);
                    }
                    *t
                },
                mine: self.mine_view(row),
                mine_veins: self.mine_veins(row),
                hidden_target: self.hidden_target(row).map(|t| {
                    let p = s.units.pos[t].to_f32();
                    [p[0], p[1], (s.units.z[t] + self.bp(t).height / 2).to_f32()]
                }),
                cargo: self.cargo_view(row),
            });
        }
    }

    fn mine_veins(&self, row: usize) -> Vec<MineVein> {
        let Some(spec) = self.bp(row).mine else {
            return Vec::new();
        };
        let Some(m) = self.state.mines.by_unit.get(&self.state.units.id(row)) else {
            return Vec::new();
        };
        let tps = TICKS_PER_SECOND as f32;
        m.veins
            .iter()
            .map(|v| MineVein {
                field: v.field,
                ore: v.ore.to_f32(),
                rate: (v.ore * (spec.per_hectare - spec.ground).max(Fx::ZERO)).to_f32(),
                dig: v.reached_at as f32 / tps,
                eta: v.reached_at.saturating_sub(m.age) as f32 / tps,
            })
            .collect()
    }

    fn mine_view(&self, row: usize) -> Option<MineView> {
        let spec = self.bp(row).mine?;
        let m = self.state.mines.by_unit.get(&self.state.units.id(row))?;
        Some(MineView {
            land: m.land,
            share: m.land.efficiency(&spec).to_f32(),
            rate: m.rate(&spec).to_f32(),
            full: m.full_rate(&spec).to_f32(),
            age: m.age as f32 / TICKS_PER_SECOND as f32,
            spread: m.spread().to_f32(),
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
