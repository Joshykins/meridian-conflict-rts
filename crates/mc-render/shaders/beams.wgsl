//!use bindings
// Work beams. The sim lists who is at work each tick; everything seen here is
// made from that on the GPU. Reclaim (kind 0): a cone that grips the target
// and narrows into the emitter, torn-off bits streaming back up it, heating
// from red through the Materials red-orange to white. Repair (kind 2): the inverse — mint-green
// patches leave the emitter and settle onto the hull. `BEAM_NANITE`, a Regency builder's
// nanite stream, and `BEAM_NANITE_SITE`, the site it feeds: below. Premultiplied:
// hot cores only add light; the coloured body also covers what is behind it, or
// over grass it would wash out.

// Mirrors the renderer's GpuBeam (renderer/work_beams.rs): mc_sim::reclaim::BeamInstance,
// where the emitter was a tick ago, the beam's own seed, and when it came on and went off.
struct Beam {
    emitter: vec3<f32>,
    // 0 reclaim. 1 nanite stream. 2 repair. 3 relay. 4 replication ray, 5 print beam,
    // 6 nanite site. 7 retired.
    kind: u32,
    to_prev: vec3<f32>,
    radius: f32,
    to: vec3<f32>,
    height: f32,
    // The emitter a tick ago, moved back with its unit: a work beam's emitter goes from
    // here to `emitter` over the tick, as the unit it is on is drawn.
    from_prev: vec3<f32>,
    // The same every tick the beam is on, so what runs along it does not jump as it moves.
    seed: f32,
    // Seconds, on the clock of globals.camera.w. `end` is negative while the beam is on.
    start: f32,
    end: f32,
    // Its length when it came on: sets a bit's trip time, fixed so bits keep their pace
    // while the beam stretches or shrinks under a moving unit.
    trip_len: f32,
    pad: f32,
}

@group(1) @binding(1) var<storage, read> beams: array<Beam>;

// Quads per beam (`BEAM_QUADS`): the ribbon, the two glows, and the bits. The other kinds
// use the first `BEAM_FIXED_QUADS` of them, but for a nanite site, which takes them all.
// Bits on a work beam: this many at least, and one per this many metres on a long one.
const MIN_BITS: f32 = 29.0;
const BIT_SPACING: f32 = 22.0;
const SHAPE_RIBBON: f32 = 0.0;
const SHAPE_GLOW: f32 = 1.0;
const SHAPE_BIT: f32 = 2.0;

// Reclaim is Materials: the HUD's red-orange (`MASS_*`, gpu_consts.rs) for its body, a
// hot core just short of white, a deeper red at its edge. Never the construction amber.
const WHITE: vec3<f32> = vec3<f32>(1.0, 0.84, 0.74);
const MATERIALS: vec3<f32> = vec3<f32>(MASS_R, MASS_G, MASS_B);
const RED: vec3<f32> = vec3<f32>(0.85, 0.05, 0.02);
const MINT: vec3<f32> = vec3<f32>(0.78, 1.0, 0.88);
const TEAL: vec3<f32> = vec3<f32>(0.18, 0.82, 0.52);
const DEEP: vec3<f32> = vec3<f32>(0.04, 0.38, 0.26);

struct BeamOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    // x shape, y metres from the target along the beam (ribbon) or heat 0..1 (bit, glow), z half-width in metres (ribbon) or seed
    @location(1) state: vec3<f32>,
    @location(2) level: f32,
    @location(3) kind: f32,
    // Replication beams only (kinds 4, 5): see `replicator_vertex`.
    @location(4) extra: vec4<f32>,
    // Nanite strands only: see `strand_ribbon`.
    @location(5) strand: vec4<f32>,
}

fn hash(n: f32) -> f32 {
    return fract(sin(n * 12.9898) * 43758.547);
}

fn hidden() -> BeamOut {
    var out: BeamOut;
    out.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
    out.uv = vec2<f32>(0.0);
    out.state = vec3<f32>(0.0);
    out.level = 0.0;
    out.kind = 0.0;
    return out;
}

// A quad of `half_px` pixels round a world point, stretched `stretch` times along the screen direction `dir`.
fn billboard(world: vec3<f32>, corner: vec2<f32>, half_px: f32, dir: vec2<f32>, stretch: f32) -> vec4<f32> {
    let c = globals.view_proj * vec4<f32>(world, 1.0);
    let side = vec2<f32>(-dir.y, dir.x);
    let offset = (dir * corner.x * stretch + side * corner.y) * half_px * globals.viewport.zw;
    return vec4<f32>((c.xy / c.w + offset) * c.w, c.z, c.w);
}

