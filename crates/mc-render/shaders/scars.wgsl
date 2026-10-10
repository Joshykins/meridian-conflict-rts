// Scars in the ground, shaded over whatever ground they lie on (terrain.wgsl): craters
// big blasts leave, ground melted and glassed, and the ground a gravitic engine breaks.
// Prepended to terrain.wgsl with `//!use scars` (needs bindings; uses terrain.wgsl's
// `grad_noise2_d`).

// ---- Craters (renderer/craters.rs) ----------------------------------------------
// A big blast's crater, shaded analytically over whatever ground it hit: a pool of
// fused glass in the middle (molten, then crusting over with the glow left in its
// cracks, then black glass), a shallow bowl with a raised lip read only through the
// normal, and ragged charcoal and soot thrown out to about the blast's radius,
// feathering into dulled, scorched ground. Everything is fixed by the crater's seed and
// age; detail that would shrink under a pixel is swapped for its average.

struct CraterShade {
    albedo: vec3<f32>,
    rough: f32,
    // Slope of the crater's surface, dh/dx and dh/dy.
    slope: vec2<f32>,
    // How much of the ground's own relief the melt smoothed away.
    fused: f32,
    glow: vec3<f32>,
    // How much of the sky's light still reaches it: black glass mirrors less of it.
    sky: f32,
    // Black glass reflects less than bare ground's 4% (Pbr metallic toward its albedo).
    metal: f32,
}

