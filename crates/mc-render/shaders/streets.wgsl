// A city's made ground on the terrain (terrain.wgsl), from the map's streets
// layer (bindings.wgsl `street_cell`, habitat.wgsl `Habitat::street`):
// asphalt and its markings, kerbs, pavements and squares, works yards, a rail
// yard's ballast and track, a spaceport's apron, rubble, shelled earth, and
// the outskirts' fields. build.rs inserts this file into shaders containing
// `//!use streets` (needs habitat). Distances are metres; `px` is the metres
// a pixel covers, and every line fades to its average as it nears a pixel.

struct StreetShade {
    // The ground's colour with the city laid on it.
    rgb: vec3<f32>,
    rough: f32,
    metal: f32,
    // Slope the made ground adds (world xy): kerbs, ruts, rails, rubble.
    grad: vec2<f32>,
    // How much of the terrain's own relief to flatten away, 0-1.
    flat: f32,
}

// A stripe `width` wide centred on x = 0, antialiased: its cover of a pixel.
fn street_stripe(x: f32, width: f32, px: f32) -> f32 {
    let aa = max(px, 1e-3);
    let sharp = clamp((0.5 * width - abs(x)) / aa + 0.5, 0.0, 1.0);
    return mix(sharp, min(width / aa, 1.0), smoothstep(width, 3.0 * width, px));
}

// Whether `along` falls on a dash of a `period`-long pattern `duty` of it
// painted, antialiased like a stripe.
fn street_dash(along: f32, period: f32, duty: f32, px: f32) -> f32 {
    let f = fract(along / period) * period;
    let on = street_stripe(f - 0.5 * duty * period, duty * period, px);
    return mix(on, duty, smoothstep(0.3 * period, period, px));
}

// Joints between slabs `size` across, laid square to the map: x their cover,
// y and z the slab's own random numbers.
fn street_slabs(xy: vec2<f32>, size: f32, joint: f32, px: f32) -> vec3<f32> {
    let cell = floor(xy / size);
    let f = xy - (cell + 0.5) * size;
    let edge = 0.5 * size - max(abs(f.x), abs(f.y));
    let line = street_stripe(edge, 2.0 * joint, px);
    return vec3<f32>(line, hash21(cell * 1.31 + 7.7), hash21(cell * 0.73 - 3.1));
}

// The outskirts' fields: the same Voronoi of jittered points the baker
// surveys them on (mc-map `siege/fields.rs`, `field_hash` there).
fn field_hash(x: i32, y: i32) -> u32 {
    var h = (u32(x) * 0x8DA6B343u) ^ (u32(y) * 0xD8163841u);
    h ^= h >> 15u;
    h *= 0x2C1B3C6Du;
    h ^= h >> 12u;
    h *= 0x297A2D39u;
    return h ^ (h >> 15u);
}

// The field `xy` lies in: its hash, and how far `xy` is from its border, metres.
struct FieldCell {
    h: u32,
    edge: f32,
}

fn field_cell(xy: vec2<f32>) -> FieldCell {
    let c = vec2<i32>(floor(xy / STREET_FIELD_CELL));
    var best = 1e20;
    var second = 1e20;
    var best_h = 0u;
    var a = vec2<f32>(0.0);
    var b = vec2<f32>(0.0);
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let k = c + vec2<i32>(x, y);
            let h = field_hash(k.x, k.y);
            let uv = vec2<f32>(f32(h & 0xFFFFu), f32(h >> 16u)) / 65536.0;
            let s = (vec2<f32>(k) + 0.15 + 0.7 * uv) * STREET_FIELD_CELL;
            let d = dot(xy - s, xy - s);
            if d < best {
                second = best;
                b = a;
                best = d;
                a = s;
                best_h = h;
            } else if d < second {
                second = d;
                b = s;
            }
        }
    }
    let n = normalize(b - a);
    var f: FieldCell;
    f.h = best_h;
    f.edge = abs(dot((a + b) * 0.5 - xy, n));
    return f;
}

