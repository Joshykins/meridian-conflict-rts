//! Structure foundation plans. A pad is the building's plan, poured a
//! little past the walls — not a hashed rectangle and not the build cell.

use super::{part, rig, MeshLod, MeshVertex};

/// Texels along one side of a pad SDF layer.
pub const PAD_FOOTPRINT_RES: u32 = 128;
/// Metres encoded around the form edge (`0.5` in the R8 is the edge).
pub const PAD_SDF_RANGE: f32 = 8.0;
/// Lot UV covered by the texture, a little past the pad mesh so the edge
/// can anti-alias. Must match `PAD_FOOTPRINT_REACH` in `common.wgsl`.
pub const PAD_FOOTPRINT_REACH: f32 = 1.12;
/// How far past the hull the pour runs, metres.
const POUR_MARGIN_M: f32 = 0.55;
/// Hull triangles that never dip below this are roofs, cranes and masts.
/// Turrets sit higher and are included anyway.
const GROUND_Z: f32 = 3.5;
/// Drop wires and posts: their XY projection is a spike, not a slab.
const MIN_XY_AREA: f32 = 0.12;

#[derive(Clone, Copy)]
enum Band {
    Hull,
    Turret,
}

/// R8 UNORM SDF of `mesh` in lot UV. `half_m` is the pad quad's half-extent
/// (the build-grid lot). `0.5` is the form edge, higher is inside.
pub fn bake_pad_footprint(mesh: &MeshLod, half_m: f32) -> Vec<u8> {
    let n = PAD_FOOTPRINT_RES as usize;
    let mut out = vec![0u8; n * n];
    if !(half_m > 0.5) || mesh.indices.len() < 3 {
        return out;
    }
    let hull = rasterize(mesh, half_m, Band::Hull);
    let turret = rasterize(mesh, half_m, Band::Turret);
    let hull_s = occ_stats(&hull, n);
    let turret_s = occ_stats(&turret, n);
    let occ = if turret_s.count > 0 && square_plinth(&hull, &hull_s, n, &turret_s) {
        // A square bunker under a plus-shaped house was pouring the cell.
        turret
    } else {
        union(&hull, &turret)
    };
    if !occ.iter().any(|&p| p) {
        return out;
    }
    let sd = signed_distance(&occ, n);
    let texel_m = (2.0 * half_m * PAD_FOOTPRINT_REACH) / n as f32;
    for i in 0..n * n {
        let metres = sd[i] * texel_m - POUR_MARGIN_M;
        let enc = 0.5 - metres / (2.0 * PAD_SDF_RANGE);
        out[i] = (enc.clamp(0.0, 1.0) * 255.0) as u8;
    }
    out
}

/// Half-extent, metres, of a hull-plan atlas layer. [-1, 1] in plan UV is
/// this square around the origin, a little past the mesh so the SDF can
/// anti-alias.
pub fn hull_plan_half(mesh: &MeshLod) -> f32 {
    mesh.vertices
        .iter()
        .map(|v| v.pos[0].abs().max(v.pos[1].abs()))
        .fold(1.0f32, f32::max)
}

/// RGBA8 hull plan of `mesh`. R is a signed-distance (same encoding as a
/// pad, no pour). G/B are the local-Z span of the hull in that texel,
/// encoded by `height`. A is unused. `half_m` is [`hull_plan_half`].
pub fn bake_hull_plan(mesh: &MeshLod, half_m: f32, height: f32) -> Vec<u8> {
    let n = PAD_FOOTPRINT_RES as usize;
    let mut out = vec![0u8; n * n * 4];
    if !(half_m > 0.5) || !(height > 0.0) || mesh.indices.len() < 3 {
        return out;
    }
    let mut occ = vec![false; n * n];
    let mut zmin = vec![f32::INFINITY; n * n];
    let mut zmax = vec![f32::NEG_INFINITY; n * n];
    for tri in mesh.indices.chunks_exact(3) {
        let [a, b, c] = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
        let va = &mesh.vertices[a];
        let vb = &mesh.vertices[b];
        let vc = &mesh.vertices[c];
        if !include_hull_triangle(va, vb, vc) {
            continue;
        }
        fill_hull_triangle(
            &mut occ, &mut zmin, &mut zmax, n, half_m, va.pos, vb.pos, vc.pos,
        );
    }
    if !occ.iter().any(|&p| p) {
        return out;
    }
    let sd = signed_distance(&occ, n);
    let texel_m = (2.0 * half_m * PAD_FOOTPRINT_REACH) / n as f32;
    let z_scale = height.max(0.5);
    for i in 0..n * n {
        let metres = sd[i] * texel_m;
        let enc = 0.5 - metres / (2.0 * PAD_SDF_RANGE);
        out[i * 4] = (enc.clamp(0.0, 1.0) * 255.0) as u8;
        if occ[i] {
            out[i * 4 + 1] = ((zmin[i] / z_scale).clamp(0.0, 1.0) * 255.0) as u8;
            out[i * 4 + 2] = ((zmax[i] / z_scale).clamp(0.0, 1.0) * 255.0) as u8;
            out[i * 4 + 3] = 255;
        }
    }
    out
}

