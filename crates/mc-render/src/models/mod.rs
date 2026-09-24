//! Procedural models. There are no authored art assets yet: every unit,
//! structure and prop is generated from code at start-up, with three levels of
//! detail. Below the last level the renderer switches to strategic icons.

/// Surface classes. The fragment shader turns these into PBR parameters, so a
/// faction's palette can change without rebuilding meshes.
pub mod material {
    /// Main armour plating (Aster: stark white).
    pub const PLATING: u32 = 0;
    /// Frame, joints, recesses (Aster: black / dark grey).
    pub const ACCENT: u32 = 1;
    /// Faction highlight emitters (Aster: near-white blue). Emissive.
    pub const GLOW: u32 = 2;
    /// Owner's team colour stripe.
    pub const TEAM: u32 = 3;
    /// Bare gunmetal: barrels, pistons.
    pub const METAL: u32 = 4;
    /// Sensors, canopies.
    pub const GLASS: u32 = 5;
    /// Treads, tyres, feet.
    pub const TREAD: u32 = 6;
    /// Conventional-weapon emitters (orange). Emissive.
    pub const GLOW_ORANGE: u32 = 7;
    pub const BARK: u32 = 8;
    pub const FOLIAGE: u32 = 9;
    pub const ROCK: u32 = 10;
    pub const CONCRETE: u32 = 11;
    /// Lit windows on city buildings. Mildly emissive.
    pub const WINDOWS: u32 = 12;
    /// Construction emitters: the yellow-orange of a build beam. Emissive.
    pub const GLOW_AMBER: u32 = 13;
    /// Armour painted dark: the faction's plating colour taken down to graphite.
    pub const PLATING_DARK: u32 = 14;
    /// Obstruction / beacon lamp (red). Emissive; the shader blinks it.
    pub const GLOW_RED: u32 = 15;
    /// Replication light (the Survival replicators' white-hot violet). Emissive.
    pub const GLOW_VIOLET: u32 = 16;
}

/// What is drawn on a face, on top of its material. Every face of a plated
/// material is textured by the shader from the face's own shape (see
/// [`MeshVertex::face`]): outlines, rivets and sub-panels fitted to it. A
/// pattern swaps that generic treatment for a specific one. `surface.wgsl`
/// holds the other half of this table.
pub mod pattern {
    /// Fitted plates by material: the default.
    pub const GENERIC: u32 = 0;
    /// One plate with its outline and nothing else: small lids, trims.
    pub const PLAIN: u32 = 1;
    /// A roller door: slats across, a hazard sill, a team band at the head.
    pub const SHUTTER: u32 = 2;
    /// A factory lift deck: print grid and corner marks; a scan runs over it while the factory builds.
    pub const DECK: u32 = 3;
    /// A marked apron: chevrons toward +s, edge lights that run outward while the factory builds.
    pub const ROADWAY: u32 = 4;
    /// Louvres over a furnace: the glow between the slats breathes, harder while the factory builds.
    pub const FURNACE: u32 = 5;
    /// A dark wall carrying power: lines of light that flow toward +s while the factory builds.
    pub const CONDUIT: u32 = 6;
    /// A roof plate with the owner's colour laid along one edge.
    pub const TEAM_BAND: u32 = 7;
    /// Bare: no fitted detail at all (decals, markings that are their own picture).
    pub const NONE: u32 = 8;
    /// Aircraft skin: flush panels with fine seams, rows of countersunk fasteners,
    /// screwed access panels and small stencils. No raised plate courses.
    pub const AIRFRAME: u32 = 9;
    /// A pile or wale standing in water: wet and dark below the waterline (model z = 0),
    /// a ragged band of weed and rust at it, rust weeping down, draught marks above.
    pub const PILE: u32 = 10;
    /// Safety stripes, black on safety orange, across the face: quay copings, hazard edges.
    pub const HAZARD: u32 = 11;
    /// A ship's hull side, laid out by the waterline (model z = 0), not by the face: antifouling
    /// under it, a black boot-top band at it, welded strakes and butts that run on across
    /// facets, draught marks at bow and stern, a hull number and a raked team slash forward.
    pub const HULL: u32 = 12;
    /// A submarine's casing: rubber anechoic tiles fitted to the face, the odd one lost,
    /// a salt line at the waterline and draught marks.
    pub const TILES: u32 = 13;
    /// A ship's walkway: grey non-skid inside a white margin, a painted edge line, tie-down points.
    pub const WALKWAY: u32 = 14;
    /// A reactor viewport, on dark plating: armoured slits onto the burning core, the
    /// plasma churning past them on a slow beat. Slits wrap round a drum.
    pub const PLASMA: u32 = 15;
    /// A reactor's power run, on dark plating: a channel down the long axis with blue
    /// pulses running out along +s for as long as the plant burns.
    pub const FLUX: u32 = 16;
    /// Dark plating (`ACCENT`) whose little level lights are the replicators' violet, not
    /// Aster's orange: the Survival replicators' obsidian.
    pub const VEINED: u32 = 17;
    pub const LAST: u32 = VEINED;
}

