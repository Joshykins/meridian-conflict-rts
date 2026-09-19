//! Procedural textures generated at start-up. Both tile seamlessly.

/// Texture edge in texels.
pub const SIZE: usize = 512;

fn hash(x: u32, y: u32, seed: u32) -> f32 {
    let mut h = x.wrapping_mul(0x85EB_CA6B) ^ y.wrapping_mul(0xC2B2_AE35) ^ seed.wrapping_mul(0x27D4_EB2F);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h & 0xFFFF) as f32 / 65535.0
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

fn fbm(u: f32, v: f32, seed: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut period = 4u32;
    for octave in 0..6 {
        sum += amp * value_noise(u * period as f32, v * period as f32, period, seed + octave);
        amp *= 0.5;
        period *= 2;
    }
    sum / 0.984
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

/// Armour plating: panels separated by grooves, some raised, with rivets.
/// rg = normal, b = cavity darkening (1 = clean plate), a = roughness variation.
pub fn panel_map() -> Vec<u8> {
    const CELLS: usize = 8;
    let cell = SIZE / CELLS;
    let mut height = vec![1.0f32; SIZE * SIZE];
    let mut cavity = vec![1.0f32; SIZE * SIZE];
    for cy in 0..CELLS {
        for cx in 0..CELLS {
            let r = hash(cx as u32, cy as u32, 5);
            // Some cells split into two or four sub-panels.
            let (nx, ny) = if r < 0.3 { (1, 1) } else if r < 0.55 { (2, 1) } else if r < 0.8 { (1, 2) } else { (2, 2) };
            for sy in 0..ny {
                for sx in 0..nx {
                    let (w, h) = (cell / nx, cell / ny);
                    let (x0, y0) = (cx * cell + sx * w, cy * cell + sy * h);
                    let lift = hash((x0 + 1) as u32, (y0 + 1) as u32, 9) * 0.35;
                    for y in 0..h {
                        for x in 0..w {
                            let edge = x.min(y).min(w - 1 - x).min(h - 1 - y) as f32;
                            let groove = (edge / 3.0).min(1.0);
                            let i = (y0 + y) * SIZE + x0 + x;
                            height[i] = groove * (0.65 + lift);
                            cavity[i] = 0.35 + 0.65 * groove;
                            // Rivets in the corners of larger panels.
                            if w > 24 && h > 24 {
                                for (rx, ry) in [(7.0, 7.0), (w as f32 - 8.0, 7.0), (7.0, h as f32 - 8.0), (w as f32 - 8.0, h as f32 - 8.0)] {
                                    let d = ((x as f32 - rx).powi(2) + (y as f32 - ry).powi(2)).sqrt();
                                    if d < 2.5 {
                                        height[i] += (1.0 - d / 2.5) * 0.25;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    let mut rgba = pack(&height, &cavity, 3.0);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let wear = fbm(x as f32 / SIZE as f32, y as f32 / SIZE as f32, 41);
            let i = (y * SIZE + x) * 4;
            rgba[i + 2] = (cavity[y * SIZE + x] * 255.0) as u8;
            rgba[i + 3] = (wear * 255.0) as u8;
        }
    }
    rgba
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
                    let sum: u32 = [(0, 0), (1, 0), (0, 1), (1, 1)].iter().map(|(dx, dy)| prev[((y * 2 + dy) * s + x * 2 + dx) * 4 + c] as u32).sum();
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

/// 8x8 bitmap font atlas: 16x8 glyph cells of 8 px, ASCII 0..128, R8.
pub const FONT_ATLAS_W: usize = 128;
pub const FONT_ATLAS_H: usize = 64;

pub fn font_atlas() -> Vec<u8> {
    let mut out = vec![0u8; FONT_ATLAS_W * FONT_ATLAS_H];
    for (code, glyph) in font8x8::legacy::BASIC_LEGACY.iter().enumerate() {
        let (gx, gy) = ((code % 16) * 8, (code / 16) * 8);
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..8 {
                if bits & (1 << col) != 0 {
                    out[(gy + row) * FONT_ATLAS_W + gx + col] = 255;
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
        assert_eq!(chain.iter().map(|l| l.0).collect::<Vec<_>>(), vec![8, 4, 2, 1]);
        assert!(chain.iter().all(|(_, d)| d.iter().all(|&b| b == 200)));
    }
}
