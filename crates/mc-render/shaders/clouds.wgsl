//!use bindings
// The sky and its clouds (sky.rs).
//
//   fs_sky        end of the scene pass: the sky and sun wherever nothing was drawn
//   fs_march      half size: rays through the cloud layer, stopped by the scene's
//                 depth. rgb light scattered toward the eye, a what gets through
//   fs_resolve    the march folded into last frame's clouds, carried to where
//                 they are now on screen: a new start offset every frame averages
//                 out instead of shimmering or showing its pattern
//   fs_composite  in scene_over, before the icons: the clouds laid over the
//                 picture (a see-through hologram of them in the middle of the
//                 screen), and lightning strokes
//
// The clouds are the weather map (cover, storm, churn: clouds_sim.wgsl) shaped
// by a tiling Perlin-Worley texture drifting with the wind. Where the player
// looks, and over what they have selected, they thin away so the battle shows.

@group(1) @binding(0) var scene_depth: texture_depth_2d;
@group(1) @binding(1) var cloud_noise: texture_3d<f32>;
// rgb and alpha as four halves in x and y, z the distance to the cloud the
// texel saw (0: none), so the resolve can carry each texel back by its own depth.
@group(1) @binding(2) var cloud_march: texture_2d<u32>;
// The accumulated clouds: last frame's (read by the resolve) and this frame's
// (written by the resolve, read by the composite).
@group(1) @binding(3) var cloud_history: texture_2d<f32>;
@group(1) @binding(4) var cloud_now: texture_2d<f32>;

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

struct FullOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_fullscreen(@builtin(vertex_index) index: u32) -> FullOut {
    let p = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var out: FullOut;
    // Depth 0 is infinitely far in reversed Z: the sky pass tests against it.
    out.clip = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(p.x, 1.0 - p.y);
    return out;
}

fn unproject(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let h = globals.inv_view_proj * vec4<f32>(ndc, depth, 1.0);
    return h.xyz / h.w;
}

fn view_ray(uv: vec2<f32>) -> vec3<f32> {
    return normalize(unproject(uv, 0.01) - globals.camera.xyz);
}

// ---- Sky -----------------------------------------------------------------------

@fragment
fn fs_sky(in: FullOut) -> @location(0) vec4<f32> {
    let d = view_ray(in.uv);
    var color = sky_radiance(d);
    let mu = dot(d, globals.sun.xyz);
    let dark = atmos.horizon_color.w;
    // The disk, and the bright aureole right round it. The moon's disk is a
    // pale silver, not the blue of the light it is standing in for.
    let disk = mix(atmos.sun_color.rgb, vec3<f32>(dot(atmos.sun_color.rgb, vec3<f32>(0.3, 0.45, 0.25))) * vec3<f32>(1.0, 0.98, 0.93), dark);
    color += disk * atmos.sky_color.w * smoothstep(0.99985, 0.99995, mu);
    color += atmos.sun_color.rgb * 0.35 * mix(1.0, 0.25, dark) * pow(max(mu, 0.0), 900.0);
    // Derivatives before the branch: a pixel's size on the sky.
    let px = max(length(fwidth(d)), 1e-5) * 0.8;
    if dark > 0.0 && d.z > -0.02 {
        color += night_sky(d, mu, px) * dark;
    }
    return vec4<f32>(min(color, vec3<f32>(60.0)), 1.0);
}

// Value noise on the sky's sphere: smooth, no seams, no poles.
fn sky_noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = p - i;
    let u = f * f * (3.0 - 2.0 * f);
    let n = i.xy + i.z * vec2<f32>(17.31, 41.7);
    let a = mix(hash21(n), hash21(n + vec2<f32>(1.0, 0.0)), u.x);
    let b = mix(hash21(n + vec2<f32>(0.0, 1.0)), hash21(n + vec2<f32>(1.0, 1.0)), u.x);
    let c = mix(hash21(n + vec2<f32>(17.31, 41.7)), hash21(n + vec2<f32>(18.31, 41.7)), u.x);
    let e = mix(hash21(n + vec2<f32>(17.31, 42.7)), hash21(n + vec2<f32>(18.31, 42.7)), u.x);
    return mix(mix(a, b, u.y), mix(c, e, u.y), u.z);
}

// One layer of stars: at most one per cell of a cube-face grid, jittered in
// its cell and drawn as a point the size of a pixel whatever the resolution.
// `px` is the angular size of a pixel. Most are faint, a handful bright.
fn star_layer(d: vec3<f32>, px: f32, cells: f32, share: f32, bright: f32, salt: f32) -> vec3<f32> {
    let a = abs(d);
    var uv: vec2<f32>;
    var face: f32;
    if a.x >= a.y && a.x >= a.z {
        uv = d.yz / a.x;
        face = select(0.0, 1.0, d.x < 0.0);
    } else if a.y >= a.z {
        uv = d.xz / a.y;
        face = select(2.0, 3.0, d.y < 0.0);
    } else {
        uv = d.xy / a.z;
        face = select(4.0, 5.0, d.z < 0.0);
    }
    let g = (uv * 0.5 + 0.5) * cells;
    let cell = floor(g);
    let id = cell + vec2<f32>(face * 1031.0 + salt, face * 577.0 - salt);
    let h = hash21(id);
    if h > share {
        return vec3<f32>(0.0);
    }
    let r = h / share;
    // Kept off the cell's edges so a star's glow never needs the neighbour.
    let at = vec2<f32>(hash21(id + 7.13), hash21(id + 3.71)) * 0.5 + 0.25;
    // Grid units to radians round here: a face's cell grows away from its middle.
    let span = 2.0 / (cells * max(max(a.x, a.y), a.z)) * dot(d, d);
    let off = (g - cell - at) * span / px;
    // Brightness spread like real magnitudes: many at the limit of sight.
    let flux = bright * (0.06 + 0.94 * pow(1.0 - r, 7.0));
    let core = exp(-dot(off, off) * 1.8);
    // Colour from the star's temperature: mostly white, some blue, some amber.
    let t = hash21(id + 11.9);
    let tint = mix(mix(vec3<f32>(0.72, 0.82, 1.0), vec3<f32>(1.0, 0.98, 0.95), smoothstep(0.0, 0.35, t)),
        vec3<f32>(1.0, 0.8, 0.58), smoothstep(0.7, 1.0, t));
    // Scintillation: the air near the horizon makes stars flicker, not the zenith.
    let air = 1.0 - smoothstep(0.0, 0.5, d.z);
    let time = atmos.weather.w;
    let flicker = 1.0 + (sin(time * (6.0 + t * 9.0) + h * 900.0) * 0.5 + sin(time * (13.0 + r * 7.0) + t * 300.0) * 0.3) * (0.12 + 0.5 * air);
    return tint * flux * core * flicker;
}

