//! Procedural textures generated at start-up. Both tile seamlessly.

/// Texture edge in texels.
pub const SIZE: usize = 512;

const D: f32 = std::f32::consts::FRAC_1_SQRT_2;
/// cos and sin of 22.5 degrees.
const C: f32 = 0.923_879_5;
const S: f32 = 0.382_683_43;
/// 16 unit gradients, evenly spaced. Value noise's four bilinear cells read as
/// a square grid once the texture is tiled or minified; these do not.
const GRADIENTS: [(f32, f32); 16] = [
    (1.0, 0.0),
    (C, S),
    (D, D),
    (S, C),
    (0.0, 1.0),
    (-S, C),
    (-D, D),
    (-C, S),
    (-1.0, 0.0),
    (-C, -S),
    (-D, -D),
    (-S, -C),
    (0.0, -1.0),
    (S, -C),
    (D, -D),
    (C, -S),
];

fn hash_u(x: u32, y: u32, seed: u32) -> u32 {
    let mut h =
        x.wrapping_mul(0x85EB_CA6B) ^ y.wrapping_mul(0xC2B2_AE35) ^ seed.wrapping_mul(0x27D4_EB2F);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    h
}

fn hash(x: u32, y: u32, seed: u32) -> f32 {
    (hash_u(x, y, seed) & 0xFFFF) as f32 / 65535.0
}

/// Value noise that repeats every `period` lattice cells.
fn value_noise(x: f32, y: f32, period: u32, seed: u32) -> f32 {
    let (xi, yi) = (x.floor(), y.floor());
    let (fx, fy) = (x - xi, y - yi);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let wrap = |v: f32| (v as i64).rem_euclid(period as i64) as u32;
    let (x0, y0, x1, y1) = (wrap(xi), wrap(yi), wrap(xi + 1.0), wrap(yi + 1.0));
    let top = hash(x0, y0, seed) * (1.0 - sx) + hash(x1, y0, seed) * sx;
    let bottom = hash(x0, y1, seed) * (1.0 - sx) + hash(x1, y1, seed) * sx;
    top * (1.0 - sy) + bottom * sy
}

/// Gradient noise that repeats every `period` lattice cells. Roughly `[-1, 1]`.
fn gradient_noise(x: f32, y: f32, period: u32, seed: u32) -> f32 {
    let (xi, yi) = (x.floor(), y.floor());
    let (fx, fy) = (x - xi, y - yi);
    let fade = |t: f32| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let (su, sv) = (fade(fx), fade(fy));
    let wrap = |v: f32| (v as i64).rem_euclid(period as i64) as u32;
    let corner = |dx: f32, dy: f32| {
        let (gx, gy) = GRADIENTS[(hash_u(wrap(xi + dx), wrap(yi + dy), seed) & 15) as usize];
        gx * (fx - dx) + gy * (fy - dy)
    };
    let bottom = corner(0.0, 0.0) + (corner(1.0, 0.0) - corner(0.0, 0.0)) * su;
    let top = corner(0.0, 1.0) + (corner(1.0, 1.0) - corner(0.0, 1.0)) * su;
    // Unit gradients peak at sqrt(1/2); rescale to fill [-1, 1].
    (bottom + (top - bottom) * sv) * std::f32::consts::SQRT_2
}

fn fbm(u: f32, v: f32, seed: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut norm = 0.0;
    // Eight cells across the tile, not four: a 4-cell value-noise lattice is
    // the square grid that showed on the ground once the texture repeated.
    // Stop at 256 cells (two texels per cell) — 512 and 1024 alias on a 512
    // map and those beats read as occasional lines after the tile repeats.
    let mut period = 8u32;
    for octave in 0..6 {
        let shift = (octave * 13) as f32;
        sum += amp
            * gradient_noise(
                u * period as f32 + shift,
                v * period as f32 - shift,
                period,
                seed + octave,
            );
        norm += amp;
        amp *= 0.58;
        period *= 2;
    }
    sum / norm * 0.5 + 0.5
}

