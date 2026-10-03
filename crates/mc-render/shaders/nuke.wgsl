//!use bindings
//!use nova
// Nuclear blasts and strategic missiles (renderer/nuke_fx.rs, docs/NUKES.md).
//
// A blast is a volume, marched at half size into a target of its own and laid over the
// picture (renderer/nuke_volume.rs):
//
//   fs_nuke_march      every ray finds the blasts' parts it crosses (the column, the
//                      ground's dust, the condensation shell), nearest first, and
//                      marches each against the scene's depth
//   fs_nuke_composite  in scene_over: the march through a small tent
//
// What it shows, in order: a fireball too bright to look at, which hangs on the ground;
// the shock going out fast along the ground and the Wilson cloud, a white shell of
// condensation that blooms round the fireball a moment behind the shock and is gone in
// seconds; the fireball rising slowly, its skin going to smoke while it still glows
// through the cracks and underneath, rolling over into a cap on a stem of dust; rings
// of condensation that the cap leaves round the stem as it climbs through wet air; and
// the base surge, a low ring of dust rolling outward. The cap drifts downwind and thins
// over minutes. The billows are the clouds' Perlin-Worley texture (sky.rs), warped, so
// no two blasts, and no two sides of one, are alike.
//
// A Regency warhead (`look` NUKE_LOOK_PLASMA) is the same body made of plasma: a star, a
// nova's shell, and a cloud of dark plasma laced with glowing threads that stands for
// minutes (nova.wgsl).
//
// Missiles are drawn here too: a lathed body a strategic missile long, and the plume
// of its motor, from `globals.strategic`.

// Mirrors nuke_fx::NUKE_SLOTS.
const NUKE_SLOTS: u32 = 64u;
// Stretches of one ray through blasts' parts, at most (a salvo's blasts overlap).
const MAX_SEGS: u32 = 32u;
const TAU: f32 = 6.2831853;

@group(1) @binding(0) var nuke_scene_depth: texture_depth_2d;
@group(1) @binding(1) var billow_tex: texture_3d<f32>;
@group(1) @binding(2) var nuke_half: texture_2d<f32>;
// The clouds' march this frame (its .z the distance to the cloud it saw) and its
// resolved picture (.a how much gets through): what of a blast is past the cloud deck
// is seen through it. Only read while atmos.ground_color.w says the clouds are drawn.
@group(1) @binding(3) var nuke_cloud_march: texture_2d<u32>;
@group(1) @binding(4) var nuke_cloud_now: texture_2d<f32>;

struct Blast {
    // Where it burst.
    at: vec3<f32>,
    // Seconds since.
    age: f32,
    // Size against a warhead's (1), seed, lightning in it now (0..1), the ground under it.
    scale: f32,
    seed: f32,
    flash: f32,
    ground: f32,
    // How far the wind has carried the smoke, xy; where lightning lights it, z height.
    drift: vec2<f32>,
    bolt_z: f32,
    // The shock's front on the ground, metres out (nuke_fx.rs `Blast::front`).
    front: f32,
    // A salvo (nuke_fx.rs "Salvos"): heat from fire folded into it and bursts near it;
    // churn the bursts near it have added to its boiling; how thick folded fire keeps it.
    fuel: f32,
    churn: f32,
    thick: f32,
    // NUKE_LOOK_*: ARC's fire and smoke, or the Regency's plasma (nova.wgsl).
    look: u32,
}

fn blast_of(i: u32) -> Blast {
    let a = globals.nukes[i * 4u];
    let b = globals.nukes[i * 4u + 1u];
    let c = globals.nukes[i * 4u + 2u];
    let d = globals.nukes[i * 4u + 3u];
    var n: Blast;
    n.at = a.xyz;
    n.age = max(globals.camera.w - a.w, 0.0);
    n.scale = b.x;
    n.seed = b.y;
    n.flash = b.z;
    n.ground = b.w;
    n.drift = c.xy;
    n.bolt_z = c.z;
    n.front = c.w;
    n.fuel = d.x;
    n.churn = d.z;
    n.thick = d.w;
    n.look = u32(d.y + 0.5);
    return n;
}

// ---- shape over time (mirrored in nuke_fx.rs) ------------------------------------------

// Radius of the fireball, which becomes the cap: out in a second, then growing slowly as
// it climbs and draws air in.
fn head_radius(n: Blast) -> f32 {
    if is_nova(n) {
        return nova_radius(n);
    }
    let t = n.age;
    return n.scale * (240.0 * sqrt(1.0 - exp(-t * 3.0)) + 430.0 * (1.0 - exp(-t / 26.0)));
}

// Height of the head's middle over the burst: it hangs a few seconds, then climbs, and
// takes about a minute to reach its height.
fn head_height(n: Blast) -> f32 {
    if is_nova(n) {
        // A nova's star stays on the burst.
        return 0.0;
    }
    let x = pow(n.age / 30.0, 1.35);
    return rise(n) * 1700.0 * (1.0 - exp(-x));
}

// What the height scales by: above a warhead's first size (scale 1) the cloud climbs
// with scale^0.6 while it spreads with scale, as a real cloud's top rises far slower with
// yield than its width grows. Mirrored in nuke_fx.rs.
fn rise(n: Blast) -> f32 {
    return min(n.scale, pow(n.scale, 0.6));
}

// How far the ball has rolled over into a cap on a ring: 0 a ball, 1 a mushroom's cap.
fn roll(n: Blast) -> f32 {
    return select(smoothstep(4.0, 24.0, n.age), 0.0, is_nova(n));
}

// How much slower a bigger blast's fire runs: a megaton ball burns for many seconds.
// Mirrored in nuke_fx.rs.
fn slow(n: Blast) -> f32 {
    return sqrt(max(n.scale, 0.3));
}

// Heat left, 0..1: the flash, then the long red fade under the cap.
fn heat_left(n: Blast) -> f32 {
    let k = slow(n);
    return max(exp(-n.age / (2.5 * k)) * 0.5 + exp(-n.age / (16.0 * k)) * 0.5, n.fuel);
}

// How far the fireball's skin has gone to smoke: at first it is all fire, and it rises
// still burning, the soot gathering on its crowns while the fire shows between them.
fn skin(n: Blast) -> f32 {
    let k = slow(n);
    return smoothstep(1.0 * k, 12.0 * k, n.age);
}

// Everything thins out over a few minutes.
// Solid only at first: from a few seconds on the cloud thins steadily (to a fortieth of
// its density by about 25 s, and no thinner, which reads as the soft, see-through cloud it ends as), and it is
// gone by 75 s. Mirrored by nuke_fx::BLAST_LIFE.
// Fire folded into it by a salvo keeps it thick (`thick`).
fn fade_left(n: Blast) -> f32 {
    let k = slow(n);
    let thin = max(exp(-4.5 * smoothstep(3.0 * k, 30.0 * k, n.age)), 0.025);
    return max(thin * (1.0 - smoothstep(45.0, 75.0, n.age)), n.thick);
}

// How much of the Wilson cloud there is: it blooms a moment behind the shock and is
// gone in a few seconds.
fn wilson_left(n: Blast) -> f32 {
    return smoothstep(0.12, 0.5, n.age) * (1.0 - smoothstep(1.3, 3.8, n.age));
}

// ---- noise -------------------------------------------------------------------------------

fn billow_at(p: vec3<f32>) -> vec4<f32> {
    return textureSampleLevel(billow_tex, repeat_sampler, p, 0.0);
}

// Billows warped by a slower field, so no lattice or repeat shows: 0..1, high on the
// crowns.
fn billows(p: vec3<f32>, seed: f32) -> f32 {
    return boiling(p, seed, 0.0);
}

// The same billows with the warping field moving through `phase`: the surface churns,
// crowns swelling and sinking, instead of sliding or standing still.
fn boiling(p: vec3<f32>, seed: f32, phase: f32) -> f32 {
    let w = billow_at(p * 0.37 + vec3<f32>(seed * 0.71, seed * 0.29, seed * 0.53 + phase));
    let q = p + (w.gba - 0.5) * 0.55;
    let b = billow_at(q);
    return b.r * 0.72 + b.g * 0.28;
}

// ---- the column: fireball, cap, stem and the collars round it -----------------------------

struct Sample {
    density: f32,
    // 0..1: how hot this part is (it glows).
    heat: f32,
    // 0 smoke, 1 dust off the ground (browner, paler).
    dust: f32,
    // 0..1: condensation (white).
    wet: f32,
    // 0 deep in a crevice between billows, 1 out on a billow's crown.
    open: f32,
}