fn craters_at(xy: vec2<f32>, alt: f32, albedo_in: vec3<f32>, rough_in: f32, px: f32) -> CraterShade {
    var s: CraterShade;
    s.albedo = albedo_in;
    s.rough = rough_in;
    s.slope = vec2<f32>(0.0);
    s.fused = 0.0;
    s.glow = vec3<f32>(0.0);
    s.sky = 1.0;
    s.metal = 0.0;
    let count = min(ground_craters.count.x, 48u);
    let now = globals.camera.w;
    // No glow on the sea floor.
    let dry = smoothstep(-0.6, 0.4, alt);
    let lum = dot(albedo_in, vec3<f32>(0.2126, 0.7152, 0.0722));
    // The ground's own light and dark carries on through the burn.
    let grain = clamp(sqrt(lum / 0.06), 0.75, 1.3);
    // Scorched ground is never bright, whatever lay there: snow melts off, sand
    // and grass blacken. Bright ground is brought down first, so the soot and
    // charcoal laid over it partly never let its pale texture through.
    let seared = min(lum, 0.07 + (lum - 0.07) * 0.12);
    for (var i = 0u; i < count; i++) {
        let c = ground_craters.items[i];
        let big = c.at.z;
        let d = xy - c.at.xy;
        let d2 = dot(d, d);
        if d2 > big * big * 1.9 {
            continue;
        }
        let age = now - c.at.w;
        if age < 0.0 {
            continue;
        }
        let heat = c.look.x;
        let cool = max(c.look.y, 1.0);
        let pool = c.look.z;
        // A Regency nova's crater carries its seed negated (craters.rs `CraterStyle::Plasma`).
        let plasma = c.look.w < 0.0;
        let seed = abs(c.look.w);
        let dl = sqrt(d2);
        let r = dl / big;
        let dir = d / max(dl, 1e-3);
        let o = vec2<f32>(seed * 3.17, seed * -2.41);

        // No two alike and none round: a few broad lobes, blotches across the ground.
        let lobe = grad_noise2(dir * 1.6 + o, 1.0);
        let blotch = grad_noise2(d + o * 7.0, big * 0.11);
        let blotch2 = grad_noise2(d - o * 5.0, big * 0.035);
        let rw = r * (1.0 + (lobe - 0.5) * 0.3) + (blotch - 0.5) * 0.08;
        // Rays of thrown earth: broad ones, and fine ones that fray as they run out,
        // flattened to their average before they narrow under a few pixels.
        let ray1 = grad_noise2(dir * 5.0 + o + vec2<f32>(r * 0.6, 0.0), 1.0);
        let fine_w = 6.2831853 * dl / 88.0;
        let ray2 = mix(grad_noise2(dir * 14.0 - o + vec2<f32>(0.0, r * 1.8), 1.0), 0.5,
            smoothstep(2.0, 6.0, px / max(fine_w, 1e-3) * 4.0));
        let ray = smoothstep(0.3, 0.72, ray1 * 0.6 + ray2 * 0.4);
        // Some sides threw far more than others.
        let sector = smoothstep(0.3, 0.72, grad_noise2(dir * 2.3 - o * 1.3, 1.0));
        // Broken along their length, so they fray instead of fanning out evenly.
        let broken = grad_noise2(d + o * 3.0, big * 0.16);
        let reach = 0.5 + ray * (0.35 + 0.6 * sector) * (0.6 + 0.8 * broken) + (lobe - 0.5) * 0.3;

        // The bowl: a flat floor (the pool), walls up to a lip, the lip's apron outside.
        let pool_r = pool * (1.0 + (lobe - 0.5) * 0.35 + (blotch2 - 0.5) * 0.14);
        let rim = max(pool * 1.65, 0.3) * (1.0 + (lobe - 0.5) * 0.12);
        let floor_r = select(rim * 0.3, pool_r, pool > 0.0);
        let span = max(rim - floor_r, 0.02);
        let t = clamp((r - floor_r) / span, 0.0, 1.0);
        let depth = 0.032;
        var dh = depth * 6.0 * t * (1.0 - t) / span;
        let x = r - rim;
        let wl = select(0.04, 0.15, x > 0.0);
        dh += 0.014 * exp(-(x * x) / (wl * wl)) * (-2.0 * x / (wl * wl));
        dh *= 0.7 + 0.6 * blotch;
        s.slope += dir * dh;

        // ---- Scorch ----
        // Solid charcoal in and round the bowl, running out in dark fingers along the
        // rays; sooty brown earth between them; then dulled, dried ground.
        let inner = 1.0 - smoothstep(rim * 1.1, rim * 1.7, rw);
        let fingers = (1.0 - smoothstep(reach * 0.4, reach, rw)) * smoothstep(0.3, 0.8, ray) * (0.35 + 0.65 * sector);
        let charred = max(inner, fingers * 0.8);
        let sooty = (1.0 - smoothstep(0.5, 1.05, rw + (broken - 0.5) * 0.35 + (0.5 - sector) * 0.2))
            * (0.55 + 0.45 * smoothstep(0.3, 0.7, blotch2 + ray * 0.3));
        let fray = grad_noise2(d - o * 2.0, big * 0.045);
        let dulled = 1.0 - smoothstep(0.9, 1.25, rw + (blotch - 0.5) * 0.4 + (fray - 0.5) * 0.3 - sector * 0.12);
        // Pale ash drifted in streaks over the charcoal.
        let ash = smoothstep(0.6, 0.78, ray2 * 0.7 + blotch2 * 0.4) * smoothstep(0.3, 0.45, r)
            * (1.0 - smoothstep(0.55, 0.8, rw)) * 0.45;
        // The burn runs out from the middle with the blast over its first second and a
        // half, instead of the whole star of charcoal being there under the flash.
        let burn_front = 1.9 * sqrt(clamp(age / 1.4, 0.0, 1.0));
        let burnt = 1.0 - smoothstep(burn_front - 0.3, burn_front, r);
        let char_rgb = vec3<f32>(0.013, 0.012, 0.011) * grain;
        let soot_rgb = vec3<f32>(0.045, 0.033, 0.023) * grain;
        let ash_rgb = vec3<f32>(0.085, 0.082, 0.078) * grain;
        let dull_rgb = vec3<f32>(min(dot(s.albedo, vec3<f32>(0.2126, 0.7152, 0.0722)), seared)) * vec3<f32>(1.08, 0.9, 0.66) * 0.68;
        var a = mix(s.albedo, dull_rgb, dulled * burnt);
        a = mix(a, soot_rgb, sooty * 0.9 * burnt);
        a = mix(a, char_rgb, charred * burnt);
        a = mix(a, ash_rgb, ash * charred * burnt);
        var rough = mix(s.rough, 0.95, max(charred, sooty));

        // Early on, the bowl's walls still glow in streaks where the melt ran up them.
        var glow = vec3<f32>(0.0);
        if pool > 0.0 {
            let streak = smoothstep(0.55, 0.85, blotch2 * 0.5 + ray2 * 0.7);
            let wall = heat * exp(-age / 18.0) * (1.0 - smoothstep(pool_r, rim * 1.1, rw)) * streak;
            glow += crater_glow_rgb(wall * 0.55, plasma) * charred;
        }

        // ---- The glassed pool ----
        // Its shore is ragged: the melt ran further in some places than others.
        let shore = pool_r + (grad_noise2(d - o * 2.0, big * 0.018) - 0.5) * 0.035;
        let in_pool = 1.0 - smoothstep(shore - 0.01, shore + 0.01, r);
        // Tongues of glass splashed up the walls.
        let splash = smoothstep(0.58, 0.68, blotch2 + (grad_noise2(d + o, big * 0.012) - 0.5) * 0.3)
            * (1.0 - smoothstep(pool_r, pool_r + 0.09, r));
        let glass = select(0.0, max(in_pool, splash * 0.85), pool > 0.0);
        if glass > 0.002 {
            let cell = max(big * 0.045, 4.0);
            let warp = vec2<f32>(grad_noise2(d + o, cell * 1.7), grad_noise2(d - o, cell * 1.7)) - 0.5;
            let q = d / cell + o * 0.37 + warp * 0.8;
            let cells = crater_cells(q);
            let small = crater_cells(q * 2.7 + 17.0);
            let pc = px / cell;
            // Some cracks gape, some are hairlines, some barely open.
            let gape = grad_noise2(d * 1.3 + o * 4.0, cell * 0.9);
            let crack = crater_crack(cells.x, 0.025 + 0.07 * gape * gape, pc);
            let fine = crater_crack(small.x, 0.05, pc * 2.7) * smoothstep(0.35, 0.8, gape) * 0.7;
            let open = clamp(crack + fine, 0.0, 1.0);
            // Heat bleeding out of a crack into the crust beside it.
            let bleed = mix(exp(-cells.x / 0.12), 0.25, smoothstep(0.25, 0.7, pc));
            let plate = cells.y;

            let u = age / cool;
            let rp = clamp(r / max(shore, 0.01), 0.0, 1.0);
            let hot = heat * (1.0 - 0.4 * rp * rp);
            // The open melt, darker skins drifting on it; crust plates freezing on it one
            // by one from the shore in; and the cracks between, which keep their heat
            // longest and go out last.
            let skin = smoothstep(0.45, 0.8, grad_noise2(d + warp * cell * 1.5 + o * 9.0, cell * 0.6));
            let body_t = hot * pow(clamp(1.0 - u * (2.0 + rp), 0.0, 1.0), 1.4) * (1.0 - 0.3 * skin);
            let crust = smoothstep(0.0, 0.035, u * (1.0 + 0.7 * rp) - 0.05 - 0.13 * plate);
            let crack_t = hot * pow(clamp(1.0 - u * (0.8 + 0.25 * rp), 0.0, 1.0), 1.8) * (0.7 + 0.3 * gape);
            var g = crater_glow_rgb(body_t, plasma) * (1.0 - crust);
            g += (crater_glow_rgb(crack_t, plasma) * open + crater_glow_rgb(crack_t * 0.6, plasma) * bleed * 0.35 * (1.0 - open)) * crust;
            glow = mix(glow, g, glass);

            // Black-green glass, glossy, rolling in broad swells; cracks dull and darker.
            // Rough enough, and rolling enough, that the sun glints off it in broken
            // patches rather than one white mirror.
            let swell = grad_noise2_d(d + o * 11.0, cell * 1.4).yz * cell * 1.4
                + grad_noise2_d(d - o * 13.0, cell * 0.5).yz * cell * 0.5 * (1.0 - smoothstep(0.3, 0.9, pc * 2.0));
            s.slope += swell * 0.05 * glass;
            // A dark rind of slag along the shore and on the splashes.
            let rind = max(1.0 - smoothstep(0.004, 0.02, abs(r - shore)), 1.0 - in_pool);
            // A nova's glass is black with a crimson depth to it.
            let glass_tint = select(vec3<f32>(0.011, 0.02, 0.016), vec3<f32>(0.022, 0.006, 0.009), plasma);
            let glass_rgb = mix(glass_tint * (0.9 + 0.2 * plate) * (1.0 - 0.6 * open),
                vec3<f32>(0.012, 0.011, 0.01), rind);
            let g_rough = mix(0.38 + 0.12 * open, 0.95, rind);
            a = mix(a, glass_rgb, glass);
            rough = mix(rough, g_rough, glass);
            s.fused = max(s.fused, glass * (1.0 - rind * 0.5));
            s.sky *= 1.0 - 0.45 * glass * (1.0 - rind);
            s.metal = max(s.metal, 0.85 * glass * (1.0 - rind));
        }
        s.albedo = a;
        s.rough = rough;
        s.glow += glow * dry;
    }
    return s;
}

