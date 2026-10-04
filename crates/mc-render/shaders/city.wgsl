// The city kit's looks (mc-models `city/`, gpu_consts.rs `city`): what a city
// structure's CONCRETE faces are, by their pattern byte. Walls in render, brick,
// stone and concrete, each instance its own colours; windows, shopfronts and
// curtain walls drawn on the facade's own grid (the frame in `face`), each pane
// glass with a room behind it, broken pane by pane as the structure is hurt.
// Prepended after surface.wgsl (it uses its noise) to shaders with `//!use city`.

struct CityIn {
    pattern: u32,
    // The point in the model, the face's frame (xy metres from its middle, y up the
    // wall; zw its half size), and the direction to the eye in the face's own axes
    // (x along the face, y up it, z out of it).
    local: vec3<f32>,
    face: vec4<f32>,
    eye: vec3<f32>,
    // The face's normal in the model, and the pixel's size in metres.
    normal: vec3<f32>,
    px: f32,
    // The instance's hash (0..1, from its id: never from an interpolated value, which
    // rounds differently on each triangle) and id, the face's random byte (0..1).
    inst: f32,
    unit_id: u32,
    seed: f32,
    // Its `city_look` word (renderer/city_fx.rs): how badly it is hurt, 0 to 1 (how
    // many of its panes are broken, how scarred its walls); burning, and for how
    // many seconds; burnt out. And the time, for a fire's flicker.
    damage: f32,
    burning: f32,
    gutted: f32,
    age: f32,
    time: f32,
    // 0 by day, 1 by night.
    night: f32,
}

struct CityLook {
    albedo: vec3<f32>,
    roughness: f32,
    metallic: f32,
    // How much of the pixel is glazing, and how much of that still holds glass that
    // reflects (a broken pane leaves a hole and a few shards).
    glass: f32,
    reflect: f32,
    // What is seen through the glass, lit by the daylight that comes in; a lit room's
    // own light; the glass's tint on what it reflects; and a pane's tilt (x along the
    // face, y up it), so no two mirror the sky alike.
    interior: vec3<f32>,
    lamp: vec3<f32>,
    tint: vec3<f32>,
    tilt: vec2<f32>,
    // How much the glass mirrors face on: plain glass, or a tower's coated glazing.
    f0: f32,
    // Light the surface gives off itself: LED bands.
    glow: vec3<f32>,
}

// Finishes: which wall a facade is, and the instance's colour of it.
const CITY_FINISH_RENDER: u32 = 0u;
const CITY_FINISH_BRICK: u32 = 1u;
const CITY_FINISH_STONE: u32 = 2u;
const CITY_FINISH_CONCRETE: u32 = 3u;
const CITY_FINISH_SHEET: u32 = 4u;

fn city_hash(inst: f32, salt: f32) -> f32 {
    return hash11(inst * 913.7 + salt * 17.31);
}

fn city_render_rgb(pick: f32) -> vec3<f32> {
    let i = u32(pick * 8.0);
    var c = vec3<f32>(0.6, 0.55, 0.45);
    if i == 1u { c = vec3<f32>(0.62, 0.47, 0.29); }
    if i == 2u { c = vec3<f32>(0.68, 0.66, 0.61); }
    if i == 3u { c = vec3<f32>(0.6, 0.43, 0.35); }
    if i == 4u { c = vec3<f32>(0.5, 0.5, 0.48); }
    if i == 5u { c = vec3<f32>(0.47, 0.5, 0.42); }
    if i == 6u { c = vec3<f32>(0.5, 0.53, 0.56); }
    if i >= 7u { c = vec3<f32>(0.64, 0.58, 0.4); }
    return c;
}

fn city_brick_rgb(pick: f32) -> vec3<f32> {
    let i = u32(pick * 6.0);
    var c = vec3<f32>(0.34, 0.13, 0.08);
    if i == 1u { c = vec3<f32>(0.27, 0.15, 0.1); }
    if i == 2u { c = vec3<f32>(0.23, 0.085, 0.055); }
    if i == 3u { c = vec3<f32>(0.52, 0.4, 0.26); }
    if i == 4u { c = vec3<f32>(0.3, 0.23, 0.19); }
    if i >= 5u { c = vec3<f32>(0.4, 0.2, 0.12); }
    return c;
}

fn city_stone_rgb(pick: f32) -> vec3<f32> {
    let i = u32(pick * 4.0);
    var c = vec3<f32>(0.57, 0.53, 0.45);
    if i == 1u { c = vec3<f32>(0.53, 0.41, 0.3); }
    if i == 2u { c = vec3<f32>(0.42, 0.42, 0.41); }
    if i >= 3u { c = vec3<f32>(0.5, 0.49, 0.46); }
    return c;
}

fn city_concrete_rgb(pick: f32) -> vec3<f32> {
    let i = u32(pick * 4.0);
    var c = vec3<f32>(0.42, 0.41, 0.39);
    if i == 1u { c = vec3<f32>(0.47, 0.44, 0.39); }
    if i == 2u { c = vec3<f32>(0.33, 0.33, 0.33); }
    if i >= 3u { c = vec3<f32>(0.58, 0.57, 0.54); }
    return c;
}

fn city_sheet_rgb(pick: f32) -> vec3<f32> {
    let i = u32(pick * 6.0);
    var c = vec3<f32>(0.4, 0.41, 0.42);
    if i == 1u { c = vec3<f32>(0.2, 0.27, 0.36); }
    if i == 2u { c = vec3<f32>(0.2, 0.3, 0.22); }
    if i == 3u { c = vec3<f32>(0.6, 0.6, 0.58); }
    if i == 4u { c = vec3<f32>(0.36, 0.14, 0.09); }
    if i >= 5u { c = vec3<f32>(0.27, 0.27, 0.26); }
    return c;
}

// Which finish a facade of `pattern` is on this instance.
fn city_finish_of(pattern: u32, inst: f32) -> u32 {
    let h = city_hash(inst, 1.0);
    if pattern == CITY_RENDER { return CITY_FINISH_RENDER; }
    if pattern == CITY_BRICK { return CITY_FINISH_BRICK; }
    if pattern == CITY_STONE || pattern == CITY_ARCHED || pattern == CITY_LOBBY { return CITY_FINISH_STONE; }
    if pattern == CITY_CONCRETE || pattern == CITY_DECKS || pattern == CITY_CURTAIN { return CITY_FINISH_CONCRETE; }
    if pattern == CITY_SHED { return CITY_FINISH_SHEET; }
    if pattern == CITY_HOUSE {
        return select(CITY_FINISH_BRICK, CITY_FINISH_RENDER, h < 0.55);
    }
    if pattern == CITY_TERRACE {
        if h < 0.5 { return CITY_FINISH_BRICK; }
        return select(CITY_FINISH_STONE, CITY_FINISH_RENDER, h < 0.8);
    }
    if pattern == CITY_FLATS {
        if h < 0.4 { return CITY_FINISH_RENDER; }
        return select(CITY_FINISH_BRICK, CITY_FINISH_CONCRETE, h < 0.72);
    }
    if pattern == CITY_OFFICE {
        if h < 0.5 { return CITY_FINISH_STONE; }
        return select(CITY_FINISH_BRICK, CITY_FINISH_CONCRETE, h < 0.8);
    }
    if pattern == CITY_RIBBON {
        if h < 0.55 { return CITY_FINISH_CONCRETE; }
        return select(CITY_FINISH_RENDER, CITY_FINISH_STONE, h < 0.8);
    }
    return CITY_FINISH_RENDER;
}

