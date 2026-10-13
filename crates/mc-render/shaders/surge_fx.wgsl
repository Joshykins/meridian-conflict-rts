//!use bindings
// A surge shell's wake (renderer/surge_fx.rs, surge_fx/pass.rs), drawn as two meshes, one
// instance of each a surge:
//
// - **The front**, the swell: a low ripple standing on the ring where the front stands,
//   all round the crater. Not a wall: a long shallow back of bent violet air rising to a
//   thin pale rose crest no taller than a man, then a short steep face to a bright line
//   along its foot. It thins away as it reaches its radius.
// - **The burn** behind it: the disc it has rolled over draped on the ground, veined with
//   cracks of light. Just behind the front they burn white and rose; they settle to
//   crimson embers pulsing slowly, faint rings of violet running out across them, and the
//   ground between is darkened as if scorched. At the end it dies down.
//
// Both are light laid over the scene premultiplied: mostly light added, a little of what
// is behind dimmed.
//
// The Edict's strike (renderer/nuke_fx/edict.rs) is drawn here too, each record by its
// `look` on finer meshes (`SURGE_FX_EDICT_*`), with none of the Saker's violet: its molten
// wave (a low molten crest rolling out, the ground behind it molten), its shock (a hard
// line of light racing out on the blast front's law) and the lens of pinched plasma
// pressed flat over its mark. They have entry points of their own (`vs_edict_*`,
// `fs_edict_*`) in the second half of this file.

//!rust crate::renderer::surge_fx::pass::GpuSurge
struct SurgeWake {
    // The crater, and the renderer time the shell landed.
    centre: vec3<f32>,
    start: f32,
    // Metres out it rolls, and metres a second.
    radius: f32,
    speed: f32,
    // Seconds the ground burns once the front has rolled out.
    linger: f32,
    // Sets each surge's noise apart.
    seed: f32,
    // How hard it lands: the front's height and light.
    power: f32,
    // SURGE_FX_SAKER, SURGE_FX_EDICT_WAVE, ...
    look: u32,
    _pad1: f32,
    _pad2: f32,
}

@group(1) @binding(0) var<storage, read> surges: array<SurgeWake>;

const WHITE: vec3<f32> = vec3<f32>(1.0, 0.93, 0.95);
const ROSE: vec3<f32> = vec3<f32>(1.0, 0.45, 0.5);
const CRIMSON: vec3<f32> = vec3<f32>(0.75, 0.03, 0.06);
const EMBER: vec3<f32> = vec3<f32>(0.32, 0.01, 0.03);
const VIOLET: vec3<f32> = vec3<f32>(0.55, 0.16, 1.0);
// Metres between the burn's cracks, coarse and fine.
const CRACK: f32 = 7.0;
const CRACK_FINE: f32 = 2.6;
// Metres apart the aftershock rings run, and their pace against the front's.
const SHOCK_GAP: f32 = 26.0;
const SHOCK_PACE: f32 = 0.4;
// The swell: metres of back behind its crest, of face ahead of it, and where along its
// section the crest stands.
const BACK: f32 = 12.0;
const FACE: f32 = 1.8;
const CREST: f32 = 0.82;

struct SurgeOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    // The front: round the ring (0..1), and across its section (0 back foot, 1 face foot).
    @location(2) at: vec2<f32>,
    @location(3) @interpolate(flat) surge: u32,
}

fn hidden() -> SurgeOut {
    var out: SurgeOut;
    out.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
    return out;
}

fn ground_at(xy: vec2<f32>) -> f32 {
    return max(terrain_height(xy), globals.map.z);
}

// Seconds since the shell landed.
fn age_of(s: SurgeWake) -> f32 {
    return globals.camera.w - s.start;
}

// Seconds the front takes to roll out.
fn roll_of(s: SurgeWake) -> f32 {
    return s.radius / max(s.speed, 1.0);
}

// Metres out the front stands now: past its radius it eases on a little as it thins. The
// Edict's shock runs out on the blast's square-root law (renderer `Surge::front`) and on
// past its radius as it thins over its linger.
fn front_of(s: SurgeWake) -> f32 {
    let age = age_of(s);
    if s.look == SURGE_FX_EDICT_SHOCK {
        let t = clamp(age / roll_of(s), 0.0, 1.0);
        let over = max(age - roll_of(s), 0.0);
        return s.radius * (1.0 / 12.0 + 11.0 / 12.0 * sqrt(t)) + s.speed * 0.3 * over / (1.0 + over);
    }
    let over = clamp(age - roll_of(s), 0.0, SURGE_FX_SPENT);
    return min(age * s.speed, s.radius) + s.speed * over * (0.5 - 0.25 * over / SURGE_FX_SPENT);
}


