#!/usr/bin/env bash
# Runs the determinism matrix on Linux (WSL) and on Windows and compares the final
# hashes. Lockstep peers on the two platforms must agree bit for bit; the tests
# alone only prove that each build agrees with itself.
#
#   scripts/determinism-cross.sh        (from WSL; the Windows side builds in Temp)
set -euo pipefail
cd "$(dirname "$0")/.."

filter() { grep -aE '^determinism: ' | tr -d '\r' | sort; }

linux=$(cargo test -q --release -p mc-sim --test determinism -- --nocapture 2>&1 | filter)
repo_win=$(wslpath -w "$PWD")
windows=$(powershell.exe -NoProfile -Command "
    \$env:CARGO_TARGET_DIR = \"\$env:TEMP\\meridian-target-determinism\"
    Set-Location '$repo_win'
    cargo test -q --release -p mc-sim --test determinism -- --nocapture 2>&1 | Out-String
" | filter)

echo "linux:   ${linux:-<no hashes; did the tests fail?>}"
echo "windows: ${windows:-<no hashes; did the tests fail?>}"
[ -n "$linux" ] && [ "$linux" = "$windows" ] && echo "same on both" && exit 0
echo "DIFFERENT: the platforms would desync" >&2
exit 1
