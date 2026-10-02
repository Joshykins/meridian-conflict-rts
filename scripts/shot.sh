#!/usr/bin/env bash
# Build the game on the Windows side (the real GPU) and take a headless shot,
# from WSL, in one command. The picture lands in artifacts/shots/ here.
#
#   scripts/shot.sh unit KEY [flags]   a unit alone, several angles in one PNG
#                                      (meridian --unit-shot; see its --help)
#   scripts/shot.sh run [flags]        any other headless shot (--range, --scene,
#                                      --ui, ...); --screenshot is added for you
#   scripts/shot.sh variants KEY MESH... [flags]
#   scripts/shot.sh variants KEY=MESH,MESH KEY=MESH,MESH... [flags]
#                                      design variants: one labelled sheet per
#                                      unit, one row per mesh key (e.g.
#                                      tank_light~slim; `base` is the unit's own
#                                      mesh), default views front34,left,rear34.
#                                      Many units at once cost one renderer
#                                      rebuild per round of variants, not one
#                                      per variant (10 units x 3 ~ 1 minute).
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
#   scripts/shot.sh variants aster_t1_tank base tank_light~slim tank_light~twin
#   scripts/shot.sh variants aster_t1_tank=base,tank_light~slim aster_t1_scout=base,scout~b
#   scripts/shot.sh variants aster_t1_frigate=base,frigate~b --map twin_shoals   (ships at sea)
#
# Design variants live in the model catalogue as extra keys, `<mesh>~<name>`,
# built like any model (ModelDef::new("tank_light~slim", ...)). Each is drawn as
# the unit, with the unit's own traits. Once one is chosen it becomes the mesh
# and the others are deleted (CLAUDE.md: no parked code).
#
# Unit shots go to a shot server (meridian --shot-server) that keeps a warm
# renderer between shots, so a shot costs its views (~1-3 s), not the ~10 s
# start-up. It is restarted when the build changes, told to re-read data/ when
# data/ changes and to recompile the shaders when a .wgsl changes, and handed
# freshly built meshes when a model file in crates/mc-models changes (built here
# at opt-level 0 in seconds; none of the three needs a build of the game). It
# leaves by itself after 30 idle minutes.
#
# Each checkout has its own incremental build (the `shot` profile, in
# %TEMP%\meridian-target-shot-<checkout>) and server, and nothing is built when
# no source file changed since the last build. Every build is also kept by what
# it was built from (%TEMP%\meridian-shot-exes\<engine hash>, the last dozen):
# a new worktree whose engine sources (all but the model code and shaders) match
# one takes it and builds nothing, and the model code and shaders reach its
# server as edits do. The build runs under a lock and servers run from copies of
# the exe, so sessions never lock each other's binary. A worktree without baked
# maps gets the main checkout's, hard-linked.
set -euo pipefail

