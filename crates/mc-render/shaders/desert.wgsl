// Canyon-country desert (`desert()` in bindings.wgsl): Grand Canyon and Lake
// Powell. build.rs inserts this file after habitat.wgsl into shaders that
// contain the line `//!use desert` (terrain.wgsl).
//
// The rock is horizontal beds, coloured by height above the lake: the bake
// (crates/mc-map/src/bake/canyon.rs, which holds the same numbers) cuts cliffs
// at the hard beds and slopes and benches at the soft ones, so a cliff shows the
// bed it was cut from. Cliffs carry the beds, their thin inner layers, desert
// varnish streaking down them and talus below; flat ground is soil, sand and
// slickrock of the bed it lies on, dotted with dark shrubs. Below the old
// full-pool line the reservoir's bathtub ring bleaches everything: white
// mineral crust on the rock, pale cracked silt on gentle ground.

// ---- The beds, metres above the lake (alt = z - water) --------------------
// The same numbers as crates/mc-map/src/bake/canyon.rs.
// Top of the bathtub ring: the old full-pool line.
const CANYON_RING_TOP: f32 = 55.0;
// Inner gorge schist and granite, 0..52, mostly under the ring's crust.
const CANYON_GORGE_TOP: f32 = 52.0;
// Tapeats sandstone ledge, 52..66.
const CANYON_TAPEATS_TOP: f32 = 66.0;
// The Tonto bench, 64..100: broad, gentle, playable shale ground.
const CANYON_BENCH_BASE: f32 = 64.0;
const CANYON_BENCH_TOP: f32 = 100.0;
// The Redwall cliff, 100..190.
const CANYON_REDWALL_TOP: f32 = 190.0;
// Supai group stair-steps, 190..275.
const CANYON_SUPAI_TOP: f32 = 275.0;
// Hermit shale slope, 275..300.
const CANYON_HERMIT_TOP: f32 = 300.0;
// Coconino sandstone cliff, 300..340.
const CANYON_COCONINO_TOP: f32 = 340.0;
// Kaibab/Toroweap cap and rim, 340..370.
const CANYON_KAIBAB_TOP: f32 = 370.0;
// The rim plateau from here up (to ~430).
const CANYON_RIM_BASE: f32 = 365.0;

// ---- Colours (linear albedo) ------------------------------------------------
const CANYON_CRUST: vec3<f32> = vec3<f32>(0.47, 0.44, 0.38);
const CANYON_SILT: vec3<f32> = vec3<f32>(0.27, 0.235, 0.18);
const CANYON_SCHIST: vec3<f32> = vec3<f32>(0.065, 0.057, 0.054);
const CANYON_VEIN: vec3<f32> = vec3<f32>(0.22, 0.11, 0.085);
const CANYON_TAPEATS: vec3<f32> = vec3<f32>(0.19, 0.10, 0.05);
const CANYON_SHALE: vec3<f32> = vec3<f32>(0.21, 0.18, 0.095);
const CANYON_LIMESTONE: vec3<f32> = vec3<f32>(0.26, 0.225, 0.19);
const CANYON_REDWALL: vec3<f32> = vec3<f32>(0.25, 0.085, 0.045);
const CANYON_SUPAI: vec3<f32> = vec3<f32>(0.30, 0.07, 0.032);
const CANYON_SUPAI_SLOPE: vec3<f32> = vec3<f32>(0.235, 0.062, 0.03);
const CANYON_HERMIT: vec3<f32> = vec3<f32>(0.25, 0.055, 0.028);
const CANYON_COCONINO: vec3<f32> = vec3<f32>(0.42, 0.315, 0.19);
const CANYON_KAIBAB: vec3<f32> = vec3<f32>(0.31, 0.27, 0.21);
const CANYON_RIM_SOIL: vec3<f32> = vec3<f32>(0.29, 0.22, 0.15);
const CANYON_BENCH_SOIL: vec3<f32> = vec3<f32>(0.25, 0.20, 0.11);
const CANYON_RED_SOIL: vec3<f32> = vec3<f32>(0.27, 0.095, 0.05);
const CANYON_RED_SAND: vec3<f32> = vec3<f32>(0.32, 0.15, 0.08);
const CANYON_BEACH: vec3<f32> = vec3<f32>(0.44, 0.35, 0.26);
// Broad patches over any flat ground: pale caliche and dark desert pavement.
const CANYON_CALICHE: vec3<f32> = vec3<f32>(0.36, 0.33, 0.27);
const CANYON_PAVEMENT: vec3<f32> = vec3<f32>(0.12, 0.10, 0.08);
const CANYON_VARNISH: vec3<f32> = vec3<f32>(0.035, 0.026, 0.022);
// Shrubs: blackbrush low down, grey-green sage on the rim.
const CANYON_BLACKBRUSH: vec3<f32> = vec3<f32>(0.058, 0.058, 0.043);
const CANYON_SAGE: vec3<f32> = vec3<f32>(0.13, 0.14, 0.105);

