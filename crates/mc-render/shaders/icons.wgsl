//!use bindings
// Strategic icons, selection rings and health bars.
//
// Icons: units whose model would be smaller than a few pixels are drawn as a
// constant-size symbol instead, so the whole war stays readable at full
// zoom-out. They come from the cull pass's last draw slot.

//!rust crate::renderer::Mark
struct Mark {
    // Visible-list style entity index (DYNAMIC_BIT set for units).
    unit_index: u32,
    // bit 0: hovered (else selected). bit 1: enemy.
    kind: u32,
    // Construction fill, zero to one. Negative: no construction bar.
    work: f32,
    // Shield fill, zero to one. Negative: no shield line.
    shield: f32,
}

@group(1) @binding(0) var<storage, read> marks: array<Mark>;

struct IconOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) icon: u32,
    @location(2) @interpolate(flat) owner_flags: u32,
}

fn entity_center(e: Entity) -> vec3<f32> {
    return mix(e.prev_pos, e.pos, globals.sun.w);
}

@vertex
fn vs_icon(@location(0) corner: vec2<f32>, @builtin(instance_index) instance: u32) -> IconOut {
    var out: IconOut;
    let index = visible[instance];
    if index == NOT_VISIBLE {
        // A row of the icon slot with no icon (cull.wgsl): a zero-area quad.
        out.clip = vec4<f32>(0.0, 0.0, 0.0, 1.0);
        out.uv = corner;
        out.icon = 0u;
        out.owner_flags = 0u;
        return out;
    }
    let e = load_entity(index);
    let model = models[e.blueprint];
    var shape = model.icon & 0xFFu;
    var size_px = 17.0;
    if (e.owner_flags & STATE_UNIDENTIFIED) != 0u {
        // Unknown radar contact: a small diamond, not the real class.
        shape = 31u;
        size_px = 14.0;
    } else if shape == 0u {
        size_px = 26.0;
    } else if (model.icon & 0x10000u) == 0u || (model.icon & 0x40000u) != 0u {
        // Structures, and aircraft: their role outlines need the extra pixels.
        size_px = 20.0;
    }
    if shape == 13u {
        size_px = 8.0;
    }
    if shape == ICON_TITAN {
        // A tier-5 titan: bigger than anything else, framed (`fs_icon`).
        size_px = ICON_TITAN_PX;
    }
    if shape == ICON_SALVAGE_DRONE {
        size_px = 14.0;
    }
    let center = globals.view_proj * vec4<f32>(entity_center(e) + vec3<f32>(0.0, 0.0, model.height * 0.5), 1.0);
    // Paused work: the quad reaches out to the right to carry a pause mark beside the symbol.
    let paused = (e.status[0] & UNIT_PAUSED) != 0u
        && (e.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST | STATE_UNIDENTIFIED)) == 0u;
    var c = corner;
    if paused {
        c.x = corner.x * PAUSE_REACH + (PAUSE_REACH - 1.0);
    }
    let ndc = center.xy / center.w + c * size_px * globals.viewport.zw;
    // Icons ignore depth; keep w so clipping behind the camera still works.
    out.clip = vec4<f32>(ndc * center.w, center.w * 0.5, center.w);
    out.uv = c;
    out.icon = select(model.icon, 31u, (e.owner_flags & STATE_UNIDENTIFIED) != 0u)
        | select(0u, ICON_PAUSED, paused);
    out.owner_flags = e.owner_flags;
    return out;
}

// `IconOut::icon` bit: draw the pause mark. Clear of the shape and tech bytes and the model's icon bits.
const ICON_PAUSED: u32 = 0x80000000u;
// A paused icon's quad runs from -1 to 2 * PAUSE_REACH - 1 across; the mark sits in the added part.
const PAUSE_REACH: f32 = 1.55;

// Two upright bars: the pause mark, centred on the origin.
fn sd_pause(p: vec2<f32>) -> f32 {
    let bar = vec2<f32>(0.13, 0.36);
    return min(sd_box(p - vec2<f32>(-0.19, 0.0), bar), sd_box(p - vec2<f32>(0.19, 0.0), bar));
}

fn sd_box(p: vec2<f32>, b: vec2<f32>) -> f32 {
    let d = abs(p) - b;
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0);
}

fn sd_diamond(p: vec2<f32>, r: f32) -> f32 {
    return (abs(p.x) + abs(p.y) - r) * 0.7071;
}

fn sd_triangle(p: vec2<f32>, r: f32) -> f32 {
    let k = 1.7320508;
    var q = vec2<f32>(abs(p.x) - r, p.y + r / k);
    if q.x + k * q.y > 0.0 {
        q = vec2<f32>(q.x - k * q.y, -k * q.x - q.y) * 0.5;
    }
    q.x -= clamp(q.x, -2.0 * r, 0.0);
    return -length(q) * sign(q.y);
}

fn sd_hexagon(p: vec2<f32>, r: f32) -> f32 {
    let k = vec3<f32>(-0.8660254, 0.5, 0.57735);
    var q = abs(p);
    q = q - 2.0 * min(dot(k.xy, q), 0.0) * k.xy;
    q = q - vec2<f32>(clamp(q.x, -k.z * r, k.z * r), r);
    return length(q) * sign(q.y);
}