@vertex
fn vs_beam(@location(0) corner: vec2<f32>, @builtin(instance_index) instance: u32) -> BeamOut {
    let b = beams[instance / BEAM_QUADS];
    let slot = instance % BEAM_QUADS;
    let time = globals.camera.w;
    if b.kind != 0u && b.kind != 2u && b.kind != 3u && b.kind != BEAM_NANITE_SITE && slot >= BEAM_FIXED_QUADS {
        return hidden();
    }
    if b.kind == BEAM_NANITE_SITE {
        return nanite_site_vertex(b, slot, corner);
    }
    if b.kind >= 4u {
        return replicator_vertex(b, slot, corner, instance);
    }
    if b.kind == BEAM_NANITE {
        return nanite_vertex(b, slot, corner);
    }
    // Both ends move with their units over the tick, at the alpha the units are drawn at.
    let foot = mix(b.to_prev, b.to, globals.sun.w);
    let emitter = mix(b.from_prev, b.emitter, globals.sun.w);
    let grip = foot + vec3<f32>(0.0, 0.0, b.height * 0.55);
    let span = emitter - grip;
    let len = max(length(span), 0.01);
    let axis = span / len;

    // Cut back to the part in front of the eye: with an end behind the camera the beam
    // would otherwise vanish whole as you scroll in over it.
    var a = globals.view_proj * vec4<f32>(grip, 1.0);
    var e = globals.view_proj * vec4<f32>(emitter, 1.0);
    if a.w < RAY_NEAR && e.w < RAY_NEAR {
        return hidden();
    }
    let a0 = a;
    let e0 = e;
    // How far along grip -> emitter each drawn end sits.
    var t_a = 0.0;
    var t_e = 1.0;
    if a0.w < RAY_NEAR {
        t_a = (RAY_NEAR - a0.w) / (e0.w - a0.w);
        a = mix(a0, e0, t_a);
    }
    if e0.w < RAY_NEAR {
        t_e = (RAY_NEAR - a0.w) / (e0.w - a0.w);
        e = mix(a0, e0, t_e);
    }
    var dir = (e.xy / e.w - a.xy / a.w) * globals.viewport.xy;
    let dir_len = length(dir);
    if dir_len > 0.001 {
        dir = dir / dir_len;
    } else {
        dir = vec2<f32>(0.0, 1.0);
    }
    let side = vec2<f32>(-dir.y, dir.x);
    var out: BeamOut;
    out.uv = corner;
    out.kind = f32(b.kind);
    let repair = b.kind == 2u;
    // Salvage riding home: reclaim-coloured bits only, no ribbon.
    let ferry = b.kind == 3u;
    // The light comes up quickly when the beam comes on and dies as quickly when it goes off.
    // What is on its way is not cut short: see the bits below.
    var power = smoothstep(0.0, 0.18, time - b.start);
    if b.end >= 0.0 {
        power = power * (1.0 - smoothstep(0.0, 0.25, time - b.end));
    }
    out.level = power;

    if ferry && slot <= 2u {
        return hidden();
    }
    if slot == 0u {
        // Reclaim: wide where it grips the target, narrow at the emitter.
        // Repair: a thinner tool, moderate on the hull, tight at the projector.
        let at_emitter = corner.x > 0.0;
        let half_m = select(
            select(
                select(clamp(b.radius * 0.45, 0.9, 5.0), clamp(b.radius * 0.22, 0.28, 0.7), ferry),
                clamp(b.radius * 0.16, 0.35, 1.6),
                repair
            ),
            select(select(0.55, 0.22, ferry), 0.28, repair),
            at_emitter
        );
        let p = select(a, e, at_emitter);
        let half_px = max(half_m * globals.lod.x / max(p.w, 1.0), 1.6);
        let ndc = p.xy / p.w + side * corner.y * half_px * globals.viewport.zw;
        out.clip = vec4<f32>(ndc * p.w, p.z, p.w);
        // Half-width as drawn, in metres: from far off the ribbon is kept a few pixels wide.
        out.state = vec3<f32>(SHAPE_RIBBON, len * select(t_a, t_e, at_emitter), half_px * max(p.w, 1.0) / globals.lod.x);
        return out;
    }
    if slot <= 2u {
        // Reclaim bites the hull and pours into the emitter. Repair lights the
        // projector and a scatter of patches where the bits land.
        let at_emitter = slot == 2u;
        let world = select(grip, emitter, at_emitter);
        let radius = select(
            select(
                select(clamp(b.radius * 1.15, 2.0, 12.0), clamp(b.radius * 0.7, 0.5, 1.4), ferry),
                clamp(b.radius * 0.55, 1.2, 5.0),
                repair
            ),
            select(select(1.5, 0.7, ferry), 2.2, repair),
            at_emitter
        );
        let w = select(a.w, e.w, at_emitter);
        if select(a0.w, e0.w, at_emitter) < RAY_NEAR {
            return hidden();
        }
        let flicker = 0.75 + 0.25 * sin(time * 31.0 + f32(instance)) * sin(time * 17.3);
        out.clip = billboard(world, corner, max(radius * globals.lod.x / max(w, 1.0), 2.5), vec2<f32>(1.0, 0.0), 1.0);
        out.state = vec3<f32>(SHAPE_GLOW, select(0.0, 1.0, at_emitter), 0.0);
        out.level = power * flicker * select(select(0.55, 0.7, repair), select(1.6, 1.35, repair), at_emitter) * select(1.0, 0.55, ferry);
        return out;
    }

    // A bit on the beam: as many as its length asks for, so a long beam is as busy along
    // its run as a short one. Seeded by the beam's own seed, which stays put while the
    // beam moves and when the list of beams changes.
    if f32(slot - 3u) >= clamp(len / BIT_SPACING, MIN_BITS, f32(BEAM_QUADS - 3u)) {
        return hidden();
    }
    let seed = f32(slot) * 7.31 + b.seed;
    // From the length the beam came on at, not `len`: `turns` is time / trip, so a trip
    // that changed as the unit moved would spin every bit's phase many times a second.
    let trip = clamp(b.trip_len / 42.0, 0.55, 3.2) * (0.8 + 0.5 * hash(seed + 1.0));
    let turns = time / trip + hash(seed + 2.0);
    let phase = fract(turns);
    // A bit exists only if it left while the beam was on: reclaim fills from the
    // target and empties into the emitter; repair fills from the emitter and
    // settles on the hull. Nothing pops in or out mid-air.
    let born = time - phase * trip;
    if born < b.start || (b.end >= 0.0 && born > b.end) {
        return hidden();
    }
    // Reclaim: slow to tear loose, then faster (0 at the hull, 1 at the emitter).
    // Repair: leaves fast, eases onto the plate (1 at the emitter, 0 at the hull).
    let along = select(pow(phase, 1.7), pow(1.0 - phase, 2.1), repair);
    var b1 = cross(axis, vec3<f32>(0.0, 0.0, 1.0));
    if dot(b1, b1) < 1e-4 {
        b1 = vec3<f32>(1.0, 0.0, 0.0);
    }
    b1 = normalize(b1);
    let b2 = cross(axis, b1);
    let swing = select(-1.0, 1.0, hash(seed + 3.0) > 0.5);
    let angle = hash(seed + 4.0) * 6.2832 + along * 3.4 * swing;
    let off = b.radius * (0.25 + 0.75 * hash(seed + 5.0)) * pow(1.0 - along, 1.4);
    let start = (hash(seed + 6.0) - 0.5) * b.radius;
    let world = grip + axis * (start + (len - start) * along) + (b1 * cos(angle) + b2 * sin(angle)) * off;
    let size = mix(1.3, 0.35, along) * (0.6 + 0.9 * hash(seed + 7.0)) * clamp(b.radius / 7.0, 0.7, 2.2) * select(1.0, 0.9, ferry);
    let c = globals.view_proj * vec4<f32>(world, 1.0);
    if c.w < RAY_NEAR {
        return hidden();
    }
    let half_px = max(size * globals.lod.x / max(c.w, 1.0), 2.0);
    // Drawn out along its flight the faster it goes.
    out.clip = billboard(world, corner, half_px, dir, 1.0 + along * 2.0);
    // Heat follows along: reclaim heats toward the emitter; repair cools as it seats.
    out.state = vec3<f32>(SHAPE_BIT, along, hash(seed + 8.0));
    out.level = smoothstep(0.0, 0.1, phase) * (1.0 - smoothstep(0.9, 1.0, phase)) * select(1.0, 0.85, ferry);
    return out;
}

// Pulses running along a work beam, 0.55..1: reclaim's toward the emitter, repair's toward
// the hull. 7 m apart up close; from farther off the spacing doubles, octave by octave
// (each fading into the next), so they stay about 24 pixels apart however long the beam.
fn beam_pulse(run: f32, run_px: f32, time: f32, repair: bool) -> f32 {
    let octave = max(log2(run_px * 24.0 / 7.0), 0.0);
    let o = floor(octave);
    let blend = octave - o;
    let dir = select(-1.0, 1.0, repair);
    // About two pulses a second pass any point, whatever their spacing.
    var level = 0.0;
    for (var k = 0; k < 2; k++) {
        let spacing = 7.0 * exp2(o + f32(k));
        let phase = run / spacing + dir * time * 2.2;
        let wave = 0.5 + 0.5 * sin(phase * 6.2831853);
        level += wave * select(1.0 - blend, blend, k == 1);
    }
    return 0.55 + 0.45 * level;
}

