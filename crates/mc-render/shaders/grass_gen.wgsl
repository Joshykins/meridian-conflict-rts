//!use bindings
//!use habitat
//!use desert

// Grass, the compute half (renderer/grass.rs). Every frame, before the scene is
// drawn:
//   cs_reset          clears the draw counts;
//   cs_gather_*       lists what presses on the grass near the eye: ground units
//                     and wrecks, the structures' paved lots, scorch marks, track marks;
//   cs_trample        works out the trample map around the eye from that list: what
//                     stands on the ground, what is burnt, which way the grass is pushed
//                     now, and how flat it still lies from what went over it before;
//   cs_tufts          grows a tuft wherever the ground shows grass (habitat.wgsl), thinned
//                     with distance, and files it in a detail band for grass.wgsl to draw;
//   cs_finish         caps each band's count at its room.
// Set 1: 0 tufts, 1 draw commands and counters, 2 the trample map, 3 the press
// list, 4 the stains (scorch marks, then lots), 5 the track marks.

struct Press {
    // Mover: where, its heading's cos/sin. Lot: centre, half size. Scorch:
    // centre. Track: start, end.
    a: vec2<f32>,
    b: vec2<f32>,
    // Mover: half length, half width. Track: half gauge, one track's width.
    // Scorch: radius, strength. Lot: unused.
    r: vec2<f32>,
    // Mover: travel direction (cos/sin packed as f16), weight. Track: how fresh.
    s: f32,
    kind: u32,
}

const PRESS_MOVER: u32 = 1u;
const PRESS_LOT: u32 = 2u;
const PRESS_SCORCH: u32 = 3u;
const PRESS_TRACK: u32 = 4u;
// A boulder, ruin or other prop on the ground: centre, radius.
const PRESS_STONE: u32 = 5u;

//!rust crate::renderer::grass::GrassPush
struct GrassPush {
    // Cell (0, 0)'s corner in world metres, the cell's size, the reach from the eye.
    grid: vec4<f32>,
    // Cells across and down; scorch stains, lots after them.
    dims: vec4<u32>,
    // The trample map's origin in whole metres now, and last frame.
    window: vec4<i32>,
    // Track marks in the ring; 1 to forget the trample map; the frame.
    extra: vec4<u32>,
    // Seconds since last frame, density scale, full-density pixels per cell,
    // pixels per cell below which the grass is gone (cells of GRASS_CELL_M).
    tune: vec4<f32>,
    // The ring this dispatch grows: x its inner radius (0 from the eye out),
    // y 1 where a coarser ring takes over past `grid.w`, z the whole grass's
    // reach (it fades out toward it), w unused.
    ring: vec4<f32>,
}

@group(1) @binding(0) var<storage, read_write> tufts: array<Tuft>;
// Three DrawIndexedIndirect commands (5 words each), then the press count.
@group(1) @binding(1) var<storage, read_write> args: array<atomic<u32>, 20>;
// Per metre: x what stands, what is burnt, the push now (8 bits each, push signed);
// y how flat it lies, the way it lies (8 bits each, signed).
@group(1) @binding(2) var<storage, read_write> trample: array<vec2<u32>>;
@group(1) @binding(3) var<storage, read_write> presses: array<Press>;
@group(1) @binding(4) var<storage, read> stains: array<Stain>;
@group(1) @binding(5) var<storage, read> track_marks: array<TrackMark>;
var<immediate> push: GrassPush;


fn in_reach(xy: vec2<f32>, pad: f32) -> bool {
    let d = xy - globals.camera.xy;
    let r = push.grid.w + pad;
    return dot(d, d) < r * r;
}

fn add_press(p: Press) {
    let i = atomicAdd(&args[15], 1u);
    if i < GRASS_MAX_PRESS {
        presses[i] = p;
    }
}

