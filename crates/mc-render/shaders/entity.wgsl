//!use bindings
// Units, structures, wrecks and props. One multi-draw-indirect call renders
// every visible model; per-instance data comes from the visible list the cull
// pass built, so `instance_index` (which includes firstInstance) indexes it.

struct EntityPush {
    // 1: shadow pass
    pass_kind: u32,
}

var<immediate> push: EntityPush;

const MAT_PLATING: u32 = 0u;
const MAT_ACCENT: u32 = 1u;
const MAT_GLOW: u32 = 2u;
const MAT_TEAM: u32 = 3u;
const MAT_METAL: u32 = 4u;
const MAT_GLASS: u32 = 5u;
const MAT_TREAD: u32 = 6u;
const MAT_GLOW_ORANGE: u32 = 7u;
const MAT_BARK: u32 = 8u;
const MAT_FOLIAGE: u32 = 9u;
const MAT_ROCK: u32 = 10u;
const MAT_CONCRETE: u32 = 11u;
const MAT_WINDOWS: u32 = 12u;

const PART_TURRET: u32 = 1u;
const PART_SPINNER: u32 = 2u;
const PART_LOCOMOTION: u32 = 3u;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) material: u32,
    @location(4) part: u32,
    @builtin(instance_index) instance: u32,
}

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) @interpolate(flat) material: u32,
    @location(4) @interpolate(flat) owner_flags: u32,
    // x build fraction, y health, z local height fraction, w per-entity random
    @location(5) state: vec4<f32>,
    @location(6) local: vec3<f32>,
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    let e = load_entity(visible[in.instance]);
    let model = models[e.blueprint];
    let t = globals.sun.w;
    let time = globals.camera.w;
    var scale = 1.0;
    if e.scale != 0u {
        scale = f32(e.scale) * 0.001;
    }

    var p = in.pos;
    var n = in.normal;
    if in.part == PART_TURRET {
        let pivot = model.turret_pivot.xyz;
        p = rot_z(p - pivot, e.turret_yaw) + pivot;
        n = rot_z(n, e.turret_yaw);
    } else if in.part == PART_SPINNER && (e.owner_flags & (KIND_WRECK | FLAG_UNDER_CONSTRUCTION)) == 0u {
        let pivot = model.spinner_pivot.xyz;
        let spin = time * 1.6 + f32(e.unit_id & 255u);
        p = rot_z(p - pivot, spin) + pivot;
        n = rot_z(n, spin);
    } else if in.part == PART_LOCOMOTION && (e.owner_flags & FLAG_MOVING) != 0u && (e.owner_flags & KIND_WRECK) == 0u {
        // A small gait/track shudder; real leg animation needs skinned parts.
        let phase = time * 9.0 + p.x * 0.8 + sign(p.y) * 1.57;
        p.z += max(sin(phase), 0.0) * 0.12 * model.height * 0.1;
    }

    let heading = lerp_angle(e.prev_heading, e.heading, t);
    var origin = mix(e.prev_pos, e.pos, t);
    if (e.owner_flags & KIND_PROP) != 0u {
        // Props carry an approximate height; stand them on the real surface.
        origin.z = terrain_height(origin.xy);
    }
    var local = p * scale;

    // Mobile units lean with the ground under them.
    var up = vec3<f32>(0.0, 0.0, 1.0);
    if (model.icon & 0x10000u) != 0u {
        up = terrain_normal(origin.xy, max(e.radius, 4.0));
    }
    let fwd0 = vec3<f32>(cos(heading), sin(heading), 0.0);
    let left = normalize(cross(up, fwd0));
    let fwd = cross(left, up);
    let world = origin + fwd * local.x + left * local.y + up * local.z;
    let world_n = normalize(fwd * n.x + left * n.y + up * n.z);

    var out: VsOut;
    if push.pass_kind == 1u {
        out.clip = globals.shadow_view_proj * vec4<f32>(world, 1.0);
    } else {
        out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    }
    out.world = world;
    out.normal = world_n;
    out.uv = in.uv;
    out.material = in.material;
    out.owner_flags = e.owner_flags;
    out.state = vec4<f32>(e.build, e.health, in.pos.z / max(model.height, 0.1), hash11(f32(e.unit_id & 0xFFFFu)));
    out.local = in.pos;
    return out;
}

@fragment
fn fs_shadow(in: VsOut) {
    // Unbuilt parts of a construction site cast no shadow.
    if (in.owner_flags & FLAG_UNDER_CONSTRUCTION) != 0u && in.state.z > in.state.x {
        discard;
    }
}

