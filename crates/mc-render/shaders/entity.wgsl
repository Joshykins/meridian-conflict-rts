//!use bindings
//!use habitat
//!use surface
//!use regency
//!use scenery
//!use warp_hull
//!use emp
//!use wreck
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
const MAT_GLOW_LASER: u32 = 17u;
const MAT_PRECURSOR: u32 = 18u;
const MAT_PRECURSOR_DARK: u32 = 19u;
const MAT_GLOW_PRECURSOR: u32 = 20u;
const MAT_GLOW_NAV_RED: u32 = 21u;
const MAT_GLOW_NAV_GREEN: u32 = 22u;
const MAT_GLOW_LAMP: u32 = 23u;
const MAT_GLOW_SHIELD: u32 = 24u;
const MAT_PRECURSOR_INLAY: u32 = 25u;
const MAT_VISOR: u32 = 26u;

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
// A strategic launcher's blast doors and the rounds it holds (models::part, nuke_fx).
const PART_SILO_DOOR: u32 = 24u;
const PART_SILO_ROUND: u32 = 25u;
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
// On a leg: a reverse-kneed leg's bone from the hock to the ankle (`rig::TARSUS`).
const LIMB_TARSUS: u32 = 4u;
// Folding gear: swung back about `model.fold` while the unit is not building.
const LIMB_FOLD: u32 = 7u;
// The head on the end of the folding gear: bends about `model.fold_wrist`, then rides the arm.
const LIMB_FOLD_HEAD: u32 = 9u;
// A walker's head: looks about while it stands idle (`idle_pose`).
const LIMB_HEAD: u32 = 10u;
// A many-legged walker's tail: bends toward the turret's facing (`rig::TAIL`, `crawl_tail`).
const LIMB_TAIL: u32 = 15u;
// Build-arm gear at work (`rig::WORK_*`): twists, runs out, or opens and closes.
const WORK_TWIST: u32 = 1u;
const WORK_EXTEND: u32 = 2u;
const WORK_BREATHE: u32 = 3u;
// A turret of its own on the turret, about `model.mount`.
const LIMB_MOUNT: u32 = 8u;
// Gun houses of their own on the hull, about `model.houses[limb - LIMB_HOUSE]` (`rig::HOUSE_FIRST`).
const LIMB_HOUSE: u32 = 11u;
// Houses 4..8 reuse the four house limbs with this bit set (`rig::HOUSE_HIGH`).
const RIG_HOUSE_HIGH: u32 = 0x800000u;

// House `slot` (0..8): pivot (xyz) and kick-back travel (w).
fn house_of(model: ModelInfo, slot: u32) -> vec4<f32> {
    if slot < 4u {
        return model.houses[slot];
    }
    return model.houses_high[slot - 4u];
}

// The weapon house `slot` is bound to, plus one; zero for no house there.
fn house_weapon_of(model: ModelInfo, slot: u32) -> f32 {
    if slot < 4u {
        return model.house_weapon[slot];
    }
    return model.house_weapon_high[slot - 4u];
}
// How far open a breech door stands (0 shut, 1 open) from the gun's recoil kick this tick
// and last (`mirror::barrel_recoil`: one the tick it fires, easing home as (1 - s)^3 over
// its run, s the share of the run gone). It snaps open as the gun kicks, stands open
// while the spent cartridge is thrown, and shuts before the tube is home. The tick the
// gun fires starts from s = 0, not from last tick's rest.
fn breech_open(prev: f32, now: f32, t: f32) -> f32 {
    if now <= 0.0 {
        return 0.0;
    }
    let s_now = 1.0 - pow(now, 1.0 / 3.0);
    let s_prev = select(0.0, 1.0 - pow(max(prev, 0.0), 1.0 / 3.0), prev > 0.0);
    let s = mix(s_prev, s_now, t);
    return smoothstep(0.0, 0.05, s) * (1.0 - smoothstep(0.55, 0.85, s));
}

// How far a gun on the arm is kicked back, on the side `y` says: a twin gun on the other
// side from the main one (`mirror::UNIT_TWIN_*`) kicks on its own shots (its `HousePose`
// kick), every other arm gun with the main gun (`recoil`).
fn arm_kick(e: Entity, t: f32, y: f32) -> f32 {
    let twin = (e.status[1] >> ARM_TWIN_SHIFT) & ARM_TWIN_MASK;
    let right = (e.status[1] & ARM_TWIN_RIGHT) != 0u;
    if twin == 0u || (e.status[1] >> UNIT_HOUSE_SHIFT) == 0u || (y < 0.0) != right {
        return mix(e.prev_recoil, e.recoil, t);
    }
    let w = twin - 1u;
    let kicks = house_pose(e).kick[w / 2u];
    let k = select(kicks.xy, kicks.zw, (w & 1u) == 1u);
    return mix(k.x, k.y, t);
}

// The unit's gun-house poses (`mirror::HousePose`), for a unit whose `status[1]` names
// some. The index is held inside the buffer: a bad one reads the wrong pose, never
// past the end.
fn house_pose(e: Entity) -> HousePose {
    let i = (e.status[1] >> UNIT_HOUSE_SHIFT) - 1u;
    return houses[min(i, arrayLength(&houses) - 1u)];
}

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
    // Into or out of warp (warp_hull.wgsl): how far into the streak, 1 going in, 1 dampened,
    // then 0 at the stern to 1 at the nose.
    @location(15) warp: vec4<f32>,
    // What crackles over the hull, between ticks: x how stunned by an EMP it is (emp.wgsl),
    // y how full its warp drive's charge is while it spools (warp_hull.wgsl), 0 to 1.
    @location(16) @interpolate(flat) crackle: vec2<f32>,
    // A settled wreck's section (wreck.wgsl): the stretch of the hull it keeps along model
    // x (metres), 1 when it draws the hull's inside, 1 when it is posed at all.
    @location(17) @interpolate(flat) wreck: vec4<f32>,
}

struct Weld {
    // xyz local print origin, w 1 while the beam is on, then decaying.
    pos: vec4<f32>,
}

@group(0) @binding(15) var<storage, read> welds: array<Weld>;

// One track link's pitch in model units: 0.4 on a tank, growing with the body (`reach`,
// its radius) to 1.0 on the biggest, so a giant's belt is heavy links, not a fine mesh.
fn tread_pitch(reach: f32) -> f32 {
    return 0.4 * clamp(reach / 6.0, 1.0, 2.5);
}

// Links in one period of a belt's wear bands, which still read as it rolls when the
// links themselves are a blur.
const TREAD_WRAP: f32 = 8.0;

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

// A strategic launcher's load cycle (`gpu_consts::launcher`): how far its store's lid is
// open (x) and its hoist let down (y), 0 to 1. It runs while the launcher is assembling a
// round: short of its stock, on auto-build or with rounds queued, powered and not paused.
// Otherwise both rest shut and up. Each launcher runs at its own phase.
fn load_cycle(e: Entity, time: f32) -> vec2<f32> {
    let word = e.status[2];
    let short = (word & LAUNCHER_STOCK_MASK) < ((word >> LAUNCHER_CAPACITY_SHIFT) & 0xFFu);
    let wanted = (word & LAUNCHER_MANUAL) == 0u || (word >> LAUNCHER_QUEUED_SHIFT) != 0u;
    let idle = (e.owner_flags & (KIND_WRECK | KIND_GHOST | FLAG_UNDER_CONSTRUCTION | STATE_UNPOWERED)) != 0u
        || (e.status[0] & UNIT_PAUSED) != 0u;
    if (word & LAUNCHER_MARK) == 0u || !short || !wanted || idle {
        return vec2<f32>(0.0);
    }
    let u = fract(time / LAUNCHER_CYCLE_S + f32(e.unit_id & 255u) * 0.173);
    // Lid back, block down, a pause below, block up, lid shut, a pause before the next.
    let lid = smoothstep(0.0, 0.14, u) * (1.0 - smoothstep(0.74, 0.88, u));
    let hoist = smoothstep(0.16, 0.36, u) * (1.0 - smoothstep(0.5, 0.7, u));
    return vec2<f32>(lid, hoist);
}

