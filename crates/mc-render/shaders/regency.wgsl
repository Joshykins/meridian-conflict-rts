// Regency plate and bronze (`pattern::EMBER`): the detail cut into them and their
// colours. Prepended after surface.wgsl (it uses its noise and bands) to shaders
// that contain the line `//!use regency` (entity.wgsl).
//
// A Regency walker is built of faceted solids. Few of its faces carry a frame of
// their own, and its plate is near black, so a seam cannot just be a darker line.
// The detail here is cut in the model's own space, in three dimensions: panels are
// the cells of a block of space cut up and cut again, and a seam is where a face
// crosses a cell wall, so it runs on round a solid from facet to facet the way a
// panel line runs round a real hull. Every distance is measured along the face, so
// a wall met at a glancing angle draws a line as fine as any other. On black, what
// draws the panels is light: a sunk seam with a bright lip, a sheen that differs
// panel to panel, polished bolt heads, vents, and here and there a red line let
// into the plate. The bronze machinery under the plate is turned and engraved,
// polished gold where it is worked and dark in its grooves.
//
// Relief comes out as a slope in model space (`RegencyLook::slope`), which
// entity.wgsl carries into the world with `reg_to_world`.

// Regency plate: dark gunmetal with a cool cast, a little brighter than lacquer black.
const REG_GUNMETAL: vec3<f32> = vec3<f32>(0.02, 0.021, 0.025);
// What catches the light on a plate's lip or a bolt head.
const REG_STEEL_LIT: vec3<f32> = vec3<f32>(0.2, 0.205, 0.225);
// The machinery: a dark bronze a little off true bronze, dark in its grooves, and the
// gold it polishes to where it is worked.
const REG_BRONZE: vec3<f32> = vec3<f32>(0.3, 0.19, 0.095);
const REG_BRONZE_DEEP: vec3<f32> = vec3<f32>(0.075, 0.045, 0.025);
const REG_GOLD: vec3<f32> = vec3<f32>(0.8, 0.56, 0.25);

// The walls panels are cut along, in the mirrored model space (y folded onto the
// left flank, so both flanks match): one swept back and up the way the plates lie,
// one square to the flank, one square to both.
fn reg_axis(k: u32) -> vec3<f32> {
    switch k {
        case 0u: { return vec3<f32>(0.8192, 0.0, 0.5736); }
        case 1u: { return vec3<f32>(0.0, 1.0, 0.0); }
        default: { return vec3<f32>(-0.5736, 0.0, 0.8192); }
    }
}

struct RegencyIn {
    // Model space, metres, and the face's own normal there (unit).
    local: vec3<f32>,
    n: vec3<f32>,
    // Metres of surface under one pixel.
    px: f32,
    // Length of a typical plate on this model (`SurfaceIn::scale`).
    scale: f32,
    time: f32,
    health: f32,
    // A tube's frame (`MeshVertex::face` wrapping): metres along it, and the way that
    // grows in model space; zero `along_dir` on anything else.
    along: f32,
    along_dir: vec3<f32>,
}

struct RegencyLook {
    // Slope of the relief, model space, lying in the face.
    slope: vec3<f32>,
    // Multiplies the plate's colour: seams dark, panels of different sheen.
    tone: f32,
    // Lit metal: a plate's lip or a bolt head (steel), bronze polished to gold.
    lift: f32,
    // Added to roughness.
    rough: f32,
    emissive: vec3<f32>,
}

fn regency_none() -> RegencyLook {
    return RegencyLook(vec3<f32>(0.0), 1.0, 0.0, 0.0, vec3<f32>(0.0));
}

// One panel: the cell of `q` in a grid of `size`, cut again up to three times.
struct RegPanel {
    lo: vec3<f32>,
    hi: vec3<f32>,
    id: f32,
}

