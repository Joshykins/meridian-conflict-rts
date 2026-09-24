//! Unit portraits for the interface: a small software rasteriser that draws a
//! model from the RTS camera onto a transparent background. The test previews
//! (`preview.rs`) draw with it too, on a ground colour.

use glam::{Affine3A, Vec2, Vec3};

use super::{library, material, rig, MeshLod, Model};

/// Flat palette (linear RGB) and whether the material is emissive.
pub fn material_color(id: u32) -> ([f32; 3], bool) {
    match id {
        material::PLATING => ([0.93, 0.94, 0.95], false),
        material::ACCENT => ([0.13, 0.14, 0.16], false),
        material::GLOW => ([0.62, 0.9, 1.0], true),
        material::TEAM => ([0.85, 0.1, 0.12], false),
        material::METAL => ([0.42, 0.44, 0.47], false),
        material::GLASS => ([0.1, 0.24, 0.34], false),
        material::TREAD => ([0.07, 0.07, 0.08], false),
        material::GLOW_ORANGE => ([1.0, 0.58, 0.15], true),
        material::BARK => ([0.3, 0.21, 0.14], false),
        material::FOLIAGE => ([0.16, 0.36, 0.14], false),
        material::ROCK => ([0.45, 0.43, 0.4], false),
        material::CONCRETE => ([0.62, 0.61, 0.58], false),
        material::WINDOWS => ([0.95, 0.85, 0.55], true),
        material::GLOW_AMBER => ([1.0, 0.72, 0.12], true),
        material::PLATING_DARK => ([0.2, 0.21, 0.24], false),
        material::GLOW_RED => ([1.0, 0.12, 0.1], true),
        material::GLOW_VIOLET => ([0.72, 0.45, 1.0], true),
        _ => ([1.0, 0.0, 1.0], false),
    }
}

/// A portrait of the model for `mesh` (a blueprint's `visual.mesh`) at its tech 1
/// look, or None for a key there is no model for. See [`thumbnail_of`].
pub fn thumbnail(mesh: &str, size: usize, azimuth_degrees: f32, team: [f32; 3]) -> Option<Vec<u8>> {
    // Only the full detail level is needed, and framing makes the scale moot.
    let def = library::find(mesh)?;
    let mut builder = super::builder::MeshBuilder::new(0, Affine3A::IDENTITY);
    (def.build)(&mut builder, 1);
    Some(rgba(&rasterise(&builder.finish(), size, azimuth_degrees, team)))
}

/// A portrait of `model`: RGBA8, sRGB, straight alpha, `size` x `size`, with
/// a transparent background and 2x2 supersampled edges. From 50 degrees above
/// the horizon and `azimuth_degrees` around the model (0 looks at its nose,
/// -38 is the usual three-quarter view), framed to fit. The owner's colour is
/// `team` (linear RGB); upgrade pieces are left off. Takes about a millisecond
/// at 128 px.
pub fn thumbnail_of(model: &Model, size: usize, azimuth_degrees: f32, team: [f32; 3]) -> Vec<u8> {
    rgba(&rasterise(&model.lods[0], size, azimuth_degrees, team))
}

/// Samples per pixel each way.
const SS: usize = 3;

/// Model z below which a portrait squashes the mesh flat (metres).
const PORTRAIT_FLOOR: f32 = -4.0;

/// Edge of the shadow map the key light casts with.
const SHADOW_MAP: usize = 320;

/// What the rasteriser drew, at `SS` times the asked size each way: lit and
/// tone-mapped linear colour and coverage per sample, the emissive light that
/// blooms around it, and the shadow it throws on the ground under it.
pub(super) struct Samples {
    pub n: usize,
    pub color: Vec<[f32; 3]>,
    pub covered: Vec<bool>,
    /// Linear HDR light given off by glowing materials.
    pub glow: Vec<[f32; 3]>,
    /// How much of the key light's shadow falls on the ground here, 0..1.
    pub shade: Vec<f32>,
}

/// How a material takes light: roughness and how metallic it is.
fn finish(id: u32) -> (f32, f32) {
    match id {
        material::METAL => (0.34, 0.9),
        material::GLASS => (0.08, 0.0),
        material::TREAD => (0.9, 0.0),
        material::ACCENT | material::PLATING_DARK => (0.46, 0.15),
        material::TEAM => (0.4, 0.05),
        material::CONCRETE | material::ROCK | material::BARK | material::FOLIAGE => (0.88, 0.0),
        _ => (0.52, 0.05),
    }
}

