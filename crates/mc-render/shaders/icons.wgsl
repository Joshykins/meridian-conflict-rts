//!use bindings
// Strategic icons, selection rings and health bars.
//
// Icons: units whose model would be smaller than a few pixels are drawn as a
// constant-size symbol instead, so the whole war stays readable at full
// zoom-out. They come from the cull pass's last draw slot.

struct Mark {
    // Visible-list style entity index (DYNAMIC_BIT set for units).
    entity: u32,
    // 0: selected (ring + bar), 1: hovered (ring only)
    kind: u32,
}

@group(1) @binding(0) var<storage, read> marks: array<Mark>;

struct IconOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) icon: u32,
    @location(2) @interpolate(flat) owner_flags: u32,
    @location(3) @interpolate(flat) health: f32,
}

fn entity_center(e: Entity) -> vec3<f32> {
    return mix(e.prev_pos, e.pos, globals.sun.w);
}

@vertex
fn vs_icon(@location(0) corner: vec2<f32>, @builtin(instance_index) instance: u32) -> IconOut {
    let e = load_entity(visible[instance]);
    let model = models[e.blueprint];
    let shape = model.icon & 0xFFu;
    var size_px = 17.0;
    if shape == 0u {
        size_px = 26.0;
    } else if (model.icon & 0x10000u) == 0u {
        size_px = 20.0;
    }
    if shape == 13u {
        size_px = 8.0;
    }
    let center = globals.view_proj * vec4<f32>(entity_center(e) + vec3<f32>(0.0, 0.0, model.height * 0.5), 1.0);
    var out: IconOut;
    let ndc = center.xy / center.w + corner * size_px * globals.viewport.zw;
    // Icons ignore depth; keep w so clipping behind the camera still works.
    out.clip = vec4<f32>(ndc * center.w, center.w * 0.5, center.w);
    out.uv = corner;
    out.icon = model.icon;
    out.owner_flags = e.owner_flags;
    out.health = e.health;
    return out;
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

// Signed distance of the symbol, in a [-1, 1] square. Negative inside.
fn icon_shape(shape: u32, p: vec2<f32>) -> f32 {
    switch shape {
        // Commander: disc inside a ring.
        case 0u: { return min(length(p) - 0.42, abs(length(p) - 0.74) - 0.09); }
        // Engineer: hollow square.
        case 1u: { return abs(sd_box(p, vec2<f32>(0.5))) - 0.13; }
        // Bot: triangle.
        case 2u: { return sd_triangle(p + vec2<f32>(0.0, 0.1), 0.62); }
        // Tank: diamond.
        case 3u: { return sd_diamond(p, 0.78); }
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
    for (var i = 0u; i < tech && i < 4u; i++) {
        let x = (f32(i) - (f32(min(tech, 4u)) - 1.0) * 0.5) * 0.3;
        d = min(d, sd_box(in.uv - vec2<f32>(x, -0.84), vec2<f32>(0.1, 0.07)));
    }
    let aa = fwidth(d) * 1.2;
    let fill = 1.0 - smoothstep(-aa, aa, d);
    let outline = 1.0 - smoothstep(-aa, aa, d - 0.16);
    if outline <= 0.01 {
        discard;
    }
    var color = globals.team_colors[in.owner_flags & 7u].rgb;
    if (in.owner_flags & FLAG_UNDER_CONSTRUCTION) != 0u {
        color = color * 0.45;
    }
    // Damaged units flash toward red.
    color = mix(vec3<f32>(1.0, 0.1, 0.05), color, smoothstep(0.0, 0.5, in.health));
    return vec4<f32>(mix(vec3<f32>(0.0), color * 1.3, fill), outline);
}

struct MarkOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) kind: u32,
    @location(2) @interpolate(flat) health: f32,
}

// Ground ring around a marked unit.
@vertex
fn vs_ring(@location(0) corner: vec2<f32>, @builtin(instance_index) instance: u32) -> MarkOut {
    let mark = marks[instance];
    let e = load_entity(mark.entity);
    let c = entity_center(e);
    // Never thinner than a few pixels, so selections stay visible when zoomed out.
    let dist = distance(c, globals.camera.xyz);
    let r = max(e.radius * 1.25, dist * 7.0 / globals.lod.x);
    let world = c + vec3<f32>(corner * r, 0.6);
    var out: MarkOut;
    out.clip = globals.view_proj * vec4<f32>(world.xy, max(world.z, terrain_height(world.xy) + 0.4), 1.0);
    out.uv = corner;
    out.kind = mark.kind;
    out.health = e.health;
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
    if in.kind == 1u {
        color = vec3<f32>(1.0, 1.0, 1.0);
    }
    return vec4<f32>(color * 1.5, band * 0.9);
}

// Health bar floating above a marked unit, constant size on screen.
@vertex
fn vs_bar(@location(0) corner: vec2<f32>, @builtin(instance_index) instance: u32) -> MarkOut {
    let mark = marks[instance];
    let e = load_entity(mark.entity);
    let model = models[e.blueprint];
    let top = entity_center(e) + vec3<f32>(0.0, 0.0, model.height * 1.15 + 1.5);
    let center = globals.view_proj * vec4<f32>(top, 1.0);
    let half = vec2<f32>(22.0, 2.5);
    var out: MarkOut;
    var ndc = center.xy / center.w + (corner * half + vec2<f32>(0.0, 10.0)) * globals.viewport.zw;
    if mark.kind != 0u {
        ndc = vec2<f32>(4.0);
    }
    out.clip = vec4<f32>(ndc * center.w, center.w * 0.5, center.w);
    out.uv = corner;
    out.kind = mark.kind;
    out.health = e.health;
    return out;
}

@fragment
fn fs_bar(in: MarkOut) -> @location(0) vec4<f32> {
    let x = in.uv.x * 0.5 + 0.5;
    let border = max(abs(in.uv.x) - 0.955, abs(in.uv.y) - 0.6);
    if border > 0.0 {
        return vec4<f32>(0.0, 0.0, 0.0, 0.85);
    }
    if x > in.health {
        return vec4<f32>(0.05, 0.05, 0.05, 0.8);
    }
    let color = mix(vec3<f32>(1.0, 0.12, 0.05), vec3<f32>(0.2, 1.0, 0.3), smoothstep(0.2, 0.7, in.health));
    return vec4<f32>(color * 1.2, 1.0);
}
