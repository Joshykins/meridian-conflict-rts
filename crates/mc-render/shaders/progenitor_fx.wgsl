//!use bindings
// Survival's Progenitor in hard light (renderer/progenitor_fx.rs, progenitor_fx/pass.rs):
// a Shaper's Lance charging and striking, its anti-nuke's shards, replication shells
// and the Unmaker's warheads, and what each leaves on the ground. Hard light is not
// plasma and not fire: crisp-edged and flat, cold blue-white going white-hot, lines,
// panes and rings rather than glows and balls, and it goes out by withdrawing or
// breaking into lines rather than by billowing away.
//
// Everything is built of pieces (`ProgenitorPiece`), one instance of its kind's mesh
// each, drawn kind by kind (`PROGENITOR_FX_*` kinds):
//
// - **Beam**: a line of light from `a` to `b` turned to the eye, `size_from` wide at
//   `a` and `size_to` at `b`. It is drawn on from `a` over its `grow` share of its life
//   and withdraws into `b` at the end. Flat-topped and hard-edged with a line of light
//   down each edge; a drive's (`PULSES`) has bright chevrons running down it, a trail
//   (`TRAIL`) fades from `b` back to `a`, and a Lance's core or an unmaking line (`LINE`)
//   is a thin thread.
// - **Ring**: round `a` in the plane facing `b`, or laid on the ground (`DRAPE`). Its
//   radius runs from `size_from` to `size_to` over its `grow` share, `width` metres
//   across. In arcs (`ARCS`, more with its tier), a target (`TARGET`: dashes turning, a
//   ring inside, three ticks), or a mark burnt on the ground (`MARK`: a disc of rings,
//   spokes and a lattice that cools from white to blue and goes out, the ground under it
//   scorched).
// - **Prism**: an open column of `PRISM_SIDES` faces standing from `a` up `b`,
//   `size_from` across at its foot and `size_to` at its head, rising over its `grow`
//   share. Lit at its corners; a print column (`PRINT`) has bands of light climbing it,
//   a pillar streaks of light running up.
// - **Body**: a solid Precursor body, nose at `a`, flying along `b`, its back toward
//   `c`: a flattened six-faced hull with its nose cut on a slant, a waist where the dark
//   core shows, pale alloy lit by the sun with seams of cold light. `size_from` its
//   half-width, `size_to` its length. Of hard light instead (`LIGHT`): a shard.
// - **Shell**: a sphere of hard light round `a` (its upper half, `DOME`), its radius
//   running from `size_from` to `size_to` over its `grow` share, squashed to `c.x` of its
//   height (zero: round): a lattice of lines with its triangles lit faintly.
//
// A piece is lit from `start` (renderer time) for `life` seconds at `power`. Light is
// added onto the scene (premultiplied, alpha only where a mark darkens the ground).

//!rust crate::renderer::progenitor_fx::pass::GpuPiece
struct ProgenitorPiece {
    a: vec3<f32>,
    start: f32,
    b: vec3<f32>,
    life: f32,
    c: vec3<f32>,
    power: f32,
    size_from: f32,
    size_to: f32,
    kind: u32,
    flags: u32,
    seed: f32,
    tier: f32,
    width: f32,
    grow: f32,
}

@group(1) @binding(0) var<storage, read> pieces: array<ProgenitorPiece>;

const LIGHT: vec3<f32> = vec3<f32>(PROGENITOR_FX_LIGHT_R, PROGENITOR_FX_LIGHT_G, PROGENITOR_FX_LIGHT_B);
const HOT: vec3<f32> = vec3<f32>(PROGENITOR_FX_HOT_R, PROGENITOR_FX_HOT_G, PROGENITOR_FX_HOT_B);
const ALLOY: vec3<f32> = vec3<f32>(PROGENITOR_FX_ALLOY_R, PROGENITOR_FX_ALLOY_G, PROGENITOR_FX_ALLOY_B);
const CORE: vec3<f32> = vec3<f32>(PROGENITOR_FX_CORE_R, PROGENITOR_FX_CORE_G, PROGENITOR_FX_CORE_B);