// The corner of quad `quad` that vertex `corner` (0..6) of it stands on, as steps out
// (`x`) and round (`y`).
fn grid_corner(quad: u32, corner: u32, around: u32) -> vec2<u32> {
    var di = 0u;
    var dj = 0u;
    switch corner {
        case 1u, 3u: {
            dj = 1u;
        }
        case 2u, 5u: {
            di = 1u;
        }
        case 4u: {
            di = 1u;
            dj = 1u;
        }
        default: {}
    }
    return vec2<u32>(quad / around + di, quad % around + dj);
}

// The swell's height `s` across its section, 0 at its back foot to 1 at its face's: a
// long low rise up its back to the crest, a short fall down its face.
fn swell(s: f32) -> f32 {
    let up = sin(0.5 * PI * clamp(s / CREST, 0.0, 1.0));
    let down = cos(0.5 * PI * clamp((s - CREST) / (1.0 - CREST), 0.0, 1.0));
    return select(down, up * up, s < CREST);
}

@vertex
fn vs_surge_front(@builtin(vertex_index) v: u32, @builtin(instance_index) instance: u32) -> SurgeOut {
    let s = surges[instance];
    let age = age_of(s);
    let front = front_of(s);
    if age < 0.0 || age > roll_of(s) + SURGE_FX_SPENT || front < 2.0 {
        return hidden();
    }
    let g = grid_corner(v / 6u, v % 6u, SURGE_FX_SEGMENTS);
    let across = f32(g.x) / f32(SURGE_FX_SECTION);
    let round = f32(g.y) / f32(SURGE_FX_SEGMENTS);
    let a = round * 2.0 * PI;
    let out_xy = vec2<f32>(cos(a), sin(a));
    // Close in, the swell is short: it grows to its full length over its first 40 m.
    let grown = smoothstep(0.0, 40.0, front);
    let back = BACK * (0.3 + 0.7 * grown);
    let behind = front - back * (1.0 - across / CREST);
    let ahead = front + FACE * (across - CREST) / (1.0 - CREST);
    let r = max(select(ahead, behind, across < CREST), 0.0);
    let xy = s.centre.xy + out_xy * r;
    let tall = (1.2 + 0.7 * s.power) * (0.5 + 0.5 * grown);
    let h = tall * swell(across);
    let p = vec3<f32>(xy, ground_at(xy) + 0.15 + h);
    var out: SurgeOut;
    out.surge = instance;
    // Leaning back over its long back, out over its face.
    let lean = select(-0.6, tall / back, across < CREST);
    out.normal = normalize(vec3<f32>(out_xy * lean, 1.0));
    out.world = p;
    out.at = vec2<f32>(round, across);
    out.clip = globals.view_proj * vec4<f32>(p, 1.0);
    return out;
}

@vertex
fn vs_surge_ground(@builtin(vertex_index) v: u32, @builtin(instance_index) instance: u32) -> SurgeOut {
    let s = surges[instance];
    let age = age_of(s);
    let front = min(age * s.speed, s.radius);
    if age < 0.0 || age > roll_of(s) + s.linger || front < 2.0 {
        return hidden();
    }
    let g = grid_corner(v / 6u, v % 6u, SURGE_FX_SEGMENTS);
    let u = f32(g.x) / f32(SURGE_FX_RINGS);
    let round = f32(g.y) / f32(SURGE_FX_SEGMENTS);
    let a = round * 2.0 * PI;
    // Rows even over the disc's area: as many out at its rim as its middle has room for.
    let r = front * sqrt(u);
    let xy = s.centre.xy + vec2<f32>(cos(a), sin(a)) * r;
    let p = vec3<f32>(xy, ground_at(xy) + 0.3);
    var out: SurgeOut;
    out.surge = instance;
    out.normal = vec3<f32>(0.0, 0.0, 1.0);
    out.world = p;
    out.at = vec2<f32>(round, u);
    out.clip = globals.view_proj * vec4<f32>(p, 1.0);
    return out;
}

// Cracks of a cellular pattern `cell` metres across: 1 on a crack, falling to 0 between,
// softened where they would be finer than a pixel, so far off they glow rather than
// sparkle. Its seed sets one surge's cracks apart from another's.
fn cracks(xy: vec2<f32>, cell: f32, seed: f32) -> f32 {
    let p = xy / cell + vec2<f32>(seed, seed * 1.7);
    let i = floor(p);
    let f = p - i;
    var d1 = 8.0;
    var d2 = 8.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let o = vec2<f32>(f32(x), f32(y));
            let c = i + o;
            let jitter = vec2<f32>(hash21(c), hash21(c + vec2<f32>(17.3, 5.1)));
            let d = length(o + jitter - f);
            if d < d1 {
                d2 = d1;
                d1 = d;
            } else if d < d2 {
                d2 = d;
            }
        }
    }
    let gap = d2 - d1;
    let width = max(0.07, fwidth(gap) * 1.5);
    let line = 1.0 - smoothstep(0.0, width, gap);
    let fine = clamp(length(fwidth(p)) * 2.0, 0.0, 1.0);
    return mix(line, 0.05, fine);
}