/// Which rigid part of the model a vertex belongs to. The vertex shader
/// animates parts; the mesh itself is static.
pub mod part {
    pub const HULL: u32 = 0;
    /// Yaws around `Model::turret_pivot` by the unit's turret angle.
    pub const TURRET: u32 = 1;
    /// Spins continuously around `Model::spinner_pivot` (radar dishes, extractor intakes).
    pub const SPINNER: u32 = 2;
    pub const ROTOR: u32 = 4;
    /// A VTOL's engine pods, fore and aft: tilted about their pivots
    /// (`vtol_nacelles`) between hover and cruise by how the aircraft flies; a
    /// `rig::SPIN` fan or turbine in one turns about the pod's own axis.
    pub const VTOL_FRONT: u32 = 5;
    pub const VTOL_REAR: u32 = 6;
    /// A carrier's hold doors: two leaves hinged at the hold's sides that swing
    /// down and out as the hold opens (`UnitInstance::deploy`).
    pub const HOLD_DOOR: u32 = 7;
    /// A carrier's drone cradles: lowered out of the hold with the flock.
    pub const CRADLE: u32 = 8;
    /// A core mine's pile driver: authored resting on the pipe string, hauled up and
    /// dropped along z on the mine's beat (`UnitInstance::gait`) by `Pit::stroke`.
    pub const RAM: u32 = 9;
    /// A core mine's pipe string down the bore: driven down one `Pit::section` with each
    /// blow. It repeats every section, so it seems to go on down for ever.
    pub const STRING: u32 = 10;
    /// A core mine's next pipe section: authored waiting at `Pit::rack`, it rises out of
    /// the magazine there, swings over the bore onto the string, and is driven down with it.
    pub const FEED: u32 = 11;
    /// Drawn only where the structure stands in water: an offshore rig's stilts.
    pub const AFLOAT: u32 = 12;
    /// Drawn only where the structure stands on land: the pit and the ground it breaks.
    pub const ASHORE: u32 = 13;
    /// A reactor's pump or injector: rides up and down along z a short stroke, each at
    /// its own phase round the plant (from where it stands), while the plant runs.
    pub const PUMP: u32 = 14;
    /// An airbase's hatch leaves: slid apart along y, each away from the middle, by the
    /// pit's radius times how far the hatch is open (`UnitInstance::deploy`).
    pub const HATCH: u32 = 15;
    /// A lift ship's ventral ramp: authored down, swung up about its hinge as it closes
    /// (`bastion::RAMP_HINGE`, `UnitInstance::deploy`).
    pub const RAMP: u32 = 16;
    /// A spacecraft's legs: swung up about their hinges into bays in the belly as the
    /// gear stows (`aster::air::capital`, `capital_rig`).
    pub const GEAR: u32 = 17;
    /// Reserved former transport fan part; spacecraft have no rotating lift fans.
    pub const FAN: u32 = 18;
    /// A spacecraft drive's iris: vanes turning slowly about the drive's axis (`capital_rig`).
    pub const DRIVE: u32 = 19;
    /// A lift ship's lower leg: telescoped up into its `GEAR` leg, then stowed with it.
    pub const GEAR_STRUT: u32 = 20;
    /// A lift ship's foot: its pads fold up, then it rides the strut and the leg.
    pub const GEAR_FOOT: u32 = 21;
    /// A lift ship's gear bay doors: authored shut, swung down open as the legs come out.
    pub const GEAR_DOOR: u32 = 22;
    /// Tread / leg surfaces: the shader scrolls or bobs these with distance travelled.
    pub const LOCOMOTION: u32 = 3;
}

