// The Regency's pinch-fusion warhead going off (`nuke_look` PLASMA; docs/NUKES.md "The
// Regency's warhead"). Prepended to shaders that contain the line `//!use nova`
// (nuke.wgsl, which owns `Blast`, `Sample`, the billows and the shapes over time used
// here; WGSL does not mind the order).
//
// Not a mushroom: the ball stays where the warhead went off, held there in its own field,
// never climbing or leaning off downwind, a dying star with no stem and no cap, ringed
// like a supernova's remnant (`nova_radius`; `nuke_fx::Blast` mirrors it). Made of plasma instead of
// fire and smoke:
//
//   the star      a white-hot ball with a rose limb, its surface in granulation cells,
//                 too bright to look at, for the first second or two
//   the shell     the nova itself: a hollow shell thrown out past the damage radius,
//                 white with prism at its leading edge, tearing into red threads that
//                 hang for half a minute (part 2, in place of the Wilson cloud)
//   the star      the ball cools to a dark, blackened crimson laced with a web of hard
//                 glowing threads and sits on the burst, turning; a slow pulse
//                 runs through it. It comes apart into lobes and clears between about
//                 20 and 40 s, and is gone by NOVA_LIFE
//   the rings     thin bright rings, as round SN 1987A, level about the burst: a wide one
//                 round its equator and a narrower one over its pole (the other pole's
//                 would be under the ground), gone within half a minute
//   the ground    the shock's front is a sheet of red plasma skimming the ground (the
//                 ground in view, so it follows hills), and the surge a low, dark ring of
//                 glassy dust
//
// Every part fades out against the scene in front of it rather than being cut off where
// it meets the ground (`nova_soft`).
//
// Hard-edged throughout (docs/STYLE.md "No mist"): surfaces are firm, light lies in
// threads and cells, never in a soft glow swelling out of it.

// Seconds a Regency blast is drawn for. Mirrored by nuke_fx::NOVA_LIFE.
const NOVA_LIFE: f32 = 50.0;

fn is_nova(n: Blast) -> bool {
    return n.look == NUKE_LOOK_PLASMA;
}

// Plasma's colour at `temp`: white, rose, red, crimson, a dark violet-red ember.
fn plasma_color(temp: f32) -> vec3<f32> {
    let h = clamp(temp, 0.0, 1.0);
    let ember = vec3<f32>(0.3, 0.012, 0.075);
    let crimson = vec3<f32>(0.85, 0.035, 0.05);
    let red = vec3<f32>(1.0, 0.19, 0.09);
    let rose = vec3<f32>(1.0, 0.6, 0.64);
    let white = vec3<f32>(1.0, 0.95, 0.97);
    var c = mix(ember, crimson, smoothstep(0.0, 0.3, h));
    c = mix(c, red, smoothstep(0.3, 0.55, h));
    c = mix(c, rose, smoothstep(0.55, 0.8, h));
    c = mix(c, white, smoothstep(0.8, 1.0, h));
    return c;
}

// Solid for its first seconds; then it thins, clearing between about 20 and 40 s (the
// body is so deep that it stays opaque until only a tenth is left), and is gone by
// NOVA_LIFE. Fire folded in by a salvo keeps it thick.
fn nova_fade(n: Blast) -> f32 {
    let k = slow(n);
    let left = 1.0 - smoothstep(4.0 * k, 45.0 * k, n.age);
    return max(left * left * (1.0 - smoothstep(NOVA_LIFE - 20.0, NOVA_LIFE, n.age)), n.thick);
}

// How far the star has come apart, 0..1: it breaks into lobes and holes as it clears.
fn nova_breakup(n: Blast) -> f32 {
    let k = slow(n);
    return smoothstep(12.0 * k, 40.0 * k, n.age) * (1.0 - n.thick);
}

// How much its threads still glow, 0..1: they cool over half a minute.
fn nova_glow(n: Blast) -> f32 {
    let k = slow(n);
    let own = 0.4 * exp(-n.age / (8.0 * k)) + 0.6 * exp(-n.age / (28.0 * k));
    return max(own, n.fuel) * (1.0 - smoothstep(NOVA_LIFE - 30.0, NOVA_LIFE, n.age));
}

// The white-hot star: everything at first, gone in a couple of seconds.
fn nova_star(n: Blast) -> f32 {
    return exp(-n.age / (1.1 * slow(n)));
}

