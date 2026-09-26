"""Reach Command: ARC's battle music. C minor, 120 bpm, adaptive.

Disciplined and heavy, a superpower's war machine, but minor and never
triumphant for long: ARC is the side that falls. The director moves between
three bands of intensity:

    calm      0.00-0.30  strings, sub, drone, a quiet arp; the Seed motif on a bell
    tension   0.25-0.60  the galloping ostinato and taiko; a snare roll into the fight
    battle    0.55-1.00  full kit, stabs, and (above 0.7) the horn melody

Stingers play over whatever is running (nuke, commander_lost, enemy_commander,
titan) and the match ends on an ending (victory, defeat).
"""

from compose import Pattern, arp, clip, grid, keys, section, song, track
from common import (
    COL_ARP,
    COL_BASS,
    COL_BELL,
    COL_BRASS,
    COL_DRUMS,
    COL_PAD,
    COL_STRINGS,
    duck,
    echo,
    eq,
    hall,
    master,
)
from instruments import (
    ANVIL,
    BOOM,
    CRASH,
    HAT,
    HAT_OPEN,
    KICK,
    SHAKER,
    SNARE,
    SNARE2,
    TAIKO,
    TAIKO_HI,
    TOM_HI,
    TOM_LO,
    TOM_MID,
    get,
)

BAR = 4

# i - VI - III - VII: the epic minor loop. (strings voicing, bass)
LOOP = [("G3 C4 Eb4", "C2"), ("Ab3 C4 Eb4", "Ab1"), ("G3 Bb3 Eb4", "Eb2"), ("F3 Bb3 D4", "Bb1")]
# Calm colours the same loop: added ninth, major seventh.
CALM = [("G3 C4 D4 Eb4", "C2"), ("G3 Ab3 C4 Eb4", "Ab1"), ("G3 Bb3 D4 Eb4", "Eb2"), ("F3 Bb3 C4 D4", "Bb1")]
# Tension walks the bass down to the dominant: Cm, Cm/Bb, Ab, G.
TENSION = [("G3 C4 Eb4", "C2"), ("G3 C4 Eb4", "Bb1"), ("Ab3 C4 Eb4", "Ab1"), ("G3 B3 D4", "G1")]


def chords(name, prog, bars_each, vel=84):
    p = Pattern(name, len(prog) * bars_each * BAR)
    for i, (ch, _) in enumerate(prog):
        p.chord(i * bars_each * BAR, bars_each * BAR, ch, vel)
    return p


def sub(name, prog, bars_each, vel=100):
    p = Pattern(name, len(prog) * bars_each * BAR)
    for i, (_, b) in enumerate(prog):
        p.add(i * bars_each * BAR, bars_each * BAR - 0.25, b, vel)
    return p


def gallop(name, prog, bars_each):
    """The tension ostinato: eighth + two sixteenths on every beat, on the root."""
    p = Pattern(name, len(prog) * bars_each * BAR)
    for i, (_, b) in enumerate(prog):
        root = keys(b)[0] + 12
        for beat in range(bars_each * BAR):
            t = i * bars_each * BAR + beat
            accent = 112 if beat % 4 == 0 else 96
            p.add(t, 0.45, root, accent)
            p.add(t + 0.5, 0.22, root, 78)
            p.add(t + 0.75, 0.22, root, 84)
    return p


def drive(name, prog):
    """The battle ostinato: straight sixteenths, the last beat of each bar climbing."""
    p = Pattern(name, len(prog) * BAR)
    for i, (_, b) in enumerate(prog):
        root = keys(b)[0] + 12
        for s in range(16):
            t = i * BAR + s * 0.25
            k = root
            if s == 14:
                k = root + 12
            elif s == 15:
                k = root + 7
            v = (112, 70, 88, 72)[s % 4]
            p.add(t, 0.2, k, v)
    return p


def calm_arp(name, prog, bars_each):
    p = Pattern(name, len(prog) * bars_each * BAR)
    for i, (ch, _) in enumerate(prog):
        top = [k + 12 for k in keys(ch)]
        arp(p, i * bars_each * BAR, bars_each * BAR, top, step=0.5, order="updown", vel=(64, 44), length=0.42)
    return p


