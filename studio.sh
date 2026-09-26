#!/usr/bin/env bash
# Builds and runs mc-studio, the music workstation, natively on Windows from a
# WSL checkout (the same way play.sh runs the game: WSL has no real GPU or
# sound device). It builds into its own target directory, so a running game
# never locks the studio's build and the two never wait on each other.
#
#   ./studio.sh                        opens the studio on data/music
#   ./studio.sh reach_command          opens data/music/reach_command.ron
#   ./studio.sh path/to/song.ron
#   ./studio.sh --build-only
#
# Songs save as text into data/music; a running game (./play.sh) picks up a
# saved change to the song it is playing within about a second.
set -euo pipefail
cd "$(dirname "$0")"

repo=$(wslpath -w "$PWD")
build_only=false
args=()
for a in "$@"; do
    if [ "$a" = "--build-only" ]; then
        build_only=true
        continue
    fi
    # A bare song name means data/music/<name>.ron.
    if [[ "$a" != -* && "$a" != *.ron && -f "data/music/$a.ron" ]]; then
        a="data\\music\\$a.ron"
    fi
    args+=("'${a//\'/\'\'}'")
done

run='& "$env:CARGO_TARGET_DIR\release\mc-studio.exe" '"${args[*]:-}"
$build_only && run='Write-Host "built $env:CARGO_TARGET_DIR\release\mc-studio.exe"'

powershell.exe -NoProfile -Command "
    \$env:CARGO_TARGET_DIR = \"\$env:LOCALAPPDATA\\meridian-studio-target\"
    Set-Location '$repo'
    cargo build --release -p mc-studio
    if (\$LASTEXITCODE -ne 0) { exit \$LASTEXITCODE }
    $run
"
