#!/usr/bin/env bash
# The channel branches (docs/RELEASES.md, "Branches"): dev, where every session
# lands; playtest, what playtesters run; release, what everyone runs. Each is
# always contained in the one below it, so a promotion is a fast-forward.
#
#   scripts/channel.sh status                  where each branch stands, here and on origin
#   scripts/channel.sh promote playtest [OPTS] playtest <- dev, then publish and push
#   scripts/channel.sh promote release [OPTS]  release <- playtest, then publish and push
#   scripts/channel.sh hotfix playtest|release TOPIC
#                                              a worktree ../mc-hotfix-TOPIC on a new
#                                              branch hotfix-TOPIC from that channel's
#                                              branch; prints its path
#   scripts/channel.sh land [OPTS]             from inside a hotfix worktree, all
#                                              committed: runs scripts/check.sh,
#                                              fast-forwards the channel's branch,
#                                              publishes it, merges it down
#                                              (release -> playtest -> dev) and pushes
#   scripts/channel.sh merge-down CHANNEL      the merge-down alone, again after
#                                              resolving a conflict it stopped on
#   scripts/channel.sh finish                  from inside a landed hotfix worktree:
#                                              removes it and its branch
#
# A promotion or a hotfix is one pass: the commit is checked (scripts/check.sh,
# in a worktree of its own), the branch moves, scripts/release.sh builds its tip
# and publishes it to the build store, and only then are the branch and a tag
# (builds/CHANNEL-N) pushed to origin. A commit that fails the check moves nothing. If the build or the
# upload fails, the branch goes back to where it was and nothing is pushed.
# OPTS go to release.sh: --notes FILE (the patch notes; else drafted from the
# commits), --no-upload (a trial: build the zip, publish and push nothing, and
# put the branch back).
set -euo pipefail

main=$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")
here=$(git rev-parse --show-toplevel)

branch_of() {
    case $1 in
        playtest) echo playtest ;;
        release) echo release ;;
        *) echo "channel.sh: the channel is playtest or release" >&2; exit 2 ;;
    esac
}

# The branch one below (where a fix goes on to): release -> playtest -> dev.
below() {
    case $1 in
        release) echo playtest ;;
        playtest) echo dev ;;
    esac
}

# A channel branch made here for the first time starts at dev's root, so its
# first promotion publishes.
ensure_branches() {
    local root
    root=$(git rev-list --max-parents=0 dev | tail -1)
    for b in playtest release; do
        git rev-parse -q --verify "refs/heads/$b" >/dev/null || {
            echo "channel.sh: no $b branch yet; making it"
            git branch "$b" "$root"
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
    echo "$1"
}

# Origin must not be ahead: a channel moved from another machine is fetched
# and merged here first, never overwritten.
in_step_with_origin() {
    git fetch -q origin
    local b
    for b in "$@"; do
        git rev-parse -q --verify "refs/remotes/origin/$b" >/dev/null || continue
        git merge-base --is-ancestor "origin/$b" "refs/heads/$b" || {
            echo "channel.sh: origin/$b has commits $b lacks (moved from another machine?): bring them in first" >&2
            exit 1
        }
    done
}

# Publishes channel $1 from its branch's tip; on failure puts each BRANCH=SHA
# given before `--` back at SHA, where it was before it moved. With
# --no-upload among the options after `--` it is a trial, and they go back anyway.
publish() {
    local channel=$1
    shift
    local -A was=()
    while (($#)) && [[ $1 != -- ]]; do was[${1%%=*}]=${1#*=}; shift; done
    shift
    local trial=0 o b
    for o in "$@"; do [[ $o == --no-upload ]] && trial=1; done
    put_back() {
        for b in "${!was[@]}"; do
            git update-ref "refs/heads/$b" "${was[$b]}"
            echo "channel.sh: $b put back at ${was[$b]:0:10}" >&2
        done
    }
    if ! "$here/scripts/release.sh" "$channel" "$@"; then
        echo "channel.sh: publishing $channel failed" >&2
        put_back
        exit 1
    fi
    if ((trial)); then
        echo "trial: built, nothing published or pushed"
        put_back
        exit 0
    fi
}

# Pushes the channel branches named and tags the published build.
push_published() {
    local channel=$1
    shift
    local b number
    b=$(branch_of "$channel")
    number=$(git rev-list --count "$b")
    local tag=builds/$channel-$number
    git tag -a "$tag" -m "$channel build $number" "$b"
    git push -q origin "$@" "$tag"
    echo "pushed $* and $tag"
}

cmd=${1:-}
case $cmd in
    status)
        ensure_branches
        git fetch -q origin || true
        for b in release playtest dev; do
            printf "%-9s %s  build %s" "$b" "$(git log -1 --format='%h %cr  %s' "$b" | cut -c1-60)" "$(git rev-list --count "$b")"
            if git rev-parse -q --verify "refs/remotes/origin/$b" >/dev/null; then
                printf "  (origin %s)" "$(git rev-parse --short "origin/$b")"
            fi
            echo
        done
        echo "promote playtest would bring $(git rev-list --count playtest..dev) commit(s); promote release $(git rev-list --count release..playtest)"
        ;;
    promote)
        ensure_branches
        channel=${2:-}
        shift 2 || true
        case $channel in
            playtest) from=dev ;;
            release) from=playtest ;;
            *) echo "usage: scripts/channel.sh promote playtest|release [--notes FILE] [--no-upload]" >&2; exit 2 ;;
        esac
        b=$(branch_of "$channel")
        in_step_with_origin "$from" "$b"
        [[ $(git rev-parse "$b") != "$(git rev-parse "$from")" ]] || { echo "channel.sh: $b is already at $from" >&2; exit 1; }
        target=$(git rev-parse "$from")
        "$here/scripts/check.sh" --head "$target" || { echo "channel.sh: $from fails the check; nothing promoted" >&2; exit 1; }
        [[ $(git rev-parse "$from") == "$target" ]] || { echo "channel.sh: $from moved during the check; run promote again" >&2; exit 1; }
        old=$(git rev-parse "$b")
        fast_forward "$b" "$target"
        echo "promoted $b to $from ($(git rev-list --count "$old..$b") commits); publishing"
        publish "$channel" "$b=$old" -- "$@"
        push_published "$channel" "$from" "$b"
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
        in_step_with_origin "$b" playtest dev
        git rebase -q "$b"
        scripts/check.sh
        old=$(git rev-parse "$b")
        fast_forward "$b" HEAD
        echo "landed on $b; publishing"
        publish "$(channel_of "$b")" "$b=$old" -- "${@:2}"
        merge_down "$b"
        case $b in
            release) push_published release release playtest dev ;;
            playtest) push_published playtest playtest dev ;;
        esac
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
        sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//'
        exit 2
        ;;
esac