def stabs(name, prog):
    p = Pattern(name, len(prog) * BAR)
    for i, (ch, _) in enumerate(prog):
        ks = [k + 12 for k in keys(ch)]
        for s, v in ((2, 96), (6, 84), (10, 100), (13, 90)):
            p.chord(i * BAR + s * 0.25, 0.2, ks, v)
    return p


# --- Calm -------------------------------------------------------------------

drone = Pattern("drone", 4 * BAR).chord(0, 16, "C2 G2", 90)
calm_strings = chords("calm_strings", CALM, 2, vel=76)
calm_sub = sub("calm_sub", CALM, 2, vel=92)
calm_arps = calm_arp("calm_arp", CALM, 2)

# The Seed motif in C (G C D Eb, falling back), over the calm loop.
seed = Pattern("seed", 8 * BAR)
seed.seq(0, [(None, 1), ("G4", 1), ("C5", 1), ("D5", 1), ("Eb5", 2), ("D5", 1), ("C5", 1)], vel=90)
seed.seq(8, [(None, 1), ("Eb4", 1), ("Ab4", 1), ("C5", 1), ("Bb4", 3), ("G4", 1)], vel=84)
seed.seq(16, [(None, 1), ("Bb4", 1), ("Eb5", 1), ("G5", 1), ("F5", 2), ("Eb5", 1), ("D5", 1)], vel=88)
seed.seq(24, [(None, 1), ("F5", 1), ("Eb5", 2), ("D5", 4)], vel=82)

pulse = Pattern("pulse", 2 * BAR)
grid(pulse, {TAIKO: "o............... ........g.......", SHAKER: "..g...g...g...g. ..g...g...g...g."})

# --- Tension ----------------------------------------------------------------

tension_strings = chords("tension_strings", TENSION, 2, vel=86)
tension_sub = sub("tension_sub", TENSION, 2, vel=100)
tension_gallop = gallop("tension_gallop", TENSION, 2)

war_drums = Pattern("war_drums", 2 * BAR)
grid(
    war_drums,
    {
        TAIKO: "X.....o.x....... X.....o.x...o.o.",
        TOM_LO: "........o....... ........o...g.g.",
        SHAKER: "gqgqgqgqgqgqgqgq gqgqgqgqgqgqgqgq",
    },
)

# A roll across the last bar into the fight, then silence on the last eighth.
roll = Pattern("roll", 8 * BAR)
for i in range(14):
    roll.add(28 + i * 0.25, 0.25, SNARE2, 36 + i * 6)
roll.add(28, 4, TAIKO_HI, 70)
roll.add(31, 0.5, TAIKO, 110)

# --- Battle -----------------------------------------------------------------

battle_strings = chords("battle_strings", LOOP, 1, vel=92)
battle_sub = sub("battle_sub", LOOP, 1, vel=104)
battle_drive = drive("battle_drive", LOOP)
battle_stabs = stabs("battle_stabs", LOOP)

beat = Pattern("beat", 2 * BAR)
grid(
    beat,
    {
        KICK: "x.......x.x..... x.......x.x...x.",
        SNARE: "....x.......x... ....x.......x.x.",
        HAT: "xgogxgogxgogxgog xgogxgogxgogxg..",
        HAT_OPEN: "................ ..............o.",
        TAIKO: "X............... X...........o...",
    },
)

# Same beat, the second bar ending in a tom run down the kit.
beat_fill = Pattern("beat_fill", 2 * BAR)
grid(
    beat_fill,
    {
        KICK: "x.......x.x..... x.......x.......",
        SNARE: "....x.......x... ....x...........",
        HAT: "xgogxgogxgogxgog xgogxgogx.......",
        TOM_HI: "................ ..........xo....",
        TOM_MID: "................ ............xo..",
        TOM_LO: "................ ..............xX",
        TAIKO: "X............... X...............",
    },
)

crash = Pattern("crash", 8 * BAR).add(0, 1, CRASH, 110)