// Hard threads: where the warped billows cross their middle, thin bright lines, a web over
// the surface, never a haze. `sharp` narrows them. 0..~1.
fn nova_threads(p: vec3<f32>, seed: f32, phase: f32, sharp: f32) -> f32 {
    let w = billow_at(p * 0.31 + vec3<f32>(seed * 0.7, seed * 0.3, seed * 0.5 + phase));
    let q = p + (w.gba - 0.5) * 0.9;
    let b = billow_at(q * 0.8);
    let r1 = max(1.0 - abs(b.g * 2.0 - 1.0), 0.0);
    let r2 = max(1.0 - abs(b.b * 2.0 - 1.0), 0.0);
    return pow(r1, sharp) * 0.7 + pow(r2, sharp * 1.5) * 0.5;
}

// The star's radius: out in a second, swelling for 25 s, then drawn back in on
// itself as it dies. Mirrored by nuke_fx::Blast::head_radius.
fn nova_radius(n: Blast) -> f32 {
    let t = n.age;
    let grow = 200.0 * sqrt(1.0 - exp(-t * 3.0)) + 120.0 * (1.0 - exp(-t / 14.0));
    return n.scale * grow * (1.0 - 0.45 * smoothstep(25.0, NOVA_LIFE, t));
}

// How many rings stand round the star (`nova_ring`).
const NOVA_RINGS: u32 = 2u;

// Ring `k` (0 the equator's, 1 the one over its pole): height off its middle, radius,
// thickness, brightness.
fn nova_ring(n: Blast, k: u32) -> vec4<f32> {
    let t = max(n.age - 1.2, 0.0);
    let rc = nova_radius(n);
    let out = 1.0 - exp(-t / 10.0);
    let thick = n.scale * (10.0 + 10.0 * (1.0 - exp(-t / 20.0)));
    if k == 0u {
        return vec4<f32>(0.0, rc * (1.15 + 0.85 * out) + 2.0 * n.scale * t, thick * 1.3, 1.0);
    }
    return vec4<f32>(rc * (0.75 + 0.4 * out), rc * (0.75 + 0.45 * out) + 1.2 * n.scale * t, thick, 0.6);
}

fn nova_ring_left(n: Blast) -> f32 {
    return smoothstep(1.2, 3.0, n.age) * (1.0 - smoothstep(12.0, 30.0, n.age));
}

// How far past the star's middle its rings can reach, out and up.
fn nova_ring_reach(n: Blast) -> f32 {
    let r = nova_ring(n, 0u);
    return r.y + r.z * 3.0;
}

// Fades a part out over the last stretch before the scene in front of it, so nothing is cut
// off with a hard line where it meets the ground. `gap` metres from the scene along the ray.
fn nova_soft(n: Blast, gap: f32) -> f32 {
    return smoothstep(0.0, 25.0 + 35.0 * n.scale, gap);
}

