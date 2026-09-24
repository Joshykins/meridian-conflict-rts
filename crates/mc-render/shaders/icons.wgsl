//!use bindings
// Strategic icons, selection rings and health bars.
//
// Icons: units whose model would be smaller than a few pixels are drawn as a
// constant-size symbol instead, so the whole war stays readable at full
// zoom-out. They come from the cull pass's last draw slot.

struct Mark {
    // Visible-list style entity index (DYNAMIC_BIT set for units).
    entity: u32,
    // bit 0: hovered (else selected). bit 1: enemy.
    kind: u32,
    // Construction fill, zero to one. Negative: no construction bar.
    work: f32,
    // Shield fill, zero to one. Negative: no shield line.
    shield: f32,
    // Half length and half width of a long hull (a capital ship), metres: its ring is
    // an ellipse fitted to the hull. Zero: a circle round `radius`.
    hull: vec2<f32>,
    _pad: vec2<f32>,
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
    let center = globals.view_proj * vec4<f32>(entity_center(e) + vec3<f32>(0.0, 0.0, model.height * 0.5), 1.0);
    // Paused work: the quad reaches out to the right to carry a pause mark beside the symbol.
    let paused = (e._pad3a & UNIT_PAUSED) != 0u
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
        // tall and narrow shoots aircraft, wide and flat bombs, crossed rotors attack the ground.
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
        // Gunship: crossed rotor blades over the body, tail boom and tail rotor.
        case 19u: {
            // Rotated 45 degrees about the hub, the two blades are the two axes.
            let q = (p - vec2<f32>(0.0, 0.1)) * 0.70710678;
            let blades = min(sd_box(vec2<f32>(q.x + q.y, q.y - q.x), vec2<f32>(0.9, 0.085)),
                sd_box(vec2<f32>(q.y - q.x, q.x + q.y), vec2<f32>(0.9, 0.085)));
            let body = (length((p - vec2<f32>(0.0, 0.1)) / vec2<f32>(0.22, 0.36)) - 1.0) * 0.22;
            let boom = sd_box(p - vec2<f32>(0.0, -0.44), vec2<f32>(0.06, 0.3));
            let tail = sd_box(p - vec2<f32>(0.0, -0.72), vec2<f32>(0.24, 0.06));
            return min(blades, min(body, min(boom, tail)));
        }
        // Airbase: the bunker in the ground, and a V over it pointing down into it.
        case 20u: {
            let bunker = sd_box(p - vec2<f32>(0.0, -0.46), vec2<f32>(0.74, 0.18)) - 0.04;
            let v = min(sd_segment(p, vec2<f32>(-0.52, 0.62), vec2<f32>(0.0, 0.04)),
                sd_segment(p, vec2<f32>(0.52, 0.62), vec2<f32>(0.0, 0.04))) - 0.13;
            return min(bunker, v);
        }
        // Capital transport: wedge prow and broad rectangular stern drive shoulders.
        case 21u: {
            let hull = sd_box(p - vec2<f32>(0.0, -0.22), vec2<f32>(0.32, 0.58));
            let prow = max(abs(p.x) - (0.40 - p.y * 0.34), max(-p.y, p.y - 0.88));
            let q = vec2<f32>(abs(p.x), p.y);
            let drives = sd_box(q - vec2<f32>(0.47, -0.39), vec2<f32>(0.15, 0.39));
            return min(hull, min(prow, drives));
        }
        default: { return sd_box(p, vec2<f32>(0.6)); }
    }
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
    let icon = mix(vec3<f32>(0.0), color * 1.3, fill);
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

// Thin ground ring around a marked unit.
@vertex
fn vs_ring(@location(0) corner: vec2<f32>, @builtin(instance_index) instance: u32) -> MarkOut {
    let mark = marks[instance];
    let e = load_entity(mark.entity);
    let c = entity_center(e);
    // Never thinner than a few pixels, so selections stay visible when zoomed out.
    let dist = distance(c, globals.camera.xyz);
    let least = dist * 7.0 / globals.lod.x;
    var offset = corner * max(e.radius * 1.25, least);
    if mark.hull.x > 0.0 {
        // A long hull: an ellipse a little outside it, turned with the ship.
        let radii = max(mark.hull * vec2<f32>(1.12, 1.3) + 6.0, vec2<f32>(least));
        let yaw = lerp_angle(e.prev_heading, e.heading, globals.sun.w);
        let v = corner * radii;
        offset = vec2<f32>(v.x * cos(yaw) - v.y * sin(yaw), v.x * sin(yaw) + v.y * cos(yaw));
    }
    let world = c + vec3<f32>(offset, 0.6);
    var out: MarkOut;
    out.clip = globals.view_proj * vec4<f32>(world.xy, max(world.z, terrain_height(world.xy) + 0.4), 1.0);
    out.uv = corner;
    out.kind = mark.kind;
    out.health = e.health;
    out.build = mark.work;
    out.shield = mark.shield;
    return out;
}

