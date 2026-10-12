// The open sea: its wave trains, each region's own sea, the weather on the water,
// the whitecaps, the view of it from far off, and how light goes through each
// climate's water. Drawn by water.wgsl, which says what the surface is made of.
// build.rs inserts this file after bindings.wgsl into shaders that contain the
// line `//!use sea`.

// ---------------------------------------------------------------- waves

// Downwind, as the longest trains run.
const SEA_WIND: vec2<f32> = vec2<f32>(0.91, 0.41);

// The sea is a sum of wave trains from a 150 m swell down to half-metre chop,
// each running at its own speed (deep water: sqrt(g k)), so crests form, pass
// through each other and break up as real water does instead of a pattern
// sliding over it. Headings fan out round the wind, the short ones most.
const SEA_TRAINS: u32 = 20u;

struct SeaWaves {
    slope: vec2<f32>,
    // The mid-scale waves' height here in units of their spread: past 2 on the steepest crests.
    crest: f32,
    // The same a second and two and a half seconds ago: a crest that broke here
    // then has left its foam lying where it was.
    crest_then: vec2<f32>,
    // The long swell's height, -1 in a trough to 1 on a crest (for the view from far off).
    swell: f32,
    // Slope variance of the trains the pixel is too coarse to show.
    lost: f32,
}

// Seconds back the foam left by a breaking crest is looked for (`crest_then`).
const SEA_FOAM_BACK: vec2<f32> = vec2<f32>(1.0, 2.5);