// A field's crop, by its hash: ploughed furrows, young rows, ripe grain or
// stubble, the rows running the field's own way.
fn field_crop(xy: vec2<f32>, ground: vec3<f32>, battered: f32, px: f32) -> StreetShade {
    var s: StreetShade;
    let f = field_cell(xy);
    let h = f.h;
    let a = f32((h >> 8u) & 0xFFu) / 256.0 * 3.14159265;
    let along = vec2<f32>(cos(a), sin(a));
    let across = dot(xy, vec2<f32>(-along.y, along.x));
    let crop = (h >> 20u) % 4u;
    // A metre or so of grass and weeds left at each border.
    let margin = smoothstep(1.5, 3.0, f.edge);
    let rows = 0.5 + 0.5 * sin(across * 6.2831853 / 0.75);
    let fine = 1.0 - smoothstep(0.25, 0.7, px);
    let soil = ground * vec3<f32>(0.85, 0.8, 0.72);
    var rgb = soil;
    var g = vec2<f32>(0.0);
    var rough = 0.95;
    if crop == 0u {
        // Ploughed: ridges and furrows, the furrows' bottoms damper.
        rgb = soil * mix(0.7, 1.12, mix(0.5, rows, fine));
        g = vec2<f32>(-along.y, along.x) * cos(across * 6.2831853 / 0.75) * 0.35 * fine;
    } else if crop == 1u {
        // Young crop in rows on the soil.
        let leaf = vec3<f32>(0.07, 0.13, 0.035) * (0.85 + 0.3 * grad_noise2(xy, 6.0));
        rgb = mix(soil, leaf, mix(0.55, smoothstep(0.35, 0.75, rows), fine));
    } else if crop == 2u {
        // Ripe grain, rippled in broad streaks.
        let ripe = vec3<f32>(0.33, 0.27, 0.12) * (0.82 + 0.3 * grad_noise2(xy + 31.0, 9.0) + 0.1 * rows * fine);
        rgb = ripe;
        rough = 0.85;
    } else {
        // Stubble after the harvest, the harvester's swathes along the rows.
        let straw = vec3<f32>(0.26, 0.22, 0.13);
        let swathe = 0.5 + 0.5 * sin(across * 6.2831853 / 6.0);
        rgb = mix(soil, straw, 0.6 + 0.25 * swathe) * (0.9 + 0.2 * grad_noise2(xy, 3.0));
    }
    // Churned by tracks and scorched where the war came through.
    let churn = battered * smoothstep(0.45, 0.75, grad_noise2(xy + 77.0, 14.0));
    rgb = mix(rgb, soil * 0.55, churn * 0.8);
    s.rgb = mix(ground, rgb, margin);
    s.rough = rough;
    s.metal = 0.0;
    s.grad = g * margin;
    s.flat = 0.7 * margin;
    return s;
}

// Asphalt, its patches and wear, and what the war has done to it.
fn asphalt(xy: vec2<f32>, battered: f32, px: f32) -> vec3<f32> {
    let fine = 1.0 - smoothstep(0.08, 0.3, px);
    let grit = grad_noise2(xy, 0.31) * fine + 0.5 * (1.0 - fine);
    let mottle = grad_noise2(xy + 13.0, 2.4) * 0.6 + grad_noise2(xy - 41.0, 11.0) * 0.4;
    var rgb = vec3<f32>(0.052, 0.054, 0.058) * (0.75 + 0.35 * mottle + 0.25 * grit);
    // Patched repairs: darker, fresher rectangles of tarmac.
    let repair = step(0.78, value_noise2(floor(xy / 4.0) * 4.0, 9.0));
    rgb = mix(rgb, vec3<f32>(0.035, 0.036, 0.04), repair * 0.8);
    return rgb;
}

// Cracks, potholes, chips of masonry and dust over any hard ground, by how
// battered it is: x the crack and pothole darkening, y the debris's cover,
// z the dust's.
fn street_damage(xy: vec2<f32>, battered: f32, px: f32) -> vec3<f32> {
    if battered < 0.02 {
        return vec3<f32>(0.0);
    }
    let cells = crater_cells(xy / 2.7);
    let crack = crater_crack(cells.x, 0.035, px / 2.7) * smoothstep(0.15, 0.6, battered + (cells.y - 0.5) * 0.4);
    let pothole = smoothstep(0.82, 0.9, value_noise2(xy, 3.1) + battered * 0.25) * battered;
    // Chips and chunks on a fine lattice, more of them the worse it is.
    let cell = floor(xy / 0.7);
    let r = hash21(cell + 0.37);
    let chip = select(0.0, 1.0, r < battered * 0.18) * (1.0 - smoothstep(0.1, 0.35, px));
    let dust = battered * smoothstep(0.3, 0.75, grad_noise2(xy - 19.0, 7.0));
    return vec3<f32>(max(crack, pothole), chip, dust);
}