struct PieceOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    // Per kind: a beam's (along, across -1..1); a ring's (across 0..1 inside out, round
    // 0..1); a column's (round in faces, up 0..1); a body's (along 0..1, round in faces);
    // a sphere's (round 0..1, up 0..1).
    @location(1) uv: vec2<f32>,
    @location(2) @interpolate(flat) piece: u32,
    // The surface's outward normal (a body, a column), else zero.
    @location(3) normal: vec3<f32>,
    // Metres: a beam's length drawn and how much of its width is its own (a beam held
    // to a pixel's width is dimmed by it); a ring's radius here and its outer radius.
    @location(4) metres: vec2<f32>,
}

fn hidden() -> PieceOut {
    var out: PieceOut;
    out.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
    return out;
}

fn has(p: ProgenitorPiece, flag: u32) -> bool {
    return (p.flags & flag) != 0u;
}

fn age_of(p: ProgenitorPiece) -> f32 {
    return globals.camera.w - p.start;
}

// How far through its life, 0..1.
fn share_of(p: ProgenitorPiece) -> f32 {
    return saturate(age_of(p) / max(p.life, 1e-3));
}

fn alive(p: ProgenitorPiece) -> bool {
    let age = age_of(p);
    return age >= 0.0 && age <= p.life;
}

// Drawn on over its `grow` share of its life (at once with none): fast then settling, or
// slow then fast for a cage closing in (`CLOSE`).
fn rise_of(p: ProgenitorPiece) -> f32 {
    if p.grow <= 0.0 {
        return 1.0;
    }
    let t = saturate(share_of(p) / p.grow);
    if has(p, PROGENITOR_FX_CLOSE) {
        return t * t;
    }
    return 1.0 - pow(1.0 - t, 3.0);
}

// Its light over its life: a flash as it is drawn on, held, then going out over the
// last part of its life.
fn light_of(p: ProgenitorPiece) -> f32 {
    let t = share_of(p);
    let hold = max(p.grow, 0.35);
    let out = 1.0 - smoothstep(hold, 1.0, t);
    let flash = exp(-age_of(p) * 8.0) * step(0.0, p.grow - 0.001);
    return p.power * out * (1.0 + flash * 1.5);
}

// Metres a pixel covers at `at`.
fn pixel_m(at: vec3<f32>) -> f32 {
    return length(at - globals.camera.xyz) / max(globals.lod.x, 1.0);
}

// Two directions square to `n` and to each other.
fn basis(n: vec3<f32>) -> mat2x3<f32> {
    let up = select(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(1.0, 0.0, 0.0), abs(n.z) > 0.9);
    let e1 = normalize(cross(n, up));
    return mat2x3<f32>(e1, cross(n, e1));
}

fn ground_at(xy: vec2<f32>) -> f32 {
    return max(terrain_height(xy), globals.map.z);
}

fn finish(rgb: vec3<f32>, a: f32, clip: vec4<f32>) -> vec4<f32> {
    let k = past_clouds_at(clip);
    return vec4<f32>(max(rgb, vec3<f32>(0.0)) * k, saturate(a) * k);
}

const QUAD: array<u32, 6> = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);

// ---- Beams ------------------------------------------------------------------------

@vertex
fn vs_beam(@builtin(vertex_index) vid: u32, @builtin(instance_index) inst: u32) -> PieceOut {
    let p = pieces[inst];
    if !alive(p) {
        return hidden();
    }
    let step_i = vid / 6u;
    let corner = QUAD[vid % 6u];
    let s = f32(step_i + select(0u, 1u, corner == 1u || corner == 2u)) / f32(PROGENITOR_FX_BEAM_STEPS);
    let side = select(-1.0, 1.0, corner >= 2u);
    // Drawn on from `a`; a strike withdraws into `b` at the end, a trail does not.
    let head = rise_of(p);
    let tail = select(smoothstep(0.72, 1.0, share_of(p)), 0.0, has(p, PROGENITOR_FX_TRAIL));
    let along = mix(tail, head, s);
    let at = mix(p.a, p.b, along);
    let dir = normalize(p.b - p.a + vec3<f32>(0.0, 0.0, 1e-5));
    let eye = globals.camera.xyz;
    let across = normalize(cross(dir, normalize(eye - at)));
    var width = mix(p.size_from, p.size_to, along);
    // Never thinner than a pixel and a half; then dimmer by as much.
    let least = pixel_m(at) * 1.5;
    let own = saturate(width / least);
    width = max(width, least);
    var out: PieceOut;
    out.world = at + across * side * width * 0.5;
    out.clip = globals.view_proj * vec4<f32>(out.world, 1.0);
    out.uv = vec2<f32>(along, side);
    out.piece = inst;
    out.normal = vec3<f32>(0.0);
    out.metres = vec2<f32>(along * length(p.b - p.a), own);
    return out;
}