fn nova_column(n: Blast, world: vec3<f32>) -> Sample {
    var s = empty_sample();
    let up = world.z - n.at.z;
    let p = world - vec3<f32>(n.at.xy + lean(n, up), n.at.z);
    let rc = head_radius(n);
    let hc = head_height(n);
    let r = roll(n);
    let t = n.age;
    let seed = n.seed * 13.0;
    let k = slow(n);
    let star = nova_star(n);
    // Held in its field, the plasma churns slower than fire does.
    let phase = 0.4 * (1.0 - exp(-t / (1.5 * k))) + t * 0.012 / k + n.churn;

    // The head.
    let body = head_body(n, p, rc, hc, r);
    if body < 1.45 {
        let q = p - vec3<f32>(0.0, 0.0, hc);
        let rho = length(q.xy);
        let dir = select(q.xy / max(rho, 1e-3), vec2<f32>(1.0, 0.0), rho < 1e-3);
        // It turns over slowly with the ring, as the fire does.
        let a = t * 0.07 * max(r, 0.5 * smoothstep(0.6, 4.0, t));
        let pr = vec2<f32>(rho - rc * 0.58 * r, q.z);
        let turned = vec2<f32>(pr.x * cos(a) - pr.y * sin(a), pr.x * sin(a) + pr.y * cos(a));
        let np = vec3<f32>(dir * (rc * 0.58 * r + turned.x), turned.y) / (rc * 1.45)
            + vec3<f32>(0.0, 0.0, -t * 0.008);
        let b = boiling(np, seed, phase);
        let fine = billow_at(np * 2.9 + vec3<f32>(seed * 0.3, 0.0, phase * 2.6)).g;
        // A smooth star at first; it breaks up into lobes as it cools, less than smoke.
        let lumps = mix(0.12, 0.5, smoothstep(0.3, 3.0 * k, t));
        let edge = body - (b - 0.5) * 0.9 * lumps - (fine - 0.5) * 0.04 * lumps;
        // Always a firm surface, eaten into from its billows as it comes apart.
        let d = smoothstep(1.0, 0.93, edge + 0.45 * nova_breakup(n) * (1.2 - b));
        if d > 0.0 {
            s.density = d * 0.15 / max(n.scale, 0.3);
            let b_sun = boiling(np + globals.sun.xyz * 0.07, seed, phase);
            s.open = clamp(0.5 + (b - b_sun) * 8.0, 0.0, 1.0) * smoothstep(0.2, 0.6, b) * 0.75 + 0.25 * smoothstep(0.3, 0.75, b);
            // The star: granulation, bright cells with darker lanes between.
            let cells = smoothstep(0.35, 0.7, b) * 0.6 + fine * 0.4;
            let star_heat = 0.72 + 0.28 * cells;
            // Then a web of threads over a dark body, hot deep inside and under the cap,
            // with a slow pulse running up through it.
            let threads = nova_threads(np * 0.6, seed, phase * 1.5, mix(4.0, 16.0, smoothstep(1.0, 12.0 * k, t)));
            let deep = smoothstep(0.95, 0.3, body - (b - 0.5) * 0.4);
            let under = smoothstep(0.1, -0.8, q.z / rc) * r;
            let pulse = 0.7 + 0.3 * sin(t * 0.8 - q.z / rc * 3.0 + n.seed * TAU);
            let lace = smoothstep(0.35, 0.9, threads) * pulse;
            let late = max(lace, max(deep * deep * mix(0.9, 0.12, smoothstep(2.0, 20.0 * k, t)), under * 0.3));
            s.heat = clamp(mix(late, star_heat, star), 0.0, 1.0);
        }
    }

    // The rings.
    let left = nova_ring_left(n);
    if left > 0.0 {
        let q = p - vec3<f32>(0.0, 0.0, hc);
        let rr = length(q.xy);
        let ang = atan2(q.y, q.x);
        for (var k = 0u; k < NOVA_RINGS; k++) {
            let ring = nova_ring(n, k);
            let z = q.z - ring.x;
            if abs(z) > ring.z * 2.5 {
                continue;
            }
            let tube = length(vec2<f32>((rr - ring.y) * 0.55, z)) / ring.z;
            if tube >= 1.3 {
                continue;
            }
            let rs = seed + 9.0 + f32(k) * 4.0;
            let thr = nova_threads(vec3<f32>(cos(ang) * 3.0, sin(ang) * 3.0, z / (ring.z * 3.0) + t * 0.01), rs, t * 0.01, 4.0);
            // Broken into lengths of thread, never a whole ruled circle; the slow turn
            // carries the breaks round.
            let turn = ang + t * 0.01 * (1.0 + f32(k));
            let breaks = smoothstep(0.3, 0.55, billow_at(vec3<f32>(cos(turn) * 1.3, sin(turn) * 1.3, n.seed * 3.0 + f32(k) * 0.37)).r);
            let rd = smoothstep(1.0, 0.7, tube) * smoothstep(0.15, 0.5, thr) * breaks * left * ring.w * 0.012 / max(n.scale, 0.3);
            if rd > s.density {
                s.heat = (0.6 + 0.4 * thr) * ring.w;
                s.dust = 0.0;
                s.open = thr;
                // Marks the ring for the shading (bright, never sooty).
                s.wet = 1.0;
            }
            s.density = max(s.density, rd);
        }
    }
    s.density *= nova_fade(n);
    return s;
}

