// Waves on the shore: swell that feels the bottom, rears up, breaks, runs in as
// a line of white water and washes up the beach as a thin sheet, leaving the
// sand wet behind it. Shared by water.wgsl (the breakers and their white water)
// and terrain.wgsl (the wash on the sand). build.rs inserts this file after
// bindings.wgsl into shaders that contain the line `//!use shore`.
//
// mc-render/src/shore.rs does the same sums on the CPU, so a wave is heard where
// and when it is seen to break: a change here is made there too.
//
// Every breaker is one crest of a wave coordinate
//     P = (time + travel + lag) / SURF_PERIOD
// that reaches a point when P passes a whole number. `travel` is how long the
// swell takes from here to the waterline at the shallow-water speed sqrt(g h)
// over the local slope, so crests close up and slow as the water shoals and
// line up with the shore whatever way it faces; `lag` bends them along it.

// Bubbly foam texture, 0-1; `cover` 0-1 is how much of the pixel it should fill.
fn foam_lace(p: vec2<f32>, time: f32, pixel: f32, cover: f32) -> f32 {
    if cover <= 0.001 {
        return 0.0;
    }
    let big = grad_noise2(p + vec2<f32>(time * 0.35, -time * 0.22), 4.8);
    let mid = grad_noise2(p * 1.3 + vec2<f32>(-time * 0.3, time * 0.25) + 37.0, 1.7);
    let fine = grad_noise2(p * 1.7 + vec2<f32>(time * 0.2, time * 0.4) + 91.0, 0.55);
    // Faded by the cells as sampled (the octaves are scaled up by 1.3 and 1.7), and
    // gone well before they reach a pixel or two: sampled finer they alias into
    // rows of streaks wherever the lace is half open.
    let fine_w = smoothstep(1.5, 4.0, 0.55 / 1.7 / max(pixel, 0.001));
    let mid_w = smoothstep(1.5, 4.0, 1.7 / 1.3 / max(pixel, 0.001));
    let big_w = smoothstep(1.5, 4.0, 4.8 / max(pixel, 0.001));
    let pattern = mix(0.5, big, big_w) * 0.5 + mix(0.5, mid, mid_w) * 0.32 + mix(0.5, fine, fine_w) * 0.18;
    // Where the texture is too fine to see, fade to its average instead of flickering.
    let soft = mix(mix(0.35, 0.14, mid_w), 0.06, fine_w);
    let edge = 1.0 - cover;
    return smoothstep(edge - soft, edge + soft, pattern) * smoothstep(0.0, 0.15, cover);
}

struct Shore {
    // Metres of water over the bed; on land, minus the height of the ground above it.
    depth: f32,
    // Rise per metre of the bed, measured broadly, and the way up it (to the shore).
    slope: f32,
    up: vec2<f32>,
}

fn shore_at(xy: vec2<f32>, depth: f32) -> Shore {
    let r = SURF_SLOPE_REACH;
    let gx = terrain_height(xy + vec2<f32>(r, 0.0)) - terrain_height(xy - vec2<f32>(r, 0.0));
    let gy = terrain_height(xy + vec2<f32>(0.0, r)) - terrain_height(xy - vec2<f32>(0.0, r));
    let g = vec2<f32>(gx, gy) / (2.0 * r);
    var out: Shore;
    out.depth = depth;
    out.slope = length(g);
    out.up = g / max(out.slope, 0.0001);
    return out;
}

// The breakers' size on the water at `xy`: open coast, a reef-sheltered tropical
// shore, or a canyon lake, and bigger the harder the wind blows (`surf_wind`).
fn surf_climate(xy: vec2<f32>) -> f32 {
    // (At a climate wall one region's surf ends and the next one's begins, as their
    // seas do: within a few metres.)
    let climate = climate_within(xy, REGIONS_BLEND_M);
    var base = side_mix(1.0, SURF_TROPICAL, climate.x);
    if climate.y > 0.0 {
        base = side_mix(base, SURF_DESERT, climate.y);
    }
    return base * surf_wind(length(atmos.wind.zw));
}

