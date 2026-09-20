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
const MAT_GLOW_AMBER: u32 = 13u;
const MAT_PLATING_DARK: u32 = 14u;

// The yellow-orange of construction: build beams, build emitters, a refit going up.
const AMBER: vec3<f32> = vec3<f32>(1.0, 0.6, 0.1);

const PART_TURRET: u32 = 1u;
const PART_SPINNER: u32 = 2u;
const PART_LOCOMOTION: u32 = 3u;

// `rig` bits (models::rig): the leg bone a vertex rides, and the upgrade piece flag.
const LIMB_THIGH: u32 = 1u;
const LIMB_SHIN: u32 = 2u;
const LIMB_FOOT: u32 = 3u;
const LIMB_ARM_GUN: u32 = 4u;
const LIMB_ARM_TOOL: u32 = 5u;
const RIG_UPGRADE: u32 = 0x100u;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) material: u32,
    @location(4) part: u32,
    @location(5) rig: u32,
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
    // Tech level in the low byte, 0x100 for a mobile unit.
    @location(7) @interpolate(flat) model_class: u32,
    // x: 1 for an upgrade piece, y: how built the piece is (0 a hologram, 1 done), z: the unit's refit progress
    @location(8) @interpolate(flat) refit: vec3<f32>,
    // xyz local print origin, w distance to the farthest corner of the bounds
    @location(9) @interpolate(flat) weld: vec4<f32>,
}

// Turns in the model's fore-and-aft plane: from +x toward +z.
fn rot_xz(v: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec3<f32>(v.x * c - v.z * s, v.y, v.x * s + v.z * c);
}

// How much of a walk the unit is in, zero standing to one in full stride, and
// where in the cycle it is. The sim counts the ground covered, so the feet
// keep pace with the ground whatever the speed and do not slide.
fn walk_state(e: Entity, model: ModelInfo) -> vec2<f32> {
    let stride = model.leg_hip.w;
    let t = globals.sun.w;
    let ease = 1.0 / (0.04 * stride);
    let amount = mix(clamp(e.gait.z * ease, 0.0, 1.0), clamp(e.gait.y * ease, 0.0, 1.0), t);
    return vec2<f32>(amount, fract((e.gait.x - e.gait.y * (1.0 - t)) / stride));
}

// The body over its legs, leaning toward the planted one. Walking, it is highest as it
// vaults over that leg; running (a foot planted for under half the cycle), it sinks onto
// it and is highest in the air between steps.
fn walk_bob(walk: vec2<f32>, model: ModelInfo) -> vec3<f32> {
    let stance = model.leg_ankle.w;
    let mid = stance * 0.5;
    let rise = select(0.16, -0.2, stance < 0.5);
    return vec3<f32>(0.0, cos((walk.y - mid) * 2.0 * PI) * 0.08, cos((walk.y - mid) * 4.0 * PI) * rise) * model.leg_knee.w * walk.x;
}

