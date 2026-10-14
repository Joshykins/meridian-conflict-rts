//! The authoring schema. These types mirror the RON files one to one and are
//! converted to the fixed-point blueprint tables by `compile`.

use crate::{
    cat, BlueprintId, Body, BuildArm, Builder, DataError, Dive, Economy, FactionId, Mine, Motion,
    Reclaimer, Shield, UnitBlueprint, Visual, Weapon, HULL_SHIELD_PAD, MAX_DRONES, MAX_WEAPONS,
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

/// The Regency's plasma shot, by how hard gravity squeezes it (docs/STYLE.md, "The Regency
/// suite"). The rungs mirror ARC's cannon, rail and electric bore.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
pub enum PlasmaGrade {
    /// A bound charge: a hard red slug.
    Plasmeric,
    /// Condensed by gravity and driven downrange as a tight, dense stream.
    Pinched,
    /// Pushed until it fuses; the gun fires the fusion events out.
    PinchFusion,
    /// A plasma charge held and steered in gravity containment: the Gravitic Seeker
    /// (missiles) and the Gravitic Torpedo. Drawn as a guided munition, never as a gun's
    /// plasma shot (`Weapon::plasma_shot`).
    Gravitic,
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
    /// A reclaim structure: the Extractor's disc (salvage feeds materials as a mine does)
    /// inside a ring for its reach. Every salvage icon carries that disc.
    Salvage = 29,
    // retired: 30 (the salvage boat).
    // 31 is the shader's own unidentified radar contact (icons.wgsl), never a kind.
    /// A salvage carrier (the commander's drone port) from above: the disc with swept
    /// wings and a tail.
    SalvageCarrier = 32,
    /// A carrier's salvage drone: the disc alone, drawn small.
    SalvageDrone = 33,
    /// A torpedo bomber from above: a gull-winged plane with a finned torpedo under it.
    TorpedoBomber = 34,
    /// A sensor ship from above: a hammerhead of sensors across a slim hull, the
    /// eye-line cut across the head.
    SensorShip = 35,
    /// A warp dampener: a broken ring (its field) with four chevrons pulling in on a
    /// point at its middle, where it drags a jump down.
    Damper = 36,
    /// A space elevator: a cable coming down into three clamp jaws.
    Elevator = 37,
    /// A tier-5 winged titan from the front, hovering: three blade feathers fanned off
    /// each shoulder vane, guns hung at its sides, legs trailing to points. Framed like
    /// the [`IconKind::Titan`].
    WingedTitan = 38,
    /// A four-legged siege walker in profile: legs arched over a slung hull, the beam
    /// head low at the front, a rocket pack on its back.
    SiegeWalker = 39,
    /// A reverse-kneed assault walker in profile: a hunched hull with a launcher hump,
    /// a low pointed head and the gun arm out in front.
    AssaultWalker = 40,
    /// A tier-5 gravitic engine from the front: a crown of shards leaning in to a point
    /// over a well, on a broken ground line. Framed like the [`IconKind::Titan`].
    GraviticEngine = 41,
    /// An air staging base: a bunker in the ground and a V over it pointing down into it.
    AirStaging = 42,
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
    /// Its skin (`data/skins/<skin>.ron`): what its models' surfaces look like.
    pub skin: String,
    /// Its crest's colour, linear RGB.
    #[expect(
        dead_code,
        reason = "the set-up screens read it in their own parse (mc-game ui/faction.rs)"
    )]
    pub highlight_color: [f32; 3],
    /// Its adjacency conduits: the lines between buildings that save each other upkeep.
    pub power_line: PowerLine,
    /// The faction's shield fields. Pale cyan when left out.
    #[serde(default = "default_shield_color")]
    pub shield_color: [f32; 3],
    /// How its structures go up. Presentation only.
    #[serde(default)]
    pub construction: Construction,
    /// Its own voices, over the shared ones (`data/sounds`). A unit file's own still wins.
    #[serde(default)]
    pub sounds: FactionSounds,
    /// How its torpedoes look running. Presentation only.
    #[serde(default)]
    pub torpedo_look: TorpedoLook,
    /// How its structures light themselves after dusk. Presentation only.
    #[serde(default)]
    pub structure_lamps: StructureLamps,
    /// How its shield fields look, domes and personal fields alike. Presentation only.
    #[serde(default)]
    pub shield_look: ShieldLook,
    /// How its missile defence is seen killing a missile. Presentation only.
    #[serde(default)]
    pub anti_missile_look: AntiMissileLook,
    /// How its strategic missiles and nuclear blasts look and sound. Presentation only.
    #[serde(default)]
    pub nuke_look: crate::strategic::StrategicLook,
}

/// How a faction's shield fields are drawn (shields.wgsl, entity.wgsl `fs_hull`). The
/// number reaches the shaders in the shield's `packed` word (`mc_sim::mirror`,
/// `gpu_consts::shield_look`): never renumber one.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Deserialize)]
#[repr(u8)]
pub enum ShieldLook {
    /// Glass in the faction's `shield_color` over a honeycomb, a launch beam climbing to
    /// the pole (ARC).
    #[default]
    Honeycomb = 0,
    /// A veil in pinch fusion's prism: white-hot glass with pastel colour drifting over
    /// it like a curtain, on a lattice of red-tinged triangles (the Regency).
    Prism = 1,
    /// Survival's Progenitor (a Shaper's dome, `effects: (shield_look: Some(Progenitor))`):
    /// panes of hard light with lit seams that flash and knit again where struck, more
    /// courses and rings round it the higher its Shaper's tier.
    Progenitor = 2,
}

/// How a faction's missile defence (`anti_missile`) is drawn taking a missile down
/// (mc-render `laser_fx`, `crush_fx`). Only the look: what it kills, and when, is the
/// sim's either way.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Deserialize)]
pub enum AntiMissileLook {
    /// A red laser that strikes the casing at full brightness and fades fast, once a tick
    /// until it fails (ARC).
    #[default]
    Laser,
    /// The Regency's gravity lens: the mount takes hold of the missile, a thin red tether
    /// rippling out to it and a ring of bent light closing round it each tick, and crushes
    /// it, imploding and popping red, hard and short (the Regency).
    Gravitic,
}

/// How a faction's adjacency conduits run and look (mc-render `adjacency_links`): one line
/// per link from the provider's lot centre to the neighbour's. Presentation only.
#[derive(Clone, Copy, PartialEq, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PowerLine {
    /// The light of an energy line (a materials line is lit in the interface's materials
    /// colour). Linear RGB.
    pub color: [f32; 3],
    pub path: LinePath,
    pub look: LineLook,
}

/// The way a conduit runs between two buildings.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
pub enum LinePath {
    /// Square runs along the lot axes: out of the provider across the seam, a turn along
    /// it where the two are not in line, and across again into the neighbour.
    Straight,
    /// One smooth arc bowed to the side, from centre to centre.
    Curve,
    /// Runs along the lot axes and at 45 degrees between them, turning only at 45.
    Diagonal,
}

/// What the conduit is made of, and what stands along it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
pub enum LineLook {
    /// An armoured cable pinned down by steel clamps at intervals, a junction box at
    /// each turn (ARC).
    Clamped,
    /// A cable under lapped dark plates with lit seams, running between low faceted
    /// field nodes (the Regency).
    Plated,
}

/// The lamps a faction's structures carry at night when their data names none
/// (mc-render `lights`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Deserialize)]
pub enum StructureLamps {
    /// Warm sodium floodlights high on the corners, thrown across the yard (ARC).
    #[default]
    Sodium,
    /// Low red embers round the foot of the plating: pools of dull red light on the
    /// ground and the lower plates, the yard left dark (the Regency).
    Ember,
}

/// How a torpedo looks running: its body under the water (sprites.wgsl) and what it
/// leaves on the surface (renderer `water_fx`, water.wgsl). A weapon names its own
/// (`RawWeapon::torpedo_look`); otherwise an interceptor runs `Sprint` and any other
/// torpedo its side's faction's. The number crosses to the renderer in
/// `ProjectileInstance::wake`: never renumber one.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Deserialize)]
#[repr(u8)]
pub enum TorpedoLook {
    /// Compressed air: a white seam of bubbles coming up over a pale band (ARC).
    #[default]
    Bubbles = 0,
    /// A plasma drive boiling the water: a glassy steam line with few bubbles and a
    /// red glow under it (the Regency).
    Plasma = 1,
    /// An interceptor's sprint: a thin fizzing line snaking behind a small bright
    /// motor, gone in a few seconds.
    Sprint = 2,
    /// A heavy torpedo: a long fat body, twin screws throwing up a broad doubled
    /// wake that lies on the water a long time.
    Heavy = 3,
    /// A long-range pump-jet: quiet, its air let go in gulps, a dotted line.
    Pulse = 4,
}

