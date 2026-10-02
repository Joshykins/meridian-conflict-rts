// The Regency's squeezed plasma as light (renderer/regency_guns_fx.rs): the charge a
// Pinched or Pinch-fusion gun gathers in front of its bore, the burst where a plasma shot
// lets go, the globs of plasma it throws and the clumps a blast or a muzzle throws out.
// Prepended to puffs.wgsl with `//!use plasma_puffs` (it uses that file's `Puff` and
// `PuffOut`).
//
// All of it is light added to the scene. `appearance.rgb` is the plasma's colour,
// brightness in its size; `appearance.w` how far it has gone over to fusion: 0 red
// plasma, 1 fusion, white at the heart with the prism's pinks (common.wgsl `prism`)
// drifting over it. Nothing here is wound or turned about its middle: the plasma churns
// in cells and lumps, and no shape runs round the centre in arms.

fn is_plasma_puff(kind: u32) -> bool {
    return kind == PUFF_PLASMA_ORB || kind == PUFF_PLASMA_BURST || kind == PUFF_PLASMA_GLOB
        || kind == PUFF_PLASMA_WAKE || kind == PUFF_STAR_CORE || kind == PUFF_SUPERNOVA
        || kind == PUFF_NOVA_WISP;
}

// Turned to the eye, where it was born; a burst is drawn a little toward the eye so the
// ground it stands on does not cut it in half.
//
// `params.w` is the seed in its fraction and how long it lingers in its whole part
// (renderer `push_lingering`): a puff that lingers `n` lives `1 + n` times as long and is
// drawn at an age that runs at the old pace at first and slows toward its end, so it opens
// as it would have and then cools slowly. Where it is carried still goes by the clock, but
// a lingering wake is braked harder by the air (a thrown clump comes to a stop and hangs).
fn plasma_puff_vertex(p: Puff, corner: vec2<f32>, life_age: f32, o: PuffOut) -> PuffOut {
    var out = o;
    let kind = u32(p.params.z);
    let age = 1.0 - pow(1.0 - life_age, 1.0 + floor(p.params.w));
    // A charge grows steadily; a burst throws itself out fast and slows; a wake swells.
    var grow = select(age, 1.0 - pow(1.0 - age, 3.0), kind == PUFF_PLASMA_BURST);
    if kind == PUFF_PLASMA_WAKE {
        grow = sqrt(age);
    }
    if kind == PUFF_NOVA_WISP {
        grow = sqrt(age);
    }
    if kind == PUFF_SUPERNOVA {
        // Torn outward and slowed by what it sweeps up, still drifting out as it fades.
        grow = (1.0 - pow(1.0 - age, 4.0)) * 0.85 + 0.15 * age;
    }
    let size = mix(p.params.x, p.params.y, grow);
    out.state = vec3<f32>(age, p.params.z, fract(p.params.w));
    out.uv = corner;
    if kind == PUFF_SUPERNOVA {
        return supernova_vertex(p, corner, size, out);
    }
    let to_eye = normalize(globals.camera.xyz - p.pos);
    var pos = p.pos + to_eye * select(0.0, size * 0.35, kind == PUFF_PLASMA_BURST);
    if kind == PUFF_PLASMA_GLOB {
        // Thrown: the air takes its speed off, and it sags.
        let t = life_age * p.life;
        pos = p.pos + p.vel * ((1.0 - exp(-3.0 * t)) / 3.0) - vec3<f32>(0.0, 0.0, 2.5 * t * t);
    }
    if kind == PUFF_PLASMA_WAKE {
        // Left hanging: it drifts off on what it was given, slowing, and rises as it cools.
        // One that lingers is braked harder, so it comes to a stop and hangs as it cools.
        let t = life_age * p.life;
        let drag = 1.6 + 1.2 * floor(p.params.w);
        pos = p.pos + p.vel * ((1.0 - exp(-drag * t)) / drag) + vec3<f32>(0.0, 0.0, 1.2 * t);
    }
    if kind == PUFF_NOVA_WISP {
        // Flung out of a supernova: it coasts on what it was given, slowing, and rises a little.
        let t = life_age * p.life;
        pos = p.pos + p.vel * ((1.0 - exp(-PUFF_NOVA_WISP_DRAG * t)) / PUFF_NOVA_WISP_DRAG) + vec3<f32>(0.0, 0.0, 0.8 * t);
    }
    let center = globals.view_proj * vec4<f32>(pos, 1.0);
    let px = max(size * globals.lod.x / max(center.w, 1.0), 2.5);
    var offset = corner * px;
    if kind == PUFF_NOVA_WISP {
        // Drawn out along the way it flies while it is fast, so it reads as a streamer: `uv.x`
        // runs along its flight.
        let t = life_age * p.life;
        let ahead = globals.view_proj * vec4<f32>(pos + p.vel * (exp(-PUFF_NOVA_WISP_DRAG * t) * 0.05), 1.0);
        let along = (ahead.xy / ahead.w - center.xy / center.w) / globals.viewport.zw;
        let dir = select(vec2<f32>(1.0, 0.0), normalize(along), length(along) > 1e-3);
        let stretch = 1.0 + 1.0 * exp(-1.4 * t);
        offset = (dir * corner.x * stretch + vec2<f32>(-dir.y, dir.x) * corner.y) * px;
    }
    let ndc = center.xy / center.w + offset * globals.viewport.zw;
    // A burst on a hull stands in front of it: it is the hit, not something inside it.
    // A star stands at its face: rings passing in front of it hide it.
    let lift = select(select(0.5, 0.9, kind == PUFF_PLASMA_BURST), 0.42, kind == PUFF_STAR_CORE);
    let depth = front_depth(pos, size * lift);
    out.clip = vec4<f32>(ndc * center.w, depth * center.w, center.w);
    out.world = effect_billboard_world(pos, corner, size);
    return out;
}

