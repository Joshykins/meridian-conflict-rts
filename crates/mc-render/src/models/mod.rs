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
    /// A white-hot violet light. Emissive. (The Survival replicators' until they became
    /// Precursor work: they use `GLOW_PRECURSOR` now.)
    pub const GLOW_VIOLET: u32 = 16;
    /// Missile-defence emitters: a steady laser red, so the anti-missile kit reads apart
    /// from the rest of the model. Emissive; never blinks.
    pub const GLOW_LASER: u32 = 17;
    /// Precursor alloy: the Foundry's pale, stone-matte grey. Textured by
    /// `pattern::PRECURSOR` unless a model asks for another.
    pub const PRECURSOR: u32 = 18;
    /// Precursor recesses, joints and the dark between plates.
    pub const PRECURSOR_DARK: u32 = 19;
    /// Precursor light: a cold blue-white, alive (it breathes and runs). Emissive.
    pub const GLOW_PRECURSOR: u32 = 20;
    /// A ship's port sidelight: steady navigation red. Emissive; never blinks.
    pub const GLOW_NAV_RED: u32 = 21;
    /// A ship's starboard sidelight: steady navigation green. Emissive.
    pub const GLOW_NAV_GREEN: u32 = 22;
    /// A plain working lamp: masthead, stern and deck lights, lit ports. A warm
    /// incandescent white, dimmer than the faction's emitters. Emissive.
    pub const GLOW_LAMP: u32 = 23;
    /// Shield projector emitters: the faction's shield colour (`faction.ron` `shield_color`,
    /// ARC gold), so a generator reads as the source of its field. Emissive.
    pub const GLOW_SHIELD: u32 = 24;
    /// A Precursor light channel lying dormant: dark glass let into the alloy, a cold
    /// sheen on it, stirring faintly as a survival facility wakes. Most of a Precursor
    /// structure's channels are this; only its working parts carry `GLOW_PRECURSOR`.
    pub const PRECURSOR_INLAY: u32 = 25;
    /// A helmet visor: mirrored gold-orange glass with a faint warm light behind it,
    /// so it reads as the commander's face from strategic zoom.
    pub const VISOR: u32 = 26;
    pub const LAST: u32 = VISOR;
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
    /// Dark plating (`ACCENT`) whose little level lights are the Precursors' cold blue, not
    /// Aster's orange. (Was the Survival replicators' obsidian; they wear `PRECURSOR` now.)
    pub const VEINED: u32 = 17;
    /// Precursor plate: incised angular panel lines and inlaid seams of light.
    /// Every precursor-material face gets it unless it asks for another.
    pub const PRECURSOR: u32 = 18;
    /// A charge coil's light (`GLOW` faces only), stage 0 at the breech to 7 at the muzzle:
    /// `COIL + stage`. The shader breathes it idle, climbs it stage by stage through the
    /// weapon's charge, blinds at the shot and lets it cool (the Behemoth's AEB-3; the
    /// renderer feeds the charge through `UnitInstance::mount`, `renderer/titan_charge.rs`).
    pub const COIL: u32 = 19;
    pub const COIL_STAGES: u32 = 8;
    /// A capacitor ring's lugs on such a weapon (`METAL` faces only): turned about the
    /// bore's axis (the model's rotary axis mirrored across the centreline) slowly at rest
    /// and hard as it charges; `COIL_TURN_BACK` the other way.
    pub const COIL_TURN: u32 = COIL + COIL_STAGES;
    pub const COIL_TURN_BACK: u32 = COIL_TURN + 1;
    /// Dark plating (`ACCENT`) whose little level lights are the Naga's red, and lit at every
    /// tier: a Naga hide's seams of light. (Aster's black carries no lit lines at all.)
    pub const EMBER: u32 = COIL_TURN_BACK + 1;
    pub const LAST: u32 = EMBER;
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
    /// An airbase's hatch leaves (the parked Roost model): slid apart along y, each away from the middle, by the
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
    /// A strategic launcher's store hatch lid: slid open and shut again on each load
    /// cycle while a round is assembling (`gpu_consts::launcher`). 23 was the network
    /// nodes' door leaves, removed with them.
    pub const LAUNCHER_LID: u32 = crate::gpu_consts::launcher::PART_LID;
    /// A strategic launcher's blast-door leaves: authored shut, meeting on y = 0; slid
    /// apart along y, each away from the middle, by the opening's half width times how far
    /// the doors are open (`UnitInstance::deploy`): 5.2 m on the silo, 5.0 m on the
    /// interceptor array (`aster::strategic`; `entity.wgsl` has the same numbers).
    pub const SILO_DOOR: u32 = 24;
    /// The rounds a strategic launcher holds ready: the silo's warhead in its tube, the
    /// array's interceptors in their cells. Drawn only while it has that many in stock: a
    /// round in the cell at quadrant k (x < 0 first, then y < 0) is drawn while stock > k;
    /// the silo's single tube while stock > 0.
    pub const SILO_ROUND: u32 = 25;
    /// A joining wall's pieces, `WALL_COUNT` of them from here: the shader draws the one
    /// each quarter's neighbours call for (`gpu_consts::wall`, `super::wall::shown`).
    pub const WALL_FIRST: u32 = crate::gpu_consts::wall::PART_FIRST;
    pub const WALL_COUNT: u32 = 4 * crate::gpu_consts::wall::CASES;
    /// A strategic launcher's hoist block, with the lower half of its cables: let down
    /// into the open hatch and hauled up on each load cycle (`gpu_consts::launcher`).
    pub const LAUNCHER_HOIST: u32 = crate::gpu_consts::launcher::PART_HOIST;
    const _: () = assert!(LAUNCHER_HOIST >= WALL_FIRST + WALL_COUNT);
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
    /// On a leg (`part::LOCOMOTION`): a reverse-kneed leg's lower bone, from the hock down to
    /// the ankle (`MeshBuilder::set_hock`). Shares its number with `ARM_GUN`, never on legs.
    pub const TARSUS: u32 = 4;
    pub const LIMB_MASK: u32 = 0xF;
    /// Slides back along the barrel when the gun fires (`Model::recoil`).
    pub const RECOIL: u32 = 1 << 4;
    /// A breech door on the `ARM_GUN` limb: swings open about `Model::breech` as the gun
    /// fires and shuts as it runs out (`gpu_consts::breech`).
    pub const BREECH: u32 = crate::gpu_consts::breech::RIG;
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
    /// A many-legged walker's tail (`Crawl::tail`): bends toward where the turret faces,
    /// nothing at its root and the whole yaw at its top, where the turret's own pieces ride.
    /// Which segment (turning about which of `Crawl::tail_joints`) rides `TAIL_SEG` bits.
    /// On the hull the same limb is a pincer: `TAIL_SEG` then `CLAW_ARM` or `CLAW_JAW`.
    pub const TAIL: u32 = 15;
    pub const TAIL_SEG_SHIFT: u32 = 16;
    pub const TAIL_SEG_MASK: u32 = 0xF << TAIL_SEG_SHIFT;
    /// A pincer's arm, swinging about `Crawl::claw`'s shoulder.
    pub const CLAW_ARM: u32 = 14;
    /// A pincer's moving finger: rides the arm and opens about its hinge.
    pub const CLAW_JAW: u32 = 15;
    /// Which pair of a many-legged walker's legs (`Crawl`) a leg vertex belongs to, 0..4,
    /// in the `UPGRADE_AT` bits: those only mean anything on refit pieces, never on legs.
    pub const PAIR_SHIFT: u32 = 16;
    pub const PAIR_MASK: u32 = 0x7 << PAIR_SHIFT;
    /// A gun house of its own on the hull (`Model::houses`): limbs `HOUSE_FIRST..HOUSE_FIRST + HOUSE_COUNT`,
    /// one per house, each bound to a weapon whose yaw and pitch the mirror publishes
    /// (`mirror::HousePose`). The house turns about its pivot; its `RECOIL` verts pitch and kick too.
    pub const HOUSE_FIRST: u32 = 11;
    /// Houses 4..8 reuse limbs `HOUSE_FIRST..HOUSE_FIRST + 4` with `HOUSE_HIGH` set.
    pub const HOUSE_COUNT: u32 = 8;
    /// Marks a house limb as houses 4..8. It borrows the top bit of `UPGRADE_AT`, which only
    /// means anything on refit pieces, so a house past the fourth cannot be one.
    pub const HOUSE_HIGH: u32 = 1 << 23;
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
    /// How far back the sole's corners are cut at 45 degrees (metres, model space): a
    /// giant's print in the ground takes the sole's outline (`ground.wgsl` `footprint`).
    pub sole_chamfer: f32,
    /// A reverse-kneed leg (`MeshBuilder::set_hock`): the hock between the knee and the
    /// ankle, where the leg bends back, and how much of the leg's swing the bone below it
    /// (`rig::TARSUS`) follows. None: the shin runs from the knee to the ankle.
    pub hock: Option<([f32; 3], f32)>,
    /// More than one pair of legs: then `hip`/`knee`/`ankle` are the first pair's, and
    /// every leg is posed from its own pair here (`MeshBuilder::set_crawl_legs`).
    pub crawl: Option<Crawl>,
}

