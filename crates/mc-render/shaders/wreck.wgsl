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

// Where a wreck is worn away from the top as blasts (or the years, for a map's own)
// take its mass, raggedly: `height` is the point's share of the model's height, `health`
// of the mass left. Reclaim does not wear it away (`wreck_reclaim`).
fn wreck_worn(local: vec3<f32>, height: f32, health: f32, reach: f32) -> bool {
    let wear = 1.0 - clamp(health, 0.0, 1.0);
    let ragged = (value_noise2(local.xy + vec2<f32>(local.z * 0.7, local.z * 0.4), max(reach * 0.15, 0.8)) - 0.5) * 0.7 * wear;
    return height + ragged > health * 0.75 + 0.25;
}

// ---- the burnt-out surface --------------------------------------------------
//
// Nothing here tiles. What a wreck is made of comes from three places, so no two
// stretches of a big hull and no two wrecks look alike:
// - the face's own plates (`surf_courses` on the face frame): each plate burnt its
//   own way, charred in its paint, burnt to bare steel, or gone, showing the dark
//   inside; buckled, with its seams and the edges of the face scraped bright;
// - a field of the model position (`surf_fbm3`, hashed, never a texture), at scales
//   from the hull's size down to a hand's width: where the fire took hold, heat
//   tint on the bare steel, rust, ash settled on what faces up, streaks run down
//   what stands up, grime low down;
// - a tone of its own for every face, so the broad flat facets of a big hull break up.
// Faces lying in the same plane share the model-position field, so they cannot
// flicker against each other.

const WRECK_SOOT: vec3<f32> = vec3<f32>(0.011, 0.0105, 0.01);
const WRECK_STEEL: vec3<f32> = vec3<f32>(0.06, 0.05, 0.043);
const WRECK_STRAW: vec3<f32> = vec3<f32>(0.07, 0.05, 0.03);
const WRECK_BLUED: vec3<f32> = vec3<f32>(0.036, 0.034, 0.042);
const WRECK_RUST: vec3<f32> = vec3<f32>(0.075, 0.038, 0.02);
const WRECK_ASH: vec3<f32> = vec3<f32>(0.12, 0.112, 0.102);
const WRECK_DIRT: vec3<f32> = vec3<f32>(0.08, 0.066, 0.05);
const WRECK_SCRAPE: vec3<f32> = vec3<f32>(0.2, 0.19, 0.18);

struct WreckSurface {
    m: Pbr,
    // Slope of the relief along the face's s and t, as `Surface::slope`.
    slope: vec2<f32>,
    // The deep relief's rise, metres, one screen pixel right and one down (`wreck_depth`).
    bump: vec2<f32>,
}

// The deep relief of burnt, battered metal, metres out of the surface at a model
// position: broad dents, crumple creases (ridged, so they fold rather than roll),
// blistered char and corrosion pits. Scaled to the hull but bounded, so a tank is
// as battered up close as a battleship is from the game's camera. Octaves finer
// than a few pixels fade out (`surf_fbm3`), so it never sparkles.
// Turned off the noise lattice's axes, each octave its own way: value noise's grid
// shows in relief as boxy creases square to the hull.
const WRECK_TURN_A: mat3x3<f32> = mat3x3<f32>(
    vec3<f32>(0.64, 0.6, -0.48), vec3<f32>(-0.77, 0.48, -0.42), vec3<f32>(-0.02, 0.64, 0.77));
const WRECK_TURN_B: mat3x3<f32> = mat3x3<f32>(
    vec3<f32>(0.36, -0.8, 0.48), vec3<f32>(0.93, 0.29, -0.22), vec3<f32>(0.04, 0.53, 0.85));

fn wreck_depth(p: vec3<f32>, size: f32, fw: f32) -> f32 {
    let a = WRECK_TURN_A * p;
    let b = WRECK_TURN_B * p;
    let dent_cell = clamp(size * 0.1, 0.9, 9.0);
    let dent = surf_fbm3(a + vec3<f32>(13.1, 7.7, 3.3), dent_cell, fw) * dent_cell * 0.22;
    let fold_cell = clamp(size * 0.03, 0.35, 2.4);
    let fold = (0.3 - abs(surf_fbm3(b + vec3<f32>(41.3, 17.9, 5.1), fold_cell, fw))) * fold_cell * 0.3;
    let blister = surf_fbm3(b + vec3<f32>(3.9, 29.3, 61.7), 0.3, fw) * 0.035;
    let pit = -smoothstep(0.08, 0.3, surf_fbm3(a + vec3<f32>(77.7, 51.3, 2.9), 0.12, fw)) * 0.02;
    return dent + fold + blister + pit;
}

