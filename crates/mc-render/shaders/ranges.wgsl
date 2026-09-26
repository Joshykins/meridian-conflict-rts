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
    // Line-widths further out it is drawn: another kind's ring of the same reach lies
    // under it (rings.rs `collect`).
    nudge: f32,
    // Above zero: the ring the HUD points at (a hovered weapon card), drawn bolder with a
    // wash on the side it reaches. Below zero: another ring is in focus; this one steps back.
    focus: f32,
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
// Pixels of wash inside a ring in focus.
const BAND: f32 = 22.0;

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
    // Pixels of wash on the reached side of a ring in focus, zero otherwise.
    @location(8) @interpolate(flat) band: f32,
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
    let rank = f32(min((ring.group >> 8u) & 0xFFu, 2u));
    var half = select(HALF_WIDTH * (1.0 - 0.3 * rank), 0.38, kind == 9u);
    var ink = 1.0 - 0.28 * rank;
    let lit = ring.focus > 0.0;
    if lit {
        half = HALF_WIDTH * 1.7;
        ink = 1.0;
    } else if ring.focus < 0.0 {
        ink *= 0.35;
    }
    let band = select(0.0, BAND, lit);
    var world = vec3<f32>(xy, max(terrain_height(xy), globals.map.z) + 0.5);
    var dist = distance(world, globals.camera.xyz);
    if ring.nudge > 0.0 && !is_edge {
        // About three pixels a step at any zoom (`dist * lod`): side by side, not on top.
        let out_m = ring.nudge * 3.0 * dist / max(globals.lod.x, 1.0);
        let moved = ring.center + dir * (radius + out_m);
        world = vec3<f32>(moved, max(terrain_height(moved), globals.map.z) + 0.5);
        dist = distance(world, globals.camera.xyz);
    }
    let clip = globals.view_proj * vec4<f32>(world, 1.0);

    // A constant width on screen: step sideways from the ring's direction there.
    let ahead = globals.view_proj * vec4<f32>(world + vec3<f32>(along, 0.0) * dist * 0.01, 1.0);
    var offset = vec2<f32>(0.0);
    var across = side * (half + 1.0);
    if clip.w > 0.01 && ahead.w > 0.01 {
        let t = (ahead.xy / ahead.w - clip.xy / clip.w) * globals.viewport.xy;
        let len = length(t);
        if len > 1e-6 {
            offset = vec2<f32>(-t.y, t.x) / len * side * (half + 1.0) * 2.0 * globals.viewport.zw;
        }
        if lit {
            // Which way on screen the reach lies from this line: in from the outer
            // edge, out from the dead zone's, into the wedge from its sides.
            var inward = -dir;
            if is_inner {
                inward = dir;
            }
            if is_edge {
                inward = select(1.0, -1.0, part == 3u) * vec2<f32>(-dir.y, dir.x);
            }
            let deep = globals.view_proj * vec4<f32>(world + vec3<f32>(inward, 0.0) * dist * 0.01, 1.0);
            if deep.w > 0.01 {
                let n = (deep.xy / deep.w - clip.xy / clip.w) * globals.viewport.xy;
                if length(n) > 1e-6 {
                    let into = normalize(n);
                    // One side of the strip on the line's far edge, the other a band deep.
                    across = select(-band, half + 1.0, side > 0.0);
                    offset = into * -across * 2.0 * globals.viewport.zw;
                }
            }
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

    out.across = across;
    out.around = around;
    out.covered = covered;
    out.dist = dist;
    out.color = ring.color;
    out.dashed = select(0.0, r, is_inner);
    out.half = half;
    out.ink = ink;
    out.band = band;
    return out;
}

@fragment
fn fs_range(in: RingOut) -> @location(0) vec4<f32> {
    // Inside another ring of its kind by more than a metre: the union's outline is
    // elsewhere. The metre keeps arcs of the same reach, which lie exactly on each
    // other's edge (a ship's turrets, each with its own wedge), from masking each other
    // away on rounding.
    if in.covered > 1.0 {
        discard;
    }
    var alpha = clamp(in.half + 0.5 - abs(in.across), 0.0, 1.0);
    if in.dashed > 0.0 {
        // Dashes a steady length on screen: their number is a power of two, so
        // where the count changes with distance the pattern still lines up.
        let wanted = 2.0 * PI * in.dashed / max(in.dist * 0.022, 0.5);
        let dashes = exp2(floor(log2(max(wanted, 8.0))));
        if fract(in.around * dashes) > 0.5 {
            alpha = 0.0;
        }
        alpha *= 0.85;
    }
    var a = alpha * 0.9 * in.ink;
    if in.band > 0.0 && in.across < 0.0 {
        // The wash of a ring in focus: strongest at the line, breathing, with a light
        // that runs round the reach.
        let k = 1.0 - clamp(-in.across / in.band, 0.0, 1.0);
        let breath = 0.8 + 0.2 * sin(globals.camera.w * 3.5);
        let run = pow(fract(in.around - globals.camera.w * 0.18), 24.0);
        a = max(a, k * k * (0.30 * breath + 0.45 * run));
    }
    if a <= 0.004 {
        discard;
    }
    return vec4<f32>(in.color * (0.6 + 0.9 * in.ink), a);
}
