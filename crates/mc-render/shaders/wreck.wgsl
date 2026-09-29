// ---- Wrecks: how a destroyed unit lies and looks (entity.wgsl) ------------------------
//
// Nothing here is per unit. How a wreck lies comes from its model (size, height, reach)
// and domain (a structure, a land or sea unit, an aircraft, a spacecraft) and from how it
// came down (`mc_sim::tables::Landing`, in the pose word `mirror/wrecks.rs` writes into
// `Entity::refit_modules`). A big hull is broken into sections by the mirror: each is an
// instance that keeps its stretch of the hull along the model's length (`arm_pitch.zw`, in
// shares of the model's reach) and lies where the mirror put it, with a second instance
// for the hull's inside (the model mirrored across its centre line, so its inner faces
// face out and show through the torn ends).

struct WreckPose {
    // The pose word is set: a posed wreck, not a spent casing.
    posed: bool,
    landing: u32,
    section: u32,
    count: u32,
    // The hull's inside, drawn mirrored.
    inner: bool,
    // The stretch of the hull this instance keeps along model x, metres, and its middle.
    lo: f32,
    hi: f32,
    centre: f32,
    // Seconds since it was left (a large number once long settled), and how far it has
    // settled into its pose: at once on a blast or an impact, slowly on the seabed.
    age: f32,
    settled: f32,
    // It went down in the sea: it slumps onto the bottom and burns out slowly.
    sank: bool,
}

fn wreck_pose(e: Entity, model: ModelInfo, t: f32) -> WreckPose {
    var w: WreckPose;
    let word = e.refit_modules;
    w.posed = (e.owner_flags & KIND_WRECK) != 0u && e.packed == 0u && (word & WRECK_POSED) != 0u;
    w.landing = word & WRECK_LANDING_MASK;
    w.section = (word >> WRECK_SECTION_SHIFT) & 15u;
    w.count = max((word >> WRECK_COUNT_SHIFT) & 15u, 1u);
    w.inner = w.posed && (word & WRECK_INNER) != 0u;
    let reach = max(model.bounds_radius, 1.0);
    w.lo = select(-1e6, e.arm_pitch.z * reach, w.posed && w.count > 1u);
    w.hi = select(1e6, e.arm_pitch.w * reach, w.posed && w.count > 1u);
    w.centre = select(0.0, 0.5 * (max(w.lo, -reach) + min(w.hi, reach)), w.posed && w.count > 1u);
    // A fresh wreck carries its age (`mirror::UnitInstance::gait`); one long settled lies as it fell.
    w.age = select(1000.0, mix(e.gait.x, e.gait.y, t), e.gait.z > 0.5);
    w.sank = w.landing == WRECK_LANDING_SANK || w.landing == WRECK_LANDING_DITCHED;
    w.settled = smoothstep(0.0, select(0.35, 2.5, w.sank), w.age);
    return w;
}

// How deep a wreck lies in the ground, metres: a structure slumps onto its lot, a vehicle
// settles into the dirt, a ship into the silt, and anything out of the sky is driven in,
// a spacecraft's sections deepest. The more of it is gone (`health`, the share of its mass
// left) the lower what is left lies.
fn wreck_bury(model: ModelInfo, w: WreckPose, health: f32) -> f32 {
    if !w.posed {
        return 0.0;
    }
    let mobile = (model.icon & ICON_MOBILE) != 0u;
    let capital = (model.icon & ICON_CAPITAL) != 0u;
    var share = 0.06;
    if !mobile {
        share = 0.025;
    } else if w.landing == WRECK_LANDING_CRASHED {
        share = select(0.13, 0.17, capital);
    } else if w.landing == WRECK_LANDING_SANK || w.landing == WRECK_LANDING_DITCHED {
        share = 0.1;
    } else if capital {
        share = 0.09;
    }
    share = share * w.settled + (1.0 - clamp(health, 0.0, 1.0)) * 0.25;
    // A big hull goes in by less of its height than a small one: the ground gives less
    // way the more there is of it.
    let h = min(max(model.height, 1.0), max(model.bounds_radius, 1.0) * 0.9);
    return share * h / (1.0 + h / 40.0);
}

