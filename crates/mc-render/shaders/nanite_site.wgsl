// Nanite construction (`mc_data::Construction::Nanite`, the Regency): how a site
// condenses out of its swarm (entity.wgsl `fs_main`, `fs_shadow`, `fs_prepass`). Put in
// front of shaders that say `//!use nanite_site`. A site forms from the ground up; a
// hull raised aloft (`ModelInfo::nanite_order`, `NANITE_ORDER_KEEL`) along its keel from
// the middle out, and if it falls half built it falls as far as it had formed.

// ---- Nanite construction (`mc_data::Construction::Nanite`, the Regency) --------------
// `status[1]` bit of a Regency site (`mirror::UNIT_NANITE`), and where `model_class` carries it.
const UNIT_NANITE: u32 = 2u;
const CLASS_NANITE: u32 = 0x4000u;
const NANITE_VIOLET: vec3<f32> = vec3<f32>(0.66, 0.12, 1.0);
const NANITE_RED: vec3<f32> = vec3<f32>(1.0, 0.06, 0.1);
// Metres the swarm lies on the hull over the front at most (two fifths of a short hull).
const NANITE_SWARM_SKIN: f32 = 7.0;
// Metres behind the front the fresh band runs, violet then red, before it is plate (two
// fifths of a short hull).
const NANITE_BAND: f32 = 8.0;
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

// The plate under the band while the site is at work, `live` 1 then going to 0 as it
// settles: not finished yet. The seams between the cells it condensed in still glow, a red
// net that pulses up toward the front and fades with depth below it.
fn nanite_veins(color: vec3<f32>, local: vec3<f32>, age: f32, seed: f32, time: f32, live: f32) -> vec3<f32> {
    let p = local.xy * 0.55 + vec2<f32>(local.z * 0.4, seed * 9.0);
    let g = vec2<f32>(value_noise2(p, 1.0), value_noise2(p + vec2<f32>(5.2, 1.3), 1.0));
    let seam = 1.0 - smoothstep(0.0, 0.06, abs(g.x - g.y));
    let pulse = 0.55 + 0.45 * sin(local.z * 0.9 - time * 2.2 + g.x * 4.0);
    let fade = (1.0 - smoothstep(0.0, 18.0, age)) * live;
    return color * (1.0 - 0.3 * live) + NANITE_RED * seam * pulse * fade * 1.4;
}

