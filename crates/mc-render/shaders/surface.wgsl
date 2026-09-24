// Fitted surfaces: what is drawn on a unit's faces. Prepended to shaders that
// contain the line `//!use surface`.
//
// Nothing here is a tiled picture. Every face of a model carries its own frame
// (`MeshVertex::face`: where the fragment sits on the face, and how big the face
// is), so detail is laid out to fit: a plate's outline follows the face's real
// edge, courses of plates divide it without a sliver left over, rivets are
// spaced to land on the corners, the lights in the black run level. A pattern
// (`models::pattern`) swaps the generic treatment for a specific one. Relief is
// a height function, differenced over about a pixel for the normal, so it is
// filtered at any zoom. Emission is in HDR units and may move with time.

const PAT_GENERIC: u32 = 0u;
const PAT_PLAIN: u32 = 1u;
const PAT_SHUTTER: u32 = 2u;
const PAT_DECK: u32 = 3u;
const PAT_ROADWAY: u32 = 4u;
const PAT_FURNACE: u32 = 5u;
const PAT_CONDUIT: u32 = 6u;
const PAT_TEAM_BAND: u32 = 7u;
const PAT_NONE: u32 = 8u;
const PAT_AIRFRAME: u32 = 9u;
const PAT_PILE: u32 = 10u;
const PAT_HAZARD: u32 = 11u;
const PAT_HULL: u32 = 12u;
const PAT_TILES: u32 = 13u;
const PAT_WALKWAY: u32 = 14u;
const PAT_PLASMA: u32 = 15u;
const PAT_FLUX: u32 = 16u;
const PAT_VEINED: u32 = 17u;

// The lights let into dark plating, and the hot end of a furnace.
// Redder than it should look: a bright emitter's green climbs first through the tonemap.
const SURF_ORANGE: vec3<f32> = vec3<f32>(1.0, 0.27, 0.03);
const SURF_AMBER: vec3<f32> = vec3<f32>(1.0, 0.6, 0.1);
const SURF_SAFETY: vec3<f32> = vec3<f32>(0.78, 0.30, 0.03);
// The replicators' light (`PAT_VEINED`), in the black where Aster has its orange.
const SURF_VIOLET: vec3<f32> = vec3<f32>(0.5, 0.18, 1.0);
// A reactor's burning core: deep blue where it is thin, near white where it is hot.
const SURF_PLASMA_DEEP: vec3<f32> = vec3<f32>(0.10, 0.34, 1.0);
const SURF_PLASMA_HOT: vec3<f32> = vec3<f32>(0.72, 0.90, 1.0);

struct SurfaceIn {
    // Metres from the middle of the face, and the face's half size.
    st: vec2<f32>,
    half: vec2<f32>,
    // s goes right round a tube: no edge that way.
    wraps: bool,
    // Dark plating: it carries lights, not plate courses.
    dark: bool,
    pattern: u32,
    // Per face and per unit, zero to one.
    seed: f32,
    unit: f32,
    // The unit's id: burn marks are placed from it in whole numbers, so the
    // renderer can stand smoke and flame on the same spots (`models::burns`).
    unit_id: u32,
    // Length of a typical plate on this model, metres.
    scale: f32,
    // Metres of surface under one pixel.
    px: f32,
    time: f32,
    // The unit is at work (a factory building).
    working: f32,
    tech: f32,
    // Zero on a tech 1 line unit, where nothing is lit: its lines are paint.
    lit: f32,
    // One on a mobile unit: its work lights sit lower while it is idle than a building's.
    mobile: f32,
    health: f32,
    // Model space, with the model's reach and height, for damage that spans faces.
    local: vec3<f32>,
    reach: f32,
    height: f32,
}

struct Surface {
    // Slope of the relief along s and t.
    slope: vec2<f32>,
    // Multiplies albedo: seams, recesses.
    cavity: f32,
    // Paint laid over the material.
    paint: vec4<f32>,
    // How much of the owner's colour.
    team: f32,
    // Bare, scuffed or burnt steel showing through.
    bare: f32,
    rough: f32,
    // Soot over everything, emitters included.
    soot: f32,
    emissive: vec3<f32>,
}

struct SurfaceCell {
    // Metres from the middle of the cell, its half size, and its own random.
    p: vec2<f32>,
    half: vec2<f32>,
    id: f32,
}

// How much of a pixel `fw` wide the band |d| < w covers: a line keeps its
// energy when it gets thinner than a pixel instead of breaking into dots.
fn surf_band(d: f32, w: f32, fw: f32) -> f32 {
    let f = max(fw, 1e-5);
    return saturate((d + w) / f + 0.5) - saturate((d - w) / f + 0.5);
}

// One for d past `edge`, antialiased.
fn surf_step(d: f32, edge: f32, fw: f32) -> f32 {
    return saturate((d - edge) / max(fw, 1e-5) + 0.5);
}

fn surf_rise(d: f32, gap: f32, bevel: f32) -> f32 {
    return smoothstep(gap, gap + bevel, d);
}

// How many whole pieces of about `want` fit in `len`.
fn surf_fit(len: f32, want: f32) -> f32 {
    return max(1.0, round(len / max(want, 1e-3)));
}

fn surf_edge(p: vec2<f32>, half: vec2<f32>) -> f32 {
    let d = half - abs(p);
    return min(d.x, d.y);
}

fn surf_face_edge(i: SurfaceIn, st: vec2<f32>) -> f32 {
    let d = i.half - abs(st);
    return select(min(d.x, d.y), d.y, i.wraps);
}

// Courses of plates: rows across the face, each cut into whole plates, and
// neighbouring rows cut differently so the joints never line up into a grid.
fn surf_courses(i: SurfaceIn, st_in: vec2<f32>, want: vec2<f32>) -> SurfaceCell {
    var st = st_in;
    var half = i.half;
    if !i.wraps && half.y > half.x * 1.5 {
        st = st.yx;
        half = half.yx;
    }
    let rows = surf_fit(2.0 * half.y, want.y);
    let rh = 2.0 * half.y / rows;
    let r = clamp(floor((st.y + half.y) / rh), 0.0, rows - 1.0);
    var n = surf_fit(2.0 * half.x, want.x);
    let odd = (r + floor(i.seed * 2.0)) % 2.0;
    if odd > 0.5 && 2.0 * half.x / (n + 1.0) > 0.55 * want.x {
        n += 1.0;
    }
    let cw = 2.0 * half.x / n;
    var x = st.x + half.x;
    if i.wraps {
        x = x - 2.0 * half.x * floor(x / (2.0 * half.x));
    }
    let c = clamp(floor(x / cw), 0.0, n - 1.0);
    var cell: SurfaceCell;
    cell.p = vec2<f32>(x - (c + 0.5) * cw, st.y + half.y - (r + 0.5) * rh);
    cell.half = vec2<f32>(cw, rh) * 0.5;
    cell.id = hash21(vec2<f32>(r * 7.31 + i.seed * 91.7, c * 3.17 + i.seed * 17.3));
    return cell;
}

// A ring of rivets just inside a rectangle, spaced to land on its corners.
// Returns the dome's height, zero to one.
fn surf_rivets(p: vec2<f32>, half: vec2<f32>, inset: f32, pitch: f32, radius: f32) -> f32 {
    let inner = half - vec2<f32>(inset);
    if inner.x < radius * 2.0 || inner.y < radius * 2.0 {
        return 0.0;
    }
    let px = 2.0 * inner.x / surf_fit(2.0 * inner.x, pitch);
    let py = 2.0 * inner.y / surf_fit(2.0 * inner.y, pitch);
    let kx = round((p.x + inner.x) / px) * px - inner.x;
    let ky = round((p.y + inner.y) / py) * py - inner.y;
    let a = vec2<f32>(clamp(kx, -inner.x, inner.x), select(-inner.y, inner.y, p.y > 0.0));
    let b = vec2<f32>(select(-inner.x, inner.x, p.x > 0.0), clamp(ky, -inner.y, inner.y));
    let d = min(distance(p, a), distance(p, b)) / radius;
    return saturate(1.0 - d * d);
}