// The night sky: the band of the galaxy and its dust, then the stars, both
// dimmed by the air near the horizon and washed out round the moon.
fn night_sky(d: vec3<f32>, mu: f32, px: f32) -> vec3<f32> {
    // Starlight lost in the air along the way: the lowest few degrees are bare.
    let up = max(d.z, 0.0);
    let clear = exp(-0.1 / (up + 0.04)) * smoothstep(-0.02, 0.04, d.z);
    // Near the moon its glare hides all but the brightest.
    let glare = 1.0 - 0.85 * smoothstep(0.9, 0.995, mu);
    // The galaxy: a band round a great circle, clumped and split by dust.
    let pole = normalize(vec3<f32>(0.35, -0.62, 0.7));
    let across = dot(d, pole);
    let q = d * 6.0;
    let clumps = sky_noise(q) * 0.6 + sky_noise(q * 2.3 + 5.1) * 0.3 + sky_noise(q * 5.7 + 1.7) * 0.1;
    let band = exp(-across * across / 0.028) * smoothstep(0.3, 0.75, clumps);
    let dust = exp(-(across - 0.03) * (across - 0.03) / 0.003) * smoothstep(0.35, 0.7, sky_noise(q * 1.7 + 9.3));
    let galaxy = band * (1.0 - 0.8 * dust);
    var sky = vec3<f32>(0.62, 0.66, 0.8) * galaxy * 0.08;
    // Stars: a sparse bright layer, and a fine faint one crowded into the band.
    var stars = star_layer(d, px, 160.0, 0.05, 7.0, 0.0);
    stars += star_layer(d, px, 420.0, 0.05 + 0.25 * galaxy, 1.1, 91.0);
    sky += stars * glare;
    return sky * clear;
}

// ---- Clouds --------------------------------------------------------------------

// 1 where clouds are drawn in full, 0 where they have been thinned away:
// round the point the camera looks at, round the selection, and right in
// front of the eye so flying through the layer never whites out the screen.
fn clearing(p: vec3<f32>) -> f32 {
    var keep = 1.0;
    let v = atmos.view;
    if v.w > 0.0 {
        // A ragged window, not a drawn circle: its edge wanders with the cloud.
        let q = p.xy - atmos.wind.xy;
        let d = distance(p.xy, v.xy) + (grad_noise2(q, v.z * 0.5) - 0.5) * v.z * 0.6;
        keep *= mix(1.0, smoothstep(v.z * 0.4, v.z * 1.25, d), v.w);
    }
    let n = u32(atmos.counts.x);
    for (var i = 0u; i < n; i++) {
        let c = atmos.clears[i];
        let d = distance(p.xy, c.xy);
        keep *= mix(1.0, smoothstep(c.z * 0.55, c.z * 1.5, d), c.w);
    }
    // A bubble of clear air round the eye, as big as the view is close: down
    // among the clouds the battle stays in sight; from high up it is too
    // small to reach them.
    // Down among the clouds it clears the cloud near the eye, so the view is
    // never a whiteout, but stops short of the ground being looked at: the
    // cloud there is at the height aircraft fly, and they fly through it (a
    // bubble reaching the ground cleared every cloud round the battle). From
    // high above it shrinks back and the layer is seen whole.
    let eye = globals.camera.xyz;
    let above = eye.z - cloud_floor(eye.xy);
    let among = 1.0 - smoothstep(atmos.layer.y + 300.0, atmos.layer.y + 2500.0, above);
    // From above it is only big enough that a tower top reaching the eye does
    // not white out the screen, and its edge is ragged: a clean sphere up to
    // 1200 m across sliced every tall cloud near the camera into a smooth wall
    // with a hard rim, which showed as bands and streaks as the view moved.
    let bubble = mix(clamp(atmos.view.z * 0.12, 250.0, 600.0), max(atmos.view.z * 0.45, 120.0), among);
    let rag = grad_noise2(p.xy + vec2<f32>(p.z * 0.7, -p.z * 0.4) - atmos.wind.xy, bubble * 0.6) - 0.5;
    keep *= smoothstep(bubble * 0.4, bubble, distance(p, eye) + rag * bubble * 0.7);
    return keep;
}

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
}