// `long` scales the swell (calm in the shallows), `wind` the wind waves (the sea
// state), `chop` the short waves (gusts). `pixel` is the footprint's long side,
// which filters the slope; `across` its mean width, which filters the crests the
// whitecaps break on, so seen low across the water they still break.
fn sea_waves(xy: vec2<f32>, time: f32, pixel: f32, across: f32, long: f32, wind: f32, chop: f32) -> SeaWaves {
    var out: SeaWaves;
    out.slope = vec2<f32>(0.0);
    out.lost = 0.0;
    var crest = 0.0;
    var crest_then = vec2<f32>(0.0);
    var crest_var = 0.0;
    var swell = 0.0;
    var swell_w = 0.0;
    let heading = atan2(SEA_WIND.y, SEA_WIND.x);
    // Four broad fields drifting downwind. Each train takes its strength and a
    // bend in its crests from its own blend of them: endless plane waves summed
    // tile into a lattice the eye picks out, where real trains come and go in
    // patches and their crests wander.
    let drift = SEA_WIND * time * 1.5;
    let fields = vec4<f32>(
        grad_noise2(xy - drift, 910.0),
        grad_noise2(xy.yx + vec2<f32>(311.0, -127.0) - drift * 0.7, 530.0),
        grad_noise2(xy + vec2<f32>(-743.0, 219.0) - drift * 1.3, 290.0),
        grad_noise2(xy.yx + vec2<f32>(97.0, 613.0) - drift * 0.5, 170.0),
    ) - 0.5;
    // The fields above bend a short train's crests by a fraction of a wave over
    // hundreds of them, so it still runs dead straight for dozens of metres and
    // two of them cross in a regular hatch. These wander at the scale of a few
    // wavelengths, as chop does: a coarse pair for the wind waves, finer pairs
    // for the short waves and the ripples, faded out once their cells are under
    // a few pixels.
    let wander = vec4<f32>(
        grad_noise2(xy - drift * 1.1 + vec2<f32>(57.0, 211.0), 31.0),
        grad_noise2(xy.yx - drift * 0.9 + vec2<f32>(-389.0, 43.0), 31.0),
        soft_noise(xy - drift * 1.4 + vec2<f32>(131.0, -71.0), 9.0, pixel),
        soft_noise(xy.yx - drift * 1.2 + vec2<f32>(19.0, 457.0), 9.0, pixel),
    ) - 0.5;
    let ripple = vec2<f32>(
        soft_noise(xy - drift * 1.6 + vec2<f32>(-23.0, 97.0), 3.0, pixel),
        soft_noise(xy.yx - drift * 1.5 + vec2<f32>(71.0, -13.0), 3.0, pixel),
    ) - 0.5;
    for (var i = 0u; i < SEA_TRAINS; i++) {
        let fi = f32(i);
        // Off a strict geometric series by up to 8%: exact ratios between the
        // trains' lengths line their crossings up into a repeating diamond.
        let lambda = 150.0 * pow(0.735, fi) * (1.0 + 0.16 * (fract(fi * 0.7548777 + 0.31) - 0.5));
        let k = 6.283185 / lambda;
        // Steepness (k times height): gentle swell, steeper wind waves.
        let steep = mix(0.03, 0.075, smoothstep(1.0, 9.0, fi)) * select(1.0, long, i < 6u) * select(1.0, chop, i > 11u)
            * select(1.0, wind, i >= 4u);
        // A train only a few pixels long is handed to the roughness: its sharp
        // crests carry harmonics finer than the pixels, and in the sun's glint
        // they alias into a hatch of hard lines.
        let fade = smoothstep(2.5, 8.0, lambda / max(pixel, 0.001));
        let fade_crest = smoothstep(1.5, 5.0, lambda / max(across, 0.001));
        out.lost += steep * steep * 0.3 * (1.0 - fade);
        if fade_crest <= 0.0 {
            continue;
        }
        let spread = mix(0.8, 2.3, fi / f32(SEA_TRAINS - 1u));
        // This train's blend of the fields: one for its strength, another for its bend.
        let mix_a = vec4<f32>(sin(fi * 1.7), cos(fi * 2.3), sin(fi * 0.9 + 1.0), cos(fi * 3.1 + 2.0));
        let mix_b = vec4<f32>(cos(fi * 1.3 + 0.5), sin(fi * 2.9), cos(fi * 0.7 + 2.0), sin(fi * 1.9 + 1.0));
        let lot = clamp(0.5 + dot(fields, mix_a) * 1.6, 0.0, 1.0);
        // Close to, a few trains still rule a whole screen and two of them cross
        // in diamonds: the short ones also come and go in patches a few waves across.
        let near = clamp(0.5 + dot(wander, mix_b.wzyx) * 2.4, 0.0, 1.0);
        let local = mix(1.0, mix(0.3, 1.45, near), smoothstep(30.0, 12.0, lambda));
        let strength = mix(0.2, 1.55, lot * lot * (3.0 - 2.0 * lot)) * local;
        // Cycles of wander each field may add: bounded so its gradient never
        // stretches the train's wavelength by more than about a third.
        let turn = vec2<f32>(cos(fi * 2.13 + 0.4), sin(fi * 2.13 + 0.4));
        let wobble = dot(wander.xy, turn) * min(2.0, 22.0 / lambda) + dot(wander.zw, turn.yx) * min(1.6, 7.0 / lambda)
            + dot(ripple, turn) * min(1.2, 2.4 / lambda);
        let bend = dot(fields, mix_b) * 2.2 + wobble;
        let a = heading + (fract(fi * 0.618034 + 0.13) * 2.0 - 1.0) * spread;
        let dir = vec2<f32>(cos(a), sin(a));
        // In cycles, folded before the multiply so kilometres of map and hours of
        // play keep the phase precise.
        let along = dot(dir, xy) / lambda;
        let cycles = sqrt(9.81 * k) / 6.283185 * time;
        let phase = 6.283185 * (fract(along) - fract(cycles) + fract(fi * 0.3713)) + bend * 6.283185;
        let s = sin(phase);
        // exp(sin - 1): sharp crests, broad troughs, as wind waves stand.
        let e = exp(s - 1.0);
        let amp = steep / k * fade_crest * strength;
        out.slope += dir * (amp * k * e * cos(phase)) * (fade / fade_crest);
        if lambda > 3.0 && lambda < 70.0 {
            // Weighed by steepness more than height: a crest breaks where the water
            // is steep, and weighed by height the two longest trains ruled the sum
            // and set the caps out on a diamond lattice.
            let weight = amp * pow(k, 0.7);
            crest += weight * (e - 0.466);
            crest_var += weight * weight * 0.0915;
            // Seconds ago the phase here was further on by the train's angular frequency.
            let w = sqrt(9.81 * k);
            let then_a = exp(sin(phase + w * SEA_FOAM_BACK.x) - 1.0);
            let then_b = exp(sin(phase + w * SEA_FOAM_BACK.y) - 1.0);
            crest_then += weight * (vec2<f32>(then_a, then_b) - 0.466);
        }
        if lambda >= 40.0 {
            swell += s * fade;
            swell_w += fade;
        }
    }
    out.crest = crest * inverseSqrt(max(crest_var, 1e-8));
    out.crest_then = crest_then * inverseSqrt(max(crest_var, 1e-8));
    out.swell = swell / max(swell_w, 0.001);
    return out;
}

