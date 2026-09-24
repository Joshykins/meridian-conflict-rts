//!use bindings
//!use surface
// Units, structures, wrecks and props. One multi-draw-indirect call renders
// every visible model; per-instance data comes from the visible list the cull
// pass built, so `instance_index` (which includes firstInstance) indexes it.

struct EntityPush {
    // 1: shadow pass. 2: hull-shield skin.
    pass_kind: u32,
    // Live shield records, when this is the hull-shield pass.
    count: u32,
}

var<immediate> push: EntityPush;

// Mirrors the shield pass. Only the hull-shield entry points read these.
struct Shield {
    pos: vec3<f32>,
    radius: f32,
    prev_open: f32,
    open: f32,
    health: f32,
    packed: u32,
    unit_id: u32,
    projector: f32,
    height: f32,
    overlap: u32,
    contact_n: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    contacts: array<u32, 16>,
}

struct ShieldHit {
    pos: vec3<f32>,
    start: f32,
    strength: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

@group(1) @binding(0) var<storage, read> shields: array<Shield>;
@group(1) @binding(1) var<storage, read> shield_hits: array<ShieldHit>;
// Only `fs_hull` reads this: the outermost hull-field skin's depth (`fs_hull_depth`).
@group(2) @binding(7) var hull_front: texture_depth_2d;

// Team colour on a hull: paint a shade under the HUD's colour with only a faint
// glow, so it marks the side without outshining the metal.
const TEAM_PAINT: f32 = 0.45;
const TEAM_GLOW: f32 = 0.02;
const MAT_PLATING: u32 = 0u;
const MAT_ACCENT: u32 = 1u;
const MAT_GLOW: u32 = 2u;
const MAT_TEAM: u32 = 3u;
const MAT_METAL: u32 = 4u;
const MAT_GLASS: u32 = 5u;
const MAT_TREAD: u32 = 6u;
const MAT_GLOW_ORANGE: u32 = 7u;
const MAT_BARK: u32 = 8u;
const MAT_FOLIAGE: u32 = 9u;
const MAT_ROCK: u32 = 10u;
const MAT_CONCRETE: u32 = 11u;
const MAT_WINDOWS: u32 = 12u;
const MAT_GLOW_AMBER: u32 = 13u;
const MAT_PLATING_DARK: u32 = 14u;
const MAT_GLOW_RED: u32 = 15u;
const MAT_GLOW_VIOLET: u32 = 16u;

// The yellow-orange of construction: build beams, build emitters, a refit going up.
const AMBER: vec3<f32> = vec3<f32>(1.0, 0.6, 0.1);

const PART_TURRET: u32 = 1u;
const PART_SPINNER: u32 = 2u;
const PART_LOCOMOTION: u32 = 3u;
// A core mine's pile driver, its pipe string and the next section (`models::part::RAM`,
// `STRING`, `FEED`), all moved on the mine's beat.
const PART_RAM: u32 = 9u;
const PART_STRING: u32 = 10u;
const PART_FEED: u32 = 11u;
// Pieces drawn only in water (an offshore rig's stilts) or only on land (the pit).
const PART_AFLOAT: u32 = 12u;
const PART_ASHORE: u32 = 13u;
const PART_PUMP: u32 = 14u;
const PART_HATCH: u32 = 15u;
// Half an airbase's square shaft, metres (`mc_sim::airbase::SHAFT_HALF`, `models/aster/airbase.rs`).
const AIRBASE_SHAFT: f32 = 18.5;
// Height of its opening plane over the lot (`models/aster/airbase.rs` SHAFT_OPEN).
const AIRBASE_OPEN: f32 = 0.3;
// Depth squeeze down an airbase's shaft: gentler than `PIT_SQUEEZE`, so the far side of
// the 18 m bore, 21 m down, stays in front of the ground a few decimetres under the opening.
const AIRBASE_SQUEEZE: f32 = 0.002;
// Down a pit, depth is squeezed to this share of the true distance below the opening:
// the bottom of the bore (~160 m) must stay nearer than the levelled ground 2.4 m under it.
const PIT_SQUEEZE: f32 = 0.012;

// `rig` bits (models::rig): the leg bone a vertex rides, and the upgrade piece flag.
const LIMB_THIGH: u32 = 1u;
const LIMB_SHIN: u32 = 2u;
const LIMB_FOOT: u32 = 3u;
const LIMB_ARM_GUN: u32 = 4u;
const LIMB_ARM_TOOL: u32 = 5u;
const LIMB_ARM_BOOM: u32 = 6u;
// Folding gear: swung back about `model.fold` while the unit is not building.
const LIMB_FOLD: u32 = 7u;
// The head on the end of the folding gear: bends about `model.fold_wrist`, then rides the arm.
const LIMB_FOLD_HEAD: u32 = 9u;
// A walker's head: looks about while it stands idle (`idle_pose`).
const LIMB_HEAD: u32 = 10u;
// Build-arm gear at work (`rig::WORK_*`): twists, runs out, or opens and closes.
const WORK_TWIST: u32 = 1u;
const WORK_EXTEND: u32 = 2u;
const WORK_BREATHE: u32 = 3u;
// A turret of its own on the turret, about `model.mount`.
const LIMB_MOUNT: u32 = 8u;
// Gun houses of their own on the hull, about `model.houses[limb - LIMB_HOUSE]` (`rig::HOUSE_FIRST`).
const LIMB_HOUSE: u32 = 11u;
// How far into a refit what it takes off has faded and gone.
const LEAVE_BY: f32 = 0.15;
const RIG_SPIN: u32 = 0x40000000u;
const RIG_RECOIL: u32 = 0x10u;
const RIG_FLOAT: u32 = 0x20u;
const RIG_LIFT: u32 = 0x40u;
const RIG_DEPLOY: u32 = 0x80u;
const RIG_UPGRADE: u32 = 0x100u;
// How far a factory deck drops to release a finished hull. Authored up.
const FACTORY_LIFT_DROP: f32 = 0.90;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) material: u32,
    @location(4) part: u32,
    @location(5) rig: u32,
    // The vertex on its own face, and the face's half size (`MeshVertex::face`).
    @location(6) face: vec4<f32>,
    // Pattern in the low byte, then a byte of per-face random.
    @location(7) surface: u32,
    @builtin(instance_index) instance: u32,
}

struct VsOut {
    // Invariant: the hull field's depth pre-pass and its colour pass must land on the same depth.
    @builtin(position) @invariant clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) @interpolate(flat) material: u32,
    @location(4) @interpolate(flat) owner_flags: u32,
    // x build fraction, y health, z local height fraction, w per-entity random
    @location(5) state: vec4<f32>,
    @location(6) local: vec3<f32>,
    // Tech level in the low byte, 0x100 for a mobile unit, 0x200 a hovercraft;
    // then the face's pattern (bits 16..24) and its random byte (24..32).
    @location(7) @interpolate(flat) model_class: u32,
    // x: 1 for an upgrade piece, y: how built the piece is (0 a hologram, 1 done), z: the unit's refit progress
    @location(8) @interpolate(flat) refit: vec3<f32>,
    // x first weld index, y how many, z model reach, w model height
    @location(9) @interpolate(flat) weld: vec4<f32>,
    @location(10) face: vec4<f32>,
    @location(11) @interpolate(flat) unit_id: u32,
    // Share of the model's height its running gear's dust reaches (`Model::dust_line`).
    @location(12) @interpolate(flat) dust: f32,
    // A spacecraft's drives (`ModelInfo::capital`): x main thrust, y lift-jet thrust, 0..1,
    // z its bells' throat depth; x -1 on other models.
    @location(13) @interpolate(flat) drive: vec4<f32>,
    // Which drive (w 1) or lift jet (w 2) this glow belongs to: xyz its mouth; w 0 neither.
    @location(14) @interpolate(flat) drive_at: vec4<f32>,
}

struct Weld {
    // xyz local print origin, w 1 while the beam is on, then decaying.
    pos: vec4<f32>,
}

@group(0) @binding(15) var<storage, read> welds: array<Weld>;

// Distance along a track belt, increasing in the circulation that drives
// the hull forward: top run +x, front down, underside -x, rear up. Side
// faces (the lozenge you see from the flank) split on height, so the
// visible upper half matches the top run.
fn tread_along(p: vec3<f32>, n: vec3<f32>) -> f32 {
    let side = abs(n.y) > abs(n.x) && abs(n.y) > abs(n.z);
    if side {
        return select(p.x, -p.x, p.z > 0.45);
    }
    return -p.x * n.z + p.z * n.x;
}

// Turns in the model's fore-and-aft plane: from +x toward +z.
fn rot_xz(v: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec3<f32>(v.x * c - v.z * s, v.y, v.x * s + v.z * c);
}

// How much of a walk the unit is in, zero standing to one in full stride, and
// where in the cycle it is. The sim counts the ground covered, so the feet
// keep pace with the ground whatever the speed and do not slide.
// A core mine's pile driver at `beat` (0..1, the blow at 0) as a share of its stroke: a
// small kick back off the pipe, a haul up, a hold at the top while the next section swings
// in under it, then a fall that gathers speed until it strikes. The renderer's dust and
// the blow's sound land on the same beat.
fn hammer_lift(beat: f32) -> f32 {
    if beat < 0.08 {
        return 0.03 * sin(beat / 0.08 * PI);
    }
    if beat < 0.84 {
        return smoothstep(0.08, 0.4, beat);
    }
    let fall = (beat - 0.84) / 0.16;
    return 1.0 - fall * fall;
}

// Where a core mine's pipe pieces are at `beat`, as an offset from where they are authored.
// The driver rides up by `hammer_lift`. The string only moves when the falling driver has
// met the new section and drives the two down together, one section by the blow: the
// string repeats every section, so starting again from the top is seamless. The next
// section waits under the rack's collar, rises out of it, swings over the bore onto the
// string, then goes down with it; by the next beat it is part of the string.
fn pipe_offset(part: u32, beat: f32, model: ModelInfo) -> vec3<f32> {
    let lift = model.spinner_pivot.w * hammer_lift(beat);
    if part == PART_RAM {
        return vec3<f32>(0.0, 0.0, lift);
    }
    let section = model.pit_feed.z;
    let driven = select(0.0, section - min(lift, section), beat >= 0.84);
    if part == PART_STRING {
        return vec3<f32>(0.0, 0.0, -driven);
    }
    let rise = smoothstep(0.08, 0.36, beat);
    let swing = smoothstep(0.44, 0.74, beat);
    return vec3<f32>(-model.pit_feed.xy * swing, -section * (1.0 - rise) - driven);
}

fn walk_state(e: Entity, model: ModelInfo) -> vec2<f32> {
    let stride = model.leg_hip.w;
    let t = globals.sun.w;
    let ease = 1.0 / (0.04 * stride);
    let amount = mix(clamp(e.gait.z * ease, 0.0, 1.0), clamp(e.gait.y * ease, 0.0, 1.0), t);
    return vec2<f32>(amount, fract((e.gait.x - e.gait.y * (1.0 - t)) / stride));
}

// The body over its legs, leaning toward the planted one. Walking, it is highest as it
// vaults over that leg; running (a foot planted for under half the cycle), it sinks onto
// it and is highest in the air between steps. A walker with a crouch (`Legs::crouch`)
// settles that far onto bent knees as it gets into its stride.
fn walk_bob(walk: vec2<f32>, model: ModelInfo) -> vec3<f32> {
    let stance = model.leg_ankle.w;
    let mid = stance * 0.5;
    let rise = select(0.16, -0.2, stance < 0.5);
    let bob = vec3<f32>(0.0, cos((walk.y - mid) * 2.0 * PI) * 0.08, cos((walk.y - mid) * 4.0 * PI) * rise) * model.leg_knee.w;
    return (bob - vec3<f32>(0.0, 0.0, model.surface.z)) * walk.x;
}

// Where an idle walker's gaze rests, -0.5..0.5, one number a `period` seconds on
// average. It turns to each in a quick move, then holds; the holds vary in length.
fn glance(time: f32, seed: f32, period: f32) -> f32 {
    let s = time / period + seed * 7.0 + 0.3 * sin(time * 0.41 + seed * 19.0);
    let k = floor(s);
    let turn = smoothstep(0.0, 0.16, fract(s));
    return mix(hash11(k + seed * 131.0), hash11(k + 1.0 + seed * 131.0), turn) - 0.5;
}

// A walker with a head (`rig::HEAD`) at rest: x head yaw, y head pitch, z the gun arm's
// pitch, w the tool arm's (radians, added to their aim). It looks about, now and then
// checks its gun, and its arms hang loose and sway a little with its breathing. All of
// it fades out as it aims, fires or builds; walking, the arms swing against the legs
// and the head looks about less.
fn idle_pose(e: Entity, model: ModelInfo, walk: vec2<f32>, time: f32) -> vec4<f32> {
    let t = globals.sun.w;
    let seed = hash11(f32(e.unit_id & 0xFFFFu) * 0.731 + 3.3);
    let aim = abs(lerp_angle(e.prev_turret_yaw, e.turret_yaw, t)) * 3.0
        + abs(mix(e.arm_pitch.x, e.arm_pitch.y, t)) * 6.0
        + abs(mix(e.arm_pitch.z, e.arm_pitch.w, t)) * 6.0
        + mix(e.prev_recoil, e.recoil, t) * 4.0
        + clamp(mix(e.prev_deploy, e.deploy, t), 0.0, 1.0) * 4.0;
    let calm = 1.0 - clamp(aim, 0.0, 1.0);
    let still = 1.0 - walk.x;
    // The head: turns to a new point every few seconds, mostly ahead, a little down.
    let yaw = glance(time, seed, 3.4) * 1.1 + 0.03 * sin(time * 0.9 + seed * 40.0);
    let nod = glance(time, seed + 0.37, 3.4) * 0.22 - 0.05 + 0.02 * sin(time * 1.3 + seed * 23.0);
    let look = calm * mix(1.0, 0.35, walk.x);
    // Breathing sway, a little out of step between the arms.
    let breath = time * 1.15 + seed * 60.0;
    var gun = 0.05 * sin(breath) + 0.025 * sin(time * 0.53 + seed * 11.0);
    var tool = 0.05 * sin(breath + 0.9) + 0.025 * sin(time * 0.61 + seed * 17.0);
    // Now and then it raises the gun arm a little, holds it, and lets it down again.
    let check = time / 11.0 + seed * 5.0;
    let lift = select(0.0, 0.26, hash11(floor(check) + seed * 71.0) > 0.55);
    let held = smoothstep(0.0, 0.1, fract(check)) * (1.0 - smoothstep(0.26, 0.4, fract(check)));
    gun += lift * held;
    // Walking: the arms swing against the legs (the tool arm is on the left, +y).
    let swing = 0.12 * cos(walk.y * 2.0 * PI);
    gun = gun * still + swing * walk.x;
    tool = tool * still - swing * walk.x;
    return vec4<f32>(yaw * look, nod * look, gun * calm, tool * calm);
}