/// How a faction's construction sites look while they go up. The number reaches the
/// entity shader (`mc-sim` mirror `UNIT_NANITE`): never renumber one.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Deserialize)]
#[repr(u8)]
pub enum Construction {
    /// Printed up in layers under a straight build beam, welded as it goes (ARC).
    #[default]
    Print = 0,
    /// Built by nanites: a black swarm poured from the builder in a stream condenses into
    /// the structure, violet light with red in it running through it (the Regency).
    Nanite = 1,
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
    /// What it is in a word or two, the build tile's title over its name (`UnitBlueprint::title`).
    #[serde(default)]
    pub title: Option<String>,
    /// Only playtest and dev builds have it: a release build leaves it out, and every
    /// build list and upgrade that names it (`crate::playtest`).
    #[serde(default)]
    pub playtest: bool,
    pub tech: u8,
    pub categories: Vec<String>,
    pub health: f64,
    #[serde(default)]
    pub regen: f64,
    /// Its regen is nanites at work: the nanites are drawn crawling over the hull while it
    /// mends.
    #[serde(default)]
    pub nanite_regen: bool,
    pub cost: Cost,
    pub radius: f64,
    pub height: f64,
    #[serde(default)]
    pub footprint: (u8, u8),
    /// Half-extents, metres, of what a structure blocks on the nav grid, in its own
    /// frame (x along its heading). Defaults to the radius both ways.
    #[serde(default)]
    pub hull: Option<(f64, f64)>,
    /// The hull shots strike, for a long one a disc of `radius` misfits (`Body`).
    #[serde(default)]
    pub body: Option<RawBody>,
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
    pub water_build: bool,
    /// Built on the seabed under deep water (with `water_build`): it stands on the bottom,
    /// wholly under the surface, so only sonar finds it and only torpedoes reach it. Its
    /// model's spire runs up to the surface whatever the depth.
    #[serde(default)]
    pub seabed: bool,
    /// A salvage drone it makes for itself and sends out to reclaim wrecks.
    #[serde(default)]
    pub drone: Option<String>,
    /// How far from it its drones go looking for wrecks.
    #[serde(default)]
    pub drone_radius: f64,
    /// Where its drones sit when home (x forward, y left, z up, metres, on the turret
    /// when it has one): one drone for each.
    #[serde(default)]
    pub drone_sockets: Vec<(f64, f64, f64)>,
    /// Metres above (positive) or below (negative) its socket that a drone lines up
    /// before it settles onto a pad or rises onto a pylon, and drops to letting go.
    #[serde(default)]
    pub drone_approach: f64,
    /// An aircraft that orbits (`UnitBlueprint::orbit`): the radius it circles, metres,
    /// where it is left idle.
    #[serde(default)]
    pub orbit: Option<f64>,
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
    /// swing about this point, not the unit's middle: the Regency battle scorpion's tail bends
    /// round from its root on the back. None: the middle.
    #[serde(default)]
    pub turret_at: Option<(f64, f64)>,
    /// Guns in houses of their own on the hull, each turning about its `pivot` as a ship's
    /// mounts do: lets a land hull carry them too.
    #[serde(default)]
    pub hull_mounts: bool,
    /// A giant walker's crushing footfalls (`RawStomp`).
    #[serde(default)]
    pub stomp: Option<RawStomp>,
    /// Dashes into the middle of the enemies it sees ([`crate::dash::Dash`]).
    #[serde(default)]
    pub dash: Option<crate::dash::RawDash>,
    /// Tears a rift to any point and holds a gravity well there
    /// ([`crate::gravitic::Gravitic`]).
    #[serde(default)]
    pub gravitic: Option<crate::gravitic::RawGravitic>,
    /// The Edict, a barrel-less main gun fired from the hull ([`crate::edict::Edict`]).
    #[serde(default)]
    pub edict: Option<crate::edict::RawEdict>,
    #[serde(default)]
    pub motion: Option<RawMotion>,
    #[serde(default)]
    pub economy: RawEconomy,
    #[serde(default)]
    pub mine: Option<RawMine>,
    /// A material fabricator ([`crate::Fabricator`]).
    #[serde(default)]
    pub fabricator: Option<RawFabricator>,
    /// Volatile: a blast when it is destroyed ([`crate::DeathBlast`]).
    #[serde(default)]
    pub death_blast: Option<RawDeathBlast>,
    /// What it saves the buildings it stands against ([`crate::Adjacency`]).
    #[serde(default)]
    pub adjacency: Option<RawAdjacency>,
    /// A strategic launcher ([`crate::strategic::Strategic`]).
    #[serde(default)]
    pub strategic: Option<crate::strategic::RawStrategic>,
    /// A space elevator ([`crate::elevator::Elevator`]).
    #[serde(default)]
    pub elevator: Option<crate::elevator::RawElevator>,
    /// A Lance: a great beam that charges on its target, then strikes ([`crate::lance::Lance`]).
    #[serde(default)]
    pub lance: Option<crate::lance::RawLance>,
    /// What grows with the unit's level ([`crate::growth::Growth`]).
    #[serde(default)]
    pub growth: Option<crate::growth::RawGrowth>,
    #[serde(default)]
    pub builder: Option<RawBuilder>,
    #[serde(default)]
    pub reclaimer: Option<RawReclaimer>,
    /// A lift ship: sets down, lowers a ramp and carries land units ([`crate::Transport`]).
    #[serde(default)]
    pub transport: Option<RawTransport>,
    /// A capital ship's warp drive ([`crate::Warp`]).
    #[serde(default)]
    pub warp: Option<RawWarp>,
    /// Drags enemy warps down around it ([`crate::WarpDamper`]).
    #[serde(default)]
    pub warp_damper: Option<RawWarpDamper>,
    /// Mends every friendly unit around it ([`crate::RepairField`]).
    #[serde(default)]
    pub repair_field: Option<RawRepairField>,
    /// An aircraft's tank ([`crate::Fuel`]).
    #[serde(default)]
    pub fuel: Option<RawFuel>,
    /// Where aircraft come down to refuel and mend ([`crate::Staging`]).
    #[serde(default)]
    pub staging: Option<RawStaging>,
    /// Builds air wings aboard ([`crate::AirWings`]).
    #[serde(default)]
    pub air_wings: Option<RawAirWings>,
    #[serde(default)]
    pub shield: Option<RawShield>,
    #[serde(default)]
    pub upgrades_to: Option<String>,
    /// Build power it puts its own upgrade on with, if it is not a builder
    /// (`UnitBlueprint::upgrade_power`).
    #[serde(default)]
    pub upgrade_power: Option<f64>,
    /// Parts refitted onto the unit where it stands, slot by slot (`crate::refit`).
    #[serde(default)]
    pub refits: Vec<crate::refit::RawRefitSlot>,
    #[serde(default)]
    pub weapons: Vec<RawWeapon>,
    /// Share of its materials its wreck keeps; by tier when left out ([`default_wreck`]).
    #[serde(default)]
    pub wreck_fraction: Option<f64>,
    #[serde(default)]
    pub effects: crate::EffectSettings,
    #[serde(default)]
    pub skin: Option<String>,
    /// Available to scenario authors, excluded from skirmish production/catalogues.
    #[serde(default)]
    pub mission_only: bool,
    pub mesh: String,
    pub icon: IconKind,
    /// Lamps on the hull (`crate::LightMount`); left out, the renderer's usual set.
    #[serde(default)]
    pub lights: Option<Vec<crate::LightMount>>,
    #[serde(default)]
    pub sounds: UnitSounds,
}

/// Share of its materials a unit's wreck keeps: more the higher its tier, so the
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
    pub materials: f64,
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
    /// With `deploy`: it plants whenever it stands with nowhere to go, not only once it has
    /// something to shoot, so one parked ahead of a raid is ready for it.
    #[serde(default)]
    pub deploy_idle: bool,
    /// A warship that fights broadside on (at sea or in the air): engaged and stopped, it
    /// lays its hull so the mark lies this many degrees off the bow, on whichever beam is
    /// nearer. Zero: it only turns to bring its main gun to bear.
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
    /// A hover titan riding high on its lift (the Seraph): it floats straight over every
    /// unit and structure in its way, rising over what is under it, and over steep ground
    /// and the sea, instead of pathing round them. Another titan still stands in its way.
    /// Hover only.
    #[serde(default)]
    pub floats: bool,
    /// A hover aircraft that hangs still near what it fights (the Reaper), its guns laid
    /// on it, instead of circling it as a gunship does. Air hover only.
    #[serde(default)]
    pub hangs: bool,
    /// An aircraft that cruises above the weather (a spy plane): a gun or launcher that
    /// would take it must reach it along the line of sight, its height included, not only
    /// across the map (`World::slant_reaches`). Air only.
    #[serde(default)]
    pub above_weather: bool,
    /// A jet that fights from a circle near the reach of its guns (the Voulge) rather than
    /// making runs over its mark. Air, not hover, only.
    #[serde(default)]
    pub stand_off: bool,
}

/// The hull shots strike (`Body`): metres forward and aft of the origin along the
/// heading to its ends, and its half width.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawBody {
    pub fore: f64,
    pub aft: f64,
    pub beam: f64,
}

/// How a submarine dives: `depth` metres of water over its deck when dived, and
/// `time` seconds to go down or come up.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawDive {
    pub depth: f64,
    pub time: f64,
}

/// A cone weapon (the Regency Wake Tank's wake): each shot rolls out from the muzzle as one
/// front, `speed` metres a second, across `angle` degrees either side of the gun's facing
/// (up to 180, a full circle) and out to its range, and strikes everything it may shoot as the front reaches it,
/// rather than one target. What stands nearer takes more: full `damage` at the muzzle,
/// falling in a straight line to `edge` of it at full range. Ground between the muzzle and
/// a unit shields it (the wake rolls over the ground; a crest stops it), and so does a
/// shield dome it is under. It never reaches what is under the water, nor aircraft unless
/// its `targets` say so. The shot is `hitscan`: nothing flies but the front.
#[derive(Deserialize, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub struct RawCone {
    pub angle: f64,
    pub speed: f64,
    #[serde(default = "RawCone::default_edge")]
    pub edge: f64,
    /// Seconds the front stuns what it strikes (an EMP: no orders, fire or way on), all but
    /// experimentals; not what a shield dome covers. Zero (the default): no stun.
    #[serde(default)]
    pub stun: f64,
}

impl RawCone {
    fn default_edge() -> f64 {
        0.4
    }
}

/// A gravitic lens (the Regency's tech 3 lens): its shot is no round but a front of
/// gravity that runs out to its target at the weapon's `speed`, following it, and opens a
/// well `radius` metres round it where it strikes. The well sits on the target, wherever
/// it goes, until the target dies, leaves the gun's range or the gun lets go. Everything
/// of the enemy's in it (never aircraft) takes the weapon's `damage` a second and keeps
/// `slow` of its speed, the same anywhere in it and however long it is held.
#[derive(Deserialize, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub struct RawLens {
    pub radius: f64,
    pub slow: f64,
}

/// A strafing beam: held beams fixed in an aircraft's hull, one from each of its
/// `muzzles` (port and starboard), laid down along the nose so each lands `ahead` metres
/// in front of the hull, `gap` metres apart, toed out from the muzzles. On a run the beams come on as where they
/// land reaches `length` / 2 short of the mark, rake the ground through it as the
/// aircraft flies on, and go out `length` / 2 past it: two lines seared over the ground,
/// striking what stands on them. A beam swings a little off the nose (`arc`) to stay on a
/// mark off the run's line. Only on an aircraft that makes runs (not `hover`).
#[derive(Deserialize, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub struct RawStrafe {
    pub ahead: f64,
    pub length: f64,
    pub gap: f64,
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
    /// The fireball of ionised air the strike raises where it lands, metres across its
    /// heart, and how long it burns, seconds. Cosmetic; zero: none (the weapon's own
    /// splash alone).
    #[serde(default)]
    pub blast: f64,
    #[serde(default)]
    pub blast_time: f64,
    /// What is left of the charge after the strike spreads out from the hit as a
    /// lightning storm (`RawStorm`). None: the strike is all of it.
    #[serde(default)]
    pub storm: Option<RawStorm>,
}