fn city_finish_rgb(finish: u32, inst: f32) -> vec3<f32> {
    let pick = city_hash(inst, 2.0);
    if finish == CITY_FINISH_BRICK { return city_brick_rgb(pick); }
    if finish == CITY_FINISH_STONE { return city_stone_rgb(pick); }
    if finish == CITY_FINISH_CONCRETE { return city_concrete_rgb(pick); }
    if finish == CITY_FINISH_SHEET { return city_sheet_rgb(pick); }
    return city_render_rgb(pick);
}

// Anti-aliased: 1 inside |d| < half, over about a pixel.
fn city_box(d: f32, half: f32, px: f32) -> f32 {
    return clamp((half - abs(d)) / max(px, 1e-5) + 0.5, 0.0, 1.0);
}

// A line `w` wide at d = 0, one every `period` metres, anti-aliased: as it goes under
// the pixel it fades to the share of the wall its lines cover.
fn city_line(d: f32, w: f32, period: f32, px: f32) -> f32 {
    let cover = clamp((w * 0.5 - abs(d)) / max(px, 1e-5) + 0.5, 0.0, 1.0);
    return mix(min(w / period, 1.0), cover, smoothstep(0.7 * px, 2.0 * px, w));
}

// The integral of a train of lines `w` wide, one every `period`, centred on its
// multiples, from 0 to x.
fn city_lines_integral(x: f32, w: f32, period: f32) -> f32 {
    let y = x + 0.5 * w;
    return floor(y / period) * w + clamp(fract(y / period) * period, 0.0, w);
}

// That train of lines box-filtered over a pixel `fw` wide (in the same units): how
// much of the pixel is line. Exact, so a fine joint never shimmers into moire.
fn city_lines(x: f32, w: f32, period: f32, fw: f32) -> f32 {
    let h = max(fw, 1e-4) * 0.6;
    return clamp((city_lines_integral(x + h, w, period) - city_lines_integral(x - h, w, period)) / (2.0 * h), 0.0, 1.0);
}

// A wall's own texture at `st` (metres on the wall: x along, y up), on `rgb`.
fn city_wall(finish: u32, rgb: vec3<f32>, st: vec2<f32>, local: vec3<f32>, px: f32, inst: f32) -> vec3<f32> {
    var c = rgb;
    let fw = fwidth(st);
    let broad = surf_fbm3(local + vec3<f32>(inst * 300.0), 9.0, px);
    c *= 1.0 + 0.35 * broad;
    if finish == CITY_FINISH_BRICK {
        // Stretcher bond: 65 mm bricks in 75 mm courses, 225 mm long, 10 mm joints.
        let course = floor(st.y / 0.075);
        let shift = 0.5 * fract(course * 0.5);
        let brick = floor(st.x / 0.225 + shift);
        let tone = hash21(vec2<f32>(brick, course) + inst * 37.0);
        c *= mix(1.0, 0.8 + 0.4 * tone, surf_resolved(0.15, px));
        let bed = city_lines(st.y, 0.01, 0.075, fw.y);
        let perp = city_lines(st.x + shift * 0.225, 0.01, 0.225, fw.x) * (1.0 - bed);
        c = mix(c, vec3<f32>(0.42, 0.4, 0.36), (bed + perp) * 0.7);
    } else if finish == CITY_FINISH_STONE {
        let course = floor(st.y / 0.42);
        let shift = 0.5 * fract(course * 0.5);
        let block = floor(st.x / 0.9 + shift);
        c *= mix(1.0, 0.9 + 0.2 * hash21(vec2<f32>(block, course) + inst * 11.0), surf_resolved(0.4, px));
        let bed = city_lines(st.y, 0.012, 0.42, fw.y);
        let perp = city_lines(st.x + shift * 0.9, 0.012, 0.9, fw.x) * (1.0 - bed);
        c *= 1.0 - 0.3 * (bed + perp);
    } else if finish == CITY_FINISH_CONCRETE {
        // Precast panels: a joint every 3.6 m along and at each floor, form-tie holes.
        let jx = fract(st.x / 3.6);
        let jy = fract(st.y / 3.2);
        let vis = surf_resolved(0.3, px);
        let joint = max(1.0 - smoothstep(0.0, 0.006, min(jx, 1.0 - jx)), 1.0 - smoothstep(0.0, 0.01, min(jy, 1.0 - jy)));
        c *= 1.0 - 0.3 * joint * vis;
        c *= 1.0 + 0.08 * (hash21(floor(st / vec2<f32>(3.6, 3.2)) + inst) - 0.5) * vis;
    } else if finish == CITY_FINISH_SHEET {
        // Profiled sheet: a trapezoid rib every 0.25 m, upright.
        let rib = fract(st.x / 0.25);
        let vis = surf_resolved(0.25, px);
        c *= 1.0 - 0.22 * vis * smoothstep(0.35, 0.5, abs(rib - 0.5));
    } else {
        // Render: smooth, a little blotchy where it was patched.
        let patched = smoothstep(0.55, 0.75, surf_noise3(local * 0.35 + vec3<f32>(inst * 50.0)));
        c *= 1.0 - 0.07 * patched;
    }
    // Grime up from the ground, and rain run down from the top of the wall.
    c *= 1.0 - 0.28 * (1.0 - smoothstep(0.0, 2.2, local.z));
    let streak = smoothstep(0.55, 0.85, surf_noise3(vec3<f32>(st.x * 1.3, st.y * 0.06, inst * 9.0)));
    c *= 1.0 - 0.12 * streak * surf_resolved(0.8, px);
    return c;
}

// A wall knocked about: pocked where shells and bullets struck (dark pits with a
// fresh pale rim), scorched in broad patches, all of it sooted once gutted.
fn city_scars(c: vec3<f32>, i: CityIn) -> vec3<f32> {
    let d = i.damage;
    if d <= 0.0 && i.gutted < 0.5 {
        return c;
    }
    var out = c;
    let n = surf_noise3(i.local * 0.9 + vec3<f32>(i.inst * 57.0));
    let cut = 0.86 - 0.3 * d;
    let pit = smoothstep(cut, cut + 0.03, n);
    let rim = smoothstep(cut - 0.05, cut, n) * (1.0 - pit);
    let vis = surf_resolved(0.5, i.px);
    out = mix(out, vec3<f32>(0.62, 0.6, 0.55), rim * 0.6 * vis * step(0.01, d));
    out = mix(out, vec3<f32>(0.08, 0.075, 0.07), pit * 0.9 * step(0.01, d));
    let scorch = smoothstep(0.55, 0.85, surf_noise3(i.local * 0.09 + vec3<f32>(i.inst * 13.0, 3.0, 7.0)));
    out = mix(out, vec3<f32>(0.03, 0.028, 0.026), scorch * smoothstep(0.3, 1.0, d) * 0.8);
    // Burnt out: soot over everything, heaviest high up where the smoke rolled out.
    let streak = surf_noise3(vec3<f32>(i.local.x * 0.7, i.local.y * 0.7, i.local.z * 0.05));
    out = mix(out, vec3<f32>(0.03, 0.026, 0.024), i.gutted * (0.45 + 0.35 * streak));
    return out;
}

