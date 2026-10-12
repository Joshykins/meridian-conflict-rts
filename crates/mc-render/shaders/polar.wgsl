// The polar ground (`Climate::Polar`, bindings.wgsl `polar_at`): what terrain.wgsl
// draws over the temperate look where the climate is polar. Snow lies bright and
// cold, bluer in its hollows and shade, carved by the wind into long drifts and,
// up close, sastrugi, glinting in the sun; what the snow leaves bare is dark rock,
// scree, gravel and moraine, with nothing green. Its ice is white-to-blue: smooth
// wind-scoured blue ice, layered cliffs fluted by meltwater, and snowed-over sea ice
// crossed by pressure ridges and refrozen leads. build.rs inserts this file after habitat.wgsl into
// shaders that contain the line `//!use polar`.

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

// Polar snow at `xy`, a pixel `px` metres across: bright white, faintly blue, bluer in
// the hollows (`concavity`); laid by the wind into long low drifts tens of metres
// along it, and up close into sastrugi, sharp ridges a metre or two apart running with
// it, strongest in the patches the wind has worked; closer still, ripples a hand
// apart. Each fades out as it shrinks toward a pixel.
fn polar_snow(xy: vec2<f32>, px: f32, concavity: f32, fine: f32) -> PolarSnow {
    let run = POLAR_DRIFT * inverseSqrt(dot(POLAR_DRIFT, POLAR_DRIFT));
    let side = vec2<f32>(-run.y, run.x);
    let a = dot(xy, run);
    let c = dot(xy, side);
    // Drifts: cells 56 m along the wind and 14 m across it, and 20 m along and 4 m
    // across; sastrugi: cells 13 m along and 1.6 m across; ripples 2 m along and 0.3 m
    // across.
    var drift = vec3<f32>(0.5, 0.0, 0.0);
    let drift_w = 1.0 - smoothstep(2.0, 7.0, px);
    if drift_w > 0.0 {
        drift = grad_noise2_d(vec2<f32>(a * 0.25, c) + 311.0, 14.0);
    }
    var dune = vec3<f32>(0.5, 0.0, 0.0);
    let dune_w = 1.0 - smoothstep(0.6, 2.0, px);
    if dune_w > 0.0 {
        dune = grad_noise2_d(vec2<f32>(a * 0.2, c) - 141.0, 4.0);
    }
    var ridge = vec3<f32>(0.5, 0.0, 0.0);
    var ridge_w = 1.0 - smoothstep(0.12, 0.5, px);
    if ridge_w > 0.0 {
        ridge_w *= mix(0.35, 1.0, smoothstep(0.35, 0.7, grad_noise2(xy + 91.0, 44.0)));
        ridge = grad_noise2_d(vec2<f32>(a * 0.12, c) - 57.0, 1.6);
    }
    var ripple = vec3<f32>(0.5, 0.0, 0.0);
    let ripple_w = 1.0 - smoothstep(0.03, 0.1, px);
    if ripple_w > 0.0 {
        ripple = grad_noise2_d(vec2<f32>(a * 0.15, c) + 19.0, 0.3);
    }
    // Each pattern's slope back on the map (the chain rule through the stretch).
    let drift_g = (run * (drift.y * 0.25) + side * drift.z) * 5.0;
    let dune_g = (run * (dune.y * 0.2) + side * dune.z) * 1.6;
    let ridge_g = (run * (ridge.y * 0.12) + side * ridge.z) * 0.5;
    let ripple_g = (run * (ripple.y * 0.15) + side * ripple.z) * 0.03;
    var out: PolarSnow;
    out.grad = drift_g * drift_w + dune_g * dune_w + ridge_g * ridge_w + ripple_g * ripple_w;
    // Wind-packed crust between the ridges is a touch duller and bluer.
    let packed = (1.0 - ridge.x) * ridge_w * 0.5 + (1.0 - drift.x) * drift_w * 0.25;
    // Fresh snow on the drifts' crests, a little greyer and bluer in their troughs.
    let crest = (drift.x - 0.5) * drift_w * 0.6 + (dune.x - 0.5) * dune_w * 0.5 + (ridge.x - 0.5) * ridge_w * 0.3;
    let white = vec3<f32>(0.91, 0.935, 0.965) * (0.97 + 0.05 * fine) * (1.0 + crest * 0.22);
    let blue = white * vec3<f32>(0.78, 0.9, 1.07);
    out.rgb = mix(white, blue, clamp(concavity * 1.8 + packed * 0.6 - crest * 0.5, 0.0, 1.0));
    return out;
}

