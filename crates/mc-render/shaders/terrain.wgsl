//!use bindings
//!use shore
//!use habitat
//!use streets
//!use desert
//!use rock
//!use scars
// Terrain: CDLOD quadtree patches over a streamed heightmap.
//
// Every node draws the same 64x64 grid. At the finest level the grid vertices
// sit exactly on height samples and each cell is split along the (x,y)-(x+1,y+1)
// diagonal, which is the triangulation the simulation collides with.

struct Node {
    // xy origin, z size in metres, w level
    rect: vec4<f32>,
    // x morph start distance, y morph end distance
    morph: vec4<f32>,
}

struct TerrainPush {
    // 1: shadow pass (use the shadow matrix)
    pass_kind: u32,
    // 1: draw the build grid overlay
    build_grid: u32,
}

@group(1) @binding(0) var<storage, read> nodes: array<Node>;
var<immediate> push: TerrainPush;

const GRID: f32 = 64.0;

struct VsOut {
    // Invariant: the depth pre-pass and the colour pass must land on the same depth.
    @builtin(position) @invariant clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    // On a crag (`crag_relief`): how much of one it is, how much of its fine
    // facets it carries, and how far its mass and its facets lift the wall.
    @location(1) crag: vec4<f32>,
}

@vertex
fn vs_main(@location(0) grid: vec2<f32>, @builtin(instance_index) instance: u32) -> VsOut {
    let node = nodes[instance];
    let size = node.rect.z;
    var xy = node.rect.xy + grid * (size / GRID);

    // CDLOD morph: slide odd vertices onto their even neighbours as the node
    // approaches the distance where its parent takes over.
    let eye = globals.camera.xyz;
    let approx = vec3<f32>(xy, terrain_height(xy));
    let dist = distance(approx, eye);
    let k = clamp((dist - node.morph.x) / max(node.morph.y - node.morph.x, 1.0), 0.0, 1.0);
    let odd = fract(grid * 0.5) * 2.0;
    xy = xy - odd * (size / GRID) * k;
    xy = clamp(xy, vec2<f32>(0.0), globals.map.xy);

    var world = vec3<f32>(xy, terrain_height(xy));
    var out: VsOut;
    // Crags finer than the samples: the wall broken into rock (rock.wgsl).
    let crag = crag_relief(world, distance(world, eye), node.morph.z);
    world.z += crag.lift;
    out.crag = vec4<f32>(crag.weight, crag.fine, crag.mass_lift, crag.facet_lift);
    if (push.pass_kind & PASS_KIND_MASK) == PASS_SHADOW {
        out.clip = globals.shadow_cascades[push.pass_kind >> PASS_CASCADE_SHIFT] * vec4<f32>(world, 1.0);
    } else {
        out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    }
    out.world = world;
    return out;
}

@fragment
fn fs_shadow() {
}

// Gradient noise with its derivative: (value in [0, 1], d/dx, d/dy per metre).
// Same lattice and hash as grad_noise2, so the two can be mixed.
fn grad_noise2_d(xy: vec2<f32>, cell: f32) -> vec3<f32> {
    let p = noise_lattice(xy, cell);
    let i = floor(p);
    let f = p - i;
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let du = 30.0 * f * f * (f * (f - 2.0) + 1.0);
    let g00 = grad_vec(i);
    let g10 = grad_vec(i + vec2<f32>(1.0, 0.0));
    let g01 = grad_vec(i + vec2<f32>(0.0, 1.0));
    let g11 = grad_vec(i + vec2<f32>(1.0, 1.0));
    let v00 = dot(g00, f);
    let v10 = dot(g10, f - vec2<f32>(1.0, 0.0));
    let v01 = dot(g01, f - vec2<f32>(0.0, 1.0));
    let v11 = dot(g11, f - vec2<f32>(1.0, 1.0));
    let v = v00 + u.x * (v10 - v00) + u.y * (v01 - v00) + u.x * u.y * (v00 - v10 - v01 + v11);
    let d = g00 + u.x * (g10 - g00) + u.y * (g01 - g00) + u.x * u.y * (g00 - g10 - g01 + g11)
        + du * vec2<f32>(u.y * (v00 - v10 - v01 + v11) + (v10 - v00), u.x * (v00 - v10 - v01 + v11) + (v01 - v00));
    let k = 0.70710678 * 0.5;
    return vec3<f32>(clamp(v * k + 0.5, 0.0, 1.0), d * (k / cell));
}

// Real-world repeat of each ground scan in metres, from Poly Haven's
// dimensions, stretched a little where the true size would be sub-pixel from
// a battle camera. Indexed by material number (colour layer / 2).
fn ground_period(m: i32) -> f32 {
    switch m {
        case 0: { return 17.0; }  // rock face
        case 1: { return 2.2; }   // leafy grass
        case 2: { return 15.0; }  // aerial meadow
        case 3: { return 3.4; }   // mossy leaf-littered grass
        case 4: { return 2.6; }   // needle forest floor
        case 5: { return 3.6; }   // scree
        case 6: { return 5.0; }   // dry dirt
        case 7: { return 38.0; }  // aerial rocky highland
        case 8: { return 3.2; }   // sand and gravel
        default: { return 2.8; }  // mud
    }
}

// Scanned relief depth in metres, for the parallax trace.
fn ground_relief(m: i32) -> f32 {
    switch m {
        case 0: { return 0.60; }
        case 2: { return 0.30; }
        case 5: { return 0.12; }
        case 6: { return 0.10; }
        case 7: { return 1.40; }
        default: { return 0.06; }
    }
}

// Brings each scan to one coherent palette under this sun and exposure: raw
// photo albedo is two to three times brighter than the scene is lit for, and
// each was shot in different light. Mostly per channel, part luminance only,
// so the stones in a grass scan do not all turn green.
fn ground_tint(m: i32) -> vec3<f32> {
    switch m {
        case 1: { return vec3<f32>(0.231, 0.375, 0.274); }  // leafy grass
        case 2: { return vec3<f32>(0.552, 0.719, 0.980); }  // aerial meadow
        case 3: { return vec3<f32>(0.283, 0.406, 0.499); }  // mossy leaf litter
        case 4: { return vec3<f32>(0.196, 0.217, 0.242); }  // needle forest floor
        case 5: { return vec3<f32>(0.407, 0.468, 0.557); }  // scree
        case 6: { return vec3<f32>(0.405, 0.468, 0.579); }  // dry dirt
        case 7: { return vec3<f32>(0.633, 0.775, 1.392); }  // aerial rocky highland
        case 8: { return vec3<f32>(0.876, 1.152, 1.551); }  // sand and gravel
        default: { return vec3<f32>(0.465, 0.485, 0.746); } // mud
    }
}

// A scan's colour under this scene's palette. The aerial meadow covers most
// open ground, and its grey gravel patches came out pale blue-white under the
// tint: a bright blot repeating every 15 m, a grid of them from a battle
// camera. Those are pulled to the meadow's own olive, and its light and dark
// narrowed, so the scan gives texture without spots.
fn ground_albedo(p: TerrainPatch, m: i32) -> vec3<f32> {
    let rgb = p.color.rgb * ground_tint(m);
    if m != 2 {
        return rgb;
    }
    let lum = max(dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722)), 1e-5);
    let hi = max(rgb.r, max(rgb.g, rgb.b));
    let sat = (hi - min(rgb.r, min(rgb.g, rgb.b))) / max(hi, 1e-5);
    // The tinted scan's median brightness and mean hue.
    let median = 0.0813;
    let olive = vec3<f32>(1.110, 1.037, 0.314);
    let pale = 1.0 - smoothstep(0.58, 0.78, sat);
    let hue = mix(rgb / lum, olive, pale * 0.85);
    return hue * median * pow(lum / median, 0.55);
}

// The tropical palette (`Habitat::tropical`): bright coral-cream beach sand with the
// scan's gravel softened out of it, and lush, saturated greens instead of the
// temperate olive. `temperate` is `ground_albedo`'s colour; other materials
// (rock, dirt, mud, forest floor) keep it.
const TROPIC_SAND: vec3<f32> = vec3<f32>(0.66, 0.50, 0.36);
fn tropical_albedo(p: TerrainPatch, m: i32, temperate: vec3<f32>) -> vec3<f32> {
    let luma = vec3<f32>(0.2126, 0.7152, 0.0722);
    if m == 8 {
        // The raw scan's median brightness is 0.177; its light and dark kept
        // at a third of their strength, so the beach reads as fine sand.
        let lum = max(dot(p.color.rgb, luma), 1e-4);
        return TROPIC_SAND * pow(lum / 0.177, 0.35);
    }
    var lush = vec3<f32>(0.0);
    switch m {
        case 1: { lush = vec3<f32>(0.40, 1.24, 0.24); }  // leafy grass
        case 2: { lush = vec3<f32>(0.56, 1.20, 0.20); }  // aerial meadow
        case 3: { lush = vec3<f32>(0.42, 1.20, 0.30); }  // mossy leaf litter
        default: { return temperate; }
    }
    let lum = max(dot(temperate, luma), 1e-5);
    let hue = mix(temperate / lum, lush / dot(lush, luma), 0.65);
    return hue * lum * 1.15;
}