// The breakers' size in a wind of `speed` m/s, 1 on a fair day's 12 m/s.
fn surf_wind(speed: f32) -> f32 {
    return clamp(0.4 + speed * SURF_WIND_GAIN, 0.6, 1.5);
}

// The bed the breakers roll in over: as far out as the real one, never steeper
// than SURF_MAX_SLOPE. Metres of water `x` metres out from the waterline.
fn surf_bed(s: Shore) -> vec2<f32> {
    let slope = clamp(s.slope, SURF_MIN_SLOPE, SURF_MAX_SLOPE);
    let out = s.depth / max(s.slope, SURF_MIN_SLOPE);
    return vec2<f32>(out * slope, slope);
}

// Seconds the swell at `depth` takes to reach the waterline over a bed rising at `slope`.
fn surf_travel(depth: f32, slope: f32) -> f32 {
    return 2.0 * sqrt(max(depth, 0.0) / 9.81) / max(slope, SURF_MIN_SLOPE);
}

// The point the breakers at `xy` are worked out for (`surf_lag`, `surf_size`): `xy`
// itself, but on a map with regions each region's surf is its own, read a long way
// off per region, so a line of breakers does not run on through a climate wall.
// (shore.rs `surf_point`.)
fn surf_point(xy: vec2<f32>) -> vec2<f32> {
    if !has_regions() {
        return xy;
    }
    return xy + f32(region_probe(xy).region) * vec2<f32>(SURF_REGION_STEP_X, SURF_REGION_STEP_Y);
}

// Seconds by which the breakers at `xy` (a `surf_point`) run ahead of the ones
// further along: slow sines only, so the CPU gets the very same number. The long terms bend the
// crests along the shore, the short ones kink them, and the terms in time let
// the gap between one wave and the next come and go by a second or so.
fn surf_lag(xy: vec2<f32>, time: f32) -> f32 {
    return 1.4 * sin(dot(xy, vec2<f32>(0.0061, 0.0023)) + time * 0.021)
        + 0.9 * sin(dot(xy, vec2<f32>(-0.0027, 0.0074)) + 1.7)
        + 0.4 * sin(dot(xy, vec2<f32>(0.0152, -0.0101)) + time * 0.05)
        + 1.1 * sin(dot(xy, vec2<f32>(0.043, 0.029)) + time * 0.09)
        + 0.7 * sin(dot(xy, vec2<f32>(-0.071, 0.052)) + 2.3)
        + 0.9 * sin(dot(xy, vec2<f32>(0.0009, 0.0013)) + time * 0.11);
}

// The size of breaker `m` at `xy` (a `surf_point`), about 1 at its biggest: the swell comes in
// uneven sets, and each wave stands up in sections along the shore, big in one
// stretch and too small to break in the next.
fn surf_size(m: f32, xy: vec2<f32>) -> f32 {
    let sets = 0.6 + 0.4 * (0.65 * sin(m * 0.93 + 0.4) + 0.35 * sin(m * 2.39 + 1.3));
    let along = 0.5 * sin(dot(xy, vec2<f32>(0.0113, 0.0041)) + m * 1.7)
        + 0.3 * sin(dot(xy, vec2<f32>(-0.021, 0.033)) + m * 2.9)
        + 0.2 * sin(dot(xy, vec2<f32>(0.061, 0.047)) + m * 4.3);
    return sets * (0.68 + 0.32 * along);
}

// How much of breaker `m` at `xy` breaks white, 0-1: a section too small for
// its water rolls in as a plain swell, a gap in the line of white.
fn surf_breaking(m: f32, xy: vec2<f32>) -> f32 {
    return smoothstep(0.22, 0.42, surf_size(m, xy));
}

// The waves at `xy` on the water.
struct Surf {
    // Added to the surface's height gradient.
    slope: vec2<f32>,
    // How much white water there is, for the foam's lace (0-1).
    foam: f32,
    // The breaking lip: solid white, no lace.
    lip: f32,
    // 0-1, how far up a rearing face this is: its thin water glows.
    face: f32,
}

