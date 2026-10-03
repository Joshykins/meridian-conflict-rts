//!use bindings
//!use surface
// Adjacency conduits (renderer/adjacency_links.rs): one line per link along its path
// (adjacency_links/path.rs) from the provider's lot centre to the neighbour's: a slim
// cable whose core is lit in the link's colour, slow pulses running from the provider
// into the neighbour, and the pieces of its faction's look along it:
// - `LINK_LOOK_CLAMPED` (ARC): an armoured cable pinned by steel clamps, a junction box
//   with a lamp at each turn; a bound pair's cable is banded amber and black and its
//   clamps painted amber;
// - `LINK_LOOK_PLATED` (the Regency): a dark cable under lapped graphite plates whose
//   lips glow, between low faceted field nodes; a bound pair's lips and nodes burn amber.
// Everything follows `terrain_height`, and a new link runs out from the provider over
// `SETTLE_SECONDS`.

//!rust crate::renderer::adjacency_links::LinkInstance
struct LinkInstance {
    points: array<vec2<f32>, LINK_POINTS>,
    count: u32,
    length: f32,
    // Render time the line began to run out.
    start: f32,
    flags: u32,
    // The core's light, sRGB 0xRRGGBB.
    rgb: u32,
    // Where its pulses are in their cycle, 0..1.
    phase: f32,
}

@group(1) @binding(0) var<storage, read> links: array<LinkInstance>;

// The cable's half width up close, and at least this much per metre from the eye so it
// stays a line a pixel or two wide when the camera pulls back. The pieces along it grow
// with it a little, then shrink away into it: from far off, as in the strategic view,
// only the lit line shows, not a chain of blocks swollen to a building's size.
const CABLE_HALF_M: f32 = 0.35;
const CABLE_HALF_PER_M: f32 = 0.0015;
// The cable's widening (its half width over `CABLE_HALF_M`) at which the pieces stop
// growing, and at which they are gone.
const PIECE_GROW_MAX: f32 = 1.5;
const PIECE_GONE: f32 = 3.0;
// A highlighted line is this much wider.
const HIGHLIGHT_WIDEN: f32 = 1.35;
// The cable's height over its half width: a little flattened, as a cable lying on the
// ground is.
const CABLE_RISE: f32 = 0.75;
// It lies this far off the ground, plus a little per metre of distance so it never
// fights the terrain for depth; the pieces' sides reach this far below it.
const LIFT_M: f32 = 0.04;
const SINK_M: f32 = 0.2;
// The lit core is this share of the cable's width.
const CORE: f32 = 0.38;
// Pulses run at this speed, this far apart; a bound pair's bands are this long.
const PULSE_SPEED: f32 = 5.0;
const PULSE_GAP_M: f32 = 9.0;
const BAND_M: f32 = 1.6;
// Pieces come about this far apart: clamps, or the plates (every fourth a node).
const CLAMP_STEP_M: f32 = 7.0;
const PLATE_STEP_M: f32 = 4.0;
const NODE_EVERY: u32 = 4u;

const CABLE_VERTICES: u32 = LINK_SEGMENTS * 12u;
const PIECE_VERTICES: u32 = 48u;

// What a fragment belongs to.
const PART_CABLE: u32 = 0u;
const PART_CLAMP: u32 = 1u;
const PART_JUNCTION: u32 = 2u;
const PART_PLATE: u32 = 3u;
const PART_NODE: u32 = 4u;

struct VsOut {
    @builtin(position) @invariant clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    // The cable: metres from the provider's end, across it -1..1. A piece: metres along
    // and across it from its middle, and how far up its side (0..1).
    @location(1) local: vec3<f32>,
    // The cable's side, for its normal.
    @location(2) side: vec2<f32>,
    @location(3) @interpolate(flat) part: u32,
    @location(4) @interpolate(flat) link: u32,
    // A piece's top (1) or side (0); the piece's size over its size up close.
    @location(5) @interpolate(flat) top: u32,
    @location(6) @interpolate(flat) grow: f32,
}

const QUAD = array<vec2<f32>, 6>(
    vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
);

