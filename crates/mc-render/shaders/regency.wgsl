// Regency plate and machinery (`pattern::EMBER`): their colour and sheen. Prepended
// after surface.wgsl and metal.wgsl (it uses their noise, bands and scan) to shaders that contain the line
// `//!use regency` (entity.wgsl).
//
// The Regency look is clean: broad black plates under a red iridescent film, whose
// facets each catch the light a little differently, a lit edge where a face ends, and now and then one panel line
// that follows the face's own outline (`surf_ember_line`, cut by surface.wgsl's
// relief). Nothing is laid across the model regardless of its shape, and nothing on
// the plate is lit: a Regency model's red is in the slots it is built with. The
// dark graphite machinery under the plates is plain and smooth, a turned collar near each end of a
// column, polished where it is worked.
//
// Relief comes out as a slope in model space (`RegencyLook::slope`), which
// entity.wgsl carries into the world with `reg_to_world`.

// The foundation strips' plate (foundations.wgsl): a dark warm graphite.
const REG_GUNMETAL: vec3<f32> = vec3<f32>(0.1, 0.092, 0.082);
// Where the plate is worn through its film to bright steel (and the foundation
// strips' lit lips).
const REG_STEEL_LIT: vec3<f32> = vec3<f32>(0.36, 0.34, 0.315);
// The plate: dark charcoal steel under a thin film. Seen square on it is charcoal; as a
// face turns away the film's sheen rises, and its hue cycles crimson, deep red,
// copper and back with the angle, each facet started at its own point in the cycle.
const REG_FILM_BLACK: vec3<f32> = vec3<f32>(0.1, 0.092, 0.084);
const REG_FILM_RED: vec3<f32> = vec3<f32>(0.36, 0.018, 0.022);
const REG_FILM_DEEP: vec3<f32> = vec3<f32>(0.2, 0.008, 0.016);
const REG_FILM_COPPER: vec3<f32> = vec3<f32>(0.34, 0.075, 0.02);
// The second plate (`PLATING` under `pattern::EMBER`): a paler steel under a thinner
// film, for a two-tone hull whose bands of it stand against the charcoal.
const REG_FILM_PALE: vec3<f32> = vec3<f32>(0.15, 0.14, 0.13);
// The machinery under the plates: a dark graphite a shade off the plate, darker in
// its grooves, and the lighter steel it polishes to where it is worked.
const REG_WORKS: vec3<f32> = vec3<f32>(0.12, 0.112, 0.1);
const REG_WORKS_DEEP: vec3<f32> = vec3<f32>(0.02, 0.019, 0.017);
const REG_WORKS_POLISH: vec3<f32> = vec3<f32>(0.24, 0.225, 0.205);

struct RegencyIn {
    // Model space, metres, and the face's normal there (unit).
    local: vec3<f32>,
    n: vec3<f32>,
    // Metres of surface under one pixel, and the model-space step to the next pixel
    // right and down.
    px: f32,
    dl1: vec3<f32>,
    dl2: vec3<f32>,
    // Length of a typical plate on this model (`SurfaceIn::scale`).
    scale: f32,
    // `MeshVertex::face`: a rectangle frame, a tube's (negative z), the edge form
    // (negative w: distances to the face's own edges) or none (all zero).
    face: vec4<f32>,
    // How each of `face`'s four numbers grows in model space.
    grads: array<vec3<f32>, 4>,
    // Per face, zero to one, shared with its mirror twin.
    seed: f32,
    // A nanite face (`nanite::PATTERN_SLOT`): a lit slot down its middle.
    nanite: bool,
    sentry: u32,
    // Seconds, and the unit's own random, zero to one.
    time: f32,
    unit: f32,
}

struct RegencyLook {
    // Slope of the relief, model space, lying in the face.
    slope: vec3<f32>,
    // Multiplies the colour: each plate its own sheen, a panel line dark.
    tone: f32,
    // Lit metal: a plate's edge, machinery polished where it is worked.
    lift: f32,
    // Added to roughness.
    rough: f32,
    // Light of its own (HDR): a nanite slot's.
    glow: vec3<f32>,
}