@fragment
fn fs_beam(in: PieceOut) -> @location(0) vec4<f32> {
    let p = pieces[in.piece];
    let age = age_of(p);
    let x = abs(in.uv.y);
    let along_m = in.metres.x;
    var core = 0.0;
    var edge = 0.0;
    if has(p, PROGENITOR_FX_LINE) {
        core = 1.0 - smoothstep(0.15, 1.0, x);
    } else {
        // Hard light: flat across, crisp at its sides, a line of light down each edge.
        core = 1.0 - smoothstep(0.32, 0.46, x);
        edge = exp(-abs(x - 0.7) * 22.0) * 0.8;
    }
    var pulse = 0.0;
    if has(p, PROGENITOR_FX_PULSES) {
        // Chevrons running down it from the Lens to the strike.
        let chevron = fract((along_m - x * 4.0) / 16.0 - age * 7.0);
        pulse = smoothstep(0.75, 0.92, chevron) * (1.0 - smoothstep(0.92, 1.0, chevron)) * (1.0 - x);
    }
    var fade = 1.0;
    if has(p, PROGENITOR_FX_TRAIL) {
        // Brightest at its new end.
        fade = in.uv.x;
        // A trail breaks into dashes as it ages.
        let dash = step(0.35 * share_of(p), fract(along_m / 9.0 + p.seed));
        fade *= mix(1.0, dash, smoothstep(0.2, 0.6, share_of(p)));
    }
    let k = light_of(p) * fade * in.metres.y;
    let rgb = HOT * (core * 1.6 + pulse * 2.5) + LIGHT * (edge * 1.4 + core * 0.4);
    return finish(rgb * k, 0.0, in.clip);
}

// ---- Rings ------------------------------------------------------------------------

fn ring_outer(p: ProgenitorPiece) -> f32 {
    return mix(p.size_from, p.size_to, rise_of(p));
}

fn ring_inner(p: ProgenitorPiece, outer: f32) -> f32 {
    if has(p, PROGENITOR_FX_MARK | PROGENITOR_FX_TARGET) {
        return 0.0;
    }
    return max(outer - p.width, 0.0);
}

@vertex
fn vs_ring(@builtin(vertex_index) vid: u32, @builtin(instance_index) inst: u32) -> PieceOut {
    let p = pieces[inst];
    if !alive(p) {
        return hidden();
    }
    let quad = vid / 6u;
    let corner = QUAD[vid % 6u];
    let seg = quad % PROGENITOR_FX_RING_SEGMENTS + select(0u, 1u, corner == 1u || corner == 2u);
    let band = quad / PROGENITOR_FX_RING_SEGMENTS + select(0u, 1u, corner >= 2u);
    let outer = ring_outer(p);
    let inner = ring_inner(p, outer);
    let across = f32(band) / f32(PROGENITOR_FX_RING_BANDS);
    // A mark's bands crowd toward the middle, where its lattice is.
    let r = mix(inner, outer, select(across, across * across, has(p, PROGENITOR_FX_MARK)));
    let round = f32(seg) / f32(PROGENITOR_FX_RING_SEGMENTS);
    let ang = round * 6.2831855;
    var at: vec3<f32>;
    if has(p, PROGENITOR_FX_DRAPE) {
        let xy = p.a.xy + vec2<f32>(cos(ang), sin(ang)) * r;
        at = vec3<f32>(xy, ground_at(xy) + 0.8 + r * 0.004);
    } else {
        let e = basis(normalize(p.b + vec3<f32>(0.0, 0.0, 1e-6)));
        at = p.a + (e[0] * cos(ang) + e[1] * sin(ang)) * r;
    }
    var out: PieceOut;
    out.world = at;
    out.clip = globals.view_proj * vec4<f32>(at, 1.0);
    out.uv = vec2<f32>(select(across, saturate(r / max(outer, 1e-3)), has(p, PROGENITOR_FX_MARK)), round);
    out.piece = inst;
    out.normal = vec3<f32>(0.0);
    out.metres = vec2<f32>(r, outer);
    return out;
}

