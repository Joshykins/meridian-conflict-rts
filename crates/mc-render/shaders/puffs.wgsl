//!use bindings
// Particles that are not light: dust off the tracks, gun smoke, clods of earth
// thrown by an impact, and the sparks that go with them. Like the flashes they
// are written once and animate on the GPU from their birth time.

struct Puff {
    origin: vec3<f32>,
    opacity: f32,
    pos: vec3<f32>,
    start: f32,
    // Metres per second at birth.
    vel: vec3<f32>,
    life: f32,
    // x size at birth, y size at the end (metres), z kind, w seed
    params: vec4<f32>,
    // Custom dust RGB (negative means natural), brightness. Ion: emitter velocity, 1.
    appearance: vec4<f32>,
}

const PUFF_DUST: u32 = 0u;
const PUFF_SMOKE: u32 = 1u;
const PUFF_CLOD: u32 = 2u;
const PUFF_SPARK: u32 = 3u;
// Flame: light at first, smoke by the end.
const PUFF_FIRE: u32 = 4u;
// The same on the scale of a reactor going up: tens of metres across, so it is dimmer
// and more ragged, or it would be a white disc.
const PUFF_FIREBALL: u32 = 5u;
// A missile's exhaust: thicker and slower to fade than gun smoke.
const PUFF_TRAIL: u32 = 6u;
// A blue energy slug's wake: the same ribbon as a missile trail, lit instead of sooty.
const PUFF_ARC: u32 = 7u;
// A glowing hex fragment thrown off a shattered dome.
const PUFF_SHARD: u32 = 8u;
// A blue-white spark from an energy projector or an arc hit.
const PUFF_BOLT: u32 = 9u;
// Soft blue disc around an energy slug. A glow, not a stretched sausage.
const PUFF_PLASMA: u32 = 10u;
// Aircraft exhaust: pale, unlit smoke with a soft expanding cloud silhouette.
const PUFF_CONTRAIL: u32 = 11u;
const PUFF_ION: u32 = 28u;
const PUFF_CLOUD_WISP: u32 = 29u;
// The Bastion's drives (renderer/bastion_fx.rs). A plume: a ribbon like an ion
// trail, carried with the ship (appearance.xyz), appearance.w its heat 0..1.
const PUFF_THRUST: u32 = 30u;
// A drive glow carried with the ship; appearance.w below zero is ground heated
// by a lift jet (its strength), white-blue at the heart, amber at the rim.
const PUFF_THRUST_GLOW: u32 = 31u;
// A hull lamp's flare, carried with the ship (appearance.xyz); vel is its colour
// (HDR), appearance.w how it shines: 0 steady, 1 strobe, 2..3 a turning beacon.
const PUFF_LAMP: u32 = 32u;
// A lamp's beam seen through the air: a ribbon along vel widening to size, carried
// with the ship, appearance.w its strength (night and dust).
const PUFF_LAMP_CONE: u32 = 33u;
// A faint, solid ribbon following a bomb, separate from the airy aircraft cloud.
const PUFF_BOMB_TRAIL: u32 = 12u;
const PUFF_SPLINTER: u32 = 13u;
// A directed plasma bolt: stretched along its velocity, no gravity.
const PUFF_PLASMA_BOLT: u32 = 14u;
const PUFF_SHATTER_BLAST: u32 = 15u;
const PUFF_TREE_SMOKE: u32 = 16u;
const PUFF_TREE_FIRE: u32 = 17u;
const PUFF_SHOCK_DUST: u32 = 18u;
const PUFF_SHOCK_SMOKE: u32 = 19u;
// Napalm held in the scorch for the burn, not a flame that climbs away.
const PUFF_GROUND_FIRE: u32 = 20u;
// A spent brass casing thrown out of a gun: tumbles, hits the ground, hops once, lies there.
// One that comes down on the sea is gone at the surface (water_fx.rs splashes it).
const PUFF_CASING: u32 = 21u;
// Heat torn off by a lot-clearing field (renderer/clearing.rs): the reclaim
// beam's colours, pulled up faster the longer it rises.
const PUFF_RECLAIM: u32 = 22u;
// Water on and under the sea (renderer/water_fx.rs). Thrown water and spray die
// at the surface: puffs draw after the water, so one under it would show on top.
// Water thrown up: a blob of droplets or a torn sheet flying an arc.
const PUFF_DROPLET: u32 = 23u;
// Fine spray hanging over the water, drifting off on the wind.
const PUFF_SPRAY: u32 = 24u;
// A column of white water on the surface; vel.z is how fast its top leaves the water.
const PUFF_COLUMN: u32 = 25u;
// Air rising through the water at vel.z m/s: dimmed and tinted by its depth.
const PUFF_BUBBLE: u32 = 26u;
// White vapour off a hot hull meeting the water.
const PUFF_STEAM: u32 = 27u;
// Gravity on a water column's top, and on thrown water (which the air slows).
const COLUMN_FALL: f32 = 11.0;
// Casings fall a little faster than real gravity, like everything else thrown here.
const CASING_FALL: f32 = 10.0;
const CASING_DRAG: f32 = 1.3;

fn casing_flight(p: Puff, t: f32) -> vec3<f32> {
    return p.pos + p.vel * ((1.0 - exp(-CASING_DRAG * t)) / CASING_DRAG)
        - vec3<f32>(0.0, 0.0, CASING_FALL * t * t);
}

// Where a casing is `t` seconds after it left the gun (xyz), and whether it has
// come to rest (w: 0 in the air, 1 lying still).
fn casing_at(p: Puff, t: f32) -> vec4<f32> {
    let pos = casing_flight(p, t);
    if pos.z > terrain_height(pos.xy) {
        return vec4<f32>(pos, 0.0);
    }
    // Under the ground by now: find when it struck.
    var lo = 0.0;
    var hi = t;
    for (var i = 0; i < 7; i++) {
        let mid = (lo + hi) * 0.5;
        let q = casing_flight(p, mid);
        if q.z > terrain_height(q.xy) { lo = mid; } else { hi = mid; }
    }
    let hit = casing_flight(p, hi);
    let ground = terrain_height(hit.xy);
    let v = p.vel * exp(-CASING_DRAG * hi) - vec3<f32>(0.0, 0.0, 2.0 * CASING_FALL * hi);
    // One small hop, most of the throw lost in the dirt.
    let hop = vec3<f32>(v.xy * 0.3, min(abs(v.z) * 0.28, 2.5));
    let up = hop.z / CASING_FALL;
    let dt = min(t - hi, up);
    let lie = vec3<f32>(hit.xy + hop.xy * dt, ground + 0.06 + max(hop.z * dt - 0.5 * CASING_FALL * dt * dt, 0.0));
    return vec4<f32>(lie, select(0.0, 1.0, t - hi >= up));
}


@group(1) @binding(0) var<storage, read> puffs: array<Puff>;

struct PuffOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world: vec3<f32>,
    // x age 0..1, y kind, z seed
    @location(2) @interpolate(flat) state: vec3<f32>,
    @location(3) @interpolate(flat) cloud_size: f32,
    @location(4) @interpolate(flat) roll: vec3<f32>,
    @location(5) @interpolate(flat) origin: vec3<f32>,
    @location(6) @interpolate(flat) opacity: f32,
    @location(7) @interpolate(flat) appearance: vec4<f32>,
    // Local light (lights.rs) arriving at this corner: fires and blasts glow in their smoke.
    @location(8) lamp: vec3<f32>,
}

