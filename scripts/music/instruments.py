"""The instrument library: synth patches and drum kits shared by the songs.

Each is written to data/music/instruments/<name>.ron as a preset the studio can
load, and copied into the songs that use it (a song carries its own copy, so
editing a song's instrument never changes another song).
"""

from ron import V, Newtype


def osc(wave, gain, **kw):
    d = {"wave": V(wave), "gain": float(gain)}
    d.update(kw)
    return d


def env(a, d, s, r):
    return {"a": float(a), "d": float(d), "s": float(s), "r": float(r)}


def filt(cutoff, mode="LowPass", resonance=0.0, env_oct=0.0, keytrack=0.0, velocity=0.0, drive=0.0):
    return {
        "mode": V(mode),
        "cutoff": float(cutoff),
        "resonance": float(resonance),
        "env": float(env_oct),
        "keytrack": float(keytrack),
        "velocity": float(velocity),
        "drive": float(drive),
    }


def lfo(to, rate, amount, shape="Sine", sync=False, delay=0.0, retrigger=False):
    d = {"shape": V(shape), "to": V(to), "rate": float(rate), "amount": float(amount)}
    if sync:
        d["sync"] = True
    if delay:
        d["delay"] = float(delay)
    if retrigger:
        d["retrigger"] = True
    return d


def synth(oscs, f, amp, mod_env, lfos=None, glide=0.0, mono=False, voices=12, velocity=0.5, gain=1.0, punch=0.0, punch_time=0.0):
    d = {"oscs": oscs, "filter": f, "amp": amp, "mod_env": mod_env}
    if lfos:
        d["lfos"] = lfos
    if glide:
        d["glide"] = float(glide)
    if mono:
        d["mono"] = True
    d["voices"] = voices
    d["velocity"] = float(velocity)
    d["gain"] = float(gain)
    if punch:
        d["punch"] = float(punch)
        d["punch_time"] = float(punch_time)
    return Newtype("Synth", d)


def body(frm, to, sweep, decay, gain, overtone=0.0):
    d = {"from": float(frm), "to": float(to), "sweep": float(sweep), "decay": float(decay), "gain": float(gain)}
    if overtone:
        d["overtone"] = float(overtone)
    return d


def hiss(cutoff, decay, gain, mode="BandPass", resonance=0.0, attack=0.0, bursts=1, spread=0.0):
    d = {"mode": V(mode), "cutoff": float(cutoff), "resonance": float(resonance), "decay": float(decay), "gain": float(gain)}
    if attack:
        d["attack"] = float(attack)
    if bursts > 1:
        d["bursts"] = bursts
        d["spread"] = float(spread)
    return d


def ring(freq, decay, gain, highpass=6000.0):
    return {"freq": float(freq), "decay": float(decay), "gain": float(gain), "highpass": float(highpass)}


def drum(name, key, b=None, h=None, r=None, click=0.0, drive=0.0, gain=1.0, pan=0.0, choke=0, velocity=0.5, fixed=True):
    d = {"name": name, "key": key}
    if b:
        d["body"] = b
    if h:
        d["hiss"] = h
    if r:
        d["ring"] = r
    if click:
        d["click"] = float(click)
    if drive:
        d["drive"] = float(drive)
    d["gain"] = float(gain)
    if pan:
        d["pan"] = float(pan)
    if choke:
        d["choke"] = choke
    d["velocity"] = float(velocity)
    if not fixed:
        d["fixed"] = False
    return d


def kit(drums):
    return Newtype("Kit", {"drums": drums})


# Keys the kits use (General MIDI where it has one).
KICK, RIM, SNARE, CLAP, SNARE2 = 36, 37, 38, 39, 40
HAT, HAT_PEDAL, HAT_OPEN = 42, 44, 46
TOM_LO, TOM_MID, TOM_HI = 41, 45, 48
CRASH, RIDE = 49, 51
TAIKO, TAIKO_HI, BOOM, ANVIL, SHAKER = 35, 43, 34, 56, 70
TICK, CHIRP, BREATH, THUD, HEART = 75, 76, 77, 33, 32