// The shock's front, a sheet of plasma skimming the ground, and the surge: a low dark ring
// of glassy dust that boils out and settles.
fn nova_surge(n: Blast, world: vec3<f32>, ground: f32) -> Sample {
    var s = empty_sample();
    s.dust = 1.0;
    let xy = world.xy - n.at.xy;
    let d = length(xy);
    let up = world.z - ground;
    let t = n.age;
    // Where the ground climbs or falls far from the burst's, the front has gone over or
    // past it (and surge_box ends).
    let rel = (ground - n.ground) / max(n.scale, 0.3);
    let held = smoothstep(-140.0, -90.0, rel) * (1.0 - smoothstep(220.0, 330.0, rel));
    if up < -8.0 || held <= 0.0 {
        return s;
    }
    let dir = xy / max(d, 1.0);
    let b = billows(vec3<f32>(xy / (220.0 * n.scale), up / (140.0 * n.scale) - t * 0.02), n.seed * 5.0);
    let thr = nova_threads(vec3<f32>(dir * 3.0, d / (160.0 * n.scale)), n.seed * 9.0, t * 0.05, 4.0);
    // The front: a thin hard sheet, brightest at its leading edge, gone in a few seconds.
    let behind = n.front - d;
    let width = 10.0 + n.front * 0.025;
    let sheet_h = n.scale * (10.0 + 16.0 * thr) * (1.0 - smoothstep(0.0, 3200.0 * n.scale, n.front));
    let sheet = smoothstep(width * 1.5, 0.0, abs(behind)) * step(-width, behind)
        * smoothstep(sheet_h, sheet_h * 0.4, up) * (1.0 - smoothstep(1.5, 5.0, t)) * (0.4 + 0.6 * thr);
    // The surge.
    let rr = n.scale * (130.0 + 760.0 * (1.0 - exp(-t / 14.0)));
    let hh = n.scale * (25.0 + 55.0 * (1.0 - exp(-t / 9.0))) * (0.55 + 0.9 * b);
    let dr = (d - rr) / (rr * 0.2);
    let ring = exp(-dr * dr) + smoothstep(rr, rr * 0.2, d) * 0.1;
    let surge_d = ring * smoothstep(hh, hh * 0.4, up) * smoothstep(0.3, 0.6, b * 0.7 + thr * 0.4)
        * smoothstep(0.6, 3.0, t) * (1.0 - smoothstep(25.0, 60.0, t));
    s.density = (sheet * 1.6 + surge_d) * held * 0.0032 / max(n.scale, 0.3);
    if sheet > surge_d {
        s.heat = 1.0;
        s.dust = 0.0;
    } else {
        // Embers in the glassy dust while it is young.
        s.heat = thr * 0.5 * exp(-t / 10.0);
    }
    s.open = b;
    return s;
}

// The nova's shell: where it is, and how thick.
fn nova_shell_radius(n: Blast) -> f32 {
    return n.scale * (640.0 * sqrt(1.0 - exp(-n.age / 0.9)) + 5.0 * n.age);
}

fn nova_shell_thick(n: Blast) -> f32 {
    return nova_shell_radius(n) * 0.055 + 8.0 * n.scale;
}

fn nova_shell_left(n: Blast) -> f32 {
    return smoothstep(0.0, 0.12, n.age) * (1.0 - smoothstep(14.0, 36.0, n.age));
}

fn nova_shell(n: Blast, world: vec3<f32>) -> Sample {
    var s = empty_sample();
    let v = world - n.at;
    let d = length(v);
    let rs = nova_shell_radius(n);
    let th = nova_shell_thick(n);
    // A dome: it thins away toward the ground, where the sheet takes the front on, and is
    // never drawn under it.
    let over = smoothstep(0.0, 60.0 * n.scale + th * 2.0, world.z - n.ground);
    let shell = smoothstep(1.0, 0.45, abs(d - rs) / th) * over;
    if shell <= 0.0 {
        return s;
    }
    let dir = v / max(d, 1.0);
    // Whole at first; it tears open into threads and holes as it thins.
    let tear = smoothstep(0.4, 7.0, n.age);
    let thr = nova_threads(dir * 2.4 + vec3<f32>(0.0, 0.0, n.age * 0.015), n.seed * 11.0, n.age * 0.02, mix(1.5, 8.0, tear));
    let holes = smoothstep(mix(0.0, 0.48, tear), mix(0.1, 0.62, tear), billows(dir * 1.3 + vec3<f32>(n.seed), n.seed * 7.0));
    let lit = mix(1.0, thr, tear) * holes;
    s.density = shell * lit * nova_shell_left(n) * 0.0022 / max(n.scale, 0.3);
    s.heat = clamp(lit, 0.0, 1.0);
    s.open = thr;
    return s;
}

