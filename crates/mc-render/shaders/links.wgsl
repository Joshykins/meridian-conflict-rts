//!use bindings
//!use surface
//!use metal
// Adjacency conduits across the seam where two lots meet (renderer/adjacency_links.rs).
// Per link, in the seam's frame (u along the shared edge, w across it, from the
// provider's lot into the neighbour's):
// - a steel tray laid along the seam, its centre line lit in the resource's colour;
// - a row of couplers across it: a housing on the provider's side, a body that tapers
//   from it into a socket on the neighbour's, and a slot along the body's top, lit,
//   with pulses running from the provider into the neighbour;
// - for a bound pair (`LINK_BOUND`), hazard banding on the tray and an amber strap
//   round each coupler at the seam.
// Everything sits on `terrain_height` and comes up out of the ground when a link is new.

//!rust crate::renderer::adjacency_links::LinkInstance
struct LinkInstance {
    a: vec2<f32>,
    b: vec2<f32>,
    // Across the seam, from the provider's lot into the neighbour's.
    toward: vec2<f32>,
    // Render time it began to come up.
    start: f32,
    flags: u32,
}

struct LinkPush {
    pass_kind: u32,
    unused: u32,
}

@group(1) @binding(0) var<storage, read> links: array<LinkInstance>;
var<immediate> push: LinkPush;

// It stands this far off the ground, plus a little per metre of distance so it never
// fights the terrain for depth.
const LIFT_M: f32 = 0.05;
// How deep it starts, buried, while a new link comes up; and how far the sides reach
// below the ground, so a slope never shows a gap under them.
const BURIED_M: f32 = 1.4;
const SINK_M: f32 = 0.25;
// Sizes below are for a seam this long (two 2x2 lots); a longer one scales them up,
// to this much at most.
const SCALE_EDGE_M: f32 = 24.0;
const MAX_SCALE: f32 = 2.2;
// The tray: half width, height, and how far short of the seam's ends it stops.
const TRAY_HALF_M: f32 = 1.3;
const TRAY_H_M: f32 = 0.22;
const TRAY_MARGIN_M: f32 = 3.0;
// A tray segment is at most this long, so it bends with the ground.
const TRAY_STEP_M: f32 = 6.0;
// Couplers come about this far apart.
const COUPLER_STEP_M: f32 = 11.0;
// A coupler's body: half width, how far it reaches into each lot, and its height at
// the provider's end, at the seam and at the neighbour's end.
const BODY_HALF_M: f32 = 0.85;
const BODY_FROM_M: f32 = -2.6;
const BODY_TO_M: f32 = 3.3;
const BODY_H: vec3<f32> = vec3<f32>(1.05, 0.8, 0.55);
// The provider's housing and the neighbour's socket: half width, reach, height.
const HOUSING_HALF_M: f32 = 1.35;
const HOUSING_W: vec2<f32> = vec2<f32>(-3.9, -2.3);
const HOUSING_H_M: f32 = 1.5;
const SOCKET_HALF_M: f32 = 1.1;
const SOCKET_W: vec2<f32> = vec2<f32>(3.1, 3.9);
const SOCKET_H_M: f32 = 0.85;
// A bound pair's strap round the body at the seam.
const STRAP_HALF_M: f32 = 0.97;
const STRAP_W_M: f32 = 0.34;
// The lit slot along a body's top: half width; pulses run at this speed, this far apart.
const SLOT_HALF_M: f32 = 0.24;
const PULSE_SPEED: f32 = 2.2;
const PULSE_GAP_M: f32 = 3.2;

// Vertices: the tray's segments of three faces, then per coupler its body (two boxes
// of four faces), housing, socket and strap (five faces each).
const TRAY_VERTICES: u32 = LINK_MAX_TRAY_SEGMENTS * 18u;
const COUPLER_VERTICES: u32 = 48u + 3u * 30u;

// What a fragment belongs to.
const PART_TRAY: u32 = 0u;
const PART_BODY: u32 = 1u;
const PART_HOUSING: u32 = 2u;
const PART_SOCKET: u32 = 3u;
const PART_STRAP: u32 = 4u;

// Box faces.
const FACE_TOP: u32 = 0u;
const FACE_LOW_U: u32 = 1u;
const FACE_HIGH_U: u32 = 2u;
const FACE_FROM: u32 = 3u;
const FACE_TO: u32 = 4u;

struct VsOut {
    @builtin(position) @invariant clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    // Across the piece from its middle (m), w across the seam, and height above ground.
    @location(1) local: vec3<f32>,
    @location(2) @interpolate(flat) part: u32,
    @location(3) @interpolate(flat) face: u32,
    @location(4) @interpolate(flat) flags: u32,
}

