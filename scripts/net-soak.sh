#!/usr/bin/env bash
# A long network match with no window: a relay, several headless clients that
# each run the whole simulation and fuzz orders (--bot chaos), and AI commanders
# in the other seats. One client hangs up part-way and rejoins from a snapshot.
# Fails on any desync, and when the clients' final hashes differ.
#
#   scripts/net-soak.sh [--map NAME] [--bots N] [--seats N] [--ticks N] [--seed N] [--windows]
#
# --windows makes the last client the Windows build (built into Temp), which is a
# cross-platform determinism check in a real match: AI, pathing, every system.
set -euo pipefail
cd "$(dirname "$0")/.."

map=haldens_grip bots=2 seats=6 ticks=6000 seed=7 windows=0 port=0
while [ $# -gt 0 ]; do
    case "$1" in
        --map) map=$2; shift 2 ;;
        --bots) bots=$2; shift 2 ;;
        --seats) seats=$2; shift 2 ;;
        --ticks) ticks=$2; shift 2 ;;
        --seed) seed=$2; shift 2 ;;
        --port) port=$2; shift 2 ;;
        --windows) windows=1; shift ;;
        *) echo "unknown option $1" >&2; exit 2 ;;
    esac
done
[ "$port" = 0 ] && port=$((17000 + RANDOM % 1000))

target=${CARGO_TARGET_DIR:-target}
cargo build -q --release -p mc-game -p mc-net
out=$(mktemp -d "${TMPDIR:-/tmp}/net-soak.XXXX")
echo "net-soak: $bots clients, $seats seats on $map, $ticks ticks; logs in $out"

if [ "$windows" = 1 ]; then
    repo_win=$(wslpath -w "$PWD")
    echo "net-soak: building the Windows client"
    powershell.exe -NoProfile -Command "
        \$env:CARGO_TARGET_DIR = \"\$env:TEMP\\meridian-target-soak\"
        Set-Location '$repo_win'
        cargo build -q --release -p mc-game 2>&1 | Out-String
    " > "$out/windows-build.log" || true  # PowerShell exits 1 whenever cargo wrote to stderr
    win_temp=$(powershell.exe -NoProfile -Command '[Console]::Write($env:TEMP)')
    win_exe="$(wslpath -u "$win_temp")/meridian-target-soak/release/meridian.exe"
    [ -x "$win_exe" ] || { echo "net-soak: no Windows build, see $out/windows-build.log" >&2; exit 1; }
    # Windows cannot follow the worktree's symlinked maps: hand it the real file.
    map_win=$(wslpath -w "$(readlink -f "maps/$map.mcmap")")
fi

"$target/release/mc-relay" --bind 0.0.0.0:$port --players "$bots" --auto-start \
    --replay-dir "$out" > "$out/relay.log" 2>&1 &
relay=$!
trap 'kill $relay 2>/dev/null || true' EXIT
sleep 0.5

pids=()
for i in $(seq 0 $((bots - 1))); do
    args=(--connect 127.0.0.1:$port --bot chaos --ticks "$ticks" --name "bot$i")
    # The first to join hosts: its seats, seed and AI define the match.
    [ "$i" = 0 ] && args+=(--players "$seats" --seed "$seed" --ai-difficulty hard --teams 2)
    # One client drops a third of the way in and comes back from a snapshot.
    [ "$i" = 1 ] && args+=(--drop-at $((ticks / 3)))
    if [ "$windows" = 1 ] && [ "$i" = $((bots - 1)) ]; then
        (cd "$(dirname "$win_exe")" && "$win_exe" --map "$map_win" "${args[@]}") > "$out/bot$i.log" 2>&1 &
    else
        "$target/release/meridian" --map "$map" "${args[@]}" > "$out/bot$i.log" 2>&1 &
    fi
    pids+=($!)
    sleep 0.3
done

failed=0
for i in "${!pids[@]}"; do
    code=0
    wait "${pids[$i]}" || code=$?
    case $code in
        0) ;;
        3) echo "net-soak: bot$i saw a DESYNC" >&2; failed=1 ;;
        *) echo "net-soak: bot$i failed (exit $code)" >&2; failed=1 ;;
    esac
done
grep -h "DESYNC\|net-bot:" "$out"/bot*.log | tr -d '\r' || true
hashes=$(grep -ahoE 'tick [0-9]+ hash [0-9a-f]+$' "$out"/bot*.log | tr -d '\r' | sort -u)
if [ "$(echo "$hashes" | grep -c .)" != 1 ]; then
    echo "net-soak: the clients ended on different hashes" >&2
    failed=1
fi
[ "$failed" = 0 ] && echo "net-soak: passed ($hashes)"
exit $failed
