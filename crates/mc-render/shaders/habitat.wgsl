// Where things grow on the ground and how the air moves over it, shared by the
// terrain (which paints the ground from it), the grass (grass.wgsl, which grows
// where the terrain shows grass) and the trees (entity.wgsl, which sway in the
// same wind). build.rs inserts this file after bindings.wgsl into shaders that
// contain the line `//!use habitat`.

// What the ground at a map point is like: the soil fields, wetness and cover
// the terrain's material weights come from, and those weights.
struct Habitat {
    // Ground height, height above the sea, 1 - the normal's z.
    z: f32,
    alt: f32,
    slope: f32,
    // Warped, non-periodic fields: soil patches (46 m), fine patches (13 m),
    // habitats (310 m); the warp they share.
    patchy: f32,
    fine: f32,
    broad: f32,
    warp: vec2<f32>,
    // How much the ground dips below its surroundings (negative on ridges), and
    // that dip alone.
    curvature: f32,
    concavity: f32,
    wet: f32,
    // ground_cover_at: canopy closure, conifer share; canopy eased.
    cover: vec2<f32>,
    canopy: f32,
    sand_w: f32,
    rock_face: f32,
    // ground_snow_at: glacier ice, lying snow, 1 on a map with a snow layer.
    layer: vec3<f32>,
    lying: f32,
    ice_w: f32,
    snow_w: f32,
    // Not under trees and not beach.
    open: f32,
    highland: f32,
    // Open ground's patchwork: lush swards (27 m) and tussocky meadow (61 m).
    sward: f32,
    tussock: f32,
    // Material weights, by terrain material number: 1 leafy grass, 2 meadow,
    // 3 mossy litter, 4 needle floor, 5 scree, 6 dry dirt, 7 high rock, 8 sand,
    // 9 mud. 0 unused (the cliff face is `rock_face`).
    w: array<f32, 10>,
}