@vertex
fn vs_puff(@location(0) corner: vec2<f32>, @builtin(instance_index) instance: u32) -> PuffOut {
    var out = puff_vertex(corner, instance);
    out.lamp = vec3<f32>(0.0);
    if out.clip.w > 0.0 {
        out.lamp = local_light_volume(out.world);
    }
    return out;
}

fn puff_vertex(corner: vec2<f32>, instance: u32) -> PuffOut {
    let p = puffs[instance];
    let t = globals.camera.w - p.start;
    let age = t / max(p.life, 0.001);
    var out: PuffOut;
    out.origin = p.origin;
    out.opacity = p.opacity;
    out.appearance = p.appearance;
    out.cloud_size = 0.0;
    out.roll = vec3<f32>(1.0, 0.0, 0.0);
    out.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
    if age < 0.0 || age >= 1.0 || p.life <= 0.0 {
        return out;
    }
    let kind = u32(p.params.z);
    if kind == PUFF_CLOUD_WISP {
        // Sample the cloud air mass carried from the birth point. This only adds
        // wisps; no cloud cover, noise, or wind field is changed by the ship.
        let source = p.pos.xy + atmos.wind.zw * t;
        let weather = weather_at(source);
        let base = cloud_floor(source) + atmos.layer.x;
        let height = (p.pos.z-base)/max(atmos.layer.y-atmos.layer.x,1.0);
        let altitude = smoothstep(-0.15,0.12,height)*(1.0-smoothstep(0.85,1.35,height));
        let moisture = (1.0-cloud_shadow_coarse(vec3<f32>(source,p.pos.z))) * 3.0;
        out.opacity *= smoothstep(0.12,0.65,weather.x)*altitude*clamp(moisture,0.0,1.0);
        if out.opacity < 0.015 { return out; }
    }
    var pos: vec3<f32>;
    // How far a casing has tumbled (radians), frozen once it lies still.
    var tumble = 0.0;
    if kind == PUFF_CASING {
        let at = casing_at(p, t);
        pos = at.xyz;
        // Into the sea: under the water it would draw on top of it.
        if pos.z < globals.map.z - 0.02 && terrain_height(pos.xy) < globals.map.z {
            return out;
        }
        let rate = 22.0 + p.params.w * 20.0;
        // Lying still: the turn it had when it landed, roughly.
        tumble = p.params.w * 40.0 + rate * select(t, min(t, 0.35 + p.params.w * 0.3), at.w > 0.5);
    } else if kind == PUFF_SHARD {
        // Detach and settle: drag kills the throw, then a slow fall. Stay above
        // the ground so a plate born on the rim does not vanish the frame it
        // is born — it fades with age instead.
        let drag = 2.2;
        pos = p.pos + p.vel * ((1.0 - exp(-drag * t)) / drag) + vec3<f32>(0.0, 0.0, -2.8 * t * t);
        pos.z = max(pos.z, terrain_height(pos.xy) + 0.4);
    } else if kind == PUFF_CLOD || kind == PUFF_SPARK || kind == PUFF_BOLT || kind == PUFF_SPLINTER {
        // Thrown: a plain arc, gone once it is back in the ground.
        pos = p.pos + p.vel * t - vec3<f32>(0.0, 0.0, 14.0 * t * t);
        // Gone into the ground, or into the sea: under the water it would draw on top of it.
        if pos.z < max(terrain_height(pos.xy) - 0.1, globals.map.z - 0.2) {
            return out;
        }
    } else if kind == PUFF_PLASMA_BOLT {
        pos = p.pos + p.vel * t;
        pos.z = max(pos.z, terrain_height(pos.xy) + 0.3);
    } else if kind == PUFF_PLASMA {
        // A sheath: hangs with the slug, a little drift, no stretch.
        pos = p.pos + p.vel * t * 0.2;
        pos.z = max(pos.z, terrain_height(pos.xy) + 0.35);
    } else if kind == PUFF_TRAIL || kind == PUFF_ARC || kind == PUFF_BOMB_TRAIL {
        // Vel is the ribbon tangent, not a drift. Smoke hangs and lifts; an arc wake stays with the shot.
        pos = p.pos + vec3<f32>(0.0, 0.0, select(0.08 * t, 0.22 * t, kind == PUFF_TRAIL));
    } else if kind == PUFF_SHOCK_DUST || kind == PUFF_SHOCK_SMOKE {
        // Sustained outward motion lets the billows roll over the terrain.
        let travel = (1.0 - exp(-0.85 * t)) / 0.85;
        pos = p.pos + p.vel * travel + vec3<f32>(0.0, 0.0, 0.18 * t);
    } else if kind == PUFF_TREE_SMOKE {
        // Persistent buoyancy and the wind form a rising plume: `vel.xy` is the
        // wind carrying it and never dies away, `vel.z` a push up that does.
        pos = p.pos + vec3<f32>(p.vel.xy * t, p.vel.z * ((1.0 - exp(-1.4 * t)) / 1.4) + 2.4 * t);
    } else if kind == PUFF_GROUND_FIRE {
        // A short lick, then it stays in the patch. A constant climb carried
        // the fire off the scorch while the burn was still going.
        let drag = 1.6;
        let lick = (1.0 - exp(-3.2 * t)) * 0.85;
        pos = p.pos + p.vel * ((1.0 - exp(-drag * t)) / drag) + vec3<f32>(0.0, 0.0, lick);
        let ground = terrain_height(pos.xy);
        pos.z = ground + max(p.params.x * 0.42, 1.5);
        // A fraction of a metre toward the camera, so a coplanar scorch does
        // not eat the flame. Not a clip-space bias: that jumps in front of
        // units once the camera is high.
        let eye_dir = globals.camera.xyz - pos;
        let eye_dist = length(eye_dir);
        if eye_dist > 1.0 {
            pos += eye_dir / eye_dist * 0.75;
        }
    } else if kind == PUFF_RECLAIM {
        let drag = 2.0;
        pos = p.pos + p.vel * ((1.0 - exp(-drag * t)) / drag) + vec3<f32>(0.0, 0.0, 1.6 * t * t);
    } else if kind == PUFF_DROPLET {
        // Thrown water: an arc the air slows a little, gone when it is back in the sea.
        let drag = 0.7;
        pos = p.pos + p.vel * ((1.0 - exp(-drag * t)) / drag) - vec3<f32>(0.0, 0.0, 0.5 * COLUMN_FALL * t * t);
        if pos.z < max(terrain_height(pos.xy), globals.map.z) {
            return out;
        }
    } else if kind == PUFF_SPRAY {
        // Hangs where it was thrown, drifts down the wind and settles a little.
        let drag = 1.6;
        pos = p.pos + p.vel * ((1.0 - exp(-drag * t)) / drag) + vec3<f32>(0.9 * t, 0.35 * t, -0.2 * t);
        pos.z = max(pos.z, max(terrain_height(pos.xy), globals.map.z) + 0.2);
    } else if kind == PUFF_STEAM {
        let drag = 2.0;
        pos = p.pos + p.vel * ((1.0 - exp(-drag * t)) / drag) + vec3<f32>(0.8 * t, 0.3 * t, 2.2 * t);
    } else if kind == PUFF_BUBBLE {
        // Rising at its own pace, wobbling, gone where it reaches the air.
        let wob = p.params.w * 40.0;
        let wobble = vec3<f32>(sin(t * 7.0 + wob), cos(t * 6.3 + wob * 1.7), 0.0) * p.params.x * 0.6;
        pos = p.pos + vec3<f32>(p.vel.xy * ((1.0 - exp(-1.5 * t)) / 1.5), p.vel.z * t) + wobble;
        if pos.z > globals.map.z - 0.08 {
            return out;
        }
    } else if kind == PUFF_COLUMN {
        pos = p.pos;
    } else if kind == PUFF_THRUST_GLOW || kind == PUFF_LAMP {
        pos = p.pos + p.appearance.xyz * t;
    } else if kind == PUFF_FIRE || kind == PUFF_TREE_FIRE {
        // A flame licks up. A few puffs are the smoke that peels off: they
        // climb farther and drift aside instead of staying in the fire.
        let drag = 3.2;
        let peel = p.params.w > 0.82;
        let side = vec3<f32>(p.params.w - 0.5, fract(p.params.w * 9.17) - 0.5, 0.0);
        pos = p.pos
            + p.vel * ((1.0 - exp(-drag * t)) / drag)
            + vec3<f32>(0.0, 0.0, select(1.7, 3.6, peel) * t)
            + select(vec3<f32>(0.0), side * 1.5 * t, peel);
        pos.z = max(pos.z, terrain_height(pos.xy) + 0.15);
    } else {
        // Carried by air: the push it was born with dies away and it drifts up.
        let drag = 2.6;
        pos = p.pos + p.vel * ((1.0 - exp(-drag * t)) / drag) + vec3<f32>(0.0, 0.0, 0.5 * t);
    }
    let size = mix(p.params.x, abs(p.params.y), sqrt(age));
    if kind == PUFF_COLUMN {
        // Its top rises and falls back like anything thrown; it stands on the
        // water as an upright billboard turned to the eye, a little into the sea
        // so its foot is lost in the splash.
        let top = p.vel.z * t - 0.5 * COLUMN_FALL * t * t;
        if top < 0.3 {
            return out;
        }
        let foot = vec3<f32>(p.pos.xy, globals.map.z - 0.4);
        let to_eye = globals.camera.xyz - foot;
        let across = vec3<f32>(-to_eye.y, to_eye.x, 0.0);
        let right = across / max(length(across), 0.001);
        let rise = corner.y * 0.5 + 0.5;
        let world = foot + right * corner.x * size * (0.5 + 0.25 * rise) + vec3<f32>(0.0, 0.0, rise * (top + size * 0.4 + 0.4));
        let clip = globals.view_proj * vec4<f32>(world, 1.0);
        if size * globals.lod.x / max(clip.w, 1.0) < 0.6 {
            return out;
        }
        out.clip = clip;
        out.uv = corner;
        out.world = world;
        out.state = vec3<f32>(age, p.params.z, p.params.w);
        // x seconds in, y height, z the speed it left the water at.
        out.roll = vec3<f32>(t, top, p.vel.z);
        out.cloud_size = size;
        return out;
    }
    if kind == PUFF_SHOCK_DUST || kind == PUFF_SHOCK_SMOKE {
        // Lift the growing billow enough that the terrain does not slice its
        // dense center into a flat-bottomed dot.
        pos.z = max(pos.z, terrain_height(pos.xy) + size * 0.18);
    }
    if kind == PUFF_ION || kind == PUFF_THRUST || kind == PUFF_LAMP_CONE {
        // A nozzle-anchored ribbon, entirely aft of the socket. vel stores axis *
        // length. World-space corners retain their real depth against the nacelle.
        let axis = normalize(p.vel);
        let root = p.pos + p.appearance.xyz * t;
        let view = normalize(globals.camera.xyz - root);
        var side = cross(axis, view);
        if length(side) < 0.01 { side = cross(axis, vec3<f32>(0.0, 0.0, 1.0)); }
        side = normalize(side);
        let along = (corner.x + 1.0) * 0.5;
        let world = root + p.vel * along + side * corner.y * size;
        out.clip = globals.view_proj * vec4<f32>(world, 1.0);
        out.uv = corner;
        out.world = world;
        out.state = vec3<f32>(age, p.params.z, p.params.w);
        return out;
    }
    if kind == PUFF_PLASMA_BOLT {
        var tangent = p.vel;
        let span = length(tangent);
        if span > 0.05 {
            tangent = tangent / span;
        } else {
            tangent = vec3<f32>(1.0, 0.0, 0.0);
        }
        let half = mix(9.6, 2.2, age) * clamp(p.params.x / 0.65, 0.3, 1.5);
        let center = globals.view_proj * vec4<f32>(pos, 1.0);
        let a = globals.view_proj * vec4<f32>(pos + tangent * half, 1.0);
        let b = globals.view_proj * vec4<f32>(pos - tangent * half, 1.0);
        let sa = a.xy / a.w;
        let sb = b.xy / b.w;
        var dir = (sa - sb) * globals.viewport.xy;
        let len = length(dir);
        if len > 0.001 {
            dir = dir / len;
        } else {
            dir = vec2<f32>(0.0, 1.0);
        }
        let side = vec2<f32>(-dir.y, dir.x);
        let width_px = max(size * 1.2 * globals.lod.x / max(center.w, 1.0), 2.8);
        let along = select(sb, sa, corner.x > 0.0);
        let ndc = along + side * corner.y * width_px * globals.viewport.zw;
        let z = mix(center.z, center.w, 0.1);
        out.clip = vec4<f32>(ndc * center.w, z, center.w);
        out.uv = corner;
        out.world = pos;
        out.state = vec3<f32>(age, p.params.z, p.params.w);
        return out;
    }
    if kind == PUFF_TRAIL || kind == PUFF_ARC || kind == PUFF_BOMB_TRAIL {
        // A negative end-size marks a faint ribbon (the Bulwark's wake).
        pos.z = max(pos.z, terrain_height(pos.xy) + 0.45);
        let span = max(length(p.vel), 0.08);
        let tangent = p.vel / span;
        // Length is the stretch this puff covers, not the puff's growing width —
        // growing the ribbon along the path walks it ahead of the shot.
        let half = max(span * select(1.08, 0.6, kind == PUFF_BOMB_TRAIL), 0.08);
        let center = globals.view_proj * vec4<f32>(pos, 1.0);
        let a = globals.view_proj * vec4<f32>(pos + tangent * half, 1.0);
        let b = globals.view_proj * vec4<f32>(pos - tangent * half, 1.0);
        let sa = a.xy / a.w;
        let sb = b.xy / b.w;
        var dir = (sa - sb) * globals.viewport.xy;
        let len = length(dir);
        if len > 0.001 {
            dir = dir / len;
        } else {
            dir = vec2<f32>(1.0, 0.0);
        }
        let side = vec2<f32>(-dir.y, dir.x);
        let width_px = max(size * 0.5 * globals.lod.x / max(center.w, 1.0), select(2.2, 0.65, kind == PUFF_BOMB_TRAIL));
        if width_px < 0.6 {
            return out;
        }
        let along = select(sb, sa, corner.x > 0.0);
        let ndc = along + side * corner.y * width_px * globals.viewport.zw;
        // One depth for the whole ribbon, pulled toward the camera so hills
        // and units do not slice it (reversed-Z: near is 1).
        let z = mix(center.z, center.w, select(0.16, 0.0, kind == PUFF_BOMB_TRAIL));
        out.clip = vec4<f32>(ndc * center.w, z, center.w);
        out.uv = corner;
        out.world = pos;
        let mark = select(p.params.w, -1.0, kind == PUFF_ARC && p.params.y < 0.0);
        out.state = vec3<f32>(age, p.params.z, mark);
        return out;
    }
    if kind == PUFF_SHOCK_DUST || kind == PUFF_SHOCK_SMOKE {
        // Give each cloud fragment its actual billboard position and depth,
        // so the terrain contact can fade smoothly instead of cutting a line.
        let right = normalize(vec3<f32>(globals.view_proj[0].x,
            globals.view_proj[1].x, globals.view_proj[2].x));
        let up = normalize(vec3<f32>(globals.view_proj[0].y,
            globals.view_proj[1].y, globals.view_proj[2].y));
        let world = pos + (right * corner.x * 1.6 + up * corner.y * 1.05) * size * 0.5;
        out.clip = globals.view_proj * vec4<f32>(world, 1.0);
        out.world = world;
        out.uv = corner;
        out.state = vec3<f32>(age, p.params.z, p.params.w);
        out.cloud_size = size;
        let drift = vec3<f32>(p.vel.xy, 0.0);
        let flow = vec2<f32>(dot(drift, right), dot(drift, up));
        out.roll = vec3<f32>(flow / max(length(flow), 0.001),
            t * 0.9 + length(drift) * ((1.0 - exp(-0.85 * t)) / 0.85) / max(size * 0.35, 1.0));
        return out;
    }
    let center = globals.view_proj * vec4<f32>(pos, 1.0);
    let mote = kind == PUFF_SPARK || kind == PUFF_BOLT || kind == PUFF_SHARD || kind == PUFF_SPLINTER || kind == PUFF_CASING || kind == PUFF_RECLAIM;
    // A floor keeps a fire visible once the camera is far enough that its true
    // size would fall under the cull and the whole patch would vanish at once.
    let floor_px = select(select(select(0.0, 1.2, mote), 3.2, kind == PUFF_FIRE), 6.0, kind == PUFF_GROUND_FIRE);
    let px = max(size * globals.lod.x / max(center.w, 1.0), floor_px);
    if px < 0.6 {
        return out;
    }
    let stretch = select(
        select(vec2<f32>(1.0), vec2<f32>(1.05, 1.55), kind == PUFF_TREE_FIRE),
        vec2<f32>(1.2, 1.45),
        kind == PUFF_GROUND_FIRE);
    let ndc = center.xy / center.w + corner * stretch * px * globals.viewport.zw;
    // Coarse terrain sits above the height sample. A flame tested at its own
    // depth is rejected as soon as the blast (which is biased forward) ends,
    // except the few tall enough to clear the mesh.
    let depth = select(center.z, center.z * 1.02, kind == PUFF_GROUND_FIRE);
    out.clip = vec4<f32>(ndc * center.w, depth, center.w);
    out.uv = corner;
    out.world = effect_billboard_world(pos, corner, size);
    out.state = vec3<f32>(age, p.params.z, p.params.w);
    if kind == PUFF_LAMP {
        out.roll = p.vel;
    }
    if kind == PUFF_CASING {
        // x tumble, y pixels across: a casing a pixel or two wide is drawn as a dot.
        out.roll = vec3<f32>(tumble, px, 0.0);
    }
    return out;
}