/// How a vertex is rigged beyond its part: which bone of a walking leg it
/// rides, and whether it belongs to the unit's next upgrade.
pub mod rig {
    /// Low bits: the leg bone. The vertex shader poses legs by two-bone IK
    /// from `Model::legs`; the side comes from the sign of the vertex's y.
    pub const THIGH: u32 = 1;
    pub const SHIN: u32 = 2;
    pub const FOOT: u32 = 3;
    /// A forearm that pitches about `Model::arm_pivot` to point up or down at what
    /// it aims at: the first weapon's arm, and the build arm.
    pub const ARM_GUN: u32 = 4;
    pub const ARM_TOOL: u32 = 5;
    /// Upper boom of a two-bone build arm: pitches about the turret/shoulder,
    /// carrying the `ARM_TOOL` forearm with it.
    pub const ARM_BOOM: u32 = 6;
    pub const LIMB_MASK: u32 = 0xF;
    /// Slides back along the barrel when the gun fires (`Model::recoil`).
    pub const RECOIL: u32 = 1 << 4;
    /// Hover skirt: the shader drops it on water and tucks it up on land.
    pub const FLOAT: u32 = 1 << 5;
    /// Factory build deck: up while a unit is printing, then lowers to release it.
    pub const LIFT: u32 = 1 << 6;
    /// Siege outriggers / recoil spade: folded up when packed, planted when deployed.
    pub const DEPLOY: u32 = 1 << 7;
    /// Part of what the unit's upgrade adds: not drawn until the refit is under
    /// way, then a hologram, then built. Bits 16..24 say when in the refit it
    /// goes up (0..=255 of the way through).
    pub const UPGRADE: u32 = 1 << 8;
    pub const UPGRADE_AT_SHIFT: u32 = 16;
    pub const UPGRADE_AT_MASK: u32 = 0xFF << UPGRADE_AT_SHIFT;
    /// Folding gear: swung about `Model::fold` out of the way while the unit is not building.
    pub const FOLD: u32 = 7;
    /// A refit module's piece: its look bit plus one in bits 9..15 (zero: always there).
    /// Drawn while the unit has the module; raised during the refit that fits it,
    /// at the time in the `UPGRADE_AT` bits.
    pub const MODULE_SHIFT: u32 = 9;
    pub const MODULE_MASK: u32 = 0x3F << MODULE_SHIFT;
    /// A piece a refit module takes off: that module's look bit plus one in bits 24..30.
    pub const UNTIL_SHIFT: u32 = 24;
    pub const UNTIL_MASK: u32 = 0x3F << UNTIL_SHIFT;
    /// A weapon on a turret of its own on the turret (a shoulder gun): turns and pitches about
    /// `Model::mount`.
    pub const MOUNT: u32 = 8;
    /// The head at the end of the `FOLD` gear: pitches about `Model::fold_wrist` (folded
    /// back along the arm when stowed, aimed at the work when out), then rides the arm.
    pub const FOLD_HEAD: u32 = 9;
    /// A walker's head: turns and nods about `Model::neck` while the unit stands idle.
    pub const HEAD: u32 = 10;
    /// A gun house of its own on the hull (`Model::houses`): limbs `HOUSE_FIRST..HOUSE_FIRST + HOUSE_COUNT`,
    /// one per house, each bound to a weapon whose yaw and pitch the mirror publishes
    /// (`mirror::HousePose`). The house turns about its pivot; its `RECOIL` verts pitch and kick too.
    pub const HOUSE_FIRST: u32 = 11;
    pub const HOUSE_COUNT: u32 = 4;
    /// Rotary barrels: turn about the axis `Model::spins` gives for the loadout.
    pub const SPIN: u32 = 1 << 30;
    /// Build-arm gear that works while the unit builds, eased in and out with the builder's
    /// deploy: twists back and forth about the arm's axis, runs out along it, or opens and
    /// closes round it. Two bits: `WORK_TWIST`, `WORK_EXTEND`, or both (`WORK_BREATHE`).
    pub const WORK_TWIST: u32 = 1 << 15;
    pub const WORK_EXTEND: u32 = 1 << 31;
    pub const WORK_BREATHE: u32 = WORK_TWIST | WORK_EXTEND;
    pub const WORK_MASK: u32 = WORK_BREATHE;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshVertex {
    /// Model space, metres: x forward, y left, z up, origin on the ground under the centre.
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    /// Metres along the surface (box-projected); the shader tiles panel-line normal maps with it.
    pub uv: [f32; 2],
    pub material: u32,
    pub part: u32,
    /// `rig` bits.
    pub rig: u32,
    /// Where the vertex sits on its own face, so the shader can fit detail to the face's
    /// shape: xy metres from the middle of the face's bounding rectangle, along the face's
    /// own axes (y runs up a wall), zw that rectangle's half size. A negative half width
    /// marks x as going right round a tube: no outline there. All zero: no frame.
    pub face: [f32; 4],
    /// [`pattern`] in the low byte, then a byte of per-face randomness.
    pub surface: u32,
}

#[derive(Clone, Debug, Default)]
pub struct MeshLod {
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<u32>,
}

pub const LOD_COUNT: usize = 3;

/// Where a tracked model touches the ground, in model space (metres).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Treads {
    /// Distance from the centre line to the middle of each track.
    pub half_gauge: f32,
    /// Width of one track.
    pub width: f32,
    /// x of the tracks' rear end, where the dust comes off.
    pub rear: f32,
}