LIB = {
    # Bass under everything: a sine an octave down with a little triangle for
    # presence on small speakers. Mono, a hair of glide.
    "sub_bass": synth(
        [osc("Sine", 0.9), osc("Triangle", 0.35, octave=1)],
        filt(420, "LowPass4", 0.05),
        env(0.01, 0.4, 0.9, 0.35),
        env(0.01, 0.3, 0.5, 0.3),
        glide=0.03,
        mono=True,
        velocity=0.3,
    ),
    # The driving 16th ostinato: saw + square an octave down through a
    # resonant 24 dB low-pass that snaps open on each note.
    "pulse_bass": synth(
        [osc("Saw", 0.7, unison=2, detune=6, width=0.2), osc("Square", 0.45, octave=-1)],
        filt(300, "LowPass4", 0.32, env_oct=3.0, keytrack=0.35, velocity=1.2, drive=0.25),
        env(0.002, 0.25, 0.65, 0.08),
        env(0.001, 0.16, 0.08, 0.1),
        mono=False,
        voices=4,
        velocity=0.6,
    ),
    # Strings: two supersaw layers, slow bow, delayed vibrato.
    "wide_strings": synth(
        [osc("Saw", 0.55, unison=7, detune=16, width=0.9), osc("Saw", 0.22, octave=1, unison=5, detune=11, width=0.7)],
        filt(1900, "LowPass", 0.12, env_oct=0.7, keytrack=0.5, velocity=0.6),
        env(0.45, 1.2, 0.85, 1.4),
        env(0.6, 1.5, 0.6, 1.2),
        lfos=[lfo("Pitch", 5.1, 0.09, delay=0.6), lfo("Cutoff", 0.11, 0.25)],
        voices=16,
        velocity=0.45,
        gain=0.9,
    ),
    # Brass: detuned saw stack with a filter that swells and a touch of drive.
    "brass": synth(
        [osc("Saw", 0.6, unison=3, detune=9, width=0.5), osc("Saw", 0.35, octave=-1, unison=2, detune=5), osc("Pulse", 0.2, shape=0.3)],
        filt(520, "LowPass", 0.18, env_oct=2.6, keytrack=0.6, velocity=1.4, drive=0.35),
        env(0.035, 0.5, 0.75, 0.3),
        env(0.06, 0.45, 0.35, 0.3),
        lfos=[lfo("Pitch", 5.4, 0.06, delay=0.35)],
        voices=10,
        velocity=0.7,
        gain=0.95,
    ),
    # Bright stabs for off-beats: short, fat, filtered hard.
    "stab": synth(
        [osc("Saw", 0.6, unison=5, detune=14, width=0.8), osc("Square", 0.3, octave=-1)],
        filt(700, "LowPass", 0.25, env_oct=3.2, keytrack=0.4, velocity=1.0, drive=0.2),
        env(0.002, 0.22, 0.0, 0.18),
        env(0.001, 0.14, 0.0, 0.15),
        voices=12,
        velocity=0.6,
    ),
    # The Seed motif: an FM bell, inharmonic (3.5), bright at the strike and mellowing.
    "seed_bell": synth(
        [osc("Fm", 0.7, shape=0.32, ratio=3.5, retrigger=True), osc("Sine", 0.35, octave=1, retrigger=True)],
        filt(7000, "LowPass", 0.0, keytrack=0.3),
        env(0.001, 2.8, 0.0, 2.4),
        env(0.001, 1.1, 0.05, 1.0),
        voices=10,
        velocity=0.6,
        gain=0.85,
    ),
    # A plucked arp: saw and pulse, the filter closing fast.
    "pluck": synth(
        [osc("Saw", 0.55, unison=2, detune=7, width=0.5), osc("Pulse", 0.35, shape=0.25, octave=1)],
        filt(650, "LowPass", 0.28, env_oct=3.4, keytrack=0.6, velocity=0.8),
        env(0.001, 0.32, 0.0, 0.25),
        env(0.001, 0.18, 0.0, 0.2),
        voices=10,
        velocity=0.6,
    ),
    # A dark drone: two low saws beating slowly, a folded sine, a filter that breathes.
    "drone": synth(
        [osc("Saw", 0.55, unison=3, detune=7, width=0.6), osc("Fold", 0.4, shape=0.35, octave=-1)],
        filt(260, "LowPass4", 0.35, keytrack=0.2),
        env(2.5, 2.0, 1.0, 3.0),
        env(3.0, 2.0, 0.7, 2.0),
        lfos=[lfo("Cutoff", 0.06, 1.1), lfo("Pan", 0.09, 0.35, shape="Triangle")],
        voices=6,
        velocity=0.2,
        gain=0.9,
    ),
    # A breathy choir-like pad: triangles in unison with a little noise, through a band of low-pass.
    "breath_pad": synth(
        [osc("Triangle", 0.6, unison=5, detune=12, width=0.9), osc("Noise", 0.045), osc("Sine", 0.25, octave=1, unison=3, detune=6, width=0.6)],
        filt(1500, "LowPass", 0.2, env_oct=0.5, keytrack=0.6),
        env(1.2, 1.5, 0.85, 2.2),
        env(1.5, 2.0, 0.6, 2.0),
        lfos=[lfo("Cutoff", 0.17, 0.35, shape="Triangle"), lfo("Amp", 4.5, 0.06, delay=1.0)],
        voices=14,
        velocity=0.3,
        gain=1.1,
    ),
    # A lead that sings: square and saw, glide, delayed vibrato.
    "lead": synth(
        [osc("Square", 0.45, unison=2, detune=5, width=0.3), osc("Saw", 0.4, fine=6)],
        filt(2400, "LowPass", 0.22, env_oct=1.2, keytrack=0.5, velocity=0.8, drive=0.15),
        env(0.02, 0.6, 0.8, 0.35),
        env(0.02, 0.5, 0.5, 0.3),
        lfos=[lfo("Pitch", 5.6, 0.14, delay=0.45)],
        glide=0.06,
        mono=True,
        velocity=0.5,
        gain=0.85,
    ),
    # Regency: a folded, FM-growling bass whose filter chatters in time.
    "growl": synth(
        [osc("Fold", 0.6, shape=0.55), osc("Fm", 0.45, shape=0.35, ratio=0.5, octave=-1)],
        filt(420, "LowPass4", 0.45, env_oct=2.2, keytrack=0.3, velocity=1.0, drive=0.5),
        env(0.004, 0.3, 0.7, 0.12),
        env(0.002, 0.22, 0.2, 0.12),
        lfos=[lfo("Cutoff", 2.0, 0.9, shape="Triangle", sync=True, retrigger=True)],
        voices=4,
        velocity=0.6,
    ),
    # Regency: glass. FM at an irrational ratio, soft strike, long ring.
    "glass": synth(
        [osc("Fm", 0.6, shape=0.22, ratio=1.41, retrigger=True), osc("Sine", 0.3, octave=2, fine=-8)],
        filt(5200, "LowPass", 0.1, keytrack=0.4),
        env(0.004, 3.5, 0.0, 3.0),
        env(0.004, 2.0, 0.1, 2.0),
        lfos=[lfo("Pitch", 0.3, 0.05, shape="Triangle")],
        voices=12,
        velocity=0.5,
        gain=0.8,
    ),
    # Regency: a wailing lead, a folded sine bending under slow vibrato.
    "wail": synth(
        [osc("Fold", 0.55, shape=0.3), osc("Saw", 0.25, unison=3, detune=10, width=0.6, octave=1)],
        filt(1800, "BandPass", 0.35, env_oct=1.0, keytrack=0.7),
        env(0.12, 0.8, 0.8, 0.9),
        env(0.2, 1.0, 0.5, 0.8),
        lfos=[lfo("Pitch", 4.2, 0.22, delay=0.3), lfo("Shape", 0.3, 0.4)],
        glide=0.12,
        mono=True,
        velocity=0.4,
        gain=1.0,
    ),
    # Survival: the Progenitor's sequencer. Short FM ticks with a metallic ratio.
    "machine_seq": synth(
        [osc("Fm", 0.6, shape=0.45, ratio=2.01, retrigger=True), osc("Pulse", 0.3, shape=0.15)],
        filt(1200, "LowPass", 0.4, env_oct=2.4, keytrack=0.5, velocity=1.0),
        env(0.001, 0.14, 0.0, 0.1),
        env(0.001, 0.09, 0.0, 0.08),
        voices=8,
        velocity=0.7,
        gain=0.9,
    ),
    # Survival: a horn for the waves, a big low saw stack that opens slowly.
    "horn": synth(
        [osc("Saw", 0.6, unison=5, detune=12, width=0.6, octave=-1), osc("Square", 0.35, octave=-2), osc("Saw", 0.3, unison=3, detune=8)],
        filt(260, "LowPass4", 0.25, env_oct=2.4, keytrack=0.4, velocity=1.0, drive=0.45),
        env(0.12, 1.2, 0.8, 1.2),
        env(0.35, 1.2, 0.5, 1.0),
        lfos=[lfo("Pitch", 4.8, 0.05, delay=0.5)],
        voices=8,
        velocity=0.5,
        gain=1.0,
    ),
}