@compute @workgroup_size(1)
fn cs_reset() {
    for (var b = 0u; b < 3u; b++) {
        let blades = GRASS_BAND_BLADES[b];
        let segments = GRASS_BAND_SEGMENTS[b];
        // Indices of the bands laid end to end (grass::band_indices).
        var first = 0u;
        for (var k = 0u; k < b; k++) {
            first += GRASS_BAND_BLADES[k] * (GRASS_BAND_SEGMENTS[k] * 2u - 1u) * 3u;
        }
        atomicStore(&args[b * 5u], blades * (segments * 2u - 1u) * 3u);
        atomicStore(&args[b * 5u + 1u], 0u);
        atomicStore(&args[b * 5u + 2u], first);
        atomicStore(&args[b * 5u + 3u], 0u);
        atomicStore(&args[b * 5u + 4u], GRASS_BAND_FIRST[b]);
    }
    atomicStore(&args[15], 0u);
}

// Units on the ground and wrecks lying on it.
@compute @workgroup_size(64)
fn cs_gather_units(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= globals.counts.x {
        return;
    }
    let e = dynamic_entities[id.x];
    if (e.owner_flags & (KIND_PROP | KIND_GHOST)) != 0u || (e.owner_flags & FLAG_IN_FACTORY) != 0u {
        return;
    }
    let model = models[e.blueprint];
    let wreck = (e.owner_flags & KIND_WRECK) != 0u;
    if ((model.icon >> 16u) & 1u) == 0u && !wreck {
        return;
    }
    let t = globals.sun.w;
    let pos = mix(e.prev_pos, e.pos, t);
    let r = max(e.radius, 0.5);
    if !in_reach(pos.xy, r + 4.0) {
        return;
    }
    // Aircraft push the grass through the stirred-up air (flow_at), not by
    // standing on it; a hover skirt a metre up still flattens it.
    let ground = terrain_height(pos.xy);
    if pos.z - ground > max(2.5, r * 0.35) || ground < globals.map.z {
        return;
    }
    let heading = lerp_angle(e.prev_heading, e.heading, t);
    let run = e.pos.xy - e.prev_pos.xy;
    let going = select(vec2<f32>(cos(heading), sin(heading)), run / max(length(run), 1e-4), dot(run, run) > 1e-4);
    var p: Press;
    p.a = pos.xy;
    p.b = vec2<f32>(cos(heading), sin(heading));
    // A hull is longer than it is wide; the bounds sphere is a little loose.
    p.r = vec2<f32>(r * 0.72, r * 0.48);
    // Heavier units press harder and leave the grass flat for longer.
    p.s = bitcast<f32>(pack2x16float(going));
    p.kind = PRESS_MOVER | (u32(clamp(r * 10.0, 0.0, 65535.0)) << 8u);
    add_press(p);
}

// Props that sit on the ground: boulders, ruins, the Precursor's pieces. Not
// trees, which stand up out of the grass (taller than they are wide).
@compute @workgroup_size(64)
fn cs_gather_props(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= globals.counts.y {
        return;
    }
    let e = static_entities[id.x];
    if !in_reach(e.pos.xy, 40.0) || (e.owner_flags & KIND_PROP) == 0u {
        return;
    }
    let model = models[e.blueprint];
    let scale = f32(e.packed) / 1000.0;
    let r = model.bounds_radius * scale;
    if model.height > model.bounds_radius * 1.4 || r < 0.3 || !in_reach(e.pos.xy, r + 2.0) {
        return;
    }
    var p: Press;
    p.a = e.pos.xy;
    p.b = vec2<f32>(0.0);
    p.r = vec2<f32>(r * 0.8, 0.0);
    p.s = 0.0;
    p.kind = PRESS_STONE;
    add_press(p);
}

// Scorch marks, then the structures' lots.
@compute @workgroup_size(64)
fn cs_gather_stains(@builtin(global_invocation_id) id: vec3<u32>) {
    let scorches = push.dims.z;
    if id.x >= scorches + push.dims.w {
        return;
    }
    let s = stains[id.x];
    var p: Press;
    p.a = s.pos;
    p.b = vec2<f32>(0.0);
    if id.x < scorches {
        if !in_reach(s.pos, s.radius * 1.3) {
            return;
        }
        p.r = vec2<f32>(s.radius, f32(s.strength_seed & 0xFFu) / 255.0);
        p.kind = PRESS_SCORCH;
    } else {
        if !in_reach(s.pos, s.radius * 1.5) {
            return;
        }
        p.r = vec2<f32>(s.radius, 0.0);
        p.kind = PRESS_LOT;
    }
    p.s = 0.0;
    add_press(p);
}