/// Most leg pairs a many-legged walker can have (`entity.wgsl` `ModelInfo::crawl`).
pub const MAX_CRAWL_PAIRS: usize = 4;
/// Most joints a many-legged walker's tail can have (`Crawl::tail_joints`).
pub const MAX_TAIL_JOINTS: usize = 12;
/// `ModelInfo::crawl`'s length in vec4s: the header, three per leg pair, two joints of the
/// tail per vec4, and the pincers' shoulder and jaw hinge.
pub const CRAWL_SLOTS: usize = 1 + 3 * MAX_CRAWL_PAIRS + MAX_TAIL_JOINTS / 2 + 2;

/// A many-legged walker (the Naga commander): each left leg's joints at rest, the right
/// one its mirror. A leg's bones swing in the vertical plane through its hip and foot, and
/// that plane turns about the hip to follow the foot through its stride.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crawl {
    pub pairs: usize,
    /// Per pair: hip, knee, ankle (the foot's tip on the ground), model space.
    pub joints: [[[f32; 3]; 3]; MAX_CRAWL_PAIRS],
    /// Where in the cycle each pair's left foot lifts (0..1); its right foot is half a cycle on.
    pub phase: [f32; MAX_CRAWL_PAIRS],
    /// The tail's root and top heights (`rig::TAIL`): zero for no tail.
    pub tail: [f32; 2],
    /// The tail's spine at rest, root first, as (x, z) in its plane (y = 0): each
    /// `rig::TAIL` segment `i` turns about joint `i`, and the turret's own pieces ride the
    /// last joint (`entity.wgsl` `tail_pose`). `tail_count` of them are used.
    pub tail_joints: [[f32; 2]; MAX_TAIL_JOINTS],
    pub tail_count: usize,
    /// The left pincer's shoulder and its moving finger's hinge (`rig::CLAW_ARM`,
    /// `rig::CLAW_JAW`); the right is the mirror. None for no pincers.
    pub claw: Option<[[f32; 3]; 2]>,
    /// The weapon slot each pincer throws with, left then right (`set_claw_throws`), so
    /// it snaps and kicks on its own weapon's shots. None: they throw nothing.
    pub throws: Option<[u8; 2]>,
}

