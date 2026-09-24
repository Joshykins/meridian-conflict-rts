//!use bindings
// Things painted over the terrain: crater/scorch stains, ore fields,
// and structure foundations. These are decals only; the
// ground under them is never deformed.

// Mirrors mc_sim::mirror::StainInstance.
struct Stain {
    pos: vec2<f32>,
    radius: f32,
    strength_seed: u32,
}

@group(1) @binding(0) var<storage, read> stains: array<Stain>;

// One stretch of track marks: both tracks of a vehicle between two points of
// its path. Written once into a ring; fades out with age.
struct TrackMark {
    start_xy: vec2<f32>,
    end_xy: vec2<f32>,
    // Centre line to the middle of each track, and one track's width.
    half_gauge: f32,
    width: f32,
    start: f32,
    life: f32,
}

@group(1) @binding(1) var<storage, read> track_marks: array<TrackMark>;

struct StainOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world: vec3<f32>,
    @location(2) @interpolate(flat) strength_seed: u32,
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
    return out;
}

@fragment
fn fs_stain(in: StainOut) -> @location(0) vec4<f32> {
    if (in.strength_seed & STAIN_CRATER) != 0u {
        return crater(in);
    }
    if (in.strength_seed & STAIN_MOLTEN) != 0u {
        return molten(in);
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
        discard;
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
    var lit = shade_pbr(m, nrm, normalize(eye - in.world), globals.sun.xyz, sun_shadow(in.world, base_n));
    lit += local_lights(m, in.world, nrm, normalize(eye - in.world));
    lit = mix(lit, vec3<f32>(0.01, 0.009, 0.008), 0.55 + lobe * 0.2);
    return vec4<f32>(apply_haze(lit, in.world, eye), alpha);
}

// A stain with this bit set is a crater blown in round a wreck (renderer/wreck_fx.rs).
const STAIN_CRATER: u32 = 0x80000000u;
// Ground an electric bore's discharge left molten (renderer/bore_fx.rs). Its low byte is
// the heat left, 255 fresh to 0 cold; the renderer rewrites it every frame.
const STAIN_MOLTEN: u32 = 0x40000000u;

// Molten ground: a pool that glows white-yellow when fresh, then orange, then a dull red,
// while a dark glassy crust closes over it from the rim in. Late on the glow shows only
// in the cracks of the crust. The sim's charcoal scorch lies under it and stays after.
fn molten(in: StainOut) -> vec4<f32> {
    let heat = f32(in.strength_seed & 0xFFu) / 255.0;
    let seed = f32((in.strength_seed >> 8u) & 0x7FFFu);
    let spun = rot_z(vec3<f32>(in.uv, 0.0), seed * 0.37).xy;
    let eye = globals.camera.xyz;
    let dist = distance(eye, in.world);
    let n = textureSample(noise_map, repeat_sampler, in.world.xy / 3.0 + vec2<f32>(seed * 0.013, seed * 0.007)).rg;
    let lobe = value_noise2(spun * 2.4 + vec2<f32>(seed * 0.02, 1.7), 1.0);
    // Radius in pool radii (the patch reaches 1.35 of them), ragged.
    let r = length(in.uv) * 1.35 + (lobe - 0.5) * 0.4 + (n.x - 0.5) * 0.2;
    let body = 1.0 - smoothstep(0.65, 1.05, r);
    if body < 0.02 {
        discard;
    }
    // The crust grows in from the rim as the heat goes.
    let crust = smoothstep(0.0, 0.6, (1.0 - heat) * 1.3 + r * 0.45 + (n.y - 0.5) * 0.35 - 0.2);
    // Cracks in the crust: thin lines where a cell field crosses its middle, faded out
    // before they could shrink under a pixel.
    let cells = value_noise2(in.world.xy * 0.42 + vec2<f32>(seed * 0.1, seed * 0.03), 1.0);
    let line = max(fwidth(cells) * 1.5, 0.03);
    let crack = (1.0 - smoothstep(0.0, line, abs(cells - 0.5))) * (1.0 - smoothstep(250.0, 700.0, dist));
    let open = max(1.0 - crust, crack * 0.8);
    // Hottest down the middle of the gouge; the rim is the first to go dark. Only the
    // middle of a fresh track is white-hot: the rest is orange, then red as it cools.
    let core = 1.0 - smoothstep(0.0, 0.95, r);
    let t = heat * mix(0.6, 0.97, core);
    let hot = mix(
        mix(vec3<f32>(0.5, 0.03, 0.006), vec3<f32>(1.0, 0.22, 0.02), smoothstep(0.1, 0.55, t)),
        vec3<f32>(1.0, 0.72, 0.36),
        smoothstep(0.78, 1.0, t),
    );
    let glow = hot * (pow(heat, 1.8) * 2.4 * (0.3 + 0.7 * core) + heat * 0.25) * open;

    let base_n = terrain_normal(in.world.xy, clamp(dist * 0.004, 4.0, 24.0));
    let relief = clamp(1.0 - dist / 600.0, 0.2, 1.0);
    let nrm = normalize(base_n + vec3<f32>((n - 0.5) * relief * 0.4, 0.0));
    var albedo = mix(vec3<f32>(0.018, 0.016, 0.015), vec3<f32>(0.04, 0.035, 0.03), n.y);
    albedo = apply_fog_of_war(albedo, in.world.xy);
    var m: Pbr;
    m.albedo = albedo;
    m.metallic = 0.0;
    // Glassy slag: the crust takes a sheen.
    m.roughness = 0.45;
    m.emissive = vec3<f32>(0.0);
    let v = normalize(eye - in.world);
    var lit = shade_pbr(m, nrm, v, globals.sun.xyz, sun_shadow(in.world, base_n));
    lit += local_lights(m, in.world, nrm, v);
    let alpha = body * clamp(0.55 + heat * 0.45, 0.0, 0.95);
    return vec4<f32>(apply_haze(lit + glow, in.world, eye), alpha);
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
    var lit = shade_pbr(m, nrm, v, globals.sun.xyz, sun_shadow(in.world, base_n));
    lit += local_lights(m, in.world, nrm, v);
    // The bottom of the bowl sees less sky.
    lit *= mix(1.0, 0.55, (1.0 - smoothstep(0.0, 0.85, r)) * inside);
    let alpha = clamp(max(inside * 0.92, ejecta * 0.75), 0.0, 0.92);
    if alpha < 0.02 {
        discard;
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
    // or selecting a mine, or Ctrl held), plus 2 when a mine the viewer has
    // seen is working this field.
    let raw = stains[first].radius;
    let tapped = raw >= 1.5;
    let hi = clamp(raw - select(0.0, 2.0, tapped), 0.0, 1.0);
    let pulse = 0.5 + 0.5 * sin(globals.camera.w * 3.2 - length(p - stains[first].pos) * 0.03);

    // The outline: all a field is in play, a faint red-orange line (pale gold
    // once tapped); bright and wider while highlighted.
    let rim_w = 0.8 + far * 2.2 + hi * (1.0 + far * 3.0);
    let rim = 1.0 - smoothstep(rim_w * 0.5, rim_w * 0.5 + aa * 1.5, abs(sd + rim_w * 0.6));
    // The materials red-orange (hud::MASS, 0xFF6B3D), linear.
    let ore_col = vec3<f32>(1.0, 0.147, 0.047);
    let worked = select(0.0, 1.0, tapped);
    let rim_col = apply_fog_of_war(ore_col, p) * (0.6 + far * 0.6 + worked * 0.5 + hi * (1.2 + 0.5 * pulse));
    if hi < 0.01 {
        // A field a seen mine is working is filled lightly as well, so it
        // stands apart on the strategic view; the rest are outlines only.
        let inside = 1.0 - smoothstep(-aa, aa, sd);
        let a = max(rim * mix(0.4, 0.8, worked), inside * worked * (0.08 + 0.14 * far));
        if a < 0.01 {
            discard;
        }
        let col = mix(apply_fog_of_war(ore_col * 0.6, p), rim_col, rim);
        return vec4<f32>(apply_haze(col, in.world, eye), a);
    }

    // Highlighted: the field turns to thin glass over the ore, which the
    // vein geometry (fs_vein) shows underneath; a faint hatch drifts across it.
    let inside = 1.0 - smoothstep(-aa, aa, sd);
    let hatch_w = mix(4.0, 40.0, far);
    let hatch = smoothstep(0.82, 1.0, sin((p.x + p.y) / hatch_w * 3.14159 + globals.camera.w * 1.5) * 0.5 + 0.5);
    let fill = inside * (0.06 + 0.1 * hatch + 0.06 * pulse);
    let col = mix(apply_fog_of_war(ore_col * 0.7, p), rim_col, rim);
    let alpha = clamp(max(fill, rim * 0.9) * hi + rim * 0.4 * (1.0 - hi), 0.0, 0.9);
    if alpha < 0.01 {
        discard;
    }
    return vec4<f32>(apply_haze(col, in.world, eye), alpha);
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
    // Nothing is paved on the seabed: an offshore rig stands on its piles.
    let above = in.world.z - globals.map.z;
    if above < 0.0 {
        discard;
    }
    let owner = in.packed & 7u;
    let build = f32((in.packed >> 8u) & 0xFFu) / 255.0;
    let blueprint = in.packed >> 16u;
    let ghost = (in.packed >> 4u) & 1u;
    let team = globals.team_colors[owner].rgb;

    let wp = in.world.xy;
    let local = in.uv * in.half_m;
    // Metres per pixel, for edges that stay sharp near and do not shimmer far.
    let px = max(length(fwidth(wp)), 0.004);
    let far = smoothstep(0.06, 0.35, px);

    // Metres inside the kerb; negative out on the dirt. The slab runs a hand's
    // breadth past the lot so two lots side by side overlap rather than meet
    // at half cover each, which let the ground show through as a dark seam.
    let edge_in = in.half_m + 0.3 - max(abs(local.x), abs(local.y));
    let ragged = textureSample(noise_map, repeat_sampler, wp / 5.0).b;
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
    let sd = pad_mesh_sd(in.uv, blueprint);
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
    let view = normalize(eye - in.world);
    var color = shade_pbr(m, nrm, view, globals.sun.xyz, sun_shadow(in.world, base_n));
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
    @location(0) uv: vec2<f32>,
    @location(1) world: vec3<f32>,
    // x half gauge, y width, z fade
    @location(2) shape: vec3<f32>,
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
    let reach = m.half_gauge + m.width * 0.5 + 0.1;
    // A little overlap lengthwise, so a turning vehicle leaves no wedges of clean ground.
    let xy = (m.start_xy + m.end_xy) * 0.5 + along * corner.x * (len * 0.5 + m.width * 0.2) + across * corner.y * reach;
    let world = vec3<f32>(xy, terrain_height(xy));
    let clip = globals.view_proj * vec4<f32>(world, 1.0);
    if reach * globals.lod.x / max(clip.w, 1.0) < 2.0 {
        return out;
    }
    out.clip = clip;
    out.clip.z += 0.00002 * out.clip.w + 0.02;
    out.uv = vec2<f32>(dot(xy, along), corner.y * reach);
    out.world = world;
    out.shape = vec3<f32>(m.half_gauge, m.width, 1.0 - smoothstep(0.55, 1.0, age));
    return out;
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