const QUAD = array<vec2<f32>, 6>(
    vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
);

struct Seam {
    a: vec2<f32>,
    along: vec2<f32>,
    toward: vec2<f32>,
    len: f32,
    lift: f32,
    // Every size below is multiplied by this: big buildings get big conduits.
    scale: f32,
}

fn seam_xy(s: Seam, u: f32, w: f32) -> vec2<f32> {
    return s.a + s.along * u + s.toward * w;
}

struct Piece {
    world: vec3<f32>,
    // Across the box from u0..u1's middle, w, and height above ground.
    local: vec3<f32>,
}

// A vertex of a box over the ground from u0..u1, w0..w1, its top `h0` high at w0 and
// `h1` at w1. `k` is the vertex of the face's quad.
fn box_vertex(s: Seam, u0: f32, u1: f32, w0: f32, w1: f32, h0: f32, h1: f32, face: u32, k: u32) -> Piece {
    let q = QUAD[k];
    var u = mix(u0, u1, q.y);
    var w = mix(w0, w1, q.x);
    let top = mix(h0, h1, q.x);
    var up = top;
    switch face {
        case FACE_TOP: {}
        case FACE_LOW_U, FACE_HIGH_U: {
            u = select(u0, u1, face == FACE_HIGH_U);
            up = mix(-SINK_M, top, q.y);
        }
        default: {
            u = mix(u0, u1, q.x);
            w = select(w0, w1, face == FACE_TO);
            up = mix(-SINK_M, select(h0, h1, face == FACE_TO), q.y);
        }
    }
    let xy = seam_xy(s, u, w * s.scale);
    var out: Piece;
    out.world = vec3<f32>(xy, terrain_height(xy) + s.lift + up * s.scale);
    out.local = vec3<f32>((u - 0.5 * (u0 + u1)) / s.scale, w, max(up, 0.0));
    return out;
}

struct Built {
    piece: Piece,
    part: u32,
    face: u32,
    shown: bool,
}

fn tray_vertex(s: Seam, vi: u32) -> Built {
    var out: Built;
    let span = max(s.len - 2.0 * TRAY_MARGIN_M, 1.0);
    let margin = 0.5 * (s.len - span);
    let segments = clamp(u32(ceil(span / TRAY_STEP_M)), 1u, LINK_MAX_TRAY_SEGMENTS);
    let seg = vi / 18u;
    out.shown = seg < segments;
    let step = span / f32(segments);
    let u0 = margin + step * f32(seg);
    // Top, then the two long sides.
    let face = array<u32, 3>(FACE_TOP, FACE_FROM, FACE_TO)[(vi % 18u) / 6u];
    out.piece = box_vertex(s, u0, u0 + step, -TRAY_HALF_M, TRAY_HALF_M, TRAY_H_M, TRAY_H_M, face, vi % 6u);
    // The tray's `local.x` is metres along the seam, for its panels.
    out.piece.local.x = mix(u0, u0 + step, QUAD[vi % 6u].y) / s.scale;
    if face != FACE_TOP {
        out.piece.local.x = mix(u0, u0 + step, QUAD[vi % 6u].x) / s.scale;
    }
    out.part = PART_TRAY;
    out.face = face;
    return out;
}

fn coupler_count(len: f32, scale: f32) -> u32 {
    let span = max(len - 2.0 * TRAY_MARGIN_M, 1.0);
    return clamp(u32(round(span / (COUPLER_STEP_M * scale))), 1u, LINK_MAX_COUPLERS);
}

