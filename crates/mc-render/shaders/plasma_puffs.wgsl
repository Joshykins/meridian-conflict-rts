// The Regency's squeezed plasma as light (renderer/regency_guns_fx.rs): the charge a
// Pinched or Pinch-fusion gun gathers in front of its bore, the burst where a plasma shot
// lets go, and the globs of plasma it throws. Prepended to puffs.wgsl with `//!use plasma_puffs` (it uses that file's
// `Puff` and `PuffOut`).
//
// Both are light added to the scene. `appearance.rgb` is the plasma's colour, brightness
// in its size; `appearance.w` how far it has gone over to fusion: 0 red plasma, 1 a
// fusion knot, white at the heart with every colour running round its rim.

fn is_plasma_puff(kind: u32) -> bool {
    return kind == PUFF_PLASMA_ORB || kind == PUFF_PLASMA_BURST || kind == PUFF_PLASMA_GLOB;
}

// A colour off the wheel, `h` 0 to 1 round it: the fusion rim's.
fn plasma_hue(h: f32) -> vec3<f32> {
    return clamp(abs(fract(h + vec3<f32>(0.0, 0.667, 0.333)) * 6.0 - 3.0) - 1.0, vec3<f32>(0.0), vec3<f32>(1.0));
}

// Turned to the eye, where it was born; a burst is drawn a little toward the eye so the
// ground it stands on does not cut it in half.
fn plasma_puff_vertex(p: Puff, corner: vec2<f32>, age: f32, o: PuffOut) -> PuffOut {
    var out = o;
    let kind = u32(p.params.z);
    // A charge grows steadily; a burst throws itself out fast and slows.
    let grow = select(age, 1.0 - pow(1.0 - age, 3.0), kind == PUFF_PLASMA_BURST);
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
    let center = globals.view_proj * vec4<f32>(pos, 1.0);
    let px = max(size * globals.lod.x / max(center.w, 1.0), 2.5);
    let ndc = center.xy / center.w + corner * px * globals.viewport.zw;
    // A burst on a hull stands in front of it: it is the hit, not something inside it.
    let lift = select(0.5, 0.9, kind == PUFF_PLASMA_BURST);
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
    return plasma_burst(in, d);
}

// A charge: a ball of plasma held in the pinch, its skin boiling, filaments wound in
// toward a white heart. Laid once a tick, each lasting two, so its brightness is a tent
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
    // Filaments wound in on the heart.
    let wind = sin(angle * 5.0 + log(max(d, 0.02)) * 7.0 - now * mix(9.0, 16.0, fusion) + seed * 6.0);
    let filament = pow(max(wind, 0.0), 6.0) * smoothstep(0.08, 0.3, d) * (1.0 - smoothstep(skin, 0.95, d));
    let halo = pow(max(1.0 - d, 0.0), 2.6);
    let heart = exp(-d * d / mix(0.012, 0.03, fusion));
    // Fusion: every colour running round the rim, the heart white.
    let rim = exp(-pow((d - skin) / 0.05, 2.0));
    let chroma = plasma_hue(angle / 6.28318 + now * 0.9 + d * 2.0 + seed);
    let level = length(rgb);
    let white = vec3<f32>(level * 0.75);
    // In fusion the halo and the filaments run with every colour too, not only the rim.
    let body = mix(rgb, chroma * level * 0.9, fusion * 0.65);
    var light = body * (ball * 0.55 + halo * 0.6 + filament * 1.4)
        + mix(rgb * 0.8 + white * 0.6, white * 1.6, fusion) * heart * 3.0;
    light += chroma * level * rim * fusion * 3.0;
    // It shivers as it is squeezed: harder in fusion.
    let shiver = 0.88 + 0.12 * sin(now * 53.0 + seed * 31.0) + fusion * 0.2 * (hash11(floor(now * 24.0) + seed) - 0.5);
    return vec4<f32>(light * tent * shiver, 0.0);
}

// Where a plasma shot lets go: no ring and no dust, a ragged bloom of plasma thrown out in
// licking fronds from a white-hot heart, red at its edges, that breaks up into glowing
// shreds and goes out. In fusion it opens white with every colour in its fringe, then
// cools back to red.
fn plasma_burst(in: PuffOut, d: f32) -> vec4<f32> {
    let seed = in.state.z;
    let age = in.state.x;
    let rgb = in.appearance.rgb;
    let fusion = clamp(in.appearance.w, 0.0, 1.0);
    let angle = atan2(in.uv.y, in.uv.x);
    let a = vec2<f32>(cos(angle), sin(angle)) * (2.0 + seed * 3.0);
    // Fronds: the edge reaches out further at some angles than others, and they writhe.
    let frond = grad_noise2(a + vec2<f32>(seed * 17.0, age * 3.0), 1.0);
    let reach = mix(0.45, 0.98, frond);
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
    // Licks of brighter plasma running out along the fronds.
    let lick = smoothstep(0.55, 0.9, grad_noise2(a * 2.5 + vec2<f32>(d * 6.0 - age * 4.0, seed * 5.0), 1.0));
    var c = mix(ember, rgb * (1.0 + lick * 0.8), smoothstep(0.05, 0.35, heat));
    c = mix(c, white * 1.3, smoothstep(0.7, 0.98, heat) * (1.0 - smoothstep(0.0, 0.5, age)));
    // Fusion opens with every colour in its fringe; it cools back to red.
    let fringe = exp(-pow((field - 0.18) / 0.1, 2.0)) * (1.0 - smoothstep(0.1, 0.6, age));
    c += plasma_hue(angle / 6.28318 + seed + d) * level * fringe * fusion * 1.4;
    let fade = pow(1.0 - age, 1.5);
    // Mottled through, so it is a body of plasma and not a flat shape.
    let mottle = 0.5 + 0.5 * value_noise2(in.uv * 3.5 + vec2<f32>(seed * 13.0, -age * 3.0), 1.0);
    return vec4<f32>(c * mottle * smoothstep(0.0, 0.22, field) * torn * fade, 0.0);
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
