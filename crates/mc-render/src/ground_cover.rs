//! What grows on the ground, from the map's trees: the terrain shader lays
//! forest floor and canopy shade where the forests actually stand, instead of
//! guessing from noise that has nothing to do with where the baker put them.
//! A map with a snow layer adds its glacier ice and lying snow.

use mc_map::{MapFile, PropKind};

/// Longest edge of the cover map in texels. Small maps get 8 m texels; the
/// 80 km basin gets 20 m ones.
const MAX_TEXELS: usize = 4096;
const MIN_CELL_M: f32 = 8.0;

pub struct GroundCover {
    pub width: u32,
    pub height: u32,
    /// RGBA8: r = canopy (how closed the forest is overhead), g = how much of
    /// that canopy is conifer, b = glacier ice, a = lying snow (1 to 255 on a
    /// map with a snow layer, 0 on one without).
    pub texels: Vec<u8>,
}

pub fn ground_cover(map: &MapFile) -> GroundCover {
    let size = map.info().size_metres().to_f32();
    let cell = (size[0].max(size[1]) / MAX_TEXELS as f32).max(MIN_CELL_M);
    let w = ((size[0] / cell).ceil() as usize).max(1);
    let h = ((size[1] / cell).ceil() as usize).max(1);
    let mut crown = vec![0f32; w * h];
    let mut needle = vec![0f32; w * h];
    for p in map.props().iter().filter(|p| p.kind.is_tree()) {
        let xy = p.pos.to_f32();
        let scale = p.scale_milli as f32 / 1000.0;
        let (reach, conifer) = match p.kind {
            // Crown radii of the tree models (props.rs), in metres at scale 1.
            PropKind::TreeBroadleaf => (6.0, 0.0),
            PropKind::TreeConifer => (3.9, 1.0),
            PropKind::TreePine => (5.2, 1.0),
            _ => (1.5, 0.0),
        };
        // Each crown adds its footprint in texels; a closed forest sums past one.
        let area = std::f32::consts::PI * (reach * scale).powi(2) / (cell * cell);
        let (x, y) = ((xy[0] / cell) as usize, (xy[1] / cell) as usize);
        if x < w && y < h {
            crown[y * w + x] += area;
            needle[y * w + x] += area * conifer;
        }
    }
    // Spread each crown over the texels around it: a forest floor reaches a
    // little past the outermost trunks.
    let radius = ((10.0 / cell).round() as usize).max(1);
    blur(&mut crown, w, h, radius);
    blur(&mut needle, w, h, radius);
    let mut texels = vec![0u8; w * h * 4];
    for i in 0..w * h {
        let canopy = 1.0 - (-crown[i] * 1.6).exp();
        texels[i * 4] = (canopy * 255.0).round() as u8;
        texels[i * 4 + 1] = ((needle[i] / crown[i].max(1e-4)).clamp(0.0, 1.0) * 255.0).round() as u8;
    }
    if let Some(snow) = map.snow() {
        let (sw, sh) = map.info().snow_dims();
        let pitch = (mc_map::CELL_SIZE_M as u32 * mc_map::format::SNOW_STRIDE) as f32;
        let at = |x: usize, y: usize, c: usize| snow[(y.min(sh as usize - 1) * sw as usize + x.min(sw as usize - 1)) * 2 + c] as f32;
        for y in 0..h {
            for x in 0..w {
                // Texel centre in snow samples, bilinear between the four round it.
                let (sx, sy) = (((x as f32 + 0.5) * cell / pitch), ((y as f32 + 0.5) * cell / pitch));
                let (x0, y0) = (sx.floor() as usize, sy.floor() as usize);
                let (fx, fy) = (sx.fract(), sy.fract());
                for c in 0..2 {
                    let v = (at(x0, y0, c) * (1.0 - fx) + at(x0 + 1, y0, c) * fx) * (1.0 - fy)
                        + (at(x0, y0 + 1, c) * (1.0 - fx) + at(x0 + 1, y0 + 1, c) * fx) * fy;
                    // Snow is kept off zero, so the shader can tell a map
                    // with a snow layer (it lays only the layer's snow) from
                    // one without (it lays snow by height).
                    let v = if c == 1 { 1.0 + v * 254.0 / 255.0 } else { v };
                    texels[(y * w + x) * 4 + 2 + c] = v.round() as u8;
                }
            }
        }
    }
    GroundCover { width: w as u32, height: h as u32, texels }
}

/// Two box passes each way: close enough to a Gaussian for a ground mask.
fn blur(v: &mut [f32], w: usize, h: usize, r: usize) {
    let mut tmp = vec![0f32; v.len()];
    for _ in 0..2 {
        for y in 0..h {
            box_line(&v[y * w..(y + 1) * w], &mut tmp[y * w..(y + 1) * w], r);
        }
        let mut col = vec![0f32; h];
        let mut out = vec![0f32; h];
        for x in 0..w {
            for y in 0..h {
                col[y] = tmp[y * w + x];
            }
            box_line(&col, &mut out, r);
            for y in 0..h {
                v[y * w + x] = out[y];
            }
        }
    }
}

fn box_line(src: &[f32], dst: &mut [f32], r: usize) {
    let n = src.len();
    let norm = 1.0 / (2 * r + 1) as f32;
    let mut sum: f32 = src[..(r + 1).min(n)].iter().sum();
    for i in 0..n {
        dst[i] = sum * norm;
        if i + r + 1 < n {
            sum += src[i + r + 1];
        }
        if i >= r {
            sum -= src[i - r];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_blur_keeps_the_total_away_from_edges() {
        let mut v = vec![0f32; 64 * 64];
        v[32 * 64 + 32] = 1.0;
        blur(&mut v, 64, 64, 3);
        let total: f32 = v.iter().sum();
        assert!((total - 1.0).abs() < 1e-4, "{total}");
        assert!(v[32 * 64 + 32] > v[32 * 64 + 36]);
    }
}