// The light of what `nova_*` sampled: a dark body lit dimly by the sun, and the plasma's
// own light in its threads, cells and sheets.
fn nova_shade(n: Blast, part: u32, world: vec3<f32>, s: Sample, rc: f32, hc: f32, r: f32) -> vec3<f32> {
    let sun = globals.sun.xyz;
    let sun_col = atmos.sun_color.rgb * max(atmos.sun_color.w, 0.25);
    let sky = atmos.sky_color.rgb;
    let k = slow(n);
    let star = nova_star(n);
    let glow = nova_glow(n);
    let glare = max(exp(-n.age / (3.0 * k)), n.fuel * n.fuel * n.fuel * 0.6);
    if part == 2u {
        // The shell: white with prism running over its leading edge, then red threads
        // cooling to crimson.
        let temp = clamp(0.98 * exp(-n.age / 1.6) + 0.5 * s.heat * exp(-n.age / 14.0) + 0.12, 0.0, 1.0);
        let col = mix(plasma_color(temp), prism(s.open * 1.7 + n.seed * 3.0) * 1.2, 0.55 * exp(-n.age / 2.5));
        return col * s.heat * (48.0 * exp(-n.age / 1.0) + 7.0 * exp(-n.age / 7.0) + 1.6);
    }
    var shade = mix(0.16, 1.0, s.open);
    if part == 0u {
        let up = world.z - n.at.z;
        let p = world - vec3<f32>(n.at.xy + lean(n, up), n.at.z) + sun * rc * 0.45;
        let inside = max(1.15 - head_body(n, p, rc, hc, r), 0.0);
        shade *= exp(-inside * 3.2);
    }
    // Cooled plasma is a blackened crimson, a little lighter as it thins.
    let body = mix(vec3<f32>(0.018, 0.008, 0.011), vec3<f32>(0.045, 0.028, 0.034), smoothstep(10.0, 90.0, n.age));
    let dust = vec3<f32>(0.04, 0.034, 0.034);
    let albedo = mix(body, dust, s.dust);
    let burning = clamp(s.heat * (star + glow) * 2.0, 0.0, 1.0);
    var c = albedo * (sun_col * (0.05 + 1.0 * shade) + sky * 0.25) * (1.0 - burning * 0.7);
    // The core lights what is round it from inside, a deep red long after the flash.
    let glow_at = n.at + vec3<f32>(lean(n, hc), hc);
    c += albedo * plasma_color(0.35 + 0.6 * star) * (star * 10.0 + glow * 1.6) * exp(-distance(world, glow_at) / (rc * 0.9));
    if s.wet > 0.5 {
        // The ring: a bright line of plasma, rose at first, red as it cools.
        return c + plasma_color(0.3 + 0.3 * s.heat * glow + 0.3 * star) * s.heat * (5.0 * glow + 30.0 * star + 1.2);
    }
    // Its own light: the star far past white at first (the bloom takes it), then threads
    // that glow red for half a minute.
    let temp = s.heat * (0.25 + 0.5 * glow) + 0.75 * star;
    c += plasma_color(temp) * pow(s.heat, 2.4) * (7.0 * glow + 14.0 * exp(-n.age / (6.0 * k)) + 70.0 * glare + 260.0 * star);
    // Arcs in the cloud: a violet-white flare round where one struck.
    if n.flash > 0.0 && part == 0u {
        let bolt_at = vec3<f32>(glow_at.xy, n.at.z + n.bolt_z);
        c += vec3<f32>(0.9, 0.72, 1.0) * n.flash * 2.6 * (1.0 - 0.6 * s.open) * exp(-distance(world, bolt_at) / (rc * 0.22));
    }
    return c;
}