@compute @workgroup_size(64)
fn cs_gather_tracks(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= push.extra.x {
        return;
    }
    let m = track_marks[id.x];
    let age = (globals.camera.w - m.start) / max(m.life, 0.001);
    if age < 0.0 || age >= 1.0 {
        return;
    }
    let mid = (m.start_xy + m.end_xy) * 0.5;
    if !in_reach(mid, distance(m.start_xy, m.end_xy) * 0.5 + m.half_gauge + 2.0) {
        return;
    }
    var p: Press;
    p.a = m.start_xy;
    p.b = m.end_xy;
    p.r = vec2<f32>(m.half_gauge, m.width);
    // Crushed grass stands back up well before the ruts fade.
    p.s = 1.0 - smoothstep(0.25, 0.8, age);
    p.kind = PRESS_TRACK;
    add_press(p);
}

fn snorm8(v: f32) -> u32 {
    return u32(clamp(v * 127.5 + 127.5, 0.0, 255.0));
}

fn unorm8(v: f32) -> u32 {
    return u32(clamp(v * 255.0 + 0.5, 0.0, 255.0));
}

fn from_snorm8(v: u32) -> f32 {
    return (f32(v & 0xFFu) - 127.5) / 127.5;
}

// Distance from `p` to the segment a-b, and how far along it (0-1).
fn segment_distance(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let ab = b - a;
    let t = clamp(dot(p - a, ab) / max(dot(ab, ab), 1e-6), 0.0, 1.0);
    return distance(p, a + ab * t);
}

// The presses that reach this workgroup's 16 m square.
var<workgroup> local_press: array<u32, 256>;
var<workgroup> local_count: atomic<u32>;

