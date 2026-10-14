// The clouds' shape (clouds.wgsl, after bindings): what the air holds over each
// spot, baked into the column map once a frame (`bake_column`), and the cloud at
// a point read from it (`cloud_at`, `cloud_shape`).
//
// Everything about a column (the weather, the day's base, how tall the cloud
// grows, how firm and grey it is, the wakes and swirls that push its billows
// about) changes over tens of metres or more. Worked out at every step of every
// ray, it was five gradient noises and a dozen lookups a sample, and again at
// each of the light's taps toward the sun: most of what the march cost. Baked,
// a sample reads it back in a few filtered fetches.

// Horizontal repeat of the billow texture and of the fine erosion, metres.
const SHAPE_PERIOD: f32 = 5200.0;
const DETAIL_PERIOD: f32 = 460.0;
// The billows' second octave, as a share of their period: no simple ratio, so
// the two never line up again within the map.
const OCTAVE_SHARE: f32 = 0.387;
// How far the billows move a column's top, as a share of its depth, peak to peak.
const TOP_LUMP: f32 = 0.6;
// How far the fine erosion moves the upper part of a column's top, likewise.
const TOP_CAULI: f32 = 0.22;
// How far round a point the storm is averaged for how tall it grows, metres.
const STORM_SPREAD: f32 = 420.0;
// Each lookup of the noise turned to its own angle (cos, sin), so no tile's
// rows run along the map's axes or along each other's.
const TURN_SHAPE: vec2<f32> = vec2<f32>(0.9394, 0.3429);
const TURN_OCTAVE: vec2<f32> = vec2<f32>(0.6118, -0.7910);
const TURN_DETAIL: vec2<f32> = vec2<f32>(0.2675, 0.9636);

fn turn(p: vec2<f32>, r: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(p.x * r.x - p.y * r.y, p.x * r.y + p.y * r.x);
}
// Extinction of the thickest cloud, per metre.
const CLOUD_SIGMA: f32 = 0.045;

// ---- How cloud droplets scatter light -------------------------------------------

// The droplets' mean diameter, micrometres (fair-weather cumulus, 10-20).
const CLOUD_DROPLET_UM: f32 = 16.0;

// Draine's phase function (Draine 2003): Henyey-Greenstein with a cos² term
// `alpha` that lifts the light thrown back and to the sides.
fn phase_draine(mu: f32, g: f32, alpha: f32) -> f32 {
    return phase_hg(mu, g) * (1.0 + alpha * mu * mu) / (1.0 + alpha * (1.0 + 2.0 * g * g) / 3.0);
}

// Light scattered by water droplets `d` micrometres across through cosine `mu`
// from straight on (Jendersie and d'Eon 2023): a Henyey-Greenstein lobe for the
// narrow bright peak round the sun, blended with Draine's for the rest, fitted to
// Mie scattering. `soften` scales both lobes' anisotropy, 1 as fitted: the light
// scattered more than once has forgotten more of the way it came.
// The share of the light `phase_mie` throws into its narrow forward peak: carried
// on almost straight, it lights the cloud deeper in, as light scattered again.
fn mie_peak(d: f32) -> f32 {
    return 1.0 - exp(-0.599085 / (d - 0.641583) - 0.665888);
}

fn phase_mie(mu: f32, d: f32, soften: f32) -> f32 {
    let g_hg = exp(-0.0990567 / (d - 1.67154)) * soften;
    let g_d = exp(-2.20679 / (d + 3.91029) - 0.428934) * soften;
    let alpha = exp(3.62489 - 8.29288 / (d + 5.52825));
    let w_d = exp(-0.599085 / (d - 0.641583) - 0.665888);
    return mix(phase_hg(mu, g_hg), phase_draine(mu, g_d, alpha), w_d);
}

// ---- The column map ------------------------------------------------------------
// `cloud_columns` (sky/columns.rs): `CLOUD_COLUMNS_LAYERS` layers over the map and a
// margin round it, and `cloud_columns_far` as many over the air far out round the map
// (the horizon a low eye looks at, tens of kilometres off), coarser. Per texel:
//   0  base (m), reach (how tall the column grows, m; 0 no cloud), how much of the
//      billows the cover fills, storm
//   1  sheet, seeded, firm, flat (1 a closed deck or a sheet)
//   2  erosion, grey, extinction scale (per metre), billow period (m)
//   3  where the billows are looked up, off the point (xy), and the erosion (zw):
//      a wheeling storm's turn, the wakes' bend, churned air's swirl
//   4  the last aircraft wake through here: its height and how much it thins the
//      cloud; how hard it rains; how far into polar country (snow, not rain)
//   5  the cloud floor (the smoothed land, m), the floor the mist lies on (m), how
//      far into autumn country (deeper mist, all day), how much mist the ground
//      holds (thinner over open sea, none in desert air)

