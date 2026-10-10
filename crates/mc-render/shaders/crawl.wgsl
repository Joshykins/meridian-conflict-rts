// A many-legged walker's legs (`models::Crawl`, `ModelInfo::crawl`): where its body rides
// over its feet, how each two-boned leg is solved through its stride, how it sets itself
// for a shot, and how a walker that braces to fire (`Crawl::brace`) steps its feet out and
// settles its body to plant. Used by entity.wgsl (`//!use crawl`).

// A many-legged walker (`models::Crawl`, `model.crawl`): where the body rides over its
// feet, from the ground under its first and last pairs (as `walk_ground` does for two feet).
fn crawl_body_ground(e: Entity, model: ModelInfo, t: f32) -> f32 {
    let heading = lerp_angle(e.prev_heading, e.heading, t);
    let at = mix(e.prev_pos, e.pos, t).xy;
    let fwd = vec2<f32>(cos(heading), sin(heading));
    let lft = vec2<f32>(-fwd.y, fwd.x);
    let centre = terrain_height(at);
    let last = u32(model.crawl[0].x + 0.5) - 1u;
    let front = model.crawl[3u].xy;
    let back = model.crawl[3u + 3u * last].xy;
    let g0 = terrain_height(at + fwd * front.x + lft * front.y) - centre;
    let g1 = terrain_height(at + fwd * front.x - lft * front.y) - centre;
    let g2 = terrain_height(at + fwd * back.x + lft * back.y) - centre;
    let g3 = terrain_height(at + fwd * back.x - lft * back.y) - centre;
    return mix(0.25 * (g0 + g1 + g2 + g3), min(min(g0, g1), min(g2, g3)), 0.4);
}

// A future/past ground contact along the body's translation and turn. During stance
// `advance` counts back at exactly the body's ground speed: a pivoting body's planted
// feet stay fixed in world space, then lift and step around the outside of its turn.
fn crawl_contact(foot: vec2<f32>, advance: f32, travel: vec2<f32>, turn: f32,
                 ground: f32) -> vec3<f32> {
    let v = travel / max(ground, 0.001);
    let omega = turn / max(ground, 0.001);
    let angle = omega * advance;
    let c = cos(angle);
    let s = sin(angle);
    var shift = v * advance;
    if abs(omega) > 0.0001 {
        shift = vec2<f32>(s * v.x - (1.0 - c) * v.y,
                         (1.0 - c) * v.x + s * v.y) / omega;
    }
    return vec3<f32>(vec2<f32>(c * foot.x - s * foot.y, s * foot.x + c * foot.y) + shift, angle);
}