// ---------------------------------------------------------------- a region's own sea

// On a map with regions (regions.wgsl) each region's sea is its own: its waves, its
// gusts and its whitecaps are the one field every sea has, read in a frame of the
// region's own (turned, scaled by its weather, and a long way off), so no crest,
// lane or line of foam carries on from one sea into the next across a climate
// wall. The seas hand over within the few metres either side of the wall that the
// ground does (`REGIONS_BLEND_M`).
struct SeaFrame {
    // A map point in the field: turned and scaled by `turn` (a cosine and a sine,
    // times `scale`), then moved by `shift`.
    turn: vec2<f32>,
    shift: vec2<f32>,
    // Field metres to a map metre: past 1 the waves are shorter on the map.
    scale: f32,
    // How rough the region's weather makes its water against a fair day's.
    rough: f32,
    // How open the water is to the wind: 1 the open sea, 0.55 a canyon lake, which
    // is sheltered (the wind has no fetch to raise a sea).
    open: f32,
}

// How far apart the regions' frames lie in the field.
const SEA_REGION_STEP: vec2<f32> = vec2<f32>(3301.0, -2713.0);

// The sea of a map without regions: the field as it lies.
fn sea_frame_plain() -> SeaFrame {
    return SeaFrame(vec2<f32>(1.0, 0.0), vec2<f32>(0.0), 1.0, 1.0, select(1.0, 0.55, desert()));
}

// Region `r`'s sea (`Atmosphere::region_sea`).
fn sea_frame(r: u32) -> SeaFrame {
    let sea = atmos.region_sea[r];
    return SeaFrame(sea.xy, f32(r) * SEA_REGION_STEP, length(sea.xy), sea.z, select(1.0, 0.55, globals.region_climate[r].y > 0.5));
}

// The sea `xy` is part of, all one region's: for what need not hand over at a wall.
fn sea_frame_at(xy: vec2<f32>) -> SeaFrame {
    if !has_regions() {
        return sea_frame_plain();
    }
    return sea_frame(region_probe(xy).region);
}

// `xy` in the frame's field.
fn sea_point(frame: SeaFrame, xy: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(xy.x * frame.turn.x - xy.y * frame.turn.y, xy.x * frame.turn.y + xy.y * frame.turn.x) + frame.shift;
}

// A slope of the field's waves as the map has it: turned back, and as steep as the
// field made it (a finer field's waves are lower, not steeper).
fn sea_slope(frame: SeaFrame, slope: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(slope.x * frame.turn.x + slope.y * frame.turn.y, slope.y * frame.turn.x - slope.x * frame.turn.y) / frame.scale;
}

// The weather on the water here, from the sky's weather map.
struct SeaState {
    // The wind over the water against a fair day's 12 m/s, squalls under a storm
    // included: 0.7 a light breeze, 1 fair, past 2 a gale.
    wind: f32,
    // 0-1: how much of a storm is overhead.
    storm: f32,
    // 0-1: how hard it rains.
    rain: f32,
    // How open the water is to the wind: 1 the open sea, 0.55 a canyon lake.
    open: f32,
}

// `weather` is the sky's weather over the water (`weather_at`), `frame` the sea it
// is part of.
fn sea_state(weather: vec4<f32>, frame: SeaFrame) -> SeaState {
    var out: SeaState;
    out.storm = clamp(weather.y, 0.0, 1.0);
    out.rain = clamp(weather.w, 0.0, 1.0);
    out.open = frame.open;
    out.wind = length(atmos.wind.zw) / 12.0 * (1.0 + out.storm * 0.9) * out.open * frame.rough;
    return out;
}

