#!/usr/bin/env bash
# A shared copy of the third-party crates' build outputs, so a new worktree's
# first scripts/check.sh compiles only the workspace crates (about 250 crates,
# several CPU-minutes, are already built).
#
#   scripts/target-seed.sh take    fill this checkout's target/debug and
#                                  target/gate from the seed, if they are empty
#   scripts/target-seed.sh give    refresh the seed from this checkout's
#                                  target/debug and target/gate
#
# check.sh takes before it builds and gives after the checks pass, so the seed
# follows Cargo.lock, the profiles and the toolchain as they change. Workspace crates are left out: their build
# outputs are keyed by the checkout's path and no other checkout can use them.
# The compiled crates (deps/) are hard links, so a seed costs no disk and
# taking it is instant: rustc replaces an rlib or rmeta by renaming a new file
# over it, so a checkout that rebuilds a crate never changes the seed's copy.
# Build-script outputs and fingerprints, which are written in place, are copied.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
seed=${MERIDIAN_TARGET_SEED:-$HOME/.cache/meridian-conflict/target-seed}
profiles=(debug gate)

# The fingerprint hashes of the workspace crates' units in target/PROFILE.
workspace_units() {
    local dir=$1
    [ -d "$dir/.fingerprint" ] || return 0
    find "$dir/.fingerprint" -maxdepth 1 -name 'mc-*' -printf '%f\n' | sed 's/.*-//'
}

# deps/ as hard links, build/ and .fingerprint/ as copies (see above).
copy() {
    local from=$1 to=$2
    [ -d "$from/deps" ] && cp -al "$from/deps" "$to/deps"
    [ -d "$from/build" ] && cp -a "$from/build" "$to/build"
    [ -d "$from/.fingerprint" ] && cp -a "$from/.fingerprint" "$to/.fingerprint"
    return 0
}

case "${1:-}" in
    take)
        [ -d "$seed/current" ] || exit 0
        for p in "${profiles[@]}"; do
            dir="$root/target/$p"
            [ -d "$seed/current/$p" ] || continue
            [ -d "$dir/deps" ] && continue
            mkdir -p "$dir"
            copy "$seed/current/$p" "$dir"
            echo "== seeded target/$p from $seed"
        done
        ;;
    give)
        # The lock file and the profiles decide what every third-party crate
        # builds to; the seed is refreshed when either changes.
        lock=$(cat "$root/Cargo.lock" "$root/Cargo.toml" "$root/rust-toolchain.toml" | sha256sum | cut -c1-16)
        if [ "$(cat "$seed/current/lock" 2> /dev/null)" = "$lock" ]; then
            exit 0
        fi
        mkdir -p "$seed"
        new="$seed/$lock-$$"
        rm -rf "$new"
        mkdir -p "$new"
        for p in "${profiles[@]}"; do
            dir="$root/target/$p"
            [ -d "$dir/deps" ] || continue
            mkdir -p "$new/$p"
            copy "$dir" "$new/$p"
            # Workspace units, test binaries and all, by their unit hash.
            for hash in $(workspace_units "$dir"); do
                find "$new/$p" -maxdepth 2 -name "*-$hash*" -exec rm -rf {} +
            done
        done
        echo "$lock" > "$new/lock"
        # Swap the link in one rename, so a checkout taking the seed at the same
        # moment sees the old one or the new one, never half of either.
        ln -sfn "$(basename "$new")" "$seed/current.$$"
        mv -T "$seed/current.$$" "$seed/current"
        # Old seeds are only links; drop all but the current one.
        find "$seed" -mindepth 1 -maxdepth 1 -type d ! -name "$(basename "$new")" -mmin +10 \
            -exec rm -rf {} +
        ;;
    *)
        sed -n '2,/^set -euo/p' "$0" | grep '^#' | sed 's/^# \{0,1\}//'
        exit 2
        ;;
esac