// How much it is seen edge on, 0 square on to 1 edge on.
fn edge_of(in: SurgeOut) -> f32 {
    let view = normalize(globals.camera.xyz - in.world);
    return 1.0 - abs(dot(normalize(in.normal), view));
}

fn fs_surge_front_lit(in: SurgeOut) -> vec4<f32> {
    let s = surges[in.surge];
    let age = age_of(s);
    let across = in.at.y;
    // Spent, it thins away from its back forward; it comes up in a blink.
    let spent = clamp((age - roll_of(s)) / SURGE_FX_SPENT, 0.0, 1.0);
    let fade = smoothstep(0.0, 0.05, age) * (1.0 - spent);
    // Light running round the ring in lumps, drawn out along it, finer the wider it is.
    let span = max(8.0, 2.0 * PI * front_of(s) / 9.0);
    let warp = value_noise2(vec2<f32>(in.at.x * span * 0.4, age * 0.8) + s.seed, 1.0);
    let lumps = value_noise2(vec2<f32>(in.at.x * span + warp * 2.0, across * 2.0 - age * 1.5) + s.seed * 1.3, 1.0);
    // A thin crest line, and a brighter line along its foot ahead.
    let crest = exp(-pow((across - CREST) / 0.05, 2.0));
    let foot = exp(-pow((across - 1.0) / 0.04, 2.0));
    // Its back a faint haze of bent violet air, thickest just under the crest.
    let body = smoothstep(0.0, CREST, across) * (1.0 - smoothstep(CREST, 1.0, across));
    let edge = edge_of(in);
    let hot = 0.6 + 0.6 * smoothstep(0.45, 0.8, lumps);
    let light = (mix(ROSE, WHITE, 0.5 * crest) * crest * 1.4 + ROSE * foot * 0.9) * hot
        + VIOLET * body * (0.08 + 0.35 * pow(edge, 2.0)) * (0.5 + lumps);
    let cover = clamp(0.06 * body + 0.1 * crest, 0.0, 0.2);
    // Its back is eaten away first as it thins.
    let alive = smoothstep(-0.05, 0.05, across + 0.3 * lumps - spent * 1.2);
    return vec4<f32>(light * s.power, cover) * alive * fade;
}

fn fs_surge_ground_lit(in: SurgeOut) -> vec4<f32> {
    let s = surges[in.surge];
    let age = age_of(s);
    let d = in.world.xy - s.centre.xy;
    let r = length(d);
    // Seconds since the front passed here; nothing ahead of it (masked at the end, not
    // discarded: the cracks take derivatives).
    let reached = age - r / max(s.speed, 1.0);
    let passed = max(reached, 0.0);
    let total = roll_of(s) + s.linger;
    let dying = 1.0 - smoothstep(total - SURGE_FX_FADE, total, age);
    let fresh = exp(-passed / SURGE_FX_FRESH);
    // Cracks wandering over the ground, coarse and fine, bent so they are not cells.
    let bend = vec2<f32>(value_noise2(in.world.xy + s.seed * 7.0, 9.0),
        value_noise2(in.world.xy + s.seed * 3.0 + 40.0, 9.0)) * 4.0;
    let coarse = cracks(in.world.xy + bend, CRACK, s.seed);
    let fine = cracks(in.world.xy + bend * 0.6, CRACK_FINE, s.seed + 3.1);
    // Only some of them open: patches of cracked ground between quiet ones, and each crack
    // broken along its length, so it wanders and stops rather than walling cells in.
    let patches = smoothstep(0.42, 0.72, value_noise2(in.world.xy + s.seed * 5.0, 22.0));
    let runs = smoothstep(0.35, 0.6, value_noise2(in.world.xy + s.seed * 9.0, 3.5));
    let veins = (coarse * runs + 0.4 * fine * smoothstep(0.5, 0.75, runs)) * patches;
    // Each patch pulses slowly, out of step with its neighbours.
    let phase = value_noise2(in.world.xy + s.seed * 11.0, 14.0) * 2.0 * PI;
    let pulse = 0.6 + 0.4 * sin(age * 2.1 + phase);
    // Rings of violet running out across it behind the front, the swell's aftershocks.
    let shock_r = r - (age * s.speed * SHOCK_PACE);
    // Broken up round the ring, so they read as a ripple through the cracks, not a target.
    let torn = smoothstep(0.35, 0.75, value_noise2(vec2<f32>(atan2(d.y, d.x) * 12.0, shock_r * 0.05) + s.seed, 1.0));
    let shock = pow(0.5 + 0.5 * cos(shock_r / SHOCK_GAP * 2.0 * PI), 10.0)
        * smoothstep(0.0, 0.6, passed) * torn;
    // Fresh, the cracks white and rose with a wash of heat between them; settled, crimson
    // embers; at the rim, softened so the disc's edge does not show.
    let rim = 1.0 - smoothstep(s.radius * 0.88, s.radius, r);
    let ember = mix(EMBER, CRIMSON, pulse) * veins * 0.9;
    let hot = mix(CRIMSON, mix(ROSE, WHITE, fresh * fresh), fresh) * (veins * 1.3 + 0.02) * fresh;
    let light = (ember + hot + VIOLET * shock * veins * 0.8) * rim;
    let cover = 0.16 * (1.0 - 0.5 * fresh) * rim;
    return vec4<f32>(light, cover) * dying * step(0.0, reached);
}

