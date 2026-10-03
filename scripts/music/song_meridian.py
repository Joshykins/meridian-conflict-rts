"""Meridian: the main theme, cut like a trailer. D minor, 80 bpm, about two minutes.

Reach Command is the war's texture; this is its tune. One melody carries the
whole piece, from a lone piano to the full orchestra and choir:

    D-E-F-A, up to the D above, and a slow fall back through the minor.

The harmony is minor and heavy but moves: i - bVI - iv - V, with the major
dominant (A, its C# the leading note) pulling back to D minor, and the flat
second (Eb, the Neapolitan) as the dark turn Reach Command leans on too. The
climax lifts through bVI - bVII - i, the one heroic gesture, and the last
statement climbs a whole step to E minor. It ends on a bare fifth, not a
triad: the war is not won.

    open      4 bars  the D pedal, a tam-tam, the motif alone on the piano, choir "oo"
    rise      8 bars  cello spiccato in 3+3+2, the theme low on the horns, timpani building
    theme     8 bars  the theme on horns and trumpets, trombones under, drums, choir "ah"
    break     2 bars  one tutti blow, then only a timpani roll and the choir swelling
    climax    8 bars  the answer high on violins and trumpets: bVI - bVII - i
    lift      8 bars  the theme again, a step higher, everything
    close     4 bars  one last blow, then the piano motif over a bare fifth

The 3+3+2 accent (beats 1, 2.5, 4) is Reach Command's pulse, kept here so the
two belong together.
"""

from compose import Pattern, clip, grid, keys, section, song, track
from common import COL_BASS, COL_BELL, COL_BRASS, COL_DRUMS, COL_PAD, COL_STRINGS, eq, hall, master
from ron import Newtype

BAR = 4

GRAN_CASSA = 34
TAIKO = 35
WAR_DRUM = 41
CYMBAL = 49
SWELL = 51
TAM_TAM = 52


def use(name):
    return Newtype("Use", name)


# Chords: (bass root, voicing). Voicings sit in the choir's and violas' range.
CH = {
    "Dm": ("D2", "A3 D4 F4 A4"),
    "Bb": ("Bb1", "Bb3 D4 F4 Bb4"),
    "Gm": ("G1", "G3 D4 G4 Bb4"),
    "A": ("A1", "A3 C#4 E4 A4"),
    "Eb": ("Eb2", "G3 Eb4 G4 Bb4"),
    "C": ("C2", "G3 C4 E4 G4"),
}

# The theme's harmony and the climax's, a bar each.
PROG_A = ["Dm", "Bb", "Gm", "A", "Dm", "Eb", "Gm", "A"]
PROG_B = ["Bb", "C", "Dm", "Dm", "Gm", "Eb", "A", "Dm"]

# The theme, in D minor, 8 bars. Rises D-E-F-A, sighs down, the leading note in bar 4,
# the flat second (Eb) in bar 6, and it waits on the dominant.
THEME = [
    ("D4", 1.5), ("E4", 0.5), ("F4", 1), ("A4", 1),
    ("D5", 2.5), ("C5", 0.5), ("Bb4", 0.5), ("A4", 0.5),
    ("Bb4", 1.5), ("A4", 0.5), ("G4", 1), ("D4", 1),
    ("E4", 2), ("C#4", 1), ("E4", 1),
    ("F4", 1.5), ("E4", 0.5), ("D4", 1), ("A4", 1),
    ("G4", 1.5), ("F4", 0.5), ("Eb4", 1), ("Bb4", 1),
    ("D5", 2), ("C5", 1), ("Bb4", 1),
    ("A4", 4),
]  # fmt: skip

# The answer, an octave up: climbs F-G-A over bVI-bVII-i, peaks on Bb, falls home to D.
ANSWER = [
    ("F5", 2), ("D5", 1), ("F5", 1),
    ("G5", 2), ("E5", 1), ("C5", 1),
    ("A5", 2), ("G5", 1), ("F5", 1),
    ("E5", 1.5), ("F5", 0.5), ("D5", 2),
    ("G5", 1.5), ("A5", 0.5), ("Bb5", 2),
    ("G5", 1.5), ("F5", 0.5), ("Eb5", 2),
    ("E5", 2), ("A5", 2),
    ("D5", 4),
]  # fmt: skip


def melody(name, notes, vel=100, octave=0, gap=0.97, swell=None):
    p = Pattern(name, 8 * BAR)
    t = 0.0
    for i, (k, d) in enumerate(notes):
        v = vel if swell is None else swell[0] + (swell[1] - swell[0]) * i / (len(notes) - 1)
        p.add(t, d * gap, keys(k)[0] + 12 * octave, v)
        t += d
    return p