fn plasma_puff_color(in: PuffOut, d: f32) -> vec4<f32> {
    if d > 1.0 {
        discard;
    }
    if u32(in.state.y) == PUFF_PLASMA_ORB {
        return plasma_orb(in, d);
    }
    if u32(in.state.y) == PUFF_PLASMA_GLOB {
        return plasma_glob(in, d);
    }
    if u32(in.state.y) == PUFF_PLASMA_WAKE {
        return plasma_wake(in, d);
    }
    if u32(in.state.y) == PUFF_STAR_CORE {
        return star_core(in, d);
    }
    if u32(in.state.y) == PUFF_SUPERNOVA {
        return supernova(in, d);
    }
    if u32(in.state.y) == PUFF_NOVA_WISP {
        return nova_wisp(in, d);
    }
    return plasma_burst(in, d);
}

// A charge: a ball of plasma held in the pinch. Its skin boils, its face churns in hotter
// and cooler cells drifting across it, a soft corona flickers round it and a white heart
// burns at its middle. Laid once a tick, each lasting two, so its brightness is a tent
// over its life and the overlapping ones add up to a steady ball.
fn plasma_orb(in: PuffOut, d: f32) -> vec4<f32> {
    let now = globals.camera.w;
    let seed = in.state.z;
    let age = in.state.x;
    let rgb = in.appearance.rgb;
    let fusion = clamp(in.appearance.w, 0.0, 1.0);
    let tent = 1.0 - abs(age * 2.0 - 1.0);
    let angle = atan2(in.uv.y, in.uv.x);
    // The skin: a radius that boils, faster as it goes over to fusion.
    let round = vec2<f32>(cos(angle), sin(angle)) * 1.6 + seed * 9.0;
    let boil = grad_noise2(round + vec2<f32>(now * mix(5.0, 11.0, fusion), 0.0), 1.0);
    let skin = 0.5 + 0.12 * (boil - 0.5);
    let ball = 1.0 - smoothstep(skin - 0.06, skin + 0.02, d);
    // Its face: cells of plasma drifting across it two ways at once, seen on a ball (the
    // cells crowd toward its edge).
    let bulge = 1.0 + 0.8 * smoothstep(0.0, skin, d);
    let drift = vec2<f32>(now, -now * 0.7) * mix(0.9, 2.4, fusion);
    let cell_a = value_noise2(in.uv * 5.0 * bulge + drift + seed * 17.0, 1.0);
    let cell_b = value_noise2(in.uv * 11.0 * bulge - drift * 1.6 + seed * 5.0, 1.0);
    let cells = cell_a * 0.65 + cell_b * 0.35;
    let limb = smoothstep(skin * 0.45, skin, d) * ball;
    let face = ball * (0.35 + 1.1 * cells * cells) + limb * 0.5;
    // The corona: soft, flickering in lumps that come and go, not in arms.
    let flare = value_noise2(in.uv * 3.2 + vec2<f32>(seed * 7.0, now * 2.6), 1.0);
    let halo = pow(max(1.0 - d, 0.0), 2.4) * (0.45 + 1.0 * flare * flare) * (1.0 - ball * 0.5);
    let heart = exp(-d * d / mix(0.012, 0.034, fusion));
    let level = length(rgb);
    let white = vec3<f32>(level * 0.75);
    // Fusion: the prism's pinks drift over it in patches, brightest round its rim.
    let chroma = prism(cells * 0.9 + flare * 0.6 + now * PRISM_RATE + seed);
    let rim = exp(-pow((d - skin) / 0.06, 2.0));
    let body = mix(rgb, chroma * level * 0.9, fusion * 0.6);
    var light = body * (face * 0.6 + halo * 0.7)
        + mix(rgb * 0.8 + white * 0.6, white * 1.7, fusion) * heart * 3.0;
    light += chroma * level * rim * fusion * 2.2;
    // It shivers as it is squeezed: harder in fusion.
    let shiver = 0.88 + 0.12 * sin(now * 53.0 + seed * 31.0) + fusion * 0.2 * (hash11(floor(now * 24.0) + seed) - 0.5);
    return vec4<f32>(light * tent * shiver, 0.0);
}