fn empty_sample() -> Sample {
    var s: Sample;
    s.density = 0.0;
    s.heat = 0.0;
    s.dust = 0.0;
    s.wet = 0.0;
    s.open = 1.0;
    return s;
}

// The column leans off downwind as it climbs; a nova's star, held in its field, does not.
fn lean(n: Blast, up: f32) -> vec2<f32> {
    return select(1.0, 0.0, is_nova(n)) * n.drift * clamp(up / (1600.0 * rise(n)), 0.0, 1.0);
}

// The head's shape without its billows: < 1 inside. `p` is from the burst, leaned.
fn head_body(n: Blast, p: vec3<f32>, rc: f32, hc: f32, r: f32) -> f32 {
    let q = p - vec3<f32>(0.0, 0.0, hc);
    let rho = length(q.xy);
    let ang = atan2(q.y, q.x);
    // Never a perfect ball: a few broad lobes round it, different every blast.
    let lobe = 1.0 + 0.05 * sin(3.0 * ang + n.seed * TAU) + 0.035 * sin(5.0 * ang + n.seed * 17.0)
        + 0.03 * sin(2.0 * ang + n.seed * 41.0);
    let rl = rc * lobe;
    let squash = mix(1.0, 2.2, r);
    let ball = length(vec3<f32>(q.xy, q.z * squash)) / rl;
    // The cap: a vortex ring with the top filled over it, hollow underneath where the
    // stem goes in.
    let ring = rl * 0.62 * r;
    let tube = rl * mix(1.0, 0.46, r);
    let torus = length(vec2<f32>(rho - ring, q.z * 1.15)) / tube;
    let dome = length(vec3<f32>(q.xy, (q.z - tube * 0.35) * squash * 1.1)) / (rl * 0.92);
    return mix(ball, min(torus, dome), r);
}

fn column(n: Blast, world: vec3<f32>) -> Sample {
    var s = empty_sample();
    let up = world.z - n.at.z;
    let p = world - vec3<f32>(n.at.xy + lean(n, up), n.at.z);
    let rc = head_radius(n);
    let hc = head_height(n);
    let r = roll(n);
    let t = n.age;
    let seed = n.seed * 13.0;
    let sk = skin(n);

    // The head.
    let body = head_body(n, p, rc, hc, r);
    if body < 1.45 {
        let q = p - vec3<f32>(0.0, 0.0, hc);
        // The billows turn with the ring: up the middle, over the top, down the outside.
        let rho = length(q.xy);
        let dir = select(q.xy / max(rho, 1e-3), vec2<f32>(1.0, 0.0), rho < 1e-3);
        // It starts turning over as soon as it is a ball, not only once it is a cap.
        let a = t * 0.09 * max(r, 0.55 * smoothstep(0.6, 4.0, t));
        let pr = vec2<f32>(rho - rc * 0.58 * r, q.z);
        let turned = vec2<f32>(pr.x * cos(a) - pr.y * sin(a), pr.x * sin(a) + pr.y * cos(a));
        let np = vec3<f32>(dir * (rc * 0.58 * r + turned.x), turned.y) / (rc * 1.45)
            + vec3<f32>(0.0, 0.0, -t * 0.012);
        // Boiling: fast while the ball is young and hot, slowing to a churn in the cap.
        let k = slow(n);
        let phase = 0.55 * (1.0 - exp(-t / (2.0 * k))) + t * 0.018 / k + n.churn;
        let b = boiling(np, seed, phase);
        // Cauliflower: crowns bulge the surface out, crevices cut in, and finer knots
        // fray the edge; the knots churn faster than the billows they sit on.
        let fine = billow_at(np * 2.9 + vec3<f32>(seed * 0.3, 0.0, phase * 2.6)).g;
        // At first a brilliant, nearly smooth ball: it breaks into billows as it swells.
        let lumps = mix(0.3, 1.0, smoothstep(0.15, 1.6 * k, t));
        let edge = body - (b - 0.5) * 1.05 * lumps - (fine - 0.5) * 0.18 * lumps;
        // A firm surface while it is fire, softening as it goes to smoke.
        let d = smoothstep(1.0, mix(0.95, 0.86, skin(n)), edge);
        if d > 0.0 {
            s.density = d * 0.07 / max(n.scale, 0.3);
            // Which side of its billow this is: thinner toward the sun is the lit side,
            // so every lobe has a bright crown and a shadowed flank.
            let b_sun = boiling(np + globals.sun.xyz * 0.07, seed, phase);
            s.open = clamp(0.5 + (b - b_sun) * 8.0, 0.0, 1.0) * smoothstep(0.2, 0.6, b) * 0.75 + 0.25 * smoothstep(0.3, 0.75, b);
            // Hot deep inside; the skin cools to smoke first, and the fire shows through
            // the crevices between billows and out of the cap's underside.
            let deep = smoothstep(0.95, 0.3, body - (b - 0.5) * 0.4);
            let cracks = 1.0 - smoothstep(0.3, 0.6, b);
            let under = smoothstep(0.1, -0.8, q.z / rc) * r;
            // While it climbs, fire tears through the crevices between sooty crowns.
            let torn = cracks * cracks * (1.0 - smoothstep(6.0 * k, 22.0 * k, t)) * 0.8;
            let cool = max(deep * deep * mix(0.45, 1.0, cracks) + under * (0.35 + 0.4 * cracks), torn);
            // Even at first the ball is not one flat white: hotter deep in and in its
            // boiling knots, cooler at its limb.
            // The burning ball boils: bright knots where the billows well up, darker
            // mottling between them, and soot that gathers in patches as it cools.
            let knots = smoothstep(0.38, 0.78, b) * (0.75 + 0.5 * (fine - 0.5));
            // Soot gathers on the crowns first (they cool against the air), spreading down
            // them as it climbs, while the crevices between keep their fire.
            let soot_lo = mix(0.95, 0.3, smoothstep(1.0 * k, 9.0 * k, t));
            let soot = smoothstep(soot_lo, soot_lo + 0.22, b * 0.8 + fine * 0.3) * smoothstep(0.8 * k, 3.0 * k, t);
            let young = (0.55 + 0.45 * deep) * mix(0.4, 1.0, knots) * (1.0 - soot * 0.92);
            s.heat = clamp(mix(young, cool * (1.0 - soot * 0.8), sk), 0.0, 1.0);
        }
    }

    // The stem: the hot cloud rising draws cooler air and dust in along the ground and up
    // under it, so the stem is an updraft: dust converging on its foot, then streaming and
    // twisting upward into the cap's hollow underside, the same boiling billows as the
    // cap. Never a tube: wide at its foot, it narrows to a waist low down, then widens
    // steadily as it climbs until it opens into the cap. It grows once the fireball has lifted off the ground and stays under the cap
    // for as long as the cap is there, so the cap never hangs on nothing.
    // Fire fed into it by a salvo holds the stem up under the cap.
    let stem_on = smoothstep(2.5, 9.0, t) * max(1.0 - smoothstep(50.0, 72.0, t), smoothstep(0.02, 0.25, n.thick));
    let bottom = n.ground - n.at.z;
    let top = hc;
    if stem_on > 0.0 && up > bottom - 10.0 && up < top {
        let h = clamp((up - bottom) / max(top - bottom, 1.0), 0.0, 1.0);
        let skirt = exp(-(up - bottom) / (90.0 * n.scale));
        let flare = smoothstep(0.7, 1.0, h);
        let q = p.xy;
        let rr = length(q);
        let rise_speed = 55.0 * n.scale;
        // Faint swellings riding up it, so the edge is not ruled straight. Looked up
        // only near enough the stem to matter.
        var swell = 0.5;
        if rr < (n.scale * 34.0 + rc * 0.75 + skirt * rc * 0.6) * 1.9 {
            swell = billow_at(vec3<f32>(seed * 0.21, 0.5, (up - t * rise_speed) / (rc * 0.8))).r;
        }
        // The waist a quarter of the way up; above it the stem widens steadily, then
        // opens into the cap over its top fifth.
        let widen = smoothstep(0.25, 1.0, h);
        let rs = (n.scale * 34.0 + rc * (0.1 + 0.3 * widen * sqrt(widen) + 0.35 * flare * flare)) * (0.9 + 0.2 * swell)
            + skirt * rc * 0.6;
        if rr < rs * 1.7 {
            let k = slow(n);
            let phase = 0.55 * (1.0 - exp(-t / (2.0 * k))) + t * 0.018 / k + n.churn;
            // Drawn upward and twisting as it goes; at its foot, drawn in along the ground.
            let ang = atan2(q.y, q.x) + up / (rc * 2.2) + t * 0.04;
            let rad = rr + t * 30.0 * n.scale * skirt;
            let sp = vec3<f32>(cos(ang) * rad, sin(ang) * rad, 0.0) / (rc * 0.9)
                + vec3<f32>(0.0, 0.0, (up - t * rise_speed) / (rc * 1.9));
            let b = boiling(sp, seed + 3.0, phase);
            // Streaks along the flow: fine knots stretched upward.
            let fine = billow_at(sp * vec3<f32>(3.2, 3.2, 1.1) + vec3<f32>(seed * 0.3 + 5.0, 0.0, phase * 2.6)).g;
            let dd = rr / rs - (b - 0.5) * 0.95 - (fine - 0.5) * 0.3;
            // It thins only about two thirds as fast as the cap (`fade_left`^0.65; the whole
            // sample is multiplied by `fade_left` below): narrow, it would otherwise go
            // see-through long before the cap it holds up.
            let sd = smoothstep(1.0, 0.8, dd) * stem_on * 0.075 / max(n.scale, 0.3) * pow(max(fade_left(n), 1e-3), -0.35);
            if sd > s.density * 0.5 {
                // Dusty at its foot, the cap's smoke higher up.
                s.dust = 0.6 * (1.0 - smoothstep(0.0, 0.45, h)) + 0.35 * skirt;
                let b_sun = boiling(sp + globals.sun.xyz * 0.07, seed + 3.0, phase);
                s.open = clamp(0.5 + (b - b_sun) * 8.0, 0.0, 1.0) * smoothstep(0.2, 0.6, b) * 0.75 + 0.25 * smoothstep(0.3, 0.75, b);
                // Fire at its foot while the ground still burns, and the cap's glow at its top.
                let foot = exp(-(up - bottom) / (60.0 * n.scale)) * exp(-t / 6.0);
                s.heat = max(s.heat, max(flare * 0.3 * exp(-t / 12.0), foot * 0.7));
            }
            s.density = max(s.density, sd);
        }
    }

    // Collars: the cap pushes wet air up as it climbs through it, and a flat ring of
    // condensation is left round the stem at that height, spreading and fading.
    for (var k = 0; k < 1; k++) {
        let hk = rise(n) * (480.0 + 420.0 * f32(k));
        // When the head passed that height (head_height solved for the time).
        let tk = 30.0 * pow(max(-log(max(1.0 - hk / (1700.0 * rise(n)), 1e-3)), 0.0), 1.0 / 1.35);
        let since = t - tk;
        if since > 0.0 && since < 70.0 {
            let z = up - hk;
            let thick = n.scale * (10.0 + since * 0.5);
            if abs(z) < thick * 3.0 {
                let rr = length(p.xy);
                let rk = n.scale * (240.0 * sqrt(1.0 - exp(-tk * 3.0)) + 430.0 * (1.0 - exp(-tk / 26.0)));
                let outer = rk * (1.15 + 0.1 * f32(k)) + since * 5.0 * n.scale;
                let inner = n.scale * 120.0 + rk * 0.45;
                let ragged = billows(vec3<f32>(p.xy / (outer * 0.9), f32(k) * 0.37 + since * 0.004), seed + 7.0);
                let ring = smoothstep(inner, inner * 1.6, rr) * smoothstep(outer, outer * (0.55 + 0.3 * ragged), rr);
                let sheet = exp(-z * z / (thick * thick)) * ring * smoothstep(0.4, 0.75, ragged);
                let cd = sheet * smoothstep(0.0, 3.0, since) * exp(-since / 14.0) * 0.0022 / max(n.scale, 0.3);
                if cd > s.density {
                    s.wet = 1.0;
                    s.dust = 0.0;
                    s.heat = 0.0;
                    s.open = ragged;
                }
                s.density = max(s.density, cd);
            }
        }
    }
    s.density *= fade_left(n);
    return s;
}

