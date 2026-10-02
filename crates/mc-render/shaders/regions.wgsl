// A map's regions (mc_data::regions): stretches of it with a climate and a weather of
// their own, parted by climate walls. Where the walls run is in `Atmosphere` (`walls`,
// `wall_sides`, `regions`), which a shader reads as `atmos`: the scene set's
// (bindings.wgsl) or the weather simulation's own (clouds_sim.wgsl). build.rs inserts
// this file with bindings.wgsl, and into shaders that contain the line `//!use regions`.
//
// mc_data's `Walls::probe` and `Walls::weights` do the same sums on the CPU.

// Whether the map has regions. Without them all of it is region 0, and nothing here
// need be asked.
fn has_regions() -> bool {
    return atmos.regions.x > 0.5;
}

// Where a point lies by wall segment `i`: x metres from it, y how far along it its
// nearest point lies (0 at its first end to 1 at its other; at or past those the
// nearest point is that end).
fn wall_reach(i: u32, xy: vec2<f32>) -> vec2<f32> {
    let a = atmos.walls[i].xy;
    let b = atmos.walls[i].zw;
    let e = b - a;
    let t = dot(xy - a, e) / dot(e, e);
    // Past an end the nearest point is that end as it was given, so two segments
    // that share it measure the very same distance.
    var near = a + e * t;
    if t <= 0.0 {
        near = a;
    } else if t >= 1.0 {
        near = b;
    }
    return vec2<f32>(distance(xy, near), t);
}

// Metres from a point to wall segment `i`'s line carried on past its ends: positive
// on the left hand of the segment, walking from its first end.
fn wall_off_line(i: u32, xy: vec2<f32>) -> f32 {
    let a = atmos.walls[i].xy;
    let e = atmos.walls[i].zw - a;
    let p = xy - a;
    return (e.x * p.y - e.y * p.x) * atmos.wall_sides[i].w;
}

// Where a point lies among the walls.
struct RegionProbe {
    // The region it is in.
    region: u32,
    // Metres to the nearest wall.
    wall: f32,
    // Metres along that wall, from its first point, of the wall's nearest point.
    along: f32,
}

// The region on `xy`'s side of the nearest wall segment, how far that wall is and how
// far along it. Where the nearest point of several segments is the one end they share
// (a corner, or walls meeting), the segment whose line the point is furthest from
// decides; a segment the point lies beside counts as nearer than another's end within
// `REGIONS_TIE_M` (`Walls::probe` says why). A point on a wall is on its right.
// Only where `has_regions()`.
fn region_probe(xy: vec2<f32>) -> RegionProbe {
    var wall = 1.0e9;
    var hit = 0u;
    var hit_t = 0.0;
    var end = true;
    let count = u32(atmos.regions.x);
    for (var i = 0u; i < count; i++) {
        let reach = wall_reach(i, xy);
        let at_end = reach.y <= 0.0 || reach.y >= 1.0;
        var margin = 0.0;
        if at_end && !end {
            margin = -REGIONS_TIE_M;
        } else if !at_end && end {
            margin = REGIONS_TIE_M;
        }
        var nearer = reach.x < wall + margin;
        if !nearer && at_end && end && reach.x == wall {
            // The end the two share is the nearest point of both.
            nearer = abs(wall_off_line(i, xy)) > abs(wall_off_line(hit, xy));
        }
        if nearer {
            wall = reach.x;
            hit = i;
            hit_t = reach.y;
            end = at_end;
        }
    }
    let side = atmos.wall_sides[hit];
    var out: RegionProbe;
    out.region = u32(select(side.y, side.x, wall_off_line(hit, xy) > 0.0));
    out.wall = wall;
    out.along = side.z + clamp(hit_t, 0.0, 1.0) / side.w;
    return out;
}

// How much of each region's look a point takes.
struct RegionShares {
    // The region the point is in, and metres to the nearest wall.
    region: u32,
    wall: f32,
    // Whether the point is near enough a wall to take some of another region's look.
    // If not it is all `region`'s, and `w` is not filled in (`region_share`).
    mixed: bool,
    // Each region's share, summing to 1.
    w: array<f32, REGIONS_MAX>,
}

