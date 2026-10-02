// Fog of war as drawn (renderer/fog_field.rs): a field FOG_FIELD_SCALE times
// finer than the sim's FOG_CELL_M grid, which the scene's shaders read through
// `fog_at`. x: visible now, y: explored; both 0-1.
//
// What is visible is drawn from the vision discs themselves, each round and at
// its unit's place this frame (between the tick's two positions), so an edge
// is a true circle and moves as smoothly as the unit: cs_fog_clear empties the
// cover, cs_fog_discs lays every disc into it (the most any disc gives a
// texel), and cs_fog_field eases the field towards it. Explored ground is all
// the field has ever shown; when it starts over (fog turned on, other eyes, a
// seek back) it takes the sim's explored grid, blurred round, as it stands.

//!rust crate::renderer::fog_field::FogPush
struct FogPush {
    // How far visible eases to its cover this frame (1 jumps there); how far
    // through the tick the discs are drawn; 1 to start over from the grid.
    ease: f32,
    alpha: f32,
    restart: f32,
    _pad0: f32,
    // Discs in `discs`; the field's width in texels.
    disc_count: u32,
    field_width: u32,
    _pad1: u32,
    _pad2: u32,
}

//!rust mc_sim::mirror::VisionDisc
struct VisionDisc {
    start: vec2<f32>,
    end: vec2<f32>,
    radius: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

@group(0) @binding(0) var grid: texture_2d<f32>;
@group(0) @binding(1) var field: texture_storage_2d<rgba16float, read_write>;
@group(0) @binding(2) var<storage, read> discs: array<VisionDisc>;
// Per field texel: the cover, 0 to FOG_COVER_FULL.
@group(0) @binding(3) var<storage, read_write> cover: array<atomic<u32>>;

var<immediate> fog: FogPush;

const FOG_COVER_FULL: f32 = 65535.0;
// The sim lights whole cells round a disc, so its edge lies past the unit's
// vision by about this much on average; the drawn edge's middle sits there.
const FOG_EDGE_OUT_M: f32 = 48.0;
// Half the edge's soft ramp, metres.
const FOG_EDGE_HALF_M: f32 = 40.0;
// Spread of the explored grid's blur, in grid cells.
const FOG_SIGMA: f32 = 0.8;
const FOG_DISC_THREADS: u32 = 64u;
// Dispatches across this many disc workgroups per row.
const FOG_DISC_ROW: u32 = 65535u;

fn fog_texel_m() -> f32 {
    return FOG_CELL_M / f32(FOG_FIELD_SCALE);
}

@compute @workgroup_size(8, 8)
fn cs_fog_clear(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(field);
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    atomicStore(&cover[id.y * fog.field_width + id.x], 0u);
}

// One workgroup per disc, its threads striding over the texels of its square.
@compute @workgroup_size(64)
fn cs_fog_discs(
    @builtin(workgroup_id) group: vec3<u32>,
    @builtin(local_invocation_index) lane: u32,
) {
    let i = group.y * FOG_DISC_ROW + group.x;
    if i >= fog.disc_count {
        return;
    }
    let d = discs[i];
    let centre = mix(d.start, d.end, fog.alpha);
    let edge = d.radius + FOG_EDGE_OUT_M;
    let reach = edge + FOG_EDGE_HALF_M;
    let texel = fog_texel_m();
    let size = vec2<i32>(textureDimensions(field));
    let lo = clamp(vec2<i32>(floor((centre - reach) / texel)), vec2<i32>(0), size);
    let hi = clamp(vec2<i32>(ceil((centre + reach) / texel)), vec2<i32>(0), size);
    let span = hi - lo;
    let count = u32(max(span.x, 0) * max(span.y, 0));
    for (var k = lane; k < count; k += FOG_DISC_THREADS) {
        let at = lo + vec2<i32>(i32(k % u32(span.x)), i32(k / u32(span.x)));
        let p = (vec2<f32>(at) + 0.5) * texel;
        let seen = 1.0 - smoothstep(edge - FOG_EDGE_HALF_M, edge + FOG_EDGE_HALF_M, distance(p, centre));
        if seen > 0.0 {
            atomicMax(&cover[u32(at.y) * fog.field_width + u32(at.x)], u32(seen * FOG_COVER_FULL));
        }
    }
}

// The sim's explored grid at this texel, blurred round.
fn fog_grid_explored(id: vec2<u32>) -> f32 {
    let cells = vec2<i32>(textureDimensions(grid));
    // This texel's centre in grid texels, whose centres sit at whole numbers.
    let at = (vec2<f32>(id) + 0.5) / f32(FOG_FIELD_SCALE) - 0.5;
    let base = vec2<i32>(floor(at));
    var sum = 0.0;
    var weight = 0.0;
    for (var dy = -2; dy <= 3; dy++) {
        for (var dx = -2; dx <= 3; dx++) {
            let cell = base + vec2<i32>(dx, dy);
            let d = vec2<f32>(cell) - at;
            let w = exp(-dot(d, d) / (2.0 * FOG_SIGMA * FOG_SIGMA));
            sum += textureLoad(grid, clamp(cell, vec2<i32>(0), cells - 1), 0).g * w;
            weight += w;
        }
    }
    return smoothstep(0.2, 0.8, sum / weight);
}

@compute @workgroup_size(8, 8)
fn cs_fog_field(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(field);
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let seen = f32(atomicLoad(&cover[id.y * fog.field_width + id.x])) / FOG_COVER_FULL;
    let was = textureLoad(field, vec2<i32>(id.xy)).rg;
    let visible = mix(was.x, seen, fog.ease);
    var explored = max(was.y, visible);
    if fog.restart > 0.5 {
        explored = max(fog_grid_explored(id.xy), visible);
    }
    textureStore(field, vec2<i32>(id.xy), vec4<f32>(visible, explored, 0.0, 0.0));
}