// Where the beds lie under `xy`: the height shifted by a slow wobble of a few
// metres, since real beds are only near level.
fn canyon_bed_alt(xy: vec2<f32>, alt: f32) -> f32 {
    return alt + (grad_noise2(xy + 1733.0, 1100.0) - 0.5) * 5.0 + (grad_noise2(xy - 911.0, 260.0) - 0.5) * 2.0;
}

fn canyon_edge(a: f32, at: f32, w: f32) -> f32 {
    return smoothstep(at - w, at + w, a);
}

// Streaks running down a steep face, 0-1 around 0.5: noise drawn out `run`
// metres down the face and `cell` across it. Laid from both horizontal axes and
// blended by which way the face looks, so no frame turns with the wall. Once a
// streak is narrower than a pixel it flattens to its mean.
fn canyon_streak(xy: vec2<f32>, a: f32, n: vec3<f32>, px: f32, cell: f32, run: f32, seed: f32) -> f32 {
    let wx = n.x * n.x;
    let wy = n.y * n.y;
    let k = smoothstep(0.4, 0.6, wx / max(wx + wy, 1e-4));
    let facing_x = grad_noise2(vec2<f32>(xy.y / cell, a / run) + seed, 1.0);
    let facing_y = grad_noise2(vec2<f32>(xy.x / cell, a / run) - seed, 1.0);
    // Blended noise keeps its contrast (as ice_flow_mix does).
    let keep = inverseSqrt(k * k + (1.0 - k) * (1.0 - k));
    let s = 0.5 + (mix(facing_y, facing_x, k) - 0.5) * keep;
    return mix(s, 0.5, smoothstep(0.35, 1.1, px / cell));
}

// The rock of one bed as a cliff shows it.
struct CanyonRock {
    rgb: vec3<f32>,
    // How much the bed takes varnish streaks (the great cliffs most).
    varnish: f32,
    // Its thin beds' tones and ledges (lit lips over shadowed notches): a
    // brightness factor about 0, for steep faces only (on a slope they would
    // read as contour lines).
    ledge: f32,
}

