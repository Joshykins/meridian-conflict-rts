// Regency plate and bronze (`pattern::EMBER`): their colour and sheen. Prepended
// after surface.wgsl (it uses its noise and bands) to shaders that contain the line
// `//!use regency` (entity.wgsl).
//
// The Regency look is clean: broad satin plates whose facets each catch the light a
// little differently, a lit edge where a face ends, and now and then one panel line
// that follows the face's own outline (`surf_ember_line`, cut by surface.wgsl's
// relief). Nothing is laid across the model regardless of its shape, and nothing on
// the plate is lit: a Regency model's red is in the slots it is built with. The
// bronze under the plates is plain and smooth, a turned collar near each end of a
// column, polished where it is worked.
//
// Relief comes out as a slope in model space (`RegencyLook::slope`), which
// entity.wgsl carries into the world with `reg_to_world`.

// Regency plate: dark gunmetal with a cool cast, grey enough for its seams and wear to read.
const REG_GUNMETAL: vec3<f32> = vec3<f32>(0.042, 0.044, 0.049);
// The plate as metal (its reflectance), and where it is worn bright.
const REG_STEEL: vec3<f32> = vec3<f32>(0.11, 0.11, 0.115);
const REG_STEEL_LIT: vec3<f32> = vec3<f32>(0.34, 0.34, 0.35);
// The machinery: a dark bronze a little off true bronze, and the gold it polishes to.
const REG_BRONZE: vec3<f32> = vec3<f32>(0.3, 0.19, 0.095);
const REG_BRONZE_DEEP: vec3<f32> = vec3<f32>(0.075, 0.045, 0.025);
const REG_GOLD: vec3<f32> = vec3<f32>(0.8, 0.56, 0.25);

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
}

struct RegencyLook {
    // Slope of the relief, model space, lying in the face.
    slope: vec3<f32>,
    // Multiplies the colour: each plate its own sheen, a panel line dark.
    tone: f32,
    // Lit metal: a plate's edge (steel), bronze polished to gold.
    lift: f32,
    // Added to roughness.
    rough: f32,
}

fn regency_none() -> RegencyLook {
    return RegencyLook(vec3<f32>(0.0), 1.0, 0.0, 0.0);
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

// The steel scan (`metal_scan`, data/textures/metal): its grain, scratches and sheen,
// as variation about its own mean, never its colour. Mapped in model space by three
// planar projections blended by the face's normal, shifted per face (`seed`) so
// neighbouring plates never show the same patch; the GPU's mips take it to its mean
// with distance.
struct RegScan {
    // Multiplies the colour; added to roughness; slope of its relief (model space);
    // how deep a scratch is here (0 none).
    tone: f32,
    rough: f32,
    slope: vec3<f32>,
    scratch: f32,
}

// The scan's means (scripts/import-metal.py prints them): what reads as "no change".
const REG_SCAN_LUM: f32 = 0.548;
const REG_SCAN_ROUGH: f32 = 0.16;

fn reg_scan_sample(uv: vec2<f32>, d1: vec2<f32>, d2: vec2<f32>, u: vec3<f32>, v: vec3<f32>) -> RegScan {
    let c = textureSampleGrad(terrain_materials, repeat_sampler, uv, METAL_SCAN_LAYER, d1, d2);
    let nm = textureSampleGrad(terrain_materials, repeat_sampler, uv, METAL_SCAN_LAYER + 1, d1, d2);
    let lum = dot(c.rgb, vec3<f32>(0.3, 0.59, 0.11));
    let t = nm.xy * 2.0 - 1.0;
    // A tangent normal leaning +u means the surface falls toward +u.
    return RegScan(lum / REG_SCAN_LUM - 1.0, c.a - REG_SCAN_ROUGH, -(t.x * u + t.y * v), nm.z);
}

fn reg_scan_mix(a: RegScan, b: RegScan, t: f32) -> RegScan {
    return RegScan(mix(a.tone, b.tone, t), mix(a.rough, b.rough, t), mix(a.slope, b.slope, t), mix(a.scratch, b.scratch, t));
}

fn reg_scan_add(a: RegScan, b: RegScan, w: f32) -> RegScan {
    return RegScan(a.tone + b.tone * w, a.rough + b.rough * w, a.slope + b.slope * w, a.scratch + b.scratch * w);
}

// One projection of the scan, never repeating on a grid: a slow noise picks, place
// to place, which of eight offsets of the scan shows, and blends across from one to
// the next (Quilez, "texture repetition").
fn reg_scan_axis(uv: vec2<f32>, d1: vec2<f32>, d2: vec2<f32>, u: vec3<f32>, v: vec3<f32>, axis: f32) -> RegScan {
    let pick = surf_noise3(vec3<f32>(uv * 0.45, axis * 17.0)) * 8.0;
    let i = floor(pick);
    let f = fract(pick);
    let o0 = vec2<f32>(hash11(i * 13.7 + axis), hash11(i * 7.1 + axis + 0.5));
    let o1 = vec2<f32>(hash11((i + 1.0) * 13.7 + axis), hash11((i + 1.0) * 7.1 + axis + 0.5));
    let a = reg_scan_sample(uv + o0, d1, d2, u, v);
    let b = reg_scan_sample(uv + o1, d1, d2, u, v);
    return reg_scan_mix(a, b, smoothstep(0.25, 0.75, f));
}

fn reg_scan(i: RegencyIn) -> RegScan {
    var w = pow(abs(i.n), vec3<f32>(4.0));
    w /= max(w.x + w.y + w.z, 1e-6);
    // A big model is seen from further off: its scan is laid coarser, so it still reads.
    let k = 1.0 / (METAL_SCAN_TILE_M * clamp(i.scale * 0.7, 1.0, 3.5));
    let shift = vec2<f32>(fract(i.seed * 7.91), fract(i.seed * 3.37));
    let p = i.local * k;
    let a = i.dl1 * k;
    let b = i.dl2 * k;
    var out = RegScan(0.0, 0.0, vec3<f32>(0.0), 0.0);
    if w.x > 0.01 {
        out = reg_scan_add(out, reg_scan_axis(p.yz + shift, a.yz, b.yz, vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), 1.0), w.x);
    }
    if w.y > 0.01 {
        out = reg_scan_add(out, reg_scan_axis(p.xz + shift, a.xz, b.xz, vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), 2.0), w.y);
    }
    if w.z > 0.01 {
        out = reg_scan_add(out, reg_scan_axis(p.xy + shift, a.xy, b.xy, vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), 3.0), w.z);
    }
    return out;
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
    return out;
}

