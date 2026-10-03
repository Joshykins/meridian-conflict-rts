//! Procedural models. There are no authored art assets yet: every unit,
//! structure and prop is generated from code at start-up, with three levels of
//! detail. Below the last level the renderer switches to strategic icons.
//!
//! This crate is only the models, apart from the renderer, so an edit to a model
//! recompiles nothing but models, and a small program (`mc-models`) can build
//! meshes without linking the renderer. mc-render re-exports it as `models`.

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
    /// ARC blue-white), so a generator reads as the source of its field. Emissive.
    pub const GLOW_SHIELD: u32 = 24;
    /// A Precursor light channel lying dormant: dark glass let into the alloy, a cold
    /// sheen on it, stirring faintly as a survival facility wakes. Most of a Precursor
    /// structure's channels are this; only its working parts carry `GLOW_PRECURSOR`.
    pub const PRECURSOR_INLAY: u32 = 25;
    /// A helmet visor: mirrored gold-orange glass with a faint warm light behind it,
    /// so it reads as the commander's face from strategic zoom.
    pub const VISOR: u32 = 26;
    /// Reclaim emitters: the Materials red-orange (`gpu_consts::mass`), burning steady
    /// while the unit reclaims and banked low while it does not. Emissive. Every
    /// reclaimer's emitter tips use it, so reclaim never reads as construction amber.
    pub const GLOW_MATERIALS: u32 = crate::gpu_consts::mass::GLOW_MATERIAL;
    /// Pinch fusion's light (`gpu_consts::prism`): a white-hot face with a soft pastel
    /// prism turning round its rim, rose, magenta, lavender and peach-gold. A Regency star
    /// core. Emissive.
    pub const GLOW_PRISM: u32 = crate::gpu_consts::prism::GLOW_MATERIAL;
    pub const LAST: u32 = GLOW_PRISM;
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
    /// A charge coil's light, stage 0 at the breech to 7 at the muzzle: `COIL + stage`.
    /// The shader breathes it idle, climbs it stage by stage through the weapon's charge,
    /// blinds at the shot and lets it cool (the renderer feeds the charge through
    /// `UnitInstance::mount`, `renderer/titan_charge.rs`). On `GLOW` faces the Behemoth's
    /// AEB-3 blue; on `GLOW_LASER` faces the Regency's red-white plasma coil (the
    /// Sunspear's), where stage `gpu_consts::charge_gear::HEAT_STAGE` is a vent's heat.
    pub const COIL: u32 = 19;
    pub const COIL_STAGES: u32 = 8;
    /// A capacitor ring's lugs on such a weapon (`METAL` faces only): turned about the
    /// bore's axis (the model's rotary axis mirrored across the centreline) slowly at rest
    /// and hard as it charges; `COIL_TURN_BACK` the other way.
    pub const COIL_TURN: u32 = COIL + COIL_STAGES;
    pub const COIL_TURN_BACK: u32 = COIL_TURN + 1;
    /// Dark plating (`ACCENT`) whose little level lights are the Regency's red, and lit at every
    /// tier: a Regency hide's seams of light. (Aster's black carries no lit lines at all.)
    pub const EMBER: u32 = COIL_TURN_BACK + 1;
    /// A chute carrying reclaimed material (`ACCENT` faces): dark glazing over the channel,
    /// and while the unit reclaims a stream of glowing clumps falling down it, in model z,
    /// so a spiral or a raked run carries the same fall (`gpu_consts::mass`).
    pub const MASS_FLOW: u32 = crate::gpu_consts::mass::FLOW_PATTERN;
    /// Rock melted by a beam (`ACCENT` faces): a dark glassy crust broken by glowing cracks
    /// over a melt that runs down, hotter with depth below model z = 0 and with the tier
    /// (`gpu_consts::melt`). A Regency mine's bore.
    pub const MOLTEN: u32 = crate::gpu_consts::melt::PATTERN;
    /// A fusion plant's star (`GLOW` faces): white-hot, blue at its limb, threads of
    /// plasma streaming over it, breathing (`gpu_consts::reactor`).
    pub const CORE: u32 = crate::gpu_consts::reactor::PATTERN_CORE;
    /// A band of charge (`GLOW` faces): pulses running round the model's z axis, where the
    /// core stands (`gpu_consts::reactor`).
    pub const CHARGE: u32 = crate::gpu_consts::reactor::PATTERN_CHARGE;
    /// A heat sink's hot core (`GLOW_ORANGE` faces): its heat rippling along it in waves
    /// (`gpu_consts::reactor`).
    pub const HEAT: u32 = crate::gpu_consts::reactor::PATTERN_HEAT;
    /// A window onto a fusion plant's plasma (`GLOW` faces): blue streaming round the
    /// model's z axis, bright threads in it (`gpu_consts::reactor`).
    pub const FUSION: u32 = crate::gpu_consts::reactor::PATTERN_FUSION;
    /// A capital warship's armour, on plated faces of any shade: strakes of uneven depth
    /// cut into plates of uneven length, the odd hatch, grille or stencil, rows of lit
    /// orange ports along the walls, running lamps at the seams (`gpu_consts::warship`).
    pub const WARSHIP: u32 = crate::gpu_consts::warship::PATTERN;
    /// A fabricator's matter (`GLOW_MATERIALS` faces): lit by how hard it works, flashing at
    /// each stroke, sputtering when short of energy, dark at rest (`gpu_consts::fab`).
    pub const FAB_MATTER: u32 = crate::gpu_consts::fab::PATTERN_MATTER;
    /// A fabricator's status lamp (`GLOW_MATERIALS` faces): steady at work, blinking amber
    /// when short of energy, a slow standby glow at rest (`gpu_consts::fab`).
    pub const FAB_LAMP: u32 = crate::gpu_consts::fab::PATTERN_LAMP;
    pub const LAST: u32 = FAB_LAMP;
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
    /// (`Model::vtol`) between hover and cruise by how the aircraft flies; a
    /// `rig::SPIN` fan or turbine in one turns about the pod's own axis.
    pub const VTOL_FRONT: u32 = 5;
    pub const VTOL_REAR: u32 = 6;
    /// A transport's hold doors, opened by `UnitInstance::deploy` (the Courier's plug
    /// doors slide into its shoulders).
    pub const HOLD_DOOR: u32 = 7;
    // retired: 8 (the Osprey's drone cradles)
    /// A core mine's pile driver: authored resting on the pipe string, hauled up and
    /// dropped along z on the mine's beat (`UnitInstance::gait`) by `Pit::stroke`.
    pub const RAM: u32 = 9;
    /// A core mine's pipe string down the bore: driven down one `Pit::section` with each
    /// blow. It repeats every section, so it seems to go on down for ever.
    pub const STRING: u32 = 10;
    /// A core mine's next pipe section: authored waiting at `Pit::rack`, it rises out of
    /// the magazine there, swings over the bore onto the string, and is driven down with it.
    pub const FEED: u32 = 11;
    /// Drawn only where the structure stands in water: an emplacement's floats.
    pub const AFLOAT: u32 = 12;
    // retired: 13 (a core mine's pit, drawn only on land)
    // retired: 14 (a reactor's pump, riding up and down)
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
    /// A spacecraft drive's nozzle: it swivels on its gimbal as the hull turns and opens
    /// out with thrust (`capital_rig`, `gpu_consts::drive`).
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
    /// A cell launcher's hatches: swung open about their outer edges before a salvo
    /// (`gpu_consts::cells`, `UnitInstance::deploy`).
    pub const CELL_HATCH: u32 = crate::gpu_consts::cells::PART_HATCH;
    /// The missile standing in a cell: drawn while that cell is loaded (`gpu_consts::cells`).
    pub const CELL_ROUND: u32 = crate::gpu_consts::cells::PART_ROUND;
    const _: () = assert!(CELL_HATCH > LAUNCHER_HOIST && CELL_ROUND > CELL_HATCH);
    /// Tread / leg surfaces: the shader scrolls or bobs these with distance travelled.
    pub const LOCOMOTION: u32 = 3;
    /// A storage structure's fill piece at `level` (`gpu_consts::store`), and its lamps.
    pub const STORE_FILL_FIRST: u32 = crate::gpu_consts::store::PART_FILL_FIRST;
    pub const STORE_LAMP: u32 = crate::gpu_consts::store::PART_LAMP;
    /// A gyroscope's piece: turns about its own axis through `Model::spinner_pivot`, the
    /// axis and rate in the word's high bits (`gpu_consts::orbit`,
    /// `MeshBuilder::with_orbit`). Compare `word & ORBIT_MASK`.
    pub const ORBIT: u32 = crate::gpu_consts::orbit::PART;
    pub const ORBIT_MASK: u32 = crate::gpu_consts::orbit::PART_MASK;
    const _: () = assert!(ORBIT > STORE_LAMP);
    /// A reactor's collars round its z axis: collar `k` is `REACTOR_COLLAR_FIRST + k`,
    /// turned about the axis while the plant runs (`gpu_consts::reactor`).
    pub const REACTOR_COLLAR_FIRST: u32 = crate::gpu_consts::reactor::PART_COLLAR_FIRST;
    const _: () = assert!(REACTOR_COLLAR_FIRST > ORBIT);
    /// A reactor's heat sink fins, lifting in a wave while the plant runs
    /// (`gpu_consts::reactor`).
    pub const REACTOR_FIN: u32 = crate::gpu_consts::reactor::PART_FIN;
    const _: () =
        assert!(REACTOR_FIN >= REACTOR_COLLAR_FIRST + crate::gpu_consts::reactor::COLLARS);
    /// A leg into the sea, drawn only afloat like `AFLOAT`: what is authored at or below
    /// z = 0 stands on the seabed under it (`gpu_consts::pile`).
    pub const PILE: u32 = crate::gpu_consts::pile::PART;
    const _: () = assert!(PILE > REACTOR_FIN);
    /// A fabricator's indexer and press (`gpu_consts::fab`): turned a step about the model's
    /// z axis, and let down and raised, each beat it works.
    pub const FAB_INDEX: u32 = crate::gpu_consts::fab::PART_INDEX;
    pub const FAB_PRESS: u32 = crate::gpu_consts::fab::PART_PRESS;
    const _: () = assert!(FAB_INDEX > PILE && FAB_PRESS > FAB_INDEX);

    /// Drawn only where the structure stands in water (`AFLOAT` or `PILE`).
    pub(crate) fn afloat_only(part: u32) -> bool {
        part == AFLOAT || part == PILE
    }
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
    /// A charge gun's working gear (`Model::charge_gear`, `gpu_consts::charge_gear`): which
    /// motion the vertex makes with the charge and the heat after the shot.
    pub const CHARGE_GEAR_SHIFT: u32 = crate::gpu_consts::charge_gear::SHIFT;
    pub const CHARGE_GEAR_MASK: u32 = crate::gpu_consts::charge_gear::MASK << CHARGE_GEAR_SHIFT;
    /// Hover skirt: the shader drops it on water and tucks it up on land.
    pub const FLOAT: u32 = 1 << 5;
    /// Factory build deck: up while a unit is printing, then lowers to release it.
    pub const LIFT: u32 = 1 << 6;
    /// Siege gear planted when the unit deploys: the Trebuchet's and the Arbalest's
    /// ground stakes (`STAKE`, `STAKE_SPIKE`).
    pub const DEPLOY: u32 = 1 << 7;
    /// On `DEPLOY` verts: a ground stake's launcher tube, and the spike it fires
    /// (`gpu_consts::stake`).
    pub const STAKE: u32 = crate::gpu_consts::stake::RIG;
    pub const STAKE_SPIKE: u32 = crate::gpu_consts::stake::RIG_SPIKE;
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
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    bytemuck::Pod,
    bytemuck::Zeroable,
    serde::Serialize,
    serde::Deserialize,
)]
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
    /// marks x as going right round a tube: no outline there. All zero: no frame. A
    /// negative w marks the edge form instead (Regency faces no rectangle fits): metres
    /// to up to four of the face's own edges, the fourth as `-(d + face_edges::BIAS)`.
    pub face: [f32; 4],
    /// [`pattern`] in the low byte, then a byte of per-face randomness.
    pub surface: u32,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MeshLod {
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<u32>,
}