fn fs_beam_lit(in: BeamOut) -> vec4<f32> {
    let time = globals.camera.w;
    let run = in.state.y;
    // Kind 3 is the ferry home: reclaim colours, particles only.
    let repair = in.kind > 1.5 && in.kind < 2.5;
    // Sampled for every shape: a texture is read in uniform control flow.
    let n = textureSample(noise_map, repeat_sampler, vec2<f32>(run * 0.035 + time * 0.9, in.uv.y * 0.11 + time * 0.07)).b;
    // Metres along the beam per pixel, before any branch: pulses must stay wider than a
    // few pixels, or a long beam seen from far off breaks into dashes.
    let run_px = max(fwidth(run), 1e-3);
    // And across it (a ribbon's uv.y runs -1..1 over its drawn width).
    let across_px = fwidth(in.uv.y);
    if in.state.x > 8.5 {
        return nanite_fragment(in, n);
    }
    if in.state.x > 2.5 {
        return replicator_fragment(in, n);
    }
    if in.state.x < 0.5 {
        // Reclaim: hot core, Materials red-orange about it, red to the edge.
        // Repair: mint core, teal body, deep green edge — mass going back in.
        let half_m = max(in.state.z, 0.05);
        let y = abs(in.uv.y) * half_m;
        let pulse = beam_pulse(run, run_px, time, repair);
        // The hot core is 0.14 m across up close, and never under about two pixels, so a
        // far beam keeps a bright thread down its middle instead of breaking up.
        let core_px = across_px * half_m;
        let core_w = max(0.02, core_px * core_px);
        let core = exp(-y * y / core_w) * (0.8 + 0.4 * n);
        let body = exp(-y * y / (0.16 * half_m * half_m + 0.06)) * (0.45 + 0.8 * n) * pulse;
        let edge = pow(max(1.0 - abs(in.uv.y), 0.0), 1.5) * (0.35 + 0.65 * n);
        // Drawn only a few pixels wide, the core is most of the beam: it takes the body's
        // colour and its pulses, or a far beam reads as a white wire.
        let far = smoothstep(0.08, 0.5, across_px);
        let hot = mix(1.0, pulse, far);
        let color = select(
            mix(WHITE * 5.0, MATERIALS * 3.2 + WHITE * 0.4, far) * core * hot + MATERIALS * body * 1.5 + RED * edge * 0.8,
            mix(MINT * 4.4, TEAL * 3.0 + MINT * 0.4, far) * core * hot + TEAL * body * 1.45 + DEEP * edge * 0.95,
            repair
        );
        return vec4<f32>(color * in.level, clamp(body * 0.7 + edge * 0.55, 0.0, 0.85) * in.level);
    }
    if in.state.x < 1.5 {
        let d = length(in.uv);
        if d > 1.0 {
            discard;
        }
        let fall = pow(1.0 - d, 2.2);
        let hot = select(
            mix(mix(RED, MATERIALS, fall), WHITE, in.state.y * fall),
            mix(mix(DEEP, TEAL, fall), MINT, in.state.y * fall),
            repair
        );
        return vec4<f32>(hot * fall * 2.0 * in.level, clamp(fall * 0.5 * in.level, 0.0, 0.6));
    }
    // A shard, not a spark: a hard-edged, crooked plate that tumbles as it goes.
    let spin = in.state.z * 6.2832 + time * (2.5 + in.state.z * 6.0) * select(-1.0, 1.0, in.state.z > 0.5);
    let p = vec2<f32>(in.uv.x * cos(spin) - in.uv.y * sin(spin), in.uv.x * sin(spin) + in.uv.y * cos(spin));
    let skew = p.y + p.x * (in.state.z - 0.5) * 0.9;
    let d = max(abs(p.x) * (1.45 + in.state.z * 0.5), abs(skew) * 2.3);
    if d > 1.0 {
        discard;
    }
    let heat = in.state.y;
    let color = select(
        mix(mix(RED, MATERIALS, smoothstep(0.0, 0.45, heat)), WHITE, smoothstep(0.55, 1.0, heat)),
        mix(mix(DEEP, TEAL, smoothstep(0.0, 0.4, heat)), MINT, smoothstep(0.5, 1.0, heat)),
        repair
    );
    // Its torn edge runs hotter than its face.
    let solid = (1.0 - smoothstep(0.85, 1.0, d)) * (0.7 + 0.6 * smoothstep(0.45, 0.9, d));
    // Reclaim: dull red as it tears loose, white hot on the last of the way in.
    // Repair: white-hot as it leaves, cooling to teal as it seats.
    let glow = 0.85 + 0.5 * smoothstep(0.2, 0.6, heat) + 4.0 * smoothstep(0.75, 1.0, heat);
    return vec4<f32>(color * solid * glow * in.level, solid * in.level * 0.95);
}

// ---- Replication (Survival) ---------------------------------------------------------
// Kind 4, the replication ray: from the engine's crown to a node being raised, often
// across the whole map. `radius` is its core radius in metres, `height` how far the
// node is raised (0..1). A blinding white core in a cold blue sheath with filaments
// spiralling round it, pulses running out from the engine, a flare at either end, a
// splash of light on the ground at the site and motes of matter drawn up into it.
// Kind 5, the print beam: from a projector to a unit being printed; `radius` and
// `height` are the unit's. Two blue fans sweep the unit's volume (one up and down,
// one side to side) while packets of matter stream out and land all over it.
// Everything is built from its two end points, so detail does not depend on length;
// a far end behind the eye is pulled in to just in front of it.

const RAY_CATCH: f32 = 46.0;
const RAY_NEAR: f32 = 2.0;
// The Precursors' replication light: a cold blue with little green in it (colder and
// deeper than Aster's cyan emitters), an ice white, and a core just short of white.
const REP_BLUE: vec3<f32> = vec3<f32>(0.4, 0.64, 1.0);
const REP_ICE: vec3<f32> = vec3<f32>(0.74, 0.87, 1.0);
const HOT: vec3<f32> = vec3<f32>(0.94, 0.97, 1.0);
const SHAPE_RAY_SHEATH: f32 = 3.0;
const SHAPE_RAY_CORE: f32 = 4.0;
const SHAPE_FLARE: f32 = 5.0;
const SHAPE_SPLASH: f32 = 6.0;
const SHAPE_MOTE: f32 = 7.0;
const SHAPE_FAN: f32 = 8.0;

// The part of segment a-b in front of the eye (clip w over `RAY_NEAR`); x < 0 when none.
fn rep_clip(a: vec3<f32>, b: vec3<f32>) -> array<vec3<f32>, 3> {
    let wa = (globals.view_proj * vec4<f32>(a, 1.0)).w;
    let wb = (globals.view_proj * vec4<f32>(b, 1.0)).w;
    if wa < RAY_NEAR && wb < RAY_NEAR {
        return array<vec3<f32>, 3>(a, b, vec3<f32>(-1.0));
    }
    var p = a;
    var q = b;
    if wa < RAY_NEAR {
        p = mix(a, b, (RAY_NEAR - wa) / (wb - wa));
    }
    if wb < RAY_NEAR {
        q = mix(a, b, (wa - RAY_NEAR) / (wa - wb));
    }
    return array<vec3<f32>, 3>(p, q, vec3<f32>(1.0));
}

// A camera-facing ribbon from a to b (already in front of the eye), `half_m` metres
// wide, never under `min_px`. uv.x is 1 at a, -1 at b; state.y metres from `origin`.
fn rep_ribbon(a: vec3<f32>, b: vec3<f32>, origin: vec3<f32>, corner: vec2<f32>, half_m: f32, min_px: f32, shape: f32) -> BeamOut {
    var out: BeamOut;
    let ca = globals.view_proj * vec4<f32>(a, 1.0);
    let cb = globals.view_proj * vec4<f32>(b, 1.0);
    var dir = (cb.xy / cb.w - ca.xy / ca.w) * globals.viewport.xy;
    let dl = length(dir);
    dir = select(vec2<f32>(0.0, 1.0), dir / max(dl, 1e-4), dl > 0.001);
    let side = vec2<f32>(-dir.y, dir.x);
    let at_a = corner.x > 0.0;
    let c = select(cb, ca, at_a);
    let world = select(b, a, at_a);
    let half_px = max(half_m * globals.lod.x / max(c.w, 1.0), min_px);
    let ndc = c.xy / c.w + side * corner.y * half_px * globals.viewport.zw;
    out.clip = vec4<f32>(ndc * c.w, c.z, c.w);
    out.uv = corner;
    let drawn = half_px * max(c.w, 1.0) / globals.lod.x;
    out.state = vec3<f32>(shape, distance(world, origin), drawn);
    // x the true half-width in metres, y metres a pixel covers here.
    out.extra = vec4<f32>(half_m, max(c.w, 1.0) / globals.lod.x, 0.0, 0.0);
    return out;
}

// A billboard round `world`, pulled `pull` metres toward the eye so the model it sits on
// does not cut it.
fn rep_billboard(world: vec3<f32>, corner: vec2<f32>, size_m: f32, min_px: f32, pull: f32) -> vec4<f32> {
    let eye = globals.camera.xyz;
    let to_eye = eye - world;
    let d = length(to_eye);
    let p = world + to_eye / max(d, 1e-3) * min(pull, d * 0.5);
    let c = globals.view_proj * vec4<f32>(p, 1.0);
    let half_px = max(size_m * globals.lod.x / max(c.w, 1.0), min_px);
    return billboard(p, corner, half_px, vec2<f32>(1.0, 0.0), 1.0);
}

