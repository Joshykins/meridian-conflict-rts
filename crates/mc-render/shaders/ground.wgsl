//!use bindings
// Things painted over the terrain: crater/scorch stains, ore fields,
// and structure foundations. These are decals only; the
// ground under them is never deformed.


@group(1) @binding(0) var<storage, read> stains: array<Stain>;


@group(1) @binding(1) var<storage, read> track_marks: array<TrackMark>;

struct StainOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world: vec3<f32>,
    @location(2) @interpolate(flat) strength_seed: u32,
    @location(3) @interpolate(flat) radius: f32,
}

// `grid` spans [-1, 1] on a small mesh so the decal follows the ground.
@vertex
fn vs_stain(@location(0) grid: vec2<f32>, @builtin(instance_index) instance: u32) -> StainOut {
    let s = stains[instance];
    let seed = f32((s.strength_seed >> 8u) & 0x7FFFu);
    let spun = rot_z(vec3<f32>(grid, 0.0), seed * 0.37).xy;
    let xy = s.pos + spun * s.radius * 1.35;
    let ground = terrain_height(xy);
    let world = vec3<f32>(xy, ground);
    var out: StainOut;
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    // Pull toward the camera a little instead of lifting: no floating at glancing angles.
    out.clip.z += 0.00002 * out.clip.w + 0.02;
    let px = s.radius * globals.lod.x / max(out.clip.w, 1.0);
    if px < 0.75 {
        out.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
    }
    out.uv = grid;
    out.world = world;
    out.strength_seed = s.strength_seed;
    out.radius = s.radius;
    return out;
}

// No stain ever discards (SPIR-V OpKill), nor does a giant's footprint: a friend's RX 7900
// XTX hung in the stains draw, the device lost, the moment molten ground (then a stain) or
// a crater (aircraft shot down) was drawn, and those two kinds were the ones discarding
// inside a helper (2026-10-01). Blended, no depth written:
// a clear colour draws nothing, as a discard did.
@fragment
fn fs_stain(in: StainOut) -> @location(0) vec4<f32> {
    if (in.strength_seed & STAIN_CRATER) != 0u {
        return crater(in);
    }
    let strength = f32(in.strength_seed & 0xFFu) / 255.0;
    let seed = f32(in.strength_seed >> 8u);
    // Each scorch is a different ragged blot. A shared bowl made a carpet of
    // identical brown rings wherever bombs overlapped.
    let spun = rot_z(vec3<f32>(in.uv, 0.0), seed * 0.37).xy;
    let n = textureSample(noise_map, repeat_sampler, in.world.xy / (9.0 + fract(seed * 0.017) * 14.0) + vec2<f32>(seed * 0.013, seed * 0.007)).rg;
    let lobe = value_noise2(spun * (2.2 + fract(seed * 0.13) * 3.0) + vec2<f32>(seed * 0.02, 3.1), 1.0);
    let d = length(spun * (0.75 + lobe * 0.55));
    let edge = d + (n.x - 0.5) * 0.85 + (lobe - 0.5) * 0.35;
    let mask = (1.0 - smoothstep(0.25, 0.95, edge)) * (0.45 + n.y * 0.7);
    let alpha = clamp(mask * (0.22 + strength * 0.45), 0.0, 0.62);
    if alpha < 0.02 {
        return vec4<f32>(0.0);
    }
    let eye = globals.camera.xyz;
    let dist = distance(eye, in.world);
    let base_n = terrain_normal(in.world.xy, clamp(dist * 0.004, 4.0, 24.0));
    let relief = clamp(1.0 - dist / 700.0, 0.2, 1.0);
    let nrm = normalize(base_n + vec3<f32>((n - 0.5) * relief * 0.55, 0.0));
    // Charcoal, not scorched soil. Overlaps stay dark instead of turning brown.
    var albedo = mix(vec3<f32>(0.012, 0.011, 0.01), vec3<f32>(0.028, 0.026, 0.024), n.y);
    albedo = apply_fog_of_war(albedo, in.world.xy);
    var m: Pbr;
    m.albedo = albedo;
    m.metallic = 0.0;
    m.roughness = 0.96;
    m.emissive = vec3<f32>(0.0);
    var lit = shade_pbr_vis(m, nrm, normalize(eye - in.world), globals.sun.xyz, sun_shadow(in.world, base_n), screen_ao(in.clip.xy));
    lit += local_lights(m, in.world, nrm, normalize(eye - in.world));
    lit = mix(lit, vec3<f32>(0.01, 0.009, 0.008), 0.55 + lobe * 0.2);
    return vec4<f32>(apply_haze(lit, in.world, eye), alpha);
}

