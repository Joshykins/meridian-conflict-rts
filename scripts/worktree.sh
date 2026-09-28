#!/usr/bin/env bash
# One worktree per session (CLAUDE.md section 1): your own branch, build,
# target dirs and shot server, so another session's half-done edit never
# breaks your build and your shots never wait on their builds.
#
#   scripts/worktree.sh start TOPIC   make ../mc-TOPIC on a new branch TOPIC from
#                                     dev, with the baked maps; prints its path
#   scripts/worktree.sh land          from inside the worktree: everything must be
#                                     committed; rebases onto dev, runs
#                                     scripts/check.sh, fast-forwards dev to it
#   scripts/worktree.sh land --no-check
#                                     the same without check.sh (a docs or data
#                                     change you have already checked)
#   scripts/worktree.sh finish        from inside the worktree, after landing:
#                                     stops its shot server and removes the
#                                     worktree, its branch and its build dirs
#
# Land small and often (every unit, every fix): a branch that lives for hours
# collects conflicts. Builds in a new worktree share compiled dependencies
# through sccache when it is installed (scripts/check.sh and shot.sh use it).
set -euo pipefail

main=$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")
here=$(git rev-parse --show-toplevel)
cmd=${1:-}

case "$cmd" in
    start)
        topic=${2:?"usage: scripts/worktree.sh start TOPIC"}
        [[ $topic =~ ^[a-z0-9][a-z0-9-]*$ ]] || { echo "worktree.sh: TOPIC is lower-case letters, digits and -" >&2; exit 2; }
        dir="$(dirname "$main")/mc-$topic"
        [[ ! -e $dir ]] || { echo "worktree.sh: $dir already exists" >&2; exit 1; }
        git -C "$main" worktree add -q "$dir" -b "$topic" dev
        # Hard links, not symlinks: the Windows build reads the maps through
        # \\wsl.localhost, which does not follow symlinks.
        for m in "$main"/maps/*.mcmap; do
            [[ -e $m ]] && { ln "$m" "$dir/maps/" 2>/dev/null || cp "$m" "$dir/maps/"; }
        done
        echo "$dir"
        ;;
    land)
        [[ $here != "$main" ]] || { echo "worktree.sh: land from inside your worktree, not the main checkout" >&2; exit 2; }
        branch=$(git rev-parse --abbrev-ref HEAD)
        if [[ -n $(git status --porcelain --untracked-files=no) ]]; then
            echo "worktree.sh: commit your work first:" >&2
            git status --short --untracked-files=no >&2
            exit 1
        fi
        git rebase -q dev || { echo "worktree.sh: the rebase onto dev stopped on a conflict: resolve it, git rebase --continue, and land again" >&2; exit 1; }
        if [[ ${2:-} != --no-check ]]; then
            scripts/check.sh || { echo "worktree.sh: check.sh failed; fix it (or tell the owner of a failure that is not yours) and land again" >&2; exit 1; }
        fi
        # Fast-forward only: dev gets exactly the commits checked here. Git refuses,
        # and changes nothing, if a file it would update is being edited in the
        # main checkout.
        before=$(git -C "$main" rev-parse dev)
        if ! git -C "$main" merge -q --ff-only "$branch"; then
            echo "worktree.sh: dev could not fast-forward (dev moved, or the main checkout has edits in these files); land again" >&2
            exit 1
        fi
        echo "landed $(git rev-list --count "$before..dev") commit(s) of $branch on dev"
        ;;
    finish)
        [[ $here != "$main" ]] || { echo "worktree.sh: finish from inside your worktree" >&2; exit 2; }
        branch=$(git rev-parse --abbrev-ref HEAD)
        git merge-base --is-ancestor "$branch" dev \
            || { echo "worktree.sh: $branch has commits dev does not; land them first" >&2; exit 1; }
        [[ -z $(git status --porcelain --untracked-files=no) ]] \
            || { echo "worktree.sh: $here has uncommitted edits" >&2; exit 1; }
        scripts/shot.sh stop 2> /dev/null || true
        checkout=$(basename "$here")
        temp_cache="/tmp/meridian-shot-wintemp"
        if [[ -s $temp_cache ]]; then
            win_temp=$(cat "$temp_cache")
            rm -rf "$win_temp/meridian-target-shot-$checkout" "$win_temp/meridian-shot-server-$checkout" 2> /dev/null || true
        fi
        cd "$main"
        git worktree remove --force "$here"
        git branch -q -d "$branch"
        echo "removed $here and branch $branch"
        ;;
    *)
        sed -n '2,/^set -euo/p' "$0" | grep '^#' | sed 's/^# \{0,1\}//'
        exit 2
        ;;
esac