// ---- the ground: the shock's dust curtain and the base surge -------------------------------

fn surge(n: Blast, world: vec3<f32>) -> Sample {
    var s = empty_sample();
    s.dust = 1.0;
    let xy = world.xy - n.at.xy;
    let d = length(xy);
    let up = world.z - n.ground;
    let t = n.age;
    if up < -8.0 {
        return s;
    }
    let dir = xy / max(d, 1.0);
    let b = billows(vec3<f32>(xy / (220.0 * n.scale), up / (140.0 * n.scale) - t * 0.02), n.seed * 5.0);
    let tatter = billows(vec3<f32>(dir * 2.3 + vec2<f32>(n.seed * 3.1), t * 0.01), n.seed * 9.0);
    // The shock's front: a thin curtain of dust whipped up where it passes, fast.
    let front = n.front;
    let behind = front - d;
    let width = 14.0 + front * 0.03;
    let curtain_h = n.scale * (22.0 + 26.0 * tatter) * (1.0 - smoothstep(0.0, 3200.0 * n.scale, front));
    let curtain = exp(-behind * behind / (2.0 * width * width)) * step(-width * 2.0, behind)
        * smoothstep(curtain_h, 0.0, up) * (1.0 - smoothstep(3.0, 8.0, t)) * 0.5;
    // The base surge: a low ring of dust that boils out along the ground and settles.
    let rr = n.scale * (140.0 + 980.0 * (1.0 - exp(-t / 15.0)));
    let hh = n.scale * (35.0 + 75.0 * (1.0 - exp(-t / 9.0))) * (0.55 + 0.9 * b);
    // Squared by hand: pow() of a negative base is NaN on NVIDIA (exp2(y * log2(x))).
    let dr = (d - rr) / (rr * 0.22);
    let ring = exp(-dr * dr) + smoothstep(rr, rr * 0.2, d) * 0.12;
    let surge_d = ring * smoothstep(hh, hh * 0.1, up) * smoothstep(0.25, 0.6, b * 0.7 + tatter * 0.45)
        * smoothstep(0.6, 3.0, t) * (1.0 - smoothstep(20.0, 55.0, t));
    s.density = (curtain + surge_d) * 0.0032 / max(n.scale, 0.3);
    s.open = b;
    return s;
}

// ---- the Wilson cloud: a shell of condensation round the fireball ------------------------

fn wilson(n: Blast, world: vec3<f32>) -> Sample {
    var s = empty_sample();
    s.wet = 1.0;
    let rw = n.front * 0.96;
    let v = world - n.at;
    let d = length(v);
    let up = world.z - n.ground;
    let th = rw * 0.05 + 10.0;
    let dw = (d - rw) / th;
    let shell = exp(-dw * dw);
    if shell < 0.01 || up < 0.0 {
        return s;
    }
    // In bands where the air was wetter, patchy, thinning toward the ground and gone
    // from the bottom up as it clears.
    let ew = v / max(d, 1.0);
    let b = billows(ew * 1.6 + vec3<f32>(n.seed, n.seed * 2.0, n.age * 0.03), n.seed * 11.0);
    let band = 0.75 + 0.25 * sin(up / (70.0 * n.scale) + b * 6.0 + n.seed * 6.0);
    // As it goes it breaks into patches and thins, all over at once.
    let gaps = mix(0.42, 0.7, smoothstep(0.8, 3.0, n.age));
    s.density = shell * wilson_left(n) * smoothstep(gaps, gaps + 0.35, b * 0.6 + band * 0.5) * smoothstep(0.0, 60.0, up)
        * 0.0008 / max(n.scale, 0.3);
    s.open = b;
    return s;
}

// The shell round the burst (part 2): the Wilson cloud, or a Regency nova's shell.
fn shell_left(n: Blast) -> f32 {
    return select(wilson_left(n), nova_shell_left(n), is_nova(n));
}

fn shell_radius(n: Blast) -> f32 {
    if is_nova(n) {
        return nova_shell_radius(n);
    }
    return n.front * 0.96;
}

fn shell_thick(n: Blast) -> f32 {
    return select(shell_radius(n) * 0.05 + 10.0, nova_shell_thick(n), is_nova(n));
}

// ---- light --------------------------------------------------------------------------------