fn replicator_vertex(b: Beam, slot: u32, corner: vec2<f32>, instance: u32) -> BeamOut {
    let time = globals.camera.w;
    let foot = mix(b.to_prev, b.to, globals.sun.w);
    var out: BeamOut;
    out.kind = f32(b.kind);
    out.uv = corner;
    let ray = b.kind == 4u;
    // The ray charges up over half a second; the print head snaps on.
    var power = smoothstep(0.0, select(0.2, 0.5, ray), time - b.start);
    if b.end >= 0.0 {
        power = power * (1.0 - smoothstep(0.0, select(0.3, 0.6, ray), time - b.end));
    }
    out.level = power;
    if power <= 0.0 && slot < 4u {
        return hidden();
    }
    if ray {
        let r = max(b.radius, 1.0);
        let raise = clamp(b.height, 0.0, 1.0);
        let land = foot + vec3<f32>(0.0, 0.0, RAY_CATCH);
        let seg = rep_clip(b.emitter, land);
        let shown = seg[2].x > 0.0;
        // The node's last moments pull harder: the ray swells as it finishes.
        let swell = 1.0 + 0.35 * smoothstep(0.85, 1.0, raise);
        out.extra.z = raise;
        if slot == 0u {
            if !shown { return hidden(); }
            var o = rep_ribbon(seg[0], seg[1], b.emitter, corner, r * 3.4 * swell, 16.0, SHAPE_RAY_SHEATH);
            o.kind = out.kind; o.level = power; o.extra.z = raise;
            return o;
        }
        if slot == 1u {
            if !shown { return hidden(); }
            var o = rep_ribbon(seg[0], seg[1], b.emitter, corner, r * swell, 4.0, SHAPE_RAY_CORE);
            o.kind = out.kind; o.level = power; o.extra.z = raise;
            return o;
        }
        if slot == 2u || slot == 3u {
            let at_emitter = slot == 2u;
            let world = select(land, b.emitter, at_emitter);
            let size = select(r * 5.5, r * 7.0, at_emitter) * swell;
            out.clip = rep_billboard(world, corner, size, select(26.0, 40.0, at_emitter), size * 0.8);
            out.state = vec3<f32>(SHAPE_FLARE, select(1.0, 0.0, at_emitter), f32(instance) * 0.37);
            return out;
        }
        if slot == 4u {
            // Light splashed over the ground round the site: a flat quad under it.
            let half = 60.0 + r * 2.0;
            let world = foot + vec3<f32>(corner * half, 2.5);
            out.clip = globals.view_proj * vec4<f32>(world, 1.0);
            out.state = vec3<f32>(SHAPE_SPLASH, half, 0.0);
            return out;
        }
        let span = land - b.emitter;
        let len = max(length(span), 1.0);
        if slot < 13u {
            // Pulses running out from the engine to the site, well spaced, at a steady clip.
            let i = f32(slot - 5u);
            let speed = 1400.0;
            let s = fract(time * speed / len + i / 8.0);
            let born = time - s * len / speed;
            if born < b.start || (b.end >= 0.0 && born > b.end) {
                return hidden();
            }
            let world = b.emitter + span * s;
            let c = globals.view_proj * vec4<f32>(world, 1.0);
            if c.w < RAY_NEAR { return hidden(); }
            let half_px = max(r * 2.4 * globals.lod.x / c.w, 5.0);
            var dir = (globals.view_proj * vec4<f32>(land, 1.0)).xy;
            let e = globals.view_proj * vec4<f32>(b.emitter, 1.0);
            dir = dir / max((globals.view_proj * vec4<f32>(land, 1.0)).w, 0.01) - e.xy / max(e.w, 0.01);
            dir = dir * globals.viewport.xy;
            dir = select(vec2<f32>(1.0, 0.0), normalize(dir), length(dir) > 1e-3);
            out.clip = billboard(world, corner, half_px, dir, 3.0);
            out.state = vec3<f32>(SHAPE_MOTE, 1.0, 0.0);
            out.level = power * (0.6 + 0.4 * sin(s * 3.14159));
            return out;
        }
        // Motes of matter drawn up off the ground round the site into the ray.
        let seed = f32(slot) * 5.31 + b.to.x * 0.013 + b.to.y * 0.029;
        let trip = 2.2 + 1.6 * hash(seed + 1.0);
        let phase = fract(time / trip + hash(seed + 2.0));
        let born = time - phase * trip;
        if born < b.start || (b.end >= 0.0 && born > b.end) {
            return hidden();
        }
        let a0 = hash(seed + 3.0) * 6.2832 + phase * 2.4;
        let r0 = (14.0 + 40.0 * hash(seed + 4.0)) * (1.0 - pow(phase, 1.6));
        let z = mix(1.0, RAY_CATCH + 30.0 * hash(seed + 5.0), pow(phase, 0.8));
        let world = foot + vec3<f32>(cos(a0) * r0, sin(a0) * r0, z);
        let size = (0.7 + 1.3 * hash(seed + 6.0)) * mix(1.0, 0.5, phase);
        let c = globals.view_proj * vec4<f32>(world, 1.0);
        if c.w < RAY_NEAR { return hidden(); }
        out.clip = billboard(world, corner, max(size * globals.lod.x / c.w, 2.0), vec2<f32>(0.0, 1.0), 1.0 + phase * 2.0);
        out.state = vec3<f32>(SHAPE_MOTE, phase, 0.0);
        out.level = power * smoothstep(0.0, 0.15, phase) * (1.0 - smoothstep(0.85, 1.0, phase));
        return out;
    }

    // Kind 5: the print beam.
    let ru = max(b.radius, 0.5);
    let hu = max(b.height, 0.5);
    let middle = foot + vec3<f32>(0.0, 0.0, hu * 0.5);
    var axis = middle - b.emitter;
    let len = max(length(axis), 0.1);
    axis = axis / len;
    var lateral = cross(axis, vec3<f32>(0.0, 0.0, 1.0));
    lateral = select(vec3<f32>(0.0, 1.0, 0.0), normalize(lateral), dot(lateral, lateral) > 1e-6);
    let seed0 = b.emitter.x * 0.37 + b.emitter.y * 0.73;
    if slot < 2u {
        // Two fans from the head: a level scan line swept up and down the unit, and an
        // upright one swept side to side. Each is a flat quad in the world.
        let at_head = corner.x > 0.0;
        var world: vec3<f32>;
        if slot == 0u {
            let zs = hu * (0.5 + 0.5 * sin(time * 2.7 + seed0));
            let line = foot + vec3<f32>(0.0, 0.0, zs) + lateral * corner.y * ru * 1.15;
            world = select(line, b.emitter + lateral * corner.y * 0.35, at_head);
        } else {
            let ys = ru * 0.95 * sin(time * 1.9 + seed0 * 1.7);
            let line = foot + lateral * ys + vec3<f32>(0.0, 0.0, hu * (0.55 + 0.55 * corner.y));
            world = select(line, b.emitter + vec3<f32>(0.0, 0.0, corner.y * 0.35), at_head);
        }
        let c = globals.view_proj * vec4<f32>(world, 1.0);
        if c.w < RAY_NEAR { return hidden(); }
        out.clip = c;
        out.state = vec3<f32>(SHAPE_FAN, select(1.0, 0.0, at_head), f32(slot));
        return out;
    }
    if slot < 4u {
        // The head's glow, and the light where matter lands on the scan line.
        let at_head = slot == 2u;
        let zs = hu * (0.5 + 0.5 * sin(time * 2.7 + seed0));
        let world = select(foot + vec3<f32>(0.0, 0.0, zs), b.emitter, at_head);
        let size = select(ru * 0.9, 3.2, at_head);
        out.clip = rep_billboard(world, corner, size, 4.0, select(ru, 2.0, at_head));
        out.state = vec3<f32>(SHAPE_FLARE, select(1.0, 0.0, at_head), 0.5);
        out.level = power * select(0.55, 0.9, at_head);
        return out;
    }
    // Packets of matter, fast, landing all over the unit's volume.
    let seed = f32(slot) * 7.31 + seed0;
    let trip = 0.28 + 0.3 * hash(seed + 1.0);
    let phase = fract(time / trip + hash(seed + 2.0));
    let born = time - phase * trip;
    if born < b.start || (b.end >= 0.0 && born > b.end) {
        return hidden();
    }
    // A new spot each trip.
    let lap = floor(time / trip + hash(seed + 2.0));
    let ang = hash(seed + lap * 1.3) * 6.2832;
    let rad = ru * sqrt(hash(seed + lap * 2.1 + 0.5)) * 0.9;
    let hz = hu * hash(seed + lap * 3.7 + 0.25);
    let goal = foot + vec3<f32>(cos(ang) * rad, sin(ang) * rad, hz);
    let along = pow(phase, 0.75);
    let world = mix(b.emitter, goal, along);
    let c = globals.view_proj * vec4<f32>(world, 1.0);
    if c.w < RAY_NEAR { return hidden(); }
    let g = globals.view_proj * vec4<f32>(goal, 1.0);
    let e = globals.view_proj * vec4<f32>(b.emitter, 1.0);
    var dir = (g.xy / max(g.w, 0.01) - e.xy / max(e.w, 0.01)) * globals.viewport.xy;
    dir = select(vec2<f32>(1.0, 0.0), normalize(dir), length(dir) > 1e-3);
    let size = clamp(ru * 0.09, 0.25, 0.9) * mix(1.0, 1.8, smoothstep(0.85, 1.0, phase));
    out.clip = billboard(world, corner, max(size * globals.lod.x / c.w, 1.8), dir, mix(3.0, 1.0, smoothstep(0.8, 1.0, phase)));
    out.state = vec3<f32>(SHAPE_MOTE, along, 1.0);
    out.level = power * smoothstep(0.0, 0.08, phase);
    return out;
}