// A crater: still a decal, but lit as a bowl. The ground slopes down into a
// dark churned middle, rises to a lip of thrown-up earth that catches the sun,
// and ragged spokes of spoil run out past the lip. The radius is the lip's.
fn crater(in: StainOut) -> vec4<f32> {
    let strength = f32(in.strength_seed & 0xFFu) / 255.0;
    let seed = f32((in.strength_seed >> 8u) & 0x7FFFu);
    let spun = rot_z(vec3<f32>(in.uv, 0.0), seed * 0.37).xy;
    let eye = globals.camera.xyz;
    let dist = distance(eye, in.world);
    // Radius in lip radii; the patch reaches 1.35 of them. The lip wanders a little.
    // Round the rim by direction, not angle: no seam where the angle wraps.
    let dir = spun / max(length(spun), 1e-4);
    let wobble = value_noise2(dir * 1.7 + vec2<f32>(seed * 0.1, seed * 0.03), 1.0) - 0.5;
    let r = length(spun) * 1.35 * (1.0 + wobble * 0.22);
    let n = textureSample(noise_map, repeat_sampler, in.world.xy / (3.0 + fract(seed * 0.019) * 3.0) + vec2<f32>(seed * 0.011, seed * 0.005)).rg;
    // Spokes of thrown spoil past the lip.
    let spokes = value_noise2(dir * 5.0 + vec2<f32>(seed * 0.3, seed * 0.07), 1.0);
    let ejecta = (1.0 - smoothstep(1.0, 1.3, r - spokes * 0.18)) * smoothstep(0.9, 1.05, r) * (0.35 + n.x * 0.65);
    // Height of the bowl and lip over the radius, and its slope, outward.
    let depth = 0.4;
    let lip = 0.16;
    let inside = 1.0 - smoothstep(0.9, 1.02, r);
    let slope = 2.0 * depth * r * inside
        - lip * 2.0 * (r - 1.0) / 0.03 * exp(-(r - 1.0) * (r - 1.0) / 0.03);
    let relief = clamp(1.0 - dist / 900.0, 0.25, 1.0);
    let base_n = terrain_normal(in.world.xy, clamp(dist * 0.004, 4.0, 24.0));
    let grit = (n - 0.5) * 0.5 * relief;
    let nrm = normalize(base_n + vec3<f32>(-dir * slope * relief + grit, 0.0));

    // Scorched and churned in the middle, torn raw earth up the sides and on the lip.
    let soil = mix(vec3<f32>(0.075, 0.063, 0.05), vec3<f32>(0.12, 0.1, 0.078), n.y);
    let charred = mix(vec3<f32>(0.016, 0.014, 0.013), vec3<f32>(0.035, 0.031, 0.027), n.x);
    let burnt = (1.0 - smoothstep(0.25, 0.8, r + (n.x - 0.5) * 0.3)) * (0.55 + 0.45 * strength);
    var albedo = mix(soil, charred, burnt);
    albedo = apply_fog_of_war(albedo, in.world.xy);
    var m: Pbr;
    m.albedo = albedo;
    m.metallic = 0.0;
    m.roughness = 0.97;
    m.emissive = vec3<f32>(0.0);
    let v = normalize(eye - in.world);
    var lit = shade_pbr_vis(m, nrm, v, globals.sun.xyz, sun_shadow(in.world, base_n), screen_ao(in.clip.xy));
    lit += local_lights(m, in.world, nrm, v);
    // The bottom of the bowl sees less sky.
    lit *= mix(1.0, 0.55, (1.0 - smoothstep(0.0, 0.85, r)) * inside);
    let alpha = clamp(max(inside * 0.92, ejecta * 0.75), 0.0, 0.92);
    if alpha < 0.02 {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(apply_haze(lit, in.world, eye), alpha);
}

// Ore field: a map polygon of rust-red ground shot through with red-orange
// ore, a clean rim so the field reads from strategic zoom. Each instance is a
// 48 m tile; `strength_seed` packs the field's corner count (low 8 bits) and
// how far back from the tile its first corner sits in `stains` (the rest).
@vertex
fn vs_ore(@location(0) grid: vec2<f32>, @builtin(instance_index) instance: u32) -> StainOut {
    let s = stains[instance];
    let xy = s.pos + grid * s.radius;
    let ground = terrain_height(xy);
    let world = vec3<f32>(xy, ground);
    var out: StainOut;
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    out.clip.z += 0.00002 * out.clip.w + 0.02;
    let px = s.radius * globals.lod.x / max(out.clip.w, 1.0);
    if px < 0.35 {
        out.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
    }
    out.uv = grid;
    out.world = world;
    let first = instance - (s.strength_seed >> 8u);
    out.strength_seed = (first << 8u) | (s.strength_seed & 0xFFu);
    return out;
}

// Signed distance to the field's outline in metres, negative inside.
fn ore_outline(p: vec2<f32>, first: u32, n: u32) -> f32 {
    var d = 1e12;
    var inside = false;
    var j = n - 1u;
    for (var i = 0u; i < n; i++) {
        let a = stains[first + i].pos;
        let b = stains[first + j].pos;
        let e = b - a;
        let w = p - a;
        let q = w - e * clamp(dot(w, e) / max(dot(e, e), 1e-6), 0.0, 1.0);
        d = min(d, dot(q, q));
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + e.x * (p.y - a.y) / e.y {
            inside = !inside;
        }
        j = i;
    }
    return select(sqrt(d), -sqrt(d), inside);
}

@fragment
fn fs_ore(in: StainOut) -> @location(0) vec4<f32> {
    if in.world.z < globals.map.z {
        discard;
    }
    let first = in.strength_seed >> 8u;
    let count = in.strength_seed & 0xFFu;
    let p = in.world.xy;
    let sd = ore_outline(p, first, count);
    if sd > 6.0 {
        discard;
    }
    let eye = globals.camera.xyz;
    let dist = distance(eye, in.world);
    let aa = max(fwidth(sd), 0.05);
    let far = smoothstep(500.0, 3000.0, dist);

    // The first corner's unused radius carries the highlight (0..1: placing
    // or selecting a mine, or Ctrl held).
    let hi = clamp(stains[first].radius, 0.0, 1.0);
    let pulse = 0.5 + 0.5 * sin(globals.camera.w * 3.2 - length(p - stains[first].pos) * 0.03);
    // Ore inside a mine's reach is spoken for: it dims to a dull rust, so the
    // ore still free to claim is what stands out.
    let claimed = ore_claimed(p);
    let free = 1.0 - claimed;

    // The outline: all a field is in play, a faint red-orange line; bright and
    // wider while highlighted, unless spoken for.
    let rim_w = 0.8 + far * 2.2 + hi * free * (1.0 + far * 3.0);
    let rim = 1.0 - smoothstep(rim_w * 0.5, rim_w * 0.5 + aa * 1.5, abs(sd + rim_w * 0.6));
    // The materials red-orange (hud::MASS, 0xFF6B3D), linear, and the dull rust
    // it fades to once claimed.
    let ore_col = mix(vec3<f32>(1.0, 0.147, 0.047), vec3<f32>(0.55, 0.3, 0.2), claimed * 0.8);
    let lift = mix(1.0, 0.75, claimed);
    let rim_col = apply_fog_of_war(ore_col, p) * (0.6 + far * 0.6 + hi * free * (1.2 + 0.5 * pulse)) * lift;
    if hi < 0.01 {
        let a = rim * mix(0.4, 0.26, claimed);
        if a < 0.01 {
            discard;
        }
        let col = mix(apply_fog_of_war(ore_col * 0.6, p), rim_col, rim);
        return vec4<f32>(apply_haze(col, in.world, eye), a);
    }

    // Highlighted: the field turns to thin glass over the ore, which the
    // vein geometry (fs_vein) shows underneath; a faint hatch drifts across it.
    // Claimed ground keeps only a dim, still film.
    let inside = 1.0 - smoothstep(-aa, aa, sd);
    let hatch_w = mix(4.0, 40.0, far);
    let hatch = smoothstep(0.82, 1.0, sin((p.x + p.y) / hatch_w * 3.14159 + globals.camera.w * 1.5) * 0.5 + 0.5);
    let fill = inside * mix(0.025, 0.06 + 0.1 * hatch + 0.06 * pulse, free);
    let col = mix(apply_fog_of_war(ore_col * 0.7, p), rim_col, rim);
    let rim_a = mix(0.9, 0.45, claimed);
    let alpha = clamp(max(fill, rim * rim_a) * hi + rim * 0.4 * (1.0 - hi), 0.0, 0.9);
    if alpha < 0.01 {
        discard;
    }
    return vec4<f32>(apply_haze(col, in.world, eye), alpha);
}

// How far `p` lies in the reach of a mine in sight, 0..1, eased in over a few
// metres at the edge and by how far the claim has faded in. The claims are Stains
// after the ore fields (renderer/ore_fields.rs): centre, reach, strength 0..255.
fn ore_claimed(p: vec2<f32>) -> f32 {
    var k = 0.0;
    let first = globals.ore_claims.x;
    for (var i = 0u; i < globals.ore_claims.y; i++) {
        let c = stains[first + i];
        let strength = f32(c.strength_seed & 0xFFu) / 255.0;
        let inside = 1.0 - smoothstep(c.radius - 8.0, c.radius + 8.0, distance(p, c.pos));
        k = max(k, inside * strength);
    }
    return k;
}

struct PadOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world: vec3<f32>,
    @location(2) @interpolate(flat) packed: u32,
    @location(3) @interpolate(flat) half_m: f32,
}