# ARC: a war kit: a deep kick, a tight snare, taiko for the big beats, toms, cymbals.
LIB["war_kit"] = kit(
    [
        drum("Boom", BOOM, b=body(95, 32, 0.09, 1.4, 1.0), h=hiss(180, 0.5, 0.25, "LowPass"), drive=0.25, gain=1.0),
        drum("Taiko", TAIKO, b=body(120, 62, 0.05, 0.55, 1.0, overtone=1.58), h=hiss(900, 0.08, 0.25, "LowPass", 0.1), click=0.35, drive=0.3, gain=0.8),
        drum("Kick", KICK, b=body(150, 46, 0.03, 0.34, 1.0), click=0.35, drive=0.25, gain=0.95),
        drum("Rim", RIM, b=body(1700, 1500, 0.005, 0.02, 0.35), h=hiss(3800, 0.025, 0.45, "BandPass", 0.5), gain=0.55, pan=0.15),
        drum("Snare", SNARE, b=body(235, 175, 0.02, 0.1, 0.55, overtone=1.7), h=hiss(3200, 0.17, 1.0, "BandPass", 0.15), click=0.2, drive=0.15, gain=1.1, velocity=0.6),
        drum("Clap", CLAP, h=hiss(1700, 0.13, 0.9, "BandPass", 0.3, bursts=3, spread=0.011), gain=0.65),
        drum("Snare roll", SNARE2, h=hiss(4200, 0.08, 0.7, "BandPass", 0.1), b=body(260, 220, 0.01, 0.05, 0.3), gain=0.55, velocity=0.8),
        drum("Tom low", TOM_LO, b=body(130, 82, 0.06, 0.4, 1.0, overtone=1.5), click=0.2, gain=0.8, pan=-0.25, fixed=False),
        drum("Tom mid", TOM_MID, b=body(175, 110, 0.05, 0.33, 1.0, overtone=1.5), click=0.2, gain=0.75, pan=0.0, fixed=False),
        drum("Tom high", TOM_HI, b=body(230, 150, 0.04, 0.28, 1.0, overtone=1.5), click=0.2, gain=0.7, pan=0.25, fixed=False),
        drum("Taiko high", TAIKO_HI, b=body(210, 120, 0.03, 0.3, 0.9, overtone=1.62), click=0.3, drive=0.2, gain=0.75, pan=0.2),
        drum("Hat", HAT, r=ring(410, 0.045, 0.7, 7000), h=hiss(9000, 0.03, 0.35, "HighPass"), gain=0.9, pan=0.3, choke=1, velocity=0.7),
        drum("Hat pedal", HAT_PEDAL, r=ring(410, 0.03, 0.45, 7000), gain=0.35, pan=0.3, choke=1),
        drum("Hat open", HAT_OPEN, r=ring(410, 0.35, 0.6, 7000), h=hiss(8000, 0.3, 0.25, "HighPass"), gain=0.75, pan=0.3, choke=1),
        drum("Crash", CRASH, r=ring(330, 1.6, 0.5, 4500), h=hiss(7000, 1.3, 0.45, "HighPass"), gain=0.8, pan=-0.35),
        drum("Anvil", ANVIL, b=body(520, 510, 0.01, 0.7, 0.35, overtone=2.76), r=ring(700, 0.4, 0.35, 2500), click=0.4, gain=0.55, pan=-0.2),
        drum("Shaker", SHAKER, h=hiss(6500, 0.05, 0.6, "HighPass", attack=0.012), gain=0.7, pan=-0.3, velocity=0.8),
    ]
)