// The fireball's colour at `heat`: white, yellow, orange, deep red.
fn fire_color(heat: f32) -> vec3<f32> {
    let h = clamp(heat, 0.0, 1.0);
    let red = vec3<f32>(0.9, 0.12, 0.02);
    let orange = vec3<f32>(1.0, 0.42, 0.06);
    let yellow = vec3<f32>(1.0, 0.78, 0.35);
    let white = vec3<f32>(1.0, 0.96, 0.9);
    var c = mix(red, orange, smoothstep(0.0, 0.4, h));
    c = mix(c, yellow, smoothstep(0.4, 0.75, h));
    c = mix(c, white, smoothstep(0.75, 1.0, h));
    return c;
}

struct Box {
    lo: vec3<f32>,
    hi: vec3<f32>,
}

fn column_box(n: Blast) -> Box {
    let rc = head_radius(n);
    let hc = head_height(n);
    var reach = max(rc * 1.9, n.scale * 1100.0 * smoothstep(8.0, 30.0, n.age)) + length(n.drift) + 60.0;
    var top = hc + rc * 1.1;
    if is_nova(n) {
        // The star and its rings.
        let rings = nova_ring_reach(n);
        reach = max(rc * 1.5, rings) + length(n.drift) + 60.0;
        top = hc + rc * 1.7 + rings * 0.4;
    }
    var b: Box;
    b.lo = vec3<f32>(n.at.xy - vec2<f32>(reach), n.ground - 20.0);
    b.hi = vec3<f32>(n.at.xy + vec2<f32>(reach), n.at.z + top + 60.0);
    return b;
}

fn surge_box(n: Blast) -> Box {
    var b: Box;
    // Out to the shock's front only while its dust curtain stands (8 s): after that the
    // base surge alone, which never spreads past 1500 m a warhead. Kept out at the front
    // it was a flat box kilometres across for a minute, and every blast of a salvo
    // overlapped every other along the ground, so all of them were marched together.
    let curtain = select(0.0, n.front, n.age < 8.0);
    let reach = max(curtain, n.scale * 1500.0) + 150.0;
    b.lo = vec3<f32>(n.at.xy - vec2<f32>(reach), n.ground - 10.0);
    b.hi = vec3<f32>(n.at.xy + vec2<f32>(reach), n.ground + 200.0 * n.scale + 20.0);
    if is_nova(n) {
        // A nova's sheet follows the ground in view up hills and down into hollows
        // (`nova_surge`, which lets it go before these bounds).
        b.lo.z = n.ground - 150.0 * n.scale - 10.0;
        b.hi.z = n.ground + 400.0 * n.scale + 20.0;
    }
    return b;
}

fn slab(ro: vec3<f32>, rd: vec3<f32>, b: Box) -> vec2<f32> {
    let inv = 1.0 / select(rd, vec3<f32>(1e-6), abs(rd) < vec3<f32>(1e-6));
    let t0 = (b.lo - ro) * inv;
    let t1 = (b.hi - ro) * inv;
    let near = max(max(min(t0.x, t1.x), min(t0.y, t1.y)), min(t0.z, t1.z));
    let far = min(min(max(t0.x, t1.x), max(t0.y, t1.y)), max(t0.z, t1.z));
    return vec2<f32>(max(near, 0.0), far);
}

// Where a ray enters and leaves a sphere (x > y: missed).
fn sphere(ro: vec3<f32>, rd: vec3<f32>, c: vec3<f32>, r: f32) -> vec2<f32> {
    let oc = ro - c;
    let b = dot(oc, rd);
    let disc = b * b - dot(oc, oc) + r * r;
    if disc <= 0.0 {
        return vec2<f32>(1.0, 0.0);
    }
    let q = sqrt(disc);
    return vec2<f32>(max(-b - q, 0.0), -b + q);
}

// A stretch of one ray through one part of one blast.
struct Seg {
    t0: f32,
    t1: f32,
    // blast index, then the part: 0 column, 1 ground, 2 condensation shell.
    blast: u32,
    part: u32,
}

struct Marched {
    light: vec3<f32>,
    trans: f32,
    // What the blast covers once the clouds in front are counted (`cloud_veil`).
    cover: f32,
    // Where along the ray most of what was seen is, for the haze.
    seen: f32,
}

fn shade_sample(n: Blast, part: u32, world: vec3<f32>, s: Sample, rc: f32, hc: f32, r: f32, heat0: f32) -> vec3<f32> {
    if is_nova(n) {
        return nova_shade(n, part, world, s, rc, hc, r);
    }
    let sun = globals.sun.xyz;
    let sun_col = atmos.sun_color.rgb * max(atmos.sun_color.w, 0.25);
    let sky = atmos.sky_color.rgb;
    // Self-shadow toward the sun from the head's shape a little way along it, and the
    // crevices between billows darker than their crowns.
    var shade = mix(0.16, 1.0, s.open);
    if part == 0u {
        let up = world.z - n.at.z;
        let p = world - vec3<f32>(n.at.xy + lean(n, up), n.at.z) + sun * rc * 0.45;
        let inside = max(1.15 - head_body(n, p, rc, hc, r), 0.0);
        shade *= exp(-inside * 3.2);
    }
    // Soot at first, going to a pale grey as the cap cools and water condenses in it.
    let smoke = mix(vec3<f32>(0.07, 0.06, 0.055), vec3<f32>(0.19, 0.185, 0.18), smoothstep(5.0 * slow(n), 30.0 * slow(n), n.age));
    let dust = vec3<f32>(0.1, 0.095, 0.09);
    let wet = vec3<f32>(0.62, 0.65, 0.7);
    let albedo = mix(mix(smoke, dust, s.dust), wet, s.wet);
    // What still burns is lit by its own fire, not the sun.
    let burning = clamp(s.heat * heat0 * 2.5, 0.0, 1.0);
    var c = albedo * (sun_col * (0.05 + 1.15 * shade) + sky * 0.25) * (1.0 - burning * 0.85);
    // The fireball lights what is round it: the stem under it, the condensation round it.
    let glow_at = n.at + vec3<f32>(lean(n, hc), hc);
    let to_glow = distance(world, glow_at);
    let reach = mix(rc * 0.8, max(n.front, rc) * 0.9, s.wet);
    // Long after the flash the cap's underside still glows a dull orange over the stem.
    let ember = 0.3 * exp(-n.age / 18.0) * smoothstep(3.0, 10.0, n.age);
    c += albedo * fire_color(0.25 + heat0 * 0.5) * (heat0 * heat0 * 8.0 + ember * 1.2) * mix(1.0, 0.12, s.wet)
        * exp(-to_glow / reach);
    // What still burns glows through: white-hot at first, then orange, then a deep red
    // seen in the cracks and under the cap.
    // The ball burns fiercely for its first seconds, knots yellow and gaps deep red,
    // then the fire sinks into the cap.
    let white = exp(-n.age / (0.8 * slow(n)));
    let blaze = exp(-n.age / (6.0 * slow(n)));
    let temp = s.heat * (0.35 + 0.65 * heat0) + 0.25 * white;
    // Not so bright once the flash is over that the tonemap bleaches the orange to pink.
    // The first seconds burn far past white so the bloom takes them; later the fire is
    // kept low enough that the tonemap does not bleach its orange to pink.
    // A burst under it or fire poured in flares it up again, from below.
    let glare = max(exp(-n.age / (3.2 * slow(n))), n.fuel * n.fuel * n.fuel * 0.6);
    c += fire_color(temp) * pow(s.heat, 1.8) * (1.2 * heat0 + 12.0 * blaze + 70.0 * glare + 240.0 * white);
    // Lightning in the cloud: a blue-white flare round where it struck.
    if n.flash > 0.0 && part == 0u {
        let bolt_at = vec3<f32>(glow_at.xy, n.at.z + n.bolt_z);
        // Local, and strongest deep in the crevices: it lights the cloud from inside, it
        // does not wash the whole ball white.
        c += vec3<f32>(0.75, 0.82, 1.0) * n.flash * 2.2 * (1.0 - 0.6 * s.open) * exp(-distance(world, bolt_at) / (rc * 0.22));
    }
    return c;
}

fn sample_part(n: Blast, part: u32, world: vec3<f32>) -> Sample {
    if is_nova(n) {
        // How far short of the scene in view this sample is, and the ground there.
        let gap = march_scene_t - distance(world, globals.camera.xyz);
        var s: Sample;
        if part == 0u {
            s = nova_column(n, world);
        } else if part == 1u {
            let ground = mix(n.ground, march_scene_z, smoothstep(400.0, 150.0, gap));
            s = nova_surge(n, world, ground);
        } else {
            s = nova_shell(n, world);
        }
        s.density *= nova_soft(n, gap);
        return s;
    }
    if part == 0u {
        return column(n, world);
    }
    if part == 1u {
        return surge(n, world);
    }
    return wilson(n, world);
}

