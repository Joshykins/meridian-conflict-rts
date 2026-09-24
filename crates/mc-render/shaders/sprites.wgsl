//!use bindings
// Additive sprites: projectile tracers and short-lived flashes/explosions.
// Both animate entirely on the GPU from data uploaded once (per tick for
// projectiles, per event for effects).

// Mirrors mc_sim::mirror::ProjectileInstance.
struct Projectile {
    prev_pos: vec3<f32>,
    // tracer colour | (1 << 8 for a missile) | (1 << 9 fired this tick: prev_pos is the muzzle) | (1 << 10 a construction beam
    // from prev_pos to the middle of the work at pos, `size` the work's radius) | (1 << 11 an energy slug with a wake)
    // | (1 << 12 conventional smoke wake) | (1 << 13 an unpowered bomb) | (1 << 14 a fading hitscan beam: `size` width,
    // extras.x start, extras.y life) | (255ths of this tick after which the shot ends at `pos`) << 16
    // | (255ths of this tick at which the shot leaves the muzzle; prev_pos is behind it) << 24
    color: u32,
    pos: vec3<f32>,
    size: f32,
    // x wake hang seconds, y plasma sheath metres around the slug, z 1 for a small-calibre tracer
    extras: vec4<f32>,
    // Nose this tick and last tick. Zero: the body follows travel.
    // A cold launch pitches these onto the target while the body stays on the lob.
    aim: vec4<f32>,
    prev_aim: vec4<f32>,
}

struct Effect {
    origin: vec4<f32>,
    pos: vec3<f32>,
    start: f32,
    // x radius, y lifetime seconds, z kind (0 blue, 1 orange, 2 explosion, 3 construction amber,
    // 4 a reactor going up, 5 a blast sitting on the ground, 6 a missile-defense kill,
    // 7 a napalm wave: the impact disc, held and rolled for the burn, 8 a red-tracer gun's flash),
    // w shock ring strength or ground-burst colour
    params: vec4<f32>,
}

@group(1) @binding(0) var<storage, read> projectiles: array<Projectile>;
@group(1) @binding(1) var<storage, read> effects: array<Effect>;

struct SpriteOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) shape: vec2<f32>,
    @location(3) world: vec3<f32>,
    @location(4) @interpolate(flat) origin: vec4<f32>,
}

// One lattice corner flaring on its own clock. The cell does not move.
fn napalm_clock(i: vec2<f32>, t: f32) -> f32 {
    let phase = hash21(i) * 6.2831853;
    let rate = 0.55 + hash21(i + vec2<f32>(17.0, 5.0)) * 1.9;
    return 0.5 + 0.5 * sin(t * rate + phase);
}

// Heat that boils in place: the lattice stays on the ground and each corner
// flares and dies, so the pattern does not travel across the patch.
fn napalm_boil(xy: vec2<f32>, cell: f32, t: f32) -> f32 {
    let p = noise_lattice(xy, cell);
    let i = floor(p);
    let f = p - i;
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let x = vec2<f32>(1.0, 0.0);
    let y = vec2<f32>(0.0, 1.0);
    let a = mix(napalm_clock(i, t), napalm_clock(i + x, t), u.x);
    let b = mix(napalm_clock(i + y, t), napalm_clock(i + x + y, t), u.x);
    return mix(a, b, u.y);
}

fn napalm_lobe(uv: vec2<f32>, center: vec2<f32>, rad: f32) -> f32 {
    return 1.0 - smoothstep(rad * 0.42, rad, length(uv - center));
}

fn weapon_color(kind: u32) -> vec3<f32> {
    if kind == 1u {
        return vec3<f32>(1.0, 0.45, 0.1);
    }
    if kind == 8u {
        // A red-tracer gun's flash (`Weapon::red`).
        return vec3<f32>(1.0, 0.035, 0.015);
    }
    if kind >= 2u {
        // Construction: yellow-orange.
        return vec3<f32>(1.0, 0.6, 0.1);
    }
    return vec3<f32>(0.28, 0.68, 1.0);
}

const BEAM: u32 = 0x400u;
const FADE_BEAM: u32 = 0x4000u;
const BOMB: u32 = 0x2000u;
// A torpedo running under the water (`PROJECTILE_TORPEDO`): no tracer, a dark body seen through the sea.
const TORPEDO: u32 = 0x10u;
// The marker colour from orbit: missiles already use this, and every other shot
// switches to it once units are strategic icons. Never white from that height.
const SHOT_YELLOW: vec3<f32> = vec3<f32>(1.0, 0.8, 0.08);

// 1 when a 6 m hull would be its strategic icon. Smooth so the colour does not pop.
fn strategic_view(dist: f32) -> f32 {
    let tank_px = 6.0 * globals.lod.x / max(dist, 1.0);
    return 1.0 - smoothstep(globals.lod.y, globals.lod.y * 4.0, tank_px);
}

// Where the shot is this frame: xyz, and w < 0 once it has landed. A shot
// that ends this tick covers its last stretch in part of the tick, at full speed.
fn shot_head(p: Projectile) -> vec4<f32> {
    let ends = f32((p.color >> 16u) & 0xFFu) / 255.0;
    var t = globals.sun.w;
    var alive = 1.0;
    // A round of a stream leaves the muzzle part of the way through the tick.
    if t < shot_starts(p) {
        alive = -1.0;
    }
    if ends > 0.0 {
        t = t / ends;
        if t > 1.0 {
            alive = -1.0;
        }
    }
    return vec4<f32>(mix(p.prev_pos, p.pos, min(t, 1.0)), alive);
}