// The lights in dark plating: level lines, broken into dashes, fitted to the face.
struct SurfaceDash {
    // Distance from the line's axis, and how far inside the dash's ends.
    d: f32,
    along: f32,
    id: f32,
    on: f32,
}

// Dark plating is cut into long plates like the rest; a plate may carry one
// light, level, centred in it, so the lights follow the plates and the plates
// follow the face.
fn surf_dark_cell(i: SurfaceIn, st: vec2<f32>) -> SurfaceCell {
    return surf_courses(i, st, vec2<f32>(i.scale * 1.7, i.scale * 0.66));
}

fn surf_dashes(i: SurfaceIn, st: vec2<f32>, cell: SurfaceCell) -> SurfaceDash {
    var dash: SurfaceDash;
    dash.id = cell.id;
    // Courses may have been laid along t on a tall narrow face: the light stays level.
    let turned = !i.wraps && i.half.y > i.half.x * 1.5;
    let p = select(cell.p, cell.p.yx, turned);
    let half = select(cell.half, cell.half.yx, turned);
    let room = half.x > i.scale * 0.2 && half.y > i.scale * 0.1;
    dash.on = select(0.0, step(0.42, hash11(cell.id * 17.0 + 0.11)), room);
    dash.d = p.y;
    dash.along = half.x * mix(0.3, 0.62, hash11(cell.id * 37.0)) - abs(p.x);
    return dash;
}

// ---- ships ------------------------------------------------------------------

// Anechoic tiles on a submarine's casing: small squares fitted to the face, each
// row set half a tile over from the last.
fn surf_tile_cell(i: SurfaceIn, st: vec2<f32>) -> SurfaceCell {
    let want = i.scale * 0.26;
    let rows = surf_fit(2.0 * i.half.y, want);
    let rh = 2.0 * i.half.y / rows;
    let r = clamp(floor((st.y + i.half.y) / rh), 0.0, rows - 1.0);
    let n = surf_fit(2.0 * i.half.x, want);
    let cw = 2.0 * i.half.x / n;
    var x = st.x + i.half.x + (r % 2.0) * cw * 0.5;
    if i.wraps {
        x = x - 2.0 * i.half.x * floor(x / (2.0 * i.half.x));
    }
    let c = floor(x / cw);
    var cell: SurfaceCell;
    cell.p = vec2<f32>(x - (c + 0.5) * cw, st.y + i.half.y - (r + 0.5) * rh);
    cell.half = vec2<f32>(cw, rh) * 0.5;
    cell.id = hash21(vec2<f32>(r * 5.11 + i.seed * 71.3, c * 2.93 + i.seed * 13.7));
    return cell;
}

// One digit of a stencilled hull number, seven segments with gaps between them.
// `p` is in the digit's own box: x -0.5..0.5, y -1..1; `fw` a pixel in those units.
fn surf_digit(p: vec2<f32>, digit: u32, fw: f32) -> f32 {
    var masks = array<u32, 10>(0x3Fu, 0x06u, 0x5Bu, 0x4Fu, 0x66u, 0x6Du, 0x7Du, 0x07u, 0x7Fu, 0x6Fu);
    let mask = masks[min(digit, 9u)];
    // a, b, c, d, e, f, g: top, top right, bottom right, bottom, bottom left, top left, middle.
    var centres = array<vec2<f32>, 7>(
        vec2<f32>(0.0, 0.88), vec2<f32>(0.4, 0.45), vec2<f32>(0.4, -0.45), vec2<f32>(0.0, -0.88),
        vec2<f32>(-0.4, -0.45), vec2<f32>(-0.4, 0.45), vec2<f32>(0.0, 0.0),
    );
    var on = 0.0;
    for (var k = 0u; k < 7u; k++) {
        if (mask & (1u << k)) == 0u {
            continue;
        }
        let across = k == 0u || k == 3u || k == 6u;
        let half = select(vec2<f32>(0.1, 0.36), vec2<f32>(0.3, 0.1), across);
        on = max(on, surf_step(surf_edge(p - centres[k], half), 0.0, fw));
    }
    return on;
}

// Draught marks at bow and stern: level ticks up the stem and the stern, every
// other one long. White on the dark below the boot-top, black on the white above.
fn surf_draught(i: SurfaceIn, fw: f32) -> vec4<f32> {
    let z = i.local.z;
    let at = i.reach * 0.8;
    let w = i.scale * 0.14;
    let pitch = i.scale * 0.2;
    let k = floor(z / pitch);
    let tick = surf_band((fract(z / pitch) - 0.25) * pitch, pitch * 0.12, fw);
    let major = (i32(k) & 1) == 0;
    let len = select(w * 0.55, w, major);
    let across = surf_step(len - abs(abs(i.local.x) - at), 0.0, fw);
    let span = surf_step(z, -i.scale * 0.5, fw) * (1.0 - surf_step(z, i.scale * 0.9, fw));
    let white = z < i.scale * 0.12;
    return vec4<f32>(select(vec3<f32>(0.02, 0.02, 0.025), vec3<f32>(0.78, 0.78, 0.74), white), tick * across * span);
}

// ---- relief -----------------------------------------------------------------

fn surf_relief_plates(i: SurfaceIn, st: vec2<f32>, bevel: f32, gap: f32) -> f32 {
    let cell = surf_courses(i, st, vec2<f32>(i.scale * 1.5, i.scale));
    let d = surf_edge(cell.p, cell.half);
    var h = surf_rise(d, gap, bevel) * (0.78 + 0.22 * cell.id);
    let roomy = min(cell.half.x, cell.half.y) > i.scale * 0.3;
    let style = hash11(cell.id * 91.0 + 0.37);
    if roomy && style > 0.45 && style <= 0.75 {
        h += surf_rivets(cell.p, cell.half, gap + bevel * 2.4, i.scale * 0.17, i.scale * 0.026) * 0.55;
    } else if roomy && style > 0.75 && style <= 0.9 {
        // An access hatch let into the plate, bolted at its corners.
        let hatch = cell.half * vec2<f32>(0.5, 0.46);
        let e = surf_edge(cell.p, hatch);
        h -= 0.5 * surf_band(e, gap * 1.2, 0.0) + 0.18 * step(0.0, e);
        h += surf_rivets(cell.p, hatch, bevel * 1.6, 1e3, i.scale * 0.024) * 0.5;
    } else if roomy && style > 0.9 {
        // Louvres: a bank of slats pressed into the plate.
        let bank = cell.half * vec2<f32>(0.56, 0.42);
        let e = surf_edge(cell.p, bank);
        if e > 0.0 {
            let pitch = 2.0 * bank.y / surf_fit(2.0 * bank.y, i.scale * 0.09);
            let f = fract((cell.p.y + bank.y) / pitch);
            h -= 0.55 * (1.0 - f) * smoothstep(0.0, bevel, e);
        }
    }
    return h;
}

fn surf_relief_shutter(i: SurfaceIn, st: vec2<f32>) -> f32 {
    let pitch = 2.0 * i.half.y / surf_fit(2.0 * i.half.y, i.scale * 0.1);
    let f = fract((st.y + i.half.y) / pitch);
    // Each slat laps the one below: a ramp, then a drop.
    let slat = 0.35 + 0.65 * f;
    let guide = surf_rise(i.half.x - abs(st.x), 0.0, i.scale * 0.05);
    return mix(1.1, slat, guide);
}