// Premultiplied alpha: smoke and dust cover what is behind them, sparks only add light.
fn puff_color(in: PuffOut) -> vec4<f32> {
    let d = length(in.uv);
    let age = in.state.x;
    let kind = u32(in.state.y);
    if kind != PUFF_TRAIL && kind != PUFF_ARC && kind != PUFF_BOMB_TRAIL && kind != PUFF_PLASMA_BOLT && kind != PUFF_ION && kind != PUFF_THRUST && kind != PUFF_LAMP_CONE && kind != PUFF_COLUMN && d > 1.0 {
        discard;
    }
    let eye = globals.camera.xyz;
    if kind == PUFF_CASING {
        let spin = in.roll.x;
        // Tumbling end over end: seen side on it is long, end on it is a stub.
        let long = 0.3 + 0.62 * abs(cos(spin * 0.61 + in.state.z * 5.0));
        let c = cos(spin);
        let s = sin(spin);
        let q = vec2<f32>(in.uv.x * c - in.uv.y * s, in.uv.x * s + in.uv.y * c);
        let tiny = in.roll.y < 3.5;
        let body = vec2<f32>(max(abs(q.x) - long + 0.3, 0.0), q.y);
        if !tiny && length(body) > 0.3 { discard; }
        // Brass, lit along its top, with a glint each time it turns to the light.
        let round = select(clamp(q.y / 0.3, -1.0, 1.0), 0.3, tiny);
        var brass = mix(vec3<f32>(0.14, 0.085, 0.03), vec3<f32>(0.5, 0.35, 0.13), 0.55 + 0.45 * round);
        brass = apply_haze(apply_fog_of_war(brass, in.world.xy), in.world, eye);
        let glint = pow(max(cos(spin * 1.7 + in.state.z * 9.0), 0.0), 14.0) * 1.6;
        // Just out of the breech it is still hot.
        let hot = max(1.0 - age * 7.0, 0.0);
        let fog = fog_at(in.world.xy).x;
        let light = (vec3<f32>(1.0, 0.8, 0.45) * glint + vec3<f32>(1.0, 0.32, 0.05) * hot * 1.5) * fog;

        let alpha = 1.0 - smoothstep(0.7, 1.0, age);
        return vec4<f32>((brass + light) * alpha, alpha);
    }
    if kind == PUFF_SPLINTER {

        let spin = in.state.z * 6.283 + age * 3.0;
        let q = vec2<f32>(in.uv.x * cos(spin) - in.uv.y * sin(spin), in.uv.x * sin(spin) + in.uv.y * cos(spin));
        if abs(q.x) + abs(q.y) * 4.0 > 0.95 { discard; }
        let alpha = (1.0 - age) * 0.85;
        let color = mix(vec3<f32>(1.5, 1.9, 2.0), vec3<f32>(0.2, 0.45, 0.65), age);
        return vec4<f32>(color * alpha, alpha);
    }
    if kind == PUFF_RECLAIM {
        // White-hot as it tears off, then orange, then a red that fades out.
        let heat = select(
            mix(vec3<f32>(1.0, 0.92, 0.78), vec3<f32>(1.0, 0.42, 0.07), age / 0.35),
            mix(vec3<f32>(1.0, 0.42, 0.07), vec3<f32>(0.9, 0.07, 0.02), (age - 0.35) / 0.65),
            age > 0.35);
        let flicker = 0.75 + 0.25 * sin(age * 40.0 + in.state.z * 30.0);
        let glow = pow(max(1.0 - d, 0.0), 1.6) * (1.0 - age * age) * flicker;
        return vec4<f32>(heat * 6.0 * glow, 0.0);
    }
    if kind == PUFF_SPARK {
        let heat = mix(vec3<f32>(1.0, 0.85, 0.5), vec3<f32>(1.0, 0.28, 0.04), age);
        let glow = pow(max(1.0 - d, 0.0), 1.5) * (1.0 - age * age);
        return vec4<f32>(heat * 9.0 * glow, 0.0);
    }
    if kind == PUFF_BOLT {
        let heat = mix(vec3<f32>(0.92, 0.99, 1.0), vec3<f32>(0.18, 0.42, 1.0), age);
        let glow = pow(max(1.0 - d, 0.0), 1.35) * (1.0 - age * age);
        return vec4<f32>(heat * 12.0 * glow, 0.0);
    }
    if kind == PUFF_ION {
        let x = (in.uv.x + 1.0) * 0.5;
        let y = abs(in.uv.y);
        let width = mix(0.86, 0.07, pow(x, 0.75));
        let envelope = 1.0 - smoothstep(width * 0.45, width, y);
        let core = 1.0 - smoothstep(width * 0.06, width * 0.34, y);
        // Compressible flow: four distinct bright diamonds inside a cobalt sheath.
        let cell = abs(fract(x * 4.3 - 0.12) * 2.0 - 1.0);
        let diamond = pow(max(1.0 - cell - y / max(width * 0.48, 0.01), 0.0), 2.2);
        let flutter = 0.93 + 0.07 * sin(x * 43.0 - globals.camera.w * 19.0 + in.state.z * 6.28);
        let fade = (1.0-smoothstep(0.72,1.0,age)) * (1.0-smoothstep(0.65,1.0,x));
        let rgb = vec3<f32>(0.055,0.22,1.0)*envelope*0.32
            + vec3<f32>(0.17,0.7,1.0)*core*0.12
            + vec3<f32>(0.4,0.75,1.0)*diamond*0.85;
        return vec4<f32>(rgb * fade * flutter, 0.0);
    }
    if kind == PUFF_THRUST {
        return thrust_plume(in);
    }
    if kind == PUFF_THRUST_GLOW {
        return thrust_glow(in, d);
    }
    if kind == PUFF_LAMP {
        return lamp_flare(in, d);
    }
    if kind == PUFF_LAMP_CONE {
        return lamp_cone(in);
    }
    if kind == PUFF_PLASMA_BOLT {
        let cap = length(vec2<f32>(max(abs(in.uv.x) - 0.42, 0.0), in.uv.y));
        let core = 1.0 - smoothstep(0.02, 0.55, cap);
        let bloom = 1.0 - smoothstep(0.08, 0.95, cap);
        let heat = mix(vec3<f32>(0.92, 0.99, 1.0), vec3<f32>(0.14, 0.38, 1.0), age);
        let fade = pow(max(1.0 - age, 0.0), 1.05);
        return vec4<f32>(heat*(24.0*core+9.5*bloom)*fade,0.0);
    }
    if kind == PUFF_PLASMA {
        let heat = mix(vec3<f32>(0.78, 0.96, 1.0), vec3<f32>(0.2, 0.5, 1.0), age);
        let glow = pow(max(1.0 - d, 0.0), 1.7) * (1.0 - age * age);
        return vec4<f32>(heat * 6.5 * glow, 0.0);
    }
    if kind == PUFF_SHARD {
        let spin = in.state.z * 6.283 + age * 1.8;
        let c = cos(spin);
        let s = sin(spin);
        let q = vec2<f32>(in.uv.x * c - in.uv.y * s, in.uv.x * s + in.uv.y * c);
        let g = abs(q);
        let hex = max(g.x * 0.866 + g.y * 0.5, g.y);
        if hex > 0.92 {
            discard;
        }
        // A glass plate, not a glow blob: filled hex with a hotter rim.
        let fill = 1.0 - smoothstep(0.58, 0.92, hex);
        let rim = 1.0 - smoothstep(0.72, 0.92, hex);
        let heat = mix(vec3<f32>(0.82, 0.95, 1.0), vec3<f32>(0.18, 0.48, 1.0), age);
        let glow = smoothstep(0.0, 0.08, age) * pow(max(1.0 - age, 0.0), 1.35);
        let alpha = fill * 0.42 * glow;
        return vec4<f32>(heat * (9.0 * rim + 3.2 * fill) * glow, alpha);
    }
    if kind == PUFF_CLOD {
        let alpha = (1.0 - smoothstep(0.7, 1.0, d)) * (1.0 - smoothstep(0.8, 1.0, age));
        let earth = apply_haze(apply_fog_of_war(vec3<f32>(0.075, 0.06, 0.045), in.world.xy), in.world, eye);
        return vec4<f32>(earth * alpha, alpha);
    }
    if kind == PUFF_COLUMN {
        return water_column(in);
    }
    if kind == PUFF_DROPLET || kind == PUFF_BUBBLE {
        return water_bead(in, d);
    }
    // A ragged cloud: the noise eats into the disc, more as it thins out.
    let n = textureSample(noise_map, repeat_sampler, in.uv * 0.23 + vec2<f32>(in.state.z * 3.7, in.state.z * 1.3)).b;
    var body = (1.0 - smoothstep(0.25, 1.0, d + (n - 0.5) * 0.9)) * (0.55 + n * 0.6);
    if kind == PUFF_FIREBALL || kind == PUFF_SHATTER_BLAST {
        let fine = textureSample(noise_map, repeat_sampler, in.uv * 0.61 + vec2<f32>(in.state.z * 1.9, in.state.z * 5.3)).a;
        body = (1.0 - smoothstep(0.1, 1.0, d + (n - 0.5) * 1.1 + (fine - 0.5) * 0.5)) * (0.4 + n * 0.5 + fine * 0.4);
    }
    if kind == PUFF_SHATTER_BLAST {
        let heat = mix(vec3<f32>(0.82, 0.98, 1.0), vec3<f32>(0.06, 0.3, 0.9), age);
        let flash = smoothstep(0.0, 0.035, age) * pow(1.0 - age, 1.35);
        return vec4<f32>(heat * body * flash * 8.0, 0.0);
    }
    var fade = smoothstep(0.0, 0.08, age) * pow(max(1.0 - age, 0.0), 1.4);
    if kind == PUFF_TREE_FIRE {
        // Advected turbulent fuel, with torn edges and irregular pockets of
        // hot gas. Avoid a repeated solid triangle/candle silhouette.
        let up = in.uv.y * 0.5 + 0.5;
        let flow = in.uv * vec2<f32>(2.6, 1.8) + vec2<f32>(in.state.z * 31.0, -age * 4.7);
        let coarse = grad_noise2(flow, 0.65);
        let fine = grad_noise2(flow * 2.1 + vec2<f32>(age * 0.9, 0.0), 0.42);
        let curl = (coarse - 0.5) * 0.7 * up;
        let envelope = 1.0 - abs(in.uv.x + curl) * 1.15 - up * up * 0.55;
        let fuel = coarse * 0.65 + fine * 0.35 + envelope * 0.6 - 0.40;
        let cloud = smoothstep(0.16, 0.51, fuel)
            * smoothstep(0.0, 0.12, up) * (1.0 - smoothstep(0.83, 1.0, up));
        let heat = mix(vec3<f32>(0.8, 0.065, 0.002), vec3<f32>(1.0, 0.34, 0.025), smoothstep(0.24, 0.75, fuel));
        let burn = cloud * smoothstep(0.0, 0.11, age) * (1.0 - smoothstep(0.55, 1.0, age));
        let visibility = fog_at(in.world.xy).x;
        return vec4<f32>(heat * 2.0 * burn * visibility, burn * 0.42 * visibility);
    }
    if kind == PUFF_FIRE || kind == PUFF_GROUND_FIRE {
        // Solid flame: hot core, orange body. Noise only feathers the rim,
        // so the puff burns instead of breaking into a cloud of gas.
        let lick = textureSample(noise_map, repeat_sampler, in.uv * 0.41 + vec2<f32>(in.state.z * 2.3, age * 0.55)).a;
        let edge = d + (lick - 0.5) * 0.2;
        let tongue = 1.0 - smoothstep(0.18, 0.92, edge);
        let core = pow(max(1.0 - d * 1.2, 0.0), 1.7);
        let heat = mix(
            mix(vec3<f32>(1.0, 0.96, 0.78), vec3<f32>(1.0, 0.48, 0.06), smoothstep(0.0, 0.42, d)),
            vec3<f32>(0.72, 0.08, 0.01),
            smoothstep(0.42, 0.95, d),
        );
        // A muzzle flame can peel into soot. Ground fire holds until late,
        // then eases out; the next tick has already lit a replacement.
        let peel = kind == PUFF_FIRE && in.state.z > 0.82;
        let burn = (core * 1.2 + tongue * 0.65)
            * select(1.0 - smoothstep(select(0.72, 0.94, kind == PUFF_GROUND_FIRE), 1.0, age), 1.0 - smoothstep(0.22, 0.55, age), peel);
        let smoke = select(0.0, tongue * smoothstep(0.18, 0.5, age) * pow(max(1.0 - age, 0.0), 0.75) * 0.8, peel);
        let soot = apply_haze(apply_fog_of_war(vec3<f32>(0.04, 0.038, 0.036), in.world.xy), in.world, eye);
        return vec4<f32>(heat * select(8.0, 12.0, kind == PUFF_GROUND_FIRE) * burn + soot * smoke, smoke);
    }
    if kind == PUFF_FIREBALL {
        // Burns from yellow-white through orange to a dull red, then is only the smoke it made.
        let heat = mix(mix(vec3<f32>(1.0, 0.8, 0.45), vec3<f32>(1.0, 0.35, 0.06), smoothstep(0.0, 0.35, age)), vec3<f32>(0.35, 0.05, 0.01), smoothstep(0.35, 0.8, age));
        let flame = body * smoothstep(0.0, 0.05, age) * (1.0 - smoothstep(0.45, 0.9, age));
        let smoke = clamp(body * smoothstep(0.3, 0.7, age) * pow(max(1.0 - age, 0.0), 1.2) * 0.6, 0.0, 1.0);
        let soot = apply_haze(apply_fog_of_war(vec3<f32>(0.05, 0.047, 0.045), in.world.xy), in.world, eye);
        return vec4<f32>(heat * 2.2 * flame + soot * smoke, smoke);
    }
    var color = vec3<f32>(0.5, 0.43, 0.33);
    var density = 0.55;
    if kind == PUFF_SHOCK_DUST || kind == PUFF_SHOCK_SMOKE {
        // Project layered density onto a rounded billow and roll that material
        // around an axis perpendicular to its outward ground motion.
        let dome = vec3<f32>(in.uv, sqrt(max(1.0 - dot(in.uv, in.uv), 0.0)));
        let axis = vec3<f32>(-in.roll.y, in.roll.x, 0.0);
        let angle = in.roll.z;
        let rolled = dome * cos(angle) + cross(axis, dome) * sin(angle)
            + axis * dot(axis, dome) * (1.0 - cos(angle));
        let seed = vec2<f32>(in.state.z * 3.7, in.state.z * 1.9);
        let broad = textureSample(noise_map, repeat_sampler, rolled.xy * 0.18 + seed).b;
        let folds = textureSample(noise_map, repeat_sampler,
            rolled.yz * 0.34 + seed.yx + vec2<f32>(0.17, 0.39)).a;
        let fine = textureSample(noise_map, repeat_sampler,
            rolled.zx * 0.70 + seed + vec2<f32>(0.43, 0.11)).b;
        let billow = smoothstep(0.34, 0.66, broad * 0.65 + folds * 0.35);
        let detail = smoothstep(0.25, 0.75, fine);
        let edge = d + (billow - 0.5) * 0.32 + (detail - 0.5) * 0.10;
        body = (1.0 - smoothstep(0.28, 1.0, edge))
            * (0.35 + billow * 0.65) * (0.72 + detail * 0.28);
        // Shaded folds provide depth rather than a uniformly blurred disc.
        color = mix(vec3<f32>(0.72, 0.76, 0.80),
            vec3<f32>(0.94, 0.95, 0.96), billow * 0.8 + detail * 0.2);
        density = 0.12;
        if kind == PUFF_SHOCK_SMOKE {
            color = mix(vec3<f32>(0.10, 0.095, 0.085),
                vec3<f32>(0.34, 0.32, 0.29), billow * 0.8 + detail * 0.2);
            density = 0.40;
        }
        let above_ground = in.world.z - terrain_height(in.world.xy);
        let contact = smoothstep(0.0, max(in.cloud_size * 0.22, 1.0), above_ground);
        fade = smoothstep(0.0, 0.06, age) * pow(max(1.0 - age, 0.0), 1.25) * contact;
    }
    if kind == PUFF_SMOKE || kind == PUFF_TREE_SMOKE {
        color = mix(vec3<f32>(0.07, 0.065, 0.06), vec3<f32>(0.16, 0.15, 0.14), age);
        density = 0.78;
    }
    if kind == PUFF_SPRAY || kind == PUFF_STEAM {
        // White and thin: sunlit water in the air, vapour a little greyer and denser.
        color = select(vec3<f32>(0.80, 0.84, 0.87), vec3<f32>(0.74, 0.76, 0.78), kind == PUFF_STEAM)
            * (0.55 + 0.45 * max(globals.sun.z, 0.0));
        density = select(0.22, 0.3, kind == PUFF_STEAM);
        fade = smoothstep(0.0, 0.1, age) * pow(max(1.0 - age, 0.0), 1.3);
    }
    if kind == PUFF_CLOUD_WISP {
        color = vec3<f32>(0.67,0.72,0.76) * (0.65+0.35*max(globals.sun.z,0.0));
        density = 0.10;
        fade = smoothstep(0.0,0.10,age)*pow(max(1.0-age,0.0),1.7);
    }
    if kind == PUFF_CONTRAIL {
        color = mix(vec3<f32>(0.48, 0.51, 0.54), vec3<f32>(0.42, 0.46, 0.5), age);
        density = 0.022;
        fade = smoothstep(0.0, 0.08, age) * pow(max(1.0 - age, 0.0), 1.25);
    }
    if kind == PUFF_BOMB_TRAIL {
        // Continuous coverage along the path; low opacity rather than broken puffs.
        let cap = length(vec2<f32>(max(abs(in.uv.x) - 0.75, 0.0), in.uv.y));
        body = 1.0 - smoothstep(0.2, 0.95, cap);
        color = vec3<f32>(0.72, 0.75, 0.78);
        density = 0.14;
        fade = smoothstep(0.0, 0.025, age) * pow(max(1.0 - age, 0.0), 1.1);
    }
    if kind == PUFF_TRAIL {
        // A sausage, not a disc: solid along the path, soft only at the rim.
        let cap = length(vec2<f32>(max(abs(in.uv.x) - 0.62, 0.0), in.uv.y));
        body = (1.0 - smoothstep(0.12, 0.95, cap + (n - 0.5) * 0.22)) * (0.88 + n * 0.18);
        color = mix(vec3<f32>(0.06, 0.055, 0.05), vec3<f32>(0.24, 0.23, 0.21), age);
        density = 1.0;
        fade = smoothstep(0.0, 0.04, age) * pow(max(1.0 - age, 0.0), 0.7);
    }
    if kind == PUFF_ARC {
        let cap = length(vec2<f32>(max(abs(in.uv.x) - 0.55, 0.0), in.uv.y));
        let glow = (1.0 - smoothstep(0.04, 0.92, cap)) * pow(max(1.0 - age, 0.0), 1.15);
        let heat = mix(vec3<f32>(0.82, 0.97, 1.0), vec3<f32>(0.22, 0.5, 1.0), age);
        let gain = select(11.0, 0.7, in.state.z < 0.0);
        return vec4<f32>(heat * gain * glow, 0.0);
    }
    // Preserve the shaded folds when replacing the base dust color.
    // Apply brightness before atmospheric lighting, without changing coverage.
    if in.appearance.x >= 0.0 {
        let shade = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722)) * 2.0;
        color = in.appearance.rgb * shade;
    }
    color *= in.appearance.w;
    // Smoke and dust scatter the light of fires, blasts and lamps around them.
    // Soft-capped so a fireball warms its smoke without turning the column to flame.
    let lamp = in.lamp / (1.0 + in.lamp * 0.04);
    color += color * lamp * 0.12;
    // Lit from above: the top of a cloud is brighter than its underside.
    color *= 0.8 + 0.35 * in.uv.y;
    let alpha = clamp(body * fade * density, 0.0, 1.0);
    color = apply_haze(apply_fog_of_war(color, in.world.xy), in.world, eye);
    return vec4<f32>(color * alpha, alpha);
}