// What share of the open sea is breaking white in this wind, about: none in a
// calm, a scattering on a fair day, most crests in a gale (the Beaufort scale's
// whitecaps). The z-score of `SeaWaves::crest` a crest must pass to break.
fn cap_threshold(wind: f32) -> f32 {
    return mix(2.2, 0.8, smoothstep(0.4, 2.6, wind));
}

// Noise that fades to its mean as its cells get down to a few pixels, so a
// pattern never goes finer than the screen can show. `cell` must be the cell as
// sampled (no scaling of `p`), and must not change across the surface: with world
// coordinates in the thousands, a cell that drifts shifts the pattern's phase by
// hundreds of cells a metre and it aliases into stripes.
fn soft_noise(p: vec2<f32>, cell: f32, pixel: f32) -> f32 {
    let shown = smoothstep(1.5, 4.0, cell / max(pixel, 0.001));
    if shown <= 0.0 {
        return 0.5;
    }
    return mix(0.5, grad_noise2(p, cell), shown);
}

// ---------------------------------------------------------------- the sea from far off

struct FarSea {
    // Change to the water's brightness, about 0: slicks lighter, gusts darker.
    tone: f32,
    // Roughness the ruffled water adds.
    rough: f32,
    // Whitecap cover, 0..1.
    foam: f32,
}

// What the eye reads of open water from high up, where every ripple is under a
// pixel: lanes of slick and ruffled water drawn out down the wind, gusts
// darkening the water as they run across it, and whitecaps breaking and
// fading, thickest in the gusts. Everything drifts downwind fast enough to see
// moving from a strategic height.
fn far_sea(xy: vec2<f32>, time: f32, pixel: f32, sea: SeaState) -> FarSea {
    var out: FarSea;
    let wind = normalize(SEA_WIND);
    // The wind's frame: `a` downwind, `c` across it.
    let a = dot(xy, wind);
    let c = dot(xy, vec2<f32>(-wind.y, wind.x));
    // Lanes, seven times longer than wide, drifting and slowly re-forming.
    let lane_a = soft_noise(vec2<f32>((a - time * 6.0) * 0.14, c + time * 0.7), 150.0, pixel);
    let lane_b = soft_noise(vec2<f32>((a - time * 9.0) * 0.2 + 311.0, c - 97.0 - time * 0.4), 55.0, pixel);
    let ruffle = (lane_a - 0.5) * 2.6 + (lane_b - 0.5) * 1.4;
    // Gusts: patches of ruffled water running downwind faster than the lanes.
    let bend = grad_noise2(xy + vec2<f32>(517.0, -211.0), 1400.0) - 0.5;
    let g = grad_noise2(vec2<f32>(a - time * 22.0 + bend * 700.0, c * 1.5), 640.0) * 0.65
        + grad_noise2(vec2<f32>(a - time * 27.0 - 800.0, c * 1.3 + 400.0), 240.0) * 0.35;
    let gust = smoothstep(0.46, 0.64, g);
    out.tone = -ruffle * 0.26 - gust * 0.26;
    out.rough = gust * 0.012 + max(ruffle, 0.0) * 0.004;
    // Whitecaps: a cap breaks in a cell, flares and fades; cells drift with the
    // wind. Under a few pixels a cell is only its mean cover.
    let cell = 16.0;
    let p = vec2<f32>(a - time * 7.0, c) / cell;
    let id = floor(p);
    let f = p - id - 0.5;
    let h = hash21(id + vec2<f32>(17.0, -3.0));
    // In a light breeze a cap here and there in the gusts; in a gale most cells break.
    let chance = mix(0.03, 0.75, smoothstep(0.55, 2.2, sea.wind)) * mix(0.3, 1.0, max(gust, sea.storm));
    let life = fract(time / 6.5 + h * 13.7);
    let flare = smoothstep(0.0, 0.06, life) * (1.0 - smoothstep(0.06, 0.55, life));
    // A streak of spume blown out downwind of where it broke, off the cell's middle.
    let off = f - (vec2<f32>(hash21(id + 5.0), hash21(id + 9.0)) - 0.5) * 0.4;
    let trail = select(off.x * 0.45, off.x * 1.8, off.x > 0.0);
    let shape = (1.0 - smoothstep(0.03, 0.2, length(vec2<f32>(trail, off.y * 2.6)))) * 0.8;
    let cap = flare * shape * step(hash21(id - 31.0), chance);
    let shown = smoothstep(1.5, 4.0, cell / max(pixel, 0.001));
    // Unresolved, the caps and the foam they leave are a whitening of the whole sea.
    out.foam = mix(chance * 0.14, cap, shown);
    // A gale blows the spume out in long streaks down the wind.
    let streaks = smoothstep(0.58, 0.78, soft_noise(vec2<f32>((a - time * 5.0) * 0.08, c), 9.0, pixel))
        * smoothstep(1.3, 2.3, sea.wind);
    out.foam = max(out.foam, mix(smoothstep(1.3, 2.3, sea.wind) * 0.12, streaks * 0.5, shown));
    out.tone -= sea.storm * 0.15;
    return out;
}