// Where a plasma shot lets go: no ring and no dust, and no soft bloom. A hard-edged heart
// of plasma, white-hot, that shrinks back fast as it cools, and filaments of it torn out
// round it, thin and bright, crackling as they go out. In fusion it opens white with the
// prism's pinks in its threads, then cools back to red.
fn plasma_burst(in: PuffOut, d: f32) -> vec4<f32> {
    let seed = in.state.z;
    let age = in.state.x;
    let rgb = in.appearance.rgb;
    let fusion = clamp(in.appearance.w, 0.0, 1.0);
    // Its reach is ragged in lumps laid over the quad, not by angle, so nothing runs round it.
    let lump_a = value_noise2(in.uv * 2.3 + vec2<f32>(seed * 17.0, age * 1.4), 1.0);
    let lump_b = value_noise2(in.uv * 4.8 + vec2<f32>(-age * 2.2, seed * 9.0), 1.0);
    let lumps = lump_a * 0.62 + lump_b * 0.38;
    let field = 1.0 - d / mix(0.55, 1.0, lumps);
    if field <= 0.0 {
        discard;
    }
    let level = length(rgb);
    let white = vec3<f32>(level * 0.85);
    // The heart: a solid, sharp-edged body that falls back into itself as it cools.
    let heat = clamp(field * (1.35 - age * 1.7), 0.0, 1.0);
    let solid = smoothstep(0.3, 0.38, heat);
    let heart = mix(rgb * 1.2, white * 1.6, smoothstep(0.55, 0.85, heat)) * solid;
    // Filaments: ridges of a warped noise, thin bright threads torn out of the heart.
    let warp = value_noise2(in.uv * 3.0 + vec2<f32>(seed * 5.0, age), 1.0);
    let ridge_a = 1.0 - abs(value_noise2(in.uv * 4.5 + warp * 1.6 + vec2<f32>(seed * 17.0, age * 2.5), 1.0) * 2.0 - 1.0);
    let ridge_b = 1.0 - abs(value_noise2(in.uv * 9.0 - warp * 1.2 + vec2<f32>(-age * 3.5, seed * 7.0), 1.0) * 2.0 - 1.0);
    let threads = pow(ridge_a, 12.0) + 0.6 * pow(ridge_b, 16.0);
    // They go out in pieces, each on its own, from the edge in.
    let grain = value_noise2(in.uv * 7.0 + vec2<f32>(seed * 31.0, 0.0), 1.0);
    let alive = smoothstep(age * 1.2 - 0.02, age * 1.2 + 0.04, grain * 0.6 + field * 0.6);
    let thread_rgb = mix(rgb * 1.4, white * 1.2, smoothstep(0.4, 0.9, field) * (1.0 - age));
    var c = heart + thread_rgb * threads * smoothstep(0.0, 0.12, field) * alive * 2.4;
    // Fusion: the prism in its threads while it is hot.
    c += prism(lumps * 1.3 + warp * 0.5 + seed) * level * threads * fusion * (1.0 - smoothstep(0.1, 0.6, age)) * 1.6;
    return vec4<f32>(c * pow(1.0 - age, 1.3), 0.0);
}

