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
    // Drifts: cells 56 m along the wind and 14 m across it.
    let drift = grad_noise2_d(vec2<f32>(a * 0.25, c) + 311.0, 14.0);
    let drift_w = 1.0 - smoothstep(2.0, 7.0, px);
    // Sastrugi: cells 13 m along and 1.6 m across, in patches the wind has worked.
    let ridge = grad_noise2_d(vec2<f32>(a * 0.12, c) - 57.0, 1.6);
    let ridge_w = (1.0 - smoothstep(0.12, 0.45, px)) * smoothstep(0.35, 0.7, grad_noise2(xy + 91.0, 44.0));
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
