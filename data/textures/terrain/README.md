# Terrain PBR materials

CC0 1.0 scans from Poly Haven (https://polyhaven.com/license), in texture-array order:

0. [Rock Face](https://polyhaven.com/a/rock_face) — Greg Zaal (photography), Dario Barresi (processing). 2.38 m scan; cliffs and steep slopes (triplanar); also rock props and boulders.
1. [Leafy Grass](https://polyhaven.com/a/leafy_grass) — Charlotte Baglioni. 2.0 m scan; lush short grass.
2. [Aerial Grass Rock](https://polyhaven.com/a/aerial_grass_rock) — Rob Tuytel. 15.0 m scan; open meadow with outcrops; also the 42 m large-scale light/dark layer.
3. [Forest Leaves 02](https://polyhaven.com/a/forest_leaves_02) — Rob Tuytel. 3.0 m scan; mossy, leaf-littered grass under broadleaf canopy and in damp swards.
4. [Forrest Ground 01](https://polyhaven.com/a/forrest_ground_01) — Rob Tuytel. 2.0 m scan; needle and moss floor under conifers.
5. [Rocky Trail](https://polyhaven.com/a/rocky_trail) — Amal Kumar. 2.0 m scan; scree at the foot of cliffs and stony highland.
6. [Dry Ground Rocks](https://polyhaven.com/a/dry_ground_rocks) — Rob Tuytel. 4.0 m scan; bare dry dirt on convex, dry ground.
7. [Aerial Rocks 02](https://polyhaven.com/a/aerial_rocks_02) — Rob Tuytel. 50.0 m scan; rocky highland seen from above.
8. [Gravelly Sand](https://polyhaven.com/a/gravelly_sand) — Dario Barresi. 2.48 m scan; beaches and shores.
9. [Brown Mud Leaves 01](https://polyhaven.com/a/brown_mud_leaves_01) — Rob Tuytel. 1.3 m scan; damp hollows and stream banks.

`python3 scripts/import-terrain-materials.py` downloads the verified 1K PNG channels and packs two 512x512 RGBA8 layers per material. Requires Pillow. `sources.json` records the original URLs, MD5 checksums, real-world scan size and authors. Downloads are cached in `target/terrain-sources`.

`*_color.rgba`: linear RGB albedo, linear roughness in A.
`*_detail.rgba`: OpenGL tangent-space normal XY in RG (Z is rebuilt in the shader), scanned displacement stretched to the full range in B, occlusion in A.

Layer `2 * i` is material `i`'s colour and `2 * i + 1` its detail; `textures::GROUND`, the `MAT_*` constants in `bindings.wgsl` and the importer's `MATERIALS` list must stay in the same order. Foliage and bark layers follow at `FOLIAGE_BASE`.