// Plasma thrown up or out of a blast or a gun's mouth: a ragged clump of it, hot at its
// heart while fresh and red in its body, with a sharp edge. It does not spread into a
// haze: as it cools it is eaten through in holes from the edge in, hard-edged, until it
// is gone. Gone over to fusion, its body takes the prism's pinks while it is hot.
fn plasma_wake(in: PuffOut, d: f32) -> vec4<f32> {
    let seed = in.state.z;
    let age = in.state.x;
    let rgb = in.appearance.rgb;
    let fusion = clamp(in.appearance.w, 0.0, 1.0);
    let lumps = value_noise2(in.uv * 2.8 + vec2<f32>(seed * 23.0, age * 1.8), 1.0);
    let field = 1.0 - d / mix(0.55, 1.0, lumps);
    if field <= 0.0 {
        discard;
    }
    // Eaten through as it cools.
    let grain = value_noise2(in.uv * 6.0 + vec2<f32>(seed * 13.0, age * 0.8), 1.0);
    let alive = smoothstep(-0.02, 0.03, grain * 0.55 + field * 0.75 - age * 1.05 - 0.05);
    if alive <= 0.001 {
        discard;
    }
    let level = length(rgb);
    let heat = 1.0 - smoothstep(0.0, 0.6, age);
    let ember = vec3<f32>(level * 0.45, level * 0.025, level * 0.012);
    let churn = value_noise2(in.uv * 5.0 + vec2<f32>(-age * 3.0, seed * 11.0), 1.0);
    let hue = prism(lumps + churn * 0.4 + seed + age * 0.6);
    var c = mix(ember, rgb * (0.75 + 0.6 * churn), heat);
    c = mix(c, hue * level * (0.7 + 0.6 * churn), fusion * fusion * heat * 0.75);
    c += vec3<f32>(level * 0.9, level * 0.8, level * 0.8) * pow(field, 3.0) * (1.0 - smoothstep(0.0, 0.3, age));
    // Solid through: brightest at its heart, but no soft falloff to a glow at its rim.
    let body = 0.45 + 0.55 * smoothstep(0.0, 0.5, field);
    return vec4<f32>(c * body * alive * pow(1.0 - age, 1.2), 0.0);
}