pub const LOD_COUNT: usize = 3;

/// Where a tracked model touches the ground, in model space (metres).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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
    /// How a heavy walker's hull rides its stride (`MeshBuilder::set_walk_sway`): the roll
    /// up over the planted leg (radians), the nose's dip as each foot comes down (radians)
    /// and how far the hull settles onto its knees with it (metres). Zero rides level.
    pub sway: [f32; 3],
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

/// A many-legged walker (the Regency commander): each left leg's joints at rest, the right
/// one its mirror. A leg's bones swing in the vertical plane through its hip and foot, and
/// that plane turns about the hip to follow the foot through its stride.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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
    /// The pair that is one leg on the centreline, not two (`MeshBuilder::set_lone_leg`):
    /// both halves of it walk in step, at its own phase (a tripod's third leg).
    #[serde(default)]
    pub lone: Option<usize>,
}

impl Crawl {
    /// As `entity.wgsl` reads `ModelInfo::crawl`: [0] pair count, tail root z, tail top z,
    /// tail joint count; then per pair hip (w: phase), knee (w: 1 for a lone leg), ankle; then the tail's joints,
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
            let lone = if self.lone == Some(i) { 1.0 } else { 0.0 };
            out[2 + 3 * i] = [k[0], k[1], k[2], lone];
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

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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
    /// A structure's footing dirt reaches 12% of its height, or this if lower.
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
    /// A charge gun's working gear (`rig::CHARGE_GEAR_MASK`, `gpu_consts::charge_gear`): the
    /// hub its `SPIN` gear turns about (xyz, rest pose) and the scale of its travels (w).
    pub charge_gear: Option<[f32; 4]>,
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
    /// Blocks of hatched missile cells (`MeshBuilder::cell_block`), at most two.
    pub cells: Vec<CellBlock>,
    /// Axes of `rig::SPIN` barrels (a point on the axis, which runs along x), with the module
    /// tags (`rig::MODULE`, `rig::UNTIL` values) under which each applies.
    pub spins: Vec<(u32, u32, [f32; 3])>,
    /// A hole the model digs into the ground, and the pipe it drives down it.
    pub pit: Option<Pit>,
    /// The beam a Regency mine digs its bore with (`renderer/regency_mine_fx.rs`).
    pub excavation: Option<Excavation>,
    /// A Regency power generator's star (xyz its middle, w its radius): drawn as light by the
    /// renderer (`renderer/star_core_fx.rs`) while the plant runs.
    pub star_core: Option<[f32; 4]>,
    /// Engine exhaust ports whose hot air shimmers above them (`MeshBuilder::add_exhaust`,
    /// renderer `heat_haze.rs`).
    pub exhausts: Vec<Exhaust>,
    /// Plasma lift bells that cast red plasma under the craft (`MeshBuilder::add_lift`,
    /// renderer `lift_fx.rs`).
    pub lifts: Vec<Lift>,
    /// The core a reactor holds and the electrodes round it that arcs strike from it
    /// (`MeshBuilder::set_discharge`, renderer `reactor_fx.rs`).
    pub discharge: Option<Discharge>,
    /// A VTOL's tilting engine pods (`part::VTOL_FRONT`, `VTOL_REAR`).
    pub vtol: Option<Vtol>,
}

