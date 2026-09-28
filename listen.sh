#!/usr/bin/env bash
# Breaks a piece of music apart so Claude can read it and the studio can play it.
#
#   ./listen.sh https://www.youtube.com/watch?v=...          fetch, split into stems, transcribe, chart
#   ./listen.sh https://youtu.be/... --from 1:32 --to 2:10   just that span (stems are still cached whole)
#   ./listen.sh data/music/references/track.mp3              a file you have
#   ./listen.sh score.mid | score.musicxml | score.mxl       sheet music: exact notes, no guessing
#   ./listen.sh ... --name winter_contingency                the folder name under data/music/references
#
# Results go to data/music/references/<name>/ (ignored by git): report.md (the
# chart), reference.ron (the transcription as a song for mc-studio), stems/,
# notes/*.mid, analysis.json. Runs in WSL on the CPU; tools live in .venv-listen
# (see scripts/listen/setup.sh). Demucs takes a minute or two per song.
set -euo pipefail
cd "$(dirname "$0")"

if [ ! -x .venv-listen/bin/python ]; then
    echo "no .venv-listen: run scripts/listen/setup.sh first" >&2
    exit 1
fi

# mc-music (the song checker/converter) in its own target dir, so this never waits on other builds.
export CARGO_TARGET_DIR="${MC_LISTEN_TARGET:-$HOME/.cache/meridian-listen-target}"
cargo build --release -q -p mc-music
export MC_MUSIC="$CARGO_TARGET_DIR/release/mc-music"

exec .venv-listen/bin/python scripts/listen/pipeline.py "$@"
