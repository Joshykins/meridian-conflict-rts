//! Numbers the CPU and the shaders must agree on, written once, here.
//!
//! `build.rs` compiles this same file and prepends every constant to every shader
//! as `const <PREFIX><NAME>: <type> = <value>;` (so `pass::SHADOW` is `PASS_SHADOW`
//! in WGSL). A shader never writes one of these numbers itself, and Rust never
//! keeps a second copy (CLAUDE.md section 4).
//!
//! This file may use nothing but `core`: the build script compiles it on its own.

/// Declares a module of shared constants and its WGSL spelling.
macro_rules! shared {
    ($(
        $(#[$doc:meta])*
        pub mod $module:ident as $prefix:literal {
            $($(#[$cdoc:meta])* pub const $name:ident: $ty:ident = $value:expr;)*
        }
    )*) => {
        $(
            $(#[$doc])*
            pub mod $module {
                $($(#[$cdoc])* pub const $name: $ty = $value;)*

                /// This module's constants as WGSL declarations.
                pub fn wgsl(out: &mut String) {
                    $(
                        out.push_str(&format!(
                            "const {}{}: {} = {};\n",
                            $prefix,
                            stringify!($name),
                            stringify!($ty),
                            super::Literal::wgsl($name),
                        ));
                    )*
                }
            }
        )*

        /// Every shared constant as WGSL: the prelude build.rs puts before every shader.
        pub fn wgsl() -> String {
            let mut out = String::from("// Generated from mc-models/src/gpu_consts.rs by mc-render/build.rs. Do not edit.\n");
            $($module::wgsl(&mut out);)*
            out
        }
    };
}

/// A value written as a WGSL literal of its own type.
trait Literal {
    fn wgsl(self) -> String;
}

impl Literal for u32 {
    fn wgsl(self) -> String {
        format!("{self}u")
    }
}

impl Literal for i32 {
    fn wgsl(self) -> String {
        format!("{self}i")
    }
}

impl Literal for f32 {
    fn wgsl(self) -> String {
        // Debug keeps a decimal point ("1.0"), which WGSL needs to read a float.
        format!("{self:?}")
    }
}

shared! {
    /// What a draw is for, in the low byte of the entity and terrain push constant
    /// `pass_kind`; a shadow pass puts its cascade in the next byte.
    pub mod pass as "PASS_" {
        pub const MAIN: u32 = 0;
        pub const SHADOW: u32 = 1;
        /// A hull field: the posed mesh pushed out along its skin (shield depth, water).
        pub const HULL: u32 = 2;
        /// The depth pre-pass, above the kind byte so no kind test matches it.
        pub const PREPASS: u32 = 0x10000;
        pub const KIND_MASK: u32 = 0xff;
        pub const CASCADE_SHIFT: u32 = 8;
    }

    /// The draw lists the GPU cull builds each frame (cull.wgsl), each a full set of
    /// per-slot indirect commands: the colour pass's, the depth pre-pass's (no props
    /// too small to hide anything) and one per shadow cascade (only what can cast
    /// into it). Each pass draws only its own list, so a tree outside a cascade
    /// never runs the vertex shader there.
    pub mod cull_list as "CULL_LIST_" {
        pub const MAIN: u32 = 0;
        pub const PREPASS: u32 = 1;
        /// The nearest cascade's list; cascade `c` is `SHADOW + c`.
        pub const SHADOW: u32 = 2;
        pub const COUNT: u32 = 5;
    }

    /// Levels of detail past a model's own (`models::LOD_COUNT` of them), as draw slots
    /// after its first.
    pub mod lod as "LOD_" {
        /// A prop's far level (`models::Model::far`), when it is a few pixels across.
        pub const FAR: u32 = 3;
        /// Projected radius, in pixels, under which a prop draws its far level.
        pub const FAR_PX: f32 = 5.0;
    }

    /// Which side of the clouds a shot draw is (sprites.wgsl `push.layer`): the
    /// tracers and dots go under them up close, and over them, with the
    /// strategic icons, once they are yellow markers.
    pub mod sprite_layer as "SPRITE_LAYER_" {
        pub const UNDER_CLOUD: u32 = 0;
        pub const OVER_CLOUD: u32 = 1;
    }

    /// A giant's footprint (ground.wgsl `vs_print`, renderer `titan_fx`): drawn as a grid of
    /// this many cells a side, each vertex on the ground, so it lies over hills and hollows.
    pub mod print as "PRINT_" {
        pub const GRID: u32 = 24;
    }

    /// Strategic icons (icons.wgsl), and the click reach that matches them (mc-game pick.rs).
    pub mod icon as "ICON_" {
        /// A tier-5 titan's icon, drawn wider than anything else's: output pixels across.
        pub const TITAN_PX: f32 = 34.0;
        /// `ModelInfo::icon` bit: the model is a mobile unit.
        pub const MOBILE: u32 = 0x10000;
        /// `ModelInfo::icon` bit: the unit flies.
        pub const AIR: u32 = 0x40000;
        /// `ModelInfo::icon` bit: a ship, riding the water.
        pub const NAVAL: u32 = 0x80_0000;
        /// `ModelInfo::icon` bit: a spacecraft (`UnitBlueprint::is_capital_ship`).
        pub const CAPITAL: u32 = 0x400_0000;
        /// `ModelInfo::icon` bit: it stands on the seabed (`UnitBlueprint::seabed`) and its
        /// spire reaches up to the surface (`spire`).
        pub const SEABED: u32 = 0x1000_0000;
    }

    /// How a settled wreck lies (`mc_sim::mirror::WRECK_*`, which a test holds equal;
    /// entity.wgsl `wreck_pose`): the word in its `refit_modules`.
    pub mod wreck as "WRECK_" {
        /// The word is set: the wreck is posed by it. A spent casing has none.
        pub const POSED: u32 = 0x2000;
        /// How it came down (`mc_sim::tables::Landing`), in the low bits.
        pub const LANDING_MASK: u32 = 0xF;
        pub const LANDING_IN_PLACE: u32 = 0;
        pub const LANDING_CRASHED: u32 = 1;
        pub const LANDING_SANK: u32 = 2;
        pub const LANDING_DITCHED: u32 = 3;
        /// Which section of the hull this is, and how many it broke into (4 bits each).
        pub const SECTION_SHIFT: u32 = 4;
        pub const COUNT_SHIFT: u32 = 8;
        /// A section's second instance: the hull's inside, seen through its torn ends.
        pub const INNER: u32 = 0x1000;
    }

    /// A seabed installation's spire (`icon::SEABED`, models/aster/naval/seabed_defense.rs):
    /// authored from `BASE` up to `TOP` in model metres, it is stretched in `entity.wgsl`
    /// so `TOP` meets the water's surface however deep the installation stands. What is
    /// authored above `TOP` (the cap that rides the surface) moves up with it unstretched.
    pub mod spire as "SPIRE_" {
        pub const BASE: f32 = 9.0;
        pub const TOP: f32 = 30.0;
    }

    /// The selection mark (icons.wgsl `vs_ring`).
    pub mod ring as "RING_" {
        /// Cells along each side of a mark's ground grid: a mark is drawn as
        /// `GRID * GRID` quad instances so it drapes over the terrain.
        pub const GRID: u32 = 12;
    }

    /// Desert map scenery the entity shader dresses by a face's pattern byte
    /// (models/desert.rs, models/dam.rs, scenery.wgsl). Patterns only mean panel
    /// detail on plated materials, so leaves, bark, rock and concrete use the
    /// byte for looks of their own.
    pub mod scenery as "SCENERY_" {
        /// Leaf-card pattern: the desert leaf atlas (juniper, pinyon, cottonwood).
        pub const LEAF_DESERT: u32 = 2;
        /// The desert atlas's layer after FOLIAGE_BASE (foliage.rs).
        pub const FOLIAGE_DESERT: i32 = 7;
        /// Bark: a juniper's shaggy, stringy, silver-grey bark.
        pub const BARK_SHAGGY: u32 = 4;
        /// Rock: bedded red sandstone, its laminae level in the model.
        pub const ROCK_BEDDED: u32 = 1;
        /// Concrete: cast mass concrete, its lifts and block joints showing.
        pub const CONCRETE_CAST: u32 = 1;
        /// Concrete: the white mineral ring a drawn-down reservoir leaves.
        pub const CONCRETE_RING: u32 = 2;
        /// Concrete: dark and wet, just over the water.
        pub const CONCRETE_WET: u32 = 3;
        // retired: 4 (a road's asphalt), 5 (road paint)
        /// Concrete: a deep opening's dark mouth.
        pub const CONCRETE_SHADOW: u32 = 6;
        /// Painted steel on scenery: red and white (the dam's cranes).
        pub const CONCRETE_RED: u32 = 7;
        pub const CONCRETE_WHITE: u32 = 8;
        /// Concrete: a dry spillway chute, stained dark and rust-streaked.
        pub const CONCRETE_CHUTE: u32 = 9;
        /// A pale blue-grey painted metal roof.
        pub const CONCRETE_ROOF: u32 = 10;
    }

    /// A gun's breech door (`rig::BREECH`, `Model::breech`): swings open on its hinge as
    /// the gun kicks and shuts as it runs out (entity.wgsl `breech_open`).
    pub mod breech as "BREECH_" {
        /// The rig bit. It borrows the lowest `UPGRADE_AT` bit, which only means anything
        /// on refit pieces; a breech door never is one.
        pub const RIG: u32 = 0x10000;
    }

    /// The Trebuchet's ground stakes (`models::aster::trebuchet`, `mc_models::stakes`): a
    /// launcher tube on each corner of the carriage, authored planted, posed by
    /// `entity.wgsl` `stake_pose` from the unit's deploy. Planting, one stake after another
    /// (front left, rear right, front right, rear left) its tube swings down from lying
    /// along the fender and its spike fires out into the ground, the tube kicking back as
    /// it strikes; packing runs it all the other way. Lengths are at the authored 1.88 m
    /// deck and scale with the turret pivot. The Trebuchet and the Arbalest plant them.
    pub mod stake as "STAKE_" {
        /// Rig bits on `DEPLOY` verts: a stake's tube, and its spike. They borrow
        /// `UPGRADE_AT` bits, which only mean anything on refit pieces; a stake never is one.
        pub const RIG: u32 = 0x20000;
        pub const RIG_SPIKE: u32 = 0x40000;
        /// The hinge of each tube: x of the front and rear pair, y out from the middle, z.
        pub const FRONT_X: f32 = 3.9;
        pub const REAR_X: f32 = -4.9;
        pub const Y: f32 = 4.8;
        pub const Z: f32 = 1.9;
        /// The planted tube's direction on the front +y corner (normalised where it is
        /// used): out the way its end of the carriage faces, out to the side, and down.
        /// The others mirror it.
        pub const OUT_X: f32 = 0.2;
        pub const OUT_Y: f32 = 0.75;
        pub const DOWN: f32 = 0.63;
        /// Metres of spike that fire out of the tube.
        pub const TRAVEL: f32 = 1.9;
        /// Shares of the deploy. Stake k (in firing order) starts to swing down at
        /// `START + k * STEP`, is down `SWING` later, fires `FIRE` after it started and
        /// strikes `FIRE_TIME` after that.
        pub const START: f32 = 0.02;
        pub const STEP: f32 = 0.2;
        pub const SWING: f32 = 0.14;
        pub const FIRE: f32 = 0.15;
        pub const FIRE_TIME: f32 = 0.03;
        /// As it strikes the tube is thrown back up its line this far, and settles over
        /// this share of the deploy.
        pub const KICK: f32 = 0.35;
        pub const KICK_TIME: f32 = 0.07;
    }

    /// A VTOL's pods standing up and lying down with the hull's lean (`models::vtol_tilt`,
    /// entity.wgsl `vtol_tilt`).
    pub mod vtol as "VTOL_" {
        /// Radians of pod tilt per radian of hull pitch: the pods lean much further than
        /// the hull does.
        pub const TILT_GAIN: f32 = 5.0;
        /// The rear pods lean this share as far as the front ones.
        pub const FRONT_LEAD: f32 = 0.85;
        /// Radians of tilt, fore on the outside and aft on the inside, per radian a tick
        /// the hull turns.
        pub const YAW_GAIN: f32 = 1.4;
        /// Nearly flat, flying fast; tipped back past upright, braking hard.
        pub const TILT_MIN: f32 = 0.22;
        pub const TILT_MAX: f32 = 2.25;
    }

    /// A spacecraft's stern drives (`aster::air::capital::drive`, entity.wgsl, renderer
    /// capital_fx.rs): each nozzle (`part::DRIVE`) swivels whole on a gimbal ball inside
    /// its can as the hull turns, and its petals open wider the harder it pushes. Lengths
    /// are for a drive of size 1 (a 12 m mouth), metres forward of the mouth.
    pub mod drive as "DRIVE_" {
        /// Where the nozzle swivels: the gimbal ball's centre.
        pub const GIMBAL: f32 = 18.0;
        /// Where the petals hinge: everything aft of this opens out with thrust.
        pub const PETAL_HINGE: f32 = 13.0;
        /// Radians the nozzle swings per radian a tick the hull turns: the exhaust is
        /// thrown to the side the nose turns to, pushing the stern the other way.
        pub const VECTOR_GAIN: f32 = 8.0;
        /// The furthest it swings either way, radians.
        pub const VECTOR_MAX: f32 = 0.28;
        /// How far the petals turn out from where the model has them (radians): drawn in
        /// a little at idle, opened well out at full thrust.
        pub const FLARE_IDLE: f32 = -0.05;
        pub const FLARE_FULL: f32 = 0.2;
    }

    /// A wall section that joins its neighbours (`models::wall`): each quarter of
    /// its lot holds every piece that quarter could need, and the entity shader draws the
    /// one its neighbours call for. The neighbours are `status[2]`'s bits
    /// (`mc_sim::mirror::join_walls`): bit k is the cell k × 45° counter-clockwise from
    /// the section's own +x. Quarter q lies between side bits 2q and 2q + 2, with the
    /// corner bit 2q + 1 between them.
    pub mod wall as "WALL_" {
        /// The first piece's part: quarter q's piece `case` is `PART_FIRST + q * CASES + case`.
        pub const PART_FIRST: u32 = 26;
        pub const CASES: u32 = 5;
        /// Neither side joined: a quarter of the pillar that ends or turns a wall.
        pub const CAP: u32 = 0;
        /// Only the side at bit 2q joined: the wall running out that way.
        pub const RUN_A: u32 = 1;
        /// Only the side at bit 2q + 2 joined.
        pub const RUN_B: u32 = 2;
        /// Both sides, not the corner: the inside of a turn.
        pub const JOIN: u32 = 3;
        /// Both sides and the corner: the quarter is filled, a block of walls is one
        /// thick wall.
        pub const FULL: u32 = 4;
    }

    /// A structure's lot settling into the ground, and the foundation walls round
    /// it (terrain.rs `TileCache`, renderer/foundations.rs, bindings.wgsl
    /// `terrain_height`, foundations.wgsl).
    pub mod settle as "SETTLE_" {
        /// Lots eased toward their level at once (`Globals::settling`, two vec4 each).
        /// More at a time and the oldest snap to their level.
        pub const SLOTS: u32 = 24;
        /// Seconds the ground takes to reach its new level, and a foundation wall
        /// to rise out of it.
        pub const SECONDS: f32 = 5.0;
    }

    /// Grass round the eye (renderer/grass.rs, grass_gen.wgsl, grass.wgsl).
    pub mod grass as "GRASS_" {
        /// A candidate tuft per this many metres each way.
        pub const CELL_M: f32 = 0.28;
        /// Below this many pixels a cell grows no grass: the ground's own colour
        /// carries on from there (terrain.wgsl).
        pub const MIN_PX: f32 = 1.4;
        /// The trample map's side in metres, a texel each.
        pub const WINDOW: i32 = 512;
        /// Presses (units, props, lots, scorches, track marks) gathered at most.
        pub const MAX_PRESS: u32 = 4096;
        /// Blades per tuft and segments per blade in the near, middle and far band.
        pub const NEAR_BLADES: u32 = 24;
        pub const NEAR_SEGMENTS: u32 = 4;
        pub const MID_BLADES: u32 = 14;
        pub const MID_SEGMENTS: u32 = 3;
        pub const FAR_BLADES: u32 = 8;
        pub const FAR_SEGMENTS: u32 = 2;
        /// Tufts each band holds, laid out near, middle, far in the tuft buffer.
        pub const NEAR_CAP: u32 = 98304;
        pub const MID_CAP: u32 = 262144;
        pub const FAR_CAP: u32 = 524288;
    }

    /// Puff kinds (puffs.wgsl, renderer `push_puff`). The older kinds are still
    /// spelled out on both sides; new ones are declared here.
    pub mod puff as "PUFF_" {
        /// Burning gas off a unit or a shell going up (renderer/blast_fx.rs): an
        /// opaque, billowing ball, white-yellow inside, that cools to soot from the
        /// rim in and rises as smoke.
        pub const BLAST: u32 = 38;
        /// A shard of a flak shell's casing (renderer/flak_fx.rs): a hot metal streak
        /// as long as it is fast, flung out to the burst's splash and slowed by the air.
        pub const SHRAPNEL: u32 = 37;
        /// A flak burst's smoke (renderer/flak_fx.rs): the charge burning inside a
        /// hard-edged black ball that hangs on the wind for seconds.
        pub const FLAK: u32 = 39;
        /// An ember thrown out of a round burnt down by a missile-defence laser
        /// (renderer/laser_fx.rs): a red glow, pink-white at first, cooling to deep red.
        pub const INTERCEPT: u32 = 40;
        /// A capital ship's warp (renderer/warp_fx.rs, warp_puffs.wgsl). Each carries its
        /// colour in `appearance.rgb` (brightness in its size) and how torn a dampened jump
        /// makes it in `appearance.w` (0 clean, 1 torn).
        /// A soft glow that swells and fades: the drive charging, a jump's flash.
        pub const WARP_GLOW: u32 = 44;
        /// A streak of light from `pos` along `vel` (axis times length): its head runs out
        /// along it over the first part of its life, and the line fades behind.
        pub const WARP_STREAK: u32 = 45;
        /// The rift a jump comes out of, turned to the eye: a lens of bent light round a
        /// dark heart, its arms wound in; `vel.x` how far it is open (0 a ripple, 1 open).
        pub const WARP_RIFT: u32 = 46;
        /// A jagged arc of lightning from `pos` along `vel`, flickering.
        pub const WARP_ARC: u32 = 47;
        /// A mote of charged light carried on `vel` and stopped by the air.
        pub const WARP_MOTE: u32 = 48;
        /// A warp dampener's field edge (renderer/damper_fx.rs): a faint upright curtain
        /// standing on the ground along `vel` (its run), `size` metres tall, coloured
        /// `appearance.xyz` at strength `appearance.w`, swelling in and out over its life.
        pub const VEIL: u32 = 50;
        /// A capital ship's drive exhaust (renderer/capital_fx.rs): one point of a chain
        /// laid down the plume from the nozzle's throat. Each is a stretch of glowing tube
        /// a step either way of it (`vel`, the step), in a tent that adds up to one with
        /// its neighbours, so the chain is one seamless tube that bends where it bends.
        /// Carried with the ship (`appearance.xyz`), `appearance.w` the drive's heat (0
        /// idle to 1 flat out); `params.x` the tube's radius here, `params.y` how far down
        /// the plume it is (0 the throat, 1 the tip), `params.w` how much further down
        /// the next point is. Its brightness is a tent over its life too, so each
        /// chain laid fades out as the next fades in.
        pub const PLUME: u32 = 51;
    }

    /// Colours of a fading beam (`ProjectileInstance::color` low bits under
    /// `PROJECTILE_FADE_BEAM`; sprites.wgsl). The older ones are still spelled out in the
    /// shader; new ones are declared here.
    pub mod fade_beam as "FADE_BEAM_" {
        /// A warp dampener's tether (renderer/damper_fx.rs): crimson lightning with a
        /// white-pink core, crackling.
        pub const TETHER: u32 = 10;
    }

    /// A strategic launcher (models/aster/strategic.rs, entity.wgsl). The rounds word
    /// the sim writes in `status[2]` (`mc_sim::nukes::LAUNCHER_*`; a test holds them
    /// equal), and the plant that works a load cycle while a round is assembling: a hatch
    /// lid over the store slides open, a hoist block goes down into it and back up, the
    /// lid shuts.
    pub mod launcher as "LAUNCHER_" {
        pub const STOCK_MASK: u32 = 0xFF;
        pub const CAPACITY_SHIFT: u32 = 16;
        pub const MARK: u32 = 0x2000000;
        /// Auto-build off: it assembles only rounds queued by hand.
        pub const MANUAL: u32 = 0x4000000;
        pub const QUEUED_SHIFT: u32 = 27;
        /// The Sunfall's icon (`IconKind::Silo`); every other launcher is an array.
        pub const ICON_SILO: u32 = 25;
        /// The store's hatch lid: slides open along -x on the silo, -y on the array.
        pub const PART_LID: u32 = 23;
        /// The hoist block and the lower half of its cables: let down and hauled up.
        pub const PART_HOIST: u32 = 46;
        /// Seconds in one load cycle.
        pub const CYCLE_S: f32 = 13.0;
        pub const SILO_LID_TRAVEL: f32 = 8.6;
        pub const SILO_HOIST_DROP: f32 = 3.7;
        /// Hoist vertices above this height are the cables' tops, held at the trolley.
        pub const SILO_HOIST_SPLIT: f32 = 16.0;
        pub const ARRAY_LID_TRAVEL: f32 = 2.4;
        pub const ARRAY_HOIST_DROP: f32 = 1.2;
        pub const ARRAY_HOIST_SPLIT: f32 = 7.8;
    }

    /// A storage structure's fill gauge and status lamps (entity.wgsl `store_material`):
    /// the render mirror writes its side's store into `status[2]` (`mc_sim::store_lights`,
    /// a test holds the bits equal). A fill piece is lit while the store is at least as
    /// full as its level; a lamp shows the store's state. Without `MARK` both keep their
    /// authored look.
    pub mod store as "STORE_" {
        /// How full the store is, 0 to 255.
        pub const FILL_MASK: u32 = 0xFF;
        pub const STATE_SHIFT: u32 = 8;
        pub const STATE_MASK: u32 = 0x3;
        pub const MARK: u32 = 0x1000;
        /// States (`mc_sim::store_lights::StoreState`): lamps dark, amber, red, green.
        pub const NEUTRAL: u32 = 0;
        pub const DRAINING: u32 = 1;
        pub const EMPTY: u32 = 2;
        pub const FULL: u32 = 3;
        /// Fill pieces are parts `PART_FILL_FIRST + level`, `level` below `FILL_LEVELS`: lit
        /// while the store is at least `(level + 0.5) / FILL_LEVELS` full.
        pub const PART_FILL_FIRST: u32 = 49;
        pub const FILL_LEVELS: u32 = 16;
        /// A status lamp's lens.
        pub const PART_LAMP: u32 = 65;
    }

    /// Hatched missile cells (`models::CellBlock`, entity.wgsl): each hatch swings up and
    /// out about its outer edge by how far the hatches are open (`UnitInstance::deploy`,
    /// `mc_sim::launch_cells`); the missile standing in a cell is drawn while its bit is
    /// set in `status[2]` (`launch_cells::loaded_cells`). The layout is the model's own
    /// (`ModelInfo::cells`).
    pub mod cells as "CELLS_" {
        /// A hatch, and its hinge knuckles.
        pub const PART_HATCH: u32 = 47;
        /// The missile standing in a cell.
        pub const PART_ROUND: u32 = 48;
        /// How far a hatch swings open, radians: up, over and a little past upright.
        pub const SWING: f32 = 1.85;
    }

    /// Work-beam kinds (beams.wgsl) that the sim writes into `BeamInstance::kind`
    /// (`mc_sim::reclaim`). The older kinds are still spelled out in the shader.
    pub mod beam as "BEAM_" {
        /// A Regency builder's nanite stream, emitter to weld (`reclaim::BEAM_NANITE`).
        pub const NANITE: u32 = 1;
        /// A Regency site being fed: its rings and rising filaments (`reclaim::BEAM_NANITE_SITE`).
        pub const NANITE_SITE: u32 = 6;
        /// A scavenger tower's head searching: a dimmer reclaim beam on the ground, no
        /// bits (`reclaim::BEAM_SWEEP`).
        pub const SWEEP: u32 = 7;
        /// How bright a sweep is beside the beam that bites: a little dimmer.
        pub const SWEEP_LEVEL: f32 = 0.6;
        /// Quads drawn per beam (renderer `work_beams`, beams.wgsl `vs_beam`). A reclaim or
        /// repair beam uses as many of them for its bits as its length asks for.
        pub const QUADS: u32 = 64;
        /// Quads the replication and nanite kinds use; the rest are hidden for them.
        pub const FIXED_QUADS: u32 = 32;
    }

    /// Materials (the sim's mass) as a light: the HUD's Materials red-orange
    /// (`hud::MASS`, 0xFF6B3D) in linear RGB. Reclaim's beams, the stream down a reclaim
    /// tower's chutes and anything else that shows mass on the move share it.
    pub mod mass as "MASS_" {
        pub const R: f32 = 1.0;
        pub const G: f32 = 0.147;
        pub const B: f32 = 0.0467;
        /// `pattern::MASS_FLOW`: a chute with material falling down it while the unit reclaims.
        pub const FLOW_PATTERN: u32 = 30;
        /// How fast the clumps fall down a chute, metres a second.
        pub const FLOW_SPEED: f32 = 7.0;
        /// `material::GLOW_MATERIALS`: a reclaim emitter, lit in this colour while the unit
        /// reclaims (`unit_flag::RECLAIMING`) and banked low while it does not.
        pub const GLOW_MATERIAL: u32 = 27;
    }

    /// Bits of a selection mark's `kind` (renderer `Mark`, icons.wgsl `fs_ring`).
    pub mod mark as "MARK_" {
        /// Hovered; without it the mark is a selection.
        pub const HOVER: u32 = 1;
        /// Hostile: the ground brackets go red.
        pub const ENEMY: u32 = 2;
        /// No selection ring, only the status bars (work under way, seen unselected).
        pub const BARS_ONLY: u32 = 4;
        /// What a click would take apart: ringed in the Materials red-orange (`MASS_*`).
        pub const RECLAIM: u32 = 8;
    }

    /// Effect kinds (sprites.wgsl `Effect::params.z`, lights.rs `effect`). The older kinds
    /// are still spelled out on both sides; new ones are declared here.
    pub mod effect as "EFFECT_" {
        /// The last of a hull going up a reclaim beam: a flare in the Materials red-orange.
        pub const MATERIALS: u32 = 10;
    }

    /// Heat haze over engine exhausts (renderer/heat_haze.rs, screen.wgsl `haze_bend`).
    pub mod haze as "HAZE_" {
        /// Plumes handed to the tone map at most: the nearest to the eye.
        pub const MAX_PLUMES: u32 = 32;
        /// Past this many metres from the eye a plume is not looked at.
        pub const REACH_M: f32 = 700.0;
        /// The most a plume shifts the scene, output pixels.
        pub const MAX_PX: f32 = 4.0;
    }

    /// A twin gun on a walker's arm that kicks on its own shots (`mc_sim::mirror::UNIT_TWIN_*`
    /// in `UnitInstance::status[1]`; a test holds them equal): one more than its weapon, read
    /// from the unit's `HousePose`, and whether it sits on the right (-y).
    pub mod arm_twin as "ARM_TWIN_" {
        pub const SHIFT: u32 = 2;
        pub const MASK: u32 = 0x7;
        pub const RIGHT: u32 = 0x20;
    }

    /// A capital ship's jump in `UnitInstance::status[0]` (`mc_sim::mirror::UNIT_WARP_DAMPED`
    /// and `UNIT_IN_WARP`; a test holds them equal). `UnitInstance::fx` carries the warp
    /// stretch and the EMP stun.
    pub mod warp_status as "WARP_STATUS_" {
        /// The jump is snagged by a live enemy dampener: torn and slow.
        pub const DAMPED: u32 = 0x1000;
        /// In warp: listed for its own side, never drawn.
        pub const IN_WARP: u32 = 0x2000;
    }

    /// A salvage drone docked on an aircraft (`mc_sim::mirror::UNIT_RIDING`, a test holds
    /// them equal): drawn in the frame its carrier is drawn in (entity.wgsl `riding_frame`).
    pub mod dock as "DOCK_" {
        /// `UnitInstance::status[0]`: riding; `status[2]` is one more than the carrier's
        /// index among the frame's units.
        pub const RIDING: u32 = 0x400;
    }

    /// Bits of `UnitInstance::owner_flags` (`owner | flags << 8`) the shaders read that the
    /// older hand-written `FLAG_*` list in common.wgsl does not hold.
    pub mod unit_flag as "UNIT_FLAG_" {
        /// `mc_sim::tables::flag::RECLAIMING << 8`: pulling mass out of a wreck this tick.
        pub const RECLAIMING: u32 = 0x10000;
    }
}