/// Armour the game's shader textures from each face's frame (seams, plates).
fn framed(id: u32) -> bool {
    matches!(id, material::PLATING | material::ACCENT | material::TEAM | material::PLATING_DARK)
}

/// The sky a shiny surface reflects: a bright softbox where the key light is,
/// a cool dome and a dark floor.
fn sky(dir: Vec3, light: Vec3) -> Vec3 {
    let up = dir.z;
    let dome = if up > 0.0 {
        Vec3::new(0.62, 0.66, 0.72).lerp(Vec3::new(0.3, 0.4, 0.56), up.sqrt())
    } else {
        Vec3::new(0.2, 0.19, 0.18).lerp(Vec3::new(0.07, 0.07, 0.075), (-up * 3.0).min(1.0))
    };
    let softbox = dir.dot(light).max(0.0).powf(24.0) * 3.2;
    dome + Vec3::splat(softbox)
}

/// A filmic curve (ACES, fitted), so highlights roll off instead of clipping flat.
fn filmic(c: Vec3) -> Vec3 {
    let f = |x: f32| ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0);
    Vec3::new(f(c.x), f(c.y), f(c.z))
}

/// Rasterises triangle `s` (screen points with a per-corner value `z`) into an
/// `n` x `n` grid, calling `hit(index, barycentrics, depth)` for every sample centre inside.
fn scan(s: [Vec2; 3], z: [f32; 3], n: usize, mut hit: impl FnMut(usize, [f32; 3], f32)) {
    let area = (s[1] - s[0]).perp_dot(s[2] - s[0]);
    if area.abs() < 1e-6 {
        return;
    }
    let (min, max) = (s[0].min(s[1]).min(s[2]).floor(), s[0].max(s[1]).max(s[2]).ceil());
    if max.x < 0.0 || max.y < 0.0 || min.x >= n as f32 || min.y >= n as f32 {
        return;
    }
    for y in (min.y.max(0.0) as usize)..(max.y.min(n as f32 - 1.0) as usize + 1) {
        for x in (min.x.max(0.0) as usize)..(max.x.min(n as f32 - 1.0) as usize + 1) {
            let q = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
            let w0 = (s[1] - q).perp_dot(s[2] - q) / area;
            let w1 = (s[2] - q).perp_dot(s[0] - q) / area;
            let w2 = 1.0 - w0 - w1;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            hit(y * n + x, [w0, w1, w2], w0 * z[0] + w1 * z[1] + w2 * z[2]);
        }
    }
}