// ---------------------------------------------------------------- the open sea

// A point of the water's surface, as `open_sea` is asked about it.
struct SeaSpot {
    xy: vec2<f32>,
    // The sky's weather over it (`weather_at`).
    weather: vec4<f32>,
    time: f32,
    // The pixel's footprint on the water, metres: its long side, which filters the
    // slope, and its mean width, for what reads as a shape rather than a glint.
    pixel: f32,
    across: f32,
    // How much of the swell the depth lets stand (it calms toward the shore, where
    // the breakers take over), and how far the shallows calm the short waves.
    amp: f32,
    calm: f32,
    // 1 where the pixel is metres across over open water: the view from a strategic
    // height, where the far sea's lanes, gusts and swell take over.
    far: f32,
}

// What one sea makes of a point of its surface: its weather there, its waves, and
// the whitecaps they break into.
struct OpenSea {
    // As `SeaState`.
    wind: f32,
    storm: f32,
    rain: f32,
    open: f32,
    // As `SeaWaves`: the surface's slope, the slope variance of the trains the pixel
    // is too coarse to show, and the long swell's height (-1 to 1).
    slope: vec2<f32>,
    lost: f32,
    swell: f32,
    // As `FarSea`, where the spot is `far`.
    far_tone: f32,
    far_rough: f32,
    far_foam: f32,
    // Whitecaps: the churned white water of a crest as it breaks, and the cover of
    // the foam it drops and the spume a gale blows from it, for the lace.
    cap: f32,
    lace: f32,
}

