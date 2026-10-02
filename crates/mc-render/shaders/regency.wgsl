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

// Regency plate: dark gunmetal with a cool cast, a little brighter than lacquer black.
const REG_GUNMETAL: vec3<f32> = vec3<f32>(0.02, 0.021, 0.025);
// What catches the light on a plate's edge.
const REG_STEEL_LIT: vec3<f32> = vec3<f32>(0.2, 0.205, 0.225);
// The machinery: a dark bronze a little off true bronze, and the gold it polishes to.
const REG_BRONZE: vec3<f32> = vec3<f32>(0.3, 0.19, 0.095);
const REG_BRONZE_DEEP: vec3<f32> = vec3<f32>(0.075, 0.045, 0.025);
const REG_GOLD: vec3<f32> = vec3<f32>(0.8, 0.56, 0.25);

struct RegencyIn {
    // Model space, metres.
    local: vec3<f32>,
    // Metres of surface under one pixel.
    px: f32,
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

fn regency_plate(i: RegencyIn) -> RegencyLook {
    var out = regency_none();
    let fw = max(i.px, 1e-4);
    let o = reg_outline(i);
    // Each facet a plate of its own, a shade apart: what draws a faceted solid on black.
    out.tone = 0.7 + 0.6 * i.seed;
    out.rough = (fract(i.seed * 3.7) - 0.5) * 0.12;
    // The edge of a face catches the light, on any face broad enough to have one.
    let lip = i.scale * 0.07;
    let edged = smoothstep(lip * 2.0, lip * 4.0, o.small) * surf_resolved(lip, fw);
    out.lift = 0.8 * (1.0 - smoothstep(0.0, lip, o.e)) * edged;
    // A face given its own edges has no bevel from surface.wgsl: round its edge here.
    let bevel = i.scale * 0.035;
    var slope = vec3<f32>(0.0);
    if o.edge_form {
        let rise = 1.0 - smoothstep(0.0, bevel * 1.2, o.e);
        slope += o.dir * (rise * 0.7 * edged);
    }
    // The panel line: dark down it, its far lip lit, a groove in the relief.
    let w = i.scale * 0.018;
    let line = reg_panel_line(i, o);
    let seen = surf_resolved(w * 3.0, fw);
    let groove = surf_band(line.x, w, fw) * seen;
    out.tone *= 1.0 - 0.7 * groove;
    out.lift += 0.45 * surf_band(line.x - w * 2.2, w, fw) * seen;
    slope += line.yzw * (1.0 - smoothstep(w * 0.4, w * 1.6, line.x)) * 0.5 * seen;
    out.slope = slope * (1.0 - smoothstep(bevel * 3.0, bevel * 9.0, fw));
    // A faint brushing in the sheen only, never in the colour: no blotches.
    out.rough += 0.06 * surf_fbm3(i.local * vec3<f32>(0.3, 1.0, 1.0), i.scale * 0.8, fw);
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
    out.rough = 0.06 * surf_fbm3(i.local, i.scale * 0.8, fw);
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
        m.albedo = REG_GUNMETAL * clamp(0.6 + lum * 3.5, 0.6, 2.8) * look.tone;
        m.albedo = mix(m.albedo, REG_STEEL_LIT, saturate(look.lift) * 0.5);
        // A satin gunmetal: it catches the light, but a broad flat plate at the sun's
        // mirror angle must not glint white across its whole face.
        // Lit edges are brighter metal, not glossier: a glossy lip only picks up the
        // reflections' noise.
        m.roughness = clamp(clamp(m.roughness * 0.8, 0.56, 0.68) + look.rough, 0.45, 0.8);
        m.metallic = min(m.metallic, 0.25);
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