impl Crawl {
    /// As `entity.wgsl` reads `ModelInfo::crawl`: [0] pair count, tail root z, tail top z,
    /// tail joint count; then per pair hip (w: phase), knee, ankle; then the tail's joints,
    /// two to a vec4 (x, z, x, z); then the pincer's shoulder (w: 1 when there are pincers)
    /// and its jaw hinge (w: the left pincer's weapon slot plus one, and the right's plus
    /// one times 16; zero for none).
    pub fn gpu(&self) -> [[f32; 4]; CRAWL_SLOTS] {
        let mut out = [[0.0; 4]; CRAWL_SLOTS];
        out[0] = [
            self.pairs as f32,
            self.tail[0],
            self.tail[1],
            self.tail_count as f32,
        ];
        for i in 0..self.pairs {
            let [h, k, a] = self.joints[i];
            out[1 + 3 * i] = [h[0], h[1], h[2], self.phase[i]];
            out[2 + 3 * i] = [k[0], k[1], k[2], 0.0];
            out[3 + 3 * i] = [a[0], a[1], a[2], 0.0];
        }
        let base = 1 + 3 * MAX_CRAWL_PAIRS;
        for (i, j) in self.tail_joints[..self.tail_count].iter().enumerate() {
            out[base + i / 2][(i % 2) * 2] = j[0];
            out[base + i / 2][(i % 2) * 2 + 1] = j[1];
        }
        if let Some([shoulder, hinge]) = self.claw {
            let at = base + MAX_TAIL_JOINTS / 2;
            out[at] = [shoulder[0], shoulder[1], shoulder[2], 1.0];
            let throws = self
                .throws
                .map_or(0.0, |[l, r]| (l as u32 + 1 + 16 * (r as u32 + 1)) as f32);
            out[at + 1] = [hinge[0], hinge[1], hinge[2], throws];
        }
        out
    }
}