/// An Arc Howitzer's tube, for its firing sequence (`RawWeapon::howitzer`).
#[derive(Deserialize, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub struct RawHowitzer {
    /// Metres from the breech to `muzzle`.
    pub length: f64,
    /// Half the breech housing's height, metres (the model's `r`).
    pub radius: f64,
}

/// A gun's spent casing: thrown out of its ejection port at `port` (unit space, in the
/// gun's frame like `muzzle`, so it turns and pitches with the gun) with every shot, along
/// `throw` (a direction in the same frame) at `kick` m/s. It falls, bursts where it lands
/// (`damage` to enemies within `splash`), lies there a while and sinks away: it is worth
/// nothing. `casing` is the hidden blueprint it is drawn as.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct RawSabot {
    pub port: (f64, f64, f64),
    pub throw: (f64, f64, f64),
    pub kick: f64,
    pub damage: f64,
    pub splash: f64,
    pub casing: String,
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
    pub materials_income: f64,
    pub energy_income: f64,
    pub energy_upkeep: f64,
    pub materials_storage: f64,
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
    /// Energy a second drawn while it grows; see [`crate::Mine::grow_upkeep`].
    #[serde(default)]
    pub grow_upkeep: f64,
    /// It drives a pile hammer, blow by blow (the ARC core mine). False: it draws its
    /// materials up without striking (the Regency Taproot), so there is no beat to show or hear.
    #[serde(default = "yes")]
    pub hammer: bool,
}

/// A material fabricator's output; see [`crate::Fabricator`].
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawFabricator {
    /// Materials a second, with its energy upkeep paid in full.
    pub materials: f64,
}

/// A provider's saving for its neighbours; see [`crate::Adjacency`].
#[derive(Deserialize, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub(crate) enum RawAdjacency {
    /// Saves energy, by the energy it makes.
    Energy,
    /// Saves materials, by the materials it makes.
    Materials,
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
    /// Builds its products in the sky over it (`Builder::aloft`).
    #[serde(default)]
    pub aloft: bool,
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
    /// Materials a second the whole unit pulls, split across its working heads.
    pub power: f64,
    pub range: f64,
    /// Seconds a head must stay on a target before its beam comes on.
    #[serde(default)]
    pub charge: f64,
    /// Keeps reclaiming what it passes while it moves (`Reclaimer::mobile`).
    #[serde(default)]
    pub mobile: bool,
    pub heads: Vec<RawReclaimHead>,
}

/// One reclaim head (`mc_data::ReclaimHead`), in the model's frame.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawReclaimHead {
    pub emitter: (f64, f64, f64),
    /// Its yaw axis and pitch trunnion. Absent: it turns about the unit's middle, level.
    #[serde(default)]
    pub pivot: Option<(f64, f64, f64)>,
    /// Degrees per second; zero leaves the head fixed.
    #[serde(default)]
    pub turn: f64,
    /// How far it pitches, degrees off level, `(down, up)`: down is written negative and
    /// up positive, so `(-85, 80)` is 85 down (almost straight down) and 80 up.
    #[serde(default = "default_head_pitch")]
    pub pitch: (f64, f64),
}

fn default_head_pitch() -> (f64, f64) {
    (-85.0, 85.0)
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

/// A capital ship's warp drive.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawWarp {
    /// Energy a jump's charge takes for each kilometre it reaches (at least one).
    pub per_km: f64,
    /// The base of the charge's time, seconds: a jump of `km` kilometres charges for
    /// `spool * (10 + km) / 10` at full power (`Warp::charge_ticks`).
    pub spool: f64,
    /// Seconds after coming out before it can spool again.
    pub cooldown: f64,
    /// Metres per second in warp.
    pub speed: f64,
    /// How the jump looks and sounds ([`crate::WarpStyle`]); ARC's by default.
    #[serde(default)]
    pub style: crate::WarpStyle,
}

/// A nanite repair field: the friendly units within `radius` metres heal `rate` of their
/// full health a second.
#[derive(Deserialize, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub struct RawRepairField {
    pub radius: f64,
    /// Health a second each friend in reach gets back, the same for every unit.
    pub heal: f64,
    /// The carrier mends itself by the same amount.
    #[serde(default)]
    pub mends_self: bool,
}

/// An aircraft's tank: `range` kilometres of flight at full speed.
#[derive(Deserialize, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawFuel {
    pub range: f64,
}

/// An air staging base: aircraft within `radius` metres that run low come down on it.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawStaging {
    pub radius: f64,
    /// Aircraft it takes in at once.
    pub pads: u8,
    /// Seconds an empty tank takes to fill.
    pub refuel: f64,
    /// Share of its full health an aircraft aboard mends a second.
    pub repair: f64,
    /// Where aircraft come down, `(x, y, z)` in the model's frame (default: the middle).
    #[serde(default)]
    pub land: (f64, f64, f64),
    /// Where they leave from, `(x, y, z, heading in degrees)`, taken in turn (default:
    /// where they came down, nose along the base). At most `MAX_LAUNCHES`.
    #[serde(default)]
    pub launch: Vec<(f64, f64, f64, f64)>,
    /// Seconds between two launches.
    #[serde(default = "default_launch_gap")]
    pub launch_gap: f64,
    /// Seconds its hatch takes to open for an aircraft coming down (`Units::deploy`).
    /// None: nothing to open.
    #[serde(default)]
    pub doors: Option<f64>,
}

/// Air wings a carrier builds aboard, one at a time with `power`.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawAirWings {
    pub power: f64,
    /// Metres round the carrier a launched wing guards.
    pub guard: f64,
    pub wings: Vec<RawWing>,
}

/// One kind of wing: `count` of the aircraft `aircraft`, built and launched together.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawWing {
    pub name: String,
    pub aircraft: String,
    pub count: u8,
}

fn default_launch_gap() -> f64 {
    1.0
}