// One plate of a face, burnt its own way: x its random, y distance inside its edge.
fn wreck_plate(i: SurfaceIn, st: vec2<f32>) -> vec2<f32> {
    let cell = surf_courses(i, st, vec2<f32>(i.scale * 1.5, i.scale));
    return vec2<f32>(cell.id, surf_edge(cell.p, cell.half));
}

// The plates' relief: the fitted plating the unit had (seams, rivets, hatches),
// each plate buckled out or in by its own amount across its width.
fn wreck_relief(i: SurfaceIn, st: vec2<f32>) -> f32 {
    let cell = surf_courses(i, st, vec2<f32>(i.scale * 1.5, i.scale));
    let q = cell.p / max(cell.half, vec2<f32>(1e-3));
    let buckle = (hash11(cell.id * 61.3 + 0.7) - 0.5) * (1.0 - dot(q, q) * 0.5)
        + (hash11(cell.id * 23.9 + 0.2) - 0.5) * q.x * q.y;
    return surf_relief_plates(i, st, i.scale * 0.035, i.scale * 0.012) + buckle * 1.6;
}

// `i` is the face as `surface_at` takes it; `up` the world normal's z; `paint` the
// material as the unit wore it; `inner` a hull's inside.
// `dl1`/`dl2` are the model position's steps to the next pixel right and down.
fn wreck_surface(paint: Pbr, i: SurfaceIn, up: f32, inner: bool, dl1: vec3<f32>, dl2: vec3<f32>) -> WreckSurface {
    var out: WreckSurface;
    out.slope = vec2<f32>(0.0);
    out.bump = vec2<f32>(0.0);
    var m = paint;
    m.emissive = vec3<f32>(0.0);
    let fw = max(i.px, 1e-4);
    let p = i.local + vec3<f32>(i.unit * 53.0, i.unit * 91.0, i.unit * 17.0);
    let size = clamp(i.reach, 2.0, 200.0);
    // Where the fire took hold, from the hull's size down: broad, patches, then flakes
    // and pitting at sizes fixed in metres, so a tank and a battleship are both close-grained.
    // Off the lattice's axes and warped, or the patches come out square along the hull.
    let pa = WRECK_TURN_A * p;
    let pb = WRECK_TURN_B * p;
    let broad_cell = clamp(size * 0.3, 2.0, 36.0);
    let warp = vec3<f32>(surf_fbm3(pb, broad_cell * 0.45, fw), surf_fbm3(pb + vec3<f32>(19.3, 5.1, 8.7), broad_cell * 0.45, fw), 0.0)
        * broad_cell * 0.9;
    let broad = surf_fbm3(pa + warp, broad_cell, fw);
    let mid = surf_fbm3(pb + warp * 0.6 + vec3<f32>(31.7, 12.9, 7.1), clamp(size * 0.09, 0.8, 8.0), fw);
    let flake = surf_fbm3(pa + warp * 0.2 + vec3<f32>(5.3, 44.1, 19.7), 1.4, fw);
    let pit = surf_fbm3(pb + vec3<f32>(71.1, 3.9, 27.3), 0.35, fw);
    // Stretched up the hull: what ran down and what the fire drew up.
    let run = surf_fbm3(vec3<f32>(p.x, p.y, p.z * 0.1) + vec3<f32>(9.1, 61.7, 0.0), clamp(size * 0.025, 0.3, 2.2), fw);
    // Deep relief, differenced one pixel each way, and how far down in it this point lies.
    let d0 = wreck_depth(p, size, fw);
    out.bump = vec2<f32>(wreck_depth(p + dl1, size, fw) - d0, wreck_depth(p + dl2, size, fw) - d0);
    let hollow = saturate(0.5 - d0 / max(clamp(size * 0.1, 0.9, 9.0) * 0.18, 0.05));

    if inner {
        m.albedo = WRECK_SOOT * (0.55 + 1.2 * saturate(0.5 + broad + pit));
        m.metallic = 0.0;
        m.roughness = 1.0;
        out.m = m;
        return out;
    }

    // Each face its own tone.
    let face_tone = 0.86 + 0.28 * hash11(i.seed * 97.3 + i.unit * 13.1);
    // The face's plates, where it has a frame to lay them on.
    let framed = max(i.half.x, i.half.y) > 0.0;
    var plate = vec2<f32>(0.5, 1e3);
    if framed {
        plate = wreck_plate(i, i.st);
        let e = max(fw * 0.75, i.scale * 0.002);
        let h0 = wreck_relief(i, i.st);
        let depth = i.scale * 0.06;
        out.slope = vec2<f32>(wreck_relief(i, i.st + vec2<f32>(e, 0.0)) - h0, wreck_relief(i, i.st + vec2<f32>(0.0, e)) - h0) * (depth / e);
        out.slope *= 1.0 - smoothstep(i.scale * 0.1, i.scale * 0.3, fw);
    }
    let roll = hash11(plate.x * 53.1 + 0.3);
    let plate_tone = 0.9 + 0.2 * hash11(plate.x * 19.7 + 0.9);
    // Plates want room before they are lost or stripped: a sliver of a face stays as it is.
    let roomy = framed && min(i.half.x, i.half.y) > i.scale * 0.4;
    // Whole plates only lean the burn one way or the other: a plate flipped wholesale
    // reads as a grid of squares across a big hull. One is torn away only where the fire
    // was hottest.
    let gone = roomy && roll < 0.12 && broad + mid * 0.5 > 0.12;
    let stripped = select(0.0, 0.12, roomy && roll >= 0.12 && roll < 0.4);

    // Charred paint, down to soot where it burnt hardest.
    let lum = dot(paint.albedo, vec3<f32>(0.2126, 0.7152, 0.0722));
    let charred = mix(vec3<f32>(lum), paint.albedo, 0.3) * 0.09;
    let burn = saturate(0.58 + broad * 1.5 + mid * 0.9 + flake * 0.5 + pit * 0.4);
    var albedo = mix(charred, WRECK_SOOT, smoothstep(0.3, 0.75, burn));
    // Bare steel where the paint burnt off, temper-tinted straw to blue by the heat it took.
    // Ragged down to the flakes and pits, never a clean-edged patch.
    let bare = smoothstep(0.52, 0.8, 0.42 + mid * 0.8 + flake * 1.4 + pit * 0.9 + stripped);
    let heat = saturate(0.5 + broad * 3.0 + flake);
    let tint = mix(WRECK_STRAW, WRECK_BLUED, smoothstep(0.35, 0.75, heat));
    let steel = mix(WRECK_STEEL, tint, 0.4 + 0.3 * flake) * (0.6 + 0.7 * saturate(0.5 + pit * 2.0));
    albedo = mix(albedo, steel, bare);
    // Rust on the bare steel, and run down from it over what stands up.
    let steep = 1.0 - abs(up);
    let rust = smoothstep(0.08, 0.3, run + mid * 0.5) * (bare * 0.7 + steep * 0.3);
    albedo = mix(albedo, WRECK_RUST * (0.7 + 0.6 * saturate(0.5 + pit * 2.0)), rust * 0.75);
    // Soot drawn up the walls by the fire.
    albedo = mix(albedo, WRECK_SOOT, steep * smoothstep(0.02, 0.2, -run) * 0.7);
    // Ash settled on what faces the sky.
    let ash = smoothstep(0.55, 0.9, up) * smoothstep(0.0, 0.3, mid * 0.6 + flake * 0.6 + pit * 0.5 - broad * 0.4);
    albedo = mix(albedo, WRECK_ASH * (0.7 + 0.5 * saturate(0.5 + pit * 2.0)), ash * 0.35);
    // Grime and earth thrown up low down.
    let share = i.local.z / max(i.height, 1.0);
    let low = (1.0 - smoothstep(0.0, 0.22, share + flake * 0.15)) * (0.5 + 0.5 * steep);
    albedo = mix(albedo, WRECK_DIRT, low * 0.6);

    albedo *= face_tone * plate_tone;
    // Soot and shadow gathered in the dents and folds.
    albedo *= 1.0 - 0.55 * smoothstep(0.35, 0.95, hollow);
    // Burnt steel is dull: a sheen, not a mirror of the sky.
    var metallic = 0.3 * bare * (1.0 - rust);
    var roughness = mix(mix(0.96, 0.74, bare), 1.0, max(ash, rust * 0.8));

    if framed {
        // Seams gape and edges take the knocks: paint scraped back to bright steel along them.
        let seam = surf_band(plate.y, i.scale * 0.02, fw);
        let d_face = surf_face_edge(i, i.st);
        let scrape = (1.0 - smoothstep(0.0, i.scale * 0.06, d_face)) * smoothstep(0.1, 0.3, flake + pit * 0.8);
        albedo = mix(albedo, WRECK_SCRAPE * face_tone, scrape * 0.8);
        metallic = mix(metallic, 0.85, scrape);
        roughness = mix(roughness, 0.45, scrape);
        albedo *= 1.0 - 0.7 * seam;
        if gone {
            // The plate is gone: the dark inside shows, its torn edge lighter.
            let lip = 1.0 - smoothstep(0.0, i.scale * 0.06, plate.y);
            albedo = mix(WRECK_SOOT * 0.6, WRECK_STEEL * 0.8, lip);
            metallic = 0.2 * lip;
            roughness = 1.0;
            out.slope = vec2<f32>(0.0);
        }
    }

    m.albedo = albedo;
    m.metallic = metallic;
    m.roughness = roughness;
    out.m = m;
    return out;
}