// ---- Melted ground (renderer/ground_melt.rs) ----------------------------------------
// The melt field shaded over whatever ground it lies on: a heat, glass and scorch per
// square metre, so overlapping burns run together as one surface. Scorch sears, browns
// and chars the ground; past the melt point it is molten, white-yellow, then orange, with
// darker skins drifting on it that thicken and join as it cools; then it sets, crust at
// its edge and glass where it ran hottest, its glow lingering longest in a wandering web
// of veins, and the scar stays. The field is
// read a metre or two off true so its square cells never show, and every detail that
// would shrink under a pixel is swapped for its average.

fn melt_shade(xy: vec2<f32>, alt: f32, albedo_in: vec3<f32>, rough_in: f32, px: f32) -> CraterShade {
    var s: CraterShade;
    s.albedo = albedo_in;
    s.rough = rough_in;
    s.slope = vec2<f32>(0.0);
    s.fused = 0.0;
    s.glow = vec3<f32>(0.0);
    s.sky = 1.0;
    s.metal = 0.0;
    let warp = vec2<f32>(grad_noise2(xy, 3.1), grad_noise2(xy + vec2<f32>(41.3, -17.9), 3.1)) - 0.5;
    let m = melt_sample(xy + warp * 1.8);
    if m.x + m.y + m.z < 0.004 {
        return s;
    }
    let heat = m.x;
    let dry = smoothstep(-0.6, 0.4, alt);
    let fine_aa = smoothstep(0.12, 0.45, px);
    let fine = mix(grad_noise2(xy + warp * 2.0, 0.7), 0.5, fine_aa);
    let broad = grad_noise2(xy - warp * 3.0, 2.4);
    let lum = dot(albedo_in, vec3<f32>(0.2126, 0.7152, 0.0722));
    // The ground's own light and dark carries on through the burn.
    let grain = clamp(sqrt(lum / 0.06), 0.75, 1.3);

    // ---- The scar, from the edge in ----
    // Ground only seared: dried, yellowed and paled. Then umber soil, then charcoal in
    // ragged fingers with pale ash drifted over it. Where it melted at all, a rough
    // sintered crust spotted with rust, pushed up into a lumpy rind along its shore; and
    // where it ran white-hot, glass: black-green or bottle-brown by the ground it was,
    // sunk a little, crazed with fine cracks and pitted with burst bubbles.
    let mid = mix(grad_noise2(xy + warp * 1.5, 0.9), 0.5, smoothstep(0.2, 0.6, px));
    let patchy = broad * 0.6 + mid * 0.4;
    let dried = vec3<f32>(min(lum, 0.09)) * vec3<f32>(1.15, 0.95, 0.68);
    let sear = smoothstep(0.02, 0.22, m.z + (patchy - 0.5) * 0.16);
    var a = mix(albedo_in, dried, sear * 0.65);
    let seared = smoothstep(0.22, 0.5, m.z + (patchy - 0.5) * 0.35 + (fine - 0.5) * 0.15);
    a = mix(a, vec3<f32>(0.042, 0.03, 0.021) * grain, seared * 0.85);
    let charred = smoothstep(0.5, 0.8, m.z + (patchy - 0.5) * 0.45 + (fine - 0.5) * 0.2);
    // Charcoal, with blotches of burnt grey-brown soil left in it.
    let soil = smoothstep(0.55, 0.75, broad * 0.7 + mid * 0.3);
    a = mix(a, mix(vec3<f32>(0.013, 0.012, 0.011), vec3<f32>(0.038, 0.03, 0.024), soil * 0.8) * grain, charred * 0.95);
    var rough = mix(rough_in, 0.95, max(seared, charred));

    let fused = smoothstep(0.015, 0.08, m.y + (patchy - 0.5) * 0.05 + (fine - 0.5) * 0.03);
    let ash = smoothstep(0.6, 0.78, mid * 0.6 + fine * 0.4 + (1.0 - broad) * 0.15 - 0.05)
        * charred * (1.0 - fused);
    a = mix(a, vec3<f32>(0.16, 0.155, 0.145) * grain, ash * 0.7);
    if fused > 0.002 {
        let glassy = smoothstep(0.4, 0.85, m.y + (broad - 0.5) * 0.4 + (fine - 0.5) * 0.2);
        // Relief, from how the glass field slopes: the rind where the melt shored up,
        // the pool sunk behind it, both lumpy.
        let e = 0.8;
        let at = xy + warp * 1.8;
        let grad = vec2<f32>(
            melt_sample(at + vec2<f32>(e, 0.0)).y - melt_sample(at - vec2<f32>(e, 0.0)).y,
            melt_sample(at + vec2<f32>(0.0, e)).y - melt_sample(at - vec2<f32>(0.0, e)).y) / (2.0 * e);
        let f = m.y;
        let k = (f - 0.08) / 0.07;
        // Broken: in places the rind is a heap, in places there is none.
        let lump = smoothstep(0.35, 0.75, grad_noise2(xy + warp * 2.0, 3.5)) * (0.6 + mid * 0.8);
        let dh = 0.16 * lump * exp(-k * k) * (-2.0 * k / 0.07)
            - 0.1 * smoothstep(0.1, 0.6, f) * (1.0 - smoothstep(0.1, 0.6, f)) * 6.0;
        s.slope = grad * dh * 0.6;
        // Glass rolling in low swells, so the sun glints off it in broken patches.
        let swell = grad_noise2_d(xy + warp * 4.0, 2.2).yz * 2.2
            + grad_noise2_d(xy - warp * 2.0, 0.8).yz * 0.8 * (1.0 - fine_aa);
        s.slope += swell * 0.05 * glassy;

        // Sintered crust: dark grey-brown, rust where iron in it burnt.
        let rust = smoothstep(0.55, 0.75, patchy + (fine - 0.5) * 0.3);
        let crust = mix(vec3<f32>(0.032, 0.026, 0.021), vec3<f32>(0.075, 0.034, 0.014), rust * 0.7) * grain;
        // Glass: what the ground was, darkened nearly to black and shifted green or brown.
        let hue = grad_noise2(xy - warp * 5.0, 6.0);
        let tinge = mix(mix(vec3<f32>(0.016, 0.012, 0.008), vec3<f32>(0.007, 0.017, 0.012), hue),
            vec3<f32>(0.006, 0.006, 0.006), smoothstep(0.4, 0.8, broad));
        var glass_rgb = mix(albedo_in * 0.1, tinge, 0.7);
        // Crazing: a few fine cracks where the glass shrank as it set, in some patches only.
        let craze_cell = 1.4;
        let craze_w = 0.035;
        let craze_soft = max(craze_w * 0.5, px / craze_cell);
        let ridge = 1.0 - abs(grad_noise2(xy + warp * 0.8, craze_cell) * 2.0 - 1.0);
        let crazed_here = smoothstep(0.5, 0.7, grad_noise2(xy + vec2<f32>(-13.0, 29.0), 5.0));
        let craze = mix(smoothstep(1.0 - craze_w - craze_soft, 1.0 - craze_w * 0.3, ridge), craze_w * 1.2,
            smoothstep(0.3, 0.9, px / craze_cell)) * smoothstep(0.3, 0.8, glassy) * crazed_here;
        // Burst bubbles: small dull pits, most near the crust.
        let pit = smoothstep(0.8, 0.88, grad_noise2(xy + vec2<f32>(7.3, 2.1), 0.3))
            * (1.0 - fine_aa) * (1.0 - 0.6 * glassy);
        // Thicker and thinner glass: broad lighter and darker reaches across it.
        glass_rgb *= (0.65 + 0.9 * grad_noise2(xy + warp * 3.0, 7.0)) * (1.0 - 0.6 * craze - 0.5 * pit);
        let melt_rgb = mix(crust, glass_rgb, glassy);
        a = mix(a, melt_rgb, fused);
        let melt_rough = mix(0.88 - 0.1 * rust, 0.1 + 0.14 * fine, glassy) + 0.45 * craze + 0.5 * pit;
        rough = mix(rough, clamp(melt_rough, 0.1, 0.97), fused);
        s.fused = fused * (0.5 + 0.5 * glassy);
        s.sky = 1.0 - 0.45 * glassy * fused;
        s.metal = 0.85 * glassy * fused * (1.0 - craze) * (1.0 - pit);
    }

    if heat > 0.004 {
        let time = globals.camera.w;
        let liquid = smoothstep(MELT_FIELD_MELT - 0.04, MELT_FIELD_MELT + 0.12, heat);
        // 0 white-hot, 1 at the melt point.
        let cooling = clamp((1.0 - heat) / (1.0 - MELT_FIELD_MELT), 0.0, 1.0);
        // Skins drifting slowly on the melt, more of them and joining up as it cools.
        let drift = vec2<f32>(time * 0.11, time * -0.07);
        let raft = grad_noise2(xy + warp * 3.0 + drift, 1.6) * 0.65
            + mix(grad_noise2(xy - drift * 1.6 + vec2<f32>(9.1, 3.7), 0.6), 0.5, fine_aa) * 0.35;
        let skin = smoothstep(0.78 - 0.5 * cooling, 0.9 - 0.42 * cooling, raft) * liquid;
        // Veins: a web of thin lines wandering through the skins and, once it has set,
        // the glass. Ridged noise, so no two meshes alike and no straight edges.
        let ridge = 1.0 - abs(grad_noise2(xy + warp * 2.5, 1.9) * 2.0 - 1.0);
        let width = 0.07 + 0.06 * heat;
        let soft = max(width * 0.5, px / 1.9);
        let line = smoothstep(1.0 - width - soft, 1.0 - width * 0.3, ridge);
        let vein = mix(line, width * 1.6, smoothstep(0.35, 0.9, px / 1.9));
        var glow = crater_heat_rgb(heat * (1.0 - 0.6 * skin)) * liquid;
        glow += crater_heat_rgb(heat * 0.85) * vein * skin;
        // Set but still hot: dull red going out, the veins last.
        glow += crater_heat_rgb(heat * 0.8) * (0.2 + 0.8 * vein) * (1.0 - liquid);
        a = mix(a, vec3<f32>(0.012, 0.011, 0.01), liquid);
        rough = mix(rough, mix(0.4, 0.75, skin), liquid);
        // A touch dimmer than a blast's glassed pool (craters_at): these are many.
        s.glow = glow * 0.6 * dry;
    }
    s.albedo = a;
    s.rough = rough;
    return s;
}

