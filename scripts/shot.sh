#!/usr/bin/env bash
# Build the game on the Windows side (the real GPU) and take a headless shot,
# from WSL, in one command. The picture lands in artifacts/shots/ here.
#
#   scripts/shot.sh unit KEY [flags]   a unit alone, several angles in one PNG
#                                      (meridian --unit-shot; see its --help)
#   scripts/shot.sh run [flags]        any other headless shot (--range, --scene,
#                                      --ui, ...); --screenshot is added for you
#   scripts/shot.sh stop               stop this checkout's shot server
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
# Unit shots go to a shot server (meridian --shot-server) that keeps a warm
# renderer between shots, so a shot costs its views (~1-3 s), not the ~10 s
# start-up. It is restarted when the build changes, told to re-read data/ when
# data/ changes and to recompile the shaders when a .wgsl changes (no cargo build
# for either), and leaves by itself after 30 idle minutes.
#
# Every session shares one incremental build (the `shot` profile, in
# %TEMP%\meridian-target-shot-<checkout>), and nothing is built when no source
# file changed since the last build. The build runs under a lock and runs from
# copies of the exe, so sessions never lock each other's binary. A worktree gets
# its own target dir and server; copy the .mcmap it needs into its maps/
# (symlinks do not resolve from Windows).
set -euo pipefail

repo=$(git rev-parse --show-toplevel)
checkout=$(basename "$repo")
out_name=""
build=1
while [[ $# -gt 0 ]]; do
    case "$1" in
        -o) out_name="$2"; shift 2 ;;
        --no-build) build=0; shift ;;
        -h|--help) sed -n '2,33p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) break ;;
    esac