/// Orthographic render from the RTS camera, framed to fit. Back faces are
/// culled, so a face wound the wrong way shows as a hole. Lit like a studio
/// shot of the in-game model: a shadowing key light, sky and bounce, ambient
/// occlusion, the seams between armour panels, reflections on metal and glass.
pub(super) fn rasterise(mesh: &MeshLod, size: usize, azimuth_degrees: f32, team: [f32; 3]) -> Samples {
    let (elevation, azimuth) = (50f32.to_radians(), azimuth_degrees.to_radians());
    let to_camera = Vec3::new(
        elevation.cos() * azimuth.cos(),
        elevation.cos() * azimuth.sin(),
        elevation.sin(),
    );
    let right = (-to_camera).cross(Vec3::Z).normalize();
    let up = right.cross(-to_camera);
    // The key light from the upper left of the view and a little behind, so the
    // shadow falls toward the viewer's lower right and the tops catch it.
    let light = (Vec3::Z * 0.8 - right * 0.62 - to_camera * 0.12 + up * 0.1).normalize();
    let shown = |v: &super::MeshVertex| v.rig & rig::UPGRADE == 0;
    // A portrait shows a hull down to its keel, not piles down to the seabed:
    // anything deeper than this is squashed up to it.
    let at = |v: &super::MeshVertex| Vec3::new(v.pos[0], v.pos[1], v.pos[2].max(PORTRAIT_FLOOR));

    // Frame what is drawn, then rasterise with SS x SS supersampling.
    let flat = |p: Vec3| Vec2::new(p.dot(right), p.dot(up));
    let (lo, hi) = mesh
        .vertices
        .iter()
        .filter(|v| shown(v))
        .fold((Vec2::MAX, Vec2::MIN), |(lo, hi), v| (lo.min(flat(at(v))), hi.max(flat(at(v)))));
    let n = size * SS;
    let mut samples = Samples {
        n,
        color: vec![[0.0; 3]; n * n],
        covered: vec![false; n * n],
        glow: vec![[0.0; 3]; n * n],
        shade: vec![0.0; n * n],
    };
    if lo.x > hi.x {
        return samples;
    }
    let (z_lo, z_hi) = mesh
        .vertices
        .iter()
        .filter(|v| shown(v))
        .fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(at(v).z), hi.max(at(v).z)));
    // A little room round the model for its glow.
    let scale = n as f32 * 0.9 / (hi - lo).max_element().max(1e-3);
    let middle = (lo + hi) * 0.5;
    let project = |p: Vec3| {
        let q = (flat(p) - middle) * scale;
        Vec2::new(n as f32 * 0.5 + q.x, n as f32 * 0.5 - q.y)
    };
    // One output pixel, in metres.
    let pixel = SS as f32 / scale;

    // The key light's shadow map: depth toward the light over the model's extent.
    let lx = light.cross(Vec3::Z).try_normalize().unwrap_or(Vec3::X);
    let ly = light.cross(lx);
    let (llo, lhi) = mesh
        .vertices
        .iter()
        .filter(|v| shown(v))
        .fold((Vec2::MAX, Vec2::MIN), |(lo, hi), v| {
            let p = at(v);
            let q = Vec2::new(p.dot(lx), p.dot(ly));
            (lo.min(q), hi.max(q))
        });
    let lscale = (SHADOW_MAP as f32 - 4.0) / (lhi - llo).max_element().max(1e-3);
    let to_map = |p: Vec3| (Vec2::new(p.dot(lx), p.dot(ly)) - llo) * lscale + Vec2::splat(2.0);
    let mut shadow_map = vec![f32::MIN; SHADOW_MAP * SHADOW_MAP];
    for t in mesh.indices.chunks(3) {
        let v = |i: u32| &mesh.vertices[i as usize];
        if !shown(v(t[0])) {
            continue;
        }
        let p = [at(v(t[0])), at(v(t[1])), at(v(t[2]))];
        scan(p.map(to_map), p.map(|q| q.dot(light)), SHADOW_MAP, |i, _, d| {
            if d > shadow_map[i] {
                shadow_map[i] = d;
            }
        });
    }
    // Lit by the key light, 0..1, softened over the texels round it.
    let map_texel = 1.0 / lscale;
    let lit_at = |p: Vec3, n: Vec3| {
        let bias = map_texel * (1.5 + 2.5 * (1.0 - n.dot(light).max(0.0)));
        let q = to_map(p + n * map_texel * 0.8);
        let d = p.dot(light) + bias;
        let mut lit = 0.0;
        for oy in -1..=1 {
            for ox in -1..=1 {
                let (x, y) = (q.x as isize + ox, q.y as isize + oy);
                if x < 0 || y < 0 || x >= SHADOW_MAP as isize || y >= SHADOW_MAP as isize {
                    lit += 1.0;
                    continue;
                }
                lit += if shadow_map[y as usize * SHADOW_MAP + x as usize] > d { 0.0 } else { 1.0 };
            }
        }
        lit / 9.0
    };

    // The geometry pass: nearest surface per sample, with what shading needs.
    struct Hit {
        pos: Vec3,
        normal: Vec3,
        material: u32,
        face: [f32; 4],
        tone: f32,
    }
    let mut depth = vec![f32::MIN; n * n];
    let mut hits: Vec<Option<Hit>> = (0..n * n).map(|_| None).collect();
    for t in mesh.indices.chunks(3) {
        let v = |i: u32| &mesh.vertices[i as usize];
        let (a, b, c) = (v(t[0]), v(t[1]), v(t[2]));
        if !shown(a) {
            continue;
        }
        let p = [at(a), at(b), at(c)];
        if (p[1] - p[0]).cross(p[2] - p[0]).dot(to_camera) <= 0.0 {
            continue;
        }
        let normals = [a, b, c].map(|v| Vec3::from(v.normal));
        // Panels differ a shade from their neighbours, as painted plates do.
        let tone = 1.0 + (((a.surface >> 8) & 0xFF) as f32 / 255.0 - 0.5) * 0.09;
        scan(p.map(project), p.map(|q| q.dot(to_camera)), n, |i, w, d| {
            if d <= depth[i] {
                return;
            }
            depth[i] = d;
            let lerp3 = |x: [Vec3; 3]| x[0] * w[0] + x[1] * w[1] + x[2] * w[2];
            let normal = lerp3(normals).try_normalize().unwrap_or(Vec3::Z);
            let fa = [a.face, b.face, c.face];
            let face = [
                fa[0][0] * w[0] + fa[1][0] * w[1] + fa[2][0] * w[2],
                fa[0][1] * w[0] + fa[1][1] * w[1] + fa[2][1] * w[2],
                a.face[2],
                a.face[3],
            ];
            hits[i] = Some(Hit {
                pos: lerp3(p),
                normal,
                material: a.material,
                face,
                tone,
            });
        });
    }

    // Ambient occlusion from the depth buffer: how far the surroundings rise
    // toward the camera, looked for in eight directions at two reaches.
    let reach = (n as f32 / 34.0).max(3.0);
    let world_reach = reach / scale;
    let occlusion = |x: usize, y: usize, d: f32| {
        let mut open = 0.0;
        for k in 0..8 {
            let a = k as f32 * std::f32::consts::TAU / 8.0 + 0.4;
            let dir = Vec2::new(a.cos(), a.sin());
            let mut worst: f32 = 0.0;
            for step in [0.45, 1.0] {
                let q = Vec2::new(x as f32, y as f32) + dir * reach * step;
                if q.x < 0.0 || q.y < 0.0 || q.x >= n as f32 || q.y >= n as f32 {
                    continue;
                }
                let dn = depth[q.y as usize * n + q.x as usize];
                if dn == f32::MIN {
                    continue;
                }
                let rise = (dn - d) / (world_reach * step);
                // Steep drops away do not open a hole, and far rises fade.
                let falloff = (1.0 - (dn - d) / (world_reach * 3.0)).clamp(0.0, 1.0);
                worst = worst.max((rise.clamp(0.0, 1.5) - 0.12).max(0.0) * falloff);
            }
            open += 1.0 - worst.min(1.0);
        }
        (open / 8.0).powf(1.4)
    };

    let key = Vec3::new(1.0, 0.95, 0.86) * 2.0;
    let exposure = 0.62;
    let ground_z = z_lo.min(0.0);
    for y in 0..n {
        for x in 0..n {
            let i = y * n + x;
            let Some(h) = &hits[i] else { continue };
            let (base, emissive) = match h.material {
                material::TEAM => (team, false),
                id => material_color(id),
            };
            let base = Vec3::from(base);
            samples.covered[i] = true;
            if emissive {
                let hot = base * 2.2;
                samples.color[i] = filmic(hot * exposure).into();
                samples.glow[i] = (base * 1.4).into();
                continue;
            }
            let nrm = h.normal;
            let v = to_camera;
            let (mut rough, metal) = finish(h.material);
            // Plating is a touch less white than its flat colour, as the game's lighting leaves it.
            let mut albedo = if h.material == material::PLATING { base * 0.74 } else { base } * h.tone;
            // Seams between armour panels, lit on the edge that faces the light.
            if framed(h.material) {
                let (st, half) = (Vec2::new(h.face[0], h.face[1]), Vec2::new(h.face[2].abs(), h.face[3].abs()));
                let wraps = h.face[2] < 0.0;
                if half.min_element() > pixel * 2.5 {
                    let ex = if wraps { f32::MAX } else { half.x - st.x.abs() };
                    let e = ex.min(half.y - st.y.abs());
                    let w = pixel * 0.75;
                    if e < w {
                        albedo *= 0.38;
                        rough = 0.8;
                    } else if e < w * 2.2 {
                        albedo *= 1.12;
                    }
                }
            }
            let occl = occlusion(x, y, depth[i]);
            // Low parts sit a little in the shadow of the rest.
            let height = ((h.pos.z - z_lo) / (z_hi - z_lo).max(1e-3)).clamp(0.0, 1.0);
            let ao = occl * (0.78 + 0.22 * height.sqrt());
            let ndl = nrm.dot(light).max(0.0);
            let lit = if ndl > 0.0 { lit_at(h.pos, nrm) } else { 0.0 };
            let ndv = nrm.dot(v).max(1e-3);
            let f0 = Vec3::splat(0.04).lerp(albedo, metal);
            let fresnel = f0 + (Vec3::ONE - f0) * (1.0 - ndv).powi(5) * (1.0 - rough * 0.8);
            let hemi = Vec3::new(0.2, 0.19, 0.17).lerp(Vec3::new(0.46, 0.5, 0.6), 0.5 + 0.5 * nrm.z);
            let diffuse = albedo * (1.0 - metal) * (hemi * ao + key * ndl * lit);
            let half_v = (light + v).normalize();
            let power = 2.0 / (rough * rough * rough * rough).max(1e-4) - 2.0;
            let spec = key * lit * ndl * nrm.dot(half_v).max(0.0).powf(power) * (power + 8.0) / 25.0;
            let reflect = (nrm * 2.0 * nrm.dot(v) - v).normalize();
            let env = sky(reflect, light) * (1.0 - rough).powf(1.5) * ao;
            // A cool rim along the far edges, to lift the outline off a dark tile.
            let rim = Vec3::new(0.5, 0.6, 0.75) * (1.0 - ndv).powi(3) * 0.35 * ao;
            let c = diffuse + fresnel * (spec + env) + rim;
            samples.color[i] = filmic(c * exposure).into();

            // Its shadow on the ground, cast along the key light onto the plane under it.
            let g = h.pos - light * ((h.pos.z - ground_z) / light.z.max(0.2));
            let s = project(Vec3::new(g.x, g.y, ground_z));
            if s.x >= 0.0 && s.y >= 0.0 && s.x < n as f32 && s.y < n as f32 {
                samples.shade[s.y as usize * n + s.x as usize] = 1.0;
            }
        }
    }
    samples
}

