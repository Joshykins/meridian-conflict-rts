#!/usr/bin/env python3
"""Pack the CC0 ambientCG metal scans the Regency plate is finished with.
Requires Pillow and NumPy. Run from any directory; the downloads are cached under
target/metal-sources and checked against their MD5s.

Writes two 512x512 RGBA8 layers to data/textures/metal (credits and layout in
the README there):
  steel_color.rgba   Metal 054 A: linear RGB albedo, linear roughness in A
  steel_normal.rgba  Metal 054 A's OpenGL tangent normal XY in RG; in B the
                     scratches of Metal 054 B (0 none, 1 deepest); A 255
"""
import hashlib
import io
from pathlib import Path
import urllib.request
import zipfile

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
SIZE = 512
DEST = ROOT / "data/textures/metal"
CACHE = ROOT / "target/metal-sources"
SOURCES = {
    "Metal054A": "5666b319f7e859dfeeebbcfe8b4754e2",
    "Metal054B": "fbd9b7cb942865ea2499a1dff110a7e6",
}
HEADERS = {"User-Agent": "MeridianConflict/1.0 (CC0 metal material import)"}


def fetch(asset: str) -> zipfile.ZipFile:
    CACHE.mkdir(parents=True, exist_ok=True)
    path = CACHE / f"{asset}_1K-JPG.zip"
    if not path.exists() or hashlib.md5(path.read_bytes()).hexdigest() != SOURCES[asset]:
        url = f"https://ambientcg.com/get?file={asset}_1K-JPG.zip"
        with urllib.request.urlopen(urllib.request.Request(url, headers=HEADERS)) as r:
            path.write_bytes(r.read())
    data = path.read_bytes()
    if hashlib.md5(data).hexdigest() != SOURCES[asset]:
        raise SystemExit(f"{path}: MD5 does not match the one this script was written for")
    return zipfile.ZipFile(io.BytesIO(data))


def channel(z: zipfile.ZipFile, asset: str, name: str, mode: str) -> np.ndarray:
    with z.open(f"{asset}_1K-JPG_{name}.jpg") as f:
        im = Image.open(f)
        im.load()
    im = im.convert(mode).resize((SIZE, SIZE), Image.LANCZOS)
    return np.asarray(im, dtype=np.float32) / 255.0


def main() -> None:
    base = fetch("Metal054A")
    srgb = channel(base, "Metal054A", "Color", "RGB")
    linear = np.where(srgb <= 0.04045, srgb / 12.92, ((srgb + 0.055) / 1.055) ** 2.4)
    rough = channel(base, "Metal054A", "Roughness", "L")
    normal = channel(base, "Metal054A", "NormalGL", "RGB")
    # The scratches: where Metal 054 B's roughness stands above its own surroundings.
    scratched = channel(fetch("Metal054B"), "Metal054B", "Roughness", "L")
    lo, hi = np.percentile(scratched, 50), np.percentile(scratched, 99.5)
    scratches = ((scratched - lo) / max(hi - lo, 1e-3)).clip(0.0, 1.0)
    DEST.mkdir(parents=True, exist_ok=True)
    color = np.dstack([linear, rough])
    detail = np.dstack([normal[..., 0], normal[..., 1], scratches, np.ones_like(rough)])
    for name, layer in (("steel_color", color), ("steel_normal", detail)):
        (DEST / f"{name}.rgba").write_bytes((layer * 255.0 + 0.5).clip(0, 255).astype(np.uint8).tobytes())
    lum = (linear @ np.array([0.3, 0.59, 0.11])).mean()
    print(f"wrote {DEST}: mean luminance {lum:.3f}, mean roughness {rough.mean():.3f}, "
          f"mean scratches {scratches.mean():.3f}")


if __name__ == "__main__":
    main()