@fragment
fn fs_ring(in: PieceOut) -> @location(0) vec4<f32> {
    let p = pieces[in.piece];
    let age = age_of(p);
    let t = share_of(p);
    let r = in.metres.x;
    let outer = max(in.metres.y, 1e-3);
    let px = pixel_m(in.world);
    let w = max(0.5, px * 1.2);
    let round = in.uv.y;
    var rgb = vec3<f32>(0.0);
    var a = 0.0;
    let k = light_of(p);
    if has(p, PROGENITOR_FX_MARK) {
        // Burnt into the ground: rings, spokes and a lattice, white-hot at first, cooling
        // to the cold blue and going out; the ground darkened under it.
        let cool = smoothstep(0.0, 0.35, t);
        let lines_c = mix(HOT * 2.0, LIGHT, cool);
        let rr = r / outer;
        var lines = 0.0;
        for (var i = 1; i <= 3; i++) {
            lines = max(lines, 1.0 - smoothstep(w * 0.5, w, abs(rr - f32(i) / 3.0) * outer));
        }
        let spokes = 6.0 + 2.0 * min(p.tier, 4.0);
        let sp = abs(fract(round * spokes + p.seed) - 0.5) * 2.0;
        let spoke = (1.0 - smoothstep(w * 0.5, w, sp * 0.5 / spokes * 6.2831855 * r)) * step(0.33, rr);
        let tri = tri_lattice(in.world.xy - p.a.xy, max(outer * 0.12, 3.0));
        let lattice = smoothstep(0.86, 0.97, tri.x) * (1.0 - smoothstep(0.3, 1.0, rr)) * 0.35;
        let glow = lines + spoke * 0.8 + lattice;
        let out = 1.0 - smoothstep(0.0, 1.0, t);
        rgb = lines_c * glow * p.power * out * (1.0 - rr * 0.4);
        // The scorch: a dark disc with a soft edge that fades slower than the lines.
        a = (1.0 - smoothstep(0.75, 1.0, rr)) * 0.35 * (1.0 - smoothstep(0.4, 1.0, t));
        return finish(rgb, a, in.clip);
    }
    if has(p, PROGENITOR_FX_TARGET) {
        // Dashes turning round its rim, a finer ring inside and three ticks pointing in.
        let rim = 1.0 - smoothstep(w * 0.6, w * 1.4, abs(r - outer + w));
        let dash = step(0.45, fract(round * 24.0 - age * 0.4));
        let inner_c = 1.0 - smoothstep(w * 0.4, w, abs(r - outer * 0.62));
        let tick_a = abs(fract(round * 3.0 + age * 0.08) - 0.5) * 2.0 * 3.1415927 * r / 3.0;
        let tick = (1.0 - smoothstep(w * 0.6, w * 1.2, tick_a)) * step(outer * 0.66, r) * step(r, outer * 0.86);
        rgb = LIGHT * (rim * dash * 1.4 + inner_c * 0.7 + tick * 1.2) + HOT * tick * 0.6;
        return finish(rgb * k, 0.0, in.clip);
    }
    // A ring of light: a hard outer edge, the band behind it faint, fading in from inside.
    let across = in.uv.x;
    var rim = 1.0 - smoothstep(w * 0.5, w * 1.2, outer - r);
    let band = pow(across, 3.0) * 0.45;
    var gap = 1.0;
    if has(p, PROGENITOR_FX_ARCS) {
        let arcs = 3.0 + min(p.tier, 6.0);
        let f = fract(round * arcs + p.seed + age * 0.06 * select(-1.0, 1.0, p.seed > 0.5));
        gap = step(0.1, f) * step(f, 0.94);
        // A notch of light at the end of each arc.
        rim += (1.0 - smoothstep(0.0, 0.03, abs(f - 0.92))) * 0.8;
    }
    rgb = (HOT * rim * 1.8 + LIGHT * band) * gap;
    return finish(rgb * k, 0.0, in.clip);
}