# The horn line, above 0.7: heroic for four bars, then it sinks.
horn = Pattern("horn", 8 * BAR)
horn.seq(
    0,
    [
        ("G4", 1.5), ("F4", 0.5), ("Eb4", 1), ("D4", 1),
        ("C4", 2), ("Eb4", 1), ("G4", 1),
        ("Bb4", 2), ("G4", 1), ("Eb4", 1),
        ("F4", 3), ("D4", 1),
        ("G4", 1.5), ("F4", 0.5), ("Eb4", 1), ("G4", 1),
        ("C5", 2), ("Bb4", 1), ("Ab4", 1),
        ("G4", 2), ("Bb4", 1), ("Eb5", 1),
        ("D5", 3), (None, 1),
    ],
    vel=100,
    gap=0.96,
)

# --- Breakdown --------------------------------------------------------------

breakdown_strings = Pattern("breakdown_strings", 4 * BAR)
breakdown_strings.chord(0, 8, "Ab3 C4 Eb4 G4", 80).chord(8, 8, "F3 Bb3 D4 F4", 88)
breakdown_sub = Pattern("breakdown_sub", 4 * BAR).add(0, 7.75, "Ab1", 96).add(8, 7.75, "Bb1", 100)
breakdown_drums = Pattern("breakdown_drums", 4 * BAR)
grid(breakdown_drums, {BOOM: "X............... ................ ................ ................"})
grid(breakdown_drums, {TAIKO: "................ X............... X.......o....... X...o...X...o.o."})
for i in range(8):
    breakdown_drums.add(14 + i * 0.25, 0.25, SNARE2, 50 + i * 9)

# --- Stingers ---------------------------------------------------------------

nuke_drums = Pattern("nuke_drums", 2 * BAR)
nuke_drums.add(0, 1, BOOM, 127).add(0, 1, CRASH, 120).add(0, 1, ANVIL, 90).add(0.06, 1, TAIKO_HI, 100)
nuke_brass = Pattern("nuke_brass", 2 * BAR)
nuke_brass.chord(0, 7.5, "C3 Db3 G3 C4 Db4", 110)

lost_strings = Pattern("lost_strings", 2 * BAR)
lost_strings.seq(0, [("Eb5", 2), ("D5", 2), ("C5", 2), ("B4", 2)], vel=96)
lost_drums = Pattern("lost_drums", 2 * BAR).add(0, 1, BOOM, 110)

fanfare = Pattern("fanfare", 2 * BAR)
t3 = 1 / 3
fanfare.seq(0, [("G4", t3), ("C5", t3), ("Eb5", t3), ("G5", 3), ("F5", t3), ("Eb5", t3), ("F5", t3), ("G5", 3)], vel=108, gap=0.95)
fanfare_drums = Pattern("fanfare_drums", 2 * BAR)
grid(fanfare_drums, {TAIKO: "X...X........... X...X...X.X.XXXX", CRASH: "X............... ................"})

titan_drums = Pattern("titan_drums", 2 * BAR)
grid(titan_drums, {TAIKO: "X..X..X.X.X.XXXX X...............", BOOM: "................ X..............."})
titan_brass = Pattern("titan_brass", 2 * BAR).chord(4, 4, "C3 G3 C4", 112)

# --- Endings ----------------------------------------------------------------

victory_strings = Pattern("victory_strings", 4 * BAR)
victory_strings.chord(0, 2, "Ab3 C4 Eb4", 96).chord(2, 2, "Bb3 D4 F4", 102).chord(4, 12, "C4 E4 G4 C5", 110)
victory_brass = Pattern("victory_brass", 4 * BAR)
victory_brass.chord(0, 2, "Ab2 Eb3 C4", 100).chord(2, 2, "Bb2 F3 D4", 106).chord(4, 11, "C3 G3 E4", 114)
victory_sub = Pattern("victory_sub", 4 * BAR).add(0, 2, "Ab1", 100).add(2, 2, "Bb1", 100).add(4, 12, "C2", 110)
victory_drums = Pattern("victory_drums", 4 * BAR)
for i in range(16):
    victory_drums.add(i * 0.25, 0.25, SNARE2, 40 + i * 5)
victory_drums.add(4, 1, CRASH, 124).add(4, 1, BOOM, 120).add(4, 1, TAIKO, 124)

