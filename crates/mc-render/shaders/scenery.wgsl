// Desert scenery the entity shader dresses by a face's pattern byte (gpu_consts.rs
// `scenery`): a juniper's shaggy bark, bedded red sandstone, and the dam's mass
// concrete with its lifts, block joints, streaks and the reservoir's mineral ring.
// Prepended after surface.wgsl (it uses its noise) to shaders with `//!use scenery`.

// The canyon's palette, linear albedo, as the desert terrain has it (desert.wgsl
// CANYON_*): keep the two in step so a fallen block matches the cliff it fell from
// and the dam's ring matches the ring on the rock.
const SCENERY_SUPAI: vec3<f32> = vec3<f32>(0.40, 0.165, 0.09);
const SCENERY_REDWALL: vec3<f32> = vec3<f32>(0.38, 0.20, 0.125);
const SCENERY_COCONINO: vec3<f32> = vec3<f32>(0.56, 0.46, 0.32);
const SCENERY_VARNISH: vec3<f32> = vec3<f32>(0.035, 0.026, 0.022);
const SCENERY_CRUST: vec3<f32> = vec3<f32>(0.56, 0.53, 0.47);

struct SceneryLook {
    albedo: vec3<f32>,
    roughness: f32,
}

// A juniper's bark: the broadleaf scan bleached silver-grey, torn into long
// loose strips along the stem (`along` metres up it, `around` metres round it).
fn shaggy_bark(scan: vec3<f32>, around: f32, along: f32) -> vec3<f32> {
    let grey = dot(scan, vec3<f32>(0.3, 0.59, 0.11));
    let strip = floor(around * 9.0 + 0.6 * sin(along * 1.3));
    let tone = hash11(strip * 1.37 + 0.5);
    let loose = smoothstep(0.35, 0.95, surf_noise3(vec3<f32>(strip, along * 0.9, 0.0)));
    let silver = mix(vec3<f32>(grey), scan, 0.15) * vec3<f32>(2.5, 2.45, 2.4);
    // Loose strips peel away to the red-brown under-bark.
    let under = vec3<f32>(0.16, 0.085, 0.05);
    return mix(silver * (0.75 + 0.5 * tone), under, loose * 0.3);
}

// Bedded sandstone. `scan` is the terrain's rock texture there (for grain), `local`
// the point in the model (beds are level in it), `n` the world normal, `px` the
// pixel's size in metres. Beds a metre or so thick, each its own shade between
// brick red and buff, laminae inside them, a dark parting at each bed's foot, and
// desert varnish streaked down the steep faces.
fn bedded_sandstone(scan: vec3<f32>, local: vec3<f32>, n: vec3<f32>, px: f32) -> SceneryLook {
    var out: SceneryLook;
    let grain = dot(scan, vec3<f32>(0.3, 0.59, 0.11));
    let z = local.z + 0.25 * (surf_noise3(local * 0.2) - 0.5);
    let bed = floor(z / 0.95);
    let within = fract(z / 0.95);
    let pick = hash11(bed * 7.13 + 3.1);
    // Mostly brick-red beds, now and then a cream one.
    var tone = mix(SCENERY_SUPAI, SCENERY_REDWALL, smoothstep(0.2, 0.6, pick));
    tone = mix(tone, SCENERY_COCONINO, smoothstep(0.75, 0.95, pick));
    let lamina = fract(z / 0.16);
    tone *= 1.0 - 0.1 * smoothstep(0.6, 1.0, lamina) * surf_resolved(0.16, px);
    tone *= 1.0 - 0.4 * (1.0 - smoothstep(0.0, 0.07, within)) * surf_resolved(0.3, px);
    let steep = 1.0 - clamp(abs(n.z) * 1.4, 0.0, 1.0);
    let streak = smoothstep(0.5, 0.8, surf_noise3(vec3<f32>(local.xy * 0.9, local.z * 0.07)));
    tone = mix(tone, SCENERY_VARNISH * 2.0, streak * steep * 0.55);
    // Pale, sand-dusted tops.
    tone = mix(tone, SCENERY_COCONINO * 0.9, smoothstep(0.7, 0.95, n.z) * 0.45);
    out.albedo = tone * (0.55 + 1.5 * grain);
    out.roughness = 0.92;
    return out;
}

// The dam's concrete, by pattern. `local` is the point in the model (z = 0 at the
// crest road), `face` the face's frame (xy metres on it, zw its half size, a
// negative z for a tube), `n` the world normal, `px` the pixel's size in metres.
fn dam_concrete(pattern: u32, local: vec3<f32>, face: vec4<f32>, n: vec3<f32>, px: f32) -> SceneryLook {
    var out: SceneryLook;
    out.roughness = 0.82;
    let broad = surf_fbm3(local, 38.0, px);
    let steep = 1.0 - clamp(abs(n.z) * 1.3, 0.0, 1.0);
    if pattern == SCENERY_CONCRETE_ROAD {
        let grit = surf_fbm3(local, 2.0, px);
        out.albedo = vec3<f32>(0.075, 0.072, 0.068) * (1.0 + 0.5 * grit + 0.4 * broad);
        out.roughness = 0.9;
        return out;
    }
    if pattern == SCENERY_CONCRETE_LINE {
        out.albedo = vec3<f32>(0.62, 0.48, 0.14) * (0.9 + 0.3 * broad);
        out.roughness = 0.7;
        return out;
    }
    // Warm, pale mass concrete, each block cast a shade of its own.
    var c = vec3<f32>(0.5, 0.47, 0.41) * (1.0 + 0.5 * broad);
    // Block joints: the model cuts its walls one face per block, so a joint is
    // where a face's frame runs out across it.
    let half = abs(face.z);
    if face.z > 4.0 && steep > 0.3 {
        let joint = 1.0 - smoothstep(0.08, 0.3, half - abs(face.x));
        c *= 1.0 - 0.32 * joint * surf_resolved(0.5, px);
    }
    // Lift lines every 1.5 m up the face, a thin dark seam at each pour.
    let lift = fract(local.z / 1.5);
    c *= 1.0 - 0.14 * steep * (1.0 - smoothstep(0.0, 0.06, lift)) * surf_resolved(0.4, px);
    // Streaks running down from the crest and the gallery openings.
    let run = surf_noise3(vec3<f32>(local.x * 0.35, local.y * 0.35, local.z * 0.025));
    c *= 1.0 - 0.28 * steep * smoothstep(0.55, 0.85, run);
    // Grime and seep where the face meets the rock and the water.
    let low = smoothstep(-70.0, -118.0, local.z);
    c *= 1.0 - 0.3 * low;
    if pattern == SCENERY_CONCRETE_RING {
        // The mineral ring: chalky white, crisp at the old full pool (the band's
        // top edge in the mesh), drip-streaked and greyer toward the water.
        let drips = smoothstep(0.35, 0.75, surf_noise3(vec3<f32>(local.x * 0.9, local.y * 0.9, local.z * 0.05)));
        let white = SCENERY_CRUST * (0.97 + 0.3 * broad);
        c = mix(white, c * 1.1, 0.25 * drips + 0.2 * smoothstep(-20.0, -60.0, local.z));
        out.roughness = 0.9;
    } else if pattern == SCENERY_CONCRETE_WET {
        c *= vec3<f32>(0.42, 0.42, 0.4);
        out.roughness = 0.35;
    }
    out.albedo = c;
    return out;
}
