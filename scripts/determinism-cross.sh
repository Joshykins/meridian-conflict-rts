#!/usr/bin/env bash
# Runs the determinism matrix, and a match played by the planning AI
# (battle::a_commander_ai...), on Linux (WSL) and on Windows and compares the final
# hashes. Lockstep peers on the two platforms must agree bit for bit; the tests
# alone only prove that each build agrees with itself.
#
#   scripts/determinism-cross.sh        (from WSL; the Windows side builds in Temp)
set -euo pipefail
cd "$(dirname "$0")/.."

# PowerShell can wrap a native command's line as an error record ("cargo : determinism: ..."),
# so match the hash line anywhere, and never fail the pipe when there is none.
filter() { { grep -aoE 'determinism: [a-z_]+ final [0-9a-f]+' || true; } | sort; }

# A failing build or test shows as missing hashes below, not as a silent exit here.
linux=$(cargo test -q --release -p mc-sim --test sim -- determinism:: battle::a_commander_ai --nocapture 2>&1 | filter) || true
repo_win=$(wslpath -w "$PWD")
windows=$(powershell.exe -NoProfile -Command "
    \$env:CARGO_TARGET_DIR = \"\$env:TEMP\\meridian-target-determinism\"
    Set-Location '$repo_win'
    cargo test -q --release -p mc-sim --test sim -- determinism:: battle::a_commander_ai --nocapture 2>&1 | Out-String
" | filter) || true  # PowerShell exits 1 whenever cargo wrote to stderr

echo "linux:   ${linux:-<no hashes; did the tests fail?>}"
echo "windows: ${windows:-<no hashes; did the tests fail?>}"
[ -n "$linux" ] && [ "$linux" = "$windows" ] && echo "same on both" && exit 0
echo "DIFFERENT: the platforms would desync" >&2
exit 1
