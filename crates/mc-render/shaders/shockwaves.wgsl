//!use bindings
// A pressure front carrying softly lit dust and fine condensation.
// A conservative sphere mesh bounds an analytic, perfectly smooth front.

struct Shockwave {
    pos: vec3<f32>,
    start: f32,
    // x reach in metres, y lifetime seconds, z weapon colour (0 blue, 1 orange), w how hard the front is
    params: vec4<f32>,
    // Barrel direction for a muzzle blast; zero for an isotropic sphere.
    axis: vec3<f32>,
    _pad: f32,
    tint: vec4<f32>,
}

@group(1) @binding(0) var<storage, read> waves: array<Shockwave>;

// Must match `SHOCKWAVE_LAT` / `SHOCKWAVE_LON` in the renderer.
const LAT: u32 = 32u;
const LON: u32 = 64u;

struct WaveOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) age: f32,
    @location(2) strength: f32,
    @location(3) tint: vec3<f32>,
    @location(4) @interpolate(flat) center: vec3<f32>,
    @location(5) @interpolate(flat) reach: f32,
    @location(6) @interpolate(flat) axis: vec3<f32>,
    @location(7) @interpolate(flat) custom_tint: f32,
}

fn sphere_dir(lat: u32, lon: u32) -> vec3<f32> {
    let u = f32(lon) / f32(LON);
    let v = f32(lat) / f32(LAT);
    let theta = u * 2.0 * PI;
    let phi = v * PI;
    let s = sin(phi);
    return vec3<f32>(s * cos(theta), s * sin(theta), cos(phi));
}

@vertex
fn vs_shockwave(@builtin(vertex_index) v: u32, @builtin(instance_index) instance: u32) -> WaveOut {
    let e = waves[instance];
    let age = (globals.camera.w - e.start) / max(e.params.y, 0.001);
    var out: WaveOut;
    out.center = e.pos;
    out.axis = e.axis;
    out.custom_tint = e.tint.w;
    out.age = age;
    out.strength = e.params.w;
    out.reach = 0.0;
    if age < 0.0 || age >= 1.0 || e.params.x <= 0.0 {
        out.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
        return out;
    }

    let grow = 1.0 - (1.0 - age) * (1.0 - age);
    let reach = e.params.x * grow;
    if reach < 0.2 {
        out.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
        return out;
    }

    let quad = v / 6u;
    let corner = v % 6u;
    let lat = quad / LON;
    let lon = quad % LON;
    var dlat = 0u;
    var dlon = 0u;
    switch corner {
        case 1u, 3u: {
            dlon = 1u;
        }
        case 2u: {
            dlat = 1u;
        }
        case 4u: {
            dlat = 1u;
            dlon = 1u;
        }
        case 5u: {
            dlat = 1u;
        }
        default: {}
    }
    let dir = sphere_dir(lat + dlat, lon + dlon);
    // Enclose the analytic sphere even between mesh vertices.
    let world = e.pos + dir * reach * 1.015;
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    out.world = world;
    out.reach = reach;
    if e.params.z < 0.5 {
        // Energy: the front is the same blue as the slug.
        out.tint = vec3<f32>(0.32, 0.72, 1.0);
    } else {
        out.tint = vec3<f32>(0.93, 0.91, 0.86);
    }
    if e.tint.w > 0.5 { out.tint = e.tint.rgb; }
    return out;
}

// Blended projections avoid a pole/seam and keep the texture attached to the
// expanding front. Noise changes opacity, never the spherical silhouette.
fn wave_dust_noise(p: vec3<f32>, n: vec3<f32>, cell: f32) -> f32 {
    let weights = abs(n) / max(dot(abs(n), vec3<f32>(1.0)), 0.001);
    return value_noise2(p.yz, cell) * weights.x
        + value_noise2(p.zx + vec2<f32>(19.0, 7.0), cell) * weights.y
        + value_noise2(p.xy + vec2<f32>(43.0, 31.0), cell) * weights.z;
}

struct WavePixel {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
}