# Regency: a hive. A soft heartbeat, clicks and chirps like insects, breath swells, a low thud.
LIB["hive_kit"] = kit(
    [
        drum("Heart", HEART, b=body(70, 42, 0.04, 0.3, 1.0), h=hiss(140, 0.12, 0.3, "LowPass"), drive=0.15, gain=0.95, velocity=0.6),
        drum("Thud", THUD, b=body(85, 36, 0.06, 0.7, 1.0, overtone=1.33), h=hiss(300, 0.2, 0.3, "LowPass", 0.2), drive=0.35, gain=1.0),
        drum("Kick", KICK, b=body(120, 44, 0.035, 0.32, 1.0), click=0.15, drive=0.35, gain=0.9),
        drum("Crack", SNARE, b=body(330, 240, 0.012, 0.06, 0.4), h=hiss(2400, 0.11, 0.85, "BandPass", 0.45), drive=0.3, gain=0.75),
        drum("Clap", CLAP, h=hiss(1300, 0.1, 0.85, "BandPass", 0.45, bursts=4, spread=0.008), gain=0.6),
        drum("Tick", TICK, h=hiss(5600, 0.012, 0.9, "BandPass", 0.8), gain=0.4, pan=0.35, velocity=0.8),
        drum("Chirp", CHIRP, b=body(3200, 1800, 0.01, 0.03, 0.4), h=hiss(4200, 0.02, 0.3, "BandPass", 0.7), gain=0.35, pan=-0.35),
        drum("Breath", BREATH, h=hiss(1100, 0.45, 0.7, "BandPass", 0.3, attack=0.35), gain=0.45, pan=-0.1, velocity=0.4),
        drum("Rattle", SHAKER, h=hiss(7200, 0.04, 0.5, "HighPass", attack=0.004, bursts=3, spread=0.018), gain=0.35, pan=0.25, velocity=0.8),
        drum("Metal", ANVIL, r=ring(560, 0.9, 0.4, 2200), b=body(460, 455, 0.01, 0.9, 0.2, overtone=2.41), gain=0.45, pan=0.3),
        drum("Low tom", TOM_LO, b=body(110, 70, 0.06, 0.45, 1.0, overtone=1.4), click=0.1, drive=0.2, gain=0.75, pan=-0.2, fixed=False),
        drum("Swell", CRASH, r=ring(290, 2.2, 0.3, 3000), h=hiss(5000, 2.0, 0.35, "HighPass", attack=0.9), gain=0.5),
    ]
)

