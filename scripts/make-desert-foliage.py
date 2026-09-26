#!/usr/bin/env python3
"""Draws the desert leaf atlas (juniper, pinyon, cottonwood) procedurally and
writes data/textures/foliage/desert.rgba: 512x512 RGBA8, linear albedo with
antialiased coverage in alpha and colour bled into the gaps, the same layout as
the other leaf atlases. Seeded, so reruns match. Shares its brushes with
make-tropical-foliage.py.

Regions ([u0, v0, u1, v1], `DESERT_REGIONS` in crates/mc-render/src/foliage.rs):
  [0, 0, 0.5, 0.5]     a Utah juniper spray: blue-grey rope-like scale twigs
                       branching out from the middle, a few waxy blue berries
  [0.5, 0, 1, 0.5]     a pinyon's twig ends seen from above: short stiff dark
                       needles in brushes, radiating from the middle
  [0, 0.5, 0.5, 1]     a cluster of cottonwood leaves: bright yellow-green,
                       broad and pointed, on long stalks from the middle
  [0.5, 0.5, 1, 1]     a dense lobed clump of cottonwood leaves, for distant crowns

Requires NumPy and Pillow.
"""

import importlib.util
import math
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("tropical", HERE / "make-tropical-foliage.py")
T = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(T)

N, SIZE = T.N, T.SIZE
OUT = HERE.parent / "data/textures/foliage/desert.rgba"

# Linear albedo. The juniper grey-blue and dull, the pinyon a deep dark green,
# the cottonwood the brightest leaf in the game: a yellow-green lit from behind.
JUNIPER = (0.105, 0.128, 0.108)
JUNIPER_TWIG = (0.11, 0.1, 0.085)
BERRY = (0.2, 0.26, 0.36)
PINYON = (0.036, 0.07, 0.034)
PINYON_TWIG = (0.07, 0.055, 0.04)
COTTONWOOD = (0.11, 0.2, 0.026)
COTTONWOOD_STALK = (0.12, 0.15, 0.05)


def scale_cord(canvas, pts, width, rng):
    """A juniper twig: overlapping scale leaves stamped along the polyline
    `pts`, narrowing toward its tip, so it reads beaded and rope-like."""
    pts = np.array(pts, np.float64)
    seg = np.linalg.norm(np.diff(pts, axis=0), axis=1)
    total = seg.sum()
    step = width * 0.9
    run = 0.0
    k = 0
    while run < total - step:
        # Where `run` falls on the polyline.
        i = min(int(np.searchsorted(np.cumsum(seg), run, side="right")), len(seg) - 1)
        before = np.cumsum(seg)[i] - seg[i]
        t = (run - before) / max(seg[i], 1e-9)
        p = pts[i] + (pts[i + 1] - pts[i]) * t
        d = pts[i + 1] - pts[i]
        angle = math.atan2(d[1], d[0]) + (0.28 if k % 2 else -0.28)
        w = width * (1.0 - 0.45 * run / total)
        T.leaf(canvas, (p[0], p[1]), angle, w * 2.1, w, T.tint(rng, JUNIPER, 0.1, warm=-0.05), rng,
               taper=0.8, point=0.9, fold=0.35, shade=0.25)
        run += step * (1.0 - 0.3 * run / total)
        k += 1


def wiggle(start, angle, length, rng, bend):
    """A gently curving polyline from `start`."""
    pts = [start]
    a = angle
    for _ in range(6):
        a += bend + rng.normal(0, 0.08)
        x, y = pts[-1]
        pts.append((x + math.cos(a) * length / 6, y + math.sin(a) * length / 6))
    return pts


def juniper_spray(canvas, rng):
    """Top left: sprays of scale twigs branching out from the middle."""
    cx, cy = N * 0.25, N * 0.25
    mains = 11
    for m in range(mains):
        a = m * math.tau / mains + rng.normal(0, 0.18)
        length = rng.uniform(190, 228)
        bend = rng.normal(0, 0.05)
        spine = wiggle((cx + math.cos(a) * 12, cy + math.sin(a) * 12), a, length, rng, bend)
        canvas.line(spine[:4], 6, 3, JUNIPER_TWIG)
        # Side twigs alternate either side of the spine, shorter toward its tip.
        for j in range(1, 6):
            for side in (-1, 1):
                p = spine[j]
                sa = a + bend * j + side * rng.uniform(0.55, 0.95)
                sl = length * (0.52 - 0.07 * j) * rng.uniform(0.8, 1.1)
                twig = wiggle(p, sa, sl, rng, -side * 0.04)
                scale_cord(canvas, twig, rng.uniform(9.5, 11.5), rng)
                for q in twig[2:5:2]:
                    ta = sa + side * rng.uniform(0.5, 0.9)
                    scale_cord(canvas, wiggle(q, ta, sl * 0.45, rng, 0.0), 8.5, rng)
        scale_cord(canvas, spine, 13.0, rng)
    # Berries: waxy blue-white beads caught among the twigs.
    for _ in range(18):
        a = rng.uniform(0, math.tau)
        r = rng.uniform(30, 190)
        x, y = cx + math.cos(a) * r, cy + math.sin(a) * r
        rad = rng.uniform(5.5, 7.0)
        s = int(rad * 2 + 4)
        yy, xx = np.mgrid[0:s, 0:s].astype(np.float64) + 0.5 - s / 2
        d = np.hypot(xx, yy)
        cover = np.clip(rad + 0.5 - d, 0, 1).astype(np.float32)
        lit = np.clip(1.2 - np.hypot(xx + rad * 0.35, yy + rad * 0.35) / rad, 0.55, 1.2)
        rgb = np.array(T.tint(rng, BERRY, 0.1), np.float32) * lit[..., None].astype(np.float32)
        canvas.over(int(x - s / 2), int(y - s / 2), rgb, cover, shade=0.3)