// A leg vertex posed for this moment of the stride. Two bones, hip to knee and
// knee to ankle, solved so the ankle is where the foot has to be: planted and
// passing under the body, or lifted and swinging forward.
fn walk_leg(pos: vec3<f32>, normal: vec3<f32>, limb: u32, model: ModelInfo, walk: vec2<f32>) -> array<vec3<f32>, 2> {
    let stride = model.leg_hip.w;
    let lift = model.leg_knee.w;
    let hip0 = model.leg_hip.xz;
    let knee0 = model.leg_knee.xz;
    let ankle0 = model.leg_ankle.xz;
    // The right leg is half a cycle behind the left.
    let phase = fract(walk.y + select(0.5, 0.0, pos.y > 0.0));
    let stance = model.leg_ankle.w;
    let reach = stance * stride;
    var foot = vec2<f32>(0.0);
    var pitch = 0.0;
    if phase < stance {
        foot.x = reach * (0.5 - phase / stance);
    } else {
        let u = (phase - stance) / (1.0 - stance);
        foot = vec2<f32>(reach * (u * u * (3.0 - 2.0 * u) - 0.5), lift * sin(u * PI));
        // Toe down as it leaves the ground, up as it reaches for the next step.
        pitch = 0.4 * sin(u * 2.0 * PI);
    }
    let hip = hip0 + vec2<f32>(0.0, walk_bob(walk, model).z);
    let l1 = distance(knee0, hip0);
    let l2 = distance(ankle0, knee0);
    let want = ankle0 + foot * walk.x - hip;
    let d = clamp(length(want), abs(l1 - l2) + 0.01, l1 + l2 - 0.01);
    let aim = atan2(want.y, want.x);
    let ankle = hip + vec2<f32>(cos(aim), sin(aim)) * d;
    // The knee stays on the side of the leg it rests on.
    let a = knee0 - hip0;
    let b = ankle0 - hip0;
    let side = select(1.0, -1.0, a.x * b.y - a.y * b.x > 0.0);
    let bend = acos(clamp((l1 * l1 + d * d - l2 * l2) / (2.0 * l1 * d), -1.0, 1.0));
    let knee = hip + vec2<f32>(cos(aim + side * bend), sin(aim + side * bend)) * l1;

    var turn = 0.0;
    var pivot0 = vec2<f32>(0.0);
    var pivot = vec2<f32>(0.0);
    if limb == LIMB_THIGH {
        turn = atan2(knee.y - hip.y, knee.x - hip.x) - atan2(a.y, a.x);
        pivot0 = hip0;
        pivot = hip;
    } else if limb == LIMB_SHIN {
        turn = atan2(ankle.y - knee.y, ankle.x - knee.x) - atan2(ankle0.y - knee0.y, ankle0.x - knee0.x);
        pivot0 = knee0;
        pivot = knee;
    } else {
        turn = -pitch * walk.x;
        pivot0 = ankle0;
        pivot = ankle;
    }
    let p = rot_xz(pos - vec3<f32>(pivot0.x, 0.0, pivot0.y), turn) + vec3<f32>(pivot.x, 0.0, pivot.y);
    return array<vec3<f32>, 2>(p, rot_xz(normal, turn));
}

fn rot_x(v: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec3<f32>(v.x, v.y * c - v.z * s, v.y * s + v.z * c);
}

fn rot_y(v: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec3<f32>(v.x * c + v.z * s, v.y, -v.x * s + v.z * c);
}

// A spacecraft (`ModelInfo::capital`, `models::capital`): legs or stern drives.
fn capital_ship(model: ModelInfo) -> bool {
    return model.capital[0].w != 0.0 || model.capital[4].x != 0.0;
}

// How high a spacecraft's hull (its feet's plane) is over the ground, between ticks.
fn capital_height(e: Entity, t: f32) -> f32 {
    let at = mix(e.prev_pos, e.pos, t);
    return at.z - terrain_height(at.xy);
}

// How far out its gear is, 0 stowed to 1 down: all out at touchdown, stowed from 60 m up
// (`transport::GEAR_HEIGHT`, as `mirror::UNIT_GEAR_SHIFT` says), smooth between ticks.
fn capital_gear(h: f32) -> f32 {
    return 1.0 - clamp(h / 60.0, 0.0, 1.0);
}

// How far the hull sinks on its shock struts while its feet stay planted: it comes down
// onto them hard, rebounds a little and settles slightly compressed, timed by the ramp
// that opens once it is down (`deploy`); lifting off, the struts run back out.
fn capital_sink(model: ModelInfo, e: Entity, t: f32, h: f32) -> f32 {
    let ground = 1.0 - smoothstep(0.0, 2.5, h);
    let d = clamp(mix(e.prev_deploy, e.deploy, t), 0.0, 1.0);
    let size = 0.5 * (model.capital[3].y + model.capital[3].z);
    return size * ground * (0.8 + 1.0 * exp(-12.0 * d) * cos(25.0 * d));
}

// A model vertex after the unit was destroyed. The same intact mesh, wrecked a
// different way for every wreck (`seed`, zero to one): the turret is blown off
// its ring and lies beside the hull, the hull is crumpled by a smooth field
// (so faces that share a corner still meet), caved in where the killing blow
// landed, and a vehicle settles crooked on its broken running gear. The
// fragment shader takes its normals from the warped surface.
fn wrecked(pos: vec3<f32>, part: u32, model: ModelInfo, seed: f32) -> vec3<f32> {
    let r1 = hash11(seed * 173.3 + 1.7);
    let r2 = hash11(seed * 311.9 + 5.3);
    let r3 = hash11(seed * 97.1 + 9.1);
    let height = max(model.height, 1.0);
    let reach = max(model.bounds_radius, 1.0);
    let mobile = (model.icon & 0x10000u) != 0u;
    var p = pos;
    if part == PART_TURRET {
        let pivot = model.turret_pivot.xyz;
        var q = rot_z(p - pivot, (r1 - 0.5) * 5.0);
        q = rot_x(q, 0.3 + r2 * 0.45);
        q = rot_y(q, (r3 - 0.5) * 0.5);
        // Thrown clear to one side or the other, never along the hull, where it would land in it.
        let away = select(-1.5707963, 1.5707963, r3 > 0.5) + (r2 - 0.5) * 1.1;
        let thrown = reach * (0.95 + r1 * 0.25);
        p = q + vec3<f32>(cos(away), sin(away), 0.0) * thrown;
        p.z = max(p.z + height * 0.16, 0.02);
        return p;
    }
    // Crumple: a smooth field of the position, nothing at the ground and most at the top.
    let amp = clamp(height * 0.11, 0.15, 1.4);
    let at = (p.xy + vec2<f32>(p.z * 0.61, p.z * 0.37)) / (reach * 1.7) + vec2<f32>(seed * 3.1, seed * 7.7);
    let field = textureSampleLevel(noise_map, repeat_sampler, at, 2.0);
    let rise = smoothstep(0.04, 0.55, p.z / height);
    p += vec3<f32>(field.b - 0.5, field.a - 0.5, -abs(field.b - field.a)) * 2.4 * amp * rise;
    // Caved in around where it was hit.
    let hit = vec2<f32>(r1 - 0.5, r2 - 0.5) * reach * 0.9;
    let d = distance(p.xy, hit) / (reach * 0.5);
    p.z *= 1.0 - 0.6 * exp(-d * d);
    if part == PART_LOCOMOTION {
        // Tracks thrown and splayed, legs folded.
        p = vec3<f32>(p.x, p.y * 1.07, p.z * 0.82);
    }
    if mobile {
        p = rot_y(rot_x(p, (r2 - 0.5) * 0.2), (r3 - 0.5) * 0.14);
        // A ship's keel lies in the silt: nothing to flatten against.
        let naval = (model.icon & 0x800000u) != 0u;
        p.z = select(max(p.z - height * 0.03, 0.0), p.z - height * 0.03, naval);
    }
    return p;
}

// The way a hull field pushes this vertex out, and how far in pushes (`models::shell`).
fn shell_dir(surface: u32) -> vec4<f32> {
    let s = surface >> 16u;
    let q = vec2<f32>(f32(s & 63u), f32((s >> 6u) & 63u)) / 63.0 * 2.0 - 1.0;
    var d = vec3<f32>(q, 1.0 - abs(q.x) - abs(q.y));
    let t = max(-d.z, 0.0);
    d.x += select(t, -t, d.x >= 0.0);
    d.y += select(t, -t, d.y >= 0.0);
    return vec4<f32>(normalize(d), 1.0 + f32((s >> 12u) & 15u) * 0.1);
}

