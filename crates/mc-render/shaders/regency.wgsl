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

// A model's plate and machinery take their colours from its skin's `film`
// (data/skins/regency.ron, `SKIN_FILM_*`): charcoal steel under a film whose sheen
// cycles crimson, deep red, copper as a face turns away, a paler second plate,
// graphite machinery darker in its grooves and polished where worked.
//
// The foundation strips (foundations.wgsl) read no skin yet; until they do, these are
// theirs, the same as the Regency skin's: their dark warm graphite, their lit lips'
// bright steel, and their graphite inlay.
const REG_GUNMETAL: vec3<f32> = vec3<f32>(0.1, 0.092, 0.082);
const REG_STEEL_LIT: vec3<f32> = vec3<f32>(0.36, 0.34, 0.315);
const REG_WORKS: vec3<f32> = vec3<f32>(0.12, 0.112, 0.1);
const REG_WORKS_DEEP: vec3<f32> = vec3<f32>(0.02, 0.019, 0.017);

// A film colour of a skin (`SKIN_FILM_*`).
fn reg_film_color(skin: u32, which: u32) -> vec3<f32> {
    return skins[skin].film[which].rgb;
}

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
    optic: u32,
    // Its skin (`ModelInfo::skin`): the film's colours and the lamps' red.
    skin: u32,
    // Seconds, and the unit's own random, zero to one.
    time: f32,
    unit: f32,
    // One while the unit builds (`FLAG_BUILDING`): its nanite slots run fast and burn bright.
    working: f32,
    // Its Edict (`reg_edict`): x its state (entity.wgsl `edict_state`), y the firing point
    // along model x, z the model's reach.
    edict: vec3<f32>,
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

// The lamps let into the plate burn as the skin's `laser` slot does (the
// GLOW_LASER material), so their hue and brightness cannot drift apart.
fn regency_lamp_light(skin: u32) -> vec3<f32> {
    return skins[skin].slots[2u * SKIN_LASER + 1u].rgb;
}

fn regency_none() -> RegencyLook {
    return RegencyLook(vec3<f32>(0.0), 1.0, 0.0, 0.0, vec3<f32>(0.0));
}

// Regency plate, or Regency plate with nanite slots let in (both are coloured here).
fn regency_pattern(p: u32) -> bool {
    return p == PAT_EMBER || p == NANITE_PATTERN_SLOT
        || p == EMBER_PLATE_SLOT || p == EMBER_PLATE_RIM || p == EMBER_PLATE_MACHINED
        || p == EMBER_PLATE_DEEP
        || p == NANITE_PATTERN_RIM;
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

// How many times brighter nanite light burns: one at rest, while the unit builds
// `NANITE_WORK_GLOW` on a quick beat.
fn reg_nanite_work(i: RegencyIn) -> f32 {
    let work = NANITE_WORK_GLOW * (0.75 + 0.25 * sin(i.time * NANITE_WORK_BEAT + i.unit * 31.0));
    return mix(1.0, work, i.working);
}

// The slot cut in a nanite face: dark glazing, framed by a groove and a lit lip, the
// nanites streaming along it in violet, thickening and thinning, a slow beat over them;
// while the unit builds they race along it, burning several times brighter on a quick beat.
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
    let speed = mix(NANITE_FLOW_SPEED, NANITE_WORK_FLOW_SPEED, i.working);
    let flow = i.local + vec3<f32>(i.unit * 41.0, i.unit * 13.0, -i.time * speed);
    let knot = smoothstep(0.35, 0.8, surf_noise3(flow * vec3<f32>(1.3, 1.3, 0.7)));
    let churn = 0.5 + surf_fbm3(flow * 2.0, i.scale * 0.08, fw);
    let seen = surf_resolved(half, fw);
    let stream = mix(0.55, 0.25 + 1.3 * knot * churn, seen);
    let beat = mix(0.8 + 0.2 * sin(i.time * 1.7 + i.unit * 31.0), reg_nanite_work(i), i.working);
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
    // The long seams alone (the courses' joins, not the cuts across them): the distance to
    // the nearest and its own random, zero to one (`reg_edict` lights a few of them).
    course: vec2<f32>,
}