def held(name, prog, vel, octave=0, top=None, bars=8):
    """Each chord held for its bar; `top` keeps only the highest n notes."""
    p = Pattern(name, bars * BAR)
    for i, c in enumerate(prog):
        ks = [k + 12 * octave for k in keys(CH[c][1])]
        if top:
            ks = ks[-top:]
        p.chord(i * BAR, BAR - 0.08, ks, vel)
    return p


def roots(name, prog, vel, octave=0, bars=8):
    p = Pattern(name, bars * BAR)
    for i, c in enumerate(prog):
        r = keys(CH[c][0])[0] + 12 * octave
        p.chord(i * BAR, BAR - 0.08, [r, r + 12], vel)
    return p


def ostinato(name, prog, vel=(104, 70), sixteenths=False, bars=8):
    """Cello spiccato on each bar's root: eighths accented 3+3+2, the last pair up an octave."""
    p = Pattern(name, bars * BAR)
    for i, c in enumerate(prog):
        root = keys(CH[c][0])[0] + 12
        for e in range(8):
            t = i * BAR + e * 0.5
            k = root + 12 if e == 6 else root
            v = vel[0] if e in (0, 3, 6) else vel[1]
            if sixteenths and e not in (0, 3, 6):
                p.add(t, 0.22, k, vel[1] - 6).add(t + 0.25, 0.22, k, vel[1])
            else:
                p.add(t, 0.4, k, v)
    return p


def accents(name, prog, vel, octave=0, voicing=False, length=0.45, bars=8):
    """Short hits on the 3+3+2 accents: the root (timpani, piano) or the chord (brass)."""
    p = Pattern(name, bars * BAR)
    for i, c in enumerate(prog):
        bass, chord = CH[c]
        ks = keys(chord) if voicing else [keys(bass)[0], keys(bass)[0] + 12]
        ks = [k + 12 * octave for k in ks]
        for at, dv in ((0, 0), (1.5, -16), (3, -10)):
            p.chord(i * BAR + at, length, ks, vel + dv)
    return p


def timp(name, prog, vel=104, run=True, bars=8):
    """Timpani on the accents, and a run of sixteenths into the next phrase."""
    p = Pattern(name, bars * BAR)
    for i, c in enumerate(prog):
        r = keys(CH[c][0])[0] + 24
        while r > 50:  # the drums' range, D2..A2-ish
            r -= 12
        for at, dv in ((0, 0), (1.5, -18), (3, -10)):
            p.add(i * BAR + at, 0.5, r, vel + dv)
    if run:
        r = keys(CH[prog[-1]][0])[0] + 24
        while r > 50:
            r -= 12
        for j in range(8):
            p.add(bars * BAR - 2 + j * 0.25, 0.25, r, 70 + j * 6)
    return p


def roll(name, beats, key, lo, hi, bars):
    p = Pattern(name, bars * BAR)
    n = int(beats * 4)
    start = bars * BAR - beats
    for i in range(n):
        p.add(start + i * 0.25, 0.25, key, lo + (hi - lo) * i / max(1, n - 1))
    return p


# --- open: the D pedal, the motif alone ------------------------------------------

open_pedal = Pattern("open_pedal", 4 * BAR).add(0, 16 - 0.1, "D2", 74)
open_cellos = Pattern("open_cellos", 4 * BAR).add(0, 16 - 0.1, "D3", 40).add(8, 8 - 0.1, "A3", 34)
open_hit = Pattern("open_hit", 4 * BAR).add(0, 8, TAM_TAM, 92).add(0, 2, GRAN_CASSA, 110)
open_choir = Pattern("open_choir", 4 * BAR).chord(4, 12 - 0.1, "D3 A3 D4", 54)
# The motif, high and slow, like the lament: D-E-F-A, and the D above held.
open_piano = Pattern("open_piano", 4 * BAR)
open_piano.seq(2, [("D5", 1.5), ("E5", 0.5), ("F5", 1), ("A5", 1), ("D6", 4)], vel=60)
open_piano.seq(10, [("C6", 1), ("Bb5", 1), ("A5", 4)], vel=52)
open_piano.chord(0, 15.5, "D2 D3", 46)

# --- rise: the ostinato starts, the theme low and quiet ---------------------------