/// A VTOL's tilting engine pods, one or two a side (`MeshBuilder::set_vtol`). Each is
/// authored lying along +x, nozzle aft; the entity shader tilts it about its pivot by how
/// the aircraft leans (`vtol_tilt`), and the renderer's engine plumes leave its nozzle the
/// same way. Model space, as authored.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Vtol {
    /// Pivots of the front (`part::VTOL_FRONT`) and rear (`VTOL_REAR`) pod on the left
    /// (+y) side; the right is the mirror. With one pair, only the first.
    pub pivots: [[f32; 3]; 2],
    /// Pods a side: 1 or 2.
    pub pairs: u8,
    /// How far behind its pivot a pod's nozzle mouth is, along the pod, and its radius.
    pub nozzle: [f32; 2],
    /// Ducted lift fans (a lift field), not jets (a drive plume).
    pub fans: bool,
}

impl Vtol {
    /// For `ModelInfo::vtol`: the pivots, w 1 for jets or 2 for fans on the first, the
    /// nozzle's distance behind on the second.
    pub fn gpu(&self) -> [[f32; 4]; 2] {
        let [f, r] = self.pivots;
        [
            [f[0], f[1], f[2], if self.fans { 2.0 } else { 1.0 }],
            [r[0], r[1], r[2], self.nozzle[0]],
        ]
    }