// The habitat at `xy`, whose ground stands at `z` with normal `base_n`. `px` is
// the metres a pixel covers there: the tropical beach's tufts fade out as they
// shrink to a pixel.
fn habitat(xy: vec2<f32>, z: f32, base_n: vec3<f32>, px: f32) -> Habitat {
    var h: Habitat;
    let water = globals.map.z;
    h.z = z;
    h.slope = 1.0 - base_n.z;
    h.alt = z - water;
    let alt = h.alt;
    let slope = h.slope;

    // Warped, non-periodic fields: soil patches (tens of metres), habitats
    // (hundreds) and moisture (the better part of a kilometre).
    let warp = vec2<f32>(grad_noise2(xy, 117.0), grad_noise2(xy + 79.0, 103.0)) * 58.0;
    h.warp = warp;
    let patchy = grad_noise2(xy + warp, 46.0);
    let fine = grad_noise2(xy + warp * 0.25, 13.0);
    let broad = grad_noise2(xy + warp * 2.0, 310.0);
    h.patchy = patchy;
    h.fine = fine;
    h.broad = broad;
    let moist_field = grad_noise2(xy + warp * 3.0 + 517.0, 740.0);
    let neighbours = terrain_height(xy + vec2<f32>(24.0, 0.0))
        + terrain_height(xy - vec2<f32>(24.0, 0.0))
        + terrain_height(xy + vec2<f32>(0.0, 24.0))
        + terrain_height(xy - vec2<f32>(0.0, 24.0));
    let curvature = (neighbours * 0.25 - z) / 14.0;
    let concavity = clamp(curvature, 0.0, 0.6);
    h.curvature = curvature;
    h.concavity = concavity;
    // Water gathers in hollows and near the shore; ridges and high ground dry out.
    var wet = clamp(moist_field * 0.75 + curvature * 1.4 + (1.0 - smoothstep(4.0, 40.0, alt)) * 0.35
        - smoothstep(120.0, 320.0, alt) * 0.3, 0.0, 1.0);
    if desert() {
        // Dry country: only the hollows hold a little damp.
        wet = clamp(curvature * 0.8 + (moist_field - 0.5) * 0.2, 0.0, 0.35);
    }
    h.wet = wet;
    let cover = ground_cover_at(xy);
    let canopy = smoothstep(0.08, 0.75, cover.x);
    h.cover = cover;
    h.canopy = canopy;

    var sand_w = 1.0 - smoothstep(2.5, 10.0, alt + (patchy - 0.5) * 6.0);
    if tropical() {
        // The tropics lay a wider beach, and it does not end on a contour: grass
        // runs down it in tongues and tufts and sand shows through the grass
        // above it, over a band several metres of height deep.
        let tongue = grad_noise2(xy + warp * 1.5 - 311.0, 150.0);
        let edge = alt + (patchy - 0.5) * 9.0 + (fine - 0.5) * 5.0 + (tongue - 0.5) * 10.0;
        let band = 1.0 - smoothstep(1.5, 17.0, edge);
        // Tufts a few metres across, faded out as they get down to a pixel.
        let shown = smoothstep(1.0, 3.0, 3.4 / max(px, 0.001));
        let tuft = mix(0.5, grad_noise2(xy + 91.0, 3.4) * 0.65 + grad_noise2(xy - 47.0, 8.5) * 0.35, shown);
        let mixed = sqrt(clamp(band * (1.0 - band) * 4.0, 0.0, 1.0));
        sand_w = clamp(band + (tuft - 0.5) * 2.4 * mixed, 0.0, 1.0);
    }
    if desert() {
        // A narrow beach along the lake (it rises and falls, so no wide strand),
        // and sand drifted into the washes and hollows of gentle ground.
        let beach = 1.0 - smoothstep(0.6, 3.5, alt + (patchy - 0.5) * 2.5 + (fine - 0.5) * 1.2);
        let wash = smoothstep(0.18, 0.55, concavity + (fine - 0.5) * 0.16 + (patchy - 0.5) * 0.14)
            * smoothstep(3.0, 8.0, alt);
        sand_w = max(beach, wash * 0.8) * (1.0 - smoothstep(0.04, 0.12, slope + (fine - 0.5) * 0.03));
    }
    h.sand_w = sand_w;
    let rock_face = smoothstep(0.10, 0.27, slope + (patchy - 0.5) * 0.12);
    h.rock_face = rock_face;
    // Snow on the heights, and whatever the map's snow layer lays: its 16 m
    // samples get a ragged edge from the ground's own patchiness, and it
    // slides off anything steep. Glacier ice from the same layer.
    let layer = ground_snow_at(xy);
    h.layer = layer;
    h.lying = smoothstep(0.3, 0.7, layer.y + (patchy - 0.5) * 0.55 + (fine - 0.5) * 0.25)
        * (1.0 - smoothstep(0.5, 0.85, slope + (fine - 0.5) * 0.15));
    h.ice_w = smoothstep(0.3, 0.7, layer.x + (patchy - 0.5) * 0.35);
    let by_height = smoothstep(350.0, 450.0, alt + (broad - 0.5) * 95.0)
        * (1.0 - smoothstep(0.25, 0.45, slope));
    h.snow_w = mix(by_height, h.lying, layer.z) * (1.0 - h.ice_w);
    if desert() {
        // The canyon's rim is high desert, not snowfield.
        h.snow_w = 0.0;
    }
    let open = (1.0 - sand_w) * (1.0 - canopy);
    let highland = smoothstep(140.0, 300.0, alt + (broad - 0.5) * 140.0);
    h.open = open;
    h.highland = highland;

    // Open ground is a patchwork a few tens of metres across: swards of lush
    // grass, tussocky meadow and mossy ground, as seen from above.
    let sward = grad_noise2(xy + warp * 0.6 + 211.0, 27.0);
    let tussock = grad_noise2(xy - warp * 0.4 - 97.0, 61.0);
    h.sward = sward;
    h.tussock = tussock;

    h.w[0] = 0.0;
    // Lush grass in the damp lowlands, meadow with outcrops where it is drier.
    h.w[1] = open * smoothstep(0.30, 0.70, wet + (fine - 0.5) * 0.3) * (1.0 - highland * 0.7) * (0.4 + sward * 1.2);
    h.w[2] = open * (0.35 + smoothstep(0.35, 0.75, broad) * 0.8) * (1.0 - smoothstep(0.45, 0.80, wet))
        * (1.6 - sward) * (0.6 + tussock * 0.8);
    // Mossy litter: under broadleaf canopy, along forest edges and in damp swards.
    h.w[3] = (1.0 - sand_w) * (canopy * (1.0 - cover.y) * 1.3
        + smoothstep(0.02, 0.3, cover.x) * (1.0 - canopy) * 0.4)
        + open * smoothstep(0.62, 0.8, tussock) * smoothstep(0.3, 0.6, wet) * 0.9;
    h.w[4] = (1.0 - sand_w) * canopy * cover.y * 1.3;
    // Scree skirts the cliffs; stony ground covers the heights.
    h.w[5] = (1.0 - sand_w) * (smoothstep(0.05, 0.16, slope) * (1.0 - rock_face) * 1.4
        + highland * smoothstep(0.55, 0.75, patchy) * 0.8);
    // Bare dry dirt on convex, dry patches.
    h.w[6] = open * smoothstep(0.58, 0.78, patchy + (0.5 - wet) * 0.35 - curvature * 0.8)
        * (1.0 - highland * 0.5) * 1.3;
    h.w[7] = (1.0 - sand_w) * (highland * (0.6 + smoothstep(0.03, 0.12, slope))
        + smoothstep(0.78, 0.9, broad * 0.6 + patchy * 0.5) * 0.9) * (1.0 - canopy);
    h.w[8] = sand_w * 2.0;
    h.w[9] = (1.0 - sand_w) * smoothstep(0.55, 0.85, wet + concavity * 0.6) * (1.0 - smoothstep(0.08, 0.2, slope)) * 1.2;
    if desert() {
        // Nothing lush: dry dirt, talus, slickrock and sand (desert.wgsl colours them).
        let ground = 1.0 - sand_w;
        h.w[1] = 0.0;
        h.w[2] = 0.0;
        h.w[3] = 0.0;
        h.w[4] = 0.0;
        h.w[9] = 0.0;
        h.w[5] = ground * (smoothstep(0.03, 0.12, slope) * (1.0 - rock_face) * 1.5
            + smoothstep(0.62, 0.8, patchy) * 0.35);
        h.w[6] = ground * (0.75 + canopy * 0.5) * (1.0 - smoothstep(0.05, 0.14, slope) * 0.6);
        // Slickrock: the bed's own rock bare on convex ground and in broad patches.
        h.w[7] = ground * (smoothstep(0.6, 0.85, broad * 0.55 + patchy * 0.5 - curvature * 0.6)
            + smoothstep(0.04, 0.1, slope) * 0.3) * (1.0 - canopy);
    }
    return h;
}