const COLUMN_LAYERS: i32 = i32(CLOUD_COLUMNS_LAYERS);
// The near map's margin past each edge of the map, as a share of its size.
const COLUMNS_MARGIN: f32 = 0.125;
// How far past the map's edges the far map reaches, metres. Past it there is no cloud.
const COLUMNS_FAR: f32 = 40000.0;

// Where `cloud_columns` covers: x0, y0, width, height (far: `cloud_columns_far`).
fn columns_rect(far: bool) -> vec4<f32> {
    let size = atmos.weather.yz;
    var edge = size * COLUMNS_MARGIN;
    if far {
        edge = vec2<f32>(COLUMNS_FAR);
    }
    return vec4<f32>(-edge, size + edge * 2.0);
}

// A spot in the column map: its uv, and which map (0 near, 1 far, -1 past the
// far map's edge).
struct ColumnAt {
    uv: vec2<f32>,
    map: i32,
}

fn column_at(xy: vec2<f32>) -> ColumnAt {
    let near = columns_rect(false);
    let uv = (xy - near.xy) / near.zw;
    if all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0)) {
        return ColumnAt(uv, 0);
    }
    let far = columns_rect(true);
    let fuv = (xy - far.xy) / far.zw;
    if all(fuv >= vec2<f32>(0.0)) && all(fuv <= vec2<f32>(1.0)) {
        return ColumnAt(fuv, 1);
    }
    return ColumnAt(fuv, -1);
}

fn column_layer(at: ColumnAt, k: i32) -> vec4<f32> {
    if at.map == 0 {
        return textureSampleLevel(cloud_columns, clamp_sampler, at.uv, k, 0.0);
    }
    return textureSampleLevel(cloud_columns_far, clamp_sampler, at.uv, k, 0.0);
}

// Layer 0 at `at`: the base and reach that say whether a point can be in cloud at all.
fn column_base(at: ColumnAt) -> vec4<f32> {
    if at.map < 0 {
        return vec4<f32>(0.0);
    }
    var c = column_layer(at, 0);
    // The far map fades out toward its edge: no wall of cloud standing at the end.
    if at.map > 0 {
        let edge = min(min(at.uv.x, 1.0 - at.uv.x), min(at.uv.y, 1.0 - at.uv.y));
        c.y *= smoothstep(0.0, 0.08, edge);
    }
    return c;
}