fn find_hull_shield(unit_id: u32) -> i32 {
    let n = push.count;
    for (var i = 0u; i < n; i++) {
        let s = shields[i];
        if s.unit_id != unit_id || ((s.packed >> 25u) & 1u) == 0u {
            continue;
        }
        if mix(s.prev_open, s.open, globals.sun.w) > 0.001 {
            return i32(i);
        }
    }
    return -1;
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    let e = load_entity(visible[in.instance]);
    let model = models[e.blueprint];
    let t = globals.sun.w;
    let time = globals.camera.w;
    var scale = 1.0;
    if (e.owner_flags & KIND_PROP) != 0u && e.scale != 0u {
        scale = f32(e.scale) * 0.001;
    }

    var p = in.pos;
    var n = in.normal;
    // A core mine stands in the sea on stilts, raised clear of the water, with no pit; on
    // land it has its pit and no stilts (`models::Pit`).
    let rig_afloat = model.pit.y > 0.0 && terrain_height(e.pos.xy) < globals.map.z - 0.5;
    if (in.part == PART_AFLOAT && !rig_afloat) || (in.part == PART_ASHORE && rig_afloat) {
        var hidden: VsOut;
        hidden.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
        return hidden;
    }
    if rig_afloat && in.part != PART_AFLOAT {
        p.z += model.pit_feed.w;
    }
    // A hull field poses the shared shell direction instead, so a corner's faces stay joined.
    var shell_stretch = 1.0;
    if push.pass_kind == 2u {
        let shell = shell_dir(in.surface);
        n = shell.xyz;
        shell_stretch = shell.w;
    }
    // A standing tree in the air: where the wind and blasts push its top, and how hard
    // its leaves are shaken (`tree_air`). A trampled one is past caring.
    var tree = vec3<f32>(0.0);
    let is_tree = (e.owner_flags & KIND_PROP) != 0u && e.arm_pitch.x == 0.0
        && (in.material == MAT_FOLIAGE || in.material == MAT_BARK);
    if is_tree {
        tree = tree_air(e.pos, max(model.height * scale, 1.0), e.unit_id);
    }
    if in.material == MAT_FOLIAGE {
        // Leaf cards are lit as the crown they belong to: the mesh carries the
        // crown's outward normal at each corner (`MeshBuilder::leaf_card`).
        // Small independent leaf movement on top of a slow sway of the crown.
        n = select(normalize(vec3<f32>(in.pos.xy * 0.18, 0.7)), in.face.xyz, dot(in.face.xyz, in.face.xyz) > 0.25);
        let flex = clamp(in.pos.z / max(model.height, 1.0), 0.0, 1.0);
        let card = f32((in.surface >> 8u) & 0x7Fu);
        let phase = time * 1.5 + in.pos.x * 1.7 + in.pos.y * 1.1 + card * 0.37 + f32(e.unit_id % 31u);
        p += vec3<f32>(sin(phase * (1.0 + tree.z * 0.8)), cos(phase * 0.83), 0.0) * (0.04 + 0.11 * tree.z) * flex;
    }

    // What the next upgrade adds is not there at all until the refit is under way.
    let tier_piece = (in.rig & RIG_UPGRADE) != 0u;
    if tier_piece && (e.upgrade <= 0.0 || (e.owner_flags & (KIND_WRECK | KIND_GHOST)) != 0u) {
        var hidden: VsOut;
        hidden.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
        return hidden;
    }
    // Refit modules: a module's pieces are there while the unit has it fitted, and the pieces
    // a module replaces while it does not. During a refit, what the new loadout adds goes up
    // like an upgrade piece and what it takes off fades to a hologram.
    let need = (in.rig >> 9u) & 63u;
    let until = (in.rig >> 24u) & 63u;
    let have = model.modules;
    let next = select(have, e.refit_modules, e.refit_modules != 0u && e.upgrade > 0.0);
    let shown_now = (need == 0u || (have & (1u << (need - 1u))) != 0u)
        && (until == 0u || (have & (1u << (until - 1u))) == 0u);
    let shown_next = (need == 0u || (next & (1u << (need - 1u))) != 0u)
        && (until == 0u || (next & (1u << (until - 1u))) == 0u);
    let arriving = !shown_now && shown_next;
    let leaving = shown_now && !shown_next;
    // What comes off is gone before what replaces it starts to go up.
    if (!shown_now && !arriving) || (leaving && e.upgrade > LEAVE_BY) {
        var hidden: VsOut;
        hidden.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
        return hidden;
    }
    let piece = tier_piece || arriving || leaving;
    let limb = in.rig & 0xFu;
    let walks = model.leg_hip.w > 0.0 && (e.owner_flags & KIND_WRECK) == 0u;
    var walk = vec2<f32>(0.0);
    if walks {
        walk = walk_state(e, model);
    }
    // A walker with a head is never quite still (`idle_pose`).
    var idle = vec4<f32>(0.0);
    if walks && model.surface.w > 0.0 && in.part == PART_TURRET
        && (e.owner_flags & (KIND_GHOST | FLAG_UNDER_CONSTRUCTION | FLAG_IN_FACTORY)) == 0u {
        idle = idle_pose(e, model, walk, time);
    }
    // A hull going down (`WRECK_SINKING`, 2) is posed like a falling wreck: whole, pitched and
    // rolled by the sim as it sinks, not crumpled. It settles into an ordinary wreck on the seabed.
    let falling = (e.owner_flags & KIND_WRECK) != 0u && (e.scale == 1u || e.scale == 2u);
    // A trampled tree tips over from its foot (renderer/fallen_trees.rs).
    let toppled = (e.owner_flags & KIND_PROP) != 0u && e.arm_pitch.x != 0.0;
    if (e.owner_flags & KIND_WRECK) != 0u && !falling {
        p = wrecked(p, in.part, model, hash11(f32(e.unit_id & 0xFFFFu)));
    } else if in.part == PART_TURRET {
        // Folding gear: an arm that hangs down the back while the unit is not building. When
        // it builds the arm swings back, up and over the outside of the shoulder, then the head
        // at its wrist unfolds from along the arm and points down at the work, the way the
        // build arm does. Stowing runs the same path back: head first, then the arm.
        if (limb == LIMB_FOLD || limb == LIMB_FOLD_HEAD) && model.fold.w != 0.0 {
            let out = clamp(mix(e.prev_deploy, e.deploy, t), 0.0, 1.0);
            let arm = smoothstep(0.0, 0.7, out);
            let head = smoothstep(0.45, 1.0, out);
            if limb == LIMB_FOLD_HEAD {
                let aim = mix(e.arm_pitch.z, e.arm_pitch.w, t) * smoothstep(0.8, 1.0, out);
                // A slow search over the work while it is out.
                let scan = 0.04 * sin(time * 2.3 + f32(e.unit_id % 17u)) * smoothstep(0.9, 1.0, out);
                let wrist = model.fold_wrist.xyz;
                let bend = (1.0 - head) * model.fold_wrist.w + aim + scan;
                p = rot_xz(p - wrist, bend) + wrist;
                n = rot_xz(n, bend);
            }
            let hinge = model.fold.xyz;
            let swing = (1.0 - arm) * model.fold.w;
            // Out over the shoulder's outboard side through the middle of the sweep, so it
            // goes round the pauldron, not through it; square at both ends.
            let roll = -sign(hinge.y) * 0.65 * sin(3.14159265 * arm);
            p = rot_x(rot_xz(p - hinge, swing), roll) + hinge;
            n = rot_x(rot_xz(n, swing), roll);
        }
        // The head nods and turns about its neck, with a slight tilt into the turn.
        if limb == LIMB_HEAD && model.surface.w > 0.0 {
            let neck = vec3<f32>(model.turret_pivot.w, 0.0, model.surface.w);
            let q = rot_z(rot_xz(rot_x(p - neck, -idle.x * 0.15), idle.y), idle.x);
            p = q + neck;
            n = rot_z(rot_xz(rot_x(n, -idle.x * 0.15), idle.y), idle.x);
        }
        // A shoulder gun: the tube kicks back, pitches about its trunnion, and turns off the torso.
        if limb == LIMB_MOUNT && model.mount.w >= 0.0 && any(model.mount.xyz != vec3<f32>(0.0)) {
            let pivot = model.mount.xyz;
            if (in.rig & RIG_RECOIL) != 0u {
                p.x -= model.mount.w * mix(e.spin_recoil.z, e.spin_recoil.w, t);
            }
            let pitch = mix(e.mount.z, e.mount.w, t);
            p = rot_xz(p - pivot, pitch) + pivot;
            n = rot_xz(n, pitch);
            let yaw = lerp_angle(e.mount.x, e.mount.y, t);
            p = rot_z(p - pivot, yaw) + pivot;
            n = rot_z(n, yaw);
        }
        // Rotary barrels turn about their axis while they are spun up.
        // Only the barrels this loadout's axis is for: while a refit moves the gun, the ones
        // arriving or leaving stay still rather than turning about the other gun's axis.
        let spin_tags = ((in.rig >> 9u) & 63u) | (((in.rig >> 24u) & 63u) << 6u);
        if (in.rig & RIG_SPIN) != 0u && model.spin.w > 0.0 && spin_tags == u32(model.spin.x + 0.5) {
            let turn = mix(e.spin_recoil.x, e.spin_recoil.y, t);
            let axis = vec3<f32>(0.0, model.spin.y, model.spin.z);
            let q = p - axis;
            p = vec3<f32>(q.x, q.y * cos(turn) - q.z * sin(turn), q.y * sin(turn) + q.z * cos(turn)) + axis;
            n = vec3<f32>(n.x, n.y * cos(turn) - n.z * sin(turn), n.y * sin(turn) + n.z * cos(turn));
        }
        // Suite gear on the build arm works while the unit builds, eased with the deploy.
        let work = ((in.rig >> 15u) & 1u) | ((in.rig >> 30u) & 2u);
        if work != 0u && model.arm_pivot.w > 0.0 {
            let ramp = smoothstep(0.0, 1.0, clamp(mix(e.prev_deploy, e.deploy, t), 0.0, 1.0));
            let axis = vec3<f32>(0.0, model.arm_pivot.y, model.arm_pivot.z);
            let beat = time * 6.5 + f32(e.unit_id % 13u);
            if work == WORK_TWIST {
                // Back and forth about the forearm, a quarter turn each way.
                let turn = ramp * (0.45 * sin(time * 2.6 + f32(e.unit_id % 7u)) + 0.12 * sin(beat));
                p = rot_x(p - axis, turn) + axis;
                n = rot_x(n, turn);
            } else if work == WORK_EXTEND {
                // Runs out along the forearm, and pumps a little while it works.
                p.x += ramp * (1.28 + 0.1 * sin(beat));
            } else {
                // Opens round the forearm's axis in a slow pulse.
                let q = p - axis;
                let open = 1.0 + ramp * (0.16 + 0.12 * sin(beat * 0.6));
                p = axis + vec3<f32>(q.x, q.y * open, q.z * open);
            }
        }
        // A forearm points up or down at what it aims at, about its elbow, before the torso turns.
        // A two-bone build arm (`arm_pivot.w > 1.5`) folds the boom about the shoulder first.
        if limb >= LIMB_ARM_GUN && limb <= LIMB_ARM_BOOM && model.arm_pivot.w > 0.0 {
            let tool_pitch = mix(e.arm_pitch.z, e.arm_pitch.w, t);
            let gun_pitch = mix(e.arm_pitch.x, e.arm_pitch.y, t);
            let elbow = vec3<f32>(model.arm_pivot.x, 0.0, model.arm_pivot.z);
            let two_bone = model.arm_pivot.w > 1.5;
            if two_bone && limb == LIMB_ARM_BOOM {
                let shoulder = model.turret_pivot.xyz;
                p = rot_xz(p - shoulder, gun_pitch) + shoulder;
                n = rot_xz(n, gun_pitch);
            } else if two_bone && limb == LIMB_ARM_TOOL {
                let shoulder = model.turret_pivot.xyz;
                p = rot_xz(p - elbow, tool_pitch) + elbow;
                n = rot_xz(n, tool_pitch);
                p = rot_xz(p - shoulder, gun_pitch) + shoulder;
                n = rot_xz(n, gun_pitch);
            } else {
                let pitch = select(tool_pitch + idle.w, gun_pitch + idle.z, limb == LIMB_ARM_GUN);
                p = rot_xz(p - elbow, pitch) + elbow;
                n = rot_xz(n, pitch);
                // The tube kicks back along its aim the instant it fires, then runs home.
                // Stepping the slide ten times a second reads as jitter, same as the turret.
                if (in.rig & RIG_RECOIL) != 0u && model.recoil.w > 0.0 {
                    let kick = mix(e.prev_recoil, e.recoil, t);
                    p -= rot_xz(model.recoil.xyz, pitch) * (model.recoil.w * kick);
                }
            }
        } else if (in.rig & RIG_RECOIL) != 0u && model.recoil.w > 0.0 && limb != LIMB_MOUNT {
            // A fixed turret gun: slide along the bore in turret space, then yaw with the turret.
            let kick = mix(e.prev_recoil, e.recoil, t);
            p -= model.recoil.xyz * (model.recoil.w * kick);
        }
        // The turret glides between ticks like the hull does; stepping it ten times a second reads as jitter.
        let yaw = lerp_angle(e.prev_turret_yaw, e.turret_yaw, t);
        let pivot = model.turret_pivot.xyz;
        p = rot_z(p - pivot, yaw) + pivot;
        n = rot_z(n, yaw);
    } else if (model.icon & 0x2000000u) != 0u && in.part == 7u {
        // Courier stern bay plug doors slide into its shoulders.
        let open = smoothstep(0.0, 1.0, mix(e.prev_deploy, e.deploy, t));
        p.y += sign(p.y) * open * 14.2;
    } else if (model.icon & 0x400000u) != 0u && in.part == 7u {
        // The Osprey's hold doors: two leaves hinged at the hold's sides
        // (`osprey::DOOR_HINGE`) that swing down and out to let the flock drop.
        let open = smoothstep(0.0, 1.0, mix(e.prev_deploy, e.deploy, t));
        let hinge = vec3<f32>(0.0, sign(p.y) * 2.55, 0.56);
        let ang = sign(p.y) * open * 1.45;
        p = rot_x(p - hinge, ang) + hinge;
        n = rot_x(n, ang);
    } else if model.capital[6].w != 0.0 && in.part == 16u {
        // A lift ship's belly ramp, authored lying on the ground; it swings up about
        // its hinge at the back of the hold floor to close (`CapitalRig::ramp`,
        // closed swing 0.46365 rad).
        let open = smoothstep(0.0, 1.0, mix(e.prev_deploy, e.deploy, t));
        let hinge = vec3<f32>(model.capital[6].z, 0.0, model.capital[6].w);
        let ang = (1.0 - open) * 0.46365;
        p = rot_y(p - hinge, ang) + hinge;
        n = rot_y(n, ang);
    } else if model.capital[0].w != 0.0 && (in.part == 17u || (in.part >= 20u && in.part <= 22u)) {
        // A spacecraft's landing gear (`CapitalRig::legs`), authored all the way out and
        // staged over how far out it is: the bay doors (22) swing open about the bay's long
        // edges, the leg (17) swings down about its hinge, the strut (20) telescopes out and
        // the foot (21) unfolds its pads.
        let gear = capital_gear(capital_height(e, t));
        let side = select(-1.0, 1.0, p.y >= 0.0);
        let fore = p.x > 0.5 * (model.capital[0].x + model.capital[1].x);
        let leg = select(model.capital[1], model.capital[0], fore);
        let size = select(model.capital[3].z, model.capital[3].y, fore);
        if in.part == 22u {
            let open = smoothstep(0.0, 0.18, gear);
            let y0 = select(model.capital[2].z, model.capital[2].x, fore);
            let y1 = select(model.capital[2].w, model.capital[2].y, fore);
            let inner = abs(p.y) < (y0 + y1) * 0.5;
            let hinge = vec3<f32>(0.0, side * select(y1, y0, inner), model.capital[3].x);
            let ang = -side * select(-1.0, 1.0, inner) * open * 1.7;
            p = rot_x(p - hinge, ang) + hinge;
            n = rot_x(n, ang);
        } else {
            let swing = smoothstep(0.12, 0.6, gear);
            let ext = smoothstep(0.55, 0.82, gear);
            let unfold = smoothstep(0.75, 0.92, gear);
            // The pads (dark plating) fold up about their inner top edges.
            if in.part == 21u && in.material == 14u {
                let d = sign(p.x - leg.x);
                let pad = vec3<f32>(leg.x + d * size, 0.0, leg.z - 33.6 * size);
                let ang = -d * (1.0 - unfold) * 1.4;
                p = rot_y(p - pad, ang) + pad;
                n = rot_y(n, ang);
            }
            if in.part != 17u {
                let axis = normalize(vec3<f32>(0.0, -1.5 * side, 13.2));
                p += axis * ((1.0 - ext) * 12.0 * size);
            }
            let hinge = vec3<f32>(leg.x, side * leg.y, leg.z);
            let ang = leg.w * (1.0 - swing) * 1.5708;
            p = rot_y(p - hinge, ang) + hinge;
            n = rot_y(n, ang);
        }
    } else if model.capital[4].x != 0.0 && in.part == 19u
        && (e.owner_flags & (KIND_WRECK | KIND_GHOST | FLAG_UNDER_CONSTRUCTION)) == 0u {
        // A spacecraft drive's iris vanes turn about its axis (`CapitalRig::drives`).
        let dr = model.capital[4];
        let cy = sign(p.y) * select(dr.z, dr.w, abs(p.y) > 0.5 * (dr.z + dr.w));
        let c = vec3<f32>(dr.x, cy, dr.y);
        let phase = time * 0.65 * sign(p.y) + cy * 0.13;
        p = rot_x(p - c, phase) + c;
        n = rot_x(n, phase);
    } else if (model.icon & 0x400000u) != 0u && in.part == 8u {
        // The cradles lower the drones out of the hold (`air_support::drone_socket`).
        p.z -= mix(e.prev_deploy, e.deploy, t) * 1.9;
    } else if in.part == 5u || in.part == 6u {
        let motion = e.pos - e.prev_pos;
        let yaw = lerp_angle(e.prev_heading, e.heading, t);
        let forward = vec2<f32>(cos(yaw), sin(yaw));
        let local_speed = dot(motion.xy, forward);
        let lateral = dot(motion.xy, vec2<f32>(-forward.y, forward.x));
        let carrier = (model.icon & 0x400000u) != 0u;
        // Pod pivots: `kestrel::NACELLES` and `osprey::NACELLES` (`models::vtol_nacelles`).
        let pivot = select(
            vec3<f32>(select(2.55, -3.2, in.part == 6u), sign(p.y) * 4.75, 1.55),
            vec3<f32>(select(3.3, -3.5, in.part == 6u), sign(p.y) * 6.7, 1.5),
            carrier,
        );
        // The fan or turbine turns about the pod's own axis (authored along x) before
        // the pod tilts; the two pods on a side run out of step.
        if (in.rig & RIG_SPIN) != 0u && (e.owner_flags & (FLAG_UNDER_CONSTRUCTION | FLAG_IN_FACTORY | KIND_GHOST)) == 0u {
            let rate = select(30.0, 17.0, carrier);
            let turn = time * rate + f32(e.unit_id & 255u) + select(0.0, 1.3, in.part == 6u);
            let q = p - pivot;
            p = vec3<f32>(q.x, q.y * cos(turn) - q.z * sin(turn), q.y * sin(turn) + q.z * cos(turn)) + pivot;
            n = vec3<f32>(n.x, n.y * cos(turn) - n.z * sin(turn), n.y * sin(turn) + n.z * cos(turn));
        }
        let tilt = select(
            clamp(1.5708 - local_speed * 0.18 - motion.z * 0.06, 0.35, 2.5),
            // Per-tick travel. Cruise speed is a few metres a tick; that lays the
            // nacelles flat. A climb keeps them nearer vertical. No sideslip roll.
            mix(1.5708, 0.08, clamp(local_speed * 0.62 - motion.z * 1.1, 0.0, 1.0)),
            carrier,
        );
        p = rot_xz(p - pivot, tilt) + pivot;
        n = rot_xz(n, tilt);
        if !carrier {
            let roll = clamp(lateral * 0.12, -0.45, 0.45);
            p = rot_x(p - pivot, roll) + pivot;
            n = rot_x(n, roll);
        }
    } else if limb >= LIMB_HOUSE && limb < LIMB_HOUSE + 4u && model.house_weapon[limb - LIMB_HOUSE] > 0.5 && (e._pad3b >> 8u) > 0u {
        // A gun house of its own on the hull (a warship's turret): turns about its pivot by its
        // weapon's yaw off the hull; what recoils inside it pitches about the pivot and kicks back.
        let slot = limb - LIMB_HOUSE;
        let house = model.houses[slot];
        let w = u32(model.house_weapon[slot] + 0.5) - 1u;
        let hp = houses[(e._pad3b >> 8u) - 1u];
        let pose = hp.pose[w];
        let pivot = house.xyz;
        if (in.rig & RIG_RECOIL) != 0u {
            let kicks = hp.kick[w / 2u];
            let kick = select(kicks.xy, kicks.zw, (w & 1u) == 1u);
            // A spacecraft's rotary barrels turn about the bore while that gun is firing
            // (its kick is live between shots); `capital::rotary_house` tags them `rig::SPIN`.
            if (in.rig & RIG_SPIN) != 0u && capital_ship(model) {
                let firing = step(0.001, max(kick.x, kick.y));
                let turn = (time * 24.0 + f32(slot) * 0.9) * firing;
                let q = p - pivot;
                p = vec3<f32>(q.x, q.y * cos(turn) - q.z * sin(turn), q.y * sin(turn) + q.z * cos(turn)) + pivot;
                n = vec3<f32>(n.x, n.y * cos(turn) - n.z * sin(turn), n.y * sin(turn) + n.z * cos(turn));
            }
            p.x -= house.w * mix(kick.x, kick.y, t);
            let pitch = mix(pose.z, pose.w, t);
            p = rot_xz(p - pivot, pitch) + pivot;
            n = rot_xz(n, pitch);
        }
        let yaw = lerp_angle(pose.x, pose.y, t);
        p = rot_z(p - pivot, yaw) + pivot;
        n = rot_z(n, yaw);
    } else if limb == LIMB_MOUNT && any(model.mount.xyz != vec3<f32>(0.0)) {
        // A mount on the hull (a ship's AA gun): kicks back, pitches about its trunnion and
        // turns, like a shoulder gun, but off the hull. The mirror gives its yaw off the first
        // weapon's turret, so that turret's own yaw is added back.
        let pivot = model.mount.xyz;
        if (in.rig & RIG_RECOIL) != 0u {
            p.x -= model.mount.w * mix(e.spin_recoil.z, e.spin_recoil.w, t);
        }
        let pitch = mix(e.mount.z, e.mount.w, t);
        p = rot_xz(p - pivot, pitch) + pivot;
        n = rot_xz(n, pitch);
        let yaw = lerp_angle(e.mount.x + e.prev_turret_yaw, e.mount.y + e.turret_yaw, t);
        p = rot_z(p - pivot, yaw) + pivot;
        n = rot_z(n, yaw);
    } else if (in.part == PART_RAM || in.part == PART_STRING || in.part == PART_FEED)
        && (e.owner_flags & (KIND_WRECK | FLAG_UNDER_CONSTRUCTION)) == 0u {
        // The mirror publishes blows struck and what a tick adds (`mines::hammer_gait`).
        p += pipe_offset(in.part, fract(e.gait.x - e.gait.y * (1.0 - t)), model);
    } else if in.part == PART_HATCH && (e.owner_flags & (KIND_WRECK | KIND_GHOST | FLAG_UNDER_CONSTRUCTION)) == 0u {
        // An airbase's hatch leaves telescope apart under the deck while aircraft come in:
        // the inner ones (resting nearer the middle than half the hatch) run its whole
        // half width, the outer ones half as far. The pit reaches just past the square
        // hatch's corners, so half its width is `(radius - 0.4) / sqrt 2`
        // (`models/aster/airbase.rs`).
        let open = smoothstep(0.0, 1.0, mix(e.prev_deploy, e.deploy, t));
        let half = (model.pit.y - 0.4) * 0.70710678;
        let travel = select(half * 0.5, half, abs(p.y) < half * 0.5);
        p.y += sign(p.y) * open * travel;
    } else if in.part == PART_PUMP && (e.owner_flags & (KIND_WRECK | FLAG_UNDER_CONSTRUCTION | STATE_UNPOWERED)) == 0u {
        // A reactor's pumps and injectors: a short stroke, a quick drive down and a slower
        // draw back, phased by where each stands so they work round the plant in turn.
        let phase = atan2(p.y, p.x) / 6.2831853 + f32(e.unit_id & 255u) * 0.137;
        let beat = fract(time * 0.7 + phase);
        let stroke = clamp(model.height * 0.035, 0.3, 1.1);
        p.z -= stroke * select(1.0 - (beat - 0.25) / 0.75, beat / 0.25, beat < 0.25);
    } else if (in.part == PART_SPINNER || in.part == 4u) && (e.owner_flags & (KIND_WRECK | FLAG_UNDER_CONSTRUCTION | STATE_UNPOWERED)) == 0u {
        let pivot = model.spinner_pivot.xyz;
        let rate = select(select(1.6, 0.48, (e.owner_flags & STATE_CHARGING) != 0u), 38.0, in.part == 4u);
        let spin = time * rate + f32(e.unit_id & 255u);
        p = rot_z(p - pivot, spin) + pivot;
        n = rot_z(n, spin);
    } else if in.part == PART_LOCOMOTION && walks && limb != 0u {
        let posed = walk_leg(p, n, limb, model, walk);
        p = posed[0];
        n = posed[1];
    } else if in.part == PART_LOCOMOTION && !walks && (model.icon & 0x20000u) == 0u
        && (e.owner_flags & FLAG_MOVING) != 0u
        && (in.rig & RIG_DEPLOY) == 0u
        && terrain_height(e.pos.xy) >= globals.map.z - 0.25 {
        // Running gear without a rig: a small shudder. Quiet while floating.
        // Hover skirts are not tracks: they do not crawl or bounce with the belt.
        let phase = time * 9.0 + p.x * 0.8 + sign(p.y) * 1.57;
        p.z += max(sin(phase), 0.0) * 0.12 * model.height * 0.1;
    }
    // Factory deck: up while a hull is printing, down to release it. A refit
    // leaves the deck down so the factory's own guns stay out of the work.
    let producing = (e.owner_flags & FLAG_BUILDING) != 0u && e.upgrade <= 0.0
        && (e.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST | FLAG_UNDER_CONSTRUCTION)) == 0u;
    if (in.rig & RIG_LIFT) != 0u
        && (e.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST | FLAG_UNDER_CONSTRUCTION)) == 0u
    {
        p.z -= select(FACTORY_LIFT_DROP, 0.0, producing);
    }
    if (in.rig & RIG_DEPLOY) != 0u
        && (e.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST | FLAG_UNDER_CONSTRUCTION)) == 0u
    {
        // Authored planted. Packed: side legs fold up along the carriage,
        // the rear spade lifts against the spine. Hinges match the Trebuchet
        // at its authored deck (1.88 m); the mesh may be scaled to a bigger
        // blueprint, so the race scales with the turret pivot height.
        let stow = 1.0 - mix(e.prev_deploy, e.deploy, t);
        let s = select(1.0, model.turret_pivot.z / 1.88, model.turret_pivot.z > 0.5);
        if stow > 0.001 {
            if abs(p.y) > 3.8 * s {
                let hinge = vec3<f32>(-0.3 * s, sign(p.y) * 4.72 * s, 1.38 * s);
                let ang = sign(p.y) * stow * 1.65;
                p = rot_x(p - hinge, ang) + hinge;
                n = rot_x(n, ang);
            } else if p.x < -5.25 * s {
                let hinge = vec3<f32>(-5.2 * s, 0.0, 1.22 * s);
                let ang = stow * 1.35;
                p = rot_y(p - hinge, ang) + hinge;
                n = rot_y(n, ang);
            }
        }
    }
    if walks && in.part != PART_LOCOMOTION {
        p += walk_bob(walk, model);
    }
    // Hovercraft: the hull rides a cushion, the rubber skirt hangs behind it.
    if (model.icon & 0x20000u) != 0u
        && (e.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST | FLAG_UNDER_CONSTRUCTION)) == 0u
    {
        let bob = 0.10 + 0.08 * sin(time * 1.65 + f32(e.unit_id & 255u) * 0.31);
        p.z += select(bob, bob * 0.22, in.part == PART_LOCOMOTION);
    }
    // A hovering aircraft is never quite still: a slow heave on its lift.
    var hover_sway = 0.0;
    if (model.icon & 0x80000u) != 0u && (model.icon & 0x40000u) != 0u
        && (e.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST | FLAG_UNDER_CONSTRUCTION | FLAG_IN_FACTORY)) == 0u
    {
        let seed = f32(e.unit_id & 255u) * 0.37;
        // A spacecraft stands still on its legs: the heave fades out as the gear comes
        // down, and a capital hull only rolls a hair in flight.
        let transport = capital_ship(model);
        let calm = select(1.0, 1.0 - smoothstep(0.3, 0.9, capital_gear(capital_height(e, t))), transport);
        p.z += (0.09 * sin(time * 1.15 + seed) + 0.04 * sin(time * 2.9 + seed * 1.7)) * calm;
        hover_sway = select(0.012, 0.003, transport) * sin(time * 0.8 + seed) * calm;
    }

    // A spacecraft settles on its shock struts (`capital_sink`): everything but the struts
    // and feet goes down, so the feet stay planted and the struts slide into the legs.
    if model.capital[0].w != 0.0 && in.part != 20u && in.part != 21u
        && (e.owner_flags & (KIND_WRECK | KIND_GHOST)) == 0u {
        p.z -= capital_sink(model, e, t, capital_height(e, t));
    }
    let heading = lerp_angle(e.prev_heading, e.heading, t);
    var origin = mix(e.prev_pos, e.pos, t);
    if (e.owner_flags & KIND_PROP) != 0u {
        // Props carry an approximate height; stand them on the real surface.
        origin.z = terrain_height(origin.xy) - e.arm_pitch.z;
    }
    // Afloat, a float skirt is the hull's sides and the treads draw in behind
    // it; on land the skirt folds in between the tracks. Depth is how far the
    // ground under the hull sits below the water line. A hover bag stays put.
    if (e.owner_flags & (KIND_WRECK | KIND_PROP)) == 0u && in.part == PART_LOCOMOTION && !walks
        && (model.icon & 0x20000u) == 0u {
        let depth = globals.map.z - terrain_height(origin.xy);
        let floating = smoothstep(0.15, 1.5, depth);
        if (in.rig & RIG_FLOAT) != 0u {
            p.z = mix(p.z * 0.6 + 0.15, p.z - 0.25, floating);
            p.y *= mix(0.55, 1.0, floating);
        } else if floating > 0.0 {
            p.y *= 1.0 - floating * 0.16;
        }
    }
    var local = p * scale;

    // Mobile units lean with the ground under them; a ship rides the water, not the seabed.
    var up = vec3<f32>(0.0, 0.0, 1.0);
    if (model.icon & 0x10000u) != 0u && (model.icon & 0x40000u) == 0u && (model.icon & 0x800000u) == 0u {
        up = terrain_normal(origin.xy, max(e.radius, 4.0));
        // On a lift ship's ramp or hold floor it leans with the deck (`mirror::UNIT_ON_DECK`).
        if (e._pad3a & 0x1000000u) != 0u {
            let d = vec2<f32>(f32(i32(e._pad3c << 16u) >> 16u), f32(i32(e._pad3c) >> 16u)) / 32767.0;
            up = vec3<f32>(d, sqrt(max(1.0 - dot(d, d), 0.0)));
        }
    }
    let fwd0 = vec3<f32>(cos(heading), sin(heading), 0.0);
    var left = normalize(cross(up, fwd0));
    var fwd = cross(left, up);
    if (model.icon & 0x40000u) != 0u
        && (e.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST | FLAG_UNDER_CONSTRUCTION | FLAG_IN_FACTORY)) == 0u {
        // A climb pitches the fuselage; hovering over sloping ground stays level.
        let travel = e.pos - e.prev_pos;
        var pitch = clamp(atan2(travel.z, max(length(travel.xy), 2.0)), -0.20, 0.20);
        if (model.icon & 0x80000u) != 0u {
            pitch = -clamp(dot(travel.xy, fwd0.xy) * 0.035, -0.12, 0.12);
        }
        if (model.icon & 0x1100000u) != 0u {
            pitch = mix(e.arm_pitch.x, e.arm_pitch.y, t);
        }
        let pitch_fwd = fwd * cos(pitch) + up * sin(pitch);
        up = up * cos(pitch) - fwd * sin(pitch);
        fwd = pitch_fwd;
    }
    // A ship rides the swell: a slow heave, pitch and roll, bigger on a small boat than on a
    // frigate, out of step from hull to hull. A fast boat trims bow-up, a diving hull takes
    // the bow down, a planing boat leans into its turns (a big hull a little out of them).
    // Dived, it is still.
    var swell_roll = 0.0;
    if (model.icon & 0x800000u) != 0u
        && (e.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST | FLAG_UNDER_CONSTRUCTION | FLAG_IN_FACTORY)) == 0u {
        let k = clamp(7.0 / max(e.radius, 1.0), 0.35, 1.2);
        let afloat = 1.0 - smoothstep(0.3, 1.8, globals.map.z - origin.z);
        let w = 1.3 * sqrt(k);
        let phase = dot(origin.xy, vec2<f32>(0.013, 0.021)) + f32(e.unit_id % 61u) * 0.7;
        let s1 = sin(time * w + phase);
        let s2 = sin(time * w * 0.71 + phase * 1.7 + 1.3);
        let s3 = sin(time * w * 1.23 + phase * 0.6 + 2.1);
        let travel = e.pos - e.prev_pos;
        let ahead = dot(travel.xy, fwd0.xy);
        let turn = lerp_angle(0.0, e.heading - e.prev_heading, 1.0);
        let pitch = (0.035 * s2 + 0.018 * s3) * k * afloat
            + clamp(ahead * 0.012 * k, 0.0, 0.07)
            + clamp(travel.z * 1.5, -0.14, 0.14);
        let pitch_fwd = fwd * cos(pitch) + up * sin(pitch);
        up = up * cos(pitch) - fwd * sin(pitch);
        fwd = pitch_fwd;
        swell_roll = (0.05 * s3 + 0.03 * s1) * k * afloat - clamp(turn * ahead * (k - 0.6) * 0.12, -0.08, 0.08);
        origin.z += (0.22 * s1 + 0.08 * s3) * k * afloat;
        // A big hull heels outward in a turn, visibly and slowly: its turn is slow, so the
        // heel comes on and goes off over seconds with it.
        let big = smoothstep(20.0, 50.0, e.radius);
        swell_roll += clamp(turn * ahead * 0.9, -0.05, 0.05) * big * afloat;
        // Guns on houses of their own kick the hull: a broadside heels it away from the
        // side it fired to and shoves it a little sideways. The heel rises over the first
        // quarter of the recoil's run and settles through the rest, a turret at a time.
        if (e._pad3b >> 8u) > 0u && (model.icon & 0x800000u) != 0u {
            let hp = houses[(e._pad3b >> 8u) - 1u];
            var heel = 0.0;
            for (var slot = 0u; slot < 4u; slot++) {
                if model.house_weapon[slot] < 0.5 {
                    continue;
                }
                let w = u32(model.house_weapon[slot] + 0.5) - 1u;
                let kicks = hp.kick[w / 2u];
                let kick = select(kicks.xy, kicks.zw, (w & 1u) == 1u);
                let kk = mix(kick.x, kick.y, t);
                if kk <= 0.001 {
                    continue;
                }
                // The kick is a cubic ease back home; its cube root is how much is left.
                let done = 1.0 - pow(kk, 1.0 / 3.0);
                let shape = smoothstep(0.0, 0.25, done) * pow(1.0 - done, 1.5);
                let yaw = lerp_angle(hp.pose[w].x, hp.pose[w].y, t);
                heel += shape * sin(yaw);
            }
            // A battleship heels two or three degrees to a full broadside; a destroyer less.
            heel = clamp(heel, -2.5, 2.5);
            swell_roll += heel * (0.008 + 0.014 * big) * afloat;
            origin -= left * heel * (0.08 + 0.35 * big) * afloat;
        }
    }
    // A tracked hull afloat (riding the surface, not crawling the seabed) bobs
    // and rolls a little, like a small boat. Ships and hovercraft have their own.
    if (model.icon & 0x10000u) != 0u && (model.icon & (0x20000u | 0x40000u | 0x800000u)) == 0u && !walks
        && (e.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST | FLAG_UNDER_CONSTRUCTION | FLAG_IN_FACTORY)) == 0u {
        let afloat = smoothstep(0.15, 1.5, min(origin.z, globals.map.z) - terrain_height(origin.xy));
        if afloat > 0.0 {
            let phase = dot(origin.xy, vec2<f32>(0.013, 0.021)) + f32(e.unit_id % 61u) * 0.7;
            let s1 = sin(time * 1.5 + phase);
            let s2 = sin(time * 1.1 + phase * 1.7 + 1.3);
            origin.z += (0.07 * s1 - 0.08) * afloat;
            swell_roll += 0.035 * s2 * afloat;
            let pitch = 0.02 * sin(time * 1.3 + phase * 0.6 + 2.1) * afloat;
            let pitch_fwd = fwd * cos(pitch) + up * sin(pitch);
            up = up * cos(pitch) - fwd * sin(pitch);
            fwd = pitch_fwd;
        }
    }
    if falling || toppled {
        let pitch = mix(e.arm_pitch.x, e.arm_pitch.y, t);
        let pitch_fwd = fwd * cos(pitch) + up * sin(pitch);
        up = up * cos(pitch) - fwd * sin(pitch);
        fwd = pitch_fwd;
    }
    // The sim publishes eased roll in the existing instance padding. Air
    // models skip terrain lean, so this rotates in free air.
    let bank = mix(e._pad2.x, e._pad2.y, t) + swell_roll + hover_sway;
    let bank_left = left * cos(bank) + up * sin(bank);
    up = up * cos(bank) - left * sin(bank);
    left = bank_left;
    var world = origin + fwd * local.x + left * local.y + up * local.z;
    var world_n = normalize(fwd * n.x + left * n.y + up * n.z);
    // A standing tree bends over its foot with the wind and away from blasts
    // (`tree_air`): the top goes furthest, the foot not at all, and it keeps
    // its length, so the top dips a little as it goes over.
    if is_tree {
        let tall = max(model.height * scale, 1.0);
        let rise = clamp((world.z - origin.z) / tall, 0.0, 1.3);
        let lean = tree.xy;
        let off = lean * rise * rise;
        world += vec3<f32>(off, -dot(lean, lean) * rise * rise * rise / (2.0 * tall));
        world_n = normalize(world_n + vec3<f32>(lean * (rise * 2.0 / tall), 0.0));
    }
    var hull = -1;

    // A hull field is the posed mesh, pushed out along its skin — not a bubble.
    if push.pass_kind == 2u {
        hull = find_hull_shield(e.unit_id);
        if hull < 0 || (e.owner_flags & (KIND_WRECK | KIND_GHOST | KIND_PROP | FLAG_UNDER_CONSTRUCTION)) != 0u {
            var hidden: VsOut;
            hidden.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
            return hidden;
        }
        // Along the shared shell direction, plus a little away from the middle so a
        // sheet whose two sides cancel still stands off its own surface. Thin: the
        // field is a skin on the plates, and a sharp corner is not let balloon.
        let stand = 0.14 + model.height * 0.003;
        let radial = normalize(world - origin + vec3<f32>(0.0, 0.0, 0.002));
        world += world_n * (stand * min(shell_stretch, 1.6)) + radial * (stand * 0.15);
        world_n = normalize(world_n + radial * 0.15);
    }

    var out: VsOut;
    if push.pass_kind == 1u {
        out.clip = globals.shadow_view_proj * vec4<f32>(world, 1.0);
    } else {
        out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    }
    // Down a pit (`ModelInfo::pit`) the terrain is drawn across the hole. What is inside and
    // below the opening is pulled up in depth to just behind where the eye's ray crosses the
    // opening, keeping its own order, so it shows through the hole. Both depths are linear
    // across a flat face, so this holds across triangles, not only at corners. Where that ray
    // crosses outside the opening, what it reaches is behind solid rock: it keeps its true
    // depth and the terrain hides it. The change-over lies under the lip and the slab, which
    // stand over the opening's plane and hide either depth, and the pit's pieces are short
    // enough that no triangle spans much of it.
    if push.pass_kind == 0u && model.pit.y > 0.0 && !rig_afloat && p.z < model.pit.x && dot(p.xy, p.xy) < model.pit.y * model.pit.y {
        let eye = globals.camera.xyz;
        let open = origin.z + model.pit.x * scale;
        let k = (open - eye.z) / (world.z - eye.z);
        if k > 0.0 && k < 1.0 {
            let crossing = eye + (world - eye) * k;
            let mouth = globals.view_proj * vec4<f32>(crossing, 1.0);
            let at_mouth = mouth.z / mouth.w;
            let at_true = out.clip.z / out.clip.w;
            let outside = length(crossing.xy - origin.xy) - model.pit.y * scale;
            let seen = 1.0 - smoothstep(0.0, model.pit.y * scale * 0.2, outside);
            // An airbase's shaft (icon 20) is wide and deep for how little ground lies under
            // its opening: squeezed as hard as the mine's bore, its far side went behind it.
            let squeeze = select(PIT_SQUEEZE, AIRBASE_SQUEEZE, (model.icon & 0xFFu) == 20u);
            out.clip.z = mix(at_true, at_mouth - (at_mouth - at_true) * squeeze, seen) * out.clip.w;
        }
    }
    // An aircraft going down an airbase's shaft (`mirror::UNIT_IN_SHAFT`) is seen through
    // the open hatch the same way: what is below the opening and whose eye ray crosses the
    // opening inside the shaft is drawn at the opening's depth. `_pad3c` is the shaft's
    // middle from the aircraft, decimetres.
    if push.pass_kind == 0u && (e._pad3a & 0x400u) != 0u {
        let d = vec2<f32>(f32(i32(e._pad3c << 16u) >> 16u), f32(i32(e._pad3c) >> 16u)) * 0.1;
        let middle = origin.xy + d;
        let open = terrain_height(middle) + AIRBASE_OPEN;
        let eye = globals.camera.xyz;
        if world.z < open && eye.z > open {
            let k = (open - eye.z) / (world.z - eye.z);
            let crossing = eye + (world - eye) * k;
            let mouth = globals.view_proj * vec4<f32>(crossing, 1.0);
            let at_mouth = mouth.z / mouth.w;
            let at_true = out.clip.z / out.clip.w;
            let off = abs(crossing.xy - middle);
            let outside = max(off.x, off.y) - AIRBASE_SHAFT;
            let seen = 1.0 - smoothstep(0.0, 1.0, outside);
            out.clip.z = mix(at_true, at_mouth - (at_mouth - at_true) * AIRBASE_SQUEEZE, seen) * out.clip.w;
        }
    }
    out.world = world;
    out.normal = world_n;
    if in.material == MAT_TREAD && (model.icon & 0x20000u) == 0u {
        out.uv = vec2<f32>(tread_along(in.pos, in.normal), in.uv.y);
    } else {
        out.uv = in.uv;
    }
    out.material = select(in.material, u32(hull), push.pass_kind == 2u);
    out.owner_flags = e.owner_flags;
    out.state = vec4<f32>(e.build, select(e.health, 2.0, falling), in.pos.z / max(model.height, 0.1), hash11(f32(e.unit_id & 0xFFFFu)));
    out.local = in.pos;
    // Tech in the low byte, then mobile (bit 8) and hover (bit 9) from the icon flags.
    // Naval (icon bit 23) rides in bit 11.
    out.model_class = ((model.icon >> 8u) & 0xFFu) | ((model.icon >> 8u) & 0x300u) | ((model.icon >> 12u) & 0x800u)
        | ((in.surface & 0xFFFFu) << 16u)
        | select(0u, 0x1000u, (model.icon & 0x1000000u) != 0u)
        // Bit 10: printed by a replicator (`mirror::UNIT_REPLICATING` in `_pad3[1]`).
        | select(0u, 0x400u, (e._pad3b & 1u) != 0u);
    out.face = in.face;
    out.unit_id = e.unit_id;
    // A spacecraft's drives burn with its speed over the ground; its lift jets with its
    // climb or descent. The glow on each knows its drive's or jet's mouth.
    out.drive = vec4<f32>(-1.0, 0.0, 0.0, 0.0);
    out.drive_at = vec4<f32>(0.0);
    let drives = model.capital[4];
    let jets = model.capital[5];
    if drives.x != 0.0 || any(jets != vec4<f32>(0.0)) {
        let motion = e.pos - e.prev_pos;
        let size = max(model.capital[6].y, 0.01);
        out.drive = vec4<f32>(clamp(length(motion.xy) / 6.0, 0.0, 1.0), clamp(abs(motion.z) / 2.5, 0.0, 1.0), 16.0 * size, size);
        let l = in.pos;
        if drives.x != 0.0 && l.x < drives.x + 24.0 * size {
            let cy = sign(l.y) * select(drives.z, drives.w, abs(l.y) > 0.5 * (drives.z + drives.w));
            out.drive_at = vec4<f32>(drives.x, cy, drives.y, 1.0);
        } else if any(jets != vec4<f32>(0.0)) {
            let j = select(jets.zw, jets.xy, l.x > 0.5 * (jets.x + jets.z));
            let jz = model.capital[6].x;
            if length(vec2<f32>(l.x - j.x, abs(l.y) - j.y)) < 7.0 && l.z > jz && l.z < jz + 4.0 {
                out.drive_at = vec4<f32>(j.x, sign(l.y) * j.y, jz, 2.0);
            }
        }
    }
    out.dust = select(0.62, model.surface.y / max(model.height, 0.1), model.surface.y > 0.0);
    // Pieces go up one after another over the first four fifths of the refit, each taking a fifth.
    // What is coming off turns to a hologram at the start.
    let at = f32((in.rig >> 16u) & 0xFFu) / 255.0;
    var begins = at * 0.8;
    if arriving {
        // A module's own pieces wait for what they replace to go.
        begins = LEAVE_BY + at * 0.65;
    }
    var built = clamp((e.upgrade - begins) / 0.2, 0.0, 1.0);
    if leaving {
        built = 1.0 - clamp(e.upgrade / (LEAVE_BY * 0.6), 0.0, 1.0);
    }
    out.refit = vec3<f32>(select(0.0, 1.0, piece), built, e.upgrade);
    // Sized by the body, not by how far a raised gun reaches (`Model::surface_reach`).
    let reach = max(select(model.bounds_radius, model.surface.x, model.surface.x > 0.0), 1.0);
    let height = max(model.height, 1.0);
    out.weld = vec4<f32>(f32(e.weld_first), f32(e.weld_count), reach, height);
    if push.pass_kind == 2u {
        // The wrap has no welds. x carries where its emitter sits: the top of
        // the hull over the model's middle, from the baked plan.
        out.weld.x = hull_crown(e.blueprint, model.height);
    }
    return out;
}

