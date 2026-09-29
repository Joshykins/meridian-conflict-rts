// The light round a capital ship's warp (renderer/warp_fx.rs): the drive's charge, the
// streak and flash as it goes, the rift it comes out of. Prepended to shaders with
// `//!use warp_puffs` (puffs.wgsl, whose `Puff` and `PuffOut` these use).
//
// Every one is light added to the scene (the rift's heart also hides what is behind it).
// `appearance.rgb` is its colour, brightness in its size; `appearance.w` how torn a
// dampened jump makes it: 0 clean, 1 stuttering, jittering and flickering.

fn is_warp_puff(kind: u32) -> bool {
    return kind >= PUFF_WARP_GLOW && kind <= PUFF_WARP_MOTE;
}

// Changes `rate` times a second, differently for each `seed`: 0 to 1.
fn warp_flicker(rate: f32, seed: f32) -> f32 {
    return hash11(floor(globals.camera.w * rate) + seed * 173.0);
}

// Where a warp puff is drawn. `out` comes with its origin, opacity and appearance set.
fn warp_puff_vertex(p: Puff, corner: vec2<f32>, t: f32, age: f32, o: PuffOut) -> PuffOut {
    var out = o;
    let kind = u32(p.params.z);
    let size = mix(p.params.x, p.params.y, sqrt(age));
    out.state = vec3<f32>(age, p.params.z, p.params.w);
    out.roll = p.vel;
    out.uv = corner;
    if kind == PUFF_WARP_STREAK || kind == PUFF_WARP_ARC {
        // A ribbon from `pos` along `vel`, turned about its axis to face the eye, never
        // thinner than a couple of pixels so a far streak still reads.
        let span = length(p.vel);
        if span < 0.01 {
            return out;
        }
        let axis = p.vel / span;
        let view = normalize(globals.camera.xyz - p.pos);
        var side = cross(axis, view);
        if length(side) < 0.01 {
            side = cross(axis, vec3<f32>(0.0, 0.0, 1.0));
        }
        side = normalize(side);
        let along = p.pos + p.vel * ((corner.x + 1.0) * 0.5);
        let w = (globals.view_proj * vec4<f32>(along, 1.0)).w;
        let floor_m = 2.5 * max(w, 1.0) / max(globals.lod.x, 1.0);
        let world = along + side * corner.y * max(size, floor_m);
        out.clip = globals.view_proj * vec4<f32>(world, 1.0);
        out.world = world;
        return out;
    }
    var pos = p.pos;
    if kind == PUFF_WARP_GLOW || kind == PUFF_WARP_MOTE {
        // Carried off at `vel`, the air taking it off.
        let drag = select(1.2, 1.8, kind == PUFF_WARP_MOTE);
        pos = p.pos + p.vel * ((1.0 - exp(-drag * t)) / drag);
    }
    let center = globals.view_proj * vec4<f32>(pos, 1.0);
    let floor_px = select(select(3.0, 1.6, kind == PUFF_WARP_MOTE), 4.0, kind == PUFF_WARP_RIFT);
    let px = max(size * globals.lod.x / max(center.w, 1.0), floor_px);
    let ndc = center.xy / center.w + corner * px * globals.viewport.zw;
    // A glow round a hull is tested at its near side, so the hull does not slice it.
    var depth = center.z;
    if kind == PUFF_WARP_GLOW {
        depth = front_depth(pos, size * 0.5) * center.w;
    }
    out.clip = vec4<f32>(ndc * center.w, depth, center.w);
    out.world = effect_billboard_world(pos, corner, size);
    return out;
}

// Its colour, premultiplied: light in rgb, and how much of what is behind it is hidden.
fn warp_puff_color(in: PuffOut, d: f32) -> vec4<f32> {
    let kind = u32(in.state.y);
    let age = in.state.x;
    let seed = in.state.z;
    let rgb = in.appearance.rgb;
    let torn = clamp(in.appearance.w, 0.0, 1.0);
    let now = globals.camera.w;
    // A torn jump's light gutters: dropping out and flaring a dozen or more times a second.
    let gutter = mix(1.0, 0.25 + 1.2 * warp_flicker(19.0, seed), torn);
    if kind == PUFF_WARP_STREAK {
        return warp_streak(in, rgb, torn, age, seed) * gutter;
    }
    if kind == PUFF_WARP_ARC {
        return warp_arc(in, rgb, age, seed);
    }
    if d > 1.0 {
        discard;
    }
    if kind == PUFF_WARP_RIFT {
        return warp_rift(in, d, rgb, torn, age, seed);
    }
    if kind == PUFF_WARP_MOTE {
        let glow = pow(max(1.0 - d, 0.0), 1.6);
        let twinkle = 0.55 + 0.45 * sin(now * 27.0 + seed * 40.0);
        let fade = 1.0 - age * age;
        return vec4<f32>(rgb * glow * twinkle * fade * gutter, 0.0);
    }
    // A glow: a white heart in a coloured halo, in fast and out slowly.
    let fade = smoothstep(0.0, 0.08, age) * (1.0 - smoothstep(0.3, 1.0, age));
    let halo = pow(max(1.0 - d, 0.0), 2.2);
    let core = pow(max(1.0 - d * 1.7, 0.0), 2.6);
    let white = mix(rgb, vec3<f32>(length(rgb) * 0.6), 0.7);
    return vec4<f32>((rgb * halo * 0.7 + white * core * 2.4) * fade * gutter, 0.0);
}

