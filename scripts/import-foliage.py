#!/usr/bin/env python3
"""Compose CC0 Poly Haven leaves, needles and bark into the renderer's foliage
layers. Requires Pillow and NumPy. Run from any directory; source files are
cached under target/foliage-sources and verified against Poly Haven's MD5s.

Writes 512x512 RGBA8 layers to data/textures/foliage (see its README.md):
  broadleaf.rgba  leaf-spray atlas, linear albedo + cutout coverage
  conifer.rgba    fir frond / pine tuft / whole-fir atlas, same layout
  bark_color.rgba, bark_normal.rgba          broadleaf bark (bark_brown_02)
  pine_bark_color.rgba, pine_bark_normal.rgba conifer bark (pine_bark)
The composition is seeded, so a rerun reproduces the same bytes.
"""
import concurrent.futures
import hashlib
import io
import json
import math
from pathlib import Path
import urllib.request

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

ROOT = Path(__file__).resolve().parents[1]
SIZE = 512
WORK = SIZE * 2  # composed at twice the size, then box-filtered for antialiased edges
DEST = ROOT / "data/textures/foliage"
CACHE = ROOT / "target/foliage-sources"
HEADERS = {"User-Agent": "MeridianConflict/1.0 (CC0 foliage import)"}

SOURCES = {
    "searsia_lucida": ["Diffuse", "Alpha"],
    "tree_small_02": ["leaves_diff", "leaves_alpha"],
    "fir_tree_01": ["twig_diff", "twig_alpha"],
    "pine_tree_01": ["twig_diff", "twig_alpha"],
    "bark_brown_02": ["Diffuse", "nor_gl", "Rough", "AO"],
    "pine_bark": ["Diffuse", "nor_gl", "Rough", "AO"],
}


def get(url):
    return urllib.request.urlopen(urllib.request.Request(url, headers=HEADERS), timeout=90).read()


def fetch_all():
    records, images = {}, {}
    jobs = []
    for asset, keys in SOURCES.items():
        metadata = json.loads(get("https://api.polyhaven.com/files/" + asset))
        for key in keys:
            jobs.append((asset, key, metadata[key]["1k"]["png"]))

    def one(job):
        asset, key, source = job
        url = source["url"]
        path = CACHE / url.rsplit("/", 1)[-1]
        if not path.exists():
            path.write_bytes(get(url))
        data = path.read_bytes()
        assert hashlib.md5(data).hexdigest() == source["md5"], url
        return asset, key, {"url": url, "md5": source["md5"]}, Image.open(io.BytesIO(data))

    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
        for asset, key, record, image in pool.map(one, jobs):
            records.setdefault(asset, {})[key] = record
            images[(asset, key)] = image
    return records, images


def gray(image):
    """Single channel 0..1, whatever the PNG's bit depth."""
    a = np.asarray(image).astype(np.float32)
    if a.ndim == 3:
        a = a[..., 0]
    return a / (65535.0 if image.mode.startswith("I") else 255.0)


def srgb_to_linear(c):
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)


def linear_to_byte(c):
    return np.clip(np.round(c * 255.0), 0, 255).astype(np.uint8)


# ---- sprites -------------------------------------------------------------------

class Sprite:
    """A premultiplied linear RGBA cut-out, rotated so it points along +x from
    `anchor` (its base: a leaf's stalk, a twig's cut end)."""

    def __init__(self, rgba, anchor):
        self.rgba = rgba
        self.anchor = anchor
        self.length = rgba.shape[1] - anchor[0]


def island(mask, seed):
    """The connected blob of `mask` (bool array) around `seed`, via flood fill."""
    image = Image.fromarray(np.where(mask, 255, 0).astype(np.uint8)).copy()
    ImageDraw.floodfill(image, seed, 128)
    return np.array(image) == 128