// The cloud at `p`, given the weather there (`weather_at(p.xy)`). `detail` 0
// skips the fine erosion (light rays); otherwise it is how many times its
// usual size the erosion is drawn, so seen from far off it stays a few march
// texels across instead of shimmering or being dropped (which left the whole
// strategic view soft, rounded blobs). The clearing is applied by the
// caller, to how opaque the cloud is, never to its shape.
fn cloud_at(p: vec3<f32>, w: vec4<f32>, detail: f32) -> CloudSample {
    var out: CloudSample;
    let cover = w.x;
    if cover < 0.015 {
        return out;
    }
    let storm = clamp(w.y, 0.0, 1.0);
    // Height varies from cloud to cloud: bases wander (some hang low enough to
    // swallow a gunship, some sit over the jets), and where the
    // air is lifting (a field drifting with the wind) cumulus grows into tall
    // towers, most of all in towering weather (`shape.x`).
    let tall = atmos.shape.x;
    let air = p.xy - atmos.wind.xy;
    let lift = grad_noise2(air + 1711.0, 2600.0 * max(atmos.shape.y, 0.5));
    // Every cloud its own: a field drifting with the wind decides whether this
    // one is a thin shred the sun shines through or a dense heap with a grey
    // belly, and storms and thick cover lean to the heavy kind. With one
    // density and one brightness everywhere every cloud was the same white
    // cotton ball.
    let kind = grad_noise2(air + vec2<f32>(2917.0, -811.0), 1700.0 * max(atmos.shape.y, 0.5));
    // Under a full deck the thin patches and the sheet's holes close up:
    // overcast has few breaks.
    // (The weather's own cover: the map's is never over 1.)
    let deck_closed = smoothstep(1.5, 1.85, atmos.layer.w);
    let firm = max(smoothstep(0.42, 0.58, kind + storm * 0.5), deck_closed * 0.8);
    // And in wide stretches of fair weather the heaps give way to a flat,
    // broken sheet (stratocumulus): shallow, lumpy underneath, full of holes.
    let sheet = smoothstep(0.5, 0.6, grad_noise2(air + vec2<f32>(-5113.0, 3301.0), 5600.0 * max(atmos.shape.y, 0.5))) * (1.0 - storm);
    let floor = cloud_floor(p.xy);
    let base = floor + atmos.layer.x - storm * 80.0 + (grad_noise2(air - 377.0, 4300.0) - 0.5) * (240.0 + 120.0 * tall);
    let deck = atmos.layer.y - atmos.layer.x;
    let fair_top = floor + atmos.layer.y + deck * 2.6 * pow(lift, 2.2) * tall;
    // Thin cover makes shallow cloud; the fullest air the tallest.
    let fill = mix(0.55, 1.0, min(cover, 1.0));
    // Shreds are shallow as well as thin.
    let fair_depth = (fair_top - base) * fill * mix(0.7, 1.0, firm) * mix(1.0, 0.3, sheet * (1.0 - deck_closed));
    let storm_depth = (floor + atmos.layer.z - base) * fill;
    // The billows below decide how tall a storm grows here and lift or drop
    // every top: tested against the highest it could reach before looking
    // them up.
    // How tall a storm grows, from the storm round about: the weather map's
    // storm has sharp steps (a core's rim), and taken as it is each stood up
    // as a sheer wall whose sunlit crest ran across the storm like a tube.
    // Not for the light's taps (`detail` 0): they are coarse, and it would be
    // sixteen more lookups a sample.
    var tall_storm = storm;
    if storm > 0.01 && detail > 0.0 {
        var around = storm * 2.0;
        for (var k = 0; k < 4; k++) {
            let a = f32(k) * 1.5708 + 0.4;
            around += clamp(weather_at(p.xy + vec2<f32>(cos(a), sin(a)) * STORM_SPREAD).y, 0.0, 1.0);
        }
        tall_storm = around / 6.0;
    }
    let reach = mix(fair_depth, storm_depth, pow(tall_storm, 1.3));
    if p.z <= base || p.z >= base + reach * (1.0 + TOP_LUMP * 0.5) {
        return out;
    }
    out.storm = storm;
    // The billows drift with the wind; churned air twists them about.
    let drift = vec3<f32>(atmos.wind.xy, 0.0);
    let churn = clamp(w.z, 0.0, 1.0);
    // Aircraft wakes: the flow they stirred bends the billows about (the
    // vortices twist them), and a fresh streak thins the cloud in a narrow
    // tube at the height the aircraft flew.
    let stir = flow_at(p.xy);
    // A bend of some tens of metres at most: past that the billows shear
    // into ribbons instead of twisting.
    let bend = stir.xy * 14.0;
    let q = p - drift - vec3<f32>(bend / (1.0 + length(bend) / 70.0), 0.0);
    let swirl = vec3<f32>(sin(q.y * 0.004 + atmos.weather.w * 0.3), cos(q.x * 0.004 - atmos.weather.w * 0.25), 0.0) * churn * 140.0;
    // The tube it thins is tight below the aircraft and rolls up a little above
    // it, and it is ragged: puffs torn out along it, not a clean cut. Cut
    // clean and tall (it once reached 650 m up), a flight on patrol sliced the
    // layer into straight strips with hard edges.
    let above = p.z - stir.z;
    var streak = stir.w * exp(-pow(above / select(45.0, 180.0, above > 0.0), 2.0));
    if streak > 0.01 {
        streak *= smoothstep(0.3, 0.75, grad_noise2(q.xy + vec2<f32>(q.z * 0.7, 0.0), 140.0));
    }
    // Bigger weather, bigger billows.
    let period = SHAPE_PERIOD * mix(1.0, max(atmos.shape.y, 0.5), 0.6);
    // Two lookups of the billows at unrelated sizes and angles. One alone
    // repeated every period across the map, the same lumps over and over, and
    // where cover was thin (the tops) only its cells' ridges were left: the
    // same smooth rubbery tubes everywhere.
    let bq = q.xy + swirl.xy;
    let n = textureSampleLevel(cloud_noise, repeat_sampler, vec3<f32>(turn(bq, TURN_SHAPE) / period, q.z / (period * 0.25)), 0.0);
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
    // The fine erosion, pushed about by the billows' own lumps so its tile (a
    // few hundred metres) never shows as the same cauliflower over and over.
    var fine = 0.5;
    if detail > 0.0 {
        let dp = DETAIL_PERIOD * detail;
        let warp = (vec2<f32>(m.y, n.z) - 0.5) * dp * 0.9;
        let e = textureSampleLevel(cloud_noise, repeat_sampler,
            vec3<f32>(turn(q.xy - swirl.xy * 2.0 + warp, TURN_DETAIL) / dp, q.z / (dp * 0.6)) + vec3<f32>(0.0, 0.0, atmos.weather.w * 0.004), 0.0);
        fine = e.y * 0.625 + e.z * 0.25 + e.w * 0.125;
    }
    // Up top it moves the top itself: turrets and cauliflower. Eroding only
    // the cloud's thin fringe left a storm's top smooth, as a storm is opaque
    // well inside that fringe, and a low sun drew its gentle swells as tubes.
    let h = h0 + (0.5 - fine) * TOP_CAULI * smoothstep(0.35, 0.85, h0);
    if h >= 1.0 {
        return out;
    }
    out.height = h;
    // Flat, crisp bases; domes that narrow as they climb, so each cell rounds
    // off on top; a storm towers, then spreads into an anvil.
    let fair = mix(smoothstep(0.0, 0.08, h) * (1.0 - h * h * 0.85), smoothstep(0.0, 0.2, h) * (1.0 - smoothstep(0.55, 1.0, h)), sheet);
    let tower = smoothstep(0.0, 0.04, h) * mix(1.0 - h * 0.25, 1.05, smoothstep(0.72, 0.9, h)) * (1.0 - smoothstep(0.88, 1.0, h));
    let profile = mix(fair, tower, storm);
    // Full cover still keeps some of the billows' relief, so a deck or a
    // storm top reads as lumps and not a smooth sheet.
    // A sheet spreads further than heaps would in the same air.
    let c = min(cover + sheet * 0.25, 1.0) * profile * mix(0.9, 0.78, storm);
    var dens = clamp((billow - (1.0 - c)) / max(c, 0.12), 0.0, 1.0);
    if dens <= 0.0 {
        return out;
    }
    if detail > 0.0 {
        // Erode the edges: wispy at the base and in churned air, cauliflower on top.
        let wisp = mix(fine, 1.0 - fine, smoothstep(0.1, 0.5, h));
        let erode = 0.4 + churn * 0.4 + storm * 0.15 + ((1.0 - firm) * 0.12 + sheet * 0.2) * (1.0 - deck_closed);
        dens = clamp((dens - wisp * erode) / (1.0 - wisp * erode), 0.0, 1.0);
    }
    // A heap is firm inside, so it ends at an edge instead of a wide thin
    // fringe; a shred stays soft and see-through.
    dens = mix(dens, dens * (2.0 - dens), firm);
    dens *= 1.0 - 0.55 * streak;
    out.sigma = dens * CLOUD_SIGMA * (0.65 + 0.8 * storm) * mix(0.12, 1.0, firm);
    out.firm = firm;
    out.above = (1.0 - h) * reach;
    if detail > 0.0 {
        // Older, wetter cloud is greyer: patches of it, more in heavy weather.
        let age = grad_noise2(air - vec2<f32>(1409.0, 2203.0), 1400.0);
        out.grey = (smoothstep(0.42, 0.62, age) * 0.45 + storm * 0.25 + deck_closed * 0.15 + sheet * 0.2) * (0.4 + 0.6 * firm);
    }
    return out;
}