// A leg vertex posed for this moment of the stride. Two bones, hip to knee and
// knee to ankle, solved so the ankle is where the foot has to be: planted and
// passing under the body, or lifted and swinging forward.
fn walk_leg(pos: vec3<f32>, normal: vec3<f32>, limb: u32, model: ModelInfo, walk: vec2<f32>) -> array<vec3<f32>, 2> {
    let stride = model.leg_hip.w;
    let lift = model.leg_knee.w;
    let hip0 = model.leg_hip.xz;
    let knee0 = model.leg_knee.xz;
    let ankle0 = model.leg_ankle.xz;
    // The right leg is half a cycle behind the left.
    let phase = fract(walk.y + select(0.5, 0.0, pos.y > 0.0));
    let stance = model.leg_ankle.w;
    let reach = stance * stride;
    var foot = vec2<f32>(0.0);
    var pitch = 0.0;
    if phase < stance {
        foot.x = reach * (0.5 - phase / stance);
    } else {
        let u = (phase - stance) / (1.0 - stance);
        foot = vec2<f32>(reach * (u * u * (3.0 - 2.0 * u) - 0.5), lift * sin(u * PI));
        // Toe down as it leaves the ground, up as it reaches for the next step.
        pitch = 0.4 * sin(u * 2.0 * PI);
    }
    let hip = hip0 + vec2<f32>(0.0, walk_bob(walk, model).z);
    let l1 = distance(knee0, hip0);
    let l2 = distance(ankle0, knee0);
    let want = ankle0 + foot * walk.x - hip;
    let d = clamp(length(want), abs(l1 - l2) + 0.01, l1 + l2 - 0.01);
    let aim = atan2(want.y, want.x);
    let ankle = hip + vec2<f32>(cos(aim), sin(aim)) * d;
    // The knee stays on the side of the leg it rests on.
    let a = knee0 - hip0;
    let b = ankle0 - hip0;
    let side = select(1.0, -1.0, a.x * b.y - a.y * b.x > 0.0);
    let bend = acos(clamp((l1 * l1 + d * d - l2 * l2) / (2.0 * l1 * d), -1.0, 1.0));
    let knee = hip + vec2<f32>(cos(aim + side * bend), sin(aim + side * bend)) * l1;

    var turn = 0.0;
    var pivot0 = vec2<f32>(0.0);
    var pivot = vec2<f32>(0.0);
    if limb == LIMB_THIGH {
        turn = atan2(knee.y - hip.y, knee.x - hip.x) - atan2(a.y, a.x);
        pivot0 = hip0;
        pivot = hip;
    } else if limb == LIMB_SHIN {
        turn = atan2(ankle.y - knee.y, ankle.x - knee.x) - atan2(ankle0.y - knee0.y, ankle0.x - knee0.x);
        pivot0 = knee0;
        pivot = knee;
    } else {
        turn = -pitch * walk.x;
        pivot0 = ankle0;
        pivot = ankle;
    }
    let p = rot_xz(pos - vec3<f32>(pivot0.x, 0.0, pivot0.y), turn) + vec3<f32>(pivot.x, 0.0, pivot.y);
    return array<vec3<f32>, 2>(p, rot_xz(normal, turn));
}

fn rot_x(v: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec3<f32>(v.x, v.y * c - v.z * s, v.y * s + v.z * c);
}

fn rot_y(v: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec3<f32>(v.x * c + v.z * s, v.y, -v.x * s + v.z * c);
}