// A leg vertex of a many-legged walker, posed for this moment of the stride. Each leg has
// two bones, hip to knee and knee to the foot's tip, in the vertical plane through its
// hip and foot; the foot is planted and passes back under the body, or lifts and swings
// forward, each pair at its own phase (the right leg half a cycle after the left, or with
// it for a lone leg on the centreline). The plane turns about the hip to follow the foot,
// and the bones are solved in it. `body` is
// how far the hips ride off their rest (the body's bob and the ground under it).
fn crawl_leg(pos: vec3<f32>, normal: vec3<f32>, limb: u32, pair: u32, model: ModelInfo,
             walk: vec2<f32>, e: Entity, t: f32, body: vec3<f32>) -> array<vec3<f32>, 2> {
    let left = pos.y > 0.0;
    let flip = vec3<f32>(1.0, select(-1.0, 1.0, left), 1.0);
    let hip0 = model.crawl[1u + 3u * pair].xyz * flip;
    let knee0 = model.crawl[2u + 3u * pair].xyz * flip;
    let foot0 = model.crawl[3u + 3u * pair].xyz * flip;
    // A lone leg on the centreline (a tripod's third, `Crawl::lone`) walks both its halves
    // in step; a pair's right leg is half a cycle after its left.
    let lone = model.crawl[2u + 3u * pair].w > 0.5;
    let phase = fract(walk.y - model.crawl[1u + 3u * pair].w + select(select(0.5, 0.0, left), 0.0, lone));
    let stride = model.leg_hip.w;
    let lift = model.leg_knee.w;
    // Each pair's own stance (`Crawl::stance`): a shorter one is a shorter step.
    let stance = model.crawl[3u + 3u * pair].w;
    let reach = stance * stride;
    var step = vec2<f32>(0.0);
    if phase < stance {
        step.x = reach * (0.5 - phase / stance);
    } else {
        let u = (phase - stance) / (1.0 - stance);
        // Match the planted foot's velocity at both ends, with a soft lift and
        // landing. The leg retracts briefly before reaching ahead, without a snap.
        let advance = reach * (u * u * (3.0 - 2.0 * u) - 0.5)
            - stride * (1.0 - stance) * (2.0 * u * u * u - 3.0 * u * u + u);
        let arc = sin(u * PI);
        step = vec2<f32>(advance, lift * arc * arc);
    }
    // The foot comes down on the ground under it.
    let heading = lerp_angle(e.prev_heading, e.heading, t);
    let at = mix(e.prev_pos, e.pos, t).xy;
    let fwd = vec2<f32>(cos(heading), sin(heading));
    let lft = vec2<f32>(-fwd.y, fwd.x);
    let travel = e.pos.xy - e.prev_pos.xy;
    let local_travel = vec2<f32>(dot(travel, fwd), dot(travel, lft));
    let turn = atan2(sin(e.heading - e.prev_heading), cos(e.heading - e.prev_heading));
    // Where the foot stands at rest now: in toward the hip packed, stepping out to brace.
    let rest = crawl_brace_foot(foot0, hip0, pair * 2u + select(1u, 0u, left), model, e, t);
    let contact = crawl_contact(rest.xy, step.x * walk.x, local_travel, turn, e.gait.y);
    let ground = terrain_height(at + fwd * contact.x + lft * contact.y) - terrain_height(at);
    let foot = vec3<f32>(contact.xy, rest.z + step.y * walk.x + ground);
    let hip = hip0 + body;

    // The leg's plane at rest and now, and the bones in it: r out from the hip, z up.
    let az0 = atan2(foot0.y - hip0.y, foot0.x - hip0.x);
    let az = atan2(foot.y - hip.y, foot.x - hip.x);
    let dir0 = vec2<f32>(cos(az0), sin(az0));
    let k0 = vec2<f32>(dot(knee0.xy - hip0.xy, dir0), knee0.z - hip0.z);
    let f0 = vec2<f32>(dot(foot0.xy - hip0.xy, dir0), foot0.z - hip0.z);
    let f1 = vec2<f32>(length(foot.xy - hip.xy), foot.z - hip.z);
    let l1 = length(k0);
    let l2 = distance(f0, k0);
    let d = clamp(length(f1), abs(l1 - l2) + 0.01, l1 + l2 - 0.01);
    let aim = atan2(f1.y, f1.x);
    // The knee stays on the side of the leg it rests on (up, for a spider's knee).
    let side = select(1.0, -1.0, k0.x * f0.y - k0.y * f0.x > 0.0);
    let bend = acos(clamp((l1 * l1 + d * d - l2 * l2) / (2.0 * l1 * d), -1.0, 1.0));
    let k1 = vec2<f32>(cos(aim + side * bend), sin(aim + side * bend)) * l1;
    let tip = vec2<f32>(cos(aim), sin(aim)) * d;

    var q = rot_z(pos - hip0, -az0);
    var m = rot_z(normal, -az0);
    if limb == LIMB_FOOT {
        // Level contact patch: only the shin articulates, never roll the whole foot.
        let qfoot = rot_z(pos - foot0, contact.z) + foot;
        return array<vec3<f32>, 2>(qfoot, rot_z(normal, contact.z));
    }
    if limb == LIMB_THIGH {
        let turn = atan2(k1.y, k1.x) - atan2(k0.y, k0.x);
        q = rot_xz(q, turn);
        m = rot_xz(m, turn);
    } else {
        let turn = atan2(tip.y - k1.y, tip.x - k1.x) - atan2(f0.y - k0.y, f0.x - k0.x);
        q = rot_xz(q - vec3<f32>(k0.x, 0.0, k0.y), turn) + vec3<f32>(k1.x, 0.0, k1.y);
        m = rot_xz(m, turn);
    }
    return array<vec3<f32>, 2>(rot_z(q, az) + hip, rot_z(m, az));
}

