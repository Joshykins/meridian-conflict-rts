//!use bindings
// Additive sprites: projectile tracers and short-lived flashes/explosions.
// Both animate entirely on the GPU from data uploaded once (per tick for
// projectiles, per event for effects).

// Mirrors mc_sim::mirror::ProjectileInstance.
struct Projectile {
    prev_pos: vec3<f32>,
    // tracer colour | (1 << 8 for a missile) | (1 << 9 fired this tick: prev_pos is the muzzle) | (1 << 10 a construction beam
    // from prev_pos to the middle of the work at pos, `size` the work's radius) | (255ths of this tick after which the shot ends at `pos`) << 16
    color: u32,
    pos: vec3<f32>,
    size: f32,
}

struct Effect {
    pos: vec3<f32>,
    start: f32,
    // x radius, y lifetime seconds, z kind (0 blue, 1 orange, 2 explosion, 3 construction amber, 4 a reactor going up), w shock ring strength
    params: vec4<f32>,
}

@group(1) @binding(0) var<storage, read> projectiles: array<Projectile>;
@group(1) @binding(1) var<storage, read> effects: array<Effect>;

struct SpriteOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) shape: vec2<f32>,
}

fn weapon_color(kind: u32) -> vec3<f32> {
    if kind == 1u {
        return vec3<f32>(1.0, 0.45, 0.1);
    }
    if kind >= 2u {
        // Construction: yellow-orange.
        return vec3<f32>(1.0, 0.6, 0.1);
    }
    return vec3<f32>(0.45, 0.8, 1.0);
}

const BEAM: u32 = 0x400u;


