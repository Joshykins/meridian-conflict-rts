"""Reach Command: ARC's battle music. C minor, 96 bpm, a recorded orchestra.

ARC is the side that is losing, so nothing here is bright or heroic for long.
The harmony sits on a C pedal and leans on the dark degrees: the flat second
(Db over C), the minor fourth, the tritone (F#/Gb against C), a diminished
seventh over the pedal. Brass plays open fifths, never major triads; the
melody moves by falling steps and half-step sighs. Every part is a recorded
instrument from the library (data/music/instruments), so swapping one is a
change of name.

    calm      0.00-0.30  held strings over the pedal; harp tolls; piano lament in calm_b
    tension   0.25-0.60  cello spiccato in 3+3+2, tremolo violins, timpani, low horns swell
    battle    0.55-1.00  tritone swing in open fifths, horn stabs, drums; the horn theme in battle_b
    breakdown            the floor drops out: pedal, tremolo, tam-tam, the lament again
"""

from compose import Pattern, clip, grid, keys, section, song, track
from common import COL_BASS, COL_BELL, COL_BRASS, COL_DRUMS, COL_PAD, COL_STRINGS, eq, hall, master
from ron import Newtype

BAR = 4

# Percussion keys (data/music/samples/percussion).
GRAN_CASSA = 34
TAIKO = 35
SNARE = 38
WAR_DRUM = 41
CYMBAL = 49
SWELL = 51
TAM_TAM = 52


def use(name):
    return Newtype("Use", name)


def held(name, prog, bars_each, vel, octave=0):
    """Chords held for `bars_each` bars each (slightly short so bows change)."""
    p = Pattern(name, len(prog) * bars_each * BAR)
    for i, ch in enumerate(prog):
        p.chord(i * bars_each * BAR, bars_each * BAR - 0.1, [k + 12 * octave for k in keys(ch)], vel)
    return p


def ostinato(name, roots, bars_each, vel=(104, 72), sixteenths=False):
    """Cello spiccato: eighths accented 3+3+2, the last pair up an octave and back."""
    p = Pattern(name, len(roots) * bars_each * BAR)
    accents = (0, 3, 6)
    for i, r in enumerate(roots):
        root = keys(r)[0]
        for b in range(bars_each):
            for e in range(8):
                t = (i * bars_each + b) * BAR + e * 0.5
                k = root + 12 if e == 6 else root
                v = vel[0] if e in accents else vel[1]
                if sixteenths and e in (2, 5):
                    p.add(t, 0.22, k, vel[1] - 8)
                    p.add(t + 0.25, 0.22, k, vel[1])
                else:
                    p.add(t, 0.4, k, v)
    return p


def timpani_hits(name, roots, bars_each, fill=True):
    """On the 3+3+2 accents, with a roll-like run into each change."""
    p = Pattern(name, len(roots) * bars_each * BAR)
    for i, r in enumerate(roots):
        root = keys(r)[0]
        start = i * bars_each * BAR
        for b in range(bars_each):
            t = start + b * BAR
            p.add(t, 1, root, 112 if b == 0 else 96)
            p.add(t + 1.5, 0.5, root, 84)
            p.add(t + 3, 0.5, root, 90)
        if fill:
            end = start + bars_each * BAR
            for j in range(4):
                p.add(end - 1 + j * 0.25, 0.25, root, 70 + j * 10)
    return p


def stabs(name, fifths, vel=100):
    """Horn stabs on the off-beats, open fifths (no thirds), one bar per fifth."""
    p = Pattern(name, len(fifths) * BAR)
    for i, f in enumerate(fifths):
        ks = keys(f)
        for s, v in ((1.5, vel), (3.5, vel - 12)):
            p.chord(i * BAR + s, 0.4, ks, v)
    return p


# --- Calm: waiting ------------------------------------------------------------

# Over a C pedal: Cm, Db/C (the flat second), Fm/C, B dim7/C; two bars each.
CALM = ["G3 C4 Eb4", "Ab3 Db4 F4", "Ab3 C4 F4", "B3 D4 F4 Ab4"]