// The clouds' shade on everything under them (sky.rs `shade`, read by
// `cloud_shadow`): per texel of the map, the sunlight left after a ray from
// below the layer crosses the same clouds the march draws. The shade was once
// the weather map's cover, smoothed: dark wherever the air was cloudy at all,
// under the billows' holes and the thin shreds as much as under the heaps, so
// the land went dark with nothing drawn overhead.
@group(1) @binding(5) var cloud_shade_out: texture_storage_2d<rgba16float, read_write>;

// How far under the cloud floor's base the shade is taken: below the lowest
// base a cloud can wander down to.
const SHADE_BELOW: f32 = 300.0;
// Taps through fair-weather cloud, then on up through storm towers.
const SHADE_STEPS: i32 = 16;
const SHADE_STORM_STEPS: i32 = 8;
// A new start offset every frame, folded into what the texel had: the taps
// are tens of metres apart and a fixed offset would band the shade.
const SHADE_BLEND: f32 = 0.25;
// Erosion drawn this many times its size: the shade's texels are coarse.
const SHADE_DETAIL: f32 = 2.0;

@compute @workgroup_size(8, 8)
fn cs_shade(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(cloud_shade_out);
    if any(id.xy >= size) {
        return;
    }
    let xy = (vec2<f32>(id.xy) + 0.5) / vec2<f32>(size) * atmos.weather.yz;
    let floor = cloud_floor(xy);
    let start = vec3<f32>(xy, floor + atmos.layer.x - SHADE_BELOW);
    // The same slant `cloud_shadow` casts along.
    let s = globals.sun.xyz;
    let d = normalize(vec3<f32>(s.xy, max(s.z, 0.2)));
    let deck = atmos.layer.y - atmos.layer.x;
    let fair_top = floor + atmos.layer.y + deck * 2.6 * atmos.shape.x + deck * (1.0 + TOP_LUMP) + 200.0;
    let storm_top = floor + atmos.layer.z + (atmos.layer.z - atmos.layer.x) * TOP_LUMP * 0.5;
    let fair_len = (fair_top - start.z) / d.z;
    let storm_len = max(storm_top - fair_top, 0.0) / d.z;
    let jitter = fract(hash21(vec2<f32>(id.xy) + 0.5) + atmos.frame.x * 0.618034);
    var tau = 0.0;
    let dt = fair_len / f32(SHADE_STEPS);
    for (var k = 0; k < SHADE_STEPS; k++) {
        let p = start + d * (dt * (f32(k) + jitter));
        let w = weather_at(p.xy);
        if w.x < 0.015 {
            continue;
        }
        tau += cloud_at(p, w, SHADE_DETAIL).sigma * dt;
    }
    // Above the fair tops only a storm's towers reach.
    let ds = storm_len / f32(SHADE_STORM_STEPS);
    for (var k = 0; k < SHADE_STORM_STEPS; k++) {
        let p = start + d * (fair_len + ds * (f32(k) + jitter));
        let w = weather_at(p.xy);
        if w.x < 0.015 || w.y < 0.02 {
            continue;
        }
        tau += cloud_at(p, w, SHADE_DETAIL).sigma * ds;
    }
    let lit = exp(-tau);
    let texel = vec2<i32>(id.xy);
    let old = textureLoad(cloud_shade_out, texel);
    // Alpha 0: never written (sky.rs clears it so), take this frame whole.
    let blend = select(SHADE_BLEND, 1.0, old.a < 0.5);
    textureStore(cloud_shade_out, texel, vec4<f32>(mix(old.r, lit, blend), 0.0, 0.0, 1.0));
}

// Interleaved gradient noise: a per-pixel start offset for the march.
// A start offset with no pattern across the screen (a per-texel hash), stepped
// by the golden ratio each frame so every texel walks its whole range over a
// few frames; fs_resolve averages them. A patterned offset (interleaved
// gradient noise) showed through thin cloud as a lattice.
fn ign(p: vec2<f32>) -> f32 {
    return fract(hash21(floor(p) + vec2<f32>(0.5, 0.5)) + atmos.frame.x * 0.618034);
}

// How far along the ray the cloud the march saw lies, for the resolve.
var<private> march_depth: f32;

@fragment
fn fs_march(in: FullOut) -> @location(0) vec4<u32> {
    march_depth = 0.0;
    let c = march(in);
    return vec4<u32>(pack2x16float(c.rg), pack2x16float(c.ba), bitcast<u32>(march_depth), 0u);
}

fn march_texel(px: vec2<i32>) -> vec4<f32> {
    let v = textureLoad(cloud_march, px, 0);
    return vec4<f32>(unpack2x16float(v.x), unpack2x16float(v.y));
}