@fragment
fn fs_ring(in: MarkOut) -> @location(0) vec4<f32> {
    let d = length(in.uv);
    let w = fwidth(d);
    let band = smoothstep(0.80 - w, 0.80, d) * (1.0 - smoothstep(0.94, 0.94 + w, d));
    if band <= 0.01 {
        discard;
    }
    var color = vec3<f32>(0.35, 1.0, 0.45);
    if (in.kind & 2u) != 0u {
        color = vec3<f32>(1.0, 0.22, 0.14);
    } else if (in.kind & 1u) != 0u {
        color = vec3<f32>(1.0, 1.0, 1.0);
    }
    return vec4<f32>(color * 1.5, band * 0.9);
}

// Status bars sit on the ground at the unit's feet, as wide as the hull.
// Shield on top, health, then construction. Pixel heights must match fs_bar.
@vertex
fn vs_bar(@location(0) corner: vec2<f32>, @builtin(instance_index) instance: u32) -> MarkOut {
    let mark = marks[instance];
    let e = load_entity(mark.entity);
    let center_w = entity_center(e);
    let feet = vec3<f32>(center_w.xy, max(center_w.z, terrain_height(center_w.xy)));
    let center = globals.view_proj * vec4<f32>(feet, 1.0);
    let dist = max(distance(center_w, globals.camera.xyz), 1.0);
    let px = globals.lod.x / dist;
    let half_w = max(e.radius * 1.35 * px, 22.0);
    let show_build = mark.work >= 0.0;
    let show_shield = mark.shield >= 0.0;
    let health_h = 8.0;
    let shield_h = 4.5;
    let build_h = 6.5;
    let gap = 2.0;
    let up = select(0.0, shield_h + gap, show_shield);
    let down = select(0.0, build_h + gap, show_build);
    let half_h = 0.5 * (health_h + up + down);
    // South of the footprint, so the stack sits on the ground in front of the hull.
    let mid_y = -max(e.radius * 0.9 * px, 8.0) - half_h - 2.0;
    var out: MarkOut;
    var ndc = center.xy / center.w + (corner * vec2<f32>(half_w, half_h) + vec2<f32>(0.0, mid_y)) * globals.viewport.zw;
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

fn bar_fill(uv: vec2<f32>, fill: f32, color: vec3<f32>) -> vec4<f32> {
    let ax = abs(uv.x);
    let ay = abs(uv.y);
    if ax > 1.0 || ay > 1.0 {
        return vec4<f32>(0.0);
    }
    // Heavy black frame so the line reads on snow, grass and rock.
    if ax > 0.965 || ay > 0.70 {
        return vec4<f32>(0.0, 0.0, 0.0, 0.95);
    }
    if uv.x * 0.5 + 0.5 > fill {
        return vec4<f32>(0.03, 0.03, 0.04, 0.9);
    }
    return vec4<f32>(color * 1.45, 1.0);
}

fn row_uv(uv_x: f32, y: f32, top: f32, h: f32) -> vec2<f32> {
    return vec2<f32>(uv_x, 1.0 - 2.0 * (y - top) / h);
}

@fragment
fn fs_bar(in: MarkOut) -> @location(0) vec4<f32> {
    let health_color = mix(vec3<f32>(1.0, 0.12, 0.05), vec3<f32>(0.2, 1.0, 0.3), smoothstep(0.2, 0.7, in.health));
    let show_shield = in.shield >= 0.0;
    let show_build = in.build >= 0.0;
    let health_h = 8.0;
    let shield_h = 4.5;
    let build_h = 6.5;
    let gap = 2.0;
    let up = select(0.0, shield_h + gap, show_shield);
    let down = select(0.0, build_h + gap, show_build);
    let total = health_h + up + down;
    // uv.y = +1 at the top of the stack: shield, then health, then construction.
    let y = (1.0 - in.uv.y) * 0.5 * total;
    var cursor = 0.0;
    if show_shield {
        if y < cursor + shield_h {
            return bar_fill(row_uv(in.uv.x, y, cursor, shield_h), in.shield, vec3<f32>(0.48, 0.83, 1.0));
        }
        cursor += shield_h;
        if y < cursor + gap {
            return vec4<f32>(0.0);
        }
        cursor += gap;
    }
    if y < cursor + health_h {
        return bar_fill(row_uv(in.uv.x, y, cursor, health_h), in.health, health_color);
    }
    cursor += health_h;
    if show_build {
        if y < cursor + gap {
            return vec4<f32>(0.0);
        }
        cursor += gap;
        if y < cursor + build_h {
            return bar_fill(row_uv(in.uv.x, y, cursor, build_h), in.build, vec3<f32>(1.0, 0.62, 0.12));
        }
    }
    return vec4<f32>(0.0);
}