// A pane's key and how it fares: broken when its hash falls under the damage.
fn city_pane_key(i: CityIn, cell: vec2<f32>, column: f32) -> f32 {
    let dir = u32(floor(atan2(i.normal.y, i.normal.x) * 1.273 + 4.5)) & 7u;
    let index = u32(cell.x + 1024.0) + 2053u * u32(cell.y + 64.0) + 131071u * u32(column)
        + 524287u * u32(i.seed * 255.0) + 9u * dir;
    return surf_ihash(i.unit_id, index);
}

struct CityRoom {
    rgb: vec3<f32>,
    // How lit the room is by its own lamps at night, 0 or 1.
    lit: f32,
}

// The room behind a window, by interior mapping: a box `half` wide and tall (the
// cell) and `depth` deep behind the glass, `at` the point on the glass from the
// cell's middle, `d` the view ray in the face's axes (z out of the wall).
fn city_room(i: CityIn, at: vec2<f32>, half: vec2<f32>, depth: f32, key: f32, office: bool) -> CityRoom {
    var out: CityRoom;
    let d = -i.eye;
    let dz = min(d.z, -0.05);
    var t = depth / -dz;
    var hit = 0u;
    if abs(d.x) > 1e-4 {
        let tx = (sign(d.x) * half.x - at.x) / d.x;
        if tx < t { t = tx; hit = 1u; }
    }
    if abs(d.y) > 1e-4 {
        let ty = (sign(d.y) * half.y - at.y) / d.y;
        if ty < t { t = ty; hit = select(3u, 2u, d.y > 0.0); }
    }
    let p = at + d.xy * t;
    let wall_pick = hash11(key * 71.0 + 3.0);
    var wall = mix(vec3<f32>(0.55, 0.5, 0.42), vec3<f32>(0.45, 0.5, 0.52), step(0.6, wall_pick));
    wall = mix(wall, vec3<f32>(0.5, 0.45, 0.36), step(0.85, wall_pick));
    if office {
        wall = vec3<f32>(0.5, 0.5, 0.48);
    }
    var c = wall;
    if hit == 1u {
        c = wall * 0.75;
    } else if hit == 2u {
        c = vec3<f32>(0.62, 0.6, 0.56);
    } else if hit == 3u {
        c = select(vec3<f32>(0.16, 0.1, 0.06), vec3<f32>(0.18, 0.18, 0.19), office);
    } else {
        // The back wall: a dark band of furniture along its foot, a picture or a door.
        let low = p.y + half.y;
        c = mix(c, c * 0.35, step(low, 0.9) * step(0.3, hash11(key * 13.0)));
        let frame_at = (hash11(key * 5.0) - 0.5) * half.x;
        c = mix(c, vec3<f32>(0.2, 0.18, 0.16), step(abs(p.x - frame_at), 0.35) * step(abs(low - 1.7), 0.3) * step(0.5, hash11(key * 19.0)));
    }
    // Deeper is darker: the daylight falls off into the room.
    c *= exp(-t * 0.22);
    out.rgb = c;
    out.lit = select(0.0, 1.0, hash11(key * 29.0 + 7.0) < 0.24);
    return out;
}

// The grid of a facade pattern: storey and bay (m), the window's share of the cell's
// width, its sill and head (shares of the cell's height), panes across, room depth.
struct CityGrid {
    storey: f32,
    bay: f32,
    wide: f32,
    sill: f32,
    head: f32,
    panes: f32,
    depth: f32,
}

fn city_grid(p: u32) -> CityGrid {
    var g = CityGrid(CITY_FLATS_STOREY, CITY_FLATS_BAY, CITY_FLATS_WIDE, CITY_FLATS_SILL, CITY_FLATS_HEAD, f32(CITY_FLATS_PANES), 4.5);
    if p == CITY_HOUSE {
        g = CityGrid(CITY_HOUSE_STOREY, CITY_HOUSE_BAY, CITY_HOUSE_WIDE, CITY_HOUSE_SILL, CITY_HOUSE_HEAD, f32(CITY_HOUSE_PANES), 3.8);
    } else if p == CITY_TERRACE {
        g = CityGrid(CITY_TERRACE_STOREY, CITY_TERRACE_BAY, CITY_TERRACE_WIDE, CITY_TERRACE_SILL, CITY_TERRACE_HEAD, f32(CITY_TERRACE_PANES), 4.0);
    } else if p == CITY_OFFICE {
        g = CityGrid(CITY_OFFICE_STOREY, CITY_OFFICE_BAY, CITY_OFFICE_WIDE, CITY_OFFICE_SILL, CITY_OFFICE_HEAD, f32(CITY_OFFICE_PANES), 7.0);
    } else if p == CITY_RIBBON {
        g = CityGrid(CITY_RIBBON_STOREY, CITY_RIBBON_BAY, CITY_RIBBON_WIDE, CITY_RIBBON_SILL, CITY_RIBBON_HEAD, f32(CITY_RIBBON_PANES), 7.0);
    } else if p == CITY_CURTAIN {
        g = CityGrid(CITY_CURTAIN_STOREY, CITY_CURTAIN_BAY, CITY_CURTAIN_WIDE, CITY_CURTAIN_SILL, CITY_CURTAIN_HEAD, f32(CITY_CURTAIN_PANES), 9.0);
    } else if p == CITY_ARCHED {
        g = CityGrid(CITY_ARCHED_STOREY, CITY_ARCHED_BAY, CITY_ARCHED_WIDE, CITY_ARCHED_SILL, CITY_ARCHED_HEAD, f32(CITY_ARCHED_PANES), 12.0);
    } else if p == CITY_SHED {
        g = CityGrid(1000.0, CITY_SHED_BAY, 0.92, CITY_SHED_SILL, CITY_SHED_HEAD, f32(CITY_SHED_PANES), 14.0);
    } else if p == CITY_GUTTED {
        g = CityGrid(CITY_GUTTED_STOREY, CITY_GUTTED_BAY, 0.46, 0.28, 0.84, 1.0, 5.0);
    }
    return g;
}

fn city_n_of(len: f32, pitch: f32) -> f32 {
    return max(1.0, round(len / pitch));
}