// ---- Columns ----------------------------------------------------------------------

@vertex
fn vs_prism(@builtin(vertex_index) vid: u32, @builtin(instance_index) inst: u32) -> PieceOut {
    let p = pieces[inst];
    if !alive(p) {
        return hidden();
    }
    let sides = PROGENITOR_FX_PRISM_SIDES;
    let quad = vid / 6u;
    let corner = QUAD[vid % 6u];
    let side = quad % sides + select(0u, 1u, corner == 1u || corner == 2u);
    let band = quad / sides + select(0u, 1u, corner >= 2u);
    let up = f32(band) / f32(PROGENITOR_FX_PRISM_BANDS);
    let height = length(p.b);
    let axis = p.b / max(height, 1e-3);
    let e = basis(axis);
    let turn = p.seed * 6.2831855 + age_of(p) * 0.25;
    let ang = f32(side) / f32(sides) * 6.2831855 + turn;
    let r = mix(p.size_from, p.size_to, up);
    let radial = e[0] * cos(ang) + e[1] * sin(ang);
    let at = p.a + axis * height * rise_of(p) * up + radial * r;
    var out: PieceOut;
    out.world = at;
    out.clip = globals.view_proj * vec4<f32>(at, 1.0);
    out.uv = vec2<f32>(f32(side), up);
    out.piece = inst;
    out.normal = radial;
    out.metres = vec2<f32>(height * rise_of(p), r);
    return out;
}

@fragment
fn fs_prism(in: PieceOut) -> @location(0) vec4<f32> {
    let p = pieces[in.piece];
    let age = age_of(p);
    let h = in.uv.y;
    let tall = in.metres.x;
    let face = 2.0 * in.metres.y * sin(3.1415927 / f32(PROGENITOR_FX_PRISM_SIDES));
    let px = pixel_m(in.world);
    let w = max(0.5, px * 1.2);
    let c = abs(fract(in.uv.x + 0.5) - 0.5) * face;
    let corner_l = 1.0 - smoothstep(w * 0.5, w * 1.3, c);
    let view = normalize(globals.camera.xyz - in.world);
    let graze = 1.0 - abs(dot(normalize(in.normal), view));
    var fill = 0.08 + graze * 0.25;
    var lines = corner_l;
    if has(p, PROGENITOR_FX_PRINT) {
        // Bands of light climbing it as it prints.
        let b = fract(h * tall / 9.0 - age * 1.4);
        lines += (1.0 - smoothstep(0.0, max(w / 9.0, 0.06), abs(b - 0.5))) * 0.9;
        fill += 0.08;
    } else {
        // A pillar: streaks running up it.
        let lane = floor(in.uv.x * 5.0);
        let streak = fract(h * tall / 60.0 - age * 2.2 + hash11(lane + p.seed * 13.0));
        lines += smoothstep(0.7, 0.98, streak) * 0.7 * step(0.4, hash11(lane * 1.7 + p.seed));
    }
    // Faded into the air at its head, sharp at its foot.
    let top = pow(1.0 - h, 1.3) * smoothstep(0.0, 0.03, h + 0.02);
    let k = light_of(p) * top;
    let rgb = HOT * lines * 1.4 + LIGHT * fill;
    return finish(rgb * k, 0.0, in.clip);
}

// ---- Bodies -----------------------------------------------------------------------

// A body's profile, nose (0) to tail (1): along, half-width share.
fn body_station(k: u32) -> vec2<f32> {
    let s = array<vec2<f32>, 8>(
        vec2<f32>(0.0, 0.12), vec2<f32>(0.14, 0.72), vec2<f32>(0.32, 1.0), vec2<f32>(0.6, 1.0),
        vec2<f32>(0.64, 0.8), vec2<f32>(0.69, 0.92), vec2<f32>(0.96, 0.78), vec2<f32>(1.0, 0.0));
    return s[min(k, 7u)];
}