def needle_brush(canvas, tip, heading, rng, count, reach):
    """A pinyon twig end: pairs of short stiff needles fanning forward and out."""
    for _ in range(count):
        a = heading + rng.normal(0, 0.75)
        l = reach * rng.uniform(0.6, 1.0)
        base = (tip[0] - math.cos(heading) * rng.uniform(0, 40), tip[1] - math.sin(heading) * rng.uniform(0, 40))
        for pair in (-1, 1):
            b = a + pair * rng.uniform(0.04, 0.12)
            end = (base[0] + math.cos(b) * l, base[1] + math.sin(b) * l)
            mid = ((base[0] + end[0]) / 2 + rng.normal(0, 2), (base[1] + end[1]) / 2 + rng.normal(0, 2))
            canvas.line([base, mid, end], 5.5, 2.0, T.tint(rng, PINYON, 0.18, warm=0.04))


def pinyon_tuft(canvas, rng):
    """Top right: twigs radiating from the middle, each ending in a brush."""
    cx, cy = N * 0.75, N * 0.25
    twigs = 9
    for k in range(twigs):
        a = k * math.tau / twigs + rng.normal(0, 0.2)
        r = rng.uniform(95, 135)
        tip = (cx + math.cos(a) * r, cy + math.sin(a) * r)
        canvas.line([(cx, cy), tip], 9, 5, PINYON_TWIG)
        needle_brush(canvas, tip, a, rng, 34, 100)
    # The middle, seen through the brushes.
    for _ in range(3):
        needle_brush(canvas, (cx + rng.normal(0, 20), cy + rng.normal(0, 20)), rng.uniform(0, math.tau), rng, 30, 90)


def cottonwood_leaf(canvas, base, angle, length, rng, up=0.0, shade=0.4):
    # Deltoid: widest near the stalk, drawn to a point.
    T.leaf(canvas, base, angle, length, length * rng.uniform(0.8, 0.92), T.tint(rng, COTTONWOOD, 0.14, warm=0.08 * up),
           rng, bend=rng.normal(0, 0.05), taper=0.42, point=0.38, veins=5, gloss=0.3, fold=0.2, shade=shade)


def cottonwood_cluster(canvas, rng):
    """Bottom left: leaves on long flat stalks from the middle, two rings."""
    cx, cy = N * 0.25, N * 0.75
    for ring, (count, lo, hi, stalk) in enumerate([(16, 92, 108, 128), (13, 84, 100, 78), (9, 76, 92, 36), (4, 66, 80, 8)]):
        start = rng.uniform(0, math.tau)
        for k in range(count):
            a = start + k * math.tau / count + rng.normal(0, 0.15)
            s = stalk * rng.uniform(0.8, 1.15)
            b = (cx + math.cos(a) * s, cy + math.sin(a) * s)
            canvas.line([(cx, cy), b], 5, 3, COTTONWOOD_STALK)
            cottonwood_leaf(canvas, b, a + rng.normal(0, 0.2), rng.uniform(lo, hi), rng, up=ring * 0.5)


def cottonwood_clump(canvas, rng):
    """Bottom right: a lobed mass of leaves pointing outward, for far crowns."""
    cx, cy = N * 0.75, N * 0.75
    lobes = [(rng.uniform(0, math.tau), rng.uniform(0.75, 1.0)) for _ in range(6)]

    def edge(a):
        r = 0.0
        for la, lr in lobes:
            r = max(r, lr * math.cos(min(abs((a - la + math.pi) % math.tau - math.pi), math.pi / 2)) ** 0.6)
        return 225 * max(r, 0.65)

    count = 260
    for i in range(count):
        a = rng.uniform(0, math.tau)
        k = math.sqrt(rng.uniform(0.0, 1.0))
        r = edge(a) * k * 0.95
        heading = a + rng.normal(0, 0.4 + 1.6 * (1 - k))
        up = i / count
        length = rng.uniform(58, 80) * (1.0 - 0.2 * up)
        b = (cx + math.cos(a) * r - math.cos(heading) * length * 0.45,
             cy + math.sin(a) * r - math.sin(heading) * length * 0.45)
        cottonwood_leaf(canvas, b, heading, length, rng, up=up, shade=0.45)


def main():
    canvas = T.Canvas()
    rng = np.random.default_rng(20260926)
    juniper_spray(canvas, rng)
    pinyon_tuft(canvas, rng)
    cottonwood_cluster(canvas, rng)
    cottonwood_clump(canvas, rng)
    data = T.finish(canvas.p)
    assert len(data) == SIZE * SIZE * 4
    OUT.write_bytes(data)
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()