fn coupler_vertex(s: Seam, vi: u32, bound: bool) -> Built {
    var out: Built;
    let c = vi / COUPLER_VERTICES;
    let r = vi % COUPLER_VERTICES;
    let count = coupler_count(s.len, s.scale);
    out.shown = c < count;
    let span = max(s.len - 2.0 * TRAY_MARGIN_M, 1.0);
    let mid = 0.5 * (s.len - span) + span * (f32(c) + 0.5) / f32(count);
    if r < 48u {
        // The body in two boxes meeting at the seam: top, sides, and its far end.
        let half = r / 24u;
        let f = (r % 24u) / 6u;
        let face = select(f, select(FACE_FROM, FACE_TO, half == 1u), f == 3u);
        let w0 = select(BODY_FROM_M, 0.0, half == 1u);
        let w1 = select(0.0, BODY_TO_M, half == 1u);
        let h0 = select(BODY_H.x, BODY_H.y, half == 1u);
        let h1 = select(BODY_H.y, BODY_H.z, half == 1u);
        let bh = BODY_HALF_M * s.scale;
        out.piece = box_vertex(s, mid - bh, mid + bh, w0, w1, h0, h1, face, r % 6u);
        out.part = PART_BODY;
        out.face = face;
        return out;
    }
    let j = r - 48u;
    let face = (j % 30u) / 6u;
    out.face = face;
    let k = j % 6u;
    if j < 30u {
        out.piece = box_vertex(s, mid - HOUSING_HALF_M * s.scale, mid + HOUSING_HALF_M * s.scale, HOUSING_W.x, HOUSING_W.y, HOUSING_H_M, HOUSING_H_M, face, k);
        out.part = PART_HOUSING;
    } else if j < 60u {
        out.piece = box_vertex(s, mid - SOCKET_HALF_M * s.scale, mid + SOCKET_HALF_M * s.scale, SOCKET_W.x, SOCKET_W.y, SOCKET_H_M, SOCKET_H_M, face, k);
        out.part = PART_SOCKET;
    } else {
        let h = BODY_H.y + 0.09;
        out.piece = box_vertex(s, mid - STRAP_HALF_M * s.scale, mid + STRAP_HALF_M * s.scale, -STRAP_W_M, STRAP_W_M, h, h, face, k);
        out.part = PART_STRAP;
        out.shown = out.shown && bound;
    }
    return out;
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {
    let l = links[ii];
    var out: VsOut;
    let t = clamp((globals.camera.w - l.start) / SETTLE_SECONDS, 0.0, 1.0);
    let rise = t * t * (3.0 - 2.0 * t);
    if rise <= 0.001 {
        out.clip = vec4<f32>(0.0);
        return out;
    }
    var s: Seam;
    s.a = l.a;
    s.len = distance(l.a, l.b);
    s.along = (l.b - l.a) / max(s.len, 0.001);
    s.toward = l.toward;
    s.scale = clamp(s.len / SCALE_EDGE_M, 1.0, MAX_SCALE);
    let far = distance(globals.camera.xy, 0.5 * (l.a + l.b));
    s.lift = mix(-BURIED_M, LIFT_M + far * 0.0004, rise);
    var b: Built;
    if vi < TRAY_VERTICES {
        b = tray_vertex(s, vi);
    } else {
        b = coupler_vertex(s, vi - TRAY_VERTICES, (l.flags & LINK_BOUND) != 0u);
    }
    if !b.shown {
        out.clip = vec4<f32>(0.0);
        return out;
    }
    let world = b.piece.world;
    if (push.pass_kind & PASS_KIND_MASK) == PASS_SHADOW {
        out.clip = globals.shadow_cascades[push.pass_kind >> PASS_CASCADE_SHIFT] * vec4<f32>(world, 1.0);
    } else {
        out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    }
    out.world = world;
    out.local = b.piece.local;
    out.part = b.part;
    out.face = b.face;
    out.flags = l.flags;
    return out;
}

@fragment
fn fs_shadow() {
}

// A `0xRRGGBB` sRGB colour as linear light.
fn tone_rgb(rgb: u32) -> vec3<f32> {
    let c = vec3<f32>(f32((rgb >> 16u) & 0xFFu), f32((rgb >> 8u) & 0xFFu), f32(rgb & 0xFFu)) / 255.0;
    return pow(c, vec3<f32>(2.2));
}

// Soft line `width` wide at each integer of `x`, faded once a pixel (`px` in units of `x`)
// is too coarse to show it.
fn seam_line(x: f32, width: f32, px: f32) -> f32 {
    let d = min(fract(x), 1.0 - fract(x));
    return (1.0 - smoothstep(width * 0.5, width * 0.5 + px, d)) * (1.0 - smoothstep(0.15, 0.4, px));
}

// Hazard banding: amber and black stripes at 45 degrees, 0.5 m apart.
fn hazard(xy: vec2<f32>) -> vec3<f32> {
    let stripe = step(0.5, fract((xy.x + xy.y) / 0.5));
    return mix(vec3<f32>(0.62, 0.36, 0.035), vec3<f32>(0.025, 0.024, 0.022), stripe);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let eye = globals.camera.xyz;
    let p = in.world;
    let v = normalize(eye - p);
    var n = normalize(cross(dpdx(p), dpdy(p)));
    if dot(n, v) < 0.0 {
        n = -n;
    }
    let px = max(length(fwidth(p)), 0.004);
    let grain = noise_varied(p.xy + p.z * vec2<f32>(0.37, 0.61), 2.5);
    let energy = (in.flags & LINK_ENERGY) != 0u;
    let bright = (in.flags & LINK_HIGHLIGHT) != 0u;
    let tone = tone_rgb(select(TONE_MASS, TONE_ENERGY, energy));
    let time = globals.camera.w;

    // Gunmetal steel, the same as the foundations' plating.
    var albedo = vec3<f32>(0.2, 0.215, 0.23) * (0.85 + 0.25 * grain.r);
    var metal = 0.75;
    var rough = 0.5 + 0.2 * grain.g;
    var glow = 0.0;
    let top = in.face == FACE_TOP;
    if in.part == PART_TRAY {
        albedo *= 0.8;
        if top {
            // Cover plates 3 m long, the lit line down the middle, and for a bound
            // pair hazard banding along both edges.
            albedo *= 1.0 - 0.5 * seam_line(in.local.x / 3.0, 0.03, px / 3.0);
            let w = abs(in.local.y);
            let line = 1.0 - smoothstep(0.16, 0.16 + px, w);
            let groove = 1.0 - smoothstep(0.3, 0.3 + px, w);
            albedo *= 1.0 - 0.6 * groove * (1.0 - line);
            glow = line * select(0.4, 1.4, bright);
            if (in.flags & LINK_BOUND) != 0u && w > 0.75 {
                albedo = hazard(in.local.xy) * (0.9 + 0.2 * grain.r);
                metal = 0.1;
                rough = 0.6;
            }
        }
    } else if in.part == PART_BODY {
        albedo *= 1.1;
        if top {
            // The slot: lit in the resource's colour, pulses running from the provider
            // into the neighbour (up `w`), each a bright head trailing back.
            let slot = 1.0 - smoothstep(SLOT_HALF_M, SLOT_HALF_M + px, abs(in.local.x));
            let lip = 1.0 - smoothstep(SLOT_HALF_M + 0.08, SLOT_HALF_M + 0.08 + px, abs(in.local.x));
            albedo *= 1.0 - 0.6 * lip * (1.0 - slot);
            let speed = select(PULSE_SPEED, PULSE_SPEED * 1.6, bright);
            let phase = fract((in.local.y - time * speed) / PULSE_GAP_M);
            let pulse = pow(phase, 5.0);
            glow = slot * (select(0.1, 0.5, bright) + pulse * select(2.5, 5.0, bright));
        }
    } else if in.part == PART_HOUSING {
        albedo *= 0.75;
        // A lamp on the face toward the neighbour: where the conduit draws from.
        if in.face == FACE_TO {
            let lamp = (1.0 - smoothstep(0.6, 0.6 + px, abs(in.local.x)))
                * (1.0 - smoothstep(0.22, 0.22 + px, abs(in.local.z - 0.95)));
            glow = lamp * select(0.6, 2.0, bright);
        }
        if top {
            albedo *= 1.0 - 0.5 * seam_line(in.local.x / 0.7 + 0.5, 0.06, px / 0.7);
        }
    } else if in.part == PART_SOCKET {
        albedo = vec3<f32>(0.28, 0.3, 0.32) * (0.9 + 0.2 * grain.r);
        rough = 0.42;
    } else {
        // The bound pair's strap.
        albedo = hazard(vec2<f32>(in.local.x, in.local.z + in.local.y)) * (0.9 + 0.2 * grain.r);
        metal = 0.1;
        rough = 0.55;
    }
    // Rain wets it as it does the ground.
    let soaked = weather_at(p.xy).w;
    albedo *= 1.0 - 0.25 * soaked;
    rough = mix(rough, 0.3, soaked * 0.75);

    var m: Pbr;
    m.albedo = mix(albedo, tone * 0.3, min(glow, 1.0));
    m.metallic = mix(metal, 0.0, min(glow, 1.0));
    m.roughness = rough;
    m.emissive = tone * glow * 3.0;
    let shadow = sun_shadow(p, n);
    let sky_vis = 0.85 * screen_ao(in.clip.xy);
    var color = shade_pbr_vis(m, n, v, globals.sun.xyz, shadow, sky_vis);
    color += albedo * lightning_light(p, n) * 0.35;
    color += local_lights(m, p, n, v);
    color = apply_fog_of_war(color, p.xy);
    color = apply_haze(color, p, eye);
    if (in.flags & LINK_PLANNED) != 0u {
        // A placement ghost's: see-through, its light added over the scene.
        let a = 0.45;
        return vec4<f32>(mix(color, tone * 0.6, 0.35) * a + m.emissive * 0.5, a);
    }
    return vec4<f32>(color, 1.0);
}