// ---- A gravitic engine's broken ground (renderer/gravitic_engine_fx/ground.rs) ---------
// Three kinds of break, each shaded over whatever ground it lies on: cracks running out
// round the engine's foot as it gathers (`RIFT_FOOT`), the rift's fissure opening behind
// its front from the foot to the mark (`RIFT_LINE`), and the well's ground broken into
// plates (`RIFT_WELL`). The cracks glow violet with white-hot hearts while the engine
// feeds them (`Rift::glow`, which the CPU eases, dims and flickers), and the plates beside
// them are tilted up (`time.w`, read through the normal). Once let go the light goes out
// and the cracks stay as a dark scar, the plates settling flat. Detail under a pixel is
// swapped for its average (`crater_crack`).

// The light in the cracks at `t` (0 out, 1 full, past 1 white-hot), in HDR: deep violet,
// violet, lavender, white.
fn rift_rgb(t: f32) -> vec3<f32> {
    let k = clamp(t, 0.0, 1.6);
    var c = mix(vec3<f32>(0.2, 0.012, 0.5), vec3<f32>(0.56, 0.1, 1.0), smoothstep(0.05, 0.45, k));
    c = mix(c, vec3<f32>(0.85, 0.62, 1.0), smoothstep(0.6, 1.0, k));
    c = mix(c, vec3<f32>(1.0, 0.94, 1.0), smoothstep(1.0, 1.45, k));
    return c * (k * 1.6 + k * k * k * 5.0);
}