// One ground material seen from above: three rotated patches of the scan
// (terrain_projection), so no tile repeats. `d` is the pixel footprint
// derivative of world xy.
fn ground_material(xy: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>, m: i32, ray: vec2<f32>, fade: f32) -> TerrainPatch {
    let period = ground_period(m);
    let relief = ground_relief(m) / period * fade;
    return terrain_projection(xy / period, dx / period, dy / period, m * 2, ray, relief);
}

struct Stones {
    // How much of the pixel is stone, and the stone's surface gradient (dz/dx, dz/dy).
    cover: f32,
    grad: vec2<f32>,
    // Contact shadow in the soil around each stone, 1 = none.
    ao: f32,
    tone: f32,
}

// Angular stones on a jittered grid: one candidate per `cell` metres, kept
// with probability `density`, each fully inside its cell so only one cell is
// ever evaluated. A stone is a jittered convex polygon: a flat, weathered top
// and planar faces sloping down to the soil, so each side catches the sun
// differently the way broken rock does. `px` is the pixel footprint in metres.
fn stones(xy: vec2<f32>, cell: f32, density: f32, px: f32, seed: f32) -> Stones {
    var out: Stones;
    out.ao = 1.0;
    out.tone = 0.5;
    let p = xy / cell;
    let id = floor(p);
    let h = hash21(id + seed);
    if h > density {
        return out;
    }
    let f = p - id;
    let r = 0.08 + 0.22 * pow(hash21(id + seed + 17.3), 1.6);
    let c = vec2<f32>(hash21(id + seed + 3.1), hash21(id + seed + 7.7)) * (1.0 - 3.2 * r) + 1.6 * r;
    let rel = (f - c) / r;
    // Six faces round the stone, unevenly spaced and set back.
    var d = -1.0;
    var face = vec2<f32>(0.0);
    let turn = hash21(id + seed + 11.9) * 6.2831853;
    for (var i = 0; i < 6; i++) {
        let fi = f32(i);
        let angle = turn + fi * 1.0471976 + (hash21(id + seed + fi * 3.7) - 0.5) * 0.8;
        let dir = vec2<f32>(cos(angle), sin(angle));
        let setback = 0.7 + 0.45 * hash21(id + seed + fi * 5.3 + 1.1);
        let di = dot(rel, dir) / setback;
        if di > d {
            d = di;
            face = dir / setback;
        }
    }
    let width = px / (cell * r) * 1.5;
    out.cover = 1.0 - smoothstep(1.0 - width, 1.0 + width, d);
    // Flat top inside half the radius, then the face slopes down to the soil.
    let side = smoothstep(0.35, 0.6, d);
    let steepness = 0.9 + 0.8 * hash21(id + seed + 29.3);
    out.grad = -face * steepness * side * out.cover;
    out.ao = 1.0 - 0.3 * (1.0 - smoothstep(1.0, 1.35, d)) * (1.0 - out.cover);
    out.tone = hash21(id + seed + 23.1);
    return out;
}

// ---- Glacier ice -------------------------------------------------------------------

struct IceShade {
    rgb: vec3<f32>,
    // Added to the shading normal, world space.
    bend: vec3<f32>,
    glow: vec3<f32>,
    rough: f32,
}

// Broken ice: cells one unit across. x how far from the nearest crack (F2 - F1, in
// cells), y the nearest cell's own random number, z how far from its middle (F1).
fn ice_cells(p: vec2<f32>) -> vec3<f32> {
    let i = floor(p);
    let f = p - i;
    var d1 = 8.0;
    var d2 = 8.0;
    var id = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let g = vec2<f32>(f32(x), f32(y));
            let h = i + g;
            let r = g + vec2<f32>(hash21(h), hash21(h + 19.7)) * 0.8 + 0.1 - f;
            let d = dot(r, r);
            if d < d1 {
                d2 = d1;
                d1 = d;
                id = hash21(h + 7.3);
            } else if d < d2 {
                d2 = d;
            }
        }
    }
    return vec3<f32>(sqrt(d2) - sqrt(d1), id, sqrt(d1));
}

// What the ice's surface shows, laid along one fixed direction of flow.
struct IceFlowTex {
    // Bands of clear ice drawn out along the flow, rubble stripes and grime
    // patches (noise, thresholded by the caller).
    clear: f32,
    stripe: f32,
    grime: f32,
    // Gaps between broken blocks (big and small, pixel cover), and the fresh
    // faces beside them.
    gap: f32,
    gap2: f32,
    face: f32,
    // How near its block's middle (1 in the middle), and the block's own random.
    cap: f32,
    shade: f32,
    // How the block leans, world space.
    tilt: vec2<f32>,
    // Crevasses: the slot (pixel cover), its lit lip, how dark it is inside (by
    // how wide it opens), and which way its walls lean (world space).
    crevasse: f32,
    lip: f32,
    deep: f32,
    lean: vec2<f32>,
    // Ogives: arcs of darker ice down the tongue.
    ogive: f32,
}

// The flow's direction is quantised to twelve fixed headings and the two nearest are
// blended: a frame that turned with the flow would shear anything laid in world
// coordinates into marbling, the further from the origin the worse.
fn ice_flow_tex(xy: vec2<f32>, heading: f32, meander: f32, bow: f32, px: f32, broken: f32, tension: f32) -> IceFlowTex {
    let a = heading * 0.2617994;
    let down = vec2<f32>(cos(a), sin(a));
    let side = vec2<f32>(-down.y, down.x);
    let u = dot(xy, side) + meander + heading * 131.0;
    let v = dot(xy, down) + heading * 71.0;
    var t: IceFlowTex;
    let band = grad_noise2(vec2<f32>(u, v * 0.07) + 71.0, 8.0);
    let band_wide = grad_noise2(vec2<f32>(u, v * 0.05) - 19.0, 34.0);
    t.clear = band * 0.55 + band_wide * 0.45;
    t.stripe = grad_noise2(vec2<f32>(u, v * 0.03) - 213.0, 48.0);
    t.grime = grad_noise2(vec2<f32>(u, v * 0.3) + 90.0, 45.0);
    // Blocks drawn out across the flow, as the slots between them run.
    let warp = vec2<f32>(grad_noise2(xy, 21.0), grad_noise2(xy + 5.0, 21.0)) - 0.5;
    let q = vec2<f32>(u / 15.0, v / 9.0) + warp * 0.7;
    let c1 = ice_cells(q);
    let c2 = ice_cells(q * 2.7 + 17.0);
    let pc = px / 9.0;
    t.gap = crater_crack(c1.x, 0.03 + 0.13 * broken, pc) * (0.12 + 0.88 * broken);
    t.gap2 = crater_crack(c2.x, 0.04, pc * 2.7) * smoothstep(0.5, 0.9, broken) * 0.45;
    t.face = (1.0 - smoothstep(0.0, 0.26, c1.x)) * broken * (1.0 - t.gap);
    t.cap = 1.0 - smoothstep(0.16, 0.48, c1.z + (c1.y - 0.5) * 0.3);
    t.shade = c1.y;
    let lean = vec2<f32>(hash11(c1.y * 91.0), hash11(c1.y * 37.0 + 3.0)) - 0.5 + (c2.y - 0.5) * 0.35;
    t.tilt = side * lean.x + down * lean.y;

    // Crevasses open across the flow some 30 m apart, bowed and wandering, each row
    // broken into offset pieces; more of them open the harder the ice is pulled.
    let phase = v / 30.0 + bow * 1.3;
    let row = floor(phase);
    let edge = min(fract(phase), 1.0 - fract(phase)) * 2.0;
    let piece = grad_noise2(vec2<f32>(u + row * 41.0, row * 17.0), 55.0);
    let width = 0.08 + 0.2 * hash21(vec2<f32>(row, floor(u / 34.0)));
    let kept = smoothstep(0.6 - 0.2 * tension, 0.68 - 0.2 * tension, piece) * tension;
    let aa = px / 30.0 * 2.0;
    // Too thin for the pixel, a field of them still darkens the ice by the share they open.
    let far = smoothstep(0.5, 1.4, aa / width);
    t.crevasse = mix(1.0 - smoothstep(width * 0.55 - aa, width + aa, edge), width * 0.7, far) * kept;
    t.lip = (1.0 - smoothstep(width, width * 1.9 + aa, edge)) * kept * (1.0 - t.crevasse) * (1.0 - far);
    t.deep = smoothstep(0.08, 0.2, width);
    t.lean = down * (fract(phase) - 0.5) * t.crevasse * (1.0 - far);
    t.ogive = smoothstep(0.25, 0.75, abs(fract(v / 70.0 + bow * 0.6) - 0.5) * 2.0);
    return t;
}