// One size of glinting grains: cells `cell` metres across, each with one grain at its
// own place facing its own way; the grain flashes where its face mirrors the sun into
// the eye (`h` the half vector, `up` the ground's normal). About a pixel across.
fn polar_glint_level(xy: vec2<f32>, cell: f32, h: vec3<f32>, up: vec3<f32>, px: f32) -> f32 {
    let p = xy / cell;
    let i = floor(p);
    let at = vec2<f32>(hash21(i + 3.1), hash21(i + 17.9)) * 0.6 + 0.2;
    let tilt = vec2<f32>(hash21(i + 41.3), hash21(i + 77.7)) - 0.5;
    let n = normalize(up + vec3<f32>(tilt * 1.4, 0.0));
    let lit = smoothstep(0.996, 0.9993, dot(n, h));
    let d = length(p - i - at) * cell;
    return lit * (1.0 - smoothstep(px * 0.35, px * 1.0, d));
}

// Sunlight glinting off the snow's grains, toward the eye `v` from the sun `sun`, on
// ground facing `up`, a pixel `px` metres across: sparse points a pixel or so across.
// The grains lie in the world on cells of fixed sizes (powers of two metres, about
// five pixels across, the two nearest blended), so a glint stays where it lies and
// only flashes as the eye or the sun moves, never crawls.
fn polar_glints(xy: vec2<f32>, v: vec3<f32>, sun: vec3<f32>, up: vec3<f32>, px: f32) -> f32 {
    let h = normalize(v + sun);
    let lv = log2(max(px * 5.0, 0.0625));
    let l0 = floor(lv);
    let f = lv - l0;
    let g0 = polar_glint_level(xy, exp2(l0), h, up, px);
    let g1 = polar_glint_level(xy + 37.0, exp2(l0 + 1.0), h, up, px);
    return mix(g0, g1, f);
}