fn march(in: FullOut) -> vec4<f32> {
    // The farthest of the full-size depths at this texel's corners.
    let full = vec2<f32>(textureDimensions(scene_depth));
    // Full-size pixels per march texel, from how fast uv moves across it.
    let scale = round(vec2<f32>(abs(dpdx(in.uv.x)), abs(dpdy(in.uv.y))) * full);
    let px = vec2<i32>(floor(in.uv * full - scale * 0.5 + 0.5));
    let step = vec2<i32>(max(scale - 1.0, vec2<f32>(0.0)));
    let dims = vec2<i32>(full) - vec2<i32>(1);
    var depth = 1.0;
    for (var i = 0; i < 4; i++) {
        depth = min(depth, textureLoad(scene_depth, min(px + vec2<i32>(i & 1, i >> 1u) * step, dims), 0));
    }
    let eye = globals.camera.xyz;
    let rd = view_ray(in.uv);
    // A march texel's angle across, before any branch.
    let texel_angle = max(length(fwidth(rd)), 1e-6);
    var t_scene = 1.0e9;
    if depth > 0.0 {
        t_scene = distance(unproject(in.uv, depth), eye);
    }
    // Where the ray is inside the layer, from the lowest storm base to the highest top.
    let z0 = atmos.frame.w + atmos.layer.x - 300.0;
    let z1 = atmos.shape.w + atmos.layer.z + (atmos.layer.z - atmos.layer.x) * TOP_LUMP * 0.5;
    // At most 50 km of layer along the ray, counted from where it enters: the
    // strategic view over an 80 km map looks down from well over 100 km, and
    // a cap counted from the eye cut every ray off before it reached the layer.
    var t0 = 0.0;
    var t1 = 50000.0;
    if abs(rd.z) > 1e-5 {
        let a = (z0 - eye.z) / rd.z;
        let b = (z1 - eye.z) / rd.z;
        t0 = max(min(a, b), 0.0);
        t1 = min(max(a, b), t0 + t1);
    } else if eye.z < z0 || eye.z > z1 {
        return with_rain(vec4<f32>(0.0, 0.0, 0.0, 1.0), eye, rd, t_scene, in.clip.xy);
    }
    t1 = min(t1, t_scene);
    if t1 <= t0 {
        return with_rain(vec4<f32>(0.0, 0.0, 0.0, 1.0), eye, rd, t_scene, in.clip.xy);
    }

    let sun = globals.sun.xyz;
    let mu = dot(rd, sun);
    // Two lobes: the bright silver lining toward the sun, a little back-glow.
    let phase = mix(phase_hg(mu, 0.75), phase_hg(mu, -0.2), 0.3);
    let phase_soft = mix(phase_hg(mu, 0.35), phase_hg(mu, -0.1), 0.3);
    let span = t1 - t0;
    // Steps grow with distance: fine where clouds are near and large on screen.
    // Not rounded: a whole number of steps would jump as the camera moves.
    // Looking down from the strategic view the ray crosses the layer steeply:
    // at 420 m a step a kilometre of cloud got three samples.
    let steps = clamp(span / mix(50.0, 100.0, smoothstep(2000.0, 30000.0, t0)), 16.0, 120.0);
    let dt = span / steps;
    let jitter = ign(in.clip.xy);
    var t = t0 + dt * jitter;
    var through = 1.0;
    var light = vec3<f32>(0.0);
    var depth_sum = 0.0;
    var weight_sum = 0.0;
    // The erosion's smallest lumps (about a sixteenth of its period) kept to
    // two march texels or more where the ray enters the cloud.
    let detail = max(1.0, t0 * texel_angle * 32.0 / DETAIL_PERIOD);
    let flash_count = u32(atmos.counts.y);
    // Coarse steps through clear air; on finding cloud, back up and go on at a
    // quarter of the stride until it has been clear for two coarse steps. At
    // the coarse stride alone a thin part was hit on some frames and missed on
    // others, and the clouds' edges crawled.
    var fine = 0;
    for (var i = 0; i < 240; i++) {
        if t >= t1 || through < 0.015 {
            break;
        }
        let stride = select(dt, dt * 0.25, fine > 0);
        let p = eye + rd * t;
        let w = weather_at(p.xy);
        // Open sky: nothing more to look up.
        if w.x < 0.015 {
            fine = max(fine - 1, 0);
            t += stride;
            continue;
        }
        var c = cloud_at(p, w, detail);
        if c.sigma > 0.0 && fine == 0 {
            fine = 8;
            t = max(t - dt * 0.75, t0);
            continue;
        }
        if c.sigma <= 0.0 {
            fine = max(fine - 1, 0);
        } else {
            fine = 8;
            // Where the player looks through, the cloud keeps its shape and its
            // light but lets the ground show: thinned, not removed.
            let keep = clearing(p);
            let sigma_full = c.sigma;
            c.sigma = sigma_full * mix(0.025, 1.0, keep * keep);
            // Light from the sun: four taps toward it through the coarse cloud,
            // reaching a kilometre, so a cloud's far side sits in its own shade.
            var tau = 0.0;
            var reach = 0.0;
            for (var k = 0; k < 4; k++) {
                let seg = 70.0 * exp2(f32(k));
                let q = p + sun * (reach + seg * 0.5);
                tau += cloud_at(q, weather_at(q.xy), 0.0).sigma * seg;
                reach += seg;
            }
            // Beer's law with the light scattered many times approximated by
            // ever-softer octaves (Hillaire 2016), and the dark "powder" rims.
            let sun_through = exp(-tau) * phase + exp(-tau * 0.3) * 0.3 * phase_soft + exp(-tau * 0.06) * 0.25 * 0.0796;
            // Powder darkens thin cloud only looking toward the sun (its
            // in-scatter has not built up yet there). From above, with the sun
            // behind the eye, it greyed every rim and softened the outline.
            let powder = mix(1.0, 1.0 - exp(-c.sigma * 90.0), smoothstep(-0.2, 0.5, mu));
            let direct = atmos.sun_color.rgb * sun_through * mix(0.55, 1.0, powder) * 2.6 * mix(1.0, atmos.sun_color.w, 0.8);
            // Sky light from above, dimmer low in the cloud and inside storms;
            // the land's light from below on the base.
            // The column above hides the sky from a point low in the cloud: a
            // small cloud's base stays bright, a big heap's belly goes grey.
            let hidden = exp(-c.above * 0.0028 * mix(0.35, 1.0, c.firm));
            let sky = atmos.sky_color.rgb * mix(0.3, 1.15, hidden) * mix(0.55, 1.0, c.height) * (1.0 - c.storm * 0.55)
                + atmos.ground_color.rgb * 0.35 * (1.0 - c.height);
            let albedo = 1.0 - c.grey;
            var glow = vec3<f32>(0.0);
            for (var f = 0u; f < flash_count; f++) {
                let fl = atmos.flashes[f];
                let r = distance(p, fl.xyz);
                glow += vec3<f32>(0.62, 0.7, 1.0) * fl.w * 26.0 * exp(-r / 520.0);
            }
            let source = ((direct + sky) * albedo + glow) * c.sigma;
            let absorb = exp(-c.sigma * stride);
            light += through * source * (1.0 - absorb) / c.sigma;
            let w = through * (1.0 - absorb);
            depth_sum += t * w;
            weight_sum += w;
            through *= absorb;
        }
        t += stride;
    }
    if weight_sum > 0.0 {
        // Air between the eye and the cloud: far clouds fade into the sky.
        march_depth = depth_sum / weight_sum;
        let at = eye + rd * march_depth;
        let covered = 1.0 - through;
        let hazed = apply_haze(light / max(covered, 1e-3), at, eye);
        light = hazed * covered;
    }
    return with_rain(vec4<f32>(min(light, vec3<f32>(40.0)), through), eye, rd, t_scene, in.clip.xy);
}

