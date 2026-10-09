// A winged flier's rig (`models::Flight`, `ModelInfo::flight`): how its wings, trailing
// legs, head and arms move with what it does, and how its whole body hangs and leans.
// Everything is read off the unit as it is drawn, nothing written back: its travel
// this tick and last (`Entity::gait`), its turn (`Entity::drive_swing`; both eased by the
// renderer, flight_ease.rs), its aim, and its dash (`Entity::deploy`,
// `mirror::dash_pose`: 0 to 1 through the windup, then -1 at the landing easing back to 0).
//
// - Idle it breathes: the body heaves and sways slowly on its lift, the wings rise and
//   settle with each breath while a ripple runs down each fan from the crown to the
//   spike, the legs swing a little out of step, the head looks about.
// - Moving, the wings sweep back and the fans close toward their middle, more as it
//   makes more of its top speed; speeding up throws them further back, braking opens
//   them. The body leans into the travel and banks into a turn, the inner wing tucking
//   and the outer opening. The legs trail behind, the shins folding back.
// - Winding up a dash, it draws itself in: the wings flare open, rise and come forward,
//   the legs draw up under it and the body crouches. At the landing everything is
//   thrown out by the blow and springs back over a second.
// Used by entity.wgsl (`//!use flight`).

// What drives the pose, worked out once per vertex from the instance.
struct Flight {
    // The share of its top speed it makes (0 to 1), and how hard it is speeding up
    // (positive) or braking (negative), -1 to 1.
    speed: f32,
    surge: f32,
    // How hard it is turning, -1 to 1, positive to the left.
    turn: f32,
    // Through a dash's windup, 0 to 1; how hard it has just landed, 1 at the blow.
    charge: f32,
    land: f32,
    // 1 at rest, falling to 0 as its guns come up to fight.
    calm: f32,
    // Seconds, and a per-unit seed.
    time: f32,
    seed: f32,
}

// Rest pitch below which a lowered gun arm counts as at ease (radians).
const FLIGHT_ARM_EASE: f32 = -1.1;

fn flies(model: ModelInfo) -> bool {
    return model.flight[0].w > 0.0;
}

fn flight_state(e: Entity, model: ModelInfo, t: f32, time: f32) -> Flight {
    var f: Flight;
    let top = max(model.flight[0].w, 0.01);
    let now = e.gait.y / top;
    let was = e.gait.z / top;
    f.speed = clamp(mix(was, now, t), 0.0, 1.0);
    // A full speed change in about a second reads as a full surge.
    f.surge = clamp((now - was) * 8.0, -1.0, 1.0);
    // Eased by the renderer (flight_ease.rs), as is the speed in `gait`: the sim's own
    // turn and surge jump between nothing and full at a tick, and the wings snapped.
    f.turn = clamp(mix(e.drive_swing.x, e.drive_swing.y, t), -1.0, 1.0);
    let dash = mix(e.prev_deploy, e.deploy, t);
    f.charge = clamp(dash, 0.0, 1.0);
    f.land = clamp(-dash, 0.0, 1.0);
    let raised = clamp((mix(e.arm_pitch.x, e.arm_pitch.y, t) - FLIGHT_ARM_EASE) / 0.8, 0.0, 1.0);
    f.calm = 1.0 - max(raised, f.charge);
    f.time = time;
    f.seed = hash11(f32(e.unit_id & 0xFFFFu) * 0.613 + 1.7);
    return f;
}

// A breath: a slow swell with a little of a quicker one on it, -1 to 1.
fn flight_breath(f: Flight, offset: f32) -> f32 {
    let b = f.time * 0.85 + f.seed * 40.0 + offset;
    return 0.8 * sin(b) + 0.2 * sin(b * 2.3 + 0.7);
}

// The landing's spring: thrown out at the blow, then back past rest and settling.
fn flight_spring(f: Flight) -> f32 {
    let u = 1.0 - f.land;
    return f.land * cos(u * 9.0);
}

// For idle_pose's slots: x head yaw, y head nod, z the gun arms' pitch (added to their
// aim), w unused. It looks about slowly while at ease, and its lowered arms sway with its
// breathing, a little out of step with the wings.
fn flight_idle(f: Flight) -> vec4<f32> {
    let look = f.calm * (1.0 - 0.6 * f.speed);
    let yaw = glance(f.time, f.seed, 4.2) * 0.9 + 0.03 * sin(f.time * 0.7 + f.seed * 30.0);
    let nod = glance(f.time, f.seed + 0.41, 4.2) * 0.18 - 0.06;
    let arms = (0.045 * flight_breath(f, 0.9) - 0.12 * f.speed - 0.25 * f.charge) * f.calm
        + 0.2 * flight_spring(f);
    return vec4<f32>(yaw * look, nod * look, arms, 0.0);
}