fn ice_flow_mix(a: IceFlowTex, b: IceFlowTex, w: f32) -> IceFlowTex {
    var t: IceFlowTex;
    // Noise blended as is flattens toward grey, and thresholds on it stop firing:
    // keep its contrast.
    let k = inverseSqrt(w * w + (1.0 - w) * (1.0 - w));
    t.clear = 0.5 + (mix(a.clear, b.clear, w) - 0.5) * k;
    t.stripe = 0.5 + (mix(a.stripe, b.stripe, w) - 0.5) * k;
    t.grime = 0.5 + (mix(a.grime, b.grime, w) - 0.5) * k;
    t.gap = mix(a.gap, b.gap, w);
    t.gap2 = mix(a.gap2, b.gap2, w);
    t.face = mix(a.face, b.face, w);
    t.cap = mix(a.cap, b.cap, w);
    t.shade = mix(a.shade, b.shade, w);
    t.tilt = mix(a.tilt, b.tilt, w);
    t.crevasse = mix(a.crevasse, b.crevasse, w);
    t.lip = mix(a.lip, b.lip, w);
    t.deep = mix(a.deep, b.deep, w);
    t.lean = mix(a.lean, b.lean, w);
    t.ogive = mix(a.ogive, b.ogive, w);
    return t;
}

// An ice face seen straight on: `h` along it, `z` up it, `side` the way `h` runs.
fn ice_face(h: f32, z: f32, side: vec2<f32>, pz: f32, dz: f32, top: f32, sheer: f32, fine: f32, snow_rgb: vec3<f32>) -> IceShade {
    // Split into tall columns and slabs, cracked finer inside each.
    let wq = vec2<f32>(h / 8.0, z / 17.0) + vec2<f32>(grad_noise2(vec2<f32>(h, z * 0.5), 23.0) - 0.5, 0.0) * 0.9;
    let w1 = ice_cells(wq);
    let w2 = ice_cells(vec2<f32>(h / 2.6, z / 4.4) + 31.0);
    let crack = crater_crack(w1.x, 0.05, pz / 8.0);
    let crack2 = crater_crack(w2.x, 0.05, pz / 2.6) * 0.55;
    let pick = w1.y;
    // Clear blue, shattered white, old dirty grey, and now and then deep blue.
    var face = mix(vec3<f32>(0.08, 0.3, 0.46), vec3<f32>(0.18, 0.46, 0.6), fract(pick * 7.0));
    face = mix(face, vec3<f32>(0.55, 0.64, 0.7), smoothstep(0.52, 0.76, pick));
    face = mix(face, vec3<f32>(0.27, 0.29, 0.3), smoothstep(0.86, 0.95, pick));
    face = mix(face, vec3<f32>(0.03, 0.15, 0.27), 1.0 - smoothstep(0.0, 0.14, pick));
    face *= 0.85 + 0.3 * w2.y;
    // Darker into its cracks.
    face *= 0.7 + 0.3 * smoothstep(0.0, 0.35, w1.x);
    // A few thin layers of dirt across it, wavering, unevenly spaced; only on a sheer
    // face (on a mere slope they would read as contour lines).
    let lz = z + (grad_noise2(vec2<f32>(h, 0.0) + 7.0, 70.0) - 0.5) * 18.0
        + (grad_noise2(vec2<f32>(h, z) * 0.5, 9.0) - 0.5) * 2.5;
    let li = floor(lz / 11.0);
    let lf = fract(lz / 11.0);
    let thick = 0.025 + 0.05 * hash11(li * 3.1 + 0.7);
    let dirt_layer = (1.0 - smoothstep(thick, thick + 0.03 + dz / 11.0, abs(lf - 0.5)))
        * step(0.5, hash11(li * 7.7 + 1.3)) * sheer;
    face = mix(face, vec3<f32>(0.1, 0.09, 0.08), dirt_layer * 0.8);
    // Melt streaks run down it.
    let streak = smoothstep(0.55, 0.85, grad_noise2(vec2<f32>(h / 1.4, z / 28.0), 1.0));
    face *= 1.0 - 0.25 * streak;
    // Slabs that lean back hold snow on their ledges; snow along the lip.
    let lean = hash11(pick * 53.0 + 2.0);
    let ledge = smoothstep(0.8, 0.92, lean);
    let snowed = max(ledge * 0.85, top * 0.7);
    face = mix(face, snow_rgb, snowed);
    face = mix(face, vec3<f32>(0.02, 0.07, 0.12), max(crack, crack2));
    var out: IceShade;
    out.rgb = face;
    out.rough = mix(0.28, 0.7, snowed);
    // Each slab faces its own way.
    out.bend = vec3<f32>(side * (hash11(pick * 11.0) - 0.5) * 1.1, (lean - 0.5) * 0.9)
        * (1.0 - smoothstep(0.4, 1.0, pz / 8.0));
    let clear_face = (1.0 - smoothstep(0.52, 0.76, pick)) * (1.0 - snowed);
    out.glow = vec3<f32>(0.012, 0.05, 0.075) * clear_face;
    return out;
}