def cut(images, asset, keys, box, base, clean=0, join=3):
    """Cuts one leaf or twig out of an atlas. `box` is (x0, y0, x1, y1) in the 1K
    source; `base` a point near its base, which becomes the sprite's anchor.
    `clean` opens the mask by that many pixels, removing stalk cards and shards;
    `join` is how far apart pieces (needles) may be and still count as one part."""
    diff = images[(asset, keys[0])].convert("RGB")
    color = srgb_to_linear(np.asarray(diff).astype(np.float32)[box[1]:box[3], box[0]:box[2]] / 255.0)
    alpha = gray(images[(asset, keys[1])])[box[1]:box[3], box[0]:box[2]]
    solid = alpha > 0.5
    if clean:
        m = Image.fromarray(np.where(solid, 255, 0).astype(np.uint8))
        m = m.filter(ImageFilter.MinFilter(clean * 2 + 1)).filter(ImageFilter.MaxFilter(clean * 2 + 1))
        solid = np.asarray(m) > 127
    # Keep the largest-looking blob: flood from the most interior solid pixel.
    ys, xs = np.nonzero(solid)
    centre = np.array([xs.mean(), ys.mean()])
    pick = np.argmin((xs - centre[0]) ** 2 + (ys - centre[1]) ** 2)
    joined = Image.fromarray(np.where(solid, 255, 0).astype(np.uint8)).filter(ImageFilter.MaxFilter(join))
    blob = island(np.asarray(joined) > 127, (int(xs[pick]), int(ys[pick]))) & solid
    grown = Image.fromarray(np.where(blob, 255, 0).astype(np.uint8)).filter(ImageFilter.MaxFilter(5))
    alpha = alpha * (np.asarray(grown) > 127)
    # Base: the covered pixel nearest the hint; the axis runs from it through the middle.
    ys, xs = np.nonzero(alpha > 0.5)
    pts = np.stack([xs, ys], 1).astype(np.float64)
    b = np.array(base, dtype=np.float64) - [box[0], box[1]]
    root = pts[np.argmin(((pts - b) ** 2).sum(1))]
    axis = pts.mean(0) - root
    axis /= np.linalg.norm(axis)
    if clean or join < 5:
        # A leaf: along its blade's long axis, from the end nearest the hint.
        _, vecs = np.linalg.eigh(np.cov((pts - pts.mean(0)).T))
        axis = vecs[:, 1] * np.sign(vecs[:, 1] @ axis or 1.0)
        proj = (pts - pts.mean(0)) @ axis
        root = pts.mean(0) + axis * proj.min()
    ends = [root]
    angle = math.atan2(axis[1], axis[0])
    # Rotate so the axis runs along +x, base on the left.
    pre = np.dstack([color * alpha[..., None], alpha])
    h, w = alpha.shape
    reach = int(math.ceil(math.hypot(w, h)))
    out = np.zeros((reach, reach, 4), np.float32)
    c, s = math.cos(angle), math.sin(angle)
    centre_out = np.array([reach / 2, reach / 2])
    # output (x, y) -> input: rotate by +angle about the centres.
    a0, b0 = c, -s
    d0, e0 = s, c
    cx, cy = w / 2, h / 2
    coeffs = (a0, b0, cx - a0 * centre_out[0] - b0 * centre_out[1],
              d0, e0, cy - d0 * centre_out[0] - e0 * centre_out[1])
    for ch in range(4):
        img = Image.fromarray(pre[..., ch].astype(np.float32))
        out[..., ch] = np.asarray(img.transform((reach, reach), Image.Transform.AFFINE, coeffs, Image.Resampling.BICUBIC))
    out = np.clip(out, 0.0, None)
    out[..., 3] = np.clip(out[..., 3], 0.0, 1.0)
    # Where the base end landed, then trim to the covered box.
    to_out = lambda p: np.array([c * (p[0] - cx) + s * (p[1] - cy), -s * (p[0] - cx) + c * (p[1] - cy)]) + centre_out
    anchor = to_out(ends[0])
    ys, xs = np.nonzero(out[..., 3] > 0.02)
    x0, x1, y0, y1 = xs.min(), xs.max() + 1, ys.min(), ys.max() + 1
    x0 = min(x0, int(anchor[0]))
    return Sprite(out[y0:y1, x0:x1].copy(), (anchor[0] - x0, anchor[1] - y0))


