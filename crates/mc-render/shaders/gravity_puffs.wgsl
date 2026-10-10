// A gravitic engine's held energy as light (renderer/gravitic_engine_fx): a column of it
// standing between the shards, never a ball. Prepended to puffs.wgsl with
// `//!use gravity_puffs` (it uses that file's `Puff` and `PuffOut`).
//
// `PUFF_GRAVITY_VORTEX`: from `pos` to `pos + vel`, turned about its own axis to face the
// eye. Strands of violet light twist round a white-hot thread, the column pinched at its
// waist and frayed toward its ends, faint and turning slowly while the engine idles,
// wound tight and whipping round as it fills (`appearance.w`). `appearance.rgb` its
// colour and brightness. Light added, nothing hidden: laid once a tick, two ticks long,
// its brightness a tent over its life so the overlapping ones add up to a steady column.

fn is_gravity_puff(kind: u32) -> bool {
    return kind == PUFF_GRAVITY_VORTEX;
}

fn gravity_puff_vertex(p: Puff, corner: vec2<f32>, age: f32, o: PuffOut) -> PuffOut {
    var out = o;
    out.state = vec3<f32>(age, p.params.z, fract(p.params.w));
    out.uv = corner;
    let span = max(length(p.vel), 0.01);
    let axis = p.vel / span;
    let mid = p.pos + p.vel * 0.5;
    let to_eye = normalize(globals.camera.xyz - mid);
    var right = cross(axis, to_eye);
    if dot(right, right) < 1e-6 {
        right = cross(axis, vec3<f32>(1.0, 0.0, 0.0));
    }
    right = normalize(right);
    // At least a few pixels across, so it still reads from far out.
    let center = globals.view_proj * vec4<f32>(mid, 1.0);
    let half = max(p.params.x, 2.0 * max(center.w, 1.0) / max(globals.lod.x, 1.0));
    let world = mid + right * corner.x * half + axis * corner.y * span * 0.5;
    out.world = world;
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    return out;
}

fn gravity_puff_color(in: PuffOut) -> vec4<f32> {
    let now = globals.camera.w;
    let seed = in.state.z;
    let tent = 1.0 - abs(in.state.x * 2.0 - 1.0);
    let wound = clamp(in.appearance.w, 0.0, 1.0);
    let rgb = in.appearance.rgb;
    let h = in.uv.y * 0.5 + 0.5;
    // Pinched at its waist, frayed toward its ends, which fade out.
    let ends = smoothstep(0.0, 0.2, h) * (1.0 - smoothstep(0.8, 1.0, h));
    let waist = 0.32 + 0.6 * pow(abs(h - 0.5) * 2.0, 1.4);
    // The whole column shivers sideways in waves running up it.
    let shiver = (grad_noise2(vec2<f32>(h * 5.0 - now * (1.0 + 3.0 * wound), seed * 9.0), 1.0) - 0.5) * 0.14;
    let x = in.uv.x - shiver;
    let spin = now * (0.6 + 5.5 * wound);
    let twist = 6.2831853 * (1.0 + 1.6 * wound);
    var strands = 0.0;
    var front = 0.0;
    for (var i = 0; i < 7; i++) {
        let fi = f32(i);
        let k = hash11(fi * 7.31 + seed * 13.0);
        let theta = twist * h * (0.8 + 0.4 * k) + spin * (1.0 + 0.25 * fi) + fi * 0.8975979 + seed * 6.0;
        let r = waist * (0.55 + 0.45 * k);
        let at = sin(theta) * r;
        let facing = cos(theta);
        let width = 0.035 + 0.035 * (0.5 + 0.5 * facing);
        let line = exp(-pow((x - at) / width, 2.0));
        strands += line * (0.45 + 0.55 * (0.5 + 0.5 * facing));
        front = max(front, line * step(0.0, facing));
    }
    // The white-hot thread down its middle, flickering along its length.
    let flick = 0.6 + 0.4 * grad_noise2(vec2<f32>(h * 9.0 + now * 4.0, seed * 3.0), 1.0);
    let thread = exp(-pow(x / (0.035 + 0.05 * wound), 2.0)) * flick;
    // A soft sheath of violet round all of it.
    let sheath = exp(-pow(x / max(waist, 0.05), 2.0));
    let violet = vec3<f32>(0.55, 0.12, 1.0);
    let lilac = vec3<f32>(0.85, 0.65, 1.0);
    let white = vec3<f32>(1.0, 0.95, 1.0);
    var light = violet * sheath * 0.22 + mix(violet, lilac, front) * strands * 0.8
        + white * thread * (0.6 + 0.8 * wound);
    // Where strands cross, they burn white.
    light += white * max(strands - 1.2, 0.0) * 0.5;
    let fade = tent * ends * (1.0 - smoothstep(0.92, 1.0, abs(in.uv.x)));
    return vec4<f32>(light * rgb * fade, 0.0);
}