// Smooth 1D value noise.
fn rep_noise(x: f32) -> f32 {
    let i = floor(x);
    let f = fract(x);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(hash(i), hash(i + 1.0), u) * 2.0 - 1.0;
}

fn replicator_fragment(in: BeamOut, n: f32) -> vec4<f32> {
    let time = globals.camera.w;
    let shape = in.state.x;
    let run = in.state.y;
    if shape < SHAPE_RAY_SHEATH + 0.5 {
        // The sheath: cold blue, heat shimmer, filaments spiralling round the core, pulses.
        let half_drawn = max(in.state.z, 0.05);
        // Far off the ribbon is held wider than the ray: the glow widens with it.
        let r = max(in.extra.x, half_drawn) / 3.4;
        let px = max(in.extra.y, 1e-3);
        let y = in.uv.y * half_drawn;
        let raise = in.extra.z;
        let shimmer = 0.75 + 0.5 * n;
        let sheath = exp(-y * y / (r * r * 2.6)) * shimmer;
        let haze = pow(max(1.0 - abs(in.uv.y), 0.0), 2.0) * (0.5 + 0.5 * n);
        // Pulses out along the ray from the engine.
        let pulse = exp(-pow(abs(fract(run / 420.0 - time * 3.2) - 0.5), 2.0) * 70.0);
        var fil = 0.0;
        for (var i = 0; i < 3; i++) {
            let fi = f32(i);
            let turn = run * 0.085 - time * 7.0 + fi * 2.094;
            let jitter = rep_noise(run * 0.11 + time * 13.0 + fi * 31.0) * 0.35;
            let across = r * 1.75 * (sin(turn) + jitter);
            let front = 0.55 + 0.45 * cos(turn);
            let w = max(r * 0.12, px * 0.9);
            let d = y - across;
            // Crackle: the strands flicker on and off in short runs.
            let on = step(0.28, hash(floor(run * 0.05 + fi * 7.0) + floor(time * 22.0) * 3.1));
            fil += exp(-d * d / (w * w)) * front * (0.35 + 0.65 * on);
        }
        let hot = 1.0 + raise * 0.5;
        let color = REP_BLUE * (sheath * (1.4 + pulse * 1.6) + haze * 0.5) * hot + REP_ICE * fil * 2.4 * hot;
        let a = clamp(sheath * 0.35 + haze * 0.14 + fil * 0.25, 0.0, 0.8);
        return vec4<f32>(color * in.level, a * in.level);
    }
    if shape < SHAPE_RAY_CORE + 0.5 {
        let half_drawn = max(in.state.z, 0.05);
        let r = max(in.extra.x, half_drawn * 0.8);
        let y = in.uv.y * half_drawn;
        let core = exp(-y * y / (r * r * 0.35));
        let body = exp(-y * y / (r * r * 1.6));
        let pulse = exp(-pow(abs(fract(run / 420.0 - time * 3.2) - 0.5), 2.0) * 70.0);
        let flick = 0.9 + 0.1 * sin(time * 43.0 + run * 0.02);
        let color = HOT * core * (9.0 + pulse * 6.0) * flick + mix(REP_BLUE, REP_ICE, 0.5) * body * 3.5;
        return vec4<f32>(color * in.level, clamp(body * 0.6, 0.0, 0.9) * in.level);
    }
    if shape < SHAPE_FLARE + 0.5 {
        let d = length(in.uv);
        if d > 1.0 {
            discard;
        }
        // A hot point, a blue bloom and four long thin rays that turn slowly.
        let a = atan2(in.uv.y, in.uv.x) + time * select(0.25, -0.18, run > 0.5) + in.state.z;
        let spikes = pow(abs(cos(a * 2.0)), 40.0) * (1.0 - d) * 1.6 + pow(abs(cos(a * 2.0 + 0.785)), 90.0) * (1.0 - d) * 0.8;
        let core = exp(-d * d * 60.0);
        let bloom = pow(1.0 - d, 3.0);
        let flick = 0.85 + 0.15 * sin(time * 31.0 + in.state.z * 9.0);
        let color = HOT * (core * 10.0 + spikes * 3.0) + REP_BLUE * bloom * 3.0;
        return vec4<f32>(color * flick * in.level, clamp(bloom * 0.35 + core * 0.5, 0.0, 0.8) * in.level);
    }
    if shape < SHAPE_SPLASH + 0.5 {
        // Rings running out over the ground from the site, and a hot heart under it.
        let half = run;
        let r = length(in.uv) * half;
        if r > half {
            discard;
        }
        var rings = 0.0;
        for (var k = 0; k < 3; k++) {
            let front = fract(time / 1.6 + f32(k) / 3.0) * half;
            let d = r - front;
            rings += exp(-d * d / 6.0) * (1.0 - front / half);
        }
        let heart = exp(-r / 9.0);
        let spokes = pow(abs(sin(atan2(in.uv.y, in.uv.x) * 6.0 + time * 0.6)), 24.0) * exp(-r / 26.0);
        let edge = 1.0 - smoothstep(half * 0.7, half, r);
        let color = REP_BLUE * (rings * 1.8 + spokes * 0.9) * edge + HOT * heart * 2.2;
        return vec4<f32>(color * in.level, clamp(rings * 0.12 + heart * 0.3, 0.0, 0.5) * edge * in.level);
    }
    if shape < SHAPE_MOTE + 0.5 {
        let d = length(in.uv);
        if d > 1.0 {
            discard;
        }
        let fall = pow(1.0 - d, 2.0);
        let color = mix(REP_BLUE * 1.5, HOT * 3.0, run * fall);
        return vec4<f32>(color * fall * 2.2 * in.level, clamp(fall * 0.45, 0.0, 0.6) * in.level);
    }
    // A print fan: faint by the head, brightest along the line where it lays matter down.
    let along = run;
    let edge = abs(in.uv.y);
    let lines = pow(0.5 + 0.5 * sin(along * 40.0 - time * 30.0), 8.0);
    let front = exp(-pow(abs((1.0 - along) * 22.0), 2.0));
    let sides = smoothstep(0.8, 1.0, edge);
    let body = along * along * (0.25 + 0.4 * lines) + sides * along * 0.6;
    let color = REP_BLUE * body * 2.2 + HOT * front * 5.0 + REP_ICE * front * 2.0;
    return vec4<f32>(color * in.level, clamp(body * 0.22 + front * 0.4, 0.0, 0.7) * in.level);
}

// ---- Nanite work (the Regency) -----------------------------------------------------------
// `BEAM_NANITE`: from a builder's emitter to the weld on a Regency site. Not a beam but a
// bundle of strands of particles shot slowly across the gap: each a hairline thread that
// writhes like liquid, beaded with motes drifting along it, violet as it leaves and red by
// the time it arrives. The strands bow apart a little and turn slowly round the line
// between the ends, meeting at both; their heads creep out when the work starts, and when
// it stops their tails drain into the site. The moment the work starts both ends flash and
// a lead (a faint hairline with motes racing down it) snaps across the gap, holding it
// until the strands arrive. A knot of light where they pour in throws off motes, each let
// finish its flight when the work stops.
// `BEAM_NANITE_SITE`: round a site while it is fed, or round a refit. Splashes in slow
// motion with no gravity: a thin violet ring appears round the hull (most near the build
// front), its rim lifts into a crown, the light drains from the ring into the crown's
// peaks, and each peak is drawn up into a filament of red particles that leans outward,
// tears free of the ring, keeps drifting up and out past the hull's top and thins away.
// Colours: the violet of Regency construction, red where it has cooled toward plate.