/// A walker's legs, in model space (metres): the joints of the left (+y) leg
/// standing at rest. The right leg is its mirror image, half a cycle behind.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Legs {
    pub hip: [f32; 3],
    pub knee: [f32; 3],
    pub ankle: [f32; 3],
    /// Ground covered by one full cycle (both feet). A power of two, so the
    /// sim's wrapping distance counter never breaks the stride.
    pub stride: f32,
    /// Share of the cycle a foot spends planted. Over a half is a walk; under it
    /// is a run, with both feet off the ground between steps.
    pub stance: f32,
    /// How high a foot is lifted on its way forward.
    pub lift: f32,
    /// How far the hips sink in full stride: bent knees give the legs the reach a
    /// long walking stride needs. Zero walks stood up.
    pub crouch: f32,
    /// Sole in model space: metres behind the ankle, ahead of it, and the
    /// sole's width. Zero if this walker does not stamp the ground.
    pub foot: [f32; 3],
}

#[derive(Clone, Debug)]
pub struct Model {
    pub key: String,
    /// Full detail, reduced, and a handful of boxes.
    pub lods: [MeshLod; LOD_COUNT],
    pub turret_pivot: [f32; 3],
    pub spinner_pivot: [f32; 3],
    /// Radius of the bounding sphere around the model origin.
    pub bounds_radius: f32,
    /// How big the model is for its surface: the bounds with guns and arms at rest, not
    /// swung up. Plate courses and burn marks are sized from it, so a long gun that can
    /// point at the sky does not coarsen the whole hull's texture.
    pub surface_reach: f32,
    /// Height up to which the running gear's dust coats the model (the field dirt in
    /// `entity.wgsl`): 62% of its height unless the model says (`MeshBuilder::set_dust_line`).
    pub dust_line: f32,
    /// Set for tracked vehicles: they mark the ground and raise dust.
    pub treads: Option<Treads>,
    /// Set for walkers: their legs are posed by the vertex shader.
    pub legs: Option<Legs>,
    /// Set for hovercraft: the hull rides a cushion, not treads or legs.
    pub hover: bool,
    /// The left (+y) elbow, for models whose forearms pitch (`rig::ARM_GUN`, `ARM_TOOL`);
    /// the right one is its mirror image. The unit file's `pivot`s say the same.
    pub arm_pivot: Option<[f32; 3]>,
    /// The elbow is the second bone of a folding boom: the shoulder is `turret_pivot`.
    pub arm_boom: bool,
    /// Rest-space barrel axis (xyz) and how far `rig::RECOIL` verts kick back
    /// (w, metres). None if the tube does not slide.
    pub recoil: Option<[f32; 4]>,
    /// Hinge (xyz) of the `rig::FOLD` gear and how far it swings back when stowed (w, radians).
    pub fold: Option<[f32; 4]>,
    /// Wrist (xyz) of the head on the `rig::FOLD` gear and how far it folds back when
    /// stowed (w, radians).
    pub fold_wrist: Option<[f32; 4]>,
    /// Where a walker's head (`rig::HEAD`) turns: on the centreline, at this x and z.
    pub neck: Option<[f32; 2]>,
    /// Trunnion (xyz) of the `rig::MOUNT` turret and how far its tube kicks back (w, metres).
    pub mount: Option<[f32; 4]>,
    /// Gun houses of their own (`rig::HOUSE_FIRST + i`), in slot order.
    pub houses: Vec<House>,
    /// Axes of `rig::SPIN` barrels (a point on the axis, which runs along x), with the module
    /// tags (`rig::MODULE`, `rig::UNTIL` values) under which each applies.
    pub spins: Vec<(u32, u32, [f32; 3])>,
    /// A hole the model digs into the ground, and the pipe it drives down it.
    pub pit: Option<Pit>,
}