fn surf_relief_deck(i: SurfaceIn, st: vec2<f32>) -> f32 {
    // Tread plate: raised diamonds, alternating.
    let pitch = i.scale * 0.075;
    let q = vec2<f32>(st.x + st.y, st.x - st.y) / pitch;
    let cell = floor(q);
    let f = fract(q) - 0.5;
    let turned = (cell.x + cell.y) % 2.0 != 0.0;
    let g = select(f, f.yx, turned);
    let bar = saturate(1.0 - max(abs(g.x) / 0.42, abs(g.y) / 0.14));
    return 0.8 + 0.2 * smoothstep(0.0, 0.5, bar);
}

fn surf_relief_furnace(i: SurfaceIn, st: vec2<f32>, bevel: f32) -> f32 {
    let rim = min(i.half.x, i.half.y) * 0.16;
    let e = surf_edge(st, i.half - vec2<f32>(rim));
    if e <= 0.0 {
        return 1.0;
    }
    var along = st.y;
    var span = i.half.y - rim;
    if i.half.x > i.half.y {
        along = st.x;
        span = i.half.x - rim;
    }
    let pitch = 2.0 * span / surf_fit(2.0 * span, i.scale * 0.12);
    let f = fract((along + span) / pitch);
    // A tilted slat, then the gap down into the fire.
    let slat = select(0.75 - 0.5 * f / 0.62, -0.4, f > 0.62);
    return mix(1.0, slat, smoothstep(0.0, bevel, e));
}

// ---- airframe skin ----------------------------------------------------------

// Aircraft skin is cut into larger, flush panels than armour plate.
fn surf_skin_cell(i: SurfaceIn, st: vec2<f32>) -> SurfaceCell {
    return surf_courses(i, st, vec2<f32>(i.scale * 2.1, i.scale * 1.05));
}

// Countersunk fasteners in a row just inside every panel's edge. They fade out as
// they go under a pixel, so a row never breaks into stipple.
fn surf_skin_fasteners(i: SurfaceIn, cell: SurfaceCell, px: f32) -> f32 {
    let r = i.scale * 0.014;
    let seen = 1.0 - smoothstep(r * 0.6, r * 1.6, px);
    if seen <= 0.0 {
        return 0.0;
    }
    return surf_rivets(cell.p, cell.half, i.scale * 0.045, i.scale * 0.075, r) * seen;
}

// What a panel carries: 0 nothing, 1 an access panel, 2 a stencil, 3 a formation light.
fn surf_skin_style(i: SurfaceIn, cell: SurfaceCell) -> u32 {
    if min(cell.half.x, cell.half.y) <= i.scale * 0.3 {
        return 0u;
    }
    let style = hash11(cell.id * 91.0 + 0.37);
    if style > 0.92 {
        return 3u;
    }
    if style > 0.8 {
        return 2u;
    }
    if style > 0.55 {
        return 1u;
    }
    return 0u;
}

// The access panel of a style-1 panel: `p` about its middle, and its half size.
fn surf_skin_hatch(cell: SurfaceCell) -> SurfaceCell {
    var hatch: SurfaceCell;
    hatch.p = cell.p - vec2<f32>(cell.half.x * (hash11(cell.id * 13.0) - 0.5) * 0.5, 0.0);
    hatch.half = cell.half * vec2<f32>(0.34, 0.42);
    hatch.id = cell.id;
    return hatch;
}

fn surf_relief_airframe(i: SurfaceIn, st: vec2<f32>, gap: f32) -> f32 {
    let cell = surf_skin_cell(i, st);
    var h = 1.0 - 0.7 * (1.0 - smoothstep(0.0, gap * 1.4, surf_edge(cell.p, cell.half)));
    h -= 0.35 * surf_skin_fasteners(i, cell, i.px);
    if surf_skin_style(i, cell) == 1u {
        let hatch = surf_skin_hatch(cell);
        h -= 0.6 * (1.0 - smoothstep(0.0, gap, abs(surf_edge(hatch.p, hatch.half))));
    }
    return h;
}

// Height of the surface at `st`, about zero (a seam) to one (a plate's face).
// A reactor viewport's armoured slits: x how open the slit is here (antialiased by
// the caller's pixel), y whether this is inside the frame at all. Slits run along the
// face's longer axis round a drum, across it on a flat port.
fn surf_plasma_slit(i: SurfaceIn, st: vec2<f32>) -> vec2<f32> {
    let fw = max(i.px, 1e-4);
    let rim = min(i.scale * 0.1, min(i.half.x, i.half.y) * 0.2);
    let inner = i.half - vec2<f32>(rim);
    let framed = select(surf_step(surf_edge(st, inner), 0.0, fw), surf_step(inner.y - abs(st.y), 0.0, fw), i.wraps);
    let span = i.half.x;
    let pitch = 2.0 * span / surf_fit(2.0 * span, i.scale * 0.14);
    let f = abs(fract((st.x + span) / pitch) - 0.5) * pitch;
    // Past a few pixels a slit is only a tint: keep its share of light, lose the edges.
    let open = mix(1.0 - surf_step(f, pitch * 0.3, fw), 0.6, smoothstep(pitch * 0.25, pitch * 0.6, fw));
    return vec2<f32>(open, framed);
}

fn surf_relief(i: SurfaceIn, st: vec2<f32>) -> f32 {
    let bevel = i.scale * 0.035;
    let gap = i.scale * 0.012;
    let small = select(min(i.half.x, i.half.y), i.half.y, i.wraps);
    // A face too small to carry an outline is trim: it stays flat.
    let outlined = smoothstep(bevel * 2.5, bevel * 5.0, small);
    var h = mix(1.0, surf_rise(surf_face_edge(i, st), 0.0, bevel * 1.2), outlined);
    switch i.pattern {
        case 1u: {}
        case 2u: { h = min(h, surf_relief_shutter(i, st)); }
        case 3u: { h *= surf_relief_deck(i, st); }
        case 4u: {}
        case 5u: { h = min(h, surf_relief_furnace(i, st, bevel)); }
        case 9u: {
            if small > i.scale * 0.3 {
                h = min(h, surf_relief_airframe(i, st, gap));
            }
        }
        case 10u: {
            // Girth welds where the pile's sections were joined, every few metres up it.
            let pitch = 4.0;
            let d = abs(fract(st.y / pitch + 0.5) - 0.5) * pitch;
            h = min(h, 0.55 + 0.45 * surf_rise(d, 0.03, 0.12));
        }
        case 11u: {}
        // A hull's facets are one skin: no outline where two meet. Its seams are paint.
        case 12u: { h = 1.0; }
        case 13u: {
            let cell = surf_tile_cell(i, st);
            h = 0.8 + 0.2 * surf_rise(surf_edge(cell.p, cell.half), gap * 0.8, bevel * 0.6);
        }
        case 15u: {
            let slit = surf_plasma_slit(i, st);
            h = min(h, 1.0 - 0.8 * slit.x * slit.y);
        }
        case 16u: {
            let d = abs(select(st.y, st.x, i.half.y > i.half.x));
            h = min(h, 0.5 + 0.5 * surf_rise(d, i.scale * 0.05, bevel));
        }
        default: {
            if i.dark {
                // Long plates, shallow seams, and the lights sit in slots.
                let cell = surf_dark_cell(i, st);
                if small > i.scale * 0.3 {
                    h = min(h, 0.45 + 0.55 * surf_rise(surf_edge(cell.p, cell.half), gap * 0.7, bevel * 0.8) * (0.85 + 0.15 * cell.id));
                }
                let dash = surf_dashes(i, st, cell);
                let slot = surf_band(dash.d, i.scale * 0.022, 0.0) * step(0.0, dash.along) * dash.on;
                h -= 0.6 * slot * outlined;
            } else if small > i.scale * 0.3 {
                h = min(h, surf_relief_plates(i, st, bevel, gap));
            }
        }
    }
    return h;
}