    /// The pods a side, front first: each one's pivot and whether it is the front one.
    pub fn pods(&self) -> impl Iterator<Item = ([f32; 3], bool)> + '_ {
        self.pivots
            .iter()
            .take(self.pairs.clamp(1, 2) as usize)
            .enumerate()
            .map(|(i, p)| (*p, i == 0))
    }
}

/// How far a VTOL pod stands up from lying along the hull (radians; pi/2 points the nozzle
/// straight down), for a hull pitched `pitch` (negative: nose down) that is turning at
/// `turn` radians a tick, on the `left` side or not, `front` pod or rear. The pods lean
/// forward to drive the aircraft on, back past upright to brake it, and those on the
/// outside of a turn lean forward while the inside ones lean back. `entity.wgsl` has the
/// same function over the same constants.
pub fn vtol_tilt(pitch: f32, turn: f32, left: bool, front: bool) -> f32 {
    use gpu_consts::vtol::*;
    let side = if left { 1.0 } else { -1.0 };
    let lead = if front { 1.0 } else { FRONT_LEAD };
    (std::f32::consts::FRAC_PI_2 + pitch * TILT_GAIN * lead + turn * side * YAW_GAIN)
        .clamp(TILT_MIN, TILT_MAX)
}

/// An engine exhaust port: the hot air rising off it bends the scene behind it (renderer
/// `heat_haze.rs`). Model space, at the blueprint's size.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Exhaust {
    /// The middle of the port's mouth.
    pub at: [f32; 3],
    /// Which way the gas leaves it (unit length).
    pub toward: [f32; 3],
    /// The mouth's radius.
    pub radius: f32,
}