fn shot_starts(p: Projectile) -> f32 {
    return f32(p.color >> 24u) / 255.0;
}

// Where the shot left the muzzle, on its first stretch: `prev_pos`, or part of the
// way along for a round that left during the tick.
fn shot_muzzle(p: Projectile) -> vec3<f32> {
    let ends = f32((p.color >> 16u) & 0xFFu) / 255.0;
    let span = select(1.0, ends, ends > 0.0);
    return mix(p.prev_pos, p.pos, min(shot_starts(p) / span, 1.0));
}

// One tick of travel, also for a shot on its shortened last stretch.
fn shot_step(p: Projectile) -> vec3<f32> {
    let ends = f32((p.color >> 16u) & 0xFFu) / 255.0;
    if ends > 0.0 {
        return (p.pos - p.prev_pos) / ends;
    }
    return p.pos - p.prev_pos;
}

@vertex
fn vs_projectile(@location(0) corner: vec2<f32>, @builtin(instance_index) instance: u32) -> SpriteOut {
    let p = projectiles[instance];
    if (p.color & (BOMB | TORPEDO | 0x8000u)) != 0u {
        // Bombs, torpedoes and cold-ejected missiles have no powered exhaust.
        var hidden: SpriteOut;
        hidden.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
        return hidden;
    }
    var at = shot_head(p);
    var head = at.xyz;
    let beam = (p.color & BEAM) != 0u;
    let fade_beam = (p.color & FADE_BEAM) != 0u;
    if fade_beam {
        let age = (globals.camera.w - p.extras.x) / max(p.extras.y, 0.001);
        if age < 0.0 || age >= 1.0 {
            var hidden: SpriteOut;
            hidden.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
            return hidden;
        }
        head = p.pos;
        at.w = 1.0;
    } else if beam {
        head = p.pos;
        at.w = 1.0;
    }
    // A short burning trace, not a beam; and on a shot's first stretch it starts at the muzzle, not behind it.
    let stride = shot_step(p);
    let missile = (p.color & 0x100u) != 0u;
    // A sea skimmer (0x20) burns low and bright; a high-arc missile (0x40) boosts on a
    // long flame going up and falls cold once it is over the top.
    let skim = missile && (p.color & 0x20u) != 0u;
    let boost = missile && (p.color & 0x40u) != 0u;
    if boost && stride.z < 0.0 {
        var hidden: SpriteOut;
        hidden.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
        return hidden;
    }
    var trace = length(stride) * 0.2;
    if missile {
        // The flame follows the interpolated tail every frame. The solid nose
        // and body stay black, and the flame does not lag behind acceleration.
        let half_length = clamp(p.size * 1.4, 1.4, 4.8);
        head -= normalize(stride + vec3<f32>(0.0, 0.0, 1e-6)) * half_length;
        trace = half_length * select(0.65, 1.1, skim) * select(1.0, 2.4, boost);
    }
    if (p.color & 0x800u) != 0u {
        // An energy slug: a longer blue streak the wake hangs off.
        trace = length(stride) * 0.75;
    }
    if (p.color & 0x200u) != 0u {
        trace = min(trace, distance(head, shot_muzzle(p)));
    }
    var tail = head - normalize(stride + vec3<f32>(0.0, 0.0, 1e-6)) * trace;
    if fade_beam || beam {
        tail = p.prev_pos;
    }
    let a = globals.view_proj * vec4<f32>(head, 1.0);
    let b = globals.view_proj * vec4<f32>(tail, 1.0);
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
    // Width in pixels: true size up close, at least a couple of pixels from orbit.
    var width_px = max(p.size * 0.6 * globals.lod.x / max(a.w, 1.0), 1.6);
    if missile {
        width_px = max(clamp(p.size * 1.4, 1.4, 4.8) * 0.09 * globals.lod.x / max(a.w, 1.0), 0.7);
        width_px *= select(1.0, 1.7, skim) * select(1.0, 2.2, boost);
    }
    if (p.color & 0x800u) != 0u {
        width_px *= 1.45;
    }
    let plasma_m = p.extras.y;
    var core_frac = 0.0;
    if plasma_m > 0.0 && !beam && !fade_beam {
        let core_px = width_px;
        width_px = max(width_px, plasma_m * 0.95 * globals.lod.x / max(a.w, 1.0));
        core_frac = core_px / max(width_px, 0.001);
    }
    if fade_beam {
        let laser = (p.color & 0xFu) == 1u;
        let age = (globals.camera.w - p.extras.x) / max(p.extras.y, 0.001);
        // A shatter beam fades as it dies. An intercept laser holds, then cuts.
        let fade = select(pow(max(1.0 - age, 0.0), 1.2), 1.0 - smoothstep(0.72, 1.0, age), laser);
        let floor_px = select(3.4, 1.05, laser);
        width_px = max(p.size * globals.lod.x / max(a.w, 1.0), floor_px) * select(0.6 + 0.5 * fade, 0.9, laser);
        width_px = width_px * select(1.0, fade, laser);
        if (p.color & 0xFu) == 4u {
            // The plasma column keeps its cylindrical width until it extinguishes.
            width_px = max(p.size * globals.lod.x / max(a.w, 1.0), 5.0);
        }
    } else if beam {
        width_px = max(0.55 * globals.lod.x / max(a.w, 1.0), 1.4);
    }
    let along = select(sb, sa, corner.x > 0.0) + dir * corner.x * width_px * globals.viewport.zw;
    let ndc = along + side * corner.y * width_px * globals.viewport.zw;
    let w = select(b.w, a.w, corner.x > 0.0);
    let z = select(b.z, a.z, corner.x > 0.0);
    var out: SpriteOut;
    out.clip = vec4<f32>(ndc * w, z, w);
    if at.w < 0.0 {
        out.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
    }
    out.uv = corner;
    out.color = weapon_color(p.color & 0xFu) * 9.0;
    if (p.color & 0x800u) != 0u {
        // Energy slug: a hotter, more white-cyan streak the wake hangs off.
        out.color *= 1.85;
    }
    if skim {
        out.color *= 1.5;
    }
    if boost {
        // White-hot: a booster, not a motor.
        out.color = mix(out.color, vec3<f32>(1.0, 0.92, 0.8) * 9.0, 0.6) * 1.8;
    }
    out.shape = vec2<f32>(core_frac, f32(p.color & 0xFu));
    if p.extras.z > 0.5 && !beam && !fade_beam {
        // A small-calibre tracer (extras.z): red-orange through, only a warm core;
        // above one, it leans on to a deep red.
        let red = clamp(p.extras.z - 1.0, 0.0, 1.0);
        out.color = mix(vec3<f32>(1.0, 0.19, 0.03), vec3<f32>(1.0, 0.015, 0.01), red) * mix(9.0, 11.0, red);
        // 1.25: a tracer with an orange-hot core; 1.4: a red round, red-hot right through.
        out.shape.y = select(1.25, 1.4, red > 0.5);
    }
    if fade_beam {
        let laser = (p.color & 0xFu) == 1u;
        let age = (globals.camera.w - p.extras.x) / max(p.extras.y, 0.001);
        let fade = select(pow(max(1.0 - age, 0.0), 1.2), 1.0 - smoothstep(0.72, 1.0, age), laser);
        if laser {
            out.color = vec3<f32>(1.0, 0.42, 0.06) * 6.0 * fade;
            out.shape = vec2<f32>(-distance(head, tail), 4.0);
        } else if (p.color & 0xFu) == 4u {
            let envelope = smoothstep(0.0, 0.035, age) * (1.0 - smoothstep(0.82, 1.0, age));
            out.color = vec3<f32>(envelope);
            out.shape = vec2<f32>(-distance(head, tail), 5.0);
        } else if (p.color & 0xFu) == 3u {
            // An electric bore's lightning (renderer/bore_fx.rs): near white, a blue edge.
            let envelope = 1.0 - smoothstep(0.35, 1.0, age);
            let flicker = 0.8 + 0.2 * sin((globals.camera.w - p.extras.x) * 67.0);
            out.color = vec3<f32>(0.48, 0.72, 1.0) * 9.0 * envelope * flicker;
            out.shape = vec2<f32>(-distance(head, tail), 3.0);
        } else {
            out.color = vec3<f32>(0.25, 0.65, 1.0) * 4.0 * fade;
            out.shape = vec2<f32>(-distance(head, tail), 3.0);
        }
    } else if beam {
        // Never quite steady; and the fragment shader needs its length for the pulses running down it.
        out.color *= 0.75 + 0.25 * sin(globals.camera.w * 37.0 + f32(instance));
        out.shape = vec2<f32>(-distance(head, tail), 2.0);
    } else {
        let strategic = strategic_view(a.w);
        // Red rounds stay red from orbit: the colour is the gun's signature.
        out.color = mix(out.color, SHOT_YELLOW * 9.0, strategic * select(1.0, 0.0, p.extras.z > 1.5));
        // The shell's white-yellow core would keep the streak looking white from orbit.
        out.shape.y *= 1.0 - strategic;
        // Drop the plasma sheath too: from orbit it would read as a wide blue ribbon.
        out.shape.x *= 1.0 - strategic;
    }
    return out;
}

