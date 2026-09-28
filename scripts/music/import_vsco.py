#!/usr/bin/env python3
"""Imports recorded instruments from VSCO 2 Community Edition into data/music/samples.

VSCO 2 CE (https://github.com/sgossner/VSCO-2-CE) is CC0: free for any use,
no credit needed. Only the files listed in SETS below are fetched (cached in
~/.cache/meridian-vsco), then each note is:

- pitch-checked (pyin), so a set's octave naming and each note's tuning come
  from the audio, not the file name;
- trimmed to its onset, cut to a set length, and for held sets given a loop in
  its steady part with the crossfade baked in, so a note can last any length;
- for held sets, marked where it speaks (`speak`, see SPEAK_DB): a bowed or
  blown note swells for up to half a second, by a different time on every
  note, and the sampler starts there so every note lands on the beat;
- levelled: every set's loudest layer to one loudness (its first second), each
  softer layer 7 dB under the one above it; the recordings' own gaps (up to
  28 dB, mostly microphone gain) would leave soft notes inaudible in a mix, and
  the soft takes still sound soft;
- written as Ogg Vorbis at 32 kHz.

Each set gets data/music/samples/<set>/set.ron, which the Sampler instrument
reads. Run with the listen venv (it has numpy, librosa, soundfile, ffmpeg):

    .venv-listen/bin/python scripts/music/import_vsco.py [set ...] [--force]

`--speak` only re-measures where the notes of the held sets already on disk
speak and rewrites their set.ron (no download, no re-encode).
"""

import os
import re
import subprocess
import sys
import urllib.parse
import urllib.request
from pathlib import Path

import imageio_ffmpeg
import librosa
import numpy as np
import soundfile as sf

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "data" / "music" / "samples"
CACHE = Path.home() / ".cache" / "meridian-vsco"
BASE = "https://raw.githubusercontent.com/sgossner/VSCO-2-CE/master/"
RATE = 32000
NAMES = {"C": 0, "C#": 1, "D": 2, "D#": 3, "E": 4, "F": 5, "F#": 6, "G": 7, "G#": 8, "A": 9, "A#": 10, "B": 11}

# name: title, files (paths in the library), how to read note/layer/round from a
# file name, which layers to keep, and how to cut it.
#   held:   loop the steady part (strings, brass, winds); else play to the end
#   length: seconds kept (held: at most, before the loop end)
#   mono:   fold to mono (low instruments, where width is only noise)
#   keys:   for unpitched sets, file -> (key, layer)
SETS = {
    "violins": dict(
        title="Violins (section, held)",
        dir="Strings/Violin Section/susVib",
        layers=["v1", "v2"],
        held=True,
        length=4.5,
    ),
    "violins_trem": dict(
        title="Violins (section, tremolo)",
        dir="Strings/Violin Section/Trem",
        layers=["v1", "v2"],
        held=True,
        length=4.0,
    ),
    "violas": dict(
        title="Violas (section, held)",
        dir="Strings/Viola Section/susvib",
        layers=["v1", "v2"],
        held=True,
        length=4.5,
    ),
    "cellos": dict(
        title="Cellos (section, held)",
        dir="Strings/Cello Section/susvib",
        layers=["v1", "v3"],
        held=True,
        length=4.5,
    ),
    "cellos_spic": dict(
        title="Cellos (section, spiccato)",
        dir="Strings/Cello Section/spic",
        layers=["v1", "v2"],
        held=False,
        length=0.9,
    ),
    "basses": dict(
        title="Double bass (held)",
        dir="Strings/Solo Contrabass/SusNV",
        layers=["v1", "v3"],
        rounds=["rr1"],
        held=True,
        length=4.5,
        mono=True,
    ),
    "horns": dict(
        title="French horn (held)",
        dir="Brass/F Horn/sus",
        layers=["v1", "v3"],
        held=True,
        length=4.0,
    ),
    "horns_short": dict(
        title="French horn (short)",
        dir="Brass/F Horn/stac",
        layers=["v1", "v3"],
        held=False,
        length=1.2,
    ),
    "trombones": dict(
        title="Tenor trombone (held)",
        dir="Brass/Tenor Trombone/sus",
        layers=["v1", "v3"],
        held=True,
        length=4.0,
    ),
    "tuba": dict(
        title="Tuba (held)",
        dir="Brass/Tuba/sus",
        layers=["v1", "v3"],
        held=True,
        length=4.0,
        mono=True,
    ),
    "harp": dict(
        title="Harp",
        dir="Strings/Harp",
        layers=None,
        held=False,
        length=5.0,
    ),
    "piano": dict(
        title="Upright piano",
        dir="Keys/Upright Nr1",
        match=r"UR1_",
        layers=["pp", "f"],
        rounds=["RR1"],
        held=False,
        length=7.0,
    ),
    "timpani": dict(
        title="Timpani",
        dir="Percussion/Timpani",
        match=r"_Hit_",
        layers=["v1", "v4"],
        held=False,
        length=4.0,
        mono=True,
        # Five drums with no note in their names: each drum's note is its principal
        # partial (see `principal`), the median over all its hits.
        drum=r"Timpani(\d)",
    ),
}

