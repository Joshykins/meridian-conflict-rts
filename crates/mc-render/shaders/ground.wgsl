//!use bindings
// Things painted over the terrain: crater/scorch stains, mass-deposit cracks,
// structure foundations and the water surface. These are decals only; the
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
    let seed = f32(s.strength_seed >> 8u);
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
    let strength = f32(in.strength_seed & 0xFFu) / 255.0;
    let seed = f32(in.strength_seed >> 8u);
    let d = length(in.uv);
    // Ragged edge: the noise texture perturbs the radius.
    let n = textureSample(noise_map, repeat_sampler, in.world.xy / 23.0 + vec2<f32>(seed * 0.013)).ba;
    let edge = d + (n.x - 0.5) * 0.55;
    let mask = (1.0 - smoothstep(0.35, 1.0, edge)) * (0.75 + n.y * 0.5);
    let alpha = clamp(mask * (0.35 + strength * 1.1), 0.0, 0.93);
    if alpha < 0.01 {
        discard;
    }
    let char_color = apply_fog_of_war(vec3<f32>(0.025, 0.022, 0.02), in.world.xy);
    return vec4<f32>(apply_haze(char_color, in.world, globals.camera.xyz), alpha);
}

// Mass deposit: stone split by the ore. Dark in the cut, a dull mineral seam
// along the lips — never a neon glow. The lot is the extractor's 2x2 square
// with rounded corners; seed yaws the cracks so sites differ.
@vertex
fn vs_deposit(@location(0) grid: vec2<f32>, @builtin(instance_index) instance: u32) -> StainOut {
    let s = stains[instance];
    // Axis-aligned with the build grid; a little past the lot so the ring can anti-alias.
    let reach = s.radius * 1.06;
    let xy = s.pos + grid * reach;
    let ground = terrain_height(xy);
    let world = vec3<f32>(xy, ground);
    var out: StainOut;
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    out.clip.z += 0.00002 * out.clip.w + 0.02;
    let px = s.radius * globals.lod.x / max(out.clip.w, 1.0);
    if px < 1.1 {
        out.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
    }
    out.uv = grid * 1.06;
    out.world = world;
    out.strength_seed = s.strength_seed;
    return out;
}

// Rounded box of half-size `b`, corner radius `r`. `b` is the outer bound,
// so the straight edges sit on the 2x2 lot and the corners pull in.
fn sd_rounded_box(p: vec2<f32>, b: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - b + vec2<f32>(r);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - r;
}

// Distance in the plane to a jagged split along `dir`. Wander stays shallow
// so this stays a real gap — angular distance smears once the path bends.
fn crack_gap(uv: vec2<f32>, dir: vec2<f32>, seed: f32, tag: f32) -> vec2<f32> {
    let along = dot(uv, dir);
    let across = uv.x * dir.y - uv.y * dir.x;
    let wander = (value_noise2(vec2<f32>(along * 2.6 + seed, tag), 1.0) - 0.5) * 0.10;
    let kink = value_noise2(vec2<f32>(along * 6.2 + seed * 1.4, tag * 1.7), 1.0) - 0.5;
    let jag = wander + sign(kink) * kink * kink * 0.055;
    return vec2<f32>(along, abs(across - jag));
}