// Lane lines, painted (white) or (yellow), worn where the road is battered.
fn paint(xy: vec2<f32>, cover: f32, battered: f32) -> f32 {
    let wear = smoothstep(0.2, 0.75, grad_noise2(xy, 0.9) + 0.2 - battered * 0.6);
    return cover * mix(1.0, wear, 0.35 + 0.6 * battered);
}

// The road's way at `xy`: a unit vector along it, the same either way the
// road was laid (so dashes keep their phase across the centreline).
fn road_tangent(xy: vec2<f32>) -> vec2<f32> {
    let e = 1.5;
    let g = vec2<f32>(street_offset(xy + vec2<f32>(e, 0.0)) - street_offset(xy - vec2<f32>(e, 0.0)),
        street_offset(xy + vec2<f32>(0.0, e)) - street_offset(xy - vec2<f32>(0.0, e)));
    var t = normalize(vec2<f32>(-g.y, g.x) + vec2<f32>(1e-5, 0.0));
    if t.x < -1e-3 || (abs(t.x) <= 1e-3 && t.y < 0.0) {
        t = -t;
    }
    return t;
}

// What a road's running surface looks like at offset `o` from its centreline.
fn road_surface(xy: vec2<f32>, c: StreetCell, ground: vec3<f32>, px: f32) -> StreetShade {
    var s: StreetShade;
    let o = c.offset;
    let d = abs(o);
    let near = px < 1.2;
    let t = select(vec2<f32>(1.0, 0.0), road_tangent(xy), near);
    let along = dot(xy, t);
    let white = vec3<f32>(0.62, 0.62, 0.6);
    let yellow = vec3<f32>(0.6, 0.43, 0.07);
    s.rough = 0.9;
    s.metal = 0.0;
    s.grad = vec2<f32>(0.0);
    s.flat = 1.0;
    let markings = !c.junction && near;
    if c.road == STREET_ROAD_LANE {
        // A farm track: two wheel ruts, a grass crown, mud.
        let mud = ground * vec3<f32>(0.7, 0.62, 0.52);
        let rut = street_stripe(d - 1.15, 0.6, px);
        let crown = 1.0 - smoothstep(0.35, 0.7, d);
        s.rgb = mix(mix(ground * 0.85, mud, 0.6), mud * 0.7, rut);
        s.rgb = mix(s.rgb, ground, crown * 0.8);
        s.grad = -sign(o) * normalize(vec2<f32>(-t.y, t.x)) * rut * 0.2;
        s.flat = 0.6;
        return s;
    }
    if c.road == STREET_ROAD_RAIL {
        // Ballast, two tracks of sleepers and rails.
        let stone = vec3<f32>(0.15, 0.14, 0.125) * (0.7 + 0.5 * grad_noise2(xy, 0.4 + px));
        s.rgb = stone;
        s.rough = 0.95;
        for (var k = -1; k <= 1; k += 2) {
            let tr = o - f32(k) * 2.25;
            let sleeper = street_stripe(tr, 2.6, px) * street_dash(along, 0.65, 0.36, px);
            s.rgb = mix(s.rgb, vec3<f32>(0.075, 0.068, 0.06), sleeper);
            let rail = max(street_stripe(abs(tr) - 0.72, 0.07, px), 0.0);
            s.rgb = mix(s.rgb, vec3<f32>(0.32, 0.31, 0.3), rail);
            s.metal = max(s.metal, rail);
            s.rough = mix(s.rough, 0.35, rail);
        }
        return s;
    }
    s.rgb = asphalt(xy, c.battered, px);
    var cover = 0.0;
    if c.road == STREET_ROAD_STREET {
        if c.half >= 8.0 && markings {
            cover = street_stripe(o, 0.12, px) * street_dash(along, 9.0, 0.35, px);
        }
    } else if c.road == STREET_ROAD_AVENUE {
        // The planted median and its kerbs.
        let median = 1.0 - smoothstep(2.9, 3.1, d);
        let soil = ground * vec3<f32>(0.8, 0.85, 0.7);
        s.rgb = mix(s.rgb, soil, median);
        let kerb = street_stripe(d - 3.12, 0.25, px);
        s.rgb = mix(s.rgb, vec3<f32>(0.3, 0.29, 0.27), kerb);
        s.grad += sign(o) * normalize(vec2<f32>(-t.y, t.x)) * kerb * 0.6;
        s.flat = 1.0 - median * 0.7;
        if markings {
            cover = street_stripe(d - 3.55, 0.15, px);
            let lanes = floor((c.half - 3.4 - 4.5) / 3.4);
            for (var k = 1.0; k <= 3.0; k += 1.0) {
                if k <= lanes {
                    cover = max(cover, street_stripe(d - 3.4 - 3.4 * k, 0.13, px) * street_dash(along, 9.0, 0.35, px));
                }
            }
            cover = max(cover, street_stripe(d - (c.half - 4.5), 0.13, px));
        }
    } else if c.road == STREET_ROAD_HIGHWAY {
        if markings {
            if c.half > 10.0 {
                // A divided highway's double yellow line and its lanes.
                let yl = max(street_stripe(d - 0.18, 0.12, px), 0.0);
                s.rgb = mix(s.rgb, yellow, paint(xy, yl, c.battered));
                cover = street_stripe(d - c.half * 0.5, 0.15, px) * street_dash(along, 12.0, 0.33, px);
            } else {
                cover = street_stripe(o, 0.12, px) * street_dash(along, 9.0, 0.4, px);
            }
            cover = max(cover, street_stripe(d - (c.half - 0.5), 0.15, px));
        }
        // The gravel shoulder past the asphalt.
        let shoulder = smoothstep(c.half - 0.1, c.half + 0.2, d);
        let gravel = ground * vec3<f32>(0.8, 0.78, 0.72) * (0.8 + 0.4 * grad_noise2(xy, 0.5 + px));
        s.rgb = mix(s.rgb, gravel, shoulder);
    }
    s.rgb = mix(s.rgb, white, paint(xy, cover, c.battered));
    if c.road == STREET_ROAD_STREET || c.road == STREET_ROAD_AVENUE {
        // The kerb: a band of stone standing a little proud of the gutter.
        let kerb = street_stripe(d - c.half - 0.15, 0.3, px);
        s.rgb = mix(s.rgb, vec3<f32>(0.3, 0.29, 0.27), kerb);
        s.grad -= sign(o) * normalize(vec2<f32>(-t.y, t.x)) * street_stripe(d - c.half, 0.08, px) * 1.5;
        // A darker gutter along it.
        s.rgb *= 1.0 - 0.3 * street_stripe(d - c.half + 0.35, 0.5, px);
    }
    return s;
}