// A facade with windows on its grid.
fn city_facade(i: CityIn) -> CityLook {
    var o: CityLook;
    o.roughness = 0.88;
    o.tint = vec3<f32>(1.0);
    let g = city_grid(i.pattern);
    let finish = city_finish_of(i.pattern, i.inst);
    var base = city_finish_rgb(finish, i.inst);
    let w = 2.0 * abs(i.face.z);
    let h = 2.0 * abs(i.face.w);
    let u = i.face.x + abs(i.face.z);
    let v = i.face.y + abs(i.face.w);
    let shed = i.pattern == CITY_SHED;
    let curtain = i.pattern == CITY_CURTAIN;
    let ribbon = i.pattern == CITY_RIBBON;
    let gutted = i.pattern == CITY_GUTTED || i.gutted > 0.5;
    let n = city_n_of(w, g.bay);
    let m = select(city_n_of(h, g.storey), 1.0, shed);
    let cw = w / n;
    let ch = h / m;
    let cell = vec2<f32>(floor(u / cw), floor(v / ch));
    let cu = u - (cell.x + 0.5) * cw;
    let cv = v - cell.y * ch;
    if gutted {
        base = mix(base, vec3<f32>(0.1, 0.09, 0.085), 0.55);
    }
    var wall = city_scars(city_wall(finish, base, vec2<f32>(u, v), i.local, i.px, i.inst), i);
    if curtain {
        // Spandrels: dark glass over the slab, with the mullions running on through.
        wall = mix(vec3<f32>(0.05, 0.06, 0.07), base * 0.3, 0.3);
        o.roughness = 0.3;
        o.metallic = 0.6;
    }
    // The window in its cell; an arched one rounds its head.
    let win_half = vec2<f32>(0.5 * g.wide * cw, 0.5 * (g.head - g.sill) * ch);
    let win_mid = vec2<f32>(0.0, 0.5 * (g.head + g.sill) * ch);
    var q = vec2<f32>(cu, cv) - win_mid;
    var inside_y = city_box(q.y, win_half.y, i.px);
    if i.pattern == CITY_ARCHED {
        let spring = win_half.y - win_half.x;
        let above = max(q.y - spring, 0.0);
        let r = length(vec2<f32>(q.x, above));
        inside_y *= clamp((win_half.x - r) / max(i.px, 1e-5) + 0.5, 0.0, 1.0);
    }
    // The window itself reads while it is a pixel or two across; its bars, blinds and
    // sill only once they are a few pixels across.
    let resolve = smoothstep(0.6 * i.px, 1.8 * i.px, min(win_half.x, win_half.y));
    let detail = smoothstep(1.2 * i.px, 3.5 * i.px, min(win_half.x / g.panes, win_half.y * 0.3));
    let area = g.wide * (g.head - g.sill);
    let opening = mix(area, city_box(q.x, win_half.x, i.px) * inside_y, resolve);
    // Frames, mullions between panes, a transom near the head.
    let frame_w = select(0.07, 0.05, curtain);
    let pane_w = 2.0 * win_half.x / g.panes;
    let column = clamp(floor((q.x + win_half.x) / pane_w), 0.0, g.panes - 1.0);
    let pq = q.x + win_half.x - (column + 0.5) * pane_w;
    var bars = city_line(abs(pq) - 0.5 * pane_w, frame_w, pane_w, i.px);
    bars = max(bars, city_line(abs(q.y) - win_half.y, frame_w * 1.4, win_half.y * 2.0, i.px));
    if !curtain && !shed && i.pattern != CITY_ARCHED {
        bars = max(bars, city_line(q.y - win_half.y * 0.55, frame_w, win_half.y * 2.0, i.px) * step(2.4, 2.0 * win_half.y));
    }
    if shed {
        bars = max(bars, city_line(fract(q.y / 0.9 + 0.5) - 0.5, frame_w / 0.9, 1.0, i.px / 0.9));
    }
    bars *= detail;
    let key = city_pane_key(i, cell, column);
    let room_key = city_pane_key(i, cell, 0.0);
    // Broken: the hash under the damage, a few panes holding however bad it gets. A
    // gutted shell has nothing left in its holes.
    let broken = select(step(key, min(i.damage * 1.15, 0.9)), 1.0, gutted);
    // A burning building: fire in some of its broken rooms, spreading over more
    // storeys the longer it burns, flickering.
    let seat = floor(hash11(i.inst * 31.0) * m);
    let reach = 0.5 + i.age / 25.0;
    let fire_share = clamp(0.25 + i.age / 80.0, 0.25, 0.85);
    let flames = i.burning * (1.0 - gutted_f(gutted)) * broken
        * step(abs(cell.y - seat), reach) * step(hash11(room_key * 91.0), fire_share);
    let flicker = 0.65 + 0.35 * sin(i.time * 9.0 + room_key * 40.0) * sin(i.time * 13.7 + room_key * 17.0);
    // Shards left in the frame of a broken pane.
    let edge = min(win_half.y - abs(q.y), 0.5 * pane_w - abs(pq));
    let shard = (1.0 - gutted_f(gutted)) * broken
        * step(edge, 0.05 + 0.25 * surf_noise3(vec3<f32>(i.local.xy * 7.0 + i.local.z * 3.0, key * 40.0)));
    let room = city_room(i, vec2<f32>(cu, cv - 0.5 * ch), vec2<f32>(0.5 * cw, 0.5 * ch), g.depth, room_key, curtain || ribbon || i.pattern == CITY_OFFICE);
    var inner = room.rgb;
    // Blinds and curtains, drawn down a share of the window, on intact panes.
    let blind = hash11(room_key * 43.0) * 0.7 - 0.12;
    let blind_at = step(win_half.y - q.y, blind * 2.0 * win_half.y);
    inner = mix(inner, vec3<f32>(0.62, 0.58, 0.5) * (0.85 + 0.1 * step(0.5, fract(q.y * 12.0))), blind_at * (1.0 - broken) * detail * select(1.0, 0.4, curtain));
    let drape = step(0.55, hash11(room_key * 61.0)) * step(win_half.x * 0.75, abs(q.x)) * (1.0 - broken);
    let drape_rgb = mix(vec3<f32>(0.42, 0.2, 0.14), vec3<f32>(0.5, 0.48, 0.4), hash11(room_key * 3.0));
    inner = mix(inner, drape_rgb * 0.6, drape * detail * select(1.0, 0.0, curtain));
    // A broken room is dark and burnt.
    inner = mix(inner, inner * vec3<f32>(0.3, 0.27, 0.25), broken * select(1.0, 1.3, gutted));
    // Far off a window is its average: dark glass.
    inner = mix(vec3<f32>(0.08, 0.075, 0.07), inner, resolve);
    // Sills under the windows, rain run down from them, soot over the broken ones.
    var wall_here = wall;
    if !curtain && !shed {
        let sill = city_box(cu, win_half.x + 0.08, i.px) * city_box(q.y + win_half.y + 0.06, 0.06, i.px);
        wall_here = mix(wall_here, base * 1.25 + vec3<f32>(0.04), sill * detail);
        let below = clamp(-(q.y + win_half.y + 0.12) / 0.9, 0.0, 1.0);
        let run = city_box(cu, win_half.x * 0.8, i.px) * step(0.001, below) * (1.0 - below);
        wall_here *= 1.0 - 0.07 * run * detail;
        // Soot licked up the wall over broken windows, taller and blacker over a fire.
        let tall = select(1.5, 2.6, flames > 0.0 || gutted);
        let over = clamp((q.y - win_half.y) / (tall * ch), 0.0, 1.0);
        let lick = 0.75 + 0.5 * surf_noise3(vec3<f32>(i.local.xy * 0.8, i.local.z * 0.25));
        let burnt = select(broken, 1.0, gutted) * step(0.001, over) * (1.0 - over * lick);
        let plume = city_box(cu, win_half.x * (1.0 + 0.8 * over), i.px);
        let strength = max(smoothstep(0.25, 0.9, i.damage), max(flames, gutted_f(gutted)));
        wall_here = mix(wall_here, vec3<f32>(0.025, 0.022, 0.02), clamp(burnt * plume * strength, 0.0, 0.92));
    }
    let frame_rgb = select(vec3<f32>(0.62, 0.61, 0.58), vec3<f32>(0.12, 0.12, 0.13), curtain || ribbon || i.pattern == CITY_OFFICE || shed || hash11(i.inst * 5.0) < 0.35);
    let frame = max(bars, shard * 0.7) * opening;
    o.albedo = mix(wall_here, frame_rgb, frame);
    o.glass = opening * (1.0 - frame);
    o.reflect = (1.0 - broken) * (1.0 - select(0.0, 1.0, gutted));
    o.interior = inner;
    o.lamp = inner * select(vec3<f32>(1.0, 0.72, 0.42), vec3<f32>(0.85, 0.9, 1.0), curtain || i.pattern == CITY_OFFICE)
        * 2.2 * room.lit * i.night * (1.0 - broken) * (1.0 - gutted_f(gutted));
    // Fire seen through the hole: hottest low in the room, smoke darkening its head.
    let blaze = mix(vec3<f32>(1.0, 0.5, 0.12), vec3<f32>(0.9, 0.22, 0.04), smoothstep(-win_half.y, win_half.y, q.y));
    o.lamp += blaze * (4.0 * flicker) * flames * (1.0 - 0.7 * smoothstep(0.2, 1.0, q.y / max(win_half.y, 0.1)));
    o.interior = mix(o.interior, vec3<f32>(0.6, 0.25, 0.06), flames * 0.6);
    var tint = vec3<f32>(0.92, 0.97, 1.0);
    if curtain {
        let t = hash11(i.inst * 77.0);
        tint = select(select(vec3<f32>(0.55, 0.85, 0.85), vec3<f32>(0.95, 0.75, 0.5), t < 0.6), vec3<f32>(0.75, 0.85, 1.0), t < 0.35);
        o.interior *= 0.55;
    }
    o.tint = tint * (0.85 + 0.3 * hash11(key * 17.0));
    o.f0 = select(0.06, 0.24, curtain);
    o.tilt = (vec2<f32>(hash11(key * 7.0), hash11(key * 9.0)) - 0.5) * select(0.06, 0.025, curtain);
    return o;
}