// One crack: a jagged split from the pit, with a secondary fork. Returns
// (sharp lip, broader scar, deep cut).
fn crack_arm(uv: vec2<f32>, seed: f32, i: f32) -> vec3<f32> {
    let h = hash11(seed * 1.71 + i * 17.3);
    let yaw = seed * 0.41 + i * 0.68 + h * 0.5;
    let dir = vec2<f32>(cos(yaw), sin(yaw));
    let gap = crack_gap(uv, dir, seed, yaw);
    let reach = 0.78 + h * 0.18;
    let fade = (1.0 - smoothstep(reach * 0.70, reach, gap.x)) * step(0.0, gap.x);
    let half = mix(0.026, 0.006, clamp(gap.x / reach, 0.0, 1.0));
    let lip = (1.0 - smoothstep(0.0, half, gap.y)) * fade;
    let scar = (1.0 - smoothstep(0.0, half * 3.4, gap.y)) * fade;
    let cut = (1.0 - smoothstep(0.0, half * 0.42, gap.y)) * fade;

    var out = vec3<f32>(lip, scar, cut);
    if h > 0.28 && gap.x > 0.16 && gap.x < reach * 0.92 {
        let byaw = yaw + (h * 2.0 - 1.0) * 0.72;
        let bdir = vec2<f32>(cos(byaw), sin(byaw));
        let bgap = crack_gap(uv, bdir, seed + 3.1, byaw);
        let bfade = smoothstep(0.16, 0.28, bgap.x) * (1.0 - smoothstep(reach * 0.5, reach * 0.88, bgap.x));
        let bhalf = mix(0.020, 0.005, clamp(bgap.x, 0.0, 1.0));
        out.x = max(out.x, (1.0 - smoothstep(0.0, bhalf, bgap.y)) * bfade);
        out.y = max(out.y, (1.0 - smoothstep(0.0, bhalf * 3.2, bgap.y)) * bfade);
        out.z = max(out.z, (1.0 - smoothstep(0.0, bhalf * 0.4, bgap.y)) * bfade);
    }
    if h > 0.55 && gap.x > 0.32 && gap.x < reach * 0.7 {
        let hyaw = yaw + (h - 0.55) * 1.4 - 0.35;
        let hdir = vec2<f32>(cos(hyaw), sin(hyaw));
        let hgap = crack_gap(uv, hdir, seed + 8.7, hyaw);
        let hfade = smoothstep(0.32, 0.40, hgap.x) * (1.0 - smoothstep(reach * 0.56, reach * 0.72, hgap.x));
        out.x = max(out.x, (1.0 - smoothstep(0.0, 0.007, hgap.y)) * hfade * 0.7);
        out.y = max(out.y, (1.0 - smoothstep(0.0, 0.018, hgap.y)) * hfade * 0.45);
    }
    return out;
}

@fragment
fn fs_deposit(in: StainOut) -> @location(0) vec4<f32> {
    if in.world.z < globals.map.z {
        discard;
    }
    let seed = f32(in.strength_seed >> 8u);
    let uv = in.uv;
    let corner = 0.22;
    let sd = sd_rounded_box(uv, vec2<f32>(1.0), corner);
    if sd > 0.04 {
        discard;
    }

    var lip = 0.0;
    var scar = 0.0;
    var cut = 0.0;
    for (var i = 0; i < 9; i++) {
        let arm = crack_arm(uv, seed, f32(i));
        lip = max(lip, arm.x);
        scar = max(scar, arm.y);
        cut = max(cut, arm.z);
    }

    // Keep the fissures inside the lot, fading before they meet the ring.
    let inside = 1.0 - smoothstep(-0.10, -0.012, sd);
    lip *= inside;
    scar *= inside;
    cut *= inside;

    // Collapsed pit at the centre: dark, not a glowing core.
    let r = length(uv);
    let pit = 1.0 - smoothstep(0.03, 0.22, r);
    let pit_edge = smoothstep(0.05, 0.12, r) * (1.0 - smoothstep(0.16, 0.24, r));
    lip = max(lip, pit_edge);
    scar = max(scar, pit * 0.85);
    cut = max(cut, pit * 0.55);

    let grit = noise_varied(in.world.xy, 4.4);
    let grit2 = noise_varied(in.world.xy + vec2<f32>(seed, seed * 0.7), 19.0);
    lip *= 0.72 + grit.b * 0.4;
    scar *= 0.7 + grit.b * 0.35;
    // Chips of broken stone along the scar.
    let chips = step(0.78, grit2.a) * scar * 0.55;

    // Screen-constant rounded-square ring so the site stays readable at mid zoom.
    let ring = (1.0 - smoothstep(0.0, fwidth(sd) * 2.4 + 0.010, abs(sd)))
        * (1.0 - smoothstep(0.018, 0.045, sd));

    let alpha = max(max(scar * 0.82, lip * 0.95), max(cut * 0.9, ring * 0.7));
    if alpha < 0.018 {
        discard;
    }

    let stone = vec3<f32>(0.055, 0.05, 0.046);
    let deep = vec3<f32>(0.018, 0.016, 0.014);
    let lip_col = vec3<f32>(0.12, 0.1, 0.088);
    let mineral = vec3<f32>(0.28, 0.2, 0.11);
    var color = mix(stone, deep, clamp(scar * 0.85 + pit * 0.35, 0.0, 1.0));
    color = mix(color, lip_col, lip * 0.55);
    color = mix(color, deep, cut * 0.75);
    color = mix(color, mineral, lip * 0.22 + chips * 0.35);
    // A thin, muted mark on the ring — mass-green, not amber.
    color = mix(color, vec3<f32>(0.22, 0.38, 0.24), ring * 0.55);

    color = apply_fog_of_war(color, in.world.xy);
    return vec4<f32>(apply_haze(color, in.world, globals.camera.xyz), clamp(alpha, 0.0, 0.94));
}