pedal = Pattern("pedal", 8 * BAR).add(0, 32 - 0.1, "C2", 58)
drone = Pattern("drone", 8 * BAR).chord(0, 32 - 0.1, "C2 G2", 34)
calm_violas = held("calm_violas", CALM, 2, vel=52)
calm_violins = held("calm_violins", ["C4 Eb4", "Db4 F4", "C4 F4", "D4 Ab4"], 2, vel=46, octave=1)

# The harp tolls low: C and G, a bar apart, like a slow bell.
toll = Pattern("toll", 8 * BAR)
for b in range(0, 8, 2):
    toll.chord(b * BAR, 3.9, "C2 C3", 76)
    toll.add((b + 1) * BAR + 2, 1.9, "G2", 64)

# The lament, on the piano: falling steps and half-step sighs.
lament = Pattern("lament", 8 * BAR)
lament.seq(0, [(None, 1), ("G4", 1), ("Ab4", 1), ("G4", 2), ("Eb4", 2), ("D4", 1)], vel=58)
lament.seq(8, [("F4", 2), ("Eb4", 1), ("Db4", 3), ("C4", 2)], vel=54)
lament.seq(16, [(None, 1), ("Ab4", 1), ("G4", 1), ("F4", 2), ("Eb4", 1), ("F4", 2)], vel=56)
lament.seq(24, [("D4", 2), ("Eb4", 1), ("B3", 5)], vel=50)
# The left hand: one low octave per chord.
for i, low in enumerate(["C2 C3", "Db2 Db3", "F1 F2", "G1 G2"]):
    lament.chord(i * 2 * BAR, 7.5, low, 44)

heartbeat = Pattern("heartbeat", 2 * BAR)
grid(heartbeat, {GRAN_CASSA: "o...............  ................"})

# --- Tension: something is coming --------------------------------------------

# C pedal; Cm, Db/C, Cm, Gb/C (the tritone over the pedal); two bars each.
TENSION = ["G3 C4 Eb4", "Ab3 Db4 F4", "G3 C4 Eb4", "Gb3 Bb3 Db4"]
tension_violas = held("tension_violas", TENSION, 2, vel=70)
tension_trem = held("tension_trem", ["G4 C5", "Ab4 Db5", "G4 C5", "Gb4 Db5"], 2, vel=60)
tension_bass = Pattern("tension_bass", 8 * BAR).add(0, 32 - 0.1, "C2", 80)
tension_cellos = ostinato("tension_cellos", ["C3", "C3", "C3", "C3"], 2, vel=(96, 64))
# Low horns swell on an open fifth and fall away.
tension_horns = Pattern("tension_horns", 8 * BAR)
tension_horns.chord(4, 12, "C3 G3", 60).chord(20, 12, "Db3 Ab3", 64)
tension_timpani = timpani_hits("tension_timpani", ["C3", "C3", "C3", "C3"], 2, fill=False)

war_drums = Pattern("war_drums", 2 * BAR)
grid(
    war_drums,
    {
        TAIKO: "X.....o.....o... X.....o.....o.o.",
        WAR_DRUM: "........g....... ........g...g...",
        GRAN_CASSA: "o............... ................",
    },
)

# Into the fight: a snare roll and a cymbal swell over the last two bars.
build = Pattern("build", 8 * BAR)
for i in range(28):
    build.add(25 + i * 0.25, 0.25, SNARE, 30 + i * 3)
build.add(24, 8, SWELL, 90)
build.add(31.5, 0.5, TAIKO, 118)

# --- Battle: the tritone swing -------------------------------------------------

# Winter Contingency's shape: i5 - #IV5 - i5 - bVI5, one bar each, twice.
battle_low = Pattern("battle_low", 8 * BAR)
for rep in range(2):
    for i, r in enumerate(["C2 G2", "F#1 C#2", "C2 G2", "Ab1 Eb2"]):
        battle_low.chord((rep * 4 + i) * BAR, BAR - 0.1, r, 104)
battle_bones = Pattern("battle_bones", 8 * BAR)
for rep in range(2):
    for i, r in enumerate(["C3 G3", "F#2 C#3", "C3 G3", "Ab2 Eb3"]):
        battle_bones.chord((rep * 4 + i) * BAR, BAR - 0.2, r, 100)