// The ignition's light in the air round the star: white at its heart, a rose and red halo
// kilometres across, sinking over about fifteen seconds. Light only, as `ignition_glow`.
fn nova_ignition(n: Blast, eye: vec3<f32>, rd: vec3<f32>, scene_t: f32) -> vec3<f32> {
    let k = slow(n);
    let env = smoothstep(0.0, 0.06, n.age) * (0.65 * exp(-n.age / (2.6 * k)) + 0.35 * exp(-n.age / (6.0 * k)))
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
    let seen = mix(0.3, 1.0, smoothstep(t - 2.0 * rc, t, scene_t));
    let core = exp(-d / (0.5 * rc));
    let halo = 1.0 / (1.0 + (d / (2.4 * rc)) * (d / (2.4 * rc)));
    let core_col = plasma_color(0.75 + 0.25 * smoothstep(0.2, 0.7, env));
    let halo_col = plasma_color(0.42 + 0.2 * env);
    return (core_col * 28.0 * env * env * core + halo_col * 5.5 * pow(env, 1.6) * halo) * seen * cloud_veil(t);
}

// ---- the Regency's missiles in flight (vs_strategic / fs_strategic, MISSILE_PLASMA) -------
//
// The round in the Mangonel's vault (mc-models regency/strategic `warhead`) flying: an
// eight-faceted graphite body in steel courses under a faceted prow, chines down its
// sides and, at the tail, four wings swept forward. Violet line work is cut into the prow,
// chevrons on its facets and seams down every other edge, and burns brighter as the round
// heats coming down. An interceptor is the same round, smaller.
//
// The round is lofted pieces: each a profile of rings (along the body 0 tail .. 1 nose, a
// half-width and a centre, shares of the body's radius) drawn with `facets` flat sides. A
// plate's cross-section is a thin lozenge of a fixed thickness.

const NOVA_GRAPHITE: vec3<f32> = vec3<f32>(0.03, 0.03, 0.034);
const NOVA_STEEL: vec3<f32> = vec3<f32>(0.2, 0.205, 0.225);
const NOVA_VIOLET: vec3<f32> = vec3<f32>(0.62, 0.12, 1.0);
// The body, then four chines, then four wings: 7 * 8 * 6 + 8 * 3 * 4 * 6 vertices.
const NOVA_PIECES: u32 = 9u;

struct NovaPiece {
    first: u32,
    bands: u32,
    facets: u32,
    rot: f32,
    // A plate's thickness (shares of the body's radius); 0 for the body.
    thick: f32,
}

fn nova_round_ring(i: u32) -> vec3<f32> {
    let p = array<vec3<f32>, 16>(
        // 0: the body.
        vec3<f32>(0.0, 0.0, 0.0), vec3<f32>(0.0, 0.8, 0.0), vec3<f32>(0.03, 1.0, 0.0),
        vec3<f32>(0.78, 1.0, 0.0), vec3<f32>(0.86, 0.86, 0.0), vec3<f32>(0.93, 0.6, 0.0),
        vec3<f32>(0.98, 0.28, 0.0), vec3<f32>(1.0, 0.0, 0.0),
        // 8: a chine.
        vec3<f32>(0.2, 0.0, 1.0), vec3<f32>(0.3, 0.2, 1.2), vec3<f32>(0.72, 0.2, 1.2),
        vec3<f32>(0.82, 0.0, 1.0),
        // 12: a wing, its tip forward of its root.
        vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.14, 0.55, 1.55), vec3<f32>(0.22, 0.5, 2.3),
        vec3<f32>(0.28, 0.0, 2.8));
    return p[min(i, 15u)];
}

fn nova_piece(i: u32) -> NovaPiece {
    if i == 0u {
        return NovaPiece(0u, 7u, 8u, 0.0, 0.0);
    }
    let k = f32((i - 1u) % 4u) * TAU * 0.25;
    if i <= 4u {
        return NovaPiece(8u, 3u, 4u, k, 0.08);
    }
    return NovaPiece(12u, 3u, 4u, k + TAU * 0.125, 0.09);
}

// A corner of piece `p`: ring `k` of its profile, `side` round it.
fn nova_piece_point(p: NovaPiece, k: u32, side: f32, length: f32, radius: f32) -> vec3<f32> {
    let pr = nova_round_ring(p.first + k);
    let a = side / f32(p.facets) * TAU;
    let thick = select(pr.y, p.thick, p.thick > 0.0);
    let cs = vec2<f32>(cos(a) * pr.y + pr.z, sin(a) * thick);
    let r = vec2<f32>(cs.x * cos(p.rot) - cs.y * sin(p.rot), cs.x * sin(p.rot) + cs.y * cos(p.rot));
    return vec3<f32>(pr.x * length, r * radius);
}