// Glacier ice as it flows downhill:
// * bare ice, grey-blue under a weathered crust, bluer where clear bands of it are
//   drawn out along the flow, arcs of light and dark ice bowed down the tongue;
// * broken ice: seracs, blocks split off where it is pulled apart (in icefalls, along
//   its edges and in fields), blue in their fresh faces and dark down the gaps,
//   capped with snow;
// * crevasses: slots across the flow where it steepens, some bridged by snow;
// * dirt: rubble in stripes along the flow, grime spreading down the tongue, dark
//   specks up close, grey moraine along the edges;
// * snow: drifts on the ice, and firn above the glacier's snow line;
// * faces (margins, calving fronts): columns and slabs of blue, white and dirty ice,
//   a few layers of dirt across them, melt streaks, snow on ledges and the lip.
fn glacier_shade(xy: vec2<f32>, z: f32, alt: f32, base_n: vec3<f32>, cover: f32, fine: f32,
    patchy: f32, px: f32, dz: f32, snow_rgb: vec3<f32>) -> IceShade {
    // The flow, read from the ground over some 140 m: the crown and
    // the blocks the ice breaks into must not turn it with every hummock.
    let flow_n = terrain_normal(xy, 70.0);
    let fall = length(flow_n.xy);
    // Rise over run (1 - n.z is far smaller on gentle ice).
    let grade = fall / max(flow_n.z, 0.05);
    let local_grade = length(base_n.xy) / max(base_n.z, 0.05);
    let down = select(vec2<f32>(0.0, -1.0), -flow_n.xy / max(fall, 1e-4), fall > 1e-3);
    let flowing = smoothstep(0.003, 0.02, fall);
    let steep = smoothstep(0.08, 0.3, grade);
    let wall = smoothstep(0.75, 1.4, local_grade);
    // How far in from the ice's edge.
    let inner = smoothstep(0.55, 0.97, cover + (fine - 0.5) * 0.2);
    let high = smoothstep(260.0, 560.0, alt + (patchy - 0.5) * 120.0);
    let low = 1.0 - smoothstep(60.0, 320.0, alt);

    // Where it is broken into blocks: icefalls, fields of it, along the edges.
    let icefall = smoothstep(0.18, 0.5, grade) * (1.0 - wall);
    let shattered = smoothstep(0.5, 0.66, grad_noise2(xy + 911.0, 210.0));
    let broken = max(max(icefall, shattered), (1.0 - inner) * 0.85);

    // Crevasses open where the ice steepens and is pulled apart, in fields where it
    // is stretched round a bend or over a hump, and toward the sea, where the tongue
    // speeds up and is torn apart before it calves. (Its edges break into blocks
    // instead.) Laid in the flow's frame, not by height: contours of hummocky ice
    // loop into whorls.
    let calving = 1.0 - smoothstep(40.0, 220.0, alt);
    let field = smoothstep(0.5, 0.66, grad_noise2(xy - 301.0, 260.0));
    let tension = max(max(smoothstep(0.08, 0.2, grade), calving * 0.9), field * 0.8)
        * (1.0 - smoothstep(0.5, 0.8, grade)) * flowing * (1.0 - wall) * (0.3 + 0.7 * inner);

    let meander = grad_noise2(xy + 37.0, 150.0) * 36.0 + grad_noise2(xy - 11.0, 31.0) * 5.0;
    let bow = grad_noise2(xy + 37.0, 90.0) * 0.8 + grad_noise2(xy - 11.0, 23.0) * 0.15;
    let heading = fract(atan2(down.y, down.x) / 3.14159265) * 12.0;
    let h0 = floor(heading);
    let tex = ice_flow_mix(
        ice_flow_tex(xy, h0, meander, bow, px, broken, tension),
        ice_flow_tex(xy, h0 + 1.0, meander, bow, px, broken, tension),
        smoothstep(0.25, 0.75, heading - h0));

    // ---- Bare ice ----
    let clear = smoothstep(0.45, 0.8, tex.clear);
    var rgb = mix(vec3<f32>(0.36, 0.42, 0.47), vec3<f32>(0.15, 0.33, 0.47), clear * 0.85);
    rgb *= 0.86 + 0.26 * fine;
    var rough = mix(0.5, 0.26, clear);
    var bend = vec3<f32>(0.0);

    // Ogives: arcs of darker ice down the tongue, below where it fell through an icefall.
    rgb *= 1.0 - 0.16 * tex.ogive * flowing * (1.0 - steep) * (1.0 - high);

    // Grime spreading down the tongue, patchy; dust melted into little pits up close.
    let grime = low * smoothstep(0.3, 0.7, tex.grime + (fine - 0.5) * 0.3 + low * 0.15);
    rgb = mix(rgb, vec3<f32>(0.24, 0.23, 0.21) * (0.8 + 0.4 * fine), grime * 0.7);
    let speck = smoothstep(0.8, 0.92, grad_noise2(xy + 3.3, 1.3)) * (1.0 - smoothstep(0.08, 0.25, px));
    rgb = mix(rgb, vec3<f32>(0.08, 0.08, 0.08), speck * (0.3 + 0.5 * low));
    // High up, snow bridges some crevasses.
    let bridged = step(0.55, grad_noise2(xy + 71.0, 19.0)) * high;

    // ---- Broken ice ----
    let blocks = 1.0 - smoothstep(0.3, 0.8, px / 9.0);
    rgb *= mix(1.0, 0.8 + 0.36 * tex.shade, broken * blocks);
    rgb = mix(rgb, vec3<f32>(0.16, 0.42, 0.56) * (0.85 + 0.3 * tex.shade), tex.face * 0.7);
    // Snow caps each block, heavier higher up.
    let snowy = clamp(0.3 + 0.7 * high + 0.3 * (patchy - 0.5), 0.0, 1.0);
    let cap = smoothstep(0.0, 0.3, tex.cap + (fine - 0.5) * 0.25 - 0.4) * broken * snowy * (1.0 - wall);
    rgb = mix(rgb, snow_rgb * 1.05, cap * 0.9);
    bend += vec3<f32>(tex.tilt * 1.4 * broken * blocks, 0.0);

    // ---- Snow on the ice ----
    let drifted = smoothstep(0.62, 0.82, grad_noise2(xy + meander - 57.0, 38.0) + high * 0.3 - low * 0.2 - steep * 0.3 - grime * 0.5)
        * (1.0 - wall);
    rgb = mix(rgb, snow_rgb, drifted * 0.85);
    let firn = smoothstep(560.0, 640.0, alt + (patchy - 0.5) * 90.0 - steep * 60.0) * (1.0 - wall);
    rgb = mix(rgb, snow_rgb, firn * 0.8);
    let snow_on = max(max(cap, drifted * 0.85), firn * 0.8);
    rough = mix(rough, 0.72, snow_on);

    // Rubble stripes and moraine lie over the snow.
    let stripe = smoothstep(0.64, 0.74, tex.stripe + (fine - 0.5) * 0.12) * smoothstep(0.004, 0.015, fall) * (1.0 - wall);
    rgb = mix(rgb, vec3<f32>(0.11, 0.1, 0.09) * (0.75 + 0.5 * fine), stripe * 0.9);
    let margin = (1.0 - smoothstep(0.55, 0.92, cover + (fine - 0.5) * 0.35)) * (1.0 - wall);
    rgb = mix(rgb, vec3<f32>(0.15, 0.14, 0.13) * (0.7 + 0.6 * patchy), margin * 0.7);
    rough = mix(rough, 0.92, max(stripe, margin) * 0.8);

    // Down the gaps and slots: deep blue, darker the wider they open.
    let down_in = max(tex.gap, tex.gap2) * (1.0 - wall);
    rgb = mix(rgb, vec3<f32>(0.02, 0.08, 0.15), down_in);
    rgb = mix(rgb, vec3<f32>(0.74, 0.8, 0.85), tex.lip * 0.5);
    let slot = tex.crevasse * (1.0 - 0.8 * bridged);
    rgb = mix(rgb, mix(vec3<f32>(0.04, 0.17, 0.29), vec3<f32>(0.01, 0.04, 0.09), tex.deep), slot);
    rgb = mix(rgb, snow_rgb * 0.92, tex.crevasse * bridged * 0.8);
    // A slot's walls lean in across the flow.
    bend += vec3<f32>(tex.lean * (1.0 - 0.8 * bridged) * 1.6, 0.0);
    var glow = vec3<f32>(0.004, 0.02, 0.035) * (slot + down_in * 0.5);

    // ---- Faces ----
    // Seen from both sides it could face (triplanar): no frame to turn with the wall.
    if wall > 0.0 {
        let pz = max(px, dz);
        let top = smoothstep(0.45, 0.8, base_n.z + (fine - 0.5) * 0.25);
        let sheer = smoothstep(1.6, 3.0, local_grade);
        let wx = base_n.x * base_n.x;
        let wy = base_n.y * base_n.y;
        let k = wx / max(wx + wy, 1e-4);
        var f: IceShade;
        if k > 0.02 && k < 0.98 {
            let a = ice_face(xy.y, z, vec2<f32>(0.0, 1.0), pz, dz, top, sheer, fine, snow_rgb);
            let b = ice_face(xy.x, z, vec2<f32>(1.0, 0.0), pz, dz, top, sheer, fine, snow_rgb);
            let s = smoothstep(0.3, 0.7, k);
            f.rgb = mix(b.rgb, a.rgb, s);
            f.rough = mix(b.rough, a.rough, s);
            f.bend = mix(b.bend, a.bend, s);
            f.glow = mix(b.glow, a.glow, s);
        } else if k >= 0.98 {
            f = ice_face(xy.y, z, vec2<f32>(0.0, 1.0), pz, dz, top, sheer, fine, snow_rgb);
        } else {
            f = ice_face(xy.x, z, vec2<f32>(1.0, 0.0), pz, dz, top, sheer, fine, snow_rgb);
        }
        rgb = mix(rgb, f.rgb, wall);
        rough = mix(rough, f.rough, wall);
        bend += f.bend * wall;
        glow += f.glow * wall;
    }

    var out: IceShade;
    out.rgb = rgb;
    out.bend = bend;
    out.glow = glow;
    out.rough = rough;
    return out;
}