// One thread per metre of the trample map, in world order: thread (x, y) is the
// cell `window.xy + (x, y)`, stored at that cell modulo GRASS_WINDOW, so the map
// scrolls with the eye without being moved.
@compute @workgroup_size(16, 16)
fn cs_trample(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(local_invocation_index) lid: u32,
    @builtin(workgroup_id) group: vec3<u32>,
) {
    if lid == 0u {
        atomicStore(&local_count, 0u);
    }
    workgroupBarrier();
    let origin = push.window.xy;
    let lo = vec2<f32>(vec2<i32>(group.xy * 16u) + origin);
    let hi = lo + vec2<f32>(16.0);
    let total = min(atomicLoad(&args[15]), GRASS_MAX_PRESS);
    for (var i = lid; i < total; i += 256u) {
        let p = presses[i];
        var reach = 0.0;
        var box_lo = p.a;
        var box_hi = p.a;
        switch p.kind & 0xFFu {
            case PRESS_MOVER: { reach = p.r.x * 1.6 + 4.0; }
            case PRESS_LOT: { reach = p.r.x + 1.5; }
            case PRESS_STONE: { reach = p.r.x + 1.0; }
            case PRESS_SCORCH: { reach = p.r.x * 1.3; }
            default: {
                box_lo = min(p.a, p.b);
                box_hi = max(p.a, p.b);
                reach = p.r.x + p.r.y + 1.0;
            }
        }
        if all(box_hi + reach >= lo) && all(box_lo - reach <= hi) {
            let k = atomicAdd(&local_count, 1u);
            if k < 256u {
                local_press[k] = i;
            }
        }
    }
    workgroupBarrier();

    let cell = vec2<i32>(id.xy) + origin;
    let slot = vec2<u32>(((cell % GRASS_WINDOW) + GRASS_WINDOW) % GRASS_WINDOW);
    let index = slot.y * u32(GRASS_WINDOW) + slot.x;
    let xy = vec2<f32>(cell) + vec2<f32>(0.5);

    // What this cell kept from last frame, if it was in last frame's window.
    let before = push.window.zw;
    let kept = push.extra.y == 0u && all(cell >= before) && all(cell < before + vec2<i32>(GRASS_WINDOW));
    var flat = 0.0;
    var lay = vec2<f32>(0.0);
    if kept {
        let old = trample[index].y;
        // Crushed grass rises again over half a minute or so.
        flat = f32(old & 0xFFu) / 255.0 * exp(-push.tune.x / 22.0);
        lay = vec2<f32>(from_snorm8(old >> 8u), from_snorm8(old >> 16u));
    }

    var stands = 0.0;
    var burnt = 0.0;
    var pushed = vec2<f32>(0.0);
    let n = min(atomicLoad(&local_count), 256u);
    for (var k = 0u; k < n; k++) {
        let p = presses[local_press[k]];
        switch p.kind & 0xFFu {
            case PRESS_MOVER: {
                // In the hull's own frame: along it, across it.
                let d = xy - p.a;
                let q = vec2<f32>(dot(d, p.b), dot(d, vec2<f32>(-p.b.y, p.b.x))) / max(p.r, vec2<f32>(0.3));
                let e = length(q);
                let weight = f32(p.kind >> 8u) / 10.0;
                let going = unpack2x16float(bitcast<u32>(p.s));
                // Under it, the grass is crushed the way it is going; round
                // it, a metre or two is shoved aside.
                let under = 1.0 - smoothstep(0.75, 1.05, e);
                let press = under * clamp(0.45 + weight * 0.12, 0.0, 1.0);
                if press > flat {
                    lay = normalize(mix(lay + vec2<f32>(1e-4, 0.0), going, press));
                    flat = press;
                }
                // The grass is parted well clear of a hull, so no unit ever wades
                // out of sight: a couple of metres, more round a big one.
                let skirt = 1.0 - smoothstep(0.0, 2.2 + p.r.y * 0.5, (e - 0.9) * min(p.r.y, 3.0));
                let away = d / max(length(d), 1e-3);
                pushed += away * skirt * (1.0 - under * 0.5);
            }
            case PRESS_LOT: {
                let d = abs(xy - p.a) - vec2<f32>(p.r.x);
                stands = max(stands, 1.0 - smoothstep(-0.6, 0.6, max(d.x, d.y)));
            }
            case PRESS_STONE: {
                let d = distance(xy, p.a) / max(p.r.x, 0.3);
                stands = max(stands, 1.0 - smoothstep(0.75, 1.15, d));
            }
            case PRESS_SCORCH: {
                let d = distance(xy, p.a);
                burnt = max(burnt, (1.0 - smoothstep(0.55, 1.1, d / max(p.r.x, 0.3))) * mix(0.55, 1.0, p.r.y));
            }
            default: {
                // Both tracks of a vehicle's run.
                let run = p.b - p.a;
                let along = run / max(length(run), 1e-4);
                let side = vec2<f32>(-along.y, along.x) * p.r.x;
                let d = min(segment_distance(xy, p.a + side, p.b + side), segment_distance(xy, p.a - side, p.b - side));
                let press = (1.0 - smoothstep(p.r.y * 0.4, p.r.y * 0.5 + 0.6, d)) * p.s * 0.9;
                if press > flat {
                    lay = normalize(mix(lay + vec2<f32>(1e-4, 0.0), along, press));
                    flat = press;
                }
            }
        }
    }
    let l = length(pushed);
    if l > 1.0 {
        pushed /= l;
    }
    trample[index] = vec2<u32>(
        unorm8(stands) | (unorm8(burnt) << 8u) | (snorm8(pushed.x) << 16u) | (snorm8(pushed.y) << 24u),
        unorm8(flat) | (snorm8(lay.x) << 8u) | (snorm8(lay.y) << 16u),
    );
}

struct Trodden {
    stands: f32,
    burnt: f32,
    pushed: vec2<f32>,
    flat: f32,
    lay: vec2<f32>,
}