// One edge of a polygon's signed distance (after Inigo Quilez's sdPolygon): x is the
// squared distance to the edge from `a` to `b`, y is -1 when the edge flips inside/outside.
fn poly_edge(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    let e = b - a;
    let w = p - a;
    let q = w - e * clamp(dot(w, e) / dot(e, e), 0.0, 1.0);
    let c = vec3<bool>((p.y >= a.y), (p.y < b.y), (e.x * w.y > e.y * w.x));
    let flip = all(c) || !any(c);
    return vec2<f32>(dot(q, q), select(1.0, -1.0, flip));
}

// Distance to the segment from `a` to `b`.
fn sd_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    return length(pa - ba * clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0));
}
// The Extractor's disc with its cross cut out, centred on `c`, radius `r`: every salvage
// icon carries it, since salvage feeds mass as a mine does.
fn sd_ore_disc(p: vec2<f32>, c: vec2<f32>, r: f32) -> f32 {
    let q = p - c;
    let cross = min(sd_box(q, vec2<f32>(0.12, r * 0.8)), sd_box(q, vec2<f32>(r * 0.8, 0.12)));
    return max(length(q) - r, -cross);
}

// Signed distance of the symbol, in a [-1, 1] square. Negative inside.
fn icon_shape(shape: u32, p: vec2<f32>) -> f32 {
    switch shape {
        // Commander: disc inside a ring.
        case 0u: { return min(length(p) - 0.42, abs(length(p) - 0.74) - 0.09); }
        // Engineer: hollow square.
        case 1u: { return abs(sd_box(p, vec2<f32>(0.5))) - 0.13; }
        // Bot: triangle.
        case 2u: { return sd_triangle(p + vec2<f32>(0.0, 0.1), 0.62); }
        // Tank: the tank itself in profile. Track run, hull, turret, and the gun out ahead.
        case 3u: {
            let run = sd_box(p - vec2<f32>(0.0, -0.4), vec2<f32>(0.66, 0.1)) - 0.14;
            let hull = sd_box(p - vec2<f32>(-0.04, -0.06), vec2<f32>(0.6, 0.14));
            let turret = sd_box(p - vec2<f32>(-0.16, 0.3), vec2<f32>(0.26, 0.14)) - 0.06;
            let gun = sd_box(p - vec2<f32>(0.5, 0.32), vec2<f32>(0.42, 0.07));
            return min(min(run, hull), min(turret, gun));
        }
        // Artillery: hollow diamond with a dot.
        case 4u: { return min(abs(sd_diamond(p, 0.74)) - 0.1, length(p) - 0.2); }
        // Anti-air: inverted triangle.
        case 5u: { return sd_triangle(vec2<f32>(p.x, -p.y) + vec2<f32>(0.0, 0.1), 0.62); }
        // Scout: chevron.
        case 6u: { return max(sd_triangle(p + vec2<f32>(0.0, 0.1), 0.62), -sd_triangle(p + vec2<f32>(0.0, 0.45), 0.55)); }
        // Factory: big square with a slot.
        case 7u: { return max(sd_box(p, vec2<f32>(0.72)), -sd_box(p - vec2<f32>(0.0, -0.3), vec2<f32>(0.34, 0.42))); }
        // Extractor: disc with a cross cut out.
        case 8u: { return max(length(p) - 0.62, -min(sd_box(p, vec2<f32>(0.12, 0.5)), sd_box(p, vec2<f32>(0.5, 0.12)))); }
        // Power: hexagon.
        case 9u: { return sd_hexagon(p, 0.62); }
        // Storage: squat box.
        case 10u: { return sd_box(p, vec2<f32>(0.62, 0.4)); }
        // Defense: hollow hexagon with a dot.
        case 11u: { return min(abs(sd_hexagon(p, 0.62)) - 0.1, length(p) - 0.2); }
        // Intel: ring.
        case 12u: { return abs(length(p) - 0.55) - 0.12; }
        // Wall: small square.
        case 13u: { return sd_box(p, vec2<f32>(0.6)); }
        // Shield: a spire under a dome.
        case 14u: {
            let dome = abs(length(p - vec2<f32>(0.0, 0.18)) - 0.5) - 0.09;
            let cap = select(1.0, dome, p.y > 0.14);
            let spire = max(sd_box(p - vec2<f32>(0.0, -0.12), vec2<f32>(0.11, 0.52)),
                sd_triangle(p - vec2<f32>(0.0, 0.28), 0.28));
            return min(cap, spire);
        }
        // Unidentified radar contact: filled diamond.
        case 31u: { return sd_diamond(p, 0.62); }
        // Ship: a warship in profile. Flared hull, bridge and mast, the deck gun forward.
        case 17u: {
            let hull = max(abs(p.y + 0.3) - 0.16, (abs(p.x) - 0.6 - (p.y + 0.46) * 0.9) * 0.75);
            let bridge = sd_box(p - vec2<f32>(-0.14, 0.02), vec2<f32>(0.34, 0.14));
            let mast = sd_box(p - vec2<f32>(-0.1, 0.36), vec2<f32>(0.05, 0.22));
            let gun = min(sd_box(p - vec2<f32>(0.42, -0.04), vec2<f32>(0.11, 0.08)),
                sd_box(p - vec2<f32>(0.62, -0.02), vec2<f32>(0.2, 0.035)));
            return min(min(hull, bridge), min(mast, gun));
        }
        // Submarine: a long hull low in the water and its sail.
        case 18u: {
            let q = (p - vec2<f32>(0.0, -0.16)) / vec2<f32>(0.9, 0.2);
            let hull = (length(q) - 1.0) * 0.2;
            let sail = sd_box(p - vec2<f32>(0.14, 0.14), vec2<f32>(0.14, 0.2)) - 0.03;
            return min(hull, sail);
        }
        // Air icons are all seen from above, nose up, and each role has its own outline:
        // tall and narrow shoots aircraft, wide and flat bombs, a pod on each wingtip attacks the ground.
        // Fighter: a slim jet, long nose, swept wings, tail fins.
        case 15u: {
            var v = array<vec2<f32>, 16>(
                vec2<f32>(0.0, 0.92), vec2<f32>(0.12, 0.5), vec2<f32>(0.14, 0.2), vec2<f32>(0.78, -0.34),
                vec2<f32>(0.78, -0.52), vec2<f32>(0.16, -0.4), vec2<f32>(0.36, -0.76), vec2<f32>(0.36, -0.88),
                vec2<f32>(0.0, -0.78), vec2<f32>(-0.36, -0.88), vec2<f32>(-0.36, -0.76), vec2<f32>(-0.16, -0.4),
                vec2<f32>(-0.78, -0.52), vec2<f32>(-0.78, -0.34), vec2<f32>(-0.14, 0.2), vec2<f32>(-0.12, 0.5));
            var d = dot(p - v[0], p - v[0]);
            var s = 1.0;
            for (var i = 0u; i < 16u; i++) {
                let e = poly_edge(p, v[i], v[(i + 15u) % 16u]);
                d = min(d, e.x);
                s *= e.y;
            }
            return s * sqrt(d);
        }
        // Bomber: a flying wing, full width, with a sawtooth trailing edge.
        case 16u: {
            var v = array<vec2<f32>, 12>(
                vec2<f32>(0.0, 0.56), vec2<f32>(1.0, -0.18), vec2<f32>(1.0, -0.4), vec2<f32>(0.7, -0.54),
                vec2<f32>(0.46, -0.34), vec2<f32>(0.22, -0.54), vec2<f32>(0.0, -0.36), vec2<f32>(-0.22, -0.54),
                vec2<f32>(-0.46, -0.34), vec2<f32>(-0.7, -0.54), vec2<f32>(-1.0, -0.4), vec2<f32>(-1.0, -0.18));
            var d = dot(p - v[0], p - v[0]);
            var s = 1.0;
            for (var i = 0u; i < 12u; i++) {
                let e = poly_edge(p, v[i], v[(i + 11u) % 12u]);
                d = min(d, e.x);
                s *= e.y;
            }
            return s * sqrt(d);
        }
        // Gunship: a straight wing with a pod on each tip, body and tail. Not a helicopter:
        // rotor, tilt-fan and jet gunships all share it (`hud/icons.rs` draws the same).
        case 19u: {
            let q = vec2<f32>(abs(p.x), p.y);
            let body = sd_segment(p, vec2<f32>(0.0, -0.5), vec2<f32>(0.0, 0.62)) - 0.15;
            let wing = sd_box(p - vec2<f32>(0.0, 0.02), vec2<f32>(0.62, 0.1));
            let pods = sd_segment(q, vec2<f32>(0.66, -0.26), vec2<f32>(0.66, 0.44)) - 0.14;
            let tail = sd_box(p - vec2<f32>(0.0, -0.66), vec2<f32>(0.3, 0.08));
            return min(min(body, wing), min(pods, tail));
        }
        // Capital transport: wedge prow and broad rectangular stern drive shoulders.
        case 21u: {
            let hull = sd_box(p - vec2<f32>(0.0, -0.22), vec2<f32>(0.32, 0.58));
            let prow = max(abs(p.x) - (0.40 - p.y * 0.34), max(-p.y, p.y - 0.88));
            let q = vec2<f32>(abs(p.x), p.y);
            let drives = sd_box(q - vec2<f32>(0.47, -0.39), vec2<f32>(0.15, 0.39));
            return min(hull, min(prow, drives));
        }
        // Nuclear silo: a missile standing in an open tube, fins at its foot.
        case 25u: {
            let body = sd_box(p - vec2<f32>(0.0, 0.02), vec2<f32>(0.12, 0.44));
            let nose = sd_segment(p, vec2<f32>(0.0, 0.46), vec2<f32>(0.0, 0.74)) - 0.08;
            let fins = sd_segment(p, vec2<f32>(-0.26, -0.44), vec2<f32>(0.26, -0.44)) - 0.07;
            let wall_l = sd_box(p - vec2<f32>(-0.5, -0.36), vec2<f32>(0.08, 0.44));
            let wall_r = sd_box(p - vec2<f32>(0.5, -0.36), vec2<f32>(0.08, 0.44));
            let floor = sd_box(p - vec2<f32>(0.0, -0.76), vec2<f32>(0.58, 0.08));
            return min(min(min(body, nose), fins), min(min(wall_l, wall_r), floor));
        }
        // Interceptor array: a missile rising out of a shield's bowl toward the mark it meets.
        case 26u: {
            let bowl = max(abs(length(p - vec2<f32>(0.0, 0.05)) - 0.66) - 0.09, p.y + 0.05);
            let body = sd_segment(p, vec2<f32>(0.0, -0.42), vec2<f32>(0.0, 0.3)) - 0.09;
            let nose = sd_segment(p, vec2<f32>(0.0, 0.3), vec2<f32>(0.0, 0.44)) - 0.05;
            let mark = min(sd_segment(p, vec2<f32>(-0.16, 0.62), vec2<f32>(0.16, 0.9)),
                sd_segment(p, vec2<f32>(-0.16, 0.9), vec2<f32>(0.16, 0.62))) - 0.06;
            return min(min(bowl, body), min(nose, mark));
        }
        // Capital warship from above, nose up: a long narrow spine with a pointed prow, flank
        // sponsons, the spinal gun a slot down its centre (`hud/icons.rs` draws the same).
        case 27u: {
            var v = array<vec2<f32>, 15>(
                vec2<f32>(0.0, 0.96), vec2<f32>(0.16, 0.62), vec2<f32>(0.2, 0.12), vec2<f32>(0.36, 0.08),
                vec2<f32>(0.36, -0.12), vec2<f32>(0.24, -0.16), vec2<f32>(0.3, -0.56), vec2<f32>(0.3, -0.9),
                vec2<f32>(-0.3, -0.9), vec2<f32>(-0.3, -0.56), vec2<f32>(-0.24, -0.16), vec2<f32>(-0.36, -0.12),
                vec2<f32>(-0.36, 0.08), vec2<f32>(-0.2, 0.12), vec2<f32>(-0.16, 0.62));
            var d = dot(p - v[0], p - v[0]);
            var s = 1.0;
            for (var i = 0u; i < 15u; i++) {
                let e = poly_edge(p, v[i], v[(i + 14u) % 15u]);
                d = min(d, e.x);
                s *= e.y;
            }
            let hull = s * sqrt(d);
            let spine = sd_box(p - vec2<f32>(0.0, -0.02), vec2<f32>(0.06, 0.6));
            return max(hull, -spine);
        }
        // A tier-5 titan, from the front: a giant mid-stride, head over rocket-pod
        // shoulders, the rotary rail cluster on its right arm and the long bore on its
        // left (`hud/icons.rs` draws the same; `fs_icon` frames it).
        case 28u: { return sd_titan(p); }
        // Salvage: the Extractor's disc (salvage feeds mass as a mine does) with its
        // domain's mark (`hud/icons.rs` draws the same). A structure rings it with its reach.
        case 29u: { return min(sd_ore_disc(p, vec2<f32>(0.0), 0.5), abs(length(p) - 0.8) - 0.07); }
        // Salvage boat: the disc riding a hull.
        case 30u: {
            let hull = max(abs(p.y + 0.52) - 0.13, (abs(p.x) - 0.66 - (p.y + 0.65) * 0.9) * 0.75);
            return min(sd_ore_disc(p, vec2<f32>(0.0, 0.2), 0.5), hull);
        }
        // Salvage carrier, from above: the disc with swept wings out of its sides and a tail.
        case 32u: {
            let wings = sd_segment(vec2<f32>(abs(p.x), p.y), vec2<f32>(0.45, -0.05), vec2<f32>(0.95, -0.3)) - 0.1;
            let tail = sd_segment(p, vec2<f32>(0.0, -0.45), vec2<f32>(0.0, -0.8)) - 0.09;
            return min(sd_ore_disc(p, vec2<f32>(0.0, 0.05), 0.52), min(wings, tail));
        }
        // Salvage drone: the disc alone, drawn small (`vs_icon`).
        case 33u: { return sd_ore_disc(p, vec2<f32>(0.0), 0.6); }
        // Torpedo bomber: a gull-winged plane in the upper part, a finned torpedo under it
        // (`hud/icons.rs` draws the same).
        case 34u: {
            var v = array<vec2<f32>, 10>(
                vec2<f32>(0.0, 0.62), vec2<f32>(0.34, 0.46), vec2<f32>(0.96, 0.6), vec2<f32>(0.96, 0.42),
                vec2<f32>(0.34, 0.26), vec2<f32>(0.0, 0.36), vec2<f32>(-0.34, 0.26), vec2<f32>(-0.96, 0.42),
                vec2<f32>(-0.96, 0.6), vec2<f32>(-0.34, 0.46));
            var d = dot(p - v[0], p - v[0]);
            var s = 1.0;
            for (var i = 0u; i < 10u; i++) {
                let e = poly_edge(p, v[i], v[(i + 9u) % 10u]);
                d = min(d, e.x);
                s *= e.y;
            }
            let wing = s * sqrt(d);
            let body = sd_segment(p, vec2<f32>(0.0, 0.1), vec2<f32>(0.0, 0.9)) - 0.12;
            let tail = sd_box(p - vec2<f32>(0.0, 0.08), vec2<f32>(0.3, 0.07));
            let torpedo = sd_segment(p, vec2<f32>(-0.42, -0.5), vec2<f32>(0.52, -0.5)) - 0.14;
            let fin = sd_box(p - vec2<f32>(-0.62, -0.5), vec2<f32>(0.06, 0.22));
            return min(min(min(wing, body), tail), min(torpedo, fin));
        }
        default: { return sd_box(p, vec2<f32>(0.6)); }
    }
}

