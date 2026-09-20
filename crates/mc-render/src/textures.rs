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
    let mut period = 8u32;
    for octave in 0..8 {
        let shift = octave as f32 * 17.37;
        sum += amp
            * gradient_noise(
                u * period as f32 + shift,
                v * period as f32 - shift,
                period,
                seed + octave,
            );
        norm += amp;
        amp *= 0.5;
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
            let (u, v) = (x as f32 / SIZE as f32, y as f32 / SIZE as f32);
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
}