// The ground at `world` as seen at screen pixel `pixel`: `crag` is the vertex
// stage's crag relief (`VsOut::crag`, zero where none), `dpx`/`dpy` how `world`
// changes a pixel across and down. Drawn forward by `fs_main`, or once per pixel
// from the pre-pass's depth by `fs_lit` (renderer/terrain_lit.rs).
fn shade_ground(world: vec3<f32>, crag: vec4<f32>, pixel: vec2<f32>, dpx: vec3<f32>, dpy: vec3<f32>) -> vec4<f32> {
    let xy = world.xy;
    // Classify beaches and submerged ground from the same heightfield as
    // the ocean, not interpolated coarse-triangle height. Otherwise LOD
    // changes move the sand band independently of the true coastline.
    let ground_z = terrain_height(xy);
    let water = globals.map.z;
    let eye = globals.camera.xyz;
    let dist = distance(world, eye);
    let dx = dpx.xy;
    let dy = dpy.xy;
    let px = max(length(dx), length(dy));

    // Geometric normal from the heightmap; finer step up close.
    let step = clamp(dist * 0.004, 4.0, 48.0);
    let base_n = terrain_normal(xy, step);
    let slope = 1.0 - base_n.z;

    // On a crag the drawn surface is the rock (`crag_relief`): its facets are lit as
    // they lie, worked out per pixel (`crag_shade`), and it is all rock. The rock
    // stands tens of metres off the heightfield, so the ground there is classified
    // at the height it is drawn at, which is where the sea meets it: a sea cliff's
    // crag otherwise took the sand, silt and caustics of the shallows its
    // heightfield foot stands in.
    let crag_w = crag.x;
    let z = mix(ground_z, world.z, smoothstep(0.004, 0.1, crag_w));
    let alt = z - water;
    var facet = base_n;
    var crag_depth_here = 0.0;
    if crag_w > 0.004 {
        var c: Crag;
        c.weight = crag_w;
        c.fine = crag.y;
        c.mass_lift = crag.z;
        c.facet_lift = crag.w;
        let shade = crag_shade(xy, base_n, px, c);
        facet = shade.normal;
        crag_depth_here = shade.depth;
    }

    // ---- Where things grow (habitat.wgsl) -----------------------------------
    let hab = habitat(xy, z, base_n, px);
    let warp = hab.warp;
    let patchy = hab.patchy;
    let fine = hab.fine;
    let broad = hab.broad;
    let curvature = hab.curvature;
    let concavity = hab.concavity;
    let wet = hab.wet;
    let cover = hab.cover;
    let canopy = hab.canopy;
    let sand_w = hab.sand_w;
    let rock_face = hab.rock_face;
    let layer = hab.layer;
    let ice_w = hab.ice_w;
    let snow_w = hab.snow_w;
    let sward = hab.sward;
    var w = hab.w;

    // The three strongest materials, less the fourth: a material fades out
    // before it is dropped, so the choice never shows as a seam.
    var a = 1;
    var b = 1;
    var c = 1;
    var wa = -1.0;
    var wb = -1.0;
    var wc = -1.0;
    var wd = 0.0;
    for (var i = 1; i < 10; i++) {
        let v = w[i];
        if v > wa {
            wd = wc; c = b; wc = wb; b = a; wb = wa; a = i; wa = v;
        } else if v > wb {
            wd = wc; c = b; wc = wb; b = i; wb = v;
        } else if v > wc {
            wd = wc; c = i; wc = v;
        } else if v > wd {
            wd = v;
        }
    }
    var lw = max(vec3<f32>(wa, wb, wc) - wd, vec3<f32>(0.0));
    lw /= max(lw.x + lw.y + lw.z, 1e-4);

    let view = normalize(eye - world);
    let tangent_view = view - base_n * dot(view, base_n);
    let ray = (tangent_view / max(dot(view, base_n), 0.28)).xy;
    let near = 1.0 - smoothstep(65.0, 220.0, dist);
    let ga = ground_material(xy, dx, dy, a, ray, near);
    var gb = ga;
    var gc = ga;
    if lw.y > 0.004 {
        gb = ground_material(xy, dx, dy, b, ray, 0.0);
    }
    if lw.z > 0.004 {
        gc = ground_material(xy, dx, dy, c, ray, 0.0);
    }
    // Height-aware blend: stones and tufts of one material poke through the
    // other instead of the two dissolving into each other.
    let hs = vec3<f32>(ga.height, gb.height, gc.height);
    let score = lw + hs * 0.45 * min(lw * 4.0, vec3<f32>(1.0));
    let top = max(score.x, max(score.y, score.z));
    var bw = max(score - (top - 0.22), vec3<f32>(0.0)) * select(vec3<f32>(0.0), vec3<f32>(1.0), lw > vec3<f32>(0.0001));
    bw /= max(bw.x + bw.y + bw.z, 1e-4);
    var ground_color = ground_albedo(ga, a) * bw.x + ground_albedo(gb, b) * bw.y + ground_albedo(gc, c) * bw.z;
    // Each climate's look where the ground takes any of it (`Habitat::tropical`,
    // `desert`: all or nothing but for the few metres where a climate wall's two
    // sides meet, where both are worked out and mixed).
    let tropic = hab.tropical;
    let arid = hab.desert;
    if tropic > 0.0 {
        ground_color = side_mix3(ground_color, tropical_albedo(ga, a, ground_albedo(ga, a)) * bw.x
            + tropical_albedo(gb, b, ground_albedo(gb, b)) * bw.y
            + tropical_albedo(gc, c, ground_albedo(gc, c)) * bw.z, tropic);
    }
    // Canyon country (desert.wgsl): the beds, their soils, talus and sand.
    let dz = max(abs(dpx.z), abs(dpy.z));
    var site: CanyonSite;
    if arid > 0.0 {
        // Beds and the ring by the drawn surface's own height: on a cliff the
        // heightfield under a pixel and the coarser mesh drawn there can be
        // metres apart, which would saw the lines.
        site = canyon_site(xy, world.z - water, base_n, px, dz);
        ground_color = side_mix3(ground_color, canyon_ground(ga, a, site) * bw.x + canyon_ground(gb, b, site) * bw.y
            + canyon_ground(gc, c, site) * bw.z, arid);
    }
    let ground_n = ga.normal * bw.x + gb.normal * bw.y + gc.normal * bw.z;
    let ground_h = dot(hs, bw);

    // Cliffs keep the triplanar rock face: it wraps round vertical faces.
    var cliff: SurfaceDetail;
    cliff.color = vec3<f32>(0.2);
    cliff.normal = base_n;
    cliff.roughness = 0.85;
    cliff.ao = 1.0;
    cliff.height = 0.5;
    // Cliffs are shaded from a wider normal: the 8 m heightfield steps down a
    // cliff in facets, and the fine normal fluted them into organ pipes.
    var cliff_n = base_n;
    if rock_face > 0.004 || crag_w > 0.004 {
        cliff_n = normalize(mix(base_n, terrain_normal(xy, max(step * 3.0, 20.0)), 0.8));
        // Mostly the facet as it lies, a little of the wall's own lean: a sliver
        // standing on end still catches the sky.
        cliff_n = normalize(mix(cliff_n, facet, crag_w * 0.75));
        cliff = terrain_surface_grad(world, cliff_n, 17.0, MAT_ROCK_FACE, 1.25, dpx, dpy);
    }
    let rock_w = max(clamp(rock_face + (cliff.height - ground_h) * 0.35 * rock_face * (1.0 - rock_face) * 4.0, 0.0, 1.0),
        crag_w);

    // ---- Relief below the 8 m heightfield -----------------------------------
    // Hummocks, ruts and small rises, rougher on stony ground, smoother in
    // meadows, mud and sand. Octaves finer than a few pixels fade out.
    let rough_ground = 0.55 + w[5] * 0.8 + w[7] * 0.9 + canopy * 0.5 - sand_w * 0.35 - w[9] * 0.3;
    let o1 = grad_noise2_d(xy + warp * 0.5, 34.0);
    let o2 = grad_noise2_d(xy + 31.0, 9.5);
    let o3 = grad_noise2_d(xy - 57.0, 2.8);
    var relief = o1.yz * 2.2 + o2.yz * 0.55 * (1.0 - smoothstep(0.8, 3.0, px))
        + o3.yz * 0.12 * (1.0 - smoothstep(0.25, 0.9, px));
    relief *= max(rough_ground, 0.15);
    // A city's paved ground is smooth; its earth and rubble keep some relief.
    let bare_city = hab.street.ground == STREET_GROUND_EARTH || hab.street.ground == STREET_GROUND_RUBBLE;
    relief *= 1.0 - hab.made * select(1.0, 0.4, bare_city);

    // Boulders you can pick out from a battle camera, where the ground is
    // stony: the rock scan, greyed like the cliffs, on a lit dome.
    // Flat and gentle ground only: a stone laid out from above would smear
    // into a streak down a steep face.
    let stony = clamp(w[5] * 0.8 + w[7] * 0.7 + w[6] * 0.3 + w[2] * 0.06
        - sand_w - w[9] * 0.5 - canopy * 0.3, 0.0, 1.0) * (1.0 - max(snow_w, ice_w))
        * (1.0 - smoothstep(0.08, 0.2, slope)) * (1.0 - canyon_trail_floor(hab.way)) + canyon_trail_margin(hab.way) * 0.8;
    // No boulders on made ground but shelled earth.
    let stony_here = stony * (1.0 - hab.made * select(1.0, 0.5, hab.street.ground == STREET_GROUND_EARTH));
    // Canyon country's flats are mostly soil and shrubs: fewer loose blocks,
    // which lit from behind read as pits.
    let boulders = stones(xy, 7.0, stony_here * side_mix(0.45, 0.18, arid), px, 91.0);
    let boulder_cover = boulders.cover * (1.0 - smoothstep(1.5, 3.0, px));
    var boulder_rgb = vec3<f32>(0.0);
    if boulder_cover > 0.004 {
        let scan = textureSampleGrad(terrain_materials, repeat_sampler, xy / 2.3, MAT_ROCK_FACE, dx / 2.3, dy / 2.3).rgb;
        let grey = dot(scan, vec3<f32>(0.2126, 0.7152, 0.0722));
        let lichen = mix(vec3<f32>(1.0), vec3<f32>(0.8, 0.92, 0.62), smoothstep(0.4, 0.8, wet) * (0.3 + 0.7 * patchy));
        // Weathered stone is paler than the turf around it.
        // Weathered stone is paler than the turf around it; the scan's own
        // light and dark keeps it from reading as a smooth pebble.
        let grain = clamp(grey / 0.07, 0.35, 2.2);
        boulder_rgb = mix(vec3<f32>(0.07), scan, 0.25) * vec3<f32>(0.95, 0.97, 1.02) * lichen
            * grain * (1.1 + boulders.tone * 0.7);
        if arid > 0.0 {
            // Blocks fallen from the cliffs above.
            boulder_rgb = side_mix3(boulder_rgb, mix(site.talus, site.rock.rgb, boulders.tone) * clamp(grey / 0.1, 0.8, 1.5)
                * (1.0 + boulders.tone * 0.3), arid);
        }
    }

    // Large-scale texture: the aerial meadow scan's light and dark at 42 m
    // keeps every kind of ground from flattening into one colour when the
    // close-up scans are all averaged away by distance.
    let macro_scan = terrain_projection(xy / 42.0, dx / 42.0, dy / 42.0, MAT_MEADOW, vec2<f32>(0.0), 0.0);
    // Its gravel patches are the brightest part of the scan; softened, they
    // no longer read as pale blots repeating every 42 m.
    let macro_lum = pow(dot(macro_scan.color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722)) / 0.125, 0.6);
    let macro_mod = mix(1.0, clamp(macro_lum, 0.55, 1.4), 0.75 * (1.0 - sand_w) * (1.0 - canopy * 0.5));

    // ---- Colour ------------------------------------------------------------
    // The scans are true to life; broad tint and brightness fields make one
    // field of grass read as greener here, sun-bleached there.
    let green_part = (w[1] + w[2] + w[3]) / max(w[1] + w[2] + w[3] + w[4] + w[5] + w[6] + w[7] + w[8] + w[9], 1e-3);
    var albedo = ground_color * ground_tone(hab, green_part);
    albedo *= macro_mod;
    // Where grass grows (renderer/grass.rs) the ground takes on the colour of
    // the field seen from afar, so far-off blades are not flecks on a different
    // ground and nothing changes where they stop being drawn. Up close the soil
    // between the blades is in their shade: the gaps read as depth.
    let grassy = grass_share(hab);
    // How much of this ground is to be seen as a mass of grass rather than
    // soil between blades: all of it past where blades are drawn up close.
    var sward_look = 0.0;
    if grassy.x > 0.001 {
        // Seen from low down, a field is its blades' pale upper halves, not
        // the shade between them.
        let upright = clamp((eye.z - z) / max(dist, 1.0), 0.0, 1.0);
        let graze = 1.0 - smoothstep(0.05, 0.45, upright);
        let field = grass_mass(grassy, hab, mix(0.5, 0.85, graze)) * ground_tone(hab, green_part) * macro_mod
            * (1.0 + graze * 0.12);
        let drawn = grass_drawn(world);
        let close = drawn * smoothstep(8.0, 24.0, GRASS_CELL_M * globals.lod.x / max(dist, 1.0));
        // Between close blades the soil shows. Further off, looking across a
        // field, only the blades are seen: whether they are drawn or not, the
        // ground takes their colour and light, so the grass and the meadow past
        // it read as one. From above the soil's patchwork still shows through.
        let across = (1.0 - close) * mix(0.3, 1.0, graze);
        sward_look = grassy.x * across;
        albedo = mix(albedo, field, grassy.x * mix(0.55, 0.95, across));
        // The waves the wind drives through it (grass_wave): the flattened
        // crests show the grass's pale side, carrying on past the drawn blades.
        // Faded out as the crests, a dozen metres apart, shrink toward ripples on screen.
        // Seen from low down the crests squash toward lines: fade on how far
        // apart they stand on screen, foreshortening included.
        let crest_px = 14.0 * globals.lod.x / max(dist, 1.0) * max(upright * 2.5, 0.02);
        let wave = grass_wave(xy).z * grassy.x * smoothstep(10.0, 34.0, min(crest_px, 14.0 * globals.lod.x / max(dist, 1.0)));
        let pale = dot(albedo, vec3<f32>(0.2126, 0.7152, 0.0722));
        albedo = mix(albedo, vec3<f32>(pale) * vec3<f32>(1.25, 1.2, 0.95), wave * 0.22) * (1.0 + wave * 0.26 - grassy.x * 0.05);
        albedo *= 1.0 - 0.5 * grassy.x * close;
    }
    albedo = mix(albedo, boulder_rgb, boulder_cover);
    albedo *= mix(1.0, boulders.ao, 1.0 - smoothstep(1.5, 3.0, px));
    // A closed canopy keeps the floor in shade even where the sun's shadow
    // map has faded out at a distance.
    albedo *= 1.0 - canopy * 0.35;
    // Cliffs: the stone of the wall (rock.wgsl), as the rock pieces on it are.
    let mineral = dot(cliff.color, vec3<f32>(0.2126, 0.7152, 0.0722));
    var faced = albedo;
    if arid < 1.0 && rock_w > 0.004 {
        faced = mix(albedo, cliff_stone(xy, z, slope, cliff.color, broad, patchy, layer.z > 0.5), rock_w);
    }
    if arid > 0.0 {
        faced = side_mix3(faced, mix(albedo, canyon_cliff_stone(site, cliff.color, slope), rock_w), arid);
    }
    albedo = faced;
    // A crag's clefts are in their own shade; the rock standing out of the wall
    // catches the light.
    albedo *= mix(1.0, 0.5 + 0.55 * smoothstep(-7.0, 5.0, crag_depth_here), crag_w);
    var canyon_grad = vec2<f32>(0.0);
    var canyon_rough = -1.0;
    if arid > 0.0 {
        // What the ground is without any of it, for where two regions meet.
        let plain = albedo;
        let steep = smoothstep(0.07, 0.2, slope + (fine - 0.5) * 0.04);
        // Desert varnish: dark streaks down the faces, longest on the great cliffs.
        let v = smoothstep(0.44, 0.72, site.streak * 0.55 + site.streak_wide * 0.6 - 0.07);
        let varnish = v * site.rock.varnish * steep;
        albedo = mix(albedo, CANYON_VARNISH + albedo * 0.18, varnish * 0.75);
        // Runnels and talus tongues down the slopes below the cliffs.
        let slope_w = smoothstep(0.025, 0.1, slope) * (1.0 - rock_w);
        let runnel = site.streak_wide * 0.6 + site.streak * 0.4;
        albedo *= mix(1.0, 0.74 + 0.48 * smoothstep(0.3, 0.72, runnel), slope_w);
        // Shrubs dotting the flats, each with its shadow.
        let density = canyon_shrub_density(xy, site.a, alt, slope, sand_w, canopy, patchy, hab.way);
        let shrubs = canyon_shrubs(xy, density, px);
        let sage = smoothstep(CANYON_COCONINO_TOP, CANYON_RIM_BASE + 10.0, site.a);
        let bush = mix(CANYON_BLACKBRUSH, CANYON_SAGE, clamp(sage + (shrubs.tone - 0.5) * 0.6, 0.0, 1.0))
            * (0.8 + 0.4 * shrubs.tone) * shrubs.leaf;
        // Where the bushes stand up as blades (grass_gen.wgsl), only the shade
        // and litter under them is painted: dark painted shapes on the ground
        // there would read as pits beside the bushes.
        let standing = grass_drawn(world);
        albedo *= 1.0 - shrubs.shadow * mix(0.55, 0.25, standing);
        albedo = mix(albedo, mix(bush, albedo * 0.75, standing * 0.85), shrubs.cover);
        canyon_grad = shrubs.grad * arid;
        // The bathtub ring over everything below the old full-pool line.
        let ring = canyon_ring(xy, world.z - water, steep, site.streak, dz);
        let cracks = canyon_mud_cracks(xy, px);
        // Silt dries paler on the rises, stays darker and damper in the dips.
        let silt = CANYON_SILT * (0.8 + 0.3 * fine) * (0.85 + 0.3 * patchy) * (1.0 - concavity * 0.6)
            * (1.0 - cracks * 0.55);
        albedo = mix(albedo, silt, ring.silt * (1.0 - shrubs.cover));
        // The crust keeps the rock's grain and runs down it in drips, greyer
        // where it thinned and washed.
        let drip = site.streak * 0.6 + site.streak_wide * 0.4;
        let crust = CANYON_CRUST * clamp(pow(mineral / 0.07, 0.3), 0.75, 1.2)
            * (0.8 + 0.32 * smoothstep(0.3, 0.75, drip)) * (1.0 + site.rock.ledge * 0.6 * steep);
        albedo = mix(albedo, crust, ring.crust * mix(0.7, 0.95, smoothstep(0.35, 0.65, drip)));
        albedo *= 1.0 + ring.lines;
        albedo = mix(albedo, vec3<f32>(0.05, 0.055, 0.04), ring.wet * 0.6);
        canyon_rough = mix(mix(0.9, 0.62, varnish), 0.95, ring.crust);
        // The trails, over all of it: pale trodden dust inside a dark margin.
        if hab.way > 0.004 {
            let trail = canyon_trail(xy, hab.way, site, px);
            let gravel = mix(CANYON_PAVEMENT, site.talus, 0.3);
            albedo = mix(albedo, gravel, trail.margin * 0.8);
            albedo = mix(albedo, trail.rgb, trail.cover * (1.0 - boulder_cover));
            canyon_rough = mix(canyon_rough, 0.8, trail.cover);
        }
        albedo = side_mix3(plain, albedo, arid);
    }
    // A city's made ground (streets.wgsl): roads and their markings, kerbs,
    // pavements, yards, rail yards, rubble, shelled earth, fields.
    var street_grad = vec2<f32>(0.0);
    var street_rough = -1.0;
    var street_metal = 0.0;
    var street_flat = 0.0;
    if hab.made > 0.004 {
        let st = street_shade(xy, hab.street, albedo, px);
        albedo = mix(albedo, st.rgb, hab.made);
        street_grad = st.grad * hab.made;
        street_rough = st.rough;
        street_metal = st.metal * hab.made;
        street_flat = st.flat * hab.made;
    }
    // Snow: drifts a little brighter and darker, bluer in its hollows.
    let drift = 0.93 + 0.1 * fine + 0.05 * sward;
    let snow_rgb = vec3<f32>(0.65, 0.69, 0.74) * drift * mix(vec3<f32>(1.0), vec3<f32>(0.9, 0.95, 1.05), concavity * 1.5);
    // On a crag, snow lies where the rock holds it, wherever the ground round it
    // has snow: on ledges, and down the gullies between the ribs.
    let ledge = smoothstep(0.6, 0.85, facet.z);
    let gully = 1.0 - smoothstep(-8.0, 1.0, crag_depth_here + (fine - 0.5) * 4.0);
    let lying_here = max(mix(smoothstep(350.0, 450.0, alt + (broad - 0.5) * 95.0),
        smoothstep(0.3, 0.7, layer.y + (patchy - 0.5) * 0.55), layer.z), snow_w);
    let crag_snow = lying_here * max(ledge, gully * 0.9) * (1.0 - arid);
    let snow_here = mix(snow_w, crag_snow, crag_w);
    albedo = mix(albedo, snow_rgb, snow_here);

    // Glacier ice (`glacier_shade`).
    var ice_bend = vec3<f32>(0.0);
    var ice_glow = vec3<f32>(0.0);
    var ice_rough = 0.5;
    if ice_w > 0.004 {
        let ice = glacier_shade(xy, z, alt, base_n, layer.x, fine, patchy, px,
            max(abs(dpx.z), abs(dpy.z)), snow_rgb);
        albedo = mix(albedo, ice.rgb, ice_w);
        ice_bend = ice.bend;
        ice_glow = ice.glow * ice_w;
        ice_rough = ice.rough;
    }

    // Scan normals, then the procedural relief and stones as world slopes.
    let g = ground_n.xy / max(ground_n.z, 0.3);
    let grad = vec3<f32>(g * 1.1 * (1.0 - street_flat) - relief - boulders.grad * boulder_cover - canyon_grad
        - street_grad, 0.0);
    let ground_normal = normalize(base_n + (grad - base_n * dot(grad, base_n)));
    var n = normalize(mix(ground_normal, cliff.normal, rock_w));
    n = normalize(mix(n, base_n, max(snow_here * (1.0 - crag_w * 0.6), ice_w) * 0.7));
    n = normalize(n + ice_bend * ice_w);
    let ground_rough = ga.color.a * bw.x + gb.color.a * bw.y + gc.color.a * bw.z;
    var rough = mix(clamp(ground_rough, 0.75, 1.0), clamp(cliff.roughness, 0.7, 0.95), rock_w);
    // A far meadow is lit as its blades are (grass.wgsl): a smooth, soft
    // surface with a sheen at grazing angles, not the soil scan's bumps.
    n = normalize(mix(n, base_n, sward_look * 0.8));
    rough = mix(rough, 0.52, sward_look);
    rough = mix(rough, 0.8, boulder_cover * 0.5);
    rough = mix(rough, 0.7, snow_here);
    rough = mix(rough, ice_rough, ice_w);
    if canyon_rough >= 0.0 {
        rough = mix(rough, canyon_rough, rock_w * arid);
    }
    if street_rough >= 0.0 {
        rough = mix(rough, street_rough, hab.made);
    }
    // Rain darkens the ground and gives it a sheen while it falls.
    let soaked = weather_at(xy).w;
    albedo *= 1.0 - 0.3 * soaked;
    rough = mix(rough, 0.42, soaked * 0.75);
    // Cavity occlusion belongs to the material, and terrain concavity adds
    // grounded shading along gullies and the feet of slopes.
    let cavity = mix(ga.normal.w * bw.x + gb.normal.w * bw.y + gc.normal.w * bw.z, cliff.ao, rock_w);
    // (A far meadow's cavities are hidden under its blades.)
    albedo *= mix(0.45 + cavity * 0.55, 1.0, sward_look * 0.8) * (1.0 - concavity * 0.8);

    // Craters big blasts left (renderer/craters.rs).
    var crater_glow = vec3<f32>(0.0);
    var crater_sky = 1.0;
    var crater_metal = 0.0;
    if ground_craters.count.x > 0u {
        let cs = craters_at(xy, alt, albedo, rough, px);
        albedo = cs.albedo;
        rough = cs.rough;
        crater_glow = cs.glow;
        crater_sky = cs.sky;
        crater_metal = cs.metal;
        n = normalize(mix(n, base_n, cs.fused * 0.9) - vec3<f32>(cs.slope, 0.0));
    }
    // Ground melted and glassed (renderer/ground_melt.rs).
    if ground_melt[0] != 0u {
        let ms = melt_shade(xy, alt, albedo, rough, px);
        albedo = ms.albedo;
        rough = ms.rough;
        crater_glow += ms.glow;
        crater_sky = min(crater_sky, ms.sky);
        crater_metal = max(crater_metal, ms.metal);
        n = normalize(mix(n, base_n, ms.fused * 0.9) - vec3<f32>(ms.slope, 0.0));
    }
    // Ground a gravitic engine broke (renderer/gravitic_engine_fx/ground.rs).
    if ground_rifts.count.x > 0u {
        let rs = rifts_at(xy, alt, albedo, rough, px);
        albedo = rs.albedo;
        rough = rs.rough;
        crater_glow += rs.glow;
        n = normalize(n - vec3<f32>(rs.slope, 0.0));
    }

    // The breakers' wash running up the beach and draining back (shore.wgsl):
    // a thin sheet of water, glossy and darker, a froth along its front, and the
    // sand wet and dark behind it.
    if alt > -0.6 && alt < SURF_RUNUP_RISE * 1.4 {
        let sw = wash(xy, shore_at(xy, -alt), globals.camera.w, px);
        albedo *= 1.0 - 0.3 * sw.wet;
        rough = mix(rough, 0.28, sw.wet * 0.75);
        albedo = mix(albedo, albedo * vec3<f32>(0.72, 0.84, 0.86), sw.cover * 0.8);
        rough = mix(rough, 0.2, sw.cover);
        n = normalize(mix(n, base_n, sw.cover * 0.85));
        let froth = foam_lace(xy, globals.camera.w, px, sw.foam * 0.7);
        albedo = mix(albedo, vec3<f32>(0.78, 0.82, 0.84), froth * 0.8);
        rough = mix(rough, 0.6, froth);
    }

    // Seabed: a little darker and bluer with depth. The water drawn on top
    // does most of the dimming, so a wreck field on the bottom still reads.
    let depth = max(-alt, 0.0);
    if depth > 3.0 && arid < 1.0 {
        // Past a few metres the land's tiled ground scans give way to silt and
        // sand varied only by broad world-space noise: through clear water from
        // high up, a scan's repeats showed as rows of blotches across the sea.
        let silty = smoothstep(3.0, 14.0, depth) * (1.0 - arid);
        let drift = grad_noise2(xy, 140.0) * 0.6 + grad_noise2(xy + vec2<f32>(53.0, 17.0), 37.0) * 0.4;
        let silt = mix(vec3<f32>(0.2, 0.2, 0.16), vec3<f32>(0.3, 0.28, 0.21), drift);
        albedo = mix(albedo, silt, silty * 0.9);
        n = normalize(mix(n, base_n, silty * 0.85));
    }
    // Each climate's seabed, by how much of each the ground here takes.
    let bare = albedo;
    let temperate = 1.0 - tropic - arid;
    albedo = vec3<f32>(0.0);
    if tropic > 0.0 {
        // Pale sand banks, so the sea over them goes turquoise (water.wgsl),
        // giving way to darker ground in the deep channels.
        albedo += bare * mix(vec3<f32>(0.85, 0.93, 0.97), vec3<f32>(0.12, 0.17, 0.24), smoothstep(10.0, 40.0, depth)) * tropic;
    }
    if arid > 0.0 {
        // The drowned canyon: pale silt and red sand in the shallows, so the lake
        // goes jade over them (water.wgsl), fading dark down the old river channel.
        var drowned = bare;
        if depth > 0.0 {
            let bed = mix(vec3<f32>(0.40, 0.33, 0.25), CANYON_RED_SAND * 0.8, smoothstep(0.45, 0.7, patchy));
            let floor_rgb = mix(bed, bare, rock_w * 0.6);
            drowned = mix(floor_rgb, floor_rgb * vec3<f32>(0.25, 0.3, 0.36), smoothstep(8.0, 45.0, depth));
        }
        albedo += drowned * arid;
    }
    if temperate > 0.0 {
        albedo += mix(bare, bare * vec3<f32>(0.5, 0.68, 0.74), clamp(depth / 30.0, 0.0, 1.0)) * temperate;
    }
    if depth > 0.04 && dist < 180.0 {
        // Light that made it through the surface, crawling on the sand.
        // World-space noise: a tiled pair at 6–10 m was a diamond lattice
        // through the water from the play camera.
        let t = globals.camera.w;
        let c1 = grad_noise2(xy + vec2<f32>(t * 2.8, t * 1.6), 5.4);
        let c2 = grad_noise2(xy - vec2<f32>(t * 1.9, t * 2.7), 3.7);
        let caustic = pow(1.0 - abs(c1 - c2), 6.0) * (1.0 - smoothstep(0.5, 11.0, depth));
        albedo += vec3<f32>(0.10, 0.22, 0.18) * caustic * clamp(1.0 - dist / 160.0, 0.0, 1.0);
    }

    var m: Pbr;
    m.albedo = albedo;
    m.metallic = max(crater_metal, street_metal);
    m.roughness = rough;
    // Soil, rock, snow and meadow are matte at grazing angles; glass, ice and
    // standing water keep their glints.
    m.matte = smoothstep(0.3, 0.5, rough);
    m.emissive = ice_glow + crater_glow;
    let v = normalize(eye - world);
    var horizon = 1.0;
    let toward_sun = normalize(globals.sun.xy);
    let rise = globals.sun.z / max(length(globals.sun.xy), 0.1);
    for (var i = 0; i < 4; i++) {
        let reach = 24.0 * exp2(f32(i));
        let obstruction = terrain_height(xy + toward_sun * reach) - ground_z - rise * reach;
        horizon = min(horizon, smoothstep(-3.0, 5.0, -obstruction));
    }
    let shadow = sun_shadow(world, base_n) * horizon;
    // How much of the sky the ground sees: how far the land around rises above
    // this spot's own slope, in six directions at two reaches that grow with
    // the view, so gullies and the feet of slopes fall into shade at any zoom
    // and ridges stand out lit.
    let lean = -base_n.xy / max(base_n.z, 0.2);
    let reach = max(26.0, px * 5.0);
    var open_sky = 0.0;
    for (var i = 0; i < 6; i++) {
        let a = f32(i) * 1.0471976 + 0.5236;
        let dir = vec2<f32>(cos(a), sin(a));
        var rise = 0.0;
        for (var k = 0; k < 2; k++) {
            let r = reach * select(1.0, 3.6, k == 1);
            let above = terrain_height(xy + dir * r) - (ground_z + dot(lean, dir * r));
            rise = max(rise, above / r);
        }
        open_sky += 1.0 - rise * inverseSqrt(1.0 + rise * rise);
    }
    let sky_vis = pow(open_sky / 6.0, 1.6) * (0.55 + cavity * 0.45) * crater_sky * screen_ao(pixel);
    var color = shade_pbr_vis(m, n, v, globals.sun.xyz, shadow, sky_vis);
    color += albedo * lightning_light(world, n) * 0.35;
    color += local_lights(m, world, n, v);

    if push.build_grid == 1u {
        // Under the sea the grid is drawn on the water's surface instead (water.wgsl);
        // this hands over as the water's shore alpha comes in.
        let dry = 1.0 - smoothstep(0.015, 0.12, water - z);
        color = mix(color, build_grid_overlay(color, xy, dist), dry);
    }
    if has_regions() {
        // The climate walls' foot. Under the sea it lies on the water's surface
        // instead (water.wgsl), handed over as the grid is.
        color += wall_seam(xy, px) * (1.0 - smoothstep(0.015, 0.12, water - z));
    }

    color = apply_fog_of_war(color, xy);
    color = apply_haze(color, world, eye);
    return vec4<f32>(color, 1.0);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return shade_ground(in.world, in.crag, in.clip.xy, dpdx(in.world), dpdy(in.world));
}