battle_violins = Pattern("battle_violins", 8 * BAR)
for rep in range(2):
    for i, r in enumerate(["G4 C5 Eb5", "F#4 C#5 F#5", "G4 C5 Eb5", "Ab4 C5 Eb5"]):
        battle_violins.chord((rep * 4 + i) * BAR, BAR - 0.1, r, 88)
battle_violas = Pattern("battle_violas", 8 * BAR)
for rep in range(2):
    for i, r in enumerate(["C4 G4", "C#4 F#4", "C4 G4", "C4 Eb4"]):
        battle_violas.chord((rep * 4 + i) * BAR, BAR - 0.1, r, 92)
battle_cellos = ostinato("battle_cellos", ["C3", "F#2", "C3", "Ab2"] * 2, 1, vel=(112, 84))
battle_stabs = stabs("battle_stabs", ["C4 G4", "C#4 F#4", "C4 G4", "Eb4 Ab4"] * 2)
battle_timpani = timpani_hits("battle_timpani", ["C3", "F#2", "C3", "Ab2"] * 2, 1, fill=False)

beat = Pattern("beat", 2 * BAR)
grid(
    beat,
    {
        TAIKO: "X.....x.....x... X.....x.....x.x.",
        GRAN_CASSA: "x............... x...............",
        WAR_DRUM: "....o.......o... ....o.......o.o.",
        SNARE: "..g...g...g...g. ..g...g...g.gggg",
    },
)
beat_fill = Pattern("beat_fill", 2 * BAR)
grid(
    beat_fill,
    {
        TAIKO: "X.....x.....x... X.x.x.x.XXXXXXXX",
        GRAN_CASSA: "x............... x...............",
        SNARE: "..g...g...g...g. ..o.o.o.xxxxxxxx",
    },
)
crash = Pattern("crash", 8 * BAR).add(0, 4, CYMBAL, 110)

# --- Battle b: the answer, and the theme ---------------------------------------

# i - bVI - iv - bV: Cm, Ab, Fm, Gb; two bars each.
answer_low = Pattern("answer_low", 8 * BAR)
answer_bones = Pattern("answer_bones", 8 * BAR)
for i, (lo, mid) in enumerate([("C2 G2", "C3 G3"), ("Ab1 Eb2", "Ab2 Eb3"), ("F1 C2", "F2 C3"), ("Gb1 Db2", "Gb2 Db3")]):
    answer_low.chord(i * 2 * BAR, 2 * BAR - 0.1, lo, 104)
    answer_bones.chord(i * 2 * BAR, 2 * BAR - 0.2, mid, 96)
answer_violas = held("answer_violas", ["G3 C4 Eb4", "Ab3 C4 Eb4", "Ab3 C4 F4", "Gb3 Bb3 Db4"], 2, vel=90)
answer_violins = held("answer_violins", ["C5 Eb5", "C5 Eb5", "C5 F5", "Bb4 Db5"], 2, vel=84)
answer_cellos = ostinato("answer_cellos", ["C3", "Ab2", "F2", "Gb2"], 2, vel=(112, 84), sixteenths=True)
answer_stabs = stabs("answer_stabs", ["C4 G4", "C4 G4", "Eb4 Ab4", "Eb4 Ab4", "C4 F4", "C4 F4", "Db4 Gb4", "Db4 Gb4"])
answer_timpani = timpani_hits("answer_timpani", ["C3", "Ab2", "F2", "Gb2"], 2)

# The theme: low horns (trombones under them), grave, falling. It ends on C over
# Gb, a tritone, and never resolves before the loop comes round.
theme = Pattern("theme", 8 * BAR)
theme.seq(
    0,
    [
        ("G3", 1.5), ("Ab3", 0.5), ("G3", 1), ("C4", 1), ("Eb4", 3), ("D4", 1),
        ("C4", 1.5), ("Bb3", 0.5), ("Ab3", 1), ("G3", 1), ("Ab3", 2), ("C4", 2),
        ("F4", 1.5), ("Eb4", 0.5), ("D4", 1), ("C4", 1), ("Ab3", 3), ("G3", 1),
        ("Db4", 2), ("C4", 1), ("Bb3", 1), ("Db4", 3), ("C4", 1),
    ],
    vel=100,
    gap=0.97,
)

# --- Breakdown: the floor drops out ---------------------------------------------