// `IconKind::Titan`.
const ICON_TITAN: u32 = 28u;
// `IconKind::SalvageDrone`: drawn small, one of a carrier's swarm.
const ICON_SALVAGE_DRONE: u32 = 33u;

fn sd_poly4(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, c: vec2<f32>, d: vec2<f32>) -> f32 {
    var v = array<vec2<f32>, 4>(a, b, c, d);
    var dd = dot(p - v[0], p - v[0]);
    var s = 1.0;
    for (var i = 0u; i < 4u; i++) {
        let e = poly_edge(p, v[i], v[(i + 3u) % 4u]);
        dd = min(dd, e.x);
        s *= e.y;
    }
    return s * sqrt(dd);
}

fn sd_titan(p: vec2<f32>) -> f32 {
    var v = array<vec2<f32>, 6>(
        vec2<f32>(-0.46, 0.52), vec2<f32>(0.46, 0.52), vec2<f32>(0.34, 0.22),
        vec2<f32>(0.2, 0.02), vec2<f32>(-0.2, 0.02), vec2<f32>(-0.34, 0.22));
    var dd = dot(p - v[0], p - v[0]);
    var s = 1.0;
    for (var i = 0u; i < 6u; i++) {
        let e = poly_edge(p, v[i], v[(i + 5u) % 6u]);
        dd = min(dd, e.x);
        s *= e.y;
    }
    var d = s * sqrt(dd);
    d = min(d, sd_box(p - vec2<f32>(0.0, 0.66), vec2<f32>(0.1, 0.1)));
    d = min(d, sd_box(p - vec2<f32>(-0.36, 0.6), vec2<f32>(0.12, 0.08)));
    d = min(d, sd_box(p - vec2<f32>(0.36, 0.6), vec2<f32>(0.12, 0.08)));
    d = min(d, sd_segment(p, vec2<f32>(0.44, 0.46), vec2<f32>(0.64, 0.22)) - 0.09);
    d = min(d, sd_segment(p, vec2<f32>(-0.44, 0.46), vec2<f32>(-0.64, 0.22)) - 0.09);
    // Right arm: the rotary cluster and its barrels.
    d = min(d, sd_box(p - vec2<f32>(0.68, 0.04), vec2<f32>(0.14, 0.2)));
    d = min(d, sd_segment(p, vec2<f32>(0.68, -0.16), vec2<f32>(0.68, -0.38)) - 0.065);
    // Left arm: the bore, a long tapering spike.
    d = min(d, sd_poly4(p, vec2<f32>(-0.82, 0.26), vec2<f32>(-0.52, 0.26), vec2<f32>(-0.62, -0.44), vec2<f32>(-0.72, -0.44)));
    d = min(d, sd_box(p, vec2<f32>(0.17, 0.08)));
    // Legs mid-stride: the left planted, the right lifting.
    d = min(d, sd_segment(p, vec2<f32>(-0.1, -0.02), vec2<f32>(-0.27, -0.38)) - 0.1);
    d = min(d, sd_segment(p, vec2<f32>(-0.27, -0.38), vec2<f32>(-0.33, -0.8)) - 0.085);
    d = min(d, sd_box(p - vec2<f32>(-0.35, -0.85), vec2<f32>(0.16, 0.055)));
    d = min(d, sd_segment(p, vec2<f32>(0.1, -0.02), vec2<f32>(0.24, -0.3)) - 0.1);
    d = min(d, sd_segment(p, vec2<f32>(0.24, -0.3), vec2<f32>(0.29, -0.66)) - 0.085);
    d = min(d, sd_box(p - vec2<f32>(0.31, -0.72), vec2<f32>(0.14, 0.055)));
    return d;
}