// Six-sided outlines, along and across, metres up close, counter-clockwise.
const CLAMP_OUTLINE = array<vec2<f32>, 6>(
    vec2<f32>(0.24, 0.0), vec2<f32>(0.24, 0.8), vec2<f32>(-0.24, 0.8),
    vec2<f32>(-0.24, 0.0), vec2<f32>(-0.24, -0.8), vec2<f32>(0.24, -0.8),
);
const JUNCTION_OUTLINE = array<vec2<f32>, 6>(
    vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    vec2<f32>(-1.0, 0.0), vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0),
);
const PLATE_OUTLINE = array<vec2<f32>, 6>(
    vec2<f32>(1.3, 0.0), vec2<f32>(0.45, 0.75), vec2<f32>(-1.0, 0.75),
    vec2<f32>(-1.3, 0.0), vec2<f32>(-1.0, -0.75), vec2<f32>(0.45, -0.75),
);
const NODE_OUTLINE = array<vec2<f32>, 6>(
    vec2<f32>(1.2, 0.0), vec2<f32>(0.6, 1.04), vec2<f32>(-0.6, 1.04),
    vec2<f32>(-1.2, 0.0), vec2<f32>(-0.6, -1.04), vec2<f32>(0.6, -1.04),
);

// A point of the path `s` metres from the provider, and the way it runs there.
struct PathPoint {
    at: vec2<f32>,
    dir: vec2<f32>,
}

fn path_at(ii: u32, s: f32) -> PathPoint {
    var out: PathPoint;
    let count = links[ii].count;
    var acc = 0.0;
    for (var i = 1u; i < count; i++) {
        let a = links[ii].points[i - 1u];
        let seg = links[ii].points[i] - a;
        let len = max(length(seg), 0.001);
        if s <= acc + len || i == count - 1u {
            out.at = a + seg * clamp((s - acc) / len, 0.0, 1.0);
            out.dir = seg / len;
            return out;
        }
        acc += len;
    }
    out.at = links[ii].points[0];
    out.dir = vec2<f32>(1.0, 0.0);
    return out;
}

// Metres along the path to its point `k`.
fn length_to(ii: u32, k: u32) -> f32 {
    var acc = 0.0;
    for (var i = 1u; i <= k; i++) {
        acc += distance(links[ii].points[i - 1u], links[ii].points[i]);
    }
    return acc;
}

fn half_width(at: vec2<f32>, flags: u32) -> f32 {
    let far = distance(globals.camera.xyz, vec3<f32>(at, terrain_height(at)));
    var half = max(CABLE_HALF_M, far * CABLE_HALF_PER_M);
    if (flags & LINK_HIGHLIGHT) != 0u {
        half *= HIGHLIGHT_WIDEN;
    }
    return half;
}

fn ground(xy: vec2<f32>) -> f32 {
    let far = distance(globals.camera.xy, xy);
    return terrain_height(xy) + LIFT_M + far * 0.0004;
}

struct Built {
    world: vec3<f32>,
    local: vec3<f32>,
    top: u32,
}

