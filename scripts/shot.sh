#!/usr/bin/env bash
# Build the game on the Windows side (the real GPU) and take a headless shot,
# from WSL, in one command. The picture lands in artifacts/shots/ here.
#
#   scripts/shot.sh unit KEY [flags]   a unit alone, several angles in one PNG
#                                      (meridian --unit-shot; see its --help)
#   scripts/shot.sh run [flags]        any other headless shot (--range, --scene,
#                                      --ui, ...); --screenshot is added for you
#
# Options before the subcommand:
#   -o NAME      output file name (default: KEY or "shot", plus the time)
#   --no-build   use the last build as it is
#
# Examples:
#   scripts/shot.sh unit aster_t1_tank
#   scripts/shot.sh unit aster_t1_tank --views front34 --look 1,0,2.5 --zoom 3
#   scripts/shot.sh unit aster_t1_tank --scenario march --ticks 40 --frames 60 --turn 90
#   scripts/shot.sh run --range --unit aster_commander --scenario work --ticks 120 --follow 5
#
# Every session shares one incremental build (the `shot` profile, in
# %TEMP%\meridian-target-shot-<checkout>): an edit to one model rebuilds in
# seconds to a minute, where a fresh per-session target dir takes 3-4 minutes.
# The build runs under a lock and each run gets its own copy of the exe, so
# sessions never lock each other's binary. A worktree gets its own target dir;
# copy the .mcmap it needs into its maps/ (symlinks do not resolve from Windows).
set -euo pipefail

repo=$(git rev-parse --show-toplevel)
checkout=$(basename "$repo")
out_name=""
build=1
while [[ $# -gt 0 ]]; do
    case "$1" in
        -o) out_name="$2"; shift 2 ;;
        --no-build) build=0; shift ;;
        -h|--help) sed -n '2,27p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) break ;;
    esac
done
[[ $# -gt 0 ]] || { echo "usage: scripts/shot.sh [-o NAME] [--no-build] unit KEY [flags] | run [flags]" >&2; exit 2; }
mode="$1"; shift
case "$mode" in
    unit)
        [[ $# -gt 0 ]] || { echo "shot.sh unit needs a blueprint key" >&2; exit 2; }
        key="$1"; shift
        game_args=(--unit-shot "$key" "$@")
        stem="${out_name:-$key}" ;;
    run)
        game_args=("$@")
        stem="${out_name:-shot}" ;;
    *) echo "shot.sh: unknown mode $mode (unit or run)" >&2; exit 2 ;;
esac
stem="${stem%.png}-$(date +%H%M%S)"

win_temp=$(wslpath "$(powershell.exe -NoProfile -Command '[IO.Path]::GetTempPath()' | tr -d '\r')")
target_win="$(wslpath -w "$win_temp")meridian-target-shot-$checkout"
bin_dir="$win_temp/meridian-shot-bin"
shots_win="$win_temp/meridian-shots"
mkdir -p "$bin_dir" "$shots_win" "$repo/artifacts/shots"
repo_win=$(wslpath -w "$repo")
exe="$bin_dir/meridian-$checkout-$$.exe"
trap 'rm -f "$exe"' EXIT

t0=$(date +%s)
# One build at a time per checkout; the copy is taken before the lock drops,
# so the next session's build cannot swap the exe mid-copy.
(
    flock -w 1800 9 || { echo "shot.sh: timed out waiting for another session's build" >&2; exit 1; }
    if [[ $build == 1 ]]; then
        log="$bin_dir/build-$$.log"
        if ! powershell.exe -NoProfile -Command "\$env:CARGO_TARGET_DIR='$target_win'; Set-Location '$repo_win'; cargo build --profile shot -p mc-game *> '$(wslpath -w "$bin_dir")\\build-$$.log'; exit \$LASTEXITCODE"; then
            tr -d '\r' < "$log" | grep -v '^\s*Compiling' | tail -60 >&2
            rm -f "$log"
            echo "shot.sh: build failed" >&2
            exit 1
        fi
        tr -d '\r' < "$log" | grep -E '^(warning|error)' | sort -u | head -5 >&2 || true
        rm -f "$log"
    fi
    built="$(wslpath "$target_win")/shot/meridian.exe"
    [[ -f "$built" ]] || { echo "shot.sh: no build yet at $built (drop --no-build)" >&2; exit 1; }
    cp "$built" "$exe"
) 9>"/tmp/meridian-shot-build-$checkout.lock"
t1=$(date +%s)

out_win="$(wslpath -w "$shots_win")\\$stem.png"
# PowerShell's own quoting: each argument in single quotes, any ' doubled.
ps_args=""
for a in "${game_args[@]}" --screenshot "$out_win"; do
    ps_args+=" '${a//\'/\'\'}'"
done
powershell.exe -NoProfile -Command "Set-Location '$repo_win'; & '$(wslpath -w "$exe")'$ps_args 2>&1 | ForEach-Object { \"\$_\" } | Where-Object { \$_ -notmatch '^\s+\S+\s+[0-9.]+ ms/tick' }; exit \$LASTEXITCODE" | tr -d '\r'
t2=$(date +%s)

[[ -f "$shots_win/$stem.png" ]] || { echo "shot.sh: the game wrote no picture" >&2; exit 1; }
cp "$shots_win/$stem.png" "$repo/artifacts/shots/"
echo "build $((t1 - t0)) s, shot $((t2 - t1)) s"
echo "$repo/artifacts/shots/$stem.png"