// The shot itself: a dot at the head of the tracer that stays a few pixels wide
// from any height, bigger for a heavier weapon. White-hot up close (yellow for a
// missile); from strategic view every shot is yellow.
@vertex
fn vs_shot(@location(0) corner: vec2<f32>, @builtin(instance_index) instance: u32) -> SpriteOut {
    let p = projectiles[instance];
    // Lightning segments have their own soft caps. The ordinary shot-head sprite
    // would put a bead at every kink, turning dark as the light faded.
    if (p.color & 0x100u) != 0u || ((p.color & FADE_BEAM) != 0u && ((p.color & 0xFu) == 3u || (p.color & 0xFu) == 4u)) {
        var hidden: SpriteOut;
        hidden.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
        return hidden;
    }
    let at = shot_head(p);
    var head = at.xyz;
    var size = p.size;
    let beam = (p.color & BEAM) != 0u;
    let fade_beam = (p.color & FADE_BEAM) != 0u;
    var fade = 1.0;
    let laser = fade_beam && (p.color & 0xFu) == 1u;
    if fade_beam {
        let age = (globals.camera.w - p.extras.x) / max(p.extras.y, 0.001);
        if age < 0.0 || age >= 1.0 {
            var hidden: SpriteOut;
            hidden.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
            return hidden;
        }
        fade = select(pow(max(1.0 - age, 0.0), 1.2), 1.0 - smoothstep(0.72, 1.0, age), laser);
        head = p.pos;
        size = select(p.size * 1.35, 0.42, laser);
    } else if beam {
        // The weld: a hot knot where the beam meets the work.
        head = p.pos;
        size = 3.4 + 1.2 * sin(globals.camera.w * 23.0 + f32(instance));
    }
    var center = globals.view_proj * vec4<f32>(head, 1.0);
    if at.w < 0.0 && !beam && !fade_beam {
        center = vec4<f32>(0.0, 0.0, 0.0, -1.0);
    }
    if (p.color & BOMB) != 0u {
        let ahead = globals.view_proj * vec4<f32>(head + normalize(shot_step(p) + vec3<f32>(0.0, 0.0, 1e-6)), 1.0);
        let delta = (ahead.xy / ahead.w - center.xy / center.w) * globals.viewport.xy;
        var axis = vec2<f32>(1.0, 0.0);
        if length(delta) > 0.001 {
            axis = normalize(delta);
        }
        let side = vec2<f32>(-axis.y, axis.x);
        let scale = globals.lod.x / max(center.w, 1.0);
        let length_px = max(1.1 * scale, 1.8);
        let width_px = max(0.32 * scale, 0.75);
        let offset = axis * corner.x * length_px + side * corner.y * width_px;
        let ndc = center.xy / center.w + offset * globals.viewport.zw;
        var casing: SpriteOut;
        casing.clip = vec4<f32>(ndc * center.w, center.z, center.w);
        casing.uv = corner;
        casing.color = vec3<f32>(0.012, 0.013, 0.015);
        casing.shape = vec2<f32>(-1.0, 0.0);
        return casing;
    }
    if (p.color & TORPEDO) != 0u && at.w >= 0.0 {
        // A slim dark body under the surface. The water over it is drawn before
        // it, so the depth does what the water would: it dims and greens it.
        let ahead = globals.view_proj * vec4<f32>(head + normalize(shot_step(p) + vec3<f32>(0.0, 0.0, 1e-6)), 1.0);
        let delta = (ahead.xy / ahead.w - center.xy / center.w) * globals.viewport.xy;
        var axis = vec2<f32>(1.0, 0.0);
        if length(delta) > 0.001 {
            axis = normalize(delta);
        }
        let side = vec2<f32>(-axis.y, axis.x);
        let scale = globals.lod.x / max(center.w, 1.0);
        // From strategic view it is a yellow marker like every other shot: wider,
        // solid, and not dimmed by the water over it.
        let strategic = strategic_view(center.w);
        let length_px = max(2.8 * scale, mix(2.0, 3.6, strategic));
        let width_px = max(0.32 * scale, mix(0.8, 2.0, strategic));
        let offset = axis * corner.x * length_px + side * corner.y * width_px;
        let ndc = center.xy / center.w + offset * globals.viewport.zw;
        let depth = max(globals.map.z - head.z, 0.0);
        let seen = exp(-depth * 0.28);
        var body: SpriteOut;
        body.clip = vec4<f32>(ndc * center.w, center.z, center.w);
        body.uv = corner;
        body.color = mix(vec3<f32>(0.015, 0.035, 0.04), vec3<f32>(0.01, 0.07, 0.08), 1.0 - seen);
        body.color = mix(body.color, SHOT_YELLOW * 2.2, strategic);
        body.shape = vec2<f32>(-2.0, mix(0.85 * mix(0.35, 1.0, seen), 1.0, strategic));
        return body;
    }
    // `size` grows with damage and splash, from 0.3. The floor stays modest so a
    // siege slug does not become a huge strategic icon from orbit.
    let least_px = 1.7 + (min(size, 1.2) - 0.3) * 2.7;
    // One pixel of margin for the soft edge.
    var core_px = max(size * 0.3 * globals.lod.x / max(center.w, 1.0), least_px) + 1.0;
    var halo_px = 0.0;
    if p.extras.y > 0.0 && !beam && !fade_beam {
        // Soft plasma around the core. Fades from orbit so it is not a blue disc.
        halo_px = p.extras.y * globals.lod.x / max(center.w, 1.0) * (1.0 - strategic_view(center.w));
    }
    let px = max(core_px, halo_px + 1.0);
    let ndc = center.xy / center.w + corner * px * globals.viewport.zw;
    var out: SpriteOut;
    out.clip = vec4<f32>(ndc * center.w, center.z, center.w);
    out.uv = corner;
    out.color = vec3<f32>(1.0, 1.0, 1.0);
    if (p.color & 0xFu) == 1u {
        // A shell: white hot, a little warm.
        out.color = vec3<f32>(1.0, 0.93, 0.74);
    }
    if p.extras.z > 0.5 {
        // A small-calibre tracer's head: red-orange, not white; redder above one.
        out.color = mix(vec3<f32>(1.0, 0.38, 0.1), vec3<f32>(1.0, 0.06, 0.03), clamp(p.extras.z - 1.0, 0.0, 1.0));
    }
    if (p.color & 0x100u) != 0u {
        out.color = SHOT_YELLOW;
    }
    if (p.color & 0x800u) != 0u {
        // Energy slug: a white-hot core that blooms blue.
        out.color = vec3<f32>(1.6, 1.9, 2.6);
    }
    if fade_beam {
        out.color = select(vec3<f32>(1.5, 1.9, 2.5), vec3<f32>(1.0, 0.62, 0.18), laser) * fade;
    } else if beam {
        out.color = vec3<f32>(1.0, 0.82, 0.38) * 3.4;
    } else {
        out.color = mix(out.color, SHOT_YELLOW, strategic_view(center.w));
    }
    out.shape = vec2<f32>(px, core_px / max(px, 0.001));
    return out;
}

