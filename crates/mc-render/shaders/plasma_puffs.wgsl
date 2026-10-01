// The Regency's squeezed plasma as light (renderer/regency_guns_fx.rs): the charge a
// Pinched or Pinch-fusion gun gathers in front of its bore, the burst where a plasma shot
// lets go, the globs of plasma it throws and the wake a shot leaves hanging behind it.
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
        || kind == PUFF_PLASMA_WAKE || kind == PUFF_STAR_CORE;
}

// Turned to the eye, where it was born; a burst is drawn a little toward the eye so the
// ground it stands on does not cut it in half.
fn plasma_puff_vertex(p: Puff, corner: vec2<f32>, age: f32, o: PuffOut) -> PuffOut {
    var out = o;
    let kind = u32(p.params.z);
    // A charge grows steadily; a burst throws itself out fast and slows; a wake swells.
    var grow = select(age, 1.0 - pow(1.0 - age, 3.0), kind == PUFF_PLASMA_BURST);
    if kind == PUFF_PLASMA_WAKE {
        grow = sqrt(age);
    }
    let size = mix(p.params.x, p.params.y, grow);
    out.state = vec3<f32>(age, p.params.z, p.params.w);
    out.uv = corner;
    let to_eye = normalize(globals.camera.xyz - p.pos);
    var pos = p.pos + to_eye * select(0.0, size * 0.35, kind == PUFF_PLASMA_BURST);
    if kind == PUFF_PLASMA_GLOB {
        // Thrown: the air takes its speed off, and it sags.
        let t = age * p.life;
        pos = p.pos + p.vel * ((1.0 - exp(-3.0 * t)) / 3.0) - vec3<f32>(0.0, 0.0, 2.5 * t * t);
    }
    if kind == PUFF_PLASMA_WAKE {
        // Left hanging: it drifts off on what it was given, slowing, and rises as it cools.
        let t = age * p.life;
        pos = p.pos + p.vel * ((1.0 - exp(-1.6 * t)) / 1.6) + vec3<f32>(0.0, 0.0, 1.2 * t);
    }
    let center = globals.view_proj * vec4<f32>(pos, 1.0);
    let px = max(size * globals.lod.x / max(center.w, 1.0), 2.5);
    let ndc = center.xy / center.w + corner * px * globals.viewport.zw;
    // A burst on a hull stands in front of it: it is the hit, not something inside it.
    // A star stands at its face: rings passing in front of it hide it.
    let lift = select(select(0.5, 0.9, kind == PUFF_PLASMA_BURST), 0.42, kind == PUFF_STAR_CORE);
    out.clip = vec4<f32>(ndc * center.w, front_depth(pos, size * lift) * center.w, center.w);
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

// Where a plasma shot lets go: no ring and no dust, a lumpy bloom of plasma billowing out
// from a white-hot heart, cells of hotter plasma churning through it, red at its edges,
// that breaks up into glowing shreds and goes out. In fusion it opens white with the
// prism's pinks in its fringe, then cools back to red.
fn plasma_burst(in: PuffOut, d: f32) -> vec4<f32> {
    let seed = in.state.z;
    let age = in.state.x;
    let rgb = in.appearance.rgb;
    let fusion = clamp(in.appearance.w, 0.0, 1.0);
    // Its edge billows in lumps laid over the quad, not by angle, so nothing runs round it.
    let lump_a = value_noise2(in.uv * 2.3 + vec2<f32>(seed * 17.0, age * 1.4), 1.0);
    let lump_b = value_noise2(in.uv * 4.8 + vec2<f32>(-age * 2.2, seed * 9.0), 1.0);
    let lumps = lump_a * 0.62 + lump_b * 0.38;
    let reach = mix(0.5, 1.0, lumps);
    let field = 1.0 - d / reach;
    // Shreds: as it ages the bloom tears into pieces that each go out on their own.
    let shred = value_noise2(in.uv * 6.0 + vec2<f32>(seed * 31.0, age * 2.0), 1.0);
    let torn = smoothstep(age * 1.1 - 0.1, age * 1.1 + 0.15, shred + field * 0.6);
    if field <= 0.0 || torn <= 0.001 {
        discard;
    }
    // Hot white heart early, the colour in the body, dark red at the edges and as it dies.
    let heat = clamp(field * (1.25 - age * 1.2), 0.0, 1.0);
    let level = length(rgb);
    let white = vec3<f32>(level * 0.8);
    let ember = vec3<f32>(level * 0.5, level * 0.03, level * 0.015);
    // Cells of hotter plasma churning through the body.
    let churn = value_noise2(in.uv * 5.5 + vec2<f32>(seed * 13.0, -age * 4.0), 1.0);
    var c = mix(ember, rgb * (0.7 + churn * 1.1), smoothstep(0.05, 0.35, heat));
    c = mix(c, white * 1.3, smoothstep(0.7, 0.98, heat) * (1.0 - smoothstep(0.0, 0.5, age)));
    // Fusion opens with the prism in its fringe; it cools back to red.
    let fringe = exp(-pow((field - 0.18) / 0.12, 2.0)) * (1.0 - smoothstep(0.1, 0.6, age));
    c += prism(lumps * 1.3 + churn * 0.5 + seed) * level * fringe * fusion * 1.4;
    let fade = pow(1.0 - age, 1.5);
    // Mottled through, so it is a body of plasma and not a flat shape.
    let mottle = 0.5 + 0.5 * value_noise2(in.uv * 3.5 + vec2<f32>(seed * 13.0, -age * 3.0), 1.0);
    return vec4<f32>(c * mottle * smoothstep(0.0, 0.22, field) * torn * fade, 0.0);
}

// A plasma shot's wake: a soft, lumpy puff of glowing plasma, white-hot at its heart while
// fresh, its colour in the body, cooling through deep red to nothing as it swells. Gone
// over to fusion, its body takes the prism's pinks in drifting patches while it is hot.
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
    let level = length(rgb);
    let body = pow(field, 1.4);
    let heat = 1.0 - smoothstep(0.0, 0.65, age);
    let ember = vec3<f32>(level * 0.45, level * 0.025, level * 0.012);
    let churn = value_noise2(in.uv * 5.0 + vec2<f32>(-age * 3.0, seed * 11.0), 1.0);
    let hue = prism(lumps + churn * 0.4 + seed + age * 0.6);
    var c = mix(ember, rgb * (0.75 + 0.6 * churn), heat);
    c = mix(c, hue * level * (0.7 + 0.6 * churn), fusion * fusion * heat * 0.75);
    c += vec3<f32>(level * 0.9, level * 0.8, level * 0.8) * pow(field, 4.0) * (1.0 - smoothstep(0.0, 0.3, age));
    c += hue * level * fusion * heat * field * 0.9;
    let fade = pow(1.0 - age, 1.6);
    return vec4<f32>(c * body * fade * (0.55 + 0.6 * lumps), 0.0);
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

// Value noise in 3D, 0 to 1, one feature a unit: the star's face, which turns.
fn star_noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = p - i;
    let u = f * f * (3.0 - 2.0 * f);
    let h = dot(i, vec3<f32>(1.0, 57.0, 113.0));
    let n = mix(
        mix(mix(hash11(h), hash11(h + 1.0), u.x), mix(hash11(h + 57.0), hash11(h + 58.0), u.x), u.y),
        mix(mix(hash11(h + 113.0), hash11(h + 114.0), u.x), mix(hash11(h + 170.0), hash11(h + 171.0), u.x), u.y),
        u.z,
    );
    return n;
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
    let hue = prism(seed + now * PRISM_RATE * 0.4 + cells * 0.5 + angle / 6.2831853 * 0.5);
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
    let glow_hue = prism(seed + now * PRISM_RATE * 0.4 + angle / 6.2831853 + out_t * 0.6);
    let halo = mix(glow_hue, glow_hue * glow_hue * glow_hue, 0.7) * corona * flicker * 0.9
        * (1.0 - smoothstep(0.7, 1.0, d));
    let light = (face * on_face + halo * (1.0 - on_face)) * (1.0 + flare * 0.5);
    return vec4<f32>(light * in.appearance.rgb * tent, 0.0);
}