// Made ground off the roads, by its kind.
fn street_ground(xy: vec2<f32>, c: StreetCell, ground: vec3<f32>, px: f32) -> StreetShade {
    var s: StreetShade;
    s.rgb = ground;
    s.rough = 0.92;
    s.metal = 0.0;
    s.grad = vec2<f32>(0.0);
    s.flat = 1.0;
    let fine = 1.0 - smoothstep(0.15, 0.6, px);
    switch c.ground {
        case STREET_GROUND_PAVING: {
            // Slabs: small along the streets, larger in the squares.
            let by_road = c.road != STREET_ROAD_NONE && abs(c.offset) < c.half + 7.0;
            let size = select(2.4, 1.2, by_road);
            let sl = street_slabs(xy, size, 0.02, px);
            let tone = 0.88 + 0.2 * sl.y;
            s.rgb = vec3<f32>(0.27, 0.262, 0.245) * tone * (0.85 + 0.3 * grad_noise2(xy + 5.0, 3.5));
            s.rgb *= 1.0 - 0.45 * sl.x;
            // Some slabs cracked or lifted.
            let broken = step(1.0 - 0.3 * c.battered, sl.z);
            s.rgb = mix(s.rgb, s.rgb * 0.7, broken);
            s.rough = 0.85;
        }
        case STREET_GROUND_YARD: {
            let sl = street_slabs(xy, 6.0, 0.04, px);
            let stain = smoothstep(0.62, 0.8, grad_noise2(xy + 101.0, 5.0));
            s.rgb = vec3<f32>(0.205, 0.198, 0.188) * (0.85 + 0.25 * sl.y) * (0.85 + 0.3 * grad_noise2(xy, 1.7 + px));
            s.rgb = mix(s.rgb, vec3<f32>(0.06, 0.058, 0.055), stain * 0.6);
            s.rgb *= 1.0 - 0.5 * sl.x;
        }
        case STREET_GROUND_APRON: {
            let sl = street_slabs(xy, 7.5, 0.05, px);
            s.rgb = vec3<f32>(0.33, 0.33, 0.32) * (0.9 + 0.15 * sl.y) * (0.9 + 0.2 * grad_noise2(xy, 2.2 + px));
            s.rgb *= 1.0 - 0.55 * sl.x;
            // Faded yellow taxi lines every 60 m, and black tyre marks.
            let taxi = street_stripe(fract(xy.y / 60.0 + 0.5) * 60.0 - 30.0, 0.3, px);
            s.rgb = mix(s.rgb, vec3<f32>(0.5, 0.38, 0.08), taxi * 0.8);
            let skid = smoothstep(0.7, 0.85, grad_noise2(vec2<f32>(xy.x * 0.15, xy.y * 2.0), 3.0));
            s.rgb *= 1.0 - 0.5 * skid;
            s.rough = 0.8;
        }
        case STREET_GROUND_BALLAST: {
            // A rail yard: ballast, and tracks laid east-west 4.6 m apart.
            s.rgb = vec3<f32>(0.15, 0.14, 0.125) * (0.7 + 0.5 * grad_noise2(xy, 0.4 + px));
            let tr = (fract(xy.y / 4.6 + 0.5) - 0.5) * 4.6;
            let sleeper = street_stripe(tr, 2.6, px) * street_dash(xy.x, 0.65, 0.36, px);
            s.rgb = mix(s.rgb, vec3<f32>(0.075, 0.068, 0.06), sleeper);
            let rail = street_stripe(abs(tr) - 0.72, 0.07, px);
            s.rgb = mix(s.rgb, vec3<f32>(0.32, 0.31, 0.3), rail);
            s.metal = rail;
            s.rough = mix(0.95, 0.35, rail);
        }
        case STREET_GROUND_FIELD: {
            s = field_crop(xy, ground, c.battered, px);
        }
        case STREET_GROUND_RUBBLE: {
            // Broken concrete and brick, heaped, over pale dust.
            let big = crater_cells(xy / 1.7);
            let small = crater_cells(xy / 0.55 + 13.0);
            let dust = vec3<f32>(0.21, 0.195, 0.175) * (0.85 + 0.3 * grad_noise2(xy, 4.0));
            let brick = vec3<f32>(0.24, 0.115, 0.075);
            let concrete = vec3<f32>(0.25, 0.245, 0.235);
            let chunk = mix(concrete, brick, step(0.62, big.y)) * (0.75 + 0.45 * small.y);
            let lump = smoothstep(0.05, 0.3, big.x);
            s.rgb = mix(dust, chunk, lump * mix(0.6, 1.0, fine));
            s.rgb *= 1.0 - 0.5 * crater_crack(small.x, 0.08, px / 0.55);
            s.grad = vec2<f32>(grad_noise2(xy, 1.1) - 0.5, grad_noise2(xy + 7.0, 1.1) - 0.5) * 0.8 * fine;
            s.flat = 0.4;
        }
        case STREET_GROUND_EARTH: {
            // Trodden, shelled earth: the dirt scan with soot and churned tracks.
            let soot = smoothstep(0.6, 0.8, grad_noise2(xy + 211.0, 11.0)) * (0.4 + 0.6 * c.battered);
            s.rgb = mix(ground * vec3<f32>(0.9, 0.84, 0.76), vec3<f32>(0.03, 0.028, 0.026), soot * 0.7);
            s.flat = 0.2;
        }
        default: {}
    }
    return s;
}