// The column over map point `xy`, every layer, for `cs_columns`.
fn bake_column(xy: vec2<f32>) -> array<vec4<f32>, 6> {
    // Round a wheeling storm the billows turn with the weather (`weather_at`).
    let p = vortex_warp_in(atmos.vortex, xy);
    let w = weather_at(xy);
    let cover = w.x;
    let storm = clamp(w.y, 0.0, 1.0);
    // A Nimbus's seeded cloud (storm below zero, clouds_sim.wgsl `cs_force`): a firm
    // heap the bomber flies through the middle of (`atmos.seeded`), wherever the day's
    // bases hang; as the mark fades it settles back into the layer.
    let seeded = smoothstep(0.05, 0.5, -w.y);
    // Height varies from cloud to cloud: bases wander (some hang low enough to
    // swallow a gunship, some sit over the jets), and where the
    // air is lifting (a field drifting with the wind) cumulus grows into tall
    // towers, most of all in towering weather (`shape.x`).
    let sky = sky_at(p);
    let tall = sky.towering;
    let air = p - atmos.wind.xy;
    let lift = grad_noise2(air + 1711.0, 2600.0 * max(sky.scale, 0.5));
    // Every cloud its own: a field drifting with the wind decides whether this
    // one is a thin shred the sun shines through or a dense heap with a grey
    // belly, and storms and thick cover lean to the heavy kind. With one
    // density and one brightness everywhere every cloud was the same white
    // cotton ball.
    let kind = grad_noise2(air + vec2<f32>(2917.0, -811.0), 1700.0 * max(sky.scale, 0.5));
    // Under a full deck the thin patches and the sheet's holes close up:
    // overcast has few breaks.
    // (The weather's own cover: the map's is never over 1.)
    let deck_closed = smoothstep(1.5, 1.85, sky.cover);
    let firm = max(max(smoothstep(0.42, 0.58, kind + storm * 0.5), deck_closed * 0.8), seeded * 0.6);
    // And in wide stretches of fair weather the heaps give way to a flat,
    // broken sheet (stratocumulus): shallow, lumpy underneath, full of holes.
    let sheet = smoothstep(0.5, 0.6, grad_noise2(air + vec2<f32>(-5113.0, 3301.0), 5600.0 * max(sky.scale, 0.5))) * (1.0 - storm) * (1.0 - seeded);
    let floor = cloud_floor(p);
    let day_base = floor + atmos.layer.x - storm * 80.0 + (grad_noise2(air - 377.0, 4300.0) - 0.5) * (240.0 + 120.0 * tall);
    let base = mix(day_base, floor + atmos.seeded.x - DISTURBER_SEED_UNDER, seeded);
    let deck = atmos.layer.y - atmos.layer.x;
    let fair_top = floor + atmos.layer.y + deck * 2.6 * pow(lift, 2.2) * tall;
    // Thin cover makes shallow cloud; the fullest air the tallest.
    let fill = mix(0.55, 1.0, min(cover, 1.0));
    // Shreds are shallow as well as thin.
    let fair_depth = (fair_top - base) * fill * mix(0.7, 1.0, firm) * mix(1.0, 0.3, sheet * (1.0 - deck_closed));
    let storm_depth = (floor + sky.storm_top - base) * fill;
    // How tall a storm grows, from the storm round about: the weather map's
    // storm has sharp steps (a core's rim), and taken as it is each stood up
    // as a sheer wall whose sunlit crest ran across the storm like a tube.
    var tall_storm = storm;
    if storm > 0.01 {
        var around = 0.0;
        for (var k = 0; k < 4; k++) {
            let a = f32(k) * 1.5708 + 0.4;
            around += clamp(weather_at(xy + vec2<f32>(cos(a), sin(a)) * STORM_SPREAD).y, 0.0, 1.0);
        }
        tall_storm = (storm * 2.0 + around) / 6.0;
    }
    var reach = mix(mix(fair_depth, storm_depth, pow(tall_storm, 1.3)), DISTURBER_SEED_DEPTH, seeded);
    if cover < 0.015 {
        reach = 0.0;
    }

    // The billows drift with the wind; churned air twists them about.
    let churn = clamp(w.z, 0.0, 1.0);
    // Aircraft wakes: the flow they stirred bends the billows about (the
    // vortices twist them), and a fresh streak thins the cloud in a narrow
    // tube at the height the aircraft flew.
    let stir = flow_at(p);
    // A bend of some tens of metres at most: past that the billows shear
    // into ribbons instead of twisting.
    let bend = stir.xy * 14.0;
    let bent = bend / (1.0 + length(bend) / 70.0);
    let q = p - atmos.wind.xy - bent;
    let swirl = vec2<f32>(sin(q.y * 0.004 + atmos.weather.w * 0.3), cos(q.x * 0.004 - atmos.weather.w * 0.25)) * churn * 140.0;
    // The tube is ragged: puffs torn out along it, not a clean cut. Cut clean,
    // a flight on patrol sliced the layer into straight strips with hard edges.
    var streak = stir.w;
    if streak > 0.01 {
        streak *= smoothstep(0.3, 0.75, grad_noise2(q + vec2<f32>(stir.z * 0.7, 0.0), 140.0));
    }
    // Erode the edges: wispy at the base and in churned air, cauliflower on top.
    let erode = 0.4 + churn * 0.4 + storm * 0.15 + ((1.0 - firm) * 0.12 + sheet * 0.2) * (1.0 - deck_closed);
    // Older, wetter cloud is greyer: patches of it, more in heavy weather.
    let age = grad_noise2(air - vec2<f32>(1409.0, 2203.0), 1400.0);
    let grey = (smoothstep(0.42, 0.62, age) * 0.45 + storm * 0.25 + deck_closed * 0.15 + sheet * 0.2) * (0.4 + 0.6 * firm);
    // Full cover still keeps some of the billows' relief, so a deck or a
    // storm top reads as lumps and not a smooth sheet.
    // A sheet spreads further than heaps would in the same air.
    let filled = min(cover + sheet * 0.25, 1.0) * mix(0.9, 0.78, storm);
    // Bigger weather, bigger billows.
    let period = SHAPE_PERIOD * mix(1.0, max(sky.scale, 0.5), 0.6);
    let turned = p - xy - bent;
    // The mist (clouds.wgsl `mist_at`). Autumn karst keeps a little mist in its
    // valleys all day, wisps lying a little deeper than elsewhere.
    let fall = autumn_within(xy, REGIONS_SKY_BLEND_M);
    let land = cloud_floor(xy);
    // A hollow fills to the land round it, but no deeper than this over its floor:
    // a narrow gorge under a high smoothed floor is not hundreds of metres of fog.
    let mist_floor = min(max(land, globals.map.z), max(terrain_height(xy), globals.map.z) + MIST_POOL * (1.0 + AUTUMN_MIST_DEEPER * fall));
    // Over open sea, away from the land, it lies thinner, or the sea reads as fog;
    // desert air is too dry for it.
    let held = (1.0 - 0.6 * smoothstep(0.0, 20.0, globals.map.z - land)) * (1.0 - climate_within(xy, REGIONS_SKY_BLEND_M).y);
    return array<vec4<f32>, 6>(
        vec4<f32>(base, reach, filled, storm),
        vec4<f32>(sheet, seeded, firm, max(deck_closed, sheet)),
        vec4<f32>(erode, grey, CLOUD_SIGMA * (0.65 + 0.8 * storm) * mix(0.12, 1.0, firm), period),
        vec4<f32>(turned + swirl, turned - swirl * 2.0),
        vec4<f32>(stir.z, streak, w.w, polar_within(xy, REGIONS_SKY_BLEND_M)),
        vec4<f32>(land, mist_floor, fall, held),
    );
}

