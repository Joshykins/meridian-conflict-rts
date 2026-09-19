//!use bindings
// Additive sprites: projectile tracers and short-lived flashes/explosions.
// Both animate entirely on the GPU from data uploaded once (per tick for
// projectiles, per event for effects).

// Mirrors mc_sim::mirror::ProjectileInstance.
struct Projectile {
    prev_pos: vec3<f32>,
    color: u32,
    pos: vec3<f32>,
    size: f32,
}

struct Effect {
    pos: vec3<f32>,
    start: f32,
    // x radius, y lifetime seconds, z kind (0 blue, 1 orange, 2 explosion), w seed
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
    return vec3<f32>(0.45, 0.8, 1.0);
}

@vertex
fn vs_projectile(@location(0) corner: vec2<f32>, @builtin(instance_index) instance: u32) -> SpriteOut {
    let p = projectiles[instance];
    let t = globals.sun.w;
    let head = mix(p.prev_pos, p.pos, t);
    let tail = head - (p.pos - p.prev_pos) * 0.45;
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
    let width_px = max(p.size * globals.lod.x / max(a.w, 1.0), 1.6);
    let along = select(sb, sa, corner.x > 0.0) + dir * corner.x * width_px * globals.viewport.zw;
    let ndc = along + side * corner.y * width_px * globals.viewport.zw;
    let w = select(b.w, a.w, corner.x > 0.0);
    let z = select(b.z, a.z, corner.x > 0.0);
    var out: SpriteOut;
    out.clip = vec4<f32>(ndc * w, z, w);
    out.uv = corner;
    out.color = weapon_color(p.color) * 9.0;
    out.shape = vec2<f32>(0.0);
    return out;
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
    let center = globals.view_proj * vec4<f32>(e.pos, 1.0);
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
    out.shape = vec2<f32>(1.0, age);
    return out;
}

@fragment
fn fs_sprite(in: SpriteOut) -> @location(0) vec4<f32> {
    var glow: f32;
    if in.shape.x < 0.5 {
        // Tracer: bright core along the streak, soft edges across it.
        let across = 1.0 - abs(in.uv.y);
        let along = 1.0 - max(-in.uv.x, 0.0) * 0.7;
        glow = across * across * along;
    } else {
        // Flash: hot core plus an expanding shock ring.
        let d = length(in.uv);
        if d > 1.0 {
            discard;
        }
        let core = pow(1.0 - d, 2.5);
        let ring = exp(-pow((d - in.shape.y * 0.9) * 7.0, 2.0)) * 0.6;
        glow = core + ring;
    }
    return vec4<f32>(in.color * glow, 1.0);
}