// A thrown glob: a soft red blob, its heart hot pink-white while it is fresh, cooling to
// a deep red ember as it slows.
fn plasma_glob(in: PuffOut, d: f32) -> vec4<f32> {
    let age = in.state.x;
    let rgb = in.appearance.rgb;
    let level = length(rgb);
    let body = pow(max(1.0 - d, 0.0), 1.6);
    let heart = pow(max(1.0 - d * 2.2, 0.0), 2.0) * (1.0 - smoothstep(0.0, 0.6, age));
    let ember = vec3<f32>(level * 0.45, level * 0.025, level * 0.012);
    let c = mix(rgb, ember, smoothstep(0.2, 0.9, age)) * body
        + vec3<f32>(level * 0.8, level * 0.5, level * 0.48) * heart;
    return vec4<f32>(c * (1.0 - age * age), 0.0);
}

// Value noise in 3D, 0 to 1, one feature a unit: the star's face, which turns. Its corners
// are hashed as integers: `fract(sin(...))` on a lattice index loses its grain on the GPU a
// few cells from the origin and the noise goes to flat blocks.
fn star_hash3(i: vec3<f32>) -> f32 {
    var n = u32(i32(i.x)) * 1597334677u ^ u32(i32(i.y)) * 3812015801u ^ u32(i32(i.z)) * 2798796415u;
    n = n * 747796405u + 2891336453u;
    n = ((n >> ((n >> 28u) + 4u)) ^ n) * 277803737u;
    n = (n >> 22u) ^ n;
    return f32(n >> 8u) * (1.0 / 16777216.0);
}

fn star_noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = p - i;
    let u = f * f * (3.0 - 2.0 * f);
    let x0 = mix(star_hash3(i), star_hash3(i + vec3<f32>(1.0, 0.0, 0.0)), u.x);
    let x1 = mix(star_hash3(i + vec3<f32>(0.0, 1.0, 0.0)), star_hash3(i + vec3<f32>(1.0, 1.0, 0.0)), u.x);
    let x2 = mix(star_hash3(i + vec3<f32>(0.0, 0.0, 1.0)), star_hash3(i + vec3<f32>(1.0, 0.0, 1.0)), u.x);
    let x3 = mix(star_hash3(i + vec3<f32>(0.0, 1.0, 1.0)), star_hash3(i + vec3<f32>(1.0, 1.0, 1.0)), u.x);
    return mix(mix(x0, x1, u.y), mix(x2, x3, u.y), u.z);
}

fn star_fbm3(p: vec3<f32>) -> f32 {
    return star_noise3(p) * 0.55 + star_noise3(p * 2.03 + 7.1) * 0.28 + star_noise3(p * 4.1 + 13.7) * 0.17;
}

