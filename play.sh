#!/usr/bin/env bash
# Builds and runs the game natively on Windows from a WSL checkout.
#
# WSL has no real Vulkan driver (only the CPU rasteriser), so the game is built
# with the Windows-side cargo and run on the Windows GPU. The build output goes
# to a Windows-local directory: it must not share target/ with the Linux build,
# and compiling onto the WSL filesystem from Windows is slow.
#
#   ./play.sh                          the front end (main menu)
#   ./play.sh --map dev16              straight into a skirmish against the AI
#   ./play.sh --map meridian_basin --players 8
#   ./play.sh --scene battle
#   ./play.sh --build-only
set -euo pipefail
cd "$(dirname "$0")"

repo=$(wslpath -w "$PWD")
build_only=false
args=()
for a in "$@"; do
    if [ "$a" = "--build-only" ]; then build_only=true; else args+=("'${a//\'/\'\'}'"); fi
done

run='& "$env:CARGO_TARGET_DIR\release\meridian.exe" '"${args[*]:-}"
$build_only && run='Write-Host "built $env:CARGO_TARGET_DIR\release\meridian.exe"'

powershell.exe -NoProfile -Command "
    \$env:CARGO_TARGET_DIR = \"\$env:LOCALAPPDATA\\meridian-conflict-target\"
    Set-Location '$repo'
    cargo build --release -p mc-game
    if (\$LASTEXITCODE -ne 0) { exit \$LASTEXITCODE }
    $run
"
