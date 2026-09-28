//!use bindings
// Retaining walls on the slopes round a levelled lot (renderer/foundations.rs):
// per 8 m cell of the slope, a steel plate laid on the ground, three ribs running
// down it and a cap rail along its high edge. Everything sits on
// `terrain_height`, so it follows the slope exactly, settling included, and it
// comes up out of the ground while its lot settles (`settle::SECONDS`).

struct FoundationPush {
    pass_kind: u32,
    unused: u32,
}

@group(1) @binding(0) var<storage, read> cells: array<FoundationCell>;
var<immediate> push: FoundationPush;

const CELL_M: f32 = 8.0;
// The plate stands this far off the ground, plus a little per metre of distance so
// it never fights the terrain for depth.
const PLATE_LIFT_M: f32 = 0.06;
// Ribs down the slope (at 2, 4 and 6 m across): half width and height.
const RIB_HALF_M: f32 = 0.16;
const RIB_H_M: f32 = 0.28;
// The cap rail on the high edge: half width and height.
const CAP_HALF_M: f32 = 0.3;
const CAP_H_M: f32 = 0.4;
// How deep the plating starts, buried, before its lot settles.
const BURIED_M: f32 = 1.2;

// What a fragment belongs to.
const PART_PLATE: u32 = 0u;
const PART_RIB: u32 = 1u;
const PART_CAP: u32 = 2u;

struct VsOut {
    @builtin(position) @invariant clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    // Up the slope from its low edge, and across it, metres.
    @location(1) slope: vec2<f32>,
    @location(2) @interpolate(flat) part: u32,
}

// A quad's six corners as (along 0/1, across-or-up 0/1).
const QUAD = array<vec2<f32>, 6>(
    vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
);

// A point of the cell's slope frame (u up the slope from its low edge, v across
// it) in world xy.
fn slope_xy(c: FoundationCell, u: f32, v: f32) -> vec2<f32> {
    var a = u;
    if (c.kind & 2u) == 0u {
        a = CELL_M - u;
    }
    if (c.kind & 1u) != 0u {
        return c.origin + vec2<f32>(v, a);
    }
    return c.origin + vec2<f32>(a, v);
}