breakdown_bass = Pattern("breakdown_bass", 4 * BAR).add(0, 16 - 0.1, "C2", 70)
breakdown_trem = held("breakdown_trem", ["Ab4 C5 Eb5", "Ab4 Db5 F5"], 2, vel=56)
breakdown_violas = held("breakdown_violas", ["Ab3 C4 Eb4", "Ab3 Db4 F4"], 2, vel=54)
breakdown_drums = Pattern("breakdown_drums", 4 * BAR)
breakdown_drums.add(0, 4, TAM_TAM, 100)
grid(breakdown_drums, {GRAN_CASSA: "X............... ................ o............... o.......o......."})
for i in range(8):
    breakdown_drums.add(14 + i * 0.25, 0.25, SNARE, 40 + i * 8)
breakdown_piano = Pattern("breakdown_piano", 4 * BAR)
breakdown_piano.seq(0, [(None, 1), ("G4", 1), ("Ab4", 1), ("G4", 2), ("Eb4", 2), ("D4", 1)], vel=60)
breakdown_piano.seq(8, [("F4", 2), ("Eb4", 1), ("Db4", 3), ("C4", 2)], vel=56)
breakdown_piano.chord(0, 7.5, "C2 C3", 48).chord(8, 7.5, "Db2 Db3", 48)

patterns = [
    pedal, drone, calm_violas, calm_violins, toll, lament, heartbeat,
    tension_violas, tension_trem, tension_bass, tension_cellos, tension_horns, tension_timpani, war_drums, build,
    battle_low, battle_bones, battle_violins, battle_violas, battle_cellos, battle_stabs, battle_timpani, beat, beat_fill, crash,
    answer_low, answer_bones, answer_violas, answer_violins, answer_cellos, answer_stabs, answer_timpani, theme,
    breakdown_bass, breakdown_trem, breakdown_violas, breakdown_drums, breakdown_piano,
]  # fmt: skip

tracks = [
    track("Percussion", use("percussion"), db=-6, sends=[("hall", -8)], colour=COL_DRUMS),
    track("Timpani", use("timpani"), db=-2, pan=0.1, sends=[("hall", -8)], colour=COL_DRUMS),
    track("Basses", use("basses"), db=-4, pan=0.15, sends=[("hall", -10)], colour=COL_BASS),
    track("Tuba", use("tuba"), db=-3, pan=0.25, sends=[("hall", -8)], layer=(0.5, 0.6), colour=COL_BASS),
    track("Cellos", use("cellos"), db=-4, pan=0.2, sends=[("hall", -7)], colour=COL_STRINGS),
    track("Cello ostinato", use("cellos_spic"), db=-4, pan=0.15, sends=[("hall", -9)], layer=(0.15, 0.3), colour=COL_STRINGS),
    track("Violas", use("violas"), db=-5, pan=0.05, sends=[("hall", -6)], colour=COL_STRINGS),
    track("Violins", use("violins"), db=-6, pan=-0.2, sends=[("hall", -5)], colour=COL_STRINGS),
    track("Violins tremolo", use("violins_trem"), db=-3, pan=-0.25, sends=[("hall", -4)], colour=COL_STRINGS),
    track("Horns", use("horns"), db=0, pan=-0.1, sends=[("hall", -5)], colour=COL_BRASS),
    track("Horn stabs", use("horns_short"), db=-1, pan=-0.12, sends=[("hall", -6)], layer=(0.58, 0.7), colour=COL_BRASS),
    track("Trombones", use("trombones"), db=-3, pan=0.3, sends=[("hall", -7)], layer=(0.55, 0.65), colour=COL_BRASS),
    track("Harp", use("harp"), db=9, pan=-0.3, sends=[("hall", -5)], layer=(0.0, 0.0, 0.45), colour=COL_PAD),
    track("Piano", use("piano"), db=5, pan=0.0, sends=[("hall", -4)], effects=[eq(high_db=-3.0, high_hz=5000)], colour=COL_BELL),
]

