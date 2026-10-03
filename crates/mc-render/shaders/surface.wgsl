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
const PAT_PRECURSOR: u32 = 18u;
// Regency plate (`pattern::EMBER`, 29): plain satin plate, coloured in regency.wgsl.
const PAT_EMBER: u32 = 29u;

// The lights let into dark plating, and the hot end of a furnace.
// Redder than it should look: a bright emitter's green climbs first through the tonemap.
const SURF_ORANGE: vec3<f32> = vec3<f32>(1.0, 0.27, 0.03);
const SURF_AMBER: vec3<f32> = vec3<f32>(1.0, 0.6, 0.1);
const SURF_SAFETY: vec3<f32> = vec3<f32>(0.78, 0.30, 0.03);
// The Precursors' light (`PAT_PRECURSOR`, and `PAT_VEINED` in the black where Aster has
// its orange): an ice blue colder and whiter than Aster's emitters, with less green in it
// so it never drifts toward their cyan; the hot core of a slot is nearly white.
const SURF_PRECURSOR: vec3<f32> = vec3<f32>(0.56, 0.74, 1.0);
const SURF_PRECURSOR_HOT: vec3<f32> = vec3<f32>(0.8, 0.9, 1.0);
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
    // How level the face lies: 0 on a wall, 1 on a deck or a belly.
    up: f32,
    // How steeply the face's s and t axes climb in the world (0 level, 1 straight up).
    rise: vec2<f32>,
    time: f32,
    // The unit is at work (a factory building).
    working: f32,
    // The unit is pulling mass out of a wreck this tick (`UNIT_FLAG_RECLAIMING`).
    reclaiming: f32,
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