@fragment
fn fs_shot(in: SpriteOut) -> @location(0) vec4<f32> {
    if in.shape.x < 0.0 {
        let body = length(vec2<f32>(max(abs(in.uv.x) - 0.35, 0.0) * 1.3, in.uv.y));
        // A torpedo (-2) carries how much of it the water lets through.
        let alpha = (1.0 - smoothstep(0.8, 1.0, body)) * select(1.0, in.shape.y, in.shape.x < -1.5);
        if alpha <= 0.01 {
            discard;
        }
        return vec4<f32>(in.color * (0.85 + 0.15 * in.uv.y) * alpha, alpha);
    }
    let d = length(in.uv);
    let core_frac = select(1.0, in.shape.y, in.shape.y > 0.0);
    let core_uv = d / max(core_frac, 0.001);
    // Distance from the core rim in pixels; a dark edge keeps the dot readable on bright ground.
    let inside = (1.0 - min(core_uv, 1.0)) * in.shape.x * core_frac;
    let fill = clamp(inside - 1.0, 0.0, 1.0);
    var color = in.color * 2.2 * fill;
    var alpha = 0.0;
    if core_uv <= 1.0 {
        alpha = clamp(inside, 0.0, 1.0) * mix(0.7, 1.0, fill);
    }
    if core_frac < 0.999 {
        // Blue plasma outside the slug: additive-looking, the core stays the same size.
        let halo = pow(max(1.0 - d, 0.0), 1.25) * (1.0 - fill);
        let sheath = vec3<f32>(0.12, 0.48, 1.35) * 5.2 * halo;
        color += sheath;
        alpha = max(alpha, halo * 0.62);
    }
    if alpha <= 0.01 {
        discard;
    }
    return vec4<f32>(color, alpha);
}

