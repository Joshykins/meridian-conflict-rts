//! Factions and unit blueprints.
//!
//! Blueprints are authored as RON under `data/factions/<faction>/`. Authoring
//! uses decimal literals; loading converts them once into fixed point, so the
//! simulation only ever sees integers. Decimal parsing and the scale by 2^16
//! are exactly rounded IEEE operations, so every machine compiles the same
//! tables, and [`Blueprints::content_hash`] lets peers verify that before a
//! match starts.

mod raw;
pub mod refit;
pub mod regions;
pub mod sounds;
pub mod strategic;
pub mod survival;
mod weapon;
pub mod weather;

use mc_core::{Angle, Fx, FxVec2, FxVec3, StateHasher};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

pub use raw::{
    AntiMissileLook, BuildSounds, Construction, FactionSounds, IconKind, LineLook, LinePath,
    MoveLayer, PlasmaGrade, PowerLine, ShieldKind, ShieldLook, StructureLamps, TorpedoLook,
    Trajectory, UnitSounds, WeaponColor, WeaponSounds,
};
pub use refit::{Loadout, Module, Refit, RefitSet, RefitSlot, MAX_REFIT_SLOTS};
pub use sounds::{SoundId, SoundLibrary};
pub use weapon::{HowitzerLook, Weapon, BOMBARD_RADIUS, MAX_CLUSTER};

#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Debug,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct BlueprintId(pub u16);

#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Debug,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct FactionId(pub u8);

impl BlueprintId {
    #[inline]
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// Category bits. Weapons pick targets by mask, build menus and the AI filter by them.
pub mod cat {
    pub const MOBILE: u32 = 1 << 0;
    pub const STRUCTURE: u32 = 1 << 1;
    pub const LAND: u32 = 1 << 2;
    pub const AIR: u32 = 1 << 3;
    pub const NAVAL: u32 = 1 << 4;
    pub const COMMANDER: u32 = 1 << 5;
    pub const ENGINEER: u32 = 1 << 6;
    pub const FACTORY: u32 = 1 << 7;
    pub const ECONOMY: u32 = 1 << 8;
    pub const DEFENSE: u32 = 1 << 9;
    pub const DIRECT_FIRE: u32 = 1 << 10;
    pub const ARTILLERY: u32 = 1 << 11;
    pub const ANTI_AIR: u32 = 1 << 12;
    pub const SCOUT: u32 = 1 << 13;
    pub const WALL: u32 = 1 << 14;
    pub const EXTRACTOR: u32 = 1 << 15;
    pub const POWER: u32 = 1 << 16;
    pub const STORAGE: u32 = 1 << 17;
    pub const INTEL: u32 = 1 << 18;
    pub const EXPERIMENTAL: u32 = 1 << 19;
    pub const SHIELD: u32 = 1 << 20;
    /// Survival's hostile machinery (the Replication Engine and its nodes): never built by anyone.
    pub const REPLICATOR: u32 = 1 << 21;
    /// Spacecraft roster tag; atmospheric spacecraft retain AIR for movement and targeting.
    pub const SPACE: u32 = 1 << 22;
    /// Strategic launchers (the nuclear silo, the interceptor array): never picked by a job
    /// that asks for anything else.
    pub const STRATEGIC: u32 = 1 << 23;

