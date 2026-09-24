//! Tree foliage and bark: the texture-array layers after the ground materials
//! (`FOLIAGE_BASE + k` in bindings.wgsl, k the index in [`layers`]). Composed
//! offline from CC0 Poly Haven scans by `scripts/import-foliage.py`; credits and
//! channel layouts are in `data/textures/foliage/README.md`. Embedded so packaged
//! builds do not depend on their working directory.
//!
//! The two leaf atlases are cutouts: linear albedo, antialiased coverage in
//! alpha, colour bled into the gaps so filtering never pulls in black. The tree
//! models pick a region of an atlas with their card UVs ([`BROADLEAF_REGIONS`],
//! [`CONIFER_REGIONS`]; u right, v down, as the image is stored).

use crate::textures::SIZE;

/// Layer indices relative to `FOLIAGE_BASE`. entity.wgsl mirrors them.
pub const BROADLEAF: usize = 0;
pub const CONIFER: usize = 1;
/// Broadleaf bark: linear albedo + roughness, then OpenGL normal + occlusion.
pub const BARK: usize = 2;
pub const BARK_NORMAL: usize = 3;
/// Pine / fir bark, same layout.
pub const PINE_BARK: usize = 4;
pub const PINE_BARK_NORMAL: usize = 5;
pub const LAYERS: usize = 6;

/// Broadleaf atlas quadrants `[u0, v0, u1, v1]`: two round clusters (twigs
/// radiating from the middle), a branch end growing up from the bottom edge, and
/// a dense lobed clump for distant crowns.
pub const BROADLEAF_REGIONS: [[f32; 4]; 4] = [
    [0.0, 0.0, 0.5, 0.5],
    [0.5, 0.0, 1.0, 0.5],
    [0.0, 0.5, 0.5, 1.0],
    [0.5, 0.5, 1.0, 1.0],
];
/// Conifer atlas: a flat fir branch seen from above (trunk end at u0, 2:1), a
/// pine needle tuft seen from above, and a whole young fir from the side.
pub const CONIFER_REGIONS: [[f32; 4]; 3] = [
    [0.0, 0.0, 1.0, 0.5],
    [0.0, 0.5, 0.5, 1.0],
    [0.5, 0.5, 1.0, 1.0],
];

/// Every foliage layer, `SIZE`x`SIZE` RGBA8, in `FOLIAGE_BASE + k` order, with
/// `true` for the alpha-cutout layers that need coverage-preserving mips
/// (`textures::terrain_mips(layer, true)`).
pub fn layers() -> Vec<(Vec<u8>, bool)> {
    let layers = vec![
        (include_bytes!("../../../data/textures/foliage/broadleaf.rgba").to_vec(), true),
        (include_bytes!("../../../data/textures/foliage/conifer.rgba").to_vec(), true),
        (include_bytes!("../../../data/textures/foliage/bark_color.rgba").to_vec(), false),
        (include_bytes!("../../../data/textures/foliage/bark_normal.rgba").to_vec(), false),
        (include_bytes!("../../../data/textures/foliage/pine_bark_color.rgba").to_vec(), false),
        (include_bytes!("../../../data/textures/foliage/pine_bark_normal.rgba").to_vec(), false),
    ];
    debug_assert!(layers.len() == LAYERS && layers.iter().all(|(l, _)| l.len() == SIZE * SIZE * 4));
    layers
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::textures::terrain_mips;

    /// Share of texels in `region` of a layer that pass the shader's alpha test.
    fn coverage(pixels: &[u8], size: usize, region: [f32; 4]) -> f32 {
        let (x0, y0) = ((region[0] * size as f32) as usize, (region[1] * size as f32) as usize);
        let (x1, y1) = ((region[2] * size as f32) as usize, (region[3] * size as f32) as usize);
        let mut covered = 0;
        for y in y0..y1 {
            for x in x0..x1 {
                covered += (pixels[(y * size + x) * 4 + 3] >= 97) as usize;
            }
        }
        covered as f32 / ((x1 - x0) * (y1 - y0)) as f32
    }

    #[test]
    fn layers_are_full_size_and_cutouts_come_first() {
        let layers = layers();
        assert_eq!(layers.len(), LAYERS);
        for (i, (layer, cutout)) in layers.iter().enumerate() {
            assert_eq!(layer.len(), SIZE * SIZE * 4);
            assert_eq!(*cutout, i == BROADLEAF || i == CONIFER, "layer {i}");
        }
    }

    #[test]
    fn foliage_has_air_gaps_and_no_dark_fringes() {
        let layers = layers();
        for (layer, regions) in [
            (BROADLEAF, &BROADLEAF_REGIONS[..]),
            (CONIFER, &CONIFER_REGIONS[..]),
        ] {
            let pixels = &layers[layer].0;
            for &region in regions {
                let fraction = coverage(pixels, SIZE, region);
                assert!((0.2..0.8).contains(&fraction), "layer {layer} {region:?}: coverage {fraction}");
            }
            // Colour is bled into the gaps: no black texels for filtering to pull in.
            let dark = pixels.chunks_exact(4).filter(|p| p[3] == 0 && p[1] < 3).count();
            assert!(dark < SIZE * SIZE / 200, "layer {layer}: {dark} black gap texels");
            // Leaves are green, even where the atlas is transparent.
            let (r, g, b) = pixels.chunks_exact(4).fold((0u64, 0u64, 0u64), |s, p| {
                (s.0 + p[0] as u64, s.1 + p[1] as u64, s.2 + p[2] as u64)
            });
            assert!(g > r && g > b, "layer {layer}: mean colour is not green");
        }
    }

    #[test]
    fn distant_leaf_mips_keep_the_crown() {
        let layers = layers();
        for layer in [BROADLEAF, CONIFER] {
            let base = &layers[layer].0;
            let whole = coverage(base, SIZE, [0.0, 0.0, 1.0, 1.0]);
            for (size, mip) in terrain_mips(base, true) {
                if size >= 8 {
                    let actual = coverage(&mip, size, [0.0, 0.0, 1.0, 1.0]);
                    assert!((actual - whole).abs() < 0.07, "layer {layer} mip {size}: {actual} versus {whole}");
                }
            }
        }
    }

    #[test]
    fn bark_has_valid_normals_and_detail() {
        let layers = layers();
        for (color, normal) in [(BARK, BARK_NORMAL), (PINE_BARK, PINE_BARK_NORMAL)] {
            for pixel in layers[normal].0.chunks_exact(4) {
                let n = [pixel[0], pixel[1], pixel[2]].map(|c| c as f32 / 127.5 - 1.0);
                let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                assert!((length - 1.0).abs() < 0.04 && n[2] > 0.0, "bark normal {n:?}");
            }
            let albedo = &layers[color].0;
            let lo = albedo.chunks_exact(4).map(|p| p[1]).min().unwrap();
            let hi = albedo.chunks_exact(4).map(|p| p[1]).max().unwrap();
            assert!(hi - lo > 20, "bark lost its albedo detail");
            let rough = albedo.chunks_exact(4).map(|p| p[3] as u32).sum::<u32>() / (SIZE * SIZE) as u32;
            assert!(rough > 120, "bark is too glossy: {rough}");
        }
    }
}