// The rock at bed height `a`. `streak` is the face's streak field (canyon_streak),
// `w` metres of height a pixel spans (for anti-aliasing the bed edges).
fn canyon_rock(a: f32, xy: vec2<f32>, streak: f32, w_in: f32) -> CanyonRock {
    let w = max(w_in, 0.6);
    var rgb = mix(CANYON_SCHIST, CANYON_VEIN, smoothstep(0.62, 0.72, streak) * 0.6);
    var varnish = 0.3;
    // Thin beds inside each formation: how thick, and a seed for their tones.
    var thin = 2.5;
    var seed = 1.0;
    var contrast = 0.2;

    var k = canyon_edge(a, CANYON_GORGE_TOP, w);
    rgb = mix(rgb, CANYON_TAPEATS, k);
    varnish = mix(varnish, 0.55, k);
    if a > CANYON_GORGE_TOP { thin = 1.4; seed = 2.0; contrast = 0.3; }

    // Bright Angel shale: the greenish slope under and on the bench.
    k = canyon_edge(a, CANYON_TAPEATS_TOP, w);
    rgb = mix(rgb, CANYON_SHALE, k);
    varnish = mix(varnish, 0.1, k);
    if a > CANYON_TAPEATS_TOP { thin = 3.0; seed = 3.0; contrast = 0.12; }

    // Redwall: grey limestone stained rust red from the red beds above, in
    // streaks down the face.
    k = canyon_edge(a, CANYON_BENCH_TOP, w);
    let stain = clamp(0.78 + (streak - 0.5) * 1.4 + smoothstep(150.0, 185.0, a) * 0.2, 0.0, 1.0);
    rgb = mix(rgb, mix(CANYON_LIMESTONE, CANYON_REDWALL, stain), k);
    varnish = mix(varnish, 1.0, k);
    if a > CANYON_BENCH_TOP { thin = 9.0; seed = 4.0; contrast = 0.1; }

    // Supai: red sandstone ledges and darker red slopes, stair-stepped.
    k = canyon_edge(a, CANYON_REDWALL_TOP, w);
    let step_i = floor((a - CANYON_REDWALL_TOP) / 11.0);
    let hard = step(0.42, hash11(step_i * 5.17 + 3.3));
    let supai = mix(CANYON_SUPAI_SLOPE, CANYON_SUPAI, hard);
    rgb = mix(rgb, supai, k);
    varnish = mix(varnish, 0.5, k);
    if a > CANYON_REDWALL_TOP { thin = 3.6; seed = 5.0; contrast = 0.2; }

    // Hermit shale: a deep red slope.
    k = canyon_edge(a, CANYON_SUPAI_TOP, w);
    rgb = mix(rgb, CANYON_HERMIT, k);
    varnish = mix(varnish, 0.2, k);
    if a > CANYON_SUPAI_TOP { thin = 2.0; seed = 6.0; contrast = 0.1; }

    // Coconino: a cream cliff of old dunes; its red wash from the Hermit below
    // never reaches up it, so the contact is sharp.
    k = canyon_edge(a, CANYON_HERMIT_TOP, w);
    rgb = mix(rgb, CANYON_COCONINO, k);
    varnish = mix(varnish, 0.8, k);
    if a > CANYON_HERMIT_TOP { thin = 7.0; seed = 7.0; contrast = 0.08; }

    // Toroweap and Kaibab: pale grey-cream limestone cap and rim.
    k = canyon_edge(a, CANYON_COCONINO_TOP, w);
    rgb = mix(rgb, CANYON_KAIBAB, k);
    varnish = mix(varnish, 0.55, k);
    if a > CANYON_COCONINO_TOP { thin = 4.0; seed = 8.0; contrast = 0.16; }

    // Thin beds: each its own tone, some standing out as ledges (a lit lip over a
    // shadowed notch). They fade as a bed shrinks toward a pixel.
    // Beds of uneven thickness that thin and swell along the wall.
    let t = a / thin + (grad_noise2(vec2<f32>(a / thin * 0.6, seed * 17.0), 1.0) - 0.5) * 1.2
        + (grad_noise2(xy + seed * 71.0, 380.0) - 0.5) * 0.8;
    let i = floor(t);
    let f = t - i;
    let tone = hash11(i * 7.31 + seed * 131.7);
    let shown = 1.0 - smoothstep(0.25, 0.7, w * 2.0 / thin);
    let thin_tone = (tone - 0.5) * contrast * 2.0 * shown;
    let ledge_bed = step(0.45, tone);
    let lip = smoothstep(0.78, 0.97, f);
    let notch = 1.0 - smoothstep(0.0, 0.14, f);
    var out: CanyonRock;
    out.rgb = rgb;
    out.varnish = varnish;
    out.ledge = thin_tone + (0.2 * lip - 0.35 * notch) * ledge_bed * shown;
    return out;
}