// One wing feather's (or the vanes', `i` 3) turn about the hub, in the left wing's frame:
// x its fan (about the wing's normal; positive raises it), y the whole wing's sweep (about
// the vertical; positive sends its tips back), z its lift (the whole wing raised about
// the fore-and-aft axis). `side` 1 for the left wing, -1 for the right.
fn feather_turn(f: Flight, i: f32, side: f32) -> vec3<f32> {
    let mid = 3.0;
    let inner = max(f.turn * side, 0.0);
    let outer = max(-f.turn * side, 0.0);
    // How far the fan closes toward its middle: with speed and the inner wing of a turn;
    // breathing a little; flared open for the dash and thrown open at the landing.
    let close = FLIGHT_CLOSE_SPEED * f.speed + 0.25 * inner - 0.15 * outer + 0.05 * flight_breath(f, 0.0)
        + FLIGHT_CLOSE_CHARGE * f.charge - 0.3 * flight_spring(f) - 0.1 * max(-f.surge, 0.0);
    // A ripple runs down the fan from the crown, each feather a beat behind the one above.
    let ripple = 0.035 * sin(f.time * 1.6 + f.seed * 12.0 - i * 0.7) * (1.0 - 0.5 * f.speed);
    let fan = (i - mid) * FLIGHT_FAN_STEP * close + ripple;
    let sweep = FLIGHT_SWEEP_SPEED * f.speed + 0.3 * max(f.surge, 0.0) - 0.15 * max(-f.surge, 0.0)
        + 0.18 * inner - 0.08 * outer + FLIGHT_SWEEP_CHARGE * f.charge + 0.4 * flight_spring(f)
        + 0.03 * flight_breath(f, 1.6);
    let lift = 0.07 * flight_breath(f, 0.0) + FLIGHT_LIFT_SPEED * f.speed + FLIGHT_LIFT_CHARGE * f.charge
        - 0.25 * flight_spring(f) + 0.06 * inner;
    return vec3<f32>(fan, sweep, lift);
}

// A vertex of the flight rig, posed: `piece` its `FLIGHT_*` code. Model space at rest, before
// the torso turns. Returns the position and the normal.
fn flight_pose(p0: vec3<f32>, n0: vec3<f32>, piece: u32, model: ModelInfo, f: Flight) -> array<vec3<f32>, 2> {
    let side = select(-1.0, 1.0, p0.y >= 0.0);
    // Worked in the left side's frame.
    var p = vec3<f32>(p0.x, p0.y * side, p0.z);
    var n = vec3<f32>(n0.x, n0.y * side, n0.z);
    if piece >= FLIGHT_FEATHER && piece <= FLIGHT_VANE {
        let i = select(f32(piece - FLIGHT_FEATHER), 3.0, piece == FLIGHT_VANE);
        let turn = feather_turn(f, i, side);
        let hub = model.flight[0].xyz;
        p = rot_z(rot_x(rot_x(p - hub, turn.x), turn.z), turn.y) + hub;
        n = rot_z(rot_x(rot_x(n, turn.x), turn.z), turn.y);
    } else if piece == FLIGHT_THIGH || piece == FLIGHT_SHIN {
        // Each leg a little out of step with the other.
        let sway = flight_breath(f, select(2.2, 0.4, side > 0.0));
        let thigh = -0.38 * f.speed - 0.15 * f.surge + 0.035 * sway * f.calm
            + 0.32 * f.charge + 0.3 * flight_spring(f) - 0.08 * f.turn * side;
        let shin = -0.3 * f.speed + 0.05 * sway * f.calm - 0.55 * f.charge
            - 0.45 * flight_spring(f);
        let hip = model.flight[1].xyz;
        if piece == FLIGHT_SHIN {
            let knee = select(model.flight[3].xyz, model.flight[2].xyz, side > 0.0);
            p = rot_xz(p - knee, shin) + knee;
            n = rot_xz(n, shin);
        }
        p = rot_xz(p - hip, thigh) + hip;
        n = rot_xz(n, thigh);
    }
    return array<vec3<f32>, 2>(vec3<f32>(p.x, p.y * side, p.z), vec3<f32>(n.x, n.y * side, n.z));
}

// How hard the red rims of the rig's plates burn against their rest (`i` the feather, 3
// for the rest): a slow shimmer running down each fan while it idles, brighter with speed,
// climbing white-hot and restless through a dash's windup, a flash at the landing.
fn flight_glow(f: Flight, i: f32) -> f32 {
    let shimmer = 0.5 * pow(max(sin(f.time * 1.1 + f.seed * 9.0 - i * 0.75), 0.0), 3.0) * f.calm;
    let flicker = 0.85 + 0.15 * sin(f.time * 31.0 + i * 2.1);
    return 0.8 + shimmer + 1.1 * f.speed + 5.0 * f.charge * f.charge * flicker + 6.0 * f.land * f.land;
}

// The whole body hanging on its lift: x its pitch (negative leans it forward), y its
// roll (positive raises the left), z how far it rides up or down (metres).
fn flight_hang(f: Flight, model: ModelInfo) -> vec3<f32> {
    let size = model.flight[1].z;
    let pitch = -FLIGHT_LEAN_SPEED * f.speed - 0.12 * f.surge - FLIGHT_LEAN_CHARGE * f.charge
        + 0.1 * flight_spring(f)
        + 0.012 * flight_breath(f, 2.4);
    let roll = -0.22 * f.turn + 0.015 * sin(f.time * 0.55 + f.seed * 21.0) * f.calm;
    let heave = size * (0.025 * flight_breath(f, 0.3) * (1.0 - 0.6 * f.speed)
        - 0.06 * f.charge - 0.05 * flight_spring(f));
    return vec3<f32>(pitch, roll, heave);
}

// Leans and lifts the posed model as it hangs (`flight_hang`), about its hips.
fn flight_lean(p0: vec3<f32>, n0: vec3<f32>, model: ModelInfo, f: Flight) -> array<vec3<f32>, 2> {
    let hang = flight_hang(f, model);
    let hips = vec3<f32>(0.0, 0.0, model.flight[1].z);
    let p = rot_x(rot_xz(p0 - hips, hang.x), hang.y) + hips + vec3<f32>(0.0, 0.0, hang.z);
    let n = rot_x(rot_xz(n0, hang.x), hang.y);
    return array<vec3<f32>, 2>(p, n);
}