@vertex
fn vs_effect(@location(0) corner: vec2<f32>, @builtin(instance_index) instance: u32) -> SpriteOut {
    let e = effects[instance];
    let age = (globals.camera.w - e.start) / max(e.params.y, 0.001);
    var out: SpriteOut;
    out.origin = e.origin;
    if age < 0.0 || age >= 1.0 || e.params.x <= 0.0 {
        out.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
        return out;
    }
    let kind = u32(e.params.z);
    let grow = 1.0 - (1.0 - age) * (1.0 - age);
    let radius = e.params.x * (0.35 + 0.65 * grow);
    if kind == 6u {
        // Missile defense: the pop stays on the missile. The fragment draws the star.
        let burst = e.params.x * (0.22 + 0.78 * grow);
        let toward = normalize(globals.camera.xyz - e.pos) * 1.2;
        out.world = effect_billboard_world(e.pos + toward, corner, burst);
        let center = globals.view_proj * vec4<f32>(e.pos + toward, 1.0);
        let px = max(burst * globals.lod.x / max(center.w, 1.0), 3.0);
        let ndc = center.xy / center.w + corner * px * globals.viewport.zw;
        out.clip = vec4<f32>(ndc * center.w, center.z, center.w);
        out.uv = corner;
        out.color = vec3<f32>(1.0, 0.95, 0.85);
        // 4.2 marks the intercept star. y is the age.
        out.shape = vec2<f32>(4.2, age);
        return out;
    }
    if kind == 5u {
        // A blast on the ground: a horizontal disc draped on the terrain, not a
        // billboard punching through it.
        let xy = e.pos.xy + corner * radius;
        let world = vec3<f32>(xy, max(terrain_height(xy), e.pos.z) + 0.45);
        out.world = world;
        let clip = globals.view_proj * vec4<f32>(world, 1.0);
        out.clip = vec4<f32>(clip.xy, clip.z * 1.02, clip.w);
        out.uv = corner;
        let fade = pow(max(1.0 - age, 0.0), 2.6);
        out.color = weapon_color(u32(e.params.w)) * 7.2 * fade;
        // 2.0..2.5: a ground burst (tight core + ring, not a filled haze).
        out.shape = vec2<f32>(2.2, age);
        return out;
    }
    if kind == 7u {
        // Wider than the flames so a lobe is not sliced off into a circle.
        let disc = e.params.x * 1.18;
        let xy = e.pos.xy + corner * disc;
        let world = vec3<f32>(xy, max(terrain_height(xy), e.pos.z) + 0.4);
        out.world = world;
        let clip = globals.view_proj * vec4<f32>(world, 1.0);
        out.clip = vec4<f32>(clip.xy, clip.z * 1.02, clip.w);
        out.uv = corner;
        out.color = vec3<f32>(1.0, 0.42, 0.05);
        out.shape = vec2<f32>(5.5, age);
        return out;
    }
    var at = e.pos;
    var toward = normalize(globals.camera.xyz - at) * min(radius * 0.6, 2.5);
    if kind != 4u {
        // Clear of the ground, but leave a muzzle or a hull strike where it is —
        // lifting those by the flash radius hangs the bloom above the barrel.
        let ground = terrain_height(e.pos.xy);
        let lift = min(radius * 0.4, 6.0);
        at = vec3<f32>(e.pos.xy, max(e.pos.z, ground + lift));
        toward = normalize(globals.camera.xyz - at) * min(radius * 0.25, 1.6);
    }
    out.world = effect_billboard_world(at + toward, corner, radius);
    let center = globals.view_proj * vec4<f32>(at + toward, 1.0);
    let px = max(radius * globals.lod.x / max(center.w, 1.0), 2.0);
    let ndc = center.xy / center.w + corner * px * globals.viewport.zw;
    out.clip = vec4<f32>(ndc * center.w, center.z, center.w);
    out.uv = corner;
    var color = weapon_color(kind);
    if kind == 2u {
        color = mix(vec3<f32>(1.0, 0.85, 0.55), vec3<f32>(1.0, 0.3, 0.05), age);
    }
    if kind == 0u {
        // Energy stays cyan. A heavier shield blow is denser, not a white disc.
        // params.w above 1 (the Bulwark's impact) pushes the core bluer and hotter.
        let cool = saturate(e.params.w);
        let extra = max(e.params.w - 1.0, 0.0);
        color = mix(vec3<f32>(0.26, 0.72, 1.0), vec3<f32>(0.48, 0.86, 1.0), cool * 0.4);
        color = mix(color, vec3<f32>(0.10, 0.38, 1.0), saturate(extra));
        out.color = color * (4.0 + cool * 1.6) * (1.0 + extra * 1.5) * (1.0 - age) * (1.0 - age);
    } else {
        out.color = color * 7.0 * (1.0 - age) * (1.0 - age);
    }
    // x > 0.5 marks a flash; its fraction is the shock ring's strength. y is the age.
    out.shape = vec2<f32>(1.0 + clamp(e.params.w, 0.0, 1.0) * 0.9, age);
    if kind == 4u {
        // White beyond what the screen can show, for longer than is comfortable, then the colour of fire.
        color = mix(vec3<f32>(1.0, 0.97, 0.9), vec3<f32>(1.0, 0.45, 0.12), smoothstep(0.25, 0.9, age));
        out.color = color * 60.0 * pow(1.0 - age, 1.6);
        out.shape.x += 2.0;
        // Light this bright is not hidden by the ground it stands on: drawn in front of everything.
        out.clip.z = out.clip.w * 0.9999;
    }
    return out;
}