/// A warp dampener's field.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawWarpDamper {
    pub radius: f64,
    /// How many times longer a dampened transit lasts.
    pub drag: f64,
    /// Share of its full health a dampened ship loses coming out, 0 to 1.
    pub damage: f64,
    /// Seconds a dampened ship lies stunned.
    pub stun: f64,
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
    /// Hit points per second while the bubble is up.
    pub regen: f64,
    /// Hit points per second while it fills after a break. Unset: twice `regen`.
    #[serde(default)]
    pub recharge: Option<f64>,
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
    /// Energy a second the reload draws from the grid while it runs: the reload goes only
    /// as fast as it is paid, and faster and cheaper by what neighbouring power plants
    /// save it (`mc_sim::fed_reload`). Zero (the default): the reload is free.
    #[serde(default)]
    pub reload_energy: f64,
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
    /// A held beam (a Regency beam weapon): a `hitscan` gun that fires every tick it bears,
    /// drawn as one steady stream from the muzzle to what it strikes, glassing the ground
    /// under where it lands, and heard as its `hold` loop, not a shot a tick. Cosmetic.
    #[serde(default)]
    pub beam: bool,
    /// An ARC rail gun: a very fast real slug (it flies at `speed` and can miss), drawn
    /// white-hot with a vapour trail along the path it flew and a rail's muzzle blast.
    #[serde(default)]
    pub rail: bool,
    /// A flak shell: a slow, visible round with a timed fuse. It bursts on its own at the
    /// point it was laid on (the lead on an aircraft) if nothing sets its proximity fuse
    /// off first, so a near miss still catches the flight in its `splash`. Drawn as a
    /// glowing shell and a black-cored airburst (renderer `flak_fx`).
    #[serde(default)]
    pub flak: bool,
    /// A plasma airburst (the Regency's): the timed fuse of a `flak` shell, bursting at the
    /// point it was laid on if its proximity fuse has not gone off first, drawn as its
    /// `plasma_grade`'s own burst, not as flak.
    #[serde(default)]
    pub airburst: bool,
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
    /// A weapon group: this one weapon is a gun on each of these houses, each with its own
    /// pivot, rest and arc and its own aim, mark and reload (`crate::mounts`). Its `muzzle`
    /// and `muzzles` are then given about the pivot, the house facing the nose. Empty (the
    /// default): one gun.
    #[serde(default)]
    pub mounts: Vec<crate::mounts::RawMount>,
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
    /// light casing that fails in one tick. Heavier missiles take a longer burst. On a
    /// shell, more than zero lets the lasers shoot it down; zero, they never try.
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
    /// Of `cold_launch`, seconds a booster drives the missile straight up out of its cell
    /// in place of the pneumatic toss; the rest it coasts while thrusters swing its nose
    /// onto the mark. Zero: a toss.
    #[serde(default)]
    pub boost: f64,
    /// Seconds the cell hatches take to open: a salvo waits for them, and they shut
    /// again while the cells reload.
    #[serde(default)]
    pub hatch: f64,
    /// Each missile of a salvo goes after a different target in range while there are
    /// any not already chased; otherwise they share one.
    #[serde(default)]
    pub split: bool,
    /// With `split`: under an attack order too the salvo opens on the mark and spreads the
    /// rest over the targets round it (the Strider's seekers). False (the default): an
    /// ordered salvo stays whole on its mark.
    #[serde(default)]
    pub split_ordered: bool,
    /// Fires a whole salvo the moment its unit lands from a dash (`crate::dash`), however
    /// far through its reload it was; the reload then starts again from the volley.
    #[serde(default)]
    pub on_dash: bool,
    /// Goes off only as its unit lands from a dash, at its feet (`crate::dash`): it never
    /// picks a target of its own. A cone (the Seraph's landing wave) rolls out all round.
    #[serde(default)]
    pub dash_only: bool,
    /// Picking a target of its own, it takes the enemy nearest where it already points
    /// rather than the one nearest the hull, so it swings as little as it can between
    /// marks (the Seraph's forearm guns).
    #[serde(default)]
    pub keep_aim: bool,
    #[serde(default)]
    pub proximity: f64,
    /// Seconds the ground burns where a round lands; zero: it does not.
    #[serde(default)]
    pub burn: f64,
    /// Damage a second that burning ground does to each enemy standing in it.
    #[serde(default = "default_burn_dps")]
    pub burn_dps: f64,
    #[serde(default)]
    pub rear: bool,
    /// Which way a gun house rests, degrees off the nose, positive to the left
    /// (90: port, -90: starboard, 180: aft). Its `arc` is centred there, and it only
    /// takes targets inside that arc. Unlike `rear`, its muzzle is authored as the
    /// house faces the nose, turned about `pivot`. Zero (the default): the nose, or
    /// aft for a `rear` weapon.
    #[serde(default)]
    pub facing: f64,
    /// Degrees the gun rests at, up from level, while it has nothing to shoot (a
    /// shoulder gun rests with its torso). Negative: lowered, as a gun carried at a
    /// walker's side. Zero (the default): level, or up at the sky for an AA gun.
    #[serde(default)]
    pub rest_pitch: f64,
    /// Multiplies the muzzle flash. 1 is the size the damage implies.
    #[serde(default = "one_f64")]
    pub flash: f64,
    /// Multiplies the impact flash. 0 (the default) uses `flash`.
    #[serde(default)]
    pub impact: f64,
    /// Multiplies the muzzle and impact shockwave. 0 (the default) is none; 1 is the size the damage implies.
    /// On a cone, how many times its usual height its wall stands (at least 1).
    #[serde(default)]
    pub shockwave: f64,
    /// Multiplies the projectile tracer. 1 is the size the damage implies.
    #[serde(default = "one_f64")]
    pub tracer: f64,
    /// A projectile wake: blue energy for Blue shots, white smoke for Orange shots.
    #[serde(default)]
    pub trail: bool,
    /// Seconds the wake hangs. Zero (the default) is the usual hang, when the shot has a trail.
    /// A Regency plasma shot's (`plasma_grade`): seconds its trail glows, up to 30, so a map
    /// gun's arc hangs across the map; zero, its grade's own.
    #[serde(default)]
    pub wake: f64,
    /// Multiplies a blue plasma sheath around the traveling slug. 0 (the default)
    /// is none; 1 is the size the damage implies. A sheathed energy slug is drawn as
    /// a plasma bolt: a round head and a short tail, not a tick-long streak.
    #[serde(default)]
    pub plasma: f64,
    /// Blue-white bolts at the muzzle and the impact. Zero (the default) is none.
    #[serde(default)]
    pub bolts: u8,
    /// Multiplies how long a plasma-sheathed slug's glowing tail is drawn behind it
    /// (`plasma`). Cosmetic. Zero (the default) is one: the usual short tail.
    #[serde(default)]
    pub streak: f64,
    /// An electrically charged shell: where it lands, the charge strikes back up the
    /// last this-many metres of its flight as lightning and forks into the ground round
    /// the hit (the electric bore's look, on a lobbed shell). Cosmetic. Zero: none.
    #[serde(default)]
    pub discharge: f64,
    /// Where the shell lands on open ground (not on water, a shield or high on a hull)
    /// it melts a pool of ground this many times its splash across, which glows white,
    /// then orange, then red as it crusts over and cools. Cosmetic. Zero (the default): none.
    #[serde(default)]
    pub melt: f64,
    /// An AEB warhead: where the shot lands (not on a shield), it goes off as the electric
    /// bore's blast, a ball of ionised air this many metres across that burns blue with
    /// lightning striking out of it, in place of a shell's burst; its motor burns blue.
    /// Cosmetic. Zero (the default): none.
    #[serde(default)]
    pub ion_blast: f64,
    /// A capital rail gun (the Resolute's spinal rail, the Zenith): the firing, the
    /// slug's path and the hit are drawn and heard far bigger than any other rail gun,
    /// scaled by this. Under 0.15 (the Resolute's turrets) only the charge and the rails'
    /// heat after the shot are drawn so; the shot is an ordinary rail's. Cosmetic. Zero
    /// (the default): an ordinary rail.
    #[serde(default)]
    pub heavy_rail: f64,
    /// A bolt rifle's firing sequence (the Paladin's): while the weapon charges
    /// (`sounds.charge_time`), small arcs crawl along its radiator blades and jump to the
    /// core, then knot round the muzzle collar; on the shot, forks of lightning snap out of
    /// the muzzle and the housing vents ionised haze. The number is the gun's length in
    /// metres, breech to `muzzle`; the housing, bladed core and collar are laid out in
    /// shares of it (models `bolt_rifle`). Cosmetic. Zero (the default): none.
    #[serde(default)]
    pub arc_charge: f64,
    /// An Arc Howitzer's firing sequence (the Trebuchet's gun, and the Leviathan's Arc
    /// Cannons, the same tube): while it charges, the plasma cells on the breech housing
    /// light and arcs crawl from them over the bladed core, jump between the tubes of a
    /// house and knot round the collar; on the shot, a plasma jet, a shock ring, forks of
    /// lightning and vented haze, and the arcs dying off along the tube. Its layout in
    /// shares of the length is the model's (models `siege_howitzer`). Cosmetic.
    #[serde(default)]
    pub howitzer: Option<RawHowitzer>,
    /// A ballistic gun laid flat, a warship's rifle: its shell still arcs, but it is a
    /// direct-fire gun, not artillery, and its range ring is drawn as direct fire's.
    #[serde(default)]
    pub flat_fire: bool,
    /// A great gun (the Culverin): its firing, its shell's climb through the clouds and
    /// its landing are drawn as the event they are, at this scale: a muzzle blast that
    /// flattens the ground round the gun, a trail seen from the strategic view, a hit of
    /// its own and a crater. Cosmetic. Zero (the default): an ordinary shell.
    #[serde(default)]
    pub great_gun: f64,
    /// While it has nothing to shoot, the gun stays laid where it last aimed instead of
    /// coming back to rest.
    #[serde(default)]
    pub keeps_aim: bool,
    /// Metres: the widest circle a `Bombard` order may spread this gun's shots over.
    /// Below `BOMBARD_RADIUS` (the default, zero) it is that: a map gun is given more.
    #[serde(default)]
    pub bombard: f64,
    /// A missile's body across, in metres, as it is drawn: the tube it leaves. Cosmetic.
    /// Zero (the default): the size the damage implies.
    #[serde(default)]
    pub caliber: f64,
    /// A Regency plasma weapon's grade: names its kind on the HUD. Cosmetic. None (the
    /// default): not a plasma weapon.
    #[serde(default)]
    pub plasma_grade: Option<PlasmaGrade>,
    /// A Gravitic Seeker whose charge is pinch fusion (the Strider's): it strikes as a
    /// Pinch-fusion shot does, a small supernova, not as the gravity cage letting go.
    /// Cosmetic. False (the default): a seeker's own strike.
    #[serde(default)]
    pub fusion_strike: bool,
    /// A held pinch-fusion beam (`beam` and the `PinchFusion` grade) drawn as the power
    /// generator's star, not as an excavation lance: a white-hot core in a broad prism
    /// sheath, and where it lands it melts the ground to glass and digs no crater.
    /// Cosmetic. False (the default): the excavation lance, which digs as it glasses.
    #[serde(default)]
    pub star_beam: bool,
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
    /// A rocket rack built on the torso at its `rake` (a walker's back pods): it turns
    /// with the torso but never pitches, so its rockets leave from `muzzles` as they
    /// stand, along the rake. Without it a rack pitches about `pivot` from its rest
    /// (`pivot` to `muzzle`) up to the rake.
    #[serde(default)]
    pub fixed_rack: bool,
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
    /// Barrel spacings the cluster turns per shot at full spin: a barrel-timed gun
    /// spins this many times faster than it fires, with a barrel at the top on every
    /// shot all the same. One (the default): a barrel spacing a shot.
    #[serde(default = "one")]
    pub laps: u8,
    /// Rounds each shot is seen as: a stream of tracers spread over the time to the
    /// next shot, all carried by the one simulated projectile, so a fast gun reads as
    /// a stream without the sim flying every bullet. Cosmetic. One (the default): the
    /// shot is drawn as it is. Direct-fire guns only.
    #[serde(default = "one")]
    pub rounds: u8,
    /// Seconds a shot's rounds are spread over, when that is not the time to the next
    /// shot: a gun that pinches one shot out as a jet. Cosmetic. Zero (the default):
    /// the time to the next shot.
    #[serde(default)]
    pub round_span: f64,
    /// An orb battery: the gun holds plasma orbs, one in each of its `muzzles`, made
    /// this many at a time (`orb_time` apart) while it stands idle, and each shot spends
    /// one. A salvo ends when the store is empty. Its shots fly straight and home on their
    /// mark, retarget when it dies and return to the gun when nothing is left. Direct guns
    /// only. Zero (the default): an ordinary gun.
    #[serde(default)]
    pub orbs: u8,
    /// Seconds between the batches of an orb battery.
    #[serde(default)]
    pub orb_time: f64,
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
    /// A torpedo's own look, over the one its faction's torpedoes have. Cosmetic.
    #[serde(default)]
    pub torpedo_look: Option<TorpedoLook>,
    /// A guided missile that flies this many metres over the ground or water until its
    /// terminal run (a sea skimmer). Zero: it flies straight at its mark.
    #[serde(default)]
    pub skim: f64,
    /// A guided missile that climbs to this height before it comes down on its mark (a
    /// high arc). Zero: no climb.
    #[serde(default)]
    pub apogee: f64,
    /// A thrown charge that curves onto its mark (the Regency's Gravitic Bomb): degrees the
    /// shots of a salvo leave off the line to the mark, fanned across it and over it, and
    /// each curves back onto the target through its own angle. It flies at `speed` under
    /// no gravity, so it is a `Direct` weapon, and it is no missile: nothing intercepts it.
    /// Zero (the default): it flies the usual way.
    #[serde(default)]
    pub curve: f64,
    /// Orbit radius for a curving charge: straight first, then spiralling onto a
    /// six-point area pattern inside its splash. Zero keeps the usual fan.
    #[serde(default)]
    pub corkscrew: f64,
    /// Fires the unit's drones (`drone`) instead of shots (the Regency's Wick): each launch
    /// lets one drone go from its socket at the mark, and it flies into it and bursts there
    /// with this weapon's charge (`damage`, `splash` and its look); the drone is the shell.
    /// A launch waits for a drone on its socket, so the sockets are the magazine, and the
    /// drones spent are built again on them. `speed` is the burst's, over its last metres.
    #[serde(default)]
    pub launches: bool,
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
    /// A cone weapon: every shot strikes all it may shoot in a fan ahead (`RawCone`).
    #[serde(default)]
    pub cone: Option<RawCone>,
    /// A gravitic lens: holds a gravity well on its target (`RawLens`).
    #[serde(default)]
    pub lens: Option<RawLens>,
    /// A surge shell: where it lands a wake rolls out all round (`crate::surge`).
    #[serde(default)]
    pub surge: Option<crate::surge::RawSurge>,
    /// A Pinch-fusion gun that gathers its charge as this many knots, one at each of its
    /// projectors round the bore, drawn into one where the shot leaves: two to six, zero
    /// for one ball. It is drawn as a knotted gun throughout: its charge, a solid shell, its
    /// trail and its strike (renderer `regency_guns_fx::knotted`, the Springald's).
    /// Cosmetic.
    #[serde(default)]
    pub knots: u8,
    /// A twin-rail Pinch-fusion gun: two or more bores side by side, plated rails either
    /// side of each and between them, each bore's charge held between its rails and pinched
    /// flat by the lens heads at their tips. The gap between the rails is the gap between
    /// the bores (`muzzles`). It is drawn as a railed gun throughout: lightning walking up
    /// the rails as it charges, a discharge down them as it fires, a long white-hot lance
    /// in flight, and its own strike (renderer `regency_guns_fx::railed`, the Flamberge's
    /// main battery). Cosmetic.
    #[serde(default)]
    pub rails: bool,
    /// A strafing beam (`RawStrafe`).
    #[serde(default)]
    pub strafe: Option<RawStrafe>,
    /// A giant gun's spent sabot, thrown clear with every shot (`RawSabot`).
    #[serde(default)]
    pub sabot: Option<RawSabot>,
    /// A shot that breaks into sub-shots on its way down (`RawCluster`).
    #[serde(default)]
    pub cluster: Option<RawCluster>,
    /// A storm seed: the shot is a cloud that lets fall lightning (`RawCloud`).
    #[serde(default)]
    pub cloud: Option<RawCloud>,
    #[serde(default)]
    pub sounds: WeaponSounds,
}