// Melted rock's light at `t` (0 cold, 1 white-hot), in HDR: deep red, orange, then near
// white, and far brighter hot than warm, so a red crack is dim beside a white run.
fn surf_melt_rgb(t: f32) -> vec3<f32> {
    let k = clamp(t, 0.0, 1.0);
    var c = mix(vec3<f32>(0.4, 0.015, 0.0), vec3<f32>(1.0, 0.16, 0.02), smoothstep(0.08, 0.45, k));
    c = mix(c, vec3<f32>(1.0, 0.48, 0.12), smoothstep(0.45, 0.75, k));
    c = mix(c, vec3<f32>(1.0, 0.84, 0.62), smoothstep(0.78, 1.0, k));
    return c * (0.4 + k * 1.6 + k * k * k * 4.0);
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

// ---- warship armour ---------------------------------------------------------

// A capital ship's armour (`WARSHIP_PATTERN`). A 500 m hull cut into even courses is a
// brick wall, so nothing here repeats: the face is laid in strakes of uneven depth, each
// strake cut into plates of uneven length, and a plate may carry a hatch, a grille or a
// stencil. Walls carry rows of lit ports in some strakes; running lamps sit at some
// plates' corners. Its light is a warm orange, ports and lamps alike.
// Redder than it looks: a bright emitter's green climbs first through the tonemap.
const WS_LIGHT: vec3<f32> = vec3<f32>(1.0, 0.25, 0.03);
// The port glass where a port is dark.
const WS_GLASS: vec3<f32> = vec3<f32>(0.02, 0.022, 0.026);

struct WarshipPlate {
    // Metres from the plate's middle, along its strake (x) and up it (y), its half size.
    p: vec2<f32>,
    half: vec2<f32>,
    id: f32,
    // The strake's own random.
    strake: f32,
    // Its courses run up the face, not along it (a tall narrow face): no ports.
    turned: bool,
}

// Cells of `pitch` along 0..n*pitch joined into runs: cell k's near edge is a seam where its
// hash clears `breaks`, so runs are one cell to a few long. The run holding `x`: its start,
// its end, its random.
fn ws_run(x: f32, n: f32, pitch: f32, salt: f32, breaks: f32) -> vec3<f32> {
    let c = clamp(floor(x / pitch), 0.0, n - 1.0);
    var lo = c;
    for (var j = 0; j < 5; j++) {
        if lo <= 0.0 || hash21(vec2<f32>(lo, salt)) > breaks {
            break;
        }
        lo -= 1.0;
    }
    var hi = c + 1.0;
    for (var j = 0; j < 5; j++) {
        if hi >= n || hash21(vec2<f32>(hi, salt)) > breaks {
            break;
        }
        hi += 1.0;
    }
    return vec3<f32>(lo * pitch, hi * pitch, hash21(vec2<f32>(lo * 1.37 + 0.5, salt * 0.71 + 3.0)));
}

fn ws_plate(i: SurfaceIn, st_in: vec2<f32>) -> WarshipPlate {
    var st = st_in;
    var half = i.half;
    let turned = !i.wraps && half.y > half.x * 1.5;
    if turned {
        st = st.yx;
        half = half.yx;
    }
    let u = i.scale * 0.8;
    let rows = surf_fit(2.0 * half.y, u * 0.7);
    let rh = 2.0 * half.y / rows;
    let y = st.y + half.y;
    let salt = floor(i.seed * 251.0);
    let s = ws_run(y, rows, rh, salt + 0.5, 0.45);
    let cols = surf_fit(2.0 * half.x, u * 0.85);
    let cw = 2.0 * half.x / cols;
    var x = st.x + half.x;
    if i.wraps {
        x = x - 2.0 * half.x * floor(x / (2.0 * half.x));
    }
    let r = ws_run(x, cols, cw, salt * 7.0 + floor(s.z * 977.0), 0.62);
    var plate: WarshipPlate;
    plate.half = vec2<f32>(r.y - r.x, s.y - s.x) * 0.5;
    plate.p = vec2<f32>(x - (r.x + r.y) * 0.5, y - (s.x + s.y) * 0.5);
    plate.id = r.z;
    plate.strake = s.z;
    plate.turned = turned;
    return plate;
}

// What a plate carries: 0 nothing, 1 a second seam across it, 2 a hatch, 3 a grille,
// 4 a stencil.
fn ws_style(i: SurfaceIn, plate: WarshipPlate) -> u32 {
    let u = i.scale * 0.8;
    let k = hash11(plate.id * 71.3 + 0.17);
    if plate.half.x > u * 1.1 && k < 0.3 {
        return 1u;
    }
    if min(plate.half.x, plate.half.y) < u * 0.3 {
        return 0u;
    }
    if k < 0.44 {
        return 2u;
    }
    if k < 0.54 {
        return 3u;
    }
    if k < 0.6 {
        return 4u;
    }
    return 0u;
}

// The inner rectangle a plate's hatch or grille fills.
fn ws_inset(plate: WarshipPlate) -> SurfaceCell {
    var c: SurfaceCell;
    let off = (hash11(plate.id * 13.0) - 0.5) * plate.half.x * 0.6;
    c.p = plate.p - vec2<f32>(off, 0.0);
    c.half = plate.half * vec2<f32>(0.32, 0.55);
    c.id = plate.id;
    return c;
}

// Where the second seam of a style-1 plate falls, from its middle.
fn ws_split(plate: WarshipPlate) -> f32 {
    return (hash11(plate.id * 29.0) - 0.5) * plate.half.x * 0.8;
}

// The ports along a strake of a wall: lit windows in a row, fitted to each plate. x is
// across a port's middle, y how far inside its outline (positive inside), z its random,
// w 1 where this plate has ports at all.
fn ws_ports(i: SurfaceIn, plate: WarshipPlate) -> vec4<f32> {
    let u = i.scale * 0.8;
    let wall = 1.0 - smoothstep(0.35, 0.55, i.up);
    let row = step(0.8, hash11(plate.strake * 41.0 + 0.3)) * step(plate.half.y, u * 1.05);
    let carry = step(0.45, hash11(plate.id * 5.3 + 0.9)) * step(u * 0.5, plate.half.x);
    // A row of ports runs level: along the strake, which is the face's s, or t if turned.
    let level = 1.0 - smoothstep(0.15, 0.3, select(i.rise.x, i.rise.y, plate.turned));
    let on = wall * row * carry * level;
    if on <= 0.0 {
        return vec4<f32>(0.0);
    }
    let span = plate.half.x - u * 0.25;
    let n = surf_fit(2.0 * span, u * 0.3);
    let pitch = 2.0 * span / n;
    let k = clamp(floor((plate.p.x + span) / pitch), 0.0, n - 1.0);
    let q = vec2<f32>(plate.p.x + span - (k + 0.5) * pitch, plate.p.y - plate.half.y * 0.15);
    let window = vec2<f32>(pitch * 0.2, min(plate.half.y * 0.16, u * 0.07));
    let inside = select(-1e3, surf_edge(q, window), abs(plate.p.x) < span);
    return vec4<f32>(q.x, inside, hash21(vec2<f32>(k, plate.id * 113.0)), 1.0);
}

// A running lamp near one of a plate's lower corners: distance to it, and its random;
// distance is large where the plate has none.
fn ws_lamp(i: SurfaceIn, plate: WarshipPlate) -> vec2<f32> {
    let u = i.scale * 0.8;
    let k = hash11(plate.id * 3.3 + 0.41);
    if k < 0.93 || min(plate.half.x, plate.half.y) < u * 0.25 {
        return vec2<f32>(1e3, 0.0);
    }
    let side = select(-1.0, 1.0, k > 0.965);
    let at = vec2<f32>(side * (plate.half.x - u * 0.16), -plate.half.y + u * 0.16);
    return vec2<f32>(distance(plate.p, at), k);
}

fn surf_relief_warship(i: SurfaceIn, st: vec2<f32>, gap: f32, bevel: f32) -> f32 {
    let plate = ws_plate(i, st);
    // Strake seams are welded deep; the butts between plates in a strake shallower.
    let dy = plate.half.y - abs(plate.p.y);
    let dx = plate.half.x - abs(plate.p.x);
    var h = 1.0 - 0.75 * (1.0 - smoothstep(gap, gap + bevel, dy));
    h = min(h, 1.0 - 0.45 * (1.0 - smoothstep(gap * 0.6, gap * 0.6 + bevel * 0.7, dx)));
    // Plates stand a little proud of one another.
    h *= 0.9 + 0.1 * plate.id;
    let style = ws_style(i, plate);
    if style == 1u {
        h = min(h, 1.0 - 0.35 * (1.0 - smoothstep(gap * 0.5, gap * 0.5 + bevel * 0.5, abs(plate.p.x - ws_split(plate)))));
    } else if style == 2u {
        let hatch = ws_inset(plate);
        let e = surf_edge(hatch.p, hatch.half);
        h -= 0.5 * (1.0 - smoothstep(0.0, gap * 1.4, abs(e))) + 0.12 * step(0.0, e);
    } else if style == 3u {
        let bank = ws_inset(plate);
        let e = surf_edge(bank.p, bank.half);
        if e > 0.0 {
            let pitch = 2.0 * bank.half.y / surf_fit(2.0 * bank.half.y, i.scale * 0.09);
            let f = fract((bank.p.y + bank.half.y) / pitch);
            h -= 0.6 * (1.0 - f) * smoothstep(0.0, bevel, e);
        }
    }
    let ports = ws_ports(i, plate);
    h -= 0.4 * ports.w * smoothstep(-gap, gap, ports.y);
    return h;
}

// The armour's paint, tone, ports and lamps (`surface_at`'s case).
fn surf_warship(i: SurfaceIn, st: vec2<f32>, out_in: Surface, scuff: f32, lamp: f32) -> Surface {
    var out = out_in;
    let fw = max(i.px, 1e-4);
    let gap = i.scale * 0.012;
    let bevel = i.scale * 0.035;
    let hurt = 1.0 - saturate(i.health);
    let plate = ws_plate(i, st);
    let dy = plate.half.y - abs(plate.p.y);
    let dx = plate.half.x - abs(plate.p.x);
    out.cavity = 1.0 - 0.6 * surf_band(dy, gap * 1.2, fw) - 0.4 * surf_band(dx, gap * 0.8, fw);
    // A patchwork: whole strakes a shade apart, plates within them more, and the odd plate
    // replaced in a fresher or older grey.
    out.cavity *= 0.92 + 0.12 * plate.strake;
    out.cavity *= 0.86 + 0.24 * plate.id;
    let odd = hash11(plate.id * 97.0 + 0.6);
    out.cavity *= select(1.0, select(0.7, 1.22, odd > 0.96), odd > 0.91);
    out.rough = (hash11(plate.id * 53.0) - 0.5) * 0.22;
    let style = ws_style(i, plate);
    if style == 1u {
        out.cavity *= 1.0 - 0.45 * surf_band(plate.p.x - ws_split(plate), gap * 0.7, fw);
    } else if style == 2u {
        let hatch = ws_inset(plate);
        out.cavity *= 1.0 - 0.55 * surf_band(surf_edge(hatch.p, hatch.half), gap * 0.9, fw);
    } else if style == 3u {
        let bank = ws_inset(plate);
        let e = surf_edge(bank.p, bank.half);
        let pitch = 2.0 * bank.half.y / surf_fit(2.0 * bank.half.y, i.scale * 0.09);
        let f = fract((bank.p.y + bank.half.y) / pitch);
        out.cavity *= 1.0 - 0.75 * surf_step(e, 0.0, fw) * surf_step(f, 0.5, fw / pitch);
    } else if style == 4u {
        // A stencil: a dark label with lines of print.
        let q = plate.p - vec2<f32>(-plate.half.x * 0.4, plate.half.y * 0.35);
        let label = vec2<f32>(min(plate.half.x * 0.28, i.scale * 0.6), min(plate.half.y * 0.16, i.scale * 0.12));
        let inside = surf_step(surf_edge(q, label), 0.0, fw);
        let pitch = label.y * 0.66;
        let print = surf_band(fract(q.y / pitch + 0.5) * pitch - pitch * 0.5, pitch * 0.14, fw)
            * surf_step(surf_edge(q, label * vec2<f32>(0.88, 0.8)), 0.0, fw)
            * step(0.35, hash11(floor(q.x / (i.scale * 0.06)) + plate.id * 7.0));
        out.paint = vec4<f32>(mix(vec3<f32>(0.035, 0.035, 0.04), vec3<f32>(0.55), print * 0.45), inside * 0.9);
    }
    // Plates are chipped at their seams, worse as the ship is hurt.
    let chipped = 1.0 - smoothstep(0.0, bevel * (0.7 + 2.0 * hurt), min(dx, dy));
    out.bare = max(out.bare, chipped * smoothstep(0.8 - 0.5 * hurt, 0.97 - 0.45 * hurt, scuff + chipped * 0.3));
    // Grime run down the walls from every strake seam, streaked.
    let wall = 1.0 - smoothstep(0.35, 0.55, i.up);
    let below = saturate(1.0 - (plate.half.y - plate.p.y) / (plate.half.y * 2.0 + 1e-3));
    let streak = smoothstep(0.45, 0.85, surf_noise3(vec3<f32>(i.local.x * 0.9, i.local.y * 0.9, i.unit * 5.0 + plate.strake * 11.0)));
    out.cavity *= 1.0 - 0.18 * wall * streak * below * below;

    // Lights: the ports, then the lamps. A hurt ship's go out one by one, sputtering first.
    var light = 0.0;
    let ports = ws_ports(i, plate);
    if ports.w > 0.0 {
        let pane = surf_step(ports.y, 0.0, fw);
        let lit_port = step(0.4, ports.z);
        out.paint = mix(out.paint, vec4<f32>(WS_GLASS, 1.0), pane);
        out.rough = mix(out.rough, -0.35, pane);
        // Each port a little different: some warmer, some dimmer.
        light += pane * lit_port * (0.55 + 0.45 * hash11(ports.z * 31.0));
    }
    let l = ws_lamp(i, plate);
    let r = i.scale * 0.055;
    let bulb = 1.0 - surf_step(l.x, r, fw);
    let housing = 1.0 - surf_step(l.x, r * 1.9, fw);
    out.paint = mix(out.paint, vec4<f32>(0.02, 0.02, 0.022, 1.0), housing);
    // The odd lamp blinks, slowly, out of step with its neighbours.
    let blink = select(1.0, 0.25 + 1.5 * step(0.82, fract(i.time * 0.45 + l.y * 17.0)), l.y > 0.99);
    light += bulb * 2.0 * blink;
    let failing = hash11(plate.id * 71.0 + i.unit * 13.0);
    var alive = 1.0 - smoothstep(failing * 0.9, failing * 0.9 + 0.12, hurt * 1.05);
    let sputter = step(0.35, hash11(floor(i.time * 9.0 + plate.id * 90.0) * 0.173 + plate.id));
    alive = max(alive, (1.0 - smoothstep(failing * 0.9 + 0.12, failing * 0.9 + 0.3, hurt)) * sputter);
    out.emissive = WS_LIGHT * light * alive * lamp * 0.3 * i.lit;
    return out;
}

// ---- precursor plate --------------------------------------------------------

// Where a fragment sits in a Precursor face's cut. The face is divided along its long
// axis into a few big panels by incised grooves that are raked or pointed, never square
// across; a wide face gets a frame line inset along its long edges and a rail that
// breaks off at the cuts; some panels carry an elongated hexagonal inlay, some a light
// slot with pointed ends. No grid, no rivets.
struct PrecursorCut {
    // Distance to the nearest groove's centre line, metres.
    groove: f32,
    // The light slot: distance across its axis, and how far past its pointed ends
    // (negative inside). `lit` is zero where the panel has none.
    slot_across: f32,
    slot_end: f32,
    slot_w: f32,
    lit: f32,
    // The panel's own random.
    id: f32,
}

fn surf_precursor_cut(i: SurfaceIn, st: vec2<f32>, dark: bool) -> PrecursorCut {
    var cut: PrecursorCut;
    // Along the long axis; round a tube, along its length (s goes round it).
    let long_x = !i.wraps && i.half.x >= i.half.y;
    let along = select(st.y, st.x, long_x);
    let across = select(st.x, st.y, long_x);
    let ha = max(select(i.half.y, i.half.x, long_x), 1e-3);
    let hc = max(select(i.half.x, i.half.y, long_x), 1e-3);
    let n = surf_fit(2.0 * ha, i.scale * 2.4);
    let seg = 2.0 * ha / n;
    // How far a rake may carry a cut: never past the neighbouring one.
    let rake = min(hc, seg * 0.7);
    let u = (along + ha) / seg;
    let j0 = round(u);
    var best = 1e9;
    var panel = clamp(floor(u), 0.0, n - 1.0);
    for (var k = -1; k <= 1; k++) {
        let j = j0 + f32(k);
        if j < 0.5 || j > n - 0.5 {
            continue;
        }
        let r = hash21(vec2<f32>(j * 5.13 + i.seed * 37.0, i.seed * 11.0 + 3.1));
        // Raked one way or the other, pointed (a chevron), or now and then straight.
        var off = 0.0;
        var slope = 0.0;
        if !i.wraps {
            if r < 0.3 {
                slope = rake * 0.45 / hc;
                off = across * slope;
            } else if r < 0.6 {
                slope = rake * 0.45 / hc;
                off = -across * slope;
            } else if r < 0.88 {
                slope = rake * 0.6 / hc;
                off = (abs(across) - hc * 0.5) * slope;
            }
        }
        let d = (along - (j * seg - ha) - off) * inverseSqrt(1.0 + slope * slope);
        if abs(d) < best {
            best = abs(d);
            panel = select(j - 1.0, j, d > 0.0);
        }
    }
    cut.id = hash21(vec2<f32>(panel * 3.71 + i.seed * 53.0, i.seed * 7.3 + 0.9));
    var groove = best;
    let wide = hc > i.scale * 0.35 && !i.wraps;
    let inset = min(i.scale * 0.16, hc * 0.2);
    // A frame line inset along the long edges.
    if wide {
        groove = min(groove, abs(abs(across) - (hc - inset)));
    }
    // Lights and rails keep to opposite sides of the face; mirrored faces share the seed.
    let side = select(-1.0, 1.0, hash11(i.seed * 17.0 + 0.3) > 0.5);
    let pc = (panel + 0.5) * seg - ha;
    if wide && hc > i.scale * 0.8 && hash11(cut.id * 29.0) > 0.3 {
        groove = min(groove, abs(across - side * hc * 0.36));
    }
    // An elongated hexagonal inlay on some roomy panels.
    let roomy = wide && seg > i.scale * 1.2;
    let style = hash11(cut.id * 91.0 + 0.37);
    if roomy && style > 0.6 {
        let w = (hc - inset) * 0.42;
        let len = seg * 0.5 - max(i.scale * 0.35, rake * 0.5);
        let dc = abs(across + side * hc * 0.22);
        let hex = max(dc - w, (abs(along - pc) + dc * 0.8) - len);
        groove = min(groove, abs(hex));
    }
    cut.groove = groove;
    // The light: a slot with pointed ends, off to one side of a wide face, down the
    // middle of a narrow one. Dark faces (the recesses) carry more of them.
    let chance = select(0.2, 0.45, dark);
    let slot_at = select(0.0, -side * hc * 0.62, wide);
    let w = clamp(hc * 0.07, i.scale * 0.018, i.scale * 0.05);
    let reach = seg * 0.5 - max(i.scale * 0.22, rake * 0.55);
    let dc = abs(across - slot_at);
    cut.slot_across = dc;
    cut.slot_w = w;
    cut.slot_end = abs(along - pc) + dc - reach;
    cut.lit = select(0.0, 1.0, hash11(cut.id * 7.1 + 0.53) < chance && reach > w * 6.0);
    return cut;
}

// The Precursors' light is one living thing across a machine: a slow breath, and bands
// rising up through it from the ground, a band every half the model's height. Used by
// the plate's slots and by `GLOW_PRECURSOR` alike, so they run together.
fn precursor_pulse(z: f32, height: f32, time: f32, unit: f32) -> f32 {
    let breath = 0.8 + 0.2 * sin(time * 0.75 + unit * 31.0);
    let period = max(height * 0.5, 6.0);
    let f = fract(z / period - time * 0.3 + unit * 5.0);
    return breath * (0.7 + 1.5 * pow(f, 10.0));
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
        case MASS_FLOW_PATTERN: {}
        // Melted rock is one skin, not plates: no outline where two faces meet.
        case MELT_PATTERN: { h = 1.0; }
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
        case WARSHIP_PATTERN: {
            if small > i.scale * 0.3 {
                h = min(h, surf_relief_warship(i, st, gap, bevel));
            }
        }
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
        // Regency plate: its outline only; its panel line is regency.wgsl's.
        case PAT_EMBER: {}
        case 18u: {
            // Deep incised grooves; panels either side stand at slightly different heights,
            // and a light sits down in its slot.
            let cut = surf_precursor_cut(i, st, i.dark);
            h = min(h, 0.05 + 0.95 * surf_rise(cut.groove, i.scale * 0.02, bevel * 0.6)) * (0.9 + 0.1 * cut.id);
            let slot = max(cut.slot_across - cut.slot_w * 1.6, cut.slot_end);
            h = min(h, mix(1.0, 0.25 + 0.75 * surf_rise(slot, 0.0, bevel * 0.5), cut.lit));
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

// ---- field dirt -------------------------------------------------------------

// A thresholded noise field `v` whose features are `cell` metres across, as coverage:
// a crisp edge where the cell can be drawn, else the share of area it would cover.
// The edge is as wide as a pixel is in noise units, so it never stipples.
fn surf_blot(v: f32, at: f32, cell: f32, px: f32) -> f32 {
    let w = max(0.02, 1.5 * px / cell);
    let crisp = smoothstep(at - w, at + w, v);
    let share = saturate(1.6 * (0.82 - at));
    return mix(share, crisp, surf_resolved(cell, px));
}

const SURF_DIRT_TURN: mat3x3<f32> = mat3x3<f32>(vec3<f32>(0.7986, 0.5047, 0.3278),
    vec3<f32>(-0.6018, 0.6698, 0.4350), vec3<f32>(0.0, -0.5446, 0.8387));
const SURF_DIRT_TURN_Z: mat3x3<f32> = mat3x3<f32>(vec3<f32>(0.7986, 0.6018, 0.0),
    vec3<f32>(-0.6018, 0.7986, 0.0), vec3<f32>(0.0, 0.0, 1.0));

// Field dirt on a unit's paint (entity.wgsl): x how much of the paint it covers, y how
// wet and dark it is (0 dry dust, 1 caked mud), z a tone to vary it by, around zero.
// The broad shape is smooth (thickest at the running gear, thinning up the hull); all
// the detail is fine: grain, spatter, crusts, narrow runs and packed seams, each 3D
// noise in model space fading to its share as it goes under a few pixels. Nothing in
// it is blotches a hand to a metre across: on a hull a few metres long that reads as
// camouflage, not dirt.
//   p     model-space point in metres, offset per unit
//   rise  height over the dust line: 0 on the ground, 1 where thrown dust gives out
//   up    how much the surface faces the sky
//   kick  1 where running gear throws dirt up, 0 where nothing does
//   grit  how much weathering settles above that: runs, deck dust, packed seams
//   seam  how deep in a plate seam or rivet ring this is (1 - cavity)
fn surf_dirt(p_model: vec3<f32>, rise: f32, up: f32, kick: f32, grit: f32, seam: f32, px: f32) -> vec3<f32> {
    // Value noise has a lattice, and on a boxy hull whose faces lie along it the lattice
    // shows as squares: the fields are turned off the model's axes (`hz` only about the
    // vertical, for the ones stretched up and down).
    let p = SURF_DIRT_TURN * p_model;
    let hz = SURF_DIRT_TURN_Z * p_model;
    let grain = surf_fbm3(p + vec3<f32>(5.3, 1.7, 8.1), 0.05, px);
    // Thrown up by the tracks or wheels: thickest low, its top edge a little ragged.
    let at = rise + surf_fbm3(hz * vec3<f32>(1.0, 1.0, 2.0), 0.14, px) * 0.35;
    let film = (1.0 - smoothstep(0.0, 1.0, at)) * (0.8 + grain * 0.7);
    // Mud caked on at the very bottom, with a crust edge.
    let crust = at + surf_fbm3(p + vec3<f32>(7.1, 3.3, 1.9), 0.035, px) * 0.12;
    let caked = 1.0 - smoothstep(0.22, 0.25, crust);
    // Drying unevenly: pale crust and dark wet mud in patches a few centimetres across,
    // and here and there the paint showing through.
    let drying = surf_fbm3(p + vec3<f32>(13.0, 29.0, 3.0), 0.04, px);
    let cake = caked * saturate(0.95 + drying * 0.6 + grain * 0.8);
    // Spatter flung up past it: flecks a few centimetres across, thicker low down.
    let fleck_cell = 0.018;
    let flecks = surf_blot(surf_noise3(p / fleck_cell + vec3<f32>(41.0, 13.0, 29.0)),
        mix(0.6, 0.8, saturate(rise * 0.8)), fleck_cell, px) * (1.0 - smoothstep(0.8, 1.4, at));
    let thrown = max(max(film * 0.8, cake), flecks * 0.8) * kick;

    // Runs down the steep faces: grime washed from the ledges in narrow streaks.
    let steep = 1.0 - smoothstep(0.35, 0.8, abs(up));
    let run_cell = 0.05;
    let run = surf_noise3(vec3<f32>(hz.x / run_cell, hz.y / run_cell, hz.z / 0.9) + vec3<f32>(3.0, 19.0, 7.0));
    let runs = surf_blot(run, 0.62, run_cell, px) * steep;
    // A thin, even dust on what faces the sky.
    let deck = smoothstep(0.55, 0.9, up) * (0.7 + grain * 0.8);
    // Grime packed into the plate seams and round the rivets.
    let packed = saturate(seam * 2.2);
    let weather = max(max(runs * 0.5, deck * 0.25), packed * 0.8) * grit;

    let amount = max(thrown, weather);
    let wet = saturate(max(max(caked * (0.6 - drying * 1.4), flecks * 0.7), max(runs, packed * 0.8)) - deck * 0.5);
    return vec3<f32>(amount, wet, grain);
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
        if i.pattern == PAT_EMBER {
            // Regency hide is not paint over steel: its edges only go raw where it is hurt.
            out.bare *= hurt;
        }

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
            case WARSHIP_PATTERN: {
                if small > i.scale * 0.3 {
                    out = surf_warship(i, st, out, scuff, lamp);
                } else {
                    out.cavity = 1.0 - 0.3 * surf_band(d_face, gap * 0.8, fw) * outlined;
                }
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
            case MASS_FLOW_PATTERN: {
                // A chute of reclaimed material: dark glazing over the channel. While the unit
                // reclaims, clumps of glowing material tumble down it. The fall runs in model z,
                // not along the face, so a spiral or a raked run carries the same stream.
                let mass = vec3<f32>(MASS_R, MASS_G, MASS_B);
                let fall = i.local + vec3<f32>(0.0, 0.0, i.time * MASS_FLOW_SPEED + i.unit * 97.0);
                // Clumps about a metre long, stretched down the fall; a finer churn in them.
                let clump = surf_noise3(fall * vec3<f32>(0.9, 0.9, 0.55));
                let churn = 0.5 + surf_fbm3(fall, i.scale * 0.1, fw);
                // Too fine to see: the stream's average, not a flicker.
                let seen = surf_resolved(1.1, fw);
                let lump = mix(0.45, smoothstep(0.35, 0.75, clump), seen) * (0.7 + 0.6 * churn);
                // A rim of frame round the glazing, where the face has room for one.
                let frame = surf_step(d_face, min(bevel * 2.0, small * 0.2), fw);
                let idle = 0.012 + 0.008 * sin(i.time * 1.3 + i.unit * 31.0);
                let glow = mix(idle, 0.35 + 4.5 * lump, i.reclaiming) * frame;
                out.emissive = mass * glow * lamp * 0.55;
                out.paint = vec4<f32>(0.018, 0.017, 0.016, frame * 0.92);
                out.rough = -0.35 * frame;
                out.cavity = 1.0 - 0.3 * surf_band(d_face, gap * 0.8, fw) * outlined;
            }
            case MELT_PATTERN: {
                // Rock a beam has melted (`gpu_consts::melt`): a dark glassy crust broken by
                // cracks of light over a melt that shows through where it is hot enough, all
                // of it creeping down the wall. In model space, so the bore's faces share one
                // field. Hotter the deeper it goes and the higher the tier: a tech 1 bore is
                // mostly crust and red cracks, a deep core's runs orange to white.
                let tier = saturate((i.tech - 1.0) / 3.0);
                let deep = smoothstep(0.0, MELT_DEEP, -i.local.z);
                let heat = saturate(0.3 + 0.25 * tier + (0.3 + 0.2 * tier) * deep);
                let q = i.local + vec3<f32>(i.unit * 53.0, i.unit * 19.0, i.time * MELT_RUN_SPEED);
                // Tongues of melt, drawn out down the wall; gone to their mean under the pixel.
                let tongue_cell = 2.6;
                let tongue = mix(0.5, surf_noise3(q * vec3<f32>(1.0, 1.0, 0.4) / tongue_cell), surf_resolved(tongue_cell, fw));
                let melt = smoothstep(1.05 - heat, 1.25 - heat, tongue + 0.12 * surf_fbm3(q, 1.1, fw));
                // Cracks in the crust: thin lines where a cell field crosses its middle.
                let crack_cell = 1.4;
                let cells = surf_noise3((q + vec3<f32>(7.3, 2.1, 0.0)) / crack_cell);
                let line = max(fw / crack_cell * 1.5, 0.035);
                let crack = (1.0 - smoothstep(0.0, line, abs(cells - 0.5))) * surf_resolved(crack_cell * 0.5, fw);
                let lit = max(melt, crack * (0.35 + 0.65 * heat));
                // The melt breathes a little as the beam works it.
                let breath = 0.88 + 0.12 * sin(i.time * 2.1 + q.z * 0.9 + i.unit * 30.0);
                let t = heat * mix(0.62, 1.0, melt);
                out.emissive = surf_melt_rgb(t) * lit * breath;
                // The crust: black glass, a sheen on it.
                out.paint = vec4<f32>(0.022, 0.018, 0.017, 1.0);
                out.rough = -0.35 * (1.0 - melt);
                out.bare = 0.0;
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
            case 18u: {
                // Precursor plate: pale alloy that reads as dressed stone, cut by deep
                // angular grooves, cold light let into a few of them (`surf_precursor_cut`).
                let cut = surf_precursor_cut(i, st, i.dark);
                let wg = i.scale * 0.02;
                let groove = surf_band(cut.groove, wg, fw);
                out.bare = 0.0;
                out.cavity = (1.0 - 0.75 * groove) * (1.0 - 0.4 * surf_band(d_face, gap * 0.8, fw) * outlined);
                // Panels cut from different blocks: close, never quite the same.
                out.cavity *= 0.93 + 0.1 * cut.id;
                // Stone: a broad cloud and a finer grain, each gone to its mean under the pixel.
                let q = i.local + vec3<f32>(i.unit * 37.0, i.unit * 11.0, 0.0);
                let cloud = surf_fbm3(q, i.scale * 1.3, fw);
                let grain = surf_fbm3(q * vec3<f32>(1.0, 1.0, 2.2) + vec3<f32>(9.1), i.scale * 0.16, fw);
                out.cavity *= 1.0 + 0.22 * cloud + 0.1 * grain;
                out.rough = 0.12 * cloud - 0.06 * grain;
                // The slot: dark glass under a line of light, which bleeds a little onto its lips.
                let inside = surf_band(cut.slot_across, cut.slot_w, fw) * (1.0 - surf_step(cut.slot_end, 0.0, fw)) * cut.lit;
                let lips = surf_band(cut.slot_across, cut.slot_w * 2.6, fw) * (1.0 - surf_step(cut.slot_end, cut.slot_w, fw)) * cut.lit;
                // Nearly out on a badly hurt machine, flickering on the way.
                let fading = smoothstep(0.45, 0.95, hurt);
                let flicker = mix(1.0, step(0.3, hash11(floor(i.time * 7.0) * 0.37 + cut.id * 13.0)), fading);
                let pulse = precursor_pulse(i.local.z, i.height, i.time, i.unit) * (1.0 - 0.8 * fading) * flicker;
                // Along the slot, a slow shimmer so a long run never looks like one flat bar.
                let run = 0.85 + 0.15 * sin(st.x / i.scale * 2.3 + st.y / i.scale * 1.7 - i.time * 1.1 + cut.id * 20.0);
                out.emissive = (SURF_PRECURSOR_HOT * inside + SURF_PRECURSOR * max(lips - inside, 0.0) * 0.18) * pulse * run * 3.4;
                out.paint = vec4<f32>(0.03, 0.05, 0.07, lips * 0.85);
                out.rough -= 0.3 * lips;
            }
            // Regency plate: plain, coloured and lined in regency.wgsl.
            case PAT_EMBER: {}
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
                    // Only the Precursors' veins are lit: Aster's black carries no orange lines.
                    let veined = select(0.0, 1.0, i.pattern == PAT_VEINED);
                    let lit = surf_band(dash.d, w, fw) * surf_step(dash.along, 0.0, fw) * dash.on * outlined * veined;
                    // A slow shimmer along the line; a hurt unit's lights falter and go out.
                    let shimmer = 0.78 + 0.22 * sin(i.time * 1.3 + st.x / i.scale * 1.9 + dash.id * 40.0);
                    let failing = hash11(dash.id * 71.0 + i.unit * 13.0);
                    var alive = 1.0 - smoothstep(failing * 0.9, failing * 0.9 + 0.12, hurt * 1.05);
                    let sputter = step(0.35, hash11(floor(i.time * 9.0 + dash.id * 90.0) * 0.173 + dash.id));
                    alive = max(alive, (1.0 - smoothstep(failing * 0.9 + 0.12, failing * 0.9 + 0.3, hurt)) * sputter);
                    let tone = select(SURF_ORANGE, SURF_PRECURSOR, i.pattern == PAT_VEINED);
                    out.emissive = tone * lit * shimmer * alive * lamp * i.lit;
                    out.paint = vec4<f32>(SURF_SAFETY, lit * (1.0 - i.lit));
                    out.cavity = 1.0 - 0.5 * surf_band(dash.d, w * 1.9, fw) * surf_step(dash.along, -w, fw) * dash.on * outlined * veined;
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
