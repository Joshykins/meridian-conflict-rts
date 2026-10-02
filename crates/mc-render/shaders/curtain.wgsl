//!use bindings
// The climate wall's curtain: a sheet of Precursor light standing on the wall's line
// between its towers, from the ground and the sea up into the cloud. It is light
// and nothing else: armies, fleets and aircraft pass through it, and so does the
// eye. Seen square on it is a breath of cold blue with threads of brighter light
// hanging in it and faint rungs climbing it; seen along its length the threads
// stack up into a shimmering wall. One quad a stretch of the wall, drawn over the
// finished scene and the sea, adding light.

// Where the curtain ends, metres above the water, and how far down its light
// has faded to nothing by then.
const CURTAIN_TOP: f32 = 760.0;
// The light's colour: the cold blue-white of the Precursors' working parts
// (bindings.wgsl `divide_seam`).
const CURTAIN_LIGHT: vec3<f32> = vec3<f32>(0.45, 0.78, 1.0);

struct CurtainOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    // Metres along the wall from its first point.
    @location(1) along: f32,
    // The wall's direction here.
    @location(2) @interpolate(flat) dir: vec2<f32>,
}

@vertex
fn vs_curtain(@builtin(vertex_index) v: u32) -> CurtainOut {
    let stretch = v / 6u;
    let corner = v % 6u;
    var out: CurtainOut;
    let count = u32(globals.divide_info.x);
    if stretch + 1u >= count {
        out.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
        return out;
    }
    let a = globals.divide[stretch].xy;
    let b = globals.divide[stretch + 1u].xy;
    // Metres of wall before this stretch.
    var before = 0.0;
    for (var i = 0u; i < stretch; i++) {
        before += distance(globals.divide[i].xy, globals.divide[i + 1u].xy);
    }
    var t = 0.0;
    var up = 0.0;
    switch corner {
        case 1u, 4u: {
            t = 1.0;
        }
        case 2u, 3u: {
            up = 1.0;
        }
        case 5u: {
            t = 1.0;
            up = 1.0;
        }
        default: {}
    }
    let world = vec3<f32>(mix(a, b, t), globals.map.z + mix(-2.0, CURTAIN_TOP, up));
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    out.world = world;
    out.along = before + t * distance(a, b);
    out.dir = normalize(b - a);
    return out;
}

@fragment
fn fs_curtain(in: CurtainOut) -> @location(0) vec4<f32> {
    let time = globals.camera.w;
    let ground = max(terrain_height(in.world.xy), globals.map.z);
    // Metres above the ground or the sea.
    let h = in.world.z - ground;
    if h < 0.0 {
        discard;
    }
    let to_eye = globals.camera.xyz - in.world;
    let dist = max(length(to_eye), 1.0);
    // Metres a pixel covers here.
    let px = dist / max(globals.lod.x, 1.0);
    // 1 looking square through the sheet, 0 along it: the further the eye looks
    // through its light, the more of it there is.
    let square = abs(dot(to_eye.xy / dist, vec2<f32>(-in.dir.y, in.dir.x)));
    let depth = min(1.0 / (0.22 + square), 3.2);

    // The veil: thickest at the foot, thinning into the cloud.
    let rise = h / CURTAIN_TOP;
    let fade = 1.0 - smoothstep(0.55, 1.0, rise);
    let foot = exp(-h / 26.0);
    let veil = 0.050 * exp(-h / 210.0) + 0.016;

    // Threads: columns of brighter light hanging in the veil, each swaying a little,
    // lit in slow patches that drift up it. Finer than a pixel they melt into the veil.
    let sway = 2.4 * sin(h / 70.0 + time * 0.23 + in.along / 180.0);
    let column = value_noise2(vec2<f32>(in.along + sway, 3.7), 7.0);
    let fine = value_noise2(vec2<f32>(in.along * 1.9 - sway, 91.3), 2.6);
    let lit = value_noise2(vec2<f32>(in.along * 0.6, h * 0.5 - time * 14.0), 130.0);
    let threads = (pow(column, 5.0) * 1.6 + pow(fine, 7.0) * (1.0 - smoothstep(0.6, 2.0, px)))
        * smoothstep(0.25, 0.8, lit)
        * (1.0 - smoothstep(2.0, 7.0, px))
        * exp(-h / 260.0);

    // Rungs: thin level lines of light climbing the curtain, a few seconds apart.
    let climb = fract((h - time * 9.0) / 46.0);
    let rung = (1.0 - smoothstep(0.0, 0.03 + px / 46.0, min(climb, 1.0 - climb)))
        * (1.0 - smoothstep(0.6, 2.5, px))
        * exp(-h / 150.0);

    // The whole of it breathes, as the line of light at its foot does.
    let breath = 0.85 + 0.15 * sin(time * 0.75);
    let light = (veil + 0.17 * foot + 0.16 * threads + 0.05 * rung) * depth * fade * breath;
    return vec4<f32>(CURTAIN_LIGHT * light, 1.0);
}