struct PadOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world: vec3<f32>,
    @location(2) @interpolate(flat) packed: u32,
    @location(3) @interpolate(flat) half_m: f32,
}

fn sd_box(p: vec2<f32>, b: vec2<f32>) -> f32 {
    let d = abs(p) - b;
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0);
}

// Axis-aligned box with a different chamfer on each corner (++ +− −+ −−).
fn sd_lot_box(p: vec2<f32>, b: vec2<f32>, cuts: vec4<f32>) -> f32 {
    var sd = sd_box(p, b);
    let s = b.x + b.y;
    sd = max(sd, (p.x + p.y) - (s - cuts.x));
    sd = max(sd, (p.x - p.y) - (s - cuts.y));
    sd = max(sd, (-p.x + p.y) - (s - cuts.z));
    sd = max(sd, (-p.x - p.y) - (s - cuts.w));
    return sd;
}

fn cardinal(i: u32) -> vec2<f32> {
    switch i & 3u {
        case 0u: { return vec2<f32>(1.0, 0.0); }
        case 1u: { return vec2<f32>(-1.0, 0.0); }
        case 2u: { return vec2<f32>(0.0, 1.0); }
        default: { return vec2<f32>(0.0, -1.0); }
    }
}

fn pad_extent(e: vec2<f32>, along: f32, depth: f32) -> vec2<f32> {
    return select(vec2<f32>(depth, along), vec2<f32>(along, depth), abs(e.y) > 0.5);
}

// Extractor well: one poured slab in each cell of the 2x2, pulled back from
// the centre so the crack pit and the fissures along the grid axes stay open.
fn well_pad_sd(uv: vec2<f32>, seed: f32) -> f32 {
    var sd = 1e3;
    for (var i = 0u; i < 4u; i++) {
        let sx = select(-1.0, 1.0, (i & 1u) == 0u);
        let sy = select(-1.0, 1.0, (i & 2u) == 0u);
        // Outer half of each 12 m cell. Inner edge ~0.35 of the lot so the
        // pit (0.22) and the inner cracks stay open; outer edge just inside
        // the 2x2 so neighbouring wells do not tile as a square.
        let center = vec2<f32>(sx, sy) * 0.66;
        let b = vec2<f32>(
            mix(0.28, 0.34, hash11(seed + f32(i) * 1.7)),
            mix(0.28, 0.34, hash11(seed + f32(i) * 2.4)),
        );
        let cap = min(b.x, b.y) * 0.45;
        let cuts = vec4<f32>(
            mix(0.04, cap, hash11(seed + 3.1 + f32(i))),
            mix(0.04, cap, hash11(seed + 5.7 + f32(i))),
            mix(0.04, cap, hash11(seed + 8.2 + f32(i))),
            mix(0.04, cap, hash11(seed + 11.0 + f32(i))),
        );
        sd = min(sd, sd_lot_box(uv - center, b, cuts));
    }
    return sd;
}

