#!/usr/bin/env bash
# Commits exactly the named paths, as they are in the working tree, and nothing
# else: other sessions' edits and anything they staged stay out of the commit.
#
#   scripts/commit.sh -m "message" path...
#   scripts/commit.sh -F message.txt path...
#
# New, changed and deleted files all work; a directory means everything under
# it. The pre-commit hook runs on just these paths. Git allows one index writer
# at a time, so when another session is committing this waits and retries.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

[ $# -ge 3 ] && { [ "$1" = -m ] || [ "$1" = -F ]; } || {
    echo "usage: scripts/commit.sh -m MESSAGE|-F FILE path..." >&2
    exit 2
}
flag=$1 msg=$2
shift 2

for attempt in $(seq 1 20); do
    if err=$( { git add -A -- "$@" && git commit "$flag" "$msg" --only -- "$@"; } 2>&1 ); then
        echo "$err" | tail -n 3
        exit 0
    fi
    if grep -q 'index.lock' <<<"$err"; then
        sleep 1
        continue
    fi
    echo "$err" >&2
    exit 1
done
echo "commit.sh: the index stayed locked for 20 s; is a git process stuck?" >&2
exit 1
