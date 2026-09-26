//!use bindings
//!use habitat

// Grass, the drawing half (renderer/grass.rs): the tufts grass_gen.wgsl grew
// this frame, each drawn as a handful of curved, tapering blades rising from
// round its foot, bent by what the tuft's lean says (the air, the blasts, the
// units) plus a shiver of their own. Drawn in the scene pass after the ground.

@group(1) @binding(0) var<storage, read> tufts: array<Tuft>;

struct GrassOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    // x across the blade (-1 to 1), y up it (0 to 1), z the sun's shadow, w a
    // per-blade random, plus 2 on a flowering stem.
    @location(2) blade: vec4<f32>,
    // The tuft's look, tone and light (Tuft).
    @location(3) @interpolate(flat) packed: vec3<u32>,
    // How deep in a wind wave the tuft stands (Tuft::sheen).
    @location(4) @interpolate(flat) sheen: f32,
}

// Blade width (metres at the foot), how far the blades splay out of the tuft,
// how much they arch before bending over.
fn grass_shape(kind: u32) -> vec3<f32> {
    switch kind {
        case GRASS_LUSH: { return vec3<f32>(0.013, 0.34, 0.45); }
        case GRASS_MEADOW: { return vec3<f32>(0.010, 0.3, 0.6); }
        case GRASS_TROPICAL: { return vec3<f32>(0.034, 0.36, 0.65); }
        case GRASS_HIGHLAND: { return vec3<f32>(0.011, 0.42, 0.35); }
        default: { return vec3<f32>(0.024, 0.55, 0.2); }
    }
}

// The sun's shadow map at `world`, one tap of the nearest cascade that holds
// it: per vertex, where a blade's few pixels do not need the full filter.
fn grass_shadow(world: vec3<f32>) -> f32 {
    let strength = globals.map.w;
    if strength <= 0.0 {
        return 1.0;
    }
    for (var i = 0u; i < 3u; i++) {
        let c = shadow_coord(i, world, vec3<f32>(0.0, 0.0, 1.0));
        if c.w >= 0.97 {
            continue;
        }
        let lit = textureSampleCompareLevel(shadow_map, shadow_sampler, c.xy, i, c.z);
        return mix(1.0, lit, strength);
    }
    return 1.0;
}