// A model vertex after the unit was destroyed. The same intact mesh, wrecked a
// different way for every wreck (`seed`, zero to one): the turret is blown off
// its ring and lies beside the hull, the hull is crumpled by a smooth field
// (so faces that share a corner still meet), twisted along its length, dented,
// caved in where the killing blow landed, and a vehicle settles crooked on its
// broken running gear. A section's torn ends are crushed in. The fragment shader
// takes its normals from the warped surface. The less of it is left (`health`),
// the flatter and more crumpled it is.
//
// `age` is seconds since the wreck was left. The turret does not appear where it
// lands: it leaves its ring at the yaw it died at (`yaw`), flies a ballistic arc
// tumbling as it goes, and hops once when it hits the ground. The hull crumples
// over the blast's first moments. A wreck long settled passes a large `age`. A hull
// that went down in the sea keeps its turret, and slumps slowly onto the seabed.
fn wrecked(pos: vec3<f32>, part: u32, model: ModelInfo, seed: f32, yaw: f32, age: f32, health: f32, w: WreckPose) -> vec3<f32> {
    let r1 = hash11(seed * 173.3 + 1.7);
    let r2 = hash11(seed * 311.9 + 5.3);
    let r3 = hash11(seed * 97.1 + 9.1);
    let height = max(model.height, 1.0);
    let reach = max(model.bounds_radius, 1.0);
    let mobile = (model.icon & ICON_MOBILE) != 0u;
    let sank = w.sank;
    let wear = 1.0 - clamp(health, 0.0, 1.0);
    var p = pos;
    if part == PART_TURRET && !sank {
        let pivot = model.turret_pivot.xyz;
        // Thrown clear to one side or the other, never along the hull, where it would land in it.
        let away = select(-1.5707963, 1.5707963, r3 > 0.5) + (r2 - 0.5) * 1.1;
        let thrown = reach * (0.95 + r1 * 0.25);
        let land = vec3<f32>(vec2<f32>(cos(away), sin(away)) * thrown, height * 0.16);
        // The arc's rise over the straight line, and the time it takes under a heavy,
        // game-scale gravity so it reads as blown off, not floated off.
        let apex = height * 0.8 + reach * 0.3;
        let flight = clamp(2.0 * sqrt(2.0 * apex / 30.0), 0.5, 2.2);
        let s = clamp(age / flight, 0.0, 1.0);
        // Spin at a steady rate through the flight; one in three turns a full somersault
        // on the way, so it lands the same way up either way.
        let flip = select(0.0, 6.2831853, r2 > 0.66);
        var q = rot_z(p - pivot, mix(yaw, (r1 - 0.5) * 5.0, s));
        q = rot_x(q, (0.3 + r2 * 0.45 + flip) * s);
        q = rot_y(q, (r3 - 0.5) * 0.5 * s);
        var at = mix(pivot, land, s);
        at.z += 4.0 * apex * s * (1.0 - s);
        // One hop where it hits, a tenth of the arc's height.
        let after = age - flight;
        let hop = flight * 0.3;
        if after > 0.0 && after < hop {
            let h = after / hop;
            at.z += apex * 0.1 * 4.0 * h * (1.0 - h);
        }
        p = q + at;
        p.z = max(p.z, 0.02);
        return p;
    }
    // Out of the sky or on a blast it crumples at once; on the seabed it slumps slowly,
    // on from the pose it went down in.
    let settle = smoothstep(0.0, select(0.35, 2.5, sank), age);
    let twist = 1.0 + 1.5 * wear;
    // Crumple: a smooth field of the position, nothing at the ground and most at the top.
    let amp = clamp(height * 0.11, 0.15, 1.4) * twist;
    let at = (p.xy + vec2<f32>(p.z * 0.61, p.z * 0.37)) / (reach * 1.7) + vec2<f32>(seed * 3.1, seed * 7.7);
    let field = textureSampleLevel(noise_map, repeat_sampler, at, 2.0);
    // Dents: the same at a finer scale, so broad panels are buckled, not only bent.
    let dent = textureSampleLevel(noise_map, repeat_sampler, at * 3.7 + vec2<f32>(0.31, 0.77), 1.0);
    let rise = smoothstep(0.04, 0.55, p.z / height);
    p += vec3<f32>(field.b - 0.5, field.a - 0.5, -abs(field.b - field.a)) * 2.4 * amp * rise * settle;
    p += vec3<f32>(dent.a - 0.5, dent.b - 0.5, -abs(dent.a - 0.5)) * 1.1 * amp * rise * settle;
    // Twisted along its length and bent (hogged or sagged), more for a long hull.
    let along = clamp(p.x / reach, -1.5, 1.5);
    let long = smoothstep(4.0, 30.0, reach);
    let turn = (r1 - 0.5) * (0.25 + 0.3 * long) * along * twist * settle;
    p = vec3<f32>(p.x, p.y * cos(turn) - p.z * sin(turn), p.y * sin(turn) + p.z * cos(turn));
    p.z += (r2 - 0.5) * 0.12 * reach * long * (along * along - 0.35) * twist * settle;
    // Caved in around where it was hit.
    let hit = vec2<f32>(r1 - 0.5, r2 - 0.5) * reach * 0.9;
    let d = distance(p.xy, hit) / (reach * 0.5);
    p.z *= 1.0 - 0.6 * exp(-d * d) * settle;
    // What is left of a hull worn away is flattened toward the ground.
    p.z *= 1.0 - 0.35 * wear;
    // A section's torn ends are crushed in toward the hull's axis.
    if w.count > 1u {
        let span = min(w.hi, reach) - max(w.lo, -reach);
        let band = min(reach * 0.12, span * 0.2);
        let edge = min(abs(pos.x - w.lo), abs(pos.x - w.hi));
        let crush = (1.0 - smoothstep(0.0, band, edge)) * settle;
        p = vec3<f32>(p.x, p.y * (1.0 - 0.25 * crush), p.z * (1.0 - 0.2 * crush));
    }
    if part == PART_LOCOMOTION {
        // Tracks thrown and splayed, legs folded.
        p = vec3<f32>(p.x, p.y * mix(1.0, 1.07, settle), p.z * mix(1.0, 0.82, settle));
    }
    if mobile {
        p = rot_y(rot_x(p, (r2 - 0.5) * 0.2 * settle), (r3 - 0.5) * 0.14 * settle);
        // A ship's keel lies in the silt: nothing to flatten against.
        let naval = (model.icon & ICON_NAVAL) != 0u;
        let sag = height * 0.03 * settle;
        p.z = select(max(p.z - sag, 0.0), p.z - sag, naval || w.posed);
    }
    return p;
}