// The broad light and dark over open ground: one field of grass reads greener
// here, sun-bleached there. `green_part` is the grassy materials' share. The
// terrain multiplies its scans by it, and the grass its blades, so the two agree.
fn ground_tone(h: Habitat, green_part: f32) -> vec3<f32> {
    var tint = mix(vec3<f32>(1.07, 1.0, 0.84), vec3<f32>(0.90, 1.04, 0.94), h.wet);
    if tropical() {
        // Tropical green does not bleach to straw where it is dry.
        tint = mix(vec3<f32>(1.02, 1.03, 0.88), vec3<f32>(0.92, 1.05, 0.96), h.wet);
    }
    return mix(vec3<f32>(1.0), tint, green_part * 0.7) * (0.78 + h.broad * 0.28 + h.patchy * 0.16 + h.fine * 0.1);
}

// ---- The air near the ground ----------------------------------------------

// How sheltered `p` is by a live shield: 1 inside a dome or hull field, easing
// to 0 across its last few metres. Air under a shield is still.
fn shield_shelter(p: vec3<f32>) -> f32 {
    var s = 0.0;
    for (var i = 0u; i < effect_barriers.header.x; i++) {
        let b = effect_barriers.entries[i];
        if p.z < b.min_z - 0.1 { continue; }
        // Below the rim the barrier is a wall: only the distance across counts.
        var d = (p - b.center) * b.inverse_axes;
        d.z = max(d.z, 0.0);
        let q = length(d);
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

// The wind over a field and the waves it drives through the grass: xy the air
// (m/s, ground_air), z how deep in a wave this spot stands (0 none, 1 a crest,
// where the grass is pressed flattest and shows its pale side), w the gusts.
// Crests run downwind a little slower than the air, a dozen metres apart and
// broken into patches; a storm sets them longer and harder and lets the gusts
// run through as swathes. The grass leans with it (grass_gen.wgsl) and the
// ground under far-off grass is shaded with it (terrain.wgsl), so the waves
// carry on across a field past where blades are drawn.
fn grass_wave(xy: vec2<f32>) -> vec4<f32> {
    let air = ground_air(xy);
    let speed = length(air);
    if speed < 0.05 {
        return vec4<f32>(air, 0.0, 0.0);
    }
    let dir = air / speed;
    let gust = gust_at(xy);
    let storm = clamp(weather_at(xy).y, 0.0, 1.0);
    let p = xy - atmos.wind.xy * 0.85;
    let strong = textureSampleLevel(noise_map, repeat_sampler, tile_uv(p, 41.0), 0.0).b;
    let k = mix(0.42, 0.28, storm);
    // Crests bow a little across the wind, so they are not ruled lines.
    let across = dot(p, vec2<f32>(-dir.y, dir.x));
    let band = sin(dot(p, dir) * k + strong * 5.0 + sin(across * 0.045) * 2.0) * 0.5 + 0.5;
    // A second train, turned off the wind and set wider apart: where the two
    // meet the crests break up and gather, as real ones do.
    let turned = vec2<f32>(dir.x * 0.9 - dir.y * 0.44, dir.x * 0.44 + dir.y * 0.9);
    let band2 = sin(dot(p, turned) * k * 0.73 + strong * 3.0 + 1.7) * 0.5 + 0.5;
    // Crests are narrow gusts running through, not half the field.
    var wave = smoothstep(0.45, 0.95, (band * 0.62 + band2 * 0.38) * (0.35 + strong * 1.05));
    // Whole swathes of the field run strong while others lie almost still:
    // patches of a hundred metres, riding with the air.
    let swathe = textureSampleLevel(noise_map, repeat_sampler, tile_uv(p * 0.93 + 37.0, 97.0), 0.0).r;
    wave = min(wave * mix(0.45, 1.35, smoothstep(0.25, 0.7, swathe)) * (0.75 + 0.5 * gust), 1.0);
    wave = mix(wave, max(wave, smoothstep(0.2, 0.9, band * gust)), storm);
    // A storm's waves run hard enough to read from any height.
    return vec4<f32>(air, min(wave * smoothstep(1.5, 7.0, speed) * (1.0 + storm * 0.8), 1.0), gust);
}

// ---- Grass (renderer/grass.rs) --------------------------------------------

// One tuft of grass blades, written by grass_gen.wgsl `cs_tufts` each frame and
// drawn by grass.wgsl. Mirrors grass::TUFT_BYTES (48).
struct Tuft {
    // The tuft's foot on the ground.
    pos: vec3<f32>,
    // Tallest blade (metres) | how far round the foot the blades rise (metres), f16 each.
    size: u32,
    // Where the air, the units and the blasts push the blade tips, as a share
    // of their height (f16 x, y).
    lean: u32,
    // Blade width scale | how hard the blades shiver (0-1), f16 each.
    blade: u32,
    // Seed (16 bits) | kind << 16 (4 bits) | charred << 24 (8 bits, 0-255).
    look: u32,
    // The ground's broad tone under it (ground_tone), rgb * 127 | dryness << 24.
    tone: u32,
    // Sunlight past the hills and the clouds | sky seen | flattened, 8 bits each.
    light: u32,
    // The ground's normal xy (f16 each).
    ground: u32,
    // How deep in a wind wave it stands (0 none, 1 a crest), as grass_wave gives it.
    sheen: f32,
    spare: u32,
}

// Kinds of grass (`Tuft::look`): lush sward, tall meadow, short moss and turf,
// tall tropical grass, wiry highland grass.
const GRASS_LUSH: u32 = 0u;
const GRASS_MEADOW: u32 = 1u;
const GRASS_MOSS: u32 = 2u;
const GRASS_TROPICAL: u32 = 3u;
const GRASS_HIGHLAND: u32 = 4u;
// Canyon country's sparse dry bunchgrass, and its shrubs (desert.wgsl) seen up
// close (`desert()`).
const GRASS_DESERT: u32 = 5u;
const GRASS_SHRUB: u32 = 6u;

// Blades per tuft and segments per blade in each detail band, near to far
// (gpu_consts.rs `grass`).
const GRASS_BAND_BLADES: array<u32, 3> = array<u32, 3>(GRASS_NEAR_BLADES, GRASS_MID_BLADES, GRASS_FAR_BLADES);
const GRASS_BAND_SEGMENTS: array<u32, 3> = array<u32, 3>(GRASS_NEAR_SEGMENTS, GRASS_MID_SEGMENTS, GRASS_FAR_SEGMENTS);
// Each band's first slot in the tuft buffer, and how many it holds.
const GRASS_BAND_FIRST: array<u32, 3> = array<u32, 3>(0u, GRASS_NEAR_CAP, GRASS_NEAR_CAP + GRASS_MID_CAP);
const GRASS_BAND_CAP: array<u32, 3> = array<u32, 3>(GRASS_NEAR_CAP, GRASS_MID_CAP, GRASS_FAR_CAP);

// How much of the ground grows grass (x, 0-1) and of which kind: lush sward (y),
// meadow (z), moss and turf (w), as material weights. Where the terrain shows
// grass, not beach, rock, snow or forest floor.
fn grass_share(h: Habitat) -> vec4<f32> {
    var ws = h.w;
    var sum = 0.0;
    for (var i = 1; i < 10; i++) {
        sum += ws[i];
    }
    if desert() {
        // Scattered bunchgrass clumps between the shrubs on the bench and the rim,
        // never a field: none in the old lake bed, on sand or on anything steep.
        let benches = smoothstep(58.0, 70.0, h.alt);
        let d = 0.05 * h.open * benches * (1.0 - h.rock_face) * (1.0 - smoothstep(0.05, 0.12, h.slope))
            * (0.3 + 1.2 * smoothstep(0.35, 0.75, h.tussock));
        return vec4<f32>(d, 0.0, 1.0, 0.0);
    }
    let lush = ws[1];
    let meadow = ws[2];
    // Moss and turf: the open ground's share of the mossy litter, not the forest floor's.
    let moss = max(ws[3] - (1.0 - h.sand_w) * h.canopy * (1.0 - h.cover.y) * 1.3, 0.0);
    let share = (lush + meadow + moss * 0.6) / max(sum, 1e-3);
    let density = smoothstep(0.22, 0.62, share)
        * (1.0 - h.rock_face)
        * (1.0 - max(h.snow_w, h.ice_w))
        * (1.0 - smoothstep(0.14, 0.32, h.slope))
        * smoothstep(0.3, 1.4, h.alt)
        * (1.0 - h.canopy * 0.92)
        * (1.0 - h.sand_w);
    return vec4<f32>(density, lush, meadow, moss);
}

// How much of the grass is drawn at `world` (1 all of it, 0 none): it thins
// out as a cell shrinks toward GRASS_MIN_PX on screen and fades toward the
// edge of its reach (Globals::climate.z), as grass_gen.wgsl `cs_tufts` does.
fn grass_drawn(world: vec3<f32>) -> f32 {
    if globals.climate.y < 0.5 {
        return 0.0;
    }
    let eye = globals.camera.xyz;
    let px = GRASS_CELL_M * globals.lod.x / max(distance(world, eye), 1.0);
    let reach = globals.climate.z;
    return smoothstep(GRASS_MIN_PX, GRASS_MIN_PX * 2.2, px)
        * (1.0 - smoothstep(reach * 0.8, reach, distance(world.xy, eye.xy)));
}

// Blade colours at the foot, halfway and at the tip, before the ground's tone.
struct GrassColours {
    foot: vec3<f32>,
    mid: vec3<f32>,
    tip: vec3<f32>,
}

fn grass_colours(kind: u32, dry: f32) -> GrassColours {
    var c: GrassColours;
    switch kind {
        case GRASS_LUSH: {
            c.foot = vec3<f32>(0.032, 0.052, 0.016);
            c.mid = vec3<f32>(0.070, 0.112, 0.030);
            c.tip = mix(vec3<f32>(0.100, 0.140, 0.042), vec3<f32>(0.135, 0.140, 0.050), dry * 0.5);
        }
        case GRASS_MEADOW: {
            // Green at the foot, going to straw and gold up the blade where it is dry.
            let d = clamp(dry * 1.1 + 0.4, 0.0, 1.0);
            c.foot = vec3<f32>(0.042, 0.052, 0.017);
            c.mid = mix(vec3<f32>(0.082, 0.100, 0.030), vec3<f32>(0.135, 0.120, 0.045), d);
            c.tip = mix(vec3<f32>(0.140, 0.140, 0.052), vec3<f32>(0.225, 0.185, 0.085), d);
        }
        case GRASS_TROPICAL: {
            c.foot = vec3<f32>(0.025, 0.060, 0.014);
            c.mid = vec3<f32>(0.060, 0.150, 0.030);
            c.tip = vec3<f32>(0.110, 0.190, 0.045);
        }
        case GRASS_SHRUB: {
            // Blackbrush and sage: dark grey-green twigs, paler leaf tips.
            c.foot = vec3<f32>(0.02, 0.021, 0.016);
            c.mid = vec3<f32>(0.055, 0.058, 0.042);
            c.tip = mix(vec3<f32>(0.10, 0.11, 0.08), vec3<f32>(0.13, 0.13, 0.095), dry);
        }
        case GRASS_DESERT: {
            // Sun-cured bunchgrass: grey-straw, pale at the tips.
            c.foot = vec3<f32>(0.06, 0.052, 0.034);
            c.mid = mix(vec3<f32>(0.15, 0.13, 0.085), vec3<f32>(0.19, 0.155, 0.095), dry);
            c.tip = mix(vec3<f32>(0.25, 0.215, 0.14), vec3<f32>(0.31, 0.26, 0.17), dry);
        }
        case GRASS_HIGHLAND: {
            c.foot = vec3<f32>(0.055, 0.055, 0.028);
            c.mid = vec3<f32>(0.115, 0.105, 0.050);
            c.tip = vec3<f32>(0.175, 0.150, 0.080);
        }
        default: {
            c.foot = vec3<f32>(0.040, 0.060, 0.022);
            c.mid = vec3<f32>(0.075, 0.100, 0.032);
            c.tip = vec3<f32>(0.095, 0.120, 0.040);
        }
    }
    return c;
}

// The colour a field of grass reads as from a distance, before the ground's
// tone: its blades' middles and tips (`tip` of it the tips, more of them seen
// from low down), mixed by kind as `grass_share` gives them. The terrain takes it on where grass grows, so the ground seen between
// far-off blades, and beyond where they are drawn, is the grass's own colour.
fn grass_mass(g: vec4<f32>, h: Habitat, tip: f32) -> vec3<f32> {
    let dry = grass_dryness(h);
    var lush = grass_colours(GRASS_LUSH, dry);
    var meadow = grass_colours(GRASS_MEADOW, dry);
    if tropical() {
        lush = grass_colours(GRASS_TROPICAL, dry);
        meadow = lush;
    } else if desert() {
        lush = grass_colours(GRASS_DESERT, dry);
        meadow = lush;
    } else {
        // Highland grass takes over the heights, as `cs_tufts` picks it.
        let high = smoothstep(0.5, 0.9, h.highland);
        let wiry = grass_colours(GRASS_HIGHLAND, dry);
        lush.mid = mix(lush.mid, wiry.mid, high);
        lush.tip = mix(lush.tip, wiry.tip, high);
        meadow.mid = mix(meadow.mid, wiry.mid, high);
        meadow.tip = mix(meadow.tip, wiry.tip, high);
    }
    let moss = grass_colours(GRASS_MOSS, dry);
    let total = max(g.y + g.z + g.w, 1e-4);
    return (mix(lush.mid, lush.tip, tip) * g.y + mix(meadow.mid, meadow.tip, tip) * g.z
        + mix(moss.mid, moss.tip, tip) * g.w) / total;
}

// How sun-dried the grass is (0 lush, 1 straw): dry ground and dry habitats.
fn grass_dryness(h: Habitat) -> f32 {
    return clamp(1.0 - h.wet * 1.3 + (h.broad - 0.5) * 0.6, 0.0, 1.0);
}

// A well-mixed integer hash (PCG), for per-tuft and per-blade randoms that are
// stable wherever the tuft is in the world.
fn veg_hash(v: u32) -> u32 {
    let s = v * 747796405u + 2891336453u;
    let w = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u;
    return (w >> 22u) ^ w;
}

fn veg_rand(v: u32) -> f32 {
    return f32(veg_hash(v) >> 8u) / 16777216.0;
}
