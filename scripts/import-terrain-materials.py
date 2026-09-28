#!/usr/bin/env python3
"""Pack CC0 Poly Haven PBR textures for the renderer. Requires Pillow.
Run from any directory; source files are cached under target/terrain-sources.

Each material becomes two 512x512 RGBA8 layers:
  <asset>_color.rgba   linear RGB albedo, linear roughness in A
  <asset>_detail.rgba  OpenGL tangent normal XY in RG (Z is rebuilt in the
                       shader), scanned displacement in B, occlusion in A
The order of MATERIALS is the order of the layers in the texture array; keep it
in step with `textures::GROUND` and the material constants in bindings.wgsl.
"""
import concurrent.futures
import hashlib
import io
import json
from pathlib import Path
import urllib.request
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
SIZE = 512
DEST = ROOT / "data/textures/terrain"
CACHE = ROOT / "target/terrain-sources"
DEST.mkdir(parents=True, exist_ok=True)
CACHE.mkdir(parents=True, exist_ok=True)
HEADERS = {"User-Agent": "MeridianConflict/1.0 (CC0 terrain material import)"}

MATERIALS = [
    "rock_face",            # cliffs and steep slopes
    "leafy_grass",          # lush short grass
    "aerial_grass_rock",    # open meadow with outcrops, seen from above (15 m scan)
    "forest_leaves_02",     # mossy, leaf-littered grass
    "forrest_ground_01",    # needle and moss forest floor
    "rocky_trail",          # stony scree and gravel
    "dry_ground_rocks",     # bare dry dirt with pebbles
    "aerial_rocks_02",      # rocky highland, seen from above (50 m scan)
    "gravelly_sand",        # beaches and shores
    "brown_mud_leaves_01",  # damp hollows and stream banks
]

def get(url):
    return urllib.request.urlopen(urllib.request.Request(url, headers=HEADERS), timeout=120).read()

def material(asset):
    metadata = json.loads(get("https://api.polyhaven.com/files/" + asset))
    info = json.loads(get("https://api.polyhaven.com/info/" + asset))
    sources = {}
    def channel(key):
        source = metadata[key]["1k"]["png"]
        url = source["url"]
        path = CACHE / url.rsplit("/", 1)[-1]
        if not path.exists():
            path.write_bytes(get(url))
        data = path.read_bytes()
        assert hashlib.md5(data).hexdigest() == source["md5"], url
        sources[key] = {"url": url, "md5": source["md5"]}
        image = Image.open(io.BytesIO(data))
        if image.mode in ("I;16", "I", "I;16B"):
            image = image.convert("I").point(lambda value: value / 257).convert("L")
        return image.convert("RGB").resize((SIZE, SIZE), Image.Resampling.LANCZOS)
    with concurrent.futures.ThreadPoolExecutor(max_workers=5) as pool:
        diffuse, normal, rough, ao, height = list(pool.map(channel, ["Diffuse", "nor_gl", "Rough", "AO", "Displacement"]))
    # Store linear color: the shared array also contains linear normal data.
    linear = lambda b: round(((b / 255 / 12.92) if b / 255 <= .04045 else ((b / 255 + .055) / 1.055) ** 2.4) * 255)
    lut = [linear(i) for i in range(256)]
    # Stretch displacement to the full range: height-aware blending compares
    # materials, so a scan that only uses a sliver of the range never wins.
    heights = [h[0] for h in height.getdata()]
    lo, hi = sorted(heights)[len(heights) // 200], sorted(heights)[-len(heights) // 200 - 1]
    span = max(hi - lo, 1)
    color = bytearray()
    detail = bytearray()
    for c, n, r, a, h in zip(diffuse.getdata(), normal.getdata(), rough.getdata(), ao.getdata(), heights):
        color.extend([lut[c[0]], lut[c[1]], lut[c[2]], r[0]])
        xyz = [v / 127.5 - 1 for v in n]
        length = max(sum(v*v for v in xyz) ** .5, .0001)
        xy = [max(0, min(255, round((v / length * .5 + .5) * 255))) for v in xyz[:2]]
        detail.extend(xy + [max(0, min(255, round((h - lo) * 255 / span))), a[0]])
    (DEST / (asset + "_color.rgba")).write_bytes(color)
    (DEST / (asset + "_detail.rgba")).write_bytes(detail)
    sources["metres"] = round(info["dimensions"][0] / 1000, 2)
    sources["authors"] = info["authors"]
    return asset, sources

if __name__ == "__main__":
    with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
        records = dict(pool.map(material, MATERIALS))
    (DEST / "sources.json").write_text(json.dumps({m: records[m] for m in MATERIALS}, indent=2) + "\n")
    print(f"Packed {len(MATERIALS) * 2} 512x512 RGBA8 material layers.")
