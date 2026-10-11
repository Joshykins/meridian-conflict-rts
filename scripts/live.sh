#!/usr/bin/env bash
# Work the test range while it is being played: lay structures and prefabs, read
# the range back, move the camera, take a screenshot. The game (in the test range)
# watches its mailbox, %APPDATA%\meridian-conflict\live, and answers each request
# there; this drops a request in and prints the answer (JSON).
#
#   scripts/live.sh state [--units]    map, camera, pointer, every structure (id, key,
#                                      owner, x, y, turn, build, selected), the
#                                      player's prefabs; --units adds mobile units
#   scripts/live.sh lay FILE|-|JSON    lay structures; the body of a "lay" request:
#       {"structures": [{"key": "...", "x": 0, "y": 24, "turn": 1}, ...]}
#                                      or {"name": "<a prefab in the library>"}, and
#       "at": [x, y] | "camera" (default) | "cursor" | "pad"
#                                      where the middle goes (snapped to the 12 m grid);
#                                      members are metres east/north of it
#       "turn": "Q" | "Qm"             quarter turns anticlockwise, m mirrored first
#       "how": "done" (default) | "sites" (unbuilt) | "build" (engineers build it,
#                                      in the order given)
#       "builder": KEY, "builders": N  how=build: who (default: the lowest tech
#                                      engineer that builds all of it), how many
#       "owner": SLOT                  default 0 (blue)
#       "free_build": true|false       for the owner, first
#       "dry": true                    only say where each goes and whether it fits
#                                      The answer lists each site: fits ("yes" or
#                                      why not), laid.
#   scripts/live.sh check FILE|-|JSON  the same as lay with "dry": true
#   scripts/live.sh capture [ID...]    the selected structures (or these ids) as a
#                                      prefab, as Save Prefab makes it
#   scripts/live.sh keep FILE|-|JSON   {"name": "...", "members": [...]} into the
#                                      player's prefab library (replacing its name)
#   scripts/live.sh remove ID...       take units off, no wreck
#   scripts/live.sh remove-area X0 Y0 X1 Y1
#   scripts/live.sh clear              empty the range
#   scripts/live.sh camera X Y [ZOOM [YAW]]
#                                      look at X,Y from ZOOM metres, turned YAW degrees
#   scripts/live.sh shot [-o NAME]     screenshot of the player's screen, to
#                                      artifacts/live/NAME.png (default: the time)
#   scripts/live.sh send FILE|-|JSON   any request as it is ({"op": ..., ...})
#
# The game must be in the test range; "no game is listening" otherwise. A refused
# request answers {"ok": false, "error": "..."} and exits 1.
#
set -euo pipefail

repo=$(git rev-parse --show-toplevel)
usage() { sed -n '2,/^set -euo/p' "$0" | grep '^#' | sed 's/^# \{0,1\}//'; }
[[ $# -gt 0 ]] || { usage >&2; exit 2; }
case "$1" in -h|--help) usage; exit 0 ;; esac

# Windows' %APPDATA%, asked once (each powershell.exe start costs ~0.5 s).
appdata_cache="/tmp/meridian-live-appdata"
[[ -s $appdata_cache ]] || wslpath "$(powershell.exe -NoProfile -Command '$env:APPDATA' | tr -d '\r')" > "$appdata_cache"
box="$(cat "$appdata_cache")/meridian-conflict/live"

# A JSON body from a file, stdin, or the argument itself.
body() {
    case "$1" in
        -) cat ;;
        \{*) printf '%s' "$1" ;;
        *) cat "$1" ;;
    esac
}

listening() {
    local beat
    beat=$(stat -c %Y "$box/alive" 2>/dev/null) || return 1
    (( $(date +%s) - beat < 5 ))
}

# Drops request $1 (JSON) in the mailbox, waits for the answer, prints it.
send() {
    listening || { echo "live.sh: no game is listening: start the test range (mailbox $box)" >&2; exit 1; }
    local id req reply
    id="$(date +%s%N)-$$"
    req="$box/$id.req.json"
    reply="$box/$id.reply.json"
    printf '%s' "$1" > "$req.part"
    mv "$req.part" "$req"
    for _ in $(seq 300); do
        if [[ -e $reply ]]; then
            answer=$(cat "$reply")
            rm -f "$reply"
            printf '%s\n' "$answer"
            [[ $(jq -r '.ok' <<< "$answer") == true ]] || exit 1
            return 0
        fi
        sleep 0.1
    done
    rm -f "$req"
    echo "live.sh: no answer in 30 s (did the game leave the range?)" >&2
    exit 1
}

cmd="$1"; shift
case "$cmd" in
    state)
        units=false
        [[ ${1:-} == --units ]] && units=true
        send "{\"op\":\"state\",\"units\":$units}" ;;
    lay) send "$(body "${1:--}" | jq -c '. + {op: "lay"}')" ;;
    check) send "$(body "${1:--}" | jq -c '. + {op: "lay", dry: true}')" ;;
    capture) send "$(jq -cn --args '{op: "capture", ids: ($ARGS.positional | map(tonumber))}' "$@")" ;;
    keep) send "$(body "${1:--}" | jq -c '{op: "keep", prefab: .}')" ;;
    remove) send "$(jq -cn --args '{op: "remove", ids: ($ARGS.positional | map(tonumber))}' "$@")" ;;
    remove-area)
        [[ $# -eq 4 ]] || { echo "usage: live.sh remove-area X0 Y0 X1 Y1" >&2; exit 2; }
        send "$(jq -cn --args '{op: "remove", area: ($ARGS.positional | map(tonumber))}' "$@")" ;;
    clear) send '{"op":"clear"}' ;;
    camera)
        [[ $# -ge 2 ]] || { echo "usage: live.sh camera X Y [ZOOM [YAW]]" >&2; exit 2; }
        send "$(jq -cn --argjson x "$1" --argjson y "$2" --argjson z "${3:-null}" --argjson w "${4:-null}" \
            '{op: "camera", at: [$x, $y]} + (if $z then {zoom: $z} else {} end) + (if $w then {yaw: $w} else {} end)')" ;;
    shot)
        name="live-$(date +%Y%m%d-%H%M%S)"
        [[ ${1:-} == -o ]] && name="$2"
        answer=$(send '{"op":"shot"}')
        file=$(jq -r '.shot' <<< "$answer")
        mkdir -p "$repo/artifacts/live"
        mv "$box/$file" "$repo/artifacts/live/$name.png"
        echo "$repo/artifacts/live/$name.png" ;;
    send) send "$(body "${1:--}" | jq -c .)" ;;
    *) usage >&2; exit 2 ;;
esac
