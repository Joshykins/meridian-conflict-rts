"""Scores the drum detector against a song whose notes are known.

    .venv-listen/bin/python scripts/listen/eval_drums.py <song.ron> <drums stem wav> [--offset S]

Truth comes from the song itself: every drum note of every section in the
arrangement, at its time. Kit keys map to the detector's classes (kick, snare,
tom, cym). Prints recall/precision/F1 per class within 40 ms.
"""

import json
import os
import subprocess
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(__file__))
import pipeline  # noqa: E402

CLASS = {36: "kick", 38: "snare", 39: "snare", 40: "snare", 37: "snare", 35: "tom", 43: "tom", 41: "tom", 45: "tom", 48: "tom", 34: "tom",
         42: "cym", 44: "cym", 46: "cym", 49: "cym", 51: "cym", 70: "cym"}


def truth(song_ron):
    song = json.loads(subprocess.check_output([pipeline.mc_music(), "json", song_ron]))
    spb = 60.0 / song["tempo"]
    pats = {p["name"]: p for p in song["patterns"]}
    secs = {s["name"]: s for s in song["sections"]}
    kit_tracks = {t["name"] for t in song["tracks"] if "Kit" in t["instrument"]}
    out = {c: [] for c in set(CLASS.values())}
    t0 = 0.0
    for name in song["arrangement"]:
        s = secs[name]
        sec_beats = s["bars"] * song.get("beats_per_bar", 4)
        for c in s["clips"]:
            if c["track"] not in kit_tracks:
                continue
            p = pats[c["pattern"]]
            reps = c["times"] or int(np.ceil((sec_beats - c["at"]) / p["beats"]))
            for r in range(reps):
                base = c["at"] + r * p["beats"]
                for tick, _len, key, vel in p["notes"]:
                    beat = base + tick / 96.0
                    if beat >= sec_beats or key not in CLASS:
                        continue
                    out[CLASS[key]].append((t0 + beat * spb, vel))
        t0 += sec_beats * spb
    return out


def score(hits, tru, tol=0.04):
    rows = {}
    for c, tr in tru.items():
        tr = np.array(sorted(t for t, _ in tr))
        det = np.array(sorted(t for t, cls, _ in hits if cls == c))
        if len(tr) == 0 and len(det) == 0:
            continue
        rec = np.mean([np.min(np.abs(det - t)) < tol for t in tr]) if len(det) and len(tr) else 0.0
        pre = np.mean([np.min(np.abs(tr - d)) < tol for d in det]) if len(det) and len(tr) else 0.0
        f1 = 2 * rec * pre / (rec + pre) if rec + pre else 0.0
        rows[c] = (rec, pre, f1, len(det), len(tr))
    return rows


def main():
    song, stem = sys.argv[1], sys.argv[2]
    tru = truth(song)
    y = pipeline.load_mono(stem, 22050)
    hits = pipeline.drum_hits(y, 22050)
    for c, (rec, pre, f1, nd, nt) in sorted(score(hits, tru).items()):
        print(f"{c:<6} recall {rec * 100:5.1f}%  precision {pre * 100:5.1f}%  F1 {f1 * 100:5.1f}%   ({nd} found, {nt} true)")


if __name__ == "__main__":
    main()