/// A plasma lift bell under a hovercraft: red plasma crackles from its mouth to the ground
/// and trails behind as the craft moves (renderer `lift_fx.rs`). Model space, at the
/// blueprint's size.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Lift {
    /// The middle of the bell's mouth.
    pub at: [f32; 3],
    /// The mouth's radius.
    pub radius: f32,
}

/// A reactor's held charge: arcs crackle from the core's skin to the electrode tips round
/// it, and across the gaps between its rings, while the plant runs (renderer
/// `reactor_fx.rs`). Model space, at the blueprint's size.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Discharge {
    /// The core's middle.
    pub core: [f32; 3],
    /// The core's radius.
    pub radius: f32,
    /// The electrode tips.
    pub terminals: Vec<[f32; 3]>,
    /// Gaps an arc jumps across, one end to the other: between two rings round the core.
    pub bridges: Vec<([f32; 3], [f32; 3])>,
}

/// A mine that digs with a beam instead of a hammer (the Regency's, `models::regency::taproot`):
/// the beam runs from its emitter down into the bore (the model's `Pit`, whose opening is
/// where the drawn beam meets the ground), converging pinch beams join it at the mouth,
/// and ore is drawn up the column to the collector. Model space, at the blueprint's size.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct House {
    pub pivot: [f32; 3],
    pub travel: f32,
    pub weapon: u8,
}