// A titan's frame: four corner brackets round the whole quad (in `uv`).
fn sd_titan_frame(uv: vec2<f32>) -> f32 {
    let q = abs(uv);
    let across = sd_box(q - vec2<f32>(0.82, 0.95), vec2<f32>(0.15, 0.035));
    let down = sd_box(q - vec2<f32>(0.95, 0.82), vec2<f32>(0.035, 0.15));
    return min(across, down);
}

@fragment
fn fs_icon(in: IconOut) -> @location(0) vec4<f32> {
    let shape = in.icon & 0xFFu;
    let tech = (in.icon >> 8u) & 0xFFu;
    // The symbol sits in the upper part; tech pips go underneath.
    let p = (in.uv - vec2<f32>(0.0, 0.12)) / 0.84;
    var d = icon_shape(shape, p);
    // One pip per tech level, up to 5: five still fit inside the quad with their outline.
    for (var i = 0u; i < tech && i < 5u; i++) {
        let x = (f32(i) - (f32(min(tech, 5u)) - 1.0) * 0.5) * 0.3;
        d = min(d, sd_box(in.uv - vec2<f32>(x, -0.84), vec2<f32>(0.1, 0.07)));
    }
    // A titan's corner brackets: drawn paler than the figure, outlined like it.
    var frame = 0.0;
    if shape == ICON_TITAN {
        let f = sd_titan_frame(in.uv);
        d = min(d, f);
        let aaf = fwidth(f) * 1.2;
        frame = 1.0 - smoothstep(-aaf, aaf, f);
    }
    let aa = fwidth(d) * 1.2;
    let fill = 1.0 - smoothstep(-aa, aa, d);
    let outline = 1.0 - smoothstep(-aa, aa, d - 0.16);
    // Paused work: an amber pause mark up beside the symbol, outlined in black like it.
    var mark_fill = 0.0;
    var mark_edge = 0.0;
    if (in.icon & ICON_PAUSED) != 0u {
        let m = sd_pause(in.uv - vec2<f32>(2.0 * PAUSE_REACH - 1.0 - 0.52, 0.34));
        let aam = fwidth(m) * 1.2;
        mark_fill = 1.0 - smoothstep(-aam, aam, m);
        mark_edge = 1.0 - smoothstep(-aam, aam, m - 0.16);
    }
    if outline <= 0.01 && mark_edge <= 0.01 {
        discard;
    }
    var color = globals.team_colors[in.owner_flags & 7u].rgb;
    if (in.owner_flags & STATE_UNIDENTIFIED) != 0u {
        color = vec3<f32>(0.62, 0.65, 0.70);
    } else if (in.owner_flags & FLAG_UNDER_CONSTRUCTION) != 0u {
        color = color * 0.45;
    }
    let icon = mix(mix(vec3<f32>(0.0), color * 1.3, fill), mix(color, vec3<f32>(1.0), 0.55) * 1.3, frame);
    // Construction amber (0xFFA928), a little over one so it holds its own beside team colours.
    let amber = vec3<f32>(1.0, 0.40, 0.024) * 1.35;
    let rgb = mix(icon * outline, mix(vec3<f32>(0.0), amber, mark_fill), mark_edge);
    return vec4<f32>(rgb / max(max(outline, mark_edge), 1e-4), max(outline, mark_edge));
}


