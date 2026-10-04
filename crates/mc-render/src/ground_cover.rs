//! What grows on the ground, from the map's trees: the terrain shader lays
//! forest floor and canopy shade where the forests actually stand, instead of
//! guessing from noise that has nothing to do with where the baker put them.
//! A map with a snow layer adds its glacier ice and lying snow, one with a
//! ways layer the trails trodden into it, one with a streets layer its city's
//! roads, pavements, yards and fields.

use crate::keep::{kept, Kept};
use mc_map::{MapFile, PropKind};
use std::sync::Mutex;

/// Longest edge of the cover map in texels. Small maps get 8 m texels; the
/// 80 km basin gets 20 m ones.
const MAX_TEXELS: usize = 4096;
const MIN_CELL_M: f32 = 8.0;

#[derive(Clone)]
pub struct GroundCover {
    pub width: u32,
    pub height: u32,
    /// RGBA8: r = canopy (how closed the forest is overhead), g = how much of
    /// that canopy is conifer, b = glacier ice, a = lying snow (1 to 255 on a
    /// map with a snow layer, 0 on one without).
    pub texels: Vec<u8>,
    /// The second layer, the same size, RGBA8, from the map's ways layer: r =
    /// how trodden the ground is (0 everywhere on a map without one), g, b =
    /// the trail's heading as cos 2a, sin 2a (0.5 = 0); a = how much of a crag
    /// the ground is (`cliff_blocks::crag_field`).
    pub ways: Vec<u8>,
    /// The third layer, the same size, RGBA8, from the map's streets layer
    /// (`mc_map::StreetSample`) moved to the texels' middles: r = signed
    /// offset from the nearest road's centreline and g = its half width,
    /// both in quarter metres and blended, so they filter; b = the road's
    /// kind and junction flag, a = the ground's kind and how battered it is,
    /// each one sample's own (read unfiltered). All zero on a map without
    /// one: ground of the terrain's own on a road of no kind.
    pub streets: Vec<u8>,
}

/// A tree's crown radius at scale 1 in metres, and how much of it is conifer
/// (needle litter under it): the tree models' own reach, for everything that
/// maps canopy from the props (the ground cover here, the game's ambience and
/// map previews).
pub fn crown_of(kind: PropKind) -> (f32, f32) {
    use crate::models::{COTTONWOOD_REACH, JUNIPER_REACH, PINYON_REACH};
    match kind {
        PropKind::TreeBroadleaf => (6.0, 0.0),
        PropKind::TreeConifer => (3.9, 1.0),
        PropKind::TreePine => (5.2, 1.0),
        PropKind::TreePalm => (4.5, 0.0),
        PropKind::TreeJungle => (9.0, 0.0),
        PropKind::TreeJuniper => (JUNIPER_REACH, 1.0),
        PropKind::TreePinyon => (PINYON_REACH, 1.0),
        PropKind::TreeCottonwood => (COTTONWOOD_REACH, 0.0),
        _ => (1.5, 0.0),
    }
}

pub fn ground_cover(map: &MapFile) -> GroundCover {
    static KEPT: Kept<GroundCover> = Mutex::new(Vec::new());
    kept(&KEPT, map.content_id(), || ground_cover_made(map))
}

