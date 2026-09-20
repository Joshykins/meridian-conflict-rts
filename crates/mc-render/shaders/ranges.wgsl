//!use bindings
// Range rings: what a selected unit can reach, as thin circles draped over the
// ground. Rings of one group merge: where a ring runs through ground another
// ring of its group already covers it is not drawn, so fifty tanks show the
// outline of what they cover together, not fifty circles.

struct Ring {
    center: vec2<f32>,
    // The reach is the annulus between the two; zero `inner` has no dead zone.
    inner: f32,
    outer: f32,
    color: vec3<f32>,
    group: u32,
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
}

// Two instances per ring: the outer edge, then the inner one. The strip is
// generated from the vertex index: two vertices per step around the circle.
@vertex
fn vs_range(@builtin(vertex_index) v: u32, @builtin(instance_index) instance: u32) -> RingOut {
    let entry = instance / 2u;
    let is_inner = (instance & 1u) == 1u;
    let ring = rings[entry];
    let r = select(ring.outer, ring.inner, is_inner);
    var out: RingOut;
    out.clip = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    if r <= 0.0 {
        return out;
    }

    let around = f32(v / 2u) / f32(push.segments);
    let side = f32(v & 1u) * 2.0 - 1.0;
    let theta = around * 2.0 * PI;
    let dir = vec2<f32>(cos(theta), sin(theta));
    let xy = ring.center + dir * r;
    let world = vec3<f32>(xy, max(terrain_height(xy), globals.map.z) + 0.5);
    let dist = distance(world, globals.camera.xyz);
    let clip = globals.view_proj * vec4<f32>(world, 1.0);

    // A constant width on screen: step sideways from the ring's direction there.
    let ahead = globals.view_proj * vec4<f32>(world + vec3<f32>(-dir.y, dir.x, 0.0) * dist * 0.01, 1.0);
    var offset = vec2<f32>(0.0);
    if clip.w > 0.01 && ahead.w > 0.01 {
        let t = (ahead.xy / ahead.w - clip.xy / clip.w) * globals.viewport.xy;
        let len = length(t);
        if len > 1e-6 {
            offset = vec2<f32>(-t.y, t.x) / len * side * (HALF_WIDTH + 1.0) * 2.0 * globals.viewport.zw;
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
        let d = distance(xy, other.center);
        covered = max(covered, min(d - other.inner, other.outer - d));
    }

    out.across = side * (HALF_WIDTH + 1.0);
    out.around = around;
    out.covered = covered;
    out.dist = dist;
    out.color = ring.color;
    out.dashed = select(0.0, r, is_inner);
    return out;
}

@fragment
fn fs_range(in: RingOut) -> @location(0) vec4<f32> {
    if in.covered > 0.0 {
        discard;
    }
    var alpha = clamp(HALF_WIDTH + 0.5 - abs(in.across), 0.0, 1.0);
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
    return vec4<f32>(in.color * 1.5, alpha * 0.9);
}