fn reg_seams(i: RegencyIn, o: RegOutline) -> RegSeams {
    var out = RegSeams(vec4<f32>(1e4, 0.0, 0.0, 0.0), i.seed, vec2<f32>(1e4, 0.0));
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
        out.course = vec2<f32>(abs(off), hash11(n * 5.17 + i.seed * 37.0));
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
    // Lamps alter only their inset and emission. All the surrounding plate uses
    // the same scanned steel, relief and iridescent film as every Regency unit.
    if i.optic == EMBER_PLATE_SLOT {
        var optic = i;
        optic.scale = min(i.scale, 0.8);
        let slot = reg_nanite_slot(optic);
        if slot.z > 0.0 {
            let half = max(slot.z * 0.32, 0.035);
            let d = max(slot.x - half, slot.z * 2.1 - slot.y);
            let lit = 1.0 - smoothstep(-fw, fw, d);
            out.tone *= 1.0 - 0.9 * (1.0-smoothstep(-fw, fw, d-half));
            out.glow = regency_lamp_light(i.skin) * lit;
        }
    } else if i.optic == EMBER_PLATE_RIM || i.optic == NANITE_PATTERN_RIM {
        let inset = max(i.scale * 0.028, o.small * 0.18);
        let lit = surf_band(o.e - inset, i.scale * 0.013, fw);
        if i.optic == EMBER_PLATE_RIM {
            out.glow = regency_lamp_light(i.skin) * lit;
        } else {
            out.glow = vec3<f32>(NANITE_R, NANITE_G, NANITE_B) * lit * NANITE_RIM_GLOW
                * reg_nanite_work(i);
        }
    }
    if i.nanite {
        out = reg_nanite(i, out, fw);
    }
    if i.edict.x != 0.0 {
        // The face's own edge counts as a long seam only on a broad face (a chine, a deck's
        // edge), never round every small facet.
        let edge = select(1e4, o.e, o.small > i.scale * 3.0);
        let charge = reg_edict(i, seams.course, edge, seams.id, fw);
        out.glow = out.glow * (1.0 + charge.w) + charge.rgb;
    }
    return out;
}

