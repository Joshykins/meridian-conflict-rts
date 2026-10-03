# Metal scans for the plate

CC0 scans from ambientCG (license: https://docs.ambientcg.com/license/, CC0 1.0):

- Metal 054 A: https://ambientcg.com/view?id=Metal054A — worn iron: the Regency's grain, sheen and relief.
- Metal 054 B: https://ambientcg.com/view?id=Metal054B — scratched iron: only its scratches.
- Metal 009: https://ambientcg.com/view?id=Metal009 — brushed steel: ARC's grain, sheen and relief.
- Metal 037: https://ambientcg.com/view?id=Metal037 — scratched steel: only its long scratches.

`python3 scripts/import-metal.py` downloads the 1K JPG sets (cached in `target/metal-sources`, checked
against their MD5s) and writes two 512x512 RGBA8 layers per scan. Requires Pillow and NumPy.

`crates/mc-render/src/textures.rs` embeds them after the foliage layers, at
`gpu_consts::metal_scan::LAYER` (the Regency's `steel`) and `ARC_LAYER` (ARC's `arc`), each
with the layer after it:

| layer | file | contents |
|---|---|---|
| `LAYER` | `steel_color.rgba` | 054 A's linear albedo, roughness in A |
| `LAYER + 1` | `steel_normal.rgba` | 054 A's OpenGL tangent-space normal XY in RG; 054 B's scratches in B (0 none, 1 deepest) |
| `ARC_LAYER` | `arc_color.rgba` | 009's linear albedo, roughness in A |
| `ARC_LAYER + 1` | `arc_normal.rgba` | 009's OpenGL tangent-space normal XY in RG; 037's scratches in B |

metal.wgsl maps them onto the plate in model space (three planar projections blended by the
face's normal) and uses only how they vary about their mean: the grain, scratches and sheen,
never the scan's own colour. regency.wgsl finishes Regency plate and machinery with `steel`;
`arc_metal` lays `arc` under ARC's paint, its scratches cut through to bright steel.