// A Regency power generator's star (renderer/star_core_fx.rs): a ball of fusing plasma, not a
// solid. Its face boils in cells that drift and turn, white-hot at the heart and breaking into the
// prism's pinks toward the rim, its edge soft and never quite still; bright veins crackle
// across it and flash out; a ragged corona of streamers flickers off it; every few seconds
// it flares. Laid once a tick, each lasting two, so its light is a tent over its life and
// the overlapping ones add up to a steady star. `appearance.w` the star's own seed.
fn star_core(in: PuffOut, d: f32) -> vec4<f32> {
    let now = globals.camera.w;
    let seed = in.appearance.w;
    let tent = 1.0 - abs(in.state.x * 2.0 - 1.0);
    let face_r = 0.42;
    let angle = atan2(in.uv.y, in.uv.x);
    let round = vec2<f32>(cos(angle), sin(angle));
    // Its beat: a slow swell, and a sharp flare every few seconds that settles back.
    let beat = fract(now * 0.29 + seed * 0.37);
    let flare = exp(-beat * 8.0);
    let swell = 1.0 + 0.04 * sin(now * 1.3 + seed) + 0.06 * flare;
    // The edge boils: never a clean circle.
    let edge_noise = star_fbm3(vec3<f32>(round * 2.2, now * 0.9 + seed * 5.0));
    let rim = face_r * swell * (0.95 + 0.1 * edge_noise);
    // How far over it is: 0 on the face, 1 at the quad's edge.
    let out_t = clamp((d - rim) / (1.0 - rim), 0.0, 1.0);
    // The face, as a turning sphere under the eye.
    let q = in.uv / rim;
    let mu = sqrt(max(1.0 - dot(q, q), 0.0));
    let turn = now * 0.12 + seed;
    let ct = cos(turn);
    let st = sin(turn);
    let n = vec3<f32>(q.x * ct - mu * st, q.y, q.x * st + mu * ct);
    // Cells: one field warped by another, both drifting, so they boil.
    let warp = star_fbm3(n * 1.8 + vec3<f32>(0.0, 0.0, now * 0.35));
    let cells = star_fbm3(n * 3.6 + warp * 1.7 + vec3<f32>(now * 0.22, -now * 0.17, seed));
    // Veins: thin ridges of the noise, flashing on and off in their own places.
    let ridge = 1.0 - abs(star_noise3(n * 5.5 + vec3<f32>(seed * 3.0, now * 0.6, 0.0)) * 2.0 - 1.0);
    let flash = step(0.55, hash11(floor(now * 9.0) + seed * 17.0 + floor(angle * 2.0)));
    let vein = pow(ridge, 14.0) * (0.35 + 0.65 * flash);
    // Colour: the prism's pinks, drifting over the face and round the rim.
    // Round the rim by its direction, not its angle: an angle jumps where it wraps.
    let hue = prism(seed + now * PRISM_RATE * 0.4 + cells * 0.5 + round.x * 0.25 + round.y * 0.15);
    let deep = mix(hue, hue * hue * hue, 0.75);
    let white = vec3<f32>(1.0, 0.95, 0.98);
    let heat = clamp(mu * 0.75 + (cells - 0.5) * 0.9 + 0.2, 0.0, 1.0);
    var face = mix(deep * 1.3, white * 1.6, smoothstep(0.8, 1.08, heat));
    // Dark lanes between the cells keep it a body, not a flat disc.
    face *= 0.55 + 0.75 * smoothstep(0.25, 0.75, cells);
    // The limb: brighter and more coloured toward the edge, as light through a bubble.
    face += deep * pow(1.0 - mu, 2.5) * 1.2;
    face += mix(white, hue, 0.3) * vein * 2.0;
    let on_face = 1.0 - smoothstep(rim * 0.9, rim * 1.02, d);
    // The corona: streamers off the rim that drift outward and flicker, the flare
    // throwing them further.
    let streams = star_fbm3(vec3<f32>(round * 3.0, out_t * 2.2 - now * 0.7 + seed * 3.0));
    let licks = star_fbm3(vec3<f32>(round * 7.0 + 3.0, out_t * 4.0 - now * 1.6 + seed));
    let reach = mix(3.6, 2.2, flare);
    let corona = exp(-out_t * reach) * (0.1 + 2.6 * pow(streams, 3.0) + 1.4 * pow(licks, 4.0))
        + exp(-out_t * 14.0) * 1.3;
    let flicker = 0.88 + 0.12 * sin(now * 11.0 + seed * 7.0 + angle * 3.0);
    let glow_hue = prism(seed + now * PRISM_RATE * 0.4 + round.x * 0.5 + round.y * 0.3 + out_t * 0.6);
    let halo = mix(glow_hue, glow_hue * glow_hue * glow_hue, 0.7) * corona * flicker * 0.9
        * (1.0 - smoothstep(0.7, 1.0, d));
    let light = (face * on_face + halo * (1.0 - on_face)) * (1.0 + flare * 0.5);
    return vec4<f32>(light * in.appearance.rgb * tent, 0.0);
}