fn regency_none() -> RegencyLook {
    return RegencyLook(vec3<f32>(0.0), 1.0, 0.0, 0.0, vec3<f32>(0.0));
}

// Regency plate, or Regency plate with nanite slots let in (both are coloured here).
fn regency_pattern(p: u32) -> bool {
    return p == PAT_EMBER || p == NANITE_PATTERN_SLOT
        || p == SENTRY_PATTERN || p == SENTRY_SLOT || p == SENTRY_RIM;
}

// A nanite face's slot, down the middle of the face the long way: how far across from
// its centre line (`x`) and how far in from its nearer end (`y`) the point is, and the
// slot's half width (`z`). `z` is zero on a face too narrow to hold one.
fn reg_nanite_slot(i: RegencyIn) -> vec3<f32> {
    let f = i.face;
    var across = 0.0;
    var along = 0.0;
    var broad = 0.0;
    // Half the face's length along the slot, where it is known.
    var long = 1e4;
    if f.w < 0.0 {
        // The longest edge, and of the other three the one that faces it most nearly: a
        // long face's two sides, or across a swept plate from its leading edge to the
        // far side of its point. The slot runs between them; the other two end it.
        let d = vec4<f32>(f.xyz, -f.w - FACE_EDGES_BIAS);
        var g = i.grads;
        g[3] = -g[3];
        var partner = 1u;
        var facing = 2.0;
        for (var k = 1u; k < 4u; k++) {
            let c = dot(normalize(g[0] + vec3<f32>(1e-9)), normalize(g[k] + vec3<f32>(1e-9)));
            if c < facing {
                facing = c;
                partner = k;
            }
        }
        if facing > -0.3 {
            return vec3<f32>(0.0);
        }
        let ends = select(select(vec2<f32>(d.y, d.z), vec2<f32>(d.y, d.w), partner == 2u), vec2<f32>(d.z, d.w), partner == 1u);
        across = abs(d.x - d[partner]) * 0.5;
        along = min(ends.x, ends.y);
        broad = (d.x + d[partner]) * 0.5;
    } else if f.z >= 0.0 && max(f.z, f.w) > 0.0 {
        // The long way; on a face near square, the way that climbs the model.
        var major = select(1u, 0u, f.z >= f.w);
        if max(f.z, f.w) < min(f.z, f.w) * 1.25 {
            let climb0 = abs(i.grads[0].z) * inverseSqrt(max(dot(i.grads[0], i.grads[0]), 1e-12));
            let climb1 = abs(i.grads[1].z) * inverseSqrt(max(dot(i.grads[1], i.grads[1]), 1e-12));
            major = select(1u, 0u, climb0 > climb1);
        }
        let minor = 1u - major;
        across = abs(f[minor]);
        along = f[major + 2u] - abs(f[major]);
        broad = f[minor + 2u];
        long = f[major + 2u];
    }
    let half = clamp(broad * 0.16, i.scale * 0.035, i.scale * 0.11);
    // Too narrow, or too short to run past its rounded ends: no slot, not a stub.
    if broad < i.scale * 0.08 || long < half * 4.0 {
        return vec3<f32>(across, along, 0.0);
    }
    return vec3<f32>(across, along, half);
}

