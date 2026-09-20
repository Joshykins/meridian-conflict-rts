//!use bindings
// Particles that are not light: dust off the tracks, gun smoke, clods of earth
// thrown by an impact, and the sparks that go with them. Like the flashes they
// are written once and animate on the GPU from their birth time.

struct Puff {
    pos: vec3<f32>,
    start: f32,
    // Metres per second at birth.
    vel: vec3<f32>,
    life: f32,
    // x size at birth, y size at the end (metres), z kind, w seed
    params: vec4<f32>,
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

@group(1) @binding(0) var<storage, read> puffs: array<Puff>;

struct PuffOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world: vec3<f32>,
    // x age 0..1, y kind, z seed
    @location(2) state: vec3<f32>,
}

@vertex
fn vs_puff(@location(0) corner: vec2<f32>, @builtin(instance_index) instance: u32) -> PuffOut {
    let p = puffs[instance];
    let t = globals.camera.w - p.start;
    let age = t / max(p.life, 0.001);
    var out: PuffOut;
    out.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
    if age < 0.0 || age >= 1.0 || p.life <= 0.0 {
        return out;
    }
    let kind = u32(p.params.z);
    var pos: vec3<f32>;
    if kind == PUFF_CLOD || kind == PUFF_SPARK {
        // Thrown: a plain arc, gone once it is back in the ground.
        pos = p.pos + p.vel * t - vec3<f32>(0.0, 0.0, 14.0 * t * t);
        if pos.z < terrain_height(pos.xy) - 0.1 {
            return out;
        }
    } else {
        // Carried by air: the push it was born with dies away and it drifts up.
        let drag = 2.6;
        pos = p.pos + p.vel * ((1.0 - exp(-drag * t)) / drag) + vec3<f32>(0.0, 0.0, 0.5 * t);
    }
    let size = mix(p.params.x, p.params.y, sqrt(age));
    let center = globals.view_proj * vec4<f32>(pos, 1.0);
    let px = max(size * globals.lod.x / max(center.w, 1.0), select(0.0, 1.2, kind == PUFF_SPARK));
    if px < 0.6 {
        return out;
    }
    let ndc = center.xy / center.w + corner * px * globals.viewport.zw;
    out.clip = vec4<f32>(ndc * center.w, center.z, center.w);
    out.uv = corner;
    out.world = pos;
    out.state = vec3<f32>(age, p.params.z, p.params.w);
    return out;
}

// Premultiplied alpha: smoke and dust cover what is behind them, sparks only add light.
@fragment
fn fs_puff(in: PuffOut) -> @location(0) vec4<f32> {
    let d = length(in.uv);
    if d > 1.0 {
        discard;
    }
    let age = in.state.x;
    let kind = u32(in.state.y);
    let eye = globals.camera.xyz;
    if kind == PUFF_SPARK {
        let heat = mix(vec3<f32>(1.0, 0.85, 0.5), vec3<f32>(1.0, 0.28, 0.04), age);
        let glow = pow(max(1.0 - d, 0.0), 1.5) * (1.0 - age * age);
        return vec4<f32>(heat * 9.0 * glow, 0.0);
    }
    if kind == PUFF_CLOD {
        let alpha = (1.0 - smoothstep(0.7, 1.0, d)) * (1.0 - smoothstep(0.8, 1.0, age));
        let earth = apply_haze(apply_fog_of_war(vec3<f32>(0.075, 0.06, 0.045), in.world.xy), in.world, eye);
        return vec4<f32>(earth * alpha, alpha);
    }
    // A ragged cloud: the noise eats into the disc, more as it thins out.
    let n = textureSample(noise_map, repeat_sampler, in.uv * 0.23 + vec2<f32>(in.state.z * 3.7, in.state.z * 1.3)).b;
    var body = (1.0 - smoothstep(0.25, 1.0, d + (n - 0.5) * 0.9)) * (0.55 + n * 0.6);
    if kind == PUFF_FIREBALL {
        let fine = textureSample(noise_map, repeat_sampler, in.uv * 0.61 + vec2<f32>(in.state.z * 1.9, in.state.z * 5.3)).a;
        body = (1.0 - smoothstep(0.1, 1.0, d + (n - 0.5) * 1.1 + (fine - 0.5) * 0.5)) * (0.4 + n * 0.5 + fine * 0.4);
    }
    let fade = smoothstep(0.0, 0.08, age) * pow(max(1.0 - age, 0.0), 1.4);
    if kind == PUFF_FIRE || kind == PUFF_FIREBALL {
        // Burns from yellow-white through orange to a dull red, then is only the smoke it made.
        let heat = mix(mix(vec3<f32>(1.0, 0.8, 0.45), vec3<f32>(1.0, 0.35, 0.06), smoothstep(0.0, 0.35, age)), vec3<f32>(0.35, 0.05, 0.01), smoothstep(0.35, 0.8, age));
        let flame = body * smoothstep(0.0, 0.05, age) * (1.0 - smoothstep(0.45, 0.9, age));
        let smoke = clamp(body * smoothstep(0.3, 0.7, age) * pow(max(1.0 - age, 0.0), 1.2) * 0.6, 0.0, 1.0);
        let soot = apply_haze(apply_fog_of_war(vec3<f32>(0.05, 0.047, 0.045), in.world.xy), in.world, eye);
        return vec4<f32>(heat * select(7.0, 2.2, kind == PUFF_FIREBALL) * flame + soot * smoke, smoke);
    }
    var color = vec3<f32>(0.5, 0.43, 0.33);
    var density = 0.55;
    if kind == PUFF_SMOKE {
        color = mix(vec3<f32>(0.1, 0.095, 0.09), vec3<f32>(0.34, 0.33, 0.32), age);
        density = 0.6;
    }
    // Lit from above: the top of a cloud is brighter than its underside.
    color *= 0.8 + 0.35 * in.uv.y;
    let alpha = clamp(body * fade * density, 0.0, 1.0);
    color = apply_haze(apply_fog_of_war(color, in.world.xy), in.world, eye);
    return vec4<f32>(color * alpha, alpha);
}