rise_bass = roots("rise_bass", PROG_A, 72)
rise_cellos = ostinato("rise_cellos", PROG_A, vel=(92, 60))
rise_violas = held("rise_violas", PROG_A, 52)
rise_horns = melody("rise_horns", THEME, octave=-1, swell=(62, 90))
rise_choir = held("rise_choir", PROG_A, 52, octave=-1)
rise_timp = timp("rise_timp", PROG_A, vel=84)
rise_drums = Pattern("rise_drums", 8 * BAR)
grid(
    rise_drums,
    {
        GRAN_CASSA: "o............... ................ o............... ................ "
        "o............... ................ x............... ................",
        TAIKO: "................ ................ ................ ................ "
        "o.....o.....o... ................ x.....o.....o... x.....x.....x.x.",
    },
)
rise_swell = Pattern("rise_swell", 8 * BAR).add(24, 8, SWELL, 96)

# --- theme: the full statement -----------------------------------------------------

theme_low = roots("theme_low", PROG_A, 100)
theme_bones = melody("theme_bones", THEME, octave=-1, vel=96)
theme_horns = melody("theme_horns", THEME, vel=108)
theme_trumpets = melody("theme_trumpets", THEME, vel=94)
theme_cellos = ostinato("theme_cellos", PROG_A, vel=(112, 80))
theme_violas = held("theme_violas", PROG_A, 84)
# The violins hold the chord's top high above the tune, tremolo.
theme_trem = held("theme_trem", PROG_A, 66, octave=1, top=2)
theme_choir = held("theme_choir", PROG_A, 84)
theme_timp = timp("theme_timp", PROG_A)
theme_piano = accents("theme_piano", PROG_A, 92, octave=0)
drums = Pattern("drums", 2 * BAR)
grid(
    drums,
    {
        TAIKO: "X.....x.....x... X.....x.....x...",
        WAR_DRUM: "....o.......o... ....o.......o...",
        GRAN_CASSA: "x............... x...............",
    },
)
drums_fill = Pattern("drums_fill", 2 * BAR)
grid(
    drums_fill,
    {
        TAIKO: "X.....x.....x... X.....x.x.x.xxxx",
        WAR_DRUM: "....o.......o... ....o.o.o.o.o.o.",
        GRAN_CASSA: "x............... x...............",
    },
)
crash = Pattern("crash", 4 * BAR).add(0, 4, CYMBAL, 108)

# --- break: one blow, then the roll -------------------------------------------------

blow_chord = Pattern("blow_chord", 2 * BAR).chord(0, 1.2, "Bb1 F2 Bb2 D3 F3 Bb3", 120)
blow_high = Pattern("blow_high", 2 * BAR).chord(0, 1.2, "D4 F4 Bb4 D5", 112)
blow_drums = Pattern("blow_drums", 2 * BAR)
blow_drums.add(0, 4, TAM_TAM, 120).add(0, 1, TAIKO, 127).add(0, 1, GRAN_CASSA, 127).add(0, 4, CYMBAL, 112)
blow_roll = roll("blow_roll", 5, "A2", 40, 118, 2)
blow_choir = Pattern("blow_choir", 2 * BAR).chord(2, 6 - 0.1, "A3 C#4 E4 A4", 60)
blow_choir.auto("Volume", [(2, 0.3), (8, 1.0)])

# --- climax: the answer -----------------------------------------------------------

climax_low = roots("climax_low", PROG_B, 108)
climax_violins = melody("climax_violins", ANSWER, vel=104)
climax_trumpets = melody("climax_trumpets", ANSWER, vel=106)
climax_horns = melody("climax_horns", ANSWER, octave=-1, vel=92)
climax_bones = accents("climax_bones", PROG_B, 98, octave=-1, voicing=True, length=0.9)
climax_cellos = ostinato("climax_cellos", PROG_B, vel=(116, 86), sixteenths=True)
climax_violas = held("climax_violas", PROG_B, 92)
climax_choir = held("climax_choir", PROG_B, 100)
climax_choir_low = roots("climax_choir_low", PROG_B, 80, octave=1)
climax_timp = timp("climax_timp", PROG_B, vel=112)
climax_piano = accents("climax_piano", PROG_B, 100)

# --- close: the last blow, the motif over a bare fifth ------------------------------