@fragment
fn fs_sprite(in: SpriteOut) -> @location(0) vec4<f32> {
    if in.origin.w > 0.5 && effect_blocked(in.origin.xyz, in.world) { discard; }
    var glow: f32;
    if in.shape.x < 0.5 {
        // Tracer: bright core along the streak, soft edges across it.
        var across = 1.0 - abs(in.uv.y);
        let along = 1.0 - max(-in.uv.x, 0.0) * 0.85;
        glow = across * across * along * along;
        if in.shape.x > 0.02 && in.shape.x < 0.5 {
            // Plasma sheath: the hot streak stays the old width; the rest is soft blue.
            let core_across = max(1.0 - abs(in.uv.y) / in.shape.x, 0.0);
            glow = core_across * core_across * along * along;
            let halo = pow(across, 1.45) * along * (1.0 - core_across * 0.55);
            let plasma = vec3<f32>(0.1, 0.45, 1.4) * 9.0 * halo;
            return vec4<f32>(in.color * glow + plasma, 1.0);
        }
        if in.shape.y > 4.5 {
            // Argon plasma: a continuous white-cyan cylinder, a broad blue sheath,
            // and travelling density ripples. Lightning is drawn separately around it.
            let run = (in.uv.x * 0.5 + 0.5) * -in.shape.x;
            let flow = 0.9 + 0.1 * sin(run * 0.28 - globals.camera.w * 21.0);
            let core = pow(across, 4.0);
            let sheath = pow(across, 1.6);
            let rgb = vec3<f32>(0.10, 0.38, 1.0) * sheath * 3.6
                + vec3<f32>(0.78, 0.94, 1.0) * core * 8.5 * flow;
            return vec4<f32>(rgb * in.color.r, 1.0);
        }
        if in.shape.y > 3.5 {
            // Intercept laser: a white filament in an orange sheath, with a bead running to the missile.
            let core = pow(across, 10.0);
            let sheath = pow(across, 1.7);
            let run = in.uv.x * 0.5 + 0.5;
            let bead = exp(-pow(fract(run * 2.4 - globals.camera.w * 7.0) - 0.82, 2.0) * 90.0);
            let held = in.color.r / 6.0;
            let rgb = vec3<f32>(1.0, 0.38, 0.04) * sheath * 2.4
                + vec3<f32>(1.0, 0.96, 0.88) * core * 7.0
                + vec3<f32>(1.0, 0.72, 0.28) * bead * core * 5.0;
            return vec4<f32>(rgb * held, 1.0);
        }
        if in.shape.y > 2.5 {
            // A brief cool filament: all brightness follows the lifetime fade.
            // The old fixed white core stayed saturated until it disappeared.
            let core = pow(across, 6.2);
            let bloom = pow(across, 2.0);
            return vec4<f32>(in.color * (core * 0.7 + bloom * 0.18), 1.0);
        }
        if in.shape.y > 1.5 {
            // A construction beam: a steady hot thread with pulses running down it to the work.
            let run = (in.uv.x * 0.5 + 0.5) * -in.shape.x;
            let pulse = 0.62 + 0.38 * sin(run * 1.35 - globals.camera.w * 22.0);
            let bead = exp(-pow(fract(run * 0.22 - globals.camera.w * 3.4) - 0.5, 2.0) * 70.0);
            let core = pow(across, 5.0);
            let glow = mix(in.color * 0.5, vec3<f32>(10.0, 8.2, 3.8), core) * across * across * pulse;
            return vec4<f32>(glow + vec3<f32>(4.0, 3.0, 1.1) * bead * core, 1.0);
        }
        if in.shape.y > 0.5 && in.shape.y < 1.5 {
            // A shell's trace burns from white-yellow at the core to orange at the edge.
            let core = across * across * along;
            var hot = select(vec3<f32>(9.0, 7.4, 4.2), vec3<f32>(9.0, 4.4, 1.1), in.shape.y > 1.1);
            if in.shape.y > 1.3 {
                hot = vec3<f32>(10.0, 1.1, 0.45);
            }
            return vec4<f32>(mix(in.color, hot, core * core) * glow, 1.0);
        }
    } else {
        // Flash: hot core plus an expanding shock ring.
        let d = length(in.uv);
        if in.shape.x > 5.2 && in.shape.x < 6.0 {
            // A shifted mass, a tongue and a bite. The outline is that puddle,
            // torn by the ground. Heat flares in place instead of sliding.
            let uv = in.uv;
            let t = globals.camera.w;
            let anchor = in.origin.xy;
            let h0 = hash21(anchor);
            let h1 = hash21(anchor + vec2<f32>(4.2, 1.7));
            let dir = vec2<f32>(cos(h0 * 6.2831853), sin(h0 * 6.2831853));
            let side = vec2<f32>(-dir.y, dir.x);
            let main_c = dir * (0.04 + h1 * 0.08);
            let tongue_c = dir * (0.34 + h1 * 0.1);
            let side_c = side * (0.18 + h1 * 0.14) - dir * 0.06;
            let bite_c = -dir * 0.2 + side * (0.06 + h1 * 0.1);
            let r_main = 0.44 * (0.8 + 0.28 * napalm_boil(anchor, 8.0, t * 0.55));
            let r_tongue = 0.30 * (0.72 + 0.36 * napalm_boil(anchor + dir * 7.0, 6.0, t));
            let r_side = 0.24 * (0.7 + 0.36 * napalm_boil(anchor + side * 7.0, 6.0, t * 1.15));
            let puddle = max(
                napalm_lobe(uv, main_c, r_main),
                max(napalm_lobe(uv, tongue_c, r_tongue), napalm_lobe(uv, side_c, r_side)),
            );
            let bite = napalm_lobe(uv, bite_c, 0.22 + h1 * 0.08);
            let coast = grad_noise2(in.world.xy, 2.6);
            let torn = puddle * (0.35 + coast * 1.05) - bite * 0.8;
            let boil = napalm_boil(in.world.xy, 4.6, t);
            let tongues = napalm_boil(in.world.xy + vec2<f32>(8.0, 3.0), 2.7, t * 1.15);
            let field = torn * (0.5 + boil * 0.35 + tongues * 0.28);
            if field < 0.22 {
                discard;
            }
            let flame = smoothstep(0.22, 0.46, field);
            let hot = smoothstep(0.62, 0.88, boil * max(torn, 0.0));
            let fade = smoothstep(0.0, 0.05, in.shape.y) * (1.0 - smoothstep(0.86, 1.0, in.shape.y));
            let heat = mix(vec3<f32>(0.42, 0.04, 0.0), vec3<f32>(1.0, 0.3, 0.015), smoothstep(0.28, 0.55, field));
            let heat2 = mix(heat, vec3<f32>(1.0, 0.8, 0.38), hot);
            return vec4<f32>(heat2 * 8.5 * flame * (0.5 + 0.5 * tongues) * fade, 1.0);
        }
        if d > 1.0 {
            discard;
        }
        // Missile defense: a white snap, an orange ring, and a four-point star.
        // Nothing else in the battle draws this shape.
        if in.shape.x > 3.8 && in.shape.x < 4.8 {
            let age = in.shape.y;
            let core = exp(-d * d * 22.0) * pow(max(1.0 - age, 0.0), 1.2);
            let ring_r = mix(0.12, 0.9, age);
            let ring = exp(-pow(abs(d - ring_r) * 16.0, 2.0)) * pow(max(1.0 - age, 0.0), 0.7);
            let spike = pow(abs(cos(atan2(in.uv.y, in.uv.x) * 2.0)), 22.0)
                * pow(max(1.0 - d, 0.0), 0.45)
                * pow(max(1.0 - age, 0.0), 1.6);
            let rgb = vec3<f32>(1.0, 0.98, 0.92) * core * 26.0
                + vec3<f32>(1.0, 0.42, 0.06) * ring * 11.0
                + vec3<f32>(1.0, 0.78, 0.38) * spike * 10.0;
            return vec4<f32>(rgb, 1.0);
        }
        // A reactor's flash is a broad ball of light, not a point.
        let reactor = in.shape.x > 2.5;
        let ground = in.shape.x > 2.0 && !reactor;
        var core = pow(max(1.0 - d, 0.0), 2.5);
        if reactor {
            // No edge to it: so bright that what is seen is where it stops saturating, and that shrinks as it fades.
            core = exp(-d * d * 9.0) * (1.0 - smoothstep(0.8, 1.0, d));
        } else if ground {
            // Hot centre and a lip, not a misty disc filling the crater.
            core = pow(max(1.0 - d, 0.0), 4.8);
        }
        let off = (d - in.shape.y * 0.9) * 7.0;
        let ring = exp(-off * off) * 0.6 * (in.shape.x - select(1.0, 3.0, reactor)) / 0.9;
        glow = core + ring;
    }
    return vec4<f32>(in.color * glow, 1.0);
}