// Poured lot inside the build-grid cell (uv in [-1, 1]). Each of the four
// sides is a different length, the rear corners are cut deeper than the face,
// and an L-bay or a notch breaks the last hint of a square. Large lots stay
// fuller so the building still sits on concrete.
fn pad_sd(uv: vec2<f32>, seed: f32, half_m: f32) -> f32 {
    let big = smoothstep(8.0, 42.0, half_m);
    let min_e = mix(0.48, 0.84, big);
    var front = mix(min_e + 0.14, 0.97, hash11(seed + 0.41));
    var rear = mix(min_e, 0.88, hash11(seed + 1.27));
    var left = mix(min_e, 0.97, hash11(seed + 2.19));
    var right = mix(min_e, 0.97, hash11(seed + 3.08));
    // If the spans landed close, squeeze one so this cannot be a square.
    let gap = mix(0.30, 0.14, big);
    if abs((front + rear) - (left + right)) < gap {
        let squeeze = mix(0.24, 0.10, big);
        if hash11(seed + 3.91) > 0.5 {
            rear = max(min_e, rear - squeeze);
        } else {
            left = max(min_e, left - squeeze);
        }
    }

    let ox = (front - rear) * 0.5;
    let oy = (right - left) * 0.5;
    let hx = (front + rear) * 0.5;
    let hy = (left + right) * 0.5;
    let p = uv - vec2<f32>(ox, oy);
    let b = vec2<f32>(hx, hy);

    let cap = min(min(hx, hy) * 0.52, mix(0.44, 0.24, big));
    // Face corners stay tight; rear corners take a real bite.
    let cuts = vec4<f32>(
        mix(0.02, cap * 0.35, hash11(seed + 4.7)),
        mix(0.02, cap * 0.35, hash11(seed + 5.9)),
        mix(cap * 0.40, cap, hash11(seed + 7.3)),
        mix(cap * 0.40, cap, hash11(seed + 8.1)),
    );
    var sd = sd_lot_box(p, b, cuts);

    // Rear L-bay, offset to one side, filling leftover dirt behind the slab.
    let bay_w = mix(0.30, 0.70, hash11(seed + 14.2)) * hy;
    let leftover_r = max(0.97 - rear, 0.06);
    let bay_d = leftover_r * mix(0.65, 1.0, hash11(seed + 15.6));
    let bay_y = (hash11(seed + 16.9) * 2.0 - 1.0) * max(hy - bay_w, 0.0);
    let bay_cut = mix(0.02, 0.14, hash11(seed + 18.4));
    sd = min(sd, sd_lot_box(
        p - vec2<f32>(-hx + bay_d * 0.15, bay_y),
        vec2<f32>(bay_d * 0.5 + 0.04, bay_w),
        vec4<f32>(bay_cut),
    ));

    // Notch in one long side, so the edge is not a single straight run.
    let e = cardinal(select(2u, 3u, hash11(seed + 20.1) > 0.5));
    let along = vec2<f32>(-e.y, e.x);
    let notch_d = mix(0.10, 0.26, hash11(seed + 21.5)) * hy;
    let notch_w = mix(0.18, 0.46, hash11(seed + 22.8)) * hx;
    let notch_mid = (hash11(seed + 24.0) - 0.5) * hx * 0.55;
    sd = max(sd, -sd_box(
        p - (e * (hy - notch_d * 0.28) + along * notch_mid),
        pad_extent(e, notch_w, notch_d * 0.7),
    ));
    return sd;
}