/// A block of hatched missile cells (`part::CELL_HATCH`, `part::CELL_ROUND`): `nx` cells
/// along x by `ny` along y, `pitch` apart about `centre`, their hatches lying shut on
/// `deck`. Each hatch is hinged on its outer edge (away from the block's middle) along x,
/// or along y when `hinge_y`, `half` out from its cell's centre, and swings up and out as
/// the hatches open (`UnitInstance::deploy`). The missile standing in grid cell `i * ny + j`
/// is shown while bit `first + order[i * ny + j]` of `status[2]` is set: the cells' missiles
/// in firing order are the weapon's muzzles `first..` (`mc_sim::launch_cells`).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CellBlock {
    pub centre: [f32; 2],
    pub deck: f32,
    pub pitch: f32,
    pub half: f32,
    pub nx: u8,
    pub ny: u8,
    pub hinge_y: bool,
    pub first: u8,
    pub order: [u8; CellBlock::MAX_CELLS],
}

impl CellBlock {
    /// Cells in one block (a nibble each in the shader's order word).
    pub const MAX_CELLS: usize = 8;
    /// Blocks on one model.
    pub const MAX_BLOCKS: usize = 2;

    /// Centre of grid cell (`i`, `j`).
    pub fn grid_centre(&self, i: usize, j: usize) -> [f32; 2] {
        let at = |k: usize, n: u8, c: f32| c + (k as f32 - (n as f32 - 1.0) * 0.5) * self.pitch;
        [
            at(i, self.nx, self.centre[0]),
            at(j, self.ny, self.centre[1]),
        ]
    }

    /// Centre of the cell holding the block's `k`-th missile in firing order.
    pub fn missile_centre(&self, k: usize) -> Option<[f32; 2]> {
        let cells = self.nx as usize * self.ny as usize;
        let c = (0..cells).find(|&c| self.order[c] as usize == k)?;
        Some(self.grid_centre(c / self.ny as usize, c % self.ny as usize))
    }

    /// `ModelInfo::cells` and `cell_grid` for up to two blocks: per block its centre,
    /// deck and pitch, then its hatch half-width; its grid word (`nx | ny << 4 | hinge_y
    /// << 8`) and its order word (the missile bit of grid cell c in nibble c).
    pub fn gpu(blocks: &[CellBlock]) -> ([[f32; 4]; 4], [u32; 4]) {
        let mut cells = [[0.0; 4]; 4];
        let mut grid = [0; 4];
        for (k, b) in blocks.iter().take(Self::MAX_BLOCKS).enumerate() {
            cells[2 * k] = [b.centre[0], b.centre[1], b.deck, b.pitch];
            cells[2 * k + 1] = [b.half, 0.0, 0.0, 0.0];
            grid[k] = b.nx as u32 | (b.ny as u32) << 4 | (b.hinge_y as u32) << 8;
            grid[2 + k] = (0..b.nx as usize * b.ny as usize)
                .map(|c| ((b.first + b.order[c]) as u32 & 0xF) << (4 * c))
                .fold(0, |w, n| w | n);
        }
        (cells, grid)
    }
}