fn surf(xy: vec2<f32>, s: Shore, time: f32, pixel: f32) -> Surf {
    var out: Surf;
    out.slope = vec2<f32>(0.0);
    out.foam = 0.0;
    out.lip = 0.0;
    out.face = 0.0;
    let bed = surf_bed(s);
    let depth = bed.x;
    if depth > SURF_REACH_DEPTH || depth <= 0.0 {
        return out;
    }
    let travel = surf_travel(depth, bed.y);
    // Where the breakers here are worked out for: `xy`, or its region's own.
    let at = surf_point(xy);
    let p = (time + travel + surf_lag(at, time)) / SURF_PERIOD;
    let m = round(p);
    // < 0: the crest is still to come, seaward of here; > 0: it has passed.
    let d = p - m;
    // Its height in metres on this map.
    let height = SURF_HEIGHT * surf_size(m, at) * surf_climate(xy);
    let breaks = height * SURF_BREAK_RATIO;
    // Metres between crests here: they bunch up as the water shoals.
    let spacing = sqrt(9.81 * max(depth, 0.15)) * SURF_PERIOD;
    // Up to a few pixels a crest, only the white water's average shows.
    let shown = smoothstep(1.5, 4.0, spacing * 0.08 / max(pixel, 0.001));
    // The swell grows as it feels the bottom and stands tallest where it breaks.
    let grow = 1.0 - smoothstep(breaks, SURF_REACH_DEPTH, depth);
    // 0 where it breaks, 1 at the waterline.
    let broken = clamp((breaks - depth) / max(breaks, 0.05), 0.0, 1.0);
    let unbroken = step(breaks, depth);
    // Face: steep in front (d < 0), a long gentle back. Metres of height per unit of d.
    let x = d * spacing;
    let front = max(height * 0.35 + 1.5, 2.0) * mix(1.0, 0.45, 1.0 - unbroken);
    let back = spacing * 0.28;
    var rise = 0.0;
    var face = 0.0;
    if x < 0.0 {
        let k = -x / front;
        rise = -exp(-k * k) * 2.0 * k / front;
        face = exp(-k * k * 0.5);
    } else {
        let k = x / back;
        rise = exp(-k * k) * 2.0 * k / back;
        face = exp(-k * k * 3.0);
    }
    // The bed rises along `up`: a crest seaward of here (d < 0) slopes the water
    // down toward the shore in front of it.
    // (Faded out halfway between crests, where the next wave takes over.)
    // The swell stands up in humps along its crest, not as one even ridge: each
    // wave's are its own, and the gaps between them are where it lies low.
    let humps = mix(0.25, 1.15, smoothstep(0.2, 0.8, grad_noise2(xy + vec2<f32>(m * 41.0, m * 23.0), 48.0)));
    let lift = height * 0.5 * grow * mix(1.0, 0.4, broken) * (1.0 - smoothstep(0.4, 0.5, abs(d))) * humps;
    out.slope = s.up * rise * lift * shown;
    out.face = face * grow * unbroken * shown * humps * humps;
    // Rip channels: every couple of hundred metres the bed is cut deeper, the
    // breakers stand down over it and the white water they drop drains out
    // through it in a tongue of foam reaching past the surf.
    let rip = smoothstep(0.64, 0.8, grad_noise2(xy + vec2<f32>(-431.0, 977.0), 170.0));
    // Old foam lying all over the surf zone, broken into patches: by the average
    // wave's surf zone, so it has no seam where one wave hands over to the next.
    // It reaches further out along some stretches of the shore than others.
    let reach = mix(0.55, 1.6, grad_noise2(xy + vec2<f32>(211.0, -37.0), 95.0));
    let zone = SURF_HEIGHT * surf_climate(xy) * 0.8 * SURF_BREAK_RATIO * reach * (1.0 + rip * 1.4);
    let inside = 1.0 - smoothstep(zone * 0.8, zone * 1.2, depth);
    // Patches tens of metres across where more has gathered, and bare water between.
    let drift = grad_noise2(xy + vec2<f32>(time * 0.4, -time * 0.3), 38.0);
    let lying = (0.12 + 0.3 * smoothstep(0.35, 0.75, drift) + rip * 0.25) * inside
        * (0.5 + 0.5 * clamp(1.0 - depth / zone, 0.0, 1.0));
    // Averaged over a period, from high up.
    let mean = (0.25 + 0.3 * clamp(1.0 - depth / zone, 0.0, 1.0)) * inside * surf_climate(xy) * (0.7 + 0.6 * drift);
    if depth > breaks * 1.15 {
        out.foam = mix(mean, lying, shown);
        return out;
    }
    // How much of this wave breaks here: whole sections of it roll in unbroken,
    // and over a rip it hardly breaks at all.
    let white = surf_breaking(m, at) * (1.0 - rip * 0.75);
    // The broken crest's line is ragged: it goes over sooner in one spot than the
    // next, and each wave's ragged edge is its own.
    let ragged = (soft_ripple(xy * 0.37 + vec2<f32>(m * 31.0, m * 17.0), pixel * 0.37) - 0.5) * (3.0 + height * 3.0)
        + (soft_ripple(xy * 0.11 + vec2<f32>(-m * 19.0, m * 5.0), pixel * 0.11) - 0.5) * (6.0 + height * 4.0);
    let xr = x + ragged;
    // Tufts along the crest: the white comes over in heaps, not an even rope.
    let heaps = mix(0.1, 1.2, smoothstep(0.3, 0.65, soft_ripple(xy * 0.3 + vec2<f32>(m * 13.0, -m * 7.0), pixel * 0.3)));
    // Where it breaks: the lip curls over, a band of solid white along the crest
    // just behind the front, widest the moment it goes.
    let lip_w = max(0.8 + height * 1.2, pixel);
    let at_break = 1.0 - smoothstep(0.0, 0.5, broken);
    let lip = exp(-(xr + lip_w * 0.3) * (xr + lip_w * 0.3) / (lip_w * lip_w)) * smoothstep(breaks * 1.15, breaks * 0.9, depth);
    out.lip = lip * mix(0.55, 1.0, at_break) * mix(0.6, 1.0, surf_size(m, at) - 0.3) * shown * white * heaps;
    // White water left behind the broken crest as it runs in, thinning out, torn
    // into streaks and holes as it goes.
    let age = max(d, 0.0) * SURF_PERIOD;
    let trail = exp(-age / (1.2 + 1.6 * broken)) * step(0.0, xr) * (1.0 - smoothstep(0.3, 0.5, d));
    let ahead = exp(-xr * xr / (lip_w * lip_w * 0.6)) * (1.0 - unbroken);
    let torn = mix(0.55, 1.1, soft_ripple(xy * 0.21 + vec2<f32>(m * 7.0, time * 0.15), pixel * 0.21));
    let bore = max(trail, ahead) * (1.0 - unbroken) * smoothstep(0.0, 0.25, broken + 0.2) * white * torn;
    let detail = max(bore * 0.95, lying);
    out.foam = mix(mean, detail, shown);
    return out;
}