struct MarkOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) kind: u32,
    @location(2) @interpolate(flat) health: f32,
    @location(3) @interpolate(flat) build: f32,
    @location(4) @interpolate(flat) shield: f32,
}

struct RingOut {
    @builtin(position) clip: vec4<f32>,
    // Metres in the unit's frame: +x forward, +y left.
    @location(0) local: vec2<f32>,
    @location(1) @interpolate(flat) kind: u32,
    // The outline's box: centre (xy) and half-extents (zw), metres in the unit's frame.
    @location(2) @interpolate(flat) frame: vec4<f32>,
    // x: hull-plan half size (`ModelInfo::plan_half`), y: how far the outline stands off
    // the hull, metres, z: 0 a structure, 1 a mobile unit, 2 a titan, w: the hull plan's
    // atlas layer.
    @location(3) @interpolate(flat) plan: vec4<f32>,
}

// The selection mark on the ground: the outline of the unit's body (its baked hull plan
// without guns and arms, stood off a little), corner brackets on the box round it, and a
// heading arrow on a mobile unit. Zoomed out, the outline eases into a rounded box a few
// pixels across. Each mark is RING_GRID x RING_GRID instanced cells laid over the
// ground, so it follows hills under a big unit instead of floating or sinking into them.
@vertex
fn vs_ring(@location(0) corner: vec2<f32>, @builtin(instance_index) instance: u32) -> RingOut {
    let cells = RING_GRID * RING_GRID;
    let mark = marks[instance / cells];
    let cell = instance % cells;
    let at = (vec2<f32>(f32(cell % RING_GRID), f32(cell / RING_GRID)) + corner * 0.5 + 0.5)
        / f32(RING_GRID) * 2.0 - 1.0;
    let e = load_entity(mark.unit_index);
    let model = models[e.blueprint];
    let c = entity_center(e);
    // Metres per output pixel at the unit.
    let mpp = distance(c, globals.camera.xyz) / globals.lod.x;
    var plan_box = model.plan_box;
    if plan_box.z <= 0.0 {
        plan_box = vec4<f32>(0.0, 0.0, e.radius, e.radius);
    }
    let size = max(plan_box.z, plan_box.w);
    let stand_off = max(clamp(0.9 + 0.06 * size, 1.2, 5.0), 3.0 * mpp);
    // Never smaller than a few pixels, so selections stay visible when zoomed out.
    let half = max(plan_box.zw + stand_off, vec2<f32>(9.0 * mpp));
    // Room for the brackets, the chevron and the glow.
    let reach = half + 14.0 * mpp + 0.25 * min(half.x, half.y);
    let local = plan_box.xy + at * reach;
    let yaw = lerp_angle(e.prev_heading, e.heading, globals.sun.w);
    let offset = vec2<f32>(local.x * cos(yaw) - local.y * sin(yaw), local.x * sin(yaw) + local.y * cos(yaw));
    let xy = c.xy + offset;
    // Clear of the ground by more the farther it is, so depth never eats it.
    let lift = 0.35 + 0.0015 * distance(c, globals.camera.xyz);
    let water = globals.map.z;
    var z = c.z + 0.6;
    if (model.icon & ICON_AIR) != 0u {
        // Aircraft: flat under the airframe, kept out of hills.
        z = max(z, terrain_height(xy) + lift);
    } else if c.z < water - 1.0 {
        // A dived submarine: flat at its depth, above the seabed.
        z = max(z, terrain_height(xy) + lift);
    } else {
        // Everything else lies on the ground (or the sea) wherever the mark reaches,
        // whatever height the unit's origin stands at (a titan's is up at its hips).
        z = max(terrain_height(xy), water) + lift;
    }
    var out: RingOut;
    out.clip = globals.view_proj * vec4<f32>(xy, z, 1.0);
    out.local = local;
    out.kind = mark.kind;
    out.frame = vec4<f32>(plan_box.xy, half);
    // 0 a structure, 1 a mobile unit (it gets the heading arrow), 2 a titan: tier 5 and
    // mobile, which the HUD frames already (mc-game `titan_marks::is_titan`), so no brackets.
    let mobile = (model.icon & ICON_MOBILE) != 0u;
    let titan = mobile && ((model.icon >> 8u) & 0xFFu) >= 5u;
    let kind = select(select(0.0, 1.0, mobile), 2.0, titan);
    out.plan = vec4<f32>(model.plan_half, stand_off, kind, f32(e.blueprint));
    return out;
}