fn reg_panel(q: vec3<f32>, size: f32) -> RegPanel {
    let c = floor(q / size);
    var lo = c * size;
    var hi = lo + vec3<f32>(size);
    var id = surf_hash3(c * 1.37 + vec3<f32>(3.1, 7.7, 1.3));
    for (var k = 0u; k < 3u; k++) {
        let r0 = hash11(id * 91.7 + f32(k) * 7.31);
        let r1 = hash11(id * 37.3 + f32(k) * 3.17 + 0.5);
        let r2 = hash11(id * 13.9 + f32(k) * 1.93 + 0.25);
        if k > 0u && r0 < 0.25 {
            break;
        }
        // Across the longest side, mostly; now and then across another.
        let ext = hi - lo;
        var axis = 0u;
        if ext.y > ext[axis] {
            axis = 1u;
        }
        if ext.z > ext[axis] {
            axis = 2u;
        }
        if r1 < 0.3 {
            axis = (axis + 1u + u32(r1 * 6.0)) % 3u;
        }
        let cut = lo[axis] + ext[axis] * mix(0.3, 0.7, r2);
        if q[axis] < cut {
            hi[axis] = cut;
            id = hash11(id * 17.3 + 0.21);
        } else {
            lo[axis] = cut;
            id = hash11(id * 23.9 + 0.67);
        }
    }
    return RegPanel(lo, hi, id);
}

// Where a point lies in its panel, measured along the face: `e` to the outline (its
// corners cut off by `chamfer`) and `dir` the way it grows; `s2` to the next wall
// round and `dir2`; `small` the panel's narrowest span across the face.
struct RegEdges {
    e: f32,
    dir: vec3<f32>,
    s2: f32,
    dir2: vec3<f32>,
    small: f32,
}

fn reg_edges(q: vec3<f32>, p: RegPanel, n: vec3<f32>, chamfer: f32) -> RegEdges {
    var s1 = 1e4;
    var s2 = 1e4;
    var t1 = vec3<f32>(0.0);
    var t2 = vec3<f32>(0.0);
    var small = 1e4;
    for (var k = 0u; k < 3u; k++) {
        let a = reg_axis(k);
        let along = a - n * dot(a, n);
        let g = length(along);
        // A wall the face runs along draws no line on it.
        if g < 0.1 {
            continue;
        }
        let t = along / g;
        let below = (q[k] - p.lo[k]) / g;
        let above = (p.hi[k] - q[k]) / g;
        let s = min(below, above);
        let dir = select(-t, t, below < above);
        small = min(small, (p.hi[k] - p.lo[k]) / g);
        if s < s1 {
            s2 = s1;
            t2 = t1;
            s1 = s;
            t1 = dir;
        } else if s < s2 {
            s2 = s;
            t2 = dir;
        }
    }
    var out = RegEdges(s1, t1, s2, t2, small);
    // The Regency's plates never meet square: their corners are cut.
    let corner = (s1 + s2 - chamfer) * 0.7071;
    let both = t1 + t2;
    if corner < s1 && dot(both, both) > 0.25 {
        out.e = corner;
        out.dir = normalize(both);
    }
    return out;
}

// What a plate panel is: 0 plain with a pin line, 1 a sunk field in a raised frame,
// 2 a raised boss, 3 a vent, 4 machine plate cut into small hatches and hairlines.
fn reg_style(id: f32) -> u32 {
    let r = hash11(id * 53.1 + 0.11);
    if r < 0.28 {
        return 0u;
    }
    if r < 0.48 {
        return 1u;
    }
    if r < 0.6 {
        return 2u;
    }
    if r < 0.74 {
        return 3u;
    }
    return 4u;
}