// The scene in view on this ray (fs_nuke_march): how far to it, and its height (only
// meaningful while that distance is finite).
var<private> march_scene_t: f32;
var<private> march_scene_z: f32;

// The cloud deck on this ray: how far to it, how much light gets through it, and how
// deep it is along the ray.
var<private> cloud_t: f32;
var<private> cloud_through: f32;
var<private> cloud_depth: f32;

fn find_clouds(uv: vec2<f32>, rd: vec3<f32>) {
    cloud_t = 1.0e9;
    cloud_through = 1.0;
    cloud_depth = 1.0;
    if atmos.ground_color.w < 0.5 {
        return;
    }
    let dims = vec2<i32>(textureDimensions(nuke_cloud_march));
    let t = bitcast<f32>(textureLoad(nuke_cloud_march, clamp(vec2<i32>(uv * vec2<f32>(dims)), vec2<i32>(0), dims - 1), 0).z);
    if !(t > 0.0 && t < 1.0e8) {
        return;
    }
    let through = clamp(textureSampleLevel(nuke_cloud_now, clamp_sampler, uv, 0.0).a, 0.0, 1.0);
    // As the clouds' composite thins them at battle zoom (clouds.wgsl `veil`).
    let veil = mix(0.55, 1.0, smoothstep(3000.0, 10000.0, atmos.view.z));
    cloud_through = 1.0 - (1.0 - through) * veil;
    cloud_t = t;
    // The deck is some hundreds of metres thick; along a slanting ray, more.
    cloud_depth = min(180.0 / max(abs(rd.z), 0.12), 1500.0);
}

// How much of what lies `t` along the ray is seen past the clouds: all of it in front
// of the deck, what the deck lets through behind it, eased across its depth.
fn cloud_veil(t: f32) -> f32 {
    return mix(1.0, cloud_through, smoothstep(cloud_t - cloud_depth, cloud_t + cloud_depth, t));
}

// The ignition's glow in the air round the fireball: a white-hot core of light hugging
// the ball and a wide warm halo kilometres across, held for the first several seconds and
// sinking away over about fifteen. It is light only (no cover), added before the bloom takes it.
fn ignition_glow(n: Blast, eye: vec3<f32>, rd: vec3<f32>, scene_t: f32) -> vec3<f32> {
    let k = slow(n);
    let env = smoothstep(0.0, 0.08, n.age) * (0.65 * exp(-n.age / (3.0 * k)) + 0.35 * exp(-n.age / (6.0 * k)))
        + n.fuel * n.fuel * n.fuel * 0.3;
    if env < 0.004 {
        return vec3<f32>(0.0);
    }
    let hc = head_height(n);
    let rc = max(head_radius(n), 60.0 * n.scale);
    let at = n.at + vec3<f32>(lean(n, hc), hc);
    let t = dot(at - eye, rd);
    if t <= 0.0 {
        return vec3<f32>(0.0);
    }
    let d = distance(eye + rd * t, at);
    // Hidden behind a hill or a hull, only the lit air in front still glows.
    let seen = mix(0.3, 1.0, smoothstep(t - 2.0 * rc, t, scene_t));
    let core = exp(-d / (0.55 * rc));
    let halo = 1.0 / (1.0 + (d / (2.6 * rc)) * (d / (2.6 * rc)));
    let color = fire_color(0.6 + 0.4 * smoothstep(0.1, 0.6, env));
    // Both go faster than `env` once it sinks, the core first, so the ball shows its own
    // fire and the land its colour again instead of a cream veil over both.
    return color * (28.0 * env * env * core + 5.0 * pow(env, 1.6) * halo) * seen * cloud_veil(t);
}

fn march_seg(seg: Seg, eye: vec3<f32>, rd: vec3<f32>, jitter: f32, m_in: Marched, share: f32) -> Marched {
    var m = m_in;
    let n = blast_of(seg.blast);
    // The ground's dust is a thin layer along a long ray: too few steps and the dither
    // decides which pixels find it at all, which is grain.
    // A nova's shell is thin and bright: it takes more steps than the Wilson cloud's haze.
    let shell_steps = select(10, 24, is_nova(n));
    let steps = max(i32(f32(select(select(shell_steps, 36, seg.part == 1u), 56, seg.part == 0u)) * share), 8);
    let dt = (seg.t1 - seg.t0) / f32(steps);
    let rc = head_radius(n);
    let hc = head_height(n);
    let r = roll(n);
    let heat0 = heat_left(n);
    for (var k = 0; k < steps; k++) {
        let t = seg.t0 + (f32(k) + jitter) * dt;
        let world = eye + rd * t;
        let s = sample_part(n, seg.part, world);
        if s.density <= 1e-6 {
            continue;
        }
        let c = shade_sample(n, seg.part, world, s, rc, hc, r, heat0);
        let a = 1.0 - exp(-s.density * dt);
        let cv = cloud_veil(t);
        m.light += m.trans * a * c * cv;
        m.cover += m.trans * a * cv;
        if m.trans > 0.5 && m.trans * (1.0 - a) <= 0.5 {
            m.seen = t;
        }
        m.trans *= 1.0 - a;
        if m.trans < 0.01 {
            break;
        }
    }
    return m;
}

struct FullOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_nuke_full(@builtin(vertex_index) index: u32) -> FullOut {
    let p = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var out: FullOut;
    out.clip = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(p.x, 1.0 - p.y);
    return out;
}

// The nearest surface under the half-size texel whose middle is at `uv` (reversed Z:
// larger is nearer): what the march stops at, and what the composite matches against.
fn footprint_depth(uv: vec2<f32>) -> f32 {
    let full = vec2<f32>(textureDimensions(nuke_scene_depth));
    let base = vec2<i32>(uv * full);
    let dims = vec2<i32>(full) - 1;
    let foot = max(i32(full.x / (vec2<f32>(textureDimensions(nuke_half)).x * 2.0)), 1);
    var depth = 0.0;
    for (var i = 0; i < 4; i++) {
        let o = vec2<i32>(i & 1, i >> 1u) * foot - foot / 2;
        depth = max(depth, textureLoad(nuke_scene_depth, clamp(base + o, vec2<i32>(0), dims), 0));
    }
    return depth;
}

