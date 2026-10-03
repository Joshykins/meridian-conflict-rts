//!use bindings
//!use surface
// Adjacency conduits (renderer/adjacency_links.rs): one slim cable per link, laid on the
// ground from the provider's lot centre to the neighbour's. Its core is lit in the
// link's colour with slow pulses running from the provider into the neighbour; a bound
// pair's sheath (`LINK_BOUND`) is banded amber and black. It follows `terrain_height`,
// and a new link's cable runs out from the provider over `SETTLE_SECONDS`.

//!rust crate::renderer::adjacency_links::LinkInstance
struct LinkInstance {
    provider_at: vec2<f32>,
    consumer_at: vec2<f32>,
    // Render time the cable began to run out.
    start: f32,
    flags: u32,
    // The core's light, sRGB 0xRRGGBB.
    rgb: u32,
    // Where its pulses are in their cycle, 0..1.
    phase: f32,
}

@group(1) @binding(0) var<storage, read> links: array<LinkInstance>;

// The cable's half width up close, and at least this much per metre from the eye so
// it stays a line a pixel or two wide when the camera pulls back.
const CABLE_HALF_M: f32 = 0.35;
const CABLE_HALF_PER_M: f32 = 0.0015;
// A highlighted cable is this much wider.
const HIGHLIGHT_WIDEN: f32 = 1.35;
// Its height over its half width: a little flattened, as a cable lying on the ground is.
const CABLE_RISE: f32 = 0.75;
// It lies this far off the ground, plus a little per metre of distance so it never
// fights the terrain for depth.
const LIFT_M: f32 = 0.04;
// The lit core is this share of the width.
const CORE: f32 = 0.38;
// Pulses run at this speed, this far apart; a bound pair's bands are this long.
const PULSE_SPEED: f32 = 5.0;
const PULSE_GAP_M: f32 = 9.0;
const BAND_M: f32 = 1.6;

struct VsOut {
    @builtin(position) @invariant clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    // Metres from the provider's end, and across the cable -1..1.
    @location(1) along: f32,
    @location(2) across: f32,
    @location(3) @interpolate(flat) link: u32,
    // The cable's side, for its normal.
    @location(4) @interpolate(flat) side: vec2<f32>,
}

const QUAD = array<vec2<f32>, 6>(
    vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
);

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {
    let l = links[ii];
    var out: VsOut;
    let t = clamp((globals.camera.w - l.start) / SETTLE_SECONDS, 0.0, 1.0);
    let reach = t * t * (3.0 - 2.0 * t);
    let len = distance(l.provider_at, l.consumer_at);
    if reach <= 0.001 || len < 0.5 {
        out.clip = vec4<f32>(0.0);
        return out;
    }
    let dir = (l.consumer_at - l.provider_at) / len;
    let side = vec2<f32>(-dir.y, dir.x);
    // Per segment: the near half of the cable (across -1..0) and the far (0..1).
    let seg = vi / 12u;
    let k = vi % 12u;
    let q = QUAD[k % 6u];
    let s = len * reach * (f32(seg) + q.x) / f32(LINK_SEGMENTS);
    let across = select(q.y - 1.0, q.y, k >= 6u);
    let mid = l.provider_at + dir * s;
    let far = distance(globals.camera.xyz, vec3<f32>(mid, terrain_height(mid)));
    var half = max(CABLE_HALF_M, far * CABLE_HALF_PER_M);
    if (l.flags & LINK_HIGHLIGHT) != 0u {
        half *= HIGHLIGHT_WIDEN;
    }
    let xy = mid + side * across * half;
    let up = half * CABLE_RISE * sqrt(max(1.0 - across * across, 0.0));
    let world = vec3<f32>(xy, terrain_height(xy) + LIFT_M + far * 0.0004 + up);
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    out.world = world;
    out.along = s;
    out.across = across;
    out.link = ii;
    out.side = side;
    return out;
}

// A `0xRRGGBB` sRGB colour as linear light.
fn srgb_word(rgb: u32) -> vec3<f32> {
    let c = vec3<f32>(f32((rgb >> 16u) & 0xFFu), f32((rgb >> 8u) & 0xFFu), f32(rgb & 0xFFu)) / 255.0;
    return pow(c, vec3<f32>(2.2));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let l = links[in.link];
    let eye = globals.camera.xyz;
    let p = in.world;
    let v = normalize(eye - p);
    // A round cable: the normal turns from the ground up over the top and down again.
    let a = clamp(in.across, -1.0, 1.0);
    let n = normalize(vec3<f32>(in.side * a, sqrt(max(1.0 - a * a, 0.0)) + 0.15));
    let bright = (l.flags & LINK_HIGHLIGHT) != 0u;
    let tone = srgb_word(l.rgb);
    let time = globals.camera.w;

    // A dark armoured sheath; a bound pair's banded amber and black.
    var albedo = vec3<f32>(0.075, 0.08, 0.088);
    var metal = 0.6;
    var rough = 0.45;
    if (l.flags & LINK_BOUND) != 0u {
        let band = step(0.5, fract(in.along / BAND_M));
        albedo = mix(vec3<f32>(0.62, 0.36, 0.035), vec3<f32>(0.025, 0.024, 0.022), band);
        metal = 0.1;
        rough = 0.55;
    }
    // The lit core along the top, and the pulses running from the provider (`along`
    // rising), each a bright head trailing back.
    let px = fwidth(a);
    let core = 1.0 - smoothstep(CORE, CORE + px, abs(a));
    let pulse = pow(fract((in.along - time * PULSE_SPEED) / PULSE_GAP_M - l.phase), 4.0);
    let glow = core * (select(0.5, 1.0, bright) + pulse * select(2.0, 4.5, bright));

    var m: Pbr;
    m.albedo = mix(albedo, tone * 0.3, core);
    m.metallic = mix(metal, 0.0, core);
    m.roughness = rough;
    m.emissive = tone * glow * 3.0;
    let shadow = sun_shadow(p, n);
    let sky_vis = 0.85 * screen_ao(in.clip.xy);
    var color = shade_pbr_vis(m, n, v, globals.sun.xyz, shadow, sky_vis);
    color += local_lights(m, p, n, v);
    color = apply_fog_of_war(color, p.xy);
    color = apply_haze(color, p, eye);
    if (l.flags & LINK_PLANNED) != 0u {
        // A placement ghost's: see-through, its light added over the scene.
        let alpha = 0.45;
        return vec4<f32>(mix(color, tone * 0.6, 0.35) * alpha + m.emissive * 0.5, alpha);
    }
    return vec4<f32>(color, 1.0);
}