// The two, seen through the water from under it (bindings.wgsl `under_sea_seen`).
@fragment
fn fs_surge_front(in: SurgeOut) -> @location(0) vec4<f32> {
    return under_sea_seen(fs_surge_front_lit(in), in.clip);
}

@fragment
fn fs_surge_ground(in: SurgeOut) -> @location(0) vec4<f32> {
    return under_sea_seen(fs_surge_ground_lit(in), in.clip);
}

// ---- The Edict (renderer/nuke_fx/edict.rs) ------------------------------------------
// Its records on finer meshes (`SURGE_FX_EDICT_*`), each by its `look`:
//
// - **The lens** (`EDICT_STRIKE`): pinched plasma pressed flat over the mark where the
//   column lands, a low lobed dome far wider than tall, white-hot and boiling in cells,
//   darker and rose toward its limb with the prism's warm colours (never its lavender);
//   as it cools it goes rose, then red, and tears apart into a web of filaments that go
//   out last, hard-edged, never a haze.
// - **The shock** (`EDICT_SHOCK`): a ragged wall of dark driven dust with a broken line of
//   light along its foot, white for its first moments, then red, racing out fast at first
//   and slowing (the blast's front), thinning as it passes the radius.
// - **The molten wave** (`EDICT_WAVE`): a low crest of melt a few metres high rolling out
//   at the sim's pace, dark crust breaking over a glowing body, white-rose close in and
//   red toward the rim; behind it, on land only, the ground burning white and rose just
//   behind the front, then crimson veins and small pools that pulse, dimmer toward the
//   rim, dying down when the sim's wave does (the scar under it, scars.wgsl
//   `edict_scar`, keeps cooling for minutes).

const EDICT_WHITE: vec3<f32> = vec3<f32>(1.0, 0.95, 0.96);
const EDICT_ROSE: vec3<f32> = vec3<f32>(1.0, 0.36, 0.38);
const EDICT_RED: vec3<f32> = vec3<f32>(0.85, 0.035, 0.04);
const EDICT_EMBER: vec3<f32> = vec3<f32>(0.32, 0.008, 0.015);
const EDICT_DUST: vec3<f32> = vec3<f32>(0.035, 0.028, 0.026);
// The molten crest: metres of back behind it and of face ahead, its height at the mark,
// and where along its section it stands.
const WAVE_BACK: f32 = 70.0;
const WAVE_FACE: f32 = 9.0;
const WAVE_TALL: f32 = 6.0;
const WAVE_CREST: f32 = 0.84;
// The shock's wall: metres of back and of face, its height as it sets out.
const SHOCK_BACK: f32 = 40.0;
const SHOCK_FACE: f32 = 4.0;
const SHOCK_TALL: f32 = 7.0;
const SHOCK_CREST: f32 = 0.8;
// The lens: its height against its radius as it lands, and once pressed flat; metres
// across its boiling cells.
const LENS_TALL: f32 = 0.3;
const LENS_FLAT: f32 = 0.06;
const LENS_CELL: f32 = 24.0;
// The molten ground: its cracks, coarse and fine, and its pools, metres across; seconds
// a stretch just passed stays white-hot.
const MELT_CRACK: f32 = 38.0;
const MELT_FINE: f32 = 12.0;
const MELT_POOL: f32 = 22.0;
const MELT_FRESH: f32 = 0.6;

// How hot it burns `r` out from the mark: a tenth of it at the rim (the sim's
// `EdictWave::edge`, near enough).
fn edict_heat(s: SurgeWake, r: f32) -> f32 {
    return 1.0 - 0.85 * clamp(r / s.radius, 0.0, 1.0);
}