const NANITE_VIOLET: vec3<f32> = vec3<f32>(0.66, 0.12, 1.0);
const NANITE_RED: vec3<f32> = vec3<f32>(1.0, 0.06, 0.08);
const NANITE_HOT: vec3<f32> = vec3<f32>(1.0, 0.62, 0.72);

const SHAPE_STRAND: f32 = 9.0;
const SHAPE_NANITE_GLOW: f32 = 10.0;
const SHAPE_NANITE_MOTE: f32 = 11.0;
const SHAPE_CROWN: f32 = 12.0;
const SHAPE_FILAMENT: f32 = 13.0;
const SHAPE_LEAD: f32 = 14.0;

// The stream: strands, each in segments, then the two glows, then the motes at the weld.
const STRANDS: u32 = 6u;
const STRAND_SEGS: u32 = 4u;
// Metres a second a strand's head creeps out at, and its motes drift along it.
const STRAND_SPEED: f32 = 8.0;
const STRAND_FLOW: f32 = 3.5;
// The stream's slots after its strands: the two glows, the lead, then motes.
const STREAM_LEAD: u32 = 2u;
// How fast the lead snaps across, metres a second.
const LEAD_SPEED: f32 = 140.0;
// Round a site: splashes, each a ring in segments (every other corner a crown peak) and
// a filament drawn up out of each peak.
const SPLASHES: u32 = 4u;
const CROWN_SEGS: u32 = 10u;
const CROWN_JETS: u32 = 5u;
const SPLASH_SLOTS: u32 = 15u;
const SPLASH_PERIOD_MIN: f32 = 4.5;
const SPLASH_PERIOD_MAX: f32 = 6.5;
// Metres either side of a strand's thread its ribbon adds for motes off the line.
const STRAND_PAD: f32 = 0.22;
// How far a filament's threads fan apart by its top, a share of its length either side.
const FILAMENT_FAN: f32 = 0.09;

// Strand `i` of a stream, `s` of the way from the emitter to the weld: bowed off the
// straight line by as much as `bow` metres, turning slowly about it.
fn strand_at(b: Beam, i: f32, s: f32, time: f32) -> vec3<f32> {
    let span = b.to - b.emitter;
    let len = max(length(span), 0.01);
    let axis = span / len;
    var b1 = cross(axis, vec3<f32>(0.0, 0.0, 1.0));
    if dot(b1, b1) < 1e-4 {
        b1 = vec3<f32>(1.0, 0.0, 0.0);
    }
    b1 = normalize(b1);
    let b2 = cross(axis, b1);
    let turn = hash(i * 3.1 + 0.7) * 6.2832 + time * select(-0.22, 0.22, hash(i + 9.3) > 0.5);
    let bow = clamp(len * 0.07, 0.4, 2.6) * (0.35 + 0.65 * hash(i * 1.9 + 4.1)) * sin(3.14159 * s);
    return b.emitter + span * s + (b1 * cos(turn) + b2 * sin(turn)) * bow;
}

// A ribbon for a strand (or a filament) from a to b: `rep_ribbon`, with what the fragment
// needs to draw the thread in `strand`: x where the head is and y the tail, in metres from
// `origin`; z the strand's whole length; w its seed.
fn strand_ribbon(a: vec3<f32>, b: vec3<f32>, origin: vec3<f32>, corner: vec2<f32>, amp: f32, shape: f32, strand: vec4<f32>) -> BeamOut {
    let seg = rep_clip(a, b);
    if seg[2].x < 0.0 {
        return hidden();
    }
    var o = rep_ribbon(seg[0], seg[1], origin, corner, amp + STRAND_PAD, 1.2, shape);
    o.strand = strand;
    o.level = 1.0;
    return o;
}

fn nanite_vertex(b: Beam, slot: u32, corner: vec2<f32>) -> BeamOut {
    let time = globals.camera.w;
    let len = max(distance(b.emitter, b.to), 0.01);
    var out: BeamOut;
    out.kind = f32(BEAM_NANITE);
    out.uv = corner;
    out.level = 1.0;
    let site = b.emitter.x * 0.37 + b.emitter.y * 0.73;
    if slot < STRANDS * STRAND_SEGS {
        let i = f32(slot / STRAND_SEGS);
        let j = f32(slot % STRAND_SEGS);
        // Heads creep out one after another; when the work stops the tails follow them
        // in (quick enough on a long stream to be gone before the beam is dropped).
        let speed = STRAND_SPEED * (0.8 + 0.4 * hash(i * 5.3 + site));
        let head = clamp((time - b.start - i * 0.14) * speed / len, 0.0, 1.0);
        var tail = 0.0;
        if b.end >= 0.0 {
            tail = clamp((time - b.end) * max(speed, len / 2.2) / len, 0.0, 1.0);
        }
        if head - tail < 0.002 {
            return hidden();
        }
        let s0 = tail + (head - tail) * j / f32(STRAND_SEGS);
        let s1 = tail + (head - tail) * (j + 1.0) / f32(STRAND_SEGS);
        let amp = clamp(len * 0.012, 0.12, 0.42);
        let seed = hash(i * 7.7 + site);
        return strand_ribbon(
            strand_at(b, i, s0, time), strand_at(b, i, s1, time), b.emitter, corner, amp, SHAPE_STRAND,
            vec4<f32>(head * len, tail * len, len, seed)
        );
    }
    // All the strands in, and all drained away: what the glows follow.
    let first = clamp((time - b.start) * STRAND_SPEED * 0.8 / len, 0.0, 1.0);
    var drained = 0.0;
    if b.end >= 0.0 {
        drained = clamp((time - b.end) * max(STRAND_SPEED * 0.8, len / 2.2) / len, 0.0, 1.0);
    }
    let on = smoothstep(0.85, 1.0, first) * (1.0 - smoothstep(0.7, 1.0, drained));
    // The flash the work starts with (none on a beam that was on before we looked: its
    // start is set seconds back).
    let flash = exp(-max(time - b.start, 0.0) * 4.0);
    let slot2 = slot - STRANDS * STRAND_SEGS;
    if slot2 < 2u {
        // The knot where the strands pour into the site, and a spark at the emitter: both
        // lit at once when the work starts, flaring as they come on.
        let at_site = slot2 == 0u;
        let world = select(b.emitter, b.to, at_site);
        if (globals.view_proj * vec4<f32>(world, 1.0)).w < RAY_NEAR {
            return hidden();
        }
        let throb = 0.8 + 0.2 * sin(time * 2.3 + site) * sin(time * 3.7);
        let size = select(0.45, clamp(b.radius * 0.1, 0.7, 2.2), at_site) * throb * (1.0 + 1.4 * flash);
        out.clip = rep_billboard(world, corner, size, select(2.0, 3.0, at_site), size * 0.8);
        out.state = vec3<f32>(SHAPE_NANITE_GLOW, select(max(0.3, flash), 1.0, at_site), 0.0);
        // Until the strands arrive the knot is held lit by the lead, dimmer.
        let held = 0.45 * (1.0 - drained);
        out.level = select(1.0 - drained, max(max(on, held), flash), at_site);
        return out;
    }
    if slot2 == STREAM_LEAD {
        // The lead: snaps across in a moment, then holds the gap faintly until the strands
        // are in; gone quickly once the work stops.
        var level = (1.0 - 0.75 * smoothstep(0.6, 1.0, first)) * (0.45 + 0.55 * flash);
        if b.end >= 0.0 {
            level *= 1.0 - smoothstep(0.0, 0.35, time - b.end);
        }
        let reach = clamp((time - b.start) * LEAD_SPEED, 0.0, len);
        if level <= 0.002 || reach < 0.05 {
            return hidden();
        }
        var o = strand_ribbon(
            b.emitter, b.to, b.emitter, corner, 0.07, SHAPE_LEAD,
            vec4<f32>(reach, 0.0, len, hash(site + 2.7))
        );
        o.kind = f32(BEAM_NANITE);
        o.level = level;
        return o;
    }
    // A mote thrown off where the strands pour in, drifting out and up as it dies: from the
    // moment the work starts, and each one let finish when it stops.
    let m = f32(slot2 - STREAM_LEAD - 1u);
    let period = 1.4 + 0.8 * hash(m * 2.3 + site);
    let t = time / period + hash(m * 4.1 + site);
    let ph = fract(t);
    let born = time - ph * period;
    let k = floor(t) * 1.618 + m * 3.3 + site;
    let dir = normalize(vec3<f32>(hash(k) - 0.5, hash(k + 1.0) - 0.5, 0.25 + hash(k + 2.0)));
    let world = b.to + dir * (0.3 + 1.6 * ph) * (0.6 + 0.6 * hash(k + 3.0));
    let c = globals.view_proj * vec4<f32>(world, 1.0);
    if c.w < RAY_NEAR || born < b.start || (b.end >= 0.0 && born > b.end) {
        return hidden();
    }
    let half_px = max(0.09 * globals.lod.x / max(c.w, 1.0), 1.0);
    out.clip = billboard(world, corner, half_px, vec2<f32>(1.0, 0.0), 1.0);
    out.state = vec3<f32>(SHAPE_NANITE_MOTE, ph, hash(k + 4.0));
    out.level = smoothstep(0.0, 0.1, ph) * (1.0 - smoothstep(0.5, 1.0, ph));
    return out;
}