// The terrain shaded once per pixel before the scene pass (`fs_lit`), and the
// pre-pass's depth it was shaded from. Set 2 of `layouts.water`.
@group(2) @binding(0) var terrain_lit: texture_2d<f32>;
@group(2) @binding(1) var lit_depth: texture_depth_2d;

// The scene pass's terrain over a pre-pass: what `fs_lit` shaded at this pixel. Its
// half-pixel triangles would otherwise run `shade_ground` several times a pixel
// (the GPU shades 2x2 blocks per triangle). Crags are shaded here instead: their
// relief comes from the vertex stage.
@fragment
fn fs_over_lit(in: VsOut) -> @location(0) vec4<f32> {
    let dpx = dpdx(in.world);
    let dpy = dpdy(in.world);
    if in.crag.x > 0.0 {
        return shade_ground(in.world, in.crag, in.clip.xy, dpx, dpy);
    }
    return textureLoad(terrain_lit, vec2<i32>(in.clip.xy), 0);
}

struct LitOut {
    @builtin(position) clip: vec4<f32>,
}

@vertex
fn vs_lit(@builtin(vertex_index) i: u32) -> LitOut {
    var out: LitOut;
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    out.clip = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    return out;
}

// The point the pre-pass left at pixel `p` (xyz) and its depth (w, 0 where nothing).
fn lit_point(p: vec2<i32>) -> vec4<f32> {
    let size = vec2<i32>(textureDimensions(lit_depth));
    let q = clamp(p, vec2<i32>(0), size - vec2<i32>(1));
    let d = textureLoad(lit_depth, q, 0);
    let uv = (vec2<f32>(q) + vec2<f32>(0.5)) / vec2<f32>(size);
    let h = globals.inv_view_proj * vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, d, 1.0);
    return vec4<f32>(h.xyz / h.w, d);
}

// Of the steps to the two neighbours along an axis, the one on the same surface:
// the one whose depth changes least, so a ridge's edge does not take its slope
// from the land behind it.
fn lit_step(c: vec4<f32>, before: vec4<f32>, after: vec4<f32>) -> vec3<f32> {
    return select(c.xyz - before.xyz, after.xyz - c.xyz, abs(after.w - c.w) < abs(c.w - before.w));
}

// The ground at every pixel the pre-pass left something at (terrain or what stands
// on it: the scene pass keeps only where the terrain is in front).
@fragment
fn fs_lit(in: LitOut) -> @location(0) vec4<f32> {
    let p = vec2<i32>(in.clip.xy);
    let c = lit_point(p);
    if c.w <= 0.0 {
        return vec4<f32>(0.0);
    }
    let dpx = lit_step(c, lit_point(p - vec2<i32>(1, 0)), lit_point(p + vec2<i32>(1, 0)));
    let dpy = lit_step(c, lit_point(p - vec2<i32>(0, 1)), lit_point(p + vec2<i32>(0, 1)));
    return shade_ground(c.xyz, vec4<f32>(0.0), in.clip.xy, dpx, dpy);
}