repo=$(git rev-parse --show-toplevel)
checkout=$(basename "$repo")
out_name=""
build=1
while [[ $# -gt 0 ]]; do
    case "$1" in
        -o) out_name="$2"; shift 2 ;;
        --no-build) build=0; shift ;;
        -h|--help) sed -n '2,/^set -euo/p' "$0" | grep '^#' | sed 's/^# \{0,1\}//'; exit 0 ;;
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
# A worktree has no baked maps (they are not checked in): the main checkout's,
# hard-linked, since the game reads them through \\wsl.localhost, which does not
# follow symlinks.
if ! compgen -G "$repo/maps/*.mcmap" > /dev/null; then
    main=$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")
    for m in "$main"/maps/*.mcmap; do
        [[ -e $m ]] && { ln "$m" "$repo/maps/" 2>/dev/null || cp "$m" "$repo/maps/"; }
    done
fi
built="$target_dir/shot/meridian.exe"
lock="/tmp/meridian-shot-build-$checkout.lock"
built_stamp="/tmp/meridian-shot-built-$checkout"
# What the game in $target_dir was built from (tree_hash); `engine` only when
# the whole build came from one state of the sources.
built_engine="$target_dir/shot/engine"
built_looks="$target_dir/shot/looks"
# Games by the engine sources they were built from, for every checkout.
exes="$win_temp/meridian-shot-exes"
data_stamp="/tmp/meridian-shot-data-$checkout"
shader_stamp="/tmp/meridian-shot-shaders-$checkout"
shaders="$repo/crates/mc-render/shaders"
# Per unit key: when that unit's meshes were last rebuilt for the server.
model_stamps="/tmp/meridian-shot-models-$checkout"
models="$repo/crates/mc-models/src"

# A hash of the checkout's sources, by content: `engine` is everything a unit
# shot needs built into the game (crates/ but the model code and shaders, which
# reach a server without a build, and the textures the renderer embeds);
# `shaders` and `models` are those two, and `looks` both hashes, in that order.
tree_hash() {
    local list
    case $1 in
        engine) list=$(git -C "$repo" ls-files -co --exclude-standard -- crates Cargo.toml Cargo.lock \
                    rust-toolchain.toml data/textures/terrain data/textures/foliage \
                    | grep -v -e '^crates/mc-render/shaders/' -e '^crates/mc-models/src/'
                echo crates/mc-models/src/gpu_consts.rs) ;;
        looks) echo "$(tree_hash shaders) $(tree_hash models)"; return ;;
        shaders) list=$(git -C "$repo" ls-files -co --exclude-standard -- crates/mc-render/shaders) ;;
        models) list=$(git -C "$repo" ls-files -co --exclude-standard -- crates/mc-models/src \
                    | grep -v '/gpu_consts\.rs$') ;;
    esac
    list=$(sort -u <<< "$list" | while read -r f; do [[ -f $repo/$f ]] && echo "$f"; done)
    {
        paste -d' ' <(echo "$list") <(cd "$repo" && git hash-object --stdin-paths <<< "$list")
        # The game only makes meshes for the catalogue keys it was built with: a new
        # model key (not a `~` design variant of one) needs a build, though model code
        # does not otherwise count as engine.
        [[ $1 != engine ]] || find "$repo/crates/mc-models/src" -name '*.rs' -exec \
            perl -0777 -ne 'print "$1\n" while /ModelDef::(?:new|tiered)\(\s*"([a-z0-9_]+)"/g' {} + | sort -u
    } | sha1sum | cut -c1-16
}

# Overlays (Overwolf, Steam, Epic) hook every Vulkan program on this machine as
# implicit layers. A headless shot has no window for them, so every shot runs
# with them switched off and never depends on what they do.
no_overlays=(DISABLE_VULKAN_OW_OVERLAY_LAYER DISABLE_VULKAN_OW_OBS_CAPTURE
    DISABLE_VK_LAYER_VALVE_steam_overlay_1 DISABLE_VK_LAYER_VALVE_steam_fossilize_1
    EOS_OVERLAY_DISABLE_VULKAN_WIN64)
cmd_env=$(printf 'set %s=1\r\n' "${no_overlays[@]}")
ps_env=$(printf '$env:%s=1; ' "${no_overlays[@]}")

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
    local seen=$SECONDS asked=$SECONDS
    while [[ ! -f $done ]]; do
        server_alive && seen=$SECONDS
        # A shot takes seconds and a renderer rebuild well under a minute.
        if (( SECONDS - asked > 180 )); then
            echo "shot.sh: no answer in 3 minutes: the server is stuck or dropped the request; its log:" >&2
            tr -d '\r' < "$server_dir/server.log" | tail -5 >&2
            rm -f "$req.req"
            return 1
        fi
        if (( SECONDS - seen > 30 )); then
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

# Design variants for one or more units. Round r swaps every unit's r-th variant
# in at once (one renderer rebuild) and shoots each unit as one row; each unit's
# rows then go into a labelled sheet of its own. 10 units x 3 variants is 3
# rebuilds and 30 one-second shots, not 30 rebuilds.
shoot_variants() {
    (cd "$repo" && cargo build -q --profile models -p mc-models --bin mc-models) \
        || { echo "shot.sh: mc-models did not build" >&2; exit 1; }
    trap 'rm -f "$server_dir"/calls-$$-*.bin "$server_dir"/meshes-$$-*.bin' EXIT
    local units=() lists=() spec k u r m rounds=0 first=("${reload[@]}")
    for spec in "${specs[@]}"; do
        units+=("${spec%%=*}")
        lists+=("${spec#*=}")
    done
    for u in "${!units[@]}"; do
        said=$(ask "${first[@]}" --calls-for "${units[$u]}" \
            --calls "$(wslpath -w "$server_dir/calls-$$-$u.bin")") \
            || { echo "$said" >&2; echo "shot.sh: no mesh calls for ${units[$u]}" >&2; exit 1; }
        first=()
        IFS=, read -ra m <<< "${lists[$u]}"
        (( ${#m[@]} > rounds )) && rounds=${#m[@]}
    done
    for ((r = 0; r < rounds; r++)); do
        local pairs=() shots=()
        for u in "${!units[@]}"; do
            IFS=, read -ra m <<< "${lists[$u]}"
            [[ $r -lt ${#m[@]} ]] || continue
            local calls="$server_dir/calls-$$-$u.bin" meshes="$server_dir/meshes-$$-$u-$r.bin" as=()
            [[ ${m[$r]} == base ]] || as=(--as "${m[$r]}")
            "$repo/target/models/mc-models" "$calls" "$meshes" "${as[@]}" \
                || { echo "shot.sh: mc-models could not build ${m[$r]}" >&2; exit 1; }
            pairs+=(--models "$(wslpath -w "$meshes")" --calls "$(wslpath -w "$calls")")
            shots+=("$u")
        done
        for u in "${shots[@]}"; do
            said=$(ask "${pairs[@]}" --unit-shot "${units[$u]}" "${view_args[@]}" \
                --screenshot "$(wslpath -w "$shots_win")\\$stem-$u-row$r.png") \
                || { echo "$said" >&2; echo "shot.sh: variant $r of ${units[$u]} failed" >&2; exit 1; }
            pairs=()
        done
    done
    t2=$(date +%s%N)
    echo "build $(( (t1 - t0) / 1000000 )) ms, shots $(( (t2 - t1) / 1000000 )) ms"
    for u in "${!units[@]}"; do
        k="${units[$u]}"
        # The server now holds a variant: the unit's next plain shot puts its own mesh back.
        mkdir -p "$model_stamps"
        touch -d @0 "$model_stamps/$k"
        IFS=, read -ra m <<< "${lists[$u]}"
        local rows=() sheet="$stem-$k.png"
        for r in "${!m[@]}"; do rows+=("$shots_win/$stem-$u-row$r.png"); done
        python3 - "$repo/artifacts/shots/$sheet" "$k" "${#rows[@]}" "${rows[@]}" "${m[@]}" <<'PY'
import sys
from PIL import Image, ImageDraw, ImageFont
out, unit, n = sys.argv[1], sys.argv[2], int(sys.argv[3])
paths, names = sys.argv[4:4 + n], sys.argv[4 + n:]
rows = [Image.open(p).convert("RGB") for p in paths]
band = 44
w = max(r.width for r in rows)
sheet = Image.new("RGB", (w, sum(r.height + band for r in rows)), (18, 22, 28))
draw = ImageDraw.Draw(sheet)
try:
    font = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", 26)
except OSError:
    font = ImageFont.load_default()
y = 0
for i, (row, name) in enumerate(zip(rows, names)):
    draw.text((14, y + 8), f"{unit}   {chr(65 + i)}: {name}", fill=(120, 220, 255), font=font)
    sheet.paste(row, (0, y + band))
    y += row.height + band
sheet.save(out)
PY
        rm -f "${rows[@]}"
        echo "$repo/artifacts/shots/$sheet"
    done
    exit 0
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
    variants)
        # KEY=MESH,MESH... per unit, or the one-unit form KEY MESH MESH...
        specs=()
        if [[ ${1:-} == *=* ]]; then
            while [[ $# -gt 0 && $1 != -* ]]; do specs+=("$1"); shift; done
        else
            [[ $# -gt 1 ]] || { echo "shot.sh variants needs a blueprint key and mesh keys" >&2; exit 2; }
            key="$1"; shift
            meshes=()
            while [[ $# -gt 0 && $1 != -* ]]; do meshes+=("$1"); shift; done
            specs=("$key=$(IFS=,; echo "${meshes[*]}")")
        fi
        [[ ${#specs[@]} -gt 0 ]] || { echo "shot.sh variants needs KEY=MESH,MESH..." >&2; exit 2; }
        key="${specs[0]%%=*}"
        [[ " $* " == *" --views "* ]] || set -- --views front34,left,rear34 "$@"
        view_args=("$@")
        game_args=(--unit-shot "$key" "$@")
        stem="${out_name:-variants}" ;;
    run)
        game_args=("$@")
        stem="${out_name:-shot}" ;;
    *) echo "shot.sh: unknown mode $mode (unit, variants, run or stop)" >&2; exit 2 ;;
esac
stem="${stem%.png}-$(date +%H%M%S)"

t0=$(date +%s%N)
[[ -f $built ]] || build=1
# Build only when a source file is newer than the last build. The stamp is
# taken before cargo starts, so an edit made during the build is seen next time.
[[ $build == 0 ]] || (
    flock -n 9 || {
        echo "shot.sh: waiting for another session's build..." >&2
        flock -w 600 9 || { echo "shot.sh: gave up after 10 minutes waiting for another session's build" >&2; exit 1; }
    }
    changed=""
    if [[ $build == 1 ]]; then
        if [[ ! -f $built_stamp || ! -f $built ]]; then
            changed=yes
            # A game another checkout built from these same engine sources will do
            # (a fresh worktree off dev): its model code and shaders reach its
            # server without a build. A run-mode shot draws what the exe has built
            # in, so that needs the model code and shaders to match too.
            key=$(tree_hash engine)
            if [[ -f $exes/$key/meridian.exe ]] \
                && { [[ $mode != run ]] || [[ $(cat "$exes/$key/looks") == "$(tree_hash looks)" ]]; }; then
                mkdir -p "$(dirname "$built")"
                cp -p "$exes/$key/meridian.exe" "$built"
                cp "$exes/$key/looks" "$built_looks"
                echo "$key" > "$built_engine"
                touch "$exes/$key" "$built_stamp"
                echo "shot.sh: game $key was built already; no build" >&2
                changed=""
            fi
        else
            # A unit shot's server recompiles edited shaders itself (--shaders) and
            # takes edited models from mc-models (--models); gpu_consts.rs is
            # shared with the renderer and shaders, so it still needs a build.
            skip=()
            [[ $mode != run ]] && skip=(-not -path "$shaders/*"
                -not \( -path "$models/*" -not -name gpu_consts.rs \))
            changed=$(find "$repo/crates" "$repo/Cargo.toml" "$repo/Cargo.lock" -newer "$built_stamp" -type f \
                -not -path '*/target/*' "${skip[@]}" -print -quit)
            # A game taken from another checkout has its model code and shaders,
            # which may be older than the stamp here and still differ.
            [[ -z $changed && $mode == run && $(cat "$built_looks" 2>/dev/null) != "$(tree_hash looks)" ]] \
                && changed=yes
        fi
    fi
    if [[ -n $changed ]]; then
        touch "$built_stamp.next"
        key=$(tree_hash engine) looks=$(tree_hash looks)
        log="$bin_dir/build-$$.log"
        if ! powershell.exe -NoProfile -Command "Set-Location '$repo_win'; cargo build --profile shot -p mc-game --target-dir '$target_win' *> '$(wslpath -w "$log")'; exit \$LASTEXITCODE"; then
            tr -d '\r' < "$log" | grep -v '^\s*Compiling' | tail -60 >&2
            rm -f "$log" "$built_stamp.next"
            echo "shot.sh: build failed" >&2
            exit 1
        fi
        tr -d '\r' < "$log" | grep -E '^(warning|error)' | sort -u | head -5 >&2 || true
        rm -f "$log"
        mv "$built_stamp.next" "$built_stamp"
        echo "$looks" > "$built_looks"
        # Kept for other checkouts by what it was built from, unless a source
        # changed during the build. Never overwritten: a server may be running it.
        rm -f "$built_engine"
        if [[ $(tree_hash engine) == "$key" ]]; then
            echo "$key" > "$built_engine"
            if [[ ! -d $exes/$key ]]; then
                mkdir -p "$exes/$key.$$"
                cp -p "$built" "$exes/$key.$$/meridian.exe"
                echo "$looks" > "$exes/$key.$$/looks"
                mv -T "$exes/$key.$$" "$exes/$key" 2>/dev/null || rm -rf "$exes/$key.$$"
            fi
            # The last dozen games stay; one a server is running cannot be deleted.
            ls -1dt "$exes"/*/ 2>/dev/null | tail -n +13 | xargs -r rm -rf 2>/dev/null || true
        fi
    fi
    [[ -f $built ]] || { echo "shot.sh: no build yet at $built (drop --no-build)" >&2; exit 1; }
) 9>"$lock"
t1=$(date +%s%N)