# The orchestral percussion kit: unpitched, one file per key and layer.
KIT = dict(
    title="Orchestral percussion",
    length=6.0,
    keys={
        # Gran cassa (concert bass drum)
        "VSCO 1 Percussion/drums/bass/bdrum_mp_1.wav": (34, 0),
        "VSCO 1 Percussion/drums/bass/bdrum_fff_1.wav": (34, 1),
        # Large ethnic drum, mallet: the nearest thing to a taiko
        "VSCO 1 Percussion/drums/other/ethnic/giant/mallet/EthnicLargeMallet_hit_mf_1.wav": (35, 0),
        "VSCO 1 Percussion/drums/other/ethnic/giant/mallet/EthnicLargeMallet_hit_ff_1.wav": (35, 1),
        # Large ethnic drum, sticks: a higher, drier war drum
        "VSCO 1 Percussion/drums/other/ethnic/giant/sticks/EthnicLargeSticks_hit_mp_1.wav": (41, 0),
        "VSCO 1 Percussion/drums/other/ethnic/giant/sticks/EthnicLargeSticks_hit_fff_1.wav": (41, 1),
        # Snare
        "VSCO 1 Percussion/drums/snare/drum1/snare1_mp_1.wav": (38, 0),
        "VSCO 1 Percussion/drums/snare/drum1/snare1_ff_1.wav": (38, 1),
        # Suspended cymbal
        "Percussion/susCymb1-hit_mp_rr1.wav": (49, 0),
        "Percussion/susCymb1-hit_fff_rr1.wav": (49, 1),
        # Cymbal swell
        "Percussion/susCymb1-cresc-Median_v1.wav": (51, 0),
        # Tam-tam
        "Percussion/gongHit_p.wav": (52, 0),
        "Percussion/gongHit_fff.wav": (52, 1),
    },
)


def tree():
    """Every file path in the library (from the GitHub tree, cached)."""
    import json

    CACHE.mkdir(parents=True, exist_ok=True)
    p = CACHE / "tree.json"
    if not p.exists():
        url = "https://api.github.com/repos/sgossner/VSCO-2-CE/git/trees/master?recursive=1"
        p.write_bytes(urllib.request.urlopen(url, timeout=60).read())
    t = json.loads(p.read_text())
    return [e["path"] for e in t["tree"] if e["type"] == "blob"]


def fetch(path):
    dst = CACHE / path
    if not dst.exists():
        dst.parent.mkdir(parents=True, exist_ok=True)
        url = BASE + urllib.parse.quote(path)
        tmp = dst.with_suffix(".part")
        subprocess.run(
            ["curl", "-sSfL", "--retry", "8", "--retry-all-errors", "--retry-delay", "2",
             "-m", "300", "-o", str(tmp), url],
            check=True,
        )
        tmp.rename(dst)
    return dst


def load(path, mono):
    a, sr = sf.read(fetch(path), dtype="float32", always_2d=True)
    if a.shape[1] == 1:
        a = np.repeat(a, 2, axis=1)
    a = a[:, :2]
    if mono:
        a = a.mean(axis=1, keepdims=True)
    if sr != RATE:
        a = librosa.resample(a.T, orig_sr=sr, target_sr=RATE, res_type="soxr_hq").T
    return np.ascontiguousarray(a, dtype=np.float32)


def name_note(fname):
    m = re.search(r"_([A-G]#?)(-?\d)(?:_|\.|$)", fname)
    if not m:
        return None
    return (int(m.group(2)) + 1) * 12 + NAMES[m.group(1)]


def detect(a, short):
    """Median pitch (MIDI, fractional) of the note's first steady second (short notes: its start)."""
    x = a.mean(axis=1)
    x = x[int(0.03 * RATE) : int(0.5 * RATE)] if short else x[int(0.15 * RATE) : int(1.6 * RATE)]
    f0, voiced, _ = librosa.pyin(x, fmin=25, fmax=4200, sr=RATE, frame_length=4096)
    f0 = f0[voiced & ~np.isnan(f0)]
    if len(f0) < 5:
        return None
    return float(np.median(librosa.hz_to_midi(f0)))


