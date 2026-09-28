"""Mix set-up shared by the songs: return buses and the master chain."""

from ron import V


def hall(size=0.78, decay=3.2, predelay=24.0, damp=5200.0, lowcut=200.0, db=0.0):
    return {
        "name": "hall",
        "db": float(db),
        "effects": [V("Reverb", size=size, decay=decay, damp=damp, predelay=predelay, mix=1.0, lowcut=lowcut)],
    }


def echo(beats=0.75, feedback=0.38, tone=3800.0, db=0.0):
    return {
        "name": "echo",
        "db": float(db),
        "effects": [
            V("Delay", beats=beats, feedback=feedback, mix=1.0, pingpong=True, tone=tone),
            # The repeats get a little of the hall so they sit in the same room.
            V("Reverb", size=0.6, decay=1.8, damp=4500.0, predelay=10.0, mix=0.3, lowcut=250.0),
        ],
    }


def master(glue=True, ceiling=-1.0, db=0.0):
    fx = [V("Eq", low_db=0.0, low_hz=110.0, mid_db=-0.8, mid_hz=380.0, mid_q=0.9, high_db=1.2, high_hz=9000.0, cut_hz=24.0)]
    if glue:
        fx.append(V("Compressor", threshold=-14.0, ratio=2.2, attack=18.0, release=180.0, makeup=1.5))
    fx.append(V("Limiter", ceiling=ceiling, gain=0.0, release=90.0))
    return {"db": float(db), "effects": fx}


def duck(source, depth=-26.0, ratio=5.0, release=140.0):
    """A compressor keyed by `source` (the kick): the pad breathes with it."""
    return V("Compressor", threshold=depth, ratio=ratio, attack=1.5, release=release, makeup=0.0, sidechain=source)


def eq(low_db=0.0, low_hz=120.0, mid_db=0.0, mid_hz=1000.0, mid_q=0.8, high_db=0.0, high_hz=6000.0, cut_hz=0.0):
    return V("Eq", low_db=low_db, low_hz=low_hz, mid_db=mid_db, mid_hz=mid_hz, mid_q=mid_q, high_db=high_db, high_hz=high_hz, cut_hz=cut_hz)


# Colours for track kinds in the studio.
COL_DRUMS = 0xE0603A
COL_BASS = 0xC0452A
COL_PAD = 0x6A8CAF
COL_STRINGS = 0x8FA7C4
COL_BRASS = 0xE0A040
COL_LEAD = 0xF2F2F0
COL_BELL = 0x9FD4E0
COL_ARP = 0xB08CD8
COL_FX = 0x808080