// A strip laid on the ground from u0 to u1 at v across: face 0 its top, 1 and 2
// its sides. `k` is the vertex of the face's quad.
fn strip(c: FoundationCell, u0: f32, u1: f32, v: f32, half: f32, h: f32, face: u32, k: u32, lift: f32) -> vec3<f32> {
    let q = QUAD[k];
    let u = mix(u0, u1, q.x);
    var across = v;
    var up = h;
    if face == 0u {
        across = v + mix(-half, half, q.y);
    } else {
        across = v + select(-half, half, face == 2u);
        up = h * q.y;
    }
    let xy = slope_xy(c, u, across);
    return vec3<f32>(xy, terrain_height(xy) + lift + up);
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {
    let c = cells[ii];
    var out: VsOut;
    let t = clamp((globals.camera.w - c.start) / SETTLE_SECONDS, 0.0, 1.0);
    let rise = t * t * (3.0 - 2.0 * t);
    if rise <= 0.001 {
        out.clip = vec4<f32>(0.0);
        return out;
    }
    let far = distance(globals.camera.xy, c.origin + vec2<f32>(CELL_M * 0.5));
    let lift = mix(-BURIED_M, PLATE_LIFT_M + far * 0.0004, rise);
    var world: vec3<f32>;
    if vi < 6u {
        // The plate: the cell's two triangles, split along (0,0)-(1,1) as the
        // terrain's are, so it lies flat on them.
        let xy = c.origin + QUAD[vi] * CELL_M;
        world = vec3<f32>(xy, terrain_height(xy) + lift);
        out.part = PART_PLATE;
    } else if vi < 114u {
        // Three ribs, each in two segments that meet where it crosses the cell's
        // diagonal, so it bends with the ground there.
        let i = vi - 6u;
        let rib = i / 36u;
        let seg = (i / 18u) % 2u;
        let face = (i / 6u) % 3u;
        let across = 2.0 + 2.0 * f32(rib);
        // The diagonal x = y meets the rib where u = v, or u = 8 - v when the
        // slope's frame runs backward.
        var bend = across;
        if (c.kind & 2u) == 0u {
            bend = CELL_M - across;
        }
        let u0 = select(bend, 0.0, seg == 0u);
        let u1 = select(CELL_M, bend, seg == 0u);
        world = strip(c, u0, u1, across, RIB_HALF_M, RIB_H_M, face, i % 6u, lift);
        out.part = PART_RIB;
    } else {
        // The cap rail along the high edge, the length of the cell. The ground
        // along a cell edge is straight, so its two ends carry it.
        let i = vi - 114u;
        let face = i / 6u;
        let q = QUAD[i % 6u];
        let along = q.x * CELL_M;
        var u = CELL_M;
        var up = CAP_H_M;
        if face == 0u {
            u = CELL_M + mix(-CAP_HALF_M, CAP_HALF_M, q.y);
        } else {
            u = CELL_M + select(-CAP_HALF_M, CAP_HALF_M, face == 2u);
            up = CAP_H_M * q.y;
        }
        let edge = slope_xy(c, CELL_M, along);
        world = vec3<f32>(slope_xy(c, u, along), terrain_height(edge) + lift + up);
        out.part = PART_CAP;
    }
    // Where the point is in the slope's frame, for the plating's seams.
    let local = world.xy - c.origin;
    var up_slope = select(local.x, local.y, (c.kind & 1u) != 0u);
    if (c.kind & 2u) == 0u {
        up_slope = CELL_M - up_slope;
    }
    out.slope = vec2<f32>(up_slope, select(local.y, local.x, (c.kind & 1u) != 0u));
    if (push.pass_kind & PASS_KIND_MASK) == PASS_SHADOW {
        out.clip = globals.shadow_cascades[push.pass_kind >> PASS_CASCADE_SHIFT] * vec4<f32>(world, 1.0);
    } else {
        out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    }
    out.world = world;
    return out;
}

@fragment
fn fs_shadow() {
}

// Soft dark line `width` wide at each integer of `x`, faded out once a pixel (`px`
// in units of `x`) is too coarse to show it.
fn seam(x: f32, width: f32, px: f32) -> f32 {
    let d = min(fract(x), 1.0 - fract(x));
    return (1.0 - smoothstep(width * 0.5, width * 0.5 + px, d)) * (1.0 - smoothstep(0.15, 0.4, px));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let eye = globals.camera.xyz;
    let p = in.world;
    // Flat faces: the normal from the surface itself, turned to the eye.
    let v = normalize(eye - p);
    var n = normalize(cross(dpdx(p), dpdy(p)));
    if dot(n, v) < 0.0 {
        n = -n;
    }
    let px = max(length(fwidth(p)), 0.004);
    let grain = noise_varied(p.xy + p.z * vec2<f32>(0.37, 0.61), 2.5);

    // Gunmetal steel: the plate darker, ribs and rail a lighter worn edge.
    var albedo = vec3<f32>(0.2, 0.215, 0.23) * (0.85 + 0.25 * grain.r);
    var metal = 0.75;
    var rough = 0.5 + 0.2 * grain.g;
    if in.part == PART_PLATE {
        // Panels between the ribs, 2.67 m up the slope, with a row of bolts along
        // each seam.
        let s = in.slope / vec2<f32>(2.67, 2.0);
        let seams = max(seam(s.x, 0.02, px / 2.67), seam(s.y, 0.04, px / 2.0));
        albedo *= 1.0 - 0.55 * seams;
        let bolts = seam(in.slope.y / 0.5, 0.2, px / 0.5) * seam(s.x, 0.06, px / 2.67);
        albedo = mix(albedo, albedo * 1.5, bolts * (1.0 - smoothstep(0.03, 0.1, px)));
        // Rust runs down from the seams; grime and dirt at the foot.
        let run = smoothstep(0.55, 0.85, noise_varied(vec2<f32>(in.slope.y * 0.9, in.slope.x * 0.08), 7.0).b);
        let foot = 1.0 - smoothstep(0.0, 2.0, in.slope.x);
        albedo = mix(albedo, vec3<f32>(0.2, 0.11, 0.06), 0.45 * run);
        albedo = mix(albedo, vec3<f32>(0.14, 0.12, 0.09), 0.6 * foot);
        metal = mix(metal, 0.2, max(run * 0.6, foot));
        rough = mix(rough, 0.85, max(run, foot));
    } else if in.part == PART_RIB {
        albedo *= 1.2;
    } else {
        albedo = vec3<f32>(0.28, 0.3, 0.32) * (0.9 + 0.2 * grain.r);
        rough = 0.42;
    }
    // Rain wets it as it does the ground.
    let soaked = weather_at(p.xy).w;
    albedo *= 1.0 - 0.25 * soaked;
    rough = mix(rough, 0.3, soaked * 0.75);

    var m: Pbr;
    m.albedo = albedo;
    m.metallic = metal;
    m.roughness = rough;
    m.emissive = vec3<f32>(0.0);
    let shadow = sun_shadow(p, n);
    let sky_vis = 0.85 * screen_ao(in.clip.xy);
    var color = shade_pbr_vis(m, n, v, globals.sun.xyz, shadow, sky_vis);
    color += albedo * lightning_light(p, n) * 0.35;
    color += local_lights(m, p, n, v);
    color = apply_fog_of_war(color, p.xy);
    color = apply_haze(color, p, eye);
    return vec4<f32>(color, 1.0);
}