def grade(sprite, target, amount=1.0):
    """Scales a sprite's colour so its covered mean is `target` (linear RGB),
    keeping the scan's own vein and shading detail."""
    rgba = sprite.rgba.copy()
    a = rgba[..., 3]
    mean = (rgba[..., :3].sum((0, 1)) / max(a.sum(), 1e-6))
    scale = np.array(target) / np.maximum(mean, 1e-5)
    rgba[..., :3] *= 1.0 + (scale - 1.0) * amount
    return Sprite(rgba, sprite.anchor)


# ---- canvas -------------------------------------------------------------------

class Canvas:
    def __init__(self, width, height):
        self.p = np.zeros((height, width, 4), np.float32)

    def stamp(self, sprite, at, angle, scale, tint=(1, 1, 1), shade=0.35, squash=1.0):
        """Places `sprite` with its base at `at`, pointing along `angle` (radians,
        y down), `scale` times its size. The leaves already there are darkened
        under a soft offset copy of it: the overlap reads as depth."""
        rgba, (ax, ay) = sprite.rgba, sprite.anchor
        h, w = rgba.shape[:2]
        c, s = math.cos(angle), math.sin(angle)
        corners = np.array([[0, 0], [w, 0], [0, h], [w, h]], np.float64) - [ax, ay]
        corners[:, 1] *= squash
        world = np.stack([c * corners[:, 0] - s * corners[:, 1], s * corners[:, 0] + c * corners[:, 1]], 1) * scale + at
        H, W = self.p.shape[:2]
        x0, y0 = np.floor(world.min(0)).astype(int) - 6
        x1, y1 = np.ceil(world.max(0)).astype(int) + 6
        x0, y0, x1, y1 = max(x0, 0), max(y0, 0), min(x1, W), min(y1, H)
        if x1 <= x0 or y1 <= y0:
            return
        # output (x, y) -> sprite: inverse rotation and scale about the anchor.
        ia, ib = c / scale, s / scale
        id_, ie = -s / (scale * squash), c / (scale * squash)
        ox, oy = x0 - at[0], y0 - at[1]
        coeffs = (ia, ib, ax + ia * ox + ib * oy, id_, ie, ay + id_ * ox + ie * oy)
        size = (x1 - x0, y1 - y0)
        src = np.zeros((y1 - y0, x1 - x0, 4), np.float32)
        for ch in range(4):
            img = Image.fromarray(np.ascontiguousarray(rgba[..., ch], np.float32))
            src[..., ch] = np.asarray(img.transform(size, Image.Transform.AFFINE, coeffs, Image.Resampling.BILINEAR))
        src[..., :3] *= np.array(tint, np.float32)
        src[..., 3] = np.clip(src[..., 3], 0, 1)
        region = self.p[y0:y1, x0:x1]
        if shade > 0:
            soft = box_blur(src[..., 3], max(2, int(4 * scale + 2)))
            # Light from above: the shadow falls a little below the new leaf.
            soft = np.roll(soft, 3, axis=0)
            region[..., :3] *= (1.0 - shade * soft)[..., None]
        region[:] = src + region * (1.0 - src[..., 3:4])

    def line(self, points, width0, width1, color):
        """An antialiased tapering twig through `points`."""
        pts = np.array(points, np.float64)
        seg = np.linalg.norm(np.diff(pts, axis=0), axis=1)
        total = seg.sum()
        run = 0.0
        H, W = self.p.shape[:2]
        for (p, q, l) in zip(pts[:-1], pts[1:], seg):
            w0 = width0 + (width1 - width0) * run / total
            run += l
            w1 = width0 + (width1 - width0) * run / total
            x0, y0 = np.floor(np.minimum(p, q) - max(w0, w1) - 2).astype(int)
            x1, y1 = np.ceil(np.maximum(p, q) + max(w0, w1) + 2).astype(int)
            x0, y0, x1, y1 = max(x0, 0), max(y0, 0), min(x1, W), min(y1, H)
            if x1 <= x0 or y1 <= y0:
                continue
            yy, xx = np.mgrid[y0:y1, x0:x1].astype(np.float64) + 0.5
            d = q - p
            t = np.clip(((xx - p[0]) * d[0] + (yy - p[1]) * d[1]) / max(l * l, 1e-9), 0, 1)
            dist = np.hypot(xx - p[0] - d[0] * t, yy - p[1] - d[1] * t)
            half = (w0 + (w1 - w0) * t) * 0.5
            cover = np.clip(half + 0.5 - dist, 0, 1).astype(np.float32)
            # Round twigs: darker along their edges.
            shade = (0.55 + 0.45 * np.clip(1 - dist / np.maximum(half, 0.5), 0, 1)).astype(np.float32)
            region = self.p[y0:y1, x0:x1]
            src_a = cover[..., None]
            src_c = np.array(color, np.float32) * shade[..., None] * src_a
            region[..., :3] = src_c + region[..., :3] * (1 - src_a)
            region[..., 3:4] = src_a + region[..., 3:4] * (1 - src_a)

    def region(self, x, y, w, h):
        sub = Canvas(1, 1)
        sub.p = self.p[y:y + h, x:x + w]
        return sub