// A structure's lot: a poured foundation slab inside the build-grid cell.
@vertex
fn vs_pad(@location(0) grid: vec2<f32>, @builtin(instance_index) instance: u32) -> PadOut {
    let s = stains[instance];
    // A little past the lot so the constructed edge can anti-alias; no spin.
    let reach = s.radius * 1.06;
    let xy = s.pos + grid * reach;
    let world = vec3<f32>(xy, terrain_height(xy));
    var out: PadOut;
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    out.clip.z += 0.00002 * out.clip.w + 0.02;
    let px = s.radius * globals.lod.x / max(out.clip.w, 1.0);
    if px < 1.2 {
        out.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
    }
    out.uv = grid * 1.06;
    out.world = world;
    out.packed = s.strength_seed;
    out.half_m = s.radius;
    return out;
}

@fragment
fn fs_pad(in: PadOut) -> @location(0) vec4<f32> {
    let owner = in.packed & 7u;
    let well = (in.packed >> 3u) & 1u;
    let build = f32((in.packed >> 8u) & 0xFFu) / 255.0;
    let seed = f32((in.packed >> 16u) & 0xFFu);
    let ghost = (in.packed >> 24u) & 1u;
    let team = globals.team_colors[owner].rgb;

    let sd = select(pad_sd(in.uv, seed, in.half_m), well_pad_sd(in.uv, seed), well == 1u);
    // Sharp form edge, not a ragged falloff.
    let aa = 0.014;
    let body = 1.0 - smoothstep(0.0, aa, sd);
    if body < 0.01 {
        discard;
    }

    let n = noise_varied(in.world.xy + vec2<f32>(seed * 1.7, seed * 1.1), 5.5);
    let n2 = noise_varied(in.world.xy, 19.0);
    let plate = textureSample(
        panel_map,
        repeat_sampler,
        in.world.xy / 16.0 + vec2<f32>(seed * 0.021, seed * 0.013),
    );
    let pour = n.b * 0.4 + n2.a * 0.35 + value_noise2(in.world.xy, 3.4) * 0.25;
    var albedo = mix(vec3<f32>(0.13, 0.122, 0.112), vec3<f32>(0.22, 0.205, 0.188), pour);
    albedo *= 0.62 + plate.b * 0.38;

    // Form boards left in the pour, world-anchored so neighbouring lots join.
    let board = abs(fract(in.world.x / 2.6 + seed * 0.02) - 0.5);
    albedo *= 1.0 - (1.0 - smoothstep(0.46, 0.5, board)) * 0.1;

    // Expansion joints on the 12 m build cells that make up the lot.
    let local = in.uv * in.half_m;
    let g = abs(fract(local / BUILD_CELL_M + 0.5) - 0.5) * BUILD_CELL_M;
    let joint = (1.0 - smoothstep(0.0, 0.16, min(g.x, g.y))) * (1.0 - smoothstep(-0.07, -0.03, sd));
    albedo = mix(albedo, albedo * 0.42, joint);

    // Raised form curb and a darker steel edge plate.
    let curb = (1.0 - smoothstep(-0.07, -0.028, sd)) * body;
    let steel = (1.0 - smoothstep(-0.026, -0.004, sd)) * body;
    albedo = mix(albedo, vec3<f32>(0.28, 0.265, 0.24), curb * 0.55);
    albedo = mix(albedo, vec3<f32>(0.12, 0.115, 0.11), steel * 0.85);

    // Painted lot mark: a stripe inset from the form edge, following the plan.
    let stripe = (1.0 - smoothstep(-0.165, -0.148, sd)) * smoothstep(-0.122, -0.105, sd);
    albedo = mix(albedo, team * 0.55, stripe * 0.7);

    let eye = globals.camera.xyz;
    let dist = distance(eye, in.world);
    let base_n = terrain_normal(in.world.xy, clamp(dist * 0.004, 4.0, 24.0));
    // Fake a bevel on the curb so the edge reads as formwork, not a paper cutout.
    let nrm = normalize(base_n + vec3<f32>(in.uv * curb * 0.55, 0.0)
        + vec3<f32>((plate.xy - 0.5) * 0.35 * clamp(1.0 - dist / 700.0, 0.0, 1.0), 0.0));
    var m: Pbr;
    m.albedo = albedo;
    m.metallic = 0.08 + steel * 0.35;
    m.roughness = mix(0.78, 0.42, steel);
    m.emissive = vec3<f32>(0.0);
    var color = shade_pbr(m, nrm, normalize(eye - in.world), globals.sun.xyz, sun_shadow(in.world, base_n));

    var alpha = body * 0.94;
    alpha *= mix(0.4, 1.0, build);
    if ghost != 0u {
        color = mix(color, team, 0.22);
        alpha *= 0.38;
    }
    if alpha < 0.012 {
        discard;
    }
    color = apply_fog_of_war(color, in.world.xy);
    return vec4<f32>(apply_haze(color, in.world, eye), clamp(alpha, 0.0, 0.96));
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

struct WaterOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
}

