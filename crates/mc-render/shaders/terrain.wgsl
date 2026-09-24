//!use bindings
// Terrain: CDLOD quadtree patches over a streamed heightmap.
//
// Every node draws the same 64x64 grid. At the finest level the grid vertices
// sit exactly on height samples and each cell is split along the (x,y)-(x+1,y+1)
// diagonal, which is the triangulation the simulation collides with.

struct Node {
    // xy origin, z size in metres, w level
    rect: vec4<f32>,
    // x morph start distance, y morph end distance
    morph: vec4<f32>,
}

struct TerrainPush {
    // 1: shadow pass (use the shadow matrix)
    pass_kind: u32,
    // 1: draw the build grid overlay
    build_grid: u32,
}

@group(1) @binding(0) var<storage, read> nodes: array<Node>;
var<immediate> push: TerrainPush;

const GRID: f32 = 64.0;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
}

@vertex
fn vs_main(@location(0) grid: vec2<f32>, @builtin(instance_index) instance: u32) -> VsOut {
    let node = nodes[instance];
    let size = node.rect.z;
    var xy = node.rect.xy + grid * (size / GRID);

    // CDLOD morph: slide odd vertices onto their even neighbours as the node
    // approaches the distance where its parent takes over.
    let eye = globals.camera.xyz;
    let approx = vec3<f32>(xy, terrain_height(xy));
    let dist = distance(approx, eye);
    let k = clamp((dist - node.morph.x) / max(node.morph.y - node.morph.x, 1.0), 0.0, 1.0);
    let odd = fract(grid * 0.5) * 2.0;
    xy = xy - odd * (size / GRID) * k;
    xy = clamp(xy, vec2<f32>(0.0), globals.map.xy);

    let world = vec3<f32>(xy, terrain_height(xy));
    var out: VsOut;
    if push.pass_kind == 1u {
        out.clip = globals.shadow_view_proj * vec4<f32>(world, 1.0);
    } else {
        out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    }
    out.world = world;
    return out;
}

@fragment
fn fs_shadow() {
}

// Gradient noise with its derivative: (value in [0, 1], d/dx, d/dy per metre).
// Same lattice and hash as grad_noise2, so the two can be mixed.
fn grad_noise2_d(xy: vec2<f32>, cell: f32) -> vec3<f32> {
    let p = noise_lattice(xy, cell);
    let i = floor(p);
    let f = p - i;
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let du = 30.0 * f * f * (f * (f - 2.0) + 1.0);
    let g00 = grad_vec(i);
    let g10 = grad_vec(i + vec2<f32>(1.0, 0.0));
    let g01 = grad_vec(i + vec2<f32>(0.0, 1.0));
    let g11 = grad_vec(i + vec2<f32>(1.0, 1.0));
    let v00 = dot(g00, f);
    let v10 = dot(g10, f - vec2<f32>(1.0, 0.0));
    let v01 = dot(g01, f - vec2<f32>(0.0, 1.0));
    let v11 = dot(g11, f - vec2<f32>(1.0, 1.0));
    let v = v00 + u.x * (v10 - v00) + u.y * (v01 - v00) + u.x * u.y * (v00 - v10 - v01 + v11);
    let d = g00 + u.x * (g10 - g00) + u.y * (g01 - g00) + u.x * u.y * (g00 - g10 - g01 + g11)
        + du * vec2<f32>(u.y * (v00 - v10 - v01 + v11) + (v10 - v00), u.x * (v00 - v10 - v01 + v11) + (v01 - v00));
    let k = 0.70710678 * 0.5;
    return vec3<f32>(clamp(v * k + 0.5, 0.0, 1.0), d * (k / cell));
}

// Real-world repeat of each ground scan in metres, from Poly Haven's
// dimensions, stretched a little where the true size would be sub-pixel from
// a battle camera. Indexed by material number (colour layer / 2).
fn ground_period(m: i32) -> f32 {
    switch m {
        case 0: { return 17.0; }  // rock face
        case 1: { return 2.2; }   // leafy grass
        case 2: { return 15.0; }  // aerial meadow
        case 3: { return 3.4; }   // mossy leaf-littered grass
        case 4: { return 2.6; }   // needle forest floor
        case 5: { return 3.6; }   // scree
        case 6: { return 5.0; }   // dry dirt
        case 7: { return 38.0; }  // aerial rocky highland
        case 8: { return 3.2; }   // sand and gravel
        default: { return 2.8; }  // mud
    }
}

// Scanned relief depth in metres, for the parallax trace.
fn ground_relief(m: i32) -> f32 {
    switch m {
        case 0: { return 0.60; }
        case 2: { return 0.30; }
        case 5: { return 0.12; }
        case 6: { return 0.10; }
        case 7: { return 1.40; }
        default: { return 0.06; }
    }
}