out_win="$(wslpath -w "$shots_win")\\$stem.png"
if [[ $mode != run ]]; then
    # The server runs a copy of the build it was started from; a new build replaces it.
    exe_id=$(stat -c '%Y-%s' "$built")
    # One session at a time checks and (re)starts the server; waiting requests
    # stay queued across a restart and the new server answers them.
    exec 8>"$lock.server"
    flock -w 120 8
    if ! server_alive || [[ $(cat "$server_dir/exe" 2>/dev/null) != "$exe_id" ]]; then
        # An older build's server sees `exe` change below and leaves by itself
        # once its current request is done; its queue passes to the new one.
        # Always the same path: the GPU driver keeps its compiled pipelines per
        # program, and a new name each time would compile them all again (~15 s).
        # Under the build lock, so another session's link cannot be caught half
        # done; retried while Windows still holds the old server's file.
        # A game kept by its engine sources runs from its own path in the store,
        # which every checkout on those sources shares, so their servers share the
        # driver's pipelines too.
        exe="$server_dir/meridian-server.exe"
        engine=$(cat "$built_engine" 2>/dev/null || true)
        if [[ -n $engine && -f $exes/$engine/meridian.exe \
            && $(stat -c '%Y-%s' "$exes/$engine/meridian.exe") == "$exe_id" ]]; then
            exe="$exes/$engine/meridian.exe"
            touch "$exes/$engine"
        fi
        old_id=$(cat "$server_dir/exe" 2>/dev/null || true)
        echo "$exe_id" > "$server_dir/exe"
        # A server from before servers retired themselves is told to quit, by
        # build, so the quit cannot reach the new one.
        if server_alive && [[ -n $old_id ]]; then
            sleep 2
            server_alive && { ask --quit "$old_id" > /dev/null 2>&1 || true; }
        fi
        [[ $exe != "$server_dir"/* ]] || (
            flock -w 600 9
            for _ in $(seq 300); do cp -p "$built" "$exe" 2>/dev/null && exit 0; sleep 0.2; done
            exit 1
        ) 9>"$lock" || { echo "shot.sh: the old server did not let go of $exe within a minute" >&2; exit 1; }
        # Started through a launcher file (no redirection on the outer Start-Process,
        # which would hand the server this pipe and keep the call from returning). The
        # server runs in the \\wsl.localhost repo itself, as `run` shots do: a drive
        # mapped onto it with pushd went bad under a long-lived server ("map i/o: An
        # unexpected network error occurred. (os error 59)"). Its stderr goes to a file.
        # Backgrounded too, so a launcher that lingers cannot hold anyone up.
        server_win=$(wslpath -w "$server_dir")
        printf '@echo off\r\n%s\r\nset RUST_LOG=info,mc_render::renderer=debug,mc_render::warm=debug\r\npowershell -NoProfile -Command "Start-Process -NoNewWindow -Wait -FilePath \x27%s\x27 -ArgumentList \x27--shot-server\x27,\x27%s\x27 -WorkingDirectory \x27%s\x27 -RedirectStandardError \x27%s\\server.log\x27"\r\n' \
            "${cmd_env%$'\r'}" "$(wslpath -w "$exe")" "$server_win" "$repo_win" "$server_win" > "$server_dir/start.cmd"
        # The lock descriptors are closed for it: a launcher holding the server lock
        # would block every session's shots.
        powershell.exe -NoProfile -Command "Start-Process -WindowStyle Hidden -FilePath '$server_win\\start.cmd'" \
            < /dev/null > /dev/null 2>&1 8>&- 9>&- &
        for _ in $(seq 100); do server_alive && break; sleep 0.1; done
        server_alive || { echo "shot.sh: the shot server did not start" >&2; tr -d '\r' < "$server_dir/server.log" | tail -20 >&2; exit 1; }
        touch "$data_stamp"
        # The new server has the shaders and models its build compiled: those
        # here, unless they changed since (or the game came from another checkout),
        # when the first shot recompiles every shader and rebuilds its unit's meshes.
        rm -rf "$model_stamps"
        mkdir -p "$model_stamps"
        { read -r built_shaders built_models < "$built_looks"; } 2>/dev/null || true
        touch -r "$built_stamp" "$shader_stamp" "$model_stamps/.base"
        [[ ${built_shaders:-} == "$(tree_hash shaders)" ]] || touch -d @0 "$shader_stamp"
        [[ ${built_models:-} == "$(tree_hash models)" ]] || touch -d @0 "$model_stamps/.base"
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
    # A model edit since this unit's meshes were last made: build the small
    # mc-models program here (unoptimised, seconds), have it rebuild the meshes
    # the server's renderer asked for, and send them over.
    mkdir -p "$model_stamps"
    model_stamp="$model_stamps/$key"
    [[ -f $model_stamp ]] || touch -r "$model_stamps/.base" "$model_stamp" 2>/dev/null \
        || touch -r "$built_stamp" "$model_stamp"
    # (Variants build their own meshes: shoot_variants.)
    if [[ $mode == unit && -n $(find "$models" -newer "$model_stamp" -type f -name '*.rs' -print -quit) ]]; then
        touch "$model_stamp.next"
        (cd "$repo" && cargo build -q --profile models -p mc-models --bin mc-models) \
            || { rm -f "$model_stamp.next"; echo "shot.sh: mc-models did not build" >&2; exit 1; }
        calls="$server_dir/calls-$$.bin" meshes="$server_dir/meshes-$$.bin"
        trap 'rm -f "$calls" "$meshes"' EXIT
        ask --calls-for "$key" --calls "$(wslpath -w "$calls")" > /dev/null
        "$repo/target/models/mc-models" "$calls" "$meshes" \
            || { rm -f "$model_stamp.next"; echo "shot.sh: mc-models could not build the meshes" >&2; exit 1; }
        reload+=(--models "$(wslpath -w "$meshes")" --calls "$(wslpath -w "$calls")")
        mv "$model_stamp.next" "$model_stamp"
    fi
    if [[ $mode == unit ]]; then
        ask "${reload[@]}" "${game_args[@]}" --screenshot "$out_win"
    else
        shoot_variants
    fi
else
    # A few fixed exe paths, one per run at a time: the GPU driver keeps compiled
    # pipelines per program path, so a slot reused shot after shot starts warm.
    # A slot is taken when its lock is free and its exe is current or can be
    # replaced: an exe still running (a shot whose script was killed) cannot be.
    exe_id=$(stat -c '%Y-%s' "$built")
    exe=""
    for pass in 1 2 3 4 5 6 7 8 9 10; do
        for slot in 0 1 2 3 4 5 6 7; do
            exec 7>"$bin_dir/run-$checkout-$slot.lock"
            flock -n 7 || continue
            candidate="$bin_dir/meridian-$checkout-run$slot.exe"
            if [[ $(cat "$candidate.id" 2>/dev/null) == "$exe_id" ]] \
                || (flock -w 600 9; cp -p "$built" "$candidate" 2>/dev/null) 9>"$lock"; then
                echo "$exe_id" > "$candidate.id"
                exe=$candidate
                break 2
            fi
            exec 7>&-
        done
        sleep 3
    done
    [[ -n $exe ]] || { echo "shot.sh: every run slot is busy" >&2; exit 1; }
    # PowerShell's own quoting: each argument in single quotes, any ' doubled.
    ps_args=""
    for a in "${game_args[@]}" --screenshot "$out_win"; do
        ps_args+=" '${a//\'/\'\'}'"
    done
    # A run-mode shot takes 15-60 s; at 3 minutes it is stuck. Killing the
    # PowerShell side alone would leave the game running, so it goes by path.
    set +e
    timeout 180 powershell.exe -NoProfile -Command "${ps_env}Set-Location '$repo_win'; & '$(wslpath -w "$exe")'$ps_args 2>&1 | ForEach-Object { \"\$_\" } | Where-Object { \$_ -notmatch '^\s+\S+\s+[0-9.]+ ms/tick' }; exit \$LASTEXITCODE" | tr -d '\r'
    status=${PIPESTATUS[0]}
    set -e
    if (( status == 124 )); then
        powershell.exe -NoProfile -Command "Get-Process | Where-Object { \$_.Path -eq '$(wslpath -w "$exe")' } | Stop-Process -Force" > /dev/null 2>&1
        echo "shot.sh: the shot did not finish in 3 minutes (stopped)" >&2
        exit 1
    elif (( status != 0 )); then
        exit "$status"
    fi
fi
t2=$(date +%s%N)

[[ -f "$shots_win/$stem.png" ]] || { echo "shot.sh: the game wrote no picture" >&2; exit 1; }
cp "$shots_win/$stem.png" "$repo/artifacts/shots/"
echo "build $(( (t1 - t0) / 1000000 )) ms, shot $(( (t2 - t1) / 1000000 )) ms"
echo "$repo/artifacts/shots/$stem.png"