// Polar ice on the land (the snow layer's ice: the ice sheet's blue-ice fields, bergs,
// shelves and their walls), in place of the flowing glacier's look. It does not
// break into blocks: on gentle ground it is wind-scoured blue ice, smooth and glossy,
// streaked down the wind with clear and milky bands, the layer's snow drifted over it
// in patches with crisp edges, now and then a long thin crack; on steep ground, a
// cliff (`polar_ice_face`). `snow` is the layer's snow, `snow_rgb`/`snow_grad` how
// the snow there looks (`polar_snow`).
fn polar_ice(xy: vec2<f32>, z: f32, alt: f32, base_n: vec3<f32>, snow: f32, fine: f32, patchy: f32, px: f32,
    dz: f32, snow_rgb: vec3<f32>, snow_grad: vec2<f32>) -> IceShade {
    let run = POLAR_DRIFT * inverseSqrt(dot(POLAR_DRIFT, POLAR_DRIFT));
    let a = dot(xy, run);
    let c = dot(xy, vec2<f32>(-run.y, run.x));
    let grade = length(base_n.xy) / max(base_n.z, 0.05);
    let wall = smoothstep(0.45, 1.1, grade);

    // ---- Blue ice ----
    let band = grad_noise2(vec2<f32>(a * 0.08, c) + 71.0, 9.0) * 0.6 + grad_noise2(vec2<f32>(a * 0.06, c) - 19.0, 40.0) * 0.4;
    let clear = smoothstep(0.4, 0.75, band);
    var rgb = mix(vec3<f32>(0.66, 0.77, 0.86), vec3<f32>(0.4, 0.6, 0.77), clear);
    rgb *= 0.93 + 0.14 * fine;
    var rough = mix(0.32, 0.18, clear);
    // Long thin cracks, wandering; filled with snow, pale lines more than dark.
    let jag = vec2<f32>(grad_noise2(xy + 31.0, 24.0), grad_noise2(xy - 57.0, 24.0)) * 16.0;
    let k = grad_noise2(xy + jag - 401.0, 260.0);
    let fold = 1.0 - abs(2.0 * k - 1.0);
    let crack = smoothstep(1.0 - 0.006 - px / 260.0 * 1.5, 1.0 - 0.001, fold)
        * smoothstep(0.45, 0.6, grad_noise2(xy + 77.0, 180.0));
    rgb = mix(rgb, vec3<f32>(0.85, 0.89, 0.93), crack * 0.7);
    // The layer's snow drifted over it in broad fields, their edges drawn out a little
    // down the wind; between them, broad bare patches of blue ice.
    let warp = vec2<f32>(grad_noise2(xy + 13.0, 160.0), grad_noise2(xy - 29.0, 160.0)) * 90.0;
    let field = grad_noise2(xy + warp + 233.0, 110.0);
    let tongue = grad_noise2(vec2<f32>(a * 0.2, c) + 91.0, 12.0);
    let edge = 0.015 + min(px * 0.004, 0.12);
    let drifted = smoothstep(0.5 - edge, 0.5 + edge, snow * 0.55 + 0.28 + (field - 0.5) * 0.7 + (tongue - 0.5) * 0.12
        + (patchy - 0.5) * 0.1);
    rgb = mix(rgb, snow_rgb, drifted);
    rough = mix(rough, 0.72, drifted);

    var out: IceShade;
    out.rgb = rgb;
    out.rough = rough;
    out.bend = vec3<f32>(-snow_grad * drifted, 0.0);
    out.glow = vec3<f32>(0.004, 0.016, 0.026) * clear * (1.0 - drifted);

    // ---- Walls ----
    // Seen from both sides it could face (triplanar): no frame to turn with the wall.
    if wall > 0.0 {
        let pz = max(px, dz);
        // The lip: the ground turning flat again over the wall.
        let top = smoothstep(0.5, 0.82, base_n.z + (fine - 0.5) * 0.2);
        let wx = base_n.x * base_n.x;
        let wy = base_n.y * base_n.y;
        let s = smoothstep(0.3, 0.7, wx / max(wx + wy, 1e-4));
        var f: IceShade;
        if s > 0.0 && s < 1.0 {
            let fa = polar_ice_face(xy.y, z, alt, vec2<f32>(0.0, 1.0), pz, dz, top, fine, snow_rgb);
            let fb = polar_ice_face(xy.x, z, alt, vec2<f32>(1.0, 0.0), pz, dz, top, fine, snow_rgb);
            f.rgb = mix(fb.rgb, fa.rgb, s);
            f.rough = mix(fb.rough, fa.rough, s);
            f.bend = mix(fb.bend, fa.bend, s);
            f.glow = mix(fb.glow, fa.glow, s);
        } else if s >= 1.0 {
            f = polar_ice_face(xy.y, z, alt, vec2<f32>(0.0, 1.0), pz, dz, top, fine, snow_rgb);
        } else {
            f = polar_ice_face(xy.x, z, alt, vec2<f32>(1.0, 0.0), pz, dz, top, fine, snow_rgb);
        }
        out.rgb = mix(out.rgb, f.rgb, wall);
        out.rough = mix(out.rough, f.rough, wall);
        out.bend = mix(out.bend, f.bend, wall);
        out.glow = mix(out.glow, f.glow, wall);
    }
    return out;
}