def box_blur(a, r):
    """Separable box blur of radius `r`, clamped at the edges."""
    for axis in (0, 1):
        pad = [(0, 0), (0, 0)]
        pad[axis] = (r + 1, r)
        c = np.cumsum(np.pad(a, pad, mode="edge"), axis=axis)
        n = a.shape[axis]
        hi = np.take(c, np.arange(2 * r + 1, 2 * r + 1 + n), axis=axis)
        lo = np.take(c, np.arange(0, n), axis=axis)
        a = (hi - lo) / (2 * r + 1)
    return a


def curve(start, direction, length, bend, steps=6):
    """Points along a gently bending twig."""
    pts = [np.array(start, np.float64)]
    angle = direction
    step = length / steps
    for _ in range(steps):
        angle += bend / steps
        pts.append(pts[-1] + step * np.array([math.cos(angle), math.sin(angle)]))
    return pts


def along(pts, t):
    """Point and heading at fraction `t` of a polyline."""
    pts = np.array(pts)
    seg = np.linalg.norm(np.diff(pts, axis=0), axis=1)
    target = t * seg.sum()
    for i, l in enumerate(seg):
        if target <= l or i == len(seg) - 1:
            f = min(target / max(l, 1e-9), 1.0)
            d = pts[i + 1] - pts[i]
            return pts[i] + d * f, math.atan2(d[1], d[0])
        target -= l


# ---- compositions ---------------------------------------------------------------

TWIG = (0.045, 0.032, 0.02)
CONIFER_TWIG = (0.05, 0.03, 0.018)


def leaf_tint(rng, warm=0.0):
    """A leaf-to-leaf colour shift: brightness, and a little toward yellow or blue."""
    k = rng.uniform(0.72, 1.22)
    hue = rng.normal(warm, 0.08)
    return (k * (1 + hue * 0.9), k, k * (1 - hue * 0.5))


def leafy_twig(canvas, rng, leaves, start, direction, length, bend, leaf_size, width, density=1.0):
    """A twig with alternate leaves along it and a tuft at its tip."""
    pts = curve(start, direction, length, bend)
    canvas.line(pts, width, width * 0.35, TWIG)
    count = max(2, int(length / (leaf_size * 0.42) * density))
    for i in range(count):
        t = 0.22 + 0.78 * (i + rng.uniform(0, 0.6)) / count
        p, heading = along(pts, min(t, 1.0))
        side = 1 if i % 2 else -1
        ang = heading + side * rng.uniform(0.55, 1.15)
        s = leaf_size * (1.0 - 0.35 * t) * rng.uniform(0.8, 1.15)
        leaf = leaves[rng.integers(len(leaves))]
        canvas.stamp(leaf, p, ang, s / leaf.length, leaf_tint(rng), squash=rng.uniform(0.75, 1.1))
    tip, heading = along(pts, 1.0)
    for k in range(3):
        leaf = leaves[rng.integers(len(leaves))]
        s = leaf_size * 0.8 * rng.uniform(0.85, 1.1)
        canvas.stamp(leaf, tip, heading + (k - 1) * 0.6 + rng.normal(0, 0.15), s / leaf.length, leaf_tint(rng))