fn gutted_f(gutted: bool) -> f32 {
    return select(0.0, 1.0, gutted);
}

// A shopfront from the ground: stall riser, plate glass in a frame, the fascia with
// each shop's sign, and a pier between shops. Over it the wall carries on.
fn city_shop(i: CityIn) -> CityLook {
    var o: CityLook;
    o.roughness = 0.85;
    o.tint = vec3<f32>(0.92, 0.97, 1.0);
    let w = 2.0 * abs(i.face.z);
    let u = i.face.x + abs(i.face.z);
    let n = city_n_of(w, CITY_SHOP_BAY);
    let cw = w / n;
    let bay = floor(u / cw);
    let cu = u - (bay + 0.5) * cw;
    let z = i.local.z;
    let finish = select(CITY_FINISH_RENDER, CITY_FINISH_STONE, city_hash(i.inst, 4.0) < 0.5);
    let base = city_finish_rgb(finish, i.inst);
    let wall = city_scars(city_wall(finish, base, vec2<f32>(u, z), i.local, i.px, i.inst), i);
    let shop = hash11(bay * 3.1 + i.inst * 41.0 + i.seed * 7.0);
    // A door in some bays: the glass to the ground, off to one side.
    let pier = 0.35;
    let glass_half = 0.5 * cw - pier;
    let door_at = (hash11(shop * 9.0) - 0.5) * (glass_half - 0.7);
    let door = step(0.4, shop) * city_box(cu - door_at, 0.6, i.px);
    let lo = mix(CITY_SHOP_RISER, 0.0, door);
    let opening = city_box(cu, glass_half, i.px) * step(lo, z) * step(z, CITY_SHOP_HEAD);
    let fascia = city_box(cu, 0.5 * cw - pier * 0.5, i.px) * step(CITY_SHOP_HEAD + 0.12, z) * step(z, CITY_SHOP_TOP - 0.15);
    // The sign: the shop's colour, its letters a pale line through the middle.
    let hue = hash11(shop * 23.0);
    var sign = mix(vec3<f32>(0.08, 0.12, 0.2), vec3<f32>(0.35, 0.06, 0.05), step(0.35, hue));
    sign = mix(sign, vec3<f32>(0.08, 0.2, 0.1), step(0.6, hue));
    sign = mix(sign, vec3<f32>(0.04, 0.04, 0.04), step(0.8, hue));
    let letters = city_box(cu, glass_half * 0.55, i.px) * city_box(z - 0.5 * (CITY_SHOP_HEAD + CITY_SHOP_TOP), 0.16, i.px)
        * step(0.35, fract(cu * 2.3 + hue * 9.0));
    sign = mix(sign, vec3<f32>(0.75, 0.72, 0.62), letters);
    let pane_w = 2.0 * glass_half / f32(CITY_SHOP_PANES);
    let column = clamp(floor((cu + glass_half) / pane_w), 0.0, f32(CITY_SHOP_PANES) - 1.0);
    let pq = cu + glass_half - (column + 0.5) * pane_w;
    var bars = city_line(abs(pq) - 0.5 * pane_w, 0.07, pane_w, i.px);
    bars = max(bars, city_line(z - CITY_SHOP_HEAD + 0.6, 0.06, CITY_SHOP_HEAD, i.px));
    bars = max(bars, city_line(z - lo, 0.1, CITY_SHOP_HEAD, i.px));
    let key = city_pane_key(i, vec2<f32>(bay, 0.0), column);
    let broken = max(step(key, min(i.damage * 1.15, 0.9)), i.gutted);
    let room = city_room(i, vec2<f32>(cu, z - 0.5 * CITY_SHOP_HEAD), vec2<f32>(0.5 * cw, 0.5 * CITY_SHOP_HEAD), 9.0, key, false);
    // Shelves and stock: dark and bright blocks down the shop.
    var inner = room.rgb * (0.7 + 0.6 * step(0.5, fract(z * 1.6 + hue)));
    inner = mix(inner, inner * vec3<f32>(0.25, 0.22, 0.2), broken);
    let edge = min(CITY_SHOP_HEAD - z, 0.5 * pane_w - abs(pq));
    let shard = broken * step(edge, 0.08 + 0.4 * surf_noise3(vec3<f32>(i.local.xy * 5.0, z * 5.0 + key * 30.0)));
    let frame = max(bars, shard * 0.7) * opening;
    var albedo = mix(wall, sign, fascia);
    albedo = mix(albedo, vec3<f32>(0.1, 0.1, 0.11), frame);
    // A shutter pulled down over some of the closed shops.
    let shutter = step(0.82, hash11(shop * 31.0)) * opening;
    albedo = mix(albedo, vec3<f32>(0.4, 0.4, 0.4) * (0.8 + 0.2 * step(0.5, fract(z * 9.0))), shutter);
    o.albedo = albedo;
    o.glass = opening * (1.0 - frame) * (1.0 - shutter);
    o.reflect = 1.0 - broken;
    o.interior = inner;
    o.lamp = inner * vec3<f32>(1.0, 0.85, 0.6) * 2.0 * room.lit * i.night * (1.0 - broken);
    o.tint = vec3<f32>(0.92, 0.97, 1.0);
    o.tilt = (vec2<f32>(hash11(key * 7.0), hash11(key * 9.0)) - 0.5) * 0.02;
    o.f0 = 0.08;
    return o;
}