// ---- The cloud at a point ---------------------------------------------------------

struct CloudSample {
    // Extinction per metre.
    sigma: f32,
    // How far up its own column the point is, 0 base to 1 top.
    height: f32,
    storm: f32,
    // 0 a thin, ragged fragment the light goes through, 1 a dense heap.
    firm: f32,
    // How much darker than a fresh white cloud it is: old, wet, heavy cloud
    // is grey all through.
    grey: f32,
    // Metres of its own column above the point, for how much sky it hides.
    above: f32,
    // 1 in a closed deck or a flat sheet, 0 in a heap.
    flat: f32,
    // Inside the cloud's body before the erosion and the tops' cauliflower
    // carve it (1), where the march looks closely (`march`).
    body: f32,
}

// How much of the cover makes cloud at height `h` up a column (0 base, 1 top):
// flat, crisp bases; domes that narrow as they climb, so each cell rounds off
// on top; a storm towers, then spreads into an anvil.
fn cloud_profile(h: f32, sheet: f32, storm: f32, seeded: f32) -> f32 {
    let fair = mix(smoothstep(0.0, 0.08, h) * (1.0 - h * h * 0.85), smoothstep(0.0, 0.2, h) * (1.0 - smoothstep(0.55, 1.0, h)), sheet);
    let tower = smoothstep(0.0, 0.04, h) * mix(1.0 - h * 0.25, 1.05, smoothstep(0.72, 0.9, h)) * (1.0 - smoothstep(0.88, 1.0, h));
    // A seeded cloud is a lumpy slab round the bomber's height, not a dome: drawn as a
    // dome its gathered disc rose to a smooth cone.
    let slab = smoothstep(0.0, 0.15, h) * (1.0 - smoothstep(0.5, 1.0, h));
    return mix(mix(fair, tower, storm), slab, seeded);
}

// The cloud at `p`; see `cloud_shape`.
fn cloud_at(p: vec3<f32>, detail: f32) -> CloudSample {
    let at = column_at(p.xy);
    return cloud_shape(p, at, column_base(at), detail, true);
}