fn srgb(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0 + 0.5) as u8
}

/// A separable box blur run three times (close to a gaussian), in place over a
/// `size` x `size` grid of `K` channels.
fn blur<const K: usize>(grid: &mut [[f32; K]], size: usize, radius: usize) {
    if radius == 0 {
        return;
    }
    let mut tmp = vec![[0.0f32; K]; size * size];
    for _ in 0..3 {
        for pass in 0..2 {
            for line in 0..size {
                let at = |j: usize| if pass == 0 { line * size + j } else { j * size + line };
                let mut sum = [0.0f32; K];
                let r = radius as isize;
                for j in -r..=r {
                    let j = j.clamp(0, size as isize - 1) as usize;
                    for c in 0..K {
                        sum[c] += grid[at(j)][c];
                    }
                }
                for j in 0..size {
                    for c in 0..K {
                        tmp[at(j)][c] = sum[c] / (2 * radius + 1) as f32;
                    }
                    let out = (j as isize - r).clamp(0, size as isize - 1) as usize;
                    let into = (j as isize + r + 1).clamp(0, size as isize - 1) as usize;
                    for c in 0..K {
                        sum[c] += grid[at(into)][c] - grid[at(out)][c];
                    }
                }
            }
            grid.copy_from_slice(&tmp);
        }
    }
}

