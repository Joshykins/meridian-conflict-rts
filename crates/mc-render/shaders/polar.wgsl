// The polar ground (`Climate::Polar`, bindings.wgsl `polar_at`): what terrain.wgsl
// draws over the temperate look where the climate is polar. Snow lies bright and
// cold, bluer in its hollows and shade, carved by the wind into long drifts and,
// up close, sastrugi; what the snow leaves bare is dark rock, scree, gravel and
// moraine, with nothing green. build.rs inserts this file after habitat.wgsl into
// shaders that contain the line `//!use polar`.

// Which way the prevailing wind has carved the snow. Fixed, not the sky's wind of
// the moment: a frame that turned with it would slide the drifts kilometres out.
const POLAR_DRIFT: vec2<f32> = vec2<f32>(0.91, 0.41);

// Bare ground in the polar south: dark dolerite and basalt, grey-brown gravel and
// moraine spread on the flats. `rgb` is what the temperate palette drew there: its
// light and dark is kept as the stone's grain. `rock_w` is how much of it is cliff.
fn polar_ground(rgb: vec3<f32>, h: Habitat, rock_w: f32) -> vec3<f32> {
    let lum = dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let grain = clamp(pow(lum / 0.11, 0.75), 0.45, 1.7);
    // Near black where the rock is bare, browner in broad patches.
    let rock = mix(vec3<f32>(0.052, 0.051, 0.05), vec3<f32>(0.078, 0.068, 0.058), h.broad);
    // Gravel and moraine: paler and browner, lying on the gentle ground.
    let flats = smoothstep(0.42, 0.72, h.patchy * 0.6 + h.fine * 0.4) * (1.0 - rock_w)
        * (1.0 - smoothstep(0.04, 0.14, h.slope));
    let moraine = vec3<f32>(0.125, 0.115, 0.1) * (0.85 + 0.3 * h.tussock);
    return mix(rock, moraine, flats * 0.75) * grain;
}

struct PolarSnow {
    rgb: vec3<f32>,
    // The drifts' and sastrugi's slope, as a world slope.
    grad: vec2<f32>,
}

// Polar snow at `xy`, a pixel `px` metres across: bright, faintly blue, bluer in the
// hollows (`concavity`); laid by the wind into long low drifts tens of metres along
// it, and up close into sastrugi, sharp ridges a metre or two apart running with it,
// in patches. Each fades out as it shrinks toward a pixel.
fn polar_snow(xy: vec2<f32>, px: f32, concavity: f32, fine: f32) -> PolarSnow {
    let run = POLAR_DRIFT * inverseSqrt(dot(POLAR_DRIFT, POLAR_DRIFT));
    let side = vec2<f32>(-run.y, run.x);
    let a = dot(xy, run);
    let c = dot(xy, side);
    // Drifts: cells 56 m along the wind and 14 m across it; sastrugi: cells 13 m along
    // and 1.6 m across, in patches the wind has worked. Each worked out only while it
    // is more than a pixel or so.
    var drift = vec3<f32>(0.5, 0.0, 0.0);
    let drift_w = 1.0 - smoothstep(2.0, 7.0, px);
    if drift_w > 0.0 {
        drift = grad_noise2_d(vec2<f32>(a * 0.25, c) + 311.0, 14.0);
    }
    var ridge = vec3<f32>(0.5, 0.0, 0.0);
    var ridge_w = 1.0 - smoothstep(0.12, 0.45, px);
    if ridge_w > 0.0 {
        ridge_w *= smoothstep(0.35, 0.7, grad_noise2(xy + 91.0, 44.0));
        ridge = grad_noise2_d(vec2<f32>(a * 0.12, c) - 57.0, 1.6);
    }
    // Each pattern's slope back on the map (the chain rule through the stretch).
    let drift_g = (run * (drift.y * 0.25) + side * drift.z) * 0.9;
    let ridge_g = (run * (ridge.y * 0.12) + side * ridge.z) * 0.22;
    var out: PolarSnow;
    out.grad = drift_g * drift_w + ridge_g * ridge_w;
    // Wind-packed crust between the ridges is a touch duller and bluer.
    let packed = (1.0 - ridge.x) * ridge_w * 0.5 + (1.0 - drift.x) * drift_w * 0.25;
    let white = vec3<f32>(0.80, 0.84, 0.89) * (0.96 + 0.06 * fine);
    let blue = white * vec3<f32>(0.84, 0.93, 1.07);
    out.rgb = mix(white, blue, clamp(concavity * 1.6 + packed * 0.5, 0.0, 1.0));
    return out;
}

struct SeaIceShade {
    rgb: vec3<f32>,
    // The ridges' slope, as a world slope.
    grad: vec2<f32>,
    rough: f32,
}

