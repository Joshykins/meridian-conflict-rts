// A Progenitor Shaper's dome (`shield_look::PROGENITOR`; shields.wgsl, whose shield
// helpers this uses). Nothing like ARC's honeycomb, the Regency's prism veil or the
// Heart's dark veil: clear panes of hard light laid in courses round the dome, each a
// parallelogram whose upright seams lean (the courses lean the other way each in turn),
// edged by a seam of cold light, and the courses turning slowly against each other.
// Its Shaper's tier (the tech byte of its `packed` word) brings more and finer courses,
// glyph strokes inset in the panes, rings of light running round the seams between
// courses, and under the glass more domes inside it, each turning its own way.
//
// A strike flashes the pane it lands on white, breaks it (its fill goes out and cracks
// run from the strike to its seams) and then the pane knits again: its seam is traced
// back round it from the corner nearest the strike, a hot point leading, until it is
// whole and fills again. A ring of light runs out along the seams round it.

const PANE_LIGHT: vec3<f32> = vec3<f32>(PROGENITOR_FX_LIGHT_R, PROGENITOR_FX_LIGHT_G, PROGENITOR_FX_LIGHT_B);
const PANE_HOT: vec3<f32> = vec3<f32>(PROGENITOR_FX_HOT_R, PROGENITOR_FX_HOT_G, PROGENITOR_FX_HOT_B);
// The glass between the seams: a faint cold cast, never dark (the Heart's veil is dark).
const PANE_FILL: vec3<f32> = vec3<f32>(0.16, 0.26, 0.5);
// How much wider than a course is tall a pane is, and how far its upright seams lean
// across a course, in panes.
const PANE_ASPECT: f32 = 1.8;
const PANE_LEAN: f32 = 0.34;
// Seconds a struck pane takes to flash, break and knit whole again.
const PANE_KNIT: f32 = 1.5;

fn is_progenitor_field(s: Shield) -> bool {
    return ((s.packed >> SHIELD_LOOK_SHIFT) & SHIELD_LOOK_MASK) == SHIELD_LOOK_PROGENITOR;
}

// Its Shaper's tier, 1 up (`mc_sim::survival::shaper_tier`).
fn pane_tier(s: Shield) -> f32 {
    return f32(clamp((s.packed >> 16u) & 255u, 1u, 32u));
}

// Courses of panes from the rim to the crown at `tier`: sixteen at the first, finer
// with each tier (a dome grows with its Shaper, so its panes grow with it).
fn pane_courses(tier: f32) -> f32 {
    return clamp(round(16.0 * (1.0 + 0.15 * (tier - 1.0))), 6.0, 40.0);
}

struct Pane {
    // Its course (below zero on the wall under the rim), its place round the course,
    // and a stable hash of it.
    id: vec3<f32>,
    // 0..1 across it and up it.
    uv: vec2<f32>,
    // Metres across and up it.
    size: vec2<f32>,
    // Metres to its nearest seam.
    seam: f32,
}

// The pane at `n` on a dome of radius `r` (the round sphere it is stretched from; `n`
// a unit direction), `drop` metres down the wall under its rim, its courses turned by
// `turn` turns (each course the other way from the last).
fn pane_at(n: vec3<f32>, r: f32, drop: f32, courses: f32, turn: f32) -> Pane {
    let span = 1.5707964 / courses;
    let tall = span * r;
    let elev = asin(clamp(n.z, 0.0, 1.0));
    var course = min(floor(elev / span), courses - 1.0);
    var v = elev / span - course;
    if drop > 0.0 {
        // The wall: courses of the same height running on down from the rim.
        let k = drop / tall;
        course = -1.0 - floor(k);
        v = 1.0 - fract(k);
    }
    let mid = (max(course, 0.0) + 0.5) * span;
    let around = 6.2831855 * r * cos(mid);
    let cells = max(round(around / (tall * PANE_ASPECT)), 3.0);
    let way = select(-1.0, 1.0, (i32(course + 64.0) & 1) == 0);
    let az = atan2(n.y, n.x) / 6.2831855 + turn * way * (1.0 + 0.15 * abs(course));
    let x = az * cells + PANE_LEAN * (v - 0.5) * way + course * 0.5;
    let idx = floor(x);
    let u = x - idx;
    let slot = idx - floor(idx / cells) * cells;
    var p: Pane;
    p.id = vec3<f32>(course, slot, hash21(vec2<f32>(course * 7.31 + 0.5, slot * 1.93 + 0.25)));
    p.uv = vec2<f32>(u, v);
    p.size = vec2<f32>(around / cells, tall);
    p.seam = min(min(u, 1.0 - u) * p.size.x * 0.95, min(v, 1.0 - v) * tall);
    return p;
}