fn ground_cover_made(map: &MapFile) -> GroundCover {
    let size = map.info().size_metres().to_f32();
    let cell = (size[0].max(size[1]) / MAX_TEXELS as f32).max(MIN_CELL_M);
    let w = ((size[0] / cell).ceil() as usize).max(1);
    let h = ((size[1] / cell).ceil() as usize).max(1);
    let mut crown = vec![0f32; w * h];
    let mut needle = vec![0f32; w * h];
    for p in map.props().iter().filter(|p| p.kind.is_tree()) {
        let xy = p.pos.to_f32();
        let scale = p.scale_milli as f32 / 1000.0;
        let (reach, conifer) = crown_of(p.kind);
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
        texels[i * 4 + 1] =
            ((needle[i] / crown[i].max(1e-4)).clamp(0.0, 1.0) * 255.0).round() as u8;
    }
    if let Some(snow) = map.snow() {
        let (sw, sh) = map.info().snow_dims();
        let pitch = (mc_map::CELL_SIZE_M as u32 * mc_map::format::SNOW_STRIDE) as f32;
        let ice: Vec<u8> = snow.iter().step_by(2).copied().collect();
        let lying: Vec<u8> = snow.iter().skip(1).step_by(2).copied().collect();
        resample(&ice, (sw, sh), pitch, cell, (w, h), |i, v| {
            texels[i * 4 + 2] = v.round() as u8;
        });
        resample(&lying, (sw, sh), pitch, cell, (w, h), |i, v| {
            // Snow is kept off zero, so the shader can tell a map with a snow
            // layer (it lays only the layer's snow) from one without (it lays
            // snow by height).
            texels[i * 4 + 3] = (1.0 + v * 254.0 / 255.0).round() as u8;
        });
    }
    let mut ways = vec![0u8; w * h * 4];
    if let Some(layer) = map.ways() {
        let (sw, sh) = map.info().snow_dims();
        let pitch = (mc_map::CELL_SIZE_M as u32 * mc_map::format::SNOW_STRIDE) as f32;
        for c in 0..3 {
            let channel: Vec<u8> = layer.iter().skip(c).step_by(3).copied().collect();
            resample(&channel, (sw, sh), pitch, cell, (w, h), |i, v| {
                ways[i * 4 + c] = v.round() as u8;
            });
        }
    }
    for (i, crag) in crate::cliff_blocks::crag_field(map, cell, (w, h))
        .into_iter()
        .enumerate()
    {
        ways[i * 4 + 3] = (crag * 255.0).round() as u8;
    }
    let streets = match map.streets() {
        Some(layer) => streets_at_texels(layer, map.info().streets_dims(), cell, (w, h)),
        None => vec![0u8; w * h * 4],
    };
    GroundCover {
        width: w as u32,
        height: h as u32,
        texels,
        ways,
        streets,
    }
}

/// The streets layer's samples (one per height sample, `CELL_SIZE_M` apart)
/// at the middles of `(w, h)` texels `cell` metres across: the offsets
/// blended from the four samples round each middle (exact for a straight
/// road), the kinds of the nearest of those to a road, the ground's kind by
/// most of them, the most battered.
fn streets_at_texels(
    layer: &[u8],
    (sw, sh): (u32, u32),
    cell: f32,
    (w, h): (usize, usize),
) -> Vec<u8> {
    let pitch = mc_map::CELL_SIZE_M as f32;
    let at = |x: usize, y: usize| {
        let i = (y.min(sh as usize - 1) * sw as usize + x.min(sw as usize - 1)) * 4;
        [layer[i], layer[i + 1], layer[i + 2], layer[i + 3]]
    };
    let mut out = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let (sx, sy) = (
                (x as f32 + 0.5) * cell / pitch,
                (y as f32 + 0.5) * cell / pitch,
            );
            let (x0, y0) = (sx.floor() as usize, sy.floor() as usize);
            let (fx, fy) = (sx.fract(), sy.fract());
            let corners = [
                at(x0, y0),
                at(x0 + 1, y0),
                at(x0, y0 + 1),
                at(x0 + 1, y0 + 1),
            ];
            let weights = [
                (1.0 - fx) * (1.0 - fy),
                fx * (1.0 - fy),
                (1.0 - fx) * fy,
                fx * fy,
            ];
            let blend = |c: usize| {
                corners
                    .iter()
                    .zip(weights)
                    .map(|(s, wt)| s[c] as f32 * wt)
                    .sum::<f32>()
                    .round() as u8
            };
            let nearest = corners
                .iter()
                .min_by_key(|s| s[0].abs_diff(128))
                .copied()
                .unwrap_or_default();
            let junction = corners.iter().any(|s| s[2] & 0x80 != 0) as u8;
            let ground = corners
                .iter()
                .map(|s| s[3] & 0x0F)
                .max_by_key(|g| {
                    (
                        corners.iter().filter(|s| s[3] & 0x0F == *g).count(),
                        u8::MAX - g,
                    )
                })
                .unwrap_or(0);
            let battered = corners.iter().map(|s| s[3] >> 4).max().unwrap_or(0);
            let i = (y * w + x) * 4;
            // Where the nearest road changes between the samples, their offsets
            // jump: blended, they would make a false centreline. The nearest
            // road's own sample stands instead.
            let (lo, hi) = corners
                .iter()
                .fold((u8::MAX, 0), |(lo, hi), s| (lo.min(s[0]), hi.max(s[0])));
            let jump = (hi - lo) as f32 * 0.25 > 1.5 * pitch;
            out[i] = if jump { nearest[0] } else { blend(0) };
            out[i + 1] = if jump { nearest[1] } else { blend(1) };
            out[i + 2] = nearest[2] & 0x7F | junction << 7;
            out[i + 3] = ground | battered << 4;
        }
    }
    out
}