    pub(crate) fn parse(name: &str) -> Option<u32> {
        Some(match name {
            "Mobile" => MOBILE,
            "Structure" => STRUCTURE,
            "Land" => LAND,
            "Air" => AIR,
            "Naval" => NAVAL,
            "Commander" => COMMANDER,
            "Engineer" => ENGINEER,
            "Factory" => FACTORY,
            "Economy" => ECONOMY,
            "Defense" => DEFENSE,
            "DirectFire" => DIRECT_FIRE,
            "Artillery" => ARTILLERY,
            "AntiAir" => ANTI_AIR,
            "Scout" => SCOUT,
            "Wall" => WALL,
            "Extractor" => EXTRACTOR,
            "Power" => POWER,
            "Storage" => STORAGE,
            "Intel" => INTEL,
            "Experimental" => EXPERIMENTAL,
            "Shield" => SHIELD,
            "Replicator" => REPLICATOR,
            "Space" => SPACE,
            "Strategic" => STRATEGIC,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Faction {
    pub id: FactionId,
    /// Name used in code, asset names and short labels: "Aster".
    pub key: String,
    /// Full name: "Asterian Reach Command".
    pub name: String,
    pub abbreviation: String,
    pub description: String,
    /// The unit a player of this faction starts with.
    pub commander: BlueprintId,
    /// The faction whose roster it fields while it has none of its own.
    pub stand_in: Option<FactionId>,
    /// Linear RGB, presentation only.
    pub plating_color: [f32; 3],
    pub accent_color: [f32; 3],
    pub highlight_color: [f32; 3],
    /// Its adjacency conduits: their energy colour (also the interface's marks for them),
    /// their path and their look (`Blueprints::power_line`).
    pub power_line: PowerLine,
    /// Its shield fields' idle colour; hits and seams are drawn from it.
    pub shield_color: [f32; 3],
    /// How its construction sites look. Presentation only, so not in the content hash.
    pub construction: Construction,
    /// Its own selection answers and building sounds. Presentation only.
    pub sounds: FactionSounds,
    /// How its torpedoes look running. Presentation only.
    pub torpedo_look: TorpedoLook,
    /// How its structures are lit at night. Presentation only.
    pub structure_lamps: StructureLamps,
    /// How its shield fields look. Presentation only.
    pub shield_look: ShieldLook,
    /// How its missile defence is seen killing a missile. Presentation only.
    pub anti_missile_look: AntiMissileLook,
    /// How its strategic missiles and nuclear blasts look and sound. Presentation only.
    pub nuke_look: strategic::StrategicLook,
}

#[derive(Clone, Copy, Debug)]
pub struct Motion {
    pub layer: MoveLayer,
    /// 0..=5, see `mc-path`.
    pub size_class: u8,
    /// Metres per second.
    pub speed: Fx,
    /// Metres per second squared.
    pub accel: Fx,
    /// Angle steps per tick.
    pub turn_rate: u16,
    /// Cruise height above the surface. Zero unless `layer` is `Air`.
    pub altitude: Fx,
    pub hover: bool,
    /// Ticks to plant or pack. Zero: it fires on the move.
    pub deploy_ticks: u16,
    /// Engaged and stopped, the hull lays the mark this far off the bow (`RawMotion::broadside`).
    /// Zero: no broadside.
    pub broadside: Angle,
    /// Half the arc across the nose its main turret reaches (`RawMotion::aim_arc`); past
    /// it the body turns. 0x8000: all round.
    pub aim_arc: u16,
    /// Strides straight over structures, slopes and shallow water (`RawMotion::stride`).
    pub stride: bool,
    /// Hangs still near what it fights instead of circling it (`RawMotion::hangs`).
    pub hangs: bool,
    /// Cruises above the weather: reached only along the line of sight
    /// (`RawMotion::above_weather`).
    pub above_weather: bool,
    /// Fights from a circle near its reach instead of making runs (`RawMotion::stand_off`).
    pub stand_off: bool,
}

/// A giant walker's crushing footfall (`RawStomp`).
#[derive(Clone, Copy, Debug)]
pub struct Stomp {
    pub pace: Fx,
    pub reach: Fx,
    pub gauge: Fx,
    pub radius: Fx,
    pub damage: Fx,
}

/// An Argon Electric Bore's discharge (`Weapon::bore`): where the shot lands (at once for a
/// `hitscan` bore), the charge runs down the channel from the muzzle. Everything within `width` of it
/// takes `damage` (zero width: only the bolt; the blast is the weapon's own splash), and
/// the ground under it is left molten for `cool` seconds (cosmetic). A `blast` raises a
/// fireball of ionised air that wide where it lands, burning `blast_time` seconds
/// (cosmetic).
#[derive(Clone, Copy, Debug)]
pub struct Bore {
    pub width: Fx,
    pub damage: Fx,
    pub cool: f32,
    pub blast: f32,
    pub blast_time: f32,
    pub storm: Option<Storm>,
}

/// A cone weapon's fan (`RawCone`): `half` either side of the gun's facing; a unit at full
/// range takes `edge` of the damage, one at the muzzle all of it. Its front rolls out
/// `speed` metres a second.
#[derive(Clone, Copy, Debug)]
pub struct Cone {
    pub half: Angle,
    pub edge: Fx,
    pub speed: Fx,
}

/// A gun's spent casing (`RawSabot`).
#[derive(Clone, Copy, Debug)]
pub struct Sabot {
    /// The ejection port, in the gun's frame like `Weapon::muzzle`.
    pub port: FxVec3,
    /// Which way it is thrown, a unit vector in the same frame, and how fast (m/s).
    pub throw: FxVec3,
    pub kick: Fx,
    pub damage: Fx,
    pub splash: Fx,
    /// The hidden blueprint it is drawn as.
    pub casing: BlueprintId,
}

/// A cluster shot's split (`RawCluster`): how many sub-shots, over how wide a disc, how
/// high over its mark it breaks, and each sub-shot's blast.
#[derive(Clone, Copy, Debug)]
pub struct Cluster {
    pub count: u8,
    pub radius: Fx,
    pub height: Fx,
    pub splash: Fx,
}

/// A giant bore's lightning storm (`RawStorm`): grows over `ticks` to `radius`, `damage`
/// a second at its heart.
#[derive(Clone, Copy, Debug)]
pub struct Storm {
    pub radius: Fx,
    pub ticks: u16,
    pub damage: Fx,
}

/// The hull shots strike on a long unit, in its own frame (x along its heading): a
/// capsule from `aft` to `fore` metres along x, `beam` metres in half width, its ends
/// rounded. A 570 m warship tested as a disc of its radius took shots 200 m off its flanks.
#[derive(Clone, Copy, Debug)]
pub struct Body {
    pub fore: Fx,
    pub aft: Fx,
    pub beam: Fx,
}

/// A submarine's dive: how deep its deck goes and how long the trip takes.
#[derive(Clone, Copy, Debug)]
pub struct Dive {
    /// Metres of water over the hull's top when dived.
    pub depth: Fx,
    /// Ticks to dive or to surface.
    pub ticks: u16,
    /// Ordered down, it still comes up while a gun that works only surfaced has a mark,
    /// and goes back down when none has (`naval.rs` `run_dive`): it hides dived and
    /// surfaces to fight. Its marks are picked dived, as if it were up.
    pub ambush: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Economy {
    /// Per second.
    pub mass_income: Fx,
    pub energy_income: Fx,
    /// Per second, drawn while the unit is active.
    pub energy_upkeep: Fx,
    pub mass_storage: Fx,
    pub energy_storage: Fx,
}

/// A core mine. Every patch of land within its reach is worth materials a
/// second, ore much more than bare ground; where mines' reaches overlap the
/// ground is divided between them. Higher tiers get more out of each hectare.
/// Mines stand on land only.
#[derive(Clone, Copy, Debug)]
pub struct Mine {
    /// Metres around itself it mines, on land.
    pub reach: Fx,
    /// Materials per second per hectare of land in its territory.
    pub ground: Fx,
    /// Materials per second per hectare of ore in its territory (instead of `ground` there).
    pub per_hectare: Fx,
    /// Materials per second from the shaft itself, from the moment it is finished,
    /// whatever its territory: a new mine pays at once while its land spreads out.
    pub base: Fx,
    /// Strikes a pile hammer in a beat (`RawMine::hammer`). Presentation only.
    pub hammer: bool,
}

/// A material fabricator (MFE): materials a second made out of energy. It makes
/// them only as far as its `energy_upkeep` is paid, with nothing at all left over
/// in a full stall, unlike a mine.
#[derive(Clone, Copy, Debug)]
pub struct Fabricator {
    /// Materials per second with its upkeep paid in full.
    pub mass: Fx,
}

/// A provider's adjacency bonus: what it saves each finished building of its owner's
/// whose lot shares an edge with its own (`mc_sim::adjacency`). A reactor saves the
/// energy its neighbours use (upkeep, and what a factory builds), a fabricator the
/// materials a neighbouring factory builds with.
///
/// Each figure is what the provider would save a building it rings all the way round:
/// a neighbour gets it in proportion to how much of its perimeter the two share, so
/// the saving grows with every side covered until the building is ringed. It scales
/// with what the provider makes a second (`ring`): the data says only which resource.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Adjacency {
    /// Share of a ringed neighbour's energy use it saves.
    pub energy: Fx,
    /// Share of a ringed neighbouring factory's materials it saves.
    pub mass: Fx,
}

impl Adjacency {
    /// Energy: a ring of 2000/s reactors (Reactor III) saves 60%.
    pub const ENERGY_RING: (Fx, Fx) = (Fx::ratio(3, 5), Fx::from_int(2000));
    /// Materials: a ring of 5/s fabricators (Fabricator III) saves 40%.
    pub const MASS_RING: (Fx, Fx) = (Fx::ratio(2, 5), Fx::from_int(5));

    /// What a ring of providers making `made` a second saves, against the `(share, made)`
    /// reference: by the fourth root of the output, so a bigger plant saves more but
    /// a small one is still worth building against (energy 15/s 18%, 350/s 39%).
    pub fn ring((top, reference): (Fx, Fx), made: Fx) -> Fx {
        top * (made / reference).sqrt().sqrt()
    }
}

/// What a volatile unit does when it is destroyed: a blast that hurts every unit
/// near it, its owner's too, so a row of them can go up one after another.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeathBlast {
    /// Metres out from its centre the blast reaches.
    pub radius: Fx,
    /// Damage out to half the radius; it falls to a quarter of this at the edge.
    pub damage: Fx,
}

impl DeathBlast {
    /// Share of `damage` a unit takes `distance` metres from the centre.
    pub fn falloff(&self, distance: Fx) -> Fx {
        let half = self.radius / 2;
        if distance <= half || half <= Fx::ZERO {
            return Fx::ONE;
        }
        let over = ((distance - half) / half).min(Fx::ONE);
        Fx::ONE - over * Fx::ratio(3, 4)
    }
}

#[derive(Clone, Debug)]
pub struct Builder {
    /// Build time units contributed per second.
    pub power: Fx,
    pub range: Fx,
    pub builds: Vec<BlueprintId>,
    /// Set when the unit builds with an arm on its turret: it has to turn to
    /// face its work, and its weapons hold fire while it does.
    pub arm: Option<BuildArm>,
    /// More beams, from these points on the turret (they turn with it, and do not pitch).
    pub emitters: Vec<FxVec3>,
    /// Ticks the gear carrying `emitters` takes to unfold; zero: always out.
    pub unfold_ticks: u16,
    /// Where that gear pitches (by the build arm's pitch) to point at the work. `None`: fixed.
    pub hinge: Option<FxVec3>,
}

#[derive(Clone, Copy, Debug)]
pub struct BuildArm {
    /// Angle steps per tick the turret turns toward the work.
    pub turn: u16,
    /// Where the construction beam leaves the model, like a weapon's `muzzle`.
    pub emitter: FxVec3,
    /// The elbow the arm pitches about to point up or down at its work. `None`: it only turns.
    pub pivot: Option<FxVec3>,
    /// Shoulder of a two-bone arm. The boom (`arm_pitch[0]`) folds about it and
    /// carries the forearm. `None`: a single bone, `rest` is the forearm pitch.
    pub shoulder: Option<FxVec3>,
    /// Pitch the arm sits at when it is not working. Zero: level, pointing forward.
    /// With a `shoulder`, this is the boom's folded-up pitch; the forearm comes level.
    pub rest: Angle,
}

/// Most reclaim heads one unit carries (`Reclaimer::heads`).
pub const MAX_RECLAIM_HEADS: usize = 4;

/// Takes things apart at a distance without being a builder: a reclaim tower, a salvage
/// vehicle, boat or aircraft, a carrier's drone. Left alone it clears the wrecks within
/// `range`; it takes orders for the rest.
///
/// It works with one or more heads. Head `i` is posed as a gun in a house of its own
/// (`Weapon::mount`) in weapon slot `i`: it turns by `weapon_yaw[i]` off the hull's
/// heading about its `pivot`, and pitches by `arm_pitch[2 + i]` about the same point. A
/// unit with a reclaimer carries no weapons, so the slots are the heads'.
#[derive(Clone, Copy, Debug)]
pub struct Reclaimer {
    /// Build time units undone per second, and mass per second out of a wreck, for the
    /// whole unit: this tick's share is split evenly across the heads that are working.
    pub power: Fx,
    /// Horizontal reach from the unit's middle to the edge of its work: a wreck deep
    /// under a boat or far below an aircraft is within reach once it is this near across the map.
    pub range: Fx,
    /// Ticks a head must stay on a target before its beam comes on. Zero: it fires as it aims.
    pub charge_ticks: u16,
    /// It keeps clearing the wrecks within reach while it moves or patrols, without
    /// stopping for them (the Reclaimer). A tower never moves.
    pub mobile: bool,
    heads: [ReclaimHead; MAX_RECLAIM_HEADS],
    head_count: u8,
}

/// One head of a [`Reclaimer`]: a turret of its own on the hull.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReclaimHead {
    /// Where the beam leaves the model with the head at rest (turned to the nose, level),
    /// like a weapon's `muzzle`.
    pub emitter: FxVec3,
    /// The head's pivot: its yaw axis and pitch trunnion, on the hull. `None`: it turns
    /// about the unit's middle (`UnitBlueprint::turret_at`) and does not pitch.
    pub pivot: Option<FxVec3>,
    /// Angle steps per tick it turns (and pitches at half of) toward its work. Zero: fixed.
    pub turn: u16,
    /// How far it pitches down (negative) and up.
    pub pitch_min: Angle,
    pub pitch_max: Angle,
}

impl Reclaimer {
    /// A reclaimer with `heads` (1 to [`MAX_RECLAIM_HEADS`]).
    pub fn new(
        power: Fx,
        range: Fx,
        charge_ticks: u16,
        mobile: bool,
        heads: &[ReclaimHead],
    ) -> Self {
        assert!((1..=MAX_RECLAIM_HEADS).contains(&heads.len()));
        let mut all = [ReclaimHead::default(); MAX_RECLAIM_HEADS];
        all[..heads.len()].copy_from_slice(heads);
        Reclaimer {
            power,
            range,
            charge_ticks,
            mobile,
            heads: all,
            head_count: heads.len() as u8,
        }
    }

    pub fn heads(&self) -> &[ReclaimHead] {
        &self.heads[..self.head_count as usize]
    }
}

/// A lift ship: it sets down, lowers a ramp beneath its belly, and land units walk up it into
/// the hold and back down it (`mc_sim::transport`). In the model's frame, x along the heading.
#[derive(Clone, Copy, Debug)]
pub struct Transport {
    /// Room in the hold; a unit takes `cargo_room` of it.
    pub capacity: u16,
    /// Metres per second it climbs and comes down at.
    pub descent: Fx,
    /// The far end of the hold: where cargo is stowed and let out from.
    pub hold: FxVec2,
    /// x of the ramp's hinge, at the back of the hold, and of its lip on the ground.
    pub hinge: Fx,
    pub lip: Fx,
    /// Metres across the ramp and the hold.
    pub width: Fx,
    /// Height of the hold floor over the ground once it has set down.
    pub floor: Fx,
    /// Clear height inside the vehicle hangar.
    pub clearance: Fx,
    /// Ticks between units leaving down the ramp.
    pub unload_ticks: u16,
}

/// A capital ship's warp drive (`mc_sim::warp`): it spools up on the spot, turns its nose to
/// where it is going, drops out of the world and comes out again wherever it was sent.
#[derive(Clone, Copy, Debug)]
pub struct Warp {
    /// Energy a jump's charge takes for each kilometre from the ship to where it comes out
    /// (`Warp::charge`), drawn over `Warp::charge_ticks` while the grid can pay (slower
    /// while it cannot).
    pub per_km: Fx,
    /// The base of the charge's time: a jump of `km` kilometres charges for
    /// `spool_ticks * (10 + km) / 10` ticks at full power (`Warp::charge_ticks`). It jumps
    /// once charged and its nose is on the mark.
    pub spool_ticks: u16,
    /// Ticks after coming out before the drive can spool again.
    pub cooldown_ticks: u16,
    /// Metres a tick the ship covers in warp: the transit lasts as long as the jump is far.
    pub speed: Fx,
}

impl Warp {
    /// Kilometres a jump of `distance` metres is priced at: never less than one.
    pub fn km(distance: Fx) -> Fx {
        (distance / 1000).max(Fx::ONE)
    }

    /// Energy the charge for a jump of `distance` metres takes: `per_km` for each
    /// kilometre, and at least one kilometre's.
    pub fn charge(&self, distance: Fx) -> Fx {
        self.per_km * Self::km(distance)
    }

    /// Ticks the charge for a jump of `distance` metres takes at full power: `spool_ticks`
    /// stretched by a tenth for each kilometre, `spool_ticks * (10 + km) / 10`.
    pub fn charge_ticks(&self, distance: Fx) -> u16 {
        let stretch = Fx::from_int(10) + Self::km(distance);
        let ticks = Fx::from_int(self.spool_ticks as i32) * stretch / 10;
        ticks.ceil_int().clamp(1, u16::MAX as i32) as u16
    }
}

/// A warp dampener (`mc_sim::warp`): an enemy ship whose jump ends inside `radius` of it is
/// dragged through a slow, torn warp and thrown out hurt and stunned.
#[derive(Clone, Copy, Debug)]
pub struct WarpDamper {
    pub radius: Fx,
    /// How many times longer a dampened transit lasts.
    pub drag: Fx,
    /// Share of its full health a dampened ship loses as it comes out.
    pub damage: Fx,
    /// Ticks a dampened ship lies stunned after coming out.
    pub stun_ticks: u16,
}

/// The authored unit a compiled blueprint came from: itself, or the unit a loadout or kit is of.
fn refits_base(refits: &[RefitSet], bp: &UnitBlueprint) -> BlueprintId {
    match bp.refit {
        Some(Refit::Loadout(l)) => refits[l.set as usize].base,
        Some(Refit::Kit { set, .. }) => refits[set as usize].base,
        None => bp.id,
    }
}

/// Metres of air between a hull field and the collision hull.
pub const HULL_SHIELD_PAD: f64 = 1.6;
/// How high above an Aegis pad the launch beam is born, metres: inside the crystal.
pub const SHIELD_PROJECTOR_HEIGHT: f32 = 16.0;

/// A dome is a flattened spheroid, not a hemisphere: a small one is round, a big one
/// spreads wide at about the same height (Aegis 115 m -> 74 m tall, Aegis II 205 m ->
/// 96 m). Its vertical semi-axis for a dome of `radius`. shields.wgsl `dome_height`
/// must match.
pub fn dome_height(radius: Fx) -> Fx {
    radius.min(Fx::from_int(DOME_HEIGHT_BASE) + radius / DOME_HEIGHT_DIV)
}

/// [`dome_height`] for the renderer.
pub fn dome_height_f32(radius: f32) -> f32 {
    radius.min(DOME_HEIGHT_BASE as f32 + radius / DOME_HEIGHT_DIV as f32)
}

const DOME_HEIGHT_BASE: i32 = 45;
const DOME_HEIGHT_DIV: i32 = 4;

/// A bubble that stops incoming fire before it reaches the units under it.
#[derive(Clone, Copy, Debug)]
pub struct Shield {
    pub kind: ShieldKind,
    /// Dome radius, metres, or the horizontal wrap of a hull field.
    pub radius: Fx,
    /// Hit points the bubble can take.
    pub health: Fx,
    /// Hit points recovered per second while the bubble is up, and while it
    /// fills after a break. Engineers can raise the live rate; a shattered
    /// dome fills at this rate only, and stays down until it is full.
    pub regen: Fx,
}

impl Shield {
    #[inline]
    pub fn is_hull(self) -> bool {
        self.kind == ShieldKind::Hull
    }

    #[inline]
    pub fn is_dome(self) -> bool {
        self.kind == ShieldKind::Dome
    }
}

/// Per-blueprint presentation controls. These never alter combat damage.
#[derive(Clone, Copy, Debug, serde::Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct EffectSettings {
    /// Opacity multiplier; dust_opacity is the explicit RON spelling.
    #[serde(alias = "dust_opacity")]
    pub dust_visibility: f32,
    /// Optional linear RGB base color for dust and gun/blast smoke.
    pub dust_color: Option<[f32; 3]>,
    /// Lighting multiplier, independent of opacity and lifetime.
    pub dust_brightness: f32,
    pub dust_lifetime: f32,
    /// Linear RGB, 0..1; None preserves the weapon's natural pressure-wave tint.
    pub shockwave_color: Option<[f32; 3]>,
    /// Metres above the unit's feet where its dome's shaft is born, inside its
    /// crystal. `None`: an Aegis pad's (`SHIELD_PROJECTOR_HEIGHT`).
    pub shield_projector: Option<f32>,
}
impl Default for EffectSettings {
    fn default() -> Self {
        Self {
            dust_visibility: 1.0,
            dust_color: None,
            dust_brightness: 1.0,
            dust_lifetime: 1.0,
            shockwave_color: None,
            shield_projector: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Visual {
    pub effects: EffectSettings,
    pub mesh: String,
    pub icon: IconKind,
    /// Lamps on the hull. `None`: the renderer's usual set for the kind of unit
    /// (headlights on vehicles, floodlights on structures); `Some([])`: none.
    pub lights: Option<Vec<LightMount>>,
}

/// How a lamp throws its light.
#[derive(Clone, Copy, Debug, serde::Deserialize, PartialEq, Eq)]
pub enum LampKind {
    /// Every way at once: a yard lamp, a glowing vent.
    Point,
    /// One cone along `aim`: a floodlight, a searchlight.
    Spot,
    /// A pair of cones side by side along `aim`, `spread` metres apart: a vehicle's headlights.
    Headlights,
}

/// One lamp on a unit or structure. Cosmetic: never in the content hash.
#[derive(Clone, Copy, Debug, serde::Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct LightMount {
    pub kind: LampKind,
    /// Hull-local metres: x forward, y left, z up from the ground.
    pub at: [f32; 3],
    /// Hull-local direction the lamp points (spot and headlights).
    pub aim: [f32; 3],
    /// Linear RGB, about one.
    pub color: [f32; 3],
    /// Light at one metre, in the scene's sun units (the sun is about 3).
    pub intensity: f32,
    /// Metres at which it has faded to nothing.
    pub range: f32,
    /// Half-angle of the cone in degrees (spot and headlights).
    pub cone: f32,
    /// Metres between the two lamps of a pair (headlights).
    pub spread: f32,
    /// Only after dusk. A lamp that is part of the machine's work (a forge mouth) sets this false.
    pub night_only: bool,
}

impl Default for LightMount {
    fn default() -> Self {
        Self {
            kind: LampKind::Point,
            at: [0.0, 0.0, 2.0],
            aim: [1.0, 0.0, -0.25],
            color: [1.0, 0.86, 0.66],
            intensity: 400.0,
            range: 30.0,
            cone: 30.0,
            spread: 1.6,
            night_only: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct UnitBlueprint {
    pub id: BlueprintId,
    pub key: String,
    pub name: String,
    pub role: String,
    /// What it is in a word or two ("Land Factory"): the build tile's title, its name
    /// under it. None: the name alone, for a unit too much its own to sum up (Resolute).
    pub title: Option<String>,
    pub faction: FactionId,
    pub tech: u8,
    pub categories: u32,
    pub health: Fx,
    /// Health per second.
    pub regen: Fx,
    pub cost_mass: Fx,
    pub cost_energy: Fx,
    /// Build time units; divide by build power for seconds.
    pub build_time: Fx,
    /// Collision radius on the ground plane.
    pub radius: Fx,
    pub height: Fx,
    /// Structure size in build cells. `(0, 0)` for mobile units.
    pub footprint: (u8, u8),
    /// Half-extents, metres, of the ground a structure keeps units off, in its own
    /// frame (x along its heading). The rest of the lot is paved apron units walk on.
    pub hull: (Fx, Fx),
    /// The hull shots strike, when a disc of `radius` misfits it; `None` is that disc.
    pub body: Option<Body>,
    pub vision: Fx,
    pub radar: Fx,
    /// Sonar reach: finds submerged hulls, which vision and radar cannot.
    pub sonar: Fx,
    /// A submarine's dive.
    pub dive: Option<Dive>,
    pub water_build: bool,
    /// Stands on the seabed under deep water (`RawUnit::seabed`).
    pub seabed: bool,
    pub drone: Option<BlueprintId>,
    pub drone_radius: Fx,
    /// Where its drones sit when home, in the turret's frame: one drone for each.
    pub drone_sockets: Vec<FxVec3>,
    /// How far above (positive) or below (negative) its socket a drone lines up to
    /// dock, and drops to letting go.
    pub drone_approach: Fx,
    /// It orbits: its guard order is an Orbit, flown on the whole ring (other aircraft
    /// fly half way out), and left with no orders it circles where it is at this radius.
    pub orbit: Option<Fx>,
    pub anti_missile: Fx,
    /// Anti-missile laser emitters in the hull's frame; empty: the unit's middle.
    pub anti_missile_mounts: Vec<FxVec3>,
    /// Missiles its lasers burn at once, each from a different mount; at least one.
    pub anti_missile_lasers: u8,
    /// Where its turret turns in the hull's plane: muzzles and build emitters swing about
    /// it (`UnitBlueprint::turret_point`). None: the unit's middle.
    pub turret_at: Option<FxVec2>,
    /// Its `mount` guns are houses on the hull, each turning about its own pivot, as a
    /// ship's are, rather than shoulder guns riding the torso.
    pub hull_mounts: bool,
    pub stomp: Option<Stomp>,
    pub motion: Option<Motion>,
    pub economy: Economy,
    /// A core mine: makes materials out of the ground around it.
    pub mine: Option<Mine>,
    /// A material fabricator: turns its energy upkeep into a trickle of materials.
    pub fabricator: Option<Fabricator>,
    /// A volatile unit: the blast it makes when it is destroyed.
    pub death_blast: Option<DeathBlast>,
    /// A provider: what it saves the buildings it stands against.
    pub adjacency: Option<Adjacency>,
    /// A strategic launcher: assembles and holds nuclear warheads or interceptors.
    pub strategic: Option<strategic::Strategic>,
    pub builder: Option<Builder>,
    pub reclaimer: Option<Reclaimer>,
    /// A lift ship: carries land units in its hold.
    pub transport: Option<Transport>,
    /// A capital ship's warp drive.
    pub warp: Option<Warp>,
    /// A structure that drags enemy warps down around it.
    pub warp_damper: Option<WarpDamper>,
    /// A projected dome, or a hull wrap that only covers this unit.
    pub shield: Option<Shield>,
    pub upgrades_to: Option<BlueprintId>,
    /// Build power a unit that builds nothing puts its own upgrade on with (a
    /// Reclaimer). `None`: the caller's fallback.
    pub upgrade_power: Option<Fx>,
    pub weapons: Vec<Weapon>,
    /// Share of `cost_mass` left in the wreck.
    pub wreck_fraction: Fx,
    pub visual: Visual,
    /// Names from the sound library.
    pub sounds: UnitSounds,
    /// Background text for the interface, from the faction's `lore.ron`. Empty when it has none.
    /// Cosmetic: not in the content hash.
    pub lore: String,
    /// Set on a unit with refit slots (`Blueprints::refits`): what it has fitted,
    /// or, on a kit, the module it assembles.
    pub refit: Option<Refit>,
    /// Scrap a giant gun throws out (`Weapon::sabot`): it only ever lies about as a wreck,
    /// so it is in no list or menu.
    pub scrap: bool,
    /// Some unit's salvage drone (`drone`): it flies for its carrier, is neither picked
    /// nor selected on its own, and is never ordered.
    pub carried_drone: bool,
}

impl UnitBlueprint {
    /// Where a point on the turret (`at`, the model's plane) is off the unit's position,
    /// the hull facing `heading` and the turret `facing`: it swings about `turret_at`.
    pub fn turret_point(&self, at: FxVec2, heading: Angle, facing: Angle) -> FxVec2 {
        match self.turret_at {
            Some(p) => p.rotate(heading) + (at - p).rotate(facing),
            None => at.rotate(facing),
        }
    }

    /// Production categories (e.g. an Air factory) are not physical target layers.
    pub fn target_categories(&self) -> u32 {
        if self.motion.is_some_and(|m| m.layer == MoveLayer::Air) {
            self.categories
        } else {
            self.categories & !cat::AIR
        }
    }

    pub fn can_attack(&self, target: &UnitBlueprint) -> bool {
        self.weapons
            .iter()
            .any(|w| w.target_mask & target.target_categories() != 0)
    }

    #[inline]
    pub fn has(&self, categories: u32) -> bool {
        self.categories & categories == categories
    }

    /// Blows up when destroyed, hurting everything near it.
    #[inline]
    pub fn volatile(&self) -> bool {
        self.death_blast.is_some()
    }

    #[inline]
    pub fn is_structure(&self) -> bool {
        self.categories & cat::STRUCTURE != 0
    }

    /// Stands on a poured concrete lot: every structure but walls, the Precursor machines
    /// of survival, which hover over bare ground, and scrap (a spent casing), which is
    /// never built.
    #[inline]
    pub fn poured_lot(&self) -> bool {
        self.is_structure() && self.categories & (cat::WALL | cat::REPLICATOR) == 0 && !self.scrap
    }

    #[inline]
    pub fn is_mobile(&self) -> bool {
        self.motion.is_some()
    }

    /// Raised by builders on a lot of its own, as a structure is: every structure, and
    /// a mobile unit with a `footprint` (an experimental too big for any factory), which
    /// drives off its lot once it is finished.
    #[inline]
    pub fn built_on_site(&self) -> bool {
        self.is_structure() || self.footprint != (0, 0)
    }

    /// A mobile unit raised on a lot of its own (`built_on_site`).
    #[inline]
    pub fn is_site_built_unit(&self) -> bool {
        self.is_mobile() && self.footprint != (0, 0)
    }

    /// A capital spacecraft (`Space`, or any lift ship): it keeps station and lets its
    /// turrets cover every side, so it never wheels its hull about a target.
    pub fn is_capital_ship(&self) -> bool {
        self.has(cat::SPACE) || self.transport.is_some()
    }

    /// Which way it faces when raised on its lot. Structures face south. A unit
    /// built on a lot (a lift ship) lies along the lot's long side, nose east
    /// when the lot is wider than deep, so its hull and the lot agree.
    pub fn build_heading(&self) -> Angle {
        if !self.is_site_built_unit() {
            Angle::from_degrees(270)
        } else if self.footprint.0 >= self.footprint.1 {
            Angle::ZERO
        } else {
            Angle::from_degrees(90)
        }
    }

    /// Room in a transport: a commander takes eight slots; other land units take their size class plus one.
    /// `None` for what cannot ride in one: anything that is not a land unit.
    pub fn cargo_room(&self) -> Option<u16> {
        let m = self.motion?;
        (matches!(
            m.layer,
            MoveLayer::Land | MoveLayer::Amphibious | MoveLayer::Hover
        ) && self.transport.is_none())
        .then_some(if self.has(cat::COMMANDER) {
            8
        } else {
            m.size_class as u16 + 1
        })
    }

    /// A structure that stands only on open water (a naval yard): water-built and
    /// naval, not land. Water-built defences that are also land stand on either.
    #[inline]
    pub fn water_only(&self) -> bool {
        self.water_build && self.has(cat::NAVAL) && !self.has(cat::LAND)
    }

    /// `(power, range)` of what this unit reclaims with: a reclaimer, or the
    /// tools of a builder that can walk up to its work. Factories have neither.
    pub fn reclaims(&self) -> Option<(Fx, Fx)> {
        match (&self.reclaimer, &self.builder) {
            (Some(r), _) => Some((r.power, r.range)),
            (None, Some(b)) if self.is_mobile() => Some((b.power, b.range)),
            _ => None,
        }
    }

    /// This unit reclaims (a commander's drone port works beside its own tools).
    pub fn sends_reclaimers(&self) -> bool {
        self.reclaims().is_some()
    }

    /// Its drones are its shells (`Weapon::launches`, the Quiver's Wicks): it fires them,
    /// and they reclaim nothing.
    pub fn launches_drones(&self) -> bool {
        self.weapons.iter().any(|w| w.launches)
    }

    /// A salvage unit the factories make: mobile, unarmed, and it reclaims on its own
    /// or with drones. Not an engineer (it builds nothing). The Argus (radar) has a job
    /// of its own and is not one.
    pub fn is_salvager(&self) -> bool {
        self.is_mobile()
            && self.weapons.is_empty()
            && self.radar == Fx::ZERO
            && self.reclaimer.is_some_and(|r| r.mobile)
    }

    pub fn max_weapon_range(&self) -> Fx {
        self.weapons
            .iter()
            .map(|w| w.range_max)
            .max()
            .unwrap_or(Fx::ZERO)
    }
}

/// Most weapons one unit can carry. The sim stores weapon state in fixed slots.
pub const MAX_WEAPONS: usize = 10;
/// Most drones one unit keeps (`drone_sockets`).
pub const MAX_DRONES: usize = 6;
/// Weapons that may turn on gun houses of their own (`mount`) and be drawn turning: the
/// renderer's rig has this many house limbs (`mirror::HousePose`).
pub const MAX_HOUSES: usize = 8;

/// Highest tech tier. A unit's `tech` is `1..=MAX_TECH`.
pub const MAX_TECH: u8 = 5;

/// Every faction and unit, indexed by id. Immutable for the length of a match.
#[derive(Clone, Debug, Default)]
pub struct Blueprints {
    pub factions: Vec<Faction>,
    pub units: Vec<UnitBlueprint>,
    /// Every unit with refit slots: its slots, modules and loadouts.
    pub refits: Vec<RefitSet>,
    by_key: BTreeMap<String, BlueprintId>,
}

#[derive(Debug)]
pub enum DataError {
    Io(PathBuf, std::io::Error),
    Parse(PathBuf, String),
    Invalid(String),
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataError::Io(p, e) => write!(f, "{}: {e}", p.display()),
            DataError::Parse(p, e) => write!(f, "{}: {e}", p.display()),
            DataError::Invalid(e) => write!(f, "invalid blueprint data: {e}"),
        }
    }
}

impl std::error::Error for DataError {}

impl Blueprints {
    /// The tier a unit brings its side to: the highest tier among what it can build.
    pub fn builds_tech(&self, bp: &UnitBlueprint) -> u8 {
        bp.builder
            .as_ref()
            .and_then(|b| b.builds.iter().map(|id| self.unit(*id).tech).max())
            .unwrap_or(1)
    }

    /// What upgrading a unit into `to` costs, (materials, energy): only what the new tier
    /// costs over the one it is, for a structure as for an engineer. A refit kit (not a
    /// tier) is paid in full.
    pub fn upgrade_cost(&self, to: &UnitBlueprint) -> (Fx, Fx) {
        let from = self.units.iter().find(|u| u.upgrades_to == Some(to.id));
        match from {
            Some(f) => (
                (to.cost_mass - f.cost_mass).max(Fx::ZERO),
                (to.cost_energy - f.cost_energy).max(Fx::ZERO),
            ),
            None => (to.cost_mass, to.cost_energy),
        }
    }

    /// Build power a unit of `from` puts its own upgrade into `to` on with: its own,
    /// or its blueprint's `upgrade_power` (else `fallback`) if it builds nothing. An
    /// engineer fitting its next tier works at that tier's power, so climbing in the
    /// field takes seconds, not minutes.
    pub fn upgrade_power(&self, from: &UnitBlueprint, to: &UnitBlueprint, fallback: Fx) -> Fx {
        let own = from
            .builder
            .as_ref()
            .map_or(from.upgrade_power.unwrap_or(fallback), |b| b.power);
        let engineer = from.has(cat::ENGINEER)
            && from.is_mobile()
            && !from.has(cat::COMMANDER)
            && from.upgrades_to == Some(to.id);
        match &to.builder {
            Some(b) if engineer => own.max(b.power),
            _ => own,
        }
    }

    /// The tier a side must have reached before a unit may upgrade itself into `to`.
    /// Nothing climbs past the side's own tech (an economy structure, an engineer, a
    /// sonar or a shield alike): upgrading is not a way to reach a tier. A factory's
    /// upgrade is, so it is always open.
    pub fn upgrade_needs(&self, to: &UnitBlueprint) -> u8 {
        if to.has(cat::FACTORY) {
            1
        } else if to.has(cat::EXTRACTOR) {
            // A mine climbs one tier past the side's tech, so the economy can grow
            // before the army's tier is bought; the deep core still opens with tech 3.
            if to.tech >= 4 {
                3
            } else {
                (to.tech - 1).max(1)
            }
        } else {
            // Nothing builds at tech 4 yet: a tier 4 upgrade (the deep core) opens with tech 3.
            to.tech.min(3)
        }
    }

    /// Loads every faction under `<data_dir>/factions`. Factions and units get
    /// ids in sorted key order, so ids do not depend on directory listing order.
    pub fn load(data_dir: &Path) -> Result<Blueprints, DataError> {
        let factions_dir = data_dir.join("factions");
        let mut sources = Vec::new();
        for dir in sorted_entries(&factions_dir)? {
            if !dir.is_dir() {
                continue;
            }
            let faction_path = dir.join("faction.ron");
            let faction: raw::Faction = parse_file(&faction_path)?;
            let mut units = Vec::new();
            // A faction on a stand-in roster has no units folder yet.
            let unit_dir = dir.join("units");
            let unit_files = if unit_dir.is_dir() {
                sorted_entries(&unit_dir)?
            } else {
                Vec::new()
            };
            for file in unit_files {
                if file.extension().is_some_and(|e| e == "ron") {
                    let list: Vec<raw::Unit> = parse_file(&file)?;
                    units.extend(list);
                }
            }
            let lore_path = dir.join("lore.ron");
            let lore: BTreeMap<String, raw::RawUnitLore> = if lore_path.is_file() {
                // A map of bare strings is the older shape: the units' text alone.
                parse_file(&lore_path).or_else(|e| {
                    parse_file::<BTreeMap<String, String>>(&lore_path)
                        .map(|flat| {
                            flat.into_iter()
                                .map(|(k, lore)| {
                                    (
                                        k,
                                        raw::RawUnitLore {
                                            lore,
                                            ..Default::default()
                                        },
                                    )
                                })
                                .collect()
                        })
                        .map_err(|_| e)
                })?
            } else {
                BTreeMap::new()
            };
            // Only this faction's units and their own weapons: a name that matches nothing is a typo.
            let mut texts = BTreeMap::new();
            for (key, entry) in lore {
                let Some(unit) = units.iter().find(|u| u.key == key) else {
                    return Err(DataError::Invalid(format!(
                        "{}: unknown unit key {key}",
                        lore_path.display()
                    )));
                };
                if let Some(name) = entry
                    .weapons
                    .keys()
                    .find(|n| !refit::weapon_names(unit).any(|w| w == n.as_str()))
                {
                    return Err(DataError::Invalid(format!(
                        "{}: {key} has no weapon {name}",
                        lore_path.display()
                    )));
                }
                texts.insert(key, entry);
            }
            sources.push((faction, units, texts));
        }
        Self::compile(sources)
    }

    /// Finds `data/` next to the executable or in a parent of the working directory.
    pub fn locate_data_dir() -> Option<PathBuf> {
        let mut roots = Vec::new();
        if let Ok(exe) = std::env::current_exe() {
            roots.extend(exe.ancestors().skip(1).map(Path::to_path_buf));
        }
        if let Ok(cwd) = std::env::current_dir() {
            roots.extend(cwd.ancestors().map(Path::to_path_buf));
        }
        roots
            .into_iter()
            .map(|r| r.join("data"))
            .find(|d| d.join("factions").is_dir())
    }

    fn compile(
        mut sources: Vec<(
            raw::Faction,
            Vec<raw::Unit>,
            BTreeMap<String, raw::RawUnitLore>,
        )>,
    ) -> Result<Blueprints, DataError> {
        sources.sort_by(|a, b| a.0.key.cmp(&b.0.key));
        let mut all: Vec<(u8, raw::Unit)> = Vec::new();
        let mut lore = BTreeMap::new();
        for (fi, (_, units, texts)) in sources.iter_mut().enumerate() {
            all.extend(units.drain(..).map(|u| (fi as u8, u)));
            lore.append(texts);
        }
        all.sort_by(|a, b| a.1.key.cmp(&b.1.key));
        if all.len() > u16::MAX as usize {
            return Err(DataError::Invalid("too many unit blueprints".into()));
        }
        let mut by_key = BTreeMap::new();
        for (i, (_, u)) in all.iter().enumerate() {
            if by_key
                .insert(u.key.clone(), BlueprintId(i as u16))
                .is_some()
            {
                return Err(DataError::Invalid(format!("duplicate unit key {}", u.key)));
            }
        }
        // No two units share a name, across every faction: a name is how players tell them
        // apart. Loadouts and kits, added after, carry their unit's or module's name.
        let mut by_name = BTreeMap::new();
        for (_, u) in &all {
            if let Some(other) = by_name.insert(u.name.as_str(), u.key.as_str()) {
                return Err(DataError::Invalid(format!(
                    "units {other} and {} are both named {}",
                    u.key, u.name
                )));
            }
        }
        // Only authored units can be named in a file; loadouts and kits are added after.
        let authored = by_key.clone();
        let lookup = |key: &str, ctx: &str| {
            authored
                .get(key)
                .copied()
                .ok_or_else(|| DataError::Invalid(format!("{ctx}: unknown unit {key}")))
        };

        let mut units = Vec::with_capacity(all.len());
        for (i, (fi, u)) in all.iter().enumerate() {
            units.push(u.compile(BlueprintId(i as u16), FactionId(*fi), &lookup)?);
        }
        let refits = refit::expand(&all, &mut units, &mut by_key, &lookup)?;
        let scrap: Vec<BlueprintId> = units
            .iter()
            .flat_map(|u| u.weapons.iter().filter_map(|w| w.sabot.map(|s| s.casing)))
            .collect();
        for id in scrap {
            units[id.index()].scrap = true;
        }
        let drones: Vec<BlueprintId> = units.iter().filter_map(|u| u.drone).collect();
        for id in drones {
            units[id.index()].carried_drone = true;
        }
        for bp in &mut units {
            // A loadout reads its unit's text, and its modules' weapons theirs.
            let base = &all[refits_base(&refits, bp).index()].1.key;
            if let Some(text) = lore.get(base) {
                bp.lore = text.lore.clone();
                for w in &mut bp.weapons {
                    // Twin mounts share a name, and so share the text.
                    w.lore = text.weapons.get(&w.name).cloned().unwrap_or_default();
                }
            }
        }
        let mut factions = Vec::new();
        for (i, (f, ..)) in sources.iter().enumerate() {
            let stand_in = match &f.stand_in {
                None => None,
                Some(key) => Some(
                    sources
                        .iter()
                        .position(|(g, ..)| g.key.eq_ignore_ascii_case(key) && g.stand_in.is_none())
                        .map(|j| FactionId(j as u8))
                        .ok_or_else(|| {
                            DataError::Invalid(format!(
                                "{}: no faction {key} with its own roster to stand in",
                                f.key
                            ))
                        })?,
                ),
            };
            factions.push(Faction {
                id: FactionId(i as u8),
                key: f.key.clone(),
                name: f.name.clone(),
                abbreviation: f.abbreviation.clone(),
                description: f.description.clone(),
                commander: lookup(&f.commander, &f.key)?,
                stand_in,
                plating_color: f.plating_color,
                accent_color: f.accent_color,
                highlight_color: f.highlight_color,
                power_line: f.power_line,
                shield_color: f.shield_color,
                construction: f.construction,
                sounds: f.sounds.clone(),
                torpedo_look: f.torpedo_look,
                structure_lamps: f.structure_lamps,
                shield_look: f.shield_look,
                nuke_look: f.nuke_look,
                anti_missile_look: f.anti_missile_look,
            });
        }
        // A builder puts up only its own faction's structures. Mobile units may also come
        // from the roster its faction stands in on while it has no units of its own.
        for u in &units {
            let Some(b) = &u.builder else { continue };
            let own = u.faction;
            let borrowed = factions[own.0 as usize].stand_in;
            for &id in &b.builds {
                let made = &units[id.index()];
                let fits =
                    made.faction == own || (!made.is_structure() && Some(made.faction) == borrowed);
                if !fits {
                    return Err(DataError::Invalid(format!(
                        "{}: builds {}, which is not its faction's own{}",
                        u.key,
                        made.key,
                        if made.is_structure() {
                            " structure"
                        } else {
                            " or its stand-in's"
                        },
                    )));
                }
            }
        }
        // Standing energy draw is for powered systems only (the user's rule): a shield,
        // a radar or sonar, a warp dampener's field, or a mine's dig. Guns, launchers,
        // missile defence and reclaim (not even a tower's) run free; they cost energy to
        // build, not to keep.
        for u in &units {
            let powered = u.radar > Fx::ZERO
                || u.sonar > Fx::ZERO
                || u.shield.is_some()
                || u.warp_damper.is_some()
                || u.mine.is_some()
                || u.fabricator.is_some();
            if u.economy.energy_upkeep > Fx::ZERO && !powered {
                return Err(DataError::Invalid(format!(
                    "{}: only a shield, radar, sonar, warp dampener, mine or fabricator draws energy upkeep",
                    u.key
                )));
            }
        }
        Ok(Blueprints {
            factions,
            units,
            refits,
            by_key,
        })
    }

    #[inline]
    pub fn unit(&self, id: BlueprintId) -> &UnitBlueprint {
        &self.units[id.index()]
    }

    pub fn unit_by_key(&self, key: &str) -> Option<&UnitBlueprint> {
        self.by_key.get(key).map(|id| self.unit(*id))
    }

    pub fn id_of(&self, key: &str) -> Option<BlueprintId> {
        self.by_key.get(key).copied()
    }

    /// The adjacency conduits of the faction that fields `unit` (`Faction::power_line`):
    /// `.color` is its energy colour, linear RGB.
    pub fn power_line(&self, unit: BlueprintId) -> &PowerLine {
        &self.factions[self.unit(unit).faction.0 as usize].power_line
    }

    pub fn faction_by_key(&self, key: &str) -> Option<&Faction> {
        self.factions
            .iter()
            .find(|f| f.key.eq_ignore_ascii_case(key))
    }

    /// Hash of everything that can influence the simulation. Names, meshes and
    /// colours are left out: they cannot cause a desync.
    pub fn content_hash(&self) -> u64 {
        let mut h = StateHasher::new();
        h.write_u64(self.units.len() as u64);
        for u in &self.units {
            h.write_u8s(u.key.as_bytes());
            h.write_u64(u.faction.0 as u64);
            h.write_u64(u.tech as u64);
            h.write_u64(u.categories as u64);
            for v in [
                u.health,
                u.regen,
                u.cost_mass,
                u.cost_energy,
                u.build_time,
                u.radius,
                u.height,
                u.vision,
                u.radar,
                u.sonar,
                u.wreck_fraction,
            ] {
                h.write_i64(v.0);
            }
            h.write_u64(u.footprint.0 as u64 | (u.footprint.1 as u64) << 8);
            h.write_i64(u.hull.0 .0);
            h.write_i64(u.hull.1 .0);
            match &u.motion {
                Some(m) => {
                    h.write_u64(
                        (1 + m.layer as u64)
                            | ((m.size_class as u64) << 8)
                            | (m.turn_rate as u64) << 16,
                    );
                    h.write_i64(m.speed.0);
                    h.write_i64(m.accel.0);
                    h.write_i64(m.altitude.0);
                    h.write_u64(m.hover as u64);
                    h.write_u64(m.deploy_ticks as u64);
                    h.write_u64(m.broadside.0 as u64);
                    h.write_u64(m.aim_arc as u64);
                    h.write_u64(
                        m.stride as u64 | (m.hangs as u64) << 1 | (m.stand_off as u64) << 2,
                    );
                    h.write_u64(m.above_weather as u64);
                }
                None => h.write_u64(0),
            }
            let e = &u.economy;
            for v in [
                e.mass_income,
                e.energy_income,
                e.energy_upkeep,
                e.mass_storage,
                e.energy_storage,
            ] {
                h.write_i64(v.0);
            }
            match &u.mine {
                Some(m) => {
                    for v in [m.reach, m.ground, m.per_hectare, m.base] {
                        h.write_i64(v.0);
                    }
                }
                None => h.write_u64(u64::MAX),
            }
            h.write_i64(u.fabricator.map_or(-1, |f| f.mass.0));
            match &u.death_blast {
                Some(d) => {
                    h.write_i64(d.radius.0);
                    h.write_i64(d.damage.0);
                }
                None => h.write_u64(u64::MAX),
            }
            match &u.adjacency {
                Some(a) => {
                    h.write_i64(a.energy.0);
                    h.write_i64(a.mass.0);
                }
                None => h.write_u64(u64::MAX),
            }
            match &u.strategic {
                Some(s) => s.hash(&mut h),
                None => h.write_u64(u64::MAX),
            }
            h.write_u64(u.water_build as u64);
            h.write_u64(u.seabed as u64);
            h.write_i64(u.drone_radius.0);
            h.write_u64(u.drone_sockets.len() as u64);
            for m in &u.drone_sockets {
                h.write_i64(m.x.0);
                h.write_i64(m.y.0);
                h.write_i64(m.z.0);
            }
            h.write_i64(u.drone_approach.0);
            h.write_i64(u.orbit.map_or(-1, |r| r.0));
            h.write_i64(u.anti_missile.0);
            h.write_u64(u.anti_missile_lasers as u64);
            match u.turret_at {
                Some(p) => {
                    h.write_i64(p.x.0);
                    h.write_i64(p.y.0);
                }
                None => h.write_u64(u64::MAX),
            }
            for m in &u.anti_missile_mounts {
                h.write_i64(m.x.0);
                h.write_i64(m.y.0);
                h.write_i64(m.z.0);
            }
            h.write_u64(u.hull_mounts as u64);
            match &u.stomp {
                Some(s) => {
                    for v in [s.pace, s.reach, s.gauge, s.radius, s.damage] {
                        h.write_i64(v.0);
                    }
                }
                None => h.write_u64(u64::MAX),
            }
            match &u.dive {
                Some(d) => {
                    h.write_i64(d.depth.0);
                    h.write_u64(d.ticks as u64);
                    h.write_u64(d.ambush as u64);
                }
                None => h.write_u64(u64::MAX),
            }
            h.write_u64(u.drone.map_or(u64::MAX, |d| d.0 as u64));
            match &u.builder {
                Some(b) => {
                    h.write_i64(b.power.0);
                    h.write_i64(b.range.0);
                    h.write_u64(b.builds.len() as u64);
                    for id in &b.builds {
                        h.write_u64(id.0 as u64);
                    }
                    h.write_u64(b.arm.map_or(u64::MAX, |a| a.turn as u64));
                    h.write_u64(b.arm.map_or(u64::MAX, |a| a.rest.0 as u64));
                    for v in b
                        .arm
                        .and_then(|a| a.pivot)
                        .map_or([Fx::MAX; 3], |p| [p.x, p.y, p.z])
                    {
                        h.write_i64(v.0);
                    }
                    for v in b
                        .arm
                        .and_then(|a| a.shoulder)
                        .map_or([Fx::MAX; 3], |p| [p.x, p.y, p.z])
                    {
                        h.write_i64(v.0);
                    }
                    h.write_u64(b.emitters.len() as u64 | (b.unfold_ticks as u64) << 32);
                    for v in b.hinge.map_or([Fx::MAX; 3], |p| [p.x, p.y, p.z]) {
                        h.write_i64(v.0);
                    }
                    for e in &b.emitters {
                        h.write_i64(e.x.0);
                        h.write_i64(e.y.0);
                        h.write_i64(e.z.0);
                    }
                }
                None => h.write_u64(u64::MAX),
            }
            match &u.reclaimer {
                Some(r) => {
                    h.write_i64(r.power.0);
                    h.write_i64(r.range.0);
                    h.write_u64(
                        r.charge_ticks as u64
                            | (r.mobile as u64) << 16
                            | (r.heads().len() as u64) << 24,
                    );
                    for head in r.heads() {
                        h.write_u64(
                            head.turn as u64
                                | (head.pitch_min.0 as u64) << 16
                                | (head.pitch_max.0 as u64) << 32,
                        );
                        for v in head.pivot.map_or([Fx::MAX; 3], |p| [p.x, p.y, p.z]) {
                            h.write_i64(v.0);
                        }
                    }
                }
                None => h.write_u64(u64::MAX),
            }
            match &u.transport {
                Some(t) => {
                    h.write_u64(t.capacity as u64 | (t.unload_ticks as u64) << 16);
                    for v in [
                        t.descent,
                        t.hold.x,
                        t.hold.y,
                        t.hinge,
                        t.lip,
                        t.width,
                        t.floor,
                        t.clearance,
                    ] {
                        h.write_i64(v.0);
                    }
                }
                None => h.write_u64(u64::MAX),
            }
            match &u.warp {
                Some(d) => {
                    h.write_u64(d.spool_ticks as u64 | (d.cooldown_ticks as u64) << 16);
                    h.write_i64(d.speed.0);
                    h.write_i64(d.per_km.0);
                }
                None => h.write_u64(u64::MAX),
            }
            match &u.warp_damper {
                Some(d) => {
                    h.write_u64(d.stun_ticks as u64);
                    h.write_i64(d.radius.0);
                    h.write_i64(d.drag.0);
                    h.write_i64(d.damage.0);
                }
                None => h.write_u64(u64::MAX),
            }
            match &u.shield {
                Some(s) => {
                    h.write_u64(s.kind as u64);
                    h.write_i64(s.radius.0);
                    h.write_i64(s.health.0);
                    h.write_i64(s.regen.0);
                }
                None => h.write_u64(u64::MAX),
            }
            h.write_u64(u.upgrades_to.map_or(u64::MAX, |id| id.0 as u64));
            h.write_i64(u.upgrade_power.map_or(-1, |p| p.0));
            h.write_u64(u.weapons.len() as u64);
            for w in &u.weapons {
                for v in [
                    w.damage,
                    w.splash,
                    w.range_min,
                    w.range_max,
                    w.projectile_speed,
                    w.muzzle.x,
                    w.muzzle.y,
                    w.muzzle.z,
                ] {
                    h.write_i64(v.0);
                }
                h.write_u64(w.muzzles.len() as u64);
                for m in &w.muzzles {
                    h.write_i64(m.x.0);
                    h.write_i64(m.y.0);
                    h.write_i64(m.z.0);
                }
                h.write_u64(
                    w.reload_ticks as u64
                        | (w.salvo as u64) << 16
                        | (w.salvo_delay_ticks as u64) << 24
                        | (w.trajectory as u64) << 32
                        | (w.salvo_batch as u64) << 40,
                );
                h.write_u64(
                    w.turret_turn as u64
                        | (w.half_arc as u64) << 16
                        | (w.spread as u64) << 32
                        | (w.loft_ticks as u64) << 48,
                );
                h.write_u64(w.target_mask as u64 | (w.prefer_mask as u64) << 32);
                h.write_u64(
                    w.missile as u64
                        | (w.guided as u64) << 1
                        | (w.vertical_launch as u64) << 2
                        | (w.rear as u64) << 3
                        | (w.torpedo as u64) << 4
                        | (w.surfaced as u64) << 5
                        | (w.intercepts as u64) << 6
                        | (w.keeps_aim as u64) << 7
                        | (w.flak as u64) << 8
                        | (w.airburst as u64) << 9,
                );
                h.write_u64(w.cant.0 as u64);
                h.write_i64(w.skim.0);
                h.write_i64(w.apogee.0);
                h.write_i64(w.intercept_hp.0);
                h.write_u64(
                    w.cold_launch_ticks as u64
                        | (w.boost_ticks as u64) << 16
                        | (w.hatch_ticks as u64) << 32
                        | (w.split as u64) << 48,
                );
                h.write_i64(w.proximity.0);
                h.write_i64(w.burn_dps.0);
                h.write_i64(w.bombard_radius.0);
                h.write_u64(
                    w.burn_ticks as u64
                        | (w.mount as u64) << 16
                        | (w.spin_ticks as u64) << 17
                        | (w.slant as u64) << 33,
                );
                h.write_u64(
                    w.sweep as u64
                        | (w.facing.0 as u64) << 16
                        | (w.spin_ramp as u64) << 32
                        | (w.barrels as u64) << 48,
                );
                h.write_u64(
                    w.sway.0 as u64
                        | (w.rake.0 as u64) << 16
                        | (w.curve.0 as u64) << 32
                        | (w.launches as u64) << 48,
                );
                h.write_i64(w.walk.0);
                h.write_i64(w.corkscrew.0);
                if let Some(s) = w.sabot {
                    for v in [
                        s.port.x, s.port.y, s.port.z, s.throw.x, s.throw.y, s.throw.z, s.kick,
                        s.damage, s.splash,
                    ] {
                        h.write_i64(v.0);
                    }
                    h.write_u64(s.casing.0 as u64);
                }
                if let Some(c) = w.cone {
                    h.write_u64(c.half.0 as u64);
                    h.write_i64(c.edge.0);
                    h.write_i64(c.speed.0);
                }
                if let Some(c) = w.cluster {
                    h.write_u64(c.count as u64);
                    h.write_i64(c.radius.0);
                    h.write_i64(c.height.0);
                    h.write_i64(c.splash.0);
                }
                if let Some(b) = w.bore {
                    h.write_i64(b.width.0);
                    h.write_i64(b.damage.0);
                    if let Some(s) = b.storm {
                        h.write_i64(s.radius.0);
                        h.write_i64(s.damage.0);
                        h.write_u64(s.ticks as u64);
                    }
                }
                for v in w.pivot.map_or([Fx::MAX; 3], |p| [p.x, p.y, p.z]) {
                    h.write_i64(v.0);
                }
            }
        }
        // What can be fitted over what: the sim checks every refit against it.
        h.write_u64(self.refits.len() as u64);
        for set in &self.refits {
            h.write_u64(set.base.0 as u64);
            for slot in &set.slots {
                h.write_u64(slot.modules.len() as u64);
                for m in &slot.modules {
                    h.write_u64(
                        m.kit.0 as u64
                            | (m.after.unwrap_or(0xFF) as u64) << 16
                            | (m.tech as u64) << 24,
                    );
                }
            }
            for id in &set.loadouts {
                h.write_u64(id.0 as u64);
            }
        }
        h.write_u64(self.factions.len() as u64);
        for f in &self.factions {
            h.write_u8s(f.key.as_bytes());
            h.write_u64(f.commander.0 as u64);
            h.write_u64(f.stand_in.map_or(u64::MAX, |s| s.0 as u64));
        }
        h.finish()
    }
}

pub(crate) fn sorted_entries(dir: &Path) -> Result<Vec<PathBuf>, DataError> {
    let read = std::fs::read_dir(dir).map_err(|e| DataError::Io(dir.to_path_buf(), e))?;
    let mut paths = Vec::new();
    for entry in read {
        paths.push(
            entry
                .map_err(|e| DataError::Io(dir.to_path_buf(), e))?
                .path(),
        );
    }
    paths.sort();
    Ok(paths)
}

pub(crate) fn parse_file<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, DataError> {
    let text = std::fs::read_to_string(path).map_err(|e| DataError::Io(path.to_path_buf(), e))?;
    // Optional fields are written bare (`motion: (..)`), not wrapped in `Some`.
    ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(&text)
        .map_err(|e| DataError::Parse(path.to_path_buf(), e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
    }

    #[test]
    fn regency_stands_in_on_the_aster_roster() {
        let bp = Blueprints::load(&data_dir()).unwrap();
        let aster = bp.faction_by_key("aster").unwrap();
        let regency = bp
            .faction_by_key("regency")
            .expect("the Regency is defined");
        // Aster sorts first and keeps id 0: the renderer paints with factions[0].
        assert_eq!(aster.id, FactionId(0));
        assert_eq!(aster.stand_in, None);
        assert_eq!(regency.stand_in, Some(aster.id));
        // Their own commander, which builds only their own structures and the craft raised
        // on a lot (the Coffer).
        let commander = bp.unit(regency.commander);
        assert_eq!(commander.key, "regency_commander");
        assert_eq!(commander.faction, regency.id);
        assert!(commander.has(cat::COMMANDER));
        let set = bp.refit_set(regency.commander).expect("it refits");
        let suites: Vec<&str> = set
            .slots
            .iter()
            .flat_map(|s| &s.modules)
            .map(|m| m.key.as_str())
            .collect();
        assert_eq!(suites, ["eng_2", "eng_3", "shield"]);
        let builds = &commander.builder.as_ref().unwrap().builds;
        assert!(builds.len() >= 8);
        assert!(builds.iter().all(|&b| bp.unit(b).faction == regency.id
            && (bp.unit(b).is_structure() || bp.unit(b).is_site_built_unit())));
        // Their factories make their own engineer and, for now, the stand-in's fighters.
        for factory in bp
            .units
            .iter()
            .filter(|u| u.faction == regency.id && u.has(cat::FACTORY))
        {
            let made = &factory.builder.as_ref().unwrap().builds;
            assert!(
                made.iter()
                    .any(|&b| bp.unit(b).key == "regency_t1_engineer"),
                "{}",
                factory.key
            );
            assert!(
                made.iter().all(|&b| !bp.unit(b).is_structure()),
                "{}",
                factory.key
            );
        }
        // Nanites build their structures, and they answer in their own voices.
        assert_eq!(regency.construction, Construction::Nanite);
        assert_eq!(aster.construction, Construction::Print);
        assert_eq!(regency.torpedo_look, TorpedoLook::Plasma);
        assert_eq!(aster.torpedo_look, TorpedoLook::Bubbles);
        assert_eq!(regency.structure_lamps, StructureLamps::Ember);
        assert_eq!(aster.structure_lamps, StructureLamps::Sodium);
        assert_eq!(
            (regency.power_line.path, regency.power_line.look),
            (LinePath::Curve, LineLook::Plated)
        );
        assert_eq!(
            (aster.power_line.path, aster.power_line.look),
            (LinePath::Straight, LineLook::Clamped)
        );
        assert_eq!(regency.shield_look, ShieldLook::Prism);
        assert_eq!(aster.shield_look, ShieldLook::Honeycomb);
        assert_eq!(regency.anti_missile_look, AntiMissileLook::CounterSeeker);
        assert_eq!(aster.anti_missile_look, AntiMissileLook::Laser);
        assert!(
            regency.sounds.select.contains_key(&IconKind::Factory)
                && regency.sounds.build.is_some()
        );
    }

    /// A copy of the shipped data in a temporary folder named for `tag`, to break.
    fn data_copy(tag: &str) -> PathBuf {
        fn walk(dir: &Path) -> Vec<PathBuf> {
            let mut out = Vec::new();
            for e in std::fs::read_dir(dir).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    out.extend(walk(&p))
                } else {
                    out.push(p)
                }
            }
            out
        }
        let dir = std::env::temp_dir().join(format!("mc-data-{tag}-{}", std::process::id()));
        let from = data_dir().join("factions");
        for entry in walk(&from) {
            let target = dir
                .join("factions")
                .join(entry.strip_prefix(&from).unwrap());
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::copy(&entry, &target).unwrap();
        }
        dir
    }

    #[test]
    fn no_two_units_share_a_name() {
        // The shipped data with the Regency scout given an ARC tank's name.
        let dir = data_copy("same-name");
        let land = dir.join("factions/regency/units/land.ron");
        let text = std::fs::read_to_string(&land).unwrap();
        assert!(text.contains("\"Outrider\""));
        std::fs::write(&land, text.replacen("\"Outrider\"", "\"Warden\"", 1)).unwrap();
        let Err(err) = Blueprints::load(&dir) else {
            panic!("two units named Warden were allowed")
        };
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(
            err.to_string().contains("aster_t1_tank") && err.to_string().contains("Warden"),
            "{err}"
        );
    }

    #[test]
    fn a_builder_may_not_put_up_another_factions_structure() {
        // The shipped data with the Regency commander told to build an ARC reactor.
        let dir = data_copy("foreign-build");
        let command = dir.join("factions/regency/units/command.ron");
        let text = std::fs::read_to_string(&command).unwrap();
        let text = text.replacen(
            "\"regency_t1_power\",",
            "\"regency_t1_power\", \"aster_t1_power\",",
            1,
        );
        std::fs::write(&command, text).unwrap();
        let Err(err) = Blueprints::load(&dir) else {
            panic!("the foreign structure was allowed")
        };
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(err.to_string().contains("aster_t1_power"), "{err}");
    }

    #[test]
    fn aster_loads_and_is_consistent() {
        let bp = Blueprints::load(&data_dir()).unwrap();
        let aster = bp.faction_by_key("aster").expect("Aster is defined");
        assert_eq!(aster.name, "Asterian Reach Command");
        let acu = bp.unit(aster.commander);
        assert!(acu.has(cat::COMMANDER | cat::MOBILE));
        assert!(acu.builder.is_some());
        for key in [
            "aster_t1_engineer",
            "aster_t2_engineer",
            "aster_t3_engineer",
        ] {
            let eng = bp.unit(bp.id_of(key).unwrap());
            assert_eq!(eng.motion.unwrap().layer, MoveLayer::Hover, "{key} hovers");
            assert!(
                eng.builder.as_ref().and_then(|b| b.arm).is_some(),
                "{key} has a build arm"
            );
        }
        for (factory, engineer) in [
            ("aster_t1_air_factory", "aster_t1_engineer"),
            ("aster_t2_air_factory", "aster_t2_engineer"),
            ("aster_t3_air_factory", "aster_t3_engineer"),
        ] {
            let factory_bp = bp.unit(bp.id_of(factory).unwrap());
            let builds = &factory_bp.builder.as_ref().unwrap().builds;
            let id = bp.id_of(engineer).unwrap();
            assert!(builds.contains(&id), "{factory} builds {engineer}");
        }
        let interceptor = bp.unit(bp.id_of("aster_t1_interceptor").unwrap());
        assert_eq!(interceptor.motion.unwrap().layer, MoveLayer::Air);
        assert!(interceptor.motion.unwrap().altitude > Fx::ZERO);
        assert!(interceptor.has(cat::AIR | cat::MOBILE));
        assert!(
            interceptor.weapons[0].half_arc < 0x2000,
            "nose guns, not a free turret"
        );
        let bomber = bp.unit(bp.id_of("aster_t1_bomber").unwrap());
        assert_eq!(bomber.weapons[0].salvo, 2);
        assert_eq!(bomber.weapons[0].salvo_batch, 2);
        assert_eq!(interceptor.weapons[0].salvo_batch, 1);
        assert_eq!(bomber.weapons[0].trajectory, Trajectory::Ballistic);
        let trebuchet = bp.unit(bp.id_of("aster_t3_artillery").unwrap());
        assert!(
            trebuchet.motion.unwrap().deploy_ticks > 10,
            "the Trebuchet should plant before it fires"
        );

        for u in &bp.units {
            assert!(u.weapons.len() <= MAX_WEAPONS, "{}", u.key);
            assert!(u.health > Fx::ZERO, "{}", u.key);
            assert_eq!(
                u.built_on_site(),
                u.footprint != (0, 0),
                "{} footprint",
                u.key
            );
            assert_eq!(u.is_structure(), u.motion.is_none(), "{} motion", u.key);
            for w in &u.weapons {
                assert!(
                    w.range_max > w.range_min && w.reload_ticks > 0,
                    "{} {}",
                    u.key,
                    w.name
                );
                if w.trajectory == Trajectory::Ballistic {
                    assert!(w.loft_ticks > 0, "{} {} has no loft", u.key, w.name);
                    if w.turret_turn > 0 {
                        assert!(
                            w.pivot.is_some(),
                            "{} {} howitzer has no trunnion",
                            u.key,
                            w.name
                        );
                    }
                }
            }
        }
        // Everything except the commander can be built by something. A refit's
        // loadouts and kits are not built: they are fitted.
        for u in bp.units.iter().filter(|u| bp.is_listed(u.id)) {
            let buildable = bp.units.iter().any(|b| {
                b.builder.as_ref().is_some_and(|x| x.builds.contains(&u.id))
                    || b.upgrades_to == Some(u.id)
                    || b.drone == Some(u.id)
            });
            assert!(
                buildable || u.has(cat::COMMANDER) || u.has(cat::REPLICATOR),
                "{} cannot be built",
                u.key
            );
        }
    }

    #[test]
    fn the_commander_is_refitted_slot_by_slot() {
        let bp = Blueprints::load(&data_dir()).unwrap();
        let acu = bp.unit_by_key("aster_commander").unwrap();
        let set = bp.refit_set(acu.id).expect("the commander has refit slots");
        assert_eq!(set.base, acu.id);
        assert!(bp.is_listed(acu.id));
        let slot = |key: &str| set.slots.iter().position(|s| s.key == key).unwrap();
        let module = |slot_key: &str, key: &str| {
            let s = slot(slot_key);
            let m = set.slots[s]
                .modules
                .iter()
                .position(|m| m.key == key)
                .unwrap();
            set.slots[s].modules[m].kit
        };
        let expected: usize = set.slots.iter().map(|s| s.modules.len() + 1).product();
        assert_eq!(set.loadouts.len(), expected);
        assert_eq!(acu.weapons.len(), 1, "it lands with the machine gun alone");

        // The cannon goes on the right arm; the rail cannon only over it, and takes its place.
        let (cannon, rail) = (module("gun", "cannon"), module("gun", "railgun"));
        assert!(!bp.is_listed(cannon));
        assert_eq!(
            bp.refit_result(acu.id, rail),
            Err(refit::FitError::Needs(0))
        );
        let with_cannon = bp.refit_result(acu.id, cannon).unwrap();
        assert!(!bp.is_listed(with_cannon));
        assert_eq!(bp.base_of(with_cannon), acu.id);
        let names = |id: BlueprintId| -> Vec<String> {
            bp.unit(id).weapons.iter().map(|w| w.name.clone()).collect()
        };
        assert_eq!(names(with_cannon), ["Vulcan Machine Gun", "Breach Cannon"]);
        let with_rail = bp.refit_result(with_cannon, rail).unwrap();
        assert_eq!(
            names(with_rail),
            ["Vulcan Machine Gun", "Meridian Rail Cannon"]
        );
        assert_eq!(
            bp.refit_result(with_rail, cannon),
            Err(refit::FitError::Fitted)
        );
        // The rail cannon is drawn with the cannon's mount under it.
        let bit = |s: &str, k: &str| {
            let s = slot(s);
            1u32 << set.slots[s]
                .modules
                .iter()
                .find(|m| m.key == k)
                .unwrap()
                .bit
        };
        assert_eq!(
            bp.look(with_rail),
            bit("gun", "cannon") | bit("gun", "railgun")
        );
        assert_eq!(bp.look(acu.id), 0);

        // The back takes one pack: the shield replaces the formation engine.
        let (mfe, shield) = (module("back", "mfe"), module("back", "shield"));
        let with_mfe = bp.refit_result(with_rail, mfe).unwrap();
        let e = &bp.unit(with_mfe).economy;
        assert_eq!(e.mass_income, acu.economy.mass_income + Fx::from_int(3));
        assert_eq!(
            e.energy_income,
            acu.economy.energy_income + Fx::from_int(100)
        );
        let (set2, loadout) = bp.loadout(with_mfe).unwrap();
        assert_eq!(set2.replaces(&loadout.fitted, slot("back"), 1), Some(0));
        let with_shield = bp.refit_result(with_mfe, shield).unwrap();
        assert!(bp.unit(with_shield).shield.is_some_and(|s| s.is_hull()));
        assert_eq!(
            bp.unit(with_shield).economy.mass_income,
            acu.economy.mass_income
        );

        // The engineering suites: II, then III over it, with the tiers they unlock.
        let (eng2, eng3) = (
            module("engineering", "eng_2"),
            module("engineering", "eng_3"),
        );
        assert_eq!(
            bp.refit_result(acu.id, eng3),
            Err(refit::FitError::Needs(0))
        );
        let t3 = bp
            .refit_result(bp.refit_result(acu.id, eng2).unwrap(), eng3)
            .unwrap();
        assert_eq!(bp.unit(t3).tech, 3);
        let power = |id: BlueprintId| bp.unit(id).builder.as_ref().unwrap().power;
        assert_eq!(power(t3), Fx::from_int(160));
        let sam = bp.id_of("aster_t3_sam").unwrap();
        assert!(bp.unit(t3).builder.as_ref().unwrap().builds.contains(&sam));
        // At each tier the commander builds exactly what that tier's Mason builds:
        // bare it is a Mason, Suite II a Mason II, Suite III a Mason III (experimentals too).
        let roster = |id: BlueprintId| {
            let mut builds = bp.unit(id).builder.as_ref().unwrap().builds.clone();
            builds.sort_unstable();
            builds
        };
        let t2 = bp.refit_result(acu.id, eng2).unwrap();
        for (commander, engineer) in [
            (acu.id, "aster_t1_engineer"),
            (t2, "aster_t2_engineer"),
            (t3, "aster_t3_engineer"),
        ] {
            assert_eq!(
                roster(commander),
                roster(bp.id_of(engineer).unwrap()),
                "the commander at tech {} builds what {engineer} builds",
                bp.unit(commander).tech
            );
        }

        // The shoulder: anti-air, a howitzer, or a second projector; one at a time.
        let aa = module("shoulder", "aa");
        let aux = module("shoulder", "aux_eng");
        let full = bp.refit_result(with_rail, aa).unwrap();
        assert!(bp.unit(full).has(cat::ANTI_AIR));
        assert_eq!(bp.unit(full).weapons.len(), 3);
        let builder = bp.refit_result(t3, aux).unwrap();
        assert_eq!(power(builder), Fx::from_int(220));
        let b = bp.unit(builder).builder.as_ref().unwrap();
        assert_eq!(b.emitters.len(), 1);
        assert!(b.unfold_ticks > 0);
        // Kits carry the module's price, and are nobody's commander.
        assert_eq!(
            bp.unit(aux).cost_mass,
            set.module(slot("shoulder"), 2).cost_mass
        );
        assert!(!bp.unit(aux).has(cat::COMMANDER));
        // Loadouts are worth their base as a wreck, whatever is on them.
        assert_eq!(
            bp.unit(full).cost_mass * bp.unit(full).wreck_fraction,
            acu.cost_mass * acu.wreck_fraction
        );
        // Every loadout keeps within the sim's weapon slots and reads its unit's lore.
        for &id in &set.loadouts {
            assert!(bp.unit(id).weapons.len() <= MAX_WEAPONS);
            assert_eq!(bp.unit(id).lore, acu.lore);
        }
    }

    /// A copy of `data/` with Aster's `lore.ron` replaced by `lore` (none when `None`)
    /// and `edit` applied to its first unit file.
    fn scratch_data(name: &str, lore: Option<&str>, edit: impl Fn(String) -> String) -> PathBuf {
        let root = std::env::temp_dir().join(format!("mc-data-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let from = data_dir().join("factions/aster");
        let to = root.join("factions/aster");
        std::fs::create_dir_all(to.join("units")).unwrap();
        for file in ["faction.ron", "sounds.ron"] {
            std::fs::copy(from.join(file), to.join(file)).unwrap();
        }
        for (i, file) in sorted_entries(&from.join("units"))
            .unwrap()
            .into_iter()
            .enumerate()
        {
            let text = std::fs::read_to_string(&file).unwrap();
            let text = if i == 0 { edit(text) } else { text };
            std::fs::write(to.join("units").join(file.file_name().unwrap()), text).unwrap();
        }
        if let Some(lore) = lore {
            std::fs::write(to.join("lore.ron"), lore).unwrap();
        }
        root
    }

    #[test]
    fn lore_is_attached_to_units_and_weapons() {
        let plain = Blueprints::load(&scratch_data("none", None, |t| t)).unwrap();
        assert!(plain.units.iter().all(|u| u.lore.is_empty()));
        let acu = plain.unit_by_key("aster_commander").unwrap();
        let gun = acu.weapons[0].name.clone();
        let lore = format!(
            r#"{{
                "aster_commander": (lore: "Lands alone.", weapons: {{ "{gun}": "Rails." }}),
                "aster_t1_engineer": (lore: "The shovel."),
                "aster_t1_tank": (),
            }}"#
        );
        let bp = Blueprints::load(&scratch_data("lore", Some(&lore), |t| t)).unwrap();
        let acu = bp.unit_by_key("aster_commander").unwrap();
        assert_eq!(acu.lore, "Lands alone.");
        assert_eq!(acu.weapons[0].lore, "Rails.");
        assert_eq!(
            bp.unit_by_key("aster_t1_engineer").unwrap().lore,
            "The shovel."
        );
        assert!(bp.unit_by_key("aster_t1_tank").unwrap().lore.is_empty());
        assert_eq!(bp.content_hash(), plain.content_hash(), "lore is cosmetic");
        // The older shape: bare strings, the units' text alone.
        let flat = r#"{"aster_t1_engineer": "The shovel."}"#;
        let bp = Blueprints::load(&scratch_data("flat", Some(flat), |t| t)).unwrap();
        assert_eq!(
            bp.unit_by_key("aster_t1_engineer").unwrap().lore,
            "The shovel."
        );
    }

    #[test]
    fn lore_naming_nothing_is_an_error() {
        let err = Blueprints::load(&scratch_data(
            "key",
            Some(r#"{"aster_nothing": "x"}"#),
            |t| t,
        ))
        .unwrap_err()
        .to_string();
        assert!(err.contains("aster_nothing"), "{err}");
        let err = Blueprints::load(&scratch_data(
            "weapon",
            Some(r#"{"aster_commander": (weapons: {"Pea Shooter": "x"})}"#),
            |t| t,
        ))
        .unwrap_err()
        .to_string();
        assert!(err.contains("Pea Shooter"), "{err}");
    }

    #[test]
    fn tech_runs_from_one_to_five() {
        let tier = |tech: u8| {
            Blueprints::load(&scratch_data(&format!("tech{tech}"), None, |t| {
                t.replacen("tech: 1", &format!("tech: {tech}"), 1)
                    .replacen("tech: 2", &format!("tech: {tech}"), 1)
                    .replacen("tech: 3", &format!("tech: {tech}"), 1)
            }))
        };
        assert!(tier(4).is_ok());
        assert!(tier(5).is_ok());
        assert!(tier(0).is_err());
        assert!(tier(6).is_err());
    }

    #[test]
    fn hash_is_stable_across_loads() {
        let a = Blueprints::load(&data_dir()).unwrap();
        let b = Blueprints::load(&data_dir()).unwrap();
        assert_eq!(a.content_hash(), b.content_hash());
        assert_ne!(a.content_hash(), Blueprints::default().content_hash());
        let mut paired = a.clone();
        let bomber = paired.id_of("aster_t1_bomber").unwrap();
        paired.units[bomber.index()].weapons[0].salvo_batch = 1;
        assert_ne!(
            a.content_hash(),
            paired.content_hash(),
            "peers must reject different salvo grouping"
        );
    }
}

#[cfg(test)]
mod effect_settings_tests {
    use super::*;
    #[test]
    fn effect_settings_defaults_and_colored_ron() {
        let defaults: EffectSettings = ron::from_str("()").unwrap();
        assert_eq!(defaults, EffectSettings::default());
        assert_eq!(defaults.dust_color, None);
        assert_eq!(defaults.dust_brightness, 1.0);
        let dust: EffectSettings = ron::from_str(
            "(dust_opacity: 0.3, dust_color: Some((0.7, 0.22, 0.08)), dust_brightness: 2.0)",
        )
        .unwrap();
        assert_eq!(dust.dust_visibility, 0.3);
        assert_eq!(dust.dust_color, Some([0.7, 0.22, 0.08]));
        assert_eq!(dust.dust_brightness, 2.0);
        assert!(
            ron::from_str::<EffectSettings>("(dust_opacity: 0.3, dust_visibility: 0.7)").is_err()
        );
        let colored: EffectSettings = ron::from_str(
            "(dust_visibility: 0.4, dust_lifetime: 2.0, shockwave_color: Some((0.2, 0.6, 1.0)))",
        )
        .unwrap();
        assert_eq!(colored.dust_visibility, 0.4);
        assert_eq!(colored.dust_lifetime, 2.0);
        assert_eq!(colored.shockwave_color, Some([0.2, 0.6, 1.0]));
        assert!(ron::from_str::<EffectSettings>("(dust_lifetim: 2.0)").is_err());
    }

    #[test]
    fn a_build_tile_title_is_a_short_role_not_the_name() {
        let bp =
            Blueprints::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"))
                .unwrap();
        let titled: Vec<_> = bp.units.iter().filter(|u| u.title.is_some()).collect();
        assert!(titled.len() > 50, "only {} titled", titled.len());
        for u in titled {
            let title = u.title.as_deref().unwrap();
            // A word or two that fits the tile.
            assert!(
                !title.is_empty() && title.len() <= 16,
                "{}: {title:?}",
                u.key
            );
            assert_ne!(title, u.name, "{}: the title repeats the name", u.key);
        }
    }
}
