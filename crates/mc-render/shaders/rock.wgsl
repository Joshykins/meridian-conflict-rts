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

// ---- Crags: the mountains' walls broken into fractured rock ------------------
// Terrain nodes finer than the 8 m samples (terrain.rs `select_nodes`, over the
// blocks `CliffBlocks` marks) move each vertex of a steep wall out of it or back
// into it by `crag_relief`: the wall's surface becomes granite split along joints.

// Three random numbers in 0..1 for a lattice point.
fn crag_hash3(i: vec3<f32>) -> vec3<f32> {
    var q = vec3<f32>(dot(i, vec3<f32>(127.1, 311.7, 74.7)), dot(i, vec3<f32>(269.5, 183.3, 246.1)),
        dot(i, vec3<f32>(113.5, 271.9, 124.6)));
    return fract(sin(q) * 43758.5453);
}

// Smooth value noise in 3D, 0..1, one lattice cell a unit.
fn crag_noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = p - i;
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(mix(crag_hash3(i).x, crag_hash3(i + vec3<f32>(1.0, 0.0, 0.0)).x, u.x),
        mix(crag_hash3(i + vec3<f32>(0.0, 1.0, 0.0)).x, crag_hash3(i + vec3<f32>(1.0, 1.0, 0.0)).x, u.x), u.y);
    let b = mix(mix(crag_hash3(i + vec3<f32>(0.0, 0.0, 1.0)).x, crag_hash3(i + vec3<f32>(1.0, 0.0, 1.0)).x, u.x),
        mix(crag_hash3(i + vec3<f32>(0.0, 1.0, 1.0)).x, crag_hash3(i + vec3<f32>(1.0, 1.0, 1.0)).x, u.x), u.y);
    return mix(a, b, u.z);
}

// How far granite stands out of a wall at `p`, metres, before weighting. Joint
// blocks: space is cut into cells (taller than wide, as joints in granite run
// down a face), each cell's rock a plane tilted its own way, and the rock is the
// smooth least of them: flat facets that meet in creases at the joints, never a
// step. Over them, ribs and gullies running down the face, and a rough skin.
fn crag_depth(p: vec3<f32>) -> f32 {
    let scale = vec3<f32>(1.0 / 10.0, 1.0 / 10.0, 1.0 / 16.0);
    // The eight cells round the point: the corner it is nearest.
    let cell = floor(p * scale - 0.5);
    // Smooth minimum (exponential) over their planes, each rising away from its
    // own site so it only rules near it.
    var sum = 0.0;
    let k = 1.2;
    for (var z = 0; z <= 1; z++) {
        for (var y = 0; y <= 1; y++) {
            for (var x = 0; x <= 1; x++) {
                let c = cell + vec3<f32>(f32(x), f32(y), f32(z));
                let h = crag_hash3(c);
                let site = (c + 0.15 + 0.7 * h) / scale;
                let tilt = (crag_hash3(c + 17.0) - 0.5) * vec3<f32>(0.6, 0.6, 0.45);
                let off = p - site;
                let v = dot(off, tilt) + (h.z - 0.5) * 2.5 + 0.035 * dot(off * scale * 10.0, off * scale * 10.0);
                sum += exp(-v / k);
            }
        }
    }
    let facets = 2.5 + k * log(max(sum, 1e-6));
    let ribs = crag_noise3(p / vec3<f32>(7.0, 7.0, 34.0)) - 0.5;
    let skin = crag_noise3(p / 2.6) - 0.5;
    return clamp(facets, -3.0, 3.0) + ribs * 2.5 + skin * 0.6;
}

// A ridged field, 0-1: sharp crests where the noise crosses its middle, broad
// troughs between.
fn crag_ridged(p: vec3<f32>) -> f32 {
    let v = 1.0 - abs(crag_noise3(p) * 2.0 - 1.0);
    return v * v;
}

// The crag's mass, metres out of the wall: aretes and ribs with gullies between
// them running down the face (ridged, so the crests are sharp and the gullies
// creased), from tens of metres across down to a few. Smooth enough for 8 m
// cells at its coarsest, so it is drawn wherever the crag is.
fn crag_mass(p: vec3<f32>) -> f32 {
    let arete = crag_ridged(p / vec3<f32>(26.0, 26.0, 90.0));
    let rib = crag_ridged(p / vec3<f32>(9.0, 9.0, 30.0) + 7.0);
    let bulge = crag_noise3(p / 55.0 + 31.0) - 0.5;
    return (arete - 0.35) * 16.0 + (rib - 0.35) * 5.0 + bulge * 6.0;
}

struct Crag {
    // How far the vertex moves up or down, metres.
    lift: f32,
    // How much of a crag it is, 0-1; how far the rock stands out of the wall
    // there, metres (the fragment shader darkens the clefts by it); and how much
    // of the fine facets it carries.
    weight: f32,
    depth: f32,
    fine: f32,
}

// The relief at a vertex at `p` on the 8 m surface, `dist` metres from the eye.
// The crag's mass everywhere the crag field (`ground_crag_at`) has it outside the
// desert, faded out by 4 km; the fractured facets (`crag_depth`) only on terrain
// drawn finer than its samples, faded out by `fade` metres (`TerrainNode::morph.z`,
// zero elsewhere). The vertex moves up or down, never sideways, so the terrain
// stays a heightfield and no triangle can fold over: rock standing `depth` out of
// a wall rising `grade` is the wall raised by `depth * grade`.
fn crag_relief(p: vec3<f32>, dist: f32, fade: f32) -> Crag {
    var c: Crag;
    c.weight = ground_crag_at(p.xy) * (1.0 - desert_at(p.xy, 0.0)) * (1.0 - smoothstep(2500.0, 4000.0, dist));
    if c.weight <= 0.004 {
        c.weight = 0.0;
        return c;
    }
    let n = terrain_normal(p.xy, 12.0);
    let grade = length(n.xy) / max(n.z, 0.05);
    c.weight *= smoothstep(0.5, 1.0, grade);
    c.depth = crag_mass(p);
    var facets = 0.0;
    if fade > 0.0 {
        c.fine = c.weight * (1.0 - smoothstep(fade * 0.65, fade, dist));
        if c.fine > 0.0 {
            facets = crag_depth(p) * c.fine / c.weight;
        }
    }
    c.depth += facets;
    // The facets are a few metres across: on the steepest walls their lift is
    // held down, or a 2 m cell would stand on end and face nothing but shade.
    c.lift = (c.depth - facets) * min(grade, 3.0) * c.weight + facets * min(grade, 1.8) * c.weight;
    return c;
}