// The slot cut in a nanite face: dark glazing, framed by a groove and a lit lip, the
// nanites streaming along it in violet, thickening and thinning, a slow beat over them.
fn reg_nanite(i: RegencyIn, out_in: RegencyLook, fw: f32) -> RegencyLook {
    var out = out_in;
    let slot = reg_nanite_slot(i);
    let half = slot.z;
    if half <= 0.0 {
        return out;
    }
    let end = half * 1.5;
    // Inside the slot, its ends rounded off.
    let d = max(slot.x - half, end - slot.y);
    let inside = 1.0 - smoothstep(-fw, fw, d);
    let lip = surf_band(d - fw * 1.5, max(half * 0.18, fw * 0.5), fw);
    out.tone *= 1.0 - 0.85 * inside;
    out.lift += 0.5 * lip;
    out.rough -= 0.25 * inside;
    // The nanites: knots running along the slot through a finer churn, in model space so
    // neighbouring faces' slots run as one stream.
    let flow = i.local + vec3<f32>(i.unit * 41.0, i.unit * 13.0, -i.time * NANITE_FLOW_SPEED);
    let knot = smoothstep(0.35, 0.8, surf_noise3(flow * vec3<f32>(1.3, 1.3, 0.7)));
    let churn = 0.5 + surf_fbm3(flow * 2.0, i.scale * 0.08, fw);
    let seen = surf_resolved(half, fw);
    let stream = mix(0.55, 0.25 + 1.3 * knot * churn, seen);
    let beat = 0.8 + 0.2 * sin(i.time * 1.7 + i.unit * 31.0);
    // Brightest down the slot's middle.
    let core = 1.0 - smoothstep(0.0, half, slot.x) * 0.6;
    let violet = vec3<f32>(NANITE_R, NANITE_G, NANITE_B);
    out.glow = violet * inside * stream * core * beat * 0.9;
    return out;
}

// Where a point lies on its face, whatever form the face's outline came in: `e`
// metres in from the nearest edge and `dir` the way that grows (model space), `e2` and
// `dir2` the next edge round, `small` how broad the face is (half its narrow span),
// `edge_form` true where the face's own edges were stored (no relief from surface.wgsl).
struct RegOutline {
    e: f32,
    dir: vec3<f32>,
    e2: f32,
    dir2: vec3<f32>,
    small: f32,
    tube: bool,
    edge_form: bool,
}

fn reg_outline(i: RegencyIn) -> RegOutline {
    var o = RegOutline(1e4, vec3<f32>(0.0), 1e4, vec3<f32>(0.0), 0.0, false, false);
    let f = i.face;
    if f.w < 0.0 {
        var d = vec4<f32>(f.xyz, -f.w - FACE_EDGES_BIAS);
        var g = i.grads;
        g[3] = -g[3];
        for (var k = 0u; k < 4u; k++) {
            if d[k] < o.e {
                o.e2 = o.e;
                o.dir2 = o.dir;
                o.e = d[k];
                o.dir = g[k];
            } else if d[k] < o.e2 {
                o.e2 = d[k];
                o.dir2 = g[k];
            }
        }
        // Broad enough where the two nearest edges are far apart.
        o.small = 0.5 * (o.e + o.e2);
        o.edge_form = true;
    } else if f.z < 0.0 {
        o.e = f.w - abs(f.y);
        o.dir = -sign(f.y) * i.grads[1];
        o.small = f.w;
        o.tube = true;
    } else if max(f.z, f.w) > 0.0 {
        let q = f.zw - abs(f.xy);
        let gx = -sign(f.x) * i.grads[0];
        let gy = -sign(f.y) * i.grads[1];
        o.small = min(f.z, f.w);
        if q.x < q.y {
            o = RegOutline(q.x, gx, q.y, gy, o.small, false, false);
        } else {
            o = RegOutline(q.y, gy, q.x, gx, o.small, false, false);
        }
    }
    return o;
}

// The panel line: on about half of the broad faces, one line let in round the face's
// outline, its corners cut. Its distance (`x`) and the way that grows (`yzw`); far off
// on a face with none.
fn reg_panel_line(i: RegencyIn, o: RegOutline) -> vec4<f32> {
    let none = vec4<f32>(1e4, 0.0, 0.0, 0.0);
    if o.tube || fract(i.seed * 7.13) < 0.45 {
        return none;
    }
    var inset = i.scale * 0.3;
    if !o.edge_form {
        if o.small < i.scale * 0.9 {
            return none;
        }
        inset = clamp(o.small * 0.2, i.scale * 0.18, i.scale * 0.5);
    } else if o.e + o.e2 < inset * 2.6 {
        // A sliver, or a face too small to hold the line.
        return none;
    }
    var e = o.e;
    var dir = o.dir;
    let corner = (o.e + o.e2 - inset * 1.6) * 0.7071;
    let both = o.dir + o.dir2;
    if corner < e && dot(both, both) > 0.25 {
        e = corner;
        dir = normalize(both);
    }
    let sign_out = select(-1.0, 1.0, e > inset);
    return vec4<f32>(abs(e - inset), dir * sign_out);
}