// The open sea at `spot` as the sea `frame` has it.
fn open_sea(spot: SeaSpot, frame: SeaFrame) -> OpenSea {
    var out: OpenSea;
    // The weather on the water: the wind raises the sea, a storm whips it up and
    // the rain pocks it.
    let sea = sea_state(spot.weather, frame);
    out.wind = sea.wind;
    out.storm = sea.storm;
    out.rain = sea.rain;
    out.open = sea.open;
    // The point and the pixel's footprint in the sea's own field.
    let xy = sea_point(frame, spot.xy);
    let pixel = spot.pixel * frame.scale;
    let across = spot.across * frame.scale;
    let time = spot.time;
    // The short waves come and go in gusts running downwind.
    let gust = grad_noise2(xy - SEA_WIND * time * 5.0 + vec2<f32>(37.0, 11.0), 120.0);
    let chop = mix(0.45, 1.35, smoothstep(0.3, 0.72, gust)) * mix(0.85, 1.1, smoothstep(0.6, 2.0, sea.wind));
    let long = spot.amp * mix(1.0, 2.0, smoothstep(0.8, 2.4, sea.wind));
    let wind_sea = mix(0.8, 1.9, smoothstep(0.5, 2.4, sea.wind));
    let waves = sea_waves(xy, time, pixel, across, long, wind_sea, chop);
    out.slope = sea_slope(frame, waves.slope);
    out.lost = waves.lost;
    out.swell = waves.swell;
    if spot.far > 0.0 {
        let far_fx = far_sea(xy, time, pixel, sea);
        out.far_tone = far_fx.tone;
        out.far_rough = far_fx.rough;
        out.far_foam = far_fx.foam;
    }
    // Whitecaps where the waves pile up steepest, more of them the harder the wind
    // blows and most in the gusts; out in open water only. A crest breaks solid
    // white, and the foam it drops lies where it broke, thinning into lace, while
    // the wave runs on. Under a few pixels a cap is only its average (`far_sea`).
    let open_water = spot.amp * (1.0 - spot.calm);
    let th = cap_threshold(sea.wind);
    let gusty = mix(0.4, 1.0, max(smoothstep(0.35, 0.75, gust), sea.storm)) * open_water;
    // (By the footprint's width: seen low across the water a cap is still metres
    // wide on screen though the pixel runs a long way into the distance.)
    let caps_shown = smoothstep(1.2, 3.5, 8.0 / max(across, 0.001));
    // A cap has a soft, broken rim: its own mask, not the lace's hard-edged one,
    // which cut caps out like paper.
    let tumble = soft_noise(xy + SEA_WIND * time * 0.8, 1.6, across) * 0.6
        + soft_noise(xy + vec2<f32>(31.0, -17.0) - SEA_WIND * time * 0.5, 0.6, across) * 0.4;
    // Only some crests go over: groups of breakers drift downwind with the waves,
    // come and go, and leave the water between them unbroken.
    let groups = smoothstep(0.42, 0.7, mix(
        grad_noise2(xy - SEA_WIND * time * 6.0, 34.0),
        grad_noise2(xy.yx + vec2<f32>(77.0, -41.0) - SEA_WIND.yx * time * 5.0, 21.0),
        0.5 + 0.5 * sin(time * 0.21)));
    let breaking = smoothstep(th - 0.3, th + 0.8, waves.crest) * (0.55 + 0.9 * tumble) * mix(0.25, 1.15, groups);
    out.cap = smoothstep(0.3, 0.95, breaking) * gusty * caps_shown;
    // The foam a crest dropped is drawn out in threads down the wind as it thins.
    let wind_a = dot(xy, SEA_WIND);
    let wind_c = dot(xy, vec2<f32>(-SEA_WIND.y, SEA_WIND.x));
    let threads = soft_noise(vec2<f32>(wind_a * 0.25, wind_c), 1.1, across);
    let dropped = max(smoothstep(th - 0.2, th + 0.6, waves.crest_then.x) * 0.55, smoothstep(th - 0.1, th + 0.6, waves.crest_then.y) * 0.4)
        * mix(0.35, 1.25, threads);
    // A gale blows the spume from the crests out in streaks down the wind.
    let spume = smoothstep(0.6, 0.8, soft_noise(vec2<f32>((wind_a - time * 4.0) * 0.08, wind_c), 2.5, pixel))
        * smoothstep(1.3, 2.3, sea.wind) * open_water;
    // What a crest drops lies as lace.
    out.lace = max(max(breaking * 0.4, dropped) * gusty * caps_shown, spume * 0.65);
    return out;
}

// `sum` with `share` of `sea` added: how the seas either side of a climate wall are mixed.
fn open_sea_add(sum: OpenSea, sea: OpenSea, share: f32) -> OpenSea {
    var out = sum;
    out.wind += sea.wind * share;
    out.storm += sea.storm * share;
    out.rain += sea.rain * share;
    out.open += sea.open * share;
    out.slope += sea.slope * share;
    out.lost += sea.lost * share;
    out.swell += sea.swell * share;
    out.far_tone += sea.far_tone * share;
    out.far_rough += sea.far_rough * share;
    out.far_foam += sea.far_foam * share;
    out.cap += sea.cap * share;
    out.lace += sea.lace * share;
    return out;
}