struct NovaMissileVertex {
    // Along the body (metres), then across it.
    local: vec3<f32>,
    normal: vec3<f32>,
    along: f32,
    // Round the piece, 0..1 from facet 0.
    around: f32,
    // 0 the body, 1 a chine or wing.
    piece: u32,
    // Off the end of the mesh: not drawn.
    gone: bool,
}

fn nova_missile_vertex(vertex: u32, length: f32, radius: f32) -> NovaMissileVertex {
    var v: NovaMissileVertex;
    v.gone = true;
    var start = 0u;
    for (var i = 0u; i < NOVA_PIECES; i++) {
        let p = nova_piece(i);
        let count = p.bands * p.facets * 6u;
        if vertex < start + count {
            let w = vertex - start;
            let quad = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);
            let corner = quad[w % 6u];
            let band = w / (p.facets * 6u);
            let side = (w / 6u) % p.facets;
            let ring = band + select(0u, 1u, corner >= 2u);
            let around = side + select(0u, 1u, corner == 1u || corner == 2u);
            let p00 = nova_piece_point(p, band, f32(side), length, radius);
            let p01 = nova_piece_point(p, band, f32(side + 1u), length, radius);
            let p10 = nova_piece_point(p, band + 1u, f32(side), length, radius);
            let p11 = nova_piece_point(p, band + 1u, f32(side + 1u), length, radius);
            // Flat: the quad's own normal, from its diagonals (sound where one edge closes
            // to a point), turned outward from the piece's axis.
            var n = cross(p11 - p00, p10 - p01);
            let mid = (p00 + p01 + p10 + p11) * 0.25;
            let c = nova_round_ring(p.first + band).z * radius;
            let axis = vec2<f32>(cos(p.rot), sin(p.rot)) * c;
            n = n * select(1.0, -1.0, dot(n.yz, mid.yz - axis) < 0.0);
            v.gone = false;
            v.local = nova_piece_point(p, ring, f32(around), length, radius);
            v.normal = normalize(n + vec3<f32>(1e-6, 0.0, 0.0));
            v.along = nova_round_ring(p.first + ring).x;
            v.around = f32(around) / f32(p.facets);
            v.piece = min(i, 1u);
            return v;
        }
        start += count;
    }
    return v;
}

// A Regency missile's surface: its base colour and the violet light of its line work
// (added unlit), brighter with `heat`.
fn nova_missile_color(piece: u32, x: f32, around: f32, heat: f32) -> array<vec3<f32>, 2> {
    if piece == 1u {
        // Chines and wings: plate, a steel leading edge on the outer side.
        let edge = around < 0.12 || around > 0.88;
        return array<vec3<f32>, 2>(select(NOVA_GRAPHITE * 1.3, NOVA_STEEL * 0.8, edge), vec3<f32>(0.0));
    }
    var base = NOVA_GRAPHITE;
    if x < 0.03 || x > 0.975 || (x > 0.6 && x < 0.62) || (x > 0.78 && x < 0.8) {
        base = NOVA_STEEL;
    }
    // Across one facet, 0 at its middle, 0.5 at its edges.
    let f = fract(around * 8.0 + 1e-3);
    let d = abs(f - 0.5);
    let w = 0.0045;
    var line = 0.0;
    // A ring at the prow's foot.
    line = max(line, 1.0 - step(w * 1.4, abs(x - 0.806)));
    // A chevron on every facet, its point forward, and a smaller one ahead of it.
    line = max(line, 1.0 - step(w, abs(x - (0.9 - 0.14 * d))));
    line = max(line, (1.0 - step(w, abs(x - (0.945 - 0.08 * d)))) * step(d, 0.3));
    // Seams down every other edge, from the ring to the first chevron.
    let edge = (u32(floor(around * 8.0 + 0.5)) & 1u) == 0u;
    line = max(line, step(0.47, d) * step(0.806, x) * step(x, 0.83) * select(0.0, 1.0, edge));
    let pulse = 0.85 + 0.15 * sin(globals.camera.w * 4.0 + x * 9.0);
    return array<vec3<f32>, 2>(base, NOVA_VIOLET * line * (2.2 + 10.0 * heat) * pulse);
}
