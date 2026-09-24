# Tree foliage and bark

CC0 assets from Poly Haven (license: https://polyhaven.com/license, CC0 1.0):

- Searsia Lucida: https://polyhaven.com/a/searsia_lucida — Jenelle van Heerden (photography), James Ray Cock (modeling). Leaves.
- Tree Small 02: https://polyhaven.com/a/tree_small_02 — Rico Cilliers. Leaves.
- Fir Tree 01: https://polyhaven.com/a/fir_tree_01 — Rob Tuytel (photography), Rico Cilliers (modeling). Fir twigs.
- Pine Tree 01: https://polyhaven.com/a/pine_tree_01 — Rob Tuytel (photography), Rico Cilliers (modeling). Pine needle sprigs.
- Bark Brown 02: https://polyhaven.com/a/bark_brown_02 — Rob Tuytel. Broadleaf bark.
- Pine Bark: https://polyhaven.com/a/pine_bark — Dimitrios Savva. Conifer bark.

`python3 scripts/import-foliage.py` downloads the 1K PNGs (cached in `target/foliage-sources`, checked against
the MD5s in `sources.json`) and writes six 512x512 RGBA8 layers. Requires Pillow and NumPy. The leaf atlases
are composed, not cropped: single leaves and twigs are cut out of the scans and stamped (seeded, so reruns match)
along drawn twigs, each leaf graded to temperate greens and shading the leaves under it.

`crates/mc-render/src/foliage.rs` embeds the layers in this order, after the ground materials (`FOLIAGE_BASE + k`):

| k | file | contents |
|---|---|---|
| 0 | `broadleaf.rgba` | cutout: linear albedo, coverage in A. Quadrants: two round leaf clusters, a branch end growing up from the bottom edge, a dense lobed clump |
| 1 | `conifer.rgba` | cutout: top half a flat fir branch seen from above (trunk end on the left); bottom left a pine needle tuft seen from above; bottom right a whole young fir from the side |
| 2 | `bark_color.rgba` | Bark Brown 02: linear albedo, roughness in A (1 m tile) |
| 3 | `bark_normal.rgba` | renormalised OpenGL tangent-space normal, occlusion in A |
| 4 | `pine_bark_color.rgba` | Pine Bark, as layer 2 (2 m tile) |
| 5 | `pine_bark_normal.rgba` | as layer 3 |

Cutout layers carry the colour of the nearest leaves into their transparent texels (push-pull fill), so
filtering and mipmaps never darken the edges; the renderer builds their mips with `textures::terrain_mips(_, true)`,
which keeps alpha-test coverage constant as the crown shrinks with distance.