@fragment
fn fs_nuke_march(in: FullOut) -> @location(0) vec4<f32> {
    let count = min(u32(globals.nuke_view.z), NUKE_SLOTS);
    let uv = in.uv;
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    // From the eye through a point a hundred near-planes out, as the clouds do. A point at
    // depth 1e-7 (ten million near-planes out) left `w` mostly rounding error with the
    // camera kilometres from the origin: at some angles the ray pointed anywhere and the
    // whole cloud went missing.
    let eye = globals.camera.xyz;
    let far_h = globals.inv_view_proj * vec4<f32>(ndc, 0.01, 1.0);
    let rd = normalize(far_h.xyz / far_h.w - eye);
    // The nearest surface under this texel's footprint (reversed Z: larger is nearer), so
    // the volume never paints over the edge of something standing in front of it.
    let depth = footprint_depth(uv);
    var scene_t = 1.0e9;
    if depth > 0.0 {
        let h = globals.inv_view_proj * vec4<f32>(ndc, depth, 1.0);
        scene_t = dot(h.xyz / h.w - eye, rd);
    }
    march_scene_t = scene_t;
    march_scene_z = eye.z + rd.z * min(scene_t, 1.0e6);

    find_clouds(uv, rd);

    // Every stretch of the ray through a part of a blast.
    var segs: array<Seg, 32>;
    var count_s = 0u;
    for (var i = 0u; i < count; i++) {
        if count_s + 4u > MAX_SEGS {
            break;
        }
        let n = blast_of(i);
        let col = slab(eye, rd, column_box(n));
        if min(col.y, scene_t) > col.x {
            segs[count_s] = Seg(col.x, min(col.y, scene_t), i, 0u);
            count_s++;
        }
        let gr = slab(eye, rd, surge_box(n));
        if min(gr.y, scene_t) > gr.x && n.age < select(55.0, 60.0, is_nova(n)) {
            segs[count_s] = Seg(gr.x, min(gr.y, scene_t), i, 1u);
            count_s++;
        }
        // The shell: its near and far sides as stretches of their own, so the fireball
        // inside is seen between them.
        if shell_left(n) > 0.0 {
            let rw = shell_radius(n);
            let th = shell_thick(n);
            let outer = sphere(eye, rd, n.at, rw + th * 2.0);
            if outer.y > outer.x {
                let inner = sphere(eye, rd, n.at, max(rw - th * 2.0, 1.0));
                if inner.y > inner.x {
                    if min(inner.x, scene_t) > outer.x {
                        segs[count_s] = Seg(outer.x, min(inner.x, scene_t), i, 2u);
                        count_s++;
                    }
                    if min(outer.y, scene_t) > inner.y {
                        segs[count_s] = Seg(inner.y, min(outer.y, scene_t), i, 2u);
                        count_s++;
                    }
                } else if min(outer.y, scene_t) > outer.x {
                    segs[count_s] = Seg(outer.x, min(outer.y, scene_t), i, 2u);
                    count_s++;
                }
            }
        }
    }
    var glow = vec3<f32>(0.0);
    for (var i = 0u; i < count; i++) {
        let n = blast_of(i);
        if is_nova(n) {
            glow += nova_ignition(n, eye, rd, scene_t);
        } else {
            glow += ignition_glow(n, eye, rd, scene_t);
        }
    }
    if count_s == 0u {
        return vec4<f32>(glow, 0.0);
    }
    // Nearest first.
    for (var i = 1u; i < count_s; i++) {
        var j = i;
        while j > 0u && segs[j - 1u].t0 > segs[j].t0 {
            let tmp = segs[j];
            segs[j] = segs[j - 1u];
            segs[j - 1u] = tmp;
            j--;
        }
    }
    // A fixed dither (interleaved gradient noise): it does not crawl from frame to frame,
    // and the composite's tent averages it away.
    let px = in.clip.xy;
    let jitter = fract(52.9829189 * fract(dot(px, vec2<f32>(0.06711056, 0.00583715))));
    // Many blasts along one ray share out the steps, so a salvo does not stall the frame.
    let share = clamp(sqrt(4.0 / f32(count_s)), 0.3, 1.0);
    var m: Marched;
    m.light = vec3<f32>(0.0);
    m.trans = 1.0;
    m.cover = 0.0;
    m.seen = segs[0].t0;
    // Stretches that overlap are marched together, every part sampled at each step and
    // their light mixed by density: marched one after another, the second blast's cap
    // was cut off where the first one's box ended, a hard seam between neighbours.
    var i = 0u;
    loop {
        if i >= count_s || m.trans < 0.01 {
            break;
        }
        var last = i + 1u;
        var end = segs[i].t1;
        while last < count_s && segs[last].t0 < end {
            end = max(end, segs[last].t1);
            last++;
        }
        if last == i + 1u {
            m = march_seg(segs[i], eye, rd, jitter, m, share);
        } else {
            let start = segs[i].t0;
            // Never more than a fixed number: a carpet of blasts overlapping on screen
            // otherwise marched hundreds of steps a texel, each sampling all of them.
            let steps = min(i32(f32(56 + 16 * i32(last - i - 1u)) * share), 80);
            let dt = (end - start) / f32(steps);
            for (var k = 0; k < steps; k++) {
                let t = start + (f32(k) + jitter) * dt;
                let world = eye + rd * t;
                var dens = 0.0;
                var col = vec3<f32>(0.0);
                for (var q = i; q < last; q++) {
                    let g = segs[q];
                    if t < g.t0 || t > g.t1 {
                        continue;
                    }
                    let n = blast_of(g.blast);
                    let sp = sample_part(n, g.part, world);
                    if sp.density <= 1e-6 {
                        continue;
                    }
                    let c = shade_sample(n, g.part, world, sp, head_radius(n), head_height(n), roll(n), heat_left(n));
                    dens += sp.density;
                    col += c * sp.density;
                }
                if dens <= 1e-6 {
                    continue;
                }
                let a = 1.0 - exp(-dens * dt);
                let cv = cloud_veil(t);
                m.light += m.trans * a * col / dens * cv;
                m.cover += m.trans * a * cv;
                if m.trans > 0.5 && m.trans * (1.0 - a) <= 0.5 {
                    m.seen = t;
                }
                m.trans *= 1.0 - a;
                if m.trans < 0.01 {
                    break;
                }
            }
        }
        i = last;
    }
    let alpha = m.cover;
    if alpha < 0.001 {
        return vec4<f32>(max(m.light + glow, vec3<f32>(0.0)), 0.0);
    }
    // The air between the eye and the blast.
    let hazed = apply_haze(m.light / alpha, eye + rd * m.seen, eye) * alpha;
    // max() scrubs a NaN on NVIDIA where select() does not: one bad sample must not
    // paint a black hole through the tent.
    return max(vec4<f32>(hazed + glow, alpha), vec4<f32>(0.0));
}

@fragment
fn fs_nuke_composite(in: FullOut) -> @location(0) vec4<f32> {
    let t = 0.5 / vec2<f32>(textureDimensions(nuke_half));
    var v = textureSampleLevel(nuke_half, clamp_sampler, in.uv, 0.0) * 0.36;
    v += textureSampleLevel(nuke_half, clamp_sampler, in.uv + vec2<f32>(t.x, t.y), 0.0) * 0.16;
    v += textureSampleLevel(nuke_half, clamp_sampler, in.uv + vec2<f32>(-t.x, t.y), 0.0) * 0.16;
    v += textureSampleLevel(nuke_half, clamp_sampler, in.uv + vec2<f32>(t.x, -t.y), 0.0) * 0.16;
    v += textureSampleLevel(nuke_half, clamp_sampler, in.uv + vec2<f32>(-t.x, -t.y), 0.0) * 0.16;
    v = max(v, vec4<f32>(0.0));
    if v.a < 0.0005 && dot(v.rgb, v.rgb) < 1e-6 {
        discard;
    }
    // Where something stands in front, the tent mixes texels marched against the near
    // surface with ones marched past it: stipple along every silhouette. There, take the
    // 3x3 texels round this pixel weighted by how near their depth is to its own.
    let half_dims = vec2<f32>(textureDimensions(nuke_half));
    let full = vec2<f32>(textureDimensions(nuke_scene_depth));
    let own = max(textureLoad(nuke_scene_depth, clamp(vec2<i32>(in.uv * full), vec2<i32>(0), vec2<i32>(full) - 1), 0), 1e-7);
    let at = in.uv * half_dims - 0.5;
    let cell = floor(at);
    var sum = vec4<f32>(0.0);
    var weight = 0.0;
    var spread = 0.0;
    for (var j = -1; j <= 2; j++) {
        for (var i = -1; i <= 2; i++) {
            let texel = cell + vec2<f32>(f32(i), f32(j));
            let d = at - texel;
            let near = exp(-dot(d, d) * 1.1);
            if near < 0.02 {
                continue;
            }
            let tuv = (texel + 0.5) / half_dims;
            let td = max(footprint_depth(tuv), 1e-7);
            // Reversed Z with an infinite far plane: depth goes as 1 / distance, so the
            // log of the ratio is how far apart the two surfaces are, relatively.
            let apart = abs(log(td / own));
            spread = max(spread, apart);
            let w = near * exp(-apart * apart * 60.0) + 1e-4 * near;
            sum += max(textureLoad(nuke_half, clamp(vec2<i32>(texel), vec2<i32>(0), vec2<i32>(half_dims) - 1), 0), vec4<f32>(0.0)) * w;
            weight += w;
        }
    }
    if spread > 0.05 && weight > 0.0 {
        v = sum / weight;
    }
    return vec4<f32>(v.rgb, min(v.a, 1.0));
}

// ---- strategic missiles --------------------------------------------------------------

struct MissileOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) @interpolate(flat) kind: u32,
    @location(2) along: f32,
    @location(3) world: vec3<f32>,
    @location(4) @interpolate(flat) slot: u32,
    // A Regency body's (nova.wgsl `NovaMissileVertex`): round it, and which piece.
    @location(5) around: f32,
    @location(6) @interpolate(flat) piece: u32,
}

// Lathe profile, tail (0) to nose (1): position along, radius as a share of the body's.
fn warhead_ring(k: u32) -> vec2<f32> {
    let p = array<vec2<f32>, 12>(
        vec2<f32>(0.0, 0.0), vec2<f32>(0.0, 0.88), vec2<f32>(0.03, 1.0), vec2<f32>(0.44, 1.0),
        vec2<f32>(0.455, 0.96), vec2<f32>(0.47, 1.0), vec2<f32>(0.7, 1.0), vec2<f32>(0.76, 0.93),
        vec2<f32>(0.86, 0.72), vec2<f32>(0.94, 0.42), vec2<f32>(0.985, 0.14), vec2<f32>(1.0, 0.0));
    return p[min(k, 11u)];
}

