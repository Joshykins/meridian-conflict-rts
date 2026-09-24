//!use bindings
// Work beams. The sim lists who is at work each tick; everything seen here is
// made from that on the GPU. Reclaim (kind 0): a cone that grips the target
// and narrows into the emitter, torn-off bits streaming back up it, heating
// from red through orange to white. Repair (kind 2): the inverse — mint-green
// patches leave the emitter and settle onto the hull. Kind 1 is kept for
// construction. Premultiplied: hot cores only add light; the coloured body
// also covers what is behind it, or over grass it would wash out.

// Mirrors the renderer's GpuBeam: mc_sim::reclaim::BeamInstance, and when the beam came on and went off.
struct Beam {
    emitter: vec3<f32>,
    // 0 reclaim. 1 is kept for construction. 2 repair. 3 relay. 4 replication ray, 5 print beam.
    kind: u32,
    to_prev: vec3<f32>,
    radius: f32,
    to: vec3<f32>,
    height: f32,
    // Seconds, on the clock of globals.camera.w. `end` is negative while the beam is on.
    start: f32,
    end: f32,
    pad: vec2<f32>,
}

@group(1) @binding(1) var<storage, read> beams: array<Beam>;

// Quads per beam: the ribbon, the two glows, and the bits.
const SLOTS: u32 = 32u;
const SHAPE_RIBBON: f32 = 0.0;
const SHAPE_GLOW: f32 = 1.0;
const SHAPE_BIT: f32 = 2.0;

const WHITE: vec3<f32> = vec3<f32>(1.0, 0.92, 0.78);
const ORANGE: vec3<f32> = vec3<f32>(1.0, 0.42, 0.07);
const RED: vec3<f32> = vec3<f32>(0.9, 0.07, 0.02);
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
    let b = beams[instance / SLOTS];
    let slot = instance % SLOTS;
    let time = globals.camera.w;
    if b.kind >= 4u {
        return replicator_vertex(b, slot, corner, instance);
    }
    let foot = mix(b.to_prev, b.to, globals.sun.w);
    let grip = foot + vec3<f32>(0.0, 0.0, b.height * 0.55);
    let span = b.emitter - grip;
    let len = max(length(span), 0.01);
    let axis = span / len;

    let a = globals.view_proj * vec4<f32>(grip, 1.0);
    let e = globals.view_proj * vec4<f32>(b.emitter, 1.0);
    if a.w <= 0.01 || e.w <= 0.01 {
        return hidden();
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
        out.state = vec3<f32>(SHAPE_RIBBON, select(0.0, len, at_emitter), half_px * max(p.w, 1.0) / globals.lod.x);
        return out;
    }
    if slot <= 2u {
        // Reclaim bites the hull and pours into the emitter. Repair lights the
        // projector and a scatter of patches where the bits land.
        let at_emitter = slot == 2u;
        let world = select(grip, b.emitter, at_emitter);
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
        let flicker = 0.75 + 0.25 * sin(time * 31.0 + f32(instance)) * sin(time * 17.3);
        out.clip = billboard(world, corner, max(radius * globals.lod.x / max(w, 1.0), 2.5), vec2<f32>(1.0, 0.0), 1.0);
        out.state = vec3<f32>(SHAPE_GLOW, select(0.0, 1.0, at_emitter), 0.0);
        out.level = power * flicker * select(select(0.55, 0.7, repair), select(1.6, 1.35, repair), at_emitter) * select(1.0, 0.55, ferry);
        return out;
    }

    // A bit on the beam. Seeded by the emitter, so it does not jump when the list of beams changes.
    let seed = f32(slot) * 7.31 + b.emitter.x * 0.37 + b.emitter.y * 0.73;
    let trip = clamp(len / 42.0, 0.55, 2.2) * (0.8 + 0.5 * hash(seed + 1.0));
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
    let half_px = max(size * globals.lod.x / max(c.w, 1.0), 2.0);
    // Drawn out along its flight the faster it goes.
    out.clip = billboard(world, corner, half_px, dir, 1.0 + along * 2.0);
    // Heat follows along: reclaim heats toward the emitter; repair cools as it seats.
    out.state = vec3<f32>(SHAPE_BIT, along, hash(seed + 8.0));
    out.level = smoothstep(0.0, 0.1, phase) * (1.0 - smoothstep(0.9, 1.0, phase)) * select(1.0, 0.85, ferry);
    return out;
}

