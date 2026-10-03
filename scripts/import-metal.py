#!/usr/bin/env python3
"""Pack the CC0 ambientCG metal scans the plate is finished with.
Requires Pillow and NumPy. Run from any directory; the downloads are cached under
target/metal-sources and checked against their MD5s.

Writes two 512x512 RGBA8 layers per scan to data/textures/metal (credits and
layout in the README there):
  <name>_color.rgba   linear RGB albedo, linear roughness in A
  <name>_normal.rgba  OpenGL tangent normal XY in RG; scratches in B (0 none,
                      1 deepest); A 255

  steel  the Regency's worn iron: Metal 054 A, scratches from Metal 054 B
  arc    ARC's steel under the paint: brushed Metal 009, long gouges from Metal 037
"""
import hashlib
import io
from pathlib import Path
import urllib.request
import zipfile

import numpy as np
from PIL import Image, ImageFilter

ROOT = Path(__file__).resolve().parents[1]
SIZE = 512
DEST = ROOT / "data/textures/metal"
CACHE = ROOT / "target/metal-sources"
SOURCES = {
    "Metal054A": "5666b319f7e859dfeeebbcfe8b4754e2",
    "Metal054B": "fbd9b7cb942865ea2499a1dff110a7e6",
    "Metal009": "627e722ebfe39e75512a543523c60e9f",
    "Metal037": "0733e45754a25e6ffbf51a15e06ddfbf",
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


def channel(asset: str, name: str, mode: str) -> np.ndarray:
    with fetch(asset).open(f"{asset}_1K-JPG_{name}.jpg") as f:
        im = Image.open(f)
        im.load()
    im = im.convert(mode).resize((SIZE, SIZE), Image.LANCZOS)
    return np.asarray(im, dtype=np.float32) / 255.0


def scratches_above(asset: str) -> np.ndarray:
    """Where a scan's roughness stands above its own median (Metal 054 B)."""
    r = channel(asset, "Roughness", "L")
    lo, hi = np.percentile(r, 50), np.percentile(r, 99.5)
    return ((r - lo) / max(hi - lo, 1e-3)).clip(0.0, 1.0)


def scratches_apart(asset: str, keep: float) -> np.ndarray:
    """Where a scan's roughness departs from its surroundings either way, only the
    `keep` share that departs most: thin lines, not its broad blotches."""
    r = channel(asset, "Roughness", "L")
    blur = Image.fromarray((r * 255.0).astype(np.uint8)).filter(ImageFilter.GaussianBlur(6))
    d = np.abs(r - np.asarray(blur, dtype=np.float32) / 255.0)
    lo, hi = np.percentile(d, 100.0 * (1.0 - keep)), np.percentile(d, 99.7)
    return ((d - lo) / max(hi - lo, 1e-3)).clip(0.0, 1.0)


def pack(name: str, base: str, scratches: np.ndarray) -> None:
    srgb = channel(base, "Color", "RGB")
    linear = np.where(srgb <= 0.04045, srgb / 12.92, ((srgb + 0.055) / 1.055) ** 2.4)
    rough = channel(base, "Roughness", "L")
    normal = channel(base, "NormalGL", "RGB")
    color = np.dstack([linear, rough])
    detail = np.dstack([normal[..., 0], normal[..., 1], scratches, np.ones_like(rough)])
    for part, layer in (("color", color), ("normal", detail)):
        (DEST / f"{name}_{part}.rgba").write_bytes((layer * 255.0 + 0.5).clip(0, 255).astype(np.uint8).tobytes())
    lum = (linear @ np.array([0.3, 0.59, 0.11])).mean()
    print(f"wrote {name}: mean luminance {lum:.3f}, mean roughness {rough.mean():.3f}, "
          f"mean scratches {scratches.mean():.3f}")


def main() -> None:
    DEST.mkdir(parents=True, exist_ok=True)
    pack("steel", "Metal054A", scratches_above("Metal054B"))
    # Only Metal 037's thin scratches, not its galvanised spangle.
    pack("arc", "Metal009", scratches_apart("Metal037", 0.07))


if __name__ == "__main__":
    main()