/// Signed distance and local-Z span at plan UV `uv` ([-1, 1] is `half_m`).
/// Negative distance is inside the extruded column. Empty texels have a
/// large positive distance and a zero span.
pub fn hull_plan_at(tex: &[u8], uv: [f32; 2], height: f32) -> (f32, f32, f32) {
    let n = PAD_FOOTPRINT_RES as usize;
    if tex.len() != n * n * 4 {
        return (PAD_SDF_RANGE, 0.0, 0.0);
    }
    let to_texel = |u: f32| ((u / PAD_FOOTPRINT_REACH) * 0.5 + 0.5) * n as f32;
    let x = to_texel(uv[0]).clamp(0.0, (n - 1) as f32) as usize;
    let y = to_texel(uv[1]).clamp(0.0, (n - 1) as f32) as usize;
    let i = (y * n + x) * 4;
    let sd = (0.5 - tex[i] as f32 / 255.0) * 2.0 * PAD_SDF_RANGE;
    let z0 = tex[i + 1] as f32 / 255.0 * height;
    let z1 = tex[i + 2] as f32 / 255.0 * height;
    (sd, z0, z1)
}

/// Distance from `local` (model metres) to the baked hull column. Negative
/// is inside the mesh's vertical span at that XY.
pub fn hull_plan_sd(tex: &[u8], local: [f32; 3], half_m: f32, height: f32) -> f32 {
    if !(half_m > 0.0) {
        return PAD_SDF_RANGE;
    }
    let uv = [local[0] / half_m, local[1] / half_m];
    if uv[0].abs() > PAD_FOOTPRINT_REACH || uv[1].abs() > PAD_FOOTPRINT_REACH {
        return PAD_SDF_RANGE;
    }
    let (sd_xy, z0, z1) = hull_plan_at(tex, uv, height);
    column_sd(sd_xy, local[2], z0, z1)
}

fn column_sd(sd_xy: f32, z: f32, z0: f32, z1: f32) -> f32 {
    let dz = if z < z0 {
        z0 - z
    } else if z > z1 {
        z - z1
    } else {
        0.0
    };
    if sd_xy > 0.0 {
        (sd_xy * sd_xy + dz * dz).sqrt()
    } else if dz > 0.0 {
        dz
    } else {
        sd_xy
    }
}

fn include_hull_triangle(a: &MeshVertex, b: &MeshVertex, c: &MeshVertex) -> bool {
    for v in [a, b, c] {
        if v.part == part::SPINNER || v.rig & rig::UPGRADE != 0 {
            return false;
        }
    }
    let ab = [
        b.pos[0] - a.pos[0],
        b.pos[1] - a.pos[1],
        b.pos[2] - a.pos[2],
    ];
    let ac = [
        c.pos[0] - a.pos[0],
        c.pos[1] - a.pos[1],
        c.pos[2] - a.pos[2],
    ];
    let nx = ab[1] * ac[2] - ab[2] * ac[1];
    let ny = ab[2] * ac[0] - ab[0] * ac[2];
    let nz = ab[0] * ac[1] - ab[1] * ac[0];
    (nx * nx + ny * ny + nz * nz).sqrt() * 0.5 >= 0.02
}

/// How far a thin plate still counts as meeting the glass, metres.
const HULL_Z_PAD: f32 = 0.22;