def principal(a):
    """A timpani's note (MIDI): the lowest strong partial with another a fifth above it.

    A kettledrum's heard pitch is its principal mode, whose partials sit near 1, 1.5
    and 2 times it; a pitch tracker tends to hear an octave under it.
    """
    x = a.mean(axis=1)[int(0.08 * RATE) : int(1.2 * RATE)]
    x = x * np.hanning(len(x))
    sp = np.abs(np.fft.rfft(x, 1 << 17))
    fr = np.fft.rfftfreq(1 << 17, 1 / RATE)
    band = (fr > 50) & (fr < 500)
    sp, fr = sp[band], fr[band]
    peaks = []
    for i in np.argsort(sp)[::-1]:
        m = float(librosa.hz_to_midi(fr[i]))
        if all(abs(m - p) > 0.6 for p, _ in peaks):
            peaks.append((m, sp[i]))
        if len(peaks) == 8:
            break
    strong = [p for p, v in peaks if v > 0.2 * peaks[0][1]]
    fifths = [p for p in strong if any(abs(q - p - 7.0) < 0.8 for q in strong)]
    return min(fifths) if fifths else peaks[0][0]


def onset(a):
    env = np.abs(a).max(axis=1)
    thr = env.max() * 0.02
    i = int(np.argmax(env > thr))
    return max(0, i - int(0.004 * RATE))


# A held note speaks where it first comes within this of its loudest in its first
# second (5 ms steps); the sampler starts it SPEAK_LEAD earlier, so the bow or
# breath still starts the note.
SPEAK_DB = -8.0
SPEAK_LEAD = 0.015


def speak(a):
    hop = int(0.005 * RATE)
    env = rms_env(a[:RATE], hop)
    i = int(np.argmax(env >= env.max() * 10 ** (SPEAK_DB / 20)))
    return max(0, i * hop - int(SPEAK_LEAD * RATE))


def rms_env(a, hop):
    x = (a**2).mean(axis=1)
    n = len(x) // hop
    return np.sqrt(x[: n * hop].reshape(n, hop).mean(axis=1) + 1e-12)


# Loudness of a set's loudest layer (RMS of each note's first second), and the
# step down to each softer layer.
LEVEL_DB = -20.0
LAYER_STEP_DB = 7.0


def layer_gains(notes):
    """Gain per layer for [(layer, audio)], without letting any note clip."""
    def first_second_db(a):
        x = a[: RATE].mean(axis=1)
        return 20 * np.log10(np.sqrt((x**2).mean()) + 1e-9)

    layers = sorted({layer for layer, _ in notes})
    gains = {}
    for layer in layers:
        level = np.median([first_second_db(a) for l, a in notes if l == layer])
        want = LEVEL_DB - LAYER_STEP_DB * (len(layers) - 1 - layers.index(layer))
        gains[layer] = 10 ** ((want - level) / 20)
    peak = max(float(np.abs(a).max()) * gains[layer] for layer, a in notes)
    if peak > 0.97:
        gains = {k: v * 0.97 / peak for k, v in gains.items()}
    return gains


