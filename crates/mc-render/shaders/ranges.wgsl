//!use bindings
// Range rings: what a selected unit can reach, as thin circles draped over the
// ground. Rings of one group merge: where a ring runs through ground another
// ring of its group already covers it is not drawn, so fifty tanks show the
// outline of what they cover together, not fifty circles.
//
// A group is a kind (low byte) and a rank (above it): the farthest ring of a
// kind on a unit is rank 0, a shorter gun of the same kind rank 1, and so on.
// Lower ranks are the same colour as a finer, fainter solid line; dashes are
// only ever a dead zone's inner edge.

struct Ring {
    center: vec2<f32>,
    // The reach is the annulus between the two; zero `inner` has no dead zone.
    inner: f32,
    outer: f32,
    color: vec3<f32>,
    group: u32,
    // A part ring: the arc's centre (world angle) and half-width. PI or more is round.
    facing: f32,
    half_arc: f32,
}

struct RangePush {
    // Every ring in the buffer masks; only the first ones are drawn.
    count: u32,
    // Steps around the circle in this draw: long rings get more than short ones.
    segments: u32,
}

@group(1) @binding(0) var<storage, read> rings: array<Ring>;
var<immediate> push: RangePush;

// Pixels either side of the line's centre.
const HALF_WIDTH: f32 = 1.1;

struct RingOut {
    @builtin(position) clip: vec4<f32>,
    // Pixels from the centre of the line.
    @location(0) across: f32,
    // Zero to one around the circle.
    @location(1) around: f32,
    // Metres inside another ring's reach; positive is hidden.
    @location(2) covered: f32,
    @location(3) dist: f32,
    @location(4) @interpolate(flat) color: vec3<f32>,
    // The dead-zone edge: drawn dashed. Holds the radius, zero for the outer edge.
    @location(5) @interpolate(flat) dashed: f32,
    // Pixels of ink either side of the line. Anti-missile reach is a hairline.
    @location(6) @interpolate(flat) half: f32,
    // How strong the ink is: lower ranks are fainter.
    @location(7) @interpolate(flat) ink: f32,
}

// How far `p` lies inside a ring's reach, in metres; positive is inside.
fn inside(p: vec2<f32>, other: Ring) -> f32 {
    let d = distance(p, other.center);
    var margin = min(d - other.inner, other.outer - d);
    if other.half_arc < PI {
        let to = p - other.center;
        var off = atan2(to.y, to.x) - other.facing;
        off = off - floor((off + PI) / (2.0 * PI)) * 2.0 * PI;
        margin = min(margin, (other.half_arc - abs(off)) * d);
    }
    return margin;
}

// Four instances per ring: the outer edge, the inner one, then (for a part
// ring) its two straight edges. Each strip is generated from the vertex index:
// two vertices per step along the line.
@vertex
fn vs_range(@builtin(vertex_index) v: u32, @builtin(instance_index) instance: u32) -> RingOut {
    let entry = instance / 4u;
    let part = instance & 3u;
    let is_inner = part == 1u;
    let is_edge = part >= 2u;
    let ring = rings[entry];
    let round = ring.half_arc >= PI;
    let r = select(ring.outer, ring.inner, is_inner);
    var out: RingOut;
    out.clip = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    if r <= 0.0 || (is_edge && round) {
        return out;
    }

    let step = f32(v / 2u) / f32(push.segments);
    let side = f32(v & 1u) * 2.0 - 1.0;
    let spread = select(ring.half_arc, PI, round);
    var theta = ring.facing - spread + step * 2.0 * spread;
    var radius = r;
    // Around the arc, or out along an edge from the dead zone (or the centre).
    var along: vec2<f32>;
    if is_edge {
        theta = ring.facing + select(-spread, spread, part == 3u);
        radius = mix(ring.inner, ring.outer, step);
    }
    let dir = vec2<f32>(cos(theta), sin(theta));
    if is_edge {
        along = dir;
    } else {
        along = vec2<f32>(-dir.y, dir.x);
    }
    // Where on the whole circle this is, for the dashes: they keep their pitch on an arc.
    let around = select(step * spread / PI, 0.0, is_edge);
    let xy = ring.center + dir * radius;
    // Anti-missile reach (kind 9) is a hairline. A lower rank is finer and fainter.
    let kind = ring.group & 0xFFu;
    let rank = f32(min(ring.group >> 8u, 2u));
    let half = select(HALF_WIDTH * (1.0 - 0.3 * rank), 0.38, kind == 9u);
    let ink = 1.0 - 0.28 * rank;
    let world = vec3<f32>(xy, max(terrain_height(xy), globals.map.z) + 0.5);
    let dist = distance(world, globals.camera.xyz);
    let clip = globals.view_proj * vec4<f32>(world, 1.0);

    // A constant width on screen: step sideways from the ring's direction there.
    let ahead = globals.view_proj * vec4<f32>(world + vec3<f32>(along, 0.0) * dist * 0.01, 1.0);
    var offset = vec2<f32>(0.0);
    if clip.w > 0.01 && ahead.w > 0.01 {
        let t = (ahead.xy / ahead.w - clip.xy / clip.w) * globals.viewport.xy;
        let len = length(t);
        if len > 1e-6 {
            offset = vec2<f32>(-t.y, t.x) / len * side * (half + 1.0) * 2.0 * globals.viewport.zw;
        }
    }
    // The chords between vertices dip under rising ground; a little nearer in
    // depth (reversed Z) keeps them above it at every zoom.
    out.clip = vec4<f32>(clip.xy + offset * clip.w, clip.z * 1.02, clip.w);

    var covered = -1.0e4;
    for (var j = 0u; j < push.count; j++) {
        let other = rings[j];
        if j == entry || other.group != ring.group {
            continue;
        }
        covered = max(covered, inside(xy, other));
    }

    out.across = side * (half + 1.0);
    out.around = around;
    out.covered = covered;
    out.dist = dist;
    out.color = ring.color;
    out.dashed = select(0.0, r, is_inner);
    out.half = half;
    out.ink = ink;
    return out;
}

@fragment
fn fs_range(in: RingOut) -> @location(0) vec4<f32> {
    if in.covered > 0.0 {
        discard;
    }
    var alpha = clamp(in.half + 0.5 - abs(in.across), 0.0, 1.0);
    if in.dashed > 0.0 {
        // Dashes a steady length on screen: their number is a power of two, so
        // where the count changes with distance the pattern still lines up.
        let wanted = 2.0 * PI * in.dashed / max(in.dist * 0.022, 0.5);
        let dashes = exp2(floor(log2(max(wanted, 8.0))));
        if fract(in.around * dashes) > 0.5 {
            discard;
        }
        alpha *= 0.85;
    }
    if alpha <= 0.01 {
        discard;
    }
    return vec4<f32>(in.color * (0.6 + 0.9 * in.ink), alpha * 0.9 * in.ink);
}