fn interceptor_ring(k: u32) -> vec2<f32> {
    let p = array<vec2<f32>, 12>(
        vec2<f32>(0.0, 0.0), vec2<f32>(0.0, 0.9), vec2<f32>(0.04, 1.0), vec2<f32>(0.5, 1.0),
        vec2<f32>(0.52, 0.86), vec2<f32>(0.54, 0.86), vec2<f32>(0.56, 0.8), vec2<f32>(0.78, 0.8),
        vec2<f32>(0.88, 0.62), vec2<f32>(0.95, 0.36), vec2<f32>(0.99, 0.1), vec2<f32>(1.0, 0.0));
    return p[min(k, 11u)];
}

// A missile's drawn size against the Sunfall's round (nuke_fx.rs packs it).
fn missile_scale(word: f32) -> f32 {
    return f32((u32(word) >> MISSILE_SCALE_SHIFT) & MISSILE_SCALE_MASK) / MISSILE_SCALE_STEPS;
}

const SIDES: u32 = 14u;
// 11 bands of SIDES quads, then 4 fins (two faces each): MISSILE_VERTS.
const BODY_VERTS: u32 = 11u * 14u * 6u;

@vertex
fn vs_strategic(@builtin(vertex_index) vertex: u32, @builtin(instance_index) instance: u32) -> MissileOut {
    var out: MissileOut;
    out.slot = instance;
    let count = u32(globals.nuke_view.w);
    if instance >= count {
        out.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
        return out;
    }
    let a = globals.strategic[instance * 2u];
    let b = globals.strategic[instance * 2u + 1u];
    let kind = u32(a.w) & MISSILE_KIND_MASK;
    out.kind = kind;
    let axis = normalize(select(b.xyz, vec3<f32>(0.0, 0.0, 1.0), dot(b.xyz, b.xyz) < 1e-6));
    let warhead = kind == 0u;
    // A warhead is the round in the Sunfall's silo (strategic.rs) at scale 1.
    let size = missile_scale(a.w);
    let length = select(9.0, MISSILE_WARHEAD_LENGTH * size, warhead);
    let radius = select(0.55, MISSILE_WARHEAD_RADIUS * size, warhead);
    var local = vec3<f32>(0.0);
    var normal = vec3<f32>(1.0, 0.0, 0.0);
    let quad = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);
    out.around = 0.0;
    out.piece = 0u;
    if (u32(a.w) & MISSILE_PLASMA) != 0u {
        let v = nova_missile_vertex(vertex, length, radius);
        if v.gone {
            out.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
            return out;
        }
        local = v.local;
        normal = v.normal;
        out.along = v.along;
        out.around = v.around;
        out.piece = v.piece;
    } else if vertex < BODY_VERTS {
        let band = vertex / (SIDES * 6u);
        let side = (vertex / 6u) % SIDES;
        let corner = quad[vertex % 6u];
        let ring = band + select(0u, 1u, corner >= 2u);
        let around = side + select(0u, 1u, corner == 1u || corner == 2u);
        let pr = select(interceptor_ring(ring), warhead_ring(ring), warhead);
        let ang = f32(around) / f32(SIDES) * TAU;
        local = vec3<f32>(pr.x * length, cos(ang) * pr.y * radius, sin(ang) * pr.y * radius);
        let p0 = select(interceptor_ring(band), warhead_ring(band), warhead);
        let p1 = select(interceptor_ring(band + 1u), warhead_ring(band + 1u), warhead);
        let slope = (p0.y - p1.y) * radius / max((p1.x - p0.x) * length, 0.01);
        normal = normalize(vec3<f32>(slope, cos(ang), sin(ang)));
        out.along = pr.x;
    } else {
        // Four swept fins at the tail.
        let v = vertex - BODY_VERTS;
        let fin = v / 12u;
        let corner = quad[(v % 12u) % 6u];
        let back = (v % 12u) >= 6u;
        let ang = f32(fin) * TAU * 0.25 + TAU * 0.125;
        let span = select(1.4, 1.9, warhead) * radius;
        let profile = array<vec2<f32>, 4>(
            vec2<f32>(0.0, 1.0), vec2<f32>(0.0, 1.0 + span / radius),
            vec2<f32>(0.08, 1.0 + span / radius), vec2<f32>(0.2, 1.0));
        let q = profile[corner];
        local = vec3<f32>(q.x * length, cos(ang) * q.y * radius, sin(ang) * q.y * radius);
        normal = vec3<f32>(0.0, -sin(ang), cos(ang)) * select(1.0, -1.0, back);
        out.along = 0.02;
    }
    let reference = select(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 1.0, 0.0), abs(axis.z) > 0.95);
    let side_v = normalize(cross(axis, reference));
    let up_v = cross(side_v, axis);
    // The body's nose is at `a.xyz`; it runs back down the axis.
    let tail = a.xyz - axis * length;
    let world = tail + axis * local.x + side_v * local.y + up_v * local.z;
    out.world = world;
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    out.normal = axis * normal.x + side_v * normal.y + up_v * normal.z;
    return out;
}

@fragment
fn fs_strategic(in: MissileOut) -> @location(0) vec4<f32> {
    var n = normalize(in.normal);
    let b = globals.strategic[in.slot * 2u + 1u];
    let a = globals.strategic[in.slot * 2u];
    let heat = b.w;
    let owner = (u32(a.w) >> MISSILE_OWNER_SHIFT) & OWNER_MASK;
    let warhead = in.kind == 0u;
    let plasma = (u32(a.w) & MISSILE_PLASMA) != 0u;
    // Light metal, dark bands at the stage joint and the re-entry vehicle, a team ring.
    var base = vec3<f32>(0.62, 0.64, 0.66);
    // A Regency missile's own light (nova.wgsl `nova_missile_color`).
    var shine = vec3<f32>(0.0);
    let x = in.along;
    if plasma {
        // Its wings are thin plates, seen from both sides.
        n = select(n, -n, dot(n, globals.camera.xyz - in.world) < 0.0);
        let look = nova_missile_color(in.piece, x, in.around, heat);
        base = look[0];
        shine = look[1];
    } else if warhead {
        if (x > 0.435 && x < 0.48) || x > 0.74 { base = vec3<f32>(0.07, 0.075, 0.08); }
        if x > 0.58 && x < 0.62 { base = globals.team_colors[owner].rgb * 0.8; }
        if x < 0.03 { base = vec3<f32>(0.12, 0.11, 0.1); }
    } else {
        base = vec3<f32>(0.1, 0.105, 0.11);
        if x > 0.3 && x < 0.36 { base = vec3<f32>(0.9, 0.42, 0.08); }
        if x > 0.6 && x < 0.64 { base = globals.team_colors[owner].rgb * 0.8; }
    }
    let sun = globals.sun.xyz;
    let lambert = max(dot(n, sun), 0.0);
    let v = normalize(globals.camera.xyz - in.world);
    let h = normalize(sun + v);
    let spec = pow(max(dot(n, h), 0.0), 48.0) * 0.35;
    var c = base * (atmos.sun_color.rgb * lambert * 1.1 + atmos.sky_color.rgb * 0.6) + atmos.sun_color.rgb * spec;
    c += shine;
    // Coming down, the nose burns: orange going white at the tip (a Regency round's line
    // work burns brighter instead, `nova_missile_color`).
    if heat > 0.0 && !plasma {
        let tip = smoothstep(0.7, 1.0, x);
        c += fire_color(tip * heat) * tip * heat * 30.0;
    }
    return vec4<f32>(c, 1.0);
}

// The motor's plume: a volume of light behind the nozzle, white-hot and nozzle-wide at
// the tail of the body, flaring and going yellow, orange, red as it fades behind. Drawn
// on a lathed hull round the axis (convex, so each ray enters it once); the fragment on
// the face the eye enters through marches its ray through the glow inside, so it is as
// thick as it looks from any side, and looking up its tail it is a bright disc. Additive.
struct PlumeOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) @interpolate(flat) slot: u32,
}

