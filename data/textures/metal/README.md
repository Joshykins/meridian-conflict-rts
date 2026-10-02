# Steel for the Regency plate

CC0 scan from ambientCG (license: https://docs.ambientcg.com/license/, CC0 1.0):

- Metal 054 A: https://ambientcg.com/view?id=Metal054A — worn iron: the grain, sheen and relief.
- Metal 054 B: https://ambientcg.com/view?id=Metal054B — scratched iron: only its scratches.

`python3 scripts/import-metal.py` downloads the 1K JPG sets (cached in `target/metal-sources`, checked
against their MD5s) and writes two 512x512 RGBA8 layers. Requires Pillow and NumPy.

`crates/mc-render/src/textures.rs` embeds them after the foliage layers, at
`gpu_consts::metal_scan::LAYER` and the one after it:

| layer | file | contents |
|---|---|---|
| `LAYER` | `steel_color.rgba` | 054 A's linear albedo, roughness in A |
| `LAYER + 1` | `steel_normal.rgba` | 054 A's OpenGL tangent-space normal XY in RG; 054 B's scratches in B (0 none, 1 deepest) |

regency.wgsl maps them onto Regency plate and bronze in model space (three planar
projections blended by the face's normal) and uses only how they vary about their mean:
the grain, scratches and sheen, never the scan's own colour.