// Solid missile geometry. Local X follows flight; its motor is a separate
// effect behind the tail, so the pointed black casing never becomes a tracer.
struct MissileOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
}
fn missile_axis(p: Projectile) -> vec3<f32> {
    if (p.color & 0x8000u) != 0u {
        let nose = mix(p.prev_aim.xyz, p.aim.xyz, globals.sun.w);
        if length(nose) > 0.2 {
            return normalize(nose);
        }
    }
    return normalize(shot_step(p) + vec3<f32>(0.0, 0.0, 1e-6));
}

@vertex
fn vs_missile(@builtin(vertex_index) vertex: u32, @builtin(instance_index) instance: u32) -> MissileOut {
    let p = projectiles[instance];
    var out: MissileOut;
    let at = shot_head(p);
    if (p.color & 0x100u) == 0u || at.w < 0.0 {
        out.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
        out.normal = vec3<f32>(0.0, 0.0, 1.0);
        return out;
    }
    let half_length = clamp(p.size * 1.4, 1.4, 4.8);
    let radius = half_length * 0.14;
    var local = vec3<f32>(0.0);
    var normal = vec3<f32>(0.0);
    let quad = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);
    if vertex < 48u {
        let segment = vertex / 6u;
        let corner = quad[vertex % 6u];
        let a = (f32(segment) + select(0.0, 1.0, corner == 1u || corner == 2u)) * 0.785398163;
        local = vec3<f32>(select(-1.0, 0.55, corner >= 2u) * half_length, cos(a) * radius, sin(a) * radius);
        let middle = (f32(segment) + 0.5) * 0.785398163;
        normal = vec3<f32>(0.0, cos(middle), sin(middle));
    } else if vertex < 96u {
        let nose = vertex < 72u;
        let v = (vertex - 48u) % 24u;
        let a = (f32(v / 3u) + select(0.0, 1.0, v % 3u == 1u)) * 0.785398163;
        local = vec3<f32>(select(-1.0, 0.55, nose) * half_length, cos(a) * radius, sin(a) * radius);
        if v % 3u == 2u {
            local = vec3<f32>(select(-1.0, 1.0, nose) * half_length, 0.0, 0.0);
        }
        let middle = (f32(v / 3u) + 0.5) * 0.785398163;
        normal = select(vec3<f32>(-1.0, 0.0, 0.0), normalize(vec3<f32>(0.31, cos(middle), sin(middle))), nose);
    } else {
        let fin = (vertex - 96u) / 6u;
        let corner = quad[(vertex - 96u) % 6u];
        let a = f32(fin) * 1.570796327;
        let profile = array<vec2<f32>, 4>(
            vec2<f32>(-0.96, 0.12), vec2<f32>(-0.96, 0.38),
            vec2<f32>(-0.65, 0.38), vec2<f32>(-0.25, 0.12));
        let q = profile[corner] * half_length;
        local = vec3<f32>(q.x, cos(a) * q.y, sin(a) * q.y);
        normal = vec3<f32>(0.0, -sin(a), cos(a));
    }
    let axis = missile_axis(p);
    let reference = select(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 1.0, 0.0), abs(axis.z) > 0.95);
    let side = normalize(cross(axis, reference));
    let up = cross(side, axis);
    out.clip = globals.view_proj * vec4<f32>(at.xyz + axis * local.x + side * local.y + up * local.z, 1.0);
    out.normal = axis * normal.x + side * normal.y + up * normal.z;
    return out;
}
@fragment
fn fs_missile(in: MissileOut) -> @location(0) vec4<f32> {
    let light = 0.45 + 0.55 * abs(dot(normalize(in.normal), normalize(vec3<f32>(0.4, -0.5, 0.8))));
    return vec4<f32>(vec3<f32>(0.024, 0.028, 0.033) * light, 1.0);
}