// The open sea at `spot`: the one sea of a map without regions; on a map with them
// the sea of the region the spot is in, handing over to the next region's within a
// few metres of a climate wall (never less than a pixel or two).
fn open_sea_at(spot: SeaSpot) -> OpenSea {
    if !has_regions() {
        return open_sea(spot, sea_frame_plain());
    }
    let shares = region_shares(spot.xy, max(REGIONS_BLEND_M, spot.pixel * 1.5));
    if !shares.mixed {
        return open_sea(spot, sea_frame(shares.region));
    }
    var out: OpenSea;
    let regions = u32(atmos.regions.y);
    for (var r = 0u; r < regions; r++) {
        if shares.w[r] > 0.0 {
            out = open_sea_add(out, open_sea(spot, sea_frame(r)), shares.w[r]);
        }
    }
    return out;
}

// ---------------------------------------------------------------- light through the water

// How light goes through this map's water: absorbed per metre, red first, and
// the colour it scatters back from a column `column` metres deep, lit by `lit`.
struct Optics {
    absorb: vec3<f32>,
    scatter: vec3<f32>,
}

// `climate` is `climate_at` where the water is: how much of the tropical (x) and the
// desert's (y) water it is, all or nothing but where two regions meet at a climate wall.
fn water_optics(column: f32, lit: f32, climate: vec2<f32>) -> Optics {
    var o: Optics;
    o.absorb = sea_absorb(climate);
    let temperate = 1.0 - climate.x - climate.y;
    var scatter = vec3<f32>(0.0);
    if climate.x > 0.0 {
        // Bahamas water: very clear, so sand shows through turquoise over the
        // banks, cyan-teal at 10-20 m, and sapphire in the deep channels.
        let deep_hue = mix(TROPIC_AZURE, TROPIC_DEEP, smoothstep(TROPIC_SCATTER_DEPTHS.y, TROPIC_SCATTER_DEPTHS.z, column));
        scatter += mix(TROPIC_SHALLOW, deep_hue, smoothstep(TROPIC_SCATTER_DEPTHS.x, TROPIC_SCATTER_DEPTHS.y, column)) * climate.x;
    }
    if climate.y > 0.0 {
        // A canyon reservoir (Lake Powell, Lake Mead): clear and very saturated,
        // jade over the pale shallows, teal-blue, then cobalt down the old channel.
        let deep_hue = mix(DESERT_TEAL, DESERT_DEEP, smoothstep(DESERT_SCATTER_DEPTHS.y, DESERT_SCATTER_DEPTHS.z, column));
        scatter += mix(DESERT_JADE, deep_hue, smoothstep(DESERT_SCATTER_DEPTHS.x, DESERT_SCATTER_DEPTHS.y, column)) * climate.y;
    }
    if temperate > 0.0 {
        // Clear, lightly green coastal water: red is gone in a few metres, and
        // the seabed reads through a dozen metres or so of it.
        scatter += mix(vec3<f32>(0.010, 0.050, 0.052), vec3<f32>(0.0045, 0.020, 0.036), smoothstep(2.0, 30.0, column)) * temperate;
    }
    o.scatter = scatter * lit;
    return o;
}

// The tropical sea (`water_optics`): absorption per metre, the colour scattered
// back over the shallows and the deep, and the depths it turns between.
const TROPIC_SHALLOW: vec3<f32> = vec3<f32>(0.012, 0.075, 0.080);
// Over the drop-off: bright azure, before the channels' sapphire.
const TROPIC_AZURE: vec3<f32> = vec3<f32>(0.004, 0.042, 0.105);
const TROPIC_DEEP: vec3<f32> = vec3<f32>(0.0015, 0.011, 0.068);
// Shallow to azure over x..y metres of water, azure to deep over y..z.
const TROPIC_SCATTER_DEPTHS: vec3<f32> = vec3<f32>(7.0, 20.0, 50.0);

// The canyon reservoir (`water_optics`): the same, greener in the shallows.
const DESERT_JADE: vec3<f32> = vec3<f32>(0.006, 0.050, 0.036);
const DESERT_TEAL: vec3<f32> = vec3<f32>(0.002, 0.026, 0.050);
const DESERT_DEEP: vec3<f32> = vec3<f32>(0.001, 0.007, 0.034);
const DESERT_SCATTER_DEPTHS: vec3<f32> = vec3<f32>(3.0, 12.0, 35.0);