@vertex
fn vs_body(@builtin(vertex_index) vid: u32, @builtin(instance_index) inst: u32) -> PieceOut {
    let p = pieces[inst];
    if !alive(p) {
        return hidden();
    }
    let sides = PROGENITOR_FX_BODY_SIDES;
    let quad = vid / 6u;
    let corner = QUAD[vid % 6u];
    let side = quad % sides + select(0u, 1u, corner == 1u || corner == 2u);
    let band = quad / sides + select(0u, 1u, corner >= 2u);
    let st = body_station(band);
    let axis = normalize(p.b + vec3<f32>(0.0, 0.0, 1e-6));
    var up = p.c - axis * dot(p.c, axis);
    if dot(up, up) < 1e-6 {
        up = basis(axis)[1];
    }
    up = normalize(up);
    let right = cross(axis, up);
    // Six faces, flattened: wider than it is deep.
    let ang = (f32(side) + 0.5) / f32(sides) * 6.2831855;
    let y = cos(ang);
    let z = sin(ang) * 0.58;
    let r = p.size_from * st.y;
    // The nose cut on a slant: its back runs further forward than its belly.
    let along = st.x - select(0.0, z * 0.12, band <= 1u);
    let at = p.a - axis * along * p.size_to + (right * y + up * z) * r;
    var out: PieceOut;
    out.world = at;
    out.clip = globals.view_proj * vec4<f32>(at, 1.0);
    out.uv = vec2<f32>(st.x, f32(side));
    out.piece = inst;
    out.normal = normalize(right * y + up * z / 0.58 * 0.6);
    out.metres = vec2<f32>(p.size_to, p.size_from);
    return out;
}

@fragment
fn fs_body(in: PieceOut) -> @location(0) vec4<f32> {
    let p = pieces[in.piece];
    let eye = globals.camera.xyz;
    let view = normalize(eye - in.world);
    // Faceted: the face's own normal, turned to the eye.
    var n = normalize(cross(dpdx(in.world), dpdy(in.world)));
    if dot(n, view) < 0.0 {
        n = -n;
    }
    // Only the faces toward the eye: the hull is near enough convex.
    if dot(in.normal, view) < -0.25 {
        discard;
    }
    let along = in.uv.x;
    let px = pixel_m(in.world);
    let len = max(in.metres.x, 1e-3);
    let w = max(0.06, px * 1.1) / len;
    // Seams: round the hull where the nose meets the body, either side of the waist, and
    // the one live seam down its back.
    var seam = 0.0;
    seam = max(seam, 1.0 - smoothstep(w * 0.5, w, abs(along - 0.14)));
    seam = max(seam, 1.0 - smoothstep(w * 0.5, w, abs(along - 0.6)));
    seam = max(seam, 1.0 - smoothstep(w * 0.5, w, abs(along - 0.69)));
    let ridge = abs(fract(in.uv.y + 0.5) - 0.5);
    let back = (1.0 - smoothstep(0.02, 0.06, ridge)) * step(0.2, along) * step(along, 0.95)
        * step(0.5, dot(in.normal, normalize(p.c - normalize(p.b) * dot(p.c, normalize(p.b)) + vec3<f32>(0.0, 0.0, 1e-6))));
    seam = max(seam, back);
    let waist = step(0.6, along) * step(along, 0.69);
    let k = light_of(p);
    if has(p, PROGENITOR_FX_LIGHT) {
        // A shard of hard light: bright at its facets' edges, clear between.
        let graze = 1.0 - abs(dot(n, view));
        let rgb = HOT * (0.5 + graze * 1.6 + seam * 2.0) + LIGHT * 0.6;
        return finish(rgb * k, 0.0, in.clip);
    }
    let sun = globals.sun.xyz;
    let lambert = max(dot(n, sun), 0.0);
    let h = normalize(sun + view);
    let spec = pow(max(dot(n, h), 0.0), 40.0) * 0.4;
    let base = mix(ALLOY, CORE, waist);
    var rgb = base * (atmos.sun_color.rgb * lambert * 1.05 + atmos.sky_color.rgb * 0.7)
        + atmos.sun_color.rgb * spec * (1.0 - waist);
    rgb += (LIGHT * 2.2 + HOT * waist * 1.2) * seam * k;
    // Its tail: the drive's hard light in the cut face.
    rgb += HOT * step(0.96, along) * 3.0 * k;
    return vec4<f32>(rgb * past_clouds_at(in.clip), 1.0);
}