#[derive(Clone, Debug)]
pub struct Model {
    pub key: String,
    /// Full detail, reduced, and a handful of boxes.
    pub lods: [MeshLod; LOD_COUNT],
    /// The level past the coarse one for a prop only a few pixels across
    /// (`ModelDef::with_far`); None draws the coarse level there.
    pub far: Option<MeshLod>,
    pub turret_pivot: [f32; 3],
    pub spinner_pivot: [f32; 3],
    /// The spinner looks about rather than turning round (`MeshBuilder::set_spinner_scan`).
    pub spinner_scans: bool,
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
    /// Hinge (xyz, rest pose; the hinge runs along y) of the `rig::BREECH` door and how far
    /// it swings open (w, radians about y: negative swings the bottom back and up).
    pub breech: Option<[f32; 4]>,
    /// Wrist (xyz) of the head on the `rig::FOLD` gear and how far it folds back when
    /// stowed (w, radians).
    pub fold_wrist: Option<[f32; 4]>,
    /// Where a walker's head (`rig::HEAD`) turns: on the centreline, at this x and z.
    pub neck: Option<[f32; 2]>,
    /// Where a personal (hull) shield is thrown from, bind pose. None: the top of the hull
    /// over the model's middle.
    pub shield_emitter: Option<[f32; 3]>,
    /// Trunnion (xyz) of the `rig::MOUNT` turret and how far its tube kicks back (w, metres).
    pub mount: Option<[f32; 4]>,
    /// Gun houses of their own (`rig::HOUSE_FIRST + i`), in slot order.
    pub houses: Vec<House>,
    /// Axes of `rig::SPIN` barrels (a point on the axis, which runs along x), with the module
    /// tags (`rig::MODULE`, `rig::UNTIL` values) under which each applies.
    pub spins: Vec<(u32, u32, [f32; 3])>,
    /// A hole the model digs into the ground, and the pipe it drives down it.
    pub pit: Option<Pit>,
    /// The beam a Naga mine digs its bore with (`renderer/naga_mine_fx.rs`).
    pub excavation: Option<Excavation>,
}

/// A mine that digs with a beam instead of a hammer (the Naga's, `models::naga::taproot`):
/// the beam runs from its emitter down into the bore (the model's `Pit`, whose opening is
/// where the drawn beam meets the ground), converging pinch beams join it at the mouth,
/// and ore is drawn up the column to the collector. Model space, at the blueprint's size.
#[derive(Clone, Debug, PartialEq)]
pub struct Excavation {
    /// Where the beam leaves the emitter.
    pub emitter: [f32; 3],
    /// The beam's width.
    pub width: f32,
    /// The pinch emitters' tips, each firing a thinner beam at the mouth.
    pub pinches: Vec<[f32; 3]>,
    /// Seconds between the surges the deepest bore pulses with; zero for none.
    pub surge: f32,
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
mod dam;
mod dam_works;
mod desert;
mod footprint;
mod library;
mod naga;
mod precursor;
mod precursor_citadel;
mod precursor_forge;
mod precursor_gate;
mod precursor_mega;
mod precursor_polar;
mod precursor_sky;

#[cfg(test)]
mod preview;
mod props;
mod replicator;
pub mod shell;
#[cfg(test)]
mod tests;
mod thumbnail;
mod wall;

/// The Zenith's barrel anchors (muzzle, breech, points down the bore), for its effects.
pub use aster::zenith::{ZenithRail, ZENITH_RAIL};
/// Desert trees' crown radii at scale 1 (`ground_cover::crown_of`).
pub(crate) use desert::{COTTONWOOD_REACH, JUNIPER_REACH, PINYON_REACH};
pub use footprint::{
    bake_hull_plan, bake_pad_footprint, hull_plan_at, hull_plan_half, hull_plan_sd, pad_sdf_at,
    PAD_FOOTPRINT_REACH, PAD_FOOTPRINT_RES, PAD_SDF_RANGE,
};
pub use library::{
    all_model_keys, build_model, build_model_fitted, build_model_scaled, prop_model_key,
};
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
        "space_frigate" => Some(&aster::air::RESOLUTE_LAMPS),
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
        "space_frigate" => Some(aster::air::RESOLUTE_RIG.gpu()),
        _ => None,
    }
}