// One splash round a site: a ring, centred `centre`, in the plane of `e1`/`e2`, of
// `radius`, its peaks lifted `lift` along `n`; `k` its seed.
struct Splash {
    centre: vec3<f32>,
    e1: vec3<f32>,
    e2: vec3<f32>,
    n: vec3<f32>,
    radius: f32,
    lift: f32,
    k: f32,
}

// Corner `v` of a splash's ring (`v` and `v + CROWN_SEGS` are the same corner). Even
// corners are crown peaks: lifted and flared out; odd ones sag a little between them.
fn crown_corner(sp: Splash, v: u32) -> vec3<f32> {
    let c = v % CROWN_SEGS;
    let fc = f32(c);
    let peak = c % 2u == 0u;
    let jit = (hash(sp.k + 11.0 + fc * 1.3) - 0.5) * 0.5;
    let a = (fc + jit) * 6.2832 / f32(CROWN_SEGS) + hash(sp.k + 9.0) * 6.2832;
    let radial = sp.e1 * cos(a) + sp.e2 * sin(a);
    let up = select(-0.15, 0.6 + 0.8 * hash(sp.k + 20.0 + fc), peak) * sp.lift;
    let flare = select(0.0, 0.18 * up, peak);
    return sp.centre + radial * (sp.radius + flare) + sp.n * up;
}

// Round a Regency site at work (`BEAM_NANITE_SITE`): `emitter` is the site's foot, `to`
// the middle of its build front, `radius` and `height` the hull's.
fn nanite_site_vertex(b: Beam, slot: u32, corner: vec2<f32>) -> BeamOut {
    let time = globals.camera.w;
    let foot = b.emitter;
    let h = max(b.height, 1.0);
    let r = max(b.radius, 1.0);
    let front = clamp(mix(b.to_prev, b.to, globals.sun.w).z - foot.z, 0.0, h);
    let site = hash(foot.x * 0.37 + foot.y * 0.73);
    if slot >= SPLASHES * SPLASH_SLOTS {
        return hidden();
    }
    // Each slot shows one splash after another. The first starts with the work and the
    // others follow a share of a period apart; one born before the work stopped is let
    // run its course, the lot thinning out over the seconds after.
    let i = f32(slot / SPLASH_SLOTS);
    let part = slot % SPLASH_SLOTS;
    let period = mix(SPLASH_PERIOD_MIN, SPLASH_PERIOD_MAX, hash(i * 3.7 + site));
    let t = (time - b.start) / period + i / f32(SPLASHES);
    let ph = fract(t);
    let born = time - ph * period;
    if born < b.start || (b.end >= 0.0 && born > b.end) {
        return hidden();
    }
    var level = 1.0;
    if b.end >= 0.0 {
        level = 1.0 - smoothstep(2.0, 7.5, time - b.end);
    }
    let k = floor(t) * 7.13 + i * 1.7 + site * 31.0;
    // Half of them at the build front; the rest anywhere up the hull.
    var z = hash(k + 2.0) * h * 0.95;
    if hash(k + 1.0) < 0.5 {
        z = front + (hash(k + 2.0) - 0.5) * 0.3 * h;
    }
    z = clamp(z, 0.2, h);
    // Round the hull, the odd one reaching a little past it; spreading slowly.
    let pick = hash(k + 3.0);
    var sp: Splash;
    sp.radius = r * (0.55 + 0.55 * pick * pick) * (1.0 + 0.12 * ph);
    // Laid nearly flat, a little askew.
    sp.n = normalize(vec3<f32>((hash(k + 5.0) - 0.5) * 0.2, (hash(k + 6.0) - 0.5) * 0.2, 1.0));
    sp.e1 = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), sp.n));
    sp.e2 = cross(sp.n, sp.e1);
    sp.centre = foot + vec3<f32>(0.0, 0.0, z);
    // The crown rises, then holds while its peaks are drawn off.
    sp.lift = clamp(sp.radius * 0.45, 0.8, h * 0.6) * smoothstep(0.06, 0.5, ph);
    sp.k = k;
    if part < CROWN_SEGS {
        // A stretch of ring from a sag to a peak (so the fragment knows which end is which).
        let peak_first = part % 2u == 0u;
        let pa = crown_corner(sp, select(part, part + 1u, peak_first));
        let pb = crown_corner(sp, select(part + 1u, part, peak_first));
        let life = smoothstep(0.0, 0.1, ph) * (1.0 - smoothstep(0.4, 0.78, ph));
        if life <= 0.002 {
            return hidden();
        }
        // The ring bows out between its corners, and dips toward the trough so the crown's
        // troughs are round and its peaks pointed: how far, across the drawn ribbon, in metres.
        let chord = distance(pa, pb);
        let mid = (pa + pb) * 0.5;
        var rad = mid - sp.centre;
        rad = normalize(rad - sp.n * dot(rad, sp.n) + vec3<f32>(1e-4, 0.0, 0.0));
        let bulge = rad * sp.radius * (1.0 - cos(3.14159 / f32(CROWN_SEGS)))
            - sp.n * 0.25 * max(dot(pb - pa, sp.n), 0.0);
        let ca = globals.view_proj * vec4<f32>(pa, 1.0);
        let cb = globals.view_proj * vec4<f32>(pb, 1.0);
        let cm = globals.view_proj * vec4<f32>(mid, 1.0);
        let co = globals.view_proj * vec4<f32>(mid + bulge, 1.0);
        var bow = 0.0;
        if ca.w > RAY_NEAR && cb.w > RAY_NEAR && cm.w > RAY_NEAR && co.w > RAY_NEAR {
            var dir = (cb.xy / cb.w - ca.xy / ca.w) * globals.viewport.xy;
            dir = dir / max(length(dir), 1e-4);
            let d = (co.xy / co.w - cm.xy / cm.w) * globals.viewport.xy;
            bow = dot(d, vec2<f32>(-dir.y, dir.x)) * cm.w / globals.lod.x;
        }
        let amp = clamp(sp.radius * 0.012, 0.06, 0.2);
        var o = strand_ribbon(
            pa, pb, pa, corner, amp + abs(bow), SHAPE_CROWN,
            vec4<f32>(chord, 0.0, chord, hash(k + 30.0 + f32(part)))
        );
        o.extra.x = amp + STRAND_PAD;
        // z how far it bows out mid-way, w how much of its light has drained to the peak.
        o.extra.z = bow;
        o.extra.w = smoothstep(0.22, 0.6, ph);
        o.kind = f32(BEAM_NANITE_SITE);
        o.level = level * life;
        return o;
    }
    // A filament drawn up out of a peak: its head rises on from the crown, slowing as if
    // through thick air, never falling; then it tears free and drifts on up, thinning out.
    let j = part - CROWN_SEGS;
    let base = crown_corner(sp, j * 2u);
    var out_dir = base - sp.centre;
    out_dir = normalize(out_dir - sp.n * dot(out_dir, sp.n) + vec3<f32>(1e-4, 0.0, 0.0));
    let fj = f32(j);
    let swirl = cross(sp.n, out_dir) * (hash(k + 40.0 + fj) - 0.5) * 0.3;
    let dir = normalize(sp.n + out_dir * (0.2 + 0.35 * hash(k + 41.0 + fj)) + swirl);
    // Up past the hull's top; a low, wide hull still throws them well up.
    let reach = (h - z) * 0.5 + max(h, r * 0.6) * (0.6 + 0.6 * hash(k + 42.0 + fj));
    let rise = clamp((ph - 0.14) / 0.86, 0.0, 1.0);
    let head = reach * (1.0 - (1.0 - rise) * (1.0 - rise));
    let free = clamp((ph - 0.5) / 0.5, 0.0, 1.0);
    let tail = reach * 0.72 * pow(free, 1.3);
    if head - tail < 0.05 {
        return hidden();
    }
    let len = head;
    let amp = clamp(h * 0.035, 0.2, 0.8);
    var o = strand_ribbon(
        base + dir * tail, base + dir * head, base, corner, amp + len * FILAMENT_FAN, SHAPE_FILAMENT,
        vec4<f32>(head, tail, len, hash(k + 7.0 + fj))
    );
    o.kind = f32(BEAM_NANITE_SITE);
    o.level = level * smoothstep(0.12, 0.24, ph) * (1.0 - smoothstep(0.55, 1.0, ph));
    return o;
}