// Soot and scorch from the model position: several incommensurate scales so
// the noise tile never marches across a hull, plus streaks that climb with
// height the way fire does.
fn wreck_burn(local: vec3<f32>, seed: f32) -> vec2<f32> {
    let field = local.xy + vec2<f32>(local.z * 0.53, local.z * 0.29);
    let off = vec2<f32>(seed * 47.0, seed * 19.0);
    let at = field + off;
    let coarse = noise_varied(at, 28.0).ba;
    let mid = noise_varied(at, 11.0).ba;
    let fine = textureSample(noise_map, repeat_sampler, field * 0.21 + off * 0.03).ba;
    let climb = textureSample(noise_map, repeat_sampler, vec2<f32>(field.x * 0.08, local.z * 0.19) + off * 0.02).a;
    let blotch = value_noise2(at, 7.0);
    let soot = mix(coarse, mid, 0.55);
    return vec2<f32>(
        clamp(soot.x * 0.5 + fine.x * 0.22 + climb * 0.18 + blotch * 0.2, 0.0, 1.0),
        clamp(soot.y * 0.55 + fine.y * 0.25 + climb * 0.2, 0.0, 1.0)
    );
}

// Which parts of the hull print first. A function of the model point only,
// so a new weld never rewrites what is already up, and the site fills in
// all over rather than as a sphere from the beam.
fn print_order(local: vec3<f32>, seed: f32) -> f32 {
    let id = floor(local / 2.2);
    let chunk = hash21(id.xy + vec2<f32>(id.z * 3.1, seed * 17.0));
    let edge = value_noise2(local.xy + vec2<f32>(local.z * 0.37, seed * 9.0), 1.15);
    return clamp(chunk * 0.72 + edge * 0.28, 0.0, 1.0);
}

