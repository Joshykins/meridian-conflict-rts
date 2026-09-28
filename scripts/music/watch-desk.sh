#!/usr/bin/env bash
# Streams what happens at the studio desk, one line per event, for Claude to watch
# from a chat (the Monitor tool): "Tell Claude" messages (marked read as they are
# printed, so the studio shows them read) and proposal verdicts or reactions.
#
#   scripts/music/watch-desk.sh <song> [mc-music binary]
set -uo pipefail
cd "$(dirname "$0")/../.."
song="${1:?song}"
mc="${2:-${MC_MUSIC:-mc-music}}"
last_log=""
while true; do
    "$mc" inbox --read 2>/dev/null | grep -v "^no unread messages" | sed 's/^/[message] /'
    # Without the "5m ago" part, which changes every minute.
    now=$("$mc" log "$song" 2>/dev/null | grep -E "^p[0-9]+ (Accepted|Rejected|Superseded)|^    > " | sed -E 's/ [0-9]+[smhd] ago//' || true)
    if [ "$now" != "$last_log" ]; then
        if [ -n "$last_log" ]; then
            diff <(echo "$last_log") <(echo "$now") | grep "^>" | sed 's/^> /[verdict] /'
        fi
        last_log="$now"
    fi
    sleep 2
done