// ---- reclaim taking a wreck apart -------------------------------------------
//
// Reclaim does not wear a wreck away: the hull stays whole, and the share reclaim has
// taken (`Entity::fx.z`) shows on it as the work, eaten in from the top down, raggedly.
// While a beam works it (`fx.w`, seconds since reclaim last took mass) what is taken is
// blackened metal run through with a net of glowing Materials red-orange, light pouring
// up it toward the beam, motes lifting off, and a white-hot edge crawling on where it
// eats in, the metal ahead of it heating. Once the beam stops the work cools over a few
// seconds to a dull ember that barely breathes: a dim red net and a banked rim, so a
// wreck left half taken looks it.
//
// The last of it going (renderer wreck_finish.rs): the sim has freed the wreck and a
// copy of its hull says how far it has gone as `unmade` past 1. The net runs over the
// whole hull and flares, then the metal burns away from the top down behind a white-hot
// edge (`wreck_going`).

const RECLAIM_RED: vec3<f32> = vec3<f32>(0.85, 0.05, 0.02);
const RECLAIM_WHITE: vec3<f32> = vec3<f32>(1.0, 0.84, 0.74);

struct WreckReclaim {
    // Share of the metal's albedo kept: stripped dark where it is taken.
    keep: f32,
    glow: vec3<f32>,
}