// The Edict gathering in the hull (`EDICT_*`, renderer/edict_fx): two lines of light
// sweeping in along the hull from the bow and the stern, closing on the firing point:
// white-hot at each front with the prism's pink at its rim, in the long seams only (the
// courses' joins and a broad face's edge, never every plate's outline), and a pool of it
// growing round the point. Behind the fronts the hull stays charcoal: a few long seams keep
// a dim red, pulses running in toward the point. Stalled it gutters; after the shot the
// pool cools from white to red and the last red dies out of those few seams. `course` is
// the nearest long seam (distance, its random), `edge` a broad face's edge, `plate` the
// plate's random. Returns the light, and in w how much brighter the plate's lamps burn.
fn reg_edict(i: RegencyIn, course: vec2<f32>, edge: f32, plate: f32, fw: f32) -> vec4<f32> {
    let s = i.edict.x;
    let x = i.local.x;
    let at = i.edict.y;
    let reach = max(i.edict.z, 1.0);
    let t = i.time;
    let red = vec3<f32>(1.0, 0.05, 0.035);
    let white = vec3<f32>(1.0, 0.95, 0.97);
    // A lit seam is drawn at least most of a pixel wide so it reads from afar, its light
    // spread thinner as it is widened.
    let own = i.scale * 0.025;
    let w = max(own, fw * 0.7);
    let thin = pow(own / w, 0.6);
    let on_course = (1.0 - smoothstep(w * 0.4, w, course.x)) * thin;
    let on_edge = (1.0 - smoothstep(w * 0.4, w, edge)) * thin;
    // The few long seams that carry light behind the fronts and after the shot.
    let few = step(0.8, course.y);
    let long_lit = max(on_course * few, on_edge);
    let every = max(on_course, on_edge);
    let from_at = abs(x - at);
    if s >= 2.0 {
        // Fired: the pool flares and cools from white through red, the few long seams
        // dying out last.
        let age = s - 2.0;
        let near = exp(-from_at / 260.0);
        let flash = exp(-age / 0.15) * near;
        let hot = exp(-age / 0.7) * near;
        let ember = (1.0 - smoothstep(0.0, EDICT_AFTERGLOW * mix(0.4, 1.0, plate), age))
            * exp(-age / 2.0);
        let rgb = (white * (flash * 18.0 + hot * 8.0) + red * hot * 6.0) * every
            + red * ember * (0.8 + 4.0 * near) * long_lit;
        return vec4<f32>(rgb, 2.0 * flash + 1.5 * ember * near);
    }
    let p = abs(s);
    let run = clamp(p / EDICT_MEET, 0.0, 1.0);
    let e = pow(run, 1.15);
    // The two fronts: from the bow (reach) and the stern, closing on the firing point.
    let bow = mix(reach, at, e);
    let stern = mix(-reach * 0.8, at, e);
    // How far the front on this side has swept past here: positive behind it, toward the
    // end it came from. Each seam leads or lags a little: a front, not a ruled line.
    let rag = (course.y - 0.5) * 30.0 + 12.0 * sin(x * 0.011 + plate * 5.0);
    let past = select(stern - x, x - bow, x > at) + rag * (1.0 - run);
    let width = 18.0 + 22.0 * run;
    // The crest: sharp on its leading edge, trailing off behind.
    let lead = exp(-pow(max(-past, 0.0) / (width * 0.3), 2.0));
    let trail = exp(-pow(max(past, 0.0) / width, 2.0));
    let crest = select(lead, trail, past >= 0.0) * (1.0 - smoothstep(0.9, 1.0, run));
    // Behind the front: a dim red in a few long seams, pulses running on toward the point.
    let burnt = smoothstep(-10.0, 80.0, past);
    let pulse = pow(0.5 + 0.5 * sin(from_at * 0.02 - t * (3.0 + 7.0 * p) + course.y * 6.0), 10.0);
    let ember = burnt * (0.1 + 0.9 * pulse) * (0.4 + 0.8 * p)
        * mix(1.0, 0.25, smoothstep(0.0, 1000.0, past)) * (1.0 - 0.8 * smoothstep(0.88, 1.0, p));
    // Round the firing point the charge pools, white-hot by the end.
    let near = exp(-from_at / mix(160.0, 90.0, p)) * smoothstep(0.3, 1.0, p);
    let rim = prism(course.y + x * 0.002 - t * PRISM_RATE * 3.0);
    let pink = mix(vec3<f32>(1.0, 0.45, 0.62), rim, 0.5);
    // The front's light is in every long seam it crosses, and a breath of it on the plate
    // round them, so the sweep reads as a line of light from afar.
    let front = mix(pink, white, smoothstep(0.35, 1.0, crest)) * crest * (10.0 + 14.0 * p);
    let pool = mix(red, white, smoothstep(0.4, 1.0, near * p)) * near * (3.0 + 18.0 * p * p);
    var rgb = (front + pool) * (every + 0.03) + red * ember * 2.0 * long_lit;
    var lamps = 0.6 * ember + 3.0 * near + 2.0 * crest;
    if s < 0.0 {
        // Stalled: the light gutters, catching and failing in its own places.
        let k = floor(t * 13.0 + plate * 5.0);
        let catches = step(0.55, hash11(k + plate * 17.0));
        let gutter = 0.2 + 0.5 * catches;
        rgb *= gutter;
        lamps *= gutter;
    }
    return vec4<f32>(rgb, lamps);
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
    if i.optic == EMBER_PLATE_MACHINED {
        var machined = i;
        machined.scale = i.scale * 0.48;
        var out = regency_plate(machined);
        let o = reg_outline(machined);
        let fw = max(i.px, 1e-4);
        // Recessed circular fasteners follow a rectangular panel's corners. They
        // are surface relief, filtered out when their diameter is subpixel.
        if i.face.w > 0.0 && i.face.z > machined.scale && i.face.w > machined.scale {
            let inset = machined.scale * 0.24;
            let q = i.face.zw - abs(i.face.xy) - vec2<f32>(inset);
            let radius = machined.scale * 0.065;
            let d = length(q);
            let seen = surf_resolved(radius * 2.0, fw);
            let socket = (1.0 - smoothstep(radius - fw, radius + fw, d)) * seen;
            let rim = surf_band(d - radius * 1.5, radius * 0.22, fw) * seen;
            out.tone *= 1.0 - socket * 0.7;
            out.lift += rim * 0.45;
            out.rough += socket * 0.14;
        }
        // Fine directional machining in the coating, strongest on broad faces.
        let pitch = machined.scale * 0.055;
        let axis = normalize(cross(i.n, o.dir) + vec3<f32>(1e-6));
        let grain = sin(dot(i.local, axis) * 6.283185 / pitch);
        let resolved = surf_resolved(pitch, fw);
        out.rough += grain * 0.035 * resolved;
        out.tone *= 1.0 + grain * 0.025 * resolved;
        return out;
    }
    var out = regency_plate(i);
    if i.optic == EMBER_PLATE_DEEP {
        // The deeper shade: the plate darker, its worn edges dulled with it.
        out.tone *= EMBER_PLATE_DEEP_TONE;
        out.lift *= 0.5;
    }
    return out;
}