// Rain under the clouds: grey veils hanging from storm bases and heavy decks
// (the weather map's rain, `w`), streaked and falling. `cloud` is what the
// march found above; the rain goes behind it seen from above, in front of it
// seen from beneath.
fn with_rain(cloud: vec4<f32>, eye: vec3<f32>, rd: vec3<f32>, t_scene: f32, pixel: vec2<f32>) -> vec4<f32> {
    if atmos.shape.z <= 0.0 {
        return cloud;
    }
    // The highest the rain can hang from; each sample checks its own floor.
    let top = atmos.shape.w + atmos.layer.x - 60.0;
    var ta = 0.0;
    var tb = min(t_scene, 30000.0);
    if eye.z > top {
        if rd.z > -1e-4 {
            return cloud;
        }
        ta = (top - eye.z) / rd.z;
    } else if rd.z > 1e-4 {
        tb = min(tb, (top - eye.z) / rd.z);
    }
    if tb <= ta {
        return cloud;
    }
    // Nothing falling anywhere along it: done.
    let r0 = weather_at((eye + rd * ta).xy).w;
    let r1 = weather_at((eye + rd * tb).xy).w;
    let rm = weather_at((eye + rd * (ta + tb) * 0.5).xy).w;
    if max(r0, max(r1, rm)) < 0.01 {
        return cloud;
    }
    let steps = 8.0;
    let dt = (tb - ta) / steps;
    var t = ta + dt * ign(pixel + vec2<f32>(37.0, 11.0));
    var through = 1.0;
    var light = vec3<f32>(0.0);
    let time = atmos.weather.w;
    for (var i = 0; i < 8; i++) {
        let p = eye + rd * t;
        let r = weather_at(p.xy).w;
        if r > 0.005 {
            // Shafts: coarse noise in plan, stretched tall, drifting down.
            let q = p.xy - atmos.wind.xy * 1.1;
            let streak = textureSampleLevel(cloud_noise, repeat_sampler,
                vec3<f32>(q / 260.0, (p.z + time * 55.0) / 1800.0), 0.0).z;
            let under = cloud_floor(p.xy) + atmos.layer.x - 60.0;
            let hang = smoothstep(under, under - 250.0, p.z);
            let sigma = r * 0.00055 * (0.35 + 1.3 * streak) * hang;
            // Lit grey by the sky, a little by the sun through the cloud above.
            let source = atmos.sky_color.rgb * 1.1 + atmos.sun_color.rgb * 0.035;
            let absorb = exp(-sigma * dt);
            light += through * source * (1.0 - absorb);
            through *= absorb;
        }
        t += dt;
    }
    if eye.z > top {
        return vec4<f32>(cloud.rgb + cloud.a * light, cloud.a * through);
    }
    return vec4<f32>(light + through * cloud.rgb, through * cloud.a);
}

// ---- Falling rain, close up -------------------------------------------------------
// Streaks where the weather map says it rains, in three nested layers of tiles
// fixed to the ground (small ones close up, big ones further out), each drop
// wrapping round its tile as the camera moves, so the rain stays put in the
// world and pans past like everything else. The layer that suits the zoom is
// drawn; its neighbours fade in and out as the camera goes in and out.

const RAIN_DROPS: u32 = 15000u;
const RAIN_LAYERS: u32 = 3u;

struct RainOut {
    @builtin(position) clip: vec4<f32>,
    // x along the streak (0 head, 1 tail), y across it (-1 to 1), z strength.
    @location(0) streak: vec3<f32>,
}

@vertex
fn vs_rain(@builtin(vertex_index) vertex: u32, @builtin(instance_index) drop: u32) -> RainOut {
    var out: RainOut;
    out.clip = vec4<f32>(0.0, 0.0, -1.0, 1.0);
    if atmos.shape.z <= 0.0 {
        return out;
    }
    let eye = globals.camera.xyz;
    let focus = vec3<f32>(atmos.view.xy, terrain_height(atmos.view.xy));
    let dist = distance(eye, focus);
    // Tile size of this drop's layer: 60, 240, 960 m.
    let layer = drop % RAIN_LAYERS;
    let span = 60.0 * exp2(2.0 * f32(layer));
    // Each layer is best when its tile is about as wide as the view is far;
    // the next one in takes over well before its small tile shows as a patch.
    let fit = log2(span / max(dist * 0.9, 1.0)) * 0.5;
    var weight = exp(-fit * fit * 6.0);
    if layer == 0u {
        weight = select(weight, 1.0, fit > 0.0);
    }
    weight *= 1.0 - smoothstep(2800.0, 4500.0, dist);
    if weight < 0.02 {
        return out;
    }
    let id = f32(drop / RAIN_LAYERS) + f32(layer) * 7919.0;
    let h = vec4<f32>(hash11(id * 1.37 + 0.1), hash11(id * 2.71 + 0.3), hash11(id * 0.93 + 0.7), hash11(id * 3.17 + 0.9));
    // Where in its tile, taken round to a tile centred a little toward the
    // camera from the focus: the ground at the bottom of the screen is nearer
    // the eye than the focus is, and the drops that cover it hang nearer still.
    let toward = eye.xy - focus.xy;
    let centre = focus.xy + toward * min(0.35, span * 0.3 / max(length(toward), 1.0));
    let local = fract(vec2<f32>(h.x, h.y) - centre / span) - 0.5;
    var xy = centre + local * span;
    // Falls about its column's height in a second and a half, whatever the layer,
    // so on screen close and far rain fall alike: brisk, not drifting.
    let column = span * 0.75;
    let fall = column / mix(1.2, 1.8, h.w);
    let slant = atmos.wind.zw * 0.022;
    let phase = fract(h.z - atmos.weather.w * fall / column);
    let ground = terrain_height(xy);
    // Phase runs down as the drop falls, so it is carried downwind as it goes.
    xy -= slant * column * (phase - 0.5);
    let z = ground + phase * column;
    let rain = weather_at(xy).w;
    // Light rain is fewer drops, not fainter ones.
    if rain < 0.02 || h.x * 0.85 > rain * 1.3 + 0.08 {
        return out;
    }
    let edge = 1.0 - smoothstep(0.36, 0.5, max(abs(local.x), abs(local.y)));
    let head = vec3<f32>(xy, z);
    let vel = normalize(vec3<f32>(slant, -1.0));
    // A camera exposure's worth of motion.
    let tail = head - vel * fall * 0.04;
    let corner = vertex % 6u;
    let along = select(0.0, 1.0, corner == 1u || corner == 2u || corner == 4u);
    let side = select(-1.0, 1.0, corner == 2u || corner == 4u || corner == 5u);
    // A thin quad from head to tail, a pixel and a bit wide on screen.
    let ch = globals.view_proj * vec4<f32>(head, 1.0);
    let ct = globals.view_proj * vec4<f32>(tail, 1.0);
    if ch.w <= 0.1 || ct.w <= 0.1 {
        return out;
    }
    let sh = ch.xy / ch.w * globals.viewport.xy;
    let st = ct.xy / ct.w * globals.viewport.xy;
    let dir = normalize(st - sh + vec2<f32>(0.0, 1e-3));
    let c = mix(ch, ct, along);
    let px = vec2<f32>(-dir.y, dir.x) * side * 0.75;
    out.clip = vec4<f32>(c.xy + px * 2.0 * globals.viewport.zw * c.w, c.zw);
    out.streak = vec3<f32>(along, side, min(rain * 1.6, 1.0) * mix(0.55, 1.0, h.y) * weight * edge);
    return out;
}