// Where the shot is this frame: xyz, and w < 0 once it has landed. A shot
// that ends this tick covers its last stretch in part of the tick, at full speed.
fn shot_head(p: Projectile) -> vec4<f32> {
    let ends = f32((p.color >> 16u) & 0xFFu) / 255.0;
    var t = globals.sun.w;
    var alive = 1.0;
    if ends > 0.0 {
        t = t / ends;
        if t > 1.0 {
            alive = -1.0;
        }
    }
    return vec4<f32>(mix(p.prev_pos, p.pos, min(t, 1.0)), alive);
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
    var at = shot_head(p);
    var head = at.xyz;
    let beam = (p.color & BEAM) != 0u;
    if beam {
        head = p.pos;
        at.w = 1.0;
    }
    // A short burning trace, not a beam; and on a shot's first stretch it starts at the muzzle, not behind it.
    let stride = shot_step(p);
    var trace = length(stride) * 0.2;
    if (p.color & 0x200u) != 0u {
        trace = min(trace, distance(head, p.prev_pos));
    }
    var tail = head - normalize(stride + vec3<f32>(0.0, 0.0, 1e-6)) * trace;
    if beam {
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
    if beam {
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
    out.color = weapon_color(p.color & 0xFFu) * 9.0;
    out.shape = vec2<f32>(0.0, f32(p.color & 0xFFu));
    if beam {
        // Never quite steady; and the fragment shader needs its length for the pulses running down it.
        out.color *= 0.75 + 0.25 * sin(globals.camera.w * 37.0 + f32(instance));
        out.shape = vec2<f32>(-distance(head, tail), 2.0);
    }
    return out;
}

// The shot itself: a dot at the head of the tracer that stays a few pixels wide
// from any height, bigger for a heavier weapon. White, or yellow for a missile.
@vertex
fn vs_shot(@location(0) corner: vec2<f32>, @builtin(instance_index) instance: u32) -> SpriteOut {
    let p = projectiles[instance];
    let at = shot_head(p);
    var head = at.xyz;
    var size = p.size;
    let beam = (p.color & BEAM) != 0u;
    if beam {
        // The weld: a hot knot where the beam meets the work.
        head = p.pos;
        size = 3.4 + 1.2 * sin(globals.camera.w * 23.0 + f32(instance));
    }
    var center = globals.view_proj * vec4<f32>(head, 1.0);
    if at.w < 0.0 && !beam {
        center = vec4<f32>(0.0, 0.0, 0.0, -1.0);
    }
    // `size` grows with the square root of the damage, from 0.3.
    let least_px = 1.7 + (min(size, 1.2) - 0.3) * 2.7;
    // One pixel of margin for the soft edge.
    let px = max(size * 0.3 * globals.lod.x / max(center.w, 1.0), least_px) + 1.0;
    let ndc = center.xy / center.w + corner * px * globals.viewport.zw;
    var out: SpriteOut;
    out.clip = vec4<f32>(ndc * center.w, center.z, center.w);
    out.uv = corner;
    out.color = vec3<f32>(1.0, 1.0, 1.0);
    if (p.color & 0xFFu) == 1u {
        // A shell: white hot, a little warm.
        out.color = vec3<f32>(1.0, 0.93, 0.74);
    }
    if (p.color & 0x100u) != 0u {
        out.color = vec3<f32>(1.0, 0.8, 0.08);
    }
    if beam {
        out.color = vec3<f32>(1.0, 0.82, 0.38) * 3.4;
    }
    out.shape = vec2<f32>(px, 0.0);
    return out;
}

@fragment
fn fs_shot(in: SpriteOut) -> @location(0) vec4<f32> {
    // Distance from the rim in pixels; a dark edge keeps the dot readable on bright ground.
    let inside = (1.0 - length(in.uv)) * in.shape.x;
    let alpha = clamp(inside, 0.0, 1.0);
    if alpha <= 0.01 {
        discard;
    }
    let fill = clamp(inside - 1.0, 0.0, 1.0);
    return vec4<f32>(in.color * 2.2 * fill, alpha * mix(0.7, 1.0, fill));
}

@vertex
fn vs_effect(@location(0) corner: vec2<f32>, @builtin(instance_index) instance: u32) -> SpriteOut {
    let e = effects[instance];
    let age = (globals.camera.w - e.start) / max(e.params.y, 0.001);
    var out: SpriteOut;
    if age < 0.0 || age >= 1.0 || e.params.x <= 0.0 {
        out.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
        return out;
    }
    let grow = 1.0 - (1.0 - age) * (1.0 - age);
    let radius = e.params.x * (0.35 + 0.65 * grow);
    // Drawn a little toward the eye, so a flash on a hull or on the ground is not cut in half by what it sits on.
    let toward = normalize(globals.camera.xyz - e.pos) * min(radius * 0.6, 2.5);
    let center = globals.view_proj * vec4<f32>(e.pos + toward, 1.0);
    let px = max(radius * globals.lod.x / max(center.w, 1.0), 2.0);
    let ndc = center.xy / center.w + corner * px * globals.viewport.zw;
    out.clip = vec4<f32>(ndc * center.w, center.z, center.w);
    out.uv = corner;
    let kind = u32(e.params.z);
    var color = weapon_color(kind);
    if kind == 2u {
        color = mix(vec3<f32>(1.0, 0.85, 0.55), vec3<f32>(1.0, 0.3, 0.05), age);
    }
    out.color = color * 7.0 * (1.0 - age) * (1.0 - age);
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
    var glow: f32;
    if in.shape.x < 0.5 {
        // Tracer: bright core along the streak, soft edges across it.
        let across = 1.0 - abs(in.uv.y);
        let along = 1.0 - max(-in.uv.x, 0.0) * 0.85;
        glow = across * across * along * along;
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
            return vec4<f32>(mix(in.color, vec3<f32>(9.0, 7.4, 4.2), core * core) * glow, 1.0);
        }
    } else {
        // Flash: hot core plus an expanding shock ring.
        let d = length(in.uv);
        if d > 1.0 {
            discard;
        }
        // A reactor's flash is a broad ball of light, not a point.
        let reactor = in.shape.x > 2.5;
        var core = pow(max(1.0 - d, 0.0), 2.5);
        if reactor {
            // No edge to it: so bright that what is seen is where it stops saturating, and that shrinks as it fades.
            core = exp(-d * d * 9.0) * (1.0 - smoothstep(0.8, 1.0, d));
        }
        let off = (d - in.shape.y * 0.9) * 7.0;
        let ring = exp(-off * off) * 0.6 * (in.shape.x - select(1.0, 3.0, reactor)) / 0.9;
        glow = core + ring;
    }
    return vec4<f32>(in.color * glow, 1.0);
}
