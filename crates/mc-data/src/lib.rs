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
pub mod sounds;
pub mod survival;
pub mod weather;

use mc_core::{Angle, Fx, FxVec2, FxVec3, StateHasher};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

pub use raw::{IconKind, MoveLayer, ShieldKind, Trajectory, UnitSounds, WeaponColor, WeaponSounds};
pub use refit::{Loadout, Module, Refit, RefitSet, RefitSlot, MAX_REFIT_SLOTS};
pub use sounds::{SoundId, SoundLibrary};

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
    /// Linear RGB, presentation only.
    pub plating_color: [f32; 3],
    pub accent_color: [f32; 3],
    pub highlight_color: [f32; 3],
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
}

/// An Argon Electric Bore's discharge (`Weapon::bore`): when the argon tracer lands, the
/// charge runs back down its ionised channel. Everything within `width` of that channel
/// takes `damage` (zero width: only the bolt; the blast is the weapon's own splash), and
/// the ground under it is left molten for `cool` seconds (cosmetic).
#[derive(Clone, Copy, Debug)]
pub struct Bore {
    pub width: Fx,
    pub damage: Fx,
    pub cool: f32,
}

/// A submarine's dive: how deep its deck goes and how long the trip takes.
#[derive(Clone, Copy, Debug)]
pub struct Dive {
    /// Metres of water over the hull's top when dived.
    pub depth: Fx,
    /// Ticks to dive or to surface.
    pub ticks: u16,
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
#[derive(Clone, Copy, Debug)]
pub struct Mine {
    /// Metres around itself it mines.
    pub reach: Fx,
    /// Materials per second per hectare of land in its territory.
    pub ground: Fx,
    /// Materials per second per hectare of ore in its territory (instead of `ground` there).
    pub per_hectare: Fx,
    /// Materials per second from the shaft itself, from the moment it is finished,
    /// whatever its territory: a new mine pays at once while its land spreads out.
    pub base: Fx,
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

/// Takes things apart at a distance without being a builder: a reclaimer
/// tower. Left alone it clears the wrecks within `range`; it takes orders for the rest.
/// The head is a turret: it turns at `turn` and charges `charge_ticks` before the beam.
#[derive(Clone, Copy, Debug)]
pub struct Reclaimer {
    /// Build time units undone per second, and mass per second out of a wreck.
    pub power: Fx,
    pub range: Fx,
    /// Angle steps per tick the turret turns toward its work. Zero: it does not turn.
    pub turn: u16,
    /// Ticks it must stay on a target before the beam comes on. Zero: it fires as it aims.
    pub charge_ticks: u16,
    /// Where the reclaim beam leaves the model, like a weapon's `muzzle`.
    pub emitter: FxVec3,
}

/// An underground hangar for aircraft. Aircraft that land on its hatch are
/// taken below, where they mend; they leave again through the launch tunnels,
/// one per tunnel every `launch_ticks`. Its `reach` is the ground it looks
/// after: aircraft with nowhere to be that need to land inside it come home,
/// and a guard area set on it must lie inside it.
#[derive(Clone, Debug)]
pub struct Airbase {
    pub capacity: u8,
    pub reach: Fx,
    /// Share of a stored aircraft's full health restored per second.
    pub heal: Fx,
    /// Ticks for the landing hatch to open fully (and to close).
    pub hatch_ticks: u16,
    /// Ticks a tunnel needs between launches.
    pub launch_ticks: u16,
    /// Metres per second an aircraft leaves a tunnel at.
    pub launch_speed: Fx,
    /// Metres an aircraft runs down the inside of a tunnel, speeding up, before the mouth.
    pub run: Fx,
    pub tunnels: Vec<Tunnel>,
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

/// Where an airbase's launch tunnel comes out, in the model's frame (x along its
/// heading), and which way it fires aircraft, off the heading.
#[derive(Clone, Copy, Debug)]
pub struct Tunnel {
    pub mouth: FxVec3,
    pub yaw: Angle,
}

#[derive(Clone, Debug)]
pub struct Weapon {
    pub name: String,
    pub damage: Fx,
    /// Splash radius; zero for a single-target hit.
    pub splash: Fx,
    pub range_min: Fx,
    pub range_max: Fx,
    pub reload_ticks: u16,
    /// Shots per reload cycle and the gap between them. A delay of zero dumps the volley together.
    pub salvo: u8,
    /// Number released simultaneously at each salvo step.
    pub salvo_batch: u8,
    pub salvo_delay_ticks: u8,
    /// Metres per second.
    pub projectile_speed: Fx,
    pub trajectory: Trajectory,
    /// Crosses its range in the tick it is fired (`projectile_speed` is set to do so),
    /// drawn as a beam from the muzzle to what it hit rather than a traveling slug.
    pub hitscan: bool,
    /// Extra ticks a ballistic shell stays up. Zero: it flies at `projectile_speed`.
    pub loft_ticks: u16,
    /// Angle steps per tick. Zero means the weapon is fixed to the hull.
    pub turret_turn: u16,
    /// Half-width of the firing arc around the hull's forward axis; 0x8000 is all-round.
    pub half_arc: u16,
    /// Muzzle position in unit space (x forward, y left, z up).
    pub muzzle: FxVec3,
    /// Tube mouths a volley fires from, in the same space as `muzzle`. Empty: `muzzle` only.
    pub muzzles: Vec<FxVec3>,
    /// The elbow (or trunnion) the weapon pitches about to point up or down at its target,
    /// carrying the muzzle with it. `None`: the weapon only turns.
    pub pivot: Option<FxVec3>,
    /// Random aim error, angle steps.
    pub spread: u16,
    /// Half-angle of the cone ahead of the gun: it fires while anything it may shoot is
    /// in it, angle steps (`RawWeapon::sweep`). Zero: it fires only on target.
    pub sweep: u16,
    pub target_mask: u32,
    pub color: WeaponColor,
    pub missile: bool,
    /// Hit points an intercept laser must burn through. Zero on a missile is a
    /// light casing. Heavier missiles take a longer burst.
    pub intercept_hp: Fx,
    pub guided: bool,
    pub vertical_launch: bool,
    /// Unpowered ejection, mid-air aim, and hang before a guided missile ignites.
    pub cold_launch_ticks: u16,
    pub proximity: Fx,
    pub burn_ticks: u16,
    pub rear: bool,
    /// Which way the weapon rests and its arc is centred, off the nose (180 for `rear`).
    /// A limited arc off the nose also limits what it takes as a target (`RawWeapon::facing`).
    pub facing: Angle,
    /// Multiplies the muzzle flash. 1 is the size the damage implies.
    pub flash: f32,
    /// Multiplies the impact flash. 1 is the size the damage implies.
    pub impact: f32,
    /// Multiplies the muzzle and impact shockwave. 0 is none; 1 is the size the damage implies.
    pub shockwave: f32,
    /// Multiplies the projectile tracer. 1 is the size the damage implies.
    pub tracer: f32,
    /// A projectile wake: blue energy for Blue shots, white smoke for Orange shots.
    pub trail: bool,
    /// Seconds the wake hangs. Zero: the usual hang, when the shot has a trail.
    pub wake: f32,
    /// Multiplies a blue plasma sheath around the traveling slug. 0 is none;
    /// 1 is the size the damage implies. Does not change the tracer, flash, or impact.
    pub plasma: f32,
    /// Blue-white bolts thrown at the muzzle and the impact. Zero: none.
    pub bolts: u8,
    /// Its own turret on the unit's turret: aims about `pivot` by itself and fires while the unit works.
    pub mount: bool,
    /// Reaches what is on the ground or the water along the line of sight, so an
    /// aircraft high up cannot reach it until it comes down. Aircraft: across the map.
    pub slant: bool,
    /// Ticks a rotary gun spins up before it fires. Zero: it fires at once.
    pub spin_ticks: u16,
    /// Rounds each shot is drawn as, spread over the time to the next shot. Cosmetic:
    /// the sim flies one projectile; the mirror draws the rest behind it. One: just the shot.
    pub rounds: u8,
    /// Metres behind the muzzle where spent casings are thrown out, one per round.
    /// Zero: none. Cosmetic: not in the content hash.
    pub casings: f32,
    /// How far a stream gun's tracers lean from deep orange to red, zero to one.
    /// Cosmetic: not in the content hash.
    pub red: f32,
    /// Runs under the water, homing, and can hit a submerged hull (nothing else can).
    pub torpedo: bool,
    /// A guided missile's cruise height over ground and water (a sea skimmer); zero for none.
    pub skim: Fx,
    /// A guided missile's climb before it comes down on its mark (a high arc); zero for none.
    pub apogee: Fx,
    /// Only fires with the hull on the surface (a submarine's deck gun).
    pub surfaced: bool,
    /// Interceptor torpedo tubes: fired at enemy torpedoes in range, never at units.
    pub intercepts: bool,
    /// An Argon Electric Bore: the charge follows the tracer's channel when it lands.
    pub bore: Option<Bore>,
    /// Names from the sound library; what is `None` falls back to the library's defaults.

