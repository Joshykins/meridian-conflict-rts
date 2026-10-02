// Cliff stone, shared by the terrain's steep faces (terrain.wgsl) and the rock
// pieces dressing them (entity.wgsl, scenery `ROCK_CLIFF`; mc-models cliffs.rs),
// so a piece is the stone of the wall it stands on. build.rs inserts this file
// after desert.wgsl into shaders that contain the line `//!use rock`.

// A cliff's stone outside canyon country: the rock scan's grain (`scan`, the
// rock face material there) greyed toward weathered stone. `z` is the drawn
// height, `slope` 1 - n.z, `broad` and `patchy` the habitat's fields
// (`habitat_fields`). On a mountain map (one with a snow layer) some outcrops
// are bedded: ledges catching the light over darker bands, broken by cracks.
fn cliff_stone(xy: vec2<f32>, z: f32, slope: f32, scan: vec3<f32>, broad: f32, patchy: f32, mountain: bool) -> vec3<f32> {
    let mineral = dot(scan, vec3<f32>(0.2126, 0.7152, 0.0722));
    let stone = mix(vec3<f32>(mineral) * vec3<f32>(0.95, 0.96, 0.98), scan, 0.3)
        * (1.05 + broad * 0.5 + patchy * 0.25);
    if !mountain {
        return stone;
    }
    // Only some outcrops are bedded, and only their steep faces show it.
    let bedded = smoothstep(0.5, 0.72, grad_noise2(xy + 211.0, 320.0)) * smoothstep(0.7, 1.2, slope);
    let bend = grad_noise2(xy + 7.0, 90.0) * 14.0 + grad_noise2(xy - 31.0, 23.0) * 3.0;
    let band = sin(z * (0.22 + 0.2 * grad_noise2(xy - 5.0, 400.0)) + bend);
    let ledge = smoothstep(0.6, 0.95, band) * bedded;
    let seam = (1.0 - smoothstep(0.0, 0.1, abs(band + 0.3))) * bedded;
    let crack = smoothstep(0.8, 0.92, grad_noise2(vec2<f32>(xy.x + xy.y, z * 3.0), 6.0));
    return stone * (0.95 + 0.25 * ledge) * (1.0 - 0.3 * seam) * (1.0 - 0.3 * crack);
}

// A canyon cliff's stone: the bed it was cut from (`site`, desert.wgsl), the scan
// giving only its grain; its thin beds stand out as ledges.
fn canyon_cliff_stone(site: CanyonSite, scan: vec3<f32>, slope: f32) -> vec3<f32> {
    let mineral = dot(scan, vec3<f32>(0.2126, 0.7152, 0.0722));
    let grain = clamp(pow(mineral / 0.07, 0.45), 0.6, 1.4);
    return site.rock.rgb * grain * (1.0 + site.rock.ledge * smoothstep(0.12, 0.3, slope));
}

struct CliffRock {
    albedo: vec3<f32>,
    normal: vec3<f32>,
    roughness: f32,
}

// A rock piece's face at `p` (world), facing `n` (the face's own flat normal: what
// tells a ledge from a riser): the wall's stone as the terrain draws it there, with
// what weathers onto a block standing out of it. `px` is the pixel's size and `dz`
// the height it spans, in metres.
fn cliff_rock(p: vec3<f32>, n: vec3<f32>, px: f32, dz: f32, dpx: vec3<f32>, dpy: vec3<f32>) -> CliffRock {
    let face = n;
    var out: CliffRock;
    let xy = p.xy;
    let alt = p.z - globals.map.z;
    let scan = terrain_surface_grad(p, n, 17.0, MAT_ROCK_FACE, 1.25, dpx, dpy);
    out.normal = scan.normal;
    out.roughness = clamp(scan.roughness, 0.7, 0.95);
    let fields = habitat_fields(xy);
    // A block's faces are as steep as the wall it stands for; its tops are ledges.
    let riser = 1.0 - smoothstep(0.55, 0.8, face.z);
    let slope = mix(0.12, 1.0, riser);
    let layer = ground_snow_at(xy);
    var rgb = cliff_stone(xy, p.z, slope, scan.color, fields.w, fields.z, layer.z > 0.5);
    let arid = desert_at(xy, px);
    if arid > 0.0 {
        let site = canyon_site(xy, alt, n, px, dz);
        var canyon = canyon_cliff_stone(site, scan.color, slope);
        // Desert varnish streaking down the risers, as on the walls.
        let v = smoothstep(0.44, 0.72, site.streak * 0.55 + site.streak_wide * 0.6 - 0.07);
        canyon = mix(canyon, CANYON_VARNISH + canyon * 0.18, v * site.rock.varnish * riser * 0.75);
        // Sand blown onto the ledges.
        canyon = mix(canyon, site.sand * 0.9, (1.0 - riser) * 0.45);
        // The bathtub ring's crust below the old full-pool line.
        let mineral = dot(scan.color, vec3<f32>(0.2126, 0.7152, 0.0722));
        let ring = canyon_ring(xy, alt, riser, site.streak, dz);
        let crust = CANYON_CRUST * clamp(pow(mineral / 0.07, 0.3), 0.75, 1.2);
        canyon = mix(canyon, crust, ring.crust * 0.85);
        rgb = side_mix3(rgb, canyon, arid);
        out.roughness = mix(out.roughness, mix(0.9, 0.62, v * riser), arid);
    }
    // Snow lies on the ledges wherever the ground round it has snow.
    let by_height = smoothstep(350.0, 450.0, alt + (fields.w - 0.5) * 95.0);
    let snow = mix(by_height, smoothstep(0.3, 0.7, layer.y + (fields.z - 0.5) * 0.55), layer.z)
        * smoothstep(0.6, 0.85, face.z) * (1.0 - arid);
    rgb = mix(rgb, vec3<f32>(0.65, 0.69, 0.74), snow);
    out.albedo = rgb;
    return out;
}