@fragment
fn fs_rain(in: RainOut) -> @location(0) vec4<f32> {
    let edge = 1.0 - abs(in.streak.y);
    let fade = smoothstep(0.0, 0.15, in.streak.x) * (1.0 - smoothstep(0.55, 1.0, in.streak.x));
    let a = edge * fade * in.streak.z * 0.42;
    let color = atmos.sky_color.rgb * 1.8 + atmos.sun_color.rgb * 0.08;
    return vec4<f32>(color * a, a);
}

// ---- Carried over from frame to frame --------------------------------------------

// Catmull-Rom from nine texels in five bilinear taps (the corners are dropped:
// they weigh next to nothing). Negative lobes can overshoot, so the caller clamps.
fn sample_sharp(tex: texture_2d<f32>, uv: vec2<f32>) -> vec4<f32> {
    let size = vec2<f32>(textureDimensions(tex));
    let at = uv * size;
    let centre = floor(at - 0.5) + 0.5;
    let f = at - centre;
    let w0 = f * (-0.5 + f * (1.0 - 0.5 * f));
    let w1 = 1.0 + f * f * (-2.5 + 1.5 * f);
    let w2 = f * (0.5 + f * (2.0 - 1.5 * f));
    let w3 = f * f * (-0.5 + 0.5 * f);
    let w12 = w1 + w2;
    let o12 = w2 / w12;
    let p0 = (centre - 1.0) / size;
    let p3 = (centre + 2.0) / size;
    let p12 = (centre + o12) / size;
    var sum = textureSampleLevel(tex, clamp_sampler, vec2<f32>(p12.x, p0.y), 0.0) * (w12.x * w0.y);
    sum += textureSampleLevel(tex, clamp_sampler, vec2<f32>(p0.x, p12.y), 0.0) * (w0.x * w12.y);
    sum += textureSampleLevel(tex, clamp_sampler, vec2<f32>(p12.x, p12.y), 0.0) * (w12.x * w12.y);
    sum += textureSampleLevel(tex, clamp_sampler, vec2<f32>(p3.x, p12.y), 0.0) * (w3.x * w12.y);
    sum += textureSampleLevel(tex, clamp_sampler, vec2<f32>(p12.x, p3.y), 0.0) * (w12.x * w3.y);
    let weight = w12.x * w0.y + w0.x * w12.y + w12.x * w12.y + w3.x * w12.y + w12.x * w3.y;
    return sum / weight;
}

@fragment
fn fs_resolve(in: FullOut) -> @location(0) vec4<f32> {
    let px = vec2<i32>(in.clip.xy);
    let last = vec2<i32>(textureDimensions(cloud_march)) - vec2<i32>(1);
    let now = march_texel(px);
    if atmos.frame.y < 0.5 {
        return now;
    }
    // Where this texel's cloud was on screen last frame: back along the ray
    // as far as the cloud it saw. One plane for every texel (the middle of the
    // layer) put everything above or below it in the wrong place as the view
    // panned, the clamp threw that history away, and the clouds went back to
    // one frame's grain and crawled. Clear texels use the middle of the layer.
    let eye = globals.camera.xyz;
    let d = view_ray(in.uv);
    var t = bitcast<f32>(textureLoad(cloud_march, px, 0).z);
    if t <= 0.0 {
        let mid = cloud_floor(eye.xy + d.xy * 1000.0) + mix(atmos.layer.x, atmos.layer.y, 0.5);
        t = 20000.0;
        if abs(d.z) > 1e-4 && (mid - eye.z) / d.z > 0.0 {
            t = (mid - eye.z) / d.z;
        }
    }
    let clip = atmos.prev_view_proj * vec4<f32>(eye + d * t, 1.0);
    if clip.w <= 0.0 {
        return now;
    }
    let uv = vec2<f32>(clip.x / clip.w * 0.5 + 0.5, 0.5 - clip.y / clip.w * 0.5);
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) {
        return now;
    }
    // Keep the history inside what the neighbourhood holds now (its mean give
    // or take its spread), so a cloud that moved or a view that swung leaves
    // no ghost or streak behind.
    var sum = vec4<f32>(0.0);
    var sq = vec4<f32>(0.0);
    var lo = now;
    var hi = now;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let c = march_texel(clamp(px + vec2<i32>(x, y), vec2<i32>(0), last));
            sum += c;
            sq += c * c;
            lo = min(lo, c);
            hi = max(hi, c);
        }
    }
    let mean = sum / 9.0;
    let spread = sqrt(max(sq / 9.0 - mean * mean, vec4<f32>(0.0)));
    // With the view still, the history is the better estimate: clip it only
    // loosely (tight clipping to a noisy neighbourhood re-injects the noise)
    // and average over more frames. While it moves, clip harder and lean a
    // little more on the new frame, or history resampled every frame blurs
    // and trails; carried by its own depth it lands true, so not so hard that
    // panning shows one frame's grain.
    let moved = length((uv - in.uv) * vec2<f32>(textureDimensions(cloud_march)));
    let motion = smoothstep(0.3, 3.0, moved);
    let k = mix(3.0, 2.0, motion);
    // Read back without softening it: a still view (whose reprojection is off
    // by float error only) takes its own texel, a moving one a Catmull-Rom
    // sample, since bilinear history resampled every frame softens as it pans.
    var history: vec4<f32>;
    if moved < 0.25 {
        history = textureLoad(cloud_history, px, 0);
    } else {
        history = sample_sharp(cloud_history, uv);
    }
    let before = clamp(
        history,
        mix(mean - spread * k, max(lo, mean - spread * k), motion),
        mix(mean + spread * k, min(hi, mean + spread * k), motion),
    );
    return mix(before, now, mix(0.06, 0.14, motion));
}

