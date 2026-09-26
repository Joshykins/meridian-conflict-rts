"""Helpers for writing songs as code: notes, chords, patterns, sections.

Times are in beats (floats) here and turned into ticks (PPQ 96) on output.
"""

import os
import sys

from ron import V, Newtype, dump

PPQ = 96
NAMES = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}


def n(name):
    """'C4' -> 60, 'Eb3' -> 51, 'F#2' -> 42."""
    letter = name[0].upper()
    rest = name[1:]
    acc = 0
    while rest and rest[0] in "#b":
        acc += 1 if rest[0] == "#" else -1
        rest = rest[1:]
    return 12 * (int(rest) + 1) + NAMES[letter] + acc


def keys(spec):
    """'C4 Eb4 G4' -> [60, 63, 67]."""
    return [n(s) for s in spec.split()]


class Pattern:
    def __init__(self, name, beats):
        self.name = name
        self.beats = beats
        self.notes = []
        self.automation = []

    def add(self, at, length, key, vel=100):
        """A note `at` beats in, `length` beats long."""
        if isinstance(key, str):
            key = n(key)
        t = int(round(at * PPQ))
        l = max(1, int(round(length * PPQ)))
        self.notes.append((t, l, int(key), int(max(1, min(127, vel)))))
        return self

    def chord(self, at, length, ks, vel=90, strum=0.0):
        if isinstance(ks, str):
            ks = keys(ks)
        for i, k in enumerate(ks):
            self.add(at + i * strum, length - i * strum, k, vel)
        return self

    def seq(self, at, items, vel=100, gap=1.0):
        """Items are (key, beats) or (key, beats, vel); key None is a rest. `gap` scales length (0.9 = detached)."""
        t = at
        for it in items:
            k, d = it[0], it[1]
            v = it[2] if len(it) > 2 else vel
            if k is not None:
                self.add(t, d * gap, k, v)
            t += d
        return self

    def auto(self, target, points):
        """points: [(beat, value 0..1)]."""
        self.automation.append({"target": V(target), "points": [(int(round(b * PPQ)), float(v)) for b, v in points]})
        return self

    def to_ron(self):
        notes = sorted(self.notes, key=lambda x: (x[0], x[2]))
        d = {"name": self.name, "beats": int(self.beats), "notes": notes}
        if self.automation:
            d["automation"] = self.automation
        return d


def grid(p, rows, steps_per_beat=4, at=0.0, length=None):
    """Drums from strings, one per key: '.' rest, 'x' 100, 'X' 124, 'o' 76, 'g' 48 (ghost), '-' ties nothing.

    rows: {key: "x...x...x...x..."}. Spaces and '|' are ignored so bars can be marked."""
    vel = {"x": 100, "X": 124, "o": 76, "g": 48, "q": 30}
    step = 1.0 / steps_per_beat
    for key, s in rows.items():
        s = s.replace(" ", "").replace("|", "")
        for i, c in enumerate(s):
            if c in vel:
                p.add(at + i * step, length or step, key, vel[c])
    return p


def arp(p, at, beats, chord_keys, step=0.25, order="up", vel=(90, 70), octaves=1, length=None, accent_every=4):
    """An arpeggio over `chord_keys` for `beats` beats."""
    if isinstance(chord_keys, str):
        chord_keys = keys(chord_keys)
    ks = []
    for o in range(octaves):
        ks += [k + 12 * o for k in chord_keys]
    if order == "updown":
        seqk = ks + ks[-2:0:-1]
    elif order == "down":
        seqk = ks[::-1]
    elif order == "pendulum":
        seqk = [ks[0]] + [x for k in ks[1:] for x in (k, ks[0])]
    else:
        seqk = ks
    count = int(round(beats / step))
    for i in range(count):
        v = vel[0] if i % accent_every == 0 else vel[1]
        p.add(at + i * step, length or step * 0.9, seqk[i % len(seqk)], v)
    return p


def clip(track, pattern, at=0, times=0, transpose=0):
    d = {"track": track, "pattern": pattern, "at": int(at), "times": int(times)}
    if transpose:
        d["transpose"] = int(transpose)
    return d


def section(name, bars, clips, kind="Loop", intensity=None, nxt=None, exit_every=0):
    d = {"name": name, "bars": bars, "kind": V(kind)}
    if intensity is not None:
        d["intensity"] = (float(intensity[0]), float(intensity[1]))
    if nxt:
        d["next"] = nxt
    if exit_every:
        d["exit_every"] = exit_every
    d["clips"] = clips
    return d


def track(name, instrument, db=0.0, pan=0.0, sends=None, effects=None, layer=None, follow=None, colour=0xE0603A):
    d = {"name": name, "instrument": instrument, "db": float(db), "pan": float(pan)}
    if sends:
        d["sends"] = [{"bus": b, "db": float(x)} for b, x in sends]
    if effects:
        d["effects"] = effects
    if layer:
        f, full, until = (list(layer) + [1.0])[:3]
        d["layer"] = {"from": float(f), "full": float(full), "until": float(until)}
    if follow:
        d["follow"] = [{"target": V(t), "from": float(a), "to": float(b)} for t, a, b in follow]
    d["colour"] = colour
    return d


def song(name, tempo, root, scale, tracks, buses, master, patterns, sections, arrangement, notes="", beats_per_bar=4):
    return {
        "name": name,
        "tempo": float(tempo),
        "beats_per_bar": beats_per_bar,
        "root": root,
        "scale": V(scale),
        "tracks": tracks,
        "buses": buses,
        "master": master,
        "patterns": [p.to_ron() for p in patterns],
        "sections": sections,
        "arrangement": arrangement,
        "notes": notes,
    }


def write(path, value, force=False, header=""):
    if os.path.exists(path) and not force:
        print(f"skip {path} (exists; --force overwrites, and loses edits made in the studio)")
        return
    text = dump(value) + "\n"
    if header:
        text = "".join("// " + l + "\n" for l in header.strip().splitlines()) + text
    with open(path, "w") as f:
        f.write(text)
    print(f"wrote {path}")


FORCE = "--force" in sys.argv