@fragment
fn fs_shockwave(in: WaveOut) -> WavePixel {
    let eye = globals.camera.xyz;
    let outside = length(eye - in.center) > in.reach;
    let mesh_facing = dot(in.world - in.center, eye - in.world);
    // Use only the back wall of the proxy, even when the camera enters it.
    // The analytic hit below supplies the visible wall and its correct depth.
    if mesh_facing > 0.0 {
        discard;
    }
    let uv = in.clip.xy / globals.scene.xy;
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let h = globals.inv_view_proj * vec4<f32>(ndc, 0.01, 1.0);
    let ray = normalize(h.xyz / h.w - eye);
    let oc = eye - in.center;
    let b = dot(oc, ray);
    let disc = b * b - dot(oc, oc) + in.reach * in.reach;
    if disc <= 0.0 { discard; }
    let root = sqrt(disc);
    let distance = select(-b + root, -b - root, outside);
    if distance <= 0.0 { discard; }
    let world = eye + ray * distance;
    let ground = max(terrain_height(world.xy), globals.map.z);
    if world.z < ground - 0.2 || effect_blocked(in.center, world) { discard; }

    let n = normalize(world - in.center);
    let facing = abs(dot(n, ray));
    let radius_px = in.reach * globals.lod.x / max(length(eye - in.center), 1.0);
    let bands = shockwave_bands(facing, radius_px);
    let axis_len = length(in.axis);
    var directional = 1.0;
    if axis_len > 0.5 {
        directional = smoothstep(-0.45, 0.65, dot(n, in.axis / axis_len));
    }
    let fade = shockwave_fade(in.age);
    let inset = 1.0 - sqrt(max(1.0 - facing * facing, 0.0));
    let width = clamp(1.8 / max(radius_px, 1.0), 0.007, 0.05);
    let skirt = exp(-pow((inset - width * 4.0) / (width * 4.5), 2.0))
        * smoothstep(0.0, width * 1.2, inset);
    // Keep a whisper of pressure haze when looking through the front.

    // Material coordinates travel with the wave. Unlike the old animated sine
    // patches, this produces soft, irregular dust without swimming or wobbling.
    let seed = hash21(in.center.xy + in.center.z) * 97.0;
    let tex = n * 72.0 + vec3<f32>(seed, seed * 0.73, seed * 1.31);
    let broad = wave_dust_noise(tex, n, 9.0);
    let detail = wave_dust_noise(tex, n, 2.4);
    let fine = wave_dust_noise(tex, n, 0.65);
    // Fade the smallest grain below pixel size instead of letting it sparkle.
    let fine_weight = 1.0 - smoothstep(0.3, 1.2, 72.0 / max(radius_px, 1.0));
    let density = smoothstep(0.20, 0.78, broad * 0.6 + detail * 0.4);
    let grain = mix(1.0, 0.55 + 0.9 * fine, fine_weight);
    let core = bands.x * (0.3 + 0.7 * density);
    let dust = skirt * (0.15 + 0.85 * density) * grain;
    let body = 0.038 * (0.65 + 0.35 * density);
    // Blue weapons (the commander's rail) keep a cooler front and a thinner dust skirt.
    let blue = in.tint.b > in.tint.r + 0.12;
    let dust_weight = select(0.46, 0.20, blue);
    let alpha = (body + core * 0.32 + dust * dust_weight)
        * directional * (0.65 + 0.35 * in.strength) * fade;
    // Dust is softly sunlit. Blue fronts stay pale and cool near the ground
    // instead of picking up earth colour.
    let near_ground = 1.0 - smoothstep(2.0, max(in.reach * 0.55, 8.0), world.z - ground);
    let light = 0.85 + 0.45 * max(dot(n, globals.sun.xyz), 0.0);
    let dust_high = select(vec3<f32>(0.73, 0.74, 0.72), vec3<f32>(0.82, 0.86, 0.92), blue);
    // Over open water the front lifts pale spray, not earth.
    let over_sea = step(terrain_height(world.xy), globals.map.z);
    let earth = mix(vec3<f32>(0.65, 0.59, 0.48), vec3<f32>(0.76, 0.80, 0.82), over_sea);
    let dust_low = select(earth, vec3<f32>(0.74, 0.80, 0.90), blue);
    let dust_color = mix(dust_high, dust_low, near_ground) * light;
    let core_mix = select(0.08, 0.62, blue);
    let core_color = mix(vec3<f32>(1.35, 1.32, 1.22), in.tint * 1.55, core_mix);
    let natural = mix(dust_color, core_color, core / max(core + dust * 1.3 + body, 0.001));
    let tint = in.tint * light * (1.0 + core * 0.6);
    let color = mix(natural, tint, in.custom_tint);
    let clip = globals.view_proj * vec4<f32>(world, 1.0);
    var out: WavePixel;
    out.color = vec4<f32>(color, alpha);
    out.depth = clip.z / clip.w;
    return out;
}