fn trample_cell(cell: vec2<i32>) -> vec2<u32> {
    let origin = push.window.xy;
    if any(cell < origin) || any(cell >= origin + vec2<i32>(GRASS_WINDOW)) {
        // Nothing stands, nothing pushes, nothing lies: 128 is a signed zero.
        return vec2<u32>((128u << 16u) | (128u << 24u), (128u << 8u) | (128u << 16u));
    }
    let slot = vec2<u32>(((cell % GRASS_WINDOW) + GRASS_WINDOW) % GRASS_WINDOW);
    return trample[slot.y * u32(GRASS_WINDOW) + slot.x];
}

// The trample map at `xy`, bilinear between the four cells round it.
fn trodden_at(xy: vec2<f32>) -> Trodden {
    let p = xy - vec2<f32>(0.5);
    let c = vec2<i32>(floor(p));
    let f = p - floor(p);
    var out: Trodden;
    let wts = array<f32, 4>((1.0 - f.x) * (1.0 - f.y), f.x * (1.0 - f.y), (1.0 - f.x) * f.y, f.x * f.y);
    let offs = array<vec2<i32>, 4>(vec2<i32>(0, 0), vec2<i32>(1, 0), vec2<i32>(0, 1), vec2<i32>(1, 1));
    for (var i = 0; i < 4; i++) {
        let v = trample_cell(c + offs[i]);
        let w = wts[i];
        out.stands += f32(v.x & 0xFFu) / 255.0 * w;
        out.burnt += f32((v.x >> 8u) & 0xFFu) / 255.0 * w;
        out.pushed += vec2<f32>(from_snorm8(v.x >> 16u), from_snorm8(v.x >> 24u)) * w;
        out.flat += f32(v.y & 0xFFu) / 255.0 * w;
        out.lay += vec2<f32>(from_snorm8(v.y >> 8u), from_snorm8(v.y >> 16u)) * w;
    }
    return out;
}

// Where the air and the blasts push a tuft's tips (xy, a share of its height)
// and how hard its blades shiver (z). The prevailing wind leans it, gusts roll
// over the field downwind as waves, and a blast flattens it outward and lets it
// spring back. Shields stop both, as for the trees (entity.wgsl `tree_air`).
// w is how deep in a wind wave it stands (grass_wave), for its sheen.
fn grass_air(foot: vec3<f32>, tall: f32, seed: f32) -> vec4<f32> {
    let time = globals.camera.w;
    var lean = vec2<f32>(0.0);
    var stir = 0.0;
    var sheen = 0.0;
    let still = 1.0 - shield_shelter(foot + vec3<f32>(0.0, 0.0, 0.5));
    let field = grass_wave(foot.xy);
    let air = field.xy;
    let speed = length(air);
    if speed > 0.05 && still > 0.0 {
        let dir = air / speed;
        let gust = field.w;
        let wave = field.z;
        let storm = clamp(weather_at(foot.xy).y, 0.0, 1.0);
        // Every blade leans downwind; a wave presses it flatter as it passes.
        let steady = clamp(speed * 0.03 * (0.35 + 0.7 * gust + 0.6 * storm), 0.0, 0.6);
        let rolling = clamp(speed * 0.035 * wave * (0.6 + gust + storm), 0.0, 0.5 + 0.3 * storm);
        let side = sin(time * 1.7 + seed * 6.3) * 0.08;
        lean += (dir * (steady + rolling) + vec2<f32>(-dir.y, dir.x) * steady * side) * still;
        stir = clamp(speed * (0.5 + 0.9 * gust + wave) / 10.0, 0.0, 1.0) * still;
        sheen = wave * still;
    }
    for (var i = 0u; i < u32(globals.tree_wind.x); i++) {
        let b = globals.tree_blasts[i * 2u];
        let range = globals.tree_blasts[i * 2u + 1u].x * 1.2;
        let mid = foot + vec3<f32>(0.0, 0.0, tall * 0.5);
        let d = distance(b.xyz, mid);
        if d >= range { continue; }
        let since = time - b.w - d / max(globals.tree_blasts[i * 2u + 1u].z, 1.0);
        if since <= 0.0 || since > 3.0 { continue; }
        if effect_blocked(b.xyz, mid) { continue; }
        let away = foot.xy - b.xy;
        let dir = select(vec2<f32>(1.0, 0.0), away / max(length(away), 0.001), dot(away, away) > 0.01);
        let near = 1.0 - d / range;
        // Grass is laid flat by a blast that only rocks a tree, and is quicker
        // to spring back.
        let force = min(globals.tree_blasts[i * 2u + 1u].y * near * 0.35, 1.1);
        let shape = 1.8 * exp(-since * 2.6) * sin(since * 7.0);
        lean += dir * force * shape;
        stir = max(stir, min(force * 1.5, 1.0) * exp(-since * 1.5));
    }
    return vec4<f32>(lean, stir, sheen);
}