# Survival: the machine. Pistons, sparks, a press, a hiss of steam.
LIB["machine_kit"] = kit(
    [
        drum("Press", BOOM, b=body(80, 30, 0.07, 1.0, 1.0), h=hiss(220, 0.3, 0.35, "LowPass", 0.2), drive=0.35, gain=1.0),
        drum("Kick", KICK, b=body(160, 48, 0.028, 0.3, 1.0), click=0.45, drive=0.3, gain=0.95),
        drum("Piston", SNARE, b=body(200, 150, 0.015, 0.08, 0.5, overtone=2.1), h=hiss(2600, 0.12, 0.8, "BandPass", 0.35), r=ring(600, 0.06, 0.2, 3000), drive=0.25, gain=0.8),
        drum("Clank", ANVIL, b=body(780, 770, 0.01, 0.35, 0.3, overtone=2.76), r=ring(820, 0.25, 0.35, 2500), click=0.5, gain=0.5, pan=0.2),
        drum("Spark", HAT, h=hiss(8500, 0.02, 0.5, "HighPass", 0.2), r=ring(520, 0.02, 0.3, 8000), gain=0.4, pan=-0.3, choke=1),
        drum("Steam", HAT_OPEN, h=hiss(6000, 0.4, 0.45, "HighPass", attack=0.03), gain=0.4, pan=-0.3, choke=1),
        drum("Gear", TICK, b=body(1400, 1300, 0.005, 0.02, 0.4), h=hiss(3000, 0.02, 0.4, "BandPass", 0.6), gain=0.4, pan=0.35),
        drum("Tom low", TOM_LO, b=body(120, 76, 0.06, 0.42, 1.0, overtone=1.5), click=0.25, drive=0.2, gain=0.8, pan=-0.2, fixed=False),
        drum("Tom high", TOM_HI, b=body(200, 130, 0.04, 0.3, 1.0, overtone=1.5), click=0.25, drive=0.2, gain=0.7, pan=0.2, fixed=False),
        drum("Crash", CRASH, r=ring(310, 1.8, 0.45, 4200), h=hiss(6500, 1.4, 0.35, "HighPass"), gain=0.5, pan=0.3),
    ]
)


def get(name):
    return LIB[name]