// A Regency site's colour, and 0 in w where there is nothing there yet. It forms from the
// ground up: what has just condensed is glowing violet, which slowly cools through red
// into the finished plate; a hot line runs along the front and black motes of the swarm
// settle just above it. Above that there is nothing yet (the swarm round the site,
// beams.wgsl, shows where it will stand). The band is a share of a short hull's height and
// at most `NANITE_BAND` metres on a tall one.
fn nanite_site(color: vec3<f32>, local: vec3<f32>, build: f32, at: SiteOrder, seed: f32, time: f32) -> vec4<f32> {
    let grow = nanite_grow(build);
    let order = at.order;
    let settle = smoothstep(0.8, 1.0, build);
    let h = max(at.extent, 1.0);
    if order > grow {
        // The swarm settling onto the hull's shape over the front (the rest of it hangs
        // round the site, beams.wgsl `nanite_swarm_vertex`): black motes drifting down onto
        // the front, packed close there and thinning out above, a few glinting violet.
        let above = (order - grow) * h;
        let band = min(h * 0.4, NANITE_SWARM_SKIN * at.scale);
        let thin = above / max(band, 0.5);
        let cell = floor(local * (5.0 / at.scale) + vec3<f32>(0.0, 0.0, time * 1.5));
        let n = hash21(cell.xy + vec2<f32>(cell.z * 1.7, seed * 13.0));
        if thin > 1.0 || n < 0.45 + 0.55 * thin {
            return vec4<f32>(0.0);
        }
        let glint = step(0.993, n) * (0.5 + 0.5 * sin(time * 7.0 + n * 40.0));
        return vec4<f32>(color * 0.08 + NANITE_VIOLET * glint * 1.5 * (1.0 - settle), 1.0);
    }
    // Metres behind the front, in shares of the band: the band is the same depth on any
    // hull taller than twice it, so on a tall one it is a band and not the whole of what is up.
    let age = (grow - order) * h;
    let band = min(h * 0.4, NANITE_BAND * at.scale);
    let u = age / band;
    // Liquid light: brightness flowing up through the fresh material.
    let flow = value_noise2(local.xy * 0.5 + vec2<f32>(local.z * 0.35 - time * 0.9, seed * 5.0), 1.0);
    let violet = 1.0 - smoothstep(0.1, 0.4, u);
    let red = smoothstep(0.1, 0.4, u) * (1.0 - smoothstep(0.55, 1.0, u));
    let hot = violet + red;
    let glow = NANITE_VIOLET * violet * (0.5 + 0.5 * flow) + NANITE_RED * red * (0.35 + 0.45 * flow);
    let live = 1.0 - settle;
    let plate = nanite_veins(color, local, age, seed, time, live);
    // The fresh band is the glow over dark; the plate shows through as it cools.
    var c = mix(plate, color * 0.25, hot) + glow * (1.0 - settle * 0.85);
    // The front itself: a hot line, bright enough to bloom, with a softer glow under it.
    let edge = exp(-pow(age / (0.3 * at.scale), 2.0));
    let under = exp(-age / max(band * 0.12, 0.4));
    c += mix(NANITE_VIOLET, vec3<f32>(1.0, 0.75, 1.0), 0.4) * (edge * 6.0 + under * 0.3) * live;
    return vec4<f32>(c, 1.0);
}

// `model_class` bit: the model condenses along its keel (`ModelInfo::nanite_order`).
const CLASS_KEEL: u32 = 0x8000u;
// How much bigger the band, the swarm on the hull and the front's hot line are on a hull
// raised aloft than on a site on the ground, against its reach: seen from kilometres off.
const NANITE_KEEL_SCALE: f32 = 0.004;

// Where a point of a site stands in the order it condenses in (0 first, 1 last), the
// metres one whole of that order spans, and how much bigger its band is drawn.
struct SiteOrder {
    order: f32,
    extent: f32,
    scale: f32,
}

// When a point of a hull raised aloft condenses: along its keel from the middle out to
// bow and stern, the plating spreading out from the keel line a little behind, the front
// wavering so it is not a ruled line. `reach` is the hull's half length.
fn keel_order(local: vec3<f32>, reach: f32, seed: f32) -> f32 {
    let r = max(reach, 1.0);
    let waver = value_noise2(local.xy * (6.0 / r) + vec2<f32>(seed * 7.0, seed * 3.0), 1.0) - 0.5;
    return clamp(abs(local.x) / r * 0.88 + abs(local.y) / r * 0.3 + waver * 0.03, 0.0, 1.0);
}

// A site's order at a fragment: up from the ground (`weld.w` its height), or along the
// keel (`weld.z` its reach) for a model with `CLASS_KEEL`.
fn site_order(local: vec3<f32>, weld: vec4<f32>, model_class: u32, seed: f32) -> SiteOrder {
    if (model_class & CLASS_KEEL) != 0u {
        return SiteOrder(keel_order(local, weld.z, seed), weld.z, max(weld.z * NANITE_KEEL_SCALE, 1.0));
    }
    return SiteOrder(nanite_order(local, weld.w, seed), weld.w, 1.0);
}

// Whether a point of a hull that fell half built (a falling wreck drawn at `build`
// under one) had not formed yet: it is not there.
fn keel_unformed(local: vec3<f32>, weld: vec4<f32>, model_class: u32, build: f32, seed: f32) -> bool {
    return (model_class & CLASS_KEEL) != 0u && build < 0.999
        && keel_order(local, weld.z, seed) > nanite_grow(build);
}