/// A gun house turning on the hull by itself: where it turns (its pivot, model space), how
/// far its `rig::RECOIL` verts kick back when it fires, and which weapon of the unit it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct House {
    pub pivot: [f32; 3],
    pub travel: f32,
    pub weapon: u8,
}

/// A hole a model digs into the ground (a core mine's). The vertex shader pulls what is
/// inside and below the opening up in depth so the terrain does not hide it, drives the
/// `part::RAM`, `STRING` and `FEED` pieces on the mine's beat, and on water raises the rig
/// onto its `part::AFLOAT` stilts and leaves the `part::ASHORE` ground out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pit {
    /// Height of the opening, and its radius there.
    pub open: f32,
    pub radius: f32,
    /// How far the pile driver is hauled up before it drops.
    pub stroke: f32,
    /// Length of one pipe section: how far each blow drives the string.
    pub section: f32,
    /// Where the next section waits, raised, before it swings over the bore.
    pub rack: [f32; 2],
    /// How far everything but the stilts rises when the structure stands in water.
    pub afloat_lift: f32,
}

mod aster;
pub mod builder;
pub mod burns;
mod footprint;
mod library;
#[cfg(test)]
mod preview;
mod props;
mod replicator;
pub mod shell;
#[cfg(test)]
mod tests;
mod thumbnail;

pub use footprint::{
    bake_hull_plan, bake_pad_footprint, hull_plan_at, hull_plan_half, hull_plan_sd, pad_sdf_at,
    PAD_FOOTPRINT_REACH, PAD_FOOTPRINT_RES, PAD_SDF_RANGE,
};
pub use library::{all_model_keys, build_model, build_model_fitted, build_model_scaled, prop_model_key};
pub use thumbnail::{material_color, thumbnail, thumbnail_of};

/// The tilting engine pods of a VTOL: pivots of the front and rear pod on the left
/// (+y) side, in model space; the right side is the mirror. The entity shader tilts
/// `part::VTOL_FRONT` and `VTOL_REAR` about them (its copy of these numbers is in
/// `entity.wgsl`), and the exhaust emitter tilts the nozzles the same way.
pub fn vtol_nacelles(mesh: &str) -> Option<[[f32; 3]; 2]> {
    match mesh {
        "gunship" => Some(aster::air::KESTREL_NACELLES),
        "reclaim_carrier" => Some(aster::air::OSPREY_NACELLES),
        _ => None,
    }
}

/// Where the Osprey's four Salvage Drones sit in its hold (x, y in model space) and
/// the hold's ceiling they hang from; the sim's `drone_socket` says the same.
pub fn carrier_cradles() -> ([[f32; 2]; 4], f32) {
    (aster::air::OSPREY_CRADLES, aster::air::OSPREY_HOLD_CEILING)
}