/// Turns a tiling height map into RGBA: rg = normal xy, b = height, a = `extra`.
fn pack(height: &[f32], extra: &[f32], bump: f32) -> Vec<u8> {
    let mut out = vec![0u8; SIZE * SIZE * 4];
    let at = |x: usize, y: usize| height[(y % SIZE) * SIZE + (x % SIZE)];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = (at(x + 1, y) - at(x + SIZE - 1, y)) * bump;
            let dy = (at(x, y + 1) - at(x, y + SIZE - 1)) * bump;
            let inv = 1.0 / (dx * dx + dy * dy + 1.0).sqrt();
            let i = (y * SIZE + x) * 4;
            out[i] = ((-dx * inv * 0.5 + 0.5) * 255.0) as u8;
            out[i + 1] = ((-dy * inv * 0.5 + 0.5) * 255.0) as u8;
            out[i + 2] = (at(x, y).clamp(0.0, 1.0) * 255.0) as u8;
            out[i + 3] = (extra[y * SIZE + x].clamp(0.0, 1.0) * 255.0) as u8;
        }
    }
    out
}

/// Terrain and water detail: two independent fBm fields and the first one's normal.
pub fn noise_map() -> Vec<u8> {
    let mut a = vec![0.0; SIZE * SIZE];
    let mut b = vec![0.0; SIZE * SIZE];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (u, v) = (
                (x as f32 + 0.5) / SIZE as f32,
                (y as f32 + 0.5) / SIZE as f32,
            );
            a[y * SIZE + x] = fbm(u, v, 11);
            b[y * SIZE + x] = fbm(u, v, 97);
        }
    }
    pack(&a, &b, 6.0)
}

/// Armour plating: courses of plates of uneven length, like welded and
/// bolted steel, not a square grid. Seams are narrow; some plates sit a
/// little proud, some carry a row of bolts or an inset access cover.
/// rg = normal, b = cavity darkening (1 = clean plate), a = wear (dirt and scuffs gather where it is high).
pub fn panel_map() -> Vec<u8> {
    // Course heights and the plate lengths to draw from, in texels; both sum to / divide into SIZE.
    const COURSES: [usize; 5] = [96, 64, 136, 80, 136];
    const LENGTHS: [usize; 5] = [72, 104, 136, 176, 216];
    const SEAM: f32 = 2.0;
    let mut height = vec![1.0f32; SIZE * SIZE];
    let mut cavity = vec![1.0f32; SIZE * SIZE];
    let mut y0 = 0;
    for (course, &h) in COURSES.iter().enumerate() {
        // Each course starts somewhere else along its length, so seams never line up into a grid.
        let start = (hash(course as u32, 0, 3) * SIZE as f32) as usize;
        let mut x_run = 0;
        let mut index = 0u32;
        while x_run < SIZE {
            let mut w = LENGTHS
                [(hash(course as u32, index, 5) * LENGTHS.len() as f32) as usize % LENGTHS.len()];
            if SIZE - x_run < w + 56 {
                w = SIZE - x_run;
            }
            let lift = hash(course as u32, index, 9) * 0.3;
            let style = hash(course as u32, index, 13);
            for y in 0..h {
                for x in 0..w {
                    let edge = x.min(y).min(w - 1 - x).min(h - 1 - y) as f32;
                    let seam = (edge / SEAM).min(1.0);
                    let i = (y0 + y) * SIZE + (start + x_run + x) % SIZE;
                    let mut z = seam * (0.7 + lift);
                    let mut dark = 0.3 + 0.7 * seam;
                    let (fx, fy) = (x as f32, y as f32);
                    if style < 0.4 {
                        // A row of bolts along the top and bottom edges.
                        let along = (fx - 12.0).rem_euclid(26.0) - 13.0;
                        for by in [9.0, h as f32 - 10.0] {
                            let d = (along * along + (fy - by).powi(2)).sqrt();
                            if d < 2.6 && fx > 6.0 && fx < w as f32 - 7.0 {
                                z += (1.0 - d / 2.6) * 0.3;
                                dark = dark.min(0.55 + 0.45 * (d / 2.6));
                            }
                        }
                    } else if style < 0.55 && w >= 104 && h >= 80 {
                        // An inset access cover.
                        let (cx, cy, hw, hh) = (
                            w as f32 * 0.5,
                            h as f32 * 0.5,
                            w as f32 * 0.24,
                            h as f32 * 0.26,
                        );
                        let d = ((fx - cx).abs() - hw).max((fy - cy).abs() - hh);
                        if d.abs() < 1.5 {
                            z -= 0.35 * (1.0 - d.abs() / 1.5);
                            dark = dark.min(0.45 + 0.55 * d.abs() / 1.5);
                        }
                    }
                    height[i] = z;
                    cavity[i] = dark;
                }
            }
            x_run += w;
            index += 1;
        }
        y0 += h;
    }
    let mut rgba = pack(&height, &cavity, 3.0);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (u, v) = (x as f32 / SIZE as f32, y as f32 / SIZE as f32);
            // Broad dirt with fine scuffs on top.
            let wear = fbm(u, v, 41) * 0.75 + value_noise(u * 64.0, v * 64.0, 64, 43) * 0.25;
            let i = (y * SIZE + x) * 4;
            rgba[i + 2] = (cavity[y * SIZE + x] * 255.0) as u8;
            rgba[i + 3] = (wear.clamp(0.0, 1.0) * 255.0) as u8;
        }
    }
    rgba
}