// Soil on flat ground at bed height `a`: grey-white silt in the old lake bed,
// brown Tapeats grit, khaki shale soil on the bench, rusty soil under the
// Redwall, red soil in the red beds, buff sand on the Coconino, tan-rust rim soil.
fn canyon_soil(a: f32) -> vec3<f32> {
    var c = CANYON_SILT;
    c = mix(c, vec3<f32>(0.23, 0.165, 0.11), canyon_edge(a, CANYON_RING_TOP, 2.0));
    c = mix(c, CANYON_BENCH_SOIL, canyon_edge(a, CANYON_BENCH_BASE + 2.0, 3.0));
    c = mix(c, vec3<f32>(0.25, 0.155, 0.10), canyon_edge(a, CANYON_BENCH_TOP, 5.0));
    c = mix(c, CANYON_RED_SOIL, canyon_edge(a, CANYON_REDWALL_TOP, 5.0));
    c = mix(c, vec3<f32>(0.34, 0.27, 0.18), canyon_edge(a, CANYON_HERMIT_TOP, 5.0));
    c = mix(c, CANYON_RIM_SOIL, canyon_edge(a, CANYON_COCONINO_TOP + 5.0, 5.0));
    return c;
}

// What one spot of canyon country is made of, worked out once per pixel.
struct CanyonSite {
    // Bed height (canyon_bed_alt).
    a: f32,
    // The face's streaks: fine and broad.
    streak: f32,
    streak_wide: f32,
    // The bed here, and the rock fallen from the beds just above (talus).
    rock: CanyonRock,
    talus: vec3<f32>,
    soil: vec3<f32>,
    sand: vec3<f32>,
}

fn canyon_site(xy: vec2<f32>, alt: f32, n: vec3<f32>, px: f32, dz: f32) -> CanyonSite {
    var s: CanyonSite;
    s.a = canyon_bed_alt(xy, alt);
    s.streak = canyon_streak(xy, s.a, n, px, 2.6, 26.0, 17.0);
    s.streak_wide = canyon_streak(xy, s.a, n, px, 11.0, 90.0, -53.0);
    let field = s.streak * 0.5 + s.streak_wide * 0.5;
    s.rock = canyon_rock(s.a, xy, field, dz);
    // Weathering differs along the walls: some reaches fresher, some darker.
    s.rock.rgb *= 0.84 + 0.32 * grad_noise2(xy - 1201.0, 420.0);
    // Debris falls down the slope from the cliff above it.
    let above = canyon_rock(s.a + 22.0, xy, field, 4.0);
    s.talus = mix(above.rgb, s.rock.rgb, 0.35) * 0.9;
    // Flat ground in broad patches: pale caliche crust, dark varnished pavement.
    let lay = grad_noise2(xy + 67.0, 170.0) * 0.6 + grad_noise2(xy - 311.0, 47.0) * 0.4;
    s.soil = canyon_soil(s.a);
    s.soil = mix(s.soil, CANYON_CALICHE, smoothstep(0.52, 0.76, lay) * 0.45);
    s.soil = mix(s.soil, CANYON_PAVEMENT, (1.0 - smoothstep(0.24, 0.47, lay)) * 0.35);
    // On the rim, the Kaibab's pale limestone breaks through the soil in ledges.
    let ledges = smoothstep(0.5, 0.62, grad_noise2(xy + 919.0, 90.0) * 0.6 + grad_noise2(xy - 57.0, 23.0) * 0.4);
    s.soil = mix(s.soil, CANYON_KAIBAB * 1.1, ledges * 0.7 * canyon_edge(s.a, CANYON_RIM_BASE, 4.0));
    // Pale sand on the lake's beaches, red-orange sand in the washes above.
    s.sand = mix(CANYON_BEACH, CANYON_RED_SAND, smoothstep(3.0, 14.0, alt)) * (0.9 + 0.2 * lay);
    return s;
}

// A ground scan recoloured for canyon country: the scan gives only its light and
// dark (against its own mean, its mip tail), the site gives the colour. Scree
// is talus, rocky highland is slickrock of the bed, sand is sand, the rest soil.
fn canyon_ground(p: TerrainPatch, m: i32, site: CanyonSite) -> vec3<f32> {
    let luma = vec3<f32>(0.2126, 0.7152, 0.0722);
    let mean = textureSampleLevel(terrain_materials, repeat_sampler, vec2<f32>(0.5), m * 2, 12.0).rgb;
    let detail = clamp(pow(max(dot(p.color.rgb, luma), 1e-4) / max(dot(mean, luma), 1e-4), 0.55), 0.45, 1.7);
    var want = site.soil;
    switch m {
        case 5: { want = site.talus; }
        case 7: { want = mix(site.rock.rgb, site.soil, 0.25) * 1.05; }
        case 8: { want = site.sand; }
        default: {}
    }
    return want * detail;
}

