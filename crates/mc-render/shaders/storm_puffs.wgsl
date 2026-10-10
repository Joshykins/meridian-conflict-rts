// A storm seed's cloud (renderer/storm_cloud_fx.rs, the Nimbus's Storm Seeder): a
// thunderhead built of lumps (`PUFF_STORM_CLOUD`), each a firm billow lit as the sky's own
// clouds are (clouds.wgsl `march`: the sun through it, the sky from above, the land's
// light on its base, lightning flashes inside), so it reads as weather and not as smoke.
// Prepended to puffs.wgsl with `//!use storm_puffs` (it uses that file's `Puff` and
// `PuffOut`).
//
// Every lump of one cloud is laid at once, lowest first (so from above the crown covers
// the base), and carried with the cloud as the seed drifts on behind the aircraft: the
// drift eases to a stop as the sim's does (mc-sim `cloud_seed::path`), and the cloud swells as
// it goes, each lump showing as the cloud grows past its own share.

fn is_storm_puff(kind: u32) -> bool {
    return kind == PUFF_STORM_CLOUD;
}

// How far grown the cloud is `t` seconds after the lump was laid (0 a seed, 1 grown), and
// how far it has drifted since, as a share of the drift left then.
fn storm_growth(p: Puff, t: f32) -> vec2<f32> {
    let left = p.appearance.x;
    let u = select(1.0, clamp(t / max(left, 0.001), 0.0, 1.0), left > 0.0);
    let f = p.appearance.y + (1.0 - p.appearance.y) * u;
    return vec2<f32>(f, 1.0 - (1.0 - u) * (1.0 - u));
}

fn storm_puff_vertex(p: Puff, corner: vec2<f32>, t: f32, o: PuffOut) -> PuffOut {
    var out = o;
    let g = storm_growth(p, t);
    let f = g.x;
    let middle = p.origin + p.vel * g.y;
    let reach = max(p.appearance.z, 1.0);
    // A seed stays a small knot at first and builds slowly into its size
    // (storm_cloud_fx.rs `size_at`).
    let grown = 0.06 + 0.94 * f * f * (3.0 - 2.0 * f);
    // Spent: it spreads and thins, its base sagging as it rains itself out.
    let spent = clamp((t - (p.life - PUFF_STORM_LINGER)) / PUFF_STORM_LINGER, 0.0, 1.0);
    var offset = (p.pos - p.origin) * grown * (1.0 + 0.22 * spent);
    let height = offset.z / reach;
    offset.z -= spent * spent * reach * 0.3 * (1.0 - smoothstep(-0.3, 0.45, height));
    // Each billow churns slowly round its place.
    let s = p.params.w * 6.2832;
    offset += vec3<f32>(sin(t * 0.31 + s), cos(t * 0.27 + s * 1.7), 0.6 * sin(t * 0.4 + s * 2.3))
        * p.params.x * 0.07 * grown;
    let pos = middle + offset;
    // It shows as the cloud grows past its share, swelling out of nothing.
    let shows = smoothstep(p.appearance.w, p.appearance.w + 0.14, f) * smoothstep(0.0, 0.35, t);
    let size = p.params.x * grown * (0.55 + 0.45 * shows) * (1.0 + 0.3 * spent);
    out.opacity *= shows * (1.0 - smoothstep(0.25, 1.0, spent));
    if out.opacity < 0.004 {
        return out;
    }
    let center = globals.view_proj * vec4<f32>(pos, 1.0);
    if center.w < 1.0 {
        return out;
    }
    let px = max(size * globals.lod.x / center.w, 1.5);
    let ndc = center.xy / center.w + corner * px * globals.viewport.zw;
    // A billow, not a card: tested a little in front of its middle, so the hull flying
    // through it is not sliced flat.
    let depth = front_depth(pos, size * 0.3);
    out.clip = vec4<f32>(ndc * center.w, depth * center.w, center.w);
    out.uv = corner;
    out.world = effect_billboard_world(pos, corner, size);
    out.state = vec3<f32>(spent, p.params.z, p.params.w);
    // The cloud's middle and reach, for how high in it a pixel is (`storm_puff_color`).
    out.roll = middle;
    out.cloud_size = reach;
    // No shield clips weather.
    out.origin = vec3<f32>(0.0, 0.0, 2.0e8);
    // Its cloud's shade of grey.
    out.appearance = vec4<f32>(p.params.y, 0.0, 0.0, 0.0);
    return out;
}