/// A hole a model digs into the ground (a core mine's). The vertex shader pulls what is
/// inside and below the opening up in depth so the terrain does not hide it, drives the
/// `part::RAM`, `STRING` and `FEED` pieces on the mine's beat.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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
}

mod aster;
pub mod builder;
pub mod burns;
pub mod cliffs;
mod dam;
mod dam_works;
mod desert;
pub mod foliage;
mod footprint;
pub mod gpu_consts;
mod library;
mod precursor;
mod precursor_citadel;
mod precursor_forge;
mod precursor_gate;
mod precursor_mega;
mod precursor_polar;
mod precursor_sky;
mod precursor_tower;
mod regency;

#[cfg(test)]
mod preview;
mod props;
pub mod remote;
mod replicator;
pub mod shell;
pub mod site;
pub mod stakes;
#[cfg(test)]
mod tests;
mod thumbnail;
mod wall;

/// The Zenith's barrel anchors (muzzle, breech, points down the bore), for its effects.
pub use aster::zenith::{ZenithRail, ZENITH_RAIL};
/// Desert trees' crown radii at scale 1 (`ground_cover::crown_of`).
pub use desert::{COTTONWOOD_REACH, JUNIPER_REACH, PINYON_REACH};
pub use footprint::hull_plan_box;
pub use footprint::{
    bake_hull_plan, bake_pad_footprint, hull_plan_at, hull_plan_half, hull_plan_sd, pad_sdf_at,
    PAD_FOOTPRINT_REACH, PAD_FOOTPRINT_RES, PAD_SDF_RANGE,
};
pub use library::authored_size;
pub use library::{
    all_model_keys, build_model, build_model_fitted, build_model_scaled, prop_model_key,
};
pub use thumbnail::{material_color, thumbnail, thumbnail_of};

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

/// The design a mesh key is a variant of (`<mesh>~<name>`, a design round's extra keys,
/// share their design's rig and effect anchors).
fn design_of(mesh: &str) -> &str {
    mesh.split('~').next().unwrap_or(mesh)
}

/// The Bastion's lamps sit in fittings the model builds from the same numbers.
const BASTION_LAMPS: CapitalLamps = aster::air::BASTION_LAMPS;

/// A capital ship's lamp fittings by mesh; `None` for a hull without any.
pub fn capital_lamps(mesh: &str) -> Option<&'static CapitalLamps> {
    match design_of(mesh) {
        "space_dreadnought" => Some(&aster::air::DOMINION_LAMPS),
        "lift_ship" => Some(&BASTION_LAMPS),
        "light_transport" => Some(&aster::air::COURIER_LAMPS),
        "space_frigate" => Some(&aster::air::RESOLUTE_LAMPS),
        "sensor_ship" => Some(&aster::air::VIGIL_LAMPS),
        "rail_corvette" => Some(&aster::air::VALIANT_LAMPS),
        _ => None,
    }
}

/// A spacecraft's rig as `entity.wgsl` reads it (`ModelInfo::capital`, laid out by
/// `aster::air::capital::CapitalRig::gpu`): its landing legs and bay doors, drives, lift
/// jets and ramp. `None` for everything else. A new spacecraft adds its `CapitalRig` here.
pub fn capital_rig(mesh: &str) -> Option<[[f32; 4]; 7]> {
    match design_of(mesh) {
        "space_dreadnought" => Some(aster::air::DOMINION_RIG.gpu()),
        "lift_ship" => Some(aster::air::BASTION_RIG.gpu()),
        "light_transport" => Some(aster::air::COURIER_RIG.gpu()),
        "space_frigate" => Some(aster::air::RESOLUTE_RIG.gpu()),
        "sensor_ship" => Some(aster::air::VIGIL_RIG.gpu()),
        "rail_corvette" => Some(aster::air::VALIANT_RIG.gpu()),
        _ => None,
    }
}

pub use aster::air::SpinalRail;