// ---- Spheres ----------------------------------------------------------------------

@vertex
fn vs_shell(@builtin(vertex_index) vid: u32, @builtin(instance_index) inst: u32) -> PieceOut {
    let p = pieces[inst];
    if !alive(p) {
        return hidden();
    }
    let segs = PROGENITOR_FX_SHELL_SEGMENTS;
    let quad = vid / 6u;
    let corner = QUAD[vid % 6u];
    let seg = quad % segs + select(0u, 1u, corner == 1u || corner == 2u);
    let band = quad / segs + select(0u, 1u, corner >= 2u);
    let up = f32(band) / f32(PROGENITOR_FX_SHELL_BANDS);
    // Pole to pole, or the rim to the crown.
    let elev = select(-1.5707964 + up * 3.1415927, up * 1.5707964, has(p, PROGENITOR_FX_DOME));
    let round = f32(seg) / f32(segs);
    let turn = p.seed * 6.2831855 + age_of(p) * 0.15;
    let ang = round * 6.2831855 + turn;
    let r = mix(p.size_from, p.size_to, rise_of(p));
    let squash = select(p.c.x, 1.0, p.c.x <= 0.0);
    let dir = vec3<f32>(cos(elev) * cos(ang), cos(elev) * sin(ang), sin(elev));
    let at = p.a + vec3<f32>(dir.xy, dir.z * squash) * r;
    var out: PieceOut;
    out.world = at;
    out.clip = globals.view_proj * vec4<f32>(at, 1.0);
    out.uv = vec2<f32>(round, up);
    out.piece = inst;
    out.normal = dir;
    out.metres = vec2<f32>(r, 0.0);
    return out;
}

@fragment
fn fs_shell(in: PieceOut) -> @location(0) vec4<f32> {
    let p = pieces[in.piece];
    let r = max(in.metres.x, 1e-3);
    let px = pixel_m(in.world);
    // A lattice: lines round it and up it, and the diagonals between, so its faces are
    // triangles; each line held a pixel or so wide.
    let segs = f32(PROGENITOR_FX_SHELL_SEGMENTS) * 0.5;
    let bands = f32(PROGENITOR_FX_SHELL_BANDS) * select(0.5, 1.0, has(p, PROGENITOR_FX_DOME));
    let cell = 6.2831855 * r / segs;
    let w = max(0.06, px * 1.2 / cell);
    let gu = abs(fract(in.uv.x * segs + 0.5) - 0.5);
    let gv = abs(fract(in.uv.y * bands + 0.5) - 0.5);
    let gd = abs(fract(in.uv.x * segs - in.uv.y * bands + 0.5) - 0.5);
    let line = 1.0 - smoothstep(w * 0.5, w, min(gu, min(gv, gd * 0.7)));
    let tri = hash21(floor(vec2<f32>(in.uv.x * segs, in.uv.y * bands)) + vec2<f32>(p.seed * 9.0, 0.0));
    let view = normalize(globals.camera.xyz - in.world);
    let graze = 1.0 - abs(dot(normalize(in.normal), view));
    // Some triangles drop out as it goes: it breaks into lines.
    let t = share_of(p);
    let gone = step(tri, smoothstep(0.35, 1.0, t));
    let pane = (0.015 + 0.08 * step(0.85, tri)) * (1.0 - gone);
    let k = light_of(p);
    // Its lines cold blue going white where it is seen edge-on.
    let rgb = mix(LIGHT, HOT, graze) * line * (0.5 + graze * 0.8) + LIGHT * pane;
    return finish(rgb * k, 0.0, in.clip);
}