// A supernova's shell is a real ball of plasma `size` metres across its outer face, drawn in
// two halves so what stands in it hides what it should (renderer/supernova_fx.rs): the near
// half (`vel.x` 0) on a quad at the ball's front, so whatever stands in front of the ball
// hides it; the far half (`vel.x` 1) through its middle, so a hull or hill inside the shell
// hides the wall behind it. Either quad covers the ball's outline as the eye sees it. The
// shell's centre goes to the fragment in `roll`, its radius in `cloud_size` (negative for the
// far half).
fn supernova_vertex(p: Puff, corner: vec2<f32>, r: f32, o: PuffOut) -> PuffOut {
    var out = o;
    let eye = globals.camera.xyz;
    let to_eye = eye - p.pos;
    let dist = max(length(to_eye), 0.001);
    let toward = to_eye / dist;
    let far = p.vel.x > 0.5;
    var right = cross(vec3<f32>(0.0, 0.0, 1.0), toward);
    if length(right) < 0.05 {
        right = vec3<f32>(globals.view_proj[0].x, globals.view_proj[1].x, globals.view_proj[2].x);
    }
    right = normalize(right);
    let up = cross(toward, right);
    // The outline seen from `dist`, at the plane through the middle; at the front it is
    // no wider than the ball.
    let outline = r * dist / sqrt(max(dist * dist - r * r, 0.09 * dist * dist));
    let half = select(r, outline, far) * 1.08;
    let plane = p.pos + toward * select(min(r, dist * 0.5), 0.0, far);
    let world = plane + (right * corner.x + up * corner.y) * half;
    out.world = world;
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    out.roll = p.pos;
    out.cloud_size = select(r, -r, far);
    return out;
}

// A Regency power generator's star gone supernova: a hollow shell of plasma tearing outward,
// marched through as a volume, so it is brightest at its limb where the eye looks along it,
// its face lit and its middle hollow; the ground cuts it where it goes under. It tears into
// knots and strands as it goes and opens into holes as it thins, its edge ragged. White-hot
// at first, then the prism's pinks drifting over it, cooling to lavender and violet as it goes
// out: no reds, nothing in it burns. The hot flash fills it for a moment at the start.
// `appearance.w` its seed.
fn supernova(in: PuffOut, d: f32) -> vec4<f32> {
    let age = in.state.x;
    let seed = fract(in.appearance.w * 0.137) * 10.0;
    let centre = in.roll;
    let r = abs(in.cloud_size);
    let far = in.cloud_size < 0.0;
    let eye = globals.camera.xyz;
    let ray = normalize(in.world - eye);
    let oc = eye - centre;
    let along = dot(oc, ray);
    let miss = dot(oc, oc) - along * along;
    // The ragged outer face reaches past `r` a little.
    let outer = r * 1.06;
    if miss >= outer * outer {
        discard;
    }
    let reach = sqrt(outer * outer - miss);
    let t_out = vec2<f32>(max(-along - reach, 0.0), -along + reach);
    // The hollow inside it; the shell thickens as it goes.
    let inner = 1.0 - mix(0.1, 0.28, age);
    let ri = r * inner * 0.93;
    var seg = t_out;
    if miss < ri * ri {
        let hollow = sqrt(ri * ri - miss);
        seg = select(vec2<f32>(t_out.x, max(-along - hollow, t_out.x)), vec2<f32>(-along + hollow, t_out.y), far);
        if far {
            // The ground across the hollow hides the far wall.
            for (var k = 1; k <= 3; k++) {
                let q = eye + ray * mix(-along - hollow, seg.x, f32(k) * 0.25);
                if q.z < max(terrain_height(q.xy), globals.map.z) {
                    discard;
                }
            }
        }
    } else if far {
        discard;
    }
    let steps = 8;
    let dt = (seg.y - seg.x) / f32(steps);
    if dt <= 0.0 {
        discard;
    }
    let jitter = hash21(floor(in.clip.xy) + vec2<f32>(seed * 17.0, 3.0));
    let white = vec3<f32>(1.0, 0.95, 0.98);
    var light = vec3<f32>(0.0);
    for (var i = 0; i < steps; i++) {
        let p = eye + ray * (seg.x + (f32(i) + jitter) * dt);
        // The ground and the sea hide what is under them.
        if p.z < max(terrain_height(p.xy), globals.map.z) {
            break;
        }
        let rel = (p - centre) / r;
        let q = length(rel);
        let n = rel / max(q, 1e-4);
        let rag = star_noise3(n * 2.5 + vec3<f32>(seed, 0.0, age * 1.5));
        let rr = q / (0.93 + 0.12 * rag);
        let shell = smoothstep(inner - 0.04, inner + 0.06, rr) * (1.0 - smoothstep(0.9, 1.0, rr));
        if shell <= 0.0 {
            continue;
        }
        // Knots and strands through it.
        let knots = star_fbm3(n * 4.0 + vec3<f32>(seed * 3.0, age * 2.0, 0.0));
        let strands = pow(1.0 - abs(star_noise3(n * 7.0 + vec3<f32>(0.0, seed * 5.0, age * 3.0)) * 2.0 - 1.0), 6.0);
        // Torn open as it thins: the weaker knots go out first.
        let torn = smoothstep(0.25 * age, 0.25 * age + 0.3, knots);
        let body = shell * (0.1 + 1.6 * knots * knots + 1.1 * strands) * mix(1.0, torn, age);
        let hue = prism(seed + knots * 0.6 + n.x * 0.2 + n.y * 0.12 + age * 0.4);
        let violet = mix(vec3<f32>(0.62, 0.42, 1.0), vec3<f32>(0.85, 0.5, 1.0), strands);
        var colour = mix(white * 1.5, hue * 1.2, smoothstep(0.02, 0.2, age));
        colour = mix(colour, violet, smoothstep(0.35, 0.9, age));
        light += colour * body * dt;
    }
    // A path along the limb is about 0.9 of the radius long: there it is as bright as its body.
    light /= r * 0.9;
    if !far {
        // The flash filling it, seen through the whole ball.
        light += white * exp(-3.0 * miss / (r * r)) * pow(1.0 - age, 14.0) * 2.5;
    }
    let fade = pow(1.0 - age, 1.3) * smoothstep(0.0, 0.02, age);
    return vec4<f32>(light * in.appearance.rgb * fade, 0.0);
}