/// A cluster shot (the Regency's Heavy Gravitic Seeker): an unguided missile on a high arc
/// that breaks into `count` sub-shots once it is coming down within `height` metres of its
/// mark (at the top of its arc, when that is lower). The sub-shots fall on their own
/// points, spread over a disc `radius` metres across the parent's landing point, each with
/// an even share of the weapon's `damage` and a blast `splash` metres round. Each sub-shot
/// can be shot down on its own, with its share of the casing (`intercept`).
#[derive(Deserialize, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub struct RawCluster {
    pub count: u8,
    pub radius: f64,
    pub height: f64,
    pub splash: f64,
}

/// A storm seed (the Nimbus's Storm Seeder): the shot is a cloud let go behind the
/// aircraft, aimed at nothing, that hangs about where it was let go at the height it was
/// seeded, swelling for `grow` seconds. Grown, it lets fall `bolts` lightning strikes over
/// `strike` seconds, each in its own share of that time at a moment of its own, each onto
/// one of the foes under it within `radius` metres of its middle (onto the ground there
/// when there is none), each with an even share of the weapon's `damage` and a blast
/// `splash` metres round. Then it is spent. A direct, non-missile
/// weapon on an aircraft; the seed itself hits nothing on its way.
#[derive(Deserialize, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub struct RawCloud {
    pub grow: f64,
    pub strike: f64,
    pub bolts: u8,
    pub radius: f64,
    pub splash: f64,
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
    /// A rotary gun's barrels turning: a loop that rises with their speed.
    pub whir: Option<String>,
    /// A rotary gun's barrels coasting down with nothing left to shoot.
    pub spindown: Option<String>,
    /// A held beam's loop (`beam`), heard while it fires in place of `fire`; a gravitic
    /// lens's (`lens`) well, heard on it while it holds.
    pub hold: Option<String>,
    /// The hit as heard from far across the map, late by the distance: only for the
    /// biggest guns. Near the camera `impact`/`ground` plays instead.
    pub far: Option<String>,
    /// The shot striking a unit, and striking the ground (`impact` if left out); a
    /// gravitic lens's front striking its target.
    pub impact: Option<String>,
    pub ground: Option<String>,
    /// A spent casing (`sabot`) landing. None: the `ground` sound.
    pub casing: Option<String>,
    /// A loop heard from each shot while it flies: a cruise missile's motor, a gravitic
    /// lens's front.
    pub flight: Option<String>,
    /// A gravitic lens's well let go (`LensReleased`), heard where it collapses.
    pub release: Option<String>,
    /// A cluster shot (`cluster`) breaking into its sub-shots, heard where it splits.
    pub split: Option<String>,
    /// An orb battery (`orbs`) making a batch of orbs, heard at the gun.
    pub make: Option<String>,
    /// `fire` is one round, heard for each round a shot is drawn as (`rounds`), as that
    /// round leaves the muzzle. False (the default): heard once a shot, so a stream gun's
    /// `fire` is its whole burst.
    pub each_round: bool,
    /// Multiplies how loud the shot is heard. Zero (the default) is as loud as its damage implies.
    pub volume: f64,
    /// Multiplies how loud the charge is heard, at any zoom: a wind-up meant to carry over
    /// the battle. Zero (the default) is a wind-up heard only beside the gun.
    pub charge_volume: f64,
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
    /// A ground stake striking home as the unit plants: once a stake, as each fires into
    /// the ground (the model's stakes, `mc_models::stakes`).
    pub stake: Option<String>,
    /// Heard by its owner when the unit is selected. Left out, the library's
    /// `select` default for the unit's `icon` kind plays.
    pub select: Option<String>,
    /// A dash (`crate::dash`): its windup, the unit leaving where it stood, a loop heard
    /// along its path while it is gone (`dash_gone`), the air charging where it is about
    /// to come down (`Dash::arrive_ticks` before), and its landing.
    pub dash_charge: Option<String>,
    pub dash_leave: Option<String>,
    pub dash_gone: Option<String>,
    pub dash_arrive: Option<String>,
    pub dash_land: Option<String>,
    /// Heard where the unit is as it is finished: an aircraft raised on a lot lifting off
    /// it (the Nimbus).
    pub takeoff: Option<String>,
}

fn one() -> u8 {
    1
}

fn default_burn_dps() -> f64 {
    30.0
}

fn one_f64() -> f64 {
    1.0
}

fn full_arc() -> f64 {
    360.0
}

pub(crate) fn fx(v: f64) -> Fx {
    Fx((v * Fx::ONE.0 as f64).round() as i64)
}

/// Degrees to angle steps.
pub(crate) fn steps(deg: f64) -> f64 {
    deg * 65536.0 / 360.0
}

fn ticks(seconds: f64) -> u32 {
    (seconds * TICKS_PER_SECOND as f64).round().max(0.0) as u32
}

/// A reclaimer and its heads, checked: 1 to `MAX_RECLAIM_HEADS` heads, and no weapons
/// beside them, since head `i` takes weapon slot `i` (`mc_data::Reclaimer`).
/// A provider's full-ring savings, from what it makes a second ([`crate::Adjacency::ring`]).
fn compile_adjacency(
    key: &str,
    kind: RawAdjacency,
    e: &RawEconomy,
    fabricator: Option<&RawFabricator>,
    mine: Option<&RawMine>,
) -> Result<crate::Adjacency, DataError> {
    let invalid = |why: &str| DataError::Invalid(format!("{key}: adjacency {why}"));
    let a = match kind {
        RawAdjacency::Energy if e.energy_income > 0.0 => crate::Adjacency {
            energy: crate::Adjacency::ring(crate::Adjacency::ENERGY_RING, fx(e.energy_income)),
            materials: Fx::ZERO,
        },
        RawAdjacency::Materials
            if fabricator.is_some_and(|f| f.materials > 0.0)
                || mine.is_some_and(|m| m.base > 0.0)
                || e.materials_income > 0.0 =>
        {
            // A mine's rated output is its shaft (`base`): what it pays from the moment it
            // is finished, whatever ore lies in its reach.
            let made = e.materials_income
                + fabricator.map_or(0.0, |f| f.materials)
                + mine.map_or(0.0, |m| m.base);
            crate::Adjacency {
                energy: Fx::ZERO,
                materials: crate::Adjacency::ring(crate::Adjacency::MATERIALS_RING, fx(made)),
            }
        }
        _ => return Err(invalid("saves what it makes, and it makes none of it")),
    };
    if a.energy >= Fx::ONE || a.materials >= Fx::ONE {
        return Err(invalid(
            "of a full ring comes to all of a neighbour's use: it makes too much",
        ));
    }
    Ok(a)
}

