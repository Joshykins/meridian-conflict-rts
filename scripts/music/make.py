"""Writes the first drafts of the score into data/music.

    python3 scripts/music/make.py [--force] [song ...]

ONE-SHOT: these scripts composed the starting songs. Once written, the RON in
data/music is the source: it is edited in mc-studio or by hand, and running
this again with --force throws those edits away. Existing files are skipped
without --force. Kept for new songs to borrow the helpers from.
"""

import importlib
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))

from compose import FORCE, write  # noqa: E402
from instruments import LIB  # noqa: E402
from ron import dump  # noqa: E402

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT = os.path.join(ROOT, "data", "music")
# One song for now, iterated with the user in the studio before any others are written.
SONGS = {
    "reach_command": "song_reach_command",
}


def main():
    wanted = [a for a in sys.argv[1:] if not a.startswith("--")] or list(SONGS)
    os.makedirs(os.path.join(OUT, "instruments"), exist_ok=True)
    for name, inst in LIB.items():
        write(os.path.join(OUT, "instruments", f"{name}.ron"), inst, force=FORCE)
    for name in wanted:
        mod = importlib.import_module(SONGS[name])
        write(os.path.join(OUT, f"{name}.ron"), mod.SONG, force=FORCE)
    score = os.path.join(OUT, "score.ron")
    if FORCE or not os.path.exists(score):
        with open(score, "w") as f:
            f.write(
                "// Which song plays where. Songs are data/music/<name>.ron.\n"
                "// battle is keyed by the player's faction key; `default` for any other.\n"
                "// Cues the game sends (a song answers those it has a section for):\n"
                "// nuke, commander_lost, enemy_commander, wave, titan (stingers); victory, defeat (endings).\n"
                "(\n"
                '    menu: "reach_command",\n'
                '    battle: {"default": "reach_command"},\n'
                '    survival: "reach_command",\n'
                "    fade: 3.0,\n"
                ")\n"
            )
        print(f"wrote {score}")


if __name__ == "__main__":
    main()
