//! The authoring schema. These types mirror the RON files one to one and are
//! converted to the fixed-point blueprint tables by `compile`.

use crate::{
    cat, BlueprintId, BuildArm, Builder, DataError, Dive, Economy, FactionId, Mine, Motion,
    Reclaimer, Shield, UnitBlueprint, Visual, Weapon, HULL_SHIELD_PAD, MAX_WEAPONS,
};
use mc_core::{Angle, Fx, FxVec2, FxVec3, TICKS_PER_SECOND};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
pub enum MoveLayer {
    Land,
    Amphibious,
    Naval,
    Hover,
    /// Flies over terrain and structures at `altitude`.
    Air,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
pub enum Trajectory {
    /// Flat, fast shot. Blocked by terrain in the way.
    Direct,
    /// Lobbed under gravity; clears terrain and walls.
    Ballistic,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
pub enum WeaponColor {
    Blue,
    Orange,
}

/// How a shield sits on the unit that carries it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Deserialize)]
#[repr(u8)]
pub enum ShieldKind {
    /// A hemisphere on the ground. Covers everything under it.
    #[default]
    Dome,
    /// A field that wraps the carrier. Does not cover the ground around it.
    Hull,
}

/// Strategic icon shape. Tech level adds pips; the owner adds colour. Also the
/// kind of unit the sound library keys its selection sounds by.
///
/// The numbers are the shape ids `icons.wgsl` switches on (checked by mc-render's
/// `every_icon_kind_has_a_shape`): never renumber one, and never reuse a retired one.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Deserialize)]
#[repr(u8)]
pub enum IconKind {
    Commander = 0,
    Engineer = 1,
    Bot = 2,
    Tank = 3,
    Artillery = 4,
    AntiAir = 5,
    Scout = 6,
    Factory = 7,
    Extractor = 8,
    Power = 9,
    Storage = 10,
    Defense = 11,
    Intel = 12,
    Wall = 13,
    Shield = 14,
    /// A slim swept dart, nose up: shoots other aircraft.
    Fighter = 15,
    /// A broad flying wing: drops bombs on the ground.
    Bomber = 16,
    /// A surface warship, in profile.
    Ship = 17,
    /// A submarine, in profile.
    Submarine = 18,
    /// A rotor gunship, from above: crossed blades, body and tail boom. Any
    /// aircraft whose job is attacking the ground from a hover or strafing run.
    Gunship = 19,
    // retired: 20 (airbase)
    /// A capital transport from above: wedge prow and broad drive shoulders.
    Transport = 21,
    // retired: 22 (network gate)
    // retired: 23 (network moorage)
    // retired: 24 (network junction)
    /// A nuclear silo: a missile standing in an open tube, seen from the side.
    Silo = 25,
    /// An interceptor array: an upturned shield with a missile rising out of it.
    AntiNuke = 26,
    /// A capital warship from above: a long narrow spine with a pointed prow, the spinal
    /// gun a line down its centre, and flank sponsons.
    Warship = 27,
    /// A tier-5 titan: a giant striding on two legs, a gun on each arm, framed in corner
    /// brackets so it stands out from everything else on the map.
    Titan = 28,
}

/// One unit's entry in a faction's `lore.ron`: its own text, and its weapons' by weapon name.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
pub(crate) struct RawUnitLore {
    pub lore: String,
    pub weapons: std::collections::BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Faction {
    pub key: String,
    pub name: String,
    pub abbreviation: String,
    pub description: String,
    /// A faction still without a roster of its own names the one whose units it
    /// fields until it has one. Its `commander` is then that faction's.
    #[serde(default)]
    pub stand_in: Option<String>,
    pub commander: String,
    pub plating_color: [f32; 3],
    pub accent_color: [f32; 3],
    pub highlight_color: [f32; 3],
    /// The faction's shield fields. Pale cyan when left out.
    #[serde(default = "default_shield_color")]
    pub shield_color: [f32; 3],
    /// How its structures go up. Presentation only.
    #[serde(default)]
    pub construction: Construction,
    /// Its own voices, over the shared ones (`data/sounds`). A unit file's own still wins.
    #[serde(default)]
    pub sounds: FactionSounds,
}

/// How a faction's construction sites look while they go up. The number reaches the
/// entity shader (`mc-sim` mirror `UNIT_GROWN`): never renumber one.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Deserialize)]
#[repr(u8)]
pub enum Construction {
    /// Printed up in layers under a straight build beam, welded as it goes (ARC).
    #[default]
    Print = 0,
    /// Grown: the site rises out of a molten pool on its lot, veined red and cooling
    /// black, fed by a writhing tendril instead of a beam (the Naga).
    Grow = 1,
}

/// A faction's own voices. Presentation only.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct FactionSounds {
    /// Its units' answer when selected, by the kind of unit (the shared `defaults.select`
    /// for any kind it leaves out).
    pub select: BTreeMap<IconKind, String>,
    /// Its builders at work, in place of the shared `build_beam` / `build_start` /
    /// `build_end` (`data/sounds/build.ron`).
    pub build: Option<BuildSounds>,
}

/// The three sounds of a builder at work.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BuildSounds {
    /// Loops while any of its builders works nearby.
    pub beam: String,
    /// A builder taking up work, and letting it go.
    pub start: String,
    pub end: String,
}

fn default_shield_color() -> [f32; 3] {
    [0.62, 0.84, 1.0]
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct Unit {
    pub key: String,
    pub name: String,
    pub role: String,
    pub tech: u8,
    pub categories: Vec<String>,
    pub health: f64,
    #[serde(default)]
    pub regen: f64,
    pub cost: Cost,
    pub radius: f64,
    pub height: f64,
    #[serde(default)]
    pub footprint: (u8, u8),
    /// Half-extents, metres, of what a structure blocks on the nav grid, in its own
    /// frame (x along its heading). Defaults to the radius both ways.
    #[serde(default)]
    pub hull: Option<(f64, f64)>,
    pub vision: f64,
    #[serde(default)]
    pub radar: f64,
    /// Metres of sonar: the only thing that finds a submerged hull.
    #[serde(default)]
    pub sonar: f64,
    /// A submarine: it dives and surfaces on order, and starts dived.
    #[serde(default)]
    pub dive: Option<RawDive>,
    #[serde(default)]
    pub orbit_radius: f64,
    #[serde(default)]
    pub water_build: bool,
    #[serde(default)]
    pub drone: Option<String>,
    #[serde(default)]
    pub drone_radius: f64,
    #[serde(default)]
    pub anti_missile: f64,
    /// Where the anti-missile lasers stand on the hull (x forward, y left, z up, metres):
    /// each shot is fired from the one nearest the missile. None: from the unit's middle.
    #[serde(default)]
    pub anti_missile_mounts: Vec<(f64, f64, f64)>,
    /// Missiles its lasers burn at once, each from its own mount (default one).
    #[serde(default)]
    pub anti_missile_lasers: u8,
    /// Where its turret turns (x forward, y left, metres): its muzzles and build emitter
    /// swing about this point, not the unit's middle. The Naga commander's tail swivels
    /// about its root on the back. None: the middle.
    #[serde(default)]
    pub turret_at: Option<(f64, f64)>,
    /// A salvage hull: its reclaim beam reaches wrecks on the seabed however deep they lie.
    #[serde(default)]
    pub deep_reclaim: bool,
    /// Guns in houses of their own on the hull, each turning about its `pivot` as a ship's
    /// mounts do: lets a land hull carry them too.
    #[serde(default)]
    pub hull_mounts: bool,
    /// A giant walker's crushing footfalls (`RawStomp`).
    #[serde(default)]
    pub stomp: Option<RawStomp>,
    #[serde(default)]
    pub motion: Option<RawMotion>,
    #[serde(default)]
    pub economy: RawEconomy,
    #[serde(default)]
    pub mine: Option<RawMine>,
    /// Volatile: a blast when it is destroyed ([`crate::DeathBlast`]).
    #[serde(default)]
    pub death_blast: Option<RawDeathBlast>,
    /// A strategic launcher ([`crate::strategic::Strategic`]).
    #[serde(default)]
    pub strategic: Option<crate::strategic::RawStrategic>,
    #[serde(default)]
    pub builder: Option<RawBuilder>,
    #[serde(default)]
    pub reclaimer: Option<RawReclaimer>,
    /// A lift ship: sets down, lowers a ramp and carries land units ([`crate::Transport`]).
    #[serde(default)]
    pub transport: Option<RawTransport>,
    #[serde(default)]
    pub shield: Option<RawShield>,
    #[serde(default)]
    pub upgrades_to: Option<String>,
    /// Parts refitted onto the unit where it stands, slot by slot (`crate::refit`).
    #[serde(default)]
    pub refits: Vec<crate::refit::RawRefitSlot>,
    #[serde(default)]
    pub weapons: Vec<RawWeapon>,
    /// Share of its mass its wreck keeps; by tier when left out ([`default_wreck`]).
    #[serde(default)]
    pub wreck_fraction: Option<f64>,
    #[serde(default)]
    pub effects: crate::EffectSettings,
    pub mesh: String,
    pub icon: IconKind,
    /// Lamps on the hull (`crate::LightMount`); left out, the renderer's usual set.
    #[serde(default)]
    pub lights: Option<Vec<crate::LightMount>>,
    #[serde(default)]
    pub sounds: UnitSounds,
}

/// Share of its mass a unit's wreck keeps: more the higher its tier, so the
/// battlefields of the late game are where the materials are.
fn default_wreck(tech: u8) -> f64 {
    match tech {
        0 | 1 => 0.81,
        2 => 0.85,
        _ => 0.9,
    }
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Cost {
    pub mass: f64,
    pub energy: f64,
    pub time: f64,
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawMotion {
    pub layer: MoveLayer,
    pub size: u8,
    pub speed: f64,
    pub accel: f64,
    /// Degrees per second.
    pub turn: f64,
    /// Cruise height above the surface, metres. Only for `Air`.
    #[serde(default)]
    pub altitude: f64,
    #[serde(default)]
    pub hover: bool,
    /// Seconds to plant before it can fire, and to pack before it can move.
    /// Zero (the default): it fires on the move.
    #[serde(default)]
    pub deploy: f64,
    /// A warship that fights broadside on: engaged and stopped, it lays its hull so the
    /// mark lies this many degrees off the bow, on whichever beam is nearer. Zero: it
    /// only turns to bring its main gun to bear.
    #[serde(default)]
    pub broadside: f64,
    /// A unit whose main turret reaches only this many degrees across its nose (a
    /// scorpion's tail): engaged or at work and stopped, the whole body turns to bring the
    /// mark into that arc, and the turret aims the rest of the way. Zero: no limit.
    #[serde(default)]
    pub aim_arc: f64,
    /// A giant walker: it strides straight over structures, steep ground and shallow water
    /// instead of pathing round them. Land only.
    #[serde(default)]
    pub stride: bool,
}

/// How a submarine dives: `depth` metres of water over its deck when dived, and
/// `time` seconds to go down or come up.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawDive {
    pub depth: f64,
    pub time: f64,
}

/// An Argon Electric Bore: the shot is an argon tracer, and when it lands the charge is
/// dumped down the ionised channel it left. `width`: how far either side of that channel
/// the discharge sears (zero: only the bolt, the blast is the weapon's splash). `damage`:
/// what each enemy along it takes. `cool`: seconds the molten track glows (cosmetic).
#[derive(Deserialize, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub struct RawBore {
    #[serde(default)]
    pub width: f64,
    #[serde(default)]
    pub damage: f64,
    #[serde(default)]
    pub cool: f64,
    /// What is left of the charge after the strike spreads out from the hit as a
    /// lightning storm (`RawStorm`). None: the strike is all of it.
    #[serde(default)]
    pub storm: Option<RawStorm>,
}

/// A giant rail gun's spent sabot: thrown out `back` metres behind the muzzle with every
/// shot, it falls, bursts where it lands (`damage` to enemies within `splash`), and lies
/// there as a wreck of blueprint `wreck` worth `mass` (sabots landing together pile up
/// into one heap).
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct RawSabot {
    pub back: f64,
    pub damage: f64,
    pub splash: f64,
    pub mass: f64,
    pub wreck: String,
}

/// A giant bore's storm: from the hit it grows over `seconds` to `radius` metres, and
/// every enemy in its reach takes up to `damage` a second (full over the inner half,
/// less toward its edge) until it is spent.
#[derive(Deserialize, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub struct RawStorm {
    pub radius: f64,
    pub seconds: f64,
    pub damage: f64,
}

#[derive(Deserialize, Default, Clone)]
#[serde(deny_unknown_fields, default)]
pub struct RawEconomy {
    pub mass_income: f64,
    pub energy_income: f64,
    pub energy_upkeep: f64,
    pub mass_storage: f64,
    pub energy_storage: f64,
}

/// A core mine's economy; see [`crate::Mine`].
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawMine {
    pub reach: f64,
    pub ground: f64,
    pub per_hectare: f64,
    #[serde(default)]
    pub base: f64,
    /// It drives a pile hammer, blow by blow (the ARC core mine). False: it draws its
    /// mass up without striking (the Naga Taproot), so there is no beat to show or hear.
    #[serde(default = "yes")]
    pub hammer: bool,
}

fn yes() -> bool {
    true
}

/// A volatile unit's blast; see [`crate::DeathBlast`].
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawDeathBlast {
    pub radius: f64,
    pub damage: f64,
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawBuilder {
    pub power: f64,
    pub range: f64,
    pub builds: Vec<String>,
    #[serde(default)]
    pub arm: Option<RawBuildArm>,
    /// More construction beams, from these points on the turret (model space, like `emitter`).
    #[serde(default)]
    pub emitters: Vec<(f64, f64, f64)>,
    /// Seconds the gear carrying `emitters` takes to unfold once the unit starts
    /// building; their beams come on when it is out. Zero: they are always out.
    #[serde(default)]
    pub unfold: f64,
    /// Where the gear carrying `emitters` pitches to point at the work, like the arm's elbow.
    #[serde(default)]
    pub hinge: Option<(f64, f64, f64)>,
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawBuildArm {
    /// Degrees per second.
    pub turn: f64,
    pub emitter: (f64, f64, f64),
    /// The elbow the arm pitches about to point at its work.
    #[serde(default)]
    pub pivot: Option<(f64, f64, f64)>,
    /// Shoulder of a two-bone arm. The boom folds about it, then the forearm aims.
    #[serde(default)]
    pub shoulder: Option<(f64, f64, f64)>,
    /// Degrees the forearm sits at when stowed (or the boom, when `shoulder` is set).
    /// Zero: level. Positive folds it up.
    #[serde(default)]
    pub rest: f64,
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawReclaimer {
    pub power: f64,
    pub range: f64,
    /// Degrees per second; zero leaves the head fixed.
    #[serde(default)]
    pub turn: f64,
    /// Seconds the turret must stay on a target before the beam comes on.
    #[serde(default)]
    pub charge: f64,
    pub emitter: (f64, f64, f64),
}

/// A lift ship's hold and ramp, in the model's frame (x along the heading).
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawTransport {
    /// Room in the hold. A land unit takes its size class plus one.
    pub capacity: u16,
    /// Metres per second it climbs and comes down at, easing the last stretch to the ground.
    pub descent: f64,
    /// Where cargo is stowed and let out: the far end of the hold, `(x, y)`.
    pub hold: (f64, f64),
    /// The ramp's hinge at the back of the hold (x), and where its lip meets the ground (x).
    pub ramp: (f64, f64),
    /// Metres across the ramp and the hold.
    pub width: f64,
    /// Height of the hold floor over the ground when it has set down.
    pub floor: f64,
    /// Clear height inside the vehicle hangar.
    pub clearance: f64,
    /// Seconds between units leaving down the ramp.
    pub unload: f64,
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct RawShield {
    #[serde(default)]
    pub kind: ShieldKind,
    /// Dome radius, or the hull wrap. Zero on a hull field: derived from the unit.
    #[serde(default)]
    pub radius: f64,
    pub health: f64,
    /// Hit points per second while the bubble is up, and while it fills after a break.
    pub regen: f64,
}

/// A giant walker's footfall (`stride` walkers): each time it has covered `pace` metres a
/// foot comes down `reach` metres ahead of its middle and `gauge` to the side (left foot
/// first), crushing enemy ground units within `radius` of the sole for `damage`. Structures
/// are stepped over, never on.
#[derive(Deserialize, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawStomp {
    pub pace: f64,
    pub reach: f64,
    pub gauge: f64,
    pub radius: f64,
    pub damage: f64,
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct RawWeapon {
    pub name: String,
    pub damage: f64,
    #[serde(default)]
    pub splash: f64,
    #[serde(default)]
    pub range_min: f64,
    pub range: f64,
    /// Seconds between salvos.
    pub reload: f64,
    #[serde(default = "one")]
    pub salvo: u8,
    /// Projectiles released together at each step of a salvo.
    #[serde(default = "one")]
    pub salvo_batch: u8,
    /// Seconds between batches of a salvo. Zero dumps the volley together.
    #[serde(default)]
    pub salvo_delay: f64,
    /// Metres per second. Ignored by a `hitscan` weapon.
    #[serde(default)]
    pub speed: f64,
    pub trajectory: Trajectory,
    /// The shot crosses the whole range in one tick, and is drawn as a beam from the
    /// muzzle to what it hit rather than a traveling slug.
    #[serde(default)]
    pub hitscan: bool,
    /// An ARC rail gun: a very fast real slug (it flies at `speed` and can miss), drawn
    /// white-hot with a vapour trail along the path it flew and a rail's muzzle blast.
    #[serde(default)]
    pub rail: bool,
    /// Extra seconds a ballistic shell stays up. The shot still lands on the
    /// aim point; it just goes higher. Zero (the default) flies at `speed`.
    #[serde(default)]
    pub loft: f64,
    /// Degrees per second; zero fixes the weapon to the hull.
    #[serde(default)]
    pub turret_turn: f64,
    /// Full firing arc in degrees, centred on the hull's forward axis.
    #[serde(default = "full_arc")]
    pub arc: f64,
    pub muzzle: (f64, f64, f64),
    /// Extra tube mouths a volley walks. Empty: every shot uses `muzzle`.
    #[serde(default)]
    pub muzzles: Vec<(f64, f64, f64)>,
    /// The elbow the weapon pitches about to point up or down at its target.
    #[serde(default)]
    pub pivot: Option<(f64, f64, f64)>,
    /// Aim error in degrees.
    #[serde(default)]
    pub spread: f64,
    /// Half-angle in degrees of a cone ahead of the gun: while anything it can shoot is
    /// in it, it fires along its barrel instead of waiting to be on target. Zero: it
    /// fires only on target.
    #[serde(default)]
    pub sweep: f64,
    pub targets: Vec<String>,
    /// Of `targets`, the kinds it picks first when it chooses for itself: it leaves
    /// anything else for one of these that comes into range. Empty: the nearest.
    #[serde(default)]
    pub prefer: Vec<String>,
    pub color: WeaponColor,
    /// A solid missile casing with a separate motor flame and smoke trail.
    #[serde(default)]
    pub missile: bool,
    /// Hit points an intercept laser must burn through. Zero on a missile is a
    /// light casing that fails in one tick. Heavier missiles take a longer burst.
    #[serde(default)]
    pub intercept: f64,
    #[serde(default)]
    pub guided: bool,
    #[serde(default)]
    pub vertical_launch: bool,
    /// Degrees a vertical-launch cell leans toward the bow off the vertical: the
    /// missile leaves along it. Zero: straight up.
    #[serde(default)]
    pub cant: f64,
    /// Seconds of cold ejection, mid-air aim, and hang before motor ignition.
    #[serde(default)]
    pub cold_launch: f64,
    #[serde(default)]
    pub proximity: f64,
    #[serde(default)]
    pub burn: f64,
    #[serde(default)]
    pub rear: bool,
    /// Fires as one broadside with the unit's other `volley` weapons: a battery that is
    /// ready holds (a few seconds at most) for the others that can bear on the mark.
    #[serde(default)]
    pub volley: bool,
    /// Which way a gun house rests, degrees off the nose, positive to the left
    /// (90: port, -90: starboard, 180: aft). Its `arc` is centred there, and it only
    /// takes targets inside that arc. Unlike `rear`, its muzzle is authored as the
    /// house faces the nose, turned about `pivot`. Zero (the default): the nose, or
    /// aft for a `rear` weapon.
    #[serde(default)]
    pub facing: f64,
    /// Multiplies the muzzle flash. 1 is the size the damage implies.
    #[serde(default = "one_f64")]
    pub flash: f64,
    /// Multiplies the impact flash. 0 (the default) uses `flash`.
    #[serde(default)]
    pub impact: f64,
    /// Multiplies the muzzle and impact shockwave. 0 (the default) is none; 1 is the size the damage implies.
    #[serde(default)]
    pub shockwave: f64,
    /// Multiplies the projectile tracer. 1 is the size the damage implies.
    #[serde(default = "one_f64")]
    pub tracer: f64,
    /// A projectile wake: blue energy for Blue shots, white smoke for Orange shots.
    #[serde(default)]
    pub trail: bool,
    /// Seconds the wake hangs. Zero (the default) is the usual hang, when the shot has a trail.
    #[serde(default)]
    pub wake: f64,
    /// Multiplies a blue plasma sheath around the traveling slug. 0 (the default)
    /// is none; 1 is the size the damage implies.
    #[serde(default)]
    pub plasma: f64,
    /// Blue-white bolts at the muzzle and the impact. Zero (the default) is none.
    #[serde(default)]
    pub bolts: u8,
    /// An electrically charged shell: where it lands, the charge strikes back up the
    /// last this-many metres of its flight as lightning and forks into the ground round
    /// the hit (the electric bore's look, on a lobbed shell). Cosmetic. Zero: none.
    #[serde(default)]
    pub discharge: f64,
    /// A capital rail gun (the Resolute's spinal rail, the Zenith): the firing, the
    /// slug's path and the hit are drawn and heard far bigger than any other rail gun,
    /// scaled by this. Cosmetic. Zero (the default): an ordinary rail.
    #[serde(default)]
    pub heavy_rail: f64,
    /// Degrees a gun house on a capital hull may dip below its deck before its rails
    /// would run into the hull. A mark lower than that is out of its reach (the range
    /// rings show the dead zone under the hull). Zero (the default): no limit.
    #[serde(default)]
    pub depression: f64,
    /// Degrees a gun riding a torso with other guns (a giant's arm) may swing off where
    /// the torso points, and it pitches on its own: it lays on a mark of its own within
    /// that. Not for a unit's first weapon (the torso's own). Zero (the default): it
    /// points where the torso does.
    #[serde(default)]
    pub sway: f64,
    /// Degrees above level a rocket out of an elevated rack (`missile` with `Ballistic`)
    /// leaves at. Zero (the default): the rack's own rake, from `pivot` to `muzzle`.
    #[serde(default)]
    pub rake: f64,
    /// A turret of its own on the unit's turret (a shoulder gun): it turns and pitches
    /// about `pivot` on its own, and keeps firing while the unit builds, unless it is
    /// under water.
    #[serde(default)]
    pub mount: bool,
    /// Reach to what is on the ground or the water measured along the line of sight,
    /// not across the map: an aircraft high up cannot reach the ground with it until it
    /// comes down. Aircraft are still reached across the map.
    #[serde(default)]
    pub slant: bool,
    /// Seconds a rotary gun takes to spin up before it fires; it spins down again
    /// when it has nothing to shoot. Zero: it fires at once.
    #[serde(default)]
    pub spin_up: f64,
    /// A rotary gun that fires as it spins up, faster as it goes: it opens fire a third of
    /// the way into its `spin_up`, its reload this many times longer, and reaches its
    /// `reload` at full spin. Zero (the default): it waits for full spin.
    #[serde(default)]
    pub spin_ramp: f64,
    /// A rotary gun's barrels round its spinning cluster: it fires only as one of them
    /// comes up to the top, and the sim turns the cluster so that happens on a tick at
    /// full spin. Zero (the default): it fires whenever it is ready.
    #[serde(default)]
    pub barrels: u8,
    /// Rounds each shot is seen as: a stream of tracers spread over the time to the
    /// next shot, all carried by the one simulated projectile, so a fast gun reads as
    /// a stream without the sim flying every bullet. Cosmetic. One (the default): the
    /// shot is drawn as it is. Direct-fire guns only.
    #[serde(default = "one")]
    pub rounds: u8,
    /// Metres behind the muzzle where the gun throws out its spent casings, one per
    /// round. Cosmetic. Zero (the default): it throws none.
    #[serde(default)]
    pub casings: f64,
    /// Pushes a stream gun's tracers from deep orange to red, zero to one. Cosmetic.
    #[serde(default)]
    pub red: f64,
    /// Runs under the water to its target, homing on it, and is the only weapon that
    /// can reach a submerged hull. Direct trajectory only.
    #[serde(default)]
    pub torpedo: bool,
    /// A guided missile that flies this many metres over the ground or water until its
    /// terminal run (a sea skimmer). Zero: it flies straight at its mark.
    #[serde(default)]
    pub skim: f64,
    /// A guided missile that climbs to this height before it comes down on its mark (a
    /// high arc). Zero: no climb.
    #[serde(default)]
    pub apogee: f64,
    /// Only fires with the hull on the surface: a submarine's deck gun.
    #[serde(default)]
    pub surfaced: bool,
    /// Interceptor torpedo tubes: fired at enemy torpedoes in `range`, not at units. The
    /// interceptor runs at its torpedo and both burst when they meet (docs/NAVY.md).
    /// Torpedo only.
    #[serde(default)]
    pub intercepts: bool,
    /// An Argon Electric Bore (`RawBore`).
    #[serde(default)]
    pub bore: Option<RawBore>,
    /// A giant gun's spent sabot, thrown clear with every shot (`RawSabot`).
    #[serde(default)]
    pub sabot: Option<RawSabot>,
    #[serde(default)]
    pub sounds: WeaponSounds,
}

/// A weapon's sounds, by name from the sound library (`data/sounds`, and the
/// faction's `sounds.ron`). Leave one out and the library's default plays.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct WeaponSounds {
    pub fire: Option<String>,
    /// Heard `charge_time` seconds before a salvo, while the weapon has a target.
    pub charge: Option<String>,
    pub charge_time: f64,
    /// A rotary gun's barrels starting to turn (`spin_up`): heard as the spin-up begins.
    pub spin: Option<String>,
    /// The hit as heard from far across the map, late by the distance: only for the
    /// biggest guns. Near the camera `impact`/`ground` plays instead.
    pub far: Option<String>,
    /// The shot striking a unit, and striking the ground (`impact` if left out).
    pub impact: Option<String>,
    pub ground: Option<String>,
    /// Multiplies how loud the shot is heard. Zero (the default) is as loud as its damage implies.
    pub volume: f64,
}

/// A unit's own sounds, by name from the sound library.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct UnitSounds {
    pub death: Option<String>,
    /// A loop heard while the unit is moving.
    pub moving: Option<String>,
    /// A walker's footfall: played each time one of its feet comes down, in step with the model.
    /// On a core mine, its hammer striking the pit floor.
    pub step: Option<String>,
    /// A giant's footfall as heard from far away, late by the distance (`Motion::stride`).
    pub step_far: Option<String>,
    /// Heard by its owner when the unit is selected. Left out, the library's
    /// `select` default for the unit's `icon` kind plays.
    pub select: Option<String>,
}

fn one() -> u8 {
    1
}

fn one_f64() -> f64 {
    1.0
}

fn full_arc() -> f64 {
    360.0
}

fn fx(v: f64) -> Fx {
    Fx((v * Fx::ONE.0 as f64).round() as i64)
}

/// Degrees to angle steps.
fn steps(deg: f64) -> f64 {
    deg * 65536.0 / 360.0
}

fn ticks(seconds: f64) -> u32 {
    (seconds * TICKS_PER_SECOND as f64).round().max(0.0) as u32
}

fn mask(names: &[String], ctx: &str) -> Result<u32, DataError> {
    let mut m = 0;
    for n in names {
        m |= cat::parse(n)
            .ok_or_else(|| DataError::Invalid(format!("{ctx}: unknown category {n}")))?;
    }
    Ok(m)
}

impl Unit {
    pub(crate) fn compile(
        &self,
        id: BlueprintId,
        faction: FactionId,
        lookup: &dyn Fn(&str, &str) -> Result<BlueprintId, DataError>,
    ) -> Result<UnitBlueprint, DataError> {
        let key = &self.key;
        let effects = self.effects;
        if !effects.dust_visibility.is_finite()
            || !(0.0..=4.0).contains(&effects.dust_visibility)
            || !effects.dust_brightness.is_finite()
            || !(0.0..=4.0).contains(&effects.dust_brightness)
            || effects.dust_color.is_some_and(|rgb| {
                rgb.iter()
                    .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            })
            || !effects.dust_lifetime.is_finite()
            || !(0.0..=10.0).contains(&effects.dust_lifetime)
            || effects.shockwave_color.is_some_and(|rgb| {
                rgb.iter()
                    .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            })
        {
            return Err(DataError::Invalid(format!("{key}: effects require dust_opacity/dust_visibility and dust_brightness 0..4, dust_lifetime 0..10, and dust_color/shockwave_color RGB 0..1")));
        }
        if let Some(i) = self
            .weapons
            .iter()
            .enumerate()
            .position(|(i, w)| i >= crate::MAX_HOUSES && w.mount)
        {
            return Err(DataError::Invalid(format!(
                "{key}: weapon {i} is on a gun house, but only the first {} weapons can be",
                crate::MAX_HOUSES
            )));
        }
        if self.weapons.len() > MAX_WEAPONS {
            return Err(DataError::Invalid(format!(
                "{key}: more than {MAX_WEAPONS} weapons"
            )));
        }
        if !(1..=crate::MAX_TECH).contains(&self.tech) {
            return Err(DataError::Invalid(format!(
                "{key}: tech must be 1..={}",
                crate::MAX_TECH
            )));
        }
        let mut categories = mask(&self.categories, key)?;
        categories |= if self.motion.is_some() {
            cat::MOBILE
        } else {
            cat::STRUCTURE
        };

        let motion = match &self.motion {
            Some(m) => {
                // Classes 0..=5 (`mc_path::SIZE_CLASSES`); 5 is for the capital ships.
                if m.size > 5 {
                    return Err(DataError::Invalid(format!(
                        "{key}: size class must be 0..=5"
                    )));
                }
                if m.layer == MoveLayer::Air {
                    if m.altitude <= 0.0 {
                        return Err(DataError::Invalid(format!(
                            "{key}: air units need a cruise altitude"
                        )));
                    }
                } else if m.altitude != 0.0 {
                    return Err(DataError::Invalid(format!(
                        "{key}: altitude is only for air units"
                    )));
                }
                if m.stride && m.layer != MoveLayer::Land {
                    return Err(DataError::Invalid(format!(
                        "{key}: only a land walker strides"
                    )));
                }
                if self.stomp.is_some() && !m.stride {
                    return Err(DataError::Invalid(format!(
                        "{key}: a stomp needs a striding walker"
                    )));
                }
                Some(Motion {
                    layer: m.layer,
                    size_class: m.size,
                    speed: fx(m.speed),
                    accel: fx(m.accel),
                    turn_rate: (steps(m.turn) / TICKS_PER_SECOND as f64)
                        .round()
                        .clamp(1.0, 32767.0) as u16,
                    altitude: fx(m.altitude),
                    hover: m.hover,
                    deploy_ticks: ticks(m.deploy).clamp(0, 600) as u16,
                    broadside: Angle(
                        steps(m.broadside.clamp(0.0, 180.0)).round().min(32768.0) as u16
                    ),
                    aim_arc: if m.aim_arc > 0.0 {
                        (steps(m.aim_arc.clamp(1.0, 360.0)) / 2.0)
                            .round()
                            .min(32768.0) as u16
                    } else {
                        0x8000
                    },
                    stride: m.stride,
                })
            }
            None => None,
        };

        let builder = match &self.builder {
            Some(b) => {
                let mut builds = Vec::with_capacity(b.builds.len());
                for k in &b.builds {
                    builds.push(lookup(k, key)?);
                }
                let arm = b.arm.as_ref().map(|a| BuildArm {
                    turn: (steps(a.turn) / TICKS_PER_SECOND as f64)
                        .round()
                        .clamp(1.0, 32767.0) as u16,
                    emitter: FxVec3::new(fx(a.emitter.0), fx(a.emitter.1), fx(a.emitter.2)),
                    pivot: a.pivot.map(|p| FxVec3::new(fx(p.0), fx(p.1), fx(p.2))),
                    shoulder: a.shoulder.map(|p| FxVec3::new(fx(p.0), fx(p.1), fx(p.2))),
                    rest: Angle::from_degrees(a.rest.round() as i32),
                });
                Some(Builder {
                    power: fx(b.power),
                    range: fx(b.range),
                    builds,
                    arm,
                    emitters: b
                        .emitters
                        .iter()
                        .map(|e| FxVec3::new(fx(e.0), fx(e.1), fx(e.2)))
                        .collect(),
                    unfold_ticks: ticks(b.unfold).clamp(0, 600) as u16,
                    hinge: b.hinge.map(|p| FxVec3::new(fx(p.0), fx(p.1), fx(p.2))),
                })
            }
            None => None,
        };

        let mut weapons = Vec::with_capacity(self.weapons.len());
        for w in &self.weapons {
            let ctx = format!("{key}/{}", w.name);
            if w.loft > 0.0 && w.trajectory != Trajectory::Ballistic {
                return Err(DataError::Invalid(format!(
                    "{ctx}: loft is only for ballistic weapons"
                )));
            }
            if w.rounds > 1 && (w.trajectory != Trajectory::Direct || w.missile) {
                return Err(DataError::Invalid(format!(
                    "{ctx}: rounds is only for direct-fire guns"
                )));
            }
            if w.hitscan && (w.trajectory != Trajectory::Direct || w.missile) {
                return Err(DataError::Invalid(format!(
                    "{ctx}: hitscan is only for direct-fire guns"
                )));
            }
            if w.torpedo && (w.trajectory != Trajectory::Direct || w.missile || w.hitscan) {
                return Err(DataError::Invalid(format!(
                    "{ctx}: a torpedo is a direct, non-missile, non-hitscan weapon"
                )));
            }
            if w.rail && (w.trajectory != Trajectory::Direct || w.missile || w.hitscan || w.torpedo)
            {
                return Err(DataError::Invalid(format!(
                    "{ctx}: a rail gun fires a direct slug, not a beam, missile or torpedo"
                )));
            }
            if !w.hitscan && w.speed <= 0.0 {
                return Err(DataError::Invalid(format!(
                    "{ctx}: needs a speed, or hitscan"
                )));
            }
            if w.bore.is_some()
                && (w.trajectory != Trajectory::Direct || w.missile || w.hitscan || w.torpedo)
            {
                return Err(DataError::Invalid(format!(
                    "{ctx}: an electric bore fires a direct tracer round"
                )));
            }
            weapons.push(Weapon {
                name: w.name.clone(),
                damage: fx(w.damage),
                splash: fx(w.splash),
                range_min: fx(w.range_min),
                range_max: fx(w.range),
                reload_ticks: ticks(w.reload).clamp(1, u16::MAX as u32) as u16,
                salvo: w.salvo.max(1),
                salvo_batch: w.salvo_batch.clamp(1, w.salvo.max(1)),
                salvo_delay_ticks: ticks(w.salvo_delay).clamp(0, 255) as u8,
                // A hitscan shot still flies, one step past its range and a third over, so
                // the sim's sweep finds whatever it hits in the tick it is fired.
                projectile_speed: if w.hitscan {
                    fx(w.range.max(1.0) * 4.0 / 3.0 * TICKS_PER_SECOND as f64)
                } else {
                    fx(w.speed)
                },
                trajectory: w.trajectory,
                hitscan: w.hitscan,
                rail: w.rail,
                loft_ticks: ticks(w.loft).clamp(0, 200) as u16,
                turret_turn: (steps(w.turret_turn) / TICKS_PER_SECOND as f64)
                    .round()
                    .clamp(0.0, 32767.0) as u16,
                half_arc: if w.arc >= 360.0 {
                    0x8000
                } else {
                    (steps(w.arc) / 2.0).round() as u16
                },
                muzzle: FxVec3::new(fx(w.muzzle.0), fx(w.muzzle.1), fx(w.muzzle.2)),
                muzzles: w
                    .muzzles
                    .iter()
                    .map(|m| FxVec3::new(fx(m.0), fx(m.1), fx(m.2)))
                    .collect(),
                pivot: w.pivot.map(|p| FxVec3::new(fx(p.0), fx(p.1), fx(p.2))),
                spread: steps(w.spread).round() as u16,
                sweep: steps(w.sweep.clamp(0.0, 90.0)).round() as u16,
                target_mask: mask(&w.targets, &ctx)?,
                prefer_mask: mask(&w.prefer, &ctx)?,
                color: w.color,
                missile: w.missile,
                intercept_hp: fx(w.intercept),
                guided: w.guided,
                vertical_launch: w.vertical_launch,
                cant: Angle::from_degrees(w.cant.clamp(0.0, 80.0).round() as i32),
                cold_launch_ticks: ticks(w.cold_launch).min(600) as u16,
                proximity: fx(w.proximity),
                burn_ticks: ticks(w.burn).min(600) as u16,
                rear: w.rear,
                volley: w.volley,
                facing: if w.facing != 0.0 {
                    Angle(steps(w.facing).round() as i64 as u16)
                } else if w.rear {
                    Angle::from_degrees(180)
                } else {
                    Angle::ZERO
                },
                flash: w.flash.clamp(0.2, 4.0) as f32,
                impact: if w.impact > 0.0 {
                    w.impact.clamp(0.2, 4.0) as f32
                } else {
                    w.flash.clamp(0.2, 4.0) as f32
                },
                shockwave: w.shockwave.clamp(0.0, 4.0) as f32,
                tracer: w.tracer.clamp(0.2, 8.0) as f32,
                trail: w.trail,
                wake: w.wake.clamp(0.0, 4.0) as f32,
                plasma: w.plasma.clamp(0.0, 4.0) as f32,
                bolts: w.bolts.min(32),
                discharge: w.discharge.clamp(0.0, 400.0) as f32,
                heavy_rail: w.heavy_rail.clamp(0.0, 4.0) as f32,
                depression: Angle(steps(w.depression.clamp(0.0, 89.0)).round() as i64 as u16),
                sway: Angle(steps(w.sway.clamp(0.0, 60.0)).round() as i64 as u16),
                rake: Angle(steps(w.rake.clamp(0.0, 80.0)).round() as i64 as u16),
                mount: w.mount,
                slant: w.slant,
                spin_ticks: ticks(w.spin_up).clamp(0, 600) as u16,
                spin_ramp: (w.spin_ramp.clamp(0.0, 20.0) * 100.0).round() as u16,
                barrels: if w.spin_up > 0.0 {
                    w.barrels.min(12)
                } else {
                    0
                },
                rounds: w.rounds.clamp(1, 32),
                casings: w.casings.clamp(0.0, 40.0) as f32,
                red: w.red.clamp(0.0, 1.0) as f32,
                torpedo: w.torpedo,
                skim: fx(w.skim),
                apogee: fx(w.apogee),
                surfaced: w.surfaced,
                intercepts: w.intercepts,
                bore: w.bore.map(|b| crate::Bore {
                    width: fx(b.width.clamp(0.0, 80.0)),
                    damage: fx(b.damage.max(0.0)),
                    cool: b.cool.clamp(0.0, 600.0) as f32,
                    storm: b.storm.map(|s| crate::Storm {
                        radius: fx(s.radius.clamp(1.0, 2000.0)),
                        ticks: ticks(s.seconds).clamp(1, 1200) as u16,
                        damage: fx(s.damage.max(0.0)),
                    }),
                }),
                sabot: match &w.sabot {
                    Some(s) => Some(crate::Sabot {
                        back: fx(s.back.clamp(0.0, 1000.0)),
                        damage: fx(s.damage.max(0.0)),
                        splash: fx(s.splash.clamp(0.0, 500.0)),
                        mass: fx(s.mass.max(0.0)),
                        wreck: lookup(&s.wreck, key)?,
                    }),
                    None => None,
                },
                sounds: w.sounds.clone(),

                charge_ticks: if w.sounds.charge.is_some() {
                    ticks(w.sounds.charge_time).clamp(1, 600) as u16
                } else {
                    0
                },
                lore: String::new(),
            });
        }

        let e = &self.economy;
        // Every shield runs on the grid: without upkeep it could not drop in a stall.
        if self.shield.is_some() && e.energy_upkeep <= 0.0 {
            return Err(DataError::Invalid(format!(
                "{key}: a shield needs energy_upkeep"
            )));
        }
        Ok(UnitBlueprint {
            id,
            key: key.clone(),
            name: self.name.clone(),
            role: self.role.clone(),
            faction,
            tech: self.tech,
            categories,
            health: fx(self.health),
            regen: fx(self.regen),
            cost_mass: fx(self.cost.mass),
            cost_energy: fx(self.cost.energy),
            build_time: fx(self.cost.time),
            radius: fx(self.radius),
            height: fx(self.height),
            footprint: self.footprint,
            hull: match self.hull {
                Some((x, y)) => (fx(x), fx(y)),
                None => (fx(self.radius), fx(self.radius)),
            },
            vision: fx(self.vision),
            radar: fx(self.radar),
            sonar: fx(self.sonar),
            dive: match &self.dive {
                Some(d) => {
                    if self
                        .motion
                        .as_ref()
                        .is_none_or(|m| m.layer != MoveLayer::Naval)
                    {
                        return Err(DataError::Invalid(format!("{key}: only naval hulls dive")));
                    }
                    if d.depth <= 0.0 || d.time <= 0.0 {
                        return Err(DataError::Invalid(format!(
                            "{key}: dive needs a depth and a time"
                        )));
                    }
                    Some(Dive {
                        depth: fx(d.depth),
                        ticks: ticks(d.time).clamp(1, 600) as u16,
                    })
                }
                None => None,
            },
            orbit_radius: fx(self.orbit_radius_or_default()),
            water_build: self.water_build,
            drone: self.drone.as_ref().map(|k| lookup(k, key)).transpose()?,
            drone_radius: fx(self.drone_radius),
            anti_missile: fx(self.anti_missile),
            anti_missile_mounts: self
                .anti_missile_mounts
                .iter()
                .map(|m| FxVec3::new(fx(m.0), fx(m.1), fx(m.2)))
                .collect(),
            anti_missile_lasers: self.anti_missile_lasers.max(1),
            turret_at: self.turret_at.map(|(x, y)| FxVec2::new(fx(x), fx(y))),
            deep_reclaim: self.deep_reclaim,
            hull_mounts: self.hull_mounts,
            stomp: self.stomp.map(|s| crate::Stomp {
                pace: fx(s.pace),
                reach: fx(s.reach),
                gauge: fx(s.gauge),
                radius: fx(s.radius),
                damage: fx(s.damage),
            }),
            motion,
            economy: Economy {
                mass_income: fx(e.mass_income),
                energy_income: fx(e.energy_income),
                energy_upkeep: fx(e.energy_upkeep),
                mass_storage: fx(e.mass_storage),
                energy_storage: fx(e.energy_storage),
            },
            mine: self.mine.as_ref().map(|m| Mine {
                reach: fx(m.reach),
                ground: fx(m.ground),
                per_hectare: fx(m.per_hectare),
                base: fx(m.base),
                hammer: m.hammer,
            }),
            death_blast: match &self.death_blast {
                Some(d) if d.radius <= 0.0 || d.damage <= 0.0 => {
                    return Err(DataError::Invalid(format!(
                        "{key}: a death blast needs a radius and damage"
                    )));
                }
                Some(d) => Some(crate::DeathBlast {
                    radius: fx(d.radius),
                    damage: fx(d.damage),
                }),
                None => None,
            },
            strategic: match &self.strategic {
                Some(s) => Some(s.compile(key, fx, TICKS_PER_SECOND)?),
                None => None,
            },
            builder,
            reclaimer: self.reclaimer.as_ref().map(|r| Reclaimer {
                power: fx(r.power),
                range: fx(r.range),
                turn: (steps(r.turn) / TICKS_PER_SECOND as f64)
                    .round()
                    .clamp(0.0, 32767.0) as u16,
                charge_ticks: ticks(r.charge).clamp(0, 600) as u16,
                emitter: FxVec3::new(fx(r.emitter.0), fx(r.emitter.1), fx(r.emitter.2)),
            }),
            transport: match &self.transport {
                Some(t)
                    if t.capacity == 0
                        || t.descent <= 0.0
                        || t.ramp.1 >= t.ramp.0
                        || t.hold.0 <= t.ramp.0
                        || t.width <= 0.0
                        || t.clearance <= 0.0
                        || !self
                            .motion
                            .as_ref()
                            .is_some_and(|m| m.layer == MoveLayer::Air) =>
                {
                    return Err(DataError::Invalid(format!(
                        "{key}: a lift ship is an aircraft with room, a descent, a hold ahead of its ramp and a width"
                    )));
                }
                Some(t) => Some(crate::Transport {
                    capacity: t.capacity,
                    descent: fx(t.descent),
                    hold: FxVec2::new(fx(t.hold.0), fx(t.hold.1)),
                    hinge: fx(t.ramp.0),
                    lip: fx(t.ramp.1),
                    width: fx(t.width),
                    floor: fx(t.floor),
                    clearance: fx(t.clearance),
                    unload_ticks: ticks(t.unload).clamp(1, 600) as u16,
                }),
                None => None,
            },
            shield: self.shield.as_ref().map(|s| {
                let radius = if s.kind == ShieldKind::Hull && s.radius <= 0.0 {
                    fx(self.radius + HULL_SHIELD_PAD)
                } else {
                    fx(s.radius)
                };
                Shield {
                    kind: s.kind,
                    radius,
                    health: fx(s.health),
                    regen: fx(s.regen),
                }
            }),
            upgrades_to: match &self.upgrades_to {
                Some(k) => Some(lookup(k, key)?),
                None => None,
            },
            weapons,
            wreck_fraction: fx(self
                .wreck_fraction
                .unwrap_or_else(|| default_wreck(self.tech))),
            visual: Visual {
                effects: self.effects,
                mesh: self.mesh.clone(),
                icon: self.icon,
                lights: self.lights.clone(),
            },
            sounds: self.sounds.clone(),
            lore: String::new(),
            refit: None,
            scrap: false,
        })
    }

    /// Every aircraft can orbit. One that does not name a radius circles at three
    /// times its turning radius, and never tighter than 150 m.
    fn orbit_radius_or_default(&self) -> f64 {
        match &self.motion {
            Some(m) if m.layer == MoveLayer::Air && self.orbit_radius <= 0.0 => {
                let turn = m.turn.to_radians().max(0.01);
                (3.0 * m.speed / turn).max(150.0).round()
            }
            _ => self.orbit_radius,
        }
    }
}