fn fill_hull_triangle(
    occ: &mut [bool],
    zmin: &mut [f32],
    zmax: &mut [f32],
    n: usize,
    half_m: f32,
    a: [f32; 3],
    b: [f32; 3],
    c: [f32; 3],
) {
    let to = |p: [f32; 3]| {
        let u = ((p[0] / half_m / PAD_FOOTPRINT_REACH) * 0.5 + 0.5) * n as f32;
        let v = ((p[1] / half_m / PAD_FOOTPRINT_REACH) * 0.5 + 0.5) * n as f32;
        [u, v]
    };
    let pa = to(a);
    let pb = to(b);
    let pc = to(c);
    let z0 = a[2].min(b[2]).min(c[2]) - HULL_Z_PAD;
    let z1 = a[2].max(b[2]).max(c[2]) + HULL_Z_PAD;
    let min_x = pa[0].min(pb[0]).min(pc[0]).floor().max(0.0) as usize;
    let max_x = pa[0].max(pb[0]).max(pc[0]).ceil().min((n - 1) as f32) as usize;
    let min_y = pa[1].min(pb[1]).min(pc[1]).floor().max(0.0) as usize;
    let max_y = pa[1].max(pb[1]).max(pc[1]).ceil().min((n - 1) as f32) as usize;
    if min_x > max_x || min_y > max_y {
        return;
    }
    let area = (pb[0] - pa[0]) * (pc[1] - pa[1]) - (pb[1] - pa[1]) * (pc[0] - pa[0]);
    // One texel of slop so a vertical wall still has a slice, without
    // fattening the plan into the collision disc.
    let slop = 0.75;
    let inv = if area.abs() < 1e-8 { 0.0 } else { 1.0 / area };
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let p = [x as f32 + 0.5, y as f32 + 0.5];
            let hit = if inv == 0.0 {
                edge_near(p, pa, pb, slop)
                    || edge_near(p, pb, pc, slop)
                    || edge_near(p, pc, pa, slop)
            } else {
                let w0 = ((pb[0] - p[0]) * (pc[1] - p[1]) - (pb[1] - p[1]) * (pc[0] - p[0])) * inv;
                let w1 = ((pc[0] - p[0]) * (pa[1] - p[1]) - (pc[1] - p[1]) * (pa[0] - p[0])) * inv;
                let w2 = 1.0 - w0 - w1;
                w0 >= -1e-3 && w1 >= -1e-3 && w2 >= -1e-3
                    || edge_near(p, pa, pb, slop)
                    || edge_near(p, pb, pc, slop)
                    || edge_near(p, pc, pa, slop)
            };
            if hit {
                let i = y * n + x;
                occ[i] = true;
                zmin[i] = zmin[i].min(z0);
                zmax[i] = zmax[i].max(z1);
            }
        }
    }
}

fn edge_near(p: [f32; 2], a: [f32; 2], b: [f32; 2], slop: f32) -> bool {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let ap = [p[0] - a[0], p[1] - a[1]];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1];
    if len2 < 1e-8 {
        return ap[0] * ap[0] + ap[1] * ap[1] <= slop * slop;
    }
    let t = ((ap[0] * ab[0] + ap[1] * ab[1]) / len2).clamp(0.0, 1.0);
    let dx = ap[0] - ab[0] * t;
    let dy = ap[1] - ab[1] * t;
    dx * dx + dy * dy <= slop * slop
}

/// Signed distance in metres at lot UV `uv` ([-1, 1] is the lot). Negative
/// is inside the pour.
pub fn pad_sdf_at(tex: &[u8], uv: [f32; 2]) -> f32 {
    let n = PAD_FOOTPRINT_RES as usize;
    if tex.len() != n * n {
        return PAD_SDF_RANGE;
    }
    let to_texel = |u: f32| ((u / PAD_FOOTPRINT_REACH) * 0.5 + 0.5) * n as f32;
    let x = to_texel(uv[0]).clamp(0.0, (n - 1) as f32);
    let y = to_texel(uv[1]).clamp(0.0, (n - 1) as f32);
    let raw = tex[y as usize * n + x as usize] as f32 / 255.0;
    (0.5 - raw) * 2.0 * PAD_SDF_RANGE
}

fn rasterize(mesh: &MeshLod, half_m: f32, band: Band) -> Vec<bool> {
    let n = PAD_FOOTPRINT_RES as usize;
    let mut occ = vec![false; n * n];
    for tri in mesh.indices.chunks_exact(3) {
        let [a, b, c] = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
        if !include_triangle(
            &mesh.vertices[a],
            &mesh.vertices[b],
            &mesh.vertices[c],
            band,
        ) {
            continue;
        }
        fill_triangle(
            &mut occ,
            n,
            half_m,
            mesh.vertices[a].pos,
            mesh.vertices[b].pos,
            mesh.vertices[c].pos,
        );
    }
    occ
}