// A polar ice cliff seen straight on (a shelf's front, a berg's side): `h` along it,
// `z` up it, `side` the way `h` runs; `pz` the pixel's size on it and `dz` its height.
// White firn and blue ice in horizontal strata of uneven thickness, bluer toward the
// sea where the ice is old and dense; fluted top to bottom by meltwater into shallow
// columns, lit on their ribs and deep blue in their recesses; glossy streaks where
// meltwater ran; snow on the lip, hanging in a ragged cornice. No dirt and no blocks.
fn polar_ice_face(h: f32, z: f32, alt: f32, side: vec2<f32>, pz: f32, dz: f32, top: f32, fine: f32,
    snow_rgb: vec3<f32>) -> IceShade {
    // Strata: layers 2-6 m thick, wavering gently along the wall.
    let lz = z + (grad_noise2(vec2<f32>(h, 0.0) - 41.0, 120.0) - 0.5) * 14.0
        + (grad_noise2(vec2<f32>(h, z * 0.3) + 9.0, 25.0) - 0.5) * 2.0;
    let t = lz / 4.5 + grad_noise2(vec2<f32>(lz * 0.13, 3.0), 1.0) * 1.2;
    let li = floor(t);
    let lf = fract(t);
    let soft = min(0.12 + dz / 3.6 * 1.5, 0.5);
    let here = hash11(li * 3.71 + 0.3);
    let next = hash11((li + 1.0) * 3.71 + 0.3);
    let tone = mix(here, next, smoothstep(1.0 - soft, 1.0, lf));
    // Firn white to glacial blue, bluer low down.
    let deep = 1.0 - smoothstep(4.0, 60.0, alt);
    let blue = clamp(0.3 + tone * 0.6 + deep * 0.3 - top * 0.35, 0.0, 1.0);
    var face = mix(vec3<f32>(0.8, 0.87, 0.93), vec3<f32>(0.32, 0.56, 0.77), blue);
    // Fluting: columns a few metres across, drawn out down the wall; a broad set and a
    // fine one, faded out as they shrink toward a pixel.
    let broad = grad_noise2_d(vec2<f32>(h, z * 0.08) + 13.0, 6.0);
    var flute = broad;
    let fine_w = 1.0 - smoothstep(0.3, 1.2, pz);
    if fine_w > 0.0 {
        let fl = grad_noise2_d(vec2<f32>(h, z * 0.06) - 77.0, 1.6);
        flute = vec3<f32>(mix(broad.x, broad.x * 0.6 + fl.x * 0.4, fine_w), broad.yz + fl.yz * 0.4 * fine_w);
    }
    let broad_w = 1.0 - smoothstep(1.5, 5.0, pz);
    let recess = (1.0 - smoothstep(0.2, 0.55, flute.x)) * broad_w;
    face = mix(face, vec3<f32>(0.14, 0.4, 0.62), recess * 0.65);
    face *= 0.9 + 0.18 * smoothstep(0.4, 0.8, flute.x) * broad_w;
    // Meltwater streaks: thin glossy lines down it.
    var streak = 0.0;
    if fine_w > 0.0 {
        streak = smoothstep(0.6, 0.75, grad_noise2(vec2<f32>(h, z * 0.1) + 211.0, 2.2)) * fine_w
            * smoothstep(0.45, 0.65, grad_noise2(vec2<f32>(h, z * 0.3) - 31.0, 12.0));
    }
    face = mix(face, face * vec3<f32>(0.86, 0.93, 1.0), streak * 0.5);
    face *= 0.95 + 0.1 * fine;
    // Snow on the lip, hanging down the wall in a ragged edge.
    let hang = smoothstep(0.35, 0.65, top + (grad_noise2(vec2<f32>(h, z * 0.5) + 5.0, 3.0) - 0.5) * 0.5);
    face = mix(face, snow_rgb, hang);
    var out: IceShade;
    out.rgb = face;
    out.rough = mix(mix(0.3, 0.12, streak), 0.72, hang);
    // The columns' ribs lean out to each side: the slope along the wall.
    out.bend = vec3<f32>(side * (-flute.y) * 3.0 * broad_w * (1.0 - hang), 0.0);
    // Light through the ice: blue in its recesses and deep layers.
    out.glow = vec3<f32>(0.006, 0.03, 0.05) * (recess * 0.7 + blue * 0.3) * (1.0 - hang);
    return out;
}

struct SeaIceShade {
    rgb: vec3<f32>,
    // The ridges' slope, as a world slope.
    grad: vec2<f32>,
    rough: f32,
}