// The seams between the big plates a face is laid in: courses running along the face's
// longest edge (or a rectangle's long side), each course cut across into plates, the
// cuts staggered course to course like brickwork. The distance to the nearest seam
// (`d.x`) and the way it grows (`d.yzw`), far off on a face with none; `id` names
// the plate, for its own shade.
struct RegSeams {
    d: vec4<f32>,
    id: f32,
}

fn reg_seams(i: RegencyIn, o: RegOutline) -> RegSeams {
    var out = RegSeams(vec4<f32>(1e4, 0.0, 0.0, 0.0), i.seed);
    let want = i.scale * mix(1.1, 1.7, fract(i.seed * 5.31));
    if o.tube {
        return out;
    }
    var u = 0.0;
    var pitch = want;
    var g = vec3<f32>(0.0);
    var last = 1e4;
    if o.edge_form {
        // From the longest edge, inward.
        u = i.face.x;
        g = i.grads[0];
        // Stop short of an edge running the same way: no seam hugs the far side.
        let g0 = g * inverseSqrt(max(dot(g, g), 1e-12));
        let d = vec4<f32>(i.face.xyz, -i.face.w - FACE_EDGES_BIAS);
        var gs = i.grads;
        gs[3] = -gs[3];
        for (var k = 1u; k < 4u; k++) {
            let gk = gs[k] * inverseSqrt(max(dot(gs[k], gs[k]), 1e-12));
            if dot(gk, g0) < -0.8 {
                last = min(last, d[k]);
            }
        }
    } else if max(i.face.z, i.face.w) > 0.0 {
        // Across a rectangle's long side, fitted to it.
        let long_x = i.face.z >= i.face.w;
        let half = select(i.face.w, i.face.z, long_x);
        u = select(i.face.y, i.face.x, long_x) + half;
        g = select(i.grads[1], i.grads[0], long_x);
        pitch = 2.0 * half / surf_fit(2.0 * half, want);
        last = 2.0 * half - u;
    } else {
        return out;
    }
    let gn = g * inverseSqrt(max(dot(g, g), 1e-12));
    let c = u / pitch;
    let course = floor(c);
    let n = round(c);
    if n > 0.5 && last > pitch * 0.45 {
        let off = u - n * pitch;
        out.d = vec4<f32>(abs(off), gn * sign(off + 1e-6));
    }
    // Across the course, along the edge: cuts at a longer pitch, staggered per course.
    let t = normalize(cross(i.n, gn) + vec3<f32>(1e-6, 0.0, 0.0));
    let cross_pitch = want * mix(1.4, 2.0, fract(i.seed * 2.17));
    let v = dot(i.local, t) / cross_pitch + hash11(course * 3.7 + i.seed * 17.0);
    let off2 = (v - round(v)) * cross_pitch;
    if abs(off2) < out.d.x {
        out.d = vec4<f32>(abs(off2), t * sign(off2 + 1e-6));
    }
    out.id = hash11(course * 7.31 + floor(v) * 2.93 + i.seed * 91.0);
    return out;
}

// The steel scan (`metal_scan::LAYER`, metal.wgsl): worn iron, its grain, scratches
// and sheen. Its means (scripts/import-metal.py prints them).
const REG_SCAN_LUM: f32 = 0.548;
const REG_SCAN_ROUGH: f32 = 0.16;

fn reg_scan(i: RegencyIn) -> MetalScan {
    return metal_scan(ScanAt(i.local, i.n, i.dl1, i.dl2, i.scale, i.seed, METAL_SCAN_LAYER, REG_SCAN_LUM, REG_SCAN_ROUGH, METAL_SCAN_TILE_M));
}

