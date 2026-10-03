"""Break a piece of music apart into something Claude can read and the studio can play.

    listen.sh <youtube-url | audio file | score.mid/.musicxml/.mxl> [--from 1:32] [--to 2:10] [--name NAME]

Audio goes: fetch (yt-dlp) -> separate into stems (Demucs: drums, bass, other,
vocals) -> beats and bars (librosa on the drum stem) -> notes (Basic Pitch on
bass and other) -> drum hits by band (kick / snare / cymbals) -> key, chords
per bar in Roman numerals (music21) -> sections (self-similarity) ->
`reference.ron`, an mc-music song of the transcription, and `report.md`.

A score (MIDI, MusicXML) skips straight to the notes: exact, no guessing.

Everything lands in data/music/references/<name>/ (ignored by git):
    source.wav        the audio (full length)
    stems/*.wav       Demucs stems
    notes/*.mid       Basic Pitch transcriptions per stem
    analysis.json     every number the report is made from
    reference.ron     the transcription as a song: open it in mc-studio, A/B with the stems
    report.md         the chart and the facts, for reading
"""

import argparse
import json
import math
import os
import re
import subprocess
import sys
from collections import Counter, defaultdict

import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
REFS = os.path.join(ROOT, "data", "music", "references")
PPQ = 96
NOTE = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"]


def log(*a):
    print("[listen]", *a, file=sys.stderr, flush=True)


def seconds(t):
    """'1:32' or '92.5' -> 92.5."""
    if t is None:
        return None
    if ":" in t:
        m, s = t.split(":", 1)
        return int(m) * 60 + float(s)
    return float(t)


def slug(s):
    s = re.sub(r"[^A-Za-z0-9]+", "_", s).strip("_").lower()
    return s[:60] or "reference"


# ---------------------------------------------------------------- fetching


def fetch(url, out_dir):
    """Downloads the audio of `url` as source.wav; returns (path, title)."""
    ffmpeg = ffmpeg_exe()
    os.makedirs(out_dir, exist_ok=True)
    wav = os.path.join(out_dir, "source.wav")
    meta = os.path.join(out_dir, "source.json")
    if os.path.exists(wav) and os.path.exists(meta):
        return wav, json.load(open(meta)).get("title", "")
    ytdlp = os.path.join(os.path.dirname(sys.executable), "yt-dlp")
    info = json.loads(subprocess.check_output([ytdlp, "--no-playlist", "-J", url]))
    subprocess.check_call(
        [ytdlp, "--no-playlist", "-f", "bestaudio", "-x", "--audio-format", "wav", "--ffmpeg-location", ffmpeg, "-o", os.path.join(out_dir, "source.%(ext)s"), url]
    )
    json.dump({"title": info.get("title", ""), "url": url, "duration": info.get("duration")}, open(meta, "w"), indent=1)
    return wav, info.get("title", "")


def ffmpeg_exe():
    """ffmpeg from the imageio-ffmpeg wheel: it comes from PyPI with the other tools, no system install."""
    import imageio_ffmpeg

    return imageio_ffmpeg.get_ffmpeg_exe()


def to_wav(path, out_dir):
    os.makedirs(out_dir, exist_ok=True)
    wav = os.path.join(out_dir, "source.wav")
    if not os.path.exists(wav):
        subprocess.check_call([ffmpeg_exe(), "-y", "-loglevel", "error", "-i", path, "-ac", "2", "-ar", "44100", wav])
    return wav


def youtube_id(url):
    m = re.search(r"(?:v=|youtu\.be/|shorts/)([A-Za-z0-9_-]{6,})", url)
    return m.group(1) if m else slug(url)


# ---------------------------------------------------------------- stems


