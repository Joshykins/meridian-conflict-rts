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
// plates (`RIFT_WELL`); and a gravitic lens's trail of cracks (`RIFT_TRAIL`, lens_fx.rs).
// The cracks glow violet with white-hot hearts while the engine feeds them (`Rift::glow`,
// which the CPU eases, dims and flickers), and the plates beside them are tilted up (`time.w`, read through the normal). Once let go the light goes out
// and the cracks stay as a dark scar, the plates settling flat. Detail under a pixel is
// swapped for its average (`crater_crack`).

// The light in the cracks at `t` (0 out, 1 full, past 1 white-hot), in HDR: deep violet,
// violet, lavender, white.
fn rift_rgb(t: f32) -> vec3<f32> {
    let k = clamp(t, 0.0, 1.6);
    var c = mix(vec3<f32>(0.14, 0.012, 0.5), vec3<f32>(0.4, 0.08, 1.0), smoothstep(0.05, 0.45, k));
    c = mix(c, vec3<f32>(0.72, 0.55, 1.0), smoothstep(0.6, 1.0, k));
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
    if kind == RIFT_TRAIL {
        // A gravitic lens's trail: the Fallen's rift in small. A jagged crack splits the
        // ground under its front's path, the ground either side broken into plates tipped
        // toward it, and cracks branch off it at a slant, each its own length. Violet light
        // burns down in the crack behind the head, cools to embers (pulsing toward the
        // well while it holds) and goes out; the scar fades (`time.w`).
        let start = r.line.xy;
        let span = r.line.zw - start;
        let along = span / max(length(span), 1.0);
        let side = vec2<f32>(-along.y, along.x);
        let rel = xy - start;
        let at = dot(rel, along);
        let across = dot(rel, side);
        let front = min(age * r.time.y, r.time.z);
        if at < -reach || at > front + reach || abs(across) > reach * 4.0 {
            return s;
        }
        // It wanders off the straight line, broadly and in short jags.
        let wander = (grad_noise2(vec2<f32>(at, 0.0) + o, 70.0) - 0.5) * reach * 1.6
            + (grad_noise2(vec2<f32>(at * 1.7, 5.0) - o, 16.0) - 0.5) * reach * 0.9
            + (grad_noise2(vec2<f32>(at * 3.1, 8.0) + o, 4.0) - 0.5) * reach * 0.3;
        let t = across - wander;
        let reached = 1.0 - smoothstep(front - 2.0, front + 1.0, at);
        let scar = heave * reached;
        // Narrow and pinched where it leaves the lens's feet, gaping in places.
        let foot = smoothstep(2.0, 16.0, at);
        let gap = reach * (0.1 + 0.16 * grad_noise2(vec2<f32>(at, 11.0) + o, 13.0)) * (0.35 + 0.65 * foot);
        let fissure = (1.0 - smoothstep(gap * 0.5, gap + px * 0.7, abs(t))) * scar;
        // Plates broken off either side, tipped up toward the crack.
        let cell = reach * 0.5;
        let q = vec2<f32>(at, t) / cell + o * 0.1;
        let warp = vec2<f32>(grad_noise2(xy + o, cell * 1.6), grad_noise2(xy - o, cell * 1.6)) - 0.5;
        let cells = crater_cells(q + warp * 0.9);
        // The broken band runs out raggedly, wider in places.
        let spread = reach * (1.4 + 1.2 * grad_noise2(vec2<f32>(at, 21.0) + o, 17.0));
        let band = (1.0 - smoothstep(reach * 0.5, spread, abs(t))) * scar;
        let gape = grad_noise2(vec2<f32>(at, t) * 0.8 + o * 3.0, cell * 1.3);
        let crack = crater_crack(cells.x, 0.04 + 0.06 * gape * gape, px / cell) * band;
        let toward = -side * sign(t);
        s.slope += (toward * (0.1 + 0.35 * cells.y) * (1.0 - fissure)
            + vec2<f32>(cos(cells.y * 41.0), sin(cells.y * 41.0)) * 0.06) * band;
        // Cracks branching off it, at uneven gaps to one side or the other, slanting on the
        // way the front ran at angles of their own and bending as they go, jagged, each its
        // own length, narrowing out. A branch reaches at most 3.2 reach x 1.4 and its jags
        // on from its root: 27 m at the Lodestone's 5 m reach, inside the five steps looked
        // at.
        let step_m = 6.0;
        var branch = 0.0;
        var branch_floor = 0.0;
        let k0 = floor(at / step_m);
        for (var j = 0; j < 5; j++) {
            let k = k0 - f32(j);
            let h = hash21(vec2<f32>(k, seed * 1.3));
            let way = select(-1.0, 1.0, h > 0.5);
            let long = hash21(vec2<f32>(k + 5.0, seed));
            let run = reach * mix(0.5, 3.2, long * long);
            let out = t * way;
            if out <= 0.0 || out > run || hash21(vec2<f32>(k + 9.0, seed * 0.7)) < 0.25 {
                continue;
            }
            let root = (k + hash21(vec2<f32>(k + 3.0, seed))) * step_m;
            let slant = mix(0.15, 0.9, hash21(vec2<f32>(k + 7.0, seed)));
            let share = out / run;
            let bend = (hash21(vec2<f32>(k + 13.0, seed)) - 0.3) * 0.7 * share;
            let jag = (grad_noise2(vec2<f32>(out, k * 7.7) + o, 3.0) - 0.5) * reach * 0.7
                + (grad_noise2(vec2<f32>(out * 2.3, k * 3.1) - o, 1.1) - 0.5) * reach * 0.2;
            let off = abs(at - root - out * (slant + bend) - jag);
            let w = max(gap * mix(1.2, 0.7, long) * (1.1 - share), 0.06);
            let fade = 1.0 - smoothstep(0.7, 1.0, share);
            let line = (1.0 - smoothstep(w * 0.5, w + px * 0.7, off)) * fade;
            branch = max(branch, line);
            // Its light a thin line at its floor, never under a pixel and a bit.
            let lw = max(w * 0.25, px * 1.2);
            let lit = (1.0 - smoothstep(0.0, lw, off)) * min(w * 0.25 / lw * 1.6, 1.0) * fade;
            branch_floor = max(branch_floor, lit * (1.0 - share));
        }
        branch *= scar;
        broken = max(max(band * 0.7, fissure), branch);
        // The light: hot behind the head, cooling as it falls behind (`RIFT_TRAIL_COOL`),
        // embers fed while the lens holds, pulsing in toward the well.
        let passed = age - at / max(r.time.y, 1.0);
        let hot = select(0.0, exp(-passed / RIFT_TRAIL_COOL), passed >= 0.0) * r.glow.w;
        let pulse = pow(0.5 + 0.5 * sin((at / 55.0 - now * 1.5) * 6.2831853), 4.0);
        let ember = glow * (1.0 + r.glow.y * (1.8 * pulse - 0.45));
        // The embers glow in patches, crawling along it, never one ruled line.
        let seep = 0.1 + 0.9 * smoothstep(0.35, 0.75, grad_noise2(vec2<f32>(at - now * 6.0, 2.0) + o, 17.0));
        // Even burning hot the light comes and goes along it.
        let patchy = 0.3 + 0.7 * smoothstep(0.3, 0.6, grad_noise2(vec2<f32>(at, 31.0) + o, 11.0));
        let heat = (hot * 0.8 * patchy + ember * seep) * reached;
        // The light is down in the crack: a thin seam at its floor, wandering from wall to
        // wall, never under a pixel and a bit; the branches and the plates' cracks dimmer.
        let seam_at = (grad_noise2(vec2<f32>(at * 0.9, 3.0) + o, 4.0) - 0.5) * gap * 0.8;
        let seam_w = max(gap * 0.3, px * 1.3);
        let seam = (1.0 - smoothstep(0.0, seam_w, abs(t - seam_at))) * min(gap * 0.3 / seam_w * 1.6, 1.0) * reached;
        let near = 1.0 - smoothstep(0.0, reach * 1.2, abs(t));
        light = (seam + fissure * 0.35 + branch_floor * scar * 0.9 + crack * 0.8 * near * gape) * heat;
        spill = heat * 0.12 * exp(-abs(t) / max(gap * 3.0, 1.0));
        a = mix(a, vec3<f32>(0.006, 0.005, 0.008), max(fissure, branch * 0.85));
        // Crushed earth and dust over the broken band, darker near the crack.
        a = mix(a, a * vec3<f32>(0.4, 0.37, 0.38), band * (0.45 + 0.4 * near));
        a *= 1.0 - 0.8 * crack;
    } else if kind == RIFT_LINE {
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
        // The light is down in it: a thin seam at its floor, wandering from wall to wall,
        // burning in patches; the rest of the gap a dim violet, its walls dark.
        let seam_at = (grad_noise2(vec2<f32>(at * 0.9, 3.0) + o, 9.0) - 0.5) * gap * 0.8;
        let seam = (1.0 - smoothstep(0.0, gap * 0.22 + px * 0.6, abs(t - seam_at))) * reached;
        let seep = 0.45 + 0.55 * smoothstep(0.25, 0.75, grad_noise2(vec2<f32>(at, 7.0) + o + vec2<f32>(now * 2.0, 0.0), 38.0));
        let near_seam = 1.0 - smoothstep(0.0, reach * 0.9, abs(t));
        light = (seam * (0.9 + 0.5 * r.glow.z) + fissure * 0.3) * heat * seep
            + crack * heat * 0.12 * near_seam;
        spill = heat * 0.1 * exp(-abs(t) / max(gap * 2.5, 1.0)) * reached;
        a = mix(a, vec3<f32>(0.006, 0.005, 0.008), fissure);
        // Dust and crushed earth over the broken band, darker near the fissure.
        a = mix(a, a * vec3<f32>(0.55, 0.5, 0.5), band * (0.4 + 0.4 * near_seam));
        a *= 1.0 - 0.75 * crack;
    } else {
        let centre = r.line.xy;
        let d = xy - centre;
        let dl = length(d);
        let ang = atan2(d.y, d.x);
        let outward = d / max(dl, 1e-3);
        if kind == RIFT_FOOT {
            let inner = r.shape.z;
            // How far out the breaking has got this way: a ragged front, not a circle.
            let ragged = (grad_noise2(outward * 2.2 + o, 1.0) - 0.5) * 0.5
                + (grad_noise2(outward * 7.0 - o, 1.0) - 0.5) * 0.2;
            let front = inner + (reach - inner) * (1.0 + ragged);
            if dl > front + 30.0 || dl < inner * 0.8 {
                return s;
            }
            // A few great fissures run out from the foot, each its own length, wandering
            // and jagged; the ground between breaks into plates by smaller cracks.
            let rays = 9.0;
            let u = ang / 6.2831853 * rays + seed * 7.0;
            let k = floor(u + 0.5);
            let jag = (grad_noise2(vec2<f32>(dl, k * 13.1) + o, 45.0) - 0.5) * 1.1
                + (grad_noise2(vec2<f32>(dl * 2.0, k * 5.3) - o, 11.0) - 0.5) * 0.35
                + (hash21(vec2<f32>(k, seed * 1.7)) - 0.5) * 0.5;
            let off = abs(u - k - jag * 0.45) * dl * 6.2831853 / rays;
            let run = inner + (front - inner) * mix(0.45, 1.05, hash21(vec2<f32>(k, seed * 3.1)));
            let share = clamp((dl - inner) / max(run - inner, 1.0), 0.0, 1.0);
            let width = (0.5 + 2.6 * (1.0 - share)) * mix(0.6, 1.3, hash21(vec2<f32>(k + 9.0, seed)));
            let ray = (1.0 - smoothstep(width * 0.5, width + px, off)) * (1.0 - smoothstep(run - 25.0, run, dl));
            // The light down in it: a thin bright line at its floor, not the whole gap.
            // Never thinner than a pixel and a bit, its light spread thinner instead.
            let line_w = max(width * 0.3, px * 1.3);
            let floor_line = (1.0 - smoothstep(0.0, line_w, off)) * min(width * 0.3 / line_w * 1.8, 1.0)
                * (1.0 - smoothstep(run - 25.0, run, dl));
            let cell = 16.0;
            let warp = vec2<f32>(grad_noise2(d + o, 21.0), grad_noise2(d - o, 21.0)) - 0.5;
            let cells = crater_cells(d / cell + o * 0.1 + warp * 0.9);
            let apron = 1.0 - smoothstep(front - 40.0, front, dl);
            let gape = grad_noise2(d * 0.7 + o * 3.0, 30.0);
            let crack = crater_crack(cells.x, 0.02 + 0.05 * gape * gape, px / cell) * apron * smoothstep(inner * 0.95, inner * 1.02, dl);
            s.slope += (outward * 0.15 + vec2<f32>(cos(cells.y * 37.0), sin(cells.y * 37.0)) * 0.1)
                * apron * heave * (0.2 + 0.8 * cells.y) * (1.0 - smoothstep(inner, front, dl));
            // Light seeps up patchily, brightest near the foot.
            let seep = 0.35 + 0.65 * smoothstep(0.3, 0.7, grad_noise2(d + o * 2.0 + vec2<f32>(now * 3.0, 0.0), 24.0));
            let heat = glow * (1.1 - 0.65 * share) * seep;
            light = floor_line * heat * 1.5 + ray * heat * 0.55 + crack * heat * 0.35 * gape;
            spill = heat * 0.08 * ray;
            broken = max(ray, apron * 0.5);
            a = mix(a, vec3<f32>(0.006, 0.005, 0.008), ray * 0.9);
            a *= 1.0 - 0.75 * crack;
        } else {
            if dl > reach * 1.35 {
                return s;
            }
            // The shatter runs out from the middle as the rift strikes.
            let shattered = 1.0 - smoothstep(age * r.time.y - 25.0, age * r.time.y, dl);
            let edge = reach * (1.0 + (grad_noise2(outward * 3.0 + o, 1.0) - 0.5) * 0.1
                + (grad_noise2(outward * 11.0 - o, 1.0) - 0.5) * 0.04);
            let disc = (1.0 - smoothstep(edge * 0.94, edge * 1.01, dl)) * shattered;
            // Plates of every size: big ones out toward the edge, broken small at the heart.
            let cell = max(reach * mix(0.035, 0.075, smoothstep(0.0, reach, dl)), 6.0);
            let warp = vec2<f32>(grad_noise2(d + o, cell * 2.3), grad_noise2(d - o, cell * 2.3)) - 0.5;
            let q = d / cell + o * 0.1 + warp * 1.3;
            let cells = crater_cells(q);
            let small = crater_cells(q * 2.7 + 17.0);
            let pc = px / cell;
            let gape = grad_noise2(d * 1.3 + o * 4.0, cell * 1.4);
            let crack = crater_crack(cells.x, 0.015 + 0.07 * gape * gape, pc) * disc;
            let fine = crater_crack(small.x, 0.04, pc * 2.7) * disc * (1.0 - smoothstep(0.0, reach * 0.5, dl)) * 0.6;
            // A few great fissures from the heart out to the edge, where the most light is.
            let rays = 5.0;
            let u = ang / 6.2831853 * rays + seed * 3.0;
            let k = floor(u + 0.5);
            let jag = (grad_noise2(vec2<f32>(dl, k * 7.7) + o, 60.0) - 0.5) * 1.2
                + (grad_noise2(vec2<f32>(dl * 2.1, k * 3.1) - o, 14.0) - 0.5) * 0.35;
            let off = abs(u - k - jag * 0.4) * dl * 6.2831853 / rays;
            let run = edge * mix(0.35, 1.05, hash21(vec2<f32>(k, seed * 2.3)));
            // Each starts out from the heart a way of its own.
            let root = edge * 0.25 * hash21(vec2<f32>(k + 11.0, seed));
            let fw = (1.0 + 3.0 * (1.0 - dl / max(run, 1.0))) * smoothstep(0.0, reach * 0.06, dl);
            let fissure = (1.0 - smoothstep(fw * 0.5, fw + px, off)) * (1.0 - smoothstep(run - 30.0, run, dl))
                * smoothstep(root, root + 20.0, dl) * shattered;
            // Its edge: cracks crossing the rim here and there, uneven, jagged.
            let rim_rays = 90.0;
            let ur = ang / 6.2831853 * rim_rays + seed * 5.0;
            let kr = floor(ur + 0.5);
            let there = step(0.72, hash21(vec2<f32>(kr, seed + 1.7)));
            let rjag = (grad_noise2(vec2<f32>(dl, kr * 3.3) + o, 18.0) - 0.5) * 0.9;
            let roff = abs(ur - kr - rjag * 0.45) * dl * 6.2831853 / rim_rays;
            let lo = edge * mix(0.82, 0.97, hash21(vec2<f32>(kr, seed)));
            let hi = edge * mix(1.03, 1.28, hash21(vec2<f32>(kr + 3.0, seed)));
            let rw = mix(0.5, 2.2, hash21(vec2<f32>(kr + 5.0, seed)));
            let rim_ray = there * (1.0 - smoothstep(rw * 0.5, rw + px, roff))
                * smoothstep(lo, lo + 12.0, dl) * (1.0 - smoothstep(hi - 20.0, hi, dl)) * shattered;
            let rim = exp(-pow((dl - edge) / max(reach * 0.03, 3.0), 2.0)) * shattered;
            // Plates tipped every way, more toward the heart.
            let tilt = (0.04 + 0.14 * cells.y) * heave * disc * (1.0 - 0.5 * dl / reach);
            s.slope += vec2<f32>(cos(cells.y * 53.0), sin(cells.y * 53.0)) * tilt;
            // Only some cracks are lit at a time: light crawls through the network in
            // patches; they all burn white for a moment as the shatter passes.
            let crawl = smoothstep(0.55, 0.85, grad_noise2(d * 0.01 + vec2<f32>(now * 0.23, -now * 0.19) + o, 1.0));
            let lit = 0.15 + 0.85 * max(crawl, smoothstep(0.62, 0.8, grad_noise2(d + o * 6.0, cell * 3.0)));
            let strike = 1.5 * exp(-age / 0.9);
            let heat = glow + strike * shattered;
            light = (crack * lit * 0.8 + fine * 0.35 * lit + fissure * (0.5 + 0.4 * crawl) + rim_ray * 0.3) * heat
                + rim * glow * 0.08;
            spill = heat * 0.04 * disc + heat * 0.15 * fissure;
            broken = max(disc * 0.8, max(rim_ray, fissure));
            a = mix(a, a * 0.5 + vec3<f32>(0.012, 0.01, 0.016), disc * 0.6);
            a *= 1.0 - 0.8 * max(max(crack, rim_ray), fissure);
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
