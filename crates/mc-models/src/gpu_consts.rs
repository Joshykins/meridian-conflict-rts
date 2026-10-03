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
    /// A player slot as the GPU sees it: the low bits of `UnitInstance::owner_flags`, of a
    /// structure pad's word (`pad`), a shield's `packed` and a strategic missile's word, and
    /// the index into `Globals::team_colors`. Slots run `0..mc_core::MAX_PLAYERS`, which must
    /// fit (a test holds it).
    pub mod owner as "OWNER_" {
        pub const MASK: u32 = 0x1F;
        /// Entries in `Globals::team_colors`: one per slot `MASK` can name.
        pub const COLORS: u32 = 32;
    }

    /// A structure's pad under it (`mc_sim::pack_structure_pad`, ground.wgsl `fs_pad`; a
    /// test holds the sim's bits equal): owner in the low bits (`owner::MASK`), two flags
    /// above it, the build fraction in the second byte and the blueprint index in the top
    /// half.
    pub mod pad as "PAD_" {
        /// A plan not yet started: drawn as a ghost.
        pub const GHOST: u32 = 0x40;
        /// The lot of a faction that builds with nanites: dark machined plate, not paving.
        pub const NANITE: u32 = 0x80;
        pub const BUILD_SHIFT: u32 = 8;
        pub const BUILD_MASK: u32 = 0xFF;
        pub const BLUEPRINT_SHIFT: u32 = 16;
    }

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
    /// too small to hide anything), the colour pass's less the pre-pass's, and one
    /// per shadow cascade (only what can cast into it). Each pass draws only its own
    /// list, so a tree outside a cascade never runs the vertex shader there.
    pub mod cull_list as "CULL_LIST_" {
        pub const MAIN: u32 = 0;
        pub const PREPASS: u32 = 1;
        /// What the pre-pass left out. With the pre-pass on, the colour pass draws
        /// `PREPASS` with depth test only (so hidden layers are dropped before they
        /// are shaded, `discard` or not) and this with depth writes.
        pub const REST: u32 = 2;
        /// The nearest cascade's list; cascade `c` is `SHADOW + c`.
        pub const SHADOW: u32 = 3;
        pub const COUNT: u32 = 6;
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
        /// `ModelInfo::icon` bit: cliff rock (`cliffs.rs`). Walls carry tens of thousands
        /// of pieces, and the terrain under them casts the wall's shadow already: a piece
        /// casts only into the nearest cascade, where its ledges' shadows show.
        pub const CLIFF: u32 = 0x2000_0000;
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

    /// A structure's legs into the sea (the Wharf's quay and piles, models/aster/factories.rs):
    /// drawn only where it stands in water, like `part::AFLOAT`, and `entity.wgsl` lets
    /// each vertex authored at or below model z = 0 down onto the seabed under it, so a
    /// leg stands on the bottom however deep the water is.
    pub mod pile as "PILE_" {
        pub const PART: u32 = 72;
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
        /// Rock: a cliff's jointed face (`cliffs.rs`), coloured as the wall it stands on.
        pub const ROCK_CLIFF: u32 = 2;
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

    /// A charge gun's working gear (`rig::CHARGE_GEAR`, `Model::charge_gear`), posed by
    /// entity.wgsl `charge_gear_pose` from the unit's charge (`renderer/titan_charge.rs`):
    /// through the charge the gear opens, after the shot it holds open a moment and closes
    /// as the gun cools; vents lift with the heat after the shot. Travels are metres at
    /// the model's authored size, times `ModelInfo::charge_gear.w`. The Sunspear's.
    pub mod charge_gear as "CHARGE_GEAR_" {
        /// Where a vertex's gear sits in its rig word, and the mask after the shift. It
        /// borrows `UPGRADE_AT` bits 19..22, which only mean anything on refit pieces (and
        /// a tail's segment, on tails); charge gear is never either.
        pub const SHIFT: u32 = 19;
        pub const MASK: u32 = 0x7;
        /// Moves out from the bore along +-y (by the sign of its y): rails parting.
        pub const SPREAD: u32 = 1;
        /// Runs out along +x, the bore: projector heads reaching into the charge.
        pub const EXTEND: u32 = 2;
        /// Lifts along +z with the heat after a shot: vent flaps standing open.
        pub const VENT: u32 = 3;
        /// Turns about the upright through `ModelInfo::charge_gear.xyz`, slowly at rest and
        /// hard through the charge, running down as it cools: gimbal rings round a core.
        pub const SPIN: u32 = 4;
        /// `SPREAD` and `EXTEND` at once: a projector head riding a parting rail's tip,
        /// sliding out along it into the charge.
        pub const REACH: u32 = 5;
        pub const SPREAD_M: f32 = 0.7;
        pub const EXTEND_M: f32 = 1.1;
        pub const VENT_M: f32 = 0.9;
        /// A `pattern::COIL` stage on a `GLOW_LASER` face (the Regency's red-white plasma
        /// coil): the light that this stage means a vent's heat, lit by the shot and
        /// cooling after it, not climbing through the charge.
        pub const HEAT_STAGE: u32 = 7;
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

    /// A map's regions (`mc_data::regions`): stretches of it with a climate and a weather
    /// of their own, parted by climate walls (`Atmosphere::walls`, `region_sky`,
    /// `Globals::region_climate`; regions.wgsl).
    pub mod regions as "REGIONS_" {
        /// Regions a map may have (`mc_data::regions::MAX_REGIONS`).
        pub const MAX: u32 = 8;
        /// Segments its walls may have between them (`Walls::MAX_SEGMENTS`), two vec4 each.
        pub const WALL_SEGMENTS: u32 = 32;
        /// A segment a point lies beside is taken as nearer than another segment's end
        /// that is nearer by less than this many metres (`Walls::TIE_M`).
        pub const TIE_M: f32 = 0.05;
        /// The ground and the sea hand over from one region to the next within this
        /// many metres either side of a wall (wider only where a pixel is).
        pub const BLEND_M: f32 = 6.0;
        /// The weather hands over within this many metres either side of it, and with it
        /// the air's haze: a stretch of sky, not a ruled line.
        pub const SKY_BLEND_M: f32 = 160.0;
        /// Cloud the wind carries across a wall is drawn back to the weather of the
        /// region it is now over within seconds: fully so this close to the wall, easing
        /// off to the weather's usual slow healing by `SKY_HEAL_FAR_M`.
        pub const SKY_HEAL_NEAR_M: f32 = 250.0;
        pub const SKY_HEAL_FAR_M: f32 = 900.0;
        /// A storm cell dies away over its last metres to a wall (sky/regions.rs).
        pub const STORM_FADE_M: f32 = 600.0;
    }

    /// Fog of war as drawn (renderer/fog_field.rs, fog_field.wgsl, `fog_at`): the sim's
    /// coarse on/off grid (`mc_sim::Fog`), smoothed on the GPU into a finer field.
    pub mod fog as "FOG_" {
        /// Edge of one of the sim's fog cells, in metres.
        pub const CELL_M: f32 = 64.0;
        /// Field texels per sim cell each way.
        pub const FIELD_SCALE: u32 = 4;
    }

    /// Graphics quality bits (renderer `SceneQuality`), in `Globals.detail.w`.
    pub mod quality as "QUALITY_" {
        /// Single-patch terrain textures, one-tap shadows, staggered cloud shade.
        pub const SIMPLE_SHADING: u32 = 1;
        /// Screen-space reflections on water (water.wgsl `screen_reflect`).
        pub const WATER_REFLECTIONS: u32 = 2;
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
        /// A Regency Pinched or Pinch-fusion gun's charge, held in front of its bore
        /// (renderer/regency_guns_fx.rs, plasma_puffs.wgsl): a ball of plasma whose skin
        /// boils in churning cells round a white heart. Laid once a tick, two ticks long.
        /// `appearance.rgb` its colour and brightness, `appearance.w` how far it has gone
        /// over to fusion (white at the heart, the prism's colours drifting over it).
        pub const PLASMA_ORB: u32 = 52;
        /// Where a Regency plasma shot lets go (plasma_puffs.wgsl): a lumpy, billowing
        /// bloom of plasma from a white-hot heart that tears into shreds and goes out; no
        /// ring, no dust. `appearance` as `PLASMA_ORB`'s.
        pub const PLASMA_BURST: u32 = 53;
        /// A glob of plasma thrown out of a Regency strike (plasma_puffs.wgsl): a soft red
        /// blob with a hot heart, carried off at `vel`, slowed by the air and pulled down a
        /// little, shrinking and cooling as it goes. `appearance` as `PLASMA_ORB`'s.
        pub const PLASMA_GLOB: u32 = 54;
        /// A Regency plasma shot's wake (plasma_puffs.wgsl): a puff of glowing plasma left
        /// hanging where the shot passed, drifting off at `vel` and rising a little,
        /// swelling from `params.x` to `params.y` across and churning as it cools from its
        /// colour through deep red to nothing. Also the plasma a strike throws up.
        /// `appearance` as `PLASMA_ORB`'s.
        pub const PLASMA_WAKE: u32 = 55;
        /// A Regency power generator's star, lit while the plant runs (renderer/star_core_fx.rs,
        /// plasma_puffs.wgsl): a ball of fusing plasma, its face boiling in cells, white-hot
        /// at the heart, the prism's pinks (common.wgsl `prism`) drifting over it, a ragged
        /// corona flickering off its rim and veins crackling across it. Laid once a tick, two
        /// ticks long; `params.x` the quad's half size (the face is 0.42 of it),
        /// `appearance.rgb` its brightness, `appearance.w` the star's own seed.
        pub const STAR_CORE: u32 = 56;
        /// A Regency power generator's star gone supernova (renderer/supernova_fx.rs, plasma_puffs.wgsl):
        /// a hollow ball of plasma tearing outward, marched through as a volume so it is
        /// brightest at its limb and cut by the ground; ragged and knotted, opening into
        /// holes as it thins; white-hot at first, then the prism's pinks, cooling to
        /// lavender and violet. Its radius grows from `params.x` to `params.y` metres, fast
        /// and then slowing; `vel.x` 0 draws its near half, 1 its far half (one puff each);
        /// `appearance.rgb` its brightness, `appearance.w` its seed.
        pub const SUPERNOVA: u32 = 57;
        /// A streamer of plasma flung out of a supernova (renderer/supernova_fx.rs,
        /// plasma_puffs.wgsl): drawn out along its flight while it is fast, white-hot, then
        /// the prism's pinks, cooling to violet, never red. It coasts on `vel`, slowed by
        /// `NOVA_WISP_DRAG` (it goes `vel / NOVA_WISP_DRAG` in all), rising a little,
        /// swelling from `params.x` to `params.y`; `appearance.rgb` its brightness.
        pub const NOVA_WISP: u32 = 58;
        pub const NOVA_WISP_DRAG: f32 = 1.2;
    }

    /// Colours of a fading beam (`ProjectileInstance::color` low bits under
    /// `PROJECTILE_FADE_BEAM`; sprites.wgsl). The older ones are still spelled out in the
    /// shader; new ones are declared here.
    pub mod fade_beam as "FADE_BEAM_" {
        /// A warp dampener's tether (renderer/damper_fx.rs): crimson lightning with a
        /// white-pink core, crackling.
        pub const TETHER: u32 = 10;
        /// A Regency plasma shot's trail (renderer/regency_guns_fx.rs): a hot filament that
        /// cools to red and breaks up along its length. `aim.w` names its kind: zero for a
        /// Pinched bolt's, these for the others.
        pub const PLASMA_TRAIL: u32 = 11;
        /// A Pinch-fusion round's: starts white and takes the prism's pinks.
        pub const PLASMA_TRAIL_FUSION: f32 = 1.0;
        /// Pink-hot to red, never white: a Gravitic Seeker's filament (renderer/gravitic_fx.rs)
        /// and a cone weapon's wake (renderer/wake_fx.rs), so a fan of them reads red, not as
        /// white sticks.
        pub const PLASMA_TRAIL_PINK: f32 = 2.0;
    }

    /// How a Regency plasma shot is drawn in flight (`mc_sim::mirror::plasma_look`, carried in
    /// `ProjectileInstance::_pad[0]`; sprites.wgsl `plasma_look`). The older looks (1 to 4)
    /// are still spelled out in the shader; new ones are declared here.
    pub mod plasma_look as "PLASMA_LOOK_" {
        /// A Regency seeker, any plasma `missile` (renderer/gravitic_fx.rs): a hard-edged
        /// plasma charge, a lavender-white heart in a violet body, held in a faint shimmering
        /// gravity lens, a black smoke tube behind it. Violet and black read as a missile
        /// defence can take it. No motor, plume or body.
        /// `mc_sim::mirror::PLASMA_LOOK_GRAVITIC_SEEKER`; a test holds them equal.
        pub const GRAVITIC_SEEKER: u32 = 5;
        /// A counter-seeker (renderer/gravitic_fx.rs), the Regency's missile defence: the
        /// seeker's charge in red, never the seeker's violet. Only the renderer writes it.
        pub const COUNTER_SEEKER: u32 = 6;
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

    /// A strategic missile in flight (`Globals::strategic`, nuke_fx.rs, nuke.wgsl): the word
    /// in its first vec4's w packs its kind (2 bits), owner (`owner::MASK`), plume length in
    /// metres and drawn size (`scale * SCALE_STEPS`), each a whole number, 24 bits in all, so
    /// the float holds it exactly.
    pub mod missile as "MISSILE_" {
        /// 0 a warhead, 1 an interceptor (`mc_sim::mirror::STRATEGIC_*`).
        pub const KIND_MASK: u32 = 0x1;
        /// Drawn in the Regency's plasma look (`nuke_look::PLASMA`): a dark body with bronze
        /// bands, a red plasma plume.
        pub const PLASMA: u32 = 0x2;
        pub const OWNER_SHIFT: u32 = 2;
        pub const PLUME_SHIFT: u32 = 8;
        pub const PLUME_MASK: u32 = 0xFF;
        pub const SCALE_SHIFT: u32 = 16;
        pub const SCALE_MASK: u32 = 0xFF;
        pub const SCALE_STEPS: f32 = 64.0;
        /// A warhead's body at scale 1 (the Sunfall's round), nose to nozzle, and its radius.
        pub const WARHEAD_LENGTH: f32 = 36.0;
        pub const WARHEAD_RADIUS: f32 = 2.5;
        /// Vertices drawn per strategic missile (nuke.wgsl `vs_strategic`): the ARC body and
        /// its fins; a Regency round (nova.wgsl) uses fewer and drops the rest.
        pub const VERTS: u32 = 11 * 14 * 6 + 4 * 12;
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

    /// A gyroscope (entity.wgsl): pieces that turn about an axis of their own through
    /// `Model::spinner_pivot` while the structure runs (`MeshBuilder::with_orbit`), so
    /// rings round a core tumble each its own way. The axis and rate ride the part word above its low byte: the
    /// axis's bearing round z in `AZIMUTH`, its lean from z (0 to a right angle) in `TILT`,
    /// both in 256ths, and the rate as a signed byte of `RATE_STEP`s (rad/s).
    pub mod orbit as "ORBIT_" {
        pub const PART: u32 = 66;
        pub const PART_MASK: u32 = 0xFF;
        pub const AZIMUTH_SHIFT: u32 = 8;
        pub const TILT_SHIFT: u32 = 16;
        pub const RATE_SHIFT: u32 = 24;
        pub const RATE_STEP: f32 = 0.03125;
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

    /// An ARC fusion plant's held charge (models/aster/reactor.rs, entity.wgsl): the star
    /// it holds, the charge running round its lit bands, the fusion behind its windows,
    /// and the rings and blades that turn round it.
    pub mod reactor as "REACTOR_" {
        /// `pattern::CORE` (`GLOW` faces): the star, white-hot through its middle, a blue
        /// limb, fine threads of plasma streaming over it, breathing.
        pub const PATTERN_CORE: u32 = 32;
        /// `pattern::CHARGE` (`GLOW` faces): pulses of charge running round the model's z
        /// axis, as on a band round the core.
        pub const PATTERN_CHARGE: u32 = 33;
        /// `pattern::HEAT` (`GLOW_ORANGE` faces): a heat sink's hot core, its heat
        /// rippling along it in waves and flickering, banked low while the plant is down.
        pub const PATTERN_HEAT: u32 = 34;
        /// `pattern::FUSION` (`GLOW` faces): a window onto the fusion inside, blue plasma
        /// streaming round the model's z axis with bright threads in it, surging.
        pub const PATTERN_FUSION: u32 = 35;
        /// Pulses of charge round a band, and how many times round a second they run.
        pub const PULSES: f32 = 3.0;
        pub const PULSE_RATE: f32 = 0.9;
        /// Collar `k` (0 innermost) is part `PART_COLLAR_FIRST + k`, `k` below `COLLARS`:
        /// a ring round the plant's z axis and the blades on it, turned about it while the
        /// plant runs, neighbours the other way, each a little quicker than the one inside.
        pub const PART_COLLAR_FIRST: u32 = 67;
        pub const COLLARS: u32 = 4;
        /// Radians a second the lowest collar turns.
        pub const COLLAR_SPIN: f32 = 0.35;
        pub const COLLAR_STEP: f32 = 0.45;
        /// A heat sink's fins: lifted and settled again in a wave rolling out from the
        /// plant's z axis while it runs, by up to `FIN_LIFT` of the model's height, with
        /// `FIN_WAVE` crests a metre and `FIN_RATE` radians a second.
        pub const PART_FIN: u32 = 71;
        pub const FIN_LIFT: f32 = 0.018;
        pub const FIN_WAVE: f32 = 0.8;
        pub const FIN_RATE: f32 = 2.6;
    }

    /// A material fabricator's working beat (`mc_models::aster::fabricator`,
    /// `regency::condenser`, entity.wgsl `fab_beat`), driven by its work in
    /// `UnitInstance::deploy` (`mc_sim::mirror::fabricator_work`: 1 at full output, less
    /// while its side is short of energy, 0 paused, unpowered or going up). Each beat the
    /// press comes down, the matter it squeezes flashes, the press lifts and the indexer
    /// turns on to the next cell. Short of energy, beats are missed in proportion and
    /// the matter's light sputters; at 0 everything rests and only the standby lamps show.
    pub mod fab as "FAB_" {
        /// Seconds a beat takes. The cadence never changes speed (so nothing jumps when
        /// the work does); short of energy, beats are skipped instead.
        pub const BEAT_S: f32 = 2.2;
        /// The indexer: turned `INDEX_STEP` radians about the model's z axis late in each
        /// beat it works, and back to where it was authored by symmetry, so its pieces must
        /// repeat every `INDEX_STEP` round.
        pub const PART_INDEX: u32 = 73;
        pub const INDEX_STEP: f32 = 1.5707964;
        /// The press: let down `PRESS_TRAVEL` of the model's height early in each beat it
        /// works, held, and raised again.
        pub const PART_PRESS: u32 = 74;
        pub const PRESS_TRAVEL: f32 = 0.05;
        /// Patterns on `material::GLOW_MATERIALS` faces: the matter being condensed (lit in
        /// the Materials colour by the work, flashing at each stroke, sputtering when
        /// short), and a status lamp (steady while it works, a quick amber blink while it
        /// is short, a slow standby glow while it rests).
        pub const PATTERN_MATTER: u32 = 37;
        pub const PATTERN_LAMP: u32 = 38;
        /// `VsOut::drive_at.w` on those faces: `drive` then holds the work, the stroke's
        /// flash and whether the unit is paused.
        pub const DRIVE_TAG: f32 = 4.0;
    }

    /// Work-beam kinds (beams.wgsl) that the sim writes into `BeamInstance::kind`
    /// (`mc_sim::reclaim`). The older kinds are still spelled out in the shader.
    pub mod beam as "BEAM_" {
        /// A Regency builder's nanite stream, emitter to weld (`reclaim::BEAM_NANITE`).
        pub const NANITE: u32 = 1;
        /// A Regency site being fed: its rings and rising filaments (`reclaim::BEAM_NANITE_SITE`).
        pub const NANITE_SITE: u32 = 6;
        // retired: 7 (a scavenger tower's dim sweep beam)
        /// A nanite faction's reclaim: their stream reaching out to what it takes apart,
        /// the matter riding home down its strands (`reclaim::BEAM_NANITE_RECLAIM`).
        pub const NANITE_RECLAIM: u32 = 8;
        /// Quads drawn per beam (renderer `work_beams`, beams.wgsl `vs_beam`). A reclaim or
        /// repair beam uses as many of them for its bits as its length asks for.
        pub const QUADS: u32 = 64;
        /// Quads the replication kinds and the nanite stream use; the rest are hidden for
        /// them. A nanite site's splashes take all `QUADS`.
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

    /// Pinch fusion's light: the Regency's star cores. Plasma pinched
    /// until it fuses burns white-hot at the heart with a pastel prism round it, running
    /// rose, magenta, lavender and peach-gold (common.wgsl `prism`), like the light of a
    /// bridge between stars, not the red of the lesser grades.
    pub mod prism as "PRISM_" {
        /// `material::GLOW_PRISM`: a star core's white-hot heart in the mesh, under the
        /// star the renderer draws as light; the prism turning over it.
        pub const GLOW_MATERIAL: u32 = 28;
        /// How many times a second the prism's colours run once round.
        pub const RATE: f32 = 0.35;
    }

    /// How a shield field is drawn: the look bits of a shield's `packed` word (mc-sim
    /// `mirror::SHIELD_LOOK_SHIFT`, set from its faction's `mc_data::ShieldLook`, whose
    /// numbers these are; tests hold both equal). shields.wgsl draws domes by it,
    /// entity.wgsl `fs_hull` personal fields.
    /// How a strategic missile and a nuclear blast are drawn (`mc_data::strategic::StrategicLook`,
    /// nuke.wgsl, clouds.wgsl `gather_fires`): the blast's in the spare lane of its fourth
    /// vec4 in `Globals::nukes`. Never renumber one.
    pub mod nuke_look as "NUKE_LOOK_" {
        /// ARC's: a fireball that rolls into a mushroom of smoke.
        pub const FISSION: u32 = 0;
        /// The Regency's: a red plasma nova that leaves a glowing plasma cloud standing.
        pub const PLASMA: u32 = 1;
    }

    pub mod shield_look as "SHIELD_LOOK_" {
        pub const SHIFT: u32 = 28;
        pub const MASK: u32 = 3;
        /// ARC's: glass in the faction's shield colour over a honeycomb.
        pub const HONEYCOMB: u32 = 0;
        /// The Regency's: a veil in pinch fusion's prism (`prism`) over a lattice of
        /// red-tinged triangles.
        pub const PRISM: u32 = 1;
        /// Metres along a side of one triangle of a dome's lattice, and of a personal
        /// field's (drawn on the hull, so much finer).
        pub const CELL: f32 = 9.0;
        pub const HULL_CELL: f32 = 2.4;
    }

    /// Rock a beam has melted: the walls of a Regency mine's bore (`pattern::MOLTEN`,
    /// surface.wgsl). A glassy crust over a glowing melt, its cracks lit, the melt running
    /// down; hotter the deeper it goes and the higher the mine's tier.
    pub mod melt as "MELT_" {
        /// `pattern::MOLTEN`.
        pub const PATTERN: u32 = 31;
        /// How fast the melt runs down the walls, metres a second.
        pub const RUN_SPEED: f32 = 0.6;
        /// Metres below the opening where the walls reach full heat.
        pub const DEEP: f32 = 14.0;
    }

    /// A capital warship's armour (`pattern::WARSHIP`, surface.wgsl): strakes of uneven
    /// depth cut into plates of uneven length, some carrying hatches, grilles or stencils,
    /// rows of lit ports along its walls and running lamps at its seams.
    pub mod warship as "WARSHIP_" {
        /// `pattern::WARSHIP`.
        pub const PATTERN: u32 = 36;
    }

    /// Kind bits of a ground stain's `strength_seed` (ground.wgsl `fs_stain`,
    /// grass_gen.wgsl `cs_gather_stains`); neither set is a plain scorch.
    pub mod stain as "STAIN_" {
        /// A crater where a blast struck the ground (renderer/impact_craters.rs).
        pub const CRATER: u32 = 1 << 31;
        // retired: 1 << 30 (molten ground as stains; now the melt field, `melt_field`)
    }

    /// The melt field (renderer/ground_melt.rs, bindings.wgsl `melt_sample`): a heat per
    /// cell of ground, kept only in tiles something has heated, found through a hash
    /// table. Scene set binding 32 is one `array<u32>`: a header of `TABLE` words (x the
    /// tiles in use), then `SLOTS` table entries of two words (the tile's key, its slot
    /// plus one; zero is empty), then from `ATLAS` the tiles' cells, `TILE` x `TILE` words
    /// each, packed as unorm8 x4: heat over `HEAT_MAX`, glass, scorch, unused.
    pub mod melt_field as "MELT_FIELD_" {
        /// Metres a cell is across.
        pub const CELL: f32 = 1.0;
        /// Cells a tile is across.
        pub const TILE: u32 = 32;
        /// Tiles kept, at most.
        pub const TILES: u32 = 1024;
        /// The table has `1 << SLOT_BITS` entries.
        pub const SLOT_BITS: u32 = 12;
        pub const SLOTS: u32 = 1 << SLOT_BITS;
        /// Entries a lookup tries before it gives up on a tile.
        pub const PROBES: u32 = 16;
        /// Words before the table, and where the cells start.
        pub const TABLE: u32 = 4;
        pub const ATLAS: u32 = TABLE + SLOTS * 2;
        /// Heat a cell's byte holds at 255. 1 is white-hot.
        pub const HEAT_MAX: f32 = 1.25;
        /// Heat at which the ground melts: past it, it shows molten and leaves glass.
        pub const MELT: f32 = 0.42;
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

    /// Lens flares on bright points (renderer/lens_flare.rs, screen.wgsl `lens_flare`).
    pub mod lens as "LENS_" {
        /// Flares handed to the tone map at most: the strongest.
        pub const MAX_FLARES: u32 = 32;
    }

    /// A cone weapon's wake (renderer/wake_shell.rs, wake_shell.wgsl), drawn as two meshes
    /// bent in the vertex shader, one instance of each a wake rolling out: the front, a
    /// rolling crest arched over the fan, and the trail it leaves, a shell from the muzzle
    /// out to the crest that cools and is eaten away where the front has passed.
    pub mod wake_shell as "WAKE_SHELL_" {
        /// Wakes drawn at once; the oldest goes first past it (`wake_fx`).
        pub const MAX_SHELLS: u32 = 24;
        /// The trail's quads from the muzzle out to the wall, and across the fan.
        pub const ALONG: u32 = 40;
        pub const AROUND: u32 = 24;
        /// The front's quads along its arc, and round its section.
        pub const ARCH: u32 = 32;
        pub const TUBE: u32 = 12;
        /// The fan drawn over the sim's own: its half-angle times this, at most a half turn.
        pub const WIDTH: f32 = 1.15;
        /// Seconds a stretch of the trail stays hot (heat falls to a third), and seconds
        /// it lasts after the front has passed it.
        pub const COOL: f32 = 1.0;
        pub const LINGER: f32 = 4.5;
        /// Seconds the plasma the front leaves takes to slow from the front's pace to a
        /// stop over the ground (falling to a third of its pace).
        pub const SETTLE: f32 = 0.35;
        /// Seconds the front takes to break up once it has run out, rolling on as it slows.
        pub const BREAK: f32 = 0.6;
    }

    /// A twin gun on a walker's arm that kicks on its own shots (`mc_sim::mirror::UNIT_TWIN_*`
    /// in `UnitInstance::status[1]`; a test holds them equal): one more than its weapon, read
    /// from the unit's `HousePose`, and whether it sits on the right (-y).
    pub mod arm_twin as "ARM_TWIN_" {
        pub const SHIFT: u32 = 2;
        pub const MASK: u32 = 0x7;
        pub const RIGHT: u32 = 0x20;
    }

    /// A unit's gun-house poses (`mc_sim::mirror::UNIT_HOUSE_SHIFT` in `UnitInstance::status[1]`;
    /// a test holds them equal): bits `SHIFT..` hold its index in the houses buffer plus
    /// one, zero for none.
    pub mod unit_house as "UNIT_HOUSE_" {
        pub const SHIFT: u32 = 8;
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

    /// The metal scans the plate is finished with (metal.wgsl): two layers each in
    /// the terrain material array, after the ground and foliage layers
    /// (`mc_render::textures::terrain_materials`, a test holds the indices).
    pub mod metal_scan as "METAL_SCAN_" {
        /// The Regency's worn steel (regency.wgsl): linear albedo, roughness in A; the
        /// next layer is its normal XY and scratches.
        pub const LAYER: i32 = 28;
        /// Metres one repeat of the Regency's scan covers on a model.
        pub const TILE_M: f32 = 2.5;
        /// ARC's scratched steel under the paint (`arc_metal`), laid out the same way.
        pub const ARC_LAYER: i32 = 30;
        /// Metres one repeat of ARC's scan covers on a model.
        pub const ARC_TILE_M: f32 = 2.0;
    }

    /// A face's outline as distances to its own edges (`MeshVertex::face`, edge form): a
    /// Regency face no rectangle fits, so its lit edge and panel line follow the polygon
    /// (regency.wgsl). The fourth distance is stored negated and pushed this many metres
    /// further below zero, so a negative w marks the form and survives rounding.
    pub mod face_edges as "FACE_EDGES_" {
        pub const BIAS: f32 = 0.05;
    }

    /// Waves breaking on the shore (shore.wgsl), heard where they break
    /// (`mc_render::shore`, the ambience's wave sounds).
    pub mod surf as "SURF_" {
        /// Seconds between one breaker and the next.
        pub const PERIOD: f32 = 7.5;
        /// A breaker's height in metres before the set and the climate scale it.
        pub const HEIGHT: f32 = 1.1;
        /// A wave breaks where the water is this many times shallower than it is high.
        pub const BREAK_RATIO: f32 = 1.3;
        /// Seabed slope, rise per metre, below which a shelf counts as this slope.
        pub const MIN_SLOPE: f32 = 0.012;
        /// The steepest the breakers take the bed to be: over a shore that drops off
        /// faster they roll in as if over this, so the surf has room to be seen.
        pub const MAX_SLOPE: f32 = 0.045;
        /// Metres either side of a point the seabed's slope is measured over.
        pub const SLOPE_REACH: f32 = 20.0;
        /// Metres of water past which a swell is not yet a breaker.
        pub const REACH_DEPTH: f32 = 9.0;
        /// Metres up the beach (along it, level) the biggest wash runs.
        pub const RUNUP: f32 = 12.0;
        /// The most the wash climbs, metres of height, however steep the shore.
        pub const RUNUP_RISE: f32 = 1.2;
        /// Share of the period the wash runs up and back for.
        pub const SWASH: f32 = 0.62;
        /// The breakers on a sheltered canyon lake, and on a reef-sheltered tropical
        /// shore, as a share of the open coast's.
        pub const DESERT: f32 = 0.25;
        pub const TROPICAL: f32 = 0.8;
        /// Breakers grow by this share per m/s of wind: size 0.4 + WIND_GAIN * wind,
        /// 1 on a fair day's 12 m/s, clamped to 0.6..1.5 (shore.wgsl `surf_wind`).
        pub const WIND_GAIN: f32 = 0.05;
        /// On a map with regions each region's breakers are worked out this many
        /// metres off per region (shore.wgsl `surf_point`), so a line of them does
        /// not run on through a climate wall.
        pub const REGION_STEP_X: f32 = 7919.0;
        pub const REGION_STEP_Y: f32 = -5347.0;
    }

    /// Lengths of the water's effect list (`renderer/water_fx.rs`, water.wgsl `SeaFxList`):
    /// the rings, wakes and muzzle blasts that go up each frame, and the points of a wake's
    /// path.
    pub mod sea_fx as "SEA_FX_" {
        pub const RIPPLES: u32 = 64;
        /// Hull wakes and torpedo lines together: a squadron's volley is dozens of lines.
        pub const WAKES: u32 = 64;
        pub const BLASTS: u32 = 16;
        pub const WAKE_POINTS: u32 = 12;
    }

    /// Materials (the sim's `mass`) as the interface colours them (mc-game `hud::MASS`) and
    /// the world's materials conduits light them (renderer/adjacency_links.rs), sRGB
    /// `0xRRGGBB`. Energy is the faction's own (`mc_data::PowerLine::color`).
    pub mod tone as "TONE_" {
        pub const MASS: u32 = 0xFF6B3D;
    }

    /// An adjacency conduit (`renderer/adjacency_links.rs` `LinkInstance::flags`, links.wgsl).
    pub mod link as "LINK_" {
        /// The two buildings go down together (`mc_sim::adjacency::bound`).
        pub const BOUND: u32 = 1;
        /// One of its two buildings is selected or under the pointer: drawn brighter.
        pub const HIGHLIGHT: u32 = 2;
        /// A would-be link of a placement ghost: drawn see-through, after the scene.
        pub const PLANNED: u32 = 4;
        /// The path's inner points are turns, each with the look's junction (else they
        /// are points along a curve).
        pub const TURNS: u32 = 8;
        /// The look (`mc_data::LineLook`) sits in the flags from this bit up.
        pub const LOOK_SHIFT: u32 = 8;
        /// Looks. Never renumber one; retire a number with a comment.
        pub const LOOK_CLAMPED: u32 = 0;
        pub const LOOK_PLATED: u32 = 1;
        /// Points of a conduit's path (`LinkInstance::points`) at most.
        pub const POINTS: u32 = 16;
        /// Stretches of the cable, each laid on the ground on its own.
        pub const SEGMENTS: u32 = 64;
        /// Pieces along the cable (clamps, plates, nodes) at most, and junctions at turns.
        pub const PIECES: u32 = 24;
        pub const JUNCTIONS: u32 = 4;
    }

    /// Bits of `UnitInstance::owner_flags` (`owner | flags << 8`) the shaders read that the
    /// older hand-written `FLAG_*` list in common.wgsl does not hold.
    pub mod unit_flag as "UNIT_FLAG_" {
        /// `mc_sim::tables::flag::RECLAIMING << 8`: pulling mass out of a wreck this tick.
        pub const RECLAIMING: u32 = 0x10000;
    }
}