fn material_of(id: u32, owner: u32) -> Pbr {
    var m: Pbr;
    m.emissive = vec3<f32>(0.0);
    m.metallic = 0.0;
    m.roughness = 0.6;
    m.albedo = vec3<f32>(0.5);
    switch id {
        case 0u: { m.albedo = globals.plating.rgb; m.metallic = 0.25; m.roughness = 0.34; }
        case 1u: { m.albedo = globals.accent.rgb; m.metallic = 0.6; m.roughness = 0.48; }
        case 2u: { m.albedo = globals.glow.rgb * 0.2; m.emissive = globals.glow.rgb * 5.0; m.roughness = 0.3; }
        case 3u: { m.albedo = globals.team_colors[owner & 7u].rgb; m.metallic = 0.3; m.roughness = 0.4; m.emissive = globals.team_colors[owner & 7u].rgb * 0.25; }
        case 4u: { m.albedo = vec3<f32>(0.32, 0.33, 0.36); m.metallic = 0.95; m.roughness = 0.32; }
        case 5u: { m.albedo = vec3<f32>(0.02, 0.05, 0.09); m.metallic = 0.9; m.roughness = 0.08; m.emissive = globals.glow.rgb * 0.15; }
        case 6u: { m.albedo = vec3<f32>(0.03, 0.03, 0.035); m.roughness = 0.9; }
        case 7u: { m.albedo = vec3<f32>(0.3, 0.12, 0.02); m.emissive = vec3<f32>(1.0, 0.42, 0.08) * 4.0; m.roughness = 0.3; }
        case 8u: { m.albedo = vec3<f32>(0.16, 0.11, 0.07); m.roughness = 0.95; }
        case 9u: { m.albedo = vec3<f32>(0.07, 0.16, 0.05); m.roughness = 0.85; }
        case 10u: { m.albedo = vec3<f32>(0.3, 0.29, 0.27); m.roughness = 0.9; }
        case 11u: { m.albedo = vec3<f32>(0.42, 0.42, 0.41); m.roughness = 0.85; }
        case 12u: { m.albedo = vec3<f32>(0.05, 0.07, 0.1); m.metallic = 0.8; m.roughness = 0.15; m.emissive = vec3<f32>(0.9, 0.75, 0.45) * 0.35; }
        default: {}
    }
    return m;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let flags = in.owner_flags;
    let owner = flags & 0xFFu;
    let time = globals.camera.w;
    var m = material_of(in.material, owner);
    var n = normalize(in.normal);
    let eye = globals.camera.xyz;
    let v = normalize(eye - in.world);
    let dist = distance(eye, in.world);

    // Panel lines and plate seams from the tiling normal map, on hull materials only.
    if in.material <= MAT_METAL && in.material != MAT_GLOW {
        let detail = clamp(1.0 - dist / 700.0, 0.0, 1.0);
        let tex = textureSample(panel_map, repeat_sampler, in.uv * 0.16);
        // Cotangent frame from screen-space derivatives: no tangents in the mesh.
        let dp1 = dpdx(in.world);
        let dp2 = dpdy(in.world);
        let duv1 = dpdx(in.uv);
        let duv2 = dpdy(in.uv);
        let dp2perp = cross(dp2, n);
        let dp1perp = cross(n, dp1);
        let tangent = dp2perp * duv1.x + dp1perp * duv2.x;
        let bitangent = dp2perp * duv1.y + dp1perp * duv2.y;
        let inv = inverseSqrt(max(max(dot(tangent, tangent), dot(bitangent, bitangent)), 1e-12));
        let tn = (tex.xy * 2.0 - 1.0) * detail * 0.9;
        n = normalize(n + (tangent * tn.x + bitangent * tn.y) * inv);
        m.roughness = clamp(m.roughness + (tex.a - 0.5) * 0.25 * detail, 0.05, 1.0);
        m.albedo *= mix(1.0, 0.55 + tex.b * 0.45, detail);
    }
    if in.material == MAT_FOLIAGE {
        m.albedo *= 0.75 + in.state.w * 0.6;
    }

    if (flags & KIND_WRECK) != 0u {
        // Burnt out: charred, matte, dead emitters; fades into the ground as it is reclaimed.
        let soot = hash21(floor(in.uv * 1.7) + vec2<f32>(in.state.w * 91.0));
        m.albedo = mix(vec3<f32>(0.035, 0.03, 0.028), m.albedo * 0.22, soot * 0.6);
        m.metallic = 0.2;
        m.roughness = 0.95;
        m.emissive = vec3<f32>(0.0);
        if in.state.z > in.state.y * 0.75 + 0.25 {
            discard;
        }
    }

    let shadow = sun_shadow(in.world, n);
    var color = shade_pbr(m, n, v, globals.sun.xyz, shadow);
    var alpha = 1.0;

    if (flags & FLAG_UNDER_CONSTRUCTION) != 0u {
        // Built up to the progress line; above it a scanning hologram of what is coming.
        let line = in.state.x;
        let h = in.state.z;
        if h > line {
            let scan = fract(in.local.z * 0.9 - time * 0.7);
            let grid = step(0.86, fract(in.local.x * 0.5)) + step(0.86, fract(in.local.y * 0.5)) + step(0.9, scan);
            if grid < 0.5 {
                discard;
            }
            color = globals.glow.rgb * 1.8;
        } else {
            let edge = smoothstep(0.035, 0.0, line - h);
            color = mix(color, globals.glow.rgb * 6.0, edge);
        }
    }
    if (flags & KIND_GHOST) != 0u {
        let pulse = 0.6 + 0.4 * sin(time * 5.0);
        let rim = pow(1.0 - max(dot(n, v), 0.0), 2.0);
        // `health` carries validity for ghosts: 1 placeable, 0 blocked.
        let tint = mix(vec3<f32>(1.0, 0.15, 0.1), globals.glow.rgb, step(0.5, in.state.y));
        color = tint * (0.5 + rim * 2.5) * pulse;
        let weave = fract((in.clip.x + in.clip.y) * 0.25);
        if weave < 0.5 {
            discard;
        }
    }

    if (flags & KIND_PROP) != 0u {
        color = apply_fog_of_war(color, in.world.xy);
    }
    color = apply_haze(color, in.world, eye);
    return vec4<f32>(color, alpha);
}