// Height of the plate, 0 down a seam to about 1 on the face: `e` and `s2` as
// `RegEdges`, `inset` where the pin line or the frame's inner edge runs.
fn reg_plate_h(e: f32, s2: f32, style: u32, inset: f32, scale: f32, bolted: bool, px: f32) -> f32 {
    let gap = scale * 0.02;
    let bevel = scale * 0.045;
    let pin = scale * 0.012;
    var h = smoothstep(gap, gap + bevel, e);
    switch style {
        case 0u: {
            h -= 0.5 * (1.0 - smoothstep(pin * 0.5, pin * 1.5, abs(e - inset))) * surf_resolved(pin * 1.5, px);
        }
        case 1u: {
            h -= 0.45 * smoothstep(inset, inset + bevel * 0.7, e);
        }
        case 2u: {
            h += 0.4 * smoothstep(inset, inset + bevel, e);
        }
        case 3u: {
            // Vent slats square to the nearest edge, turning at the diagonals into chevrons.
            let open = smoothstep(inset, inset + bevel * 0.5, e) * smoothstep(inset, inset + bevel * 0.5, s2);
            let pitch = scale * 0.09;
            let f = fract(s2 / pitch);
            h -= open * (0.3 + 0.55 * smoothstep(0.45, 0.6, f) * (1.0 - smoothstep(0.85, 1.0, f)) * surf_resolved(pitch * 0.5, px));
        }
        default: {
            // A frame round machine plate a little lower (its hatches are `reg_fine_h`).
            h -= 0.2 * smoothstep(inset * 0.6, inset * 0.6 + bevel * 0.5, e);
        }
    }
    if bolted {
        // Small bolt heads in a row round the frame.
        let pitch = scale * 0.24;
        let r = length(vec2<f32>(e - inset * 0.5, (fract(s2 / pitch) - 0.5) * pitch));
        let rb = scale * 0.022;
        h = max(h, 0.85 + 0.4 * sqrt(saturate(1.0 - (r * r) / (rb * rb))) * step(r, rb) * surf_resolved(rb, px));
    }
    return h;
}

// Machine plate's own relief: hairline seams between small plates, some of them hatches
// let in a step, some slotted.
fn reg_fine_h(e: f32, s2: f32, id: f32, scale: f32, px: f32) -> f32 {
    let w = scale * 0.008;
    var h = 1.0 - 0.7 * (1.0 - smoothstep(w * 0.5, w * 1.6, e));
    let kind = hash11(id * 41.3);
    let inset = scale * 0.07;
    if kind > 0.6 {
        h -= 0.35 * smoothstep(inset, inset + scale * 0.02, e);
    } else if kind > 0.35 {
        // Three short slots.
        let pitch = scale * 0.08;
        let f = abs(fract(s2 / pitch) - 0.5) * pitch;
        h -= 0.6 * (1.0 - smoothstep(w * 0.8, w * 2.0, f)) * smoothstep(inset, inset + w, e) * step(s2, inset + pitch * 3.0)
            * surf_resolved(pitch * 0.5, px);
    }
    return h;
}