// How far round a pane's seam, 0..1 from its lower left corner, at its nearest seam.
fn pane_round(uv: vec2<f32>, size: vec2<f32>) -> f32 {
    let d = vec4<f32>(uv.y * size.y, (1.0 - uv.x) * size.x, (1.0 - uv.y) * size.y, uv.x * size.x);
    let m = min(min(d.x, d.y), min(d.z, d.w));
    if m == d.x {
        return uv.x * 0.25;
    }
    if m == d.y {
        return 0.25 + uv.y * 0.25;
    }
    if m == d.z {
        return 0.5 + (1.0 - uv.x) * 0.25;
    }
    return 0.75 + (1.0 - uv.y) * 0.25;
}

// A struck pane's seam knitting round it: lit out both ways from `start` (0..1 round
// it) as far as `k` of the way round (1 whole), and the hot points leading.
fn pane_knit(at: f32, start: f32, k: f32) -> vec2<f32> {
    let d = abs(fract(at - start + 0.5) - 0.5) * 2.0;
    let lit = step(d, k);
    let head = exp(-abs(d - k) * 40.0) * step(k, 0.999);
    return vec2<f32>(lit, head);
}

// The panes round point `n` of one layer, and the strikes on them: rgb straight
// colour, a cover. `px_m` metres a pixel covers here; `fres` how grazing the eye is.
fn pane_layer(n: vec3<f32>, s: Shield, owner: u32, drop: f32, courses: f32, turn: f32,
              px_m: f32, fres: f32, time: f32, struck: bool) -> vec4<f32> {
    let tier = pane_tier(s);
    let r = s.radius;
    let p = pane_at(n, r, drop, courses, turn);
    // Seams a steady part of a pixel wide or more; panes under a few pixels fade.
    let w = max(0.55, px_m * 0.9);
    let see = smoothstep(2.0, 10.0, min(p.size.x, p.size.y) / max(px_m, 1e-4));
    var seam = (1.0 - smoothstep(w * 0.5, w, p.seam)) * see;
    // The glass of the pane: faint, a little more light leaning up it.
    var fill = 0.05 + 0.07 * p.uv.y;
    // A few panes hold more of the light, slowly breathing.
    let held = step(0.86, p.id.z) * (0.5 + 0.5 * sin(time * 0.8 + p.id.z * 41.0));
    fill += held * 0.12;
    var glyph = 0.0;
    if tier >= 2.0 {
        // A glyph stroke inset along the foot of some panes, and up the side of others.
        let g = fract(p.id.z * 17.3);
        let along = select(p.uv.x, p.uv.y, g > 0.5);
        let across = select(p.uv.y, 1.0 - p.uv.x, g > 0.5) * select(p.size.y, p.size.x, g > 0.5);
        let reach = 0.15 + 0.55 * fract(p.id.z * 5.7);
        glyph = (1.0 - smoothstep(w * 0.4, w * 0.8, abs(across - 0.22 * select(p.size.y, p.size.x, g > 0.5))))
            * step(0.12, along) * step(along, 0.12 + reach) * step(0.35, p.id.z) * see;
    }
    var ring = 0.0;
    if tier >= 3.0 && p.id.x >= 0.0 {
        // Rings of light running round the seams at the foot of every other course.
        let foot = (1.0 - smoothstep(w * 0.5, w * 1.2, p.uv.y * p.size.y)) * see;
        let odd = step(0.5, fract(p.id.x * 0.5)) * step(p.id.x, tier - 2.0);
        let way = select(-1.0, 1.0, (i32(p.id.x) & 1) == 0);
        let az = atan2(n.y, n.x);
        let run = pow(0.5 + 0.5 * sin(az * 4.0 - time * 1.3 * way + p.id.x), 10.0);
        ring = foot * odd * (0.35 + 2.4 * run);
    }
    // Strikes: the pane struck flashes, breaks and knits again; light runs out along the
    // seams round it.
    var flash = 0.0;
    var crack = 0.0;
    var knit = 1.0;
    var lead = 0.0;
    var wave = 0.0;
    if struck {
        for (var i = 0u; i < HIT_COUNT; i++) {
            let h = shield_hits[i];
            let age = time - h.start;
            if h.strength <= 0.0 || age < 0.0 || age > PANE_KNIT + 0.2 {
                continue;
            }
            if !hit_on_field(h.pos, s) {
                continue;
            }
            let q0 = dome_q(h.pos, s);
            let n0 = normalize(vec3<f32>(q0.xy, max(q0.z, 0.0)));
            let d = length(n - n0) * r;
            let strength = min(h.strength, 4.0);
            // Out along the seams round it.
            let front = age * (70.0 + strength * 25.0);
            wave += exp(-abs(d - front) / (5.0 + strength * 2.0)) * (1.0 - smoothstep(0.3, 1.2, age)) * strength * 0.5;
            if d > max(p.size.x, p.size.y) * 1.5 {
                continue;
            }
            let p0 = pane_at(n0, r, max(s.pos.z - h.pos.z, 0.0), courses, turn);
            if any(p0.id.xy != p.id.xy) {
                continue;
            }
            // The flash, the break and the knit.
            flash += exp(-age * 9.0) * min(strength, 1.5);
            let broken = smoothstep(0.02, 0.07, age) * (1.0 - smoothstep(PANE_KNIT * 0.75, PANE_KNIT, age));
            // Cracks from the strike to the seams, on a slant: three or four straight runs.
            let o = (p.uv - p0.uv) * p.size;
            var c = 0.0;
            for (var k = 0u; k < 4u; k++) {
                let a = hash11(h.start * 3.7 + f32(k) * 1.91) * 6.2831855;
                let dir = vec2<f32>(cos(a), sin(a));
                let off = abs(o.x * dir.y - o.y * dir.x);
                let ahead = step(0.0, dot(o, dir));
                c = max(c, (1.0 - smoothstep(w * 0.3, w * 0.9, off)) * ahead);
            }
            crack = max(crack, c * broken * (1.0 - smoothstep(0.15, 0.6, age)) * see);
            let k = smoothstep(0.18, PANE_KNIT * 0.8, age);
            let kn = pane_knit(pane_round(p.uv, p.size), pane_round(p0.uv, p.size), k);
            knit = min(knit, mix(1.0, kn.x, broken));
            lead = max(lead, kn.y * broken);
            fill *= 1.0 - broken * 0.9;
        }
    }
    // Light ebbs in and out of the seams as the dome loses its charge.
    let health = saturate(s.health);
    let flick = step(health * 1.15 + 0.05, hash21(vec2<f32>(p.id.z * 13.0, floor(time * 9.0))));
    seam *= 1.0 - flick * 0.85;
    let on_seam = seam * knit;
    var rgb = PANE_FILL * fill * (0.6 + fres);
    rgb += PANE_LIGHT * (on_seam * (0.55 + wave * 2.0) + glyph * 0.6 + ring * 1.4);
    flash = min(flash, 1.5);
    rgb += PANE_HOT * (flash * (0.6 + 0.4 * (1.0 - crack)) * 1.4 + crack * 3.0 + lead * seam * 5.0 + ring * 0.4);
    var a = fill * 0.4 + on_seam * 0.4 + glyph * 0.25 + ring * 0.3 + flash * 0.12 + crack * 0.4 + lead * seam * 0.5;
    a += wave * seam * 0.3;
    return vec4<f32>(rgb, a);
}