close_hit = Pattern("close_hit", 4 * BAR).chord(0, 3, "D1 D2 A2 D3 A3 D4", 120)
close_drums = Pattern("close_drums", 4 * BAR)
close_drums.add(0, 8, TAM_TAM, 124).add(0, 1, TAIKO, 127).add(0, 1, GRAN_CASSA, 127).add(0, 4, CYMBAL, 110)
close_drums.add(12, 4, GRAN_CASSA, 70)
close_high = Pattern("close_high", 4 * BAR).chord(0, 3, "D4 A4 D5", 116)
close_fifth = Pattern("close_fifth", 4 * BAR).chord(0, 16 - 0.2, "D2 A2", 70)
close_violins = Pattern("close_violins", 4 * BAR).chord(2, 14 - 0.2, "D5 A5", 50)
close_choir = Pattern("close_choir", 4 * BAR).chord(0, 16 - 0.2, "D3 A3 D4", 70)
close_piano = Pattern("close_piano", 4 * BAR)
close_piano.seq(4, [("D5", 1.5), ("E5", 0.5), ("F5", 1), ("A5", 1), ("D6", 1), ("Eb6", 0.5), ("D6", 3.5)], vel=58)
close_piano.chord(4, 11.5, "D2 A2", 44)

patterns = [
    open_pedal, open_cellos, open_hit, open_choir, open_piano,
    rise_bass, rise_cellos, rise_violas, rise_horns, rise_choir, rise_timp, rise_drums, rise_swell,
    theme_low, theme_bones, theme_horns, theme_trumpets, theme_cellos, theme_violas, theme_trem, theme_choir,
    theme_timp, theme_piano, drums, drums_fill, crash,
    blow_chord, blow_high, blow_drums, blow_roll, blow_choir,
    climax_low, climax_violins, climax_trumpets, climax_horns, climax_bones, climax_cellos, climax_violas,
    climax_choir, climax_choir_low, climax_timp, climax_piano,
    close_hit, close_high, close_drums, close_fifth, close_violins, close_choir, close_piano,
]  # fmt: skip

tracks = [
    track("Percussion", use("percussion"), db=-2, sends=[("hall", -8)], colour=COL_DRUMS),
    track("Timpani", use("timpani"), db=-1, pan=0.1, sends=[("hall", -8)], colour=COL_DRUMS),
    track("Basses", use("basses"), db=-9, pan=0.15, sends=[("hall", -10)], colour=COL_BASS),
    track("Tuba", use("tuba"), db=-4, pan=0.25, sends=[("hall", -8)], colour=COL_BASS),
    track("Cellos", use("cellos"), db=-6, pan=0.2, sends=[("hall", -7)], colour=COL_STRINGS),
    track("Cello ostinato", use("cellos_spic"), db=-4, pan=0.15, sends=[("hall", -9)], colour=COL_STRINGS),
    track("Violas", use("violas"), db=-10, pan=0.05, sends=[("hall", -6)], colour=COL_STRINGS),
    track("Violins", use("violins"), db=-1, pan=-0.2, sends=[("hall", -5)], colour=COL_STRINGS),
    track("Violins tremolo", use("violins_trem"), db=-6, pan=-0.25, sends=[("hall", -4)], colour=COL_STRINGS),
    track("Horns", use("horns"), db=0, pan=-0.1, sends=[("hall", -5)], colour=COL_BRASS),
    track("Trumpets", use("trumpets"), db=-1, pan=-0.05, sends=[("hall", -5)], colour=COL_BRASS),
    track("Trombones", use("trombones"), db=-3, pan=0.3, sends=[("hall", -7)], colour=COL_BRASS),
    # Sung "ah" for the big moments, "oo" for the quiet ones; under the strings, a colour.
    track("Choir", use("choir_ah"), db=-6, pan=0.0, sends=[("hall", -2)], effects=[eq(high_db=-4.0, high_hz=4000)], colour=COL_PAD),
    track("Choir oo", use("choir_oo"), db=-4, pan=0.0, sends=[("hall", -2)], effects=[eq(high_db=-4.0, high_hz=4000)], colour=COL_PAD),
    track("Piano", use("piano"), db=7, pan=0.0, sends=[("hall", -4)], effects=[eq(high_db=-3.0, high_hz=5000)], colour=COL_BELL),
    track("Piano pulse", use("piano"), db=-7, pan=0.05, sends=[("hall", -8)], effects=[eq(high_db=-3.0, high_hz=5000)], colour=COL_BELL),
]


def theme_clips(t=0):
    return [
        clip("Basses", "theme_low", transpose=t),
        clip("Tuba", "theme_low", transpose=t),
        clip("Trombones", "theme_bones", transpose=t),
        clip("Horns", "theme_horns", transpose=t),
        clip("Trumpets", "theme_trumpets", transpose=t),
        clip("Cellos", "theme_low", transpose=t + 12),
        clip("Cello ostinato", "theme_cellos", transpose=t),
        clip("Violas", "theme_violas", transpose=t),
        clip("Violins tremolo", "theme_trem", transpose=t),
        clip("Choir", "theme_choir", transpose=t),
        clip("Timpani", "theme_timp", transpose=t),
        clip("Piano pulse", "theme_piano", transpose=t),
        clip("Percussion", "drums", times=3),
        clip("Percussion", "drums_fill", at=24, times=1),
        clip("Percussion", "crash"),
    ]