defeat_strings = Pattern("defeat_strings", 4 * BAR)
defeat_strings.chord(0, 4, "G3 C4 Eb4", 70).chord(4, 4, "Ab3 C4 Eb4", 66).chord(8, 4, "Ab3 C4 F4", 62).chord(12, 4, "G3 C4 Eb4", 56)
defeat_sub = Pattern("defeat_sub", 4 * BAR).add(0, 4, "C2", 80).add(4, 4, "Ab1", 76).add(8, 4, "F1", 72).add(12, 4, "C2", 64)
defeat_bell = Pattern("defeat_bell", 4 * BAR)
defeat_bell.seq(8, [("G4", 1), ("C5", 1), ("D5", 1), ("Eb5", 5)], vel=70)
defeat_drums = Pattern("defeat_drums", 4 * BAR).add(0, 1, BOOM, 96)

patterns = [
    drone, calm_strings, calm_sub, calm_arps, seed, pulse,
    tension_strings, tension_sub, tension_gallop, war_drums, roll,
    battle_strings, battle_sub, battle_drive, battle_stabs, beat, beat_fill, crash, horn,
    breakdown_strings, breakdown_sub, breakdown_drums,
    nuke_drums, nuke_brass, lost_strings, lost_drums, fanfare, fanfare_drums, titan_drums, titan_brass,
    victory_strings, victory_brass, victory_sub, victory_drums, defeat_strings, defeat_sub, defeat_bell, defeat_drums,
]  # fmt: skip

tracks = [
    track("drums", get("war_kit"), db=-12, sends=[("hall", -12)], effects=[eq(low_db=1.5, low_hz=90, mid_db=-1.5, mid_hz=450, high_db=1.0, high_hz=8000)], colour=COL_DRUMS),
    track("sub", get("sub_bass"), db=-5, effects=[duck("drums", depth=-12, ratio=2.0, release=110)], colour=COL_BASS),
    track(
        "ostinato",
        get("pulse_bass"),
        db=-2,
        sends=[("hall", -18)],
        effects=[eq(low_db=-2, low_hz=90, mid_db=2.0, mid_hz=900, cut_hz=40), duck("drums", depth=-12, ratio=2.0, release=90)],
        layer=(0.15, 0.3),
        colour=COL_BASS,
    ),
    track("drone", get("drone"), db=-12, sends=[("hall", -8)], layer=(0.0, 0.0, 0.6), colour=COL_PAD),
    track(
        "strings",
        get("wide_strings"),
        db=-5,
        sends=[("hall", -5)],
        effects=[eq(low_db=-4, low_hz=200, cut_hz=90)],
        follow=[("Cutoff", 0.45, 0.62)],
        colour=COL_STRINGS,
    ),
    track("strings hi", get("wide_strings"), db=-6, pan=0.12, sends=[("hall", -4)], effects=[eq(cut_hz=200)], colour=COL_STRINGS),
    track("arp", get("pluck"), db=-8, pan=-0.2, sends=[("echo", -6), ("hall", -10)], layer=(0.0, 0.0, 0.45), colour=COL_ARP),
    track("bell", get("seed_bell"), db=-9, pan=0.15, sends=[("hall", -4), ("echo", -12)], colour=COL_BELL),
    track("stabs", get("stab"), db=-7, pan=-0.1, sends=[("hall", -10), ("echo", -16)], effects=[eq(cut_hz=150)], layer=(0.58, 0.7), colour=COL_BRASS),
    track("horn", get("brass"), db=-3, pan=0.08, sends=[("hall", -6)], effects=[eq(cut_hz=110, mid_db=1.5, mid_hz=1400)], colour=COL_BRASS),
]

LOOPS = ["calm_a", "calm_b", "tension", "battle_a", "battle_b"]