const PLUME_SIDES: u32 = 16u;
const PLUME_RINGS: u32 = 10u;
// Hull bands, then a cap across the nozzle. Mirrored in renderer.rs (the plume draw).
const PLUME_VERTS: u32 = 10u * 16u * 6u + 16u * 3u;
// How far the hull reaches past the glow's own radius.
const PLUME_HULL: f32 = 2.4;

// The glow's radius `x` of the way down the plume, from its nozzle's.
fn plume_radius(x: f32, nozzle: f32) -> f32 {
    return nozzle * (0.75 + 3.2 * pow(max(x, 1e-4), 0.6));
}

struct PlumeFrame {
    nozzle: vec3<f32>,
    // Away from the body, down the plume.
    back: vec3<f32>,
    side: vec3<f32>,
    up: vec3<f32>,
    // Plume length and the nozzle's radius, metres; brightness.
    length: f32,
    mouth: f32,
    power: f32,
    // A Regency drive: red plasma, rose-white at the nozzle.
    plasma: bool,
}

// Where a missile's nozzle is: the tail of the body vs_strategic lathes.
fn plume_frame(slot: u32) -> PlumeFrame {
    let a = globals.strategic[slot * 2u];
    let b = globals.strategic[slot * 2u + 1u];
    let warhead = (u32(a.w) & MISSILE_KIND_MASK) == 0u;
    let axis = normalize(select(b.xyz, vec3<f32>(0.0, 0.0, 1.0), dot(b.xyz, b.xyz) < 1e-6));
    let size = missile_scale(a.w);
    let body = select(9.0, MISSILE_WARHEAD_LENGTH * size, warhead);
    let radius = select(0.55, MISSILE_WARHEAD_RADIUS * size, warhead);
    let reference = select(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 1.0, 0.0), abs(axis.z) > 0.95);
    var f: PlumeFrame;
    f.nozzle = a.xyz - axis * body;
    f.back = -axis;
    f.side = normalize(cross(axis, reference));
    f.up = cross(f.side, axis);
    f.length = f32((u32(a.w) >> MISSILE_PLUME_SHIFT) & MISSILE_PLUME_MASK);
    // The tail ring's radius (warhead_ring / interceptor_ring).
    f.mouth = radius * select(0.9, 0.88, warhead);
    f.power = select(0.7, 1.0, warhead);
    f.plasma = (u32(a.w) & MISSILE_PLASMA) != 0u;
    return f;
}

// Metres behind the nozzle of hull ring `k`; the first sits just inside the body's tail.
fn plume_ring_at(k: u32, len: f32) -> f32 {
    return select(f32(k) / f32(PLUME_RINGS) * len, -0.5, k == 0u);
}

fn plume_hull(k: u32, f: PlumeFrame) -> f32 {
    let x = max(plume_ring_at(k, f.length), 0.0) / f.length;
    return select(PLUME_HULL * plume_radius(x, f.mouth), 0.0, k >= PLUME_RINGS);
}

@vertex
fn vs_plume(@builtin(vertex_index) vertex: u32, @builtin(instance_index) instance: u32) -> PlumeOut {
    var out: PlumeOut;
    out.slot = instance;
    let count = u32(globals.nuke_view.w);
    if instance >= count {
        out.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
        return out;
    }
    let f = plume_frame(instance);
    if f.length < 1.0 {
        out.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
        return out;
    }
    let quad = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);
    let side_verts = PLUME_RINGS * PLUME_SIDES * 6u;
    var world: vec3<f32>;
    var normal: vec3<f32>;
    if vertex < side_verts {
        let band = vertex / (PLUME_SIDES * 6u);
        let side = (vertex / 6u) % PLUME_SIDES;
        let corner = quad[vertex % 6u];
        let ring = band + select(0u, 1u, corner >= 2u);
        let around = side + select(0u, 1u, corner == 1u || corner == 2u);
        let ang = f32(around) / f32(PLUME_SIDES) * TAU;
        let radial = f.side * cos(ang) + f.up * sin(ang);
        world = f.nozzle + f.back * plume_ring_at(ring, f.length) + radial * plume_hull(ring, f);
        let slope = (plume_hull(band + 1u, f) - plume_hull(band, f))
            / max(plume_ring_at(band + 1u, f.length) - plume_ring_at(band, f.length), 0.01);
        normal = normalize(radial - f.back * slope);
    } else {
        // The cap across the front, inside the body's tail.
        let v = vertex - side_verts;
        let c = v % 3u;
        let ang = f32(v / 3u + select(0u, 1u, c == 2u)) / f32(PLUME_SIDES) * TAU;
        let rim = f.side * cos(ang) + f.up * sin(ang);
        world = f.nozzle - f.back * 0.5 + select(rim * plume_hull(0u, f), vec3<f32>(0.0), c == 0u);
        normal = -f.back;
    }
    out.world = world;
    out.normal = normal;
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    return out;
}

@fragment
fn fs_plume(in: PlumeOut) -> @location(0) vec4<f32> {
    let f = plume_frame(in.slot);
    let eye = globals.camera.xyz;
    let time = globals.camera.w;
    let to_frag = in.world - eye;
    let dist = length(to_frag);
    let dir = to_frag / max(dist, 1e-4);
    // The eye's place in the plume's frame.
    let rel = eye - f.nozzle;
    let x0 = dot(rel, f.back);
    let w = rel - f.back * x0;
    let hmax = PLUME_HULL * plume_radius(1.0, f.mouth);
    let inside = x0 > -0.5 && x0 < f.length
        && length(w) < PLUME_HULL * plume_radius(max(x0, 0.0) / f.length, f.mouth);
    // One fragment a pixel: where the ray enters the hull, or the eye itself inside it.
    let facing = dot(in.normal, -dir) > 0.0;
    if facing == inside {
        discard;
    }
    var t0 = dist;
    var t1 = dist + 2.0 * f.length + 4.0 * hmax;
    if inside {
        t0 = 0.3;
        t1 = dist;
    } else {
        // Out through the bounding cylinder, or the ends.
        let db = dot(dir, f.back);
        let u = dir - f.back * db;
        let qa = dot(u, u);
        if qa > 1e-8 {
            let qb = dot(w, u);
            let disc = qb * qb - qa * (dot(w, w) - hmax * hmax);
            if disc > 0.0 {
                t1 = min(t1, (-qb + sqrt(disc)) / qa);
            }
        }
        if abs(db) > 1e-6 {
            t1 = min(t1, max((-0.5 - x0) / db, (f.length - x0) / db));
        }
    }
    let span = t1 - t0;
    if span <= 0.0 {
        discard;
    }
    let steps = 28;
    let dt = span / f32(steps);
    let jitter = hash21(in.clip.xy);
    var glow = vec3<f32>(0.0);
    for (var i = 0; i < steps; i++) {
        let p = eye + dir * (t0 + (f32(i) + jitter) * dt) - f.nozzle;
        let s = dot(p, f.back);
        let x = s / f.length;
        if x <= 0.0 || x >= 1.0 {
            continue;
        }
        let off = p - f.back * s;
        let rho2 = dot(off, off);
        // The core: nozzle-wide and white-hot, gone a few diameters back.
        let rc = f.mouth * (0.7 + s / (f.mouth * 10.0));
        let core = exp(-s / (f.mouth * 2.5)) * exp(-rho2 / (rc * rc)) / rc;
        // The flame round and behind it, spreading, tearing and cooling as it goes.
        let r = plume_radius(x, f.mouth);
        let tear = textureSampleLevel(noise_map, repeat_sampler,
            vec2<f32>(s / (f.mouth * 9.0) - time * 4.0, sqrt(rho2) / (r * 3.0) + f32(in.slot) * 0.37), 0.0).b;
        let flame = (1.0 - x) * (1.0 - x) * (0.55 + 0.45 * exp(-x * 6.0))
            * exp(-rho2 / (r * r)) / r * (0.6 + 0.8 * tear);
        let hot = select(vec3<f32>(1.0, 0.95, 0.88), vec3<f32>(1.0, 0.86, 0.9), f.plasma);
        let flame_col = select(fire_color(0.78 - x * 1.3), plasma_color(0.7 - x * 1.1), f.plasma);
        glow += (hot * core * 16.0 + flame_col * flame * 0.8) * dt;
    }
    let flicker = 0.88 + 0.12 * sin(time * 61.0 + f32(in.slot) * 1.7);
    glow *= f.power * flicker;
    // Looking up the tail it is blinding, not unbounded.
    glow *= 120.0 / (120.0 + max(glow.r, max(glow.g, glow.b)));
    return vec4<f32>(glow, 0.0);
}