// A tower's lobby from the ground: full-height glass between stone piers.
fn city_lobby(i: CityIn) -> CityLook {
    var o: CityLook;
    let w = 2.0 * abs(i.face.z);
    let u = i.face.x + abs(i.face.z);
    let n = city_n_of(w, CITY_LOBBY_BAY);
    let cw = w / n;
    let bay = floor(u / cw);
    let cu = u - (bay + 0.5) * cw;
    let z = i.local.z;
    let base = city_stone_rgb(city_hash(i.inst, 2.0));
    let wall = city_scars(city_wall(CITY_FINISH_STONE, base, vec2<f32>(u, z), i.local, i.px, i.inst), i);
    let glass_half = 0.5 * cw - 0.45;
    let opening = city_box(cu, glass_half, i.px) * step(0.0, z) * step(z, CITY_LOBBY_HEAD);
    let pane_w = 2.0 * glass_half / f32(CITY_LOBBY_PANES);
    let column = clamp(floor((cu + glass_half) / pane_w), 0.0, f32(CITY_LOBBY_PANES) - 1.0);
    let pq = cu + glass_half - (column + 0.5) * pane_w;
    var bars = city_line(abs(pq) - 0.5 * pane_w, 0.06, pane_w, i.px);
    bars = max(bars, city_line(z - 3.2, 0.08, CITY_LOBBY_HEAD, i.px));
    let key = city_pane_key(i, vec2<f32>(bay, 0.0), column);
    let broken = max(step(key, min(i.damage * 1.15, 0.9)), i.gutted);
    let room = city_room(i, vec2<f32>(cu, z - 0.5 * CITY_LOBBY_HEAD), vec2<f32>(0.5 * cw, 0.5 * CITY_LOBBY_HEAD), 14.0, key, true);
    var inner = mix(room.rgb, room.rgb * 0.25, broken);
    let frame = bars * opening;
    o.albedo = mix(wall, vec3<f32>(0.1, 0.1, 0.11), frame);
    o.roughness = 0.6;
    o.glass = opening * (1.0 - frame);
    o.reflect = 1.0 - broken;
    o.interior = inner * 1.3;
    o.lamp = inner * vec3<f32>(0.95, 0.9, 0.8) * 2.5 * i.night * (1.0 - broken);
    o.tint = vec3<f32>(0.9, 0.95, 1.0);
    o.tilt = vec2<f32>(0.0);
    o.f0 = 0.1;
    return o;
}

// A glass roof: panes on a grid of glazing bars, the station's shed.
fn city_glass_roof(i: CityIn) -> CityLook {
    var o: CityLook;
    let st = i.face.xy + abs(i.face.zw);
    let cell = floor(st / CITY_ROOF_GLASS_PITCH);
    let f = st / CITY_ROOF_GLASS_PITCH - cell - 0.5;
    var bars = city_line(abs(f.x) - 0.5, 0.06 / CITY_ROOF_GLASS_PITCH, 1.0, i.px / CITY_ROOF_GLASS_PITCH);
    bars = max(bars, city_line(abs(f.y) - 0.5, 0.04 / CITY_ROOF_GLASS_PITCH, 1.0, i.px / CITY_ROOF_GLASS_PITCH));
    let key = city_pane_key(i, cell, 0.0);
    let broken = max(step(key, min(i.damage * 1.15, 0.9)), i.gutted);
    o.albedo = vec3<f32>(0.16, 0.17, 0.18);
    o.roughness = 0.5;
    o.metallic = 0.5;
    o.glass = (1.0 - bars) * (1.0 - 0.2 * broken);
    o.reflect = 1.0 - broken;
    // Grime on the glass: soot and pigeon-grey dust.
    let grime = 0.6 + 0.4 * surf_noise3(i.local * 0.3);
    // The shed's daylit inside through grimy glass: pale, hazy.
    o.interior = vec3<f32>(0.55, 0.53, 0.5) * grime;
    o.f0 = 0.08;
    o.lamp = vec3<f32>(0.0);
    o.tint = vec3<f32>(0.85, 0.9, 0.9) * grime;
    o.tilt = (vec2<f32>(hash11(key * 7.0), hash11(key * 9.0)) - 0.5) * 0.04;
    return o;
}