sections = [
    section(
        "calm_a",
        8,
        [clip("drone", "drone"), clip("strings", "calm_strings"), clip("sub", "calm_sub"), clip("arp", "calm_arp"), clip("drums", "pulse")],
        intensity=(0.0, 0.3),
        nxt=["calm_a", "calm_b", "tension"],
    ),
    section(
        "calm_b",
        8,
        [
            clip("drone", "drone"),
            clip("strings", "calm_strings"),
            clip("sub", "calm_sub"),
            clip("arp", "calm_arp"),
            clip("bell", "seed"),
            clip("drums", "pulse"),
        ],
        intensity=(0.0, 0.3),
        nxt=["calm_a", "tension"],
    ),
    section(
        "tension",
        8,
        [
            clip("drone", "drone"),
            clip("strings", "tension_strings"),
            clip("sub", "tension_sub"),
            clip("ostinato", "tension_gallop"),
            clip("drums", "war_drums"),
            clip("drums", "roll"),
        ],
        intensity=(0.25, 0.6),
        nxt=["tension", "battle_a", "calm_a"],
        exit_every=4,
    ),
    section(
        "battle_a",
        8,
        [
            clip("strings", "battle_strings"),
            clip("sub", "battle_sub"),
            clip("ostinato", "battle_drive"),
            clip("stabs", "battle_stabs"),
            clip("drums", "beat", times=3),
            clip("drums", "beat_fill", at=24, times=1),
            clip("drums", "crash"),
        ],
        intensity=(0.55, 1.0),
        nxt=["battle_a", "battle_b", "tension"],
        exit_every=4,
    ),
    section(
        "battle_b",
        8,
        [
            clip("strings", "battle_strings"),
            clip("sub", "battle_sub"),
            clip("ostinato", "battle_drive"),
            clip("stabs", "battle_stabs"),
            clip("horn", "horn"),
            clip("drums", "beat", times=3),
            clip("drums", "beat_fill", at=24, times=1),
            clip("drums", "crash"),
        ],
        intensity=(0.7, 1.0),
        nxt=["battle_a", "breakdown", "tension"],
        exit_every=4,
    ),
    section(
        "breakdown",
        4,
        [clip("strings", "breakdown_strings"), clip("sub", "breakdown_sub"), clip("drums", "breakdown_drums"), clip("bell", "seed")],
        kind="Bridge",
        intensity=(0.5, 1.0),
        nxt=["battle_b", "battle_a"],
    ),
    section("nuke", 2, [clip("drums", "nuke_drums"), clip("horn", "nuke_brass")], kind="Stinger"),
    section("commander_lost", 2, [clip("strings hi", "lost_strings"), clip("drums", "lost_drums")], kind="Stinger"),
    section("enemy_commander", 2, [clip("horn", "fanfare"), clip("drums", "fanfare_drums")], kind="Stinger"),
    section("titan", 2, [clip("drums", "titan_drums"), clip("horn", "titan_brass")], kind="Stinger"),
    section(
        "victory",
        4,
        [clip("strings hi", "victory_strings"), clip("horn", "victory_brass"), clip("sub", "victory_sub"), clip("drums", "victory_drums")],
        kind="Ending",
    ),
    section(
        "defeat",
        4,
        [clip("strings", "defeat_strings"), clip("sub", "defeat_sub"), clip("bell", "defeat_bell"), clip("drums", "defeat_drums")],
        kind="Ending",
    ),
]

SONG = song(
    "Reach Command",
    120,
    0,
    "Minor",
    tracks,
    [hall(size=0.8, decay=2.8, predelay=22.0), echo(beats=0.75, feedback=0.36)],
    master(glue=True),
    patterns,
    sections,
    # Straight through, the way the director would climb a battle and end it.
    ["calm_a", "calm_b", "tension", "battle_a", "battle_b", "breakdown", "battle_b", "victory"],
    notes="ARC battle. C minor, 120 bpm. calm 0-0.3 (Seed motif on the bell in calm_b), tension 0.25-0.6 "
    "(gallop + taiko, roll into the fight), battle 0.55-1 (horn melody in battle_b above 0.7). "
    "Layers: ostinato fades in from 0.15, stabs from 0.58, drone and arp fade out above 0.6 / 0.45. "
    "Strings brighten with intensity (follow Cutoff). Stingers: nuke, commander_lost, enemy_commander, titan. "
    "Endings: victory (picardy C major), defeat.",
)
