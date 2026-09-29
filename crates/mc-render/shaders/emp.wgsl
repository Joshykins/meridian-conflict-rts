// An EMP stun on a model (`mc_sim::warp`, `UnitInstance::fx.zw`), prepended to shaders
// with `//!use emp`. While the stun holds the unit's lamps, glows and drives are dead
// and its hull a shade darker, with blue-white arcs crawling over it; as it wears off
// (the last 3 s, the stun falling from 1 to 0) the lights stutter back on and the arcs
// thin out.

// Seconds an arc stays in one place before it jumps to another.
const EMP_HOP: f32 = 0.085;
// How dark the paint goes under a full stun.
const EMP_DARKEN: f32 = 0.45;
// The arcs' colour: blue-white, a white core (linear, HDR).
const EMP_ARC: vec3<f32> = vec3<f32>(0.42, 0.66, 1.0);
const EMP_CORE: vec3<f32> = vec3<f32>(0.9, 0.95, 1.0);

fn emp_hash3(p: vec3<f32>) -> f32 {
    var q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    q += dot(q, q.yxz + 33.33);
    return fract((q.x + q.y) * q.z);
}

// Value noise in 3D, [0, 1], one feature per unit.
fn emp_noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = p - i;
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(emp_hash3(i), emp_hash3(i + vec3<f32>(1.0, 0.0, 0.0)), u.x);
    let b = mix(emp_hash3(i + vec3<f32>(0.0, 1.0, 0.0)), emp_hash3(i + vec3<f32>(1.0, 1.0, 0.0)), u.x);
    let c = mix(emp_hash3(i + vec3<f32>(0.0, 0.0, 1.0)), emp_hash3(i + vec3<f32>(1.0, 0.0, 1.0)), u.x);
    let d = mix(emp_hash3(i + vec3<f32>(0.0, 1.0, 1.0)), emp_hash3(i + vec3<f32>(1.0, 1.0, 1.0)), u.x);
    return mix(mix(a, b, u.y), mix(c, d, u.y), u.z);
}

// How far a stunned unit's lamps, glows and drives are lit, 0 to 1. Dead while the stun
// holds; as it wears off they stutter: lit or dark a twelfth of a second at a time,
// lit more often and brighter the nearer it is to over. `seed` tells units apart.
fn emp_power(stun: f32, time: f32, seed: f32) -> f32 {
    if stun <= 0.0 {
        return 1.0;
    }
    if stun >= 0.999 {
        return 0.0;
    }
    let slot = floor(time * 12.0);
    let on = step(stun * 0.9 + 0.05, hash11(slot * 1.37 + seed * 91.7));
    // A brownout: now and then what is lit sags to a glimmer.
    let sag = select(1.0, 0.3, hash11(slot * 0.71 + seed * 13.1) < stun * 0.5);
    return on * sag * (1.0 - stun * 0.5);
}

// Paint under a stun: a shade darker (the systems that light and heat it are off).
fn emp_albedo(albedo: vec3<f32>, stun: f32) -> vec3<f32> {
    return albedo * (1.0 - EMP_DARKEN * clamp(stun * 1.5, 0.0, 1.0));
}

// One layer of arcs: where a noise field crosses its middle, over patches that jump every
// `EMP_HOP` seconds. `q` is the point in cells, `px` a pixel in cells.
fn emp_arc_layer(q: vec3<f32>, px: f32, time: f32, seed: f32, phase: f32, cover: f32) -> vec2<f32> {
    let slot = floor(time / EMP_HOP + phase);
    let jump = vec3<f32>(hash11(slot + seed * 17.0), hash11(slot * 1.3 + seed * 5.0), hash11(slot * 0.7 + seed)) * 61.0;
    // Where arcs are this instant: patches over a share of the hull.
    let spot = emp_noise3(q * 0.3 + jump);
    let on = smoothstep(1.0 - cover, 1.0 - cover + 0.08, spot);
    if on <= 0.0 {
        return vec2<f32>(0.0);
    }
    // The arc: a crooked line (the field warped by a finer one), crawling as it holds.
    let crawl = vec3<f32>(0.0, 0.0, (time / EMP_HOP - floor(time / EMP_HOP)) * 0.35);
    // Kinked, not smooth: a coarse warp bends it, a fine one breaks it into jags.
    let warp = emp_noise3(q * 3.1 + jump.yzx) - 0.5;
    let jag = emp_noise3(q * 11.0 + jump.xzy) - 0.5;
    let f = emp_noise3(q * 1.1 + jump.zxy + crawl + warp * 0.55 + jag * 0.12);
    // Distance to the line in cells: the field climbs about 1.6 a cell.
    let d = abs(f - 0.5) / 1.6;
    // At least a pixel and a half wide, however far; a hair's width up close.
    let w = max(px * 0.8, 0.008);
    let line = 1.0 - smoothstep(0.0, w, d);
    let core = 1.0 - smoothstep(0.0, w * 0.35, d);
    // Each arc flickers as it holds.
    let flick = 0.55 + 0.45 * hash11(floor(time * 40.0) + slot + seed * 3.0);
    return vec2<f32>(line, core) * on * flick;
}

// Arcs crawling over a stunned hull, as light to add. `local` is the model-space point,
// `reach` the model's size (its cells scale with it, so a warship carries dozens of arcs
// and a tank a few), `px` a pixel's footprint in model space.
fn emp_arcs(local: vec3<f32>, reach: f32, px: f32, stun: f32, time: f32, seed: f32) -> vec3<f32> {
    if stun <= 0.0 {
        return vec3<f32>(0.0);
    }
    let cell = clamp(reach * 0.16, 0.8, 24.0);
    let q = local / cell;
    let p = px / cell;
    // Thick with arcs while it holds; thinning out as the systems come back.
    let cover = 0.14 + 0.14 * clamp(stun, 0.0, 1.0);
    let a = emp_arc_layer(q, p, time, seed, 0.0, cover);
    let b = emp_arc_layer(q * 1.7 + 11.0, p * 1.7, time, seed + 0.37, 0.5, cover * 0.8);
    let line = max(a.x, b.x * 0.8);
    let core = max(a.y, b.y);
    let strength = smoothstep(0.0, 0.35, stun);
    return (EMP_ARC * line * 3.5 + EMP_CORE * core * 6.0) * strength;
}