// Roofs, plain walls and the rest: no glass.
fn city_plain(i: CityIn) -> CityLook {
    var o: CityLook;
    o.roughness = 0.88;
    o.tint = vec3<f32>(1.0);
    let st = i.face.xy + abs(i.face.zw);
    let p = i.pattern;
    let broad = surf_fbm3(i.local + vec3<f32>(i.inst * 200.0), 6.0, i.px);
    if p == CITY_RENDER || p == CITY_BRICK || p == CITY_STONE || p == CITY_CONCRETE {
        let finish = city_finish_of(p, i.inst);
        o.albedo = city_wall(finish, city_finish_rgb(finish, i.inst), st, i.local, i.px, i.inst);
    } else if p == CITY_ROOF_TILE {
        let pick = city_hash(i.inst, 6.0);
        var c = vec3<f32>(0.36, 0.13, 0.07);
        if pick > 0.4 { c = vec3<f32>(0.14, 0.15, 0.17); }
        if pick > 0.7 { c = vec3<f32>(0.23, 0.12, 0.08); }
        if pick > 0.88 { c = vec3<f32>(0.27, 0.26, 0.25); }
        let row = floor(st.y / 0.32);
        let tile = floor(st.x / 0.3 + 0.5 * fract(row * 0.5));
        let vis = surf_resolved(0.3, i.px);
        c *= mix(1.0, 0.8 + 0.4 * hash21(vec2<f32>(tile, row) + i.inst * 9.0), vis);
        c *= 1.0 - 0.35 * (1.0 - smoothstep(0.0, 0.18, fract(st.y / 0.32))) * vis;
        // Moss and lichen in the lee, and soot.
        c = mix(c, vec3<f32>(0.1, 0.12, 0.06), 0.3 * smoothstep(0.55, 0.85, surf_noise3(i.local * 0.6)));
        o.albedo = c * (1.0 + 0.3 * broad);
        o.roughness = 0.75;
    } else if p == CITY_ROOF_FLAT {
        var c = vec3<f32>(0.2, 0.2, 0.2);
        let gravel = smoothstep(0.45, 0.6, surf_noise3(i.local * 0.15 + i.inst * 30.0));
        c = mix(c, vec3<f32>(0.36, 0.35, 0.33), gravel);
        let seam = fract(st.x / 1.0);
        c *= 1.0 - 0.15 * surf_resolved(0.2, i.px) * (1.0 - smoothstep(0.0, 0.04, min(seam, 1.0 - seam)));
        o.albedo = c * (1.0 + 0.4 * broad);
        o.roughness = 0.92;
    } else if p == CITY_ROOF_METAL || p == CITY_STEEL {
        let pick = city_hash(i.inst, select(8.0, 9.0, p == CITY_STEEL));
        var c = city_sheet_rgb(pick);
        if p == CITY_STEEL {
            c = mix(vec3<f32>(0.6, 0.6, 0.57), c, step(0.45, pick));
        } else {
            let rib = fract(st.x / 0.5);
            c *= 1.0 - 0.2 * surf_resolved(0.4, i.px) * smoothstep(0.42, 0.5, abs(rib - 0.5));
        }
        let rust = smoothstep(0.62, 0.9, surf_noise3(vec3<f32>(i.local.xy * 1.3, i.local.z * 0.3)));
        let runs = smoothstep(0.55, 0.85, surf_noise3(vec3<f32>(st.x * 2.0, st.y * 0.15, i.inst * 7.0)));
        c = mix(c, vec3<f32>(0.3, 0.14, 0.06), max(rust * 0.3, runs * 0.2) * surf_resolved(0.6, i.px));
        o.albedo = c * (1.0 + 0.25 * broad);
        o.roughness = 0.62;
        o.metallic = 0.15;
    } else if p == CITY_SHADOW {
        o.albedo = vec3<f32>(0.02, 0.02, 0.019);
        o.roughness = 0.95;
    } else if p == CITY_PAVING {
        var c = vec3<f32>(0.24, 0.24, 0.23);
        let slab = fract(st / 1.8);
        let joint = 1.0 - smoothstep(0.0, 0.02, min(min(slab.x, 1.0 - slab.x), min(slab.y, 1.0 - slab.y)));
        c *= 1.0 - 0.25 * joint * surf_resolved(0.3, i.px);
        o.albedo = c * (1.0 + 0.4 * broad);
        o.roughness = 0.9;
    } else if p == CITY_FORT || p == CITY_BLAST_DOOR {
        o = city_fort(i, st, broad);
    } else if p == CITY_RUBBLE {
        // Broken concrete and brick under a coat of dust: lumps a few tens of cm,
        // brick red in some, dark voids between, all dulled toward the dust's tan.
        let q = i.local * 2.6 + vec3<f32>(i.inst * 20.0);
        let lump = surf_noise3(q);
        let kind = surf_noise3(q * 0.37 + vec3<f32>(5.0, 1.0, 3.0));
        var c = mix(vec3<f32>(0.36, 0.35, 0.33), vec3<f32>(0.3, 0.15, 0.1), smoothstep(0.55, 0.7, kind));
        c *= 0.75 + 0.5 * lump;
        c *= 1.0 - 0.6 * smoothstep(0.25, 0.1, lump) * surf_resolved(0.3, i.px);
        c = mix(c, vec3<f32>(0.4, 0.37, 0.32), 0.35);
        c = mix(c, vec3<f32>(0.035, 0.03, 0.028), 0.35 * smoothstep(0.55, 0.8, surf_noise3(i.local * 0.2 + 9.0)));
        o.albedo = c * (1.0 + 0.3 * broad);
        o.roughness = 0.95;
    } else if p == CITY_TIMBER {
        let board = floor(st.x / 0.18);
        var c = select(vec3<f32>(0.3, 0.24, 0.18), vec3<f32>(0.3, 0.07, 0.05), city_hash(i.inst, 10.0) < 0.4);
        c *= 0.8 + 0.4 * hash11(board + i.inst * 31.0) * surf_resolved(0.18, i.px);
        let gap = fract(st.x / 0.18);
        c *= 1.0 - 0.5 * surf_resolved(0.18, i.px) * (1.0 - smoothstep(0.0, 0.08, gap));
        o.albedo = c * (1.0 + 0.4 * surf_fbm3(vec3<f32>(i.local.xy, i.local.z * 0.1), 1.5, i.px));
        o.roughness = 0.9;
    } else if p == CITY_LED {
        // A strip of LEDs behind a diffuser: cool white, or the building's tint; dim
        // by day, bright at night; dead in stretches once the building is hurt.
        let pick = city_hash(i.inst, 12.0);
        var tint = vec3<f32>(0.85, 0.95, 1.0);
        if pick > 0.5 { tint = vec3<f32>(0.4, 0.85, 1.0); }
        if pick > 0.8 { tint = vec3<f32>(1.0, 0.85, 0.6); }
        let dead = step(1.0 - i.damage * 1.2, hash11(floor(st.x / 3.0) * 7.1 + i.inst * 31.0));
        let on = (1.0 - dead) * (1.0 - i.gutted);
        o.albedo = vec3<f32>(0.6, 0.62, 0.64) * (0.3 + 0.7 * on);
        o.roughness = 0.3;
        o.glow = tint * on * mix(0.5, 2.6, i.night);
    } else if p == CITY_SOLAR {
        // Cells 16 cm square in panels 1 m by 1.7 m, a silver frame round each panel.
        let cell = city_lines(st.x, 0.01, 0.16, fwidth(st.x)) + city_lines(st.y, 0.01, 0.16, fwidth(st.y));
        let frame = max(city_lines(st.x, 0.04, 1.0, fwidth(st.x)), city_lines(st.y, 0.04, 1.7, fwidth(st.y)));
        o.albedo = mix(mix(vec3<f32>(0.025, 0.035, 0.07), vec3<f32>(0.12, 0.13, 0.15), min(cell, 1.0) * 0.5), vec3<f32>(0.55, 0.56, 0.58), frame);
        o.roughness = 0.18;
        o.metallic = 0.4;
    } else if p == CITY_COPPER {
        var c = vec3<f32>(0.22, 0.42, 0.36);
        c = mix(c, vec3<f32>(0.12, 0.2, 0.17), smoothstep(0.5, 0.8, surf_noise3(vec3<f32>(i.local.xy * 0.8, i.local.z * 0.1))));
        o.albedo = c * (1.0 + 0.3 * broad);
        o.roughness = 0.65;
    } else {
        o.albedo = vec3<f32>(0.42, 0.41, 0.39);
    }
    if p != CITY_SHADOW {
        o.albedo = city_scars(o.albedo, i);
    }
    if p == CITY_RUBBLE {
        // A heap left by a building that burned is charred through.
        o.albedo *= 1.0 - 0.55 * max(i.burning, i.gutted);
    }
    return o;
}

