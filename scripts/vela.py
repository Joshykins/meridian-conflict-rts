#!/usr/bin/env python3
"""Bake Vela and apply its complete editor recipe in a private staging folder."""
import argparse
from pathlib import Path
import os
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "maps" / "vela_m1"


def build(destination):
    destination = destination.resolve()
    if destination == ROOT or destination in ROOT.parents or destination == SOURCE.parent:
        raise ValueError("Choose a map folder, not the repository or maps directory")
    if destination != SOURCE and SOURCE in destination.parents:
        raise ValueError("The bake destination must be outside the source asset folder")
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = Path(tempfile.mkdtemp(prefix=".vela-build-", dir=destination.parent))
    staged = temporary / "map"
    backup = temporary / "previous"
    try:
        shutil.copytree(SOURCE, staged, ignore=shutil.ignore_patterns("terrain.mcmap", "placed.ron"))
        def run(*args):
            subprocess.run(args, cwd=ROOT, check=True)
        run("cargo", "run", "--profile", "gate", "-p", "mc-map", "--bin", "mc-bake", "--",
            "--layout", "vela", "--size-km", "12", "--seed", "5", "--name", "Vela",
            "-o", str(staged / "terrain.mcmap"))
        run("cargo", "run", "--profile", "gate", "-p", "mc-game", "--bin", "meridian", "--",
            "--editor", str(staged / "terrain.mcmap"), "--editor-apply", str(staged / "authoring.json"))
        existed = destination.exists()
        if existed:
            os.replace(destination, backup)
        try:
            os.replace(staged, destination)
        except OSError:
            if existed:
                os.replace(backup, destination)
            raise
        print(f"Vela ready: {destination}. Open The Contract or --editor vela_m1.")
    finally:
        # This invocation owns this uniquely-created staging directory only.
        # Keep a previous folder if recovery failed, so it can be recovered manually.
        if backup.exists() and not destination.exists():
            print(f"Previous map preserved at {backup}")
        else:
            shutil.rmtree(temporary)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=SOURCE, help="destination map folder")
    build(parser.parse_args().output)


if __name__ == "__main__":
    main()