@vertex
fn vs_grass(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> GrassOut {
    var band = 0u;
    if ii >= GRASS_BAND_FIRST[2] {
        band = 2u;
    } else if ii >= GRASS_BAND_FIRST[1] {
        band = 1u;
    }
    let segments = GRASS_BAND_SEGMENTS[band];
    let per_blade = segments * 2u + 1u;
    let blade = vi / per_blade;
    let k = vi % per_blade;
    let t = tufts[ii];
    let size = unpack2x16float(t.size);
    let lean = unpack2x16float(t.lean);
    let look = unpack2x16float(t.blade);
    let seed = t.look & 0xFFFFu;
    let kind = (t.look >> 16u) & 0xFu;
    let shape = grass_shape(kind);
    let time = globals.camera.w;

    let b = veg_hash(seed * 31u + blade * 7919u + 1u);
    let r1 = veg_rand(b);
    let r2 = veg_rand(b ^ 0x68E31DA4u);
    let r3 = veg_rand(b ^ 0xB5297A4Du);
    let r4 = veg_rand(b ^ 0x1B56C4E9u);
    let r5 = veg_rand(b ^ 0x7FEB352Du);
    let r6 = veg_rand(b ^ 0x846CA68Bu);

    // The blade's foot, on the ground's plane round the tuft's foot.
    let angle = r1 * 6.2831853;
    let outward = vec2<f32>(cos(angle), sin(angle));
    let off = outward * size.y * sqrt(r2);
    let g = unpack2x16float(t.ground);
    let gz = sqrt(max(1.0 - dot(g, g), 0.2));
    let root = vec3<f32>(t.pos.xy + off, t.pos.z - dot(g, off) / gz - 0.04);
    // Some blades of the meadow and the heights are flowering stems: taller,
    // thin, straighter, a seed head on top.
    let stem = (kind == GRASS_MEADOW && r6 > 0.8) || (kind == GRASS_HIGHLAND && r6 > 0.88)
        || (kind == GRASS_LUSH && r6 > 0.95);
    // And a few in lush turf and moss are wildflowers: a short stalk, a round head.
    let flower = !stem && (kind == GRASS_LUSH || kind == GRASS_MOSS) && r6 < 0.015;
    var tall = size.x * select(0.45 + 0.65 * r3 * r3, 1.05 + 0.3 * r3, stem);
    if flower {
        tall = size.x * (0.7 + 0.3 * r3);
    }

    // Splayed out of the tuft, leaned by the tuft's air, shivering on its own.
    let quiver = look.y * (0.04 + 0.08 * r5) * sin(time * (4.5 + 3.0 * r6) + r4 * 6.2831853 + dot(t.pos.xy, vec2<f32>(0.37, 0.21)));
    let lean_dir = lean / max(length(lean), 1e-3);
    let splay = shape.y * select(0.4 + 0.9 * r4, 0.15 + 0.3 * r4, stem || flower);
    var bend = lean * select(1.0, 1.15, stem) + outward * splay + (lean_dir + vec2<f32>(-outward.y, outward.x) * 0.6) * quiver;
    let f = length(bend);
    if f > 0.97 {
        bend *= 0.97 / f;
    }
    // Long blades cannot hold themselves up: they droop over at the top.
    let droop = select(0.15 + 0.45 * r2 * r3, 0.05, stem) * shape.z;
    let reach = min(f + droop, 0.97);
    if f > 1e-3 && reach > f {
        bend *= reach / f;
    } else if reach > f {
        bend = outward * reach;
    }
    let rise = tall * sqrt(max(1.0 - reach * reach, 0.05));
    let tip = root + vec3<f32>(bend * tall * 0.92, rise);
    // A blade rises from its foot before it bows over: the curl lifts the
    // curve's middle, so a tall blade arches and its tip hangs.
    let ctrl = root + vec3<f32>(bend * tall * (0.3 - shape.z * 0.25), rise * (0.85 + shape.z * 0.4));

    let level = min(k / 2u, segments);
    let s = f32(level) / f32(segments);
    let p = root * (1.0 - s) * (1.0 - s) + ctrl * 2.0 * (1.0 - s) * s + tip * s * s;
    let tangent = normalize((ctrl - root) * 2.0 * (1.0 - s) + (tip - ctrl) * 2.0 * s + vec3<f32>(0.0, 0.0, 1e-4));
    // Blades face about across the tuft, each turned a little its own way.
    let facing = angle + 1.5707963 + (r5 - 0.5) * 1.3;
    let flat_side = vec3<f32>(cos(facing), sin(facing), 0.0);
    let side = normalize(flat_side - tangent * dot(flat_side, tangent) + vec3<f32>(1e-4, 0.0, 0.0));
    let across = select(select(-1.0, 1.0, (k & 1u) == 1u), 0.0, k == segments * 2u);
    let eye = globals.camera.xyz;
    let dist = max(distance(p, eye), 1.0);
    // Wider where fewer blades stand for a tuft, never thinner than most of a pixel.
    let blades = f32(GRASS_BAND_BLADES[band]);
    var width = shape.x * look.x * sqrt(12.0 / blades) * (0.75 + 0.5 * r6);
    width = max(width, 1.1 * dist / globals.lod.x);
    var taper = 1.0 - pow(s, 1.6);
    if stem {
        // A thin stalk swelling into a spindle of seed near the top.
        taper = 0.4 + 1.0 * sin(3.1415927 * clamp((s - 0.6) / 0.4, 0.0, 1.0));
        if k == segments * 2u {
            taper = 0.0;
        }
    } else if flower {
        // A thin stalk, then a small head: wide at the last level, closing to
        // the tip.
        taper = select(0.3, 2.2, level + 1u == segments);
        if k == segments * 2u {
            taper = 0.0;
        }
    }
    let world = p + side * across * width * 0.5 * taper;

    var out: GrassOut;
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    out.world = world;
    // Rounded across its width, so a blade shades like a leaf, not a card.
    let face = normalize(cross(side, tangent));
    out.normal = normalize(face + side * across * 0.45);
    out.blade = vec4<f32>(across, s, grass_shadow(world), r3 + select(0.0, 2.0, stem) + select(0.0, 4.0, flower));
    out.packed = vec3<u32>(t.look, t.tone, t.light);
    out.sheen = t.sheen;
    return out;
}

@fragment
fn fs_grass(in: GrassOut) -> @location(0) vec4<f32> {
    let kind = (in.packed.x >> 16u) & 0xFu;
    let charred = f32(in.packed.x >> 24u) / 255.0;
    let tone = unpack4x8unorm(in.packed.y);
    let light = unpack4x8unorm(in.packed.z);
    let eye = globals.camera.xyz;
    // Up close each blade shows its dark foot and pale tip; a blade a pixel or
    // two wide is only a fleck, so far off the grass is shaded as one soft mass
    // (no speckle).
    let detail = smoothstep(4.0, 16.0, GRASS_CELL_M * globals.lod.x / max(distance(in.world, eye), 1.0));
    let s = mix(0.62, clamp(in.blade.y, 0.0, 1.0), detail);
    let flower = in.blade.w > 3.5;
    let stem = in.blade.w > 1.5 && !flower;
    let hue = (fract(in.blade.w) - 0.5) * mix(0.5, 1.0, detail);

    let c = grass_colours(kind, tone.w);
    var albedo = mix(mix(c.foot, c.mid, smoothstep(0.0, 0.45, s)), c.tip, smoothstep(0.45, 1.0, s));
    // Each blade a little its own shade: some greener, some straw.
    albedo *= (1.0 + hue * 0.56) * vec3<f32>(1.0 + hue * 0.3, 1.0, 1.0 - hue * 0.4);
    if stem {
        // Straw stalk, the seed head gold and pale.
        let head = smoothstep(0.55, 0.7, clamp(in.blade.y, 0.0, 1.0));
        let seed_head = mix(c.tip, vec3<f32>(0.13, 0.095, 0.05), 0.55);
        albedo = mix(mix(c.mid, c.tip, 0.5), mix(c.mid, seed_head, detail), head);
    }
    if flower {
        // White daisies, yellow buttercups, the odd purple clover head.
        let pick = fract(in.blade.w);
        var petal = vec3<f32>(0.42, 0.42, 0.39);
        if pick > 0.55 {
            petal = vec3<f32>(0.44, 0.33, 0.04);
        } else if pick > 0.4 {
            petal = vec3<f32>(0.24, 0.10, 0.29);
        }
        let head = smoothstep(0.5, 0.62, clamp(in.blade.y, 0.0, 1.0));
        albedo = mix(c.mid, mix(c.tip, petal, detail), head);
    }
    albedo *= select(tone.rgb * 2.0, vec3<f32>(1.0), flower && in.blade.y > 0.56);
    // Crushed grass bruises paler; burnt grass is black at the tips first.
    albedo = mix(albedo, albedo * vec3<f32>(1.15, 1.08, 0.85), light.w * 0.5);
    albedo = mix(albedo, vec3<f32>(0.018, 0.016, 0.014), clamp(charred * (0.6 + s * 0.8), 0.0, 1.0));

    let v = normalize(eye - in.world);
    var n = normalize(in.normal);
    n *= select(-1.0, 1.0, dot(n, v) >= 0.0);
    // A field is lit like a soft volume: the blades' own facing only in part.
    n = normalize(mix(n, vec3<f32>(0.0, 0.0, 1.0), mix(0.7, 0.35, detail)));
    // Deep in the tuft the sky is hidden and the light is low.
    let occlusion = mix(0.28, 1.0, smoothstep(0.0, 0.85, s)) * mix(1.0, 0.8, light.w);
    let sun = globals.sun.xyz;
    let shadow = in.blade.z * light.x;

    var m: Pbr;
    m.albedo = albedo;
    m.metallic = 0.0;
    m.roughness = 0.52;
    m.emissive = vec3<f32>(0.0);
    let sky_vis = light.y * occlusion * mix(1.0, screen_ao(in.clip.xy), 0.5);
    var color = shade_pbr_vis(m, n, v, sun, shadow * mix(0.55, 1.0, occlusion), sky_vis);
    // Sunlight through the blades, strongest looking into the sun: a field
    // against a low sun glows.
    let behind = max(-dot(normalize(in.normal), sun) * sign(dot(normalize(in.normal), v)), 0.0);
    let into_sun = pow(max(-dot(v, sun), 0.0), 3.0);
    color += albedo * vec3<f32>(0.9, 1.1, 0.45) * atmos.sun_color.rgb * shadow
        * (behind * 0.3 + into_sun * 0.6) * (0.3 + 0.7 * s) * (1.0 - charred) * mix(0.4, 1.0, detail);
    // A passing wave presses the blades over: their pale, sky-facing sides
    // catch the light in a band that rolls on across the field.
    let pressed = in.sheen * smoothstep(0.15, 0.7, s);
    let grey = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    color = mix(color, vec3<f32>(grey) * vec3<f32>(1.2, 1.18, 0.98), pressed * 0.22) * (1.0 + pressed * 0.32);
    color += albedo * lightning_light(in.world, n) * 0.35;
    color += local_lights(m, in.world, n, v) * occlusion;

    color = apply_fog_of_war(color, in.world.xy);
    color = apply_haze(color, in.world, eye);
    return vec4<f32>(color, 1.0);
}