// White water: sunlit on top, with the haze and the fog over it.
fn sea_white(world: vec3<f32>, up: f32) -> vec3<f32> {
    let lit = (0.55 + 0.45 * max(globals.sun.z, 0.0)) * (0.8 + 0.3 * up);
    return apply_haze(apply_fog_of_war(vec3<f32>(0.80, 0.85, 0.88) * lit, world.xy), world, globals.camera.xyz);
}

// A column of water thrown up: a jet with a ragged crown while it rises, then
// torn into falling streaks, spreading and thinning as it comes down.
fn water_column(in: PuffOut) -> vec4<f32> {
    let age = in.state.x;
    let seed = in.state.z;
    let t = in.roll.x;
    let apex = in.roll.z / COLUMN_FALL;
    let falling = smoothstep(apex * 0.7, apex * 1.5, t);
    let rise = in.uv.y * 0.5 + 0.5;
    // Streaks run up the column; finer spray breaks its edges and crown.
    let streak = textureSample(noise_map, repeat_sampler, vec2<f32>(in.uv.x * 0.32 + seed * 3.1, rise * 0.07 + t * 0.03)).b;
    let fine = textureSample(noise_map, repeat_sampler, in.uv * vec2<f32>(0.8, 0.3) + vec2<f32>(seed * 1.7, t * 0.35)).a;
    // A shell's column is narrow at the foot and bushes out into a crown, and the
    // whole of it spreads as it falls. A bullet's spout is a spike: thin at the top.
    let spout = 1.0 - smoothstep(0.5, 0.9, in.cloud_size);
    let bush = mix(0.45, 1.0, pow(smoothstep(0.0, 1.0, rise), 0.8)) * mix(0.8, 1.05, falling);
    let half = mix(bush, mix(0.95, 0.2, rise), spout);
    let edge = abs(in.uv.x) / half + (streak - 0.5) * mix(0.5, 1.1, falling) + (fine - 0.5) * 0.35;
    var body = 1.0 - smoothstep(0.5, 1.0, edge);
    body *= 1.0 - smoothstep(0.72, 1.0, rise + (fine - 0.5) * 0.4 + (streak - 0.5) * 0.25);
    // Torn into falling curtains: gaps open between the streaks.
    body *= mix(1.0, smoothstep(0.35, 0.65, streak), falling * 0.8);
    let density = mix(0.92, 0.45, falling) * (1.0 - smoothstep(0.65, 1.0, age)) * smoothstep(0.0, 0.03, age);
    let alpha = clamp(body * density, 0.0, 1.0);
    let color = sea_white(in.world, rise) * (0.85 + 0.2 * fine);
    return vec4<f32>(color * alpha, alpha);
}