// `local` the unwarped model position, `height` its share of the model's height,
// `unmade` the share taken, `since` seconds since the work last went on, `px` a pixel
// in metres, `seed` the wreck's own random.
fn wreck_reclaim(local: vec3<f32>, height: f32, reach: f32, unmade: f32, since: f32, px: f32, seed: f32, time: f32) -> WreckReclaim {
    var out: WreckReclaim;
    out.keep = 1.0;
    out.glow = vec3<f32>(0.0);
    if unmade <= 0.0 {
        return out;
    }
    let finish = clamp(unmade - 1.0, 0.0, 1.0);
    let size = clamp(reach, 2.0, 200.0);
    let fw = max(px, 1e-4);
    let p = WRECK_TURN_B * (local + vec3<f32>(seed * 37.0, seed * 71.0, seed * 13.0));
    // The order it is taken in: the top first, the front ragged at the hull's scale and finer.
    let rag = surf_fbm3(p, clamp(size * 0.3, 1.5, 30.0), fw)
        + 0.45 * surf_fbm3(p + vec3<f32>(7.3, 2.9, 4.1), clamp(size * 0.07, 0.5, 7.0), fw);
    let key = (1.0 - clamp(height, 0.0, 1.0)) * 0.7 + 0.15 + rag * 0.55;
    // Going, the net spreads over the rest of the hull within its first tenth.
    let front = mix(0.1, 1.1, clamp(unmade, 0.0, 1.0)) + finish * 3.0;
    let inside = front - key;
    // The work on (1) or long cooled (0); a wreck left alone keeps a faint ember, dimming
    // a little more over its first minute or two.
    let on = 1.0 - smoothstep(0.3, 3.0, since);
    let ember = mix(1.0, 0.6, smoothstep(5.0, 120.0, since));
    let band = 0.015 + 0.03 * on;
    let taken = smoothstep(0.0, band * 0.6, inside);
    let materials = vec3<f32>(MASS_R, MASS_G, MASS_B);

    // The net through the taken metal: the zero lines of two noise fields, the finer
    // fainter, each fading out before it is finer than a few pixels.
    let cell = clamp(size * 0.11, 0.7, 9.0);
    let n1 = surf_fbm3(p + vec3<f32>(29.1, 3.3, 17.7), cell, fw);
    let n2 = surf_fbm3(WRECK_TURN_A * p + vec3<f32>(5.9, 41.3, 8.1), cell * 0.38, fw);
    let line1 = (1.0 - smoothstep(0.0, 0.03, abs(n1))) * surf_resolved(cell * 0.5, fw);
    let line2 = (1.0 - smoothstep(0.0, 0.025, abs(n2))) * surf_resolved(cell * 0.2, fw);
    let net = max(line1, line2 * 0.65);
    // A soft heat between the lines, so the taken metal reads from far off too.
    let wash = 0.5 + 0.5 * surf_fbm3(p + vec3<f32>(13.7, 9.1, 2.3), cell * 1.6, fw);

    // Working: light pours up the net toward the beam, in pulses broken by the field.
    let rise = clamp(size * 0.06, 0.5, 5.0);
    let pulse = pow(0.5 + 0.5 * sin((local.z / rise - time * 1.7 + n1 * 5.0) * 6.2831853), 6.0);
    let working = materials * (2.6 * net + 0.06 * wash * wash) + RECLAIM_WHITE * 5.0 * net * pulse;
    // Motes of material lifting off the taken metal.
    let mote_cell = clamp(size * 0.045, 0.4, 3.0);
    let q = vec3<f32>(local.x, local.y, local.z - time * 1.3 * mote_cell) / mote_cell;
    let id = floor(q);
    let at = vec3<f32>(surf_hash3(id + 0.31), surf_hash3(id + 7.17), surf_hash3(id + 3.71)) * 0.6 + 0.2;
    let mote = (1.0 - smoothstep(0.0, 0.14, distance(fract(q), at)))
        * step(0.8, surf_hash3(id + 11.3)) * surf_resolved(mote_cell * 0.3, fw);
    // Left: a dull red net that breathes slowly, barely warm between.
    let breath = 0.8 + 0.2 * sin(time * 0.7 + seed * 40.0 + n1 * 3.0);
    let left = (RECLAIM_RED * 0.3 * net + RECLAIM_RED * 0.01 * wash) * breath * ember;
    out.glow = mix(left, working + RECLAIM_WHITE * 3.0 * mote, on) * taken;

    // The edge where it eats in: white-hot at the front and flickering while it works,
    // a banked dull rim once it stops; the metal just ahead heating.
    let edge = exp(-pow(inside / band, 2.0));
    let core = exp(-pow(inside / (band * 0.35), 2.0));
    let flicker = 0.75 + 0.25 * sin(time * 9.0 + n2 * 20.0 + seed * 17.0);
    let hot = (materials * 2.4 * edge + RECLAIM_WHITE * 5.0 * core) * flicker
        + RECLAIM_RED * 0.8 * smoothstep(-band * 4.0, 0.0, inside) * (1.0 - taken);
    let banked = (RECLAIM_RED * 0.12 * edge + RECLAIM_RED * 0.25 * core) * breath * ember;
    out.glow += mix(banked, hot, on);
    if finish > 0.0 {
        // Going: the whole hull flares as the net takes it, and a white-hot edge burns
        // down it with the metal just under it heating.
        let flare = 1.0 + 1.5 * smoothstep(0.0, 0.15, finish) * (1.0 - smoothstep(0.5, 1.0, finish));
        out.glow *= flare;
        let g = wreck_going(local, height, reach, unmade, seed);
        let going_edge = exp(-pow(g / 0.04, 2.0));
        let heating = exp(-g / 0.18) * step(0.0, g);
        out.glow += RECLAIM_WHITE * 9.0 * going_edge * flicker + materials * 2.5 * heating;
    }
    // Stripped to the blackened frame where it is taken.
    out.keep = mix(1.0, 0.25, taken);
    return out;
}

// The last of a wreck going (`unmade` past 1: 1 + how far, 0 to 1): how far under the
// edge burning down the hull from the top this point lies, raggedly; under zero where
// it is gone. Over the hull's first fifth the edge is still above it, and by the end it
// is under its keel (wreck_finish.rs `front` follows it). Cheap, as the depth pre-pass
// and the shadow pass cut by it too.
fn wreck_going(local: vec3<f32>, height: f32, reach: f32, unmade: f32, seed: f32) -> f32 {
    let finish = unmade - 1.0;
    if finish <= 0.0 {
        return 1.0;
    }
    let rag = value_noise2(local.xy + vec2<f32>(local.z * 0.6 + seed * 31.0, local.z * 0.45 + seed * 17.0), max(reach * 0.12, 0.6)) - 0.5;
    let key = 1.0 - clamp(height, 0.0, 1.0) + rag * 0.45;
    let front = mix(-0.3, 1.3, smoothstep(0.2, 1.0, finish));
    return key - front;
}