fn include_triangle(a: &MeshVertex, b: &MeshVertex, c: &MeshVertex, band: Band) -> bool {
    for v in [a, b, c] {
        if v.part == part::SPINNER || v.rig & rig::UPGRADE != 0 {
            return false;
        }
        match band {
            Band::Hull if v.part != part::HULL => return false,
            Band::Turret if v.part != part::TURRET => return false,
            _ => {}
        }
    }
    if matches!(band, Band::Hull) && a.pos[2].min(b.pos[2]).min(c.pos[2]) > GROUND_Z {
        return false;
    }
    if matches!(band, Band::Turret) && is_barrel_facet(a.pos, b.pos, c.pos) {
        return false;
    }
    let abx = b.pos[0] - a.pos[0];
    let aby = b.pos[1] - a.pos[1];
    let acx = c.pos[0] - a.pos[0];
    let acy = c.pos[1] - a.pos[1];
    (abx * acy - aby * acx).abs() * 0.5 >= MIN_XY_AREA
}

/// A gun tube is many thin facets, not one long triangle. Drop those so the
/// pour follows the house, not the barrel.
fn is_barrel_facet(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> bool {
    let min_x = a[0].min(b[0]).min(c[0]);
    let max_x = a[0].max(b[0]).max(c[0]);
    let min_y = a[1].min(b[1]).min(c[1]);
    let max_y = a[1].max(b[1]).max(c[1]);
    let w = max_x - min_x;
    let h = max_y - min_y;
    let long = w.max(h);
    let short = w.min(h);
    short < 0.8 && long / short.max(0.05) > 2.2
}

fn union(a: &[bool], b: &[bool]) -> Vec<bool> {
    a.iter().zip(b).map(|(&x, &y)| x || y).collect()
}

#[derive(Debug)]
struct OccStats {
    count: usize,
    aspect: f32,
    aabb_area: f32,
    min_x: usize,
    max_x: usize,
    min_y: usize,
    max_y: usize,
}

fn occ_stats(occ: &[bool], n: usize) -> OccStats {
    let mut count = 0usize;
    let mut min_x = n;
    let mut max_x = 0usize;
    let mut min_y = n;
    let mut max_y = 0usize;
    for y in 0..n {
        for x in 0..n {
            if !occ[y * n + x] {
                continue;
            }
            count += 1;
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
    }
    if count == 0 {
        return OccStats {
            count: 0,
            aspect: 1.0,
            aabb_area: 0.0,
            min_x: 0,
            max_x: 0,
            min_y: 0,
            max_y: 0,
        };
    }
    let aw = (max_x - min_x + 1) as f32;
    let ah = (max_y - min_y + 1) as f32;
    OccStats {
        count,
        aspect: aw.max(ah) / aw.min(ah).max(1.0),
        aabb_area: aw * ah,
        min_x,
        max_x,
        min_y,
        max_y,
    }
}

/// A square (or octagon with corner buttresses) that is bigger than the house
/// on top of it. A plus-shaped bunker has empty AABB corners and is kept.
fn square_plinth(occ: &[bool], hull: &OccStats, n: usize, turret: &OccStats) -> bool {
    if hull.count == 0 || hull.aspect >= 1.2 || hull.aabb_area <= turret.aabb_area * 1.35 {
        return false;
    }
    let inset_x = ((hull.max_x - hull.min_x) / 8).max(1);
    let inset_y = ((hull.max_y - hull.min_y) / 8).max(1);
    let corners = [
        (hull.min_x + inset_x, hull.min_y + inset_y),
        (hull.max_x - inset_x, hull.min_y + inset_y),
        (hull.min_x + inset_x, hull.max_y - inset_y),
        (hull.max_x - inset_x, hull.max_y - inset_y),
    ];
    let filled = corners
        .iter()
        .filter(|&&(x, y)| x < n && y < n && occ[y * n + x])
        .count();
    filled >= 3
}

fn fill_triangle(occ: &mut [bool], n: usize, half_m: f32, a: [f32; 3], b: [f32; 3], c: [f32; 3]) {
    let to = |p: [f32; 3]| {
        let u = ((p[0] / half_m / PAD_FOOTPRINT_REACH) * 0.5 + 0.5) * n as f32;
        let v = ((p[1] / half_m / PAD_FOOTPRINT_REACH) * 0.5 + 0.5) * n as f32;
        [u, v]
    };
    let pa = to(a);
    let pb = to(b);
    let pc = to(c);
    let min_x = pa[0].min(pb[0]).min(pc[0]).floor().max(0.0) as usize;
    let max_x = pa[0].max(pb[0]).max(pc[0]).ceil().min((n - 1) as f32) as usize;
    let min_y = pa[1].min(pb[1]).min(pc[1]).floor().max(0.0) as usize;
    let max_y = pa[1].max(pb[1]).max(pc[1]).ceil().min((n - 1) as f32) as usize;
    let area = (pb[0] - pa[0]) * (pc[1] - pa[1]) - (pb[1] - pa[1]) * (pc[0] - pa[0]);
    if area.abs() < 1e-5 {
        return;
    }
    let inv = 1.0 / area;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let p = [x as f32 + 0.5, y as f32 + 0.5];
            let w0 = ((pb[0] - p[0]) * (pc[1] - p[1]) - (pb[1] - p[1]) * (pc[0] - p[0])) * inv;
            let w1 = ((pc[0] - p[0]) * (pa[1] - p[1]) - (pc[1] - p[1]) * (pa[0] - p[0])) * inv;
            let w2 = 1.0 - w0 - w1;
            if w0 >= -1e-4 && w1 >= -1e-4 && w2 >= -1e-4 {
                occ[y * n + x] = true;
            }
        }
    }
}