// Small wiggles in the wash's front, faded to their mean where the pixel is too coarse.
fn soft_ripple(xy: vec2<f32>, pixel: f32) -> f32 {
    let cell = 2.2;
    return mix(0.5, grad_noise2(xy, cell), smoothstep(1.5, 4.0, cell / max(pixel, 0.001)));
}

// The wash: what the last wave to land left running up the beach at `xy`.
struct Wash {
    // A sheet of water over the sand, 0-1.
    cover: f32,
    // Its foaming front and the lace it leaves as it drains.
    foam: f32,
    // How wet the sand is.
    wet: f32,
}

fn wash(xy: vec2<f32>, s: Shore, time: f32, pixel: f32) -> Wash {
    var out: Wash;
    out.cover = 0.0;
    out.foam = 0.0;
    out.wet = 0.0;
    let slope = max(s.slope, SURF_MIN_SLOPE);
    // Metres up the beach from the waterline (negative out on the water).
    let up = -s.depth / slope;
    let at = surf_point(xy);
    let p = (time + surf_lag(at, time)) / SURF_PERIOD;
    let m = floor(p);
    let since = (p - m) * SURF_PERIOD;
    let size = surf_size(m, at) * surf_climate(xy);
    // Each wash runs up in lobes of its own, further here and less there, so its
    // front and the wet it leaves are never one clean line along the shore.
    // Over longer stretches, too: one wave runs far up one part of the beach and
    // barely wets the next.
    let lobes = (1.0 + (grad_noise2(xy + vec2<f32>(m * 37.0, m * 11.0), 11.0) - 0.5) * 0.8)
        * (0.55 + 0.9 * grad_noise2(xy + vec2<f32>(m * 53.0, -m * 29.0), 64.0));
    let reach = min(SURF_RUNUP * size, SURF_RUNUP_RISE * size / slope) * lobes;
    let damp = 1.0 + (grad_noise2(xy, 9.0) - 0.5) * 0.5;
    if up > reach * 1.3 + pixel * 2.0 || reach < 0.05 {
        return out;
    }
    let run = SURF_SWASH * SURF_PERIOD;
    let q = since / run;
    // The wash rises and falls back like a thrown thing: a parabola in time.
    let front = select(0.0, reach * 4.0 * q * (1.0 - q), q < 1.0)
        + (soft_ripple(xy + vec2<f32>(m * 5.0, 0.0), pixel) - 0.5) * min(reach * 0.3, 1.5)
        + (soft_ripple(xy * 0.23 + vec2<f32>(0.0, m * 7.0), pixel * 0.23) - 0.5) * min(reach * 0.5, 3.0);
    let soft = max(pixel * 1.2, 0.25);
    // Only up on the sand: under the water's own edge the sea is drawn over it.
    out.cover = (1.0 - smoothstep(front - soft, front + soft, up)) * smoothstep(-0.2, 0.4, up);
    // The frothing edge as it runs up; a thinning lace behind it as it drains.
    let edge_w = max(0.35 + reach * 0.05, pixel);
    // (Only while a wash is running: between washes a front resting at the
    // waterline drew one even white line all along the shore.)
    let edge = exp(-(up - front) * (up - front) / (edge_w * edge_w)) * select(0.55, 1.0, q < 0.5)
        * smoothstep(0.1, 0.6, front) * step(q, 1.0)
        // Torn: the froth rides the front in clots with bare gaps between.
        * smoothstep(0.3, 0.62, soft_ripple(xy * 0.45 + vec2<f32>(m * 11.0, time * 0.3), pixel * 0.45));
    // (None out under the water's edge, where it drew a white outline round the shore.)
    out.foam = max(edge, out.cover * max(0.35 - 0.3 * q, 0.0)) * smoothstep(-0.2, 0.4, up);
    // Sand the wash left: wettest where it has only just drained. Each spot drained
    // the moment the falling front passed it; the last wave's if this one is not up yet.
    let drained = 0.5 + 0.5 * sqrt(max(1.0 - up / max(reach, 0.01), 0.0));
    var after = since - drained * run;
    if after < 0.0 && out.cover < 0.5 {
        after += SURF_PERIOD;
    }
    let reached = 1.0 - smoothstep(reach * 0.9, reach * 1.1, up);
    out.wet = max(select(exp(-max(after, 0.0) / 3.5), 1.0, out.cover > 0.5) * reached, 0.3 * (1.0 - smoothstep(0.0, reach * 1.3 * damp, up)));
    // From high up, only its average: a pale damp band along the waterline.
    let shown = smoothstep(1.0, 3.0, reach / max(pixel, 0.001));
    out.cover *= shown;
    out.foam = mix(0.12 * reached, out.foam, shown);
    out.wet = mix(0.35 * reached, out.wet, shown);
    return out;
}