// The prism's warm stops only (common.wgsl `prism` without its lavender): rose, pink,
// peach-gold.
fn edict_prism(t: f32) -> vec3<f32> {
    var stops = array<vec3<f32>, 3>(
        vec3<f32>(1.0, 0.42, 0.62),
        vec3<f32>(1.0, 0.68, 0.82),
        vec3<f32>(1.0, 0.76, 0.54),
    );
    let x = fract(t) * 3.0;
    let i = min(u32(x), 2u);
    let f = smoothstep(0.0, 1.0, x - f32(i));
    return mix(stops[i], stops[(i + 1u) % 3u], f);
}

// A cellular pattern one unit across at `p`: x how far from the nearest border (F2 - F1),
// y the nearest cell's own random number. No derivatives.
fn edict_cells(p: vec2<f32>) -> vec2<f32> {
    let i = floor(p);
    let f = p - i;
    var d1 = 8.0;
    var d2 = 8.0;
    var id = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let o = vec2<f32>(f32(x), f32(y));
            let c = i + o;
            let jitter = vec2<f32>(hash21(c), hash21(c + vec2<f32>(17.3, 5.1)));
            let d = length(o + jitter - f);
            if d < d1 {
                d2 = d1;
                d1 = d;
                id = hash21(c + 7.3);
            } else if d < d2 {
                d2 = d;
            }
        }
    }
    return vec2<f32>(d2 - d1, id);
}

// A crack line along the borders of `cells`, `width` of a cell wide, softened where a
// cell is under a few pixels (`pc`: cells a pixel spans) to its average.
fn edict_crack(gap: f32, width: f32, pc: f32) -> f32 {
    let aa = max(width, pc * 1.5);
    let line = (1.0 - smoothstep(0.0, aa, gap)) * (width / aa);
    return mix(line, width * 1.6, smoothstep(0.25, 0.7, pc));
}

// The lens's radius `age` seconds after the strike: out to its radius in a blink, then
// spreading a little further as it is pressed flat.
fn lens_radius(s: SurgeWake, age: f32) -> f32 {
    let grow = 1.0 - exp(-age / max(roll_of(s) * 0.3, 0.01));
    return s.radius * grow * (1.0 + 0.2 * smoothstep(roll_of(s), roll_of(s) + s.linger, age));
}

fn vs_edict_lens(s: SurgeWake, g: vec2<u32>, instance: u32) -> SurgeOut {
    let age = age_of(s);
    if age < 0.0 || age > roll_of(s) + s.linger {
        return hidden();
    }
    let lat = f32(g.x) / f32(SURGE_FX_EDICT_SECTION);
    let round = f32(g.y) / f32(SURGE_FX_EDICT_SEGMENTS);
    let a = round * 2.0 * PI;
    let out_xy = vec2<f32>(cos(a), sin(a));
    // Lobed, never a circle: broad lobes and narrower tongues, shifting as it spreads.
    let lobes = value_noise2(out_xy * 2.2 + vec2<f32>(s.seed, age * 0.3), 1.0) * 0.7
        + value_noise2(out_xy * 7.0 - vec2<f32>(age * 0.6, s.seed), 1.0) * 0.3;
    let reach = max(lens_radius(s, age), 1.0) * (0.8 + 0.4 * lobes);
    let tall = reach * mix(LENS_TALL, LENS_FLAT, smoothstep(0.15, 1.6, age));
    let up = lat * 0.5 * PI;
    let rise = pow(sin(up), 0.7);
    let xy = s.centre.xy + out_xy * reach * cos(up);
    let p = vec3<f32>(xy, ground_at(xy) + tall * rise);
    var out: SurgeOut;
    out.surge = instance;
    out.normal = normalize(vec3<f32>(out_xy * cos(up) / reach, max(sin(up), 0.05) / tall));
    out.world = p;
    out.at = vec2<f32>(round, lat);
    out.clip = globals.view_proj * vec4<f32>(p, 1.0);
    return out;
}

