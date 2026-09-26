#!/usr/bin/env python3
"""Draws the tropical leaf atlas (palm fronds, rainforest leaves) procedurally and
writes data/textures/foliage/tropical.rgba: 512x512 RGBA8, linear albedo with
antialiased coverage in alpha and colour bled into the gaps, the same layout as
the scanned atlases from import-foliage.py. Seeded, so reruns match.

Regions ([u0, v0, u1, v1], `TROPICAL_REGIONS` in crates/mc-render/src/foliage.rs):
  [0, 0, 1, 0.5]      a coconut frond seen from above, stalk end at u0, rachis along v = 0.25
  [0, 0.5, 0.5, 1]    a cluster of big glossy rainforest leaves radiating from the middle
  [0.5, 0.5, 1, 1]    a dense lobed clump of leaves, for distant crowns

Requires NumPy and Pillow.
"""

import math
from pathlib import Path

import numpy as np

SIZE = 512
N = SIZE * 2  # composed at twice the size, then box-filtered down
OUT = Path(__file__).resolve().parent.parent / "data/textures/foliage/tropical.rgba"


def linear_to_byte(c):
    return np.clip(np.round(c * 255.0), 0, 255).astype(np.uint8)


def box_blur(a, r):
    for axis in (0, 1):
        pad = [(0, 0), (0, 0)]
        pad[axis] = (r + 1, r)
        c = np.cumsum(np.pad(a, pad, mode="edge"), axis=axis)
        n = a.shape[axis]
        hi = np.take(c, np.arange(2 * r + 1, 2 * r + 1 + n), axis=axis)
        lo = np.take(c, np.arange(0, n), axis=axis)
        a = (hi - lo) / (2 * r + 1)
    return a


class Canvas:
    """Premultiplied linear RGBA, y down."""

    def __init__(self):
        self.p = np.zeros((N, N, 4), np.float32)

    def over(self, x0, y0, rgb, alpha, shade=0.35):
        """Composites a premultiplied patch at (x0, y0); the leaves already there
        are darkened under a soft offset copy of it, so overlaps read as depth."""
        h, w = alpha.shape
        x1, y1 = min(x0 + w, N), min(y0 + h, N)
        sx0, sy0 = max(0, -x0), max(0, -y0)
        x0, y0 = max(x0, 0), max(y0, 0)
        if x1 <= x0 or y1 <= y0:
            return
        a = alpha[sy0:sy0 + y1 - y0, sx0:sx0 + x1 - x0]
        c = rgb[sy0:sy0 + y1 - y0, sx0:sx0 + x1 - x0]
        region = self.p[y0:y1, x0:x1]
        if shade > 0:
            soft = np.roll(box_blur(a, 5), 4, axis=0)
            region[..., :3] *= (1.0 - shade * soft)[..., None]
        region[..., :3] = c * a[..., None] + region[..., :3] * (1 - a[..., None])
        region[..., 3] = a + region[..., 3] * (1 - a)

    def line(self, pts, w0, w1, color):
        """An antialiased tapering stalk through `pts`."""
        pts = np.array(pts, np.float64)
        seg = np.linalg.norm(np.diff(pts, axis=0), axis=1)
        total, run = seg.sum(), 0.0
        for p, q, l in zip(pts[:-1], pts[1:], seg):
            a0 = w0 + (w1 - w0) * run / total
            run += l
            a1 = w0 + (w1 - w0) * run / total
            x0, y0 = np.floor(np.minimum(p, q) - max(a0, a1) - 2).astype(int)
            x1, y1 = np.ceil(np.maximum(p, q) + max(a0, a1) + 2).astype(int)
            x0, y0, x1, y1 = max(x0, 0), max(y0, 0), min(x1, N), min(y1, N)
            if x1 <= x0 or y1 <= y0:
                continue
            yy, xx = np.mgrid[y0:y1, x0:x1].astype(np.float64) + 0.5
            d = q - p
            t = np.clip(((xx - p[0]) * d[0] + (yy - p[1]) * d[1]) / max(l * l, 1e-9), 0, 1)
            dist = np.hypot(xx - p[0] - d[0] * t, yy - p[1] - d[1] * t)
            half = (a0 + (a1 - a0) * t) * 0.5
            cover = np.clip(half + 0.5 - dist, 0, 1).astype(np.float32)
            shade = (0.6 + 0.4 * np.clip(1 - dist / np.maximum(half, 0.5), 0, 1)).astype(np.float32)
            rgb = np.array(color, np.float32) * shade[..., None]
            self.over(x0, y0, rgb, cover, shade=0)