// What one break does to the ground at `xy` (`px` metres a pixel): adds to `s`.
fn rift_one(r: Rift, xy: vec2<f32>, px: f32, s_in: CraterShade) -> CraterShade {
    var s = s_in;
    let now = globals.camera.w;
    let age = now - r.time.x;
    if age < 0.0 {
        return s;
    }
    let kind = u32(r.shape.x + 0.5);
    let reach = r.shape.y;
    let seed = r.shape.w;
    let o = vec2<f32>(seed * 31.7, seed * -17.3);
    let heave = r.time.w;
    let glow = r.glow.x;
    var a = s.albedo;
    var light = 0.0;
    var spill = 0.0;
    var broken = 0.0;
    if kind == RIFT_LINE {
        let start = r.line.xy;
        let span = r.line.zw - start;
        let length_m = max(length(span), 1.0);
        let along = span / length_m;
        let side = vec2<f32>(-along.y, along.x);
        let rel = xy - start;
        let at = dot(rel, along);
        let across = dot(rel, side);
        let front = min(age * r.time.y, r.time.z);
        if at < -reach || at > front + reach || abs(across) > reach * 2.6 {
            return s;
        }
        // The fissure wanders off the straight line, broadly and in short jags.
        let wander = (grad_noise2(vec2<f32>(at, 0.0) + o, 90.0) - 0.5) * reach * 0.9
            + (grad_noise2(vec2<f32>(at * 1.7, 5.0) - o, 19.0) - 0.5) * reach * 0.35;
        let t = across - wander;
        // Torn open behind the front, narrow at the very head and pinched at its foot.
        let open = smoothstep(0.0, 90.0, front - at) * smoothstep(-6.0, 40.0, at);
        let gap = reach * (0.1 + 0.09 * grad_noise2(vec2<f32>(at, 11.0) + o, 27.0)) * (0.3 + 0.7 * open);
        let reached = 1.0 - smoothstep(front - 4.0, front + 2.0, at);
        let fissure = (1.0 - smoothstep(gap * 0.5, gap + px * 0.7, abs(t))) * reached;
        // Plates broken off either side, tipped up toward the fissure.
        let cell = reach * 0.34;
        let q = vec2<f32>(at, t) / cell + o * 0.1;
        let warp = vec2<f32>(grad_noise2(xy + o, cell * 1.6), grad_noise2(xy - o, cell * 1.6)) - 0.5;
        let cells = crater_cells(q + warp * 0.7);
        let band = (1.0 - smoothstep(reach * 0.5, reach * 2.3, abs(t)))
            * (1.0 - smoothstep(front - 2.0, front + reach * 0.4, at));
        let crack = crater_crack(cells.x, 0.06, px / cell) * band;
        let tilt = (0.12 + 0.4 * cells.y) * band * heave * (1.0 - fissure);
        let toward = -side * sign(t);
        s.slope += toward * tilt + vec2<f32>(cos(cells.y * 41.0), sin(cells.y * 41.0)) * 0.07 * band * heave;
        broken = max(band * 0.75, fissure);
        // Light runs down it toward the mark while the engine feeds it, and its head burns.
        let pulse = pow(0.5 + 0.5 * sin((at / 210.0 - now * 1.9) * 6.2831853), 5.0);
        let head = exp(-max(front - at, 0.0) / 70.0) * r.glow.w;
        let heat = glow * (1.0 + r.glow.y * (1.5 * pulse - 0.35)) + head * 1.3;
        light = fissure * heat * (0.85 + 0.3 * r.glow.z) + crack * heat * 0.5 * (1.0 - smoothstep(0.0, reach * 1.6, abs(t)));
        spill = heat * 0.22 * exp(-abs(t) / max(gap * 3.0, 1.0)) * reached;
        a = mix(a, vec3<f32>(0.006, 0.005, 0.008), fissure);
        a *= 1.0 - 0.75 * crack;
    } else {
        let centre = r.line.xy;
        let d = xy - centre;
        let dl = length(d);
        let ang = atan2(d.y, d.x);
        let outward = d / max(dl, 1e-3);
        if kind == RIFT_FOOT {
            let inner = r.shape.z;
            if dl > reach + 12.0 || dl < inner * 0.8 {
                return s;
            }
            // Main cracks run out from the foot, each its own length, wandering.
            let rays = 22.0;
            let u = ang / 6.2831853 * rays + seed * 7.0;
            let k = floor(u + 0.5);
            let wob = (grad_noise2(vec2<f32>(dl, k * 13.1) + o, 30.0) - 0.5) * 0.9
                + (grad_noise2(vec2<f32>(dl * 2.0, k * 5.3) - o, 9.0) - 0.5) * 0.3;
            let off = abs(u - k - wob * 0.45) * dl * 6.2831853 / rays;
            let run = inner + (reach - inner) * mix(0.35, 1.0, hash21(vec2<f32>(k, seed * 3.1)));
            let share = clamp((dl - inner) / max(run - inner, 1.0), 0.0, 1.0);
            let width = 0.6 + 3.2 * (1.0 - share);
            let ray = (1.0 - smoothstep(width * 0.5, width + px, off)) * (1.0 - smoothstep(run - 18.0, run, dl));
            // The ground shattered close round the foot.
            let apron = smoothstep(inner * 0.85, inner, dl)
                * (1.0 - smoothstep(inner + (reach - inner) * 0.2, inner + (reach - inner) * 0.6, dl));
            let cell = 7.0;
            let cells = crater_cells(d / cell + o * 0.1);
            let crack = crater_crack(cells.x, 0.06, px / cell) * apron;
            s.slope += (outward * 0.2 + vec2<f32>(cos(cells.y * 37.0), sin(cells.y * 37.0)) * 0.12) * apron * heave * (0.3 + 0.7 * cells.y);
            let heat = glow * (1.15 - 0.6 * share);
            light = max(ray, crack * 0.7) * heat;
            spill = heat * 0.12 * ray;
            broken = max(ray, apron * 0.6);
            a = mix(a, vec3<f32>(0.006, 0.005, 0.008), ray * 0.9);
            a *= 1.0 - 0.7 * crack;
        } else {
            if dl > reach * 1.35 {
                return s;
            }
            // The shatter runs out from the middle as the rift strikes.
            let shattered = 1.0 - smoothstep(age * r.time.y - 25.0, age * r.time.y, dl);
            let edge = reach * (1.0 + (grad_noise2(outward * 3.0 + o, 1.0) - 0.5) * 0.08);
            let disc = (1.0 - smoothstep(edge * 0.95, edge * 1.02, dl)) * shattered;
            let cell = max(reach * 0.055, 6.0);
            let warp = vec2<f32>(grad_noise2(d + o, cell * 1.7), grad_noise2(d - o, cell * 1.7)) - 0.5;
            let q = d / cell + o * 0.1 + warp * 0.8;
            let cells = crater_cells(q);
            let small = crater_cells(q * 2.6 + 17.0);
            let pc = px / cell;
            let gape = grad_noise2(d * 1.3 + o * 4.0, cell * 0.9);
            let crack = crater_crack(cells.x, 0.03 + 0.06 * gape, pc) * disc;
            let fine = crater_crack(small.x, 0.05, pc * 2.6) * disc * (1.0 - smoothstep(0.0, reach * 0.7, dl)) * 0.6;
            // Its edge: short cracks crossing the rim all round it, and a faint ring of light.
            let rays = 64.0;
            let u = ang / 6.2831853 * rays + seed * 5.0;
            let k = floor(u + 0.5);
            let off = abs(u - k) * dl * 6.2831853 / rays;
            let lo = reach * mix(0.8, 0.95, hash21(vec2<f32>(k, seed)));
            let hi = reach * mix(1.05, 1.3, hash21(vec2<f32>(k + 3.0, seed)));
            let rim_ray = (1.0 - smoothstep(0.8, 1.6 + px, off)) * smoothstep(lo, lo + 10.0, dl) * (1.0 - smoothstep(hi - 15.0, hi, dl)) * shattered;
            let rim = exp(-pow((dl - edge) / max(reach * 0.035, 3.0), 2.0)) * shattered;
            // Plates tipped every way, more toward the heart.
            let tilt = (0.1 + 0.32 * cells.y) * heave * disc * (1.0 - 0.5 * dl / reach);
            s.slope += vec2<f32>(cos(cells.y * 53.0), sin(cells.y * 53.0)) * tilt;
            // Light crawls through the cracks, and they burn white as the shatter passes.
            let crawl = smoothstep(0.5, 0.85, grad_noise2(d * 0.012 + vec2<f32>(now * 0.21, -now * 0.17) + o, 1.0));
            let strike = 1.4 * exp(-age / 1.1);
            let heat = glow * (0.55 + 0.75 * crawl) + strike * shattered;
            light = (crack + fine * 0.5 + rim_ray * 0.8) * heat + rim * glow * 0.12;
            spill = heat * 0.05 * disc;
            broken = max(disc * 0.8, rim_ray);
            a = mix(a, a * 0.55 + vec3<f32>(0.012, 0.01, 0.016), disc * 0.65);
            a *= 1.0 - 0.8 * max(crack, rim_ray);
        }
    }
    s.albedo = a;
    s.rough = mix(s.rough, 0.95, broken);
    s.glow += rift_rgb(light) + rift_rgb(spill) * 0.5;
    return s;
}

fn rifts_at(xy: vec2<f32>, alt: f32, albedo_in: vec3<f32>, rough_in: f32, px: f32) -> CraterShade {
    var s: CraterShade;
    s.albedo = albedo_in;
    s.rough = rough_in;
    s.slope = vec2<f32>(0.0);
    s.fused = 0.0;
    s.glow = vec3<f32>(0.0);
    s.sky = 1.0;
    s.metal = 0.0;
    let count = min(ground_rifts.count.x, RIFT_MAX);
    for (var i = 0u; i < count; i++) {
        s = rift_one(ground_rifts.items[i], xy, px, s);
    }
    // No light down on the sea floor.
    s.glow *= smoothstep(-0.6, 0.4, alt);
    return s;
}