/// A warship's spinal rail cannon by mesh (model space): its muzzle, breech and the points
/// along the rails where the charge crawls, for the rail's charge and fire effects.
pub fn spinal_rail(mesh: &str) -> Option<&'static SpinalRail> {
    match mesh {
        "space_frigate" => Some(&aster::air::RESOLUTE_SPINAL),
        _ => None,
    }
}

/// Where a turreted rail cannon's charge crawls (renderer heavy_rail_fx.rs), in the gun's
/// frame: metres along the bore from the weapon's `pivot` (the trunnion), the two rails
/// either side of the bore with the slot open between them.
pub struct TurretRail {
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
pub fn turret_rail(mesh: &str, weapon: usize) -> Option<&'static TurretRail> {
    match (mesh, weapon) {
        ("citadel", 0) => Some(&aster::CITADEL_RAIL),
        ("commander", _) => Some(&aster::COMMANDER_RAIL),
        ("space_frigate", 1..=4) => Some(&aster::air::RESOLUTE_TURRET_RAIL),
        ("rail_corvette", 0) => Some(&aster::air::VALIANT_RAIL_CHARGE),
        ("rail_trimaran", 0) => Some(&aster::NARWHAL_RAIL),
        ("submarine_titan", 3..=4) => Some(&aster::MEGALODON_RAIL),
        _ => None,
    }
}

/// Downward lift jet mouths in model space (the Bastion's belly), for the renderer's drive effects.
pub fn lift_jets(mesh: &str) -> &'static [[f32; 3]] {
    match design_of(mesh) {
        "space_dreadnought" => &aster::air::DOMINION_LIFT_JETS,
        "lift_ship" => &aster::air::BASTION_LIFT_JETS,
        "light_transport" => &aster::air::COURIER_LIFT_JETS,
        "space_frigate" => &aster::air::RESOLUTE_LIFT_JETS,
        "sensor_ship" => &aster::air::VIGIL_LIFT_JETS,
        "rail_corvette" => &aster::air::VALIANT_LIFT_JETS,
        _ => &[],
    }
}

/// Jet nozzle origins in model space, shared with the aircraft effect renderer.
pub fn aircraft_exhausts(mesh: &str) -> &'static [[f32; 3]] {
    match design_of(mesh) {
        "space_dreadnought" => &aster::air::DOMINION_NOZZLES,
        "light_transport" => &aster::air::COURIER_NOZZLES,
        "space_frigate" => &aster::air::RESOLUTE_NOZZLES,
        "sensor_ship" => &aster::air::VIGIL_NOZZLES,
        "rail_corvette" => &aster::air::VALIANT_NOZZLES,
        "lift_ship" => &aster::air::BASTION_NOZZLES,
        "interceptor" => &[[-3.31, -0.2, 0.9], [-3.31, 0.2, 0.9]],
        "bomber" => &[[-3.0, -2.2, 1.0], [-3.0, 2.2, 1.0]],
        "air_scout" => &[[-2.97, 0.0, 0.65]],
        "support_air" => &aster::air::ARGUS_NOZZLES,
        "reclaim_drone" => &aster::air::DRONE_NOZZLES,
        "fire_bomber" => &[
            [-4.17, -9.0, 1.6],
            [-4.17, -5.0, 1.6],
            [-4.17, 5.0, 1.6],
            [-4.17, 9.0, 1.6],
        ],
        "interceptor_t2" => &[[-5.11, -0.55, 0.9], [-5.11, 0.55, 0.9]],
        "torpedo_bomber" => &[[-1.95, -2.55, 0.62], [-1.95, 2.55, 0.62]],
        "superiority" => &[[-6.52, -0.72, 1.02], [-6.52, 0.72, 1.02]],
        "strategic_bomber" => &aster::air::ECLIPSE_NOZZLES,
        "assault_air" => &[[-7.37, -3.4, 3.6], [-7.37, 3.4, 3.6]],
        _ => &[],
    }
}