// A drive plume, x along it from the mouth, y across: a blue sheath round a
// white-hot core with shock diamonds, all of it longer and hotter with throttle.
fn thrust_plume(in: PuffOut) -> vec4<f32> {
    let age = in.state.x;
    let x = (in.uv.x + 1.0) * 0.5;
    let y = abs(in.uv.y);
    let heat = clamp(in.appearance.w, 0.0, 1.0);
    let now = globals.camera.w;
    let seed = in.state.z * 6.283;
    // Full at the mouth, a slight swell, drawn to a point; the edge ripples downstream.
    let ripple = 1.0 + 0.07 * sin(x * 11.0 - now * 21.0 + seed) + 0.04 * sin(x * 27.0 - now * 37.0);
    let width = mix(0.9, 0.08, pow(x, 0.85)) * mix(1.0, 1.12, smoothstep(0.0, 0.25, x)) * ripple;
    let r = y / max(width, 0.01);
    let sheath = exp(-r * r * 1.6) * (1.0 - smoothstep(0.45, 1.0, x));
    let body = (1.0 - smoothstep(0.3, 1.0, r)) * (1.0 - smoothstep(0.35, 0.95, x));
    let core_len = mix(0.22, 0.62, heat);
    let core = (1.0 - smoothstep(0.0, 0.34, r)) * (1.0 - smoothstep(core_len * 0.45, core_len, x));
    // Shock diamonds: bright knots at even steps down the core, more of them under throttle.
    let cells = mix(3.0, 6.5, heat);
    let c = fract(x * cells + 0.4);
    // Each diamond a lens of light: widest at its middle, fading down the train.
    let knot = pow(max(1.0 - abs(c * 2.0 - 1.0) * 1.3 - r * 1.5, 0.0), 1.6)
        * (1.0 - smoothstep(core_len * 0.8, core_len * 1.5, x)) * smoothstep(0.03, 0.1, x);
    let flow = 0.88 + 0.12 * sin(x * 55.0 - now * 47.0 + seed * 3.0);
    let fade = (1.0 - smoothstep(0.7, 1.0, age)) * (1.0 - smoothstep(0.8, 1.0, x));
    let rgb = vec3<f32>(0.08, 0.3, 1.0) * sheath * 0.5
        + vec3<f32>(0.3, 0.7, 1.0) * body * 0.9
        + vec3<f32>(0.85, 0.95, 1.0) * core * 2.6
        + vec3<f32>(0.95, 0.98, 1.0) * knot * 5.0;
    return vec4<f32>(rgb * (0.35 + 1.15 * heat) * flow * fade, 0.0);
}