done
[[ $# -gt 0 ]] || { echo "usage: scripts/shot.sh [-o NAME] [--no-build] unit KEY [flags] | run [flags] | stop" >&2; exit 2; }
mode="$1"; shift

# Windows' temp dir, asked once (each powershell.exe start costs ~0.5 s).
temp_cache="/tmp/meridian-shot-wintemp"
[[ -s $temp_cache ]] || wslpath "$(powershell.exe -NoProfile -Command '[IO.Path]::GetTempPath()' | tr -d '\r')" > "$temp_cache"
win_temp=$(cat "$temp_cache")
target_dir="$win_temp/meridian-target-shot-$checkout"
target_win=$(wslpath -w "$target_dir")
bin_dir="$win_temp/meridian-shot-bin"
shots_win="$win_temp/meridian-shots"
server_dir="$win_temp/meridian-shot-server-$checkout"
mkdir -p "$bin_dir" "$shots_win" "$server_dir" "$repo/artifacts/shots"
repo_win=$(wslpath -w "$repo")
built="$target_dir/shot/meridian.exe"
lock="/tmp/meridian-shot-build-$checkout.lock"
built_stamp="/tmp/meridian-shot-built-$checkout"
data_stamp="/tmp/meridian-shot-data-$checkout"
shader_stamp="/tmp/meridian-shot-shaders-$checkout"
shaders="$repo/crates/mc-render/shaders"

server_alive() {
    local beat
    beat=$(stat -c %Y "$server_dir/alive" 2>/dev/null) || return 1
    (( $(date +%s) - beat < 4 ))
}

# Sends one request (the arguments, one per line) and prints the answer.
ask() {
    local id req done
    id="$(date +%s%N)-$$"
    req="$server_dir/$id"
    printf '%s\n' "$@" > "$req.tmp"
    mv "$req.tmp" "$req.req"
    done="$req.done"
    # A restart by another session leaves a gap of a second or two with no beat;
    # the new server takes over the waiting requests.
    local seen=$SECONDS
    while [[ ! -f $done ]]; do
        server_alive && seen=$SECONDS
        if (( SECONDS - seen > 10 )); then
            echo "shot.sh: the shot server died; its log:" >&2
            tr -d '\r' < "$server_dir/server.log" | tail -30 >&2
            rm -f "$req.req"
            return 1
        fi
        sleep 0.05
    done
    local said
    said=$(tr -d '\r' < "$done")
    rm -f "$done"
    echo "${said#*$'\n'}"
    [[ ${said%%$'\n'*} == ok ]]
}

stop_server() {
    if server_alive; then
        ask --quit > /dev/null || true
        for _ in $(seq 40); do server_alive || break; sleep 0.1; done
    fi
}

if [[ $mode == stop ]]; then
    stop_server
    exit 0
fi

case "$mode" in
    unit)
        [[ $# -gt 0 ]] || { echo "shot.sh unit needs a blueprint key" >&2; exit 2; }
        key="$1"; shift
        game_args=(--unit-shot "$key" "$@")
        stem="${out_name:-$key}" ;;
    run)
        game_args=("$@")
        stem="${out_name:-shot}" ;;
    *) echo "shot.sh: unknown mode $mode (unit, run or stop)" >&2; exit 2 ;;
esac
stem="${stem%.png}-$(date +%H%M%S)"

t0=$(date +%s%N)
[[ -f $built ]] || build=1
# Build only when a source file is newer than the last build. The stamp is
# taken before cargo starts, so an edit made during the build is seen next time.
[[ $build == 0 ]] || (
    flock -w 1800 9 || { echo "shot.sh: timed out waiting for another session's build" >&2; exit 1; }
    changed=""
    if [[ $build == 1 ]]; then
        if [[ ! -f $built_stamp || ! -f $built ]]; then
            changed=yes
        else
            # A unit shot's server recompiles edited shaders itself (--shaders).
            skip=()
            [[ $mode == unit ]] && skip=(-not -path "$shaders/*")
            changed=$(find "$repo/crates" "$repo/Cargo.toml" "$repo/Cargo.lock" -newer "$built_stamp" -type f \
                -not -path '*/target/*' "${skip[@]}" -print -quit)
        fi
    fi
    if [[ -n $changed ]]; then
        touch "$built_stamp.next"
        log="$bin_dir/build-$$.log"
        if ! powershell.exe -NoProfile -Command "\$env:CARGO_TARGET_DIR='$target_win'; Set-Location '$repo_win'; cargo build --profile shot -p mc-game *> '$(wslpath -w "$log")'; exit \$LASTEXITCODE"; then
            tr -d '\r' < "$log" | grep -v '^\s*Compiling' | tail -60 >&2
            rm -f "$log" "$built_stamp.next"
            echo "shot.sh: build failed" >&2
            exit 1
        fi
        tr -d '\r' < "$log" | grep -E '^(warning|error)' | sort -u | head -5 >&2 || true
        rm -f "$log"
        mv "$built_stamp.next" "$built_stamp"
    fi
    [[ -f $built ]] || { echo "shot.sh: no build yet at $built (drop --no-build)" >&2; exit 1; }
) 9>"$lock"
t1=$(date +%s%N)

out_win="$(wslpath -w "$shots_win")\\$stem.png"
if [[ $mode == unit ]]; then
    # The server runs a copy of the build it was started from; a new build replaces it.
    exe_id=$(stat -c '%Y-%s' "$built")
    # One session at a time checks and (re)starts the server; waiting requests
    # stay queued across a restart and the new server answers them.
    exec 8>"$lock.server"
    flock -w 120 8
    if ! server_alive || [[ $(cat "$server_dir/exe" 2>/dev/null) != "$exe_id" ]]; then
        stop_server
        # Always the same path: the GPU driver keeps its compiled pipelines per
        # program, and a new name each time would compile them all again (~15 s).
        # Under the build lock, so another session's link cannot be caught half
        # done; retried while Windows still holds the old server's file.
        exe="$server_dir/meridian-server.exe"
        (
            flock -w 1800 9
            for _ in $(seq 50); do cp -p "$built" "$exe" 2>/dev/null && exit 0; sleep 0.2; done
            exit 1
        ) 9>"$lock" || { echo "shot.sh: could not replace $exe" >&2; exit 1; }
        echo "$exe_id" > "$server_dir/exe"
        # Started through a launcher file (no redirection on Start-Process, which would
        # hand the server this pipe and keep the call from returning). pushd maps the
        # \\wsl.localhost repo to a drive, since cmd cannot start in a UNC directory.
        # Backgrounded too, so a launcher that lingers cannot hold anyone up.
        server_win=$(wslpath -w "$server_dir")
        printf '@echo off\r\nset RUST_LOG=info,mc_render::renderer=debug,mc_render::warm=debug\r\npushd "%s"\r\n"%s" --shot-server "%s" 2> "%s\\server.log"\r\n' \
            "$repo_win" "$(wslpath -w "$exe")" "$server_win" "$server_win" > "$server_dir/start.cmd"
        # The lock descriptors are closed for it: a launcher holding the server lock
        # would block every session's shots.
        powershell.exe -NoProfile -Command "Start-Process -WindowStyle Hidden -FilePath '$server_win\\start.cmd'" \
            < /dev/null > /dev/null 2>&1 8>&- 9>&- &
        for _ in $(seq 100); do server_alive && break; sleep 0.1; done
        server_alive || { echo "shot.sh: the shot server did not start" >&2; tr -d '\r' < "$server_dir/server.log" | tail -20 >&2; exit 1; }
        touch "$data_stamp"
        # The new server has the shaders its build compiled.
        touch -r "$built_stamp" "$shader_stamp"
    fi
    flock -u 8
    reload=()
    if [[ -n $(find "$repo/data" -newer "$data_stamp" -type f -print -quit) ]]; then
        reload=(--reload)
        touch "$data_stamp"
    fi
    if [[ -n $(find "$shaders" -newer "$shader_stamp" -type f -print -quit) ]]; then
        reload+=(--shaders)
        touch "$shader_stamp"
    fi
    ask "${reload[@]}" "${game_args[@]}" --screenshot "$out_win"
else
    # A few fixed exe paths, one per run at a time: the GPU driver keeps compiled
    # pipelines per program path, so a slot reused shot after shot starts warm.
    for slot in 0 1 2 3 4 5 6 7 wait; do
        [[ $slot == wait ]] && { slot=0; exec 7>"$bin_dir/run-$checkout-0.lock"; flock 7; break; }
        exec 7>"$bin_dir/run-$checkout-$slot.lock"
        flock -n 7 && break
    done
    exe="$bin_dir/meridian-$checkout-run$slot.exe"
    exe_id=$(stat -c '%Y-%s' "$built")
    if [[ $(cat "$exe.id" 2>/dev/null) != "$exe_id" ]]; then
        (flock -w 1800 9; cp -p "$built" "$exe") 9>"$lock"
        echo "$exe_id" > "$exe.id"
    fi
    # PowerShell's own quoting: each argument in single quotes, any ' doubled.
    ps_args=""
    for a in "${game_args[@]}" --screenshot "$out_win"; do
        ps_args+=" '${a//\'/\'\'}'"
    done
    powershell.exe -NoProfile -Command "Set-Location '$repo_win'; & '$(wslpath -w "$exe")'$ps_args 2>&1 | ForEach-Object { \"\$_\" } | Where-Object { \$_ -notmatch '^\s+\S+\s+[0-9.]+ ms/tick' }; exit \$LASTEXITCODE" | tr -d '\r'
fi
t2=$(date +%s%N)

[[ -f "$shots_win/$stem.png" ]] || { echo "shot.sh: the game wrote no picture" >&2; exit 1; }
cp "$shots_win/$stem.png" "$repo/artifacts/shots/"
echo "build $(( (t1 - t0) / 1000000 )) ms, shot $(( (t2 - t1) / 1000000 )) ms"
echo "$repo/artifacts/shots/$stem.png"