pub(crate) use aster::air::SpinalRail;

/// A warship's spinal rail cannon by mesh (model space): its muzzle, breech and the points
/// along the rails where the charge crawls, for the rail's charge and fire effects.
pub(crate) fn spinal_rail(mesh: &str) -> Option<&'static SpinalRail> {
    match mesh {
        "space_frigate" => Some(&aster::air::RESOLUTE_SPINAL),
        _ => None,
    }
}

/// Where a turreted rail cannon's charge crawls (renderer heavy_rail_fx.rs), in the gun's
/// frame: metres along the bore from the weapon's `pivot` (the trunnion), the two rails
/// either side of the bore with the slot open between them.
pub(crate) struct TurretRail {
    /// The breech's rear face and the muzzle face, along the bore.
    pub breech: f32,
    pub muzzle: f32,
    /// Each rail's centre line off the bore (±y), and the height of the rail tops over
    /// the bore: the arcs run along the tops and jump the slot between them.
    pub rail_y: f32,
    pub rail_top: f32,
    /// Where along the bore the arcs crawl, breech forward: the open lengths of bare rail
    /// between the clamps. And half the length of each stretch.
    pub arcs: [f32; 6],
    pub arc_half: f32,
}

/// A turreted rail cannon's rails by mesh and weapon (`TurretRail`), for its charge and
/// fire effects. The commander's weapons are numbered by its refits, and its rail cannon
/// is its only heavy rail (the only weapon this is asked about).
pub(crate) fn turret_rail(mesh: &str, weapon: usize) -> Option<&'static TurretRail> {
    match (mesh, weapon) {
        ("citadel", 0) => Some(&aster::CITADEL_RAIL),
        ("commander", _) => Some(&aster::COMMANDER_RAIL),
        ("space_frigate", 1..=4) => Some(&aster::air::RESOLUTE_TURRET_RAIL),
        _ => None,
    }
}

/// Downward lift jet mouths in model space (the Bastion's belly), for the renderer's drive effects.
pub fn lift_jets(mesh: &str) -> &'static [[f32; 3]] {
    match mesh {
        "lift_ship" => &aster::air::BASTION_LIFT_JETS,
        "light_transport" => &aster::air::COURIER_LIFT_JETS,
        "space_frigate" => &aster::air::RESOLUTE_LIFT_JETS,
        _ => &[],
    }
}

/// Jet nozzle origins in model space, shared with the aircraft effect renderer.
pub fn aircraft_exhausts(mesh: &str) -> &'static [[f32; 3]] {
    match mesh {
        "light_transport" => &aster::air::COURIER_NOZZLES,
        "space_frigate" => &aster::air::RESOLUTE_NOZZLES,
        "lift_ship" => &aster::air::BASTION_NOZZLES,
        "interceptor" => &[[-3.31, -0.2, 0.9], [-3.31, 0.2, 0.9]],
        "bomber" => &[[-2.68, -2.35, 0.95], [-2.68, 2.35, 0.95]],
        "air_scout" => &[[-2.97, 0.0, 0.65]],
        "support_air" => &aster::air::ARGUS_NOZZLES,
        "reclaim_carrier" => &aster::air::OSPREY_NOZZLES,
        "reclaim_drone" => &aster::air::DRONE_NOZZLES,
        "gunship" => &aster::air::KESTREL_NOZZLES,
        "fire_bomber" => &[
            [-4.17, -9.0, 1.6],
            [-4.17, -5.0, 1.6],
            [-4.17, 5.0, 1.6],
            [-4.17, 9.0, 1.6],
        ],
        "interceptor_t2" => &[[-5.11, -0.55, 0.9], [-5.11, 0.55, 0.9]],
        "torpedo_bomber" => &[[-1.95, -2.55, 0.62], [-1.95, 2.55, 0.62]],
        "superiority" => &[[-6.52, -0.72, 1.02], [-6.52, 0.72, 1.02]],
        "strategic_bomber" => &[[-5.87, -2.2, 1.4], [-5.87, 2.2, 1.4]],
        "assault_air" => &[[-7.37, -3.4, 3.6], [-7.37, 3.4, 3.6]],
        _ => &[],
    }
}