// The dome's straight colour and cover at `p`, before it is laid over the columns.
fn progenitor_glass(p: vec3<f32>, s: Shield, dir: vec3<f32>, owner: u32, time: f32, reveal: f32) -> vec4<f32> {
    let eye = globals.camera.xyz;
    var nrm = shell_normal(p, s);
    if dot(nrm, -dir) < 0.0 {
        nrm = -nrm;
    }
    let facing = saturate(dot(nrm, -dir));
    let fres = pow(1.0 - facing, 4.0);
    let tier = pane_tier(s);
    let courses = pane_courses(tier);
    let q = dome_q(p, s);
    let round = normalize(vec3<f32>(q.xy, max(q.z, 0.0)));
    let drop = max(s.pos.z - p.z, 0.0);
    let px_m = length(p - eye) / max(globals.lod.x, 1.0);
    var glass = pane_layer(round, s, owner, drop, courses, time * 0.006, px_m, fres, time, true);
    // Deeper layers at higher tiers: a dome inside it, and another, turning their own
    // ways, seen through the outer panes (their near side only).
    let layers = min(floor((tier - 1.0) / 3.0), 2.0);
    for (var l = 1.0; l <= layers; l += 1.0) {
        let k = 1.0 - 0.07 * l;
        var inner = s;
        inner.radius = s.radius * k;
        let hits = shell_hits(eye, dir, inner);
        if hits.x <= 0.0 || hits.x >= hits.y {
            continue;
        }
        let pi = eye + dir * hits.x;
        let qi = dome_q(pi, inner);
        let ri = normalize(vec3<f32>(qi.xy, max(qi.z, 0.0)));
        let fi = pow(1.0 - saturate(abs(dot(shell_normal(pi, inner), dir))), 4.0);
        let deep = pane_layer(ri, inner, owner, max(s.pos.z - pi.z, 0.0), courses + l,
                              -time * 0.009 * l, px_m, fi, time + l * 7.0, false);
        glass = vec4<f32>(glass.rgb + deep.rgb * 0.55 * (1.0 - saturate(glass.a)), glass.a + deep.a * 0.35);
    }
    // The rim: a hard line of light on the silhouette.
    let rim = pow(1.0 - facing, 8.0);
    var rgb = glass.rgb + PANE_LIGHT * (fres * 0.5 + rim * 2.2);
    var a = glass.a + fres * 0.12 + rim * 0.3;
    // Opening: the lip of light climbing the dome; collapsing, it runs down hot.
    let open = shield_open(s);
    let dying = select(0.0, 1.0, collapsing(s));
    let born = exp(-abs(reveal) * mix(16.0, 8.0, dying)) * max(1.0 - open, dying * 0.5);
    rgb += PANE_HOT * born * 3.0;
    a += born * 0.45;
    rgb += PANE_LIGHT * contact_at(p, s) * 3.0;
    a += contact_at(p, s) * 0.3;
    a *= smoothstep(0.0, 0.1, open);
    return vec4<f32>(rgb, saturate(a));
}