// Brings each scan to one coherent palette under this sun and exposure: raw
// photo albedo is two to three times brighter than the scene is lit for, and
// each was shot in different light. Mostly per channel, part luminance only,
// so the stones in a grass scan do not all turn green.
fn ground_tint(m: i32) -> vec3<f32> {
    switch m {
        case 1: { return vec3<f32>(0.231, 0.375, 0.274); }  // leafy grass
        case 2: { return vec3<f32>(0.552, 0.719, 0.980); }  // aerial meadow
        case 3: { return vec3<f32>(0.283, 0.406, 0.499); }  // mossy leaf litter
        case 4: { return vec3<f32>(0.196, 0.217, 0.242); }  // needle forest floor
        case 5: { return vec3<f32>(0.407, 0.468, 0.557); }  // scree
        case 6: { return vec3<f32>(0.405, 0.468, 0.579); }  // dry dirt
        case 7: { return vec3<f32>(0.633, 0.775, 1.392); }  // aerial rocky highland
        case 8: { return vec3<f32>(0.876, 1.152, 1.551); }  // sand and gravel
        default: { return vec3<f32>(0.465, 0.485, 0.746); } // mud
    }
}

// A scan's colour under this scene's palette. The aerial meadow covers most
// open ground, and its grey gravel patches came out pale blue-white under the
// tint: a bright blot repeating every 15 m, a grid of them from a battle
// camera. Those are pulled to the meadow's own olive, and its light and dark
// narrowed, so the scan gives texture without spots.
fn ground_albedo(p: TerrainPatch, m: i32) -> vec3<f32> {
    let rgb = p.color.rgb * ground_tint(m);
    if m != 2 {
        return rgb;
    }
    let lum = max(dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722)), 1e-5);
    let hi = max(rgb.r, max(rgb.g, rgb.b));
    let sat = (hi - min(rgb.r, min(rgb.g, rgb.b))) / max(hi, 1e-5);
    // The tinted scan's median brightness and mean hue.
    let median = 0.0813;
    let olive = vec3<f32>(1.110, 1.037, 0.314);
    let pale = 1.0 - smoothstep(0.58, 0.78, sat);
    let hue = mix(rgb / lum, olive, pale * 0.85);
    return hue * median * pow(lum / median, 0.55);
}

// One ground material seen from above: three rotated patches of the scan
// (terrain_projection), so no tile repeats. `d` is the pixel footprint
// derivative of world xy.
fn ground_material(xy: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>, m: i32, ray: vec2<f32>, fade: f32) -> TerrainPatch {
    let period = ground_period(m);
    let relief = ground_relief(m) / period * fade;
    return terrain_projection(xy / period, dx / period, dy / period, m * 2, ray, relief);
}

struct Stones {
    // How much of the pixel is stone, and the stone's surface gradient (dz/dx, dz/dy).
    cover: f32,
    grad: vec2<f32>,
    // Contact shadow in the soil around each stone, 1 = none.
    ao: f32,
    tone: f32,
}