// Whether a point of the unwarped model (`local`) belongs to the section that keeps
// `lo`..`hi` along x. The cut is ragged, torn rather than sawn; neighbouring sections
// share it, so between them every point is drawn exactly once.
fn wreck_keeps(local: vec3<f32>, lo: f32, hi: f32, reach: f32) -> bool {
    let jag = (value_noise2(local.yz + vec2<f32>(3.7, 1.3), max(reach * 0.12, 0.6)) - 0.5) * reach * 0.22
        + (value_noise2(local.zy, max(reach * 0.035, 0.25)) - 0.5) * reach * 0.06;
    let x = local.x + jag;
    return x >= lo && x < hi;
}

// Where a wreck is worn away from the top as its mass goes (reclaimed or blasted),
// raggedly: `height` is the point's share of the model's height, `health` of the mass
// that is left.
fn wreck_worn(local: vec3<f32>, height: f32, health: f32, reach: f32) -> bool {
    let wear = 1.0 - clamp(health, 0.0, 1.0);
    let ragged = (value_noise2(local.xy + vec2<f32>(local.z * 0.7, local.z * 0.4), max(reach * 0.15, 0.8)) - 0.5) * 0.7 * wear;
    return height + ragged > health * 0.75 + 0.25;
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

// Burnt out: charred, matte, dead emitters. Soot, scorched paint and bare burnt steel,
// from a field of the model position, so two faces lying in the same plane are shaded
// alike and cannot flicker against each other. A hull's inside is soot-black.
fn wreck_material(m_in: Pbr, local: vec3<f32>, seed: f32, inner: bool) -> Pbr {
    var m = m_in;
    let burn = wreck_burn(local, seed);
    let paint = m.albedo * 0.1;
    let steel = vec3<f32>(0.075, 0.052, 0.04) * (0.6 + burn.y);
    m.albedo = mix(vec3<f32>(0.015, 0.014, 0.013), mix(paint, steel, smoothstep(0.45, 0.7, burn.y)), smoothstep(0.3, 0.75, burn.x));
    // A rotated cut of the plate map, so armour seams still read without marching.
    let plate_uv = vec2<f32>(local.x * 0.07 + local.y * 0.04, -local.x * 0.04 + local.z * 0.08) + vec2<f32>(seed * 2.3, seed * 1.1);
    let plate = textureSample(panel_map, repeat_sampler, plate_uv);
    m.albedo *= 0.6 + plate.b * 0.4;
    m.metallic = 0.3 * smoothstep(0.5, 0.7, burn.y);
    m.roughness = 0.93;
    m.emissive = vec3<f32>(0.0);
    if inner {
        m.albedo = vec3<f32>(0.008, 0.0075, 0.007) * (0.6 + burn.x);
        m.metallic = 0.0;
        m.roughness = 1.0;
    }
    return m;
}
