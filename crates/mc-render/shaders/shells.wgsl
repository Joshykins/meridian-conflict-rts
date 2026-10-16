// A solid shot's body as sprites.wgsl's `vs_missile` draws it (`MissileOut::surface`):
// a gravitic shock shell's crystal shard, and the surfaces of a cased seeker, a surge
// shell and a knotted shell. Put in front of shaders saying `//!use shells` (needs
// bindings).

// A cased seeker's charge and line work, as the strategic warhead's (nova.wgsl `NOVA_VIOLET`).
const SEEKER_RED: vec3<f32> = vec3<f32>(1.0, 0.035, 0.012);

// A gravitic shock shell's shard: half its length, metres, for a shot of size `size`.
fn gravitic_shard_half(size: f32) -> f32 {
    return clamp(size * 0.75, 1.2, 2.4);
}

struct ShardVertex {
    local: vec3<f32>,
    normal: vec3<f32>,
    surface: vec3<f32>,
}

// A point of a gravitic shard's waist: corner `k` (of eight, round from the top, turned
// `spin`) at `x` along it. Its section is a four-pointed star (every other corner drawn
// in), so it reads as a crystal, not a shell.
fn shard_corner(k: f32, x: f32, radius: f32, spin: f32) -> vec3<f32> {
    let a = k * 0.785398163 + spin;
    let r = radius * select(0.5, 1.0, (u32(k + 0.5) & 1u) == 0u);
    return vec3<f32>(x, cos(a) * r, sin(a) * r);
}

// Vertex `vertex` of a gravitic shard `half` metres half-long (`vs_missile`), turning about
// its length: a short faceted waist (vertices 0..47) between a long point forward and a
// shorter one back (48..95), eight facets each. `surface` as `MissileOut::surface`: x along
// it (-1 tail .. 1 nose), y round it (0..1), the piece (0 the waist, 1 the front, 2 the back).
fn gravitic_shard(vertex: u32, half: f32, seed: f32) -> ShardVertex {
    let radius = half * 0.3;
    let spin = globals.camera.w * 4.5 + seed * 1.7;
    let back = -0.14 * half;
    let front = 0.14 * half;
    var out: ShardVertex;
    var a: vec3<f32>;
    var b: vec3<f32>;
    var c: vec3<f32>;
    var at: vec3<f32>;
    var k: f32;
    var piece = 0.0;
    var around = 0.0;
    if vertex < 48u {
        let quad = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);
        k = f32(vertex / 6u);
        let corner = quad[vertex % 6u];
        a = shard_corner(k, back, radius, spin);
        b = shard_corner(k + 1.0, back, radius, spin);
        c = shard_corner(k + 1.0, front, radius, spin);
        let side = select(0.0, 1.0, corner == 1u || corner == 2u);
        at = shard_corner(k + side, select(back, front, corner >= 2u), radius, spin);
        around = (k + side) / 8.0;
    } else {
        let v = vertex - 48u;
        let nose = v < 24u;
        k = f32((v % 24u) / 3u);
        let x = select(back, front, nose);
        let tip = vec3<f32>(select(-0.6, 1.0, nose) * half, 0.0, 0.0);
        a = shard_corner(k, x, radius, spin);
        b = shard_corner(k + 1.0, x, radius, spin);
        c = tip;
        let corner = v % 3u;
        at = select(select(a, b, corner == 1u), tip, corner == 2u);
        around = select((k + f32(corner)) / 8.0, (k + 0.5) / 8.0, corner == 2u);
        piece = select(2.0, 1.0, nose);
    }
    var n = normalize(cross(b - a, c - a));
    if dot(n, (a + b + c) / 3.0) < 0.0 {
        n = -n;
    }
    out.local = at;
    out.normal = n;
    out.surface = vec3<f32>(at.x / half, around, piece);
    return out;
}

// A gravitic shard's surface at `s` (`MissileOut::surface`), its base colour and light: a
// deep violet crystal lit from within, brightest at its waist where the held gravity is,
// beating; its edges lilac, its points going white.
fn gravitic_shard_color(s: vec3<f32>) -> array<vec3<f32>, 2> {
    let deep = vec3<f32>(0.06, 0.012, 0.16);
    let violet = vec3<f32>(0.55, 0.12, 1.0);
    let lilac = vec3<f32>(0.85, 0.66, 1.0);
    let beat = 0.8 + 0.2 * sin(globals.camera.w * 22.0);
    let d = abs(fract(s.y * 8.0 + 1e-3) - 0.5);
    let edge = smoothstep(0.38, 0.5, d);
    let waist = exp(-s.x * s.x * 9.0);
    let tip = smoothstep(0.75, 1.0, abs(s.x));
    let inner = violet * (0.6 + 2.6 * waist) * beat;
    let glow = inner + lilac * edge * 2.2 + mix(lilac, vec3<f32>(1.0), 0.6) * tip * 3.0;
    return array<vec3<f32>, 2>(deep, glow);
}