// The film's colour at `cos_v` (the face's normal against the view).
fn reg_film(skin: u32, cos_v: f32, seed: f32, pale: bool) -> vec3<f32> {
    let off = 1.0 - saturate(cos_v);
    let rise = smoothstep(0.3, 0.9, off) * select(1.0, 0.7, pale);
    // Ping-pong crimson -> deep red -> copper -> deep red -> crimson.
    let h = abs(fract(off * 1.6 + seed * 0.8) * 2.0 - 1.0) * 2.0;
    let red = reg_film_color(skin, SKIN_FILM_RED);
    let deep = reg_film_color(skin, SKIN_FILM_DEEP);
    let copper = reg_film_color(skin, SKIN_FILM_COPPER);
    let hue = mix(mix(red, deep, smoothstep(0.0, 1.0, h)), copper, smoothstep(1.0, 2.0, h));
    let plate = select(reg_film_color(skin, SKIN_FILM_BLACK), reg_film_color(skin, SKIN_FILM_PALE), pale);
    return mix(plate, hue, rise);
}

// Regency colours over the shared palette's (`lum` keeps the model's own light and
// shade): plate (`PLATING_DARK`, and the paler `PLATING`), the darker trim (`ACCENT`) and
// the dark graphite machinery (`METAL`).
fn regency_paint(m_in: Pbr, skin: u32, works: bool, pale: bool, look: RegencyLook, cos_v: f32, seed: f32) -> Pbr {
    var m = m_in;
    let lum = dot(m.albedo, vec3<f32>(0.3, 0.59, 0.11));
    if works {
        let base = mix(reg_film_color(skin, SKIN_FILM_WORKS_DEEP), reg_film_color(skin, SKIN_FILM_WORKS), saturate(look.tone))
            * clamp(0.75 + lum, 0.75, 1.3);
        m.albedo = mix(base, reg_film_color(skin, SKIN_FILM_WORKS_POLISH), saturate(look.lift));
        m.metallic = 0.6;
        m.roughness = clamp(0.4 + look.rough - 0.18 * look.lift, 0.22, 0.6);
    } else {
        // Steel under an iridescent film, not paint: a metal, so its colour is what it
        // reflects, charcoal seen square on and a crimson to copper sheen as it turns away; what makes it read as real is its
        // sheen varying, smudged here, worn through to bright steel at its edges, dull
        // with grime in its seams.
        let tone = look.tone * clamp(0.8 + lum, 0.8, 1.2);
        m.albedo = mix(reg_film(skin, cos_v, seed, pale) * tone, reg_film_color(skin, SKIN_FILM_STEEL_LIT), saturate(look.lift) * 0.5);
        m.metallic = 0.55;
        m.roughness = clamp(0.42 + look.rough - 0.1 * saturate(look.lift), 0.24, 0.8);
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