/// Resolves the samples to straight-alpha RGBA8: the model over its soft
/// ground shadow, with the bloom of its lights spilling past its edges. Empty
/// pixels take the colour of their drawn neighbours, so a filtered edge does
/// not pick up a dark fringe.
fn rgba(samples: &Samples) -> Vec<u8> {
    let (n, size) = (samples.n, samples.n / SS);
    let mut color = vec![[0.0f32; 3]; size * size];
    let mut cover = vec![0.0f32; size * size];
    let mut glow = vec![[0.0f32; 3]; size * size];
    let mut shade = vec![[0.0f32; 1]; size * size];
    let k = 1.0 / (SS * SS) as f32;
    for i in 0..size * size {
        let at = i / size * SS * n + i % size * SS;
        let mut sum = [0.0; 3];
        let mut count = 0;
        for oy in 0..SS {
            for ox in 0..SS {
                let o = at + oy * n + ox;
                for c in 0..3 {
                    glow[i][c] += samples.glow[o][c] * k;
                }
                shade[i][0] += samples.shade[o] * k;
                if samples.covered[o] {
                    let c = samples.color[o];
                    sum = [sum[0] + c[0], sum[1] + c[1], sum[2] + c[2]];
                    count += 1;
                }
            }
        }
        if count > 0 {
            color[i] = sum.map(|c| c / count as f32);
            cover[i] = count as f32 * k;
        }
    }
    // The shadow: splatted sparsely, so closed up and softened; the bloom, wide and faint.
    for s in shade.iter_mut() {
        s[0] = (s[0] * 4.0).min(1.0);
    }
    blur(&mut shade, size, (size / 60).max(1));
    blur(&mut glow, size, (size / 28).max(1));

    let mut out = vec![0u8; size * size * 4];
    for i in 0..size * size {
        // Shadow and glow fade out toward the picture's edges, where they would be cut off.
        let (ex, ey) = ((i % size) as f32 + 0.5, (i / size) as f32 + 0.5);
        let border = (ex.min(ey).min(size as f32 - ex).min(size as f32 - ey) / (size as f32 * 0.1) - 0.1).clamp(0.0, 1.0);
        let g = glow[i].map(|c| c * 0.9);
        let g_a = (g[0].max(g[1]).max(g[2]) * 1.6).min(1.0) * border;
        let s_a = shade[i][0] * 0.55 * border;
        // Premultiplied: the model over the glow over the shadow.
        let mut pre = [0.0f32; 3];
        let mut a = s_a;
        for c in 0..3 {
            pre[c] = g[c].min(1.0) * g_a + pre[c] * (1.0 - g_a);
        }
        a = g_a + a * (1.0 - g_a);
        let m = cover[i];
        for c in 0..3 {
            let lit = (color[i][c] + g[c] * 0.35).min(1.0);
            pre[c] = lit * m + pre[c] * (1.0 - m);
        }
        a = m + a * (1.0 - m);
        let mut c = if a > 0.0 { pre.map(|p| p / a) } else { [0.0; 3] };
        if a <= 0.0 {
            let (x, y) = ((i % size) as isize, (i / size) as isize);
            let mut sum = [0.0; 3];
            let mut count = 0;
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let (nx, ny) = (x + dx, y + dy);
                if nx >= 0 && ny >= 0 && (nx as usize) < size && (ny as usize) < size {
                    let j = ny as usize * size + nx as usize;
                    if cover[j] > 0.0 {
                        sum = [sum[0] + color[j][0], sum[1] + color[j][1], sum[2] + color[j][2]];
                        count += 1;
                    }
                }
            }
            if count > 0 {
                c = sum.map(|s| s / count as f32);
            }
        }
        let alpha = (a.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        out[i * 4..i * 4 + 4].copy_from_slice(&[srgb(c[0]), srgb(c[1]), srgb(c[2]), alpha]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumbnails_are_cut_out_and_quick() {
        let start = std::time::Instant::now();
        let keys: Vec<_> = super::super::aster::MODELS.iter().map(|def| def.key).collect();
        for key in &keys {
            let rgba = thumbnail(key, 112, -38.0, [0.2, 0.5, 1.0]).expect(key);
            assert_eq!(rgba.len(), 112 * 112 * 4);
            // Corners are background; something is drawn and some of it is solid.
            for at in [0, 111, 111 * 112, 112 * 112 - 1] {
                assert_eq!(rgba[at * 4 + 3], 0, "{key}: corner is not transparent");
            }
            let solid = rgba.chunks_exact(4).filter(|p| p[3] == 255).count();
            assert!(solid > 112 * 112 / 20, "{key}: only {solid} solid pixels");
            // Straight alpha: the edge keeps the colour of what it is the edge of.
            assert!(rgba.chunks_exact(4).any(|p| p[3] > 0 && p[3] < 255));
            if let Ok(dir) = std::env::var("THUMB_DUMP_DIR") {
                std::fs::write(format!("{dir}/{key}.rgba"), &rgba).unwrap();
            }
        }
        assert!(thumbnail("no_such_mesh", 64, 0.0, [1.0; 3]).is_none());
        let each = start.elapsed().as_secs_f32() * 1000.0 / keys.len() as f32;
        eprintln!("{} thumbnails, {each:.2} ms each", keys.len());
    }
}

#[cfg(test)]
mod tech_tests {
    use super::super::{all_model_keys, build_model_scaled};

    /// The data allows tech 1..=5; models only distinguish 1..=3 and must not panic above.
    #[test]
    fn every_model_builds_at_tech_four_and_five() {
        for key in all_model_keys() {
            for tech in [0, 4, 5, 255] {
                assert!(build_model_scaled(key, 10.0, 8.0, tech).is_some(), "{key} tech {tech}");
            }
        }
    }
}