def cut_held(a, length):
    """Cuts a held note and bakes a loop into its steady part: (audio, loop_start, loop_end).

    The loop start is slid (by up to a quarter second) to where the audio best matches
    what the loop's tail crossfades into, so the two halves of the seam are in phase and
    do not cancel. A short tail after the loop end continues from the loop start, so a
    decoder that trims a few samples never cuts into the loop.
    """
    from scipy.signal import correlate

    hop = int(0.05 * RATE)
    env = rms_env(a, hop)
    t0 = int(0.7 * RATE / hop)
    if len(env) <= t0 + 10:
        t0 = len(env) // 3
    med = np.median(env[t0:])
    steady = np.nonzero(env[t0:] > 0.55 * med)[0]
    t1 = (t0 + steady[-1]) if len(steady) else len(env) - 1
    end = min(t1 * hop, int(length * RATE), len(a))
    start = int(t0 * hop + (end - t0 * hop) * 0.2)
    if end - start < int(0.8 * RATE):
        start = max(int(0.4 * RATE), end - int(0.8 * RATE))
    xf = min(int(0.3 * RATE), (end - start) // 3)
    # Best start: the xf frames before it should look like the xf frames before `end`.
    mono = a.mean(axis=1)
    tail = mono[end - xf : end]
    slide = min(int(0.25 * RATE), start - xf - 1, end - start - xf - 1)
    if slide > 16:
        lo, hi = start - slide, start + slide
        region = mono[lo - xf : hi]
        corr = correlate(region, tail, mode="valid")
        energy = np.sqrt(np.convolve(region**2, np.ones(xf), mode="valid")) * np.sqrt((tail**2).sum()) + 1e-12
        best = int(np.argmax(corr / energy))
        start = lo + best
    # Many held takes swell as they go; a loop of a swell pumps. Inside the loop the
    # level is held at what it is where the loop starts.
    a = a.copy()
    lvl = rms_env(a, hop)
    smooth = np.convolve(lvl, np.ones(5) / 5, mode="same")
    at = np.arange(len(a)) / hop
    curve = np.interp(at, np.arange(len(smooth)) + 0.5, smooth)
    target = curve[start]
    gain = np.ones(len(a), dtype=np.float32)
    gain[start:end] = np.clip(target / np.maximum(curve[start:end], 1e-6), 0.35, 2.8)
    a *= gain[:, None]
    mono = a.mean(axis=1)
    tail = mono[end - xf : end]
    out = a[:end].copy()
    t = np.linspace(0.0, 1.0, xf, dtype=np.float32)[:, None]
    head = a[start - xf : start]
    c = float((tail * head.mean(axis=1)).sum() / (np.sqrt((tail**2).sum() * (head.mean(axis=1) ** 2).sum()) + 1e-12))
    if c > 0.5:
        # In phase: a straight crossfade keeps the level.
        fade_out, fade_in = 1.0 - t, t
    else:
        fade_out, fade_in = np.cos(t * np.pi / 2), np.sin(t * np.pi / 2)
    out[end - xf : end] = a[end - xf : end] * fade_out + head * fade_in
    extra = a[start : start + int(0.05 * RATE)]
    return np.concatenate([out, extra]), start, end


def cut_shot(a, length):
    end = min(len(a), int(length * RATE))
    out = a[:end].copy()
    # Trailing silence goes; the last 60 ms fade to nothing.
    env = np.abs(out).max(axis=1)
    live = np.nonzero(env > env.max() * 0.0015)[0]
    if len(live):
        out = out[: min(len(out), live[-1] + int(0.05 * RATE))]
    f = min(len(out) // 4, int(0.06 * RATE))
    out[-f:] *= np.linspace(1.0, 0.0, f, dtype=np.float32)[:, None]
    return out


def encode(a, dst):
    dst.parent.mkdir(parents=True, exist_ok=True)
    wav = dst.with_suffix(".tmp.wav")
    sf.write(wav, a, RATE, subtype="FLOAT")
    subprocess.run(
        [imageio_ffmpeg.get_ffmpeg_exe(), "-y", "-loglevel", "error", "-i", str(wav),
         "-c:a", "libvorbis", "-q:a", "5", str(dst)],
        check=True,
    )
    wav.unlink()


def write_set(name, title, pitched, held, zones, mono):
    lines = [
        "// Generated by scripts/music/import_vsco.py from VSCO 2 Community Edition (CC0).",
        "(",
        f'    title: "{title}",',
        f"    rate: {RATE},",
        f"    pitched: {'true' if pitched else 'false'},",
        f"    held: {'true' if held else 'false'},",
        "    zones: [",
    ]
    for z in zones:
        loop = f", loop: ({z['ls']}, {z['le']})" if "ls" in z else ""
        loop += f", speak: {z['speak']}" if z.get("speak") else ""
        lines.append(
            f"        (file: \"{z['file']}\", key: {z['key']}, cents: {z['cents']:.1f}, "
            f"layer: {z['layer']}{loop}),"
        )
    lines += ["    ],", ")", ""]
    (OUT / name / "set.ron").write_text("\n".join(lines))
    print(f"{name}: {len(zones)} zones, {'mono' if mono else 'stereo'}")


def import_set(name, spec, files):
    pat = spec.get("match")
    picked = []
    for p in files:
        if not p.startswith(spec["dir"] + "/") or not p.lower().endswith(".wav"):
            continue
        f = p.rsplit("/", 1)[1]
        if pat and not re.search(pat, f):
            continue
        toks = re.split(r"[_.]", f)
        layer = 0
        if spec["layers"]:
            hit = [t for t in toks if t in spec["layers"]]
            if not hit:
                continue
            layer = spec["layers"].index(hit[0])
        rounds = spec.get("rounds")
        if rounds and not any(t in rounds for t in toks):
            continue
        picked.append((p, f, layer))
    if not picked:
        sys.exit(f"{name}: no files matched")
    mono = spec.get("mono", False)
    audio = []
    for p, f, layer in picked:
        a = load(p, mono)
        a = a[onset(a) :]
        guess = principal(a) if "drum" in spec else detect(a, not spec["held"])
        audio.append((f, layer, a, guess))
    if "drum" in spec:
        heard = {}
        for f, _, _, guess in audio:
            if guess is not None:
                heard.setdefault(re.search(spec["drum"], f).group(1), []).append(guess)
        audio = [
            (f, layer, a, float(np.median(heard[d])) if (d := re.search(spec["drum"], f).group(1)) in heard else None)
            for f, layer, a, _ in audio
        ]
    # The set's octave naming: the offset most notes agree on.
    offsets = []
    for f, _, _, guess in audio:
        n = name_note(f)
        if n is not None and guess is not None:
            offsets.append(round((guess - n) / 12) * 12)
    offset = max(set(offsets), key=offsets.count) if offsets else 0
    gains = layer_gains([(layer, a) for _, layer, a, _ in audio])
    zones = []
    rr = {}
    for f, layer, a, guess in audio:
        n = name_note(f)
        if n is not None:
            key = n + offset
            cents = 0.0 if guess is None else float(np.clip((guess - key) * 100, -60, 60))
            if guess is not None and abs(guess - key) > 1.0:
                octaves = round((guess - key) / 12)
                if abs(guess - key - 12 * octaves) < 0.7:
                    # The pitch tracker jumped an octave; the tuning still holds.
                    cents = float(np.clip((guess - key - 12 * octaves) * 100, -60, 60))
                else:
                    # A short or fading note the tracker could not follow: trust the name.
                    print(f"  {f}: name says {key}, audio says {guess:.2f}; using the name")
                    cents = 0.0
        elif guess is not None:
            key = int(round(guess))
            cents = (guess - key) * 100
        else:
            print(f"  {f}: no pitch; skipped")
            continue
        a = a * gains[layer]
        z = dict(key=key, cents=cents, layer=layer)
        if spec["held"]:
            a, ls, le = cut_held(a, spec["length"])
            z["ls"], z["le"] = ls, le
            z["speak"] = min(speak(a), ls - 1)
        else:
            a = cut_shot(a, spec["length"])
        k = (key, layer)
        rr[k] = rr.get(k, 0) + 1
        z["file"] = f"{key}_{layer}_{rr[k]}.ogg"
        encode(a, OUT / name / z["file"])
        zones.append(z)
    zones.sort(key=lambda z: (z["key"], z["layer"], z["file"]))
    write_set(name, spec["title"], True, spec["held"], zones, mono)


def import_kit():
    name = "percussion"
    zones = []
    audio = []
    for p, (key, layer) in KIT["keys"].items():
        audio.append((key, layer, load(p, False)))
    # Each drum levelled on its own (they are different instruments), layers as above.
    gains = {}
    for key in {k for k, _, _ in audio}:
        g = layer_gains([(layer, a) for k, layer, a in audio if k == key])
        gains.update({(key, layer): v for layer, v in g.items()})
    for key, layer, a in audio:
        a = cut_shot(a[onset(a) :] * gains[(key, layer)], KIT["length"])
        f = f"{key}_{layer}_1.ogg"
        encode(a, OUT / name / f)
        zones.append(dict(key=key, cents=0.0, layer=layer, file=f))
    zones.sort(key=lambda z: (z["key"], z["layer"]))
    write_set(name, KIT["title"], False, False, zones, False)


def respeak(name):
    """Re-measures `speak` for a held set on disk and rewrites its set.ron."""
    path = OUT / name / "set.ron"
    out = []
    for line in path.read_text().splitlines():
        m = re.search(r'file: "([^"]+)".*loop: \((\d+), (\d+)\)', line)
        if m:
            a, sr = sf.read(OUT / name / m.group(1), dtype="float32", always_2d=True)
            assert sr == RATE
            at = min(speak(a), int(m.group(2)) - 1)
            line = re.sub(r", speak: \d+", "", line)
            line = line.replace("),", f", speak: {at}),") if at else line
        out.append(line)
    path.write_text("\n".join(out) + "\n")
    print(f"{name}: speak re-measured")


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    force = "--force" in sys.argv
    if "--speak" in sys.argv:
        for name in args or [n for n, spec in SETS.items() if spec["held"]]:
            respeak(name)
        return
    files = tree()
    want = args or list(SETS) + ["percussion"]
    for name in want:
        if (OUT / name / "set.ron").exists() and not force:
            print(f"{name}: exists (--force to redo)")
            continue
        if (OUT / name).exists():
            for old in (OUT / name).iterdir():
                old.unlink()
        if name == "percussion":
            import_kit()
        else:
            import_set(name, SETS[name], files)


if __name__ == "__main__":
    main()