/// CC0 scanned ground materials, packed offline by scripts/import-terrain-materials.py,
/// in texture-array order. Each is two layers: linear albedo with roughness in
/// alpha, then tangent normal XY, scanned height and occlusion. Keep the order
/// in step with the material constants in bindings.wgsl.
pub const GROUND: [&str; 10] = [
    "rock_face",
    "leafy_grass",
    "aerial_grass_rock",
    "forest_leaves_02",
    "forrest_ground_01",
    "rocky_trail",
    "dry_ground_rocks",
    "aerial_rocks_02",
    "gravelly_sand",
    "brown_mud_leaves_01",
];

/// First texture-array layer after the ground materials (`FOLIAGE_BASE` in bindings.wgsl).
pub const FOLIAGE_BASE: usize = GROUND.len() * 2;

macro_rules! ground_layers {
    ($($name:literal),*) => {
        [$(
            include_bytes!(concat!("../../../data/textures/terrain/", $name, "_color.rgba")).as_slice(),
            include_bytes!(concat!("../../../data/textures/terrain/", $name, "_detail.rgba")).as_slice(),
        )*]
    };
}

/// Every layer of the terrain material array, and whether it is an alpha
/// cutout that needs coverage-preserving mips. Embedding keeps packaged
/// builds independent of their working directory.
pub fn terrain_materials() -> Vec<(Vec<u8>, bool)> {
    let ground = ground_layers!(
        "rock_face",
        "leafy_grass",
        "aerial_grass_rock",
        "forest_leaves_02",
        "forrest_ground_01",
        "rocky_trail",
        "dry_ground_rocks",
        "aerial_rocks_02",
        "gravelly_sand",
        "brown_mud_leaves_01"
    );
    let mut layers: Vec<(Vec<u8>, bool)> = ground.iter().map(|l| (l.to_vec(), false)).collect();
    layers.extend(crate::foliage::layers());
    layers
}

/// Preserve alpha-test coverage when leaves are minified. Ordinary averaged
/// alpha erases the crown at mid distance while leaving the bare trunk visible.
pub fn terrain_mips(base: &[u8], foliage: bool) -> Vec<(usize, Vec<u8>)> {
    let mut levels = mip_chain(base, SIZE);
    if !foliage { return levels; }
    let coverage = base.chunks_exact(4).filter(|p| p[3] >= 97).count() as f32 / (SIZE * SIZE) as f32;
    for (size, pixels) in &mut levels {
        if *size <= 2 {
            for pixel in pixels.chunks_exact_mut(4) { pixel[3] = 180; }
            continue;
        }
        let mut alpha: Vec<_> = pixels.chunks_exact(4).map(|p| p[3]).collect();
        alpha.sort_unstable_by(|a, b| b.cmp(a));
        let index = ((alpha.len() as f32 * coverage) as usize).min(alpha.len() - 1);
        let scale = 97.5 / alpha[index].max(1) as f32;
        for pixel in pixels.chunks_exact_mut(4) {
            pixel[3] = (pixel[3] as f32 * scale).min(255.0) as u8;
        }
    }
    levels
}

/// Pulls coarser mips toward 0.5 so a tiled noise map does not turn into a
/// grid of big texels when the GPU minifies it.
pub fn flatten_noise_mips(levels: &mut [(usize, Vec<u8>)]) {
    for (i, (_, pixels)) in levels.iter_mut().enumerate() {
        let fade = ((i as f32 - 1.0) / 5.0).clamp(0.0, 1.0);
        if fade <= 0.0 {
            continue;
        }
        let t = fade * fade;
        for p in pixels.iter_mut() {
            *p = (*p as f32 * (1.0 - t) + 127.5 * t).round() as u8;
        }
    }
}