// A thread of particles: a hairline snaking about the ribbon's middle (two slow waves run
// along it, so it writhes like liquid), beaded with motes drifting along it at `flow`
// metres a second, some missing, some hot. `m` metres along, `y` metres across, `px`
// metres a pixel, `u` how many waves in from the start. x how bright, y how much of that
// is a hot mote.
fn particle_thread(m: f32, u: f32, y: f32, px: f32, amp: f32, seed: f32, flow: f32, time: f32) -> vec2<f32> {
    let w = amp * (0.62 * sin(u * 6.28 + time * 1.1 + seed * 6.28) + 0.38 * sin(u * 15.0 - time * 1.9 + seed * 17.0));
    let d = y - w;
    let thin = max(0.03, px * 0.7);
    let line = exp(-d * d / (thin * thin)) * 0.55;
    let spacing = 0.3;
    let along = (m - flow * time) / spacing + seed * 13.0;
    let k = floor(along);
    let h = hash(k * 1.37 + seed * 91.0);
    let dx = (fract(along) - 0.5) * spacing;
    let dy = d - (hash(k * 2.11 + seed * 7.0) - 0.5) * 0.14;
    let size = max(0.06 + 0.07 * h, px * 1.1);
    let bead = exp(-(dx * dx + dy * dy) / (size * size)) * step(0.3, h);
    let twinkle = 0.55 + 0.45 * sin(time * (4.0 + 8.0 * h) + k);
    return vec2<f32>(line + bead * twinkle * (0.5 + h), bead * step(0.82, h));
}

fn nanite_fragment(in: BeamOut, n: f32) -> vec4<f32> {
    let time = globals.camera.w;
    let shape = in.state.x;
    if shape < 9.5 || shape > 11.5 {
        // A strand of the stream, its lead, a stretch of a splash's ring, or a filament.
        let filament = shape > 12.5 && shape < 13.5;
        let crown = shape > 11.5 && shape < 12.5;
        let lead = shape > 13.5;
        let m = in.state.y;
        let head = in.strand.x;
        let tail = in.strand.y;
        let total = max(in.strand.z, 0.01);
        let s = clamp(m / total, 0.0, 1.0);
        // A filament's torn end frays over a good part of it; a ring has no ends.
        let fray = select(0.5, max(0.5, 0.3 * (head - tail)), filament);
        var ends = smoothstep(tail, tail + fray, m) * (1.0 - smoothstep(head - 0.25, head + 0.05, m));
        if crown {
            ends = 1.0;
        }
        if ends <= 0.001 {
            discard;
        }
        let y = in.uv.y * in.state.z;
        let px = in.extra.y;
        let fan = select(0.0, total * FILAMENT_FAN, filament);
        // A stream is tight at both ends; a filament loosens as it rises; a ring is even.
        var env = select(0.25 + 0.75 * sin(3.14159 * s), 0.35 + 0.65 * s, filament);
        if crown {
            env = 1.0;
        }
        let amp = max(in.extra.x - STRAND_PAD - fan, 0.05) * env;
        // A ring bows out between its corners.
        let bow = select(0.0, in.extra.z * 4.0 * s * (1.0 - s), crown);
        var flow = select(STRAND_FLOW, 2.6, filament);
        if crown {
            flow = 1.2;
        } else if lead {
            flow = 30.0;
        }
        let seed = in.strand.w;
        // A bundle of threads, each writhing on its own, a wave or two a strand; a
        // filament's threads fan apart as they rise, a sheaf thrown upward.
        var bright = 0.0;
        var hot = 0.0;
        var threads = select(3, 6, filament);
        if crown {
            threads = 2;
        } else if lead {
            threads = 1;
        }
        for (var j = 0; j < threads; j++) {
            let fj = f32(j);
            let sj = seed + fj * 0.173;
            let lane = (hash(sj * 31.0) * 2.0 - 1.0) * fan * s;
            let waves = select(1.2, 1.8, filament) * (0.8 + 0.4 * hash(sj * 17.0));
            let t = particle_thread(m, s * waves, y - lane - bow, px, amp * (0.6 + 0.4 * hash(sj * 7.0)), sj, flow * (0.8 + 0.4 * hash(sj * 3.0)), time);
            let weight = select(1.0, 0.65, j > 0);
            bright += t.x * weight;
            hot += t.y * weight;
        }
        if crown {
            // The light drains out of the ring into its peaks (s = 1), which run hot.
            let drain = in.extra.w;
            let gather = mix(1.0, pow(s, 3.0) * 1.6, drain);
            // Kept deep: a bright violet line blooms out to near white.
            let tint = mix(NANITE_VIOLET, NANITE_RED, 0.15 + 0.5 * drain * s);
            let color = tint * bright * 0.9 * gather + NANITE_HOT * (hot * 0.5 + bright * drain * pow(s, 6.0) * 0.6);
            return vec4<f32>(color * in.level, clamp(bright * 0.2 * gather, 0.0, 0.35) * in.level);
        }
        // The head glows as it creeps on (the lead's as it snaps across).
        let tip = exp(-pow((head - m) / 0.5, 2.0)) * (0.5 + 0.5 * n);
        if lead {
            let color = NANITE_VIOLET * bright * 1.6 + NANITE_HOT * (tip * 2.0 + hot * 1.5);
            return vec4<f32>(color * ends * in.level, 0.0);
        }
        // Violet as it leaves (a filament at its root), red for the rest of the way.
        let tint = mix(NANITE_VIOLET, NANITE_RED, smoothstep(0.04, select(0.4, 0.3, filament), s));
        let color = tint * bright * 2.6 + NANITE_HOT * (hot * 2.4 + tip * 1.2 * bright);
        let cover = clamp(bright * 0.3, 0.0, 0.45);
        return vec4<f32>(color * ends * in.level, cover * ends * in.level);
    }
    let d = length(in.uv);
    if d > 1.0 {
        discard;
    }
    let fall = pow(1.0 - d, 2.0);
    if shape < 10.5 {
        // A glow: violet at its rim, red in it, near white in its heart where it feeds.
        let rim = mix(NANITE_VIOLET, NANITE_RED, fall * 0.7);
        let hot = mix(rim, NANITE_HOT, fall * fall * in.state.y);
        return vec4<f32>(hot * fall * 2.6 * in.level, clamp(fall * 0.4 * in.level, 0.0, 0.5));
    }
    // A mote: red, a few hot.
    let color = mix(NANITE_RED, NANITE_HOT, step(0.8, in.state.z) * fall);
    return vec4<f32>(color * fall * 3.0 * in.level, 0.0);
}

// fs_beam, seen through the water from under it (bindings.wgsl `under_sea_seen`).
@fragment
fn fs_beam(in: BeamOut) -> @location(0) vec4<f32> {
    return under_sea_seen(fs_beam_lit(in), in.clip);
}