fn regency_plate(i: RegencyIn) -> RegencyLook {
    var out = regency_none();
    let fw = max(i.px, 1e-4);
    let o = reg_outline(i);
    // Each facet a plate of its own, a shade apart: what draws a faceted solid.
    out.tone = 0.78 + 0.44 * i.seed;
    out.rough = (fract(i.seed * 3.7) - 0.5) * 0.12;
    // The edge of a face is worn bright, more in some places than others.
    let lip = i.scale * 0.07;
    let edged = smoothstep(lip * 2.0, lip * 4.0, o.small) * surf_resolved(lip, fw);
    let worn = 0.5 + saturate(surf_fbm3(i.local + vec3<f32>(3.1, 7.7, 1.3), i.scale * 0.35, fw) + 0.5);
    out.lift = 0.6 * worn * (1.0 - smoothstep(0.0, lip, o.e)) * edged;
    // A face given its own edges has no bevel from surface.wgsl: round its edge here.
    let bevel = i.scale * 0.035;
    var slope = vec3<f32>(0.0);
    if o.edge_form {
        let rise = 1.0 - smoothstep(0.0, bevel * 1.2, o.e);
        slope += o.dir * (rise * 0.7 * edged);
    }
    let w = i.scale * 0.016;
    let seen = surf_resolved(w * 3.0, fw);
    // The seams between the big plates: dark, the plate beyond each a shade apart,
    // its edge catching the light.
    let seams = reg_seams(i, o);
    let near_edge = smoothstep(lip, lip * 2.5, o.e);
    let seam = surf_band(seams.d.x, w, fw) * seen * near_edge;
    // Grime packed along the seam either side, a little.
    let packed = 1.0 - 0.25 * (1.0 - smoothstep(w, w * 6.0, seams.d.x)) * seen * near_edge;
    out.tone *= (0.84 + 0.32 * seams.id) * (1.0 - 0.85 * seam) * packed;
    out.lift += 0.65 * surf_band(seams.d.x - w * 2.0, w, fw) * seen * near_edge;
    slope += seams.d.yzw * (1.0 - smoothstep(w * 0.3, w * 1.4, seams.d.x)) * 0.5 * seen * near_edge;
    // The panel line round the face's outline: dark down it, its far lip lit.
    let line = reg_panel_line(i, o);
    let groove = surf_band(line.x, w, fw) * seen;
    out.tone *= 1.0 - 0.7 * groove;
    out.lift += 0.4 * surf_band(line.x - w * 2.2, w, fw) * seen;
    slope += line.yzw * (1.0 - smoothstep(w * 0.4, w * 1.6, line.x)) * 0.5 * seen;
    // The steel itself: grain, scratches and sheen from the scan.
    let steel = reg_scan(i);
    out.tone *= 1.0 + 1.0 * steel.tone;
    out.rough += 1.6 * steel.rough;
    // Its scratches: bright, polished metal down each, the deepest most.
    let scratch = smoothstep(0.3, 0.85, steel.scratch);
    out.lift += 0.75 * scratch;
    out.rough -= 0.2 * scratch;
    // The seams' relief fades once finer than a pixel; the scan's is filtered by its mips.
    out.slope = slope * (1.0 - smoothstep(bevel * 3.0, bevel * 9.0, fw)) + steel.slope * 0.8;
    // Broad, faint unevenness, so a big plate never shows the scan's repeat.
    out.tone *= 1.0 + 0.15 * surf_fbm3(i.local + vec3<f32>(21.0, 4.0, 13.0), i.scale * 1.6, fw);
    // Grime in the seams dulls them.
    out.rough += 0.3 * (1.0 - smoothstep(w, w * 5.0, min(seams.d.x, line.x))) * seen;
    if i.sentry >= SENTRY_PATTERN && i.sentry <= SENTRY_RIM {
        // Small triangular laminates within the large authored armour facets.
        // Project onto the plate's dominant plane, in model space, without swim.
        let an = abs(i.n);
        var uv = select(select(i.local.xy, i.local.xz, an.y > an.z),
            i.local.yz, an.x > max(an.y, an.z)) / (i.scale * 0.32);
        uv += vec2<f32>(sin(dot(uv, vec2<f32>(1.7, 0.9))),
            sin(dot(uv, vec2<f32>(0.8, 1.3)))) * 0.13;
        let cell = floor(uv);
        let q = fract(uv);
        let diagonal = select(q.x + q.y - 1.0, q.x - q.y,
            hash11(dot(cell, vec2<f32>(17.1, 93.7))) > 0.5);
        let seed = hash11(dot(cell, vec2<f32>(71.3, 29.7)) + select(0.0, 5.8, diagonal > 0.0));
        let cut = min(min(min(q.x, q.y), min(1.0-q.x, 1.0-q.y)), abs(diagonal)*0.707);
        let line = 1.0 - smoothstep(0.006, max(0.014, fw/i.scale), cut);
        out.tone = (0.78 + 0.32 * seed) * (1.0 - 0.22 * line) * (1.0 + steel.tone * 0.25);
        out.lift *= 0.25;
        out.slope *= 0.22;
        out.rough = 0.1 + 0.08 * seed;
        if i.sentry == SENTRY_SLOT {
            var optic = i;
            optic.scale = min(i.scale, 0.8);
            let slot = reg_nanite_slot(optic);
            if slot.z > 0.0 {
                let half = max(slot.z * 0.32, 0.035);
                let d = max(slot.x - half, slot.z * 2.1 - slot.y);
                let lit = 1.0 - smoothstep(-fw, fw, d);
                out.tone *= 1.0 - 0.9 * (1.0-smoothstep(-fw, fw, d-half));
                out.glow = vec3<f32>(7.0, 0.035, 0.015) * lit;
            }
        } else if i.sentry == SENTRY_RIM {
            let inset = max(i.scale * 0.028, o.small * 0.18);
            let lit = surf_band(o.e - inset, i.scale * 0.013, fw);
            out.glow = vec3<f32>(3.5, 0.025, 0.012) * lit;
        }
    }
    if i.nanite {
        out = reg_nanite(i, out, fw);
    }
    return out;
}