// The structure's ground plan, from the mesh-baked SDF atlas. Positive is
// outside the pour. Layer is the unit blueprint index.
fn pad_mesh_sd(uv: vec2<f32>, layer: u32) -> f32 {
    let n = textureNumLayers(pad_footprints);
    let i = i32(min(layer, n - 1u));
    let tex = uv / PAD_FOOTPRINT_REACH * 0.5 + 0.5;
    let raw = textureSampleLevel(pad_footprints, clamp_sampler, tex, i, 0.0).r;
    return (0.5 - raw) * 2.0 * PAD_SDF_RANGE;
}

// Metres the pad runs past its lot: the grit that spills off the kerb.
const PAD_SPILL_M: f32 = 1.2;
// The lot of a faction that builds with nanites (`PAD_NANITE`): not paved, but
// black discs ringed in graphite and joined by graphite lines, violet light in their grooves
// while it builds.
const LOT_VIOLET: vec3<f32> = vec3<f32>(0.66, 0.12, 1.0);
const LOT_RED: vec3<f32> = vec3<f32>(1.0, 0.06, 0.1);

// A structure's lot, paved kerb to kerb. The building stands on it; the rest
// is apron units walk on.
@vertex
fn vs_pad(@location(0) grid: vec2<f32>, @builtin(instance_index) instance: u32) -> PadOut {
    let s = stains[instance];
    let reach = s.radius + PAD_SPILL_M;
    let xy = s.pos + grid * reach;
    let world = vec3<f32>(xy, terrain_height(xy));
    var out: PadOut;
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    out.clip.z += 0.00002 * out.clip.w + 0.02;
    let px = s.radius * globals.lod.x / max(out.clip.w, 1.0);
    if px < 1.2 {
        out.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
    }
    // Lot UV: ±1 at the kerb.
    out.uv = grid * reach / max(s.radius, 0.5);
    out.world = world;
    out.packed = s.strength_seed;
    out.half_m = s.radius;
    return out;
}