/// 8-connected distance to the form edge, in texels. Negative inside.
fn signed_distance(occ: &[bool], n: usize) -> Vec<f32> {
    let mut dist = vec![f32::INFINITY; n * n];
    let mut queue = std::collections::VecDeque::new();
    let inside = |i: usize| occ[i];
    let push_edge = |dist: &mut [f32], queue: &mut std::collections::VecDeque<usize>, i: usize| {
        if dist[i] > 0.0 {
            dist[i] = 0.0;
            queue.push_back(i);
        }
    };
    for y in 0..n {
        for x in 0..n {
            let i = y * n + x;
            let here = inside(i);
            let mut border = x == 0 || y == 0 || x + 1 == n || y + 1 == n;
            if !border {
                border = inside(i - 1) != here
                    || inside(i + 1) != here
                    || inside(i - n) != here
                    || inside(i + n) != here;
            }
            if border {
                push_edge(&mut dist, &mut queue, i);
            }
        }
    }
    const STEP: [[i32; 2]; 8] = [
        [1, 0],
        [-1, 0],
        [0, 1],
        [0, -1],
        [1, 1],
        [1, -1],
        [-1, 1],
        [-1, -1],
    ];
    const LEN: [f32; 8] = [
        1.0,
        1.0,
        1.0,
        1.0,
        std::f32::consts::SQRT_2,
        std::f32::consts::SQRT_2,
        std::f32::consts::SQRT_2,
        std::f32::consts::SQRT_2,
    ];
    while let Some(i) = queue.pop_front() {
        let x = (i % n) as i32;
        let y = (i / n) as i32;
        let d0 = dist[i];
        for (s, &len) in STEP.iter().zip(LEN.iter()) {
            let nx = x + s[0];
            let ny = y + s[1];
            if nx < 0 || ny < 0 || nx >= n as i32 || ny >= n as i32 {
                continue;
            }
            let j = ny as usize * n + nx as usize;
            let d = d0 + len;
            if d < dist[j] {
                dist[j] = d;
                queue.push_back(j);
            }
        }
    }
    for (i, d) in dist.iter_mut().enumerate() {
        if *d == f32::INFINITY {
            *d = n as f32;
        }
        if occ[i] {
            *d = -*d;
        }
    }
    dist
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{build_model_scaled, MeshLod};

    fn baked(key: &str, radius: f32, height: f32, tech: u8, cells: u32) -> (Vec<u8>, f32) {
        let model = build_model_scaled(key, radius, height, tech).expect(key);
        let half = cells as f32 * 6.0;
        (bake_pad_footprint(&model.lods[0], half), half)
    }

    fn inside(tex: &[u8], uv: [f32; 2]) -> bool {
        pad_sdf_at(tex, uv) < 0.0
    }

    #[test]
    fn factory_traces_the_deck_and_halls() {
        let (tex, _) = baked("factory_land", 46.0, 28.0, 1, 8);
        // Print deck sits on the lot origin; halls frame it on ±y.
        assert!(inside(&tex, [0.0, 0.0]), "print deck");
        assert!(inside(&tex, [0.0, 0.54]), "side hall");
        assert!(
            !inside(&tex, [0.97, 0.97]),
            "lot corner is dirt, not a hashed slab"
        );
    }

    #[test]
    fn extractor_keeps_the_well_open() {
        let (tex, _) = baked("extractor", 10.5, 6.75, 1, 2);
        assert!(!inside(&tex, [0.0, 0.0]), "bore stays open");
        assert!(inside(&tex, [0.5, 0.5]), "a cell foot");
        assert!(!inside(&tex, [0.92, 0.92]), "outside the 2x2 feet");
    }

    #[test]
    fn howitzer_is_a_plus() {
        let (tex, _) = baked("artillery_static", 10.5, 9.0, 2, 2);
        assert!(inside(&tex, [0.5, 0.0]), "spine");
        assert!(inside(&tex, [0.0, 0.55]), "magazine wing");
        assert!(!inside(&tex, [0.68, 0.68]), "plus corner is dirt");
    }

    #[test]
    fn turret_is_not_the_build_cell() {
        let (tex, _) = baked("turret", 5.25, 6.75, 1, 1);
        assert!(inside(&tex, [0.0, 0.0]), "under the house");
        assert!(inside(&tex, [0.12, 0.0]), "turret face");
        assert!(
            !inside(&tex, [0.5, 0.5]),
            "the 1x1 cell is not poured as a square"
        );
        assert!(
            !inside(&tex, [0.55, 0.0]),
            "the barrel is not poured as a slab"
        );
    }

    #[test]
    fn empty_mesh_is_all_outside() {
        let tex = bake_pad_footprint(&MeshLod::default(), 12.0);
        assert!(tex.iter().all(|&p| p == 0));
        assert!(pad_sdf_at(&tex, [0.0, 0.0]) > 0.0);
    }

    fn hull(key: &str, radius: f32, height: f32, tech: u8) -> (Vec<u8>, f32, f32) {
        let model = build_model_scaled(key, radius, height, tech).expect(key);
        let mesh = &model.lods[0];
        let half = hull_plan_half(mesh);
        let h = mesh
            .vertices
            .iter()
            .map(|v| v.pos[2])
            .fold(0.0f32, f32::max)
            .max(0.5);
        (bake_hull_plan(mesh, half, h), half, h)
    }

    fn hull_inside(tex: &[u8], half: f32, height: f32, local: [f32; 3]) -> bool {
        hull_plan_sd(tex, local, half, height) < 0.0
    }

    #[test]
    fn commander_feet_are_not_a_disc() {
        let (tex, half, h) = hull("commander", 10.4, 24.0, 1);
        // Soles sit at y ≈ ±1.85. A radius disc would fill the gap between them.
        assert!(hull_inside(&tex, half, h, [0.4, 1.85, 0.4]), "left sole");
        assert!(hull_inside(&tex, half, h, [0.4, -1.85, 0.4]), "right sole");
        assert!(
            !hull_inside(&tex, half, h, [0.0, 0.0, 0.4]),
            "the gap between the feet is not a contact disc"
        );
    }

    #[test]
    fn commander_torso_meets_a_high_slice() {
        let (tex, half, h) = hull("commander", 10.4, 24.0, 1);
        // Authored chest sits around z 10–13; scaling to the 24 m hull
        // keeps a mid-height slice under the origin.
        let chest = [0.0, 0.0, h * 0.55];
        assert!(
            hull_inside(&tex, half, h, chest),
            "the chest is a slice, not a foot print extruded to the crown (sd={:?} half={half} h={h})",
            hull_plan_sd(&tex, chest, half, h)
        );
        assert!(
            !hull_inside(&tex, half, h, [0.0, 0.0, 0.4]),
            "the chest does not reach the glass under the body"
        );
    }

    #[test]
    fn tank_plan_is_longer_than_wide() {
        let (tex, half, h) = hull("tank_light", 4.6, 3.4, 1);
        assert!(hull_inside(&tex, half, h, [2.0, 0.0, 1.0]), "hull ahead");
        assert!(
            !hull_inside(&tex, half, h, [0.0, 4.4, 1.0]),
            "beside the tracks is air, not a radius circle"
        );
    }

    #[test]
    fn empty_hull_plan_is_outside() {
        let tex = bake_hull_plan(&MeshLod::default(), 4.0, 4.0);
        assert!(tex.iter().all(|&p| p == 0));
        assert!(hull_plan_sd(&tex, [0.0, 0.0, 0.0], 4.0, 4.0) > 0.0);
    }
}