// The wall's military concrete: cast in 1.5 m lifts with a tie-hole grid, panels of
// slightly different pours, scorch and chips; a stencilled number and a dark band on
// its outer face; blast doors in heavy ribbed steel with hazard edges.
fn city_fort(i: CityIn, st: vec2<f32>, broad: f32) -> CityLook {
    var o: CityLook;
    o.tint = vec3<f32>(1.0);
    if i.pattern == CITY_BLAST_DOOR {
        var c = vec3<f32>(0.2, 0.21, 0.21) * (1.0 + 0.4 * broad);
        let rib = fract(st.y / 1.2);
        c *= 1.0 - 0.35 * surf_resolved(0.3, i.px) * (1.0 - smoothstep(0.0, 0.08, min(rib, 1.0 - rib)));
        let edge = abs(i.face.x) - (abs(i.face.z) - 0.9);
        let stripe = step(0.5, fract((st.x + st.y) / 0.7));
        let hazard = step(0.0, edge) * surf_resolved(0.35, i.px);
        c = mix(c, mix(vec3<f32>(0.03), vec3<f32>(0.62, 0.42, 0.04), stripe), hazard);
        o.albedo = c;
        o.roughness = 0.55;
        o.metallic = 0.5;
        return o;
    }
    let panel = floor(vec2<f32>(st.x / 8.0, st.y / 4.5));
    var c = vec3<f32>(0.37, 0.37, 0.36) * (0.93 + 0.14 * hash21(panel + i.inst * 3.0));
    c *= 1.0 + 0.4 * broad;
    let vis = surf_resolved(0.25, i.px);
    let lift = fract(i.local.z / 1.5);
    c *= 1.0 - 0.18 * vis * (1.0 - smoothstep(0.0, 0.03, lift));
    let joint = fract(st.x / 8.0);
    c *= 1.0 - 0.3 * vis * (1.0 - smoothstep(0.0, 0.006, min(joint, 1.0 - joint)));
    let tie = fract(vec2<f32>(st.x / 1.0, i.local.z / 1.5 + 0.5)) - 0.5;
    c *= 1.0 - 0.45 * vis * step(length(tie * vec2<f32>(1.0, 1.5)), 0.035);
    let steep = 1.0 - abs(i.normal.z);
    // Scorch from the shelling, streaks down from the parapet.
    let scorch = smoothstep(0.6, 0.85, surf_noise3(i.local * 0.08 + vec3<f32>(i.inst * 70.0)));
    c = mix(c, vec3<f32>(0.04, 0.035, 0.03), scorch * 0.7 * steep);
    let streak = smoothstep(0.5, 0.85, surf_noise3(vec3<f32>(st.x * 0.6, i.local.z * 0.03, i.inst * 9.0)));
    c *= 1.0 - 0.25 * streak * steep;
    // The outer face (-y in the model) carries a dark band and a stencilled number.
    if i.normal.y < -0.5 && i.local.z > 9.0 && i.local.z < 13.0 {
        c = mix(c, vec3<f32>(0.09, 0.095, 0.1), 0.85 * vis);
        // Two digits a segment, 3 m tall, in the middle of each 32 m of wall.
        let section = floor(st.x / 32.0);
        let at = vec2<f32>(st.x - (section + 0.5) * 32.0, i.local.z - 11.0) / 1.5;
        let fw = max(i.px, 0.01) / 1.5;
        let tens = u32(hash11(i.inst * 13.0 + section) * 9.99);
        let ones = u32(hash11(i.inst * 17.0 + section * 3.0) * 9.99);
        let mark = max(surf_digit(at + vec2<f32>(0.6, 0.0), tens, fw), surf_digit(at - vec2<f32>(0.6, 0.0), ones, fw));
        c = mix(c, vec3<f32>(0.7, 0.7, 0.66), mark * vis);
    }
    o.albedo = c;
    o.roughness = 0.9;
    return o;
}

fn city_surface(i: CityIn) -> CityLook {
    let p = i.pattern;
    if p >= CITY_HOUSE + CITY_BLANK {
        // A facade's wall with no windows in it.
        var o: CityLook;
        let finish = city_finish_of(p - CITY_BLANK, i.inst);
        var base = city_finish_rgb(finish, i.inst);
        if p - CITY_BLANK == CITY_GUTTED {
            base = mix(base, vec3<f32>(0.1, 0.09, 0.085), 0.55);
        }
        o.albedo = city_scars(city_wall(finish, base, i.face.xy + abs(i.face.zw), i.local, i.px, i.inst), i);
        o.roughness = 0.88;
        o.tint = vec3<f32>(1.0);
        if p - CITY_BLANK == CITY_CURTAIN {
            o.albedo = vec3<f32>(0.05, 0.06, 0.07);
            o.roughness = 0.3;
            o.metallic = 0.6;
        }
        return o;
    }
    if p == CITY_SHOP {
        return city_shop(i);
    }
    if p == CITY_LOBBY {
        return city_lobby(i);
    }
    if p == CITY_ROOF_GLASS {
        return city_glass_roof(i);
    }
    if p == CITY_DECKS {
        return city_decks(i);
    }
    if p == CITY_HOUSE || p == CITY_TERRACE || p == CITY_FLATS || p == CITY_OFFICE || p == CITY_RIBBON
        || p == CITY_CURTAIN || p == CITY_SHED || p == CITY_ARCHED || p == CITY_GUTTED {
        return city_facade(i);
    }
    return city_plain(i);
}

// A car park's decks: a concrete spandrel along each floor, the dark deck behind it.
fn city_decks(i: CityIn) -> CityLook {
    var o: CityLook;
    o.tint = vec3<f32>(1.0);
    let h = 2.0 * abs(i.face.w);
    let v = i.face.y + abs(i.face.w);
    let m = city_n_of(h, CITY_DECKS_STOREY);
    let ch = h / m;
    let cv = fract(v / ch) * ch;
    let st = i.face.xy + abs(i.face.zw);
    let base = city_concrete_rgb(city_hash(i.inst, 2.0));
    let wall = city_wall(CITY_FINISH_CONCRETE, base, st, i.local, i.px, i.inst);
    let open = step(1.1, cv) * step(cv, ch - 0.35);
    let column = city_box(fract(st.x / 7.5 + 0.5) - 0.5, 0.05, i.px / 7.5);
    let gap = open * (1.0 - column);
    let resolve = surf_resolved(0.8, i.px);
    o.albedo = mix(wall, vec3<f32>(0.03, 0.03, 0.03), gap * mix(0.6, 1.0, resolve));
    o.roughness = 0.9;
    return o;
}