// The bathtub ring, which the reservoir left when it stood higher.
struct CanyonRing {
    // White crust on rock, 0-1; pale silt on gentle ground; a dark line of
    // wet rock and algae at the water's edge.
    crust: f32,
    silt: f32,
    wet: f32,
    // Faint lines of older, lower stands inside the ring (a brightness factor about 0).
    lines: f32,
}

// `alt` is the true height above the lake (the ring is level, unlike the beds),
// `steep` how much of the ground is face, `dz` metres of height a pixel spans.
fn canyon_ring(xy: vec2<f32>, alt: f32, steep: f32, streak: f32, dz: f32) -> CanyonRing {
    var r: CanyonRing;
    let top = CANYON_RING_TOP + (grad_noise2(xy + 71.0, 700.0) - 0.5) * 0.5;
    let w = max(dz * 0.7, 0.05);
    // A crisp upper edge: the line the water stood at for years.
    let inside = (1.0 - smoothstep(top - w - 0.12, top + w, alt)) * smoothstep(-0.05, 0.15, alt);
    // Whitest just under the top, where the water stood longest; streaky lower
    // down, where the crust ran and washed.
    let fresh = smoothstep(top - 16.0, top - 1.5, alt);
    let patchy = 0.72 + 0.28 * smoothstep(0.3, 0.7, streak);
    r.crust = inside * steep * mix(0.78 * patchy, 1.0, fresh);
    r.silt = inside * (1.0 - steep);
    // Older stands: thin lines at levels the lake held for a season.
    var lines = 0.0;
    let levels = array<f32, 5>(44.5, 37.0, 28.0, 17.0, 8.5);
    for (var i = 0; i < 5; i++) {
        let at = levels[i] + (grad_noise2(xy + f32(i) * 97.0, 500.0) - 0.5) * 0.6;
        let d = alt - at;
        let band = 1.0 - smoothstep(0.0, max(0.5, w * 1.5), abs(d + 0.4));
        let lip = 1.0 - smoothstep(0.0, max(0.25, w), abs(d - 0.2));
        lines += (lip * 0.12 - band * 0.18) * (1.0 - smoothstep(0.3, 1.2, w));
    }
    r.lines = lines * inside * steep;
    r.wet = (1.0 - smoothstep(0.3, 1.6, alt)) * smoothstep(-0.05, 0.1, alt);
    return r;
}

// Mud cracked into plates as the old lake bed dried: how dark the cracks make
// a pixel, 0-1, faded to its mean as the plates near a pixel.
fn canyon_mud_cracks(xy: vec2<f32>, px: f32) -> f32 {
    let cell = 1.3;
    let p = xy / cell + (vec2<f32>(grad_noise2(xy, 3.0), grad_noise2(xy + 9.0, 3.0)) - 0.5) * 0.5;
    let i = floor(p);
    let f = p - i;
    var d1 = 8.0;
    var d2 = 8.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let g = vec2<f32>(f32(x), f32(y));
            let h = i + g;
            let r = g + vec2<f32>(hash21(h + 41.0), hash21(h + 3.7)) * 0.8 + 0.1 - f;
            let d = dot(r, r);
            if d < d1 {
                d2 = d1;
                d1 = d;
            } else if d < d2 {
                d2 = d;
            }
        }
    }
    let edge = sqrt(d2) - sqrt(d1);
    let pc = px / cell;
    let width = 0.06;
    let aa = max(width, pc * 1.5);
    let line = (1.0 - smoothstep(0.0, aa, edge)) * (width / aa);
    return mix(line, width * 1.4, smoothstep(0.25, 0.7, pc));
}