def leaf(canvas, base, angle, length, width, color, rng, *, bend=0.0, taper=0.6,
         point=1.0, veins=0, gloss=0.0, fold=0.3, shade=0.35):
    """A leaf from `base` along `angle` (radians, y down): its half width at a
    fraction s of the way out is width/2 * sin(pi * s^point)^taper, its midrib
    curves off sideways by bend * s^2 * length. Lit lighter toward the tip, one
    half of the fold darker, a pale midrib, optional side veins and a glossy
    band along the lit half."""
    ca, sa = math.cos(angle), math.sin(angle)
    reach = length + width + abs(bend) * length + 4
    x0, y0 = int(base[0] - reach), int(base[1] - reach)
    size = int(2 * reach) + 1
    yy, xx = np.mgrid[0:size, 0:size].astype(np.float64) + 0.5
    dx, dy = xx + x0 - base[0], yy + y0 - base[1]
    u = dx * ca + dy * sa  # along
    v = -dx * sa + dy * ca  # across
    s = u / length
    inside = (s > 0) & (s < 1)
    sc = np.clip(s, 1e-4, 1 - 1e-4)
    centre = bend * sc * sc * length
    half = 0.5 * width * np.sin(math.pi * sc ** point) ** taper
    off = v - centre
    cover = np.clip(half + 0.6 - np.abs(off), 0, 1) * inside
    cover = cover.astype(np.float32)
    if cover.max() <= 0:
        return
    across = np.clip(off / np.maximum(half, 0.5), -1, 1)
    tone = 0.72 + 0.4 * sc
    tone = tone * np.where(across > 0, 1.0 - fold * (0.4 + 0.6 * across), 1.0 + 0.08 * across)
    if gloss > 0:
        band = np.exp(-((across + 0.45) / 0.28) ** 2) * np.sin(math.pi * sc) ** 0.8
        tone = tone + gloss * band
    rib = np.clip(1.2 - np.abs(off) / 1.2, 0, 1) * (s < 0.97)
    rgb = np.array(color, np.float32)[None, None, :] * tone[..., None].astype(np.float32)
    rib_col = np.array(color, np.float32) * np.array([1.9, 1.55, 1.3], np.float32)
    rgb = rgb * (1 - rib[..., None] * 0.8) + rib_col * (rib[..., None] * 0.8)
    if veins:
        # Side veins sweep out and forward from the midrib: faint dark lines.
        phase = (s * veins - np.abs(off) / max(width, 1) * 1.6) % 1.0
        vein = np.clip(1 - np.abs(phase - 0.5) * 2 / 0.12, 0, 1) * (np.abs(off) > 1.5)
        rgb *= (1 - 0.22 * vein)[..., None].astype(np.float32)
    canvas.over(x0, y0, rgb.astype(np.float32), cover, shade=shade)


def tint(rng, base, spread=0.12, warm=0.0):
    k = rng.uniform(1 - spread, 1 + spread)
    w = warm + rng.normal(0, 0.06)
    r, g, b = base
    return (r * k * (1 + w * 1.4), g * k, b * k * (1 - w * 0.5))


# Linear albedo. A little yellow in the palm, deep and saturated in the jungle.
PALM = (0.085, 0.21, 0.024)
PALM_STALK = (0.14, 0.17, 0.05)
JUNGLE = (0.05, 0.17, 0.022)
JUNGLE_STALK = (0.07, 0.11, 0.03)


def palm_frond(canvas, rng):
    """Top half: rachis from the stalk end on the left along the middle, pinnae
    in pairs sweeping forward, longest a third of the way out."""
    y = N // 4
    stalk = [(10 + i * 99.0, y + 6 * math.sin(i / 10 * math.pi * 0.5) - 3) for i in range(11)]
    pairs = 52
    for i in range(pairs):
        t = 0.07 + 0.9 * i / (pairs - 1)
        x = 10 + t * 990
        py = y + 6 * math.sin(t * math.pi * 0.5) - 3
        env = math.sin(math.pi * min(1.0, 0.1 + t * 0.95)) ** 0.75
        for side in (-1, 1):
            length = (235 * env + 30) * rng.uniform(0.88, 1.08)
            # Pinnae leave the rachis at ~55 degrees near the stalk, ~35 at the tip.
            angle = side * (0.96 - 0.35 * t) + rng.normal(0, 0.05)
            leaf(canvas, (x + rng.normal(0, 2), py), angle, length, rng.uniform(15, 19),
                 tint(rng, PALM, 0.14, warm=0.1 * t), rng, bend=side * rng.uniform(0.02, 0.07),
                 taper=0.55, point=0.85, fold=0.35, shade=0.3)
    canvas.line(stalk, 13, 4, PALM_STALK)