fn pack_unorm8x4(v: vec4<f32>) -> u32 {
    let c = vec4<u32>(clamp(v * 255.0 + 0.5, vec4<f32>(0.0), vec4<f32>(255.0)));
    return c.x | (c.y << 8u) | (c.z << 16u) | (c.w << 24u);
}

// One candidate tuft per cell of the grid round the eye.
@compute @workgroup_size(8, 8)
fn cs_tufts(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= push.dims.x || id.y >= push.dims.y {
        return;
    }
    let size = push.grid.z;
    let corner = push.grid.xy + vec2<f32>(id.xy) * size;
    // Seeded from the cell's place in the world, so a tuft stays put as the
    // grid follows the eye.
    let world_cell = vec2<i32>(floor(corner / size + vec2<f32>(0.5)));
    let seed = veg_hash(u32(world_cell.x) * 73856093u ^ veg_hash(u32(world_cell.y) * 19349663u));
    let xy = corner + vec2<f32>(veg_rand(seed), veg_rand(seed ^ 0x9E3779B9u)) * size;
    let eye = globals.camera.xyz;
    let flat_d = distance(xy, eye.xy);
    // Where one ring hands over to the next, each tuft picks its side at random
    // across 14 m, so the two grids blend with no seam.
    let dither = veg_rand(seed ^ 0x51ED270Bu) * 14.0;
    if flat_d > push.grid.w - dither * push.ring.y || (push.ring.x > 0.0 && flat_d < push.ring.x - dither) {
        return;
    }
    // Pixels a GRASS_CELL_M cell covers; a coarser ring's cell stands for
    // (size / GRASS_CELL_M)^2 of them.
    let cells = (size / GRASS_CELL_M) * (size / GRASS_CELL_M);
    // Most candidates are thinned away: test that on the flat distance (never
    // more than the true one) before reading the ground.
    let rank = veg_rand(seed ^ 0x85EBCA6Bu);
    let px_most = GRASS_CELL_M * globals.lod.x / max(flat_d, 1.0);
    if rank >= (px_most / push.tune.z) * (px_most / push.tune.z) * cells || px_most < push.tune.w {
        return;
    }
    let z = terrain_height(xy);
    if z < globals.map.z + 0.2 {
        return;
    }
    let foot = vec3<f32>(xy, z);
    let dist = max(distance(foot, eye), 1.0);
    // Thin the grass as it shrinks on screen: past the full-density distance
    // keep one tuft in `keep`, each covering the ground of 1 / keep cells.
    let px = GRASS_CELL_M * globals.lod.x / dist;
    // Seen from low down, blades stand up in front of each other and cover the
    // view with far fewer of them than from above.
    let upright = clamp((eye.z - z) / dist, 0.3, 1.0);
    let keep = clamp((px / push.tune.z) * (px / push.tune.z) * upright * cells, 0.0, 1.0);
    if rank >= keep || px < push.tune.w {
        return;
    }
    // Frustum: the tuft's bounds (side planes only, as for the props).
    let reach = size / sqrt(keep) + 1.2;
    let centre = foot + vec3<f32>(0.0, 0.0, 0.6);
    for (var i = 0; i < 4; i++) {
        let plane = globals.frustum[i];
        if dot(plane.xyz, centre) + plane.w < -reach {
            return;
        }
    }

    // Where the ground shows grass, and what kind.
    let n = terrain_normal(xy, 2.0);
    let hab = habitat(xy, z, n, 0.05);
    let grassy = grass_share(hab);
    let lush = grassy.y;
    let meadow = grassy.z;
    let moss = grassy.w;
    var ws = hab.w;
    var sum = 0.0;
    for (var i = 1; i < 10; i++) {
        sum += ws[i];
    }
    var density = grassy.x;
    // Clumps and gaps a few metres across, so a field is not a carpet.
    let clump = grad_noise2(xy + hab.warp * 0.1, 3.1) * 0.6 + grad_noise2(xy - 17.0, 7.3) * 0.4;
    // Tall meadow closes over its gaps; turf and moss grow in clumps.
    let closed = select(0.0, 0.35, meadow > lush + moss);
    density *= smoothstep(0.05, 0.45, clump + density * 0.5 + closed) * push.tune.y;
    // Canyon country's shrubs (desert.wgsl), the ones the ground paints as dark
    // dots from afar, stand up close as bushes: a dense dome of twigs.
    var shrub: CanyonShrubs;
    if desert() {
        let bushes = canyon_shrub_density(xy, canyon_bed_alt(xy, hab.alt), hab.alt, hab.slope, hab.sand_w,
            hab.canopy, hab.patchy);
        shrub = canyon_shrubs(xy, bushes, 0.02);
        if shrub.cover > 0.5 {
            density = push.tune.y;
        }
    }
    let thin = veg_rand(seed ^ 0xC2B2AE35u);
    if thin >= density {
        return;
    }
    // What stands, lies or burnt here.
    let trod = trodden_at(xy);
    if trod.stands > 0.5 {
        return;
    }
    var charred = trod.burnt;
    for (var i = 0u; i < ground_craters.count.x; i++) {
        let c = ground_craters.items[i];
        let d = distance(xy, c.at.xy) / max(c.at.z, 1.0);
        if d < 1.05 {
            return;
        }
        charred = max(charred, 1.0 - smoothstep(1.05, 1.9, d));
    }

    // The kind: picked at random in proportion to each kind's ground, so the
    // edge between two is a mix, not a line.
    let pick = veg_rand(seed ^ 0x27D4EB2Fu) * (lush + meadow + moss);
    var kind = GRASS_MOSS;
    if pick < lush {
        kind = GRASS_LUSH;
    } else if pick < lush + meadow {
        kind = GRASS_MEADOW;
    }
    if tropical() && kind != GRASS_MOSS {
        kind = GRASS_TROPICAL;
    } else if desert() {
        kind = select(GRASS_DESERT, GRASS_SHRUB, shrub.cover > 0.5);
    } else if hab.highland > 0.5 + veg_rand(seed ^ 0x165667B1u) * 0.4 {
        kind = GRASS_HIGHLAND;
    }
    var tall = 0.4;
    switch kind {
        case GRASS_LUSH: { tall = 0.34 + 0.22 * hab.sward; }
        case GRASS_MEADOW: { tall = 0.55 + 0.45 * hab.tussock; }
        case GRASS_TROPICAL: { tall = 0.6 + 0.5 * hab.sward; }
        case GRASS_HIGHLAND: { tall = 0.22 + 0.14 * hab.tussock; }
        case GRASS_DESERT: { tall = 0.3 + 0.25 * hab.tussock; }
        // As high as the bush is wide, lower toward its edge.
        case GRASS_SHRUB: { tall = shrub.size * (0.35 + 0.7 * shrub.rise); }
        default: { tall = 0.12 + 0.1 * hab.sward; }
    }
    // Faded in and out at the thinning threshold and the far end, so nothing pops.
    let fade = clamp((keep - rank) / max(keep * 0.3, 1e-4), 0.0, 1.0)
        * clamp((density - thin) / max(density * 0.3, 1e-4), 0.0, 1.0)
        * smoothstep(push.tune.w, push.tune.w * 2.2, px)
        * (1.0 - smoothstep(push.ring.z * 0.8, push.ring.z, flat_d));
    tall *= (0.75 + 0.5 * veg_rand(seed ^ 0xD3A2646Cu)) * (0.55 + 0.45 * smoothstep(0.0, 0.6, density))
        * (1.0 - charred * 0.6) * mix(0.25, 1.0, fade);

    // How it leans: the air and the blasts, what shoves it aside now, what went over it.
    let air = grass_air(foot, tall, f32(seed & 0xFFFFu) / 65535.0);
    var lean = air.xy + trod.pushed * 0.9;
    let flat = clamp(trod.flat, 0.0, 1.0);
    let lay_dir = trod.lay / max(length(trod.lay), 1e-3);
    lean = mix(lean, lay_dir * 0.97, flat * 0.9);
    let l = length(lean);
    if l > 1.1 {
        lean *= 1.1 / l;
    }

    // Light it gets: the sun past the hills (as the terrain does) and the clouds.
    var horizon = 1.0;
    let toward_sun = normalize(globals.sun.xy + vec2<f32>(1e-5, 0.0));
    let rise = globals.sun.z / max(length(globals.sun.xy), 0.1);
    for (var i = 0; i < 4; i++) {
        let r = 24.0 * exp2(f32(i));
        let obstruction = terrain_height(xy + toward_sun * r) - z - rise * r;
        horizon = min(horizon, smoothstep(-3.0, 5.0, -obstruction));
    }
    let sun = horizon * cloud_shadow(foot + vec3<f32>(0.0, 0.0, tall * 0.5));
    let sky = clamp(1.0 - hab.concavity * 0.6 - hab.canopy * 0.4, 0.3, 1.0);
    let green_part = (lush + meadow + hab.w[3]) / max(sum, 1e-3);
    let tone = ground_tone(hab, green_part);
    let dry = grass_dryness(hab);

    // The detail band: how many pixels a blade stands on screen.
    let tall_px = tall * globals.lod.x / dist;
    var band = 2u;
    if tall_px > 28.0 {
        band = 0u;
    } else if tall_px > 10.0 {
        band = 1u;
    }
    let slot = atomicAdd(&args[band * 5u + 1u], 1u);
    if slot >= GRASS_BAND_CAP[band] {
        return;
    }
    var t: Tuft;
    t.pos = foot;
    // Each kept tuft stands for a square of ground `cover` metres on a side.
    let cover = size / sqrt(keep);
    let spread = GRASS_CELL_M * 0.45 + cover * 0.35;
    t.size = pack2x16float(vec2<f32>(tall, spread));
    t.lean = pack2x16float(lean);
    t.blade = pack2x16float(vec2<f32>(cover / GRASS_CELL_M, air.z));
    t.look = (seed & 0xFFFFu) | (kind << 16u) | (unorm8(charred) << 24u);
    t.tone = pack_unorm8x4(vec4<f32>(tone * 0.5, dry));
    t.light = pack_unorm8x4(vec4<f32>(sun, sky, fade, flat));
    t.ground = pack2x16float(n.xy);
    // Far off and low down the crests squash into lines: fade the sheen on
    // how far apart they stand on screen, as the terrain does.
    let crest_px = 14.0 * globals.lod.x / dist * max((eye.z - z) / dist * 2.5, 0.02);
    t.sheen = air.w * smoothstep(10.0, 34.0, min(crest_px, 14.0 * globals.lod.x / dist));
    tufts[GRASS_BAND_FIRST[band] + slot] = t;
}

@compute @workgroup_size(1)
fn cs_finish() {
    for (var b = 0u; b < 3u; b++) {
        let n = atomicLoad(&args[b * 5u + 1u]);
        atomicStore(&args[b * 5u + 1u], min(n, GRASS_BAND_CAP[b]));
    }
}