// How busy a many-legged walker is, zero at rest to one fighting or building: its tail
// holds steadier and its pincers rise and open while it is.
fn crawl_busy(e: Entity, t: f32) -> f32 {
    let aim = abs(mix(e.arm_pitch.x, e.arm_pitch.y, t)) * 6.0
        + abs(mix(e.arm_pitch.z, e.arm_pitch.w, t)) * 6.0
        + mix(e.prev_recoil, e.recoil, t) * 4.0
        + clamp(mix(e.prev_deploy, e.deploy, t), 0.0, 1.0) * 4.0;
    return clamp(aim, 0.0, 1.0);
}

// How far a many-legged walker's body sinks as it sets itself for a shot: its main gun's
// kick, or a held beam's brace (`mirror::beam_brace`, which rises and falls smoothly as the
// projector runs up and down), a sixteenth of its hips' height at full.
// A walker that braces to fire (`crawl_brace_sink`) also rides higher packed.
fn crawl_set(e: Entity, model: ModelInfo, t: f32) -> f32 {
    let brace = clamp(mix(e.prev_recoil, e.recoil, t), 0.0, 1.0);
    return -model.crawl[1].z * 0.0625 * brace * crawl_alive(e) + crawl_brace_sink(e, model, t);
}

// How far a walker that braces to fire (`models::Crawl::brace`, `BRACE_*`) is braced, 0
// packed to 1 braced, between ticks; braced, as authored, when nothing moves it (a wreck,
// a ghost, a site).
fn crawl_braced(e: Entity, t: f32) -> f32 {
    return select(1.0, clamp(mix(e.prev_deploy, e.deploy, t), 0.0, 1.0), crawl_alive(e) > 0.5);
}

// How much higher a bracing walker's body rides than authored (braced): all of its sink
// packed, settling to none once its legs have stepped out.
fn crawl_brace_sink(e: Entity, model: ModelInfo, t: f32) -> f32 {
    let sink = model.crawl[BRACE_SLOT].x;
    return sink * (1.0 - smoothstep(BRACE_SINK_START, BRACE_SINK_END, crawl_braced(e, t)));
}

// Where a bracing walker's foot `leg` (left then right, front pair first) stands at rest
// now, against `foot0`, where it stands braced: packed, `widen` of the way in toward its
// hip; planting, the legs lift and step out one after another, each over `BRACE_STEP_LEN`
// of the deploy, the last landing as the body starts down.
fn crawl_brace_foot(foot0: vec3<f32>, hip0: vec3<f32>, leg: u32, model: ModelInfo, e: Entity, t: f32) -> vec3<f32> {
    let widen = model.crawl[BRACE_SLOT].y;
    if widen <= 0.0 {
        return foot0;
    }
    let legs = max(2.0 * model.crawl[0].x - 1.0, 1.0);
    let start = (BRACE_SINK_START - BRACE_STEP_LEN) * f32(leg) / legs;
    let u = clamp((crawl_braced(e, t) - start) / BRACE_STEP_LEN, 0.0, 1.0);
    let packed = hip0.xy + (foot0.xy - hip0.xy) * (1.0 - widen);
    let lift = model.leg_knee.w * sin(u * PI);
    return vec3<f32>(mix(packed, foot0.xy, smoothstep(0.0, 1.0, u)), foot0.z + lift);
}

// Whether a unit's living parts move at all: not a wreck, a ghost, a site or in a factory.
fn crawl_alive(e: Entity) -> f32 {
    return select(1.0, 0.0, (e.owner_flags & (KIND_WRECK | KIND_GHOST | FLAG_UNDER_CONSTRUCTION | FLAG_IN_FACTORY)) != 0u);
}