// Expanding fronts of work light from a print origin. `weld.w` is how far a
// front has to travel to cover the hull; `fade` dims it after the beam goes off.
fn weld_wave(local: vec3<f32>, weld: vec4<f32>, time: f32, seed: f32, fade: f32) -> f32 {
    if fade <= 0.001 || dot(weld.xyz, weld.xyz) < 0.25 {
        return 0.0;
    }
    let d = distance(local, weld.xyz);
    let t = time * 0.42 + seed * 0.15;
    let a = abs(d - fract(t) * weld.w);
    let b = abs(d - fract(t + 0.5) * weld.w);
    return (1.0 - smoothstep(0.0, 3.4, min(a, b))) * fade;
}

fn weld_reach(origin: vec3<f32>, reach: f32, height: f32) -> f32 {
    let far = vec3<f32>(
        select(reach, -reach, origin.x > 0.0),
        select(reach, -reach, origin.y > 0.0),
        select(0.0, height, origin.z > height * 0.5),
    );
    return max(distance(origin, far), 1.0);
}

fn site_waves(local: vec3<f32>, weld: vec4<f32>, time: f32, seed: f32) -> vec2<f32> {
    var wave = 0.0;
    var working = 0.0;
    let n = arrayLength(&welds);
    let count = min(u32(weld.y), 12u);
    let first = u32(weld.x);
    for (var i = 0u; i < count; i++) {
        if first + i >= n {
            break;
        }
        let rec = welds[first + i].pos;
        let origin = rec.xyz;
        let fade = rec.w;
        if fade > 0.95 {
            working = 1.0;
        }
        let reach = weld_reach(origin, weld.z, weld.w);
        let side = seed + origin.x * 0.13 + origin.y * 0.27;
        wave = max(wave, weld_wave(local, vec4<f32>(origin, reach), time, side, fade));
    }
    return vec2<f32>(wave, working);
}

// ---- Trees in the air (renderer/tree_wind.rs) ----------------------------

// How sheltered `p` is by a live shield: 1 inside a dome or hull field, easing
// to 0 across its last few metres. Air under a shield is still.
fn shield_shelter(p: vec3<f32>) -> f32 {
    var s = 0.0;
    for (var i = 0u; i < effect_barriers.header.x; i++) {
        let b = effect_barriers.entries[i];
        if p.z < b.min_z - 0.1 { continue; }
        let q = length((p - b.center) * b.inverse_axes);
        s = max(s, 1.0 - smoothstep(1.0 - 4.0 / max(b.radius, 4.5), 1.0, q));
    }
    return s;
}

// The air at a tree's foot, m/s: the prevailing wind slowed near the ground,
// plus whatever the weather has stirred up here (aircraft, blasts, storms).
fn ground_air(xy: vec2<f32>) -> vec2<f32> {
    return (atmos.wind.zw + flow_at(xy).xy) * 0.5;
}

// Gusts: patches of stronger air rolling over the ground downwind, 0-1.
fn gust_at(xy: vec2<f32>) -> f32 {
    let n = textureSampleLevel(noise_map, repeat_sampler, tile_uv(xy - atmos.wind.xy * 1.4, 170.0), 0.0).g;
    return smoothstep(0.3, 0.8, n);
}