// Desert shrubs dotting the ground: round, dark clumps a metre or two across,
// each with its shadow cast away from the sun.
struct CanyonShrubs {
    // How much of the pixel is shrub, and how much is its shadow on the ground.
    cover: f32,
    // Twigs and leaf clumps inside the shrub: a brightness factor near 1.
    leaf: f32,
    shadow: f32,
    // The shrub's surface gradient (a low dome), and its own random.
    grad: vec2<f32>,
    tone: f32,
    // The shrub's radius (metres) and how high its dome stands here (1 in the
    // middle, 0 at the edge), for the bushes grown up close (grass_gen.wgsl).
    size: f32,
    rise: f32,
}

const CANYON_SHRUB_CELL: f32 = 4.2;

// `density` 0-1 is the chance a cell holds a shrub. Shrubs sit anywhere in
// their cell and reach at most half a cell with their shadow, so the four
// nearest cells hold everything that can touch this point. Past a few pixels
// a shrub fades to its share of the ground, so the dotting never sparkles.
fn canyon_shrubs(xy: vec2<f32>, density: f32, px: f32) -> CanyonShrubs {
    var out: CanyonShrubs;
    out.tone = 0.5;
    out.leaf = 1.0;
    if density <= 0.001 {
        return out;
    }
    let cell = CANYON_SHRUB_CELL;
    let sun = globals.sun.xyz;
    let flat_sun = length(sun.xy);
    let away = -sun.xy / max(flat_sun, 1e-4);
    // Shadow length per metre of shrub height, kept inside the cell.
    let throw_k = min(flat_sun / max(sun.z, 0.05), 1.6);
    let q = xy / cell - 0.5;
    let base = floor(q);
    let mean_r = 0.8;
    // The share a shrub and its shadow cover, for the far fade.
    let mean_cover = density * 3.14159 * mean_r * mean_r / (cell * cell);
    let shown = 1.0 - smoothstep(0.45, 1.3, px / mean_r);
    if shown > 0.0 {
        for (var j = 0; j < 2; j++) {
            for (var i = 0; i < 2; i++) {
                let id = base + vec2<f32>(f32(i), f32(j));
                if hash21(id + 5.3) > density {
                    continue;
                }
                let r = 0.45 + 0.75 * hash21(id + 11.7);
                let c = (id + vec2<f32>(hash21(id + 2.1), hash21(id + 8.9))) * cell;
                let d = xy - c;
                // A lobed, not quite round clump.
                let ang = atan2(d.y, d.x);
                let lobe = 1.0 + 0.16 * sin(ang * 3.0 + hash21(id) * 6.28) + 0.1 * sin(ang * 5.0 + hash21(id + 1.0) * 6.28);
                let dl = length(d) / (r * lobe);
                let aa = px / r;
                let body = 1.0 - smoothstep(1.0 - aa, 1.0 + aa, dl);
                // Its shadow: the same clump moved away from the sun.
                let sd = length(d - away * r * 0.9 * throw_k) / (r * lobe * 1.05);
                let shade = (1.0 - smoothstep(1.0 - aa, 1.0 + aa * 1.5, sd)) * (1.0 - body);
                if body > out.cover {
                    // A low dome, steepening to its edge.
                    out.grad = -d / max(length(d), 1e-3) * (0.4 + 1.4 * smoothstep(0.2, 1.0, dl)) * body;
                    out.tone = hash21(id + 23.0);
                    out.size = r;
                    out.rise = sqrt(max(1.0 - dl * dl, 0.0));
                }
                out.cover = max(out.cover, body);
                out.shadow = max(out.shadow, shade);
            }
        }
    }
    let twigs = 1.0 - smoothstep(0.06, 0.25, px);
    out.leaf = mix(1.0, 0.6 + 0.8 * grad_noise2(xy + 13.0, 0.3), twigs * out.cover);
    out.cover = mix(mean_cover * 0.6, out.cover, shown);
    out.shadow = mix(mean_cover * 0.5 * throw_k, out.shadow, shown);
    out.grad *= shown;
    return out;
}