// A streamer of a supernova's plasma (renderer/supernova_fx.rs), drawn out along its flight
// (`uv.x`): strands running its length, a white-hot heart while it is fresh, the prism's
// pinks, cooling to lavender and violet as it slows and goes out. Never red: it is the
// star's light, not fire. `appearance.rgb` its brightness.
fn nova_wisp(in: PuffOut, d: f32) -> vec4<f32> {
    let seed = in.state.z;
    let age = in.state.x;
    let lumps = value_noise2(in.uv * 2.6 + vec2<f32>(seed * 23.0, age * 1.5), 1.0);
    let field = 1.0 - d / mix(0.6, 1.0, lumps);
    if field <= 0.0 {
        discard;
    }
    let strands = value_noise2(vec2<f32>(in.uv.x * 1.2 - age * 2.0, in.uv.y * 7.0) + seed * 13.0, 1.0);
    let body = pow(field, 1.3) * (0.35 + 1.1 * strands * strands);
    let white = vec3<f32>(1.0, 0.95, 0.98);
    let hue = prism(seed + lumps * 0.5 + age * 0.5);
    let violet = vec3<f32>(0.6, 0.4, 1.0);
    var c = mix(white, hue, smoothstep(0.0, 0.2, age));
    c = mix(c, violet, smoothstep(0.35, 0.9, age));
    let heart = pow(field, 4.0) * (1.0 - smoothstep(0.0, 0.25, age));
    let fade = pow(1.0 - age, 1.7) * smoothstep(0.0, 0.04, age);
    return vec4<f32>((c * body + white * heart) * in.appearance.rgb * fade, 0.0);
}