// Height along a machinery column: a turned collar a little in from each end.
fn reg_collar_h(d: f32, scale: f32) -> f32 {
    let at = scale * 0.22;
    let width = scale * 0.07;
    return 1.0 + 0.5 * (1.0 - smoothstep(width * 0.6, width, abs(d - at)));
}

fn regency_works(i: RegencyIn) -> RegencyLook {
    var out = regency_none();
    let fw = max(i.px, 1e-4);
    let o = reg_outline(i);
    out.tone = 0.9 + 0.2 * i.seed;
    if o.tube && o.small > i.scale * 0.6 {
        // A column or an axle: plain, a collar near each end.
        let d = max(fw * 0.75, i.scale * 0.004);
        let h0 = reg_collar_h(o.e, i.scale);
        let h1 = reg_collar_h(o.e + d, i.scale);
        let seen = surf_resolved(i.scale * 0.07, fw);
        out.slope = o.dir * ((h1 - h0) * i.scale * 0.03 / d) * seen;
        out.lift = (h0 - 1.0) * 1.4 * seen;
    } else if !o.tube {
        // A block: polished along its edges.
        let lip = i.scale * 0.04;
        out.lift = 0.5 * (1.0 - smoothstep(0.0, lip, o.e)) * smoothstep(lip * 2.0, lip * 4.0, o.small) * surf_resolved(lip, fw);
    }
    // The same scan's grain and scratches in the machinery.
    let worked = reg_scan(i);
    out.tone *= 1.0 + 0.5 * worked.tone;
    out.lift += 0.35 * smoothstep(0.3, 0.85, worked.scratch);
    out.slope += worked.slope * 0.6;
    out.rough = 0.06 * surf_fbm3(i.local, i.scale * 0.8, fw) + 0.8 * worked.rough;
    return out;
}

// Plate or machinery, by the material the model painted.
fn regency_look(i: RegencyIn, works: bool) -> RegencyLook {
    if works {
        return regency_works(i);
    }
    return regency_plate(i);
}