// A drive's glow: a nozzle mouth (blue-white), or ground under a lift jet
// (appearance.w < 0): a white-blue heart and the amber of scorched ground round it.
fn thrust_glow(in: PuffOut, d: f32) -> vec4<f32> {
    let age = in.state.x;
    let w = in.appearance.w;
    let fade = smoothstep(0.0, 0.12, age) * (1.0 - smoothstep(0.55, 1.0, age));
    let flicker = 0.9 + 0.1 * sin(globals.camera.w * 53.0 + in.state.z * 40.0);
    if w >= 0.0 {
        let halo = pow(max(1.0 - d, 0.0), 2.4);
        let core = pow(max(1.0 - d * 1.9, 0.0), 2.6);
        let rgb = vec3<f32>(0.2, 0.5, 1.0) * halo * 1.4 + vec3<f32>(0.9, 0.97, 1.0) * core * 4.0;
        return vec4<f32>(rgb * (0.15 + 1.3 * clamp(w, 0.0, 1.0)) * fade * flicker, 0.0);
    }
    let g = clamp(-w, 0.0, 1.0);
    let heart = pow(max(1.0 - d * 1.7, 0.0), 2.2);
    let rim = pow(max(1.0 - d, 0.0), 1.4) * (1.0 - heart);
    let seen = fog_at(in.world.xy).x;
    let rgb = vec3<f32>(0.8, 0.9, 1.0) * heart * 4.0 + vec3<f32>(1.0, 0.4, 0.08) * rim * 1.1 * g;
    return vec4<f32>(rgb * g * fade * flicker * seen, 0.0);
}