def radial_cluster(canvas, rng, leaves, centre, radius, leaf_size, twigs):
    """Twigs radiating from a hidden stem: a round leaf cluster seen from outside.
    Everything stays within `radius` of `centre`."""
    base = rng.uniform(0, math.tau)
    for i in range(twigs):
        direction = base + i * math.tau / twigs + rng.normal(0, 0.18)
        length = (radius - leaf_size * 0.95) * rng.uniform(0.8, 1.0)
        leafy_twig(canvas, rng, leaves, centre, direction, length, rng.normal(0, 0.35), leaf_size, leaf_size * 0.09, 1.35)
    # Leaves over the middle, where the twigs meet.
    for _ in range(twigs + 6):
        r = (radius - leaf_size) * 0.55 * math.sqrt(rng.uniform())
        a = rng.uniform(0, math.tau)
        p = np.array(centre) + r * np.array([math.cos(a), math.sin(a)])
        leaf = leaves[rng.integers(len(leaves))]
        canvas.stamp(leaf, p, rng.uniform(0, math.tau), leaf_size * rng.uniform(0.8, 1.05) / leaf.length, leaf_tint(rng))


def directional_spray(canvas, rng, leaves, base, length, leaf_size):
    """A branch end growing up the card from its base: a main twig with side twigs."""
    main = curve(base, -math.pi / 2 + rng.normal(0, 0.1), length, rng.normal(0, 0.35), steps=8)
    canvas.line(main, leaf_size * 0.16, leaf_size * 0.05, TWIG)
    sides = 10
    for i in range(sides):
        t = 0.14 + 0.74 * i / (sides - 1)
        p, heading = along(main, t)
        side = 1 if i % 2 else -1
        leafy_twig(canvas, rng, leaves, p, heading + side * rng.uniform(0.55, 0.95),
                   length * (0.44 - 0.2 * t) * rng.uniform(0.85, 1.1), -side * 0.4, leaf_size * 0.95, leaf_size * 0.08, 1.3)
    leafy_twig(canvas, rng, leaves, along(main, 0.85)[0], along(main, 0.85)[1], length * 0.2, 0.0, leaf_size * 0.9, leaf_size * 0.07)


def broadleaf_atlas(images):
    rng = np.random.default_rng(7)
    sources = [
        ("searsia_lucida", ["Diffuse", "Alpha"], [
            ((517, 29, 599, 205), (560, 205)), ((228, 75, 340, 172), (230, 172)),
            ((56, 79, 193, 216), (60, 216)), ((285, 140, 453, 245), (285, 245)),
            ((33, 213, 176, 397), (40, 397)), ((373, 235, 510, 386), (373, 386)),
            ((186, 239, 330, 404), (190, 404))], 0),
        ("tree_small_02", ["leaves_diff", "leaves_alpha"], [
            ((594, 34, 806, 181), (806, 181)), ((481, 96, 692, 271), (692, 271)),
            ((716, 174, 848, 296), (716, 296)), ((533, 272, 699, 409), (699, 409)),
            ((579, 408, 705, 534), (705, 534)), ((588, 538, 701, 635), (701, 635)),
            ((25, 569, 248, 756), (248, 756)), ((55, 732, 246, 873), (246, 873)),
            ((242, 766, 451, 894), (451, 894))], 13),
    ]
    leaves = []
    for asset, keys, boxes, clean in sources:
        for box, base in boxes:
            leaves.append(cut(images, asset, keys, box, base, clean))
    # Temperate summer greens (linear albedo), a few sun-bleached and a few deep.
    targets = [(0.050, 0.092, 0.020), (0.042, 0.082, 0.022), (0.060, 0.098, 0.018), (0.036, 0.070, 0.024)]
    leaves = [grade(l, targets[i % len(targets)]) for i, l in enumerate(leaves)]

    half = WORK // 2
    canvas = Canvas(WORK, WORK)
    # 0: round cluster; 1: a second, looser one.
    radial_cluster(canvas.region(0, 0, half, half), rng, leaves, (half / 2, half / 2), half * 0.48, 58, 8)
    radial_cluster(canvas.region(half, 0, half, half), rng, leaves, (half / 2, half / 2), half * 0.47, 64, 7)
    # 2: a branch end growing up from the bottom edge.
    directional_spray(canvas.region(0, half, half, half), rng, leaves, (half / 2, half - 6), half * 0.8, 56)
    # 3: a dense lobed clump for distant crowns: several clusters, the lower ones shaded.
    clump = canvas.region(half, half, half, half)
    for (x, y) in [(0.3, 0.66), (0.7, 0.64), (0.5, 0.7), (0.32, 0.34), (0.68, 0.33), (0.5, 0.5), (0.5, 0.3)]:
        radial_cluster(clump, rng, leaves, (x * half, y * half), half * 0.27, 50, 6)
    return finish(canvas.p)