// The film's colour at `cos_v` (the face's normal against the view).
fn reg_film(cos_v: f32, seed: f32, pale: bool) -> vec3<f32> {
    let off = 1.0 - saturate(cos_v);
    let rise = smoothstep(0.3, 0.9, off) * select(1.0, 0.7, pale);
    // Ping-pong crimson -> deep red -> copper -> deep red -> crimson.
    let h = abs(fract(off * 1.6 + seed * 0.8) * 2.0 - 1.0) * 2.0;
    let hue = mix(mix(REG_FILM_RED, REG_FILM_DEEP, smoothstep(0.0, 1.0, h)), REG_FILM_COPPER, smoothstep(1.0, 2.0, h));
    return mix(select(REG_FILM_BLACK, REG_FILM_PALE, pale), hue, rise);
}

// Regency colours over the shared palette's (`lum` keeps the model's own light and
// shade): plate (`PLATING_DARK`, and the paler `PLATING`), the darker trim (`ACCENT`) and
// the dark graphite machinery (`METAL`).
fn regency_paint(m_in: Pbr, works: bool, pale: bool, look: RegencyLook, cos_v: f32, seed: f32, sentry: bool) -> Pbr {
    var m = m_in;
    let lum = dot(m.albedo, vec3<f32>(0.3, 0.59, 0.11));
    if works {
        let base = mix(REG_WORKS_DEEP, REG_WORKS, saturate(look.tone)) * clamp(0.75 + lum, 0.75, 1.3);
        m.albedo = mix(base, REG_WORKS_POLISH, saturate(look.lift));
        m.metallic = 0.6;
        m.roughness = clamp(0.4 + look.rough - 0.18 * look.lift, 0.22, 0.6);
    } else {
        // Steel under an iridescent film, not paint: a metal, so its colour is what it
        // reflects, charcoal seen square on and a crimson to copper sheen as it turns away; what makes it read as real is its
        // sheen varying, smudged here, worn through to bright steel at its edges, dull
        // with grime in its seams.
        let tone = look.tone * clamp(0.8 + lum, 0.8, 1.2);
        m.albedo = mix(reg_film(cos_v, seed, pale) * tone, REG_STEEL_LIT, saturate(look.lift) * 0.5);
        m.metallic = 0.55;
        m.roughness = clamp(0.42 + look.rough - 0.1 * saturate(look.lift), 0.24, 0.8);
    }
    if sentry && !works {
        let red = smoothstep(0.35, 0.95, 1.0 - saturate(cos_v)) * 0.65;
        m.albedo = mix(vec3<f32>(0.065, 0.069, 0.073), vec3<f32>(0.22, 0.009, 0.014), red) * look.tone;
        m.albedo += REG_STEEL_LIT * saturate(look.lift) * 0.12;
        m.metallic = 0.42;
        m.roughness = clamp(0.48 + look.rough, 0.4, 0.72);
    }
    return m;
}

// A slope in model space, lying in the face, carried into the world by the pixel's
// own footprint (`dl` model, `dw` world, over one pixel each way).
fn reg_to_world(g: vec3<f32>, dl1: vec3<f32>, dl2: vec3<f32>, dw1: vec3<f32>, dw2: vec3<f32>) -> vec3<f32> {
    let a11 = dot(dl1, dl1);
    let a12 = dot(dl1, dl2);
    let a22 = dot(dl2, dl2);
    let det = a11 * a22 - a12 * a12;
    if det < 1e-24 {
        return vec3<f32>(0.0);
    }
    let b1 = dot(g, dl1);
    let b2 = dot(g, dl2);
    let w = dw1 * ((b1 * a22 - b2 * a12) / det) + dw2 * ((a11 * b2 - a12 * b1) / det);
    return w * (length(g) / max(length(w), 1e-9));
}

// The gradient of a face coordinate `f` in model space (`df` its change over one pixel each way).
fn reg_face_grad(df1: f32, df2: f32, dl1: vec3<f32>, dl2: vec3<f32>) -> vec3<f32> {
    let a11 = dot(dl1, dl1);
    let a12 = dot(dl1, dl2);
    let a22 = dot(dl2, dl2);
    let det = a11 * a22 - a12 * a12;
    if det < 1e-24 {
        return vec3<f32>(0.0);
    }
    return dl1 * ((df1 * a22 - df2 * a12) / det) + dl2 * ((a11 * df2 - a12 * df1) / det);
}