// A hull lamp: a hot point in a soft halo, `roll` its colour. Strobes double-flash
// (bastion_fx.rs `STROBE_PERIOD`), beacons turn (`BEACON_TURN`): bright as they face you.
fn lamp_flare(in: PuffOut, d: f32) -> vec4<f32> {
    let age = in.state.x;
    let mode = in.appearance.w;
    let now = globals.camera.w;
    var k = 1.0;
    if mode > 0.5 && mode < 1.5 {
        let ph = fract(now / 1.3);
        k = select(0.0, 1.0, ph < 0.05) + select(0.0, 0.8, ph > 0.12 && ph < 0.16);
    } else if mode >= 1.5 {
        k = 0.12 + 1.4 * pow(max(cos(now * 7.0 + (mode - 2.0) * 6.283), 0.0), 6.0);
    }
    let fade = smoothstep(0.0, 0.12, age) * (1.0 - smoothstep(0.55, 1.0, age));
    let core = pow(max(1.0 - d * 2.4, 0.0), 2.0);
    let halo = pow(max(1.0 - d, 0.0), 3.0);
    return vec4<f32>(in.roll * (core * 4.0 + halo * 0.6) * k * fade, 0.0);
}

// A lamp's beam through the air, x from the lamp, y across: narrow at the lens, full
// width at the far end, brightest near its axis and fading out to the ground.
fn lamp_cone(in: PuffOut) -> vec4<f32> {
    let age = in.state.x;
    let x = (in.uv.x + 1.0) * 0.5;
    let y = abs(in.uv.y);
    let width = mix(0.05, 1.0, x);
    let r = y / width;
    let beam = (1.0 - smoothstep(0.25, 1.0, r)) * smoothstep(0.0, 0.06, x) * (1.0 - smoothstep(0.7, 1.0, x));
    let axis = (1.0 - smoothstep(0.0, 0.45, r)) * (1.0 - x);
    // Motes in the beam drift slowly through it.
    let motes = 0.8 + 0.2 * sin(x * 23.0 - globals.camera.w * 1.3 + in.state.z * 9.0) * sin(r * 7.0 + x * 5.0);
    let fade = smoothstep(0.0, 0.12, age) * (1.0 - smoothstep(0.55, 1.0, age));
    let rgb = vec3<f32>(1.0, 0.95, 0.85) * (beam * 0.28 + axis * 0.3) * motes;
    return vec4<f32>(rgb * clamp(in.appearance.w, 0.0, 1.0) * fade, 0.0);
}