// ---- damage -----------------------------------------------------------------

fn surf_hash3(p: vec3<f32>) -> f32 {
    var q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    q += dot(q, q.yxz + 33.33);
    return fract((q.x + q.y) * q.z);
}

// Value noise of a model-space point, one feature per unit. In three dimensions, so
// it is the same on every face that passes through a place and never streaks
// along a face the way a projected 2D field does.
fn surf_noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = p - i;
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let x00 = mix(surf_hash3(i), surf_hash3(i + vec3<f32>(1.0, 0.0, 0.0)), u.x);
    let x10 = mix(surf_hash3(i + vec3<f32>(0.0, 1.0, 0.0)), surf_hash3(i + vec3<f32>(1.0, 1.0, 0.0)), u.x);
    let x01 = mix(surf_hash3(i + vec3<f32>(0.0, 0.0, 1.0)), surf_hash3(i + vec3<f32>(1.0, 0.0, 1.0)), u.x);
    let x11 = mix(surf_hash3(i + vec3<f32>(0.0, 1.0, 1.0)), surf_hash3(i + vec3<f32>(1.0, 1.0, 1.0)), u.x);
    return mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
}

// How much of a noise octave with features `cell` metres across survives at this
// zoom. Noise finer than a few pixels is what turns a thresholded edge into a
// stipple that looks like z-fighting: it fades to its mean instead.
fn surf_resolved(cell: f32, px: f32) -> f32 {
    return smoothstep(2.5 * px, 7.0 * px, cell);
}

// Noise around zero from three octaves, each dropped as it goes under the pixel.
fn surf_fbm3(p: vec3<f32>, cell: f32, px: f32) -> f32 {
    var sum = (surf_noise3(p / cell) - 0.5) * surf_resolved(cell, px);
    let c2 = cell * 0.41;
    sum += (surf_noise3(p / c2 + vec3<f32>(17.1, 9.2, 5.3)) - 0.5) * 0.5 * surf_resolved(c2, px);
    let c3 = cell * 0.17;
    sum += (surf_noise3(p / c3 + vec3<f32>(3.7, 21.4, 11.9)) - 0.5) * 0.25 * surf_resolved(c3, px);
    return sum;
}

// A crack along the level set n = 0.5, `wide` metres across wherever it runs. The
// distance to the level set is the value over the gradient; where the field is
// nearly flat there is no line to speak of, only a plateau that would fill in
// as a blob, so it is left out.
fn surf_crack(n: f32, cell: f32, wide: f32, px: f32) -> f32 {
    let slope = fwidth(n) / max(px, 1e-5);
    let d = abs(n - 0.5) / max(slope, 1e-4);
    return (1.0 - smoothstep(wide * 0.5, wide, d)) * smoothstep(0.35, 0.9, slope * cell);
}

// `models::burns::burn_hash`, line for line.
fn surf_ihash(unit_id: u32, index: u32) -> f32 {
    var n = unit_id * 1597334677u ^ index * 3812015801u;
    n ^= n >> 16u;
    n = n * 2246822519u;
    n ^= n >> 13u;
    n = n * 3266489917u;
    n ^= n >> 16u;
    return f32(n >> 8u) / 16777216.0;
}

// Burns that come up over the hull as it loses health: a few blast marks at
// places fixed for the unit, each growing in as its turn comes. A mark is a
// column, not a ball: it scorches what is above and below it alike, so from
// the game's camera it is one blotch across turret, deck and skirts, not
// slices cut off wherever a part happens to stand higher. x soot, y paint
// burnt off to steel, z unused (was embers).
fn surf_damage(i: SurfaceIn) -> vec3<f32> {
    let hurt = 1.0 - saturate(i.health);
    if hurt < 0.06 {
        return vec3<f32>(0.0);
    }
    let p = i.local + vec3<f32>(i.unit * 53.0, i.unit * 91.0, i.unit * 17.0);
    let grain = i.reach * 0.3;
    // One ragged field for every mark: the edge wanders, broadly, then finely where that can be seen.
    let ragged = surf_fbm3(p, grain, i.px) * 1.1;
    var soot = 0.0;
    var core = 0.0;
    let spread = i.reach * 0.55;
    for (var k = 0u; k < 6u; k++) {
        let fk = f32(k);
        let grow = saturate((hurt - (fk + 0.4) / 6.6) * 5.0);
        if grow <= 0.0 {
            continue;
        }
        // Placed as `models::burns::burn_marks` places them: the smoke has to rise from here.
        let c = vec3<f32>(
            (surf_ihash(i.unit_id, k * 4u) - 0.5) * 2.0 * spread,
            (surf_ihash(i.unit_id, k * 4u + 1u) - 0.5) * 2.0 * spread,
            surf_ihash(i.unit_id, k * 4u + 2u) * i.height,
        );
        let r = i.reach * (0.22 + 0.18 * surf_ihash(i.unit_id, k * 4u + 3u)) * (0.45 + 0.55 * grow);
        var off = i.local - c;
        off.z *= 0.3;
        let d = length(off) / r + ragged;
        soot = max(soot, 1.0 - smoothstep(0.4, 1.05, d));
        core = max(core, 1.0 - smoothstep(0.1, 0.4, d));
    }
    // Nothing on the hull glows: a hurt unit smokes (renderer `damage_smoke`), it does not burn.
    return vec3<f32>(soot, core, 0.0);
}

// ---- the surface ------------------------------------------------------------