// The wave's crest and the shock's wall: a ring standing where the front stands, its
// section from its back foot (0) over its crest to its face's foot (1).
fn vs_edict_ring(s: SurgeWake, g: vec2<u32>, instance: u32) -> SurgeOut {
    let age = age_of(s);
    let front = front_of(s);
    let shock = s.look == SURGE_FX_EDICT_SHOCK;
    let alive = select(roll_of(s) + SURGE_FX_SPENT * 2.0, roll_of(s) + s.linger, shock);
    if age < 0.0 || age > alive || front < 2.0 {
        return hidden();
    }
    let across = f32(g.x) / f32(SURGE_FX_EDICT_SECTION);
    let round = f32(g.y) / f32(SURGE_FX_EDICT_SEGMENTS);
    let a = round * 2.0 * PI;
    let out_xy = vec2<f32>(cos(a), sin(a));
    let crest = select(WAVE_CREST, SHOCK_CREST, shock);
    let grown = smoothstep(0.0, 200.0, front);
    let back = select(WAVE_BACK, SHOCK_BACK, shock) * (0.3 + 0.7 * grown);
    let face = select(WAVE_FACE, SHOCK_FACE, shock);
    let behind = front - back * (1.0 - across / crest);
    let ahead = front + face * (across - crest) / (1.0 - crest);
    let r = max(select(ahead, behind, across < crest), 0.0);
    let xy = s.centre.xy + out_xy * r;
    let out_of = clamp(front / s.radius, 0.0, 1.4);
    // Ragged along its length: taller and lower stretches.
    let ragged = 0.7 + 0.6 * value_noise2(out_xy * 5.0 + vec2<f32>(s.seed, 0.0), 1.0);
    let tall = select(WAVE_TALL * (0.45 + 0.55 * edict_heat(s, front)) * (0.5 + 0.5 * grown),
        SHOCK_TALL * (1.0 - 0.65 * min(out_of, 1.0)) * (0.4 + 0.6 * grown), shock)
        * s.power * ragged;
    let up = sin(0.5 * PI * clamp(across / crest, 0.0, 1.0));
    let down = cos(0.5 * PI * clamp((across - crest) / (1.0 - crest), 0.0, 1.0));
    let h = tall * select(down, up * up, across < crest);
    let p = vec3<f32>(xy, ground_at(xy) + 0.3 + h);
    var out: SurgeOut;
    out.surge = instance;
    let lean = select(-tall / face, tall / back, across < crest);
    out.normal = normalize(vec3<f32>(out_xy * lean, 1.0));
    out.world = p;
    out.at = vec2<f32>(round, across);
    out.clip = globals.view_proj * vec4<f32>(p, 1.0);
    return out;
}

@vertex
fn vs_edict_front(@builtin(vertex_index) v: u32, @builtin(instance_index) instance: u32) -> SurgeOut {
    let s = surges[instance];
    let g = grid_corner(v / 6u, v % 6u, SURGE_FX_EDICT_SEGMENTS);
    if s.look == SURGE_FX_EDICT_STRIKE {
        return vs_edict_lens(s, g, instance);
    }
    return vs_edict_ring(s, g, instance);
}

@vertex
fn vs_edict_ground(@builtin(vertex_index) v: u32, @builtin(instance_index) instance: u32) -> SurgeOut {
    let s = surges[instance];
    let age = age_of(s);
    let front = min(age * s.speed, s.radius);
    if s.look != SURGE_FX_EDICT_WAVE || age < 0.0 || age > roll_of(s) + s.linger || front < 2.0 {
        return hidden();
    }
    let g = grid_corner(v / 6u, v % 6u, SURGE_FX_EDICT_SEGMENTS);
    let u = f32(g.x) / f32(SURGE_FX_EDICT_RINGS);
    let round = f32(g.y) / f32(SURGE_FX_EDICT_SEGMENTS);
    let a = round * 2.0 * PI;
    let r = front * sqrt(u);
    let xy = s.centre.xy + vec2<f32>(cos(a), sin(a)) * r;
    // Its rows lie tens of metres apart: lifted with the distance so the ground's bumps
    // between them never break through it.
    let ground = ground_at(xy);
    let lift = 0.8 + 0.0016 * distance(globals.camera.xyz, vec3<f32>(xy, ground));
    let p = vec3<f32>(xy, ground + lift);
    var out: SurgeOut;
    out.surge = instance;
    out.normal = vec3<f32>(0.0, 0.0, 1.0);
    out.world = p;
    out.at = vec2<f32>(round, u);
    out.clip = globals.view_proj * vec4<f32>(p, 1.0);
    return out;
}