// Where the top of a tree `tall` metres high standing at `foot` is pushed to
// sideways, metres (xy), and how hard its leaves are shaken (z, 0 in still air
// to 1 in a gale or a blast). The wind leans it over and rocks it at its own pace (a big
// tree is stiffer and slower), the rocking rolling across a wood downwind in
// gusts. A blast's push runs out through the trees at FRONT_SPEED, shoves each
// away and lets it swing back and settle. Shields stop both.
fn tree_air(foot: vec3<f32>, tall: f32, id: u32) -> vec3<f32> {
    let time = globals.camera.w;
    let seed = f32(id % 97u);
    let mid = foot + vec3<f32>(0.0, 0.0, tall * 0.5);
    let give = inverseSqrt(max(tall / 10.0, 0.4));
    let swing = 1.3 * give + 0.5;
    var lean = vec2<f32>(0.0);
    var stir = 0.0;

    let still = 1.0 - shield_shelter(mid);
    let air = ground_air(foot.xy);
    let speed = length(air);
    if speed > 0.05 && still > 0.0 {
        let dir = air / speed;
        let gust = gust_at(foot.xy);
        let storm = clamp(weather_at(foot.xy).y, 0.0, 1.0);
        let steady = min(tall * 0.011 * speed * (0.45 + 0.9 * gust + 0.5 * storm) * give, tall * 0.2);
        let phase = time * swing - dot(foot.xy, dir) * 0.06 + seed * 0.37;
        let rock = sin(phase) * (0.2 + 0.45 * gust) + sin(phase * 2.31 + seed) * 0.08;
        let side = sin(time * swing * 1.37 + seed * 1.3) * 0.12;
        lean += (dir * (steady * (1.0 + rock)) + vec2<f32>(-dir.y, dir.x) * steady * side) * still;
        stir = clamp(speed * (0.6 + 0.8 * gust) / 9.0, 0.0, 1.0) * still;
    }

    for (var i = 0u; i < u32(globals.tree_wind.x); i++) {
        let b = globals.tree_blasts[i * 2u];
        let range = globals.tree_blasts[i * 2u + 1u].x;
        let d = distance(b.xyz, mid);
        if d >= range { continue; }
        let since = time - b.w - d / 140.0; // tree_wind::FRONT_SPEED
        if since <= 0.0 || since > 3.5 { continue; }
        if effect_blocked(b.xyz, mid) { continue; }
        let away = foot.xy - b.xy;
        let dir = select(vec2<f32>(1.0, 0.0), away / max(length(away), 0.001), dot(away, away) > 0.01);
        let near = 1.0 - d / range;
        let force = globals.tree_blasts[i * 2u + 1u].y * near * near * give * (tall / 10.0);
        // Thrown over in a fifth of a second, back through upright, settling.
        let shape = (1.0 - exp(-since * 14.0)) * exp(-since * 1.9) * cos(since * swing * 2.4);
        lean += dir * min(force, tall * 0.35) * shape;
        stir = max(stir, min(force / tall * 4.0, 1.0) * exp(-since * 1.2));
    }
    return vec3<f32>(lean, stir);
}

// Tree layers after FOLIAGE_BASE (foliage.rs): leaf atlases (linear albedo,
// cutout coverage), then bark as albedo/roughness + normal/occlusion pairs.
const FOLIAGE_BROADLEAF: i32 = 0;
const FOLIAGE_CONIFER: i32 = 1;
const FOLIAGE_BARK: i32 = 2;
const FOLIAGE_PINE_BARK: i32 = 4;
// A leaf card's tag (its face random byte): bit 7 picks the conifer atlas, the
// rest is a per-card random. Bark faces with the PLAIN pattern are pine bark.
const LEAF_CONIFER: u32 = 0x80u;
const BARK_PINE_PATTERN: u32 = 1u;

fn leaf_tag(in: VsOut) -> u32 {
    return (in.model_class >> 24u) & 0xFFu;
}

fn foliage_sample(in: VsOut) -> vec4<f32> {
    let layer = FOLIAGE_BASE + select(FOLIAGE_BROADLEAF, FOLIAGE_CONIFER, (leaf_tag(in) & LEAF_CONIFER) != 0u);
    return textureSample(terrain_materials, clamp_sampler, in.uv, layer);
}

fn foliage_missing(in: VsOut, leaf: vec4<f32>) -> bool {
    let charred = 1.0 - clamp(in.state.y, 0.0, 1.0);
    // Burn away small leaf groups to expose twigs, rather than turning a
    // solid green polyhedron into a solid black polyhedron.
    let group = hash21(floor(in.uv * 17.0) + floor(in.local.xy * 2.0));
    return leaf.a < 0.38 || group < charred * 0.90;
}

// A tree coming apart in a lot-clearing field (renderer/clearing.rs) carries
// `health` from one to two. It goes in chunks, the crown first and the foot
// last: above zero this chunk is gone, just under zero it is glowing. Far
// below zero for everything else.
fn vapor_edge(in: VsOut) -> f32 {
    if (in.owner_flags & KIND_PROP) == 0u || in.state.y <= 1.0 {
        return -9.0;
    }
    let v = clamp(in.state.y - 1.0, 0.0, 1.0);
    let cell = floor(in.local * 1.7);
    let r = fract(sin(dot(cell, vec3<f32>(12.9898, 78.233, 37.719))) * 43758.547);
    let order = r * 0.5 + (1.0 - clamp(in.state.z, 0.0, 1.0)) * 0.5;
    return v * 1.15 - order;
}

@fragment
fn fs_shadow(in: VsOut) {
    if in.material == MAT_FOLIAGE && foliage_missing(in, foliage_sample(in)) { discard; }
    if vapor_edge(in) > 0.0 { discard; }
    // Unbuilt parts of a construction site cast no shadow.
    if (in.owner_flags & FLAG_UNDER_CONSTRUCTION) != 0u {
        let grow = clamp(in.state.x / 0.74, 0.0, 1.0);
        if print_order(in.local, in.state.w) > grow {
            discard;
        }
    }
    // Nor does an upgrade piece that is still a hologram.
    if in.refit.x > 0.5 && in.refit.y <= 0.0 {
        discard;
    }
}