fn surface_at(i: SurfaceIn) -> Surface {
    var out: Surface;
    out.slope = vec2<f32>(0.0);
    out.cavity = 1.0;
    out.paint = vec4<f32>(0.0);
    out.team = 0.0;
    out.bare = 0.0;
    out.rough = 0.0;
    out.soot = 0.0;
    out.emissive = vec3<f32>(0.0);

    let fw = max(i.px, 1e-4);
    let bevel = i.scale * 0.035;
    let gap = i.scale * 0.012;
    let hurt = 1.0 - saturate(i.health);
    // A hull is laid out from the model position, not the face: its twisted facets have no frame.
    let framed = i.pattern != PAT_NONE && (max(abs(i.half.x), i.half.y) > 0.0 || i.pattern == PAT_HULL);
    // Lights earn their brightness by tier.
    let lamp = 1.2 + 0.9 * i.tech;

    if framed {
        let st = i.st;
        // Relief, differenced over about a pixel: the normal is filtered with the zoom.
        let e = max(fw * 0.75, i.scale * 0.002);
        let h0 = surf_relief(i, st);
        let depth = bevel * 0.85;
        out.slope = vec2<f32>(surf_relief(i, st + vec2<f32>(e, 0.0)) - h0, surf_relief(i, st + vec2<f32>(0.0, e)) - h0) * (depth / e);
        // Nothing to resolve once a bevel is a small part of a pixel.
        out.slope *= 1.0 - smoothstep(bevel * 3.0, bevel * 9.0, fw);

        let small = select(min(i.half.x, i.half.y), i.half.y, i.wraps);
        let outlined = smoothstep(bevel * 2.5, bevel * 5.0, small);
        let d_face = surf_face_edge(i, st);
        // Edges take the knocks: paint scuffed back along every real edge.
        // Around its mean once it is too fine to see, so a chipped edge never breaks into stipple.
        let scuff = 0.5 + surf_fbm3(i.local + vec3<f32>(i.unit * 29.0), i.scale * 0.22, fw);
        let worn = (1.0 - smoothstep(0.0, bevel * (1.0 + 2.5 * hurt), d_face)) * outlined;
        out.bare = worn * smoothstep(0.75 - 0.55 * hurt, 0.95 - 0.5 * hurt, scuff + worn * 0.35);

        switch i.pattern {
            case 1u: {
                out.cavity = 1.0 - 0.3 * surf_band(d_face, gap * 0.8, fw) * outlined;
            }
            case 2u: {
                // Roller door.
                let pitch = 2.0 * i.half.y / surf_fit(2.0 * i.half.y, i.scale * 0.1);
                let f = fract((st.y + i.half.y) / pitch);
                out.cavity = 1.0 - 0.55 * surf_band((f - 0.03) * pitch, pitch * 0.05, fw);
                let up = (st.y + i.half.y) / (2.0 * i.half.y);
                let sill = 1.0 - surf_step(up, 0.13, fw / (2.0 * i.half.y));
                let stripe = surf_step(abs(fract((st.x + st.y) / (i.scale * 0.32)) - 0.5) * i.scale * 0.32, i.scale * 0.08, fw);
                out.paint = vec4<f32>(mix(vec3<f32>(0.02), SURF_SAFETY, stripe), sill);
                out.team = surf_step(up, 0.88, fw / (2.0 * i.half.y));
                let guide = 1.0 - surf_step(i.half.x - abs(st.x), i.scale * 0.06, fw);
                out.paint = mix(out.paint, vec4<f32>(0.03, 0.03, 0.035, 1.0), guide);
                out.team *= 1.0 - guide;
                // The sill lights while the works are running.
                let line = surf_band(up - 0.145, 0.012, fw / (2.0 * i.half.y)) * (1.0 - guide);
                out.emissive = SURF_AMBER * line * lamp * mix(0.35, 1.6 + 0.6 * sin(i.time * 5.0), i.working);
            }
            case 3u: {
                // Lift deck: a marked print bed. The grid wakes under a scan while the factory builds.
                let grid_n = vec2<f32>(surf_fit(2.0 * i.half.x, i.scale * 0.4), surf_fit(2.0 * i.half.y, i.scale * 0.4));
                let cell = 2.0 * i.half / grid_n;
                let g = abs(fract((st + i.half) / cell + 0.5) - 0.5) * cell;
                let grid = max(surf_band(g.x, i.scale * 0.008, fw), surf_band(g.y, i.scale * 0.008, fw));
                let box_d = surf_edge(st, i.half * 0.86);
                let border = surf_band(box_d, i.scale * 0.03, fw);
                let corner = step(min(i.half.x - abs(st.x), i.half.y - abs(st.y)) , min(i.half.x, i.half.y) * 0.34)
                    * step(max(i.half.x * 0.86 - abs(st.x), i.half.y * 0.86 - abs(st.y)), min(i.half.x, i.half.y) * 0.3);
                let ring = surf_band(length(st) - min(i.half.x, i.half.y) * 0.42, i.scale * 0.02, fw);
                out.paint = vec4<f32>(SURF_SAFETY, max(border * corner, ring * 0.8));
                out.cavity = 1.0 - 0.35 * grid;
                let sweep = (abs(fract(i.time * 0.16) * 2.0 - 1.0) * 2.0 - 1.0) * i.half.x;
                let off = (st.x - sweep) / (i.scale * 0.55);
                let scan = exp(-off * off);
                let core = surf_band(st.x - sweep, i.scale * 0.015, fw);
                out.emissive = SURF_AMBER * (grid * (0.12 + i.working * (0.5 + 2.6 * scan)) + i.working * core * 5.0) * lamp * 0.5;
            }
            case 4u: {
                // Apron: chevrons toward the way out, edge lights that run outward while the factory builds.
                let lane = 1.0 - surf_step(abs(st.y), i.half.y * 0.5, fw);
                let period = i.scale * 0.85;
                let v = fract((st.x - abs(st.y) * 0.9) / period) * period;
                let chevron = surf_band(v - period * 0.5, period * 0.16, fw) * lane
                    * surf_step(surf_edge(st, i.half), i.scale * 0.2, fw);
                out.paint = vec4<f32>(SURF_SAFETY, chevron * 0.9);
                let n = surf_fit(2.0 * i.half.x, i.scale * 0.45);
                let cw = 2.0 * i.half.x / n;
                let k = clamp(floor((st.x + i.half.x) / cw), 0.0, n - 1.0);
                let at = vec2<f32>((k + 0.5) * cw - i.half.x, select(-1.0, 1.0, st.y > 0.0) * i.half.y * 0.84);
                let lampd = distance(st, at);
                let bulb = 1.0 - surf_step(lampd, i.scale * 0.05, fw);
                let housing = 1.0 - surf_step(lampd, i.scale * 0.085, fw);
                let run = pow(1.0 - fract(i.time * 1.1 - k / n * 1.0), 5.0);
                out.paint = mix(out.paint, vec4<f32>(0.02, 0.02, 0.025, 1.0), housing);
                out.emissive = SURF_AMBER * bulb * lamp * mix(0.5, 0.25 + 7.0 * run, i.working);
                out.cavity = 1.0 - 0.3 * surf_band(d_face, gap * 0.8, fw);
            }
            case 5u: {
                // Furnace louvres: the fire shows between the slats.
                let rim = min(i.half.x, i.half.y) * 0.16;
                let inner = i.half - vec2<f32>(rim);
                let e = surf_edge(st, inner);
                var along = st.y;
                var across = st.x / inner.x;
                var span = inner.y;
                if i.half.x > i.half.y {
                    along = st.x;
                    across = st.y / inner.y;
                    span = inner.x;
                }
                let pitch = 2.0 * span / surf_fit(2.0 * span, i.scale * 0.12);
                let slot = (along + span) / pitch;
                let f = fract(slot);
                let open = surf_step(f, 0.62, fw / pitch) * surf_step(e, 0.0, fw);
                // Slow breathing, and a quicker lick of flame that differs slat to slat.
                let lick = value_noise2(vec2<f32>(floor(slot) * 3.7 + i.seed * 40.0, i.time * (0.7 + 1.6 * i.working)), 1.0);
                let breath = 0.75 + 0.25 * sin(i.time * 0.9 + i.unit * 30.0);
                let heat = (1.0 - 0.55 * across * across) * (0.35 + 0.65 * lick) * breath * mix(0.55, 1.35, i.working);
                let fire = mix(mix(vec3<f32>(1.0, 0.1, 0.015), SURF_ORANGE, saturate(heat * 1.6)), vec3<f32>(1.0, 0.8, 0.42), saturate(heat * 1.4 - 0.7));
                out.emissive = fire * open * heat * lamp * 2.4;
                // The slats catch the glow from below.
                out.emissive += SURF_ORANGE * (1.0 - open) * surf_step(e, 0.0, fw) * f * heat * 0.22;
                out.cavity = 1.0 - 0.5 * open;
                out.rough = 0.15;
            }
            case 6u: {
                // Power run: unbroken lines, with pulses that close on the middle while the factory builds.
                let rows = clamp(round(2.0 * i.half.y / (i.scale * 0.16)), 1.0, 3.0);
                let rh = 2.0 * i.half.y / (rows + 1.0);
                let d = abs(fract((st.y + i.half.y) / rh + 0.5) - 0.5) * rh;
                let inside = surf_step(i.half.y - abs(st.y), rh * 0.5, fw) * surf_step(i.half.x - abs(st.x), i.scale * 0.1, fw);
                let line = surf_band(d, i.scale * 0.011, fw) * inside;
                let row = floor((st.y + i.half.y) / rh + 0.5);
                let pulse = pow(fract(abs(st.x) / (i.scale * 1.3) + i.time * 0.85 + row * 0.37), 9.0);
                // Faster and harder on a mobile builder, nearly dark while it is idle.
                let mpulse = pow(fract(abs(st.x) / (i.scale * 0.9) - i.time * 1.6 + row * 0.37), 7.0);
                let pulses = mix(pulse, mpulse, i.mobile);
                out.emissive = SURF_AMBER * line * lamp * (mix(0.4, 0.07, i.mobile) + i.working * (0.5 + mix(5.0, 7.0, i.mobile) * pulses));
                out.cavity = 1.0 - 0.3 * surf_band(d_face, gap * 0.8, fw) * outlined;
            }
            case 9u: {
                // Aircraft skin: fine seams, fastener rows, the odd access panel, stencil and light.
                out.cavity = 1.0 - 0.3 * surf_band(d_face, gap * 0.8, fw) * outlined;
                if small > i.scale * 0.3 {
                    let cell = surf_skin_cell(i, st);
                    let d_cell = surf_edge(cell.p, cell.half);
                    out.cavity *= 1.0 - 0.5 * surf_band(d_cell, gap * 0.7, fw);
                    // Panels changed at different times: none quite the same white, or sheen.
                    out.cavity *= 0.86 + 0.24 * cell.id;
                    out.rough = (hash11(cell.id * 53.0) - 0.5) * 0.2;
                    out.cavity *= 1.0 - 0.35 * surf_skin_fasteners(i, cell, fw);
                    let style = surf_skin_style(i, cell);
                    if style == 1u {
                        let hatch = surf_skin_hatch(cell);
                        out.cavity *= 1.0 - 0.55 * surf_band(surf_edge(hatch.p, hatch.half), gap * 0.8, fw);
                        // A screw in each corner.
                        let r = i.scale * 0.018;
                        out.cavity *= 1.0 - 0.5 * surf_rivets(hatch.p, hatch.half, i.scale * 0.03, 1e3, r)
                            * (1.0 - smoothstep(r * 0.6, r * 1.6, fw));
                    } else if style == 2u {
                        // A stencil: a dark label with lines of print, a safety-orange keep-out bar beside it.
                        let q = cell.p - vec2<f32>(-cell.half.x * 0.35, cell.half.y * 0.4);
                        let label = vec2<f32>(cell.half.x * 0.3, cell.half.y * 0.14);
                        let inside = surf_step(surf_edge(q, label), 0.0, fw);
                        let pitch = label.y * 0.66;
                        let print = surf_band(fract(q.y / pitch + 0.5) * pitch - pitch * 0.5, pitch * 0.14, fw)
                            * surf_step(surf_edge(q, label * vec2<f32>(0.86, 0.8)), 0.0, fw)
                            * step(0.35, hash11(floor(q.x / (i.scale * 0.05)) + cell.id * 7.0));
                        out.paint = vec4<f32>(mix(vec3<f32>(0.035, 0.035, 0.04), vec3<f32>(0.5), print * 0.4), inside * 0.9);
                        let bar = q - vec2<f32>(label.x + i.scale * 0.1, 0.0);
                        let keep = surf_step(surf_edge(bar, vec2<f32>(i.scale * 0.035, label.y)), 0.0, fw);
                        out.paint = mix(out.paint, vec4<f32>(SURF_SAFETY, 1.0), keep);
                    } else if style == 3u {
                        // A formation light: a short level strip, lit on the higher tiers.
                        let q = cell.p - vec2<f32>(0.0, cell.half.y * 0.55);
                        let strip = surf_step(surf_edge(q, vec2<f32>(cell.half.x * 0.45, i.scale * 0.016)), 0.0, fw);
                        let shimmer = 0.8 + 0.2 * sin(i.time * 1.1 + cell.id * 30.0);
                        out.emissive = SURF_ORANGE * strip * lamp * i.lit * shimmer * (1.0 - hurt);
                        out.paint = vec4<f32>(SURF_SAFETY, strip * (1.0 - i.lit));
                        out.cavity *= 1.0 - 0.4 * surf_band(surf_edge(q, vec2<f32>(cell.half.x * 0.45, i.scale * 0.016)), gap, fw);
                    }
                    let chipped = 1.0 - smoothstep(0.0, bevel * (0.6 + 2.0 * hurt), d_cell);
                    out.bare = max(out.bare, chipped * smoothstep(0.82 - 0.5 * hurt, 0.97 - 0.45 * hurt, scuff + chipped * 0.3));
                }
            }
            case 10u: {
                // Steel standing in water, coloured by the waterline (model z = 0): slick
                // and dark under it, a ragged band of weed and rust along it, a pale salt
                // line just over it, and rust weeping down from the deck.
                let z = i.local.z;
                let rag = surf_fbm3(i.local * vec3<f32>(1.0, 1.0, 0.5) + vec3<f32>(i.unit * 31.0), 1.4, fw);
                let wet = 1.0 - smoothstep(-0.5, 0.1, z + rag * 0.8);
                let weed = surf_band(z - 0.1 + rag * 1.2, 0.55, fw);
                let salt = surf_band(z - 0.95 + rag * 0.5, 0.12, fw) * (1.0 - weed);
                let column = surf_noise3(vec3<f32>(i.local.x * 3.1, i.local.y * 3.1, i.unit * 7.0));
                let streak = smoothstep(0.6, 0.85, column) * smoothstep(0.2, 1.2, z) * (0.6 + 0.4 * surf_noise3(i.local * 1.7));
                let rusty = surf_noise3(i.local * 0.9 + vec3<f32>(5.0));
                let growth = mix(vec3<f32>(0.05, 0.08, 0.035), vec3<f32>(0.24, 0.1, 0.04), smoothstep(0.35, 0.7, rusty));
                out.paint = vec4<f32>(vec3<f32>(0.018, 0.022, 0.024), wet * 0.75);
                out.paint = mix(out.paint, vec4<f32>(vec3<f32>(0.5, 0.48, 0.42), 1.0), salt * 0.5);
                out.paint = mix(out.paint, vec4<f32>(vec3<f32>(0.26, 0.11, 0.04), 1.0), streak * 0.55);
                out.paint = mix(out.paint, vec4<f32>(growth, 1.0), weed * (0.75 + 0.25 * rusty));
                out.rough = -0.35 * wet * (1.0 - weed) + 0.3 * weed;
                out.cavity = 1.0 - 0.3 * surf_band(d_face, gap * 0.8, fw) * outlined;
            }
            case 11u: {
                // Safety stripes, black on safety orange, raked at 45 degrees and fitted to
                // the face's length, along its long edges: a kerb, not a painted field.
                let long_x = i.half.x >= i.half.y;
                let along = select(st.y, st.x, long_x);
                let across = select(st.x, st.y, long_x);
                let span = select(i.half.x, i.half.y, long_x);
                let side = select(i.half.y, i.half.x, long_x);
                let pitch = 2.0 * span / surf_fit(2.0 * span, 1.1);
                let f = fract((along + across) / pitch);
                let black = surf_band((f - 0.5) * pitch, pitch * 0.25, fw);
                let kerb = surf_step(abs(across), side - 0.55, fw);
                out.paint = vec4<f32>(mix(SURF_SAFETY, vec3<f32>(0.02, 0.02, 0.025), black), kerb * 0.95);
                out.cavity = 1.0 - 0.3 * surf_band(d_face, gap * 0.8, fw) * outlined;
                out.rough = 0.1 * kerb;
            }
            case 12u: {
                // A ship's side, laid out by the waterline (model z = 0) and by the hull's
                // length, so every mark runs on across facets as one: dark antifouling under
                // the water, a black boot-top at it, then white plating in welded strakes,
                // a hull number and a raked team slash forward, draught marks at the ends.
                let z = i.local.z;
                let x = i.local.x;
                let bt = i.scale * 0.12;
                out.bare = 0.0;
                let rag = surf_fbm3(i.local * vec3<f32>(0.7, 0.7, 1.4) + vec3<f32>(i.unit * 31.0), i.scale * 0.45, fw);
                let anti = 1.0 - surf_step(z, -bt, fw);
                let boot = surf_step(z, -bt, fw) * (1.0 - surf_step(z, bt, fw));
                out.paint = vec4<f32>(vec3<f32>(0.1, 0.034, 0.028), anti);
                out.paint = mix(out.paint, vec4<f32>(0.018, 0.019, 0.021, 1.0), boot);
                // Scum and wet just over the boot-top, ragged where the swell reaches.
                let splash = (1.0 - smoothstep(bt, bt + i.scale * 0.5, z + rag * i.scale * 0.35)) * surf_step(z, bt, fw);
                out.paint = mix(out.paint, vec4<f32>(0.3, 0.31, 0.27, 1.0), splash * 0.45);
                // Grime weeping down from the deck edge and the scuppers.
                let column = surf_noise3(vec3<f32>(x / (i.scale * 0.35), sign(i.local.y) * 3.0, i.unit * 7.0));
                let streak = smoothstep(0.6, 0.85, column) * smoothstep(bt, bt + i.scale * 0.8, z)
                    * (0.55 + 0.45 * surf_noise3(i.local / (i.scale * 0.3)));
                out.paint = mix(out.paint, vec4<f32>(0.3, 0.22, 0.15, 1.0), streak * 0.35 * surf_step(z, bt, fw));
                out.rough = -0.25 * splash + 0.15 * anti;
                // Strakes: level seams, and butts staggered from one strake to the next.
                let pz = i.scale * 0.45;
                let dz = abs(fract(z / pz + 0.5) - 0.5) * pz;
                let row = floor(z / pz);
                let px = i.scale * 2.2;
                let dx = abs(fract(x / px + fract(row * 0.37) + 0.5) - 0.5) * px;
                out.cavity = 1.0 - 0.3 * max(surf_band(dz, gap, fw), surf_band(dx, gap, fw));
                // Plates welded in at different times: not quite the same white.
                out.cavity *= 0.96 + 0.05 * hash21(vec2<f32>(row * 3.1 + i.unit * 17.0, floor(x / px + fract(row * 0.37)) * 1.7));
                // The team's slash, raked back from the waterline to the deck, with a black pin line.
                let slash = x - i.reach * 0.3 + (z - bt) * 0.55;
                let wide = i.scale * 0.55;
                let above = surf_step(z, bt, fw);
                out.team = surf_band(slash - wide * 0.5, wide * 0.5, fw) * above;
                let pin = surf_band(slash - wide * 1.18, i.scale * 0.06, fw) * above;
                out.paint = mix(out.paint, vec4<f32>(0.018, 0.019, 0.021, 1.0), pin);
                // The hull number, two stencilled digits either side of the bow, read from outside.
                let dh = i.scale * 0.28;
                let u = select(x, -x, i.local.y > 0.0) - select(1.0, -1.0, i.local.y > 0.0) * i.reach * 0.56;
                let q = vec2<f32>(u, z - (bt + dh + i.scale * 0.12)) / vec2<f32>(dh, dh);
                let number = i.unit_id % 90u + 10u;
                let first = surf_digit(q + vec2<f32>(0.62, 0.0), number / 10u, fw / dh);
                let second = surf_digit(q + vec2<f32>(-0.62, 0.0), number % 10u, fw / dh);
                let digits = max(first, second) * step(0.0, x);
                out.paint = mix(out.paint, vec4<f32>(0.02, 0.021, 0.024, 1.0), digits * 0.92);
                let marks = surf_draught(i, fw);
                out.paint = mix(out.paint, vec4<f32>(marks.rgb, 1.0), marks.a);
            }
            case 13u: {
                // Anechoic tiles: rubber squares, no two quite the same black, the odd one lost
                // to show the steel under it. Wet under the waterline, a salt crust along it.
                let cell = surf_tile_cell(i, st);
                let d_cell = surf_edge(cell.p, cell.half);
                out.bare = 0.0;
                out.cavity = (1.0 - 0.3 * surf_band(d_cell, gap * 0.8, fw)) * (0.94 + 0.12 * cell.id);
                out.rough = (hash11(cell.id * 53.0) - 0.5) * 0.2;
                let lost = step(0.975, hash11(cell.id * 29.0 + 0.3)) * surf_step(d_cell, gap * 1.5, fw);
                out.paint = vec4<f32>(0.14, 0.13, 0.12, lost * 0.8);
                let z = i.local.z;
                let rag = surf_fbm3(i.local + vec3<f32>(i.unit * 23.0), i.scale * 0.4, fw);
                let salt = surf_band(z - i.scale * 0.1 + rag * i.scale * 0.15, i.scale * 0.05, fw);
                out.paint = mix(out.paint, vec4<f32>(0.42, 0.41, 0.38, 1.0), salt * 0.55);
                out.rough += -0.3 * (1.0 - smoothstep(-0.2, i.scale * 0.2, z + rag * i.scale * 0.3));
                let marks = surf_draught(i, fw);
                out.paint = mix(out.paint, vec4<f32>(marks.rgb, 1.0), marks.a * 0.85);
            }
            case 14u: {
                // Walkway: grey non-skid inside a white margin, a painted line along its edge,
                // tie-down points in a grid fitted to it.
                let margin = min(i.scale * 0.12, min(i.half.x, i.half.y) * 0.3);
                let inner = i.half - vec2<f32>(margin);
                let e = surf_edge(st, inner);
                let field = surf_step(e, 0.0, fw);
                let grit = surf_fbm3(i.local * 3.0, i.scale * 0.05, fw);
                out.paint = vec4<f32>(vec3<f32>(0.21, 0.215, 0.22) * (1.0 + grit * 0.5), field * 0.95);
                let line = surf_band(e - i.scale * 0.05, i.scale * 0.012, fw);
                out.paint = mix(out.paint, vec4<f32>(0.8, 0.8, 0.76, 1.0), line * 0.9);
                out.rough = 0.35 * field;
                let n = vec2<f32>(surf_fit(2.0 * inner.x, i.scale * 0.7), surf_fit(2.0 * inner.y, i.scale * 0.7));
                let pitch = 2.0 * inner / n;
                let g = (fract((st + inner) / pitch) - 0.5) * pitch;
                let r = i.scale * 0.035;
                let tie = (1.0 - surf_step(length(g), r, fw)) * (1.0 - smoothstep(r * 0.5, r * 1.5, fw)) * field;
                out.cavity = (1.0 - 0.3 * surf_band(d_face, gap * 0.8, fw) * outlined) * (1.0 - 0.6 * tie);
            }
            case 15u: {
                // Reactor viewport: armoured slits onto the burning core. The plasma churns
                // up past them, threaded with filaments, and the whole port beats slowly.
                let slit = surf_plasma_slit(i, st);
                let seen = slit.x * slit.y;
                let u = st / i.scale;
                let rise = i.time * (0.55 + 0.1 * i.tech);
                let churn = value_noise2(vec2<f32>(u.x * 1.3 + i.unit * 17.0, u.y * 0.9 - rise), 1.0) * 0.6
                    + value_noise2(vec2<f32>(u.x * 2.9 - rise * 0.4, u.y * 2.1 - rise * 1.7), 1.0) * 0.4;
                // Filaments: the field lines, thin and bright, winding through it.
                let wind = u.y * 1.7 + sin(u.x * 1.9 + i.time * 0.8) * 0.35 - i.time * 0.6;
                let filament = pow(1.0 - abs(fract(wind) * 2.0 - 1.0), 14.0) * smoothstep(0.02 * i.scale, 0.0, fw);
                let beat = 0.82 + 0.18 * pow(0.5 + 0.5 * sin(i.time * 1.6 + i.unit * 40.0), 3.0);
                let heat = saturate(churn * 1.2 - 0.15 + filament * 0.8) * beat;
                let plasma = mix(SURF_PLASMA_DEEP, SURF_PLASMA_HOT, saturate(heat * heat * 1.6));
                out.emissive = plasma * seen * (0.25 + 1.6 * heat) * lamp * 0.9;
                // The frame and the slit cheeks catch the light spilling out.
                out.emissive += SURF_PLASMA_DEEP * (1.0 - slit.x) * slit.y * heat * 0.12 * lamp;
                out.cavity = 1.0 - 0.3 * surf_band(d_face, gap * 0.8, fw) * outlined;
                out.rough = 0.1 * seen;
            }
            case 16u: {
                // Power run: a sunk channel down the long axis carrying the plant's output,
                // pulses running out toward +s all the time a reactor burns.
                let long_x = i.half.x >= i.half.y;
                let along = select(st.y, st.x, long_x);
                let across = abs(select(st.x, st.y, long_x));
                let run = select(i.half.y, i.half.x, long_x);
                let wide = min(select(i.half.x, i.half.y, long_x) * 0.35, i.scale * 0.05);
                let line = surf_band(across, wide * 0.35, fw) * surf_step(run - abs(along), i.scale * 0.05, fw);
                let pulse = pow(fract(along / (i.scale * 1.1) - i.time * (0.9 + 0.2 * i.tech)), 6.0);
                out.emissive = mix(SURF_PLASMA_DEEP, SURF_PLASMA_HOT, pulse) * line * lamp * (0.35 + 2.4 * pulse);
                out.paint = vec4<f32>(0.03, 0.03, 0.035, surf_band(across, wide, fw) * 0.9);
                out.cavity = 1.0 - 0.3 * surf_band(d_face, gap * 0.8, fw) * outlined;
            }
            default: {
                if i.dark {
                    // Little level lights let into the black.
                    let cell = surf_dark_cell(i, st);
                    let dash = surf_dashes(i, st, cell);
                    let w = i.scale * 0.013;
                    if small > i.scale * 0.3 {
                        // Black cannot go darker at a seam: the plates differ in sheen, and their edges catch light.
                        let d_cell = surf_edge(cell.p, cell.half);
                        out.rough = (hash11(cell.id * 53.0) - 0.5) * 0.3;
                        out.bare = max(out.bare, 0.3 * surf_band(d_cell - gap * 1.6, gap * 0.9, fw));
                    }
                    let lit = surf_band(dash.d, w, fw) * surf_step(dash.along, 0.0, fw) * dash.on * outlined;
                    // A slow shimmer along the line; a hurt unit's lights falter and go out.
                    let shimmer = 0.78 + 0.22 * sin(i.time * 1.3 + st.x / i.scale * 1.9 + dash.id * 40.0);
                    let failing = hash11(dash.id * 71.0 + i.unit * 13.0);
                    var alive = 1.0 - smoothstep(failing * 0.9, failing * 0.9 + 0.12, hurt * 1.05);
                    let sputter = step(0.35, hash11(floor(i.time * 9.0 + dash.id * 90.0) * 0.173 + dash.id));
                    alive = max(alive, (1.0 - smoothstep(failing * 0.9 + 0.12, failing * 0.9 + 0.3, hurt)) * sputter);
                    out.emissive = select(SURF_ORANGE, SURF_VIOLET, i.pattern == PAT_VEINED) * lit * shimmer * alive * lamp * i.lit;
                    out.paint = vec4<f32>(SURF_SAFETY, lit * (1.0 - i.lit));
                    out.cavity = 1.0 - 0.5 * surf_band(dash.d, w * 1.9, fw) * surf_step(dash.along, -w, fw) * dash.on * outlined;
                    // Scuffed edges read lighter on black, which is what draws its forms.
                    out.bare = max(out.bare, 0.55 * (1.0 - smoothstep(0.0, bevel * 0.9, d_face)) * outlined);
                } else if small > i.scale * 0.3 {
                    let cell = surf_courses(i, st, vec2<f32>(i.scale * 1.5, i.scale));
                    let d_cell = surf_edge(cell.p, cell.half);
                    out.cavity = 1.0 - 0.62 * surf_band(d_cell, gap, fw);
                    // No two plates quite the same paint or polish: a patchwork of greys.
                    out.cavity *= 0.84 + 0.3 * cell.id;
                    out.rough = (hash11(cell.id * 53.0) - 0.5) * 0.16;
                    let style = hash11(cell.id * 91.0 + 0.37);
                    let roomy = min(cell.half.x, cell.half.y) > i.scale * 0.3;
                    if roomy && style > 0.75 && style <= 0.9 {
                        let e = surf_edge(cell.p, cell.half * vec2<f32>(0.5, 0.46));
                        out.cavity *= 1.0 - 0.55 * surf_band(e, gap * 0.9, fw);
                    } else if roomy && style > 0.9 {
                        let bank = cell.half * vec2<f32>(0.56, 0.42);
                        let e = surf_edge(cell.p, bank);
                        let pitch = 2.0 * bank.y / surf_fit(2.0 * bank.y, i.scale * 0.09);
                        let f = fract((cell.p.y + bank.y) / pitch);
                        out.cavity *= 1.0 - 0.7 * surf_step(e, 0.0, fw) * surf_step(f, 0.55, fw / pitch);
                    }
                    let chipped = (1.0 - smoothstep(0.0, bevel * (0.8 + 2.0 * hurt), d_cell));
                    out.bare = max(out.bare, chipped * smoothstep(0.8 - 0.5 * hurt, 0.97 - 0.45 * hurt, scuff + chipped * 0.3));
                } else {
                    out.cavity = 1.0 - 0.3 * surf_band(d_face, gap * 0.8, fw) * outlined;
                }
                if i.pattern == PAT_TEAM_BAND {
                    // The owner's colour down the middle of the long axis, between two pin lines.
                    let across = select(st.y, st.x, i.half.y > i.half.x);
                    let wide = min(i.half.x, i.half.y);
                    out.team = 1.0 - surf_step(abs(across), wide * 0.2, fw);
                    let pin = surf_band(abs(across) - wide * 0.27, i.scale * 0.012, fw);
                    out.paint = vec4<f32>(0.03, 0.03, 0.035, pin);
                }
            }
        }
    }

    // Large, soft variation, so a big roof is never one flat value.
    let macro_tone = surf_noise3((i.local + vec3<f32>(i.unit * 91.0)) / (i.scale * 2.2));
    out.cavity *= 0.94 + 0.1 * macro_tone;

    let burn = surf_damage(i);
    out.soot = burn.x;
    out.bare = max(out.bare, burn.y * 0.85);
    out.emissive = out.emissive * (1.0 - burn.x);
    return out;
}