// A model vertex after the unit was destroyed. The same intact mesh, wrecked a
// different way for every wreck (`seed`, zero to one): the turret is blown off
// its ring and lies beside the hull, the hull is crumpled by a smooth field
// (so faces that share a corner still meet), caved in where the killing blow
// landed, and a vehicle settles crooked on its broken running gear. The
// fragment shader takes its normals from the warped surface.
fn wrecked(pos: vec3<f32>, part: u32, model: ModelInfo, seed: f32) -> vec3<f32> {
    let r1 = hash11(seed * 173.3 + 1.7);
    let r2 = hash11(seed * 311.9 + 5.3);
    let r3 = hash11(seed * 97.1 + 9.1);
    let height = max(model.height, 1.0);
    let reach = max(model.bounds_radius, 1.0);
    let mobile = (model.icon & 0x10000u) != 0u;
    var p = pos;
    if part == PART_TURRET {
        let pivot = model.turret_pivot.xyz;
        var q = rot_z(p - pivot, (r1 - 0.5) * 5.0);
        q = rot_x(q, 0.3 + r2 * 0.45);
        q = rot_y(q, (r3 - 0.5) * 0.5);
        // Thrown clear to one side or the other, never along the hull, where it would land in it.
        let away = select(-1.5707963, 1.5707963, r3 > 0.5) + (r2 - 0.5) * 1.1;
        let thrown = reach * (0.95 + r1 * 0.25);
        p = q + vec3<f32>(cos(away), sin(away), 0.0) * thrown;
        p.z = max(p.z + height * 0.16, 0.02);
        return p;
    }
    // Crumple: a smooth field of the position, nothing at the ground and most at the top.
    let amp = clamp(height * 0.11, 0.15, 1.4);
    let at = (p.xy + vec2<f32>(p.z * 0.61, p.z * 0.37)) / (reach * 1.7) + vec2<f32>(seed * 3.1, seed * 7.7);
    let field = textureSampleLevel(noise_map, repeat_sampler, at, 2.0);
    let rise = smoothstep(0.04, 0.55, p.z / height);
    p += vec3<f32>(field.b - 0.5, field.a - 0.5, -abs(field.b - field.a)) * 2.4 * amp * rise;
    // Caved in around where it was hit.
    let hit = vec2<f32>(r1 - 0.5, r2 - 0.5) * reach * 0.9;
    let d = distance(p.xy, hit) / (reach * 0.5);
    p.z *= 1.0 - 0.6 * exp(-d * d);
    if part == PART_LOCOMOTION {
        // Tracks thrown and splayed, legs folded.
        p = vec3<f32>(p.x, p.y * 1.07, p.z * 0.82);
    }
    if mobile {
        p = rot_y(rot_x(p, (r2 - 0.5) * 0.2), (r3 - 0.5) * 0.14);
        p.z = max(p.z - height * 0.03, 0.0);
    }
    return p;
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    let e = load_entity(visible[in.instance]);
    let model = models[e.blueprint];
    let t = globals.sun.w;
    let time = globals.camera.w;
    var scale = 1.0;
    if (e.owner_flags & KIND_PROP) != 0u && e.scale != 0u {
        scale = f32(e.scale) * 0.001;
    }

    var p = in.pos;
    var n = in.normal;
    // What the next upgrade adds is not there at all until the refit is under way.
    let piece = (in.rig & RIG_UPGRADE) != 0u;
    if piece && (e.upgrade <= 0.0 || (e.owner_flags & (KIND_WRECK | KIND_GHOST)) != 0u) {
        var hidden: VsOut;
        hidden.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
        return hidden;
    }
    let limb = in.rig & 0xFu;
    let walks = model.leg_hip.w > 0.0 && (e.owner_flags & KIND_WRECK) == 0u;
    var walk = vec2<f32>(0.0);
    if walks {
        walk = walk_state(e, model);
    }
    if (e.owner_flags & KIND_WRECK) != 0u {
        p = wrecked(p, in.part, model, hash11(f32(e.unit_id & 0xFFFFu)));
    } else if in.part == PART_TURRET {
        // A forearm points up or down at what it aims at, about its elbow, before the torso turns.
        if limb >= LIMB_ARM_GUN && model.arm_pivot.w > 0.0 {
            let ends = select(e.arm_pitch.zw, e.arm_pitch.xy, limb == LIMB_ARM_GUN);
            let pitch = mix(ends.x, ends.y, t);
            let elbow = vec3<f32>(model.arm_pivot.x, 0.0, model.arm_pivot.z);
            p = rot_xz(p - elbow, pitch) + elbow;
            n = rot_xz(n, pitch);
        }
        // The turret glides between ticks like the hull does; stepping it ten times a second reads as jitter.
        let yaw = lerp_angle(e.prev_turret_yaw, e.turret_yaw, t);
        let pivot = model.turret_pivot.xyz;
        p = rot_z(p - pivot, yaw) + pivot;
        n = rot_z(n, yaw);
    } else if in.part == PART_SPINNER && (e.owner_flags & (KIND_WRECK | FLAG_UNDER_CONSTRUCTION)) == 0u {
        let pivot = model.spinner_pivot.xyz;
        let spin = time * 1.6 + f32(e.unit_id & 255u);
        p = rot_z(p - pivot, spin) + pivot;
        n = rot_z(n, spin);
    } else if in.part == PART_LOCOMOTION && walks && limb != 0u {
        let posed = walk_leg(p, n, limb, model, walk);
        p = posed[0];
        n = posed[1];
    } else if in.part == PART_LOCOMOTION && !walks && (e.owner_flags & FLAG_MOVING) != 0u {
        // Running gear without a rig: a small shudder.
        let phase = time * 9.0 + p.x * 0.8 + sign(p.y) * 1.57;
        p.z += max(sin(phase), 0.0) * 0.12 * model.height * 0.1;
    }
    if walks && in.part != PART_LOCOMOTION {
        p += walk_bob(walk, model);
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
    out.model_class = ((model.icon >> 8u) & 0xFFu) | ((model.icon >> 8u) & 0x100u);
    // Pieces go up one after another over the first four fifths of the refit, each taking a fifth.
    let begins = f32((in.rig >> 16u) & 0xFFu) / 255.0 * 0.8;
    out.refit = vec3<f32>(select(0.0, 1.0, piece), clamp((e.upgrade - begins) / 0.2, 0.0, 1.0), e.upgrade);
    let weld = vec3<f32>(e.weld0, e.weld1, e.weld2);
    let reach = max(model.bounds_radius, 1.0);
    let height = max(model.height, 1.0);
    let far = vec3<f32>(
        select(reach, -reach, weld.x > 0.0),
        select(reach, -reach, weld.y > 0.0),
        select(0.0, height, weld.z > height * 0.5),
    );
    out.weld = vec4<f32>(weld, max(distance(weld, far), 1.0));
    return out;
}

// Soot and scorch from the model position: several incommensurate scales so
// the noise tile never marches across a hull, plus streaks that climb with
// height the way fire does.
fn wreck_burn(local: vec3<f32>, seed: f32) -> vec2<f32> {
    let field = local.xy + vec2<f32>(local.z * 0.53, local.z * 0.29);
    let off = vec2<f32>(seed * 47.0, seed * 19.0);
    let at = field + off;
    let coarse = noise_varied(at, 28.0).ba;
    let mid = noise_varied(at, 11.0).ba;
    let fine = textureSample(noise_map, repeat_sampler, field * 0.21 + off * 0.03).ba;
    let climb = textureSample(noise_map, repeat_sampler, vec2<f32>(field.x * 0.08, local.z * 0.19) + off * 0.02).a;
    let blotch = value_noise2(at, 7.0);
    let soot = mix(coarse, mid, 0.55);
    return vec2<f32>(
        clamp(soot.x * 0.5 + fine.x * 0.22 + climb * 0.18 + blotch * 0.2, 0.0, 1.0),
        clamp(soot.y * 0.55 + fine.y * 0.25 + climb * 0.2, 0.0, 1.0)
    );
}

@fragment
fn fs_shadow(in: VsOut) {
    // Unbuilt parts of a construction site cast no shadow.
    if (in.owner_flags & FLAG_UNDER_CONSTRUCTION) != 0u {
        let grow = clamp(in.state.x / 0.74, 0.0, 1.0);
        let radius = grow * grow * (3.0 - 2.0 * grow) * in.weld.w;
        if distance(in.local, in.weld.xyz) > radius {
            discard;
        }
    }
    // Nor does an upgrade piece that is still a hologram.
    if in.refit.x > 0.5 && in.refit.y <= 0.0 {
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
        case 13u: { m.albedo = vec3<f32>(0.3, 0.17, 0.02); m.emissive = AMBER * 4.0; m.roughness = 0.3; }
        case 14u: { m.albedo = globals.plating.rgb * vec3<f32>(0.14, 0.15, 0.17); m.metallic = 0.45; m.roughness = 0.42; }
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
    // A wreck's mesh is warped in the vertex shader, so its faces' normals come from the surface itself.
    let face = cross(dpdx(in.world), dpdy(in.world));
    if (flags & KIND_WRECK) != 0u && dot(face, face) > 1e-16 {
        let facing = normalize(face);
        n = facing * select(-1.0, 1.0, dot(facing, globals.camera.xyz - in.world) > 0.0);
    }
    let eye = globals.camera.xyz;
    let v = normalize(eye - in.world);
    let dist = distance(eye, in.world);

    // Panel lines and plate seams from the tiling normal map, on hull materials only.
    // Wrecks skip it: the mesh is already crumpled, and the plate tile would march.
    if ((in.material <= MAT_METAL && in.material != MAT_GLOW) || in.material == MAT_PLATING_DARK)
        && (flags & KIND_WRECK) == 0u {
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
    var tread = 1.0;
    if in.material == MAT_TREAD {
        // Track links: cleats across the run, crawling backwards while the unit drives.
        var crawl = 0.0;
        if (flags & FLAG_MOVING) != 0u && (flags & KIND_WRECK) == 0u {
            crawl = time * 6.0;
        }
        let link = fract(in.local.x * 2.6 + crawl);
        tread = 0.45;
        let cleat = smoothstep(0.0, 0.12, link) * (1.0 - smoothstep(0.55, 0.67, link));
        let detail = clamp(1.0 - dist / 500.0, 0.0, 1.0);
        m.albedo = mix(m.albedo, mix(vec3<f32>(0.012), vec3<f32>(0.075, 0.07, 0.065), cleat), detail);
        m.metallic = 0.5 * cleat * detail;
        m.roughness = mix(m.roughness, 0.55, cleat * detail);
    }
    if in.material == MAT_GLOW_AMBER && (flags & FLAG_BUILDING) != 0u {
        // Construction emitters run hot while the unit builds.
        m.emissive *= 1.6 + 0.7 * sin(time * 11.0 + in.state.w * 40.0);
    }
    if (flags & (KIND_PROP | KIND_GHOST)) == 0u && in.material != MAT_GLOW && in.material != MAT_GLOW_ORANGE && in.material != MAT_GLOW_AMBER {
        // Field dirt: dust thrown up over the running gear and lower hull, and
        // grime settling where the wear map says. Plain tech 1 kit is the
        // dirtiest; the higher tiers stay closer to parade white.
        let tech = f32(max(in.model_class & 0xFFu, 1u));
        let amount = 1.25 / tech;
        let wear = textureSample(panel_map, repeat_sampler, in.uv * 0.11 + vec2<f32>(in.state.w * 7.0)).a;
        var low = 1.0 - smoothstep(0.0, 0.62, in.state.z);
        if (in.model_class & 0x100u) == 0u {
            // Structures only pick it up around the footing.
            low = 1.0 - smoothstep(0.0, 0.12, in.state.z);
        }
        let dust = clamp((low * 0.95 + smoothstep(0.42, 0.78, wear) * 0.5) * amount * tread, 0.0, 0.85);
        m.albedo = mix(m.albedo, vec3<f32>(0.2, 0.165, 0.12) * (0.7 + wear * 0.6), dust);
        m.roughness = mix(m.roughness, 0.92, dust);
        m.metallic *= 1.0 - dust;
        m.emissive *= 1.0 - dust;
    }

    if (flags & KIND_WRECK) != 0u {
        // Burnt out: charred, matte, dead emitters; fades into the ground as it is reclaimed.
        // Soot, scorched paint and bare burnt steel, from a field of the model position, so
        // two faces lying in the same plane are shaded alike and cannot flicker against each other.
        let burn = wreck_burn(in.local, in.state.w);
        let paint = m.albedo * 0.16;
        let steel = vec3<f32>(0.11, 0.075, 0.055) * (0.6 + burn.y);
        m.albedo = mix(vec3<f32>(0.022, 0.02, 0.019), mix(paint, steel, smoothstep(0.45, 0.7, burn.y)), smoothstep(0.3, 0.75, burn.x));
        // A rotated cut of the plate map, so armour seams still read without marching.
        let plate_uv = vec2<f32>(in.local.x * 0.07 + in.local.y * 0.04, -in.local.x * 0.04 + in.local.z * 0.08) + vec2<f32>(in.state.w * 2.3, in.state.w * 1.1);
        let plate = textureSample(panel_map, repeat_sampler, plate_uv);
        m.albedo *= 0.62 + plate.b * 0.38;
        m.metallic = 0.3 * smoothstep(0.5, 0.7, burn.y);
        m.roughness = 0.92;
        m.emissive = vec3<f32>(0.0);
        if in.state.z > in.state.y * 0.75 + 0.25 {
            discard;
        }
    }

    let shadow = sun_shadow(in.world, n);
    var color = shade_pbr(m, n, v, globals.sun.xyz, shadow);
    var alpha = 1.0;

    if (flags & FLAG_UNDER_CONSTRUCTION) != 0u {
        // Grows from the weld; the last stretch is the whole thing cooling off.
        let build = in.state.x;
        let d = distance(in.local, in.weld.xyz);
        let grow = clamp(build / 0.74, 0.0, 1.0);
        let radius = grow * grow * (3.0 - 2.0 * grow) * in.weld.w;
        let cool = smoothstep(0.74, 1.0, build);
        if d > radius {
            // Still coming: an amber lattice, brightest where the front is about to take it.
            let scan = fract(dot(in.local, vec3<f32>(0.38, 0.41, 0.72)) - time * 0.7);
            let lattice = step(0.88, fract(in.local.x * 0.48 + in.local.z * 0.06))
                + step(0.88, fract(in.local.y * 0.48))
                + step(0.9, scan);
            let near = smoothstep(7.0, 0.0, d - radius);
            let speckle = step(0.975, hash21(in.local.xy + vec2<f32>(in.local.z, in.state.w)));
            if lattice + speckle < 0.5 && near < 0.7 {
                discard;
            }
            color = AMBER * (1.7 + 2.4 * near) + vec3<f32>(1.0, 0.82, 0.35) * near * near * 3.5;
        } else {
            // Printed: white-hot at the front, warm behind it, then the late cool-off.
            let front = smoothstep(2.2, 0.0, radius - d);
            let heat = (1.0 - cool) * mix(0.5, 1.0, front);
            let molten = vec3<f32>(1.0, 0.68, 0.22) * 2.5 + AMBER * 0.85;
            color = mix(color, molten, heat);
            color += vec3<f32>(1.0, 0.78, 0.32) * front * (1.0 - cool) * 3.2;
            let shimmer = 0.07 * sin(time * 11.0 + in.local.x * 1.7 + in.state.w * 18.0);
            color += AMBER * shimmer * heat;
        }
    }
    if in.refit.z > 0.0 {
        if in.refit.x > 0.5 && in.refit.y <= 0.0 {
            // A piece whose turn has not come: a scanning hologram of it, in amber.
            let scan = fract(in.local.z * 1.4 - time * 0.9);
            let grid = step(0.8, fract(in.local.x * 2.0)) + step(0.8, fract(in.local.y * 2.0)) + step(0.85, scan);
            if grid < 0.5 {
                discard;
            }
            color = AMBER * 2.2;
        } else if in.refit.x > 0.5 {
            // Going up: white-hot at first, cooling into the finished piece.
            let cool = smoothstep(0.0, 1.0, in.refit.y);
            color = mix(vec3<f32>(1.0, 0.66, 0.22) * 2.6, color, cool) + AMBER * 0.9 * (1.0 - cool);
        } else {
            // The unit being refitted: bands of work light passing up and down it.
            let sweep = abs(fract(time * 0.23) * 2.0 - 1.0);
            let off = (in.state.z - sweep) * 16.0;
            let seam = step(0.9, fract(in.local.z * 1.1 + in.local.x * 0.4));
            color += AMBER * (1.7 * exp(-off * off) + 0.25 * seam * (0.5 + 0.5 * sin(time * 6.0 + in.local.z)));
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
    // A mirror-smooth highlight can exceed what the half-float target holds and
    // turn into an infinity, which the bloom would then smear across the screen.
    color = min(max(color, vec3<f32>(0.0)), vec3<f32>(48.0));
    return vec4<f32>(color, alpha);
}