/// Box-filtered mip chain of an RGBA8 image, starting with level 1.
pub fn mip_chain(base: &[u8], size: usize) -> Vec<(usize, Vec<u8>)> {
    let mut levels = Vec::new();
    let mut prev = base.to_vec();
    let mut s = size;
    while s > 1 {
        let n = s / 2;
        let mut next = vec![0u8; n * n * 4];
        for y in 0..n {
            for x in 0..n {
                for c in 0..4 {
                    let sum: u32 = [(0, 0), (1, 0), (0, 1), (1, 1)]
                        .iter()
                        .map(|(dx, dy)| prev[((y * 2 + dy) * s + x * 2 + dx) * 4 + c] as u32)
                        .sum();
                    next[(y * n + x) * 4 + c] = (sum / 4) as u8;
                }
            }
        }
        levels.push((n, next.clone()));
        prev = next;
        s = n;
    }
    levels
}

/// The overlay's atlas, RGBA8. The 8x8 bitmap font (16x8 glyph cells of 8 px,
/// ASCII 0..128) sits in the top-left corner; the overlay packs outline glyphs
/// and images into the rest (see `overlay.rs`). White with coverage in alpha,
/// so one shader path serves glyphs and images alike.
pub const FONT_ATLAS_W: usize = 2048;
pub const FONT_ATLAS_H: usize = 2048;
pub const BITMAP_FONT_H: usize = 64;