// The cloud at `p` in the column `at`, whose layer 0 is `c0` (`column_base`): a
// sample a few metres off its own column (the light's short tap in `march`)
// takes its shape from the same one. `detail` 0 skips the fine erosion (light
// rays); otherwise it is how many times its usual size the erosion is drawn, so
// seen from far off it stays a few march texels across instead of shimmering or
// being dropped (which left the whole strategic view soft, rounded blobs).
// `grey` says how grey the cloud is (only the drawn sample needs it). The
// clearing is applied by the caller, to how opaque the cloud is, never to its shape.
fn cloud_shape(p: vec3<f32>, at: ColumnAt, c0: vec4<f32>, detail: f32, grey: bool) -> CloudSample {
    var out: CloudSample;
    let base = c0.x;
    let reach = c0.y;
    if reach <= 0.0 || p.z <= base || p.z >= base + reach * (1.0 + TOP_LUMP * 0.5) {
        return out;
    }
    let storm = c0.w;
    let c1 = column_layer(at, 1);
    let c2 = column_layer(at, 2);
    let c3 = column_layer(at, 3);
    let sheet = c1.x;
    let seeded = c1.y;
    let firm = c1.z;
    out.storm = storm;
    // The billows drift with the wind, turned and bent about (`bake_column`).
    let xy = p.xy - atmos.wind.xy;
    let bq = xy + c3.xy;
    let period = c2.w;
    // Two lookups of the billows at unrelated sizes and angles. One alone
    // repeated every period across the map, the same lumps over and over, and
    // where cover was thin (the tops) only its cells' ridges were left: the
    // same smooth rubbery tubes everywhere.
    let n = textureSampleLevel(cloud_noise, repeat_sampler, vec3<f32>(turn(bq, TURN_SHAPE) / period, p.z / (period * 0.25)), 0.0);
    let op = period * OCTAVE_SHARE;
    // The second is one flat slice, the same at every height: it also sets
    // how tall each column grows, and a height taken from noise that changed
    // with height folded tops over into stacked shelves. The light's coarse
    // taps (`detail` 0) make do with the first.
    var m = n;
    if detail > 0.0 {
        m = textureSampleLevel(cloud_noise, repeat_sampler, vec3<f32>(turn(bq, TURN_OCTAVE) / op + 0.37, 0.61), 0.0);
    }
    let billow = n.x * 0.55 + n.y * 0.15 + m.x * 0.3;
    // Every top rides up and down with the billows (the flat slice, so a top
    // never folds over). Taken from the weather map alone, a top was that
    // map's smooth field drawn in height: smooth rubbery sausages.
    let h0 = (p.z - base) / (max(reach, 1.0) * (1.0 + (m.x - 0.5) * TOP_LUMP));
    if h0 >= 1.0 + TOP_CAULI * 0.5 {
        return out;
    }
    let cauli = TOP_CAULI * smoothstep(0.35, 0.85, h0);
    // The body (`CloudSample::body`): the billows' cloud with the top as high as
    // the cauliflower can lift it, before the erosion eats into it.
    let h_body = h0 - 0.5 * cauli;
    if h_body < 1.0 && billow > 1.0 - c0.z * cloud_profile(h_body, sheet, storm, seeded) {
        out.body = 1.0;
    }
    // The fine erosion, pushed about by the billows' own lumps so its tile (a
    // few hundred metres) never shows as the same cauliflower over and over.
    var fine = 0.5;
    if detail > 0.0 {
        let dp = DETAIL_PERIOD * detail;
        let warp = (vec2<f32>(m.y, n.z) - 0.5) * dp * 0.9;
        let e = textureSampleLevel(cloud_noise, repeat_sampler,
            vec3<f32>(turn(xy + c3.zw + warp, TURN_DETAIL) / dp, p.z / (dp * 0.6)) + vec3<f32>(0.0, 0.0, atmos.weather.w * 0.004), 0.0);
        fine = e.y * 0.625 + e.z * 0.25 + e.w * 0.125;
    }
    // Up top it moves the top itself: turrets and cauliflower. Eroding only
    // the cloud's thin fringe left a storm's top smooth, as a storm is opaque
    // well inside that fringe, and a low sun drew its gentle swells as tubes.
    let h = h0 + (0.5 - fine) * cauli;
    if h >= 1.0 {
        return out;
    }
    out.height = h;
    let c = c0.z * cloud_profile(h, sheet, storm, seeded);
    var dens = clamp((billow - (1.0 - c)) / max(c, 0.12), 0.0, 1.0);
    if dens <= 0.0 {
        return out;
    }
    if detail > 0.0 {
        // Erode the edges: wispy at the base and in churned air, cauliflower on top.
        let wisp = mix(fine, 1.0 - fine, smoothstep(0.1, 0.5, h));
        let erode = c2.x;
        dens = clamp((dens - wisp * erode) / (1.0 - wisp * erode), 0.0, 1.0);
    }
    // A heap is firm inside, so it ends at an edge instead of a wide thin
    // fringe; a shred stays soft and see-through. Squared (twice as steep at
    // the edge) the fringe still read as fuzz round every heap.
    let thin = 1.0 - dens;
    dens = mix(dens, 1.0 - thin * thin * thin * thin, firm);
    // A fresh wake thins the cloud in a tube, tight below the aircraft and rolling
    // up a little above it. Cut tall (it once reached 650 m up), a flight on
    // patrol sliced the layer into strips.
    let c4 = column_layer(at, 4);
    if c4.y > 0.01 {
        let above = p.z - c4.x;
        dens *= 1.0 - 0.55 * c4.y * exp(-pow(abs(above / select(45.0, 180.0, above > 0.0)), 2.0));
    }
    out.sigma = dens * c2.z;
    out.firm = firm;
    out.above = (1.0 - h) * reach;
    out.flat = c1.w;
    if detail > 0.0 && grey {
        out.grey = c2.y;
    }
    return out;
}