// A vertex `k` of a six-sided block at `c` running along `dir`: its outline scaled by
// `grow`, its top `h` high in the middle rising `tilt` per metre toward its back.
fn block(c: vec2<f32>, dir: vec2<f32>, outline: array<vec2<f32>, 6>, grow: f32, h: f32, tilt: f32, k: u32) -> Built {
    var out: Built;
    var o: vec2<f32>;
    var up = 0.0;
    if k < 12u {
        // The top: a fan of four triangles from the first corner.
        let tri = k / 3u;
        let corner = array<u32, 3>(0u, tri + 1u, tri + 2u)[k % 3u];
        o = outline[corner];
        up = 1.0;
        out.top = 1u;
    } else {
        let side = (k - 12u) / 6u;
        let q = QUAD[(k - 12u) % 6u];
        o = outline[(side + u32(q.x)) % 6u];
        up = q.y;
        out.top = 0u;
    }
    o *= grow;
    let xy = c + dir * o.x + vec2<f32>(-dir.y, dir.x) * o.y;
    let top = (h - tilt * o.x) * grow;
    out.world = vec3<f32>(xy, ground(xy) + mix(-SINK_M, top, up));
    out.local = vec3<f32>(o, up);
    return out;
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {
    let l = links[ii];
    var out: VsOut;
    out.link = ii;
    let t = clamp((globals.camera.w - l.start) / SETTLE_SECONDS, 0.0, 1.0);
    let reach = l.length * t * t * (3.0 - 2.0 * t);
    if reach <= 0.05 || l.count < 2u {
        out.clip = vec4<f32>(0.0);
        return out;
    }
    let plated = (l.flags >> LINK_LOOK_SHIFT) == LINK_LOOK_PLATED;
    var world: vec3<f32>;
    if vi < CABLE_VERTICES {
        // The cable: per stretch, its near half (across -1..0) and its far (0..1).
        let k = vi % 12u;
        let q = QUAD[k % 6u];
        let s = reach * (f32(vi / 12u) + q.x) / f32(LINK_SEGMENTS);
        let across = select(q.y - 1.0, q.y, k >= 6u);
        let p = path_at(ii, s);
        let side = vec2<f32>(-p.dir.y, p.dir.x);
        let half = half_width(p.at, l.flags);
        let xy = p.at + side * across * half;
        let up = half * CABLE_RISE * sqrt(max(1.0 - across * across, 0.0));
        world = vec3<f32>(xy, ground(xy) + up);
        out.local = vec3<f32>(s, across, 0.0);
        out.side = side;
        out.part = PART_CABLE;
    } else {
        let j = (vi - CABLE_VERTICES) / PIECE_VERTICES;
        let k = (vi - CABLE_VERTICES) % PIECE_VERTICES;
        var p: PathPoint;
        var shown = true;
        if j < LINK_PIECES {
            // Pieces spread evenly along the path, as far as it has run out.
            let step = select(CLAMP_STEP_M, PLATE_STEP_M, plated);
            let n = clamp(u32(round(l.length / step)), 1u, LINK_PIECES);
            let s = (f32(j) + 0.5) * l.length / f32(n);
            shown = j < n && s <= reach;
            p = path_at(ii, s);
            if !plated {
                out.part = PART_CLAMP;
            } else if j % NODE_EVERY == 1u {
                out.part = PART_NODE;
            } else {
                out.part = PART_PLATE;
            }
        } else {
            // A junction at each turn of the path.
            let turn = j - LINK_PIECES + 1u;
            shown = (l.flags & LINK_TURNS) != 0u && turn + 1u < l.count && length_to(ii, turn) <= reach;
            p.at = l.points[min(turn, LINK_POINTS - 1u)];
            p.dir = normalize(p.at - l.points[min(turn, LINK_POINTS - 1u) - 1u] + vec2<f32>(1e-5, 0.0));
            out.part = select(PART_JUNCTION, PART_NODE, plated);
        }
        let widen = half_width(p.at, l.flags) / CABLE_HALF_M;
        let grow = min(widen, PIECE_GROW_MAX) * (1.0 - smoothstep(PIECE_GROW_MAX, PIECE_GONE, widen));
        if !shown || grow <= 0.0 {
            out.clip = vec4<f32>(0.0);
            return out;
        }
        var b: Built;
        switch out.part {
            case PART_CLAMP: { b = block(p.at, p.dir, CLAMP_OUTLINE, grow, 0.36, 0.0, k); }
            case PART_JUNCTION: { b = block(p.at, p.dir, JUNCTION_OUTLINE, grow, 0.8, 0.0, k); }
            case PART_PLATE: { b = block(p.at, p.dir, PLATE_OUTLINE, grow, 0.36, 0.06, k); }
            default: { b = block(p.at, p.dir, NODE_OUTLINE, grow, 0.7, 0.0, k); }
        }
        world = b.world;
        out.local = b.local;
        out.top = b.top;
        out.grow = grow;
        out.side = vec2<f32>(-p.dir.y, p.dir.x);
    }
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    out.world = world;
    return out;
}

// A `0xRRGGBB` sRGB colour as linear light.
fn srgb_word(rgb: u32) -> vec3<f32> {
    let c = vec3<f32>(f32((rgb >> 16u) & 0xFFu), f32((rgb >> 8u) & 0xFFu), f32(rgb & 0xFFu)) / 255.0;
    return pow(c, vec3<f32>(2.2));
}

const HAZARD_AMBER: vec3<f32> = vec3<f32>(0.62, 0.36, 0.035);
const HAZARD_BLACK: vec3<f32> = vec3<f32>(0.025, 0.024, 0.022);
// The light a bound pair's Regency pieces burn with.
const BOUND_GLOW: vec3<f32> = vec3<f32>(1.0, 0.45, 0.05);

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let l = links[in.link];
    let eye = globals.camera.xyz;
    let p = in.world;
    let v = normalize(eye - p);
    let bright = (l.flags & LINK_HIGHLIGHT) != 0u;
    let bound = (l.flags & LINK_BOUND) != 0u;
    let plated = (l.flags >> LINK_LOOK_SHIFT) == LINK_LOOK_PLATED;
    let tone = srgb_word(l.rgb);
    let time = globals.camera.w;
    var n: vec3<f32>;
    var albedo: vec3<f32>;
    var metal = 0.6;
    var rough = 0.45;
    var emissive = vec3<f32>(0.0);
    let lit = select(1.0, 2.2, bright);
    if in.part == PART_CABLE {
        // Round: the normal turns from the ground up over the top and down again.
        let a = clamp(in.local.y, -1.0, 1.0);
        n = normalize(vec3<f32>(in.side * a, sqrt(max(1.0 - a * a, 0.0)) + 0.15));
        if plated {
            albedo = vec3<f32>(0.035, 0.033, 0.04);
            metal = 0.85;
            rough = 0.38;
        } else if bound {
            albedo = mix(HAZARD_AMBER, HAZARD_BLACK, step(0.5, fract(in.local.x / BAND_M)));
            metal = 0.1;
            rough = 0.55;
        } else {
            albedo = vec3<f32>(0.075, 0.08, 0.088);
        }
        // The lit core along the top, and the pulses running from the provider (`along`
        // rising), each a bright head trailing back.
        let core = 1.0 - smoothstep(CORE, CORE + fwidth(a), abs(a));
        let pulse = pow(fract((in.local.x - time * PULSE_SPEED) / PULSE_GAP_M - l.phase), 4.0);
        let glow = core * (select(0.5, 1.0, bright) + pulse * select(2.0, 4.5, bright));
        albedo = mix(albedo, tone * 0.3, core);
        metal = mix(metal, 0.0, core);
        emissive = tone * glow * 3.0;
    } else {
        n = normalize(cross(dpdx(p), dpdy(p)));
        if dot(n, v) < 0.0 {
            n = -n;
        }
        let o = in.local.xy / in.grow;
        let top = in.top == 1u;
        switch in.part {
            case PART_CLAMP: {
                // Bright worn steel, painted amber on a bound pair.
                albedo = select(vec3<f32>(0.3, 0.32, 0.34), HAZARD_AMBER, bound);
                rough = select(0.4, 0.55, bound);
            }
            case PART_JUNCTION: {
                // A steel box, its lamp on top in the line's colour, hazard banded on a
                // bound pair.
                albedo = vec3<f32>(0.16, 0.17, 0.19);
                if bound && !top {
                    albedo = mix(HAZARD_AMBER, HAZARD_BLACK, step(0.5, fract((o.x + o.y + in.local.z * 0.8) / 0.5)));
                    metal = 0.1;
                }
                let lamp = f32(top) * (1.0 - smoothstep(0.35, 0.4, max(abs(o.x), abs(o.y))));
                emissive = tone * lamp * 1.6 * lit;
                albedo = mix(albedo, tone * 0.3, lamp);
            }
            case PART_PLATE: {
                // Lapped graphite; its lip glows where it stands off the cable.
                albedo = vec3<f32>(0.045, 0.043, 0.05);
                metal = 0.8;
                rough = 0.38;
                if !top {
                    let lip = smoothstep(0.35, 0.9, in.local.z);
                    emissive = select(tone, BOUND_GLOW, bound) * lip * 1.4 * lit;
                }
            }
            default: {
                // A field node: faceted graphite, its top an inset that breathes.
                albedo = vec3<f32>(0.05, 0.048, 0.056);
                metal = 0.8;
                rough = 0.35;
                if top {
                    let r = length(o);
                    let inset = 1.0 - smoothstep(0.62, 0.7, r);
                    let breathe = 0.75 + 0.25 * sin(time * 1.7 + l.phase * 6.283);
                    emissive = select(tone, BOUND_GLOW, bound) * inset * breathe * 1.8 * lit;
                    albedo = mix(albedo, tone * 0.2, inset);
                }
            }
        }
    }
    var m: Pbr;
    m.albedo = albedo;
    m.metallic = metal;
    m.roughness = rough;
    m.emissive = emissive;
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