// A blob of thrown water, or a bubble seen through the water above it.
fn water_bead(in: PuffOut, d: f32) -> vec4<f32> {
    let age = in.state.x;
    let kind = u32(in.state.y);
    if kind == PUFF_BUBBLE {
        let depth = max(globals.map.z - in.world.z, 0.0);
        // The water between it and the eye: down to it, and back up the slant of the view.
        let slant = max(normalize(globals.camera.xyz - in.world).z, 0.25);
        let path = depth + depth / slant;
        let seen = exp(-path * 0.12);
        let rim = smoothstep(0.45, 0.8, d) * (1.0 - smoothstep(0.8, 1.0, d));
        let fill = 1.0 - smoothstep(0.0, 0.9, d);
        let deep = vec3<f32>(0.06, 0.28, 0.30);
        let color = mix(vec3<f32>(0.78, 0.9, 0.92), deep, 1.0 - exp(-path * 0.2)) * (0.55 + 0.45 * max(globals.sun.z, 0.0));
        let alpha = (rim * 0.8 + fill * 0.2) * seen * 0.75 * smoothstep(0.0, 0.05, age);
        let lit = apply_haze(apply_fog_of_war(color, in.world.xy), in.world, globals.camera.xyz);
        return vec4<f32>(lit * alpha, alpha);
    }
    let n = textureSample(noise_map, repeat_sampler, in.uv * 0.3 + vec2<f32>(in.state.z * 3.7, in.state.z * 1.3)).b;
    let body = 1.0 - smoothstep(0.3, 1.0, d + (n - 0.5) * 0.7);
    let alpha = clamp(body * 0.85 * (1.0 - smoothstep(0.7, 1.0, age)), 0.0, 1.0);
    return vec4<f32>(sea_white(in.world, in.uv.y * 0.5 + 0.5) * alpha, alpha);
}

@fragment
fn fs_puff(in: PuffOut) -> @location(0) vec4<f32> {
    if effect_blocked(in.origin, in.world) { discard; }
    let puff = puff_color(in);
    let alpha = clamp(puff.a * in.opacity, 0.0, 1.0);
    // Pure additive sparks have RGB with zero alpha; retain that emitted light.
    let scale = min(in.opacity, 1.0 / max(puff.a, 0.00001));
    return vec4<f32>(puff.rgb * scale, alpha);
}