fn edict_lens_lit(in: SurgeOut, s: SurgeWake) -> vec4<f32> {
    let age = age_of(s);
    let life = roll_of(s) + s.linger;
    let cool = clamp(age / life, 0.0, 1.0);
    let lat = in.at.y;
    let edge = edge_of(in);
    // Boiling in fine cells drifting out from the mark, churning fast at first.
    let churn = age * 1.2 + 1.5 * (1.0 - exp(-age * 3.0));
    let q = (in.world.xy - s.centre.xy) / LENS_CELL;
    let cells = edict_cells(q * (1.0 - 0.15 * churn / (1.0 + churn)) + vec2<f32>(s.seed, churn * 0.4));
    let lumps = value_noise2(in.world.xy + vec2<f32>(s.seed * 13.0, churn * 30.0), 70.0) * 0.6
        + value_noise2(in.world.xy * 1.9 - vec2<f32>(churn * 40.0, s.seed * 7.0), 70.0) * 0.4;
    let grain = smoothstep(0.0, 0.3, cells.x);
    // White-hot; toward its limb darker and rose, with the prism's warm colours; then
    // rose, then red as it cools.
    let limb = pow(edge, 1.6);
    let hue = lumps * 1.3 + in.at.x * 3.0 + age * PRISM_RATE;
    var c = mix(EDICT_WHITE, edict_prism(hue), limb * 0.8);
    // Its heart stays white longest.
    c = mix(c, EDICT_ROSE, smoothstep(0.25, 0.55, cool) * (0.35 + 0.65 * limb));
    c = mix(c, EDICT_RED, smoothstep(0.45, 0.75, cool) * (0.5 + 0.5 * limb));
    let bright = (50.0 * exp(-age / 0.2) + 10.0 * exp(-age / 0.9) + 1.2)
        * mix(1.0, 0.4, limb) * (0.85 + 0.15 * grain) * (0.75 + 0.5 * lumps);
    // Torn apart as it cools, from its crown and its rim in: its body goes first, a web
    // of filaments along the cells' borders last.
    let left = 1.0 - cool * (1.7 + 0.6 * lat) + 0.4 * (lumps - 0.5);
    let body = smoothstep(-0.02, 0.02, left - 0.3);
    let web = (1.0 - smoothstep(0.02, 0.06, cells.x)) * smoothstep(-0.15, 0.0, left);
    let alive = max(body, web);
    let cover = (0.4 - 0.3 * cool) * body;
    return vec4<f32>(c * bright * alive, cover);
}

fn edict_ring_lit(in: SurgeOut, s: SurgeWake) -> vec4<f32> {
    let age = age_of(s);
    let across = in.at.y;
    let shock = s.look == SURGE_FX_EDICT_SHOCK;
    let front = front_of(s);
    let heat = edict_heat(s, front);
    let crest_at = select(WAVE_CREST, SHOCK_CREST, shock);
    let span = 2.0 * PI * front / 40.0;
    let warp = value_noise2(vec2<f32>(in.at.x * span * 0.3, age * 0.7) + s.seed, 1.0);
    let lumps = value_noise2(vec2<f32>(in.at.x * span + warp * 2.0, across * 3.0 - age * 1.2) + s.seed * 1.3, 1.0);
    let crest = exp(-pow((across - crest_at) / 0.05, 2.0));
    let foot = exp(-pow((across - 1.0) / 0.05, 2.0));
    let body = smoothstep(0.0, crest_at, across) * (1.0 - smoothstep(crest_at, 1.0, across));
    if shock {
        // White in its first moments, then rose, then a dull red as it runs out.
        let tone = mix(mix(EDICT_WHITE, EDICT_ROSE, smoothstep(0.05, 0.35, age)), EDICT_RED,
            smoothstep(0.35, 1.2, age));
        let spent = clamp((age - roll_of(s)) / max(s.linger, 0.1), 0.0, 1.0);
        let fade = smoothstep(0.0, 0.04, age) * (1.0 - spent);
        // Torn along its length into stretches tens of metres long, so it is a ragged
        // front of light skimming the ground, not a drawn circle.
        let along = in.at.x * 2.0 * PI * front / 45.0;
        let broken = smoothstep(0.35, 0.65, value_noise2(vec2<f32>(along, age * 2.0) + s.seed * 3.0, 1.0));
        let glow = (foot + 0.5 * body) * (0.4 + 0.6 * broken) * (0.7 + 0.6 * lumps)
            * (12.0 * exp(-age / 0.25) + 0.7);
        let alive = smoothstep(-0.05, 0.05, across + 0.4 * lumps - spent * 1.3);
        return vec4<f32>(tone * glow, 0.0) * alive * fade;
    }
    let spent = clamp((age - roll_of(s)) / (SURGE_FX_SPENT * 2.0), 0.0, 1.0);
    let fade = smoothstep(0.0, 0.1, age) * (1.0 - spent);
    // Its crust: dark plates riding the crest, the melt glowing between them.
    let crust = smoothstep(0.4, 0.65, value_noise2(in.world.xy + s.seed * 17.0, 12.0));
    let hot_c = mix(EDICT_RED, mix(EDICT_ROSE, EDICT_WHITE, heat * heat), heat);
    let light = (hot_c * (crest * 1.8 + foot) * (0.5 + 0.9 * lumps) * (1.0 - 0.6 * crust)
        + EDICT_RED * body * (1.0 - crust) * (0.4 + heat)) * (1.2 + 2.5 * heat) * s.power;
    let cover = clamp(0.5 * body * (0.4 + 0.6 * crust) + 0.15 * crest, 0.0, 0.6);
    let alive = smoothstep(-0.05, 0.05, across + 0.3 * lumps - spent * 1.2);
    return vec4<f32>(light + EDICT_DUST * cover, cover) * alive * fade;
}