pub fn font_atlas() -> Vec<u8> {
    // Transparent white, not transparent black: scaled text is filtered against
    // its surroundings, and black would bleed into the edges of every glyph.
    let mut out: Vec<u8> = [255, 255, 255, 0].repeat(FONT_ATLAS_W * FONT_ATLAS_H);
    for (code, glyph) in font8x8::legacy::BASIC_LEGACY.iter().enumerate() {
        let (gx, gy) = ((code % 16) * 8, (code / 16) * 8);
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..8 {
                if bits & (1 << col) != 0 {
                    let at = ((gy + row) * FONT_ATLAS_W + gx + col) * 4;
                    out[at..at + 4].fill(255);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scanned_materials_have_valid_normals_and_surface_variation() {
        let layers = terrain_materials();
        assert_eq!(layers.len(), FOLIAGE_BASE + crate::foliage::LAYERS);
        for (layer, _) in &layers { assert_eq!(layer.len(), SIZE * SIZE * 4); }
        for (i, name) in GROUND.iter().enumerate() {
            let color = &layers[i * 2].0;
            let detail = &layers[i * 2 + 1].0;
            for (layer, channel, what) in [(color, 3, "roughness"), (detail, 3, "occlusion"), (detail, 2, "height")] {
                let low = layer.chunks_exact(4).map(|p| p[channel]).min().unwrap();
                let high = layer.chunks_exact(4).map(|p| p[channel]).max().unwrap();
                assert!(high - low > 10, "{name}: {what} was clamped during import");
            }
            for pixel in detail.chunks_exact(4) {
                let (x, y) = (pixel[0] as f32 / 127.5 - 1.0, pixel[1] as f32 / 127.5 - 1.0);
                assert!(x * x + y * y <= 1.02, "{name}: normal XY longer than one");
            }
            let lo = color.chunks_exact(4).map(|p| p[1]).min().unwrap();
            let hi = color.chunks_exact(4).map(|p| p[1]).max().unwrap();
            assert!(hi - lo > 20, "{name}: material lost its albedo detail");
        }
    }

    #[test]
    fn noise_tiles_seamlessly() {
        for v in [0.13, 0.5, 0.77] {
            assert!((fbm(0.0, v, 3) - fbm(1.0, v, 3)).abs() < 1e-4);
            assert!((fbm(v, 0.0, 3) - fbm(v, 1.0, 3)).abs() < 1e-4);
        }
    }

    #[test]
    fn mips_halve_down_to_one() {
        let chain = mip_chain(&vec![200u8; 16 * 16 * 4], 16);
        assert_eq!(
            chain.iter().map(|l| l.0).collect::<Vec<_>>(),
            vec![8, 4, 2, 1]
        );
        assert!(chain.iter().all(|(_, d)| d.iter().all(|&b| b == 200)));
    }

    #[test]
    fn flattened_noise_mips_reach_mid_grey() {
        let mut chain = mip_chain(&noise_map(), SIZE);
        flatten_noise_mips(&mut chain);
        let last = &chain.last().unwrap().1;
        assert!(last.iter().all(|&b| (b as i16 - 128).abs() <= 1));
    }

    #[test]
    fn noise_wrap_matches_interior_slopes() {
        let map = noise_map();
        let height = |x: usize, y: usize| map[(y % SIZE * SIZE + x % SIZE) * 4 + 2] as i32;
        let mut seam = 0i32;
        let mut interior = 0i32;
        for y in 0..SIZE {
            seam = seam.max((height(0, y) - height(SIZE - 1, y)).abs());
            interior = interior.max((height(SIZE / 2, y) - height(SIZE / 2 + 1, y)).abs());
        }
        for x in 0..SIZE {
            seam = seam.max((height(x, 0) - height(x, SIZE - 1)).abs());
            interior = interior.max((height(x, SIZE / 2) - height(x, SIZE / 2 + 1)).abs());
        }
        assert!(
            seam <= interior + 3,
            "wrap step {seam} is a line next to interior {interior}"
        );
    }

    fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
        ((x - e0) / (e1 - e0)).clamp(0.0, 1.0)
    }

    fn close_grass(wx: f32, wy: f32) -> [f32; 3] {
        let gn = |x: f32, y: f32, cell: f32, seed: u32| {
            gradient_noise(x / cell, y / cell, 8192, seed) * 0.5 + 0.5
        };
        let tuft = gn(wx, wy, 0.72, 1);
        let soil = gn(wx + 19.2, wy + 7.4, 0.31, 2);
        let clump = smooth(0.28, 0.62, tuft);
        let along = (wx * 0.55 + wy * 0.22, wx * -0.28 + wy * 1.15);
        let across = (wx * 1.12 - wy * 0.35, wx * 0.40 + wy * 0.58);
        let fibre = gn(along.0, along.1, 0.09, 3) * (1.0 - gn(wx, wy, 1.35, 8))
            + gn(across.0, across.1, 0.09, 4) * gn(wx, wy, 1.35, 8);
        let blades = smooth(0.38, 0.78, fibre) * clump;
        let blades2 = smooth(0.50, 0.86, gn(wx + 3.1, wy + 8.8, 0.13, 5)) * clump;
        let speck = gn(wx + 4.6, wy + 11.3, 0.07, 6);
        let pebble = smooth(0.74, 0.90, gn(wx + 27.0, wy + 3.2, 0.18, 7));
        let bare = smooth(0.46, 0.74, soil) * (1.0 - clump * 0.65);
        let w = gn(wx, wy, 5.6, 9) * 0.34 + gn(wx, wy, 1.55, 10) * 0.30 + tuft * 0.18;
        let mix3 = |a: [f32; 3], b: [f32; 3], t: f32| {
            [
                a[0] * (1.0 - t) + b[0] * t,
                a[1] * (1.0 - t) + b[1] * t,
                a[2] * (1.0 - t) + b[2] * t,
            ]
        };
        let mut g = mix3([0.038, 0.085, 0.022], [0.14, 0.21, 0.055], w);
        g = mix3(g, [0.07, 0.145, 0.035], blades * 0.7);
        g = mix3(g, [0.19, 0.20, 0.05], blades2 * 0.45);
        g = mix3(g, [0.13, 0.09, 0.04], bare * 0.75);
        g = mix3(g, [0.26, 0.23, 0.18], pebble);
        let grit = 0.72 + speck * 0.45;
        [g[0] * grit, g[1] * grit, g[2] * grit]
    }

    /// Close-up grass, same octaves as the terrain shader, so a lattice of
    /// lines would show here the way it did on the ground.
    #[test]
    fn close_up_grass_has_no_axis_lattice() {
        let n = 256usize;
        let metres = 8.0;
        let mut px = vec![0u8; n * n * 3];
        for y in 0..n {
            for x in 0..n {
                let (wx, wy) = (x as f32 / n as f32 * metres, y as f32 / n as f32 * metres);
                let g = close_grass(wx, wy);
                let lit = g.map(|c| ((c * 3.4).clamp(0.0, 1.0) * 255.0) as u8);
                let i = (y * n + x) * 3;
                px[i..i + 3].copy_from_slice(&lit);
            }
        }
        let mut dx = 0u32;
        let mut dy = 0u32;
        for y in 1..n - 1 {
            for x in 1..n - 1 {
                let i = (y * n + x) * 3;
                dx += px[i].abs_diff(px[i - 3]) as u32;
                dy += px[i].abs_diff(px[i - n * 3]) as u32;
            }
        }
        let ratio = dx.max(dy) as f32 / dx.min(dy).max(1) as f32;
        assert!(
            ratio < 1.35,
            "axis-aligned lattice: dx {dx} dy {dy} ratio {ratio}"
        );
    }
}