fn regency_plate(i: RegencyIn) -> RegencyLook {
    var out = regency_none();
    let fw = max(i.px, 1e-4);
    // Folded onto one flank: the model is mirrored, and so is its plating.
    let side = select(-1.0, 1.0, i.local.y >= 0.0);
    let p = vec3<f32>(i.local.x, abs(i.local.y), i.local.z);
    let n = vec3<f32>(i.n.x, i.n.y * side, i.n.z);
    let q = vec3<f32>(dot(p, reg_axis(0u)), dot(p, reg_axis(1u)), dot(p, reg_axis(2u)));
    let size = i.scale * 1.9;
    // Nothing finer than the panels themselves to see: an even plate.
    if size < fw * 6.0 {
        return out;
    }
    let panel = reg_panel(q, size);
    let style = reg_style(panel.id);
    let span = min(min(panel.hi.x - panel.lo.x, panel.hi.y - panel.lo.y), panel.hi.z - panel.lo.z);
    let chamfer = select(0.0, span * mix(0.18, 0.4, hash11(panel.id * 7.9)), hash11(panel.id * 3.3) > 0.3);
    let at = reg_edges(q, panel, n, chamfer);
    let inset = clamp(at.small * 0.14, i.scale * 0.08, i.scale * 0.26);
    let bolted = (style == 1u || style == 2u || (style == 0u && hash11(panel.id * 5.7) > 0.5)) && at.small > i.scale * 0.6;

    // Relief, differenced over about a pixel along each way the panel is laid out.
    let bevel = i.scale * 0.045;
    let d = max(fw * 0.75, i.scale * 0.003);
    let h0 = reg_plate_h(at.e, at.s2, style, inset, i.scale, bolted, fw);
    let he = reg_plate_h(at.e + d, at.s2, style, inset, i.scale, bolted, fw);
    let hs = reg_plate_h(at.e, at.s2 + d, style, inset, i.scale, bolted, fw);
    var slope = ((he - h0) * at.dir + (hs - h0) * at.dir2) * (bevel * 0.85 / d);
    var fine_h = 1.0;
    if style == 4u {
        // Machine plate: a finer cut of space inside the frame.
        let fine = reg_panel(q + vec3<f32>(0.37, 0.61, 0.23) * size, i.scale * 0.62);
        let fat = reg_edges(q + vec3<f32>(0.37, 0.61, 0.23) * size, fine, n, i.scale * 0.05);
        let inside = smoothstep(inset * 0.6 + bevel * 0.5, inset * 0.6 + bevel, at.e);
        let seen = surf_resolved(i.scale * 0.02, fw);
        fine_h = mix(1.0, reg_fine_h(fat.e, fat.s2, fine.id, i.scale, fw), inside * seen);
        let fe = mix(1.0, reg_fine_h(fat.e + d, fat.s2, fine.id, i.scale, fw), inside * seen);
        let fs = mix(1.0, reg_fine_h(fat.e, fat.s2 + d, fine.id, i.scale, fw), inside * seen);
        slope += ((fe - fine_h) * fat.dir + (fs - fine_h) * fat.dir2) * (bevel * 0.5 / d);
    }
    slope *= 1.0 - smoothstep(bevel * 3.0, bevel * 9.0, fw);
    out.slope = vec3<f32>(slope.x, slope.y * side, slope.z);

    // Down a seam it is dark; a sunk field or vent sits in shade; each panel its own sheen.
    let gap = i.scale * 0.02;
    let seam = surf_band(at.e, gap, fw);
    out.tone = (0.65 + 0.7 * hash11(panel.id * 29.3)) * (1.0 - 0.85 * seam) * (0.5 + 0.5 * saturate(h0)) * (0.55 + 0.45 * fine_h);
    out.rough = (hash11(panel.id * 61.7) - 0.5) * 0.2;
    // The lip of every plate catches the light: on black, that is what draws a panel.
    let lip = surf_band(at.e - gap - bevel * 0.45, bevel * 0.45, fw) * surf_resolved(bevel, fw);
    out.lift = 0.5 * lip + 0.8 * saturate((h0 - 1.0) * 3.0) * surf_resolved(i.scale * 0.022, fw);
    // Fine brushing along the sweep of the plates, and edge wear that polishes the lips.
    let grain = surf_fbm3(vec3<f32>(q.x * 0.08, q.y, q.z) + vec3<f32>(panel.id * 40.0), i.scale * 0.04, fw);
    out.rough += 0.14 * grain;
    let wear = surf_fbm3(q + vec3<f32>(11.0, 3.0, 7.0), i.scale * 0.15, fw);
    out.lift += 0.3 * lip * smoothstep(0.05, 0.3, wear);

    // Red let into the pin line of a few panels, breathing slowly; it falters as the unit is hurt.
    let lit = style == 0u && hash11(panel.id * 7.7) < 0.35;
    if lit {
        let line = surf_band(at.e - inset, i.scale * 0.005, fw);
        let breath = 0.6 + 0.4 * sin(i.time * 1.2 + panel.id * 40.0 + at.s2 / i.scale * 0.8);
        let hurt = 1.0 - saturate(i.health);
        let alive = 1.0 - smoothstep(0.4, 0.9, hurt + hash11(panel.id * 9.1) * 0.3);
        out.emissive = SURF_EMBER * line * breath * alive * 1.5;
    }
    return out;
}