def conifer_atlas(images):
    rng = np.random.default_rng(11)
    fir_keys = ["twig_diff", "twig_alpha"]
    firs = [cut(images, "fir_tree_01", fir_keys, box, base, join=11) for box, base in [
        ((313, 410, 660, 794), (494, 794)), ((650, 463, 975, 850), (808, 850)),
        ((194, 48, 426, 319), (313, 319)), ((669, 53, 960, 378), (826, 378)),
        ((506, 266, 646, 409), (553, 409)), ((553, 101, 603, 234), (571, 234))]]
    firs = [grade(f, t) for f, t in zip(firs, [(0.026, 0.052, 0.025), (0.03, 0.056, 0.024), (0.024, 0.048, 0.027)] * 2)]
    pines = [cut(images, "pine_tree_01", fir_keys, box, base, join=11) for box, base in [
        ((36, 45, 225, 452), (137, 452)), ((667, 350, 958, 530), (670, 480))]]
    pines = [grade(p, t) for p, t in zip(pines, [(0.03, 0.052, 0.02), (0.034, 0.056, 0.02)])]

    canvas = Canvas(WORK, WORK)
    half = WORK // 2

    # Top half: one fir branch seen from above, trunk end on the left, 2:1.
    frond = canvas.region(0, 0, WORK, half)
    fir_frond(frond, rng, firs, (6, half * 0.5), WORK - 20, half * 0.5)

    # Bottom left: a pine tuft seen from above, needles radiating from the middle.
    tuft = canvas.region(0, half, half, half)
    centre = np.array([half / 2, half / 2])
    for ring, (count, reach) in enumerate([(9, 0.47), (7, 0.36), (5, 0.24)]):
        for i in range(count):
            a = i * math.tau / count + ring * 0.45 + rng.normal(0, 0.12)
            sprig = pines[rng.integers(len(pines))]
            start = centre + np.array([math.cos(a), math.sin(a)]) * half * 0.04
            k = rng.uniform(0.8, 1.15)
            tuft.stamp(sprig, start, a, half * reach * rng.uniform(0.9, 1.05) / sprig.length,
                       (k * 1.05, k, k * 0.9), shade=0.3)

    for i in range(6):
        a = rng.uniform(0, math.tau)
        sprig = pines[rng.integers(len(pines))]
        tuft.stamp(sprig, centre, a, half * 0.14 / sprig.length, shade=0.2)

    # Bottom right: a whole young fir from the side, for the most distant crowns.
    tree = canvas.region(half, half, half, half)
    tree.line([(half / 2, half - 2), (half / 2, half * 0.45)], 14, 6, CONIFER_TWIG)
    levels = 11
    for level in range(levels):
        t = level / (levels - 1)
        y = half * (0.86 - 0.74 * t)
        reach = half * 0.47 * (1 - t) ** 0.9 + 18
        for side in (-1, 1):
            f = firs[rng.integers(2)]
            droop = side * (math.pi / 2 - 0.5 - rng.uniform(0, 0.25))
            ang = -math.pi / 2 + droop
            k = rng.uniform(0.75, 1.1) * (0.8 + 0.3 * t)
            tree.stamp(f, (half / 2, y), ang, reach / f.length, (k, k, k), shade=0.45, squash=0.8)
        f = firs[rng.integers(len(firs))]
        tree.stamp(f, (half / 2, y + 4), -math.pi / 2, reach * 0.5 / f.length, (0.9, 0.9, 0.9), shade=0.3)
    top = firs[5]
    tree.stamp(top, (half / 2, half * 0.2), -math.pi / 2, half * 0.16 / top.length, shade=0.2)
    return finish(canvas.p)