@fragment
fn fs_pad(in: PadOut) -> @location(0) vec4<f32> {
    // Nothing is paved on the seabed: a structure in the sea stands on its piles or floats.
    let above = in.world.z - globals.map.z;
    if above < 0.0 {
        discard;
    }
    let owner = in.packed & OWNER_MASK;
    let build = f32((in.packed >> PAD_BUILD_SHIFT) & PAD_BUILD_MASK) / 255.0;
    let blueprint = in.packed >> PAD_BLUEPRINT_SHIFT;
    let ghost = select(0u, 1u, (in.packed & PAD_GHOST) != 0u);
    let team = globals.team_colors[owner].rgb;

    let wp = in.world.xy;
    let local = in.uv * in.half_m;
    // Metres per pixel, for edges that stay sharp near and do not shimmer far.
    let px = max(length(fwidth(wp)), 0.004);
    let far = smoothstep(0.06, 0.35, px);
    let nanite = (in.packed & PAD_NANITE) != 0u;

    // Metres inside the kerb; negative out on the dirt. The slab runs a hand's
    // breadth past the lot so two lots side by side overlap rather than meet
    // at half cover each, which let the ground show through as a dark seam.
    let ragged = textureSample(noise_map, repeat_sampler, wp / 5.0).b;
    var edge_in = in.half_m + 0.3 - max(abs(local.x), abs(local.y));
    // A Regency lot is no square slab: a black hub under the building, smaller discs spread out
    // round it on graphite lines (`lot_plate`), bare ground between.
    var plate = vec4<f32>(in.half_m, 0.0, 0.0, 0.0);
    let sd = pad_mesh_sd(in.uv, blueprint);
    if nanite {
        let lot_seed = hash21(floor(wp - local + vec2<f32>(0.5)) * 0.0173);
        plate = lot_plate(local, in.half_m, lot_seed, px);
        // Wherever the building stands past the hub, the hub's black runs under it, still round.
        let under = min(0.4 - sd, in.half_m * 0.95 - length(local));
        edge_in = min(edge_in, max(plate.x, under));
    }
    let paved = smoothstep(-px * 0.5, px * 0.5, edge_in);
    let spill_reach = PAD_SPILL_M * (0.35 + 0.65 * ragged);
    let spill = (1.0 - paved) * (1.0 - smoothstep(0.0, spill_reach, -edge_in));
    if paved + spill < 0.01 {
        discard;
    }
    // A pit the structure digs (`ModelInfo::pit`) is open ground, not paved: the slab's
    // depth bias would lay a film over what shows down it. An airbase's shaft (icon 20)
    // is square; its pit reaches just past the square's corners.
    let pit = models[blueprint].pit;
    if pit.y > 0.0 {
        let hatch = (models[blueprint].icon & 0xFFu) == 20u;
        let hatch_half = (pit.y - 0.4) * 0.70710678 + 0.1;
        let inside = select(length(local) < pit.y, max(abs(local.x), abs(local.y)) < hatch_half, hatch);
        if inside {
            discard;
        }
    }

    // Precast slabs on a 4 m grid anchored to the world. Lots snap to 12 m, so
    // the slabs meet the kerb whole and neighbouring lots pave as one yard.
    let slab_uv = wp / 4.0;
    let slab = floor(slab_uv);
    let tone = hash21(slab + vec2<f32>(17.0, 3.0));
    let warm = hash21(slab + vec2<f32>(5.0, 41.0));
    let to_joint = (0.5 - abs(fract(slab_uv) - 0.5)) * 4.0;
    let joint_d = min(to_joint.x, to_joint.y);
    let joint = (1.0 - smoothstep(0.04, 0.04 + px, joint_d)) * (1.0 - far);
    // Sealed expansion joints on the 12 m build cells.
    let to_cell = (0.5 - abs(fract(wp / BUILD_CELL_M) - 0.5)) * BUILD_CELL_M;
    let cell_joint = (1.0 - smoothstep(0.06, 0.06 + px, min(to_cell.x, to_cell.y))) * (1.0 - far);

    let grain = textureSample(noise_map, repeat_sampler, wp / 1.7).b;
    let broad = textureSample(noise_map, repeat_sampler, wp / 37.0).a;
    let blot = textureSample(noise_map, repeat_sampler, wp / 9.0 + vec2<f32>(0.37, 0.61)).a;

    var albedo = vec3<f32>(0.19, 0.182, 0.165);
    albedo *= 0.9 + 0.2 * tone;
    albedo = mix(albedo, albedo * vec3<f32>(1.05, 1.0, 0.92), warm * 0.5);
    albedo *= 0.93 + 0.14 * grain;
    // Weathering: broad grime anchored to the world, so it runs across neighbouring
    // lots instead of outlining each one, and the odd oil blot on the apron.
    albedo *= 1.0 - 0.22 * smoothstep(0.35, 0.75, broad);
    albedo *= 1.0 - 0.35 * smoothstep(0.7, 0.78, blot);
    albedo *= 1.0 - joint * 0.45 - (far * 0.04);
    albedo *= 1.0 - cell_joint * 0.3;

    // The kerb is only a worn arris: lots side by side pave as one yard, not tiles.
    let kerb = paved * (1.0 - smoothstep(0.25, 0.25 + px, edge_in));

    // Team marks: an L painted in each corner of the lot, worn by traffic.
    let corner = in.half_m - abs(local);
    let arm = min(3.0, in.half_m * 0.3);
    let inset = 0.7;
    let stroke = 0.28;
    let along = step(inset, min(corner.x, corner.y)) * step(max(corner.x, corner.y), inset + arm);
    let across = (1.0 - smoothstep(inset + stroke, inset + stroke + px, min(corner.x, corner.y)));
    let paint = along * across * smoothstep(0.25, 0.55, grain + 0.3) * (1.0 - far * 0.5);
    albedo = mix(albedo, team * 0.4 + 0.03, paint * 0.55);

    // Grit spilled off the kerb: concrete dust, so where it falls on the next lot
    // it does not draw a seam.
    albedo = mix(albedo, vec3<f32>(0.18, 0.17, 0.15) * (0.85 + 0.3 * grain), 1.0 - paved);

    // Contact shadow where the building meets the slab.
    let contact = mix(0.5, 1.0, smoothstep(-0.2, 2.8, sd));
    albedo *= contact;

    // Wet near the waterline.
    let wet = 1.0 - smoothstep(0.0, 0.8, above);
    albedo *= 1.0 - 0.4 * wet;

    let eye = globals.camera.xyz;
    let dist = distance(eye, in.world);
    let base_n = terrain_normal(in.world.xy, clamp(dist * 0.004, 4.0, 24.0));
    // Each slab sits a hair off level; the kerb's arris catches the light.
    let tilt = (vec2<f32>(tone, warm) - 0.5) * 0.035 * (1.0 - far);
    let bevel = sign(local) * step(abs(local.yx), abs(local.xy)) * kerb * 0.15;
    let nrm = normalize(base_n + vec3<f32>(tilt + bevel, 0.0));
    var m: Pbr;
    m.albedo = albedo;
    m.metallic = 0.0;
    m.roughness = mix(0.9, 0.35, wet);
    m.emissive = vec3<f32>(0.0);
    if nanite {
        let lot = nanite_lot(plate, px, grain, contact);
        m.albedo = lot.xyz;
        m.roughness = lot.w;
        m.metallic = mix(0.5, 0.85, plate.z);
        m.emissive = nanite_lot_glow(local, in.half_m, plate, build, px);
    }
    let view = normalize(eye - in.world);
    var color = shade_pbr_vis(m, nrm, view, globals.sun.xyz, sun_shadow(in.world, base_n), screen_ao(in.clip.xy));
    color += local_lights(m, in.world, nrm, view);

    var alpha = paved * 0.97 + spill * 0.4;
    alpha *= smoothstep(0.0, 0.12, above);
    alpha *= mix(0.35, 1.0, build);
    if ghost != 0u {
        color = mix(color, team, 0.22);
        alpha *= 0.38;
    }
    if alpha < 0.012 {
        discard;
    }
    color = apply_fog_of_war(color, in.world.xy);
    return vec4<f32>(apply_haze(color, in.world, eye), clamp(alpha, 0.0, 0.97));
}

struct TrackOut {
    @builtin(position) clip: vec4<f32>,
    // x metres along the path (world-anchored, so stretches join up), y metres across.
    // A footprint's: metres from the sole's middle, x heel to toe.
    @location(0) uv: vec2<f32>,
    @location(1) world: vec3<f32>,
    // x half gauge, y width, z fade, w a footprint's half length
    @location(2) shape: vec4<f32>,
    // A footprint's heel-to-toe direction on the ground.
    @location(3) axis: vec2<f32>,
}