fn material_of(id: u32, owner: u32) -> Pbr {
    var m: Pbr;
    m.emissive = vec3<f32>(0.0);
    m.metallic = 0.0;
    m.roughness = 0.6;
    m.albedo = vec3<f32>(0.5);
    switch id {
        case 0u: { m.albedo = globals.plating.rgb; m.metallic = 0.3; m.roughness = 0.6; }
        case 1u: { m.albedo = globals.accent.rgb; m.metallic = 0.7; m.roughness = 0.42; }
        case 2u: { m.albedo = globals.glow.rgb * 0.2; m.emissive = globals.glow.rgb * 5.0; m.roughness = 0.3; }
        case 3u: { m.albedo = globals.team_colors[owner & 7u].rgb * TEAM_PAINT; m.metallic = 0.3; m.roughness = 0.4; m.emissive = globals.team_colors[owner & 7u].rgb * TEAM_GLOW; }
        case 4u: { m.albedo = vec3<f32>(0.32, 0.33, 0.36); m.metallic = 0.95; m.roughness = 0.32; }
        case 5u: { m.albedo = vec3<f32>(0.02, 0.05, 0.09); m.metallic = 0.9; m.roughness = 0.08; m.emissive = globals.glow.rgb * 0.15; }
        case 6u: { m.albedo = vec3<f32>(0.03, 0.03, 0.035); m.roughness = 0.9; }
        case 7u: { m.albedo = vec3<f32>(0.3, 0.12, 0.02); m.emissive = vec3<f32>(1.0, 0.42, 0.08) * 4.0; m.roughness = 0.3; }
        case 8u: { m.albedo = vec3<f32>(0.16, 0.11, 0.07); m.roughness = 0.95; }
        case 9u: { m.albedo = vec3<f32>(0.07, 0.16, 0.05); m.roughness = 0.85; }
        case 10u: { m.albedo = vec3<f32>(0.3, 0.29, 0.27); m.roughness = 0.9; }
        case 11u: { m.albedo = vec3<f32>(0.42, 0.42, 0.41); m.roughness = 0.85; }
        case 12u: { m.albedo = vec3<f32>(0.05, 0.07, 0.1); m.metallic = 0.8; m.roughness = 0.15; m.emissive = vec3<f32>(0.9, 0.75, 0.45) * 0.35; }
        case 13u: { m.albedo = vec3<f32>(0.3, 0.17, 0.02); m.emissive = AMBER * 4.0; m.roughness = 0.3; }
        case 14u: { m.albedo = globals.plating.rgb * vec3<f32>(0.3, 0.33, 0.39); m.metallic = 0.45; m.roughness = 0.42; }
        case 15u: { m.albedo = vec3<f32>(0.28, 0.03, 0.03); m.emissive = vec3<f32>(1.0, 0.08, 0.06) * 5.0; m.roughness = 0.28; }
        case 16u: { m.albedo = vec3<f32>(0.16, 0.06, 0.3); m.emissive = vec3<f32>(0.52, 0.2, 1.0) * 5.5; m.roughness = 0.25; }
        default: {}
    }
    return m;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let flags = in.owner_flags;
    let owner = flags & 0xFFu;
    let time = globals.camera.w;
    var m = material_of(in.material, owner);
    if in.material == MAT_FOLIAGE {
        let leaf = foliage_sample(in);
        if foliage_missing(in, leaf) { discard; }
        m.albedo = leaf.rgb;
        m.roughness = 0.8;
    }
    var n = normalize(in.normal);
    // A wreck's mesh is warped in the vertex shader, so its faces' normals come from the surface itself.
    let face = cross(dpdx(in.world), dpdy(in.world));
    if (flags & KIND_WRECK) != 0u && dot(face, face) > 1e-16 {
        let facing = normalize(face);
        n = facing * select(-1.0, 1.0, dot(facing, globals.camera.xyz - in.world) > 0.0);
    }
    let eye = globals.camera.xyz;
    let v = normalize(eye - in.world);
    let dist = distance(eye, in.world);

    // Plating is textured from each face's own shape (surface.wgsl): outlines, fitted
    // plates and rivets, lights in the black, the patterns a model asks for, and burns
    // as it is hurt. Armour only: a gunmetal tube is left a plain tube. Wrecks skip
    // it: the mesh is crumpled, and they are burnt out all over further down.
    var soot = 0.0;
    var lights = vec3<f32>(0.0);
    if ((in.material < MAT_METAL && in.material != MAT_GLOW) || in.material == MAT_PLATING_DARK)
        && (flags & (KIND_WRECK | KIND_GHOST)) == 0u {
        var si: SurfaceIn;
        si.st = in.face.xy;
        si.half = abs(in.face.zw);
        si.wraps = in.face.z < 0.0;
        si.dark = in.material == MAT_ACCENT;
        si.pattern = (in.model_class >> 16u) & 0xFFu;
        si.seed = f32((in.model_class >> 24u) & 0xFFu) / 255.0;
        si.unit = in.state.w;
        si.unit_id = in.unit_id;
        si.scale = clamp(0.55 * pow(in.weld.z, 0.6), 0.7, 6.0);
        si.time = time;
        si.working = select(0.0, 1.0, (flags & FLAG_BUILDING) != 0u && in.refit.z <= 0.0
            && (flags & (FLAG_UNDER_CONSTRUCTION | STATE_UNPOWERED)) == 0u);
        si.tech = f32(max(in.model_class & 0xFFu, 1u));
        si.lit = select(1.0, 0.0, si.tech < 1.5 && (in.model_class & 0x100u) != 0u);
        si.mobile = select(0.0, 1.0, (in.model_class & 0x100u) != 0u);
        si.health = select(in.state.y, 1.0, (flags & (KIND_PROP | FLAG_UNDER_CONSTRUCTION)) != 0u);
        si.local = in.local;
        si.reach = in.weld.z;
        si.height = in.weld.w;
        // The face's axes in the world, from screen-space derivatives: no tangents in the mesh.
        let dp1 = dpdx(in.world);
        let dp2 = dpdy(in.world);
        let ds1 = dpdx(in.face.xy);
        let ds2 = dpdy(in.face.xy);
        // From the world, not the face: a face with no frame still needs its noise filtered.
        si.px = max(length(dp1), length(dp2));
        let sf = surface_at(si);
        let dp2perp = cross(dp2, n);
        let dp1perp = cross(n, dp1);
        let tangent = dp2perp * ds1.x + dp1perp * ds2.x;
        let bitangent = dp2perp * ds1.y + dp1perp * ds2.y;
        let ts = tangent * inverseSqrt(max(dot(tangent, tangent), 1e-12));
        let bs = bitangent * inverseSqrt(max(dot(bitangent, bitangent), 1e-12));
        n = normalize(n - ts * sf.slope.x - bs * sf.slope.y);

        m.albedo = mix(m.albedo, sf.paint.rgb, sf.paint.a);
        let team_rgb = globals.team_colors[owner & 7u].rgb;
        m.albedo = mix(m.albedo, team_rgb * TEAM_PAINT, sf.team);
        m.emissive = mix(m.emissive, team_rgb * TEAM_GLOW, sf.team);
        m.albedo *= sf.cavity;
        // Bare steel where paint is scuffed or burnt off.
        m.albedo = mix(m.albedo, vec3<f32>(0.2, 0.19, 0.185), sf.bare);
        m.metallic = mix(m.metallic, 0.85, sf.bare);
        m.roughness = clamp(mix(m.roughness + sf.rough, 0.5, sf.bare), 0.05, 1.0);
        lights = sf.emissive;
        soot = sf.soot;
    }
    // Mineral props share the terrain's rock texture and correctly oriented normals.
    if in.material == 10u {
        let rock = terrain_surface(in.world, n, 7.3, 0, 0.8);
        n = rock.normal;
        m.albedo = rock.color * (0.75 + rock.ao * 0.25);
        m.roughness = rock.roughness;
    }
    // Trees. Leaves: the crown's normal (from the mesh) bent a little toward the
    // card's own facing, colour varied per tree (state.w) and per card, and the
    // crown's heart and underside darkened (face.w). The light through the
    // leaves is added after the sun below.
    var leaf_occlusion = 1.0;
    var leaf_through = vec3<f32>(0.0);
    if in.material == MAT_FOLIAGE {
        let tag = leaf_tag(in);
        let card = f32(tag & 0x7Fu) / 127.0;
        let tree = in.state.w;
        var facing = n;
        let facet = cross(dpdx(in.world), dpdy(in.world));
        if dot(facet, facet) > 1e-16 {
            facing = normalize(facet);
            facing *= select(-1.0, 1.0, dot(facing, v) > 0.0);
        }
        n = normalize(n * 0.78 + facing * 0.22);
        let hue = tree - 0.5;
        m.albedo *= (0.8 + 0.4 * tree) * (0.86 + 0.28 * card);
        m.albedo *= vec3<f32>(1.0 + hue * 0.45, 1.0, 1.0 - hue * 0.35);
        let depth = clamp(in.face.w, 0.0, 1.0);
        leaf_occlusion = 1.0 - depth * 0.75;
        // Sun shining through a card from behind it, strongest looking into the sun.
        let sun = globals.sun.xyz;
        let behind = max(-dot(facing, sun), 0.0);
        let into_sun = pow(max(-dot(v, sun), 0.0), 4.0);
        leaf_through = m.albedo * vec3<f32>(0.9, 1.1, 0.45) * 2.7 * (behind * 0.22 + into_sun * 0.4) * leaf_occlusion;
    }
    if in.material == MAT_BARK {
        // Scanned bark: along and around a trunk's own frame where it has one
        // (face.xy, metres), else the box-projected metres.
        let pine = ((in.model_class >> 16u) & 0xFFu) == BARK_PINE_PATTERN;
        let seed = f32((in.model_class >> 24u) & 0xFFu) / 255.0;
        let metres = select(in.uv, in.face.xy, in.face.z < 0.0);
        let buv = metres / select(1.1, 2.0, pine) + vec2<f32>(seed * 0.37, seed * 0.71);
        let layer = FOLIAGE_BASE + select(FOLIAGE_BARK, FOLIAGE_PINE_BARK, pine);
        let albedo = textureSample(terrain_materials, repeat_sampler, buv, layer);
        let detail = textureSample(terrain_materials, repeat_sampler, buv, layer + 1);
        let dp1 = dpdx(in.world);
        let dp2 = dpdy(in.world);
        let duv1 = dpdx(buv);
        let duv2 = dpdy(buv);
        let t = cross(dp2, n) * duv1.x + cross(n, dp1) * duv2.x;
        let b = cross(dp2, n) * duv1.y + cross(n, dp1) * duv2.y;
        let inv = inverseSqrt(max(max(dot(t, t), dot(b, b)), 1e-12));
        let ts = detail.xyz * 2.0 - 1.0;
        // OpenGL normal maps: green points up the image, against v.
        n = normalize(n * ts.z + (t * ts.x - b * ts.y) * inv);
        m.albedo = albedo.rgb * (0.78 + 0.44 * in.state.w) * (0.45 + 0.55 * detail.a);
        m.roughness = albedo.a;
    }
    if (in.material == MAT_FOLIAGE || in.material == MAT_BARK) && (flags & KIND_PROP) != 0u {
        let charred = 1.0 - clamp(in.state.y, 0.0, 1.0);
        m.albedo = mix(m.albedo, vec3<f32>(0.016, 0.012, 0.009), charred);
        m.roughness = mix(m.roughness, 0.98, charred);
        leaf_through *= 1.0 - charred;
        let edge = vapor_edge(in);
        if edge > 0.0 {
            discard;
        }
        // The whole tree heats as the field takes it; the chunks about to go burn white.
        let hot = smoothstep(-0.16, 0.0, edge);
        let warm = clamp(in.state.y - 1.0, 0.0, 1.0);
        m.albedo = mix(m.albedo, vec3<f32>(0.05, 0.02, 0.01), max(hot, warm * 0.6));
        m.emissive += mix(vec3<f32>(0.9, 0.07, 0.02) * warm * 0.8,
            mix(vec3<f32>(1.0, 0.42, 0.07), vec3<f32>(1.0, 0.92, 0.78), hot * hot) * 7.0, hot);
        leaf_through *= 1.0 - max(hot, warm);
    }
    var tread = 1.0;
    if in.material == MAT_TREAD && (in.model_class & 0x200u) == 0u {
        // Track links: cleats around the belt. The top run crawls forward
        // with the hull; the underside crawls back so it stays on the ground.
        // A hover skirt uses the same rubber but is not a belt.
        var crawl = 0.0;
        // Afloat the belt has nothing to drive on: it stands still.
        if (flags & FLAG_MOVING) != 0u && (flags & KIND_WRECK) == 0u
            && terrain_height(in.world.xy) >= globals.map.z - 0.25 {
            crawl = time * 6.0;
        }
        let link = fract(in.uv.x * 2.6 + crawl);
        tread = 0.45;
        let cleat = smoothstep(0.0, 0.12, link) * (1.0 - smoothstep(0.55, 0.67, link));
        let detail = clamp(1.0 - dist / 500.0, 0.0, 1.0);
        m.albedo = mix(m.albedo, mix(vec3<f32>(0.012), vec3<f32>(0.075, 0.07, 0.065), cleat), detail);
        m.metallic = 0.5 * cleat * detail;
        m.roughness = mix(m.roughness, 0.55, cleat * detail);
    }
    if in.material == MAT_GLOW_AMBER && (flags & FLAG_BUILDING) != 0u && in.refit.z <= 0.0 {
        // Construction emitters run hot while the unit builds, not during a refit.
        m.emissive *= 1.6 + 0.7 * sin(time * 11.0 + in.state.w * 40.0);
    } else if in.material == MAT_GLOW_AMBER && (in.model_class & 0x100u) != 0u {
        // A mobile builder's emitters sit banked low while it is not building.
        m.emissive *= 0.14;
        m.albedo *= 0.6;
    }
    if in.material == MAT_GLOW_RED && (flags & KIND_WRECK) == 0u {
        // Aviation-style obstruction blink: a hard on, then a long dark.
        let blink = select(0.06, 1.0, fract(time * 0.85 + in.state.w) < 0.32);
        m.emissive *= blink;
        m.albedo *= 0.35 + 0.65 * blink;
    }
    if in.drive.x >= 0.0 && in.drive_at.w > 0.5 && (in.material == MAT_GLOW || in.material == MAT_GLOW_ORANGE)
        && (flags & (KIND_WRECK | KIND_GHOST | FLAG_UNDER_CONSTRUCTION)) == 0u {
        // A spacecraft's drives (`CapitalRig::drives`) and lift jets: idling they smoulder;
        // under thrust the throats go white-hot and rings of heat run out of them.
        let l = in.local;
        if in.drive_at.w < 1.5 {
            let throttle = in.drive.x;
            let size = in.drive.w;
            let r = length(l.yz - in.drive_at.yz) / size;
            let rings = 0.82 + 0.18 * sin(r * 2.0 + l.x / size * 0.9 - time * (5.0 + 6.0 * throttle));
            let core = 1.0 - smoothstep(1.5, 8.0, r);
            let flicker = 0.95 + 0.05 * sin(time * 23.0 + l.y * 3.1);
            let idle = (1.0 - smoothstep(0.5, 4.5, r)) * 0.3 * (1.0 - throttle);
            // Hotter the deeper into the bell (mouth at drive_at.x, throat drive.z in).
            let deep = mix(0.3, 1.0, clamp((l.x - in.drive_at.x) / in.drive.z, 0.0, 1.0));
            m.emissive *= (mix(0.03, 1.9, throttle) * deep + idle) * rings * flicker * (1.0 + core * 1.4 * throttle);
            // Cold, the liner is dark rough metal, not pale glass.
            m.albedo *= mix(0.12, 1.0, throttle);
            m.roughness = mix(0.7, m.roughness, throttle);
            let hot = core * throttle * 0.55 * select(1.0, 0.0, in.material == MAT_GLOW_ORANGE);
            m.emissive = mix(m.emissive, vec3<f32>(1.0, 0.96, 0.9) * dot(m.emissive, vec3<f32>(0.33)), hot);
        } else {
            m.emissive *= mix(0.22, 2.2, in.drive.y) * (0.92 + 0.08 * sin(time * 17.0 + l.x));
        }
    } else if in.drive.x < 0.0 && in.material == MAT_GLOW && (in.model_class & 0x1000u) != 0u
        && (flags & (KIND_WRECK | KIND_GHOST | FLAG_UNDER_CONSTRUCTION)) == 0u {
        let rings = 0.7 + 0.3 * sin(length(in.local.yz - vec2<f32>(sign(in.local.y) * select(27.0,44.0,abs(in.local.y)>35.5),53.0)) * 2.5 - time * 7.0);
        m.emissive *= rings * (0.8 + 0.18 * sin(time * 11.0 + in.local.y));
    }
    if in.material == MAT_GLOW && (flags & (KIND_WRECK | KIND_GHOST | KIND_PROP)) == 0u {
        if (flags & STATE_UNPOWERED) != 0u {
            // Dead emitters: the crystal and the banks go graphite.
            m.emissive *= 0.03;
            m.albedo = mix(m.albedo, vec3<f32>(0.04, 0.05, 0.055), 0.78);
        } else if (flags & STATE_CHARGING) != 0u {
            // Capacitor pulses while a shattered dome fills.
            let beat = 0.5 + 0.5 * sin(time * 3.4 + in.state.w * 11.0);
            let pulse = 0.2 + 0.8 * beat * beat;
            m.emissive *= pulse * 0.58;
        }
    }
    if (flags & (KIND_PROP | KIND_GHOST)) == 0u && in.material != MAT_GLOW && in.material != MAT_GLOW_ORANGE && in.material != MAT_GLOW_AMBER && in.material != MAT_GLOW_RED && in.material != MAT_GLOW_VIOLET {
        // Field dirt: dust thrown up over the running gear and lower hull, and
        // grime settling where the wear map says. Plain tech 1 kit is the
        // dirtiest; the higher tiers stay closer to parade white.
        let tech = f32(max(in.model_class & 0xFFu, 1u));
        let amount = 1.25 / tech;
        let wear = textureSample(panel_map, repeat_sampler, in.uv * 0.11 + vec2<f32>(in.state.w * 7.0)).a;
        var low = 1.0 - smoothstep(0.0, in.dust, in.state.z);
        var grit = 1.0;
        if (in.model_class & 0x100u) == 0u {
            // Structures only pick it up around the footing — not on a
            // howitzer tube sitting over the pit.
            low = 1.0 - smoothstep(0.0, 0.12, in.state.z);
            grit = low;
        }
        if (in.model_class & 0x800u) != 0u {
            // A ship throws up spray, not dust, and the sea keeps it rinsed: its
            // waterline and grime are the hull pattern's.
            low = 0.0;
            grit = 0.3;
        }
        let dust = clamp((low * 0.95 + smoothstep(0.42, 0.78, wear) * 0.5 * grit) * amount * tread, 0.0, 0.85);
        m.albedo = mix(m.albedo, vec3<f32>(0.2, 0.165, 0.12) * (0.7 + wear * 0.6), dust);
        m.roughness = mix(m.roughness, 0.92, dust);
        m.metallic *= 1.0 - dust;
        m.emissive *= 1.0 - dust;
        // A lamp under dust is dimmer, not out: deck and apron lights sit in the footing's dirt.
        lights *= 1.0 - dust * 0.45;
    }
    m.emissive += lights;

    if soot > 0.0 {
        // Burns from damage go over paint and dirt alike.
        m.albedo = mix(m.albedo, vec3<f32>(0.014, 0.012, 0.011), soot * 0.92);
        m.roughness = mix(m.roughness, 0.95, soot);
        m.metallic *= 1.0 - soot * 0.7;
    }

    if (flags & KIND_WRECK) != 0u {
        // Burnt out: charred, matte, dead emitters; fades into the ground as it is reclaimed.
        // Soot, scorched paint and bare burnt steel, from a field of the model position, so
        // two faces lying in the same plane are shaded alike and cannot flicker against each other.
        let burn = wreck_burn(in.local, in.state.w);
        let paint = m.albedo * 0.16;
        let steel = vec3<f32>(0.11, 0.075, 0.055) * (0.6 + burn.y);
        m.albedo = mix(vec3<f32>(0.022, 0.02, 0.019), mix(paint, steel, smoothstep(0.45, 0.7, burn.y)), smoothstep(0.3, 0.75, burn.x));
        // A rotated cut of the plate map, so armour seams still read without marching.
        let plate_uv = vec2<f32>(in.local.x * 0.07 + in.local.y * 0.04, -in.local.x * 0.04 + in.local.z * 0.08) + vec2<f32>(in.state.w * 2.3, in.state.w * 1.1);
        let plate = textureSample(panel_map, repeat_sampler, plate_uv);
        m.albedo *= 0.62 + plate.b * 0.38;
        m.metallic = 0.3 * smoothstep(0.5, 0.7, burn.y);
        m.roughness = 0.92;
        m.emissive = vec3<f32>(0.0);
        if in.state.z > in.state.y * 0.75 + 0.25 {
            discard;
        }
    }

    let shadow = sun_shadow(in.world, n);
    // Hull plating and frames are grey metal: the sky's blue is half taken out of
    // their fill light, or the shaded side of every hull reads as blue paint.
    var sky = atmos.sky_color.rgb;
    if in.material == MAT_PLATING || in.material == MAT_ACCENT || in.material == MAT_PLATING_DARK {
        sky = mix(sky, vec3<f32>(dot(sky, vec3<f32>(0.2126, 0.7152, 0.0722))), 0.55);
    }
    var color = shade_pbr_env(m, n, v, globals.sun.xyz, shadow, atmos.sun_color.rgb, sky,
        atmos.ground_color.rgb, 1.0);
    color += m.albedo * lightning_light(in.world, n) * 0.35;
    color += local_lights(m, in.world, n, v);
    if in.material == MAT_FOLIAGE {
        color = color * leaf_occlusion + leaf_through * shadow;
    }
    var alpha = 1.0;

    if (flags & FLAG_UNDER_CONSTRUCTION) != 0u {
        // Hull fills in all over from a stable print order. Waves of work
        // light run out from each weld; a new weld never rewrites what is up.
        let lit_before = color;
        let build = in.state.x;
        let grow = clamp(build / 0.74, 0.0, 1.0);
        let order = print_order(in.local, in.state.w);
        let cool = smoothstep(0.74, 1.0, build);
        let waves = site_waves(in.local, in.weld, time, in.state.w);
        let wave = waves.x;
        let working = waves.y > 0.5;
        if order > grow {
            // Still coming: an amber lattice, brightest where a wave is passing.
            let scan = fract(dot(in.local, vec3<f32>(0.38, 0.41, 0.72)) - time * 0.7);
            let lattice = step(0.88, fract(in.local.x * 0.48 + in.local.z * 0.06))
                + step(0.88, fract(in.local.y * 0.48))
                + step(0.9, scan);
            let speckle = step(0.975, hash21(in.local.xy + vec2<f32>(in.local.z, in.state.w)));
            if lattice + speckle < 0.5 && wave < 0.35 {
                discard;
            }
            color = AMBER * (1.7 + 2.4 * wave) + vec3<f32>(1.0, 0.82, 0.35) * wave * wave * 3.5;
        } else {
            // Printed: white-hot on the wave, warm while work is on, then the late cool-off.
            let fresh = select(0.0, smoothstep(0.08, 0.0, grow - order), working);
            let heat = (1.0 - cool) * select(0.2, mix(0.45, 1.0, max(wave, fresh * 0.7)), working);
            let molten = vec3<f32>(1.0, 0.68, 0.22) * 2.5 + AMBER * 0.85;
            color = mix(color, molten, heat);
            color += vec3<f32>(1.0, 0.78, 0.32) * wave * (1.0 - cool) * 3.2;
            let shimmer = 0.07 * sin(time * 11.0 + in.local.x * 1.7 + in.state.w * 18.0);
            color += AMBER * shimmer * heat;
        }
        if (in.model_class & 0x400u) != 0u {
            // Printed by a replicator (Survival): the same fill in replication violet.
            let base = select(lit_before, vec3<f32>(0.0), order > grow);
            color = base + replication_tint(color - base);
        }
    }
    if in.refit.z > 0.0 {
        if in.refit.x > 0.5 && in.refit.y <= 0.0 {
            // A piece whose turn has not come: a scanning hologram of it, in amber.
            let scan = fract(in.local.z * 1.4 - time * 0.9);
            let grid = step(0.8, fract(in.local.x * 2.0)) + step(0.8, fract(in.local.y * 2.0)) + step(0.85, scan);
            if grid < 0.5 {
                discard;
            }
            color = AMBER * 2.2;
        } else if in.refit.x > 0.5 {
            // Going up: white-hot at first, cooling into the finished piece.
            let cool = smoothstep(0.0, 1.0, in.refit.y);
            color = mix(vec3<f32>(1.0, 0.66, 0.22) * 2.6, color, cool) + AMBER * 0.9 * (1.0 - cool);
        } else {
            // The unit being refitted: bands of work light passing up and down it.
            let sweep = abs(fract(time * 0.23) * 2.0 - 1.0);
            let off = (in.state.z - sweep) * 16.0;
            let seam = step(0.9, fract(in.local.z * 1.1 + in.local.x * 0.4));
            color += AMBER * (1.7 * exp(-off * off) + 0.25 * seam * (0.5 + 0.5 * sin(time * 6.0 + in.local.z)));
        }
    }
    if (flags & KIND_GHOST) != 0u {
        let pulse = 0.6 + 0.4 * sin(time * 5.0);
        let rim = pow(1.0 - max(dot(n, v), 0.0), 2.0);
        // `health` carries validity for ghosts: 1 placeable, 0 blocked.
        let tint = mix(vec3<f32>(1.0, 0.15, 0.1), globals.glow.rgb, step(0.5, in.state.y));
        color = tint * (0.5 + rim * 2.5) * pulse;
        let weave = fract((in.clip.x + in.clip.y) * 0.25);
        if weave < 0.5 {
            discard;
        }
    }

    if (flags & KIND_PROP) != 0u {
        color = apply_fog_of_war(color, in.world.xy);
    }
    color = apply_haze(color, in.world, eye);
    // A mirror-smooth highlight can exceed what the half-float target holds and
    // turn into an infinity, which the bloom would then smear across the screen.
    color = min(max(color, vec3<f32>(0.0)), vec3<f32>(48.0));
    return vec4<f32>(color, alpha);
}