@fragment
fn fs_beam(in: BeamOut) -> @location(0) vec4<f32> {
    let time = globals.camera.w;
    let run = in.state.y;
    // Kind 3 is the ferry home: reclaim colours, particles only.
    let repair = in.kind > 1.5 && in.kind < 2.5;
    // Sampled for every shape: a texture is read in uniform control flow.
    let n = textureSample(noise_map, repeat_sampler, vec2<f32>(run * 0.035 + time * 0.9, in.uv.y * 0.11 + time * 0.07)).b;
    if in.state.x > 2.5 {
        return replicator_fragment(in, n);
    }
    if in.state.x < 0.5 {
        // Reclaim: white core, orange about it, red to the edge.
        // Repair: mint core, teal body, deep green edge — mass going back in.
        let half_m = max(in.state.z, 0.05);
        let y = abs(in.uv.y) * half_m;
        // Pulses follow the bits: reclaim toward the emitter, repair toward the hull.
        let pulse = 0.7 + 0.3 * sin(run * 0.9 + time * select(14.0, -16.0, repair));
        let core = exp(-y * y / 0.02) * (0.8 + 0.4 * n);
        let body = exp(-y * y / (0.16 * half_m * half_m + 0.06)) * (0.45 + 0.8 * n) * pulse;
        let edge = pow(max(1.0 - abs(in.uv.y), 0.0), 1.5) * (0.35 + 0.65 * n);
        let color = select(
            WHITE * core * 5.0 + ORANGE * body * 1.3 + RED * edge * 0.8,
            MINT * core * 4.4 + TEAL * body * 1.45 + DEEP * edge * 0.95,
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
            mix(mix(RED, ORANGE, fall), WHITE, in.state.y * fall),
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
        mix(mix(RED, ORANGE, smoothstep(0.0, 0.45, heat)), WHITE, smoothstep(0.55, 1.0, heat)),
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
// node is raised (0..1). A blinding white core in a violet sheath with filaments
// spiralling round it, pulses running out from the engine, a flare at either end, a
// splash of light on the ground at the site and motes of matter drawn up into it.
// Kind 5, the print beam: from a projector to a unit being printed; `radius` and
// `height` are the unit's. Two violet fans sweep the unit's volume (one up and down,
// one side to side) while packets of matter stream out and land all over it.
// Everything is built from its two end points, so detail does not depend on length;
// a far end behind the eye is pulled in to just in front of it.

const RAY_CATCH: f32 = 46.0;
const RAY_NEAR: f32 = 2.0;
const VIOLET: vec3<f32> = vec3<f32>(0.52, 0.2, 1.0);
const LILAC: vec3<f32> = vec3<f32>(0.8, 0.62, 1.0);
const HOT: vec3<f32> = vec3<f32>(1.0, 0.95, 1.0);
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
        // The sheath: violet, heat shimmer, filaments spiralling round the core, pulses.
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
        let pulse = exp(-pow(fract(run / 420.0 - time * 3.2) - 0.5, 2.0) * 70.0);
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
        let color = VIOLET * (sheath * (1.4 + pulse * 1.6) + haze * 0.5) * hot + LILAC * fil * 2.4 * hot;
        let a = clamp(sheath * 0.35 + haze * 0.14 + fil * 0.25, 0.0, 0.8);
        return vec4<f32>(color * in.level, a * in.level);
    }
    if shape < SHAPE_RAY_CORE + 0.5 {
        let half_drawn = max(in.state.z, 0.05);
        let r = max(in.extra.x, half_drawn * 0.8);
        let y = in.uv.y * half_drawn;
        let core = exp(-y * y / (r * r * 0.35));
        let body = exp(-y * y / (r * r * 1.6));
        let pulse = exp(-pow(fract(run / 420.0 - time * 3.2) - 0.5, 2.0) * 70.0);
        let flick = 0.9 + 0.1 * sin(time * 43.0 + run * 0.02);
        let color = HOT * core * (9.0 + pulse * 6.0) * flick + mix(VIOLET, LILAC, 0.5) * body * 3.5;
        return vec4<f32>(color * in.level, clamp(body * 0.6, 0.0, 0.9) * in.level);
    }
    if shape < SHAPE_FLARE + 0.5 {
        let d = length(in.uv);
        if d > 1.0 {
            discard;
        }
        // A hot point, a violet bloom and four long thin rays that turn slowly.
        let a = atan2(in.uv.y, in.uv.x) + time * select(0.25, -0.18, run > 0.5) + in.state.z;
        let spikes = pow(abs(cos(a * 2.0)), 40.0) * (1.0 - d) * 1.6 + pow(abs(cos(a * 2.0 + 0.785)), 90.0) * (1.0 - d) * 0.8;
        let core = exp(-d * d * 60.0);
        let bloom = pow(1.0 - d, 3.0);
        let flick = 0.85 + 0.15 * sin(time * 31.0 + in.state.z * 9.0);
        let color = HOT * (core * 10.0 + spikes * 3.0) + VIOLET * bloom * 3.0;
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
        let color = VIOLET * (rings * 1.8 + spokes * 0.9) * edge + HOT * heart * 2.2;
        return vec4<f32>(color * in.level, clamp(rings * 0.12 + heart * 0.3, 0.0, 0.5) * edge * in.level);
    }
    if shape < SHAPE_MOTE + 0.5 {
        let d = length(in.uv);
        if d > 1.0 {
            discard;
        }
        let fall = pow(1.0 - d, 2.0);
        let color = mix(VIOLET * 1.5, HOT * 3.0, run * fall);
        return vec4<f32>(color * fall * 2.2 * in.level, clamp(fall * 0.45, 0.0, 0.6) * in.level);
    }
    // A print fan: faint by the head, brightest along the line where it lays matter down.
    let along = run;
    let edge = abs(in.uv.y);
    let lines = pow(0.5 + 0.5 * sin(along * 40.0 - time * 30.0), 8.0);
    let front = exp(-pow((1.0 - along) * 22.0, 2.0));
    let sides = smoothstep(0.8, 1.0, edge);
    let body = along * along * (0.25 + 0.4 * lines) + sides * along * 0.6;
    let color = VIOLET * body * 2.2 + HOT * front * 5.0 + LILAC * front * 2.0;
    return vec4<f32>(color * in.level, clamp(body * 0.22 + front * 0.4, 0.0, 0.7) * in.level);
}