sections = [
    section(
        "calm_a",
        8,
        [
            clip("Basses", "pedal"),
            clip("Cellos", "drone"),
            clip("Violas", "calm_violas"),
            clip("Harp", "toll"),
            clip("Percussion", "heartbeat"),
        ],
        intensity=(0.0, 0.3),
        nxt=["calm_a", "calm_b", "tension"],
    ),
    section(
        "calm_b",
        8,
        [
            clip("Basses", "pedal"),
            clip("Cellos", "drone"),
            clip("Violas", "calm_violas"),
            clip("Violins", "calm_violins"),
            clip("Harp", "toll"),
            clip("Piano", "lament"),
            clip("Percussion", "heartbeat"),
        ],
        intensity=(0.0, 0.3),
        nxt=["calm_a", "tension"],
    ),
    section(
        "tension",
        8,
        [
            clip("Basses", "tension_bass"),
            clip("Cellos", "drone"),
            clip("Violas", "tension_violas"),
            clip("Violins tremolo", "tension_trem"),
            clip("Cello ostinato", "tension_cellos"),
            clip("Horns", "tension_horns"),
            clip("Timpani", "tension_timpani"),
            clip("Percussion", "war_drums", times=3),
            clip("Percussion", "build"),
        ],
        intensity=(0.25, 0.6),
        nxt=["tension", "battle_a", "calm_a"],
        exit_every=4,
    ),
    section(
        "battle_a",
        8,
        [
            clip("Basses", "battle_low"),
            clip("Tuba", "battle_low"),
            clip("Trombones", "battle_bones"),
            clip("Cellos", "battle_bones"),
            clip("Violas", "battle_violas"),
            clip("Violins", "battle_violins"),
            clip("Cello ostinato", "battle_cellos"),
            clip("Horn stabs", "battle_stabs"),
            clip("Timpani", "battle_timpani"),
            clip("Percussion", "beat", times=3),
            clip("Percussion", "beat_fill", at=24, times=1),
            clip("Percussion", "crash"),
        ],
        intensity=(0.55, 1.0),
        nxt=["battle_a", "battle_b", "tension"],
        exit_every=4,
    ),
    section(
        "battle_b",
        8,
        [
            clip("Basses", "answer_low"),
            clip("Tuba", "answer_low"),
            clip("Trombones", "theme", transpose=-12),
            clip("Cellos", "answer_bones"),
            clip("Violas", "answer_violas"),
            clip("Violins", "answer_violins"),
            clip("Cello ostinato", "answer_cellos"),
            clip("Horns", "theme"),
            clip("Horn stabs", "answer_stabs"),
            clip("Timpani", "answer_timpani"),
            clip("Percussion", "beat", times=3),
            clip("Percussion", "beat_fill", at=24, times=1),
            clip("Percussion", "crash"),
        ],
        intensity=(0.7, 1.0),
        nxt=["battle_a", "breakdown", "tension"],
        exit_every=4,
    ),
    section(
        "breakdown",
        4,
        [
            clip("Basses", "breakdown_bass"),
            clip("Cellos", "drone"),
            clip("Violas", "breakdown_violas"),
            clip("Violins tremolo", "breakdown_trem"),
            clip("Piano", "breakdown_piano"),
            clip("Percussion", "breakdown_drums"),
        ],
        kind="Bridge",
        intensity=(0.5, 1.0),
        nxt=["battle_b", "battle_a"],
    ),
]

SONG = song(
    "Reach Command",
    96,
    0,
    "Minor",
    tracks,
    [hall(size=0.9, decay=3.2, predelay=28.0, damp=4800.0, lowcut=160.0)],
    master(glue=True),
    patterns,
    sections,
    # Straight through, the way a battle climbs.
    ["calm_a", "calm_b", "tension", "battle_a", "battle_b", "breakdown", "battle_b"],
    notes="ARC battle, a recorded orchestra. C minor, 96 bpm, over a C pedal: flat second (Db/C), "
    "minor iv, B dim7/C, tritone (Gb/C, F#5). Brass in open fifths, no major triads; melody by falling "
    "steps. calm: held strings, harp tolls, piano lament (calm_b). tension: cello spiccato 3+3+2, tremolo "
    "violins, timpani, low horns swell, snare roll + cymbal swell into the fight. battle_a: i5-#IV5-i5-bVI5 "
    "(Winter Contingency shape). battle_b: i-bVI-iv-bV with the horn theme (trombones an octave under). "
    "breakdown: pedal, tremolo, tam-tam, the lament. Instruments are library names (Use): swap one by name.",
)