    pub sounds: WeaponSounds,
    /// Ticks before a salvo at which the weapon is heard charging. Zero: it does not charge.
    pub charge_ticks: u16,
    /// Background text for the interface, from the faction's `lore.ron`. Empty when it has none.
    /// Cosmetic: not in the content hash.
    pub lore: String,
}

impl Weapon {
    /// Casing hit points an intercept laser has to burn through. A non-missile
    /// has none. A missile that does not say otherwise fails in one tick.
    pub fn casing_hp(&self) -> Fx {
        if !self.missile {
            Fx::ZERO
        } else if self.intercept_hp > Fx::ZERO {
            self.intercept_hp
        } else {
            Fx::from_int(10)
        }
    }
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
}
impl Default for EffectSettings {
    fn default() -> Self {
        Self { dust_visibility: 1.0, dust_color: None, dust_brightness: 1.0, dust_lifetime: 1.0, shockwave_color: None }
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
    pub vision: Fx,
    pub radar: Fx,
    /// Sonar reach: finds submerged hulls, which vision and radar cannot.
    pub sonar: Fx,
    /// A submarine's dive.
    pub dive: Option<Dive>,
    pub orbit_radius: Fx,
    pub water_build: bool,
    pub drone: Option<BlueprintId>,
    pub drone_radius: Fx,
    pub anti_missile: Fx,
    /// Its reclaim beam reaches wrecks on the seabed however deep they lie.
    pub deep_reclaim: bool,
    /// Its `mount` guns are houses on the hull, each turning about its own pivot, as a
    /// ship's are, rather than shoulder guns riding the torso.
    pub hull_mounts: bool,
    pub motion: Option<Motion>,
    pub economy: Economy,
    /// A core mine: makes materials out of the ground around it.
    pub mine: Option<Mine>,
    /// A volatile unit: the blast it makes when it is destroyed.
    pub death_blast: Option<DeathBlast>,
    pub builder: Option<Builder>,
    pub reclaimer: Option<Reclaimer>,
    /// An airbase: stores, mends and launches aircraft.
    pub airbase: Option<Airbase>,
    /// A lift ship: carries land units in its hold.
    pub transport: Option<Transport>,
    /// A projected dome, or a hull wrap that only covers this unit.
    pub shield: Option<Shield>,
    pub upgrades_to: Option<BlueprintId>,
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
}

impl UnitBlueprint {
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
        (matches!(m.layer, MoveLayer::Land | MoveLayer::Amphibious | MoveLayer::Hover)
            && self.transport.is_none())
        .then_some(if self.has(cat::COMMANDER) { 8 } else { m.size_class as u16 + 1 })
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