// How many shrubs grow where: thickest on the bench and the rim, thin on the
// red beds, few in the old lake bed, none on anything steep or on a trail
// (`way`, Habitat::way).
fn canyon_shrub_density(xy: vec2<f32>, a: f32, alt: f32, slope: f32, sand: f32, canopy: f32, patchy: f32, way: f32) -> f32 {
    var d = 0.08;
    d = mix(d, 0.55, canyon_edge(a, CANYON_BENCH_BASE, 3.0));
    d = mix(d, 0.3, canyon_edge(a, CANYON_BENCH_TOP, 6.0));
    d = mix(d, 0.22, canyon_edge(a, CANYON_REDWALL_TOP, 6.0));
    d = mix(d, 0.3, canyon_edge(a, CANYON_HERMIT_TOP, 6.0));
    d = mix(d, 0.62, canyon_edge(a, CANYON_RIM_BASE, 8.0));
    // Old lake bed: a few only, above the water.
    d *= mix(0.12, 1.0, smoothstep(CANYON_RING_TOP - 1.0, CANYON_RING_TOP + 3.0, alt)) * smoothstep(0.5, 2.0, alt);
    let clumps = smoothstep(0.25, 0.75, grad_noise2(xy + 401.0, 70.0) * 0.7 + patchy * 0.5);
    return d * (0.25 + 1.1 * clumps) * (1.0 - smoothstep(0.035, 0.1, slope)) * (1.0 - sand * 0.7) * (1.0 - canopy * 0.6)
        * (1.0 - smoothstep(0.05, 0.4, way));
}

// A trail down the canyon (the map's ways layer): the soil of its bed trodden
// to a pale compacted dust, worn in patches and grooved along its run by wheels
// and treads, inside a margin of darker gravel and broken rock kicked out to
// its edges. Paler than the red and khaki ground round it, so the ways down the
// walls read from a battle camera.
struct CanyonTrail {
    rgb: vec3<f32>,
    // How much of the pixel is the trail's floor, and its gravel margin.
    cover: f32,
    margin: f32,
}

// The gravel margin round a trail's floor, 0-1, from Habitat::way.
fn canyon_trail_margin(way: f32) -> f32 {
    return smoothstep(0.03, 0.18, way) * (1.0 - smoothstep(0.38, 0.52, way));
}

// `way` is Habitat::way.
fn canyon_trail(xy: vec2<f32>, way: f32, site: CanyonSite, px: f32) -> CanyonTrail {
    var out: CanyonTrail;
    // Dust: the bed's own soil, bleached toward caliche by the traffic.
    var dust = mix(site.soil, CANYON_CALICHE, 0.32);
    // Worn in patches a few metres across: paler where it is packed, darker
    // where loose grit gathers.
    let worn = smoothstep(0.3, 0.7, grad_noise2(xy + 613.0, 8.0) * 0.55 + grad_noise2(xy - 271.0, 27.0) * 0.45);
    dust *= 0.78 + 0.42 * worn;
    // Grooves along the run, wheel and tread ruts: noise drawn out along the
    // trail and fine across it, faded as they near a pixel.
    let run = ground_way_heading(xy);
    let along = dot(xy, run);
    let across = dot(xy, vec2<f32>(-run.y, run.x));
    let ruts = grad_noise2(vec2<f32>(along / 40.0, across / 2.2) + 41.0, 1.0) * 0.6
        + grad_noise2(vec2<f32>(along / 15.0, across / 0.8) - 17.0, 1.0) * 0.4;
    let fade = 1.0 - smoothstep(0.5, 1.6, px);
    dust *= 1.0 + (smoothstep(0.3, 0.7, ruts) - 0.5) * 0.55 * fade;
    // Fine scuffs everywhere.
    let scuff = mix(0.5, grad_noise2(xy + 37.0, 1.4), 1.0 - smoothstep(0.35, 1.2, px));
    dust *= 0.88 + 0.24 * scuff;
    out.rgb = dust;
    out.cover = canyon_trail_floor(way);
    out.margin = canyon_trail_margin(way);
    return out;
}