def separate(wav, out_dir):
    """Demucs htdemucs into out_dir/stems/{drums,bass,other,vocals}.wav."""
    stems = os.path.join(out_dir, "stems")
    names = ["drums", "bass", "other", "vocals"]
    if all(os.path.exists(os.path.join(stems, f"{n}.wav")) for n in names):
        return {n: os.path.join(stems, f"{n}.wav") for n in names}
    log("separating stems (Demucs; a minute or two on the CPU)")
    tmp = os.path.join(out_dir, "demucs")
    subprocess.check_call(
        [sys.executable, "-m", "demucs", "-n", "htdemucs", "-d", "cpu", "-j", str(max(1, (os.cpu_count() or 4) // 2)), "-o", tmp, wav]
    )
    base = os.path.splitext(os.path.basename(wav))[0]
    src = os.path.join(tmp, "htdemucs", base)
    os.makedirs(stems, exist_ok=True)
    for n in names:
        os.replace(os.path.join(src, f"{n}.wav"), os.path.join(stems, f"{n}.wav"))
    return {n: os.path.join(stems, f"{n}.wav") for n in names}


def load_mono(path, sr=22050, start=None, end=None):
    import librosa

    offset = start or 0.0
    duration = None if end is None else max(0.1, end - offset)
    y, _ = librosa.load(path, sr=sr, mono=True, offset=offset, duration=duration)
    return y


# ---------------------------------------------------------------- time


def beats(drums, other, sr):
    """Tempo, beat times and the downbeat phase. Beats come from the drums when they
    carry rhythm, else from the whole texture."""
    import librosa

    # Drums weighted up but everything counted: sections without drums still have a beat.
    y = drums * 1.5 + other
    env = librosa.onset.onset_strength(y=y, sr=sr, aggregate=np.median)
    _, frames = librosa.beat.beat_track(onset_envelope=env, sr=sr, units="frames", tightness=120)
    times = librosa.frames_to_time(frames, sr=sr)
    # The tempo is what the beats actually are (median spacing), not the tracker's
    # headline figure, which can be double or half the beats it placed.
    def spacing_tempo(ts):
        return 60.0 / float(np.median(np.diff(ts))) if len(ts) > 2 else 120.0

    tempo = spacing_tempo(times)
    # One grid for the whole piece: fit beat time = start + index * period through the
    # tracked beats (robustly: twice, dropping beats more than 60 ms off the first fit),
    # then lay it from the start to the end. Stepping out from the tracked beats instead
    # piles up the tracker's small tempo error into a 16th off by the far end. This assumes
    # a steady tempo, true of most game and film cues; a tempo change shows up as drift.
    total = len(y) / sr
    if len(times) > 4:
        period = 60.0 / tempo
        idx = np.round((times - times[0]) / period)
        keep = np.ones(len(times), bool)
        for _ in range(2):
            b_, a_ = np.polyfit(idx[keep], times[keep], 1)
            keep = np.abs(times - (a_ + b_ * idx)) < 0.06
            if keep.sum() < 4:
                keep[:] = True
        b_, a_ = np.polyfit(idx[keep], times[keep], 1)
        # The tracker locks to where onset energy peaks, a little after the attack. Shift the
        # grid so the attacks themselves (onsets backtracked to their start) sit on 16ths:
        # the circular mean of every attack's offset from its nearest 16th.
        attacks = librosa.onset.onset_detect(y=y, sr=sr, backtrack=True, units="time")
        if len(attacks) > 8:
            q = b_ / 4.0
            ph = 2 * np.pi * ((attacks - a_) / q)
            shift = np.angle(np.mean(np.exp(1j * ph))) / (2 * np.pi) * q
            a_ += shift
        first = a_ - b_ * np.floor(a_ / b_)
        times = np.arange(first, total, b_)
        tempo = 60.0 / b_
    # Keep the beat in 75-165 bpm: drop every other beat, or add the ones between.
    while tempo > 165 and len(times) > 4:
        times = times[::2]
        tempo = spacing_tempo(times)
    while tempo < 75 and len(times) > 2:
        mids = (times[:-1] + times[1:]) / 2
        times = np.sort(np.concatenate([times, mids]))
        tempo = spacing_tempo(times)
    # Downbeat: the phase (of 4) where bars begin. Two clues, each normalised: low-end
    # onsets (the kick lands on 1 more than anywhere) and harmonic change (chords change on
    # the bar line far more often than mid-bar). Harmony carries passages without drums.
    low = librosa.onset.onset_strength(y=librosa.effects.preemphasis(y, coef=-0.95), sr=sr)
    lf = librosa.time_to_frames(times, sr=sr)
    lf = lf[lf < len(low)]
    kick = np.array([float(np.mean(low[lf[p::4]])) if len(lf[p::4]) else 0.0 for p in range(4)])
    chroma = librosa.feature.chroma_cqt(y=other if np.sqrt(np.mean(other**2)) > 1e-3 else y, sr=sr, hop_length=512)
    cf = np.clip(librosa.time_to_frames(times, sr=sr, hop_length=512), 0, chroma.shape[1] - 1)
    beat_chroma = librosa.util.sync(chroma, cf, aggregate=np.median)
    beat_chroma = beat_chroma / (np.linalg.norm(beat_chroma, axis=0, keepdims=True) + 1e-9)
    # Change into beat i: distance between the two beats before it and the two from it.
    change = np.zeros(beat_chroma.shape[1])
    for i in range(2, beat_chroma.shape[1] - 1):
        before = beat_chroma[:, i - 2 : i].mean(axis=1)
        after = beat_chroma[:, i : i + 2].mean(axis=1)
        change[i] = 1.0 - float(before @ after) / (np.linalg.norm(before) * np.linalg.norm(after) + 1e-9)
    harm = np.array([float(np.mean(change[p::4])) for p in range(4)])

    def z(v):
        return (v - v.mean()) / (v.std() + 1e-9)

    scores = list(z(harm) + 0.6 * z(kick))
    phase = int(np.argmax(scores))
    return tempo, times, phase, scores


def tempo_segments(drums, other, sr, min_len=12.0, change=0.04):
    """Spans of steady tempo: [(start s, end s, bpm)]. A suite or a film cue changes tempo
    between movements; one grid over all of it puts every later bar line in the wrong place.
    The local tempo (8 s autocorrelation windows) is folded into 75-165 bpm and smoothed; a new
    span starts where it moves more than `change` from the current span's tempo and stays moved."""
    import librosa
    from scipy.ndimage import median_filter

    y = drums * 1.5 + other
    env = librosa.onset.onset_strength(y=y, sr=sr, aggregate=np.median)
    local = librosa.feature.tempo(onset_envelope=env, sr=sr, aggregate=None, ac_size=8.0)
    local = np.array(local, float)
    for i in range(len(local)):
        while local[i] > 165:
            local[i] /= 2
        while local[i] < 75:
            local[i] *= 2
    fps = sr / 512
    local = median_filter(local, size=max(3, int(4 * fps)))
    total = len(y) / sr
    spans = []
    start = 0
    cur = []
    moved = 0
    need = int(5 * fps)
    for i, t in enumerate(local):
        ref = np.median(cur) if cur else t
        if cur and abs(t - ref) / ref > change:
            moved += 1
            if moved >= need and (i - moved - start) / fps >= min_len:
                cut = i - moved
                spans.append((start / fps, cut / fps, float(ref)))
                start, cur, moved = cut, list(local[cut:i + 1]), 0
                continue
        else:
            moved = 0
        cur.append(t)
    spans.append((start / fps, total, float(np.median(cur) if cur else 120.0)))
    # Tidy up: a span at double or half its neighbour's tempo is the same pulse counted
    # differently (fold it), spans within 4% are one tempo, and a short span joins its
    # neighbour. Repeat until nothing changes.
    def same(x, y):
        return abs(x - y) / y < 0.04

    changed = True
    while changed and len(spans) > 1:
        changed = False
        out = [spans[0]]
        for a, b_, bpm in spans[1:]:
            pa, pb_, pbpm = out[-1]
            for k in (2.0, 0.5):
                if same(bpm * k, pbpm):
                    bpm *= k
            if same(bpm, pbpm) or b_ - a < min_len or pb_ - pa < min_len:
                # Weight the merged tempo by duration; keep the longer span's pulse.
                w1, w2 = pb_ - pa, b_ - a
                out[-1] = (pa, b_, pbpm if w1 >= w2 else bpm)
                changed = True
            else:
                out.append((a, b_, bpm))
        spans = out
    return spans


def segmented_beats(drums, other, sr):
    """Beats across tempo changes: each steady span gets its own grid and downbeat.
    Returns (spans with tempo and bar count, beat times, downbeat beat indices)."""
    spans = tempo_segments(drums, other, sr)
    all_times, downbeats, info = [], [], []
    for a, b_, _ in spans:
        i0, i1 = int(a * sr), int(b_ * sr)
        if i1 - i0 < sr * 4:
            continue
        tempo, times, phase, _ = beats(drums[i0:i1], other[i0:i1], sr)
        times = times + a
        times = times[(times >= a - 1e-3) & (times < b_)]
        base = len(all_times)
        all_times.extend(times.tolist())
        idx = list(range(base + phase, base + len(times), 4))
        downbeats.extend(idx)
        info.append({"from": round(a, 2), "to": round(b_, 2), "tempo": round(tempo, 1), "bars": len(idx), "first_bar": len(downbeats) - len(idx)})
    return info, np.array(all_times), downbeats


def grid_segmented(times, downbeats):
    """Bar start times and time -> (bar, 16th) for a grid whose bars start at `downbeats` (beat indices)."""
    bars = np.array([times[i] for i in downbeats])
    db = np.array(downbeats)

    def locate(t):
        i = int(np.searchsorted(times, t, side="right")) - 1
        if i < 0:
            return None
        nxt = times[i + 1] if i + 1 < len(times) else times[i] + (times[i] - times[i - 1] if i > 0 else 0.5)
        frac = (t - times[i]) / max(1e-6, nxt - times[i])
        sixteenth = int(round(frac * 4))
        beat_index = i + sixteenth // 4
        sixteenth %= 4
        bar = int(np.searchsorted(db, beat_index, side="right")) - 1
        if bar < 0:
            return None
        step = (beat_index - db[bar]) * 4 + sixteenth
        if step > 15:
            return None
        return bar, step

    return bars, locate


def numeral_in(root, q, tonic):
    """A chord as a Roman numeral against `tonic` (see `key_and_chords`' numeral)."""
    if root is None:
        return "-"
    base = ["I", "bII", "II", "bIII", "III", "IV", "#IV", "V", "bVI", "VI", "bVII", "VII"][(root - tonic) % 12]
    minorish = q in ("m", "m7", "dim")
    body = base.lower().replace("B", "b") if minorish else base
    return body + {"": "", "m": "", "5": "5", "sus4": "sus4", "sus2": "sus2", "dim": "o", "7": "7", "m7": "7", "maj7": "maj7"}.get(q, "")


# Krumhansl-Kessler key profiles.
KK_MAJOR = np.array([6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88])
KK_MINOR = np.array([6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17])


def find_key(notes, bass=()):
    """(tonic pitch class, 'major'|'minor', score, runner-up text) from note events, weighted by
    duration, the bass three times over (the bass says where home is far more than the
    upper parts, which is where plain Krumhansl confuses a minor key with its relative major)."""
    pc = np.zeros(12)
    for a, b_, p, v in notes:
        pc[p % 12] += max(0.05, b_ - a)
    for a, b_, p, v in bass:
        pc[p % 12] += 3 * max(0.05, b_ - a)
    if pc.sum() <= 0:
        return 0, "minor", 0.0, ""
    scored = []
    for t in range(12):
        for mode, prof in (("major", KK_MAJOR), ("minor", KK_MINOR)):
            r = float(np.corrcoef(pc, np.roll(prof, t))[0, 1])
            scored.append((r, t, mode))
    scored.sort(reverse=True)
    r, t, mode = scored[0]
    alt = ", ".join(f"{NOTE[x[1]]} {x[2]} {x[0]:.2f}" for x in scored[1:3])
    return t, mode, r, alt


def span_key(notes, bass=()):
    """Key of a set of note events as text, or '?' when there are too few."""
    if len(notes) + len(bass) < 12:
        return "?"
    t, mode, r, _ = find_key(notes, bass)
    return f"{NOTE[t]} {mode}"


def grid_of(times, phase):
    """Bar start times (every 4th beat from the downbeat phase) and a function mapping a time to (bar, 16th)."""
    bars = times[phase::4]

    def locate(t):
        i = int(np.searchsorted(times, t, side="right")) - 1
        if i < 0:
            return None
        nxt = times[i + 1] if i + 1 < len(times) else times[i] + (times[i] - times[i - 1] if i > 0 else 0.5)
        frac = (t - times[i]) / max(1e-6, nxt - times[i])
        sixteenth = int(round(frac * 4))
        beat_index = i + sixteenth // 4
        sixteenth %= 4
        bar = (beat_index - phase) // 4
        beat_in_bar = (beat_index - phase) % 4
        return bar, beat_in_bar * 4 + sixteenth

    return bars, locate


# ---------------------------------------------------------------- notes


def transcribe(stem, out_mid, kind):
    """Basic Pitch note events [(start, end, pitch, velocity 0..1)]."""
    import basic_pitch
    from basic_pitch.inference import predict

    # The ONNX copy of the model: the default TFLite one needs a runtime built for NumPy 1.
    model = os.path.join(os.path.dirname(basic_pitch.__file__), "saved_models", "icassp_2022", "nmp.onnx")

    params = dict(onset_threshold=0.5, frame_threshold=0.3, minimum_note_length=80)
    if kind == "bass":
        params.update(minimum_frequency=27.5, maximum_frequency=400.0, onset_threshold=0.45)
    else:
        params.update(minimum_frequency=60.0, maximum_frequency=3000.0)
    _, midi, events = predict(stem, model, **params)
    midi.write(out_mid)
    return [(float(s), float(e), int(p), float(a)) for s, e, p, a, *_ in events]


def monophonic(events):
    """The lowest sounding note at each onset: a bass line from a transcription with overtones."""
    events = sorted(events, key=lambda e: (e[0], e[2]))
    out = []
    for e in events:
        if out and abs(e[0] - out[-1][0]) < 0.05:
            if e[2] < out[-1][2]:
                out[-1] = e
            continue
        out.append(e)
    return out


DRUM_PARAMS = {
    # Onset picking on the summed band flux, as a fraction of its 99th percentile.
    "onset_delta": 0.06,
    # Cymbals are picked on their own band.
    "cym_delta": 0.04,
    # Class rules on each onset's rise (log energy gained over ~35 ms) per band. Measured on
    # known kits: a kick rises in the low band (its sweep starts high) with almost no noise;
    # a tom or taiko rises there too but with a click of noise; a snare is mostly noise.
    "body_min": 1.5,
    "kick_noise_max": 0.8,
    "snare_noise": 2.0,
    "snare_mid": 1.0,
    "tom_noise_max": 2.0,
}


def drum_hits(drums, sr, params=None):
    """Drum hits as [(time, class, strength)], class kick, snare, tom (toms, taiko, big
    booms) or cym (hats, cymbals, shakers). Onsets are found once on the whole kit, then
    each is classified by how much each band rose at it: a kick is mostly sub, a tom a low
    body with little noise, a snare a body with a burst of noise. Cymbals are picked on
    their own band so a hat under a kick is still heard. Several classes may share an
    onset (a kick and a taiko together)."""
    import librosa

    P = dict(DRUM_PARAMS, **(params or {}))
    hop = 256
    S = np.abs(librosa.stft(drums, n_fft=2048, hop_length=hop))
    freqs = librosa.fft_frequencies(sr=sr, n_fft=2048)
    # Per-bin log compression, then band sums: soft hits count, one loud crash does not own the scale.
    L = np.log1p(100.0 * S)
    bands = {"sub": (30, 90), "low": (90, 260), "mid": (260, 1200), "noise": (1800, 6000), "air": (7000, 11000)}
    E = {}
    for name, (lo, hi) in bands.items():
        idx = (freqs >= lo) & (freqs < hi)
        E[name] = L[idx].mean(axis=0)
    flux = {k: np.maximum(0.0, np.diff(v, prepend=v[0])) for k, v in E.items()}

    def norm(f):
        return f / (np.percentile(f, 99) + 1e-9)

    total = norm(flux["sub"]) + norm(flux["low"]) + norm(flux["noise"])
    onsets = librosa.util.peak_pick(total, pre_max=2, post_max=2, pre_avg=10, post_avg=4, delta=P["onset_delta"] * 3, wait=3)

    def rise(name, p):
        e = E[name]
        before = e[max(0, p - 4) : max(1, p - 1)].min() if p > 1 else e[0]
        after = e[p : p + 4].max()
        return float(after - before)

    hits = []
    for p in onsets:
        t = float(librosa.frames_to_time(p, sr=sr, hop_length=hop))
        low, mid, noise = rise("low", p), rise("mid", p), rise("noise", p)
        strength = float(np.clip(total[p] / 3.0, 0.0, 1.0))
        if noise > P["snare_noise"] and mid > P["snare_mid"]:
            hits.append((t, "snare", strength))
        elif low > P["body_min"] and noise < P["kick_noise_max"]:
            hits.append((t, "kick", strength))
        elif low > P["body_min"] and noise < P["tom_noise_max"]:
            hits.append((t, "tom", strength))
    cym = norm(flux["air"])
    for p in librosa.util.peak_pick(cym, pre_max=2, post_max=2, pre_avg=8, post_avg=4, delta=P["cym_delta"] * 3, wait=2):
        hits.append((float(librosa.frames_to_time(p, sr=sr, hop_length=hop)), "cym", float(np.clip(cym[p] / 3.0, 0, 1))))
    return hits


# ---------------------------------------------------------------- harmony


def key_and_chords(notes_other, notes_bass, bar_times, end_time):
    """Key (music21, from every note weighted by length) and one chord per bar (or half bar
    when it changes mid-bar), as names and Roman numerals."""
    from music21 import chord as m21chord
    from music21 import key as m21key
    from music21 import roman, stream, note as m21note

    s = stream.Stream()
    for st, en, p, a in notes_other + notes_bass:
        n = m21note.Note(p)
        n.quarterLength = max(0.25, (en - st) * 2)
        s.append(n)
    tonic, mode, conf, alt_text = find_key(notes_other, notes_bass)
    k = m21key.Key(NOTE[tonic].replace("b", "-"), mode)
    alt = [alt_text]

    edges = list(bar_times) + [end_time]
    halves = []
    for i in range(len(edges) - 1):
        a, b = edges[i], edges[i + 1]
        m = (a + b) / 2
        halves += [(i, 0, a, m), (i, 1, m, b)]

    def profile(a, b):
        pc = np.zeros(12)
        bass_pc = np.zeros(12)
        for st, en, p, v in notes_other:
            o = max(0.0, min(b, en) - max(a, st))
            pc[p % 12] += o * (0.3 + v)
        for st, en, p, v in notes_bass:
            o = max(0.0, min(b, en) - max(a, st))
            pc[p % 12] += o * (0.3 + v) * 1.5
            bass_pc[p % 12] += o
        return pc, bass_pc

    templates = []
    for root in range(12):
        for q, ivs in (("", (0, 4, 7)), ("m", (0, 3, 7)), ("5", (0, 7)), ("sus4", (0, 5, 7)), ("sus2", (0, 2, 7)), ("dim", (0, 3, 6)), ("7", (0, 4, 7, 10)), ("m7", (0, 3, 7, 10)), ("maj7", (0, 4, 7, 11))):
            v = np.zeros(12)
            for iv in ivs:
                v[(root + iv) % 12] = 1
            # Simpler chords win ties.
            templates.append((root, q, v / np.linalg.norm(v), 1.0 - 0.04 * (len(ivs) - 3) - (0.03 if q in ("sus4", "sus2", "5") else 0)))

    def best(pc, bass_pc):
        if pc.sum() <= 1e-6:
            return None, 0.0
        x = pc / np.linalg.norm(pc)
        scored = []
        for root, q, v, w in templates:
            sc = float(x @ v) * w
            if bass_pc.sum() > 0 and np.argmax(bass_pc) == root:
                sc += 0.08
            scored.append((sc, root, q))
        scored.sort(reverse=True)
        sc, root, q = scored[0]
        return (root, q), sc

    per_bar = []
    for i in range(len(edges) - 1):
        (_, _, a, m), (_, _, _, b) = halves[2 * i], halves[2 * i + 1]
        whole, ws = best(*profile(a, b))
        first, fs = best(*profile(a, m))
        second, ss = best(*profile(m, b))
        if first and second and first != second and fs > ws + 0.05 and ss > ws + 0.05:
            per_bar.append([first, second])
        else:
            per_bar.append([whole])

    def name(c):
        if c is None:
            return "N.C."
        root, q = c
        return NOTE[root] + q

    def numeral(c):
        """Degree against the tonic with flats relative to the major scale (the film and
        pop convention: in C minor, Cm Ab Eb Bb is i bVI bIII bVII), upper case for major,
        lower for minor, with the chord's colour after it."""
        if c is None:
            return "-"
        root, q = c
        base = ["I", "bII", "II", "bIII", "III", "IV", "#IV", "V", "bVI", "VI", "bVII", "VII"][(root - k.tonic.pitchClass) % 12]
        minorish = q in ("m", "m7", "dim")
        body = base.lower() if minorish else base
        body = body.replace("B", "b") if minorish else body
        suffix = {"": "", "m": "", "5": "5", "sus4": "sus4", "sus2": "sus2", "dim": "o", "7": "7", "m7": "7", "maj7": "maj7"}[q]
        return body + suffix

    chords = [[{"name": name(c), "numeral": numeral(c), "root": (c[0] if c else None), "q": (c[1] if c else None)} for c in bar] for bar in per_bar]
    return {"key": f"{NOTE[tonic]} {mode}", "tonic": tonic, "mode": mode, "confidence": conf, "alternatives": alt}, chords


# ---------------------------------------------------------------- form


def sections(y, sr, bar_times, n_bars):
    """Section boundaries on bar lines, from chroma + timbre self-similarity; labels by likeness."""
    import librosa

    if n_bars < 4:
        return [(0, n_bars, "A")]
    hop = 512
    chroma = librosa.feature.chroma_cqt(y=y, sr=sr, hop_length=hop)
    mfcc = librosa.feature.mfcc(y=y, sr=sr, n_mfcc=13, hop_length=hop)
    rms = librosa.feature.rms(y=y, hop_length=hop)
    frames = librosa.time_to_frames(bar_times, sr=sr, hop_length=hop)
    frames = np.clip(frames, 0, chroma.shape[1] - 1)
    feats = []
    for i in range(n_bars):
        a = frames[i]
        b = frames[i + 1] if i + 1 < len(frames) else chroma.shape[1]
        b = max(b, a + 1)
        v = np.concatenate([chroma[:, a:b].mean(axis=1), mfcc[1:, a:b].mean(axis=1) / 40.0, [rms[0, a:b].mean() * 10]])
        feats.append(v)
    F = np.array(feats)
    F = (F - F.mean(axis=0)) / (F.std(axis=0) + 1e-6)
    k = int(np.clip(round(n_bars / 8), 2, 12))
    bounds = librosa.segment.agglomerative(F.T, k)
    bounds = sorted(set([0] + [int(b) for b in bounds] + [n_bars]))
    # Snap to 2-bar lines: music moves in phrases.
    bounds = sorted(set([0, n_bars] + [int(2 * round(b / 2)) for b in bounds[1:-1]]))
    segs = [(a, b) for a, b in zip(bounds, bounds[1:]) if b > a]
    means = [F[a:b].mean(axis=0) for a, b in segs]
    labels = []
    protos = []
    for m in means:
        for j, p in enumerate(protos):
            if np.linalg.norm(m - p) / math.sqrt(len(m)) < 0.55:
                labels.append(chr(65 + j))
                break
        else:
            protos.append(m)
            labels.append(chr(65 + len(protos) - 1))
    return [(a, b, l) for (a, b), l in zip(segs, labels)]


# ---------------------------------------------------------------- the song


KIT = {"kick": 36, "snare": 38, "tom": 35, "cym": 42}


def build_song(name, tempo, key, bar_times, locate, bass, other, hits, secs, n_bars):
    """The transcription as an mc-music song (JSON form; `mc-music ron` writes the RON)."""
    root = key["tonic"]
    scale = "Minor" if key["mode"] == "minor" else "Major"
    bar_ticks = 4 * PPQ

    def notes_in(events, a_bar, b_bar, mono=False):
        out = []
        for st, en, p, v in events:
            loc = locate(st)
            if loc is None:
                continue
            bar, six = loc
            if not (a_bar <= bar < b_bar):
                continue
            end = locate(en)
            if end is None:
                continue
            ticks = (bar - a_bar) * bar_ticks + six * (PPQ // 4)
            end_ticks = (end[0] - a_bar) * bar_ticks + end[1] * (PPQ // 4)
            length = max(PPQ // 4, end_ticks - ticks)
            out.append([ticks, length, p, int(np.clip(40 + v * 90, 1, 127))])
        return out

    def drums_in(a_bar, b_bar):
        seen = {}
        for t, band, s in hits:
            loc = locate(t)
            if loc is None or not (a_bar <= loc[0] < b_bar):
                continue
            ticks = (loc[0] - a_bar) * bar_ticks + loc[1] * (PPQ // 4)
            k = (ticks, KIT[band])
            seen[k] = max(seen.get(k, 0), s)
        return [[t, PPQ // 4, key_, int(np.clip(50 + s * 77, 1, 127))] for (t, key_), s in sorted(seen.items())]

    patterns, sections_out = [], []
    for i, (a, b, label) in enumerate(secs):
        sname = f"{label.lower()}{i + 1}"
        beats_n = (b - a) * 4
        clips = []
        for track, events in (("drums", None), ("bass", bass), ("other", other)):
            notes = drums_in(a, b) if track == "drums" else notes_in(events, a, b)
            if not notes:
                continue
            pname = f"{sname}_{track}"
            patterns.append({"name": pname, "beats": beats_n, "notes": notes})
            clips.append({"track": track, "pattern": pname, "at": 0, "times": 1})
        sections_out.append({"name": sname, "bars": b - a, "kind": "Loop", "clips": clips})

    lib = os.path.join(ROOT, "data", "music", "instruments")

    def inst(n):
        # Presets are RON; the song JSON wants the instrument as JSON. Convert through mc-music.
        return load_instrument_json(os.path.join(lib, f"{n}.ron"))

    tracks = [
        {"name": "drums", "instrument": inst("war_kit"), "db": -8.0, "pan": 0.0, "colour": 0xE0603A},
        {"name": "bass", "instrument": inst("pulse_bass"), "db": -6.0, "pan": 0.0, "colour": 0xC0452A},
        {"name": "other", "instrument": inst("wide_strings"), "db": -8.0, "pan": 0.0, "colour": 0x8FA7C4},
    ]
    return {
        "name": f"Reference: {name}",
        "tempo": round(tempo, 2),
        "beats_per_bar": 4,
        "root": root,
        "scale": scale,
        "tracks": tracks,
        "buses": [],
        "master": {"db": 0.0, "effects": [{"Limiter": {"ceiling": -1.0, "gain": 0.0, "release": 80.0, "on": True}}]},
        "patterns": patterns,
        "sections": sections_out,
        "arrangement": [s["name"] for s in sections_out],
        "notes": "Transcribed by scripts/listen/pipeline.py: a sketch of the reference, not the reference. Drums are band onsets (kick/snare/cymbals), bass and other are Basic Pitch notes quantised to 16ths.",
    }


def mc_music():
    """The mc-music CLI, built into the scratch target if needed."""
    exe = os.environ.get("MC_MUSIC")
    if exe and os.path.exists(exe):
        return exe
    target = os.path.join(ROOT, "target")
    subprocess.check_call(["cargo", "build", "--release", "-q", "-p", "mc-music"], cwd=ROOT)
    return os.path.join(os.environ.get("CARGO_TARGET_DIR", target), "release", "mc-music")


def load_instrument_json(path):
    """An instrument preset as JSON, by wrapping it in a throwaway song and converting."""
    import tempfile

    text = open(path).read()
    song = '(name: "x", tempo: 120, tracks: [(name: "t", instrument: ' + text + ")])"
    with tempfile.TemporaryDirectory() as d:
        p = os.path.join(d, "s.ron")
        open(p, "w").write(song)
        out = subprocess.check_output([mc_music(), "json", p])
    return json.loads(out)["tracks"][0]["instrument"]


# ---------------------------------------------------------------- report


def degree(p, tonic):
    names = ["1", "b2", "2", "b3", "3", "4", "#4", "5", "b6", "6", "b7", "7"]
    return names[(p - tonic) % 12]


def report(meta, tempo, key, chords, secs, bass_mono, hits, locate, n_bars, stems_info, span, spans=()):
    L = []
    L.append(f"# {meta.get('title') or meta.get('name')}")
    if meta.get("url"):
        L.append(f"source: {meta['url']}")
    if span[0] is not None or span[1] is not None:
        L.append(f"span: {span[0] or 0:.1f}s to {span[1] if span[1] is not None else 'end'}s")
    L.append(f"tempo {tempo:.1f} bpm, key {key['key']} (confidence {key['confidence']:.2f}; also {', '.join(key['alternatives'])}), {n_bars} bars of 4/4 assumed")
    if len(spans) > 1:
        L.append("")
        L.append("## Tempo (the piece changes tempo; each span has its own bars)")
        for x in spans:
            m0, s0 = divmod(x["from"], 60)
            m1, s1 = divmod(x["to"], 60)
            L.append(f"- {int(m0)}:{s0:04.1f}-{int(m1)}:{s1:04.1f}  {x['tempo']:.0f} bpm, {x.get('key', '?')}, bars {x['first_bar'] + 1}-{x['first_bar'] + x['bars']}")
    if len(spans) > 1:
        L.append("")
        L.append("## Chart per tempo span (numerals in that span's own key; 8 bars a line)")
        by_bar_bass = defaultdict(lambda: ["."] * 16)
        for st, en, p, v in bass_mono:
            loc = locate(st)
            if loc and 0 <= loc[0] < n_bars:
                by_bar_bass[loc[0]][loc[1]] = p
        for x in spans:
            if x.get("tonic") is None or x["bars"] < 2:
                continue
            m0, s0 = divmod(x["from"], 60)
            L.append(f"### {int(m0)}:{s0:04.1f}, {x['tempo']:.0f} bpm, {x['key']}, {x['bars']} bars")
            b0 = x["first_bar"]
            rows = []
            for i in range(b0, min(b0 + x["bars"], len(chords))):
                rows.append(" ".join(numeral_in(c["root"], c["q"], x["tonic"]) for c in chords[i]))
            for i in range(0, len(rows), 8):
                names = " | ".join(" ".join(c["name"] for c in chords[j]) for j in range(b0 + i, min(b0 + i + 8, b0 + len(rows))))
                L.append(f"- bars {b0 + i + 1:>3}: {' | '.join(rows[i:i + 8])}    [{names}]")
            # The most common bass bar, in degrees of this key.
            common = Counter(
                " ".join(degree(p, x["tonic"]) if p != "." else "." for p in by_bar_bass[i]) for i in range(b0, b0 + x["bars"]) if i in by_bar_bass
            )
            for row, cnt in common.most_common(2):
                L.append(f"  bass x{cnt}: {row}")
    L.append("")
    L.append("## Form")
    for a, b, l in secs:
        L.append(f"- {l}: bars {a + 1}-{b} ({b - a} bars)")
    L.append("")
    L.append("## Chords (Roman numerals in the key; names in brackets)")
    for a, b, l in secs:
        row = []
        for i in range(a, min(b, len(chords))):
            row.append(" ".join(c["numeral"] for c in chords[i]) + " [" + " ".join(c["name"] for c in chords[i]) + "]")
        L.append(f"- {l} (bars {a + 1}-{b}): " + " | ".join(row))
    L.append("")
    L.append("## Bass (scale degrees on a 16th grid per bar, most common bars per section)")
    by_bar = defaultdict(lambda: ["."] * 16)
    for st, en, p, v in bass_mono:
        loc = locate(st)
        if loc and 0 <= loc[0] < n_bars:
            by_bar[loc[0]][loc[1]] = degree(p, key["tonic"])
    for a, b, l in secs:
        rows = Counter(" ".join(by_bar[i]) for i in range(a, b) if i in by_bar)
        for row, n in rows.most_common(2):
            L.append(f"- {l} x{n}: {row}")
    L.append("")
    L.append("## Drums (16th grid per bar; X strong, x normal, o soft; most common bars per section)")
    grids = defaultdict(lambda: {"kick": ["."] * 16, "snare": ["."] * 16, "tom": ["."] * 16, "cym": ["."] * 16})
    for t, band, s in hits:
        loc = locate(t)
        if loc and 0 <= loc[0] < n_bars:
            grids[loc[0]][band][loc[1]] = "X" if s > 0.6 else ("x" if s > 0.3 else "o")
    for a, b, l in secs:
        rows = Counter(tuple("".join(grids[i][band]) for band in ("kick", "snare", "tom", "cym")) for i in range(a, b) if i in grids)
        for (k, s, tm, c), n in rows.most_common(2):
            L.append(f"- {l} x{n}:  kick {k}  snare {s}  toms {tm}  cymbals {c}")
    L.append("")
    L.append("## Stems (how loud each part is, and where its energy sits)")
    for name, info in stems_info.items():
        L.append(f"- {name}: {info}")
    return "\n".join(L) + "\n"


def stem_info(y, sr):
    import librosa

    rms = float(np.sqrt(np.mean(y**2)) + 1e-12)
    S = np.abs(librosa.stft(y, n_fft=4096))
    f = librosa.fft_frequencies(sr=sr, n_fft=4096)
    p = (S**2).mean(axis=1)
    tot = p.sum() + 1e-12
    bands = [(0, 60), (60, 250), (250, 2000), (2000, 6000), (6000, 20000)]
    share = [100 * p[(f >= a) & (f < b)].sum() / tot for a, b in bands]
    centroid = float((f * p).sum() / tot)
    onsets = librosa.onset.onset_detect(y=y, sr=sr)
    dur = len(y) / sr
    return f"rms {20 * math.log10(rms):.1f} dBFS, bands sub {share[0]:.0f}% bass {share[1]:.0f}% lowmid {share[2]:.0f}% highmid {share[3]:.0f}% air {share[4]:.0f}%, centroid {centroid:.0f} Hz, {len(onsets) / max(dur, 1e-3):.1f} onsets/s"


# ---------------------------------------------------------------- scores


def from_score(path, out_dir, name):
    """MIDI or MusicXML: exact notes. Chords by chordify, numerals in the analysed key."""
    from music21 import converter, roman

    s = converter.parse(path)
    k = s.analyze("key")
    mm = s.metronomeMarkBoundaries()
    tempo = float(mm[0][2].number) if mm else 120.0
    ts = s.recurse().getElementsByClass("TimeSignature")
    beats_per_bar = int(ts[0].numerator) if ts else 4
    ch = s.chordify()
    L = [f"# {name} (score)", f"tempo {tempo:.0f}, key {k.tonic.name} {k.mode} (confidence {k.correlationCoefficient:.2f}), {beats_per_bar}/4", "", "## Chords per bar"]
    for m in ch.recurse().getElementsByClass("Measure"):
        cs = [c for c in m.recurse().getElementsByClass("Chord")]
        if not cs:
            continue
        # The longest-lasting chord shapes of the bar.
        by = defaultdict(float)
        for c in cs:
            try:
                by[roman.romanNumeralFromChord(c.closedPosition(), k).figure + " [" + c.pitchedCommonName + "]"] += c.quarterLength
            except Exception:
                pass
        top = sorted(by.items(), key=lambda x: -x[1])[:2]
        L.append(f"- bar {m.number}: " + " / ".join(t for t, _ in top))
    L.append("")
    L.append("## Parts")
    parts = []
    for part in s.parts:
        notes = [n for n in part.recurse().notes]
        pitches = [p.midi for n in notes for p in (n.pitches if hasattr(n, "pitches") else [n.pitch])]
        if not pitches:
            continue
        L.append(f"- {part.partName or 'part'}: {len(notes)} notes, range {min(pitches)}-{max(pitches)}")
        parts.append(part)
    open(os.path.join(out_dir, "report.md"), "w").write("\n".join(L) + "\n")

    # As a song: one track per part (up to 8), notes in ticks, one section per 8 bars.
    bar_q = beats_per_bar
    total_q = float(s.highestTime)
    n_sections = max(1, math.ceil(total_q / (8 * bar_q)))
    lib = os.path.join(ROOT, "data", "music", "instruments")
    tracks, patterns, sections_out = [], [], []
    for pi, part in enumerate(parts[:8]):
        pname = slug(part.partName or f"part{pi + 1}")
        drum = "perc" in pname or "drum" in pname or "cymbal" in pname or "timpani" in pname
        tracks.append({"name": pname, "instrument": load_instrument_json(os.path.join(lib, "war_kit.ron" if drum else ("sub_bass.ron" if "bass" in pname or "tuba" in pname else "wide_strings.ron"))), "db": -10.0, "pan": 0.0, "colour": 0x8FA7C4})
    for si in range(n_sections):
        a_q, b_q = si * 8 * bar_q, (si + 1) * 8 * bar_q
        clips = []
        for pi, part in enumerate(parts[:8]):
            notes = []
            for n in part.flatten().notes:
                off = float(n.offset)
                if not (a_q <= off < b_q):
                    continue
                for p in n.pitches if hasattr(n, "pitches") else [n.pitch]:
                    vel = n.volume.velocity or 90
                    notes.append([int(round((off - a_q) * PPQ)), max(1, int(round(float(n.quarterLength) * PPQ))), int(p.midi), int(vel)])
            if notes:
                pn = f"s{si + 1}_{tracks[pi]['name']}"
                patterns.append({"name": pn, "beats": 8 * bar_q, "notes": notes})
                clips.append({"track": tracks[pi]["name"], "pattern": pn, "at": 0, "times": 1})
        sections_out.append({"name": f"s{si + 1}", "bars": 8, "kind": "Loop", "clips": clips})
    song = {
        "name": f"Score: {name}", "tempo": tempo, "beats_per_bar": beats_per_bar, "root": k.tonic.pitchClass,
        "scale": "Minor" if k.mode == "minor" else "Major", "tracks": tracks, "buses": [],
        "master": {"db": 0.0, "effects": []}, "patterns": patterns, "sections": sections_out,
        "arrangement": [x["name"] for x in sections_out], "notes": f"Imported from {os.path.basename(path)}",
    }
    return song


# ---------------------------------------------------------------- main


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("source")
    ap.add_argument("--from", dest="start")
    ap.add_argument("--to", dest="end")
    ap.add_argument("--name")
    args = ap.parse_args()
    src = args.source
    start, end = seconds(args.start), seconds(args.end)
    is_url = re.match(r"https?://", src) is not None
    is_score = not is_url and os.path.splitext(src)[1].lower() in (".mid", ".midi", ".xml", ".musicxml", ".mxl")
    name = args.name or (youtube_id(src) if is_url else slug(os.path.splitext(os.path.basename(src))[0]))
    out = os.path.join(REFS, name)
    os.makedirs(out, exist_ok=True)

    if is_score:
        song = from_score(src, out, name)
    else:
        if is_url:
            wav, title = fetch(src, out)
            meta = {"title": title, "url": src, "name": name}
        else:
            wav = to_wav(src, out)
            meta = {"title": os.path.basename(src), "name": name}
        stems = separate(wav, out)
        sr = 22050
        drums = load_mono(stems["drums"], sr, start, end)
        other = load_mono(stems["other"], sr, start, end)
        mix = load_mono(wav, sr, start, end)
        spans, times, downbeats = segmented_beats(drums, other, sr)
        spans_keyed = spans  # keys are added once notes exist
        bar_times, locate = grid_segmented(times, downbeats)
        n_bars = max(1, len(bar_times))
        tempo = max(spans, key=lambda x: x["to"] - x["from"])["tempo"] if spans else 120.0
        phase, phase_scores = 0, []
        log(f"{len(spans)} tempo span(s): " + ", ".join(f"{x['from']:.0f}-{x['to']:.0f}s {x['tempo']:.0f} bpm" for x in spans) + f"; {n_bars} bars")

        # Transcribe the stems (the span only, cut to its own file so times line up).
        import soundfile as sf

        os.makedirs(os.path.join(out, "notes"), exist_ok=True)
        tag = "" if start is None and end is None else f"_{int(start or 0)}_{int(end or 0)}"
        events = {}
        for part in ("bass", "other"):
            y = load_mono(stems[part], sr, start, end)
            clip = os.path.join(out, "notes", f"{part}{tag}.wav")
            sf.write(clip, y, sr)
            log(f"transcribing {part}")
            events[part] = transcribe(clip, os.path.join(out, "notes", f"{part}{tag}.mid"), part)
        bass_mono = monophonic(events["bass"])
        for x in spans:
            upper = [e for e in events["other"] if x["from"] <= e[0] < x["to"]]
            low = [e for e in bass_mono if x["from"] <= e[0] < x["to"]]
            x["key"] = span_key(upper, low)
            x["tonic"] = find_key(upper, low)[0] if x["key"] != "?" else None
        hits = drum_hits(drums, sr)
        key, chords = key_and_chords(events["other"], bass_mono, bar_times, len(mix) / sr)
        secs = sections(mix, sr, bar_times, n_bars)
        info = {n: stem_info(load_mono(p, sr, start, end), sr) for n, p in stems.items()}
        info["mix"] = stem_info(mix, sr)
        text = report(meta, tempo, key, chords, secs, bass_mono, hits, locate, n_bars, info, (start, end), spans)
        open(os.path.join(out, "report.md"), "w").write(text)
        json.dump(
            {"meta": meta, "tempo": tempo, "spans": spans, "downbeats": [int(i) for i in downbeats], "beats": [float(t) for t in times], "downbeat_phase": phase, "phase_scores": phase_scores, "key": key, "chords": chords, "sections": secs, "bass": bass_mono, "other": events["other"], "drums": hits},
            open(os.path.join(out, "analysis.json"), "w"),
        )
        song = build_song(meta["title"] or name, tempo, key, bar_times, locate, bass_mono, events["other"], hits, secs, n_bars)

    js = os.path.join(out, "reference.json")
    # numpy integers can reach the song (a tempo span's bar count): write them as ints.
    json.dump(song, open(js, "w"), default=int)
    subprocess.check_call([mc_music(), "ron", js, os.path.join(out, "reference.ron")])
    print(open(os.path.join(out, "report.md")).read())
    print(f"files in {out}")


if __name__ == "__main__":
    main()