// ---- The light volume ------------------------------------------------------------
// `cloud_light` (sky/light.rs, baked by clouds.wgsl `cs_light`): over the near column
// map's ground, slices up the layer from under the lowest base to over the highest
// storm top, closer together low down where fair-weather cloud is (a slice's height
// goes as the square of its number); rgb how much sunlight gets through from there
// (`sun_taps`), at each of the march's three octaves of scattered light
// (`LIGHT_OCTAVES`), past the first `SUN_NEAR` metres, which `sun_through` taps
// itself. Looked up again for every lit sample of every ray, the taps were three
// quarters of the march.
//
// The light got through, not the cloud in the way, is what is kept: blended between
// texels, the cloud in the way from a sunlit face (none) and from inside (much)
// averaged to half, which let no light through, and every top went grey.

// How far under the cloud base the volume starts, metres.
const LIGHT_BELOW: f32 = 300.0;
// How far toward the sun the light is tapped at the sample itself, metres: the
// light changes fastest across a cloud's sunlit face, and read from the volume
// alone it was blurred over a texel or two, the lumps lit flat.
const SUN_NEAR: f32 = 70.0;
// The light scattered once, then many times, approximated by ever-softer octaves
// (Hillaire 2016): each sees this share of the cloud in the way.
const LIGHT_OCTAVES: vec3<f32> = vec3<f32>(1.0, 0.3, 0.06);

// The volume's heights over the cloud floor: its bottom, and how far it reaches up.
fn light_span() -> vec2<f32> {
    let bottom = atmos.layer.x - LIGHT_BELOW;
    let top = atmos.layer.z + (atmos.layer.z - atmos.layer.x) * TOP_LUMP * 0.5;
    return vec2<f32>(bottom, top - bottom);
}

// The optical depth toward the sun from `p` through the coarse cloud, over the
// kilometre past `SUN_NEAR`: three taps, each twice as long as the last, so a
// cloud's far side sits in its own shade.
fn sun_taps(p: vec3<f32>) -> f32 {
    let sun = globals.sun.xyz;
    var tau = 0.0;
    var reach = 0.0;
    for (var k = 1; k < 4; k++) {
        let seg = SUN_NEAR * exp2(f32(k));
        tau += cloud_at(p + sun * (reach + seg * 0.5), 0.0).sigma * seg;
        reach += seg;
    }
    return tau;
}

// How much sunlight reaches `p` through the coarse cloud, at each octave
// (`LIGHT_OCTAVES`): a tap over the first `SUN_NEAR` metres (if `near_tap`; behind
// enough cloud that little of what is lit shows, the volume alone), then the light
// volume from there. Off its ground (cloud far out over the horizon, a few texels on
// screen) the volume's taps themselves.
fn sun_through(p: vec3<f32>, near_tap: bool) -> vec3<f32> {
    let sun = globals.sun.xyz;
    var near = 0.0;
    if near_tap {
        near = cloud_at(p + sun * (SUN_NEAR * 0.5), 0.0).sigma * SUN_NEAR;
    }
    let q = p + sun * SUN_NEAR;
    let rect = columns_rect(false);
    let uv = (q.xy - rect.xy) / rect.zw;
    var through: vec3<f32>;
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) {
        through = exp(-sun_taps(q) * LIGHT_OCTAVES);
    } else {
        let span = light_span();
        let up = clamp((q.z - cloud_floor(q.xy) - span.x) / span.y, 0.0, 1.0);
        let slices = f32(textureDimensions(cloud_light).z);
        let w = (sqrt(up) * (slices - 1.0) + 0.5) / slices;
        through = textureSampleLevel(cloud_light, clamp_sampler, vec3<f32>(uv, w), 0.0).rgb;
    }
    return through * exp(-near * LIGHT_OCTAVES);
}