fn storm_puff_color(in: PuffOut, d: f32) -> vec4<f32> {
    if d > 1.0 {
        return vec4<f32>(0.0);
    }
    let spent = in.state.x;
    let seed = in.state.z;
    let o = vec2<f32>(seed * 5.71, seed * 2.93);
    let big = textureSample(noise_map, repeat_sampler, in.uv * 0.2 + o);
    let fine = textureSample(noise_map, repeat_sampler, in.uv * 0.52 + o.yx);
    // Cauliflower: a firm, lumpy edge while it is fresh, torn and soft as it rains out.
    let lump = (big.b - 0.5) * mix(0.85, 1.2, spent) + (fine.a - 0.5) * mix(0.35, 0.7, spent);
    let edge = d - lump * 0.55;
    let body = 1.0 - smoothstep(mix(0.38, 0.25, spent), mix(0.88, 0.9, spent), edge);
    if body <= 0.002 {
        return vec4<f32>(0.0);
    }
    // A pseudo-sphere normal bumped by the billows, for the sun on its folds.
    let z = sqrt(max(1.0 - d * d, 0.0));
    let bump = (big.rg - 0.5) * 1.1 + (fine.rg - 0.5) * 0.45;
    let n = normalize(vec3<f32>(in.uv * 0.85 + bump * 0.7, z + 0.2));
    let right = normalize(vec3<f32>(globals.view_proj[0].x, globals.view_proj[1].x, globals.view_proj[2].x));
    let up = normalize(vec3<f32>(globals.view_proj[0].y, globals.view_proj[1].y, globals.view_proj[2].y));
    let toward = cross(right, up);
    let sun_dir = vec3<f32>(dot(globals.sun.xyz, right), dot(globals.sun.xyz, up), dot(globals.sun.xyz, toward));
    // The heap is lit as one (its own round shape toward the sun), its billows only
    // shading their own folds a little, so it reads as one cloud and not a pile of balls.
    let heap = normalize((in.world - in.roll) * vec3<f32>(1.0, 1.0, 1.6) + vec3<f32>(0.0, 0.0, 0.3 * in.cloud_size));
    let lit = mix(max(dot(heap, globals.sun.xyz), 0.0), max(dot(n, sun_dir), 0.0), 0.35)
        * smoothstep(-0.1, 0.25, globals.sun.z);
    // How high in the cloud: its belly is in the shade of the whole heap over it.
    let h = (in.world.z - in.roll.z) / max(in.cloud_size, 1.0);
    let crown = smoothstep(-0.35, 0.75, h);
    // As clouds.wgsl lights the sky's clouds: the sun through it, the sky from above
    // (hidden low in the heap), the land's light on its base. A storm's water is grey.
    let sun_light = atmos.sun_color.rgb * mix(1.0, atmos.sun_color.w, 0.8);
    let direct = sun_light * (0.02 + 0.3 * lit) * mix(0.08, 1.0, crown * crown);
    let sky = atmos.sky_color.rgb * mix(0.2, 0.9, crown) + atmos.ground_color.rgb * 0.25 * (1.0 - crown);
    // Wet and dark, darker in the folds and in some billows than others.
    let albedo = vec3<f32>(0.33, 0.36, 0.43) * (0.85 + 0.25 * big.a) * (0.9 + 0.1 * fine.b)
        * in.appearance.x;
    var color = (direct + sky) * albedo;
    // Lightning inside it: the sky's strokes (sky.rs `strike`) as the sky's clouds take
    // them, and the flashes in it (lights.rs), seen through the water as a blue-white glow
    // that is brightest where the billow is thin.
    var glow = vec3<f32>(0.0);
    let count = u32(atmos.counts.y);
    for (var i = 0u; i < count; i++) {
        let fl = atmos.flashes[i];
        if fl.w <= 0.0 { continue; }
        let r = distance(in.world, fl.xyz);
        glow += vec3<f32>(0.62, 0.7, 1.0) * fl.w * exp(-r / 220.0);
    }
    let lamp = in.lamp / (1.0 + in.lamp * 0.02);
    glow += lamp * vec3<f32>(0.75, 0.85, 1.0) * 0.06;
    color += glow * 0.25 * (0.5 + 0.5 * (1.0 - z));
    let density = mix(0.94, 0.6, spent);
    let alpha = clamp(body * density, 0.0, 1.0);
    color = apply_haze(apply_fog_of_war(color, in.world.xy), in.world, globals.camera.xyz);
    return vec4<f32>(color * alpha, alpha);
}