// A cased seeker's surface at `s` (`MissileOut::surface`): its base colour and the red
// light of its line work and charge (added unlit). Graphite in steel courses, as the
// strategic warhead (nova.wgsl `nova_missile_color`): chevrons cut into the prow's facets,
// a ring at its foot, seams down every other edge, and the charge seen through the tail.
fn cased_seeker_color(s: vec3<f32>) -> array<vec3<f32>, 2> {
    let graphite = vec3<f32>(0.03, 0.03, 0.034);
    let steel = vec3<f32>(0.2, 0.205, 0.225);
    let pulse = 0.85 + 0.15 * sin(globals.camera.w * 4.0 + s.x * 6.0);
    let piece = u32(s.z + 0.5);
    if piece == 2u {
        // The tail: the charge, held, seen through it.
        return array<vec3<f32>, 2>(graphite, SEEKER_RED * 6.0 * pulse);
    }
    if piece == 3u {
        // A fin: plate, its tip edge steel.
        return array<vec3<f32>, 2>(select(graphite * 1.3, steel * 0.8, s.y > 0.85), vec3<f32>(0.0));
    }
    var base = graphite;
    if s.x < -0.94 || (s.x > -0.36 && s.x < -0.3) || (s.x > 0.2 && s.x < 0.25) {
        base = steel;
    }
    // Across one facet, 0 at its middle, 0.5 at its edges.
    let d = abs(fract(s.y * 8.0 + 1e-3) - 0.5);
    let w = 0.025;
    var line = 0.0;
    if piece == 1u {
        // Along the prow, 0 at its foot to 1 at its point.
        let t = (s.x - 0.4) / 0.6;
        // A chevron on every facet, its point forward, and a smaller one ahead of it.
        line = max(line, 1.0 - step(w * 1.6, abs(t - (0.42 - 0.5 * d))));
        line = max(line, (1.0 - step(w * 1.6, abs(t - (0.7 - 0.3 * d)))) * step(d, 0.3));
    } else {
        // A ring at the prow's foot, and seams down every other edge running back from it.
        line = max(line, 1.0 - step(w, abs(s.x - 0.34)));
        let edge = (u32(floor(s.y * 8.0 + 0.5)) & 1u) == 0u;
        line = max(line, step(0.46, d) * step(0.05, s.x) * step(s.x, 0.34) * select(0.0, 1.0, edge));
    }
    return array<vec3<f32>, 2>(base, SEEKER_RED * line * 4.0 * pulse);
}

// A surge shell's surface at `s` (`MissileOut::surface`), its base colour and light: graphite
// in steel courses, a crimson driving band near its base and a ring at the prow's foot, seams
// of crimson down every other edge between them, and its base burning with the charge it
// carries. The light beats with the pressure pulses running back down its plume.
fn surge_shell_color(s: vec3<f32>) -> array<vec3<f32>, 2> {
    let graphite = vec3<f32>(0.03, 0.03, 0.034);
    let steel = vec3<f32>(0.2, 0.205, 0.225);
    let crimson = vec3<f32>(1.0, 0.03, 0.04);
    let beat = 0.75 + 0.25 * sin(globals.camera.w * 31.4);
    let piece = u32(s.z + 0.5);
    if piece == 2u {
        return array<vec3<f32>, 2>(graphite, crimson * 7.0 * beat);
    }
    var base = graphite;
    if piece == 1u && s.x > 0.85 {
        // Its point, bright steel.
        base = steel;
    }
    if s.x < -0.92 || (s.x > 0.15 && s.x < 0.22) {
        base = steel;
    }
    let d = abs(fract(s.y * 8.0 + 1e-3) - 0.5);
    var line = 0.0;
    if piece == 0u {
        line = max(line, 1.0 - step(0.05, abs(s.x + 0.66)));
        line = max(line, 1.0 - step(0.025, abs(s.x - 0.24)));
        let edge = (u32(floor(s.y * 8.0 + 0.5)) & 1u) == 0u;
        line = max(line, step(0.46, d) * step(-0.6, s.x) * step(s.x, 0.22) * select(0.0, 1.0, edge));
    }
    return array<vec3<f32>, 2>(base, crimson * line * 5.0 * beat);
}

// A knotted shell's surface at `s` (`MissileOut::surface`), its base colour and light:
// graphite, a steel point and driving band, three red seams down it a third of a turn
// apart (the three knots it was gathered from), and its base burning red with the plasma
// it carries, beating fast.
fn knotted_shell_color(s: vec3<f32>) -> array<vec3<f32>, 2> {
    let graphite = vec3<f32>(0.028, 0.028, 0.034);
    let steel = vec3<f32>(0.22, 0.22, 0.25);
    let red = vec3<f32>(1.0, 0.06, 0.03);
    let beat = 0.8 + 0.2 * sin(globals.camera.w * 40.0);
    let piece = u32(s.z + 0.5);
    if piece == 2u {
        return array<vec3<f32>, 2>(graphite, mix(red, vec3<f32>(1.0, 0.6, 0.5), 0.25) * 8.0 * beat);
    }
    var base = graphite;
    if (piece == 1u && s.x > 0.8) || s.x < -0.93 || (s.x > -0.62 && s.x < -0.55) {
        base = steel;
    }
    var line = 0.0;
    if piece == 0u {
        let d = abs(fract(s.y * 3.0 + 0.04) - 0.5);
        line = step(0.47, d) * step(-0.9, s.x);
    }
    return array<vec3<f32>, 2>(base, red * line * 5.0 * beat);
}