// A streak: its head runs out along the ribbon in the first part of its life, brightest at
// the head, a thin line left behind it that fades. Torn, it jitters off its line in steps
// and breaks up.
fn warp_streak(in: PuffOut, rgb: vec3<f32>, torn: f32, age: f32, seed: f32) -> vec4<f32> {
    let now = globals.camera.w;
    let x = (in.uv.x + 1.0) * 0.5;
    let cell = floor(x * 26.0);
    let line = (hash11(cell + floor(now * 20.0) * 7.0 + seed * 13.0) - 0.5) * 0.8 * torn;
    let y = abs(in.uv.y - line);
    let run = min(age / 0.35, 1.0);
    let head = 1.0 - (1.0 - run) * (1.0 - run);
    if x > head + 0.02 {
        discard;
    }
    let lead = pow(clamp(x / max(head, 0.001), 0.0, 1.0), 3.0);
    let left = 0.3 * (1.0 - age);
    let knot = exp(-pow((x - head) * 40.0, 2.0)) * (1.0 - run * 0.6);
    let core = exp(-y * y / 0.012);
    let halo = exp(-y * y / 0.2) * 0.4;
    let gap = mix(1.0, step(0.3, hash11(floor(x * 34.0 + now * 24.0) + seed * 5.0)), torn);
    let fade = 1.0 - smoothstep(0.4, 1.0, age);
    let white = mix(rgb, vec3<f32>(length(rgb) * 0.6), 0.75);
    let light = (white * core * 3.0 + rgb * halo) * (lead + left + knot * 3.0) * fade * gap;
    return vec4<f32>(light, 0.0);
}

// A jagged arc of lightning along the ribbon, re-struck fifteen times a second.
fn warp_arc(in: PuffOut, rgb: vec3<f32>, age: f32, seed: f32) -> vec4<f32> {
    let now = globals.camera.w;
    let x = (in.uv.x + 1.0) * 0.5;
    let strike = floor(now * 15.0) * 17.0 + seed * 97.0;
    let coarse = x * 7.0;
    let fine = x * 23.0;
    let a = mix(hash11(floor(coarse) + strike), hash11(floor(coarse) + 1.0 + strike), fract(coarse));
    let b = mix(hash11(floor(fine) + strike + 50.0), hash11(floor(fine) + 51.0 + strike), fract(fine));
    let line = ((a - 0.5) * 1.1 + (b - 0.5) * 0.4) * sin(3.14159 * x);
    let y = abs(in.uv.y - line);
    let core = exp(-y * y / 0.003);
    let halo = exp(-y * y / 0.06) * 0.35;
    let on = step(0.22, warp_flicker(31.0, seed));
    let fade = 1.0 - age;
    let white = mix(rgb, vec3<f32>(length(rgb) * 0.6), 0.6);
    return vec4<f32>((white * core * 3.0 + rgb * halo) * on * fade, 0.0);
}

// The rift a jump comes out of, turned to the eye. Closed (`roll.x` 0) it is a ripple, air
// bent in rings; opening, a ring of bent light closes round a dark heart with a white-hot
// point in it, arms of light winding in. Torn, the ring is jagged and it all flickers.
fn warp_rift(in: PuffOut, d: f32, rgb: vec3<f32>, torn: f32, age: f32, seed: f32) -> vec4<f32> {
    let now = globals.camera.w;
    let open = clamp(in.roll.x, 0.0, 1.0);
    let angle = atan2(in.uv.y, in.uv.x);
    let fade = smoothstep(0.0, 0.15, age) * (1.0 - smoothstep(0.6, 1.0, age));
    let jag = (hash11(floor(angle * 4.0 + now * 21.0) + seed * 29.0) - 0.5) * 0.16 * torn;
    let r0 = 0.5 + jag;
    let ring = exp(-pow((d - r0) / (0.05 + 0.04 * open), 2.0));
    let spin = now * (2.0 + 5.0 * open) * select(1.0, -1.0, seed > 0.5);
    let arms = pow(0.5 + 0.5 * sin(3.0 * angle + 7.0 * log(d + 0.04) - spin + seed * 6.0), 3.0);
    let band = smoothstep(0.1, 0.35, d) * (1.0 - smoothstep(0.6, 1.0, d));
    let halo = pow(max(1.0 - d, 0.0), 2.0);
    let ripple = (0.5 + 0.5 * sin(d * 34.0 - now * 9.0)) * (1.0 - d) * smoothstep(0.1, 0.4, d);
    let heart = (1.0 - smoothstep(0.14, 0.3, d)) * open;
    let point = exp(-d * d / (0.002 + 0.012 * open));
    let white = mix(rgb, vec3<f32>(length(rgb) * 0.6), 0.7);
    var light = rgb * (ring * (0.35 + 1.4 * open) + arms * band * open * 1.1 + halo * 0.3 + ripple * 0.25 * (1.0 - open))
        + white * point * 5.0 * open;
    if torn > 0.0 {
        // A red lash through the ring now and then.
        light = mix(light, vec3<f32>(length(rgb), 0.08, 0.14) * ring * 2.5, step(0.75, warp_flicker(13.0, seed + 0.5)) * torn);
        light *= 0.35 + 1.1 * warp_flicker(17.0, seed);
    }
    return vec4<f32>(light * fade, heart * 0.9 * fade);
}