fn edict_ground_lit(in: SurgeOut, s: SurgeWake, px: f32) -> vec4<f32> {
    let age = age_of(s);
    let xy = in.world.xy;
    let d = xy - s.centre.xy;
    let r = length(d);
    let reached = age - r / max(s.speed, 1.0);
    let passed = max(reached, 0.0);
    let total = roll_of(s) + s.linger;
    let dying = 1.0 - smoothstep(total - SURGE_FX_FADE * 1.5, total, age);
    let fresh = exp(-passed / MELT_FRESH);
    let heat = edict_heat(s, r);
    // The sea does not melt: it only takes the front's flash.
    let land = smoothstep(-0.5, 0.5, terrain_height(xy) - globals.map.z);
    let bend = vec2<f32>(value_noise2(xy + s.seed * 7.0, 50.0), value_noise2(xy + s.seed * 3.0 + 40.0, 50.0)) * 22.0;
    let coarse = edict_cells((xy + bend) / MELT_CRACK + s.seed);
    let fine = edict_cells((xy + bend * 0.6) / MELT_FINE + s.seed * 1.7);
    let gape = value_noise2(xy * 1.3 + s.seed * 4.0, 45.0);
    let crack = edict_crack(coarse.x, 0.03 + 0.09 * gape * gape, px / MELT_CRACK);
    let hair = edict_crack(fine.x, 0.05, px / MELT_FINE) * smoothstep(0.45, 0.8, gape);
    // Cracked over most of it near the mark, in patches toward the rim; each crack broken
    // along its length so it wanders and stops.
    let patches = 0.35 + 0.65 * smoothstep(0.3, 0.75, value_noise2(xy + s.seed * 5.0, 160.0) + 0.3 * heat);
    let runs = smoothstep(0.25, 0.55, value_noise2(xy + s.seed * 9.0, 24.0));
    let veins = clamp(crack * (0.4 + 0.6 * runs) + 0.3 * hair * heat, 0.0, 1.0) * patches;
    // Small pools of melt in the hollows, a crust skinning them; nearer the mark more.
    let pool = smoothstep(0.78 - 0.05 * heat, 0.82 - 0.05 * heat,
        value_noise2(xy + bend * 1.5 + s.seed * 13.0, MELT_POOL)) * (0.2 + 0.4 * heat);
    let skin = smoothstep(0.5, 0.8, value_noise2(xy * 2.0 + s.seed * 19.0 + age * 3.0, 9.0));
    let phase = value_noise2(xy + s.seed * 11.0, 70.0) * 2.0 * PI;
    let pulse = 0.55 + 0.45 * sin(age * 1.7 + phase);
    let rim = 1.0 - smoothstep(s.radius * 0.9, s.radius, r);
    let glowing = veins + pool * (1.0 - 0.6 * skin);
    let ember = mix(EDICT_EMBER, EDICT_RED, pulse) * glowing * (0.3 + 1.2 * heat);
    // Just behind the front: the cracks white and rose, a narrow band of heat between
    // them; red within a second.
    let tone = mix(EDICT_RED, mix(EDICT_ROSE, EDICT_WHITE, smoothstep(0.6, 1.0, fresh)), smoothstep(0.3, 0.9, fresh));
    let hot = tone * (glowing * 2.5 + 0.12 * smoothstep(0.7, 1.0, fresh)) * fresh * (0.3 + heat) * 2.0;
    // The ground between darkened, as the melt's crust.
    let cover = (0.3 + 0.25 * fresh) * rim * land;
    let light = (ember + hot * 0.7) * rim * mix(0.12, 1.0, land);
    return vec4<f32>(light, cover) * dying * step(0.0, reached);
}

@fragment
fn fs_edict_front(in: SurgeOut) -> @location(0) vec4<f32> {
    let s = surges[in.surge];
    if s.look == SURGE_FX_EDICT_STRIKE {
        return under_sea_seen(edict_lens_lit(in, s), in.clip);
    }
    return under_sea_seen(edict_ring_lit(in, s), in.clip);
}

@fragment
fn fs_edict_ground(in: SurgeOut) -> @location(0) vec4<f32> {
    // Worked out before anything branches on the record.
    let px = max(length(fwidth(in.world.xy)), 0.01);
    let s = surges[in.surge];
    return under_sea_seen(edict_ground_lit(in, s, px), in.clip);
}