/// Lamp fittings on a capital ship's hull (model space, +X forward, +Y left, metres) for
/// the renderer's lamps (`renderer/capital_fx.rs`): landing floods, nav lights, strobes,
/// ramp beacons and the hold's lamp.
pub struct CapitalLamps {
    /// Landing floodlights under the belly, aimed down; the first half lean forward.
    pub floods: &'static [[f32; 3]],
    /// Red to port (+Y), green to starboard.
    pub nav_port: [f32; 3],
    pub nav_starboard: [f32; 3],
    /// White anti-collision strobes at the extremities.
    pub strobes: &'static [[f32; 3]],
    /// Amber beacons that turn while the ramp moves.
    pub beacons: &'static [[f32; 3]],
    /// The hold's lamp, and the x where the open ramp's lip meets the ground; `None`
    /// for a ship without a ramp.
    pub hold: Option<([f32; 3], f32)>,
}

/// The Bastion's lamps sit in fittings the model builds from the same numbers.
const BASTION_LAMPS: CapitalLamps = aster::air::BASTION_LAMPS;

/// A capital ship's lamp fittings by mesh; `None` for a hull without any.
pub fn capital_lamps(mesh: &str) -> Option<&'static CapitalLamps> {
    match mesh {
        "lift_ship" => Some(&BASTION_LAMPS),
        "light_transport" => Some(&aster::air::COURIER_LAMPS),
        _ => None,
    }
}

/// A spacecraft's rig as `entity.wgsl` reads it (`ModelInfo::capital`, laid out by
/// `aster::air::capital::CapitalRig::gpu`): its landing legs and bay doors, drives, lift
/// jets and ramp. `None` for everything else. A new spacecraft adds its `CapitalRig` here.
pub fn capital_rig(mesh: &str) -> Option<[[f32; 4]; 7]> {
    match mesh {
        "lift_ship" => Some(aster::air::BASTION_RIG.gpu()),
        "light_transport" => Some(aster::air::COURIER_RIG.gpu()),
        _ => None,
    }
}

/// Downward lift jet mouths in model space (the Bastion's belly), for the renderer's drive effects.
pub fn lift_jets(mesh: &str) -> &'static [[f32; 3]] {
    match mesh {
        "lift_ship" => &aster::air::BASTION_LIFT_JETS,
        "light_transport" => &aster::air::COURIER_LIFT_JETS,
        _ => &[],
    }
}

/// Jet nozzle origins in model space, shared with the aircraft effect renderer.
pub fn aircraft_exhausts(mesh: &str) -> &'static [[f32; 3]] {
    match mesh {
        "light_transport" => &aster::air::COURIER_NOZZLES,
        "lift_ship" => &aster::air::BASTION_NOZZLES,
        "interceptor" => &[[-3.31, -0.2, 0.9], [-3.31, 0.2, 0.9]],
        "bomber" => &[[-2.68, -2.35, 0.95], [-2.68, 2.35, 0.95]],
        "air_scout" => &[[-2.97, 0.0, 0.65]],
        "support_air" => &[[-3.37,-3.5,0.9],[-3.37,3.5,0.9]],
        "reclaim_carrier" => &aster::air::OSPREY_NOZZLES,
        "reclaim_drone" => &aster::air::DRONE_NOZZLES,
        "gunship" => &aster::air::KESTREL_NOZZLES,
        "fire_bomber" => &[[-4.17,-9.0,1.6],[-4.17,-5.0,1.6],[-4.17,5.0,1.6],[-4.17,9.0,1.6]],
        "interceptor_t2" => &[[-5.11,-0.55,0.9],[-5.11,0.55,0.9]],
        "torpedo_bomber" => &[[-1.95,-2.55,0.62],[-1.95,2.55,0.62]],
        "superiority" => &[[-6.52,-0.72,1.02],[-6.52,0.72,1.02]],
        "strategic_bomber" => &[[-5.87,-2.2,1.4],[-5.87,2.2,1.4]],
        "assault_air" => &[[-7.37,-3.4,3.6],[-7.37,3.4,3.6]],
        _ => &[],
    }
}