// Distance to the hull plan's edge, metres; negative inside. Past the atlas it keeps
// growing with the distance to its edge, so the outline stays closed round any hull.
fn ring_plan_sd(local: vec2<f32>, plan_half: f32, layer: i32) -> f32 {
    let layers = textureNumLayers(hull_plans);
    let edge = max(plan_half, 0.5) * HULL_PLAN_REACH * 0.99;
    let inside = clamp(local, vec2<f32>(-edge), vec2<f32>(edge));
    if layers == 0u {
        return HULL_PLAN_RANGE;
    }
    let tex = inside / edge * 0.495 + 0.5;
    let s = textureSampleLevel(hull_plans, clamp_sampler, tex, min(layer, i32(layers) - 1), 0.0);
    // A: the body alone, guns and arms left out.
    return (0.5 - s.a) * 2.0 * HULL_PLAN_RANGE + length(local - inside);
}

fn sd_round_box(p: vec2<f32>, b: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - b + vec2<f32>(r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

@fragment
fn fs_ring(in: RingOut) -> @location(0) vec4<f32> {
    // Before any branch, so the derivatives are taken over the whole quad.
    let mpp = max(length(fwidth(in.local)) * 0.7071, 1e-4);
    let q = in.local - in.frame.xy;
    let half = in.frame.zw;
    let small = min(half.x, half.y);
    let hovered = (in.kind & 1u) != 0u;
    let enemy = (in.kind & 2u) != 0u;

    // The outline: the hull's own shape when it is big on screen, a rounded box when not.
    let boxed = sd_round_box(q, half, min(small * 0.45, 10.0));
    let hull = ring_plan_sd(in.local, in.plan.x, i32(in.plan.w)) - in.plan.y;
    let detail = smoothstep(18.0, 42.0, small / mpp);
    // A little of the box in it rounds off legs and barrels into one smooth line. The plan
    // only measures out to HULL_PLAN_RANGE, so past the box the box's own distance rules:
    // zoomed out, a capped distance would glow as if the edge were everywhere.
    let d = max(mix(boxed, hull, detail * 0.75), boxed);
    let dp = d / mpp;
    let line = 1.0 - smoothstep(0.55, 1.35, abs(dp));
    let glow = exp(-abs(dp) / 3.5) * 0.35;
    // A wash inside the edge, hatched on the diagonal, fading in toward the hull.
    let band = select(0.0, exp(dp / 7.0), dp < 0.0);
    let hatch = step(0.62, fract((q.x + q.y) / (5.0 * mpp)));
    let wash = band * (0.10 + 0.22 * hatch);

    // Corner brackets on the box, a few pixels out.
    let outer = half + vec2<f32>(5.0 * mpp);
    let arm = clamp(0.28 * min(outer.x, outer.y), 6.0 * mpp, 42.0 * mpp);
    let aq = abs(q);
    let db = abs(max(aq.x - outer.x, aq.y - outer.y)) / mpp;
    let near_corner = aq.x > outer.x - arm && aq.y > outer.y - arm;
    let bracket = select(0.0, 1.0 - smoothstep(0.9, 1.7, db), near_corner && max(aq.x - outer.x, aq.y - outer.y) < 1.7 * mpp && in.plan.z < 1.5);

    // Heading arrowhead in front of a mobile unit, past the front brackets.
    var chevron = 0.0;
    if in.plan.z > 0.5 {
        let s = max(8.0 * mpp, 0.16 * half.y);
        let tip = outer.x + 4.0 * mpp + s;
        // From the tip back: x along the unit's heading, y out to either side.
        let a = vec2<f32>(tip - q.x, abs(q.y));
        let slant = 0.819; // 1 / sqrt(1 + 0.7²)
        let head = max(max(-a.x, a.x - s), (a.y - 0.7 * a.x) * slant);
        // Its back is notched, so it reads as an arrow and not a bracket.
        let back = a.x - 0.55 * s;
        let notch = max(-back, (a.y - 0.7 * back) * slant);
        chevron = 1.0 - smoothstep(-0.5, 0.5, max(head, -notch) / mpp);
    }

    // A slow sweep of light round the outline of a selected unit.
    var sweep = 0.0;
    if !hovered {
        let a = atan2(q.y / half.y, q.x / half.x);
        sweep = pow(0.5 + 0.5 * cos(a - globals.camera.w * 2.4), 18.0);
    }

    var tint = vec3<f32>(0.30, 1.0, 0.62);
    if enemy {
        tint = vec3<f32>(1.0, 0.26, 0.16);
    } else if hovered {
        tint = vec3<f32>(0.82, 0.90, 1.0);
    }
    let bright = mix(tint, vec3<f32>(1.0), 0.55);
    let fade = select(1.0, 0.6, hovered);
    let hover_wash = select(1.0, 0.0, hovered);
    let a_line = line * (0.85 + 0.15 * sweep);
    let a_glow = glow * (1.0 + 1.6 * sweep);
    let a_wash = wash * hover_wash;
    let a_mark = max(bracket, chevron);
    let alpha = clamp(max(max(a_line, a_mark), a_glow + a_wash) * fade, 0.0, 1.0);
    if alpha <= 0.01 {
        discard;
    }
    // Lines and brackets near white, the arrow and the glow in the tint.
    let lit = bright * 1.5 * (a_line + bracket) + tint * 1.6 * chevron + tint * (a_glow + a_wash) * (1.0 + sweep);
    let rgb = lit / max(a_line + bracket + chevron + a_glow + a_wash, 1e-4);
    return vec4<f32>(rgb, alpha);
}

// Status bars sit on the ground at the unit's feet, as wide as the hull.
// Shield (always blue, whatever the faction's dome colour) on top, health, then construction. Pixel heights must match fs_bar.
@vertex
fn vs_bar(@location(0) corner: vec2<f32>, @builtin(instance_index) instance: u32) -> MarkOut {
    let mark = marks[instance];
    let e = load_entity(mark.unit_index);
    let center_w = entity_center(e);
    let feet = vec3<f32>(center_w.xy, max(center_w.z, terrain_height(center_w.xy)));
    let center = globals.view_proj * vec4<f32>(feet, 1.0);
    let dist = max(distance(center_w, globals.camera.xyz), 1.0);
    let px = globals.lod.x / dist;
    let half_w = max(e.radius * 0.8 * px, 16.0);
    let show_build = mark.work >= 0.0;
    let show_shield = mark.shield >= 0.0;
    let health_h = 6.0;
    let shield_h = 4.0;
    let build_h = 5.0;
    let gap = 2.0;
    let up = select(0.0, shield_h + gap, show_shield);
    let down = select(0.0, build_h + gap, show_build);
    let half_h = 0.5 * (health_h + up + down);
    // South of the footprint, so the stack sits on the ground in front of the hull.
    let mid_y = -max(e.radius * 0.6 * px, 6.0) - half_h - 2.0;
    var out: MarkOut;
    // NDC spans two units across the viewport, so a pixel is 2 / size.
    var ndc = center.xy / center.w + (corner * vec2<f32>(half_w, half_h) + vec2<f32>(0.0, mid_y)) * 2.0 * globals.viewport.zw;
    // Radar blips stay anonymous: a ring, no bars.
    if (e.owner_flags & STATE_UNIDENTIFIED) != 0u {
        ndc = vec2<f32>(4.0);
    }
    out.clip = vec4<f32>(ndc * center.w, center.w * 0.5, center.w);
    out.uv = corner;
    out.kind = mark.kind;
    out.health = e.health;
    out.build = mark.work;
    out.shield = mark.shield;
    return out;
}

// `per_px`: how far uv.x moves per screen pixel; `h`: the row's height in pixels.
// The frame is one pixel on every side, however wide the hull.
fn bar_fill(uv: vec2<f32>, fill: f32, color: vec3<f32>, per_px: f32, h: f32) -> vec4<f32> {
    let ax = abs(uv.x);
    let ay = abs(uv.y);
    if ax > 1.0 || ay > 1.0 {
        return vec4<f32>(0.0);
    }
    // A thin black frame so the line reads on snow, grass and rock.
    if ax > 1.0 - per_px || ay > 1.0 - 2.0 / h {
        return vec4<f32>(0.0, 0.0, 0.0, 0.9);
    }
    // What is missing stays in the bar's own colour, dimmed, so the bar's length reads.
    if uv.x * 0.5 + 0.5 > fill {
        return vec4<f32>(color * 0.12 + vec3<f32>(0.015), 0.96);
    }
    return vec4<f32>(color * 1.2, 1.0);
}

fn row_uv(uv_x: f32, y: f32, top: f32, h: f32) -> vec2<f32> {
    return vec2<f32>(uv_x, 1.0 - 2.0 * (y - top) / h);
}

@fragment
fn fs_bar(in: MarkOut) -> @location(0) vec4<f32> {
    let health_color = mix(vec3<f32>(1.0, 0.12, 0.05), vec3<f32>(0.2, 1.0, 0.3), smoothstep(0.2, 0.7, in.health));
    let show_shield = in.shield >= 0.0;
    let show_build = in.build >= 0.0;
    // Before any branch, so the derivative is taken over the whole quad.
    let per_px = fwidth(in.uv.x);
    let health_h = 6.0;
    let shield_h = 4.0;
    let build_h = 5.0;
    let gap = 2.0;
    let up = select(0.0, shield_h + gap, show_shield);
    let down = select(0.0, build_h + gap, show_build);
    let total = health_h + up + down;
    // uv.y = +1 at the top of the stack: shield, then health, then construction.
    let y = (1.0 - in.uv.y) * 0.5 * total;
    var cursor = 0.0;
    if show_shield {
        if y < cursor + shield_h {
            return bar_fill(row_uv(in.uv.x, y, cursor, shield_h), in.shield, vec3<f32>(0.08, 0.38, 1.0), per_px, shield_h);
        }
        cursor += shield_h;
        if y < cursor + gap {
            return vec4<f32>(0.0);
        }
        cursor += gap;
    }
    if y < cursor + health_h {
        return bar_fill(row_uv(in.uv.x, y, cursor, health_h), in.health, health_color, per_px, health_h);
    }
    cursor += health_h;
    if show_build {
        if y < cursor + gap {
            return vec4<f32>(0.0);
        }
        cursor += gap;
        if y < cursor + build_h {
            return bar_fill(row_uv(in.uv.x, y, cursor, build_h), in.build, vec3<f32>(1.0, 0.62, 0.12), per_px, build_h);
        }
    }
    return vec4<f32>(0.0);
}