sections = [
    section(
        "open",
        4,
        [
            clip("Basses", "open_pedal"),
            clip("Cellos", "open_cellos"),
            clip("Percussion", "open_hit"),
            clip("Choir oo", "open_choir"),
            clip("Piano", "open_piano"),
        ],
        intensity=(0.0, 0.3),
    ),
    section(
        "rise",
        8,
        [
            clip("Basses", "rise_bass"),
            clip("Cello ostinato", "rise_cellos"),
            clip("Violas", "rise_violas"),
            clip("Horns", "rise_horns"),
            clip("Choir oo", "rise_choir"),
            clip("Timpani", "rise_timp"),
            clip("Percussion", "rise_drums"),
            clip("Percussion", "rise_swell"),
        ],
        intensity=(0.2, 0.6),
    ),
    section("theme", 8, theme_clips(), intensity=(0.5, 1.0)),
    section(
        "break",
        2,
        [
            clip("Basses", "blow_chord"),
            clip("Tuba", "blow_chord"),
            clip("Trombones", "blow_chord"),
            clip("Cellos", "blow_chord"),
            clip("Horns", "blow_high"),
            clip("Trumpets", "blow_high"),
            clip("Violins", "blow_high", transpose=12),
            clip("Percussion", "blow_drums"),
            clip("Timpani", "blow_roll"),
            clip("Choir", "blow_choir"),
        ],
        kind="Bridge",
        intensity=(0.5, 1.0),
    ),
    section(
        "climax",
        8,
        [
            clip("Basses", "climax_low"),
            clip("Tuba", "climax_low"),
            clip("Trombones", "climax_bones"),
            clip("Horns", "climax_horns"),
            clip("Trumpets", "climax_trumpets"),
            clip("Violins", "climax_violins"),
            clip("Violins tremolo", "climax_violins", transpose=-12),
            clip("Cellos", "climax_low", transpose=12),
            clip("Cello ostinato", "climax_cellos"),
            clip("Violas", "climax_violas"),
            clip("Choir", "climax_choir"),
            clip("Choir oo", "climax_choir_low"),
            clip("Timpani", "climax_timp"),
            clip("Piano pulse", "climax_piano"),
            clip("Percussion", "drums", times=3),
            clip("Percussion", "drums_fill", at=24, times=1),
            clip("Percussion", "crash"),
            clip("Percussion", "crash", at=16, times=1),
        ],
        intensity=(0.8, 1.0),
    ),
    # The theme again, a whole step up, with the violins doubling it an octave above.
    section(
        "lift",
        8,
        theme_clips(2) + [clip("Violins", "theme_horns", transpose=14), clip("Percussion", "crash", at=16, times=1)],
        intensity=(0.8, 1.0),
    ),
    section(
        "close",
        4,
        [
            clip("Basses", "close_hit", transpose=2),
            clip("Tuba", "close_hit", transpose=2),
            clip("Trombones", "close_hit", transpose=2),
            clip("Horns", "close_high", transpose=2),
            clip("Trumpets", "close_high", transpose=2),
            clip("Cellos", "close_fifth", transpose=14),
            clip("Basses", "close_fifth", transpose=2, at=0),
            clip("Violins", "close_violins", transpose=2),
            clip("Choir oo", "close_choir", transpose=2),
            clip("Percussion", "close_drums"),
            clip("Piano", "close_piano", transpose=2),
        ],
        kind="Bridge",
        intensity=(0.0, 1.0),
    ),
]

SONG = song(
    "Meridian",
    80,
    2,
    "Minor",
    tracks,
    [hall(size=0.95, decay=3.8, predelay=30.0, damp=5000.0, lowcut=150.0)],
    master(glue=True),
    patterns,
    sections,
    ["open", "rise", "theme", "break", "climax", "lift", "close"],
    notes="Main theme, cut like a trailer. D minor, 80 bpm. One tune: D-E-F-A up to D, falling back, "
    "i-bVI-iv-V with the flat second (Eb) in bar 6. open: piano motif over a D pedal; rise: cello 3+3+2, "
    "theme low on horns; theme: horns + trumpets, trombones under, drums, choir; break: one blow, timpani "
    "roll; climax: the answer high (bVI-bVII-i); lift: the theme a step up (E minor); close: bare fifth.",
)