// Thick sea ice (fast ice: the snow layer's ice on flat ground a metre or two over the
// sea), a pixel `px` metres across, with `snow` of the layer's snow on it: mostly
// snow-covered (`snow_rgb`, `snow_grad`: the polar snow there) in soft wind drifts;
// where the wind has bared it, smooth grey-blue ice, milky and clearer in broad
// mottles and streaks down the wind; pressure ridges (long lines of broken ice where
// floes were driven together) and cracks and refrozen leads of new grey ice across it
// all. Not a glacier: no crevasses, ogives or moraine; and not a mosaic of blocks.
fn polar_sea_ice(xy: vec2<f32>, px: f32, fine: f32, snow: f32, snow_rgb: vec3<f32>, snow_grad: vec2<f32>) -> SeaIceShade {
    var out: SeaIceShade;
    let warp = vec2<f32>(grad_noise2(xy + 71.0, 230.0), grad_noise2(xy - 13.0, 230.0)) * 120.0;
    let p = xy + warp;
    let run = POLAR_DRIFT * inverseSqrt(dot(POLAR_DRIFT, POLAR_DRIFT));
    let a = dot(xy, run);
    let across = dot(xy, vec2<f32>(-run.y, run.x));
    // Bare ice: grey-blue, mottled milky and clear, polished in streaks down the wind.
    let mottle = grad_noise2(p, 60.0) * 0.6 + grad_noise2(vec2<f32>(a * 0.1, across) + 9.0, 12.0) * 0.4;
    var rgb = mix(vec3<f32>(0.56, 0.7, 0.81), vec3<f32>(0.76, 0.84, 0.9), smoothstep(0.3, 0.75, mottle)) * (0.95 + 0.1 * fine);
    var rough = mix(0.25, 0.45, mottle);
    // The layer's snow over most of it: its edges the wind draws out in streaks.
    var fine_streak = 0.5;
    let fine_w = 1.0 - smoothstep(0.3, 1.2, px);
    if fine_w > 0.0 {
        fine_streak = mix(0.5, grad_noise2(vec2<f32>(a * 0.15, across) + 40.0, 2.5), fine_w);
    }
    let streak = grad_noise2(vec2<f32>(a * 0.12, across) - 17.0, 9.0) * 0.6 + fine_streak * 0.4;
    let edge = 0.02 + min(px * 0.004, 0.1);
    // Bare where the layer has no snow and the wind has swept it, in broad patches.
    let swept = grad_noise2(p - 517.0, 140.0);
    let snowed = smoothstep(0.5 - edge, 0.5 + edge, snow * 0.3 + 0.5 + (streak - 0.5) * 0.2 + (swept - 0.5) * 0.7);
    rgb = mix(rgb, snow_rgb, snowed);
    rough = mix(rough, 0.72, snowed);
    out.grad = snow_grad * snowed;
    // Pressure ridges: a few long, nearly straight lines where floes were driven
    // together, across the wind (a field stretched fivefold along them), a few metres
    // wide, broken ice up close; in stretches along each line, with gaps between.
    let along = vec2<f32>(-run.y, run.x);
    let q = vec2<f32>(dot(xy, along) * 0.2, dot(xy, run)) + warp * 0.15;
    let r = grad_noise2_d(q, 220.0);
    let fold = 1.0 - abs(2.0 * r.x - 1.0);
    // The field's slope back on the map (through the stretch): the ridge's flanks.
    let flank = along * (r.y * 0.2) + run * r.z;
    let width = 0.012 + 0.012 * grad_noise2(xy - 301.0, 120.0);
    let aa = px * 3.0 * length(flank);
    let ridge = smoothstep(1.0 - width - aa, 1.0 - width * 0.3, fold)
        * smoothstep(0.5, 0.7, grad_noise2(xy + 911.0, 400.0));
    let side = select(-1.0, 1.0, r.x > 0.5);
    out.grad -= side * flank * 2.0 * ridge * 10.0;
    let shown = 1.0 - smoothstep(0.4, 1.5, px);
    var rubble = 0.5;
    if shown > 0.0 && ridge > 0.0 {
        rubble = mix(0.5, grad_noise2(xy * 1.3 + 59.0, 2.2), shown);
    }
    // Snow-dusted rubble: white lit faces, blue shade between, pale grey-blue from afar.
    rgb = mix(rgb, mix(vec3<f32>(0.62, 0.72, 0.8), vec3<f32>(0.9, 0.93, 0.96), rubble), ridge);
    rgb = mix(rgb, vec3<f32>(0.36, 0.52, 0.64), ridge * smoothstep(0.62, 0.8, rubble) * shown * 0.5);
    rough = mix(rough, 0.6, ridge);
    // Cracks and refrozen leads: thin lines of new grey-blue ice, now and then wider,
    // jagged over a few tens of metres; drifted over in places.
    let jag = vec2<f32>(grad_noise2(xy + 31.0, 24.0), grad_noise2(xy - 57.0, 24.0)) * 18.0
        + vec2<f32>(grad_noise2(xy + 7.0, 6.0), grad_noise2(xy - 3.0, 6.0)) * 3.0;
    let c = grad_noise2((p + jag) * vec2<f32>(0.6, 1.0) - 773.0, 420.0);
    let crack_fold = 1.0 - abs(2.0 * c - 1.0);
    let lead = 0.003 + 0.01 * smoothstep(0.6, 0.85, grad_noise2(xy + 147.0, 500.0));
    let caa = px / 420.0 * 1.5;
    let crack = smoothstep(1.0 - lead - caa, 1.0 - lead * 0.2, crack_fold) * (1.0 - ridge)
        * (1.0 - 0.6 * smoothstep(0.55, 0.7, streak));
    rgb = mix(rgb, vec3<f32>(0.34, 0.44, 0.52), crack * 0.85);
    rough = mix(rough, 0.15, crack);
    out.grad *= 1.0 - crack;
    out.rgb = rgb;
    out.rough = rough;
    return out;
}