fn compile_reclaimer(key: &str, r: &RawReclaimer, weapons: usize) -> Result<Reclaimer, DataError> {
    let heads = r.heads.len();
    if !(1..=crate::MAX_RECLAIM_HEADS
        .min(MAX_WEAPONS)
        .min(crate::MAX_HOUSES))
        .contains(&heads)
        || weapons > 0
    {
        return Err(DataError::Invalid(format!(
            "{key}: a reclaimer has 1 to {} heads and no weapons (the heads take the weapon slots)",
            crate::MAX_RECLAIM_HEADS
        )));
    }
    if r.power <= 0.0 || r.range <= 0.0 || r.charge < 0.0 {
        return Err(DataError::Invalid(format!(
            "{key}: a reclaimer needs power and range"
        )));
    }
    let v = |p: (f64, f64, f64)| FxVec3::new(fx(p.0), fx(p.1), fx(p.2));
    let mut out = Vec::with_capacity(heads);
    for h in &r.heads {
        let (down, up) = h.pitch;
        if !(-90.0..=0.0).contains(&down) || !(0.0..=90.0).contains(&up) {
            return Err(DataError::Invalid(format!(
                "{key}: a reclaim head pitches (down, up) within (-90, 90)"
            )));
        }
        out.push(crate::ReclaimHead {
            emitter: v(h.emitter),
            pivot: h.pivot.map(v),
            turn: (steps(h.turn) / TICKS_PER_SECOND as f64)
                .round()
                .clamp(0.0, 32767.0) as u16,
            pitch_min: Angle(steps(down).round() as i32 as u16),
            pitch_max: Angle(steps(up).round().min(16383.0) as u16),
        });
    }
    Ok(Reclaimer::new(
        fx(r.power),
        fx(r.range),
        ticks(r.charge).clamp(0, 600) as u16,
        r.mobile,
        &out,
    ))
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
        let guns: usize = self.weapons.iter().map(|w| w.mounts.len().max(1)).sum();
        if guns > crate::MAX_GUNS {
            return Err(DataError::Invalid(format!(
                "{key}: {guns} guns, but a unit carries at most {}",
                crate::MAX_GUNS
            )));
        }
        if self.weapons.len() > MAX_WEAPONS {
            return Err(DataError::Invalid(format!(
                "{key}: more than {MAX_WEAPONS} weapons"
            )));
        }
        if self.drone.is_some() != !self.drone_sockets.is_empty()
            || self.drone_sockets.len() > MAX_DRONES
        {
            return Err(DataError::Invalid(format!(
                "{key}: a unit with a drone has 1 to {MAX_DRONES} drone_sockets, and only it has any"
            )));
        }
        if self.weapons.iter().filter(|w| w.launches).count() > 1 {
            return Err(DataError::Invalid(format!(
                "{key}: one weapon at most launches the unit's drones"
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
                } else if m.altitude != 0.0 || self.orbit.is_some() {
                    return Err(DataError::Invalid(format!(
                        "{key}: altitude and orbit are only for air units"
                    )));
                }
                if m.above_weather && m.layer != MoveLayer::Air {
                    return Err(DataError::Invalid(format!(
                        "{key}: only an aircraft flies above the weather"
                    )));
                }
                if m.stride && m.layer != MoveLayer::Land {
                    return Err(DataError::Invalid(format!(
                        "{key}: only a land walker strides"
                    )));
                }
                if m.floats && m.layer != MoveLayer::Hover {
                    return Err(DataError::Invalid(format!(
                        "{key}: only a hover unit floats over what it meets"
                    )));
                }
                if m.hangs && !(m.layer == MoveLayer::Air && m.hover) {
                    return Err(DataError::Invalid(format!(
                        "{key}: only a hover aircraft hangs over its mark"
                    )));
                }
                if m.stand_off && (m.layer != MoveLayer::Air || m.hover) {
                    return Err(DataError::Invalid(format!(
                        "{key}: only a jet (air, not hover) stands off"
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
                    deploy_idle: m.deploy_idle,
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
                    floats: m.floats,
                    hangs: m.hangs,
                    above_weather: m.above_weather,
                    stand_off: m.stand_off,
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
                    aloft: b.aloft,
                })
            }
            None => None,
        };

        let mut weapons: Vec<Weapon> = Vec::with_capacity(self.weapons.len());
        for w in &self.weapons {
            let ctx = format!("{key}/{}", w.name);
            let rotary = weapons.iter().filter(|w| w.spin_ticks > 0).count();
            if w.spin_up > 0.0 && rotary >= crate::ROTARY_SLOTS {
                return Err(DataError::Invalid(format!(
                    "{ctx}: a unit spins up at most {} rotary guns",
                    crate::ROTARY_SLOTS
                )));
            }
            if w.loft > 0.0 && w.trajectory != Trajectory::Ballistic {
                return Err(DataError::Invalid(format!(
                    "{ctx}: loft is only for ballistic weapons"
                )));
            }
            if w.fixed_rack && !(w.missile && w.trajectory == Trajectory::Ballistic && w.rake > 0.0)
            {
                return Err(DataError::Invalid(format!(
                    "{ctx}: fixed_rack is only for a ballistic rocket rack with a rake"
                )));
            }
            if w.rounds > 1 && (w.trajectory != Trajectory::Direct || w.missile) {
                return Err(DataError::Invalid(format!(
                    "{ctx}: rounds is only for direct-fire guns"
                )));
            }
            if w.sounds.each_round && w.rounds < 2 {
                return Err(DataError::Invalid(format!(
                    "{ctx}: each_round needs rounds above one"
                )));
            }
            if w.round_span > 0.0 && w.rounds < 2 {
                return Err(DataError::Invalid(format!(
                    "{ctx}: round_span needs rounds above one"
                )));
            }
            if w.split_ordered && !w.split {
                return Err(DataError::Invalid(format!(
                    "{ctx}: split_ordered needs split"
                )));
            }
            if w.fusion_strike && !(w.missile && w.plasma_grade == Some(PlasmaGrade::Gravitic)) {
                return Err(DataError::Invalid(format!(
                    "{ctx}: fusion_strike is for a Gravitic Seeker"
                )));
            }
            if w.orbs > 0
                && (w.trajectory != Trajectory::Direct
                    || w.missile
                    || w.orb_time <= 0.0
                    || w.muzzles.is_empty()
                    || w.muzzles.len() > crate::MAX_ORBS
                    || w.muzzles.len() % w.orbs as usize != 0)
            {
                return Err(DataError::Invalid(format!(
                    "{ctx}: an orb battery is a direct gun with an orb_time and a muzzle \
                     for each orb it holds (at most {}, whole batches)",
                    crate::MAX_ORBS
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
            if w.beam && !w.hitscan {
                return Err(DataError::Invalid(format!(
                    "{ctx}: a beam is a hitscan gun"
                )));
            }
            if w.rail && (w.trajectory != Trajectory::Direct || w.missile || w.hitscan || w.torpedo)
            {
                return Err(DataError::Invalid(format!(
                    "{ctx}: a rail gun fires a direct slug, not a beam, missile or torpedo"
                )));
            }
            if w.cluster.is_some()
                && (w.trajectory != Trajectory::Ballistic || !w.missile || w.guided || w.hitscan)
            {
                return Err(DataError::Invalid(format!(
                    "{ctx}: a cluster shot is an unguided ballistic missile"
                )));
            }
            if let Some(c) = w.cluster {
                if !(2..=crate::weapon::MAX_CLUSTER).contains(&c.count) {
                    return Err(DataError::Invalid(format!(
                        "{ctx}: a cluster breaks into 2 to {} sub-shots",
                        crate::weapon::MAX_CLUSTER
                    )));
                }
            }
            if w.cloud.is_some()
                && (self
                    .motion
                    .as_ref()
                    .is_none_or(|m| m.layer != MoveLayer::Air)
                    || w.trajectory != Trajectory::Direct
                    || w.missile
                    || w.hitscan
                    || w.torpedo
                    || w.curve > 0.0
                    || w.cluster.is_some())
            {
                return Err(DataError::Invalid(format!(
                    "{ctx}: a storm seed is a direct, non-missile shot from an aircraft"
                )));
            }
            if let Some(c) = w.cloud {
                if !(1..=crate::weapon::MAX_BOLTS).contains(&c.bolts) || c.grow <= 0.0 {
                    return Err(DataError::Invalid(format!(
                        "{ctx}: a storm seed grows for a while and lets fall 1 to {} bolts",
                        crate::weapon::MAX_BOLTS
                    )));
                }
            }
            if w.curve > 0.0
                && (w.trajectory != Trajectory::Direct || w.missile || w.hitscan || w.torpedo)
            {
                return Err(DataError::Invalid(format!(
                    "{ctx}: a curving charge is a direct, non-missile, non-hitscan weapon"
                )));
            }
            if w.launches
                && (self.drone.is_none()
                    || w.trajectory != Trajectory::Direct
                    || w.missile
                    || w.hitscan
                    || w.torpedo
                    || w.curve > 0.0)
            {
                return Err(DataError::Invalid(format!(
                    "{ctx}: a drone launcher is a direct weapon on a unit with a `drone`"
                )));
            }
            if w.corkscrew > 0.0 && (w.curve <= 0.0 || w.cluster.is_some()) {
                return Err(DataError::Invalid(format!(
                    "{ctx}: a corkscrew needs a curving charge without cluster sub-shots"
                )));
            }
            if !w.hitscan && w.speed <= 0.0 {
                return Err(DataError::Invalid(format!(
                    "{ctx}: needs a speed, or hitscan"
                )));
            }
            if w.cone.is_some()
                && (!w.hitscan
                    || w.trajectory != Trajectory::Direct
                    || w.missile
                    || w.beam
                    || w.torpedo
                    || w.bore.is_some()
                    || w.splash > 0.0)
            {
                return Err(DataError::Invalid(format!(
                    "{ctx}: a cone is a hitscan direct shot with no splash, beam or bore"
                )));
            }
            if w.lens
                .is_some_and(|l| l.radius <= 0.0 || !(0.0..=1.0).contains(&l.slow))
                || w.lens.is_some()
                    && (w.hitscan
                        || w.trajectory != Trajectory::Direct
                        || w.missile
                        || w.beam
                        || w.torpedo
                        || w.bore.is_some()
                        || w.cone.is_some()
                        || w.splash > 0.0)
            {
                return Err(DataError::Invalid(format!(
                    "{ctx}: a lens is a direct shot with a speed and no splash, beam, bore or \
                     cone, a radius over zero and 0 <= slow <= 1"
                )));
            }
            if w.surge.is_some()
                && (w.hitscan
                    || w.beam
                    || w.bore.is_some()
                    || w.cone.is_some()
                    || w.lens.is_some()
                    || w.cloud.is_some()
                    || w.cluster.is_some())
            {
                return Err(DataError::Invalid(format!(
                    "{ctx}: a surge rides a shell that lands: no hitscan, beam, bore, cone, \
                     lens, cloud or cluster"
                )));
            }
            if w.knots > 0
                && (!(2..=6).contains(&w.knots)
                    || w.missile
                    || w.surge.is_some()
                    || w.plasma_grade != Some(PlasmaGrade::PinchFusion))
            {
                return Err(DataError::Invalid(format!(
                    "{ctx}: knots are two to six, on a Pinch-fusion gun firing a shell"
                )));
            }
            if w.rails
                && (w.muzzles.len() < 2
                    || w.missile
                    || w.knots > 0
                    || w.surge.is_some()
                    || w.airburst
                    || w.beam
                    || w.plasma_grade != Some(PlasmaGrade::PinchFusion))
            {
                return Err(DataError::Invalid(format!(
                    "{ctx}: rails are on a Pinch-fusion gun of two or more bores firing a \
                     shell: no missile, beam, knots, surge or airburst"
                )));
            }
            if w.dash_only && w.cone.is_none() {
                return Err(DataError::Invalid(format!(
                    "{ctx}: a dash_only weapon is a cone, let go at the unit's feet as it lands"
                )));
            }
            if w.strafe.is_some_and(|s| {
                s.ahead <= 0.0 || s.length <= 0.0 || s.gap < 0.0 || s.ahead >= w.range
            }) || (w.strafe.is_some()
                && (!w.beam
                    || !w.hitscan
                    || w.turret_turn != 0.0
                    || w.muzzles.is_empty()
                    || self
                        .motion
                        .as_ref()
                        .is_none_or(|m| m.layer != MoveLayer::Air || m.hover)))
            {
                return Err(DataError::Invalid(format!(
                    "{ctx}: a strafe is a held hitscan beam fixed in an aircraft that makes \
                     runs, from its muzzles, landing inside its range"
                )));
            }
            if w.bore.is_some()
                && (w.trajectory != Trajectory::Direct || w.missile || w.beam || w.torpedo)
            {
                return Err(DataError::Invalid(format!(
                    "{ctx}: an electric bore fires a direct shot"
                )));
            }
            crate::mounts::check(&ctx, w)?;
            weapons.push(Weapon {
                name: w.name.clone(),
                damage: fx(w.damage),
                splash: fx(w.splash),
                range_min: fx(w.range_min),
                range_max: fx(w.range),
                reload_ticks: ticks(w.reload).clamp(1, u16::MAX as u32) as u16,
                reload_energy: fx(w.reload_energy.clamp(0.0, 1.0e6)),
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
                flak: w.flak,
                airburst: w.airburst,
                beam: w.beam,
                loft_ticks: ticks(w.loft).clamp(0, 200) as u16,
                turret_turn: (steps(w.turret_turn) / TICKS_PER_SECOND as f64)
                    .round()
                    .clamp(0.0, 32767.0) as u16,
                half_arc: crate::mounts::half_arc(w.arc),
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
                boost_ticks: ticks(w.boost.min(w.cold_launch)).min(600) as u16,
                hatch_ticks: ticks(w.hatch).min(600) as u16,
                split: w.split,
                split_ordered: w.split_ordered,
                on_dash: w.on_dash,
                dash_only: w.dash_only,
                keep_aim: w.keep_aim,
                proximity: fx(w.proximity),
                burn_ticks: ticks(w.burn).min(600) as u16,
                burn_dps: fx(w.burn_dps.max(0.0)),
                rear: w.rear,
                rest_pitch: Angle(steps(w.rest_pitch.clamp(-90.0, 90.0)).round() as i64 as u16),
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
                wake: w
                    .wake
                    .clamp(0.0, if w.plasma_grade.is_some() { 30.0 } else { 4.0 })
                    as f32,
                plasma: w.plasma.clamp(0.0, 4.0) as f32,
                bolts: w.bolts.min(32),
                discharge: w.discharge.clamp(0.0, 400.0) as f32,
                melt: w.melt.clamp(0.0, 2.0) as f32,
                ion_blast: w.ion_blast.clamp(0.0, 200.0) as f32,
                streak: w.streak.clamp(0.0, 8.0) as f32,
                heavy_rail: w.heavy_rail.clamp(0.0, 4.0) as f32,
                arc_charge: w.arc_charge.clamp(0.0, 40.0) as f32,
                howitzer: w.howitzer.map(|h| crate::weapon::HowitzerLook {
                    length: h.length.clamp(1.0, 80.0) as f32,
                    radius: h.radius.clamp(0.1, 8.0) as f32,
                }),
                flat_fire: w.flat_fire,
                great_gun: w.great_gun.clamp(0.0, 4.0) as f32,
                keeps_aim: w.keeps_aim,
                bombard_radius: fx(w.bombard.clamp(crate::weapon::BOMBARD_RADIUS, 6000.0)),
                caliber: w.caliber.clamp(0.0, 20.0) as f32,
                plasma_grade: w.plasma_grade,
                fusion_strike: w.fusion_strike,
                star_beam: w.star_beam,
                depression: Angle(steps(w.depression.clamp(0.0, 89.0)).round() as i64 as u16),
                sway: Angle(steps(w.sway.clamp(0.0, 60.0)).round() as i64 as u16),
                rake: Angle(steps(w.rake.clamp(0.0, 80.0)).round() as i64 as u16),
                fixed_rack: w.fixed_rack,
                mount: w.mount,
                slant: w.slant,
                spin_ticks: ticks(w.spin_up).clamp(0, 600) as u16,
                rotary: if w.spin_up > 0.0 { rotary as u8 } else { 0 },
                spin_ramp: (w.spin_ramp.clamp(0.0, 20.0) * 100.0).round() as u16,
                barrels: if w.spin_up > 0.0 {
                    w.barrels.min(12)
                } else {
                    0
                },
                laps: w.laps.clamp(1, 8),
                rounds: w.rounds.clamp(1, 32),
                round_span: (w.round_span.clamp(0.0, 10.0) * TICKS_PER_SECOND as f64) as f32,
                orbs: w.orbs,
                orb_ticks: ticks(w.orb_time).clamp(1, u16::MAX as u32) as u16,
                casings: w.casings.clamp(0.0, 40.0) as f32,
                red: w.red.clamp(0.0, 1.0) as f32,
                torpedo: w.torpedo,
                torpedo_look: w.torpedo_look,
                skim: fx(w.skim),
                apogee: fx(w.apogee),
                curve: Angle(steps(w.curve.clamp(0.0, 80.0)).round() as i64 as u16),
                corkscrew: fx(w.corkscrew.clamp(0.0, 80.0)),
                launches: w.launches,
                surfaced: w.surfaced,
                intercepts: w.intercepts,
                bore: w.bore.map(|b| crate::Bore {
                    width: fx(b.width.clamp(0.0, 80.0)),
                    damage: fx(b.damage.max(0.0)),
                    cool: b.cool.clamp(0.0, 600.0) as f32,
                    blast: b.blast.clamp(0.0, 400.0) as f32,
                    blast_time: b.blast_time.clamp(0.0, 30.0) as f32,
                    storm: b.storm.map(|s| crate::Storm {
                        radius: fx(s.radius.clamp(1.0, 2000.0)),
                        ticks: ticks(s.seconds).clamp(1, 1200) as u16,
                        damage: fx(s.damage.max(0.0)),
                    }),
                }),
                strafe: w.strafe.map(|s| crate::Strafe {
                    ahead: fx(s.ahead),
                    half: fx(s.length / 2.0),
                    gap: fx(s.gap),
                }),
                cone: w.cone.map(|c| crate::Cone {
                    half: Angle(steps(c.angle.clamp(1.0, 180.0)).round() as i64 as u16),
                    edge: fx(c.edge.clamp(0.0, 1.0)),
                    speed: fx(c.speed.clamp(10.0, 2000.0)),
                    stun_ticks: ticks(c.stun.clamp(0.0, 60.0)) as u16,
                }),
                lens: w.lens.map(|l| crate::Lens {
                    radius: fx(l.radius),
                    slow: fx(l.slow),
                }),
                knots: w.knots,
                rails: w.rails,
                surge: match &w.surge {
                    Some(s) => Some(s.compile(key, fx, TICKS_PER_SECOND)?),
                    None => None,
                },
                sabot: match &w.sabot {
                    Some(s) => {
                        let (x, y, z) = s.throw;
                        let length = (x * x + y * y + z * z).sqrt();
                        if !length.is_finite() || length <= 1e-6 {
                            return Err(DataError::Invalid(format!(
                                "{key}: a casing's `throw` must point somewhere"
                            )));
                        }
                        Some(crate::Sabot {
                            port: FxVec3::new(fx(s.port.0), fx(s.port.1), fx(s.port.2)),
                            throw: FxVec3::new(fx(x / length), fx(y / length), fx(z / length)),
                            kick: fx(s.kick.clamp(0.0, 200.0)),
                            damage: fx(s.damage.max(0.0)),
                            splash: fx(s.splash.clamp(0.0, 500.0)),
                            casing: lookup(&s.casing, key)?,
                        })
                    }
                    None => None,
                },
                cluster: w.cluster.map(|c| crate::Cluster {
                    count: c.count,
                    radius: fx(c.radius.clamp(0.0, 400.0)),
                    height: fx(c.height.clamp(0.0, 4000.0)),
                    splash: fx(c.splash.clamp(0.0, 500.0)),
                }),
                cloud: w.cloud.map(|c| crate::CloudSeed {
                    grow_ticks: ticks(c.grow).clamp(1, 600) as u16,
                    strike_ticks: ticks(c.strike).clamp(0, 600) as u16,
                    bolts: c.bolts,
                    radius: fx(c.radius.clamp(0.0, 800.0)),
                    splash: fx(c.splash.clamp(0.0, 500.0)),
                }),
                sounds: w.sounds.clone(),

                charge_ticks: if w.sounds.charge.is_some() {
                    ticks(w.sounds.charge_time).clamp(1, 600) as u16
                } else {
                    0
                },
                lore: String::new(),
                mounts: Vec::new(),
                mount_weapons: Vec::new(),
            });
            if let Some(group) = weapons.last_mut() {
                crate::mounts::assemble(group, crate::mounts::compile(w));
            }
        }

        let e = &self.economy;
        // Every shield runs on the grid: without upkeep it could not drop in a stall.
        if self.shield.is_some() && e.energy_upkeep <= 0.0 {
            return Err(DataError::Invalid(format!(
                "{key}: a shield needs energy_upkeep"
            )));
        }
        if let Some(p) = self.upgrade_power {
            if self.upgrades_to.is_none() || self.builder.is_some() || p <= 0.0 {
                return Err(DataError::Invalid(format!(
                    "{key}: upgrade_power is for a unit that upgrades and builds nothing, above 0"
                )));
            }
        }
        Ok(UnitBlueprint {
            id,
            key: key.clone(),
            name: self.name.clone(),
            role: self.role.clone(),
            title: self.title.clone(),
            faction,
            tech: self.tech,
            categories,
            health: fx(self.health),
            regen: fx(self.regen),
            nanite_regen: self.nanite_regen,
            cost_materials: fx(self.cost.materials),
            cost_energy: fx(self.cost.energy),
            build_time: fx(self.cost.time),
            radius: fx(self.radius),
            height: fx(self.height),
            footprint: self.footprint,
            hull: match self.hull {
                Some((x, y)) => (fx(x), fx(y)),
                None => (fx(self.radius), fx(self.radius)),
            },
            body: match &self.body {
                Some(b) => {
                    if b.beam <= 0.0 || b.fore - b.aft < b.beam * 2.0 {
                        return Err(DataError::Invalid(format!(
                            "{key}: body needs a beam, and fore - aft at least twice it"
                        )));
                    }
                    // Shots look for hulls within their radius.
                    if b.fore > self.radius || -b.aft > self.radius || b.beam > self.radius {
                        return Err(DataError::Invalid(format!(
                            "{key}: body must lie within its radius"
                        )));
                    }
                    Some(Body {
                        fore: fx(b.fore),
                        aft: fx(b.aft),
                        beam: fx(b.beam),
                    })
                }
                None => None,
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
            water_build: self.water_build,
            seabed: self.seabed,
            drone: self.drone.as_ref().map(|k| lookup(k, key)).transpose()?,
            air_wings: match &self.air_wings {
                Some(a)
                    if a.power <= 0.0
                        || a.guard <= 0.0
                        || a.wings.is_empty()
                        || a.wings.len() > crate::MAX_WINGS
                        || self.staging.is_none()
                        || a.wings.iter().any(|w| !crate::WING_SIZE.contains(&w.count)) =>
                {
                    return Err(DataError::Invalid(format!(
                        "{key}: air wings need build power, staging to launch them from, and 1 to {} wings of {} to {} aircraft",
                        crate::MAX_WINGS,
                        crate::WING_SIZE.start(),
                        crate::WING_SIZE.end(),
                    )));
                }
                Some(a) => {
                    let mut wings = Vec::with_capacity(a.wings.len());
                    for w in &a.wings {
                        wings.push(crate::Wing {
                            name: w.name.clone(),
                            aircraft: lookup(&w.aircraft, key)?,
                            count: w.count,
                        });
                    }
                    Some(crate::AirWings {
                        power: fx(a.power),
                        guard: fx(a.guard),
                        wings,
                    })
                }
                None => None,
            },
            drone_radius: fx(self.drone_radius),
            drone_sockets: self
                .drone_sockets
                .iter()
                .map(|m| FxVec3::new(fx(m.0), fx(m.1), fx(m.2)))
                .collect(),
            drone_approach: fx(self.drone_approach),
            orbit: self.orbit.map(fx),
            anti_missile: fx(self.anti_missile),
            anti_missile_mounts: self
                .anti_missile_mounts
                .iter()
                .map(|m| FxVec3::new(fx(m.0), fx(m.1), fx(m.2)))
                .collect(),
            anti_missile_lasers: self.anti_missile_lasers.max(1),
            turret_at: self.turret_at.map(|(x, y)| FxVec2::new(fx(x), fx(y))),
            hull_mounts: self.hull_mounts,
            stomp: self.stomp.map(|s| crate::Stomp {
                pace: fx(s.pace),
                reach: fx(s.reach),
                gauge: fx(s.gauge),
                radius: fx(s.radius),
                damage: fx(s.damage),
            }),
            dash: match &self.dash {
                Some(_)
                    if self
                        .motion
                        .as_ref()
                        .is_none_or(|m| m.layer == MoveLayer::Air) =>
                {
                    return Err(DataError::Invalid(format!(
                        "{key}: a dash is for a unit that moves on the ground or the water"
                    )));
                }
                Some(d) => Some(d.compile(key, fx, TICKS_PER_SECOND)?),
                None if weapons.iter().any(|w: &Weapon| w.on_dash || w.dash_only) => {
                    return Err(DataError::Invalid(format!(
                        "{key}: an on_dash or dash_only weapon needs a dash"
                    )));
                }
                None => None,
            },
            motion,
            economy: Economy {
                materials_income: fx(e.materials_income),
                energy_income: fx(e.energy_income),
                energy_upkeep: fx(e.energy_upkeep),
                materials_storage: fx(e.materials_storage),
                energy_storage: fx(e.energy_storage),
            },
            mine: self.mine.as_ref().map(|m| Mine {
                reach: fx(m.reach),
                ground: fx(m.ground),
                per_hectare: fx(m.per_hectare),
                base: fx(m.base),
                grow_upkeep: fx(m.grow_upkeep),
                hammer: m.hammer,
            }),
            fabricator: self.fabricator.as_ref().map(|f| crate::Fabricator {
                materials: fx(f.materials),
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
            adjacency: match self.adjacency {
                Some(kind) => Some(compile_adjacency(
                    key,
                    kind,
                    e,
                    self.fabricator.as_ref(),
                    self.mine.as_ref(),
                )?),
                None => None,
            },
            strategic: match &self.strategic {
                Some(s) => Some(s.compile(key, fx, TICKS_PER_SECOND)?),
                None => None,
            },
            gravitic: match &self.gravitic {
                Some(_) if self.motion.is_some() => {
                    return Err(DataError::Invalid(format!(
                        "{key}: a gravitic engine is a structure"
                    )));
                }
                Some(g) => Some(g.compile(key, fx, TICKS_PER_SECOND)?),
                None => None,
            },
            edict: match &self.edict {
                Some(_) if self.motion.is_none() => {
                    return Err(DataError::Invalid(format!(
                        "{key}: the Edict is fired from a ship's hull"
                    )));
                }
                Some(e) => Some(e.compile(key, fx, TICKS_PER_SECOND)?),
                None => None,
            },
            elevator: match &self.elevator {
                Some(e) => Some(e.compile(key, fx, TICKS_PER_SECOND)?),
                None => None,
            },
            lance: match &self.lance {
                Some(l) => Some(l.compile(
                    key,
                    mask(&l.targets, &format!("{key} lance"))?,
                    fx,
                    TICKS_PER_SECOND,
                )?),
                None => None,
            },
            growth: match &self.growth {
                Some(g) => {
                    Some(g.compile(
                        key,
                        crate::growth::Has {
                            shield: self.shield.is_some(),
                            lance: self.lance.is_some(),
                            interceptor: self.strategic.as_ref().is_some_and(|s| {
                                s.kind == crate::strategic::StrategicKind::Interceptor
                            }),
                            warhead:
                                self.strategic.as_ref().is_some_and(|s| {
                                    s.kind == crate::strategic::StrategicKind::Nuke
                                }),
                        },
                        TICKS_PER_SECOND,
                    )?)
                }
                None => None,
            },
            builder,
            built_aloft: false,
            reclaimer: match &self.reclaimer {
                Some(r) => Some(compile_reclaimer(key, r, self.weapons.len())?),
                None => None,
            },
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
            warp: match &self.warp {
                Some(d)
                    if d.speed <= 0.0
                        || d.spool <= 0.0
                        || d.per_km <= 0.0
                        || !self
                            .motion
                            .as_ref()
                            .is_some_and(|m| m.layer == MoveLayer::Air) =>
                {
                    return Err(DataError::Invalid(format!(
                        "{key}: a warp drive is on an aircraft, with a speed, a spool and an energy cost per km"
                    )));
                }
                Some(d) => Some(crate::Warp {
                    per_km: fx(d.per_km),
                    spool_ticks: ticks(d.spool).clamp(1, 600) as u16,
                    cooldown_ticks: ticks(d.cooldown).min(6000) as u16,
                    speed: fx(d.speed / TICKS_PER_SECOND as f64),
                    style: d.style,
                }),
                None => None,
            },
            warp_damper: match &self.warp_damper {
                Some(d)
                    if d.radius <= 0.0
                        || d.drag < 1.0
                        || !(0.0..=1.0).contains(&d.damage)
                        || self.economy.energy_upkeep <= 0.0 =>
                {
                    return Err(DataError::Invalid(format!(
                        "{key}: a warp dampener needs a radius, a drag of at least 1, damage 0 to 1 and energy_upkeep"
                    )));
                }
                Some(d) => Some(crate::WarpDamper {
                    radius: fx(d.radius),
                    drag: fx(d.drag),
                    damage: fx(d.damage),
                    stun_ticks: ticks(d.stun).min(6000) as u16,
                }),
                None => None,
            },
            repair_field: match self.repair_field {
                Some(f) if f.radius <= 0.0 || f.heal <= 0.0 => {
                    return Err(DataError::Invalid(format!(
                        "{key}: a repair field needs a radius and a heal above 0"
                    )));
                }
                Some(f) => Some(crate::RepairField {
                    radius: fx(f.radius),
                    heal: fx(f.heal),
                    mends_self: f.mends_self,
                }),
                None => None,
            },
            fuel: match self.fuel {
                Some(f)
                    if f.range <= 0.0
                        || !self
                            .motion
                            .as_ref()
                            .is_some_and(|m| m.layer == MoveLayer::Air) =>
                {
                    return Err(DataError::Invalid(format!(
                        "{key}: fuel is for aircraft, with a range above 0"
                    )));
                }
                Some(f) => Some(crate::Fuel {
                    range: fx(f.range * 1000.0),
                }),
                None => None,
            },
            staging: match &self.staging {
                Some(s)
                    if s.radius <= 0.0
                        || s.pads == 0
                        || s.refuel <= 0.0
                        || !(s.repair > 0.0 && s.repair <= 1.0)
                        || s.launch_gap <= 0.0
                        || s.launch.len() > crate::MAX_LAUNCHES
                        || s.doors.is_some_and(|d| d <= 0.0) =>
                {
                    return Err(DataError::Invalid(format!(
                        "{key}: staging needs a radius, pads, a refuel time, a launch gap and a repair rate above 0, at most 1, door time above 0 and at most {} launches", crate::MAX_LAUNCHES
                    )));
                }
                Some(s) => {
                    let land = FxVec3::new(fx(s.land.0), fx(s.land.1), fx(s.land.2));
                    let mut launch = [(land, Angle::ZERO); crate::MAX_LAUNCHES];
                    for (to, l) in launch.iter_mut().zip(&s.launch) {
                        *to = (
                            FxVec3::new(fx(l.0), fx(l.1), fx(l.2)),
                            Angle(steps(l.3).round() as i64 as u16),
                        );
                    }
                    Some(crate::Staging {
                        radius: fx(s.radius),
                        pads: s.pads,
                        refuel_ticks: ticks(s.refuel).max(1),
                        repair: fx(s.repair),
                        land,
                        launch,
                        launches: s.launch.len().max(1) as u8,
                        launch_ticks: ticks(s.launch_gap).clamp(1, 600) as u16,
                        door_ticks: s.doors.map_or(0, |d| ticks(d).clamp(1, 600) as u16),
                    })
                }
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
                    recharge: fx(s.recharge.unwrap_or(s.regen * 2.0)),
                }
            }),
            upgrades_to: match &self.upgrades_to {
                Some(k) => Some(lookup(k, key)?),
                None => None,
            },
            upgrade_power: self.upgrade_power.map(fx),
            weapons,
            wreck_fraction: fx(self
                .wreck_fraction
                .unwrap_or_else(|| default_wreck(self.tech))),
            visual: Visual {
                skin: self.skin.clone(),
                effects: self.effects,
                mesh: self.mesh.clone(),
                icon: self.icon,
                lights: self.lights.clone(),
            },
            sounds: self.sounds.clone(),
            lore: String::new(),
            refit: None,
            mission_only: self.mission_only,
            scrap: false,
            carried_drone: false,
        })
    }
}
