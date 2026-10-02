// Fog of war as drawn (renderer/fog_field.rs): the sim's on/off grid, one
// texel per FOG_CELL_M cell, turned into a field FOG_FIELD_SCALE times finer
// that the scene's shaders read through `fog_at`.
//
// Each field texel takes a Gaussian over the grid round it, so the stamped
// discs' staircase rounds off into circles, then tightens the ramp back to
// about a cell wide. The field eases towards that each frame, so an edge
// glides when a unit's disc steps a cell on a sim tick instead of jumping.
//
// x: visible now, y: explored; both 0-1.

@group(0) @binding(0) var grid: texture_2d<f32>;
@group(0) @binding(1) var field: texture_storage_2d<rgba16float, read_write>;

struct FogPush {
    // x: how far visible eases to its target this frame, y: explored (1 jumps
    // there), zw unused.
    blend: vec4<f32>,
}

var<immediate> fog: FogPush;

// Spread of the blur, in grid cells: enough to round a stamped disc's steps.
const FOG_SIGMA: f32 = 0.8;

@compute @workgroup_size(8, 8)
fn cs_fog_field(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(field);
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let cells = vec2<i32>(textureDimensions(grid));
    // This texel's centre in grid texels, whose centres sit at whole numbers.
    let at = (vec2<f32>(id.xy) + 0.5) / f32(FOG_FIELD_SCALE) - 0.5;
    let base = vec2<i32>(floor(at));
    var sum = vec2<f32>(0.0);
    var weight = 0.0;
    for (var dy = -2; dy <= 3; dy++) {
        for (var dx = -2; dx <= 3; dx++) {
            let cell = base + vec2<i32>(dx, dy);
            let d = vec2<f32>(cell) - at;
            let w = exp(-dot(d, d) / (2.0 * FOG_SIGMA * FOG_SIGMA));
            sum += textureLoad(grid, clamp(cell, vec2<i32>(0), cells - 1), 0).rg * w;
            weight += w;
        }
    }
    let goal = smoothstep(vec2<f32>(0.2), vec2<f32>(0.8), sum / weight);
    let was = textureLoad(field, vec2<i32>(id.xy)).rg;
    let now = mix(was, goal, fog.blend.xy);
    textureStore(field, vec2<i32>(id.xy), vec4<f32>(now, 0.0, 0.0));
}