const HULL_HEX: f32 = 1.45;
const HULL_HIT_COUNT: u32 = 64u;
// Strike bloom and ripple scale, metres. Not the plate size: a smaller cell
// should not shrink the hit.
const HULL_HIT_SCALE: f32 = 2.4;

// Top of the hull over the model's middle, bind-pose metres: where an Aegis
// would have its needle. The lowest top of a few plan texels round the centre,
// so an antenna or a gun barrel over the middle does not lift it off the deck.
fn hull_crown(blueprint: u32, height: f32) -> f32 {
    let layers = textureNumLayers(hull_plans);
    if layers == 0u {
        return height * 0.94;
    }
    let layer = i32(min(blueprint, layers - 1u));
    var top = 2.0;
    for (var k = 0u; k < 5u; k++) {
        let a = f32(k) * 1.2566371;
        let off = select(vec2<f32>(cos(a), sin(a)) * 0.05, vec2<f32>(0.0), k == 4u);
        let s = textureSampleLevel(hull_plans, clamp_sampler, vec2<f32>(0.5) + off, layer, 0.0);
        if s.b > s.g + 0.01 {
            top = min(top, s.b);
        }
    }
    return select(height * 0.94, top * max(height, 0.5), top < 1.5);
}

// Bind-pose point the wrap is projected from. Everything on the skin runs out from here.
fn hull_emitter(crown: f32) -> vec3<f32> {
    return vec3<f32>(0.0, 0.0, crown);
}

// Emitter to this point over the model's span: 0 at the emitter, about 1 at
// the farthest plate. The wrap unfolds and its waves travel along this.
fn hull_polar(local: vec3<f32>, crown: f32, reach: f32, height: f32) -> f32 {
    let span = max(max(reach, height) * 1.05, 1.0);
    return saturate(length(local - hull_emitter(crown)) / span);
}

fn hull_hex_round(q: f32, r: f32) -> vec2<f32> {
    let s = -q - r;
    var qq = round(q);
    var rr = round(r);
    let ss = round(s);
    let qd = abs(qq - q);
    let rd = abs(rr - r);
    let sd = abs(ss - s);
    if qd > rd && qd > sd {
        qq = -rr - ss;
    } else if rd > sd {
        rr = -qq - ss;
    }
    return vec2<f32>(qq, rr);
}

fn hull_hex_on(st: vec2<f32>) -> vec2<f32> {
    let q = (st.x * 0.57735027 - st.y * 0.33333334) / HULL_HEX;
    let r = (st.y * 0.6666667) / HULL_HEX;
    let qr = hull_hex_round(q, r);
    let center = vec2<f32>(
        HULL_HEX * (1.7320508 * qr.x + 0.8660254 * qr.y),
        HULL_HEX * (1.5 * qr.y),
    );
    let local = (st - center) / HULL_HEX;
    let g = abs(local);
    let edge = max(g.y * 0.8660254 + g.x * 0.5, g.x);
    return vec2<f32>(edge, hash21(qr + vec2<f32>(13.1, 7.7)));
}

fn hull_hex_triplanar(p: vec3<f32>, n: vec3<f32>) -> vec2<f32> {
    let an = abs(n);
    let w = an * an * an * an;
    let hx = hull_hex_on(p.yz);
    let hy = hull_hex_on(p.xz);
    let hz = hull_hex_on(p.xy);
    let sum = max(w.x + w.y + w.z, 1e-5);
    let edge = (hx.x * w.x + hy.x * w.y + hz.x * w.z) / sum;
    var hsh = hz.y;
    if w.x > w.y && w.x > w.z {
        hsh = hx.y;
    } else if w.y > w.z {
        hsh = hy.y;
    }
    return vec2<f32>(edge, hsh);
}

fn hull_owns_hit(p: vec3<f32>, s: Shield) -> bool {
    let c = vec3<f32>(s.pos.x, s.pos.y, s.pos.z + s.height * 0.5);
    let r = vec3<f32>(s.radius, s.radius, max(s.height * 0.5 + 0.8, 0.5));
    return length((p - c) / max(r, vec3<f32>(1e-3))) <= 1.12;
}

fn hull_hits(p: vec3<f32>, time: f32, edge: f32, s: Shield) -> vec4<f32> {
    var rgb = vec3<f32>(0.0);
    var a = 0.0;
    for (var i = 0u; i < HULL_HIT_COUNT; i++) {
        let h = shield_hits[i];
        if h.strength <= 0.0 || !hull_owns_hit(h.pos, s) {
            continue;
        }
        let age = time - h.start;
        if age < 0.0 || age > 1.65 {
            continue;
        }
        let envelope = 1.0 - smoothstep(0.02, 0.38, age);
        let fade = 1.0 - smoothstep(0.2, 1.65, age);
        let col = vec3<f32>(0.28, 0.82, 1.0);
        // The sim lands a strike on its round collision shell, which stands off
        // a long hull. Carry it in along the ray to the middle onto this skin.
        let c = vec3<f32>(s.pos.x, s.pos.y, s.pos.z + s.height * 0.5);
        let on_skin = c + normalize(h.pos - c + vec3<f32>(0.0, 0.0, 1e-4)) * length(p - c);
        let dist = length(p - on_skin);
        let reach = HULL_HIT_SCALE * 1.4 + h.strength * 1.4;
        let fill = (1.0 - smoothstep(reach * 0.35, reach, dist)) * envelope * h.strength;
        let wave = dist - age * (12.0 + h.strength * 6.0);
        let ring = exp(-abs(wave) * 1.8) * fade * h.strength;
        let seam = pow(saturate((edge - 0.58) / 0.30), 1.25);
        rgb += col * (fill * 2.4 + ring * (0.25 + 0.75 * seam) * 4.6);
        a += fill * 0.14 + ring * 0.14;
    }
    return vec4<f32>(rgb, a);
}

// Pre-pass: the outermost skin's depth, so where a hull's pieces overlap the
// field is one surface rather than a stack of translucent layers.
@fragment
fn fs_hull_depth(in: VsOut) {
    let s = shields[in.material];
    let open = mix(s.prev_open, s.open, globals.sun.w);
    if open * 1.08 - hull_polar(in.local, in.weld.x, in.weld.z, in.weld.w) * 0.88 < 0.0 || open <= 0.001 {
        discard;
    }
}

@fragment
fn fs_hull(in: VsOut) -> @location(0) vec4<f32> {
    let s = shields[in.material];
    let open = mix(s.prev_open, s.open, globals.sun.w);
    // Unfolds from the emitter over the plates, same language as a rising dome.
    let polar = hull_polar(in.local, in.weld.x, in.weld.z, in.weld.w);
    let reveal = open * 1.08 - polar * 0.88;
    // Bind-pose frame, before discard so the derivatives stay defined.
    let skin_n = cross(dpdx(in.local), dpdy(in.local));
    if reveal < 0.0 || open <= 0.001 {
        discard;
    }
    // One layer: a piece's skin inside another's is not drawn (reversed Z, nearer is larger).
    let front = textureLoad(hull_front, vec2<i32>(in.clip.xy), 0);
    if in.clip.z < front * (1.0 - 4e-6) {
        discard;
    }
    let eye = globals.camera.xyz;
    let time = globals.camera.w;
    let dir = normalize(in.world - eye);
    var nrm = normalize(in.normal);
    if dot(nrm, -dir) < 0.0 {
        nrm = -nrm;
    }
    let facing = saturate(dot(nrm, -dir));
    // Painted on the plates so the honeycomb walks and turns with the hull.
    // Domes keep a world lattice so merged fields share one grid; a wrap does not.
    let hx = hull_hex_triplanar(in.local, skin_n / max(length(skin_n), 1e-5));
    let eye_dist = length(in.world - eye);
    let cell_px = HULL_HEX * globals.lod.x / max(eye_dist, 1.0);
    let hex_see = smoothstep(0.9, 4.5, cell_px);
    let seam = pow(saturate((hx.x - 0.66) / 0.22), 1.45) * hex_see;
    let plate = (1.0 - smoothstep(0.70, 0.90, hx.x)) * hex_see;
    let fres = pow(1.0 - facing, 2.1);
    let live = 0.68 + 0.32 * sin(time * 2.4);
    let dying = select(0.0, 1.0, ((s.packed >> 24u) & 1u) == 1u);
    var born = 0.0;
    if open < 0.96 {
        born = exp(-abs(reveal) * 22.0) * (1.0 - open);
    }
    if dying > 0.5 {
        born = exp(-abs(reveal) * 8.0) * (0.45 + 0.55 * open);
    }
    let ground = max(terrain_height(in.world.xy), globals.map.z);
    let gap = in.world.z - ground;
    let touch = saturate(exp(-gap * gap * 1.55) * 0.85 + exp(-gap * gap * 0.28) * 0.4);
    let blow = hull_hits(in.world, time, hx.x, s);
    let health = saturate(s.health);
    let stress = (1.0 - health) * (1.0 - health);
    let energy = mix(vec3<f32>(0.62, 0.84, 1.0), globals.glow.rgb, 0.2);
    let team_c = globals.team_colors[s.packed & 7u].rgb;
    let rim_c = mix(energy, team_c, 0.16);
    let ice = mix(energy, vec3<f32>(0.82, 0.94, 1.0), 0.45);
    let cyan = vec3<f32>(0.22, 0.68, 1.0);

    // The Aegis language on a skin: a tight cyan knot at the emitter, rings
    // born there that run out over the plates, packets riding the seams on the
    // same current, and plates that breathe on their own clocks.
    let emit_d = length(in.local - hull_emitter(in.weld.x));
    let knot_r = 0.8 + in.weld.w * 0.03;
    let knot = exp(-(emit_d * emit_d) / (knot_r * knot_r)) * (0.85 + 0.15 * sin(time * 14.0));
    // A launch pulse: the knot swells each time a ring leaves it.
    let launch = exp(-pow(fract(time * 0.9) - 0.04, 2.0) * 90.0);
    let corona = exp(-(emit_d * emit_d) / (knot_r * knot_r * 7.0)) * (0.35 + 0.2 * sin(time * 5.6) + launch * 0.6);
    let phase = polar * 3.2 - time * 0.9;
    let ring0 = exp(-pow(fract(phase) - 0.14, 2.0) * 150.0) * (1.0 - polar * 0.55);
    let ring1 = exp(-pow(fract(phase + 0.5) - 0.14, 2.0) * 120.0) * (1.0 - polar * 0.7) * 0.6;
    // Rings light the lattice more than the glass, so the wave walks the hexes.
    let wave = (ring0 + ring1) * (0.3 + 0.7 * seam + 0.25 * plate);
    let breathe = 0.38 + 0.62 * (0.5 + 0.5 * sin(time * 2.55 + hx.y * 6.2831855));
    let run = fract(polar * 5.2 - time * 0.8 + hx.y * 0.16);
    let packet = exp(-pow(run - 0.5, 2.0) * 40.0);
    let current = seam * (0.22 + 0.78 * packet) * breathe;
    let flow = pow(1.0 - polar, 2.2) * breathe;

    // Clear glass face-on: the lattice and the current carry the field, the
    // rim carries the shape.
    var color = rim_c * (fres * 0.62 * live + stress * 0.18 + 0.08 * live);
    color += energy * (seam * 0.55 * live + current * 1.25 + plate * 0.1 * breathe + stress * seam * 0.4 + flow * 0.18);
    color += ice * wave * 2.4;
    // Deep cyan and fairly opaque, so the knot still reads over white plating.
    color += vec3<f32>(0.06, 0.48, 1.0) * (knot * 12.0 + corona * 3.0);
    color += mix(energy, ice, dying) * born * mix(1.3, 3.4, dying);
    color += blow.rgb;
    color += cyan * touch * 2.2;

    var alpha = 0.018 + fres * 0.09 * live + stress * 0.045;
    alpha += seam * 0.08 * live + current * 0.15 + plate * 0.014 * breathe + stress * seam * 0.06 + flow * 0.02;
    alpha += wave * 0.15 + knot * 0.7 + corona * 0.2;
    alpha += born * mix(0.28, 0.72, dying) + blow.a + touch * 0.26;
    alpha *= smoothstep(0.0, 0.1, open) * mix(0.8 + 0.2 * health, 1.25, dying);

    if alpha < 0.002 {
        discard;
    }
    color = apply_haze(apply_fog_of_war(color, in.world.xy), in.world, eye);
    return vec4<f32>(color * alpha, alpha);
}

// Construction light recoloured for a replicator's print (Survival): amber and its
// whites become the replication violet and its whites; what the fill took away stays.
fn replication_tint(c: vec3<f32>) -> vec3<f32> {
    let add = max(c, vec3<f32>(0.0));
    let hi = max(add.r, max(add.g, add.b));
    let lo = min(add.r, min(add.g, add.b));
    return vec3<f32>(0.55, 0.22, 1.0) * (hi - lo) * 1.05 + vec3<f32>(lo) + min(c, vec3<f32>(0.0));
}