// How far past its sole a giant's print reaches, as a share of the sole's width: the lip
// of soil it squeezed out, the cracks and the clods it threw.
const PRINT_SPILL: f32 = 0.45;

// The ground between two of a footprint's grid vertices is not straight: how far it bulges
// above the straight edge from `xy` to `xy + 2 d`, or back to `xy - 2 d`.
fn print_bulge(xy: vec2<f32>, h: f32, d: vec2<f32>) -> f32 {
    let ahead = terrain_height(xy + d) - 0.5 * (h + terrain_height(xy + 2.0 * d));
    let behind = terrain_height(xy - d) - 0.5 * (h + terrain_height(xy - 2.0 * d));
    return max(max(ahead, behind), 0.0);
}

// A giant's footprint (`half_gauge` < 0, the sole's corner cut; renderer `titan_fx`): one
// pressed sole from heel to toe, and the ground it heaped and broke round it. Its hundred
// metres of ground is rarely flat, so it is drawn on a grid of PRINT_GRID cells a side with
// every vertex on the ground (one flat quad was cut through by any hill under it), each
// lifted by the most the ground bulges between it and its neighbours.
@vertex
fn vs_print(@builtin(vertex_index) vertex: u32, @builtin(instance_index) instance: u32) -> TrackOut {
    let m = track_marks[instance];
    let age = (globals.camera.w - m.start) / max(m.life, 0.001);
    var out: TrackOut;
    out.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
    let run = m.end_xy - m.start_xy;
    let len = length(run);
    if age < 0.0 || age >= 1.0 || len < 0.001 || m.half_gauge >= 0.0 {
        return out;
    }
    let along = run / len;
    let across = vec2<f32>(-along.y, along.x);
    let mid = (m.start_xy + m.end_xy) * 0.5;
    let ext = vec2<f32>(len, m.width) * 0.5 + m.width * PRINT_SPILL;
    // Too small on screen to see: the whole grid is dropped, not a vertex at a time.
    let centre = globals.view_proj * vec4<f32>(mid, terrain_height(mid), 1.0);
    if ext.y * globals.lod.x / max(centre.w, 1.0) < 2.0 {
        return out;
    }
    var corners = array<vec2<u32>, 6>(
        vec2<u32>(0u, 0u), vec2<u32>(1u, 0u), vec2<u32>(1u, 1u),
        vec2<u32>(0u, 0u), vec2<u32>(1u, 1u), vec2<u32>(0u, 1u),
    );
    let cell = vertex / 6u;
    let ij = vec2<u32>(cell % PRINT_GRID, cell / PRINT_GRID) + corners[vertex % 6u];
    let c = vec2<f32>(ij) / f32(PRINT_GRID) * 2.0 - 1.0;
    let xy = mid + along * c.x * ext.x + across * c.y * ext.y;
    // Half a cell each way, and half the diagonal the cell's triangles share.
    let dx = along * ext.x / f32(PRINT_GRID);
    let dy = across * ext.y / f32(PRINT_GRID);
    let h = terrain_height(xy);
    let lift = max(max(print_bulge(xy, h, dx), print_bulge(xy, h, dy)), print_bulge(xy, h, dx + dy));
    let world = vec3<f32>(xy, h + lift + 0.15);
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    out.clip.z += 0.00002 * out.clip.w + 0.02;
    out.uv = c * ext;
    out.world = world;
    out.shape = vec4<f32>(m.half_gauge, m.width, 1.0 - smoothstep(0.55, 1.0, age), len * 0.5);
    out.axis = along;
    return out;
}

@vertex
fn vs_track(@location(0) corner: vec2<f32>, @builtin(instance_index) instance: u32) -> TrackOut {
    let m = track_marks[instance];
    let age = (globals.camera.w - m.start) / max(m.life, 0.001);
    var out: TrackOut;
    out.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
    let run = m.end_xy - m.start_xy;
    let len = length(run);
    if age < 0.0 || age >= 1.0 || len < 0.001 {
        return out;
    }
    let along = run / len;
    let across = vec2<f32>(-along.y, along.x);
    let mid = (m.start_xy + m.end_xy) * 0.5;
    // A little overlap lengthwise, so a turning vehicle leaves no wedges of clean ground.
    let ext = vec2<f32>(len * 0.5 + m.width * 0.2, m.half_gauge + m.width * 0.5 + 0.1);
    let xy = mid + along * corner.x * ext.x + across * corner.y * ext.y;
    let world = vec3<f32>(xy, terrain_height(xy));
    let clip = globals.view_proj * vec4<f32>(world, 1.0);
    if ext.y * globals.lod.x / max(clip.w, 1.0) < 2.0 {
        return out;
    }
    out.clip = clip;
    out.clip.z += 0.00002 * out.clip.w + 0.02;
    out.uv = vec2<f32>(dot(xy, along), corner.y * ext.y);
    out.world = world;
    out.shape = vec4<f32>(m.half_gauge, m.width, 1.0 - smoothstep(0.55, 1.0, age), len * 0.5);
    out.axis = along;
    return out;
}

// A giant's footprint, as a dent in the ground lit by the sun. The sole's own outline (a
// pad with its corners cut at 45 degrees) pressed metres deep, deeper at the heel, its
// wall slumped and crumbling at the rim; the soil it pushed out heaped in a lip round it;
// the floor packed flat and ribbed by the sole's tread; cracks running out through the
// ground and clods thrown over it. All in metres, `p` from the sole's middle, x heel to toe.
struct Print {
    // Half the sole's length and width, the corners' cut, how deep it sank.
    half: vec2<f32>,
    cut: f32,
    depth: f32,
    // Where p = 0 lies in the world, and the sole's heel-to-toe and across directions.
    mid: vec2<f32>,
    along: vec2<f32>,
    across: vec2<f32>,
}