// The city laid on the terrain's own `ground` colour at `xy`.
fn street_shade(xy: vec2<f32>, c: StreetCell, ground: vec3<f32>, px: f32) -> StreetShade {
    var s = street_ground(xy, c, ground, px);
    if c.road != STREET_ROAD_NONE {
        let d = abs(c.offset);
        let shoulder = select(0.0, 1.8, c.road == STREET_ROAD_HIGHWAY);
        let on = 1.0 - smoothstep(c.half + shoulder - 0.05, c.half + shoulder + 0.35, d);
        if on > 0.0 {
            let r = road_surface(xy, c, ground, px);
            s.rgb = mix(s.rgb, r.rgb, on);
            s.rough = mix(s.rough, r.rough, on);
            s.metal = mix(s.metal, r.metal, on);
            s.grad = mix(s.grad, r.grad, on);
            s.flat = mix(s.flat, r.flat, on);
        }
    }
    // The war over all of it: cracks and holes, chips of masonry, dust.
    let hard = c.ground != STREET_GROUND_FIELD && c.ground != STREET_GROUND_EARTH;
    let dmg = street_damage(xy, c.battered, px);
    if hard || c.road != STREET_ROAD_NONE {
        s.rgb *= 1.0 - 0.6 * dmg.x;
        s.rgb = mix(s.rgb, vec3<f32>(0.24, 0.23, 0.21), dmg.y * 0.8);
        s.rgb = mix(s.rgb, vec3<f32>(0.2, 0.185, 0.165), dmg.z * 0.35);
    }
    return s;
}