// Height along a bronze column: a turned collar a little in from each end.
fn reg_collar_h(d: f32, scale: f32) -> f32 {
    let at = scale * 0.22;
    let width = scale * 0.07;
    return 1.0 + 0.5 * (1.0 - smoothstep(width * 0.6, width, abs(d - at)));
}

fn regency_bronze(i: RegencyIn) -> RegencyLook {
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
    // The same scan's grain and scratches in the bronze.
    let worked = reg_scan(i);
    out.tone *= 1.0 + 0.5 * worked.tone;
    out.lift += 0.35 * smoothstep(0.3, 0.85, worked.scratch);
    out.slope += worked.slope * 0.6;
    out.rough = 0.06 * surf_fbm3(i.local, i.scale * 0.8, fw) + 0.8 * worked.rough;
    return out;
}

// Plate or bronze, by the material the model painted.
fn regency_look(i: RegencyIn, bronze: bool) -> RegencyLook {
    if bronze {
        return regency_bronze(i);
    }
    return regency_plate(i);
}

// Regency colours over the shared palette's (`lum` keeps the model's own light and
// shade): plate, the darker trim (`ACCENT`) and the bronze machinery (`METAL`).
fn regency_paint(m_in: Pbr, bronze: bool, look: RegencyLook) -> Pbr {
    var m = m_in;
    let lum = dot(m.albedo, vec3<f32>(0.3, 0.59, 0.11));
    if bronze {
        let base = mix(REG_BRONZE_DEEP, REG_BRONZE, saturate(look.tone)) * clamp(0.75 + lum, 0.75, 1.3);
        m.albedo = mix(base, REG_GOLD, saturate(look.lift));
        m.metallic = 0.88;
        m.roughness = clamp(0.4 + look.rough - 0.18 * look.lift, 0.22, 0.6);
    } else {
        // Bare dark steel, not paint: a metal, so its colour is what it reflects, and
        // what makes it read as real is its sheen varying, smudged here, polished bright
        // where it is worn, dull with grime in its seams.
        let tone = look.tone * clamp(0.8 + lum, 0.8, 1.2);
        m.albedo = mix(REG_STEEL * tone, REG_STEEL_LIT, saturate(look.lift) * 0.65);
        m.metallic = 0.65;
        m.roughness = clamp(0.55 + look.rough - 0.1 * saturate(look.lift), 0.3, 0.88);
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