// Distance to the sole's outline, negative inside.
fn sole_distance(p: vec2<f32>, g: Print) -> f32 {
    let a = abs(p);
    let q = a - g.half;
    let pad = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0);
    let corner = (a.x + a.y - (g.half.x + g.half.y - g.cut)) * 0.70710678;
    return max(pad, corner);
}

// The outline as the ground took it: the rim crumbles in and out a little.
fn print_edge(p: vec2<f32>, g: Print) -> f32 {
    let world = g.mid + g.along * p.x + g.across * p.y;
    let coarse = textureSampleLevel(noise_map, repeat_sampler, world / 17.0, 0.0).b - 0.5;
    let fine = textureSampleLevel(noise_map, repeat_sampler, world / 3.5, 0.0).a - 0.5;
    return sole_distance(p, g) + (coarse * 0.9 + fine * 0.35) * g.depth * 0.45;
}

// Height of the ground in the print, metres (0 the ground round it).
fn print_height(p: vec2<f32>, g: Print) -> f32 {
    let sd = print_edge(p, g);
    let wall = g.depth * 0.5;
    // The heel strikes first and hardest; the tread's ribs stand in the packed floor.
    let heel = clamp(-p.x / g.half.x, -1.0, 1.0);
    let rib = sin(p.x * 6.2831853 / max(g.half.x * 0.09, 0.5)) * 0.5 + 0.5;
    let floor = -g.depth * (1.0 + 0.14 * heel) + rib * g.depth * 0.018;
    let pit = 1.0 - smoothstep(-wall, 0.0, sd);
    // The lip: soil squeezed out from under the sole, highest just past the wall.
    let reach = g.half.y * PRINT_SPILL * 1.2;
    let lip = smoothstep(-wall * 0.2, wall * 0.6, sd) * (1.0 - smoothstep(wall * 0.6, reach, sd));
    return floor * pit + lip * g.depth * 0.5;
}

// Porter-Duff over: `top` (colour, alpha) laid on `under`, straight alpha.
fn print_over(top: vec4<f32>, under: vec4<f32>) -> vec4<f32> {
    let a = top.a + under.a * (1.0 - top.a);
    let c = (top.rgb * top.a + under.rgb * under.a * (1.0 - top.a)) / max(a, 0.0001);
    return vec4<f32>(c, a);
}

fn footprint(in: TrackOut) -> vec4<f32> {
    var g: Print;
    g.half = vec2<f32>(in.shape.w, in.shape.y * 0.5);
    g.cut = -in.shape.x;
    g.depth = in.shape.y * 0.085;
    g.along = in.axis;
    g.across = vec2<f32>(-in.axis.y, in.axis.x);
    let p = in.uv;
    g.mid = in.world.xy - g.along * p.x - g.across * p.y;

    let h = print_height(p, g);
    let e = max(g.depth * 0.06, 0.2);
    let gx = print_height(p + vec2<f32>(e, 0.0), g) - print_height(p - vec2<f32>(e, 0.0), g);
    let gy = print_height(p + vec2<f32>(0.0, e), g) - print_height(p - vec2<f32>(0.0, e), g);
    let slope = (g.along * gx + g.across * gy) / (2.0 * e);
    let normal = normalize(vec3<f32>(-slope, 1.0));

    // The print lies in the ground's slope: the sun in that frame (x heel to toe, y across,
    // z out of the ground), so a dent on a hillside is lit as one.
    let up = terrain_normal(in.world.xy, 8.0);
    let flat_along = vec3<f32>(g.along, 0.0);
    let t = normalize(flat_along - up * dot(up, flat_along));
    let world_sun = normalize(globals.sun.xyz);
    let sun = vec3<f32>(dot(world_sun, t), dot(world_sun, cross(up, t)), dot(world_sun, up));
    // A slope turned from the sun is in its own shade: nothing on it to pick out.
    let facing = smoothstep(0.02, 0.15, sun.z);
    // The sun on it, over the sun on the bare slope; the wall on the sun's side shades the
    // floor under it (a short march toward the sun over the print's own heights).
    let rise = max(sun.z, 0.05) / max(length(sun.xy), 0.001);
    let toward = normalize(sun.xy + vec2<f32>(0.0001, 0.0));
    let span = min(g.depth * 1.5 / rise, g.half.y * 1.6);
    var lit = 1.0;
    for (var i = 1; i <= 8; i++) {
        let t = span * f32(i) / 8.0;
        let over = print_height(p + toward * t, g) - (h + t * rise);
        lit = min(lit, 1.0 - smoothstep(0.0, g.depth * 0.08, over));
    }
    let sd = print_edge(p, g);
    let inside = 1.0 - smoothstep(-g.depth * 0.5, 0.0, sd);
    // Skylight is cut off low in the pit, by the walls.
    let ao = 1.0 - 0.4 * inside * (1.0 - smoothstep(0.0, g.depth * 2.5, -sd));
    let direct = max(dot(normal, sun), 0.0) * lit / max(sun.z, 0.05);
    let in_sun = sun_shadow(in.world, up);
    // Packed earth on the floor is a shade darker than loose ground.
    var k = mix(1.0, direct, 0.8 * in_sun * facing) * ao * mix(1.0, 0.74, inside);

    var col = vec4<f32>(0.0);
    if k < 1.0 {
        col = vec4<f32>(vec3<f32>(0.025, 0.02, 0.015), min(1.0 - k, 0.92));
    } else {
        col = vec4<f32>(vec3<f32>(0.22, 0.185, 0.13), min((k - 1.0) * 0.7, 0.5));
    }

    // Fine dust blown out from under the sole settles pale round the print.
    let dust = smoothstep(-g.depth * 0.3, 0.0, sd) * (1.0 - smoothstep(0.0, g.half.y * PRINT_SPILL, sd));
    col = print_over(col, vec4<f32>(0.24, 0.2, 0.15, dust * 0.18));

    // Cracks running out from the rim, each ray its own length.
    let around = atan2(p.y / g.half.y, p.x / g.half.x) / 6.2831853 + 0.5;
    let n = textureSampleLevel(noise_map, repeat_sampler, in.world.xy / 9.0, 0.0).ba;
    let rays = 11.0;
    let ray = around * rays + (n.x - 0.5) * 0.9;
    let reach = g.half.y * mix(0.4, 1.3, hash11(floor(ray) + 3.1 * g.cut));
    let out_of = max(sd, 0.0);
    // Each crack is widest at the rim and runs out to nothing.
    let taper = 1.0 - smoothstep(0.0, reach, out_of);
    let crack = (1.0 - smoothstep(0.03, 0.07, abs(fract(ray) - 0.5) / max(taper, 0.05)))
        * taper * smoothstep(-g.depth * 0.2, g.depth * 0.4, sd);
    col = print_over(vec4<f32>(0.02, 0.016, 0.012, crack * 0.85), col);

    // Clods thrown out over the ground, fewer farther out: a dark side and a lit crown.
    let spill = g.half.y * PRINT_SPILL;
    let fine = textureSampleLevel(noise_map, repeat_sampler, in.world.xy / 4.5, 0.0).ba;
    let thrown = smoothstep(0.0, spill * 0.3, sd) * (1.0 - smoothstep(spill * 0.3, spill, sd));
    let clod = smoothstep(0.8 - thrown * 0.15, 0.86 - thrown * 0.15, fine.x) * thrown;
    let crown = select(0.03, 0.26, fine.y > 0.5);
    col = print_over(vec4<f32>(vec3<f32>(crown, crown * 0.84, crown * 0.6), clod * 0.8), col);

    // The print fades out at the quad's edge, however the lip and clods fall.
    let edge = 1.0 - smoothstep(0.8, 1.0, max(abs(p.x) / (g.half.x + spill), abs(p.y) / (g.half.y + spill)));
    let alpha = col.a * in.shape.z * edge;
    if alpha < 0.01 {
        return vec4<f32>(0.0);
    }
    let shade = apply_fog_of_war(col.rgb, in.world.xy);
    return vec4<f32>(apply_haze(shade, in.world, globals.camera.xyz), clamp(alpha, 0.0, 0.92));
}