// A storage structure's fill pieces and status lamps (`gpu_consts::store`), by its side's
// store in `status[2]`: a fill piece keeps its glow while the store is at least as full
// as its level and goes dark below it; a lamp's lens goes amber while the store drains,
// a blinking red when it is dry and green when it is full, and stays dark glass
// otherwise. Unmarked (another side's, a site, a wreck): as authored.
fn store_material(material: u32, part: u32, e: Entity) -> u32 {
    let word = e.status[2];
    let live = (e.owner_flags & (KIND_WRECK | KIND_GHOST | FLAG_UNDER_CONSTRUCTION)) == 0u;
    if part < STORE_PART_FILL_FIRST || part > STORE_PART_LAMP || (word & STORE_MARK) == 0u || !live {
        return material;
    }
    if part == STORE_PART_LAMP {
        switch (word >> STORE_STATE_SHIFT) & STORE_STATE_MASK {
            case STORE_DRAINING: { return MAT_GLOW_AMBER; }
            case STORE_EMPTY: { return MAT_GLOW_RED; }
            case STORE_FULL: { return MAT_GLOW_NAV_GREEN; }
            default: { return material; }
        }
    }
    let level = f32(part - STORE_PART_FILL_FIRST) + 0.5;
    let fill = f32(word & STORE_FILL_MASK) / f32(STORE_FILL_MASK);
    return select(MAT_ACCENT, material, fill * f32(STORE_FILL_LEVELS) >= level);
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

// Charge coils (`models::pattern::COIL`): a coil's light is its face pattern COIL + its
// stage along the bore (0 breech .. 7 muzzle); lugs that turn about the bore are
// COIL_TURN (and the next, the other way). The unit's charge rides in `mount`
// (`renderer/titan_charge.rs`): x charge start, y when due, z last shot, w CHARGE_RECORD.
const PAT_COIL: u32 = 19u;
const PAT_COIL_TURN: u32 = 27u;
const CHARGE_RECORD: f32 = -1000.0;

// x how full the charge is (0..1), y seconds since the shot (large: none since it began).
fn coil_state(e: Entity, time: f32) -> vec2<f32> {
    let start = e.mount.x;
    let due = e.mount.y;
    let shot = e.mount.z;
    var charge = 0.0;
    if start > shot && time >= start {
        let full = clamp((time - start) / max(due - start, 0.1), 0.0, 1.0);
        // A charge that never fired (the mark lost) drains away.
        charge = full * (1.0 - clamp((time - due - 1.5) / 2.0, 0.0, 1.0));
    }
    let since = select(1000.0, time - shot, shot >= start && shot > -9000.0);
    return vec2<f32>(charge, since);
}

// A charge gun's working gear (`CHARGE_GEAR_*`): x how far it stands open (through the
// charge, held a moment after the shot, closing as the gun cools), y how hot it is (lit
// by the shot, cooling over some seconds).
fn charge_gear_state(e: Entity, time: f32) -> vec2<f32> {
    let c = coil_state(e, time);
    let opening = smoothstep(0.0, 0.8, c.x);
    let after = 1.0 - smoothstep(0.4, 3.4, c.y);
    let heat = smoothstep(0.0, 0.15, c.y) * (1.0 - smoothstep(1.4, 5.2, c.y));
    return vec2<f32>(max(opening, after), heat);
}

// How far a charge gun's SPIN gear has turned: slowly at rest, a spin that climbs through
// the charge and runs down quickly after the shot (or when the charge is due and nothing
// fired). A run-down always adds a whole number of half turns (the gimbal cage looks the
// same half a turn round), so the next charge takes up from where it left off.
fn charge_gear_turn(e: Entity, time: f32) -> f32 {
    let idle = time * 0.15;
    let start = e.mount.x;
    let due = e.mount.y;
    let shot = e.mount.z;
    if start < -9000.0 {
        return idle;
    }
    let span = max(due - start, 0.5);
    let tau = 0.9;
    let rate = round(8.0 * (span / 3.0 + tau) / PI) * PI / (span / 3.0 + tau);
    let u = clamp((time - start) / span, 0.0, 1.0);
    var extra = rate * span / 3.0 * u * u * u;
    let ended = select(due, shot, shot >= start);
    if time > ended {
        extra = rate * span / 3.0 + rate * tau * (1.0 - exp(-(time - ended) / tau));
    }
    return idle + extra;
}

// A vertex of a charge gun's working gear (`gear`, a `CHARGE_GEAR_*` kind) posed in its
// rest frame, before the gun pitches and the turret turns: rails part and projector heads
// reach into the charge as it fills, vents lift with the heat after the shot, the gimbal
// cage spins about `rig.xyz`. Travels are `rig.w` times their authored metres.
fn charge_gear_pose(p0: vec3<f32>, n0: vec3<f32>, gear: u32, rig: vec4<f32>, e: Entity, time: f32) -> array<vec3<f32>, 2> {
    var p = p0;
    var n = n0;
    let s = charge_gear_state(e, time);
    if gear == CHARGE_GEAR_SPREAD || gear == CHARGE_GEAR_REACH {
        p.y += sign(p.y) * CHARGE_GEAR_SPREAD_M * rig.w * s.x;
    }
    if gear == CHARGE_GEAR_EXTEND || gear == CHARGE_GEAR_REACH {
        p.x += CHARGE_GEAR_EXTEND_M * rig.w * s.x;
    }
    if gear == CHARGE_GEAR_VENT {
        p.z += CHARGE_GEAR_VENT_M * rig.w * s.y;
    }
    if gear == CHARGE_GEAR_SPIN {
        let turn = charge_gear_turn(e, time);
        p = rot_z(p - rig.xyz, turn) + rig.xyz;
        n = rot_z(n, turn);
    }
    return array<vec3<f32>, 2>(p, n);
}

// How far the capacitor rings have turned: slowly at rest, and a spin that climbs through
// the charge and runs down after the shot. The spin is always a whole number of 1/24 turns
// by the time it has run down, so the next charge starts from where it left off.
fn coil_turn(e: Entity, time: f32) -> f32 {
    let idle = time * 0.1;
    let start = e.mount.x;
    let due = e.mount.y;
    let shot = e.mount.z;
    if start < -9000.0 {
        return idle;
    }
    let span = max(due - start, 0.5);
    let tau = 3.0;
    let step = 3.14159265 / 12.0;
    let rate = round(3.2 * (span / 3.0 + tau) / step) * step / (span / 3.0 + tau);
    let u = clamp((time - start) / span, 0.0, 1.0);
    var extra = rate * span / 3.0 * u * u * u;
    let ended = select(due, shot, shot >= start);
    if time > ended {
        extra = rate * span / 3.0 + rate * tau * (1.0 - exp(-(time - ended) / tau));
    }
    return idle + extra;
}

fn walk_state(e: Entity, model: ModelInfo) -> vec2<f32> {
    let stride = model.leg_hip.w;
    let t = globals.sun.w;
    // Full stride by 4% of a stride a tick, and by 0.64 m a tick however long the stride:
    // a giant's slow walk (the Behemoth's 64 m stride at 1.1 m a tick) must still plant
    // its feet, or they skate.
    let ease = 1.0 / min(0.04 * stride, 0.64);
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

// How far a VTOL pod stands up from lying along the hull (`models::vtol_tilt`, the same
// function over the same constants): pi/2 points its nozzle straight down.
fn vtol_tilt(e: Entity, t: f32, left: bool, front: bool) -> f32 {
    let pitch = mix(e.arm_pitch.z, e.arm_pitch.w, t);
    let turn = lerp_angle(0.0, e.heading - e.prev_heading, 1.0);
    let side = select(-1.0, 1.0, left);
    let lead = select(VTOL_FRONT_LEAD, 1.0, front);
    return clamp(1.5707964 + pitch * VTOL_TILT_GAIN * lead + turn * side * VTOL_YAW_GAIN,
        VTOL_TILT_MIN, VTOL_TILT_MAX);
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

// How a two-legged walker with a head stands at rest (`idle_stance`). Per leg (x left,
// y right): `fwd` the foot's place ahead of where it rests, `raise` the ankle lifted off
// the ground, `pitch` the sole turned (negative: toe down, heel up). `body`: x its roll
// (radians, the +y side up), y its shift toward +y, z how far the hips sink. All zero
// for anything else, and while walking.
struct Stance {
    fwd: vec2<f32>,
    raise: vec2<f32>,
    pitch: vec2<f32>,
    body: vec3<f32>,
}

// One foot's share of a weight shift: how free the leg is (0 loaded .. 1 eased), and how
// high the foot is lifted as it moves. `was` and `now` are how free it was and is to be;
// a foot coming back under the body steps first, the one going out after it.
fn stance_step(was: f32, now: f32, u: f32) -> vec2<f32> {
    let first = was > now;
    let a = select(0.5, 0.0, first);
    let k = clamp((u - a) / 0.5, 0.0, 1.0);
    return vec2<f32>(mix(was, now, smoothstep(0.0, 1.0, k)), sin(k * PI) * abs(now - was));
}

// Standing, it does not stand like a statue: legs near straight, it carries its weight on
// one with the hips hitched up a little over it and the body leaning its way, and eases
// the other, that foot a touch forward and light on the heel, the knee just off straight.
// Every so often it shifts its weight: the eased foot steps back under the body, the
// body sways over, and the other foot steps out. It breathes. Sized by the leg's length.
fn idle_stance(e: Entity, model: ModelInfo, walk: vec2<f32>, time: f32) -> Stance {
    var s: Stance;
    let still = 1.0 - smoothstep(0.0, 0.5, walk.x);
    if still <= 0.0 {
        return s;
    }
    let leg = model.leg_hip.z - model.leg_ankle.z;
    let seed = hash11(f32(e.unit_id & 0xFFFFu) * 0.377 + 9.1);
    // Which leg is eased: picked afresh about every 13 s (often the same one again),
    // the shift taking the first 1.8 s.
    let c = time / 13.0 + seed * 13.0 + 0.2 * sin(time * 0.11 + seed * 7.0);
    let k = floor(c);
    let u = clamp(fract(c) * 13.0 / 1.8, 0.0, 1.0);
    let was = select(0.0, 1.0, hash11(k - 1.0 + seed * 57.0) > 0.5);
    let now = select(0.0, 1.0, hash11(k + seed * 57.0) > 0.5);
    // How free each leg is (x left, y right) and how high its foot is up in the step.
    let l = stance_step(was, now, u);
    let r = stance_step(1.0 - was, 1.0 - now, u);
    let free = vec2<f32>(l.x, r.x);
    // +1 with the left leg loaded.
    let side = r.x - l.x;
    s.fwd = free * 0.05 * leg * still;
    s.pitch = -free * 0.05 * still;
    s.raise = (free * 0.016 + vec2<f32>(l.y, r.y) * 0.04) * leg * still;
    let breath = 0.004 * leg * sin(time * 1.15 + seed * 60.0);
    s.body = vec3<f32>(0.025 * side, 0.02 * leg * side, -0.008 * leg + breath) * still;
    return s;
}

// A leg vertex posed for this moment of the stride. Two bones, hip to knee and
// knee to ankle, solved so the ankle is where the foot has to be: planted and
// passing under the body, or lifted and swinging forward.
// Where a walker's feet stand (`walk_leg`), from the ground under each foot rather than
// under its middle: `ground` x the left foot's ground over (or under) the ground below the
// unit's middle, y the right's, z how far the body rides up or down over them (their
// mean, drawn halfway toward the lower so that foot can still reach it); `slope` the rise
// of the ground along the heading under each foot (left, right), for the sole to lie on.
struct Footing {
    ground: vec3<f32>,
    slope: vec2<f32>,
}

// A foot's place along the stride at `phase` of its cycle, before the walk eases in:
// planted and passing back under the body, then lifted and swinging forward.
fn stride_foot(phase: f32, model: ModelInfo) -> f32 {
    let stance = model.leg_ankle.w;
    let reach = stance * model.leg_hip.w;
    if phase < stance {
        return reach * (0.5 - phase / stance);
    }
    let u = (phase - stance) / (1.0 - stance);
    return reach * (u * u * (3.0 - 2.0 * u) - 0.5);
}

// How long a spent casing keeps the drawn stride of the walker that threw it (seconds).
const CASING_HOLD: f32 = 0.8;

// Where a spent casing in the air is drawn off the sim's place for it. The sim throws it
// from the gun as the sim stands the walker, but the drawn walker crouches, bobs and sways
// in its stride (`walk_bob`) and sets its hips on the ground under its feet: tens of metres
// on the Behemoth. Out of the port the case carries that difference, fading over its first
// moments in the air, so it leaves the gun where the gun is drawn and still comes down
// where the sim lands it. `e.status[2]` names the thrower, `e.gait` the case's age
// (`mirror::UnitInstance`).
fn casing_carry(e: Entity, t: f32) -> vec3<f32> {
    let src = dynamic_entities[e.status[2] - 1u];
    let model = models[src.blueprint];
    let hold = 1.0 - smoothstep(0.0, CASING_HOLD, mix(e.gait.x, e.gait.y, t));
    if model.leg_hip.w <= 0.0 || (src.owner_flags & KIND_WRECK) != 0u || hold <= 0.0 {
        return vec3<f32>(0.0);
    }
    let walk = walk_state(src, model);
    var ground = walk_ground(src, model, walk, t).ground.z;
    if model.crawl[0].x > 0.5 {
        ground = crawl_body_ground(src, model, t);
    }
    let bob = walk_bob(walk, model) + vec3<f32>(0.0, 0.0, ground);
    let heading = lerp_angle(src.prev_heading, src.heading, t);
    let fwd = vec2<f32>(cos(heading), sin(heading));
    let lft = vec2<f32>(-fwd.y, fwd.x);
    return vec3<f32>(fwd * bob.x + lft * bob.y, bob.z) * hold;
}

// How far out to the side a walker's feet come down: where they stand at rest, and in
// under the hips as it gets into its stride (`stride_upright`).
fn stride_ankle_y(model: ModelInfo, walk: vec2<f32>) -> f32 {
    if any(model.leg_hock.xyz != vec3<f32>(0.0)) {
        return abs(model.leg_ankle.y);
    }
    return mix(abs(model.leg_ankle.y), abs(model.leg_hip.y), walk.x);
}

// A leg vertex of a walker whose feet stand out wider than its hips (the commander's
// A-stance), brought in under them as it gets into its stride: the thigh and shin roll
// upright about the hip and the foot slides in with the ankle, still flat. Standing it is
// untouched. Before `walk_leg`, which bends the leg in its own (x, z) plane.
fn stride_upright(pos: vec3<f32>, normal: vec3<f32>, limb: u32, model: ModelInfo,
                  walk: vec2<f32>) -> array<vec3<f32>, 2> {
    let side = select(-1.0, 1.0, pos.y > 0.0);
    let hy = abs(model.leg_hip.y);
    let out = abs(model.leg_ankle.y) - hy;
    if limb != LIMB_THIGH && limb != LIMB_SHIN {
        return array<vec3<f32>, 2>(pos - vec3<f32>(0.0, side * out * walk.x, 0.0), normal);
    }
    let a = -side * atan2(out, model.leg_hip.z - model.leg_ankle.z) * walk.x;
    let c = cos(a);
    let s = sin(a);
    let q = pos.yz - vec2<f32>(side * hy, model.leg_hip.z);
    let r = vec2<f32>(q.x * c - q.y * s, q.x * s + q.y * c) + vec2<f32>(side * hy, model.leg_hip.z);
    let m = vec2<f32>(normal.y * c - normal.z * s, normal.y * s + normal.z * c);
    return array<vec3<f32>, 2>(vec3<f32>(pos.x, r.x, r.y), vec3<f32>(normal.x, m.x, m.y));
}

fn walk_ground(e: Entity, model: ModelInfo, walk: vec2<f32>, t: f32) -> Footing {
    var f: Footing;
    let heading = lerp_angle(e.prev_heading, e.heading, t);
    let at = mix(e.prev_pos, e.pos, t).xy;
    let fwd = vec2<f32>(cos(heading), sin(heading));
    let lft = vec2<f32>(-fwd.y, fwd.x);
    let centre = terrain_height(at);
    // Half a sole or so: the slope a foot lies on, not the bumps under its heel.
    let d = max(0.12 * model.leg_hip.w, 1.0);
    let ay = stride_ankle_y(model, walk);
    let xl = model.leg_ankle.x + stride_foot(fract(walk.y), model) * walk.x;
    let xr = model.leg_ankle.x + stride_foot(fract(walk.y + 0.5), model) * walk.x;
    let pl = at + fwd * xl + lft * ay;
    let pr = at + fwd * xr - lft * ay;
    let gl = terrain_height(pl) - centre;
    let gr = terrain_height(pr) - centre;
    f.ground = vec3<f32>(gl, gr, mix(0.5 * (gl + gr), min(gl, gr), 0.5));
    f.slope = vec2<f32>(
        (terrain_height(pl + fwd * d) - terrain_height(pl - fwd * d)) / (2.0 * d),
        (terrain_height(pr + fwd * d) - terrain_height(pr - fwd * d)) / (2.0 * d),
    );
    return f;
}

fn walk_leg(pos: vec3<f32>, normal: vec3<f32>, limb: u32, model: ModelInfo, walk: vec2<f32>, footing: Footing,
            at_ease: Stance) -> array<vec3<f32>, 2> {
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
        // A giant's great foot swings nearly flat: the tip is about 23 degrees on a
        // commander's stride, about 10 on the Behemoth's.
        pitch = 0.4 * clamp(24.0 / stride, 0.45, 1.0) * sin(u * 2.0 * PI);
    }
    // Each foot comes down on the ground under it; the hips ride over both.
    let left = pos.y > 0.0;
    let ground = select(footing.ground.y, footing.ground.x, left);
    // Standing (`idle_stance`): this foot's place and sole, and its hip as the body rolls.
    let rest = select(vec3<f32>(at_ease.fwd.y, at_ease.raise.y, at_ease.pitch.y),
        vec3<f32>(at_ease.fwd.x, at_ease.raise.x, at_ease.pitch.x), left);
    let hitch = select(-1.0, 1.0, left) * abs(model.leg_hip.y) * sin(at_ease.body.x);
    let hip = hip0 + vec2<f32>(0.0, walk_bob(walk, model).z + footing.ground.z + at_ease.body.z + hitch);
    if any(model.leg_hock.xyz != vec3<f32>(0.0)) {
        return hock_leg(pos, normal, limb, model, hip, ankle0 + foot * walk.x + vec2<f32>(0.0, ground),
            -pitch * walk.x + atan(select(footing.slope.y, footing.slope.x, left)));
    }
    let upright = stride_upright(pos, normal, limb, model, walk);
    let l1 = distance(knee0, hip0);
    let l2 = distance(ankle0, knee0);
    let want = ankle0 + foot * walk.x + vec2<f32>(rest.x, ground + rest.y) - hip;
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
        // The sole lies along the ground under it.
        turn = -pitch * walk.x + rest.z + atan(select(footing.slope.y, footing.slope.x, left));
        pivot0 = ankle0;
        pivot = ankle;
    }
    let p = rot_xz(upright[0] - vec3<f32>(pivot0.x, 0.0, pivot0.y), turn) + vec3<f32>(pivot.x, 0.0, pivot.y);
    return array<vec3<f32>, 2>(p, rot_xz(upright[1], turn));
}

// A reverse-kneed leg (`model.leg_hock`) posed to put its ankle at `goal`, the hips at
// `hip` (both in the leg's x, z plane), the sole turned `sole` about the ankle. Three bones:
// the tarsus from the hock down to the ankle leans with a share of the leg's swing (w) and
// folds up as the leg shortens (a lifted foot tucks its hock up and back, the planted leg
// crouches into both joints), then the thigh and shin are solved to put the hock on it, the
// knee staying forward of the leg as it rests.
fn hock_leg(pos: vec3<f32>, normal: vec3<f32>, limb: u32, model: ModelInfo, hip: vec2<f32>,
            goal: vec2<f32>, sole: f32) -> array<vec3<f32>, 2> {
    let hip0 = model.leg_hip.xz;
    let knee0 = model.leg_knee.xz;
    let hock0 = model.leg_hock.xz;
    let ankle0 = model.leg_ankle.xz;
    let rest = ankle0 - hip0;
    let want = goal - hip;
    let swing = atan2(want.y, want.x) - atan2(rest.y, rest.x);
    let squat = clamp(1.0 - length(want) / length(rest), -0.1, 0.5);
    let lean = model.leg_hock.w * swing + 1.2 * squat;
    let t0 = ankle0 - hock0;
    let c = cos(lean);
    let s = sin(lean);
    let tarsus = vec2<f32>(t0.x * c - t0.y * s, t0.x * s + t0.y * c);
    // The thigh and shin, hip to hock.
    let l1 = distance(knee0, hip0);
    let l2 = distance(hock0, knee0);
    let reach = goal - tarsus - hip;
    let d = clamp(length(reach), abs(l1 - l2) + 0.01, l1 + l2 - 0.01);
    let aim = atan2(reach.y, reach.x);
    let hock = hip + vec2<f32>(cos(aim), sin(aim)) * d;
    let a = knee0 - hip0;
    let b = hock0 - hip0;
    let side = select(1.0, -1.0, a.x * b.y - a.y * b.x > 0.0);
    let bend = acos(clamp((l1 * l1 + d * d - l2 * l2) / (2.0 * l1 * d), -1.0, 1.0));
    let knee = hip + vec2<f32>(cos(aim + side * bend), sin(aim + side * bend)) * l1;
    let ankle = hock + tarsus;

    var turn = 0.0;
    var pivot0 = vec2<f32>(0.0);
    var pivot = vec2<f32>(0.0);
    if limb == LIMB_THIGH {
        turn = atan2(knee.y - hip.y, knee.x - hip.x) - atan2(a.y, a.x);
        pivot0 = hip0;
        pivot = hip;
    } else if limb == LIMB_SHIN {
        turn = atan2(hock.y - knee.y, hock.x - knee.x) - atan2(hock0.y - knee0.y, hock0.x - knee0.x);
        pivot0 = knee0;
        pivot = knee;
    } else if limb == LIMB_TARSUS {
        turn = lean;
        pivot0 = hock0;
        pivot = hock;
    } else {
        turn = sole;
        pivot0 = ankle0;
        pivot = ankle;
    }
    let p = rot_xz(pos - vec3<f32>(pivot0.x, 0.0, pivot0.y), turn) + vec3<f32>(pivot.x, 0.0, pivot.y);
    return array<vec3<f32>, 2>(p, rot_xz(normal, turn));
}

// A many-legged walker (`models::Crawl`, `model.crawl`): where the body rides over its
// feet, from the ground under its first and last pairs (as `walk_ground` does for two feet).
fn crawl_body_ground(e: Entity, model: ModelInfo, t: f32) -> f32 {
    let heading = lerp_angle(e.prev_heading, e.heading, t);
    let at = mix(e.prev_pos, e.pos, t).xy;
    let fwd = vec2<f32>(cos(heading), sin(heading));
    let lft = vec2<f32>(-fwd.y, fwd.x);
    let centre = terrain_height(at);
    let last = u32(model.crawl[0].x + 0.5) - 1u;
    let front = model.crawl[3u].xy;
    let back = model.crawl[3u + 3u * last].xy;
    let g0 = terrain_height(at + fwd * front.x + lft * front.y) - centre;
    let g1 = terrain_height(at + fwd * front.x - lft * front.y) - centre;
    let g2 = terrain_height(at + fwd * back.x + lft * back.y) - centre;
    let g3 = terrain_height(at + fwd * back.x - lft * back.y) - centre;
    return mix(0.25 * (g0 + g1 + g2 + g3), min(min(g0, g1), min(g2, g3)), 0.4);
}

// A leg vertex of a many-legged walker, posed for this moment of the stride. Each leg has
// two bones, hip to knee and knee to the foot's tip, in the vertical plane through its
// hip and foot; the foot is planted and passes back under the body, or lifts and swings
// forward, each pair at its own phase (the right leg half a cycle after the left). The
// plane turns about the hip to follow the foot, and the bones are solved in it. `body` is
// how far the hips ride off their rest (the body's bob and the ground under it).
fn crawl_leg(pos: vec3<f32>, normal: vec3<f32>, limb: u32, pair: u32, model: ModelInfo,
             walk: vec2<f32>, e: Entity, t: f32, body: vec3<f32>) -> array<vec3<f32>, 2> {
    let left = pos.y > 0.0;
    let flip = vec3<f32>(1.0, select(-1.0, 1.0, left), 1.0);
    let hip0 = model.crawl[1u + 3u * pair].xyz * flip;
    let knee0 = model.crawl[2u + 3u * pair].xyz * flip;
    let foot0 = model.crawl[3u + 3u * pair].xyz * flip;
    let phase = fract(walk.y - model.crawl[1u + 3u * pair].w + select(0.5, 0.0, left));
    let stride = model.leg_hip.w;
    let lift = model.leg_knee.w;
    let stance = model.leg_ankle.w;
    let reach = stance * stride;
    var step = vec2<f32>(0.0);
    if phase < stance {
        step.x = reach * (0.5 - phase / stance);
    } else {
        let u = (phase - stance) / (1.0 - stance);
        step = vec2<f32>(reach * (u * u * (3.0 - 2.0 * u) - 0.5), lift * sin(u * PI));
    }
    // The foot comes down on the ground under it.
    let heading = lerp_angle(e.prev_heading, e.heading, t);
    let at = mix(e.prev_pos, e.pos, t).xy;
    let fwd = vec2<f32>(cos(heading), sin(heading));
    let lft = vec2<f32>(-fwd.y, fwd.x);
    let fx = foot0.x + step.x * walk.x;
    let ground = terrain_height(at + fwd * fx + lft * foot0.y) - terrain_height(at);
    let foot = vec3<f32>(fx, foot0.y, foot0.z + step.y * walk.x + ground);
    let hip = hip0 + body;

    // The leg's plane at rest and now, and the bones in it: r out from the hip, z up.
    let az0 = atan2(foot0.y - hip0.y, foot0.x - hip0.x);
    let az = atan2(foot.y - hip.y, foot.x - hip.x);
    let dir0 = vec2<f32>(cos(az0), sin(az0));
    let k0 = vec2<f32>(dot(knee0.xy - hip0.xy, dir0), knee0.z - hip0.z);
    let f0 = vec2<f32>(dot(foot0.xy - hip0.xy, dir0), foot0.z - hip0.z);
    let f1 = vec2<f32>(length(foot.xy - hip.xy), foot.z - hip.z);
    let l1 = length(k0);
    let l2 = distance(f0, k0);
    let d = clamp(length(f1), abs(l1 - l2) + 0.01, l1 + l2 - 0.01);
    let aim = atan2(f1.y, f1.x);
    // The knee stays on the side of the leg it rests on (up, for a spider's knee).
    let side = select(1.0, -1.0, k0.x * f0.y - k0.y * f0.x > 0.0);
    let bend = acos(clamp((l1 * l1 + d * d - l2 * l2) / (2.0 * l1 * d), -1.0, 1.0));
    let k1 = vec2<f32>(cos(aim + side * bend), sin(aim + side * bend)) * l1;
    let tip = vec2<f32>(cos(aim), sin(aim)) * d;

    var q = rot_z(pos - hip0, -az0);
    var m = rot_z(normal, -az0);
    if limb == LIMB_THIGH {
        let turn = atan2(k1.y, k1.x) - atan2(k0.y, k0.x);
        q = rot_xz(q, turn);
        m = rot_xz(m, turn);
    } else {
        let turn = atan2(tip.y - k1.y, tip.x - k1.x) - atan2(f0.y - k0.y, f0.x - k0.x);
        q = rot_xz(q - vec3<f32>(k0.x, 0.0, k0.y), turn) + vec3<f32>(k1.x, 0.0, k1.y);
        m = rot_xz(m, turn);
    }
    return array<vec3<f32>, 2>(rot_z(q, az) + hip, rot_z(m, az));
}

// How busy a many-legged walker is, zero at rest to one fighting or building: its tail
// holds steadier and its pincers rise and open while it is.
fn crawl_busy(e: Entity, t: f32) -> f32 {
    let aim = abs(mix(e.arm_pitch.x, e.arm_pitch.y, t)) * 6.0
        + abs(mix(e.arm_pitch.z, e.arm_pitch.w, t)) * 6.0
        + mix(e.prev_recoil, e.recoil, t) * 4.0
        + clamp(mix(e.prev_deploy, e.deploy, t), 0.0, 1.0) * 4.0;
    return clamp(aim, 0.0, 1.0);
}

// How far a many-legged walker's body sinks as it sets itself for a shot: its main gun's
// kick, or a held beam's brace (`mirror::beam_brace`, which rises and falls smoothly as the
// projector runs up and down), a sixteenth of its hips' height at full.
fn crawl_set(e: Entity, model: ModelInfo, t: f32) -> f32 {
    let brace = clamp(mix(e.prev_recoil, e.recoil, t), 0.0, 1.0);
    return -model.crawl[1].z * 0.0625 * brace * crawl_alive(e);
}

// Whether a unit's living parts move at all: not a wreck, a ghost, a site or in a factory.
fn crawl_alive(e: Entity) -> f32 {
    return select(1.0, 0.0, (e.owner_flags & (KIND_WRECK | KIND_GHOST | FLAG_UNDER_CONSTRUCTION | FLAG_IN_FACTORY)) != 0u);
}

// A tail's joint `j` (`Crawl::tail_joints`) at rest, (x, z).
fn tail_joint(model: ModelInfo, j: u32) -> vec2<f32> {
    let v = model.crawl[13u + j / 2u];
    return select(v.xy, v.zw, (j & 1u) == 1u);
}

// How a many-legged walker's tail is posed now, for segment `seg` (past the last joint:
// the turret's own pieces on its tip). Each joint turns its segment, and everything above
// it, by a pitch in the tail's plane and a little yaw across it; the segment is carried
// where the ones under it put its joint. Returns its joint at rest, where that joint is
// now, the segment's turn (columns), and in [5].x the pitches summed (what the tip has
// turned in the tail's plane).
//
// Each joint bends by: a slow wave running up the tail and a slower breath, the way a
// living tail is never still, with a smaller sway from side to side (steadier while it
// fights, livelier as it walks); a lean forward over the head once it has something to
// strike; a share of the stinger's pitch in the top joints, so the whole tail aims; and a
// rear-back on every shot. A held beam's "kick" is its brace (`mirror::beam_brace`): the
// tail stiffens onto the shot, its top set back against the stream and throbbing slowly
// with it, and eases off as the projector runs down. The turn toward the target is the
// turret's, about the root.
fn tail_pose(e: Entity, model: ModelInfo, walk: vec2<f32>, t: f32, seg: u32) -> array<vec3<f32>, 6> {
    let time = globals.camera.w;
    let count = u32(model.crawl[0].w + 0.5);
    let last = max(count, 2u) - 1u;
    let s = min(seg, last);
    let seed = hash11(f32(e.unit_id & 0xFFFFu) * 0.517 + 1.7) * 6.2831853;
    let alive = crawl_alive(e);
    let busy = crawl_busy(e, t);
    let gun = mix(e.arm_pitch.x, e.arm_pitch.y, t);
    let kick = mix(e.prev_recoil, e.recoil, t);
    // The aim: the sim holds the turret within the body's `aim_arc` and turns the body for
    // the rest. The top four joints share it, 1, 2, 3 and 4 tenths from the lowest, so the
    // tail bends round at its top instead of swivelling whole. The unit file's `turret_at`
    // is the single pivot that best matches this chain (models/regency/commander.rs test).
    let aim = lerp_angle(e.prev_turret_yaw, e.turret_yaw, t);
    let lively = alive * mix(1.0, 0.45, busy) * (1.0 - 0.8 * clamp(kick, 0.0, 1.0)) * (1.0 + 0.5 * walk.x);
    let throb = 0.012 * kick * sin(time * 7.0 + seed);
    var r0 = vec3<f32>(1.0, 0.0, 0.0);
    var r1 = vec3<f32>(0.0, 1.0, 0.0);
    var r2 = vec3<f32>(0.0, 0.0, 1.0);
    var bent = 0.0;
    let j0 = tail_joint(model, 0u);
    var rest = vec3<f32>(j0.x, 0.0, j0.y);
    var at = rest;
    for (var j = 0u; j <= s; j = j + 1u) {
        let u = f32(j) / f32(last);
        var d = lively * (0.5 + 0.5 * u) * (
            0.034 * sin(time * (1.05 + 0.5 * walk.x) - u * 3.4 + seed)
            + 0.02 * sin(time * 0.43 + u * 1.9 + seed * 1.7));
        d += alive * busy * -0.045 * smoothstep(0.35, 1.0, u);
        d += alive * select(0.0, 0.1 * gun, j + 3u > last);
        d += alive * (0.06 * kick + throb) * smoothstep(0.5, 1.0, u);
        let y = lively * (0.4 + 0.6 * u) * (
            0.022 * sin(time * 0.71 - u * 2.6 + seed * 2.3)
            + 0.012 * sin(time * 1.63 + u * 4.1 + seed * 0.7))
            + select(0.0, aim * f32(j + 4u - last) * 0.1, j + 4u > last);
        bent += d;
        // This joint's turn, in the frame the joints under it left: yaw, then pitch.
        let cy = cos(y);
        let sy = sin(y);
        let cp = cos(d);
        let sp = sin(d);
        let c0 = vec3<f32>(cy * cp, sy * cp, sp);
        let c1 = vec3<f32>(-sy, cy, 0.0);
        let c2 = vec3<f32>(-cy * sp, -sy * sp, cp);
        let n0 = r0 * c0.x + r1 * c0.y + r2 * c0.z;
        let n1 = r0 * c1.x + r1 * c1.y + r2 * c1.z;
        let n2 = r0 * c2.x + r1 * c2.y + r2 * c2.z;
        r0 = n0;
        r1 = n1;
        r2 = n2;
        if j < s {
            let jn = tail_joint(model, j + 1u);
            let next = vec3<f32>(jn.x, 0.0, jn.y);
            let q = next - rest;
            at += r0 * q.x + r1 * q.y + r2 * q.z;
            rest = next;
        }
    }
    return array<vec3<f32>, 6>(rest, at, r0, r1, r2, vec3<f32>(bent, 0.0, 0.0));
}

// How hard a pincer is throwing (`Crawl::throws`: the weapon each claw fires, in the
// jaw hinge's w): a sharp pulse off its weapon's kick (`HousePose`, one per shot of a
// salvo), gone within a fifth of a second. Zero for a claw that throws nothing.
fn claw_throw(model: ModelInfo, e: Entity, t: f32, side: f32) -> f32 {
    let code = u32(model.crawl[20u].w + 0.5);
    let slot = select((code >> 4u) & 15u, code & 15u, side > 0.0);
    if slot == 0u || (e.status[1] >> UNIT_HOUSE_SHIFT) == 0u {
        return 0.0;
    }
    let w = slot - 1u;
    let kicks = house_pose(e).kick[w / 2u];
    let k = select(kicks.xy, kicks.zw, (w & 1u) == 1u);
    return pow(clamp(mix(k.x, k.y, t), 0.0, 1.0), 6.0);
}

// A pincer vertex (`rig::CLAW_ARM`, `CLAW_JAW`), posed: the moving finger opens about its
// hinge, then the arm swings about its shoulder. At rest the arms sway a little and each
// claw works open slowly and snaps shut, out of step with the other; walking, they swing
// against the stride; fighting or building, they rise and gape, holding the charge between
// the fingers. Each throw snaps the finger wide and kicks the arm up and out.
fn claw_pose(pos: vec3<f32>, normal: vec3<f32>, jaw: bool, model: ModelInfo, e: Entity,
             walk: vec2<f32>, t: f32) -> array<vec3<f32>, 2> {
    let time = globals.camera.w;
    let side = select(-1.0, 1.0, pos.y > 0.0);
    let flip = vec3<f32>(1.0, side, 1.0);
    let shoulder = model.crawl[19u].xyz * flip;
    let hinge = model.crawl[20u].xyz * flip;
    let seed = hash11(f32(e.unit_id & 0xFFFFu) * 0.311 + side * 2.9);
    let alive = crawl_alive(e);
    let busy = crawl_busy(e, t);
    let flung = alive * claw_throw(model, e, t, side);
    let still = 1.0 - walk.x;
    var p = pos;
    var n = normal;
    if jaw {
        let c = fract(time / 3.7 + seed);
        let snap = smoothstep(0.0, 0.8, c) * (1.0 - smoothstep(0.86, 0.9, c));
        let open = alive * (0.06 + 0.2 * snap * still * (1.0 - busy) + 0.42 * busy) + 0.38 * flung;
        // The finger inside the claw opens inward, away from the fixed one.
        p = rot_z(p - hinge, -side * open) + hinge;
        n = rot_z(n, -side * open);
    }
    let swing = 0.1 * cos(walk.y * 6.2831853 + select(0.0, 3.14159, side < 0.0)) * walk.x;
    let yaw = alive * (0.05 * sin(time * 0.8 + seed * 40.0) * still + swing - 0.1 * busy) * side
        + 0.08 * flung * side;
    let pitch = alive * (0.035 * sin(time * 0.61 + seed * 23.0) + 0.16 * busy) + 0.14 * flung;
    p = rot_z(rot_xz(p - shoulder, pitch), yaw) + shoulder;
    n = rot_z(rot_xz(n, pitch), yaw);
    return array<vec3<f32>, 2>(p, n);
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

// `v` turned `angle` about the unit `axis` (Rodrigues).
fn rot_about(v: vec3<f32>, axis: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return v * c + cross(axis, v) * s + axis * dot(axis, v) * (1.0 - c);
}

// A ground stake (`gpu_consts::stake`, `mc_models::stakes`), authored planted, at deploy
// share `planted`. Each corner takes its turn (one diagonal, then the other, front first):
// its tube swings down from lying along the fender, then its spike fires out, still
// gathering speed as it strikes, and the tube is thrown back up its line and settles.
fn stake_pose(p0: vec3<f32>, n0: vec3<f32>, spike: bool, planted: f32, s: f32) -> array<vec3<f32>, 2> {
    var p = p0;
    var n = n0;
    let front = p.x > 0.0;
    let left = p.y > 0.0;
    let sx = select(-1.0, 1.0, front);
    let sy = select(-1.0, 1.0, left);
    let order = select(2.0, 0.0, front == left) + select(1.0, 0.0, front);
    let start = STAKE_START + order * STAKE_STEP;
    let hinge = vec3<f32>(select(STAKE_REAR_X, STAKE_FRONT_X, front), sy * STAKE_Y, STAKE_Z) * s;
    let down = normalize(vec3<f32>(sx * STAKE_OUT_X, sy * STAKE_OUT_Y, -STAKE_DOWN));
    let stowed = vec3<f32>(-sx, 0.0, 0.0);
    let fire = start + STAKE_FIRE;
    let struck = fire + STAKE_FIRE_TIME;
    if spike {
        let fired = clamp((planted - fire) / STAKE_FIRE_TIME, 0.0, 1.0);
        p -= down * (1.0 - fired * fired * fired) * STAKE_TRAVEL * s;
    }
    // The tube's kick: thrown back hard the instant the spike strikes, then eased home.
    let after = (planted - struck) / STAKE_KICK_TIME;
    if !spike && after > 0.0 && after < 1.0 {
        p -= down * STAKE_KICK * s * (1.0 - after) * (1.0 - after) * min(after * 8.0, 1.0);
    }
    let swung = smoothstep(start, start + STAKE_SWING, planted);
    if swung < 0.999 {
        let axis = normalize(cross(down, stowed));
        let angle = acos(clamp(dot(down, stowed), -1.0, 1.0)) * (1.0 - swung);
        p = hinge + rot_about(p - hinge, axis, angle);
        n = rot_about(n, axis, angle);
    }
    return array<vec3<f32>, 2>(p, n);
}

// A spacecraft (`ModelInfo::capital`, `models::capital`): legs or stern drives.
fn capital_ship(model: ModelInfo) -> bool {
    return model.capital[0].w != 0.0 || model.capital[4].x != 0.0;
}

// How hard a spacecraft's stern drives push, 0 idle to 1: its speed against its cruise
// (`ModelInfo::capital[3].w`), and its climb or descent. Stepped a tick at a time, like the
// drives' glow; a big hull's speed changes slowly.
fn drive_thrust(model: ModelInfo, e: Entity) -> f32 {
    let motion = e.pos - e.prev_pos;
    let cruise = max(model.capital[3].w, 0.5);
    return clamp(length(motion.xy) / cruise + abs(motion.z) / (cruise * 0.6), 0.0, 1.0);
}

// How far a spacecraft's drive nozzles are swung between ticks (radians, `gpu_consts::drive`):
// toward the side the nose turns to, eased by the renderer (drive_swing.rs, which the
// plumes in capital_fx.rs follow too).
fn drive_vector(e: Entity, t: f32) -> f32 {
    return mix(e.drive_swing.x, e.drive_swing.y, t);
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

// A hovering aircraft is never quite still: a slow heave on its lift (model-space z, x)
// and a sway on its roll (y). Zero on anything else.
fn hover_heave(e: Entity, model: ModelInfo, t: f32) -> vec2<f32> {
    if (model.icon & 0x80000u) == 0u || (model.icon & 0x40000u) == 0u
        || (e.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST | FLAG_UNDER_CONSTRUCTION | FLAG_IN_FACTORY)) != 0u {
        return vec2<f32>(0.0);
    }
    let time = globals.camera.w;
    let seed = f32(e.unit_id & 255u) * 0.37;
    // A spacecraft stands still on its legs: the heave fades out as the gear comes
    // down, and a capital hull only rolls a hair in flight.
    let transport = capital_ship(model);
    let calm = select(1.0, 1.0 - smoothstep(0.3, 0.9, capital_gear(capital_height(e, t))), transport);
    return vec2<f32>(
        (0.09 * sin(time * 1.15 + seed) + 0.04 * sin(time * 2.9 + seed * 1.7)) * calm,
        select(0.012, 0.003, transport) * sin(time * 0.8 + seed) * calm,
    );
}

// An aircraft's pitch. A climb pitches the fuselage; hovering over sloping ground stays level.
fn air_pitch(e: Entity, model: ModelInfo, t: f32) -> f32 {
    let travel = e.pos - e.prev_pos;
    var pitch = clamp(atan2(travel.z, max(length(travel.xy), 2.0)), -0.20, 0.20);
    if (model.icon & 0x80000u) != 0u {
        // A hover aircraft leans with its lift (`hover_flight::lean`): slot 1.
        pitch = mix(e.arm_pitch.z, e.arm_pitch.w, t);
    }
    // The Thunderhead, a lift ship in flight, and any spacecraft whose hull pitches
    // to lay a spinal gun (`combat::spinal_gun`) carry the hull's pitch in slot 0.
    if (model.icon & 0x1100000u) != 0u || capital_ship(model) {
        pitch = mix(e.arm_pitch.x, e.arm_pitch.y, t);
    }
    return pitch;
}

struct Frame {
    origin: vec3<f32>,
    fwd: vec3<f32>,
    left: vec3<f32>,
    up: vec3<f32>,
}

// The frame an aircraft is drawn in, as `vs_main` draws it: turned by its heading,
// pitched (`air_pitch`), rolled by its bank and sway, lifted by its heave. A drone docked
// on it is drawn in this (`DOCK_RIDING`).
fn riding_frame(src: Entity, model: ModelInfo, t: f32) -> Frame {
    var f: Frame;
    let heading = lerp_angle(src.prev_heading, src.heading, t);
    f.up = vec3<f32>(0.0, 0.0, 1.0);
    f.fwd = vec3<f32>(cos(heading), sin(heading), 0.0);
    f.left = cross(f.up, f.fwd);
    var sway = 0.0;
    var heave = 0.0;
    if (model.icon & 0x40000u) != 0u
        && (src.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST | FLAG_UNDER_CONSTRUCTION | FLAG_IN_FACTORY)) == 0u {
        let pitch = air_pitch(src, model, t);
        let pitch_fwd = f.fwd * cos(pitch) + f.up * sin(pitch);
        f.up = f.up * cos(pitch) - f.fwd * sin(pitch);
        f.fwd = pitch_fwd;
        let h = hover_heave(src, model, t);
        heave = h.x;
        sway = h.y;
    }
    let bank = mix(src._pad2.x, src._pad2.y, t) + sway;
    let bank_left = f.left * cos(bank) + f.up * sin(bank);
    f.up = f.up * cos(bank) - f.left * sin(bank);
    f.left = bank_left;
    f.origin = mix(src.prev_pos, src.pos, t) + f.up * heave;
    return f;
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    let entity_index = visible[in.instance];
    let e = load_entity(entity_index);
    let model = models[e.blueprint];
    let t = globals.sun.w;
    let time = globals.camera.w;
    var scale = 1.0;
    if (e.owner_flags & KIND_PROP) != 0u && e.packed != 0u {
        scale = f32(e.packed) * 0.001;
    }
    // A tree's own stretch in height (renderer/fallen_trees.rs `height_stretch`).
    let stretch = select(1.0, e.arm_pitch.w, (e.owner_flags & KIND_PROP) != 0u && e.arm_pitch.w > 0.0);
    // Which props the pre-pass and each shadow cascade draw at all is decided per
    // instance, by the cull (cull.wgsl `other_lists`).

    var p = in.pos;
    var n = in.normal;
    // A settled wreck's pose (wreck.wgsl). A hull's inside is its model mirrored across
    // the centre line, so the faces that looked out now look in; it casts no shadow of
    // its own.
    let wreck = wreck_pose(e, model, globals.sun.w);
    if wreck.inner {
        if (push.pass_kind & PASS_KIND_MASK) == PASS_SHADOW {
            var hidden: VsOut;
            hidden.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
            return hidden;
        }
        p.y = -p.y;
        n.y = -n.y;
    }
    let unwarped = p;
    // A core mine stands in the sea on stilts, raised clear of the water, with no pit; on
    // land it has its pit and no stilts (`models::Pit`).
    let rig_afloat = model.pit.y > 0.0 && terrain_height(e.pos.xy) < globals.map.z - 0.5;
    if (in.part == PART_AFLOAT && !rig_afloat) || (in.part == PART_ASHORE && rig_afloat) {
        var hidden: VsOut;
        hidden.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
        return hidden;
    }
    // A joining wall draws in each quarter only the piece its neighbours call for.
    if !wall_piece_shown(in.part, e.status[2]) {
        var hidden: VsOut;
        hidden.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
        return hidden;
    }
    if rig_afloat && in.part != PART_AFLOAT {
        p.z += model.pit_feed.w;
    }
    // A hull field poses the shared shell direction instead, so a corner's faces stay joined.
    var shell_stretch = 1.0;
    if push.pass_kind == PASS_HULL {
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
        tree = tree_air(e.pos, max(model.height * scale * stretch, 1.0), e.unit_id, blast_sway(entity_index));
    }
    if in.material == MAT_FOLIAGE {
        // Leaf cards are lit as the crown they belong to: the mesh carries the
        // crown's outward normal at each corner (`MeshBuilder::leaf_card`).
        // Small independent leaf movement on top of a slow sway of the crown.
        n = select(normalize(vec3<f32>(in.pos.xy * 0.18, 0.7)), in.face.xyz, dot(in.face.xyz, in.face.xyz) > 0.25);
        let flex = clamp(in.pos.z / max(model.height, 1.0), 0.0, 1.0);
        // The corners of a card move nearly together, so a card flutters rather
        // than warping. Stirred air adds a quicker shiver on top; its rate is
        // fixed (scaling the phase by the stir would race as the stir changes).
        let card = f32((in.surface >> 8u) & 0x7Fu);
        let phase = time * 1.5 + (in.pos.x + in.pos.y * 0.7) * 0.35 + card * 0.37 + f32(e.unit_id % 31u);
        let shiver = time * 6.3 + card * 1.71 + in.pos.z * 0.3;
        p += (vec3<f32>(sin(phase), cos(phase * 0.83), 0.0) * 0.04
            + vec3<f32>(sin(shiver), cos(shiver * 1.13), sin(shiver * 0.71) * 0.5) * 0.07 * tree.z) * flex;
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
    var footing: Footing;
    let crawls = walks && model.crawl[0].x > 0.5;
    if walks {
        walk = walk_state(e, model);
        footing = walk_ground(e, model, walk, t);
        if crawls {
            footing.ground.z = crawl_body_ground(e, model, t);
        }
    }
    // A walker with a head is never quite still (`idle_pose`).
    var idle = vec4<f32>(0.0);
    if walks && model.surface.w > 0.0 && in.part == PART_TURRET
        && (e.owner_flags & (KIND_GHOST | FLAG_UNDER_CONSTRUCTION | FLAG_IN_FACTORY)) == 0u {
        idle = idle_pose(e, model, walk, time);
    }
    // A two-legged walker with a head stands at ease (`idle_stance`).
    var stance: Stance;
    if walks && !crawls && model.surface.w > 0.0 && all(model.leg_hock.xyz == vec3<f32>(0.0))
        && (e.owner_flags & (KIND_GHOST | FLAG_UNDER_CONSTRUCTION | FLAG_IN_FACTORY)) == 0u {
        stance = idle_stance(e, model, walk, time);
    }
    // A hull going down (`WRECK_SINKING`, 2) is posed like a falling wreck: whole, pitched and
    // rolled by the sim as it sinks, not crumpled. It settles into an ordinary wreck on the seabed.
    let falling = (e.owner_flags & KIND_WRECK) != 0u && (e.packed == 1u || e.packed == 2u);
    // A trampled tree tips over from its foot (renderer/fallen_trees.rs).
    let toppled = (e.owner_flags & KIND_PROP) != 0u && e.arm_pitch.x != 0.0;
    if (e.owner_flags & KIND_WRECK) != 0u && !falling {
        // A fresh wreck carries its age (`mirror::UnitInstance::gait`); one long settled lies as it fell.
        p = wrecked(p, in.part, model, hash11(f32(e.unit_id & 0xFFFFu)), e.turret_yaw, wreck.age, e.health, wreck);
        // A section lies about its own middle.
        p.x -= wreck.centre;
    } else if in.part == PART_TURRET && !(limb >= LIMB_HOUSE && limb < LIMB_HOUSE + 4u) {
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
        // A charge gun's working gear moves with its charge, before the gun pitches.
        let gear = (in.rig >> CHARGE_GEAR_SHIFT) & CHARGE_GEAR_MASK;
        if gear != 0u && model.charge_gear.w > 0.0 && e.mount.w == CHARGE_RECORD {
            let posed = charge_gear_pose(p, n, gear, model.charge_gear, e, time);
            p = posed[0];
            n = posed[1];
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
                var pitch = select(tool_pitch + idle.w, gun_pitch + idle.z, limb == LIMB_ARM_GUN);
                // On a tail the prongs ride its tip, which already turns: they take only
                // the rest of the aim, so they stay on the target while the tail moves.
                if model.crawl[0].w > 1.5 {
                    pitch -= tail_pose(e, model, walk, t, 15u)[5].x;
                }
                // A breech door swings on its hinge first, then rides the gun.
                if (in.rig & BREECH_RIG) != 0u && model.breech.w != 0.0 {
                    let swing = model.breech.w * breech_open(e.prev_recoil, e.recoil, t);
                    p = rot_xz(p - model.breech.xyz, swing) + model.breech.xyz;
                    n = rot_xz(n, swing);
                }
                p = rot_xz(p - elbow, pitch) + elbow;
                n = rot_xz(n, pitch);
                // The tube kicks back along its aim the instant it fires, then runs home.
                // Stepping the slide ten times a second reads as jitter, same as the turret.
                if (in.rig & RIG_RECOIL) != 0u && model.recoil.w > 0.0 {
                    let kick = arm_kick(e, t, p.y);
                    p -= rot_xz(model.recoil.xyz, pitch) * (model.recoil.w * kick);
                }
            }
        } else if (in.rig & RIG_RECOIL) != 0u && model.recoil.w > 0.0 && limb != LIMB_MOUNT {
            // A fixed turret gun: slide along the bore in turret space, then yaw with the turret.
            let kick = mix(e.prev_recoil, e.recoil, t);
            p -= model.recoil.xyz * (model.recoil.w * kick);
        }
        // The turret glides between ticks like the hull does; stepping it ten times a second reads as jitter.
        var yaw = lerp_angle(e.prev_turret_yaw, e.turret_yaw, t);
        if model.crawl[0].w > 1.5 {
            // A jointed tail (`tail_pose`): each segment turns about its joint, carried where
            // the ones under it put it; the turret's own pieces ride the last joint. The aim
            // is in the pose (its top joints bend round to the target), so nothing swivels
            // the tail whole.
            let seg = select(15u, (in.rig >> 16u) & 15u, limb == LIMB_TAIL);
            let pose = tail_pose(e, model, walk, t, seg);
            let q = p - pose[0];
            p = pose[1] + pose[2] * q.x + pose[3] * q.y + pose[4] * q.z;
            n = pose[2] * n.x + pose[3] * n.y + pose[4] * n.z;
            yaw = 0.0;
        } else if limb == LIMB_TAIL && model.crawl[0].z > model.crawl[0].y {
            // A tail bends toward the turret's facing: its root stays on the body, its top
            // (where the turret's own pieces ride) takes the whole turn, and between them it
            // sways a little, the way a living tail is never still.
            let w = smoothstep(model.crawl[0].y, model.crawl[0].z, p.z);
            let calm = select(1.0, 0.0, (e.owner_flags & (KIND_WRECK | KIND_GHOST | FLAG_UNDER_CONSTRUCTION | FLAG_IN_FACTORY)) != 0u);
            let sway = calm * w * (1.0 - w) * (0.5 * sin(time * 0.8 + f32(e.unit_id % 29u)) + 0.25 * sin(time * 1.9 + f32(e.unit_id % 11u)));
            yaw = yaw * w + sway;
        }
        let pivot = model.turret_pivot.xyz;
        p = rot_z(p - pivot, yaw) + pivot;
        n = rot_z(n, yaw);
    } else if in.part == 0u && limb == LIMB_TAIL && model.crawl[19u].w > 0.5 {
        // A many-legged walker's pincers (`claw_pose`).
        let posed = claw_pose(p, n, ((in.rig >> 16u) & 15u) == 15u, model, e, walk, t);
        p = posed[0];
        n = posed[1];
    } else if (model.icon & 0x2000000u) != 0u && in.part == 7u {
        // Courier stern bay plug doors, and the skylight leaves over the bay, slide into its shoulders.
        let open = smoothstep(0.0, 1.0, mix(e.prev_deploy, e.deploy, t));
        p.y += sign(p.y) * open * 14.2;
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
        // A spacecraft drive's nozzle (`CapitalRig::drives`, `gpu_consts::drive`): its
        // petals open out with thrust, and the whole of it swivels on its gimbal ball to
        // throw the exhaust to the side the nose is turning to.
        let dr = model.capital[4];
        let size = max(model.capital[6].y, 0.01);
        let cy = sign(p.y) * select(dr.z, dr.w, abs(p.y) > 0.5 * (dr.z + dr.w));
        let mouth = vec3<f32>(dr.x, cy, dr.y);
        let q = p - mouth;
        let aft = DRIVE_PETAL_HINGE - q.x / size;
        let r = length(q.yz);
        if aft > 0.0 && r > 0.01 {
            let flare = mix(DRIVE_FLARE_IDLE, DRIVE_FLARE_FULL, drive_thrust(model, e));
            p = mouth + vec3<f32>(q.x, q.yz * ((r + aft * size * tan(flare)) / r));
        }
        let gimbal = mouth + vec3<f32>(DRIVE_GIMBAL * size, 0.0, 0.0);
        let swing = -drive_vector(e, t);
        p = rot_z(p - gimbal, swing) + gimbal;
        n = rot_z(n, swing);
    } else if (in.part == 5u || in.part == 6u) && model.vtol[0].w > 0.0 {
        let front = in.part == 5u;
        let fans = model.vtol[0].w > 1.5;
        let at = model.vtol[select(1, 0, front)].xyz;
        let pivot = vec3<f32>(at.x, sign(p.y) * at.y, at.z);
        // The fan or turbine turns about the pod's own axis (authored along x) before
        // the pod tilts; the two pods on a side run out of step.
        if (in.rig & RIG_SPIN) != 0u && (e.owner_flags & (FLAG_UNDER_CONSTRUCTION | FLAG_IN_FACTORY | KIND_GHOST)) == 0u {
            let rate = select(30.0, 17.0, fans);
            let turn = time * rate + f32(e.unit_id & 255u) + select(1.3, 0.0, front);
            let q = p - pivot;
            p = vec3<f32>(q.x, q.y * cos(turn) - q.z * sin(turn), q.y * sin(turn) + q.z * cos(turn)) + pivot;
            n = vec3<f32>(n.x, n.y * cos(turn) - n.z * sin(turn), n.y * sin(turn) + n.z * cos(turn));
        }
        // The pods drive the lean the sim gives the hull (`hover_flight`): forward to
        // go, back to brake, the outer pair forward in a turn.
        let tilt = vtol_tilt(e, t, p.y > 0.0, front);
        p = rot_xz(p - pivot, tilt) + pivot;
        n = rot_xz(n, tilt);
    } else if limb >= LIMB_HOUSE && limb < LIMB_HOUSE + 4u
        && house_weapon_of(model, limb - LIMB_HOUSE + select(0u, 4u, (in.rig & RIG_HOUSE_HIGH) != 0u)) > 0.5
        && (e.status[1] >> UNIT_HOUSE_SHIFT) > 0u {
        // A gun house of its own on the hull (a warship's turret): turns about its pivot by its
        // weapon's yaw off the hull; what recoils inside it pitches about the pivot and kicks back.
        let slot = limb - LIMB_HOUSE + select(0u, 4u, (in.rig & RIG_HOUSE_HIGH) != 0u);
        let house = house_of(model, slot);
        let w = u32(house_weapon_of(model, slot) + 0.5) - 1u;
        let hp = house_pose(e);
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
            } else if (in.rig & RIG_SPIN) != 0u && model.spin.w > 0.0 {
                // Rotary barrels in any other house (the Behemoth's arm, a carrier's close-in
                // gun) turn with the weapon's spin-up about the axis the model gives
                // (`Model::spins`, along x), before the house pitches.
                let turn = mix(e.spin_recoil.x, e.spin_recoil.y, t);
                let axis = vec3<f32>(0.0, model.spin.y, model.spin.z);
                let q = p - axis;
                p = vec3<f32>(q.x, q.y * cos(turn) - q.z * sin(turn), q.y * sin(turn) + q.z * cos(turn)) + axis;
                n = vec3<f32>(n.x, n.y * cos(turn) - n.z * sin(turn), n.y * sin(turn) + n.z * cos(turn));
            }
            // Capacitor rings' lugs on a charging gun turn about its bore: the rotary axis
            // mirrored across the centreline (the Behemoth's bore arm mirrors its gatling).
            let coil_pat = in.surface & 0xFFu;
            if (coil_pat == PAT_COIL_TURN || coil_pat == PAT_COIL_TURN + 1u) && e.mount.w == CHARGE_RECORD
                && model.spin.w > 0.0 {
                let turn = coil_turn(e, time) * select(1.0, -1.0, coil_pat != PAT_COIL_TURN);
                let axis = vec3<f32>(0.0, -model.spin.y, model.spin.z);
                let q = p - axis;
                p = vec3<f32>(q.x, q.y * cos(turn) - q.z * sin(turn), q.y * sin(turn) + q.z * cos(turn)) + axis;
                n = vec3<f32>(n.x, n.y * cos(turn) - n.z * sin(turn), n.y * sin(turn) + n.z * cos(turn));
            }
            p.x -= house.w * mix(kick.x, kick.y, t);
            let pitch = mix(pose.z, pose.w, t);
            p = rot_xz(p - pivot, pitch) + pivot;
            n = rot_xz(n, pitch);
        }
        let yaw = lerp_angle(pose.x, pose.y, t);
        if in.part == PART_TURRET {
            // A house riding the turret (a giant's shoulder flak): the weapon's yaw is off the
            // hull, so it turns by what it is off the turret, and the turret carries it round.
            let turret = lerp_angle(e.prev_turret_yaw, e.turret_yaw, t);
            p = rot_z(p - pivot, yaw - turret) + pivot;
            n = rot_z(n, yaw - turret);
            let tp = model.turret_pivot.xyz;
            p = rot_z(p - tp, turret) + tp;
            n = rot_z(n, turret);
        } else {
            p = rot_z(p - pivot, yaw) + pivot;
            n = rot_z(n, yaw);
        }
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
    } else if in.part == PART_SILO_DOOR && (e.owner_flags & (KIND_WRECK | KIND_GHOST | FLAG_UNDER_CONSTRUCTION)) == 0u {
        // A silo's or an interceptor array's blast doors: two leaves meeting on y = 0,
        // slid apart along y by half the opening (`models/aster/strategic.rs`): 5.2 m on
        // the silo (icon 25), 5 m on the array (icon 26). Heavy: slow to start and to stop.
        let open = smoothstep(0.0, 1.0, mix(e.prev_deploy, e.deploy, t));
        let travel = select(5.0, 5.2, (model.icon & 0xFFu) == LAUNCHER_ICON_SILO);
        p.y += sign(p.y) * open * travel;
    } else if in.part == LAUNCHER_PART_LID || in.part == LAUNCHER_PART_HOIST {
        // A launcher's load cycle while a round assembles: the store's lid slides back, the
        // hoist block goes down into the hatch and up, the lid shuts. The cables' tops
        // stay on the trolley, so they stretch.
        let cycle = load_cycle(e, time);
        let silo = (model.icon & 0xFFu) == LAUNCHER_ICON_SILO;
        if in.part == LAUNCHER_PART_LID {
            let slide = cycle.x * select(LAUNCHER_ARRAY_LID_TRAVEL, LAUNCHER_SILO_LID_TRAVEL, silo);
            p += select(vec3<f32>(0.0, -slide, 0.0), vec3<f32>(-slide, 0.0, 0.0), silo);
        } else if p.z < select(LAUNCHER_ARRAY_HOIST_SPLIT, LAUNCHER_SILO_HOIST_SPLIT, silo) {
            p.z -= cycle.y * select(LAUNCHER_ARRAY_HOIST_DROP, LAUNCHER_SILO_HOIST_DROP, silo);
        }
    } else if (in.part == CELLS_PART_HATCH || in.part == CELLS_PART_ROUND) && model.cell_grid.x != 0u {
        // A block of missile cells (`models::CellBlock`): each hatch swings up and out about
        // its outer edge as the hatches open (`deploy`), and a missile stands in each cell
        // whose bit is set in `status[2]`. A wreck or a site: shut, and empty.
        let live = (e.owner_flags & (KIND_WRECK | KIND_GHOST | FLAG_UNDER_CONSTRUCTION)) == 0u;
        // The nearer block's middle, then the nearest cell of its grid.
        var blk = 0u;
        if model.cell_grid.y != 0u
            && distance(p.xy, model.cells[2].xy) < distance(p.xy, model.cells[0].xy) {
            blk = 1u;
        }
        let at = model.cells[2u * blk];
        let grid = select(model.cell_grid.x, model.cell_grid.y, blk == 1u);
        let count = vec2<f32>(f32(grid & 15u), f32((grid >> 4u) & 15u));
        let ij = clamp(round((p.xy - at.xy) / at.w + (count - 1.0) * 0.5), vec2<f32>(0.0), count - 1.0);
        let centre = at.xy + (ij - (count - 1.0) * 0.5) * at.w;
        if in.part == CELLS_PART_HATCH {
            let open = select(0.0, smoothstep(0.0, 1.0, mix(e.prev_deploy, e.deploy, t)), live);
            let half = model.cells[2u * blk + 1u].x;
            if ((grid >> 8u) & 1u) == 0u {
                let side = select(-1.0, 1.0, centre.x >= at.x);
                let hinge = vec3<f32>(centre.x + side * half, 0.0, at.z);
                let turn = -side * open * CELLS_SWING;
                p = rot_xz(p - hinge, turn) + hinge;
                n = rot_xz(n, turn);
            } else {
                let side = select(-1.0, 1.0, centre.y >= at.y);
                let hinge = vec3<f32>(0.0, centre.y + side * half, at.z);
                let turn = -side * open * CELLS_SWING;
                p = rot_x(p - hinge, turn) + hinge;
                n = rot_x(n, turn);
            }
        } else {
            let cell = u32(ij.x) * ((grid >> 4u) & 15u) + u32(ij.y);
            let order = select(model.cell_grid.z, model.cell_grid.w, blk == 1u);
            let bit = (order >> (4u * cell)) & 15u;
            if !live || (e.status[2] & (1u << bit)) == 0u {
                p = vec3<f32>(0.0, 0.0, -50.0);
            }
        }
    } else if in.part == PART_SILO_ROUND {
        // The rounds a launcher holds (`nukes::LAUNCHER_*` in `status[2]`): the silo's one
        // tube is drawn while it has a warhead; the array's cells empty in firing order,
        // (-x -y) first, so a cell shows while it is among the last `stock` of them.
        let stock = e.status[2] & LAUNCHER_STOCK_MASK;
        let capacity = (e.status[2] >> LAUNCHER_CAPACITY_SHIFT) & 0xFFu;
        let cell = select(0u, 2u, p.x >= 0.0) + select(0u, 1u, p.y >= 0.0);
        let silo = (model.icon & 0xFFu) == LAUNCHER_ICON_SILO;
        let held = select(cell + stock >= max(capacity, 4u), stock > 0u, silo);
        let live = (e.owner_flags & (KIND_WRECK | KIND_GHOST | FLAG_UNDER_CONSTRUCTION)) == 0u
            && (e.status[2] & LAUNCHER_MARK) != 0u;
        if !(live && held) {
            p = vec3<f32>(0.0, 0.0, -50.0);
        }
    } else if in.part == PART_PUMP && (e.owner_flags & (KIND_WRECK | FLAG_UNDER_CONSTRUCTION | STATE_UNPOWERED)) == 0u {
        // A reactor's pumps and injectors: a short stroke, a quick drive down and a slower
        // draw back, phased by where each stands so they work round the plant in turn.
        let phase = atan2(p.y, p.x) / 6.2831853 + f32(e.unit_id & 255u) * 0.137;
        let beat = fract(time * 0.7 + phase);
        let stroke = clamp(model.height * 0.035, 0.3, 1.1);
        p.z -= stroke * select(1.0 - (beat - 0.25) / 0.75, beat / 0.25, beat < 0.25);
    } else if (in.part & ORBIT_PART_MASK) == ORBIT_PART && (e.owner_flags & (KIND_WRECK | FLAG_UNDER_CONSTRUCTION | STATE_UNPOWERED)) == 0u {
        // A gyroscope's ring (`gpu_consts::orbit`): about its own axis through the pivot.
        let pivot = model.spinner_pivot.xyz;
        let azimuth = f32((in.part >> ORBIT_AZIMUTH_SHIFT) & 255u) / 256.0 * 6.2831853;
        let tilt = f32((in.part >> ORBIT_TILT_SHIFT) & 255u) / 255.0 * 1.5707963;
        let axis = vec3<f32>(sin(tilt) * cos(azimuth), sin(tilt) * sin(azimuth), cos(tilt));
        let rate = f32(bitcast<i32>(in.part) >> ORBIT_RATE_SHIFT) * ORBIT_RATE_STEP;
        let spin = time * rate + f32(e.unit_id & 255u) * 0.37;
        p = rot_about(p - pivot, axis, spin) + pivot;
        n = rot_about(n, axis, spin);
    } else if (in.part == PART_SPINNER || in.part == 4u) && (e.owner_flags & (KIND_WRECK | FLAG_UNDER_CONSTRUCTION | STATE_UNPOWERED)) == 0u {
        let pivot = model.spinner_pivot.xyz;
        let rate = select(select(1.6, 0.48, (e.owner_flags & STATE_CHARGING) != 0u), 38.0, in.part == 4u);
        var spin = time * rate + f32(e.unit_id & 255u);
        if (model.icon & ICON_SPINNER_SCANS) != 0u {
            // A watching eye (`Model::spinner_scans`): it swings slowly one way, dwells,
            // and looks back, never round and round.
            let seed = f32(e.unit_id & 255u);
            let look = sin(time * 0.23 + seed) + 0.35 * sin(time * 0.61 + seed * 2.3);
            spin = 1.4 * look / 1.35 + seed;
        }
        p = rot_z(p - pivot, spin) + pivot;
        n = rot_z(n, spin);
    } else if in.part == PART_LOCOMOTION && crawls && limb != 0u {
        let body = walk_bob(walk, model) + vec3<f32>(0.0, 0.0, footing.ground.z + crawl_set(e, model, t));
        // Clamped to the pairs the model has: `crawl` is a fixed array, never read past it.
        let pair = min((in.rig >> 16u) & 7u, u32(model.crawl[0].x + 0.5) - 1u);
        let posed = crawl_leg(p, n, limb, min(pair, 3u), model, walk, e, t, body);
        p = posed[0];
        n = posed[1];
    } else if in.part == PART_LOCOMOTION && walks && limb != 0u {
        let posed = walk_leg(p, n, limb, model, walk, footing, stance);
        p = posed[0];
        n = posed[1];
        // The legs lean with the body's shift, the feet staying where they stand.
        p.y += stance.body.y * clamp(p.z / model.leg_hip.z, 0.0, 1.0);
    } else if in.part == PART_LOCOMOTION && !walks && (model.icon & 0x20000u) == 0u
        && (e.owner_flags & KIND_WRECK) == 0u
        && (in.rig & RIG_DEPLOY) == 0u
        && terrain_height(e.pos.xy) >= globals.map.z - 0.25 {
        // Running gear without a rig: a small shudder. Quiet while floating.
        // Hover skirts are not tracks: they do not crawl or bounce with the belt.
        // It follows the ground covered, not the clock, and fades in with speed, so a
        // hull nudged a hair by the crowd (one tick of MOVING) does not twitch.
        let gt = globals.sun.w;
        let step = mix(e.gait.z, e.gait.y, gt);
        let rolled = e.gait.x - e.gait.y * (1.0 - gt);
        let phase = fract(rolled * 0.25) * 6.2831853 + p.x * 0.8 + sign(p.y) * 1.57;
        p.z += max(sin(phase), 0.0) * 0.12 * model.height * 0.1 * smoothstep(0.1, 0.8, step);
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
        // Authored planted. The mesh may be scaled to a bigger blueprint, so the hinges
        // (placed at the authored 1.88 m deck) scale with the turret pivot height.
        let planted = mix(e.prev_deploy, e.deploy, t);
        let s = select(1.0, model.turret_pivot.z / 1.88, model.turret_pivot.z > 0.5);
        if (in.rig & STAKE_RIG) != 0u {
            let posed = stake_pose(p, n, (in.rig & STAKE_RIG_SPIKE) != 0u, planted, s);
            p = posed[0];
            n = posed[1];
        }
    }
    if walks && in.part != PART_LOCOMOTION {
        // Standing, the body rolls over its hips toward the loaded leg and shifts its way.
        let hips = vec3<f32>(0.0, 0.0, model.leg_hip.z);
        p = rot_x(p - hips, stance.body.x) + hips + vec3<f32>(0.0, stance.body.y, stance.body.z);
        n = rot_x(n, stance.body.x);
        p += walk_bob(walk, model) + vec3<f32>(0.0, 0.0, footing.ground.z);
        if crawls {
            p.z += crawl_set(e, model, t);
        }
    }
    // Hovercraft: the hull rides a cushion, the rubber skirt hangs behind it.
    if (model.icon & 0x20000u) != 0u
        && (e.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST | FLAG_UNDER_CONSTRUCTION)) == 0u
    {
        let bob = 0.10 + 0.08 * sin(time * 1.65 + f32(e.unit_id & 255u) * 0.31);
        p.z += select(bob, bob * 0.22, in.part == PART_LOCOMOTION);
    }
    // A hovering aircraft is never quite still (`hover_heave`). A drone docked on one
    // rides that one's heave instead (`riding_frame`).
    var hover_sway = 0.0;
    if (e.status[0] & DOCK_RIDING) == 0u {
        let heave = hover_heave(e, model, t);
        p.z += heave.x;
        hover_sway = heave.y;
    }

    // A spacecraft settles on its shock struts (`capital_sink`): everything but the struts
    // and feet goes down, so the feet stay planted and the struts slide into the legs.
    if model.capital[0].w != 0.0 && in.part != 20u && in.part != 21u
        && (e.owner_flags & (KIND_WRECK | KIND_GHOST)) == 0u {
        p.z -= capital_sink(model, e, t, capital_height(e, t));
    }
    let heading = lerp_angle(e.prev_heading, e.heading, t);
    var origin = mix(e.prev_pos, e.pos, t);
    if wall_piece(in.part) {
        p.z += wall_follow_ground(origin.xy, heading, p.xy);
    }
    if falling && e.status[2] != 0u {
        origin += casing_carry(e, t);
    }
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
    // A seabed installation's spire runs up to the surface however deep it stands: what is
    // authored between SPIRE_BASE and SPIRE_TOP stretches so the top meets the water, and
    // the cap above rides up with it. Out of the water (a unit shot on land) it is as authored.
    if (model.icon & ICON_SEABED) != 0u && p.z > SPIRE_BASE {
        let surface = max(globals.map.z - origin.z, SPIRE_TOP);
        if p.z <= SPIRE_TOP {
            p.z = SPIRE_BASE + (p.z - SPIRE_BASE) * (surface - SPIRE_BASE) / (SPIRE_TOP - SPIRE_BASE);
        } else {
            p.z += surface - SPIRE_TOP;
        }
    }
    var local = vec3<f32>(p.xy, p.z * stretch) * scale;
    // Stretched up, a trunk's sides lean less.
    if stretch != 1.0 {
        n = normalize(vec3<f32>(n.xy, n.z / stretch));
    }

    // Mobile units lean with the ground under them; a ship rides the water, not the seabed.
    var up = vec3<f32>(0.0, 0.0, 1.0);
    // A wreck of anything that moved lies on the ground (or the seabed) under it, whatever
    // it was, and down in it by how it came down (wreck.wgsl).
    let wreck_lies = wreck.posed && (model.icon & ICON_MOBILE) != 0u;
    origin.z -= wreck_bury(model, wreck, e.health);
    // A walker stands upright: its feet find the ground (`walk_ground`).
    if ((model.icon & ICON_MOBILE) != 0u && (model.icon & ICON_AIR) == 0u && (model.icon & ICON_NAVAL) == 0u && !walks)
        || wreck_lies {
        up = terrain_normal(origin.xy, max(e.radius * select(1.0, 0.5, wreck.count > 1u), 4.0));
        // A wreck comes onto the slope as it settles, from the pose it came down in.
        if wreck_lies {
            up = normalize(mix(vec3<f32>(0.0, 0.0, 1.0), up, wreck.settled));
        }
        // On a lift ship's ramp or hold floor it leans with the deck (`mirror::UNIT_ON_DECK`).
        if (e.status[0] & 0x1000000u) != 0u {
            let d = vec2<f32>(f32(i32(e.status[2] << 16u) >> 16u), f32(i32(e.status[2]) >> 16u)) / 32767.0;
            up = vec3<f32>(d, sqrt(max(1.0 - dot(d, d), 0.0)));
        }
    }
    let fwd0 = vec3<f32>(cos(heading), sin(heading), 0.0);
    var left = normalize(cross(up, fwd0));
    var fwd = cross(left, up);
    if (model.icon & 0x40000u) != 0u
        && (e.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST | FLAG_UNDER_CONSTRUCTION | FLAG_IN_FACTORY)) == 0u {
        let pitch = air_pitch(e, model, t);
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
        // Diving takes the bow down and surfacing brings it up, by how fast the hull is
        // going down or up (metres a tick). The sim eases the dive at both ends, so the
        // trim comes on and goes off with it; a soft limit, not a clamp, so it never
        // stops at a corner. A long hull trims less.
        let trim = mix(0.13, 0.09, smoothstep(20.0, 50.0, e.radius));
        let pitch = (0.035 * s2 + 0.018 * s3) * k * afloat
            + clamp(ahead * 0.012 * k, 0.0, 0.07)
            + trim * tanh(travel.z * 3.0);
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
        if (e.status[1] >> UNIT_HOUSE_SHIFT) > 0u && (model.icon & 0x800000u) != 0u {
            let hp = house_pose(e);
            var heel = 0.0;
            for (var slot = 0u; slot < 8u; slot++) {
                if house_weapon_of(model, slot) < 0.5 {
                    continue;
                }
                let w = u32(house_weapon_of(model, slot) + 0.5) - 1u;
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
    if falling || toppled || wreck.posed {
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
    // A drone docked on an aircraft is drawn where it sits in its carrier's drawn frame,
    // so it stays on its pylon however the carrier heaves, sways and leans.
    if (e.status[0] & DOCK_RIDING) != 0u && e.status[2] != 0u {
        let src = dynamic_entities[e.status[2] - 1u];
        let f = riding_frame(src, models[src.blueprint], t);
        let src_heading = lerp_angle(src.prev_heading, src.heading, t);
        let d = origin - mix(src.prev_pos, src.pos, t);
        let ahead = vec2<f32>(cos(src_heading), sin(src_heading));
        let turn = heading - src_heading;
        let at = vec3<f32>(dot(d.xy, ahead), dot(d.xy, vec2<f32>(-ahead.y, ahead.x)), d.z)
            + rot_z(local, turn);
        world = f.origin + f.fwd * at.x + f.left * at.y + f.up * at.z;
        let nt = rot_z(n, turn);
        world_n = normalize(f.fwd * nt.x + f.left * nt.y + f.up * nt.z);
    }
    // Into or out of warp: pulled out into a streak of light (warp_hull.wgsl).
    let warp_damped = (e.status[0] & WARP_STATUS_DAMPED) != 0u;
    let warp_seed = hash11(f32(e.unit_id & 0xFFFFu));
    let warp_reach = max(model.bounds_radius * scale, 1.0);
    let warp = warp_state(e.fx, t, warp_damped, warp_seed, time);
    world = warp_stretch(world, origin, fwd, left, up, local.x, warp_reach, warp, warp_damped, warp_seed, time);
    // A standing tree bends over its foot with the wind and away from blasts
    // (`tree_air`). The stem is a bending pole: stiff at the foot, curving most
    // low down and running straight through the crown, so the crown tips over
    // whole instead of being sheared sideways. Each slice of the tree is turned
    // with the stem where it crosses it and carried along its arc, so nothing
    // stretches and the top comes down as it goes over.
    if is_tree {
        let tall = max(model.height * scale * stretch, 1.0);
        let reach = length(tree.xy);
        if reach > 0.001 {
            let dir = tree.xy / reach;
            // The stem's tilt at the top, radians; it reaches over by about `reach`.
            let bend = min(asin(min(reach / tall, 0.8)) * 1.5, 0.95);
            let h = world.z - origin.z;
            let r = max(h, 0.0) / tall;
            // Share of the top's tilt at this height, and its mean up to here.
            let f = select(1.0, r * (2.0 - r), r < 1.0);
            let mean = select((r - 1.0 / 3.0) / max(r, 0.001), r - r * r / 3.0, r < 1.0);
            let a = bend * mean;
            let turn = bend * f;
            let rel = world.xy - origin.xy;
            let along = dot(rel, dir);
            let stem_h = max(h, 0.0);
            world = vec3<f32>(
                origin.xy + rel + dir * (stem_h * sin(a) + along * (cos(turn) - 1.0)),
                origin.z + h + stem_h * (cos(a) - 1.0) - along * sin(turn),
            );
            let n_along = dot(world_n.xy, dir);
            world_n = normalize(vec3<f32>(
                world_n.xy + dir * (n_along * (cos(turn) - 1.0) + world_n.z * sin(turn)),
                world_n.z * cos(turn) - n_along * sin(turn),
            ));
        }
    }
    var hull = -1;

    // A hull field is the posed mesh, pushed out along its skin — not a bubble.
    if push.pass_kind == PASS_HULL {
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
    if (push.pass_kind & PASS_KIND_MASK) == PASS_SHADOW {
        out.clip = globals.shadow_cascades[push.pass_kind >> PASS_CASCADE_SHIFT] * vec4<f32>(world, 1.0);
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
    if (push.pass_kind & PASS_KIND_MASK) == PASS_MAIN && model.pit.y > 0.0 && !rig_afloat && p.z < model.pit.x && dot(p.xy, p.xy) < model.pit.y * model.pit.y {
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
    out.world = world;
    out.normal = world_n;
    out.warp = vec4<f32>(warp, select(0.0, 1.0, warp_damped), clamp(0.5 + 0.5 * local.x / warp_reach, 0.0, 1.0));
    // A walker's rubber is its soles, not a belt: no links, nothing crawls (hover skirts
    // likewise). `model_class` bit 9 carries the same test to the fragment stage.
    let belt = (model.icon & 0x20000u) == 0u && model.leg_hip.w <= 0.0;
    if in.material == MAT_TREAD && belt {
        // The links move by the ground the hull has covered (the sim's gait, metres),
        // wrapped at `TREAD_WRAP` links (the wear bands' period), so a stopped belt stays
        // put and a nudge moves it a nudge. uv.y carries how many links the belt moves a
        // frame: at speed that is more than one, and the fragment stage blurs the links
        // rather than let them strobe backwards.
        let pitch = tread_pitch(max(select(model.bounds_radius, model.surface.x, model.surface.x > 0.0), 1.0));
        var rolled = 0.0;
        var travel = 0.0;
        // A wreck's gait is its age, not ground covered: its belt lies still.
        if terrain_height(e.pos.xy) >= globals.map.z - 0.25 && (e.owner_flags & KIND_WRECK) == 0u {
            let wrap = TREAD_WRAP * pitch;
            rolled = fract((e.gait.x - e.gait.y * (1.0 - globals.sun.w)) / wrap) * wrap;
            travel = e.gait.y * globals.climate.w / pitch;
        }
        out.uv = vec2<f32>(tread_along(in.pos, in.normal) + rolled, travel);
    } else {
        out.uv = in.uv;
    }
    out.material = select(store_material(in.material, in.part, e), u32(hull), push.pass_kind == PASS_HULL);
    out.owner_flags = e.owner_flags;
    out.state = vec4<f32>(e.build, select(e.health, 2.0, falling), in.pos.z / max(model.height, 0.1), hash11(f32(e.unit_id & 0xFFFFu)));
    out.local = unwarped;
    out.wreck = vec4<f32>(
        wreck.lo,
        wreck.hi,
        select(0.0, 1.0, wreck.inner) + select(0.0, 2.0, wreck.sank),
        select(0.0, 1.0 + min(wreck.age, 1000.0), wreck.posed),
    );
    // Tech in the low byte, then mobile (bit 8) from the icon flags and bit 9 when its
    // rubber is no belt (a hover skirt or a walker's soles).
    // Naval (icon bit 23) rides in bit 11.
    out.model_class = ((model.icon >> 8u) & 0xFFu) | ((model.icon >> 8u) & 0x100u) | select(0x200u, 0u, belt) | ((model.icon >> 12u) & 0x800u)
        | ((in.surface & 0xFFFFu) << 16u)
        | select(0u, 0x1000u, (model.icon & 0x1000000u) != 0u)
        // Bit 13: a capital ship (icon bit 26).
        | select(0u, 0x2000u, (model.icon & 0x4000000u) != 0u)
        // Bit 10: printed by a replicator (`mirror::UNIT_REPLICATING` in `status[1]`).
        | select(0u, 0x400u, (e.status[1] & 1u) != 0u)
        // Bit 14: built by nanites, not printed (`mirror::UNIT_NANITE`).
        | select(0u, CLASS_NANITE, (e.status[1] & UNIT_NANITE) != 0u);
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
    // A charge coil's light knows its stage and the unit's charge (`coil_state`).
    let coil_pat = in.surface & 0xFFu;
    if coil_pat >= PAT_COIL && coil_pat < PAT_COIL_TURN && (in.material == MAT_GLOW || in.material == MAT_GLOW_LASER)
        && e.mount.w == CHARGE_RECORD {
        let c = coil_state(e, time);
        out.drive = vec4<f32>(c.x, c.y, 0.0, 0.0);
        out.drive_at = vec4<f32>(f32(coil_pat - PAT_COIL), 0.0, 0.0, 3.0);
    }
    out.dust = select(0.62, model.surface.y / max(model.height, 0.1), model.surface.y > 0.0);
    let crackles = (e.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST)) == 0u;
    out.crackle = select(vec2<f32>(0.0), vec2<f32>(mix(e.fx.z, e.fx.w, t), warp_charge(e.fx, t)), crackles);
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
    if push.pass_kind == PASS_HULL {
        // The wrap has no welds. x and y carry where its emitter sits (z, x): the
        // model's own projector (`Model::shield_emitter`, on the centreline), else the
        // top of the hull over the model's middle, from the baked plan.
        if model.shield_emitter.w > 0.5 {
            out.weld.x = model.shield_emitter.z;
            out.weld.y = model.shield_emitter.x;
        } else {
            out.weld.x = hull_crown(e.blueprint, model.height);
            out.weld.y = 0.0;
        }
    }
    return out;
}

// `ModelInfo::icon` bit: the spinner looks about (renderer `Model::spinner_scans`).
const ICON_SPINNER_SCANS: u32 = 0x8000000u;

// ---- Nanite construction (`mc_data::Construction::Nanite`, the Regency) --------------
// `status[1]` bit of a Regency site (`mirror::UNIT_NANITE`), and where `model_class` carries it.
const UNIT_NANITE: u32 = 2u;
const CLASS_NANITE: u32 = 0x4000u;
const NANITE_VIOLET: vec3<f32> = vec3<f32>(0.66, 0.12, 1.0);
const NANITE_RED: vec3<f32> = vec3<f32>(1.0, 0.06, 0.1);

// How far the swarm has condensed into plate at `build`: over the first four fifths of the
// work, then the site settles and its light goes.
fn nanite_grow(build: f32) -> f32 {
    return clamp(build / 0.8, 0.0, 1.0);
}

// When a point of the model condenses: from the ground up, the front wavering a little
// so it is not a ruled line.
fn nanite_order(local: vec3<f32>, height: f32, seed: f32) -> f32 {
    let waver = value_noise2(local.xy * 0.35 + vec2<f32>(seed * 7.0, seed * 3.0), 1.0) - 0.5;
    return clamp(local.z / max(height, 1.0) + waver * 0.05, 0.0, 1.0);
}

// A Regency site's colour, and 0 in w where there is nothing there yet. It forms from the
// ground up: what has just condensed is glowing violet, which slowly cools through red
// into the finished plate; a thin hot line runs along the front and a haze of violet
// motes gathers just above it. Above that there is nothing yet (the rings and filaments
// round the site, beams.wgsl, show where it will stand). The glowing band is a share of
// the height, so it takes the same share of the work on any hull.
fn nanite_site(color: vec3<f32>, local: vec3<f32>, build: f32, height: f32, seed: f32, time: f32) -> vec4<f32> {
    let grow = nanite_grow(build);
    let order = nanite_order(local, height, seed);
    let settle = smoothstep(0.8, 1.0, build);
    if order > grow {
        // Motes gathering over the front, thinning out above it.
        let above = (order - grow) * max(height, 1.0);
        let cell = floor(local * 7.0 + vec3<f32>(0.0, 0.0, -time * 3.0));
        let h = hash21(cell.xy + vec2<f32>(cell.z * 1.7, seed * 13.0));
        if above > 0.8 || h < 0.88 + above * 0.12 {
            return vec4<f32>(0.0);
        }
        let twinkle = 0.5 + 0.5 * sin(time * 7.0 + h * 40.0);
        return vec4<f32>(NANITE_VIOLET * (1.0 + 1.5 * twinkle), 1.0);
    }
    // Metres behind the front: the band is the same depth on any hull, so on a tall one
    // it is a band and not the whole of what is up.
    let age = (grow - order) * max(height, 1.0);
    // Liquid light: brightness flowing up through the fresh material.
    let flow = value_noise2(local.xy * 0.8 + vec2<f32>(local.z * 0.6 - time * 0.9, seed * 5.0), 1.0);
    let violet = 1.0 - smoothstep(0.15, 0.7, age);
    let red = smoothstep(0.15, 0.7, age) * (1.0 - smoothstep(0.9, 2.2, age));
    let hot = violet + red;
    let glow = NANITE_VIOLET * violet * (0.8 + 0.7 * flow) + NANITE_RED * red * (0.35 + 0.45 * flow);
    // The fresh band is the glow over dark; the plate shows through as it cools.
    var c = mix(color, color * 0.25, hot) + glow * (1.0 - settle * 0.85);
    // The front itself: a thin, hot line.
    let edge = exp(-pow(age / 0.12, 2.0));
    c += mix(NANITE_VIOLET, vec3<f32>(1.0, 0.75, 1.0), 0.4) * edge * 2.0 * (1.0 - settle);
    return vec4<f32>(c, 1.0);
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

// The blasts' push on each standing tree, per entity as the cull indexes it
// (cull.wgsl `tree_blast`).
@group(0) @binding(31) var<storage, read> tree_sway: array<vec4<f32>>;

// What the blasts do to the tree `index` (a `visible` entry): see `tree_air`.
fn blast_sway(index: u32) -> vec3<f32> {
    let dynamic = (index & DYNAMIC_BIT) != 0u;
    return tree_sway[select(index, globals.counts.y + (index & ~DYNAMIC_BIT), dynamic)].xyz;
}

// shield_shelter, ground_air and gust_at are in habitat.wgsl.

// Where the top of a tree `tall` metres high standing at `foot` is pushed to
// sideways, metres (xy), and how hard its leaves are shaken (z, 0 in still air
// to 1 in a gale or a blast). The wind leans it over and rocks it at its own pace (a big
// tree is stiffer and slower), the rocking rolling across a wood downwind in
// gusts. `blast` is what the blasts add (`blast_sway`), worked out per tree by the
// cull. Shields stop both.
fn tree_air(foot: vec3<f32>, tall: f32, id: u32, blast: vec3<f32>) -> vec3<f32> {
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
        let phase = time * swing - dot(foot.xy, wind_heading()) * 0.06 + seed * 0.37;
        let rock = sin(phase) * (0.2 + 0.45 * gust) + sin(phase * 2.31 + seed) * 0.08;
        let side = sin(time * swing * 1.37 + seed * 1.3) * 0.12;
        lean += (dir * (steady * (1.0 + rock)) + vec2<f32>(-dir.y, dir.x) * steady * side) * still;
        stir = clamp(speed * (0.6 + 0.8 * gust) / 9.0, 0.0, 1.0) * still;
    }

    lean += blast.xy;
    stir = max(stir, blast.z);
    return vec3<f32>(lean, stir);
}

// Tree layers after FOLIAGE_BASE (foliage.rs): leaf atlases (linear albedo,
// cutout coverage), then bark as albedo/roughness + normal/occlusion pairs.
const FOLIAGE_BROADLEAF: i32 = 0;
const FOLIAGE_CONIFER: i32 = 1;
const FOLIAGE_BARK: i32 = 2;
const FOLIAGE_PINE_BARK: i32 = 4;
const FOLIAGE_TROPICAL: i32 = 6;
// A leaf card's tag (its face random byte): bit 7 picks the conifer atlas, the
// rest is a per-card random. Bark faces with the PLAIN pattern are pine bark.
const LEAF_CONIFER: u32 = 0x80u;
const BARK_PINE_PATTERN: u32 = 1u;
// A leaf card with the PLAIN pattern shows the tropical atlas (palm, jungle).
const LEAF_TROPICAL: u32 = 1u;
// Tropical bark (props.rs): pale grey-tan, and ringed for a palm's leaf scars.
const BARK_PALE_PATTERN: u32 = 2u;
const BARK_RINGED_PATTERN: u32 = 3u;

fn leaf_tag(in: VsOut) -> u32 {
    return (in.model_class >> 24u) & 0xFFu;
}

fn foliage_sample(in: VsOut) -> vec4<f32> {
    var layer = FOLIAGE_BASE + select(FOLIAGE_BROADLEAF, FOLIAGE_CONIFER, (leaf_tag(in) & LEAF_CONIFER) != 0u);
    let atlas = (in.model_class >> 16u) & 0xFFu;
    if atlas == LEAF_TROPICAL {
        layer = FOLIAGE_BASE + FOLIAGE_TROPICAL;
    } else if atlas == SCENERY_LEAF_DESERT {
        layer = FOLIAGE_BASE + SCENERY_FOLIAGE_DESERT;
    }
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

// Depth pre-pass (renderer.rs): the scene's depth before anything is shaded, so
// fs_main runs once per pixel and GTAO can read it. It must never write depth
// where fs_main would discard, or that pixel would show nothing. What is cut
// away by something only fs_main works out (sites, ghosts, a refit under way) is
// not in its list (cull.wgsl `other_lists`): fs_main writes its depth instead.
// What is cut away by a cheap test (wrecks, cut-away props) makes the same test
// here. The colour pass then shades this list over its depth without writing it
// (`entity_over_prepass`). Its draws carry `PASS_PREPASS` (gpu_consts.rs).

@fragment
fn fs_prepass(in: VsOut) {
    let flags = in.owner_flags;
    // A wreck goes in, cut as `fs_main` cuts it: worn from the top, and a broken hull's
    // section only its own stretch. Left out, every layer of a capital ship's wreck (its
    // inside too) ran the whole burnt surface: a hull filling a low view was half the frame.
    // One still falling or sinking (health 2) is the whole unit, solid as it was alive.
    let hull_down = (flags & KIND_WRECK) != 0u && in.state.y > 1.5;
    if (flags & KIND_WRECK) != 0u && !hull_down
        && (wreck_worn(in.local, in.state.z, in.state.y, in.weld.z)
            || (in.wreck.w > 0.5 && !wreck_keeps(in.local, in.wreck.x, in.wreck.y, in.weld.z))) {
        discard;
    }
    if in.material == MAT_FOLIAGE && foliage_missing(in, foliage_sample(in)) { discard; }
    if (in.material == MAT_FOLIAGE || in.material == MAT_BARK) && (flags & KIND_PROP) != 0u && vapor_edge(in) > 0.0 {
        discard;
    }
    if precursor_cutaway(in) { discard; }
}

@fragment
fn fs_shadow(in: VsOut) {
    if in.material == MAT_FOLIAGE && foliage_missing(in, foliage_sample(in)) { discard; }
    // A broken hull's section shadows only its own stretch.
    if in.wreck.w > 0.5 && !wreck_keeps(in.local, in.wreck.x, in.wreck.y, in.weld.z) { discard; }
    if vapor_edge(in) > 0.0 { discard; }
    // Unbuilt parts of a construction site cast no shadow: neither what is still to be
    // printed nor a Regency site's swarm.
    if (in.owner_flags & FLAG_UNDER_CONSTRUCTION) != 0u {
        if (in.model_class & CLASS_NANITE) != 0u {
            if nanite_order(in.local, in.weld.w, in.state.w) > nanite_grow(in.state.x) {
                discard;
            }
        } else {
            let grow = clamp(in.state.x / 0.74, 0.0, 1.0);
            if print_order(in.local, in.state.w) > grow {
                discard;
            }
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
        case 3u: { m.albedo = globals.team_colors[owner & OWNER_MASK].rgb * TEAM_PAINT; m.metallic = 0.3; m.roughness = 0.4; m.emissive = globals.team_colors[owner & OWNER_MASK].rgb * TEAM_GLOW; }
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
        // Regency construction: a violet with red in it (their counterpart to ARC's amber).
        case 16u: { m.albedo = vec3<f32>(0.18, 0.05, 0.3); m.emissive = vec3<f32>(0.66, 0.12, 1.0) * 5.0; m.roughness = 0.25; }
        case 17u: { m.albedo = vec3<f32>(0.3, 0.02, 0.02); m.emissive = vec3<f32>(1.0, 0.07, 0.05) * 5.5; m.roughness = 0.25; }
        // Precursor alloy, its dark joints, and its cold blue-white light.
        // Precursor alloy: aged metal, not paint. Weathered on props below.
        case 18u: { m.albedo = vec3<f32>(0.44, 0.45, 0.46); m.metallic = 0.55; m.roughness = 0.5; }
        case 19u: { m.albedo = vec3<f32>(0.08, 0.085, 0.095); m.metallic = 0.7; m.roughness = 0.42; }
        // A dormant light channel: dark glass with a cold sheen, the faintest glow in it.
        case 25u: { m.albedo = vec3<f32>(0.035, 0.045, 0.06); m.metallic = 0.75; m.roughness = 0.16; m.emissive = vec3<f32>(0.45, 0.78, 1.0) * 0.035; }
        case 20u: { m.albedo = vec3<f32>(0.3, 0.5, 0.7); m.emissive = vec3<f32>(0.45, 0.78, 1.0) * 5.0; m.roughness = 0.25; }
        // Ship's lamps: steady sidelights, and a warm working white.
        case 21u: { m.albedo = vec3<f32>(0.3, 0.03, 0.02); m.emissive = vec3<f32>(1.0, 0.06, 0.03) * 4.5; m.roughness = 0.2; }
        case 22u: { m.albedo = vec3<f32>(0.03, 0.3, 0.1); m.emissive = vec3<f32>(0.06, 1.0, 0.3) * 4.0; m.roughness = 0.2; }
        case 23u: { m.albedo = vec3<f32>(0.4, 0.36, 0.28); m.emissive = vec3<f32>(1.0, 0.86, 0.62) * 3.2; m.roughness = 0.2; }
        // Shield projectors, in the faction's shield colour. Saturated before it is
        // pushed, or a pale gold blows out to white in the tonemap.
        // Helmet visor: gold-orange mirror glass, lit faintly from within.
        case 26u: { m.albedo = vec3<f32>(0.46, 0.2, 0.02); m.metallic = 1.0; m.roughness = 0.12; }
        case 24u: { m.albedo = globals.shield.rgb * 0.25; m.emissive = pow(globals.shield.rgb, vec3<f32>(2.2)) * 2.6; m.roughness = 0.3; }
        default: {}
    }
    if id == PRISM_GLOW_MATERIAL {
        // A star core: white-hot (`fs_main` runs the prism over its rim).
        m.albedo = vec3<f32>(0.5, 0.42, 0.5);
        m.emissive = vec3<f32>(1.0, 0.94, 0.98) * 5.0;
        m.roughness = 0.2;
    }
    if id == MASS_GLOW_MATERIAL {
        // A reclaim emitter: Materials red-orange (`fs_main` banks it while idle).
        let materials = vec3<f32>(MASS_R, MASS_G, MASS_B);
        m.albedo = materials * 0.3;
        m.emissive = materials * 4.5;
        m.roughness = 0.3;
    }
    return m;
}

// A survival map's facility stands hundreds of metres to kilometres high, over the
// ground the game is played on. While something is selected, what of it stands high
// between the camera and the point it looks at dissolves (a screen door, so no
// sorting), so the ground and the units under a gate, a spire or a halo stay in
// sight. It comes and goes with the clouds' see-through middle (`scene.w`, sky.rs),
// so with nothing selected, and in the free camera, the architecture stands whole.
// Shadows stay whole.
fn precursor_cutaway(in: VsOut) -> bool {
    let cut = globals.scene.w;
    if (in.owner_flags & KIND_PROP) == 0u || globals.tree_wind.w <= 0.0 || cut <= 0.0 {
        return false;
    }
    let eye = globals.camera.xyz;
    let focus = globals.tree_wind.yz;
    let ground = terrain_height(focus);
    let rise = eye.z - ground;
    let above = ground + 0.2 * rise;
    if rise <= 1.0 || in.world.z < above {
        return false;
    }
    // Across from the line between the focus and the point under the eye.
    let ab = eye.xy - focus;
    let t = clamp(dot(in.world.xy - focus, ab) / max(dot(ab, ab), 1.0), -0.25, 1.0);
    let d = length(in.world.xy - (focus + ab * t));
    let r = 0.6 * rise;
    // Gone outright inside, dissolving only over a narrow fringe.
    let fade = cut * (1.0 - smoothstep(0.85 * r, r, d)) * smoothstep(above, above + 0.03 * rise, in.world.z);
    if fade >= 0.999 {
        return true;
    }

    let p = floor(in.clip.xy);
    let door = fract(52.9829189 * fract(dot(p, vec2<f32>(0.06711056, 0.00583715))));
    return door < fade * 0.97;
}

// A track link's look at one point of it (`tread_link`).
struct TreadLink {
    albedo: vec3<f32>,
    metal: f32,
    rough: f32,
}

// Taps across one link for its average look (`tread_link`), when it moves too fast to see.
const TREAD_TAPS: u32 = 12u;

// Each link is a steel shoe: a worn grouser bar across it, rubber pads either side of a
// centre guide horn, end connectors at its edges, a dark hinge gap to the next. Seen from
// the flank it is the links' side plates and their pins. `link` is how far along the link
// (0..1), `q` how far across the shoe from its centre (0..0.5), `shade` its wear.
fn tread_link(link: f32, q: f32, flank: bool, shade: f32) -> TreadLink {
    let gap = smoothstep(0.9, 0.93, link);
    let steel = vec3<f32>(0.045, 0.044, 0.043);
    let worn = vec3<f32>(0.2, 0.195, 0.185);
    var albedo = steel * shade;
    var metal = 0.35;
    var rough = 0.7;
    var recess = gap;
    if flank {
        // Side plates, a pin boss at each joint, a bevel catching light on the lead edge.
        let lead = smoothstep(0.02, 0.07, link) * (1.0 - smoothstep(0.1, 0.16, link));
        let pin = smoothstep(0.78, 0.8, link) * (1.0 - smoothstep(0.88, 0.9, link));
        albedo = mix(albedo, worn * 0.7, lead * 0.6);
        albedo = mix(albedo, worn, pin);
        metal = mix(metal, 0.85, pin);
        rough = mix(rough, 0.35, pin);
    } else {
        // Across the shoe, every 1.1 m or so: pad, horn, pad, and a connector at each end.
        let grouser = smoothstep(0.06, 0.09, link) * (1.0 - smoothstep(0.3, 0.33, link));
        let pad_along = smoothstep(0.4, 0.43, link) * (1.0 - smoothstep(0.84, 0.87, link));
        let horn = 1.0 - smoothstep(0.05, 0.07, q);
        let pad = pad_along * smoothstep(0.1, 0.12, q) * (1.0 - smoothstep(0.42, 0.44, q));
        let connector = smoothstep(0.46, 0.475, q) * (1.0 - gap);
        albedo = mix(albedo, worn * shade, max(grouser, connector * 0.7));
        metal = mix(metal, 0.85, max(grouser, connector));
        rough = mix(rough, 0.3, grouser);
        albedo = mix(albedo, vec3<f32>(0.016, 0.016, 0.018), pad);
        rough = mix(rough, 0.95, pad);
        metal = mix(metal, 0.0, pad);
        albedo = mix(albedo, worn * 1.15, horn * pad_along);
        metal = mix(metal, 0.9, horn * pad_along);
        // Dirt packs the edges of the pads.
        recess = max(recess, pad * (smoothstep(0.36, 0.43, q) + 1.0 - smoothstep(0.1, 0.17, q)) * 0.6);
    }
    albedo = mix(albedo, vec3<f32>(0.006), gap);
    albedo = mix(albedo, vec3<f32>(0.075, 0.058, 0.04), recess * 0.55 * (1.0 - gap));
    return TreadLink(albedo, metal, rough);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    if precursor_cutaway(in) { discard; }
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
    // A hull still falling or sinking (the vertex shader sends health 2) is the whole unit as it
    // died, scorched but in its paint; it burns out into a wreck once it lands on the ground or seabed.
    let hull_down = (flags & KIND_WRECK) != 0u && in.state.y > 1.5;
    let wreck = (flags & KIND_WRECK) != 0u && !hull_down;
    // A wreck's mesh is warped in the vertex shader, so its faces' normals come from the surface itself.
    let face = cross(dpdx(in.world), dpdy(in.world));
    if wreck && dot(face, face) > 1e-16 {
        let facing = normalize(face);
        n = facing * select(-1.0, 1.0, dot(facing, globals.camera.xyz - in.world) > 0.0);
    }
    let eye = globals.camera.xyz;
    let v = normalize(eye - in.world);
    let dist = distance(eye, in.world);
    // The face's normal in model space, from the surface itself (the tread tells a belt's
    // flank from its running face by it). Out here, where derivatives are defined.
    let face_n = cross(dpdx(in.local), dpdy(in.local));
    // A pixel's footprint on the model, for the EMP arcs' width (emp.wgsl).
    let local_px = length(fwidth(in.local));

    // Plating is textured from each face's own shape (surface.wgsl): outlines, fitted
    // plates and rivets, lights in the black, the patterns a model asks for, and burns
    // as it is hurt. Armour only: a gunmetal tube is left a plain tube. Wrecks skip
    // it: they are burnt out all over further down (`wreck_surface`).
    var soot = 0.0;
    // How deep in a plate seam or rivet ring: field dirt packs in there.
    var seam = 0.0;
    var lights = vec3<f32>(0.0);
    let precursor = in.material == MAT_PRECURSOR || in.material == MAT_PRECURSOR_DARK;
    if ((in.material < MAT_METAL && in.material != MAT_GLOW) || in.material == MAT_PLATING_DARK || precursor)
        && (flags & KIND_GHOST) == 0u && !wreck {
        var si: SurfaceIn;
        si.st = in.face.xy;
        si.half = abs(in.face.zw);
        si.wraps = in.face.z < 0.0;
        si.dark = in.material == MAT_ACCENT || in.material == MAT_PRECURSOR_DARK;
        si.pattern = (in.model_class >> 16u) & 0xFFu;
        // Precursor alloy is always precursor plate unless the model asks for something else.
        if precursor && si.pattern == PAT_GENERIC {
            si.pattern = PAT_PRECURSOR;
        }
        si.seed = f32((in.model_class >> 24u) & 0xFFu) / 255.0;
        si.unit = in.state.w;
        si.unit_id = in.unit_id;
        si.scale = clamp(0.55 * pow(in.weld.z, 0.6), 0.7, 6.0);
        si.time = time;
        si.working = select(0.0, 1.0, (flags & FLAG_BUILDING) != 0u && in.refit.z <= 0.0
            && (flags & (FLAG_UNDER_CONSTRUCTION | STATE_UNPOWERED)) == 0u);
        si.reclaiming = select(0.0, 1.0, (flags & UNIT_FLAG_RECLAIMING) != 0u
            && (flags & (FLAG_UNDER_CONSTRUCTION | STATE_UNPOWERED)) == 0u);
        si.tech = f32(max(in.model_class & 0xFFu, 1u));
        si.lit = select(1.0, 0.0, si.tech < 1.5 && (in.model_class & 0x100u) != 0u);
        si.mobile = select(0.0, 1.0, (in.model_class & 0x100u) != 0u);
        si.health = select(select(in.state.y, 1.0, (flags & (KIND_PROP | FLAG_UNDER_CONSTRUCTION)) != 0u), 0.0, hull_down);
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
        let team_rgb = globals.team_colors[owner & OWNER_MASK].rgb;
        m.albedo = mix(m.albedo, team_rgb * TEAM_PAINT, sf.team);
        m.emissive = mix(m.emissive, team_rgb * TEAM_GLOW, sf.team);
        m.albedo *= sf.cavity;
        seam = 1.0 - sf.cavity;
        // Bare steel where paint is scuffed or burnt off.
        m.albedo = mix(m.albedo, vec3<f32>(0.2, 0.19, 0.185), sf.bare);
        m.metallic = mix(m.metallic, 0.85, sf.bare);
        m.roughness = clamp(mix(m.roughness + sf.rough, 0.5, sf.bare), 0.05, 1.0);
        lights = sf.emissive;
        soot = sf.soot;
        if precursor && (flags & KIND_PROP) != 0u {
            // Precursor architecture is mostly unlit: its plates' light slots lie
            // dormant, dark glass. And it is not paint but old, strange metal: broad
            // shifts of tone and temper, streaks run down its steep faces, grime
            // gathered low, dust on what faces the sky.
            lights = vec3<f32>(0.0);
            let w = in.world;
            let px = si.px;
            let broad = surf_fbm3(w, 240.0, px);
            let tone = surf_fbm3(w + vec3<f32>(311.0, 97.0, 41.0), 70.0, px);
            let steep = 1.0 - abs(n.z);
            let streak = surf_fbm3(vec3<f32>(w.x, w.y, w.z * 0.07), 3.2, px);
            let grime = 1.0 - smoothstep(0.0, 45.0, in.local.z);
            let dark_plate = select(1.0, 0.45, in.material == MAT_PRECURSOR_DARK);
            var k = 0.82 + (1.5 * broad + 0.6 * tone) * dark_plate;
            k *= 1.0 - 0.55 * steep * smoothstep(0.0, 0.2, streak) * dark_plate;
            k *= 1.0 - 0.45 * grime;
            m.albedo *= max(k, 0.25);
            // Temper colours: some of it cool and blued, some warm, like old steel.
            m.albedo *= mix(vec3<f32>(1.06, 1.0, 0.9), vec3<f32>(0.9, 0.97, 1.08), clamp(0.5 + 1.6 * broad, 0.0, 1.0));
            m.albedo = mix(m.albedo, vec3<f32>(0.46, 0.43, 0.38), 0.25 * smoothstep(0.75, 0.95, n.z) * dark_plate);
            m.roughness = clamp(m.roughness + 0.25 * tone + 0.15 * steep * streak, 0.08, 1.0);
            m.metallic = clamp(m.metallic + 0.3 * broad, 0.0, 1.0);
        }
    }
    // Regency plate and bronze (regency.wgsl): panels, bolts, vents and red lines cut in
    // the model's own space, turned and engraved bronze.
    var regency = regency_none();
    let bronze = in.material == MAT_METAL;
    if ((in.model_class >> 16u) & 0xFFu) == PAT_EMBER && (bronze || in.material == MAT_PLATING_DARK)
        && (flags & (KIND_GHOST | KIND_PROP)) == 0u && !wreck {
        let dl1 = dpdx(in.local);
        let dl2 = dpdy(in.local);
        var ri: RegencyIn;
        ri.local = in.local;
        ri.n = normalize(cross(dl1, dl2) + vec3<f32>(0.0, 0.0, 1e-9));
        ri.px = local_px;
        ri.scale = clamp(0.55 * pow(in.weld.z, 0.6), 0.7, 6.0);
        ri.time = time;
        ri.health = select(in.state.y, 1.0, (flags & FLAG_UNDER_CONSTRUCTION) != 0u);
        ri.along = in.face.y;
        ri.along_dir = vec3<f32>(0.0);
        if in.face.z < 0.0 {
            let grad = reg_face_grad(dpdx(in.face.y), dpdy(in.face.y), dl1, dl2);
            ri.along_dir = grad * inverseSqrt(max(dot(grad, grad), 1e-12));
        }
        regency = regency_look(ri, bronze);
        n = normalize(n - reg_to_world(regency.slope, dl1, dl2, dpdx(in.world), dpdy(in.world)));
        lights += regency.emissive;
    }
    // Mineral props share the terrain's rock texture and correctly oriented normals.
    if in.material == 10u {
        let rock = terrain_surface(in.world, n, 7.3, 0, 0.8);
        n = rock.normal;
        m.albedo = rock.color * (0.75 + rock.ao * 0.25);
        m.roughness = rock.roughness;
        if ((in.model_class >> 16u) & 0xFFu) == SCENERY_ROCK_BEDDED {
            let px = max(length(dpdx(in.world)), length(dpdy(in.world)));
            let look = bedded_sandstone(m.albedo, in.local, n, px);
            m.albedo = look.albedo;
            m.roughness = look.roughness;
        }
    }
    // Scenery concrete with a pattern of its own: the dam (scenery.wgsl).
    let concrete = (in.model_class >> 16u) & 0xFFu;
    if in.material == MAT_CONCRETE && (flags & KIND_PROP) != 0u && concrete != 0u {
        let px = max(length(dpdx(in.world)), length(dpdy(in.world)));
        let look = dam_concrete(concrete, in.local, in.face, n, px);
        m.albedo = look.albedo;
        m.roughness = look.roughness;
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
        let bark = (in.model_class >> 16u) & 0xFFu;
        if bark == BARK_PALE_PATTERN || bark == BARK_RINGED_PATTERN {
            // The broadleaf scan bleached to a smooth grey-tan.
            let grey = dot(m.albedo, vec3<f32>(0.3, 0.59, 0.11));
            m.albedo = mix(vec3<f32>(grey), m.albedo, 0.3) * vec3<f32>(2.4, 2.2, 1.95);
            if bark == BARK_RINGED_PATTERN {
                // Leaf scars: a dark groove every 0.4 m up the stem.
                let ring = fract(in.local.z / 0.4);
                let groove = smoothstep(0.0, 0.14, ring) * smoothstep(1.0, 0.72, ring);
                m.albedo *= 0.55 + 0.45 * groove;
            }
        }
        if bark == SCENERY_BARK_SHAGGY {
            m.albedo = shaggy_bark(m.albedo, metres.x, metres.y);
        }
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
        // Track links round the belt. The top run crawls forward with the hull; the
        // underside crawls back so it stays on the ground. A hover skirt uses the same
        // rubber but is not a belt. The crawl is already in uv.x (vertex stage, from the
        // ground covered); afloat the belt has nothing to drive on and stands still.
        tread = 0.45;
        let pitch = tread_pitch(in.weld.z);
        let along = in.uv.x / pitch;
        let detail = clamp(1.0 - dist / 500.0, 0.0, 1.0);
        let flank = dot(face_n, face_n) > 1e-12 && abs(normalize(face_n).y) > 0.7;
        let q = abs(fract(in.local.y * 0.36 / pitch + 0.5) - 0.5);
        // A tank runs at tens of metres a second, more than a link a frame: the eye pairs
        // each link with the one behind it and the belt seems to run backwards. Past a
        // quarter link a frame the links fade to their average (a camera's motion blur),
        // and slow wear bands, a few metres long, are left to show it rolling forward.
        let blur = smoothstep(0.25, 0.45, in.uv.y);
        let index = floor(along) - TREAD_WRAP * floor(along / TREAD_WRAP);
        let band = sin((along / TREAD_WRAP + in.state.w) * 2.0 * PI);
        let shade = mix(0.85 + 0.3 * hash11(index + in.state.w * 97.0), 1.0, blur);
        var look = tread_link(fract(along), q, flank, shade);
        if blur > 0.0 {
            // The whole link's average, taken at fixed points so it holds still as the belt moves.
            var mean = TreadLink(vec3<f32>(0.0), 0.0, 0.0);
            for (var i = 0u; i < TREAD_TAPS; i++) {
                let tap = tread_link((f32(i) + 0.5) / f32(TREAD_TAPS), q, flank, shade);
                mean.albedo += tap.albedo / f32(TREAD_TAPS);
                mean.metal += tap.metal / f32(TREAD_TAPS);
                mean.rough += tap.rough / f32(TREAD_TAPS);
            }
            look.albedo = mix(look.albedo, mean.albedo * (1.0 + 0.45 * band), blur);
            look.metal = mix(look.metal, mean.metal, blur);
            look.rough = mix(look.rough, mean.rough, blur);
        }
        let albedo = look.albedo;
        let metal = look.metal;
        let rough = look.rough;
        m.albedo = mix(m.albedo, albedo, detail);
        m.metallic = mix(m.metallic, metal, detail);
        m.roughness = mix(m.roughness, rough, detail);
    }
    // Construction emitters: ARC's amber, the Regency's violet.
    let builds = in.material == MAT_GLOW_AMBER || in.material == MAT_GLOW_VIOLET;
    if builds && (flags & FLAG_BUILDING) != 0u && in.refit.z <= 0.0 {
        // Construction emitters run hot while the unit builds, not during a refit.
        m.emissive *= 1.6 + 0.7 * sin(time * 11.0 + in.state.w * 40.0);
    } else if builds && (in.model_class & 0x100u) != 0u {
        // A mobile builder's emitters sit banked low while it is not building.
        m.emissive *= 0.14;
        m.albedo *= 0.6;
    }
    if in.material == MASS_GLOW_MATERIAL {
        // Reclaim emitters burn steady while the unit pulls material in; otherwise, and
        // unpowered, they sit banked low.
        let live = (flags & (KIND_WRECK | KIND_GHOST | FLAG_UNDER_CONSTRUCTION | STATE_UNPOWERED)) == 0u;
        if live && (flags & UNIT_FLAG_RECLAIMING) != 0u {
            m.emissive *= 1.5;
        } else {
            m.emissive *= 0.12;
            m.albedo *= 0.6;
        }
    }
    if in.material == PRISM_GLOW_MATERIAL && (flags & (KIND_WRECK | KIND_GHOST)) == 0u {
        // Pinch fusion's light (gpu_consts `prism`): white-hot where the star faces the eye,
        // breaking into the prism only toward its rim, the way light breaks on a bubble,
        // wisps of colour drifting through it and the whole prism turning round the star.
        let facing = abs(dot(n, v));
        let wisp = 0.5 * sin(in.local.x * 1.7 + in.local.z * 1.3 + time * 2.1)
            + 0.35 * sin(in.local.y * 2.3 - in.local.z * 0.9 - time * 1.4);
        let around = atan2(in.local.y, in.local.x) / 6.2831853;
        let hue = prism((1.0 - facing) * 1.3 + around + wisp * 0.3 + time * PRISM_RATE + in.state.w);
        // Mostly white-hot: the colour lives in a band round the rim. Mostly squared, so it
        // keeps some colour this bright instead of washing out to white in the tone map.
        let deep = mix(hue, hue * hue, 0.8);
        let white = smoothstep(0.5, 0.95, facing) * (0.85 + 0.15 * wisp);
        let breathe = 0.9 + 0.1 * sin(time * 7.0 + in.state.w * 20.0);
        m.emissive = mix(deep * 2.8, vec3<f32>(1.0, 0.95, 0.98) * 5.0, white) * breathe;
        m.albedo = deep * 0.3;
    }
    if in.material == MAT_GLOW_PRECURSOR && (flags & KIND_WRECK) == 0u {
        // Precursor light breathes, and bands of it rise up the machine: the same pulse
        // as the light in its plate's slots (`precursor_pulse`), so the two run as one.
        m.emissive *= precursor_pulse(in.local.z, in.weld.w, time, in.state.w);
        let awake = globals.tree_wind.w;
        if awake > 0.0 && (flags & KIND_PROP) != 0u {
            // Survival: the facility wakes as the rounds climb. Asleep its light is
            // low and slow; awake it burns, and surges run up the machine faster.
            let surge = 0.5 + 0.5 * sin(time * (0.5 + 2.5 * awake) - in.local.z * 0.006 + in.state.w * 13.0);
            m.emissive *= mix(0.3, 1.5, awake) * (1.0 + 0.8 * awake * pow(surge, 8.0));
        }

        // A hurt Precursor machine's light gutters.
        let hurt = 1.0 - saturate(select(in.state.y, 1.0, (flags & (KIND_PROP | FLAG_UNDER_CONSTRUCTION)) != 0u));
        let gutter = select(1.0, 0.25 + 0.75 * step(0.25, hash11(floor(time * 6.0) + in.state.w * 50.0)), hurt > 0.5);
        m.emissive *= (1.0 - 0.6 * smoothstep(0.4, 1.0, hurt)) * gutter;
    }
    if in.material == MAT_PRECURSOR_INLAY && (flags & KIND_PROP) != 0u && globals.tree_wind.w > 0.0 {
        // A dormant channel stirs as the facility wakes: never lit, only less dark.
        let awake = globals.tree_wind.w;
        m.emissive *= 1.0 + 6.0 * awake * awake;
    }
    if in.material == MAT_GLOW_RED && (flags & KIND_WRECK) == 0u {
        // Aviation-style obstruction blink: a hard on, then a long dark.
        let blink = select(0.06, 1.0, fract(time * 0.85 + in.state.w) < 0.32);
        m.emissive *= blink;
        m.albedo *= 0.35 + 0.65 * blink;
    }
    if in.drive_at.w > 2.5 && in.material == MAT_GLOW_LASER
        && (flags & (KIND_WRECK | KIND_GHOST | FLAG_UNDER_CONSTRUCTION)) == 0u {
        // A Regency plasma coil (`charge_gear`): at rest a low red ember, breathing; through
        // the charge each stage lights in turn from the breech, red going pink and then
        // white as the charge fills, restless near full; the shot blinds white, then it
        // holds a hot orange-red that cools back to the ember. A vent's glow
        // (`CHARGE_GEAR_HEAT_STAGE`) stays dark until the shot, then burns and cools.
        let stage_i = u32(in.drive_at.x + 0.5);
        let charge = in.drive.x;
        let since = in.drive.y;
        let l = in.local;
        let blaze = 1.0 - smoothstep(0.04, 0.5, since);
        let flicker = 0.85 + 0.15 * sin(time * 31.0 + l.x * 0.7 + l.z * 1.3 + in.state.w * 9.0);
        var level = 0.0;
        var colour = vec3<f32>(1.0, 0.08, 0.04);
        if stage_i == CHARGE_GEAR_HEAT_STAGE {
            let heat = smoothstep(0.0, 0.2, since) * (1.0 - smoothstep(0.8, 5.2, since));
            level = 0.04 + heat * heat * 11.0 * flicker;
            colour = mix(vec3<f32>(1.0, 0.07, 0.03), vec3<f32>(1.0, 0.45, 0.16), heat * heat);
        } else {
            let stage = f32(stage_i) / 7.0;
            let breath = 0.5 + 0.5 * sin(time * 1.3 - stage * 3.0 + in.state.w * 6.0);
            level = 0.35 + 0.35 * breath;
            let reached = smoothstep(stage * 0.85, stage * 0.85 + 0.1, charge);
            let race = pow(0.5 + 0.5 * sin(time * (6.0 + 18.0 * charge) - stage * 9.0), 5.0);
            level += reached * (1.6 + 8.0 * charge * charge + race * 3.0 * charge);
            let restless = step(0.25, hash11(floor(time * 24.0) + f32(stage_i) * 7.13 + in.state.w * 91.0));
            level *= mix(1.0, 0.55 + 0.45 * restless, charge * charge * reached);
            // After the shot: hot, cooling through orange-red back to the ember.
            let cool = (1.0 - blaze) * (1.0 - smoothstep(0.4, 4.5, since));
            level += cool * cool * 4.5 * flicker;
            colour = mix(colour, vec3<f32>(1.0, 0.42, 0.5), smoothstep(2.0, 7.0, level) * charge);
            colour = mix(colour, vec3<f32>(1.0, 0.92, 0.9), smoothstep(6.0, 10.0, level) * charge);
            colour = mix(colour, vec3<f32>(1.0, 0.3, 0.08), cool * 0.6);
        }
        level = mix(level, 30.0, blaze);
        colour = mix(colour, vec3<f32>(1.0, 0.96, 0.95), blaze);
        m.emissive = colour * level;
        m.albedo = mix(vec3<f32>(0.05, 0.02, 0.02), colour * 0.3, clamp(level * 0.25, 0.0, 1.0));
        m.roughness = 0.3;
    } else if in.drive_at.w > 2.5 && in.material == MAT_GLOW
        && (flags & (KIND_WRECK | KIND_GHOST | FLAG_UNDER_CONSTRUCTION)) == 0u {
        // A charge coil (`coil_state`): at rest a slow breath runs up the arm with a faint
        // shimmer; charging, the light climbs from the breech stage by stage, pulses racing
        // up it, more restless the fuller it gets; the shot blinds; then it collapses to a
        // dull ember and comes back over some seconds. Deep blue, white only at the peak.
        let stage = in.drive_at.x / 7.0;
        let charge = in.drive.x;
        let since = in.drive.y;
        let l = in.local;
        let breath = 0.5 + 0.5 * sin(time * 1.1 - stage * 4.0 + in.state.w * 6.0);
        let shimmer = 0.92 + 0.08 * sin(time * 29.0 + l.x * 0.35 + l.z * 0.5);
        var level = (0.3 + 0.8 * breath * breath) * shimmer;
        let reached = smoothstep(stage * 0.85, stage * 0.85 + 0.08, charge);
        let race = pow(0.5 + 0.5 * sin(time * (5.0 + 16.0 * charge) - stage * 10.0), 6.0);
        level += reached * (1.2 + 5.0 * charge * charge + race * 3.0 * charge);
        let restless = step(0.3, hash11(floor(time * 22.0) + in.drive_at.x * 7.13 + in.state.w * 91.0));
        level *= mix(1.0, 0.5 + 0.5 * restless, charge * charge * 0.8 * reached);
        // After the shot: blinding, then spent, then back to rest.
        let blaze = 1.0 - smoothstep(0.05, 0.45, since);
        let spent = smoothstep(0.2, 0.9, since) * (1.0 - smoothstep(3.0, 14.0, since));
        level *= 1.0 - 0.93 * spent;
        level = mix(level, 34.0, blaze);
        let blue = vec3<f32>(0.07, 0.3, 1.0);
        let pale = vec3<f32>(0.5, 0.78, 1.0);
        var colour = mix(blue, pale, smoothstep(5.0, 12.0, level));
        colour = mix(colour, vec3<f32>(1.0, 0.97, 0.95), blaze);
        // Spent, the coils nearest the aperture hold a dull heat a while.
        let heat = spent * stage * (1.0 - smoothstep(1.0, 7.0, since));
        m.emissive = colour * level * 1.5 + vec3<f32>(1.0, 0.24, 0.04) * heat * 1.6;
        m.albedo = mix(vec3<f32>(0.03, 0.05, 0.09), blue * 0.25, clamp(level * 0.3, 0.0, 1.0));
        m.roughness = 0.3;
    } else if in.drive.x >= 0.0 && in.drive_at.w > 0.5 && (in.material == MAT_GLOW || in.material == MAT_GLOW_ORANGE)
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
    if (in.material == MAT_GLOW || in.material == MAT_GLOW_SHIELD) && (flags & (KIND_WRECK | KIND_GHOST | KIND_PROP)) == 0u {
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
    if (flags & (KIND_PROP | KIND_GHOST)) == 0u && in.material != MAT_GLOW && in.material != MAT_GLOW_ORANGE && in.material != MAT_GLOW_AMBER && in.material != MAT_GLOW_RED && in.material != MAT_GLOW_VIOLET && in.material != MAT_GLOW_LASER && in.material != MAT_GLOW_PRECURSOR
        && in.material != MAT_GLOW_NAV_RED && in.material != MAT_GLOW_NAV_GREEN && in.material != MAT_GLOW_LAMP && in.material != MAT_GLOW_SHIELD && in.material != MAT_PRECURSOR_INLAY && in.material != MAT_VISOR && in.material != MASS_GLOW_MATERIAL && in.material != PRISM_GLOW_MATERIAL {
        // Field dirt (`surf_dirt`): mud and spatter thrown up over the running gear and
        // lower hull, runs down the steep faces, dust on the decks, grime in the seams.
        // Plain tech 1 kit is the dirtiest; the higher tiers stay closer to parade white.
        // A capital ship is a spacecraft kept clean at any tier: no dust, no grime.
        let tech = f32(max(in.model_class & 0xFFu, 1u));
        let amount = select(1.25 / tech, 0.0, (in.model_class & 0x2000u) != 0u);
        var line = in.dust;
        var kick = 1.0;
        var grit = 1.0;
        if (in.model_class & 0x100u) == 0u {
            // Structures only pick it up around the footing — not on a
            // howitzer tube sitting over the pit — and no higher than a dust line
            // the model sets for a raised, kept foundation (`Model::dust_line`).
            line = min(0.12, in.dust);
            grit = 0.0;
        }
        if (in.model_class & 0x800u) != 0u {
            // A ship throws up spray, not dust, and the sea keeps it rinsed: its
            // waterline and grime are the hull pattern's.
            kick = 0.0;
            grit = 0.3;
        }
        // Regency plate (`pattern::EMBER`) is not paint: a unit gathers dust at its feet, never
        // grime, and a Regency building stands kept clean.
        if ((in.model_class >> 16u) & 0xFFu) == PAT_EMBER {
            grit = 0.0;
            kick = select(0.0, kick, (in.model_class & 0x100u) != 0u);
            // Dark gunmetal plate, darker seams, dark bronze machinery (regency.wgsl).
            m = regency_paint(m, in.material == MAT_METAL, regency);
        }
        let unit_at = vec3<f32>(in.state.w * 131.0, in.state.w * 71.0, in.state.w * 17.0);
        let rise = in.state.z / max(line, 0.02);
        let dirt = surf_dirt(in.local + unit_at, rise, n.z, kick, grit, seam, local_px);
        let dust = clamp(dirt.x * amount * tread, 0.0, 0.9);
        // Dry dust is pale and dull; wet mud dark, with a little sheen.
        let dirt_rgb = mix(vec3<f32>(0.24, 0.205, 0.155), vec3<f32>(0.1, 0.078, 0.055), dirt.y);
        m.albedo = mix(m.albedo, dirt_rgb * (1.0 + dirt.z * 0.6), dust);
        m.roughness = mix(m.roughness, mix(0.95, 0.72, dirt.y), dust);
        m.metallic *= 1.0 - dust;
        m.emissive *= 1.0 - dust;
        // A lamp under dust is dimmer, not out: deck and apron lights sit in the footing's dirt.
        lights *= 1.0 - dust * 0.45;
    }
    m.emissive += lights;
    // An EMP stun (emp.wgsl): lamps, glows and drives dead, stuttering back as it wears
    // off; the paint a shade darker.
    m.emissive *= emp_power(in.crackle.x, time, in.state.w);
    m.albedo = emp_albedo(m.albedo, in.crackle.x);

    if soot > 0.0 {
        // Burns from damage go over paint and dirt alike.
        m.albedo = mix(m.albedo, vec3<f32>(0.014, 0.012, 0.011), soot * 0.92);
        m.roughness = mix(m.roughness, 0.95, soot);
        m.metallic *= 1.0 - soot * 0.7;
    }

    if hull_down {
        // Dead: its lights are out, team glow and emitters too.
        m.emissive = vec3<f32>(0.0);
    }
    if wreck {
        // Burnt out (wreck.wgsl); worn away raggedly from the top as its mass goes, and a
        // section of a broken hull only its own stretch.
        // A hull that went down in the sea burns out over its first seconds on the bottom,
        // on from the paint it sank in.
        let painted = m;
        var si: SurfaceIn;
        si.st = in.face.xy;
        si.half = abs(in.face.zw);
        si.wraps = in.face.z < 0.0;
        si.seed = f32((in.model_class >> 24u) & 0xFFu) / 255.0;
        si.unit = in.state.w;
        si.scale = clamp(0.55 * pow(in.weld.z, 0.6), 0.7, 6.0);
        si.local = in.local;
        si.reach = in.weld.z;
        si.height = in.weld.w;
        let dp1 = dpdx(in.world);
        let dp2 = dpdy(in.world);
        si.px = max(length(dp1), length(dp2));
        let ws = wreck_surface(m, si, n.z, fract(in.wreck.z * 0.5) > 0.25, dpdx(in.local), dpdy(in.local));
        m = ws.m;
        // The buckled plates' relief, on the face's axes in the world (as the live plating's).
        let ds1 = dpdx(in.face.xy);
        let ds2 = dpdy(in.face.xy);
        let dp2perp = cross(dp2, n);
        let dp1perp = cross(n, dp1);
        let tangent = dp2perp * ds1.x + dp1perp * ds2.x;
        let bitangent = dp2perp * ds1.y + dp1perp * ds2.y;
        let ts = tangent * inverseSqrt(max(dot(tangent, tangent), 1e-12));
        let bs = bitangent * inverseSqrt(max(dot(bitangent, bitangent), 1e-12));
        n = normalize(n - ts * ws.slope.x - bs * ws.slope.y);
        // The deep relief, from its rise one pixel over each way (surface gradient, no tangents),
        // exaggerated: it is the wreck's texture, and has to read from the game's camera.
        let r1 = cross(dp2, n);
        let r2 = cross(n, dp1);
        let det = dot(dp1, r1);
        let grad = sign(det) * (ws.bump.x * r1 + ws.bump.y * r2);
        n = normalize(abs(det) * n - grad * 2.2);
        if in.wreck.z > 1.5 {
            let fresh = 1.0 - smoothstep(1.0, 5.0, in.wreck.w);
            m.albedo = mix(m.albedo, painted.albedo * 0.5, fresh);
            m.metallic = mix(m.metallic, painted.metallic, fresh);
            m.roughness = mix(m.roughness, painted.roughness, fresh);
        }
        if wreck_worn(in.local, in.state.z, in.state.y, in.weld.z)
            || (in.wreck.w > 0.5 && !wreck_keeps(in.local, in.wreck.x, in.wreck.y, in.weld.z)) {
            discard;
        }
    }

    let shadow = sun_shadow(in.world, n);
    // Hull plating and frames are grey metal: the sky's blue is half taken out of
    // their fill light, or the shaded side of every hull reads as blue paint.
    var sky = atmos.sky_color.rgb;
    if in.material == MAT_PLATING || in.material == MAT_ACCENT || in.material == MAT_PLATING_DARK
        || in.material == MAT_PRECURSOR || in.material == MAT_PRECURSOR_DARK {
        sky = mix(sky, vec3<f32>(dot(sky, vec3<f32>(0.2126, 0.7152, 0.0722))), 0.55);
    }
    let ao = screen_ao(in.clip.xy);
    m.roughness = specular_aa(n, m.roughness);
    let refl = env_reflection(in.world, reflect(-v, n), m.roughness, ao);
    var color = shade_pbr_refl(m, n, v, globals.sun.xyz, shadow, atmos.sun_color.rgb, sky,
        atmos.ground_color.rgb, ao, refl);
    if in.material == MAT_VISOR {
        // Tinted glass over a dark gold film, not paint: the lens is deep amber
        // until it catches the sky. A face-forward lens mirrors the ground from
        // the strategic camera, so the reflection is bent up into the sky, and
        // the bend fades toward the lens's lower half: sky-lit across the top,
        // dark amber below, as on a real visor. Weak at face-on (glass, F0 ~ 4%),
        // strong only at the rim, with a small hard glint of the sun.
        let r = reflect(-v, n);
        let up_r = normalize(vec3<f32>(r.xy, abs(r.z) + 0.2));
        let glass = env_reflection(in.world, up_r, 0.04, 1.0);
        let fres = 0.06 + 0.94 * pow(1.0 - clamp(dot(n, v), 0.0, 1.0), 5.0);
        let upward = smoothstep(-0.45, 0.5, n.z + r.z * 0.5);
        let glint = pow(max(dot(up_r, globals.sun.xyz), 0.0), 600.0);
        let tint = mix(vec3<f32>(0.9, 0.42, 0.08), vec3<f32>(1.0, 0.8, 0.6), fres);
        color = color * 0.75 + glass * tint * (0.08 + 0.55 * fres) * upward
            + atmos.sun_color.rgb * vec3<f32>(1.0, 0.85, 0.6) * glint * 3.0 * shadow;
    }
    color += m.albedo * lightning_light(in.world, n) * 0.35;
    color += local_lights(m, in.world, n, v);
    if in.material == MAT_FOLIAGE {
        color = color * leaf_occlusion + leaf_through * shadow;
    }
    var alpha = 1.0;

    if (flags & FLAG_UNDER_CONSTRUCTION) != 0u && (in.model_class & CLASS_NANITE) != 0u {
        // Built by nanites, not printed: a swarm condensing into it (`nanite_site`).
        let site = nanite_site(color, in.local, in.state.x, in.weld.w, in.state.w, time);
        if site.w < 0.5 {
            discard;
        }
        color = site.xyz;
    } else if (flags & FLAG_UNDER_CONSTRUCTION) != 0u {
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
            // Printed by a replicator (Survival): the same fill in the replication blue.
            let base = select(lit_before, vec3<f32>(0.0), order > grow);
            color = base + replication_tint(color - base);
        }
    }
    // A Regency refit (`mirror::UNIT_NANITE` on an upgrading unit) is nanite work: its
    // pieces come as a black swarm lit violet, not as ARC's amber print.
    let nanite_refit = (in.model_class & CLASS_NANITE) != 0u;
    if in.refit.z > 0.0 {
        if in.refit.x > 0.5 && in.refit.y <= 0.0 {
            // A piece whose turn has not come: a scanning hologram of it, in amber; for the
            // Regency, the swarm holding its shape with violet bands through it.
            let scan = fract(in.local.z * 1.4 - time * 0.9);
            let grid = step(0.8, fract(in.local.x * 2.0)) + step(0.8, fract(in.local.y * 2.0)) + step(0.85, scan);
            if nanite_refit {
                // Still to come: the swarm holding the piece's shape, gathering as the refit
                // runs (its holes close up), thin violet streaks through it.
                let q = in.local * 0.35 + vec3<f32>(in.state.w * 11.0, 0.0, time * 0.25);
                let holes = value_noise2(q.xy + vec2<f32>(q.z * 0.8, -q.z * 0.6), 1.0);
                if holes > 0.3 + in.refit.z * 0.9 {
                    discard;
                }
                let streak = fract(dot(in.local, vec3<f32>(0.21, 0.17, 0.5)) - time * 0.4);
                color = vec3<f32>(0.006) + NANITE_VIOLET * exp(-pow(streak - 0.5, 2.0) * 400.0) * 1.6;
            } else {
                if grid < 0.5 {
                    discard;
                }
                color = AMBER * 2.2;
            }
        } else if in.refit.x > 0.5 {
            // Going up: white-hot at first, cooling into the finished piece (violet from black
            // for the Regency).
            let cool = smoothstep(0.0, 1.0, in.refit.y);
            if nanite_refit {
                // The Regency: the swarm on the piece closes up and condenses into its plate,
                // a thin violet streak running through it until it settles.
                let q = in.local * 0.35 + vec3<f32>(in.state.w * 11.0, 0.0, time * 0.25);
                let holes = value_noise2(q.xy + vec2<f32>(q.z * 0.8, -q.z * 0.6), 1.0);
                if holes > 0.75 + in.refit.y * 2.0 {
                    discard;
                }
                let streak = fract(dot(in.local, vec3<f32>(0.21, 0.17, 0.5)) - time * 0.4);
                color = mix(vec3<f32>(0.006), color, smoothstep(0.2, 1.0, in.refit.y))
                    + NANITE_VIOLET * exp(-pow(streak - 0.5, 2.0) * 400.0) * (1.0 - cool) * 1.4;
            } else {
                color = mix(vec3<f32>(1.0, 0.66, 0.22) * 2.6, color, cool) + AMBER * 0.9 * (1.0 - cool);
            }
        } else {
            // The unit being refitted: bands of work light passing up and down it.
            let sweep = abs(fract(time * 0.23) * 2.0 - 1.0);
            let off = (in.state.z - sweep) * 16.0;
            let seam = step(0.9, fract(in.local.z * 1.1 + in.local.x * 0.4));
            color += select(AMBER, NANITE_VIOLET * 0.15, nanite_refit) * (1.7 * exp(-off * off) + 0.25 * seam * (0.5 + 0.5 * sin(time * 6.0 + in.local.z)));
        }
    }
    color = warp_hull_light(color, in.warp, time, in.state.w);
    // Arcs crawling over a stunned hull, or a charging drive's gathering over the plating.
    color += emp_arcs(in.local, in.weld.z, local_px, in.crackle.x, time, in.state.w);
    color += warp_charge_arcs(in.local, in.weld.z, local_px, in.crackle.y, in.warp.w, time, in.state.w);
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

// Bind-pose point the wrap is projected from, on the centreline at this height (x)
// and this far along (y). Everything on the skin runs out from here.
fn hull_emitter(at: vec2<f32>) -> vec3<f32> {
    return vec3<f32>(at.y, 0.0, at.x);
}

// Emitter to this point over the model's span: 0 at the emitter, about 1 at
// the farthest plate. The wrap unfolds and its waves travel along this.
fn hull_polar(local: vec3<f32>, at: vec2<f32>, reach: f32, height: f32) -> f32 {
    let span = max(max(reach, height) * 1.05, 1.0);
    return saturate(length(local - hull_emitter(at)) / span);
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
        let col = pow(globals.shield.rgb, vec3<f32>(2.2));
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
    if open * 1.08 - hull_polar(in.local, in.weld.xy, in.weld.z, in.weld.w) * 0.88 < 0.0 || open <= 0.001 {
        discard;
    }
}

// How much of a personal field shows when nothing is hitting it.
const HULL_IDLE: f32 = 0.45;

@fragment
fn fs_hull(in: VsOut) -> @location(0) vec4<f32> {
    let s = shields[in.material];
    let open = mix(s.prev_open, s.open, globals.sun.w);
    // Unfolds from the emitter over the plates, same language as a rising dome.
    let polar = hull_polar(in.local, in.weld.xy, in.weld.z, in.weld.w);
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
    // The faction's shield colour (faction.ron `shield_color`), as shields.wgsl draws it:
    // a denser version for the knot and contact, a paler one toward white for the rings.
    let energy = globals.shield.rgb;
    let team_c = globals.team_colors[s.packed & OWNER_MASK].rgb;
    let rim_c = mix(energy, team_c, 0.16);
    let ice = mix(energy, mix(energy, vec3<f32>(1.0), 0.45), 0.45);
    let deep = pow(energy, vec3<f32>(2.2));

    // The Aegis language on a skin: a tight knot at the emitter, rings
    // born there that run out over the plates, packets riding the seams on the
    // same current, and plates that breathe on their own clocks.
    let emit_d = length(in.local - hull_emitter(in.weld.xy));
    let knot_r = 0.8 + in.weld.w * 0.03;
    let knot = exp(-(emit_d * emit_d) / (knot_r * knot_r)) * (0.85 + 0.15 * sin(time * 14.0));
    // A launch pulse: the knot swells each time a ring leaves it.
    let launch = exp(-pow(abs(fract(time * 0.9) - 0.04), 2.0) * 90.0);
    let corona = exp(-(emit_d * emit_d) / (knot_r * knot_r * 7.0)) * (0.35 + 0.2 * sin(time * 5.6) + launch * 0.6);
    let phase = polar * 3.2 - time * 0.9;
    let ring0 = exp(-pow(abs(fract(phase) - 0.14), 2.0) * 150.0) * (1.0 - polar * 0.55);
    let ring1 = exp(-pow(abs(fract(phase + 0.5) - 0.14), 2.0) * 120.0) * (1.0 - polar * 0.7) * 0.6;
    // Rings light the lattice more than the glass, so the wave walks the hexes.
    let wave = (ring0 + ring1) * (0.3 + 0.7 * seam + 0.25 * plate);
    let breathe = 0.38 + 0.62 * (0.5 + 0.5 * sin(time * 2.55 + hx.y * 6.2831855));
    let run = fract(polar * 5.2 - time * 0.8 + hx.y * 0.16);
    let packet = exp(-pow(abs(run - 0.5), 2.0) * 40.0);
    let current = seam * (0.22 + 0.78 * packet) * breathe;
    let flow = pow(1.0 - polar, 2.2) * breathe;

    // Clear glass face-on: the lattice and the current carry the field, the
    // rim carries the shape. A personal field at rest is a faint sheen on the
    // plates — hits, the unfold and the peel carry it at full strength.
    let quiet = HULL_IDLE;
    var color = rim_c * (fres * 0.62 * live + stress * 0.18 + 0.08 * live) * quiet;
    color += energy * (seam * 0.55 * live + current * 1.25 + plate * 0.1 * breathe + stress * seam * 0.4 + flow * 0.18) * quiet;
    color += ice * wave * 2.4 * quiet;
    // Deep and fairly opaque, so the knot still reads over light plating.
    color += pow(deep, vec3<f32>(1.3)) * (knot * 12.0 + corona * 3.0) * quiet;
    color += mix(energy, ice, dying) * born * mix(1.3, 3.4, dying);
    color += blow.rgb;
    color += deep * touch * 2.2 * quiet;

    var alpha = (0.018 + fres * 0.09 * live + stress * 0.045) * quiet;
    alpha += (seam * 0.08 * live + current * 0.15 + plate * 0.014 * breathe + stress * seam * 0.06 + flow * 0.02) * quiet;
    alpha += (wave * 0.15 + knot * 0.7 + corona * 0.2) * quiet;
    alpha += born * mix(0.28, 0.72, dying) + blow.a + touch * 0.26 * quiet;
    alpha *= smoothstep(0.0, 0.1, open) * mix(0.8 + 0.2 * health, 1.25, dying);

    if alpha < 0.002 {
        discard;
    }
    color = apply_haze(apply_fog_of_war(color, in.world.xy), in.world, eye);
    return vec4<f32>(color * alpha, alpha);
}

// Construction light recoloured for a replicator's print (Survival): amber and its
// whites become the Precursors' cold replication blue and its ice whites; what the fill
// took away stays.
fn replication_tint(c: vec3<f32>) -> vec3<f32> {
    let add = max(c, vec3<f32>(0.0));
    let hi = max(add.r, max(add.g, add.b));
    let lo = min(add.r, min(add.g, add.b));
    return vec3<f32>(0.4, 0.64, 1.0) * (hi - lo) * 0.85 + vec3<f32>(lo) * vec3<f32>(0.9, 0.96, 1.0) + min(c, vec3<f32>(0.0));
}