// Angular stones on a jittered grid: one candidate per `cell` metres, kept
// with probability `density`, each fully inside its cell so only one cell is
// ever evaluated. A stone is a jittered convex polygon: a flat, weathered top
// and planar faces sloping down to the soil, so each side catches the sun
// differently the way broken rock does. `px` is the pixel footprint in metres.
fn stones(xy: vec2<f32>, cell: f32, density: f32, px: f32, seed: f32) -> Stones {
    var out: Stones;
    out.ao = 1.0;
    out.tone = 0.5;
    let p = xy / cell;
    let id = floor(p);
    let h = hash21(id + seed);
    if h > density {
        return out;
    }
    let f = p - id;
    let r = 0.08 + 0.22 * pow(hash21(id + seed + 17.3), 1.6);
    let c = vec2<f32>(hash21(id + seed + 3.1), hash21(id + seed + 7.7)) * (1.0 - 3.2 * r) + 1.6 * r;
    let rel = (f - c) / r;
    // Six faces round the stone, unevenly spaced and set back.
    var d = -1.0;
    var face = vec2<f32>(0.0);
    let turn = hash21(id + seed + 11.9) * 6.2831853;
    for (var i = 0; i < 6; i++) {
        let fi = f32(i);
        let angle = turn + fi * 1.0471976 + (hash21(id + seed + fi * 3.7) - 0.5) * 0.8;
        let dir = vec2<f32>(cos(angle), sin(angle));
        let setback = 0.7 + 0.45 * hash21(id + seed + fi * 5.3 + 1.1);
        let di = dot(rel, dir) / setback;
        if di > d {
            d = di;
            face = dir / setback;
        }
    }
    let width = px / (cell * r) * 1.5;
    out.cover = 1.0 - smoothstep(1.0 - width, 1.0 + width, d);
    // Flat top inside half the radius, then the face slopes down to the soil.
    let side = smoothstep(0.35, 0.6, d);
    let steepness = 0.9 + 0.8 * hash21(id + seed + 29.3);
    out.grad = -face * steepness * side * out.cover;
    out.ao = 1.0 - 0.3 * (1.0 - smoothstep(1.0, 1.35, d)) * (1.0 - out.cover);
    out.tone = hash21(id + seed + 23.1);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let xy = in.world.xy;
    // Classify beaches and submerged ground from the same heightfield as
    // the ocean, not interpolated coarse-triangle height. Otherwise LOD
    // changes move the sand band independently of the true coastline.
    let z = terrain_height(xy);
    let water = globals.map.z;
    let eye = globals.camera.xyz;
    let dist = distance(in.world, eye);
    // Texture derivatives come first: everything after may branch.
    let dpx = dpdx(in.world);
    let dpy = dpdy(in.world);
    let dx = dpx.xy;
    let dy = dpy.xy;
    let px = max(length(dx), length(dy));

    // Geometric normal from the heightmap; finer step up close.
    let step = clamp(dist * 0.004, 4.0, 48.0);
    let base_n = terrain_normal(xy, step);
    let slope = 1.0 - base_n.z;
    let alt = z - water;

    // ---- Where things grow -------------------------------------------------
    // Warped, non-periodic fields: soil patches (tens of metres), habitats
    // (hundreds) and moisture (the better part of a kilometre).
    let warp = vec2<f32>(grad_noise2(xy, 117.0), grad_noise2(xy + 79.0, 103.0)) * 58.0;
    let patchy = grad_noise2(xy + warp, 46.0);
    let fine = grad_noise2(xy + warp * 0.25, 13.0);
    let broad = grad_noise2(xy + warp * 2.0, 310.0);
    let moist_field = grad_noise2(xy + warp * 3.0 + 517.0, 740.0);
    let neighbours = terrain_height(xy + vec2<f32>(24.0, 0.0))
        + terrain_height(xy - vec2<f32>(24.0, 0.0))
        + terrain_height(xy + vec2<f32>(0.0, 24.0))
        + terrain_height(xy - vec2<f32>(0.0, 24.0));
    let curvature = (neighbours * 0.25 - z) / 14.0;
    let concavity = clamp(curvature, 0.0, 0.6);
    // Water gathers in hollows and near the shore; ridges and high ground dry out.
    let wet = clamp(moist_field * 0.75 + curvature * 1.4 + (1.0 - smoothstep(4.0, 40.0, alt)) * 0.35
        - smoothstep(120.0, 320.0, alt) * 0.3, 0.0, 1.0);
    let cover = ground_cover_at(xy);
    let canopy = smoothstep(0.08, 0.75, cover.x);

    let sand_w = 1.0 - smoothstep(2.5, 10.0, alt + (patchy - 0.5) * 6.0);
    let rock_face = smoothstep(0.10, 0.27, slope + (patchy - 0.5) * 0.12);
    // Snow on the heights, and whatever the map's snow layer lays: its 16 m
    // samples get a ragged edge from the ground's own patchiness, and it
    // slides off anything steep. Glacier ice from the same layer.
    let layer = ground_snow_at(xy);
    let lying = smoothstep(0.3, 0.7, layer.y + (patchy - 0.5) * 0.55 + (fine - 0.5) * 0.25)
        * (1.0 - smoothstep(0.5, 0.85, slope + (fine - 0.5) * 0.15));
    let ice_w = smoothstep(0.3, 0.7, layer.x + (patchy - 0.5) * 0.35);
    let by_height = smoothstep(350.0, 450.0, alt + (broad - 0.5) * 95.0)
        * (1.0 - smoothstep(0.25, 0.45, slope));
    let snow_w = mix(by_height, lying, layer.z) * (1.0 - ice_w);
    let open = (1.0 - sand_w) * (1.0 - canopy);
    let highland = smoothstep(140.0, 300.0, alt + (broad - 0.5) * 140.0);

    // Open ground is a patchwork a few tens of metres across: swards of lush
    // grass, tussocky meadow and mossy ground, as seen from above.
    let sward = grad_noise2(xy + warp * 0.6 + 211.0, 27.0);
    let tussock = grad_noise2(xy - warp * 0.4 - 97.0, 61.0);

    var w: array<f32, 10>;
    w[0] = 0.0;
    // Lush grass in the damp lowlands, meadow with outcrops where it is drier.
    w[1] = open * smoothstep(0.30, 0.70, wet + (fine - 0.5) * 0.3) * (1.0 - highland * 0.7) * (0.4 + sward * 1.2);
    w[2] = open * (0.35 + smoothstep(0.35, 0.75, broad) * 0.8) * (1.0 - smoothstep(0.45, 0.80, wet))
        * (1.6 - sward) * (0.6 + tussock * 0.8);
    // Mossy litter: under broadleaf canopy, along forest edges and in damp swards.
    w[3] = (1.0 - sand_w) * (canopy * (1.0 - cover.y) * 1.3
        + smoothstep(0.02, 0.3, cover.x) * (1.0 - canopy) * 0.4)
        + open * smoothstep(0.62, 0.8, tussock) * smoothstep(0.3, 0.6, wet) * 0.9;
    w[4] = (1.0 - sand_w) * canopy * cover.y * 1.3;
    // Scree skirts the cliffs; stony ground covers the heights.
    w[5] = (1.0 - sand_w) * (smoothstep(0.05, 0.16, slope) * (1.0 - rock_face) * 1.4
        + highland * smoothstep(0.55, 0.75, patchy) * 0.8);
    // Bare dry dirt on convex, dry patches.
    w[6] = open * smoothstep(0.58, 0.78, patchy + (0.5 - wet) * 0.35 - curvature * 0.8)
        * (1.0 - highland * 0.5) * 1.3;
    w[7] = (1.0 - sand_w) * (highland * (0.6 + smoothstep(0.03, 0.12, slope))
        + smoothstep(0.78, 0.9, broad * 0.6 + patchy * 0.5) * 0.9) * (1.0 - canopy);
    w[8] = sand_w * 2.0;
    w[9] = (1.0 - sand_w) * smoothstep(0.55, 0.85, wet + concavity * 0.6) * (1.0 - smoothstep(0.08, 0.2, slope)) * 1.2;

    // The three strongest materials, less the fourth: a material fades out
    // before it is dropped, so the choice never shows as a seam.
    var a = 1;
    var b = 1;
    var c = 1;
    var wa = -1.0;
    var wb = -1.0;
    var wc = -1.0;
    var wd = 0.0;
    for (var i = 1; i < 10; i++) {
        let v = w[i];
        if v > wa {
            wd = wc; c = b; wc = wb; b = a; wb = wa; a = i; wa = v;
        } else if v > wb {
            wd = wc; c = b; wc = wb; b = i; wb = v;
        } else if v > wc {
            wd = wc; c = i; wc = v;
        } else if v > wd {
            wd = v;
        }
    }
    var lw = max(vec3<f32>(wa, wb, wc) - wd, vec3<f32>(0.0));
    lw /= max(lw.x + lw.y + lw.z, 1e-4);

    let view = normalize(eye - in.world);
    let tangent_view = view - base_n * dot(view, base_n);
    let ray = (tangent_view / max(dot(view, base_n), 0.28)).xy;
    let near = 1.0 - smoothstep(65.0, 220.0, dist);
    let ga = ground_material(xy, dx, dy, a, ray, near);
    var gb = ga;
    var gc = ga;
    if lw.y > 0.004 {
        gb = ground_material(xy, dx, dy, b, ray, 0.0);
    }
    if lw.z > 0.004 {
        gc = ground_material(xy, dx, dy, c, ray, 0.0);
    }
    // Height-aware blend: stones and tufts of one material poke through the
    // other instead of the two dissolving into each other.
    let hs = vec3<f32>(ga.height, gb.height, gc.height);
    let score = lw + hs * 0.45 * min(lw * 4.0, vec3<f32>(1.0));
    let top = max(score.x, max(score.y, score.z));
    var bw = max(score - (top - 0.22), vec3<f32>(0.0)) * select(vec3<f32>(0.0), vec3<f32>(1.0), lw > vec3<f32>(0.0001));
    bw /= max(bw.x + bw.y + bw.z, 1e-4);
    let ground_color = ground_albedo(ga, a) * bw.x + ground_albedo(gb, b) * bw.y + ground_albedo(gc, c) * bw.z;
    let ground_n = ga.normal * bw.x + gb.normal * bw.y + gc.normal * bw.z;
    let ground_h = dot(hs, bw);

    // Cliffs keep the triplanar rock face: it wraps round vertical faces.
    var cliff: SurfaceDetail;
    cliff.color = vec3<f32>(0.2);
    cliff.normal = base_n;
    cliff.roughness = 0.85;
    cliff.ao = 1.0;
    cliff.height = 0.5;
    // Cliffs are shaded from a wider normal: the 8 m heightfield steps down a
    // cliff in facets, and the fine normal fluted them into organ pipes.
    var cliff_n = base_n;
    if rock_face > 0.004 {
        cliff_n = normalize(mix(base_n, terrain_normal(xy, max(step * 3.0, 20.0)), 0.8));
        cliff = terrain_surface_grad(in.world, cliff_n, 17.0, MAT_ROCK_FACE, 1.25, dpx, dpy);
    }
    let rock_w = clamp(rock_face + (cliff.height - ground_h) * 0.35 * rock_face * (1.0 - rock_face) * 4.0, 0.0, 1.0);

    // ---- Relief below the 8 m heightfield -----------------------------------
    // Hummocks, ruts and small rises, rougher on stony ground, smoother in
    // meadows, mud and sand. Octaves finer than a few pixels fade out.
    let rough_ground = 0.55 + w[5] * 0.8 + w[7] * 0.9 + canopy * 0.5 - sand_w * 0.35 - w[9] * 0.3;
    let o1 = grad_noise2_d(xy + warp * 0.5, 34.0);
    let o2 = grad_noise2_d(xy + 31.0, 9.5);
    let o3 = grad_noise2_d(xy - 57.0, 2.8);
    var relief = o1.yz * 2.2 + o2.yz * 0.55 * (1.0 - smoothstep(0.8, 3.0, px))
        + o3.yz * 0.12 * (1.0 - smoothstep(0.25, 0.9, px));
    relief *= max(rough_ground, 0.15);

    // Boulders you can pick out from a battle camera, where the ground is
    // stony: the rock scan, greyed like the cliffs, on a lit dome.
    // Flat and gentle ground only: a stone laid out from above would smear
    // into a streak down a steep face.
    let stony = clamp(w[5] * 0.8 + w[7] * 0.7 + w[6] * 0.3 + w[2] * 0.06
        - sand_w - w[9] * 0.5 - canopy * 0.3, 0.0, 1.0) * (1.0 - max(snow_w, ice_w))
        * (1.0 - smoothstep(0.08, 0.2, slope));
    let boulders = stones(xy, 7.0, stony * 0.45, px, 91.0);
    let boulder_cover = boulders.cover * (1.0 - smoothstep(1.5, 3.0, px));
    var boulder_rgb = vec3<f32>(0.0);
    if boulder_cover > 0.004 {
        let scan = textureSampleGrad(terrain_materials, repeat_sampler, xy / 2.3, MAT_ROCK_FACE, dx / 2.3, dy / 2.3).rgb;
        let grey = dot(scan, vec3<f32>(0.2126, 0.7152, 0.0722));
        let lichen = mix(vec3<f32>(1.0), vec3<f32>(0.8, 0.92, 0.62), smoothstep(0.4, 0.8, wet) * (0.3 + 0.7 * patchy));
        // Weathered stone is paler than the turf around it.
        // Weathered stone is paler than the turf around it; the scan's own
        // light and dark keeps it from reading as a smooth pebble.
        let grain = clamp(grey / 0.07, 0.35, 2.2);
        boulder_rgb = mix(vec3<f32>(0.07), scan, 0.25) * vec3<f32>(0.95, 0.97, 1.02) * lichen
            * grain * (1.1 + boulders.tone * 0.7);
    }

    // Large-scale texture: the aerial meadow scan's light and dark at 42 m
    // keeps every kind of ground from flattening into one colour when the
    // close-up scans are all averaged away by distance.
    let macro_scan = terrain_projection(xy / 42.0, dx / 42.0, dy / 42.0, MAT_MEADOW, vec2<f32>(0.0), 0.0);
    // Its gravel patches are the brightest part of the scan; softened, they
    // no longer read as pale blots repeating every 42 m.
    let macro_lum = pow(dot(macro_scan.color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722)) / 0.125, 0.6);
    let macro_mod = mix(1.0, clamp(macro_lum, 0.55, 1.4), 0.75 * (1.0 - sand_w) * (1.0 - canopy * 0.5));

    // ---- Colour ------------------------------------------------------------
    // The scans are true to life; broad tint and brightness fields make one
    // field of grass read as greener here, sun-bleached there.
    let dry_tint = mix(vec3<f32>(1.07, 1.0, 0.84), vec3<f32>(0.90, 1.04, 0.94), wet);
    let green_part = (w[1] + w[2] + w[3]) / max(w[1] + w[2] + w[3] + w[4] + w[5] + w[6] + w[7] + w[8] + w[9], 1e-3);
    var albedo = ground_color * mix(vec3<f32>(1.0), dry_tint, green_part * 0.7)
        * (0.78 + broad * 0.28 + patchy * 0.16 + fine * 0.1);
    albedo *= macro_mod;
    albedo = mix(albedo, boulder_rgb, boulder_cover);
    albedo *= mix(1.0, boulders.ao, 1.0 - smoothstep(1.5, 3.0, px));
    // A closed canopy keeps the floor in shade even where the sun's shadow
    // map has faded out at a distance.
    albedo *= 1.0 - canopy * 0.35;
    // Cliffs as before: the scan's grain, greyed toward weathered stone.
    let mineral = dot(cliff.color, vec3<f32>(0.2126, 0.7152, 0.0722));
    let stone = mix(vec3<f32>(mineral) * vec3<f32>(0.95, 0.96, 0.98), cliff.color, 0.3)
        * (1.05 + broad * 0.5 + patchy * 0.25);
    // Maps with a snow layer are mountain maps: their cliffs show strata,
    // ledges catching light over darker bands, broken by cracks.
    if layer.z > 0.5 && rock_w > 0.004 {
        // Only some outcrops are bedded, and only their steep faces show it.
        let bedded = smoothstep(0.5, 0.72, grad_noise2(xy + 211.0, 320.0)) * smoothstep(0.7, 1.2, slope);
        let bend = grad_noise2(xy + 7.0, 90.0) * 14.0 + grad_noise2(xy - 31.0, 23.0) * 3.0;
        let band = sin(z * (0.22 + 0.2 * grad_noise2(xy - 5.0, 400.0)) + bend);
        let ledge = smoothstep(0.6, 0.95, band) * bedded;
        let seam = (1.0 - smoothstep(0.0, 0.1, abs(band + 0.3))) * bedded;
        let crack = smoothstep(0.8, 0.92, grad_noise2(vec2<f32>(xy.x + xy.y, z * 3.0), 6.0));
        let s = stone * (0.95 + 0.25 * ledge) * (1.0 - 0.3 * seam) * (1.0 - 0.3 * crack);
        albedo = mix(albedo, s, rock_w);
    } else {
        albedo = mix(albedo, stone, rock_w);
    }
    // Snow: drifts a little brighter and darker, bluer in its hollows.
    let drift = 0.93 + 0.1 * fine + 0.05 * sward;
    let snow_rgb = vec3<f32>(0.65, 0.69, 0.74) * drift * mix(vec3<f32>(1.0), vec3<f32>(0.9, 0.95, 1.05), concavity * 1.5);
    albedo = mix(albedo, snow_rgb, snow_w);

    // Glacier ice, seen as it flows downhill (the frame below runs along
    // and across the flow):
    // * foliation: pale and blue bands drawn out along the flow, and grey
    //   stripes of rock debris carried down from where valleys join;
    // * crevasses: where the ice steepens it is pulled apart across the
    //   flow; slots opened in offset pieces, dark blue down inside;
    // * seracs: in icefalls the ice breaks into tilted blocks;
    // * firn: old snow over everything above the glacier's snow line,
    //   the crevasses showing through as sagging lines;
    // * ice walls: clear blue, banded by the years laid down in it, fluted
    //   by meltwater, with snow on the lip and wet dark ice at the foot.
    var ice_grad = vec2<f32>(0.0);
    var ice_glow = vec3<f32>(0.0);
    if ice_w > 0.004 {
        let fall = length(base_n.xy);
        // Rise over run (`slope` above is 1 - n.z, far smaller on gentle ice).
        let grade = fall / max(base_n.z, 0.05);
        let down = select(vec2<f32>(0.0, -1.0), -base_n.xy / max(fall, 1e-4), fall > 1e-3);
        let flowing = smoothstep(0.01, 0.05, fall);
        let side = vec2<f32>(-down.y, down.x);
        let across = dot(xy, side);
        let along = dot(xy, down);
        let steep = smoothstep(0.06, 0.2, grade);
        let wall = smoothstep(0.9, 1.7, grade);

        let band = grad_noise2(vec2<f32>(across, along * 0.08) + 71.0, 9.0);
        // Rubble in broad stripes drawn out along the flow, only where the
        // ice clearly flows (a weak flow direction curls them into loops).
        let dirt = grad_noise2(vec2<f32>(across, along * 0.02) - 213.0, 60.0);
        let debris = smoothstep(0.68, 0.8, dirt + (fine - 0.5) * 0.08) * smoothstep(0.05, 0.12, fall) * (1.0 - wall);

        // Crevasses open where the ice steepens and is pulled apart: slots
        // along the contours (across the flow), 16-34 m apart down the
        // grade, each broken into offset pieces a few tens of metres long.
        // Laid by height, so they stay square to the flow however it turns.
        // Pulled apart where it steepens, and dragged along its edges.
        let drag = (1.0 - smoothstep(0.7, 0.97, layer.x)) * smoothstep(0.25, 0.55, layer.x);
        // Crevasse fields: where the ice steepens, along its edges, and in
        // patches where it is stretched round a bend or over a hump.
        let field = smoothstep(0.52, 0.66, grad_noise2(xy - 301.0, 260.0));
        let tension = max(max(smoothstep(0.08, 0.2, grade), drag), field * 0.8) * (1.0 - smoothstep(0.5, 0.8, grade));
        let spacing = 24.0 + 22.0 * grad_noise2(xy + 5.0, 160.0);
        // Laid by a height smoothed over the neighbourhood, so the rows
        // sweep in arcs instead of tracing every hummock in the ice.
        let r = 70.0;
        let z_s = (z + terrain_height(xy + vec2<f32>(r, 0.0)) + terrain_height(xy - vec2<f32>(r, 0.0))
            + terrain_height(xy + vec2<f32>(0.0, r)) + terrain_height(xy - vec2<f32>(0.0, r))) * 0.2;
        let bend = grad_noise2(xy + 37.0, 90.0) * 0.8 + grad_noise2(xy - 11.0, 23.0) * 0.15;
        let phase = z_s / (max(grade, 0.03) * spacing) + bend;
        let row = floor(phase);
        let edge = min(fract(phase), 1.0 - fract(phase)) * 2.0;
        let piece = grad_noise2(xy + vec2<f32>(row * 41.0, row * 17.0), 55.0);
        let width = 0.14 + 0.2 * hash21(vec2<f32>(row, floor(across / 30.0)));
        // More of them open the harder the ice is pulled.
        let kept = smoothstep(0.62 - 0.14 * tension, 0.7 - 0.14 * tension, piece) * tension * flowing * (1.0 - wall);
        // Anti-aliased, and gone where a slot would be thinner than a pixel.
        let aa = max(abs(dpx.z), abs(dpy.z)) / (max(grade, 0.03) * spacing);
        let crevasse = (1.0 - smoothstep(width * 0.55 - aa, width + aa, edge)) * kept * (1.0 - smoothstep(0.7, 1.4, aa / width));
        // Their lips catch the light.
        let lip_lit = (1.0 - smoothstep(width, width * 1.9 + aa, edge)) * kept * (1.0 - crevasse) * (1.0 - smoothstep(0.7, 1.4, aa / width));
        // Seracs: tilted blocks where it falls steeply.
        let icefall = smoothstep(0.3, 0.6, grade) * (1.0 - wall);
        let block_uv = vec2<f32>(across, along) / 11.0 + bend * 0.3;
        let block = floor(block_uv);
        let inside = fract(block_uv);
        let gap = 1.0 - smoothstep(0.0, 0.12, min(min(inside.x, 1.0 - inside.x), min(inside.y, 1.0 - inside.y)));
        let tilt = vec2<f32>(hash21(block + 3.1), hash21(block + 7.9)) - 0.5;

        // Old snow above the glacier's snow line; the tongue below is bare.
        let firn = smoothstep(560.0, 640.0, alt + (patchy - 0.5) * 90.0 - steep * 60.0) * (1.0 - debris) * (1.0 - wall);
        var ice_rgb = mix(vec3<f32>(0.15, 0.29, 0.39), vec3<f32>(0.33, 0.46, 0.56), band);
        ice_rgb *= 0.92 + 0.16 * fine;
        ice_rgb = mix(ice_rgb, vec3<f32>(0.12, 0.11, 0.1) * (0.8 + 0.4 * fine), debris * 0.85);
        ice_rgb = mix(ice_rgb, snow_rgb, firn * 0.85);
        ice_rgb = mix(ice_rgb, vec3<f32>(0.72, 0.8, 0.86), lip_lit * 0.5);
        // Lateral moraine: grey rubble along the ice's edge.
        let margin = (1.0 - smoothstep(0.55, 0.92, layer.x + (fine - 0.5) * 0.35)) * (1.0 - wall);
        ice_rgb = mix(ice_rgb, vec3<f32>(0.14, 0.13, 0.12) * (0.75 + 0.5 * patchy), margin * 0.75);
        // Down in a slot: deep blue, darker the wider it opens.
        let slot = crevasse * (1.0 - 0.55 * firn);
        ice_rgb = mix(ice_rgb, mix(vec3<f32>(0.05, 0.2, 0.32), vec3<f32>(0.01, 0.05, 0.1), smoothstep(0.08, 0.2, width)), slot);
        ice_rgb = mix(ice_rgb, ice_rgb * (0.85 + 0.35 * hash21(block)), icefall);
        ice_rgb = mix(ice_rgb, vec3<f32>(0.03, 0.12, 0.2), gap * icefall);

        if wall > 0.0 {
            let layers = sin(z * 1.1 + grad_noise2(xy, 40.0) * 6.0 + grad_noise2(xy + 9.0, 8.0) * 1.5) * 0.5 + 0.5;
            let flute = grad_noise2(vec2<f32>(across * 1.6, z * 0.06), 2.5);
            var face = mix(vec3<f32>(0.06, 0.27, 0.42), vec3<f32>(0.28, 0.58, 0.72), layers * 0.6 + flute * 0.4);
            face = mix(face, vec3<f32>(0.6, 0.7, 0.76), smoothstep(0.82, 1.0, layers) * 0.5);
            // A lip of snow along the top and wet, dirty ice at the foot.
            let lip = smoothstep(0.5, 0.9, base_n.z + (fine - 0.5) * 0.2);
            face = mix(face, snow_rgb, lip * 0.6);
            ice_rgb = mix(ice_rgb, face, wall);
            ice_glow = vec3<f32>(0.012, 0.05, 0.07) * wall * ice_w * (0.5 + 0.5 * layers) * (1.0 - lip);
            ice_grad += side * (flute - 0.5) * wall * 1.4;
        }
        ice_glow += vec3<f32>(0.004, 0.02, 0.035) * slot * ice_w;
        albedo = mix(albedo, ice_rgb, ice_w);
        // A crevasse is a slot: its walls lean in across the flow. Serac
        // blocks tilt every which way.
        ice_grad += down * (fract(phase) - 0.5) * crevasse * 1.6 + tilt * icefall * 0.9;
    }

    // Scan normals, then the procedural relief and stones as world slopes.
    let g = ground_n.xy / max(ground_n.z, 0.3);
    let grad = vec3<f32>(g * 1.1 - relief - boulders.grad * boulder_cover, 0.0);
    let ground_normal = normalize(base_n + (grad - base_n * dot(grad, base_n)));
    var n = normalize(mix(ground_normal, cliff.normal, rock_w));
    n = normalize(mix(n, base_n, max(snow_w, ice_w) * 0.7));
    n = normalize(n + vec3<f32>(ice_grad * ice_w, 0.0));
    let ground_rough = ga.color.a * bw.x + gb.color.a * bw.y + gc.color.a * bw.z;
    var rough = mix(clamp(ground_rough, 0.75, 1.0), clamp(cliff.roughness, 0.7, 0.95), rock_w);
    rough = mix(rough, 0.8, boulder_cover * 0.5);
    rough = mix(rough, 0.7, snow_w);
    rough = mix(rough, 0.34, ice_w);
    // Rain darkens the ground and gives it a sheen while it falls.
    let soaked = weather_at(xy).w;
    albedo *= 1.0 - 0.3 * soaked;
    rough = mix(rough, 0.42, soaked * 0.75);
    // Cavity occlusion belongs to the material, and terrain concavity adds
    // grounded shading along gullies and the feet of slopes.
    let cavity = mix(ga.normal.w * bw.x + gb.normal.w * bw.y + gc.normal.w * bw.z, cliff.ao, rock_w);
    albedo *= (0.45 + cavity * 0.55) * (1.0 - concavity * 0.8);

    // Seabed: a little darker and bluer with depth. The water drawn on top
    // does most of the dimming, so a wreck field on the bottom still reads.
    let depth = max(-alt, 0.0);
    albedo = mix(albedo, albedo * vec3<f32>(0.5, 0.68, 0.74), clamp(depth / 30.0, 0.0, 1.0));
    if depth > 0.04 && dist < 180.0 {
        // Light that made it through the surface, crawling on the sand.
        // World-space noise: a tiled pair at 6–10 m was a diamond lattice
        // through the water from the play camera.
        let t = globals.camera.w;
        let c1 = grad_noise2(xy + vec2<f32>(t * 2.8, t * 1.6), 5.4);
        let c2 = grad_noise2(xy - vec2<f32>(t * 1.9, t * 2.7), 3.7);
        let caustic = pow(1.0 - abs(c1 - c2), 6.0) * (1.0 - smoothstep(0.5, 11.0, depth));
        albedo += vec3<f32>(0.10, 0.22, 0.18) * caustic * clamp(1.0 - dist / 160.0, 0.0, 1.0);
    }

    var m: Pbr;
    m.albedo = albedo;
    m.metallic = 0.0;
    m.roughness = rough;
    m.emissive = ice_glow;
    let v = normalize(eye - in.world);
    var horizon = 1.0;
    let toward_sun = normalize(globals.sun.xy);
    let rise = globals.sun.z / max(length(globals.sun.xy), 0.1);
    for (var i = 0; i < 4; i++) {
        let reach = 24.0 * exp2(f32(i));
        let obstruction = terrain_height(xy + toward_sun * reach) - z - rise * reach;
        horizon = min(horizon, smoothstep(-3.0, 5.0, -obstruction));
    }
    let shadow = sun_shadow(in.world, base_n) * horizon;
    // How much of the sky the ground sees: how far the land around rises above
    // this spot's own slope, in six directions at two reaches that grow with
    // the view, so gullies and the feet of slopes fall into shade at any zoom
    // and ridges stand out lit.
    let lean = -base_n.xy / max(base_n.z, 0.2);
    let reach = max(26.0, px * 5.0);
    var open_sky = 0.0;
    for (var i = 0; i < 6; i++) {
        let a = f32(i) * 1.0471976 + 0.5236;
        let dir = vec2<f32>(cos(a), sin(a));
        var rise = 0.0;
        for (var k = 0; k < 2; k++) {
            let r = reach * select(1.0, 3.6, k == 1);
            let above = terrain_height(xy + dir * r) - (z + dot(lean, dir * r));
            rise = max(rise, above / r);
        }
        open_sky += 1.0 - rise * inverseSqrt(1.0 + rise * rise);
    }
    let sky_vis = pow(open_sky / 6.0, 1.6) * (0.55 + cavity * 0.45);
    var color = shade_pbr_vis(m, n, v, globals.sun.xyz, shadow, sky_vis);
    color += albedo * lightning_light(in.world, n) * 0.35;
    color += local_lights(m, in.world, n, v);

    if push.build_grid == 1u {
        // Under the sea the grid is drawn on the water's surface instead (water.wgsl);
        // this hands over as the water's shore alpha comes in.
        let dry = 1.0 - smoothstep(0.015, 0.12, water - z);
        color = mix(color, build_grid_overlay(color, xy, dist), dry);
    }

    color = apply_fog_of_war(color, xy);
    color = apply_haze(color, in.world, eye);
    return vec4<f32>(color, 1.0);
}