@fragment
fn fs_print(in: TrackOut) -> @location(0) vec4<f32> {
    if in.world.z < globals.map.z {
        return vec4<f32>(0.0);
    }
    return footprint(in);
}

@fragment
fn fs_track(in: TrackOut) -> @location(0) vec4<f32> {
    if in.world.z < globals.map.z {
        discard;
    }
    let off = abs(abs(in.uv.y) - in.shape.x);
    let n = textureSample(noise_map, repeat_sampler, in.world.xy / 9.0).ba;
    // Pressed earth under each track, broken up by the ground, with the bite of the cleats along it.
    let rut = 1.0 - smoothstep(in.shape.y * 0.5 - 0.12, in.shape.y * 0.5 + 0.04, off + (n.x - 0.5) * 0.12);
    let cleat = 0.62 + 0.38 * smoothstep(0.35, 0.5, abs(fract(in.uv.x * 1.7) - 0.5) * 2.0);
    let alpha = rut * cleat * (0.3 + n.y * 0.35) * in.shape.z;
    if alpha < 0.01 {
        discard;
    }
    let earth = apply_fog_of_war(vec3<f32>(0.05, 0.04, 0.03), in.world.xy);
    return vec4<f32>(apply_haze(earth, in.world, globals.camera.xyz), alpha);
}

// ---- Ore veins -------------------------------------------------------------
// The ore under each field as solid tubes and nodules deep underground
// (renderer::ore_vein_mesh). Positions are xy in metres and z metres below the
// surface; the vertex shader hangs them under the terrain. Drawn additively
// with no depth test, only while the mine survey is up: the ground turns to
// glass and the deposits glow through it in the materials red-orange.

struct VeinPush {
    // 0..1: the survey highlight; time in seconds.
    highlight: f32,
    time: f32,
}

var<immediate> vein_push: VeinPush;

struct VeinOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) depth: f32,
}

@vertex
fn vs_vein(@location(0) pos: vec3<f32>, @location(1) normal: vec3<f32>) -> VeinOut {
    let world = vec3<f32>(pos.xy, terrain_height(pos.xy) + pos.z);
    var out: VeinOut;
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    out.world = world;
    out.normal = normal;
    out.depth = -pos.z;
    return out;
}

@fragment
fn fs_vein(in: VeinOut) -> @location(0) vec4<f32> {
    let hi = clamp(vein_push.highlight, 0.0, 1.0);
    if hi < 0.01 {
        discard;
    }
    let n = normalize(in.normal);
    let v = normalize(globals.camera.xyz - in.world);
    let sun = max(dot(n, globals.sun.xyz), 0.0);
    // Round: lit side, dark side, and a bright rim where the tube turns away.
    let rim = pow(1.0 - abs(dot(n, v)), 2.2);
    let ore = vec3<f32>(1.0, 0.147, 0.047);
    // Ore glinting in bands that drift through the rock.
    let band = 0.5 + 0.5 * sin(in.world.x * 0.07 + in.world.y * 0.05 + in.depth * 0.11 - vein_push.time * 1.6);
    let body = ore * (0.18 + 0.55 * sun + 0.25 * band);
    let glow = ore * rim * 1.6 + vec3<f32>(1.0, 0.7, 0.5) * pow(rim, 6.0) * 0.6;
    // The deeper, the dimmer, so depth reads.
    let fade = clamp(1.25 - in.depth / 500.0, 0.35, 1.0);
    return vec4<f32>((body + glow) * fade * hi * 2.2, 0.0);
}

// ---- A Regency lot (`PAD_NANITE`) --------------------------------------------------------
// Not a paved slab: black machined discs set in graphite. A big hub lies under the building,
// a graphite thread runs round it with beads strung on it, each smaller than the one before,
// and graphite lines run out to smaller discs spread out towards the lot's corners (one or two
// of those never laid). Every disc is ringed in graphite inside, circle within circle.

// A disc's inlay `t` metres from its middle, radius `r`: x 1 on its graphite rings (the rim,
// one half way in, and on a small disc a boss in the middle), y the dark cut beside them.
fn lot_rings(t: f32, r: f32, boss: bool, px: f32) -> vec2<f32> {
    let rim_w = clamp(0.09 * r, 0.2, 0.9);
    var d = abs(t - (r - rim_w * 0.5)) - rim_w * 0.5;
    d = min(d, abs(t - r * 0.58) - max(0.035 * r, 0.06));
    if boss {
        d = min(d, min(t - r * 0.2, abs(t - r * 0.36) - max(0.025 * r, 0.05)));
    }
    let inlay = 1.0 - smoothstep(-px * 0.5, px * 0.5, d);
    let cut = 1.0 - smoothstep(0.0, max(0.03 * r, 0.05) + px, d);
    return vec2<f32>(inlay, cut * (1.0 - inlay));
}