// ---- Laid over the picture ------------------------------------------------------

// A stroke's kinks, `k` of `n` along it: the same shape for its whole life.
fn bolt_point(top: vec3<f32>, ground: vec3<f32>, seed: f32, k: i32, n: i32) -> vec3<f32> {
    let f = f32(k) / f32(n);
    var p = mix(top, ground, f);
    if k > 0 && k < n {
        let len = distance(top, ground);
        let jag = vec2<f32>(hash11(seed + f32(k) * 1.37) - 0.5, hash11(seed * 1.9 + f32(k) * 2.11) - 0.5);
        p += vec3<f32>(jag * len * 0.16 * sin(f * PI), 0.0);
    }
    return p;
}

fn to_screen(p: vec3<f32>) -> vec3<f32> {
    let c = globals.view_proj * vec4<f32>(p, 1.0);
    let w = max(c.w, 1e-3);
    return vec3<f32>((c.x / w * 0.5 + 0.5) * globals.viewport.x, (0.5 - c.y / w * 0.5) * globals.viewport.y, c.z / w);
}

@fragment
fn fs_composite(in: FullOut) -> @location(0) vec4<f32> {
    // A small tent over the accumulated clouds, whose texels are several pixels wide.
    let texel = 1.0 / vec2<f32>(textureDimensions(cloud_now));
    let t = texel * 0.5;
    var cloud = textureSampleLevel(cloud_now, clamp_sampler, in.uv, 0.0) * 0.36;
    cloud += textureSampleLevel(cloud_now, clamp_sampler, in.uv + vec2<f32>(t.x, t.y), 0.0) * 0.16;
    cloud += textureSampleLevel(cloud_now, clamp_sampler, in.uv + vec2<f32>(-t.x, t.y), 0.0) * 0.16;
    cloud += textureSampleLevel(cloud_now, clamp_sampler, in.uv + vec2<f32>(t.x, -t.y), 0.0) * 0.16;
    cloud += textureSampleLevel(cloud_now, clamp_sampler, in.uv + vec2<f32>(-t.x, -t.y), 0.0) * 0.16;
    var light = cloud.rgb;
    var cover = 1.0 - cloud.a;

    // With something selected, the middle of the screen sees through the clouds below the camera: they
    // stay, as a faint hologram of themselves (their outline and contour lines
    // in the interface's cyan), so the battle under them shows.
    let eye = globals.camera.xyz;
    let aspect = globals.viewport.x / globals.viewport.y;
    let off_centre = length((in.uv - 0.5) * vec2<f32>(aspect, 1.0));
    let down = smoothstep(0.1, 0.4, -view_ray(in.uv).z) * smoothstep(atmos.frame.w + atmos.layer.x - 400.0, atmos.frame.w + atmos.layer.x + 200.0, eye.z);
    // Only while the player has something selected: otherwise the sky is left alone.
    let holo = (1.0 - smoothstep(0.18, 0.5, off_centre)) * down * atmos.frame.z;
    let e = texel * 3.0;
    let ax = textureSampleLevel(cloud_now, clamp_sampler, in.uv + vec2<f32>(e.x, 0.0), 0.0).a
        - textureSampleLevel(cloud_now, clamp_sampler, in.uv - vec2<f32>(e.x, 0.0), 0.0).a;
    let ay = textureSampleLevel(cloud_now, clamp_sampler, in.uv + vec2<f32>(0.0, e.y), 0.0).a
        - textureSampleLevel(cloud_now, clamp_sampler, in.uv - vec2<f32>(0.0, e.y), 0.0).a;
    let rim = smoothstep(0.1, 0.45, abs(ax) + abs(ay));
    let level = cover * 5.0;
    let band = abs(fract(level + 0.5) - 0.5);
    // Soft, a few pixels wide: a hairline would crawl as the cloud drifts.
    let px = max(fwidth(level), 1e-4);
    let contour = (1.0 - smoothstep(px * 0.5, px * 3.0, band)) * smoothstep(0.05, 0.2, cover);
    // At the zooms the battle is fought at, the clouds are a veil over it;
    // pulled back to the strategic view they are solid.
    let veil = mix(0.55, 1.0, smoothstep(3000.0, 10000.0, atmos.view.z));
    cover *= veil;
    light *= veil;
    if holo > 0.0 {
        let cyan = vec3<f32>(0.35, 0.85, 1.0);
        let ghost = light * 0.14 + cyan * (rim * 0.6 + contour * 0.2) * 0.45;
        light = mix(light, ghost, holo);
        cover = mix(cover, cover * 0.14, holo);
    }
    // Output pixels, as `to_screen` gives, so a bolt is as thin at any render scale.
    let pixel = in.clip.xy / globals.scene.z;
    let scene = textureLoad(scene_depth, vec2<i32>(in.clip.xy), 0);
    let flash_count = u32(atmos.counts.y);
    for (var b = 0u; b < flash_count; b++) {
        let bolt = atmos.bolts[b];
        if bolt.w <= 0.0 {
            continue;
        }
        // From the cloud's base under the flash down to the ground it strikes.
        let ground = vec3<f32>(bolt.xy, terrain_height(bolt.xy));
        let top = vec3<f32>(atmos.flashes[b].xy, cloud_floor(atmos.flashes[b].xy) + atmos.layer.x);
        var near = 1.0e6;
        var at_z = 0.0;
        var prev = to_screen(bolt_point(top, ground, bolt.z, 0, 14));
        for (var k = 1; k <= 14; k++) {
            let next = to_screen(bolt_point(top, ground, bolt.z, k, 14));
            let seg = next.xy - prev.xy;
            let f = clamp(dot(pixel - prev.xy, seg) / max(dot(seg, seg), 1e-4), 0.0, 1.0);
            let d = distance(pixel, prev.xy + seg * f);
            if d < near && prev.z > 0.0 && next.z > 0.0 {
                near = d;
                at_z = mix(prev.z, next.z, f);
            }
            prev = next;
        }
        // Hidden behind nearer ground or hulls.
        if at_z + 1e-7 < scene {
            continue;
        }
        let core = exp(-near * near / 2.2);
        let halo = exp(-near / 14.0) * 0.12 + exp(-near / 90.0) * 0.02;
        light += vec3<f32>(0.8, 0.86, 1.0) * bolt.w * (core * 26.0 + halo * 4.0) * mix(0.35, 1.0, cloud.a);
    }
    return vec4<f32>(light, cover);
}