@vertex
fn vs_water(@builtin(vertex_index) index: u32) -> WaterOut {
    // Two triangles covering the map, extended outward so the sea reaches the horizon.
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
    );
    let c = corners[index];
    let xy = c * globals.map.xy;
    var out: WaterOut;
    out.world = vec3<f32>(xy, globals.map.z);
    out.clip = globals.view_proj * vec4<f32>(out.world, 1.0);
    return out;
}

@fragment
fn fs_water(in: WaterOut) -> @location(0) vec4<f32> {
    let xy = in.world.xy;
    let depth = globals.map.z - terrain_height(xy);
    if depth <= 0.0 {
        discard;
    }
    let time = globals.camera.w;
    let eye = globals.camera.xyz;
    let v = normalize(eye - in.world);
    let dist = distance(eye, in.world);

    let calm = clamp(1.0 - dist / 6000.0, 0.0, 1.0);
    let w1 = textureSample(noise_map, repeat_sampler, xy / 41.0 + vec2<f32>(time * 0.021, time * 0.013)).xy - 0.5;
    let w2 = textureSample(noise_map, repeat_sampler, xy / 13.0 - vec2<f32>(time * 0.034, time * 0.027)).xy - 0.5;
    let n = normalize(vec3<f32>((w1 * 0.5 + w2 * 0.3) * calm, 1.0));

    let fresnel = 0.02 + 0.98 * pow(1.0 - max(dot(n, v), 0.0), 5.0);
    let r = reflect(-v, n);
    let sky = mix(vec3<f32>(0.5, 0.64, 0.86), vec3<f32>(0.2, 0.38, 0.72), clamp(r.z, 0.0, 1.0));
    let sun = pow(max(dot(r, globals.sun.xyz), 0.0), 400.0) * 40.0 * calm;
    let body = mix(vec3<f32>(0.05, 0.22, 0.26), vec3<f32>(0.01, 0.05, 0.12), clamp(depth / 35.0, 0.0, 1.0));
    var color = mix(body, sky, fresnel) + vec3<f32>(1.0, 0.95, 0.85) * sun;

    // Foam where the water meets the shore.
    let foam_noise = textureSample(noise_map, repeat_sampler, xy / 7.0 + vec2<f32>(time * 0.05, 0.0)).b;
    let foam = (1.0 - smoothstep(0.0, 1.6, depth)) * smoothstep(0.35, 0.7, foam_noise) * calm;
    color = mix(color, vec3<f32>(0.9), foam * 0.8);

    color = apply_fog_of_war(color, xy);
    color = apply_haze(color, in.world, eye);
    let alpha = clamp(depth / 5.0, 0.0, 1.0) * mix(0.72, 1.0, fresnel) + foam * 0.3;
    return vec4<f32>(color, clamp(alpha, 0.0, 1.0));
}