    /// This unit reclaims, or it commands drones that do.
    pub fn sends_reclaimers(&self) -> bool {
        self.reclaims().is_some() || self.drone.is_some()
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
pub const MAX_WEAPONS: usize = 4;

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

    /// The tier a side must have reached before a unit may upgrade itself into `to`.
    /// Economy structures (mines, vaults, reclaim towers) climb only as far as the
    /// side's own tech: the economy cannot run a tier ahead of what can spend it.
    pub fn upgrade_needs(&self, to: &UnitBlueprint) -> u8 {
        if to.has(cat::ECONOMY) {
            // Nothing builds at tech 4 yet: a tier 4 upgrade (the deep core) opens with tech 3.
            to.tech.min(3)
        } else {
            1
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
            for file in sorted_entries(&dir.join("units"))? {
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
                                .map(|(k, lore)| (k, raw::RawUnitLore { lore, ..Default::default() }))
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
        mut sources: Vec<(raw::Faction, Vec<raw::Unit>, BTreeMap<String, raw::RawUnitLore>)>,
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
            factions.push(Faction {
                id: FactionId(i as u8),
                key: f.key.clone(),
                name: f.name.clone(),
                abbreviation: f.abbreviation.clone(),
                description: f.description.clone(),
                commander: lookup(&f.commander, &f.key)?,
                plating_color: f.plating_color,
                accent_color: f.accent_color,
                highlight_color: f.highlight_color,
            });
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
                        1 + m.layer as u64
                            | (m.size_class as u64) << 8
                            | (m.turn_rate as u64) << 16,
                    );
                    h.write_i64(m.speed.0);
                    h.write_i64(m.accel.0);
                    h.write_i64(m.altitude.0);
                    h.write_u64(m.hover as u64);
                    h.write_u64(m.deploy_ticks as u64);
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
                    for v in [
                        m.reach,
                        m.ground,
                        m.per_hectare,
                        m.base,
                    ] {
                        h.write_i64(v.0);
                    }
                }
                None => h.write_u64(u64::MAX),
            }
            match &u.death_blast {
                Some(d) => {
                    h.write_i64(d.radius.0);
                    h.write_i64(d.damage.0);
                }
                None => h.write_u64(u64::MAX),
            }
            h.write_u64(u.water_build as u64);
            h.write_i64(u.orbit_radius.0);
            h.write_i64(u.drone_radius.0);
            h.write_i64(u.anti_missile.0);
            h.write_u64(u.deep_reclaim as u64 | (u.hull_mounts as u64) << 1);
            match &u.dive {
                Some(d) => {
                    h.write_i64(d.depth.0);
                    h.write_u64(d.ticks as u64);
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
                    h.write_u64(r.turn as u64 | (r.charge_ticks as u64) << 16);
                }
                None => h.write_u64(u64::MAX),
            }
            match &u.airbase {
                Some(a) => {
                    h.write_u64(
                        a.capacity as u64
                            | (a.hatch_ticks as u64) << 8
                            | (a.launch_ticks as u64) << 24,
                    );
                    h.write_i64(a.reach.0);
                    h.write_i64(a.heal.0);
                    h.write_i64(a.launch_speed.0);
                    h.write_i64(a.run.0);
                    for t in &a.tunnels {
                        h.write_i64(t.mouth.x.0);
                        h.write_i64(t.mouth.y.0);
                        h.write_i64(t.mouth.z.0);
                        h.write_u64(t.yaw.0 as u64);
                    }
                }
                None => h.write_u64(u64::MAX),
            }
            match &u.transport {
                Some(t) => {
                    h.write_u64(t.capacity as u64 | (t.unload_ticks as u64) << 16);
                    for v in [t.descent, t.hold.x, t.hold.y, t.hinge, t.lip, t.width, t.floor, t.clearance] {
                        h.write_i64(v.0);
                    }
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
                h.write_u64(w.target_mask as u64);
                h.write_u64(
                    w.missile as u64
                        | (w.guided as u64) << 1
                        | (w.vertical_launch as u64) << 2
                        | (w.rear as u64) << 3
                        | (w.torpedo as u64) << 4
                        | (w.surfaced as u64) << 5
                        | (w.intercepts as u64) << 6,
                );
                h.write_i64(w.skim.0);
                h.write_i64(w.apogee.0);
                h.write_i64(w.intercept_hp.0);
                h.write_u64(w.cold_launch_ticks as u64);
                h.write_i64(w.proximity.0);
                h.write_u64(
                    w.burn_ticks as u64
                        | (w.mount as u64) << 16
                        | (w.spin_ticks as u64) << 17
                        | (w.slant as u64) << 33,
                );
                h.write_u64(w.sweep as u64 | (w.facing.0 as u64) << 16);
                if let Some(b) = w.bore {
                    h.write_i64(b.width.0);
                    h.write_i64(b.damage.0);
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
                        m.kit.0 as u64 | (m.after.map_or(0xFF, |a| a) as u64) << 16 | (m.tech as u64) << 24,
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
        assert_eq!(bomber.weapons[0].salvo, 8);
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
            let m = set.slots[s].modules.iter().position(|m| m.key == key).unwrap();
            set.slots[s].modules[m].kit
        };
        let expected: usize = set.slots.iter().map(|s| s.modules.len() + 1).product();
        assert_eq!(set.loadouts.len(), expected);
        assert_eq!(acu.weapons.len(), 1, "it lands with the machine gun alone");

        // The cannon goes on the right arm; the rail cannon only over it, and takes its place.
        let (cannon, rail) = (module("gun", "cannon"), module("gun", "railgun"));
        assert!(!bp.is_listed(cannon));
        assert_eq!(bp.refit_result(acu.id, rail), Err(refit::FitError::Needs(0)));
        let with_cannon = bp.refit_result(acu.id, cannon).unwrap();
        assert!(!bp.is_listed(with_cannon));
        assert_eq!(bp.base_of(with_cannon), acu.id);
        let names = |id: BlueprintId| -> Vec<String> {
            bp.unit(id).weapons.iter().map(|w| w.name.clone()).collect()
        };
        assert_eq!(names(with_cannon), ["Vulcan Machine Gun", "Breach Cannon"]);
        let with_rail = bp.refit_result(with_cannon, rail).unwrap();
        assert_eq!(names(with_rail), ["Vulcan Machine Gun", "Meridian Rail Cannon"]);
        assert_eq!(bp.refit_result(with_rail, cannon), Err(refit::FitError::Fitted));
        // The rail cannon is drawn with the cannon's mount under it.
        let bit = |s: &str, k: &str| {
            let s = slot(s);
            1u32 << set.slots[s].modules.iter().find(|m| m.key == k).unwrap().bit
        };
        assert_eq!(bp.look(with_rail), bit("gun", "cannon") | bit("gun", "railgun"));
        assert_eq!(bp.look(acu.id), 0);

        // The back takes one pack: the shield replaces the formation engine.
        let (mfe, shield) = (module("back", "mfe"), module("back", "shield"));
        let with_mfe = bp.refit_result(with_rail, mfe).unwrap();
        let e = &bp.unit(with_mfe).economy;
        assert_eq!(e.mass_income, acu.economy.mass_income + Fx::from_int(6));
        assert_eq!(e.energy_income, acu.economy.energy_income + Fx::from_int(250));
        let (set2, loadout) = bp.loadout(with_mfe).unwrap();
        assert_eq!(set2.replaces(&loadout.fitted, slot("back"), 1), Some(0));
        let with_shield = bp.refit_result(with_mfe, shield).unwrap();
        assert!(bp.unit(with_shield).shield.is_some_and(|s| s.is_hull()));
        assert_eq!(bp.unit(with_shield).economy.mass_income, acu.economy.mass_income);

        // The engineering suites: II, then III over it, with the tiers they unlock.
        let (eng2, eng3) = (module("engineering", "eng_2"), module("engineering", "eng_3"));
        assert_eq!(bp.refit_result(acu.id, eng3), Err(refit::FitError::Needs(0)));
        let t3 = bp.refit_result(bp.refit_result(acu.id, eng2).unwrap(), eng3).unwrap();
        assert_eq!(bp.unit(t3).tech, 3);
        let power = |id: BlueprintId| bp.unit(id).builder.as_ref().unwrap().power;
        assert_eq!(power(t3), Fx::from_int(70));
        let shatter = bp.id_of("aster_t3_shatter").unwrap();
        assert!(bp.unit(t3).builder.as_ref().unwrap().builds.contains(&shatter));
        let engineer = bp.unit(bp.id_of("aster_t3_engineer").unwrap());
        let mut engineer_builds = engineer.builder.as_ref().unwrap().builds.clone();
        let mut commander_builds = bp.unit(t3).builder.as_ref().unwrap().builds.clone();
        engineer_builds.sort_unstable();
        commander_builds.sort_unstable();
        assert_eq!(
            commander_builds, engineer_builds,
            "Engineering Suite III unlocks the T3 engineer's entire build roster, including experimentals"
        );

        // The shoulder: anti-air, a howitzer, or a second projector; one at a time.
        let aa = module("shoulder", "aa");
        let aux = module("shoulder", "aux_eng");
        let full = bp.refit_result(with_rail, aa).unwrap();
        assert!(bp.unit(full).has(cat::ANTI_AIR));
        assert_eq!(bp.unit(full).weapons.len(), 3);
        let builder = bp.refit_result(t3, aux).unwrap();
        assert_eq!(power(builder), Fx::from_int(110));
        let b = bp.unit(builder).builder.as_ref().unwrap();
        assert_eq!(b.emitters.len(), 1);
        assert!(b.unfold_ticks > 0);
        // Kits carry the module's price, and are nobody's commander.
        assert_eq!(bp.unit(aux).cost_mass, set.module(slot("shoulder"), 2).cost_mass);
        assert!(!bp.unit(aux).has(cat::COMMANDER));
        // Loadouts are worth their base as a wreck, whatever is on them.
        assert_eq!(bp.unit(full).cost_mass * bp.unit(full).wreck_fraction, acu.cost_mass * acu.wreck_fraction);
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
        for (i, file) in sorted_entries(&from.join("units")).unwrap().into_iter().enumerate() {
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
        assert_eq!(bp.unit_by_key("aster_t1_engineer").unwrap().lore, "The shovel.");
        assert!(bp.unit_by_key("aster_t1_tank").unwrap().lore.is_empty());
        assert_eq!(bp.content_hash(), plain.content_hash(), "lore is cosmetic");
        // The older shape: bare strings, the units' text alone.
        let flat = r#"{"aster_t1_engineer": "The shovel."}"#;
        let bp = Blueprints::load(&scratch_data("flat", Some(flat), |t| t)).unwrap();
        assert_eq!(bp.unit_by_key("aster_t1_engineer").unwrap().lore, "The shovel.");
    }

    #[test]
    fn lore_naming_nothing_is_an_error() {
        let err = Blueprints::load(&scratch_data("key", Some(r#"{"aster_nothing": "x"}"#), |t| t))
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
    fn every_aircraft_can_orbit() {
        let bp = Blueprints::load(&data_dir()).unwrap();
        for u in &bp.units {
            if u.motion.is_some_and(|m| m.layer == MoveLayer::Air) {
                assert!(u.orbit_radius >= Fx::from_int(150), "{}", u.key);
            }
        }
        // A radius the unit file names is kept.
        let scout = bp.unit_by_key("aster_t1_air_scout").unwrap();
        assert_eq!(scout.orbit_radius, Fx::from_int(180));
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
        let dust: EffectSettings = ron::from_str("(dust_opacity: 0.3, dust_color: Some((0.7, 0.22, 0.08)), dust_brightness: 2.0)").unwrap();
        assert_eq!(dust.dust_visibility, 0.3);
        assert_eq!(dust.dust_color, Some([0.7, 0.22, 0.08]));
        assert_eq!(dust.dust_brightness, 2.0);
        assert!(ron::from_str::<EffectSettings>("(dust_opacity: 0.3, dust_visibility: 0.7)").is_err());
        let colored: EffectSettings = ron::from_str("(dust_visibility: 0.4, dust_lifetime: 2.0, shockwave_color: Some((0.2, 0.6, 1.0)))").unwrap();
        assert_eq!(colored.dust_visibility, 0.4);
        assert_eq!(colored.dust_lifetime, 2.0);
        assert_eq!(colored.shockwave_color, Some([0.2, 0.6, 1.0]));
        assert!(ron::from_str::<EffectSettings>("(dust_lifetim: 2.0)").is_err());
    }
}