// The shares at `xy` where regions hand over within `half` metres either side of a
// wall: a region's weight falls from 1 to 0 as the point goes from `half` metres
// inside it to `half` outside it (to the nearest wall the region lies along), and the
// weights are scaled to sum to 1, so three regions or more meet smoothly where walls
// do. `half` metres or more from every wall the point is all its own region's, exactly.
// Only where `has_regions()`.
fn region_shares(xy: vec2<f32>, half: f32) -> RegionShares {
    var out: RegionShares;
    let probe = region_probe(xy);
    out.region = probe.region;
    out.wall = probe.wall;
    out.mixed = false;
    if probe.wall >= half {
        return out;
    }
    // Metres outside each region: to the nearest wall it lies along.
    var outside: array<f32, REGIONS_MAX>;
    for (var r = 0u; r < REGIONS_MAX; r++) {
        outside[r] = 1.0e9;
    }
    let count = u32(atmos.regions.x);
    for (var i = 0u; i < count; i++) {
        let d = wall_reach(i, xy).x;
        let side = atmos.wall_sides[i];
        outside[u32(side.x)] = min(outside[u32(side.x)], d);
        outside[u32(side.y)] = min(outside[u32(side.y)], d);
    }
    outside[probe.region] = -probe.wall;
    var sum = 0.0;
    let regions = u32(atmos.regions.y);
    for (var r = 0u; r < regions; r++) {
        out.w[r] = 1.0 - smoothstep(-half, half, outside[r]);
        sum += out.w[r];
    }
    for (var r = 0u; r < regions; r++) {
        out.w[r] /= sum;
    }
    out.mixed = true;
    return out;
}

// Region `r`'s share in `s`.
fn region_share(s: RegionShares, r: u32) -> f32 {
    if s.mixed {
        return s.w[r];
    }
    return select(0.0, 1.0, r == s.region);
}

// The weather's set values over a point.
struct SkyValues {
    // How much of the sky the air fills (`Atmosphere::layer.w`).
    cover: f32,
    // How towering the clouds are (`shape.x`), and how readily they rain (`shape.z`).
    towering: f32,
    rain: f32,
    // How big the cloud masses are (`shape.y`): the point's own region's, never a mix
    // (noise whose size varies from place to place smears).
    scale: f32,
    // A storm's top above the cloud floor (`layer.z`).
    storm_top: f32,
}

// The weather's values where the regions' shares are `s`: each region's own
// (`Atmosphere::region_sky`), handing over across a wall.
fn sky_from(s: RegionShares) -> SkyValues {
    let own = atmos.region_sky[s.region];
    var v = own;
    if s.mixed {
        v = vec4<f32>(0.0);
        let regions = u32(atmos.regions.y);
        for (var r = 0u; r < regions; r++) {
            if s.w[r] > 0.0 {
                v += atmos.region_sky[r] * s.w[r];
            }
        }
    }
    // The storm top as sky.rs sets `layer.z`.
    return SkyValues(v.x, v.y, v.w, own.z, atmos.layer.x + 2600.0 + 4200.0 * v.y);
}

// The weather's set values over `xy`: the map's own, or on a map with regions its
// region's, handing over within `REGIONS_SKY_BLEND_M` of a wall.
fn sky_at(xy: vec2<f32>) -> SkyValues {
    if !has_regions() {
        return SkyValues(atmos.layer.w, atmos.shape.x, atmos.shape.z, atmos.shape.y, atmos.layer.z);
    }
    return sky_from(region_shares(xy, REGIONS_SKY_BLEND_M));
}

// Each region has an air mass of its own: the one field (`cloud_climate`) read this
// far off per region, so the cloud masses either side of a wall do not line up.
const REGION_AIR_STEP: vec2<f32> = vec2<f32>(41357.0, -28411.0);

// Region `r`'s air mass over `xy`: its cover and its convection.
fn region_air(r: u32, xy: vec2<f32>) -> vec2<f32> {
    let sky = atmos.region_sky[r];
    return cloud_climate(xy + f32(r) * REGION_AIR_STEP, atmos.wind.xy, sky.x, sky.z);
}

// The air mass where the regions' shares are `s`: each region's own, mixed across a wall.
fn air_mass_from(s: RegionShares, xy: vec2<f32>) -> vec2<f32> {
    if !s.mixed {
        return region_air(s.region, xy);
    }
    var air = vec2<f32>(0.0);
    let regions = u32(atmos.regions.y);
    for (var r = 0u; r < regions; r++) {
        if s.w[r] > 0.0 {
            air += region_air(r, xy) * s.w[r];
        }
    }
    return air;
}

// The weather the air mass would have by itself over `xy` (`cloud_climate`): the
// map's own, or its region's.
fn air_mass_at(xy: vec2<f32>) -> vec2<f32> {
    if !has_regions() {
        return cloud_climate(xy, atmos.wind.xy, atmos.layer.w, atmos.shape.y);
    }
    return air_mass_from(region_shares(xy, REGIONS_SKY_BLEND_M), xy);
}