/// A layer of `(sw, sh)` byte samples `pitch` metres apart, read bilinearly at
/// the centre of each of the `(w, h)` texels `cell` metres across; `put` gets
/// each texel's index and value (0 to 255).
fn resample(
    layer: &[u8],
    (sw, sh): (u32, u32),
    pitch: f32,
    cell: f32,
    (w, h): (usize, usize),
    mut put: impl FnMut(usize, f32),
) {
    let at = |x: usize, y: usize| {
        layer[y.min(sh as usize - 1) * sw as usize + x.min(sw as usize - 1)] as f32
    };
    for y in 0..h {
        for x in 0..w {
            let (sx, sy) = (
                (x as f32 + 0.5) * cell / pitch,
                (y as f32 + 0.5) * cell / pitch,
            );
            let (x0, y0) = (sx.floor() as usize, sy.floor() as usize);
            let (fx, fy) = (sx.fract(), sy.fract());
            let v = (at(x0, y0) * (1.0 - fx) + at(x0 + 1, y0) * fx) * (1.0 - fy)
                + (at(x0, y0 + 1) * (1.0 - fx) + at(x0 + 1, y0 + 1) * fx) * fy;
            put(y * w + x, v);
        }
    }
}

/// Two box passes each way: close enough to a Gaussian for a ground mask.
pub(crate) fn blur(v: &mut [f32], w: usize, h: usize, r: usize) {
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

    /// The shader's street kinds are the map format's.
    #[test]
    fn street_kinds_match_the_map_format() {
        use crate::gpu_consts::streets as g;
        use mc_map::{Ground, Road};
        let roads = [
            (Road::None, g::ROAD_NONE),
            (Road::Street, g::ROAD_STREET),
            (Road::Avenue, g::ROAD_AVENUE),
            (Road::Highway, g::ROAD_HIGHWAY),
            (Road::Lane, g::ROAD_LANE),
            (Road::Rail, g::ROAD_RAIL),
        ];
        for (road, id) in roads {
            assert_eq!(road as u32, id, "{road:?}");
            assert_eq!(road as u32 & g::JUNCTION, 0);
        }
        let grounds = [
            (Ground::Natural, g::GROUND_NATURAL),
            (Ground::Paving, g::GROUND_PAVING),
            (Ground::Lawn, g::GROUND_LAWN),
            (Ground::Yard, g::GROUND_YARD),
            (Ground::Ballast, g::GROUND_BALLAST),
            (Ground::Field, g::GROUND_FIELD),
            (Ground::Rubble, g::GROUND_RUBBLE),
            (Ground::Earth, g::GROUND_EARTH),
            (Ground::Apron, g::GROUND_APRON),
        ];
        for (ground, id) in grounds {
            assert_eq!(ground as u32, id, "{ground:?}");
            assert!(id <= g::KIND_MASK);
        }
        let sample = mc_map::StreetSample {
            junction: true,
            battered: 9,
            ..mc_map::StreetSample::NATURAL
        };
        let b = sample.bytes();
        assert_eq!(b[2] as u32 & g::JUNCTION, g::JUNCTION);
        assert_eq!(b[3] as u32 >> g::BATTERED_SHIFT, 9);
        assert_eq!(g::FIELD_CELL as f64, mc_map::format::FIELD_CELL_M);
    }

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