// Thick sea ice (fast ice: the snow layer's ice on flat ground a metre or two over the
// sea), a pixel `px` metres across, with `snow` of the layer's snow on it: pack ice,
// blocks a dozen metres across frozen together, blue-grey where bare and dusted white
// on their tops; the layer's snow lying on it in patches with crisp edges drawn out down
// the wind; pressure ridges (walls of broken blocks where floes were driven together)
// and cracks and refrozen leads across it all. Not a glacier: it does not flow, so it has
// no crevasse fields, ogives or moraine.
fn polar_sea_ice(xy: vec2<f32>, px: f32, fine: f32, snow: f32) -> SeaIceShade {
    var out: SeaIceShade;
    let warp = vec2<f32>(grad_noise2(xy + 71.0, 230.0), grad_noise2(xy - 13.0, 230.0)) * 120.0;
    let p = xy + warp;
    // The pack: blocks frozen together along darker seams.
    let jitter = vec2<f32>(grad_noise2(xy, 21.0), grad_noise2(xy + 5.0, 21.0)) - 0.5;
    let cell = ice_cells(vec2<f32>(p.x / 15.0, p.y / 12.0) + jitter * 0.7);
    let seam = crater_crack(cell.x, 0.06, px / 12.0);
    var rgb = mix(vec3<f32>(0.42, 0.56, 0.66), vec3<f32>(0.64, 0.73, 0.8), cell.y) * (0.9 + 0.15 * fine);
    let dusted = 1.0 - smoothstep(0.2, 0.5, cell.z + (cell.y - 0.5) * 0.4);
    rgb = mix(rgb, vec3<f32>(0.8, 0.84, 0.89), dusted * 0.55);
    rgb = mix(rgb, vec3<f32>(0.2, 0.33, 0.43), seam * 0.8);
    var rough = mix(0.4, 0.7, dusted);
    let blocky = 1.0 - smoothstep(0.5, 2.0, px);
    out.grad = (vec2<f32>(hash11(cell.y * 91.0), hash11(cell.y * 37.0 + 3.0)) - 0.5) * 0.3 * blocky;
    // The layer's snow: patches whose edges the wind draws out in streaks along it.
    let run = POLAR_DRIFT * inverseSqrt(dot(POLAR_DRIFT, POLAR_DRIFT));
    let a = dot(xy, run);
    let across = dot(xy, vec2<f32>(-run.y, run.x));
    var fine_streak = 0.5;
    let fine_w = 1.0 - smoothstep(0.3, 1.2, px);
    if fine_w > 0.0 {
        fine_streak = mix(0.5, grad_noise2(vec2<f32>(a * 0.15, across) + 40.0, 2.5), fine_w);
    }
    let streak = grad_noise2(vec2<f32>(a * 0.12, across) - 17.0, 9.0) * 0.6 + fine_streak * 0.4;
    let edge = 0.03 + min(px * 0.004, 0.1);
    let snowed = smoothstep(0.5 - edge, 0.5 + edge, snow + (streak - 0.5) * 0.4);
    rgb = mix(rgb, vec3<f32>(0.82, 0.86, 0.9) * (0.96 + 0.06 * fine), snowed);
    rough = mix(rough, 0.75, snowed);
    out.grad *= 1.0 - snowed;
    // Pressure ridges: lines where a ridged field peaks, a few metres wide, built of
    // broken blocks up close; strongest in stretches along each line.
    let r = grad_noise2_d(p, 170.0);
    let fold = 1.0 - abs(2.0 * r.x - 1.0);
    let width = 0.035 + 0.04 * grad_noise2(xy - 301.0, 120.0);
    let aa = px / 170.0 * 1.5;
    let ridge = smoothstep(1.0 - width - aa, 1.0 - width * 0.3, fold)
        * smoothstep(0.35, 0.6, grad_noise2(p + 911.0, 300.0));
    let side = select(-1.0, 1.0, r.x > 0.5);
    out.grad -= side * r.yz * 2.0 * ridge * 10.0;
    let shown = 1.0 - smoothstep(0.4, 1.5, px);
    var blocks = 0.5;
    if shown > 0.0 && ridge > 0.0 {
        blocks = mix(0.5, grad_noise2(xy * 1.3 + 59.0, 2.2), shown);
    }
    // Rubble: lit faces and blue shade between the blocks, grey-blue from afar.
    rgb = mix(rgb, mix(vec3<f32>(0.56, 0.65, 0.74), vec3<f32>(0.92, 0.95, 0.97), blocks), ridge);
    rgb = mix(rgb, vec3<f32>(0.3, 0.45, 0.56), ridge * smoothstep(0.62, 0.8, blocks) * shown * 0.6);
    rough = mix(rough, 0.6, ridge);
    // Cracks and refrozen leads: thin dark lines of new grey ice, once in a while wider.
    // Jagged: the line wanders over a few tens of metres, not in smooth arcs.
    let jag = vec2<f32>(grad_noise2(xy + 31.0, 24.0), grad_noise2(xy - 57.0, 24.0)) * 18.0
        + vec2<f32>(grad_noise2(xy + 7.0, 6.0), grad_noise2(xy - 3.0, 6.0)) * 3.0;
    let c = grad_noise2((p + jag) * vec2<f32>(0.6, 1.0) - 773.0, 420.0);
    let crack_fold = 1.0 - abs(2.0 * c - 1.0);
    let lead = 0.003 + 0.01 * smoothstep(0.6, 0.85, grad_noise2(xy + 147.0, 500.0));
    let caa = px / 420.0 * 1.5;
    let crack = smoothstep(1.0 - lead - caa, 1.0 - lead * 0.2, crack_fold) * (1.0 - ridge);
    rgb = mix(rgb, vec3<f32>(0.22, 0.29, 0.34), crack * 0.85);
    rough = mix(rough, 0.25, crack);
    out.rgb = rgb;
    out.rough = rough;
    return out;
}