// Height across bronze, 0 down a groove to 1 on the metal, and more on a collar.
fn reg_bronze_rings(u: f32, scale: f32) -> f32 {
    // A turned collar every so often, fine lathe grooves between.
    let pitch = scale * 0.55;
    let f = abs(fract(u / pitch + 0.5) - 0.5) * pitch;
    let collar = 1.0 - smoothstep(scale * 0.05, scale * 0.07, f);
    let groove = 1.0 - smoothstep(scale * 0.075, scale * 0.09, f);
    let fine = abs(fract(u / (scale * 0.05) + 0.5) - 0.5);
    return 1.0 + 0.45 * collar - 0.5 * (groove - collar) - 0.15 * smoothstep(0.35, 0.5, fine) * (1.0 - groove);
}

fn reg_bronze_h(e: f32, s2: f32, engraved: bool, scale: f32) -> f32 {
    let w = scale * 0.016;
    var h = smoothstep(w * 0.5, w * 1.6, e);
    if engraved {
        // A second line inside the first, and a stepped key off it: machine script.
        let inset = scale * 0.12;
        h -= 0.7 * (1.0 - smoothstep(w * 0.4, w * 1.2, abs(e - inset)));
        let key = abs(fract(s2 / (scale * 0.5)) - 0.5) * scale * 0.5;
        h -= 0.7 * (1.0 - smoothstep(w * 0.4, w * 1.2, key)) * step(inset, e) * (1.0 - step(inset * 2.0, e));
    }
    return h;
}

fn regency_bronze(i: RegencyIn) -> RegencyLook {
    var out = regency_none();
    let fw = max(i.px, 1e-4);
    let d = max(fw * 0.75, i.scale * 0.004);
    let depth = i.scale * 0.03;
    var h0 = 1.0;
    if dot(i.along_dir, i.along_dir) > 0.25 {
        // A ram, a cable or an axle: turned, ringed round.
        h0 = reg_bronze_rings(i.along, i.scale);
        let h1 = reg_bronze_rings(i.along + d, i.scale);
        out.slope = i.along_dir * ((h1 - h0) * depth / d);
    } else {
        // Blocks and ribs: cut into cells and engraved.
        let side = select(-1.0, 1.0, i.local.y >= 0.0);
        let p = vec3<f32>(i.local.x, abs(i.local.y), i.local.z);
        let n = vec3<f32>(i.n.x, i.n.y * side, i.n.z);
        let q = vec3<f32>(dot(p, reg_axis(0u)), dot(p, reg_axis(1u)), dot(p, reg_axis(2u)));
        let panel = reg_panel(q, i.scale * 1.2);
        let at = reg_edges(q, panel, n, i.scale * 0.12);
        let engraved = hash11(panel.id * 3.9) > 0.35 && at.small > i.scale * 0.35;
        h0 = reg_bronze_h(at.e, at.s2, engraved, i.scale);
        let he = reg_bronze_h(at.e + d, at.s2, engraved, i.scale);
        let hs = reg_bronze_h(at.e, at.s2 + d, engraved, i.scale);
        let slope = ((he - h0) * at.dir + (hs - h0) * at.dir2) * (depth / d);
        out.slope = vec3<f32>(slope.x, slope.y * side, slope.z);
    }
    out.slope *= surf_resolved(i.scale * 0.05, fw);
    let seen = surf_resolved(i.scale * 0.03, fw);
    // Grooves dark with old oil; high metal worn bright.
    out.tone = mix(1.0, saturate(h0), seen);
    out.lift = saturate(h0 - 1.0) * 2.0 * seen;
    // A patina in broad patches, and the grain of the turning or the file.
    let patina = surf_fbm3(i.local + vec3<f32>(5.0, 9.0, 2.0), i.scale * 0.6, fw);
    out.tone *= 0.8 + 0.5 * patina;
    out.lift += 0.35 * smoothstep(0.1, 0.35, surf_fbm3(i.local * 1.7, i.scale * 0.2, fw));
    out.rough = 0.2 * patina + 0.1 * surf_fbm3(i.local, i.scale * 0.05, fw);
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
// shade): plate, the darker seams (`ACCENT`) and the bronze machinery (`METAL`).
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