// Metres inside a line `w` wide from `a` to `b` (negative outside).
fn lot_line(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, w: f32) -> f32 {
    let ab = b - a;
    let k = clamp(dot(p - a, ab) / max(dot(ab, ab), 1e-4), 0.0, 1.0);
    return w - length(p - a - ab * k);
}

// The piece of the lot under `local`: x metres inside it (negative on bare ground), y its id
// (0..1), z how much of it is graphite inlay there, w the dark cut beside a ring.
// The graphite inlay: the Regency machinery's (regency.wgsl `REG_WORKS`), polished a
// little lighter so it still reads against the black discs.
const REG_LOT_WORKS: vec3<f32> = vec3<f32>(0.13, 0.12, 0.107);

fn lot_plate(local: vec2<f32>, half_m: f32, seed: f32, px: f32) -> vec4<f32> {
    let h = max(half_m, 1.0);
    let spin = (hash11(seed * 7.0) - 0.5) * 0.6;
    let t0 = length(local);
    // The hub.
    let hub_r = h * 0.62;
    var best = hub_r - t0;
    var at = local;
    var r = hub_r;
    var id = seed;
    var disc = true;
    var boss = false;
    // The thread round it, and the beads on it between the spokes: three a quarter, each
    // smaller than the last, going round (on a small lot only the big one).
    let orbit_r = h * 0.76;
    let thread = clamp(h * 0.022, 0.15, 0.55) - abs(t0 - orbit_r);
    if thread > best {
        best = thread;
        id = hash11(seed * 3.0 + 0.5);
        disc = false;
    }
    for (var i = 0; i < 12; i++) {
        let j = i % 3;
        if h < 9.0 && j != 0 {
            continue;
        }
        let a = spin + f32(i / 3) * 1.5707963 + (f32(j) - 1.0) * 0.36;
        let br = h * (0.1 - 0.024 * f32(j));
        let c = vec2<f32>(cos(a), sin(a)) * orbit_r;
        let e = br - distance(local, c);
        if e > best {
            best = e;
            at = local - c;
            r = br;
            id = hash11(seed * 11.0 + f32(i));
            disc = true;
            boss = true;
        }
    }
    // Out towards the corners: a line to a smaller disc, a bead threaded half way along.
    let line_w = clamp(h * 0.02, 0.18, 0.5);
    for (var k = 0; k < 4; k++) {
        let kf = f32(k);
        if k % 2 == 1 && hash11(seed * 5.0 + kf) < 0.3 {
            continue;
        }
        let a = spin + (kf + 0.5) * 1.5707963 + (hash11(seed * 17.0 + kf) - 0.5) * 0.5;
        let dir = vec2<f32>(cos(a), sin(a));
        let sr = h * (0.15 + 0.08 * hash11(seed * 23.0 + kf));
        let reach = (h - 0.25 - sr) / max(abs(dir.x), abs(dir.y));
        let c = dir * reach;
        let ln = lot_line(local, dir * hub_r, c, line_w);
        if ln > best {
            best = ln;
            id = hash11(seed * 29.0 + kf);
            disc = false;
        }
        let e = sr - distance(local, c);
        if e > best {
            best = e;
            at = local - c;
            r = sr;
            id = hash11(seed * 31.0 + kf);
            disc = true;
            boss = true;
        }
        let gap = reach - sr - orbit_r - h * 0.1;
        let bead_r = min(h * 0.045, gap * 0.35);
        if bead_r > 0.3 {
            let bc = dir * (reach - sr - gap * 0.5);
            let eb = bead_r - distance(local, bc);
            if eb > best {
                best = eb;
                at = local - bc;
                r = bead_r;
                id = hash11(seed * 37.0 + kf);
                disc = true;
                boss = false;
            }
        }
    }
    if !disc {
        return vec4<f32>(best, id, 1.0, 0.0);
    }
    let rings = lot_rings(length(at), r, boss, px);
    return vec4<f32>(best, id, rings.x, rings.y);
}

// A piece's look: black with its own shade and a worn, lighter arris; its rings and the
// lines between are graphite. Albedo in xyz, roughness in w.
fn nanite_lot(plate: vec4<f32>, px: f32, grain: f32, contact: f32) -> vec4<f32> {
    let tone = 0.85 + 0.3 * hash11(plate.y * 53.0);
    var albedo = vec3<f32>(0.022, 0.023, 0.026) * tone * (0.85 + 0.3 * grain);
    let arris = 1.0 - smoothstep(0.05, 0.3 + px, plate.x);
    albedo = mix(albedo, vec3<f32>(0.07, 0.072, 0.078), arris * 0.7);
    let works = REG_LOT_WORKS * (0.8 + 0.3 * grain);
    albedo = mix(albedo, works, plate.z) * (1.0 - 0.7 * plate.w);
    return vec4<f32>(albedo * contact, mix(0.55, 0.4, plate.z));
}

// Its light: while the building goes up, bands of violet with red in them run out along
// the pieces' edges and ring cuts from the middle; built, they are dark. Dim: a machined floor under a
// working swarm, not a lamp (the plate of a hull on it mirrors it).
fn nanite_lot_glow(local: vec2<f32>, half_m: f32, plate: vec4<f32>, build: f32, px: f32) -> vec3<f32> {
    let time = globals.camera.w;
    let seam = max((1.0 - smoothstep(0.04, 0.12 + px, plate.x)) * step(0.0, plate.x), plate.w);
    let working = 1.0 - smoothstep(0.85, 1.0, build);
    let r = length(local) / max(half_m, 1.0);
    let band = pow(0.5 + 0.5 * sin(r * 9.0 - time * 3.2), 6.0);
    let tint = mix(LOT_VIOLET, LOT_RED, band * 0.5);
    return tint * seam * working * (0.04 + 0.5 * band);
}