def fir_frond(canvas, rng, firs, base, length, width):
    """A flat fir branch pointing +x: a bare stem near the trunk, branchlets in
    pairs, widest two thirds out."""
    stem = curve(base, 0.0, length * 0.84, rng.normal(0, 0.05), steps=8)
    canvas.line(stem, 8, 2, CONIFER_TWIG)
    pairs = 9
    for i in range(pairs):
        t = 0.12 + 0.8 * i / (pairs - 1)
        p, heading = along(stem, t)
        envelope = math.sin(math.pi * min(1.0, 0.15 + t * 0.95)) ** 0.7
        for side in (-1, 1):
            f = firs[rng.integers(len(firs))]
            reach = width * 0.95 * envelope * rng.uniform(0.85, 1.1)
            ang = heading + side * rng.uniform(0.75, 1.05)
            k = rng.uniform(0.8, 1.15)
            canvas.stamp(f, p, ang, max(reach, 20) / f.length, (k, k, k), shade=0.35)
    tip = firs[rng.integers(2)]
    p, heading = along(stem, 0.8)
    canvas.stamp(tip, p, heading, length * 0.18 / tip.length, shade=0.25)


def finish(premultiplied):
    """Box-filters the 2x composition to SIZE, bleeds colour into the holes and
    packs linear RGB + coverage."""
    p = premultiplied.reshape(SIZE, 2, SIZE, 2, 4).mean((1, 3))
    color = bleed(p[..., :3], p[..., 3])
    out = np.dstack([linear_to_byte(color), linear_to_byte(p[..., 3])])
    return out.tobytes()


def bleed(premult, alpha):
    """Push-pull fill: every texel outside the leaves takes the colour of the
    nearest leaves, so filtering and mipmaps never pull in black."""
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


def bark(images, asset):
    """Linear albedo + roughness, renormalised OpenGL normal + occlusion."""
    def channel(key):
        image = images[(asset, key)]
        if image.mode in ("I;16", "I"):
            image = Image.fromarray((gray(image) * 255).astype(np.uint8))
        return np.asarray(image.convert("RGB").resize((SIZE, SIZE), Image.Resampling.LANCZOS)).astype(np.float32) / 255.0
    diffuse, normal, rough, ao = (channel(k) for k in ["Diffuse", "nor_gl", "Rough", "AO"])
    color = np.dstack([linear_to_byte(srgb_to_linear(diffuse)), linear_to_byte(rough[..., 0])])
    n = normal * 2 - 1
    n[..., 2] = np.maximum(n[..., 2], 0.08)  # resampling can overshoot past the horizon
    n /= np.maximum(np.linalg.norm(n, axis=2, keepdims=True), 1e-4)
    normals = np.dstack([linear_to_byte(n * 0.5 + 0.5), linear_to_byte(ao[..., 0])])
    return color.tobytes(), normals.tobytes()


if __name__ == "__main__":
    DEST.mkdir(parents=True, exist_ok=True)
    CACHE.mkdir(parents=True, exist_ok=True)
    records, images = fetch_all()
    (DEST / "broadleaf.rgba").write_bytes(broadleaf_atlas(images))
    (DEST / "conifer.rgba").write_bytes(conifer_atlas(images))
    for asset, name in [("bark_brown_02", "bark"), ("pine_bark", "pine_bark")]:
        color, normal = bark(images, asset)
        (DEST / (name + "_color.rgba")).write_bytes(color)
        (DEST / (name + "_normal.rgba")).write_bytes(normal)
    (DEST / "sources.json").write_text(json.dumps(records, indent=2, sort_keys=True) + "\n")
    print("Packed six 512x512 RGBA8 foliage layers.")
