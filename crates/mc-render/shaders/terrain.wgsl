//!use bindings
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
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
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

    let world = vec3<f32>(xy, terrain_height(xy));
    var out: VsOut;
    if push.pass_kind == 1u {
        out.clip = globals.shadow_view_proj * vec4<f32>(world, 1.0);
    } else {
        out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    }
    out.world = world;
    return out;
}

@fragment
fn fs_shadow() {
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let xy = in.world.xy;
    let z = in.world.z;
    let water = globals.map.z;
    let eye = globals.camera.xyz;
    let dist = distance(in.world, eye);

    // Geometric normal from the heightmap; finer step up close.
    let step = clamp(dist * 0.004, 4.0, 48.0);
    let base_n = terrain_normal(xy, step);
    let slope = 1.0 - base_n.z;

    // Detail from the noise texture at several scales. Every lookup is
    // decorrelated so the 512-texel tile never reads as a grid; grit only
    // matters up close.
    let n_grit = noise_varied(xy, 2.7);
    let n_fine = noise_varied(xy, 9.0);
    let n_mid = noise_varied(xy, 67.0);
    let n_broad = noise_varied(xy, 241.0);
    let n_macro = noise_varied(xy, 1370.0);
    let near = clamp(1.0 - dist / 900.0, 0.0, 1.0);
    let mid = clamp(1.0 - dist / 9000.0, 0.0, 1.0);
    let bump = (n_grit.xy - 0.5) * 0.55 * near
        + (n_fine.xy - 0.5) * 1.5 * near
        + (n_mid.xy - 0.5) * 0.9 * mid
        + (n_broad.xy - 0.5) * 0.45
        + (n_macro.xy - 0.5) * 0.35;
    let n = normalize(base_n + vec3<f32>(bump, 0.0));

    // Grass vs dry is world-space noise, not the texture's coarse octaves:
    // those are a lattice, and a threshold on them paints a square grid.
    let alt = z - water;
    let patchy = value_noise2(xy, 187.0) * 0.38 + value_noise2(xy, 83.0) * 0.27
        + value_noise2(xy, 37.0) * 0.18 + n_mid.b * 0.12 + n_fine.b * 0.05;
    let rock_w = smoothstep(0.10, 0.22, slope + (n_mid.b - 0.5) * 0.08);
    let sand_w = 1.0 - smoothstep(3.0, 11.0, alt + (n_mid.a - 0.5) * 6.0);
    let snow_w = smoothstep(330.0, 420.0, alt + (n_macro.a - 0.5) * 120.0) * (1.0 - smoothstep(0.25, 0.45, slope));
    let dry_w = smoothstep(0.42, 0.62, patchy);

    let grass = mix(vec3<f32>(0.045, 0.10, 0.025), vec3<f32>(0.12, 0.19, 0.05), n_fine.b * 0.35 + value_noise2(xy, 31.0) * 0.4 + value_noise2(xy, 14.0) * 0.25);
    let dry = mix(vec3<f32>(0.22, 0.19, 0.09), vec3<f32>(0.14, 0.12, 0.06), n_fine.a * 0.5 + value_noise2(xy, 19.0) * 0.5);
    let rock = mix(vec3<f32>(0.13, 0.125, 0.12), vec3<f32>(0.28, 0.26, 0.24), n_mid.a * 0.4 + n_fine.b * 0.25 + value_noise2(xy, 47.0) * 0.35);
    let sand = mix(vec3<f32>(0.42, 0.36, 0.24), vec3<f32>(0.55, 0.49, 0.35), n_fine.b * 0.55 + value_noise2(xy, 13.0) * 0.45);
    let snow = vec3<f32>(0.85, 0.88, 0.92);

    var albedo = mix(grass, dry, dry_w);
    albedo = mix(albedo, sand, sand_w);
    albedo = mix(albedo, rock, rock_w);
    albedo = mix(albedo, snow, snow_w);
    var rough = mix(0.92, 0.75, rock_w);
    rough = mix(rough, 0.55, snow_w);

    // Seabed: darker and bluer with depth; the water surface is drawn on top.
    let depth = max(-alt, 0.0);
    albedo = mix(albedo, albedo * vec3<f32>(0.25, 0.45, 0.55), clamp(depth / 25.0, 0.0, 1.0));

    var m: Pbr;
    m.albedo = albedo;
    m.metallic = 0.0;
    m.roughness = rough;
    m.emissive = vec3<f32>(0.0);
    let v = normalize(eye - in.world);
    let shadow = sun_shadow(in.world, base_n);
    var color = shade_pbr(m, n, v, globals.sun.xyz, shadow);

    if push.build_grid == 1u {
        // 12 m build cells, drawn near the camera only.
        let g = abs(fract(xy / BUILD_CELL_M + 0.5) - 0.5) * BUILD_CELL_M;
        let width = max(dist * 0.0012, 0.12);
        let line = 1.0 - smoothstep(0.0, width, min(g.x, g.y));
        color = mix(color, vec3<f32>(0.55, 0.85, 1.0) * 1.6, line * 0.225 * clamp(1.0 - dist / 1800.0, 0.0, 1.0));
    }

    color = apply_fog_of_war(color, xy);
    color = apply_haze(color, in.world, eye);
    return vec4<f32>(color, 1.0);
}