def rainforest_cluster(canvas, rng):
    """Bottom left: big glossy leaves on stalks radiating from the middle, a
    lower ring and a shorter upper one over it."""
    cx, cy = N * 0.25, N * 0.75
    for ring, (count, lo, hi, wid) in enumerate([(10, 195, 232, 96), (8, 135, 175, 78), (5, 70, 100, 52)]):
        start = rng.uniform(0, math.tau)
        for k in range(count):
            a = start + k * math.tau / count + rng.normal(0, 0.15)
            stalk = rng.uniform(16, 34) * (1.0 if ring == 0 else 0.6)
            b = (cx + math.cos(a) * stalk, cy + math.sin(a) * stalk)
            canvas.line([(cx, cy), b], 6, 4, JUNGLE_STALK)
            length = rng.uniform(lo, hi)
            leaf(canvas, b, a, length, wid * rng.uniform(0.85, 1.1), tint(rng, JUNGLE, 0.15, warm=0.05 * ring),
                 rng, bend=rng.normal(0, 0.06), taper=0.75, point=0.75, veins=9, gloss=0.35,
                 fold=0.25, shade=0.4)


def dense_clump(canvas, rng):
    """Bottom right: a lobed mass of overlapping leaves pointing outward, the
    later (upper) ones lighter, for crowns seen from far off."""
    cx, cy = N * 0.75, N * 0.75
    lobes = [(rng.uniform(0, math.tau), rng.uniform(0.75, 1.0)) for _ in range(6)]

    def edge(a):
        r = 0.0
        for la, lr in lobes:
            r = max(r, lr * math.cos(min(abs((a - la + math.pi) % math.tau - math.pi), math.pi / 2)) ** 0.6)
        return 225 * max(r, 0.65)

    count = 210
    for i in range(count):
        a = rng.uniform(0, math.tau)
        k = math.sqrt(rng.uniform(0.0, 1.0))
        r = edge(a) * k * 0.95
        # Outward at the rim, any way in the middle; (r, a) is the leaf's middle.
        heading = a + rng.normal(0, 0.4 + 1.6 * (1 - k))
        up = i / count
        length = rng.uniform(72, 105) * (1.0 - 0.25 * up)
        b = (cx + math.cos(a) * r - math.cos(heading) * length * 0.45,
             cy + math.sin(a) * r - math.sin(heading) * length * 0.45)
        leaf(canvas, b, heading, length, length * rng.uniform(0.38, 0.48),
             tint(rng, JUNGLE, 0.15, warm=0.08 * up), rng, bend=rng.normal(0, 0.08), taper=0.75, point=0.75,
             veins=6, gloss=0.25, fold=0.25, shade=0.45)


def bleed(premult, alpha):
    """Push-pull fill: texels outside the leaves take the nearest leaves' colour."""
    levels = [(premult, alpha)]
    while levels[-1][1].shape[0] > 1:
        c, a = levels[-1]
        n = c.shape[0] // 2
        levels.append((c.reshape(n, 2, n, 2, 3).mean((1, 3)), a.reshape(n, 2, n, 2).mean((1, 3))))
    c, a = levels[-1]
    color = c / np.maximum(a, 1e-6)[..., None]
    for c, a in reversed(levels[:-1]):
        up = np.repeat(np.repeat(color, 2, 0), 2, 1)
        known = c / np.maximum(a, 1e-6)[..., None]
        w = np.clip(a * 3.0, 0, 1)[..., None]
        color = known * w + up * (1 - w)
    return color


def finish(premultiplied):
    p = premultiplied.reshape(SIZE, 2, SIZE, 2, 4).mean((1, 3))
    color = bleed(p[..., :3], p[..., 3])
    return np.dstack([linear_to_byte(color), linear_to_byte(p[..., 3])]).tobytes()


def main():
    canvas = Canvas()
    rng = np.random.default_rng(20260925)
    palm_frond(canvas, rng)
    rainforest_cluster(canvas, rng)
    dense_clump(canvas, rng)
    data = finish(canvas.p)
    assert len(data) == SIZE * SIZE * 4
    OUT.write_bytes(data)
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()
