#!/usr/bin/env bash
# The channel branches (docs/RELEASES.md, "Branches"): dev, where every session
# lands; playtest, what playtesters run; master, what everyone runs. Each is
# always contained in the one below it, so a promotion is a fast-forward.
#
#   scripts/channel.sh status                  where each branch stands
#   scripts/channel.sh promote playtest        playtest <- dev
#   scripts/channel.sh promote release         master <- playtest
#   scripts/channel.sh hotfix playtest|release TOPIC
#                                              a worktree ../mc-hotfix-TOPIC on a new
#                                              branch hotfix-TOPIC from that channel's
#                                              branch; prints its path
#   scripts/channel.sh land                    from inside a hotfix worktree, all
#                                              committed: runs scripts/check.sh,
#                                              fast-forwards the channel's branch, then
#                                              merges it down (master -> playtest -> dev)
#   scripts/channel.sh merge-down CHANNEL      the merge-down alone, again after
#                                              resolving a conflict it stopped on
#   scripts/channel.sh finish                  from inside a landed hotfix worktree:
#                                              removes it and its branch
#
# Publishing is scripts/release.sh, which builds the channel branch's tip.
# Nothing here pushes: `git push origin dev playtest master` when you want to.
set -euo pipefail

main=$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")
here=$(git rev-parse --show-toplevel)

branch_of() {
    case $1 in
        playtest) echo playtest ;;
        release) echo master ;;
        *) echo "channel.sh: the channel is playtest or release" >&2; exit 2 ;;
    esac
}

# The branch one below (where a fix goes on to): master -> playtest -> dev.
below() {
    case $1 in
        master) echo playtest ;;
        playtest) echo dev ;;
    esac
}

ensure_branches() {
    for b in playtest master; do
        git rev-parse -q --verify "refs/heads/$b" >/dev/null || {
            echo "channel.sh: no $b branch yet; making it at dev"
            git branch "$b" dev
        }
    done
}

# Moves branch $1 to $2 if that is a fast-forward. A branch checked out in a
# worktree moves with `merge --ff-only` there, so its files follow.
fast_forward() {
    local b=$1 to=$2 old
    old=$(git rev-parse "refs/heads/$b")
    git merge-base --is-ancestor "$old" "$to" || {
        echo "channel.sh: $b has commits $to lacks; merge them down first (channel.sh merge-down)" >&2
        exit 1
    }
    local wt
    wt=$(git worktree list --porcelain | awk -v b="refs/heads/$b" '/^worktree /{w=$2} $0=="branch "b{print w}')
    if [[ -n $wt ]]; then
        git -C "$wt" merge -q --ff-only "$to"
    else
        git update-ref "refs/heads/$b" "$(git rev-parse "$to")" "$old"
    fi
}

# Merges branch $1 into each branch below it, in a scratch worktree, so the
# shared checkout is touched only by a fast-forward.
merge_down() {
    local from=$1 to
    while to=$(below "$from") && [[ -n $to ]]; do
        if git merge-base --is-ancestor "$from" "$to"; then
            from=$to
            continue
        fi
        local scratch
        scratch=$(dirname "$main")/mc-merge-$to
        if [[ -e $scratch ]]; then
            # Left by a merge that stopped on a conflict: carry on once it is committed.
            git -C "$scratch" merge-base --is-ancestor "$from" HEAD || {
                echo "channel.sh: $scratch holds an unfinished merge of $from into $to: resolve it, commit, and run merge-down again" >&2
                exit 1
            }
        else
            git worktree add -q --detach "$scratch" "$to"
            if ! git -C "$scratch" merge -q --no-ff -m "Merge $from into $to" "$from"; then
                echo "channel.sh: merging $from into $to stopped on a conflict in $scratch:" >&2
                echo "  resolve it and commit there, then run: scripts/channel.sh merge-down $(channel_of "$1")" >&2
                exit 1
            fi
        fi
        fast_forward "$to" "$(git -C "$scratch" rev-parse HEAD)"
        git worktree remove "$scratch"
        echo "merged $from into $to"
        from=$to
    done
}

channel_of() {
    case $1 in
        master) echo release ;;
        *) echo "$1" ;;
    esac
}

cmd=${1:-}
case $cmd in
    status)
        ensure_branches
        for b in master playtest dev; do
            printf "%-9s %s  build %s\n" "$b" "$(git log -1 --format='%h %cr  %s' "$b" | cut -c1-70)" "$(git rev-list --count "$b")"
        done
        echo "promote playtest would bring $(git rev-list --count playtest..dev) commit(s); promote release $(git rev-list --count master..playtest)"
        ;;
    promote)
        ensure_branches
        case ${2:-} in
            playtest) fast_forward playtest dev ;;
            release) fast_forward master playtest ;;
            *) echo "usage: scripts/channel.sh promote playtest|release" >&2; exit 2 ;;
        esac
        echo "promoted: scripts/release.sh ${2} publishes it"
        ;;
    hotfix)
        ensure_branches
        b=$(branch_of "${2:-}")
        topic=${3:?"usage: scripts/channel.sh hotfix playtest|release TOPIC"}
        [[ $topic =~ ^[a-z0-9][a-z0-9-]*$ ]] || { echo "channel.sh: TOPIC is lower-case letters, digits and -" >&2; exit 2; }
        dir=$(dirname "$main")/mc-hotfix-$topic
        git -C "$main" worktree add -q "$dir" -b "hotfix-$topic" "$b"
        git -C "$dir" config "branch.hotfix-$topic.channel" "$b"
        for m in "$main"/maps/*.mcmap; do
            [[ -e $m ]] && { ln "$m" "$dir/maps/" 2>/dev/null || cp "$m" "$dir/maps/"; }
        done
        echo "$dir"
        ;;
    land)
        topic_branch=$(git rev-parse --abbrev-ref HEAD)
        b=$(git config "branch.$topic_branch.channel" || true)
        [[ -n $b ]] || { echo "channel.sh: land from inside a hotfix worktree (channel.sh hotfix)" >&2; exit 2; }
        [[ -z $(git status --porcelain --untracked-files=no) ]] || { echo "channel.sh: commit your work first" >&2; exit 1; }
        git rebase -q "$b"
        scripts/check.sh
        fast_forward "$b" HEAD
        echo "landed on $b: scripts/release.sh $(channel_of "$b") publishes it"
        merge_down "$b"
        ;;
    merge-down)
        merge_down "$(branch_of "${2:-}")"
        ;;
    finish)
        topic_branch=$(git rev-parse --abbrev-ref HEAD)
        b=$(git config "branch.$topic_branch.channel" || true)
        [[ -n $b && $here != "$main" ]] || { echo "channel.sh: finish from inside a hotfix worktree" >&2; exit 2; }
        git merge-base --is-ancestor HEAD "$b" || { echo "channel.sh: $topic_branch is not landed on $b yet" >&2; exit 1; }
        cd "$main"
        git worktree remove "$here"
        git branch -q -D "$topic_branch"
        echo "removed $here"
        ;;
    *)
        sed -n '2,24p' "$0" | sed 's/^# \{0,1\}//'
        exit 2
        ;;
esac
