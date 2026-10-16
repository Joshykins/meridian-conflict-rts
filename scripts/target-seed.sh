#!/usr/bin/env bash
# A shared copy of a recent checked build, so a new worktree's first build
# compiles only what differs from it, not the whole workspace.
#
#   scripts/target-seed.sh take    fill this checkout's target/debug and
#                                  target/gate from the seed, if they are empty
#   scripts/target-seed.sh give    refresh the seed from this checkout's
#                                  target/debug and target/gate
#
# check.sh takes before it builds and gives after the checks pass. A clean
# checkout gives everything, its workspace crates too, with the commit they
# were built from, at most every 15 minutes (or at once when Cargo.lock, the
# profiles or the toolchain change); one with uncommitted edits gives only the
# third-party crates.
#
# Taking makes the seed's outputs count as fresh only where they are: every
# checked-in file the seed's commit has unchanged is dated 2000 (and so is
# every directory with no change below it), so cargo sees the outputs as newer;
# a file that differs keeps its own date and what it feeds is rebuilt. Cargo
# records a source outside a unit's own crate (the workspace Cargo.toml, a
# `#[path]` module or an `include_bytes!` from data/) by its absolute path, so
# those paths are rewritten from the seed's checkout to this one; otherwise
# cargo would judge them by the other checkout's copies. A unit that compiled
# in its checkout's path (`env!` of CARGO_MANIFEST_DIR, the tests' temp dir) is
# always rebuilt, so no binary here reads another checkout's files.
#
# Libraries (deps/*.rlib, .rmeta, .so) are hard links, so they cost no disk and
# taking is quick: rustc replaces one by renaming a new file over it, so a
# checkout that rebuilds a crate never changes the seed's copy. Everything else
# (binaries, which a linker may write in place, build-script outputs and
# fingerprints) is copied. Incremental caches are left out: rustc cannot reuse
# them from another checkout's path.
set -euo pipefail
export LC_ALL=C # sort and comm agree on the order
root=$(cd "$(dirname "$0")/.." && pwd)
seed=${MERIDIAN_TARGET_SEED:-$HOME/.cache/meridian-conflict/target-seed}
profiles=(debug gate)
old='2000-01-01 00:00:00'

# The fingerprint hashes of the workspace crates' units in target/PROFILE.
workspace_units() {
    local dir=$1
    [ -d "$dir/.fingerprint" ] || return 0
    find "$dir/.fingerprint" -maxdepth 1 -name 'mc-*' -printf '%f\n' | sed 's/.*-//'
}

# FROM's outputs into TO: libraries linked, the rest copied (see above).
copy() {
    local from=$1 to=$2
    mkdir -p "$to/deps"
    if [ -d "$from/deps" ]; then
        find "$from/deps" -maxdepth 1 -type f \( -name '*.rlib' -o -name '*.rmeta' -o -name '*.so' \) \
            -exec cp -l -t "$to/deps" {} +
        find "$from/deps" -maxdepth 1 -type f ! \( -name '*.rlib' -o -name '*.rmeta' -o -name '*.so' \) \
            -exec cp -a -t "$to/deps" {} +
    fi
    local d
    for d in build .fingerprint examples; do
        [ -d "$from/$d" ] && cp -a "$from/$d" "$to/$d"
    done
    return 0
}

# Points the absolute paths under FROM recorded in DIR's fingerprints at this
# checkout instead, keeping each file's date (cargo compares sources against
# it). Prints the units whose records it cannot read, to be rebuilt.
relocate() {
    python3 -I - "$1/" "$root/" "$2/.fingerprint" << 'EOF'
import os, struct, sys
old, new, top = sys.argv[1].encode(), sys.argv[2].encode(), sys.argv[3]
for unit in sorted(os.listdir(top)):
    for name in os.listdir(os.path.join(top, unit)):
        path = os.path.join(top, unit, name)
        if not name.startswith("dep-"):
            continue
        data = open(path, "rb").read()
        out, at, ok = bytearray(), 0, True
        while (p := data.find(old, at)) >= 0:
            # Each path is a little-endian u32 length and then its bytes.
            n = struct.unpack_from("<I", data, p - 4)[0] if p >= 4 else 0
            if n < len(old) or p + n > len(data) or b"\0" in data[p:p + n]:
                ok = False
                break
            out += data[at:p - 4] + struct.pack("<I", n - len(old) + len(new)) + new
            at = p + len(old)
        if not ok:
            print(unit)
            break
        if at:
            out += data[at:]
            st = os.stat(path)
            with open(path, "wb") as f:
                f.write(out)
            os.utime(path, ns=(st.st_atime_ns, st.st_mtime_ns))
EOF
}

# Dates every checked-in file and directory that BASE has unchanged to 2000.
age_sources() {
    local base=$1 changed
    git -C "$root" cat-file -e "$base^{commit}" 2> /dev/null || {
        echo "== the seed's commit $base is not in this repository; building its workspace crates afresh"
        return 0
    }
    changed=$( (git -C "$root" diff --name-only "$base" HEAD; git -C "$root" diff --name-only HEAD) | sort -u)
    (cd "$root" && git ls-files | sort | comm -23 - <(echo "$changed") | tr '\n' '\0' | xargs -0 -r touch -h -d "$old")
    # A directory is aged only if nothing under it changed (a build script may
    # watch a whole directory, and a deleted file leaves only its parent's date).
    (cd "$root" && find . -path ./target -prune -o -path ./.git -prune -o -type d -printf '%P\n' \
        | sort | comm -23 - <(echo "$changed" | awk -F/ '{ print ""; p = $1; print p; for (i = 2; i < NF; i++) { p = p "/" $i; print p } }' | sort -u) \
        | sed 's/^$/./' | tr '\n' '\0' | xargs -0 -r touch -d "$old")
}

case "${1:-}" in
    take)
        [ -d "$seed/current" ] || exit 0
        took=0
        from=$(cat "$seed/current/root" 2> /dev/null || true)
        for p in "${profiles[@]}"; do
            dir="$root/target/$p"
            [ -d "$seed/current/$p" ] || continue
            [ -d "$dir/deps" ] && continue
            mkdir -p "$dir"
            copy "$seed/current/$p" "$dir"
            # Sources recorded by the seed's path are judged by this checkout's
            # copies, and units built in the seed's own path rebuild here.
            if [ -n "$from" ]; then
                relocate "$from" "$dir" | while read -r unit; do
                    rm -rf "$dir/.fingerprint/$unit"
                done
                grep -laE "^# env-dep:(CARGO_MANIFEST_DIR|CARGO_TARGET_TMPDIR)=$from/" "$dir"/deps/*.d "$dir"/examples/*.d 2> /dev/null \
                    | while read -r d; do
                        hash=$(basename "$d" .d)
                        rm -rf "$dir"/.fingerprint/*-"${hash##*-}"
                    done || true
            fi
            echo "== seeded target/$p from $seed"
            took=1
        done
        if [ "$took" = 1 ] && [ -s "$seed/current/commit" ]; then
            age_sources "$(cat "$seed/current/commit")"
            echo "   with the workspace crates of $(git -C "$root" log -1 --format='%h %s' "$(cat "$seed/current/commit")" 2> /dev/null || cat "$seed/current/commit")"
        fi
        ;;
    give)
        # The lock file and the profiles decide what every third-party crate
        # builds to; the seed is refreshed at once when either changes.
        lock=$(cat "$root/Cargo.lock" "$root/Cargo.toml" "$root/rust-toolchain.toml" | sha256sum | cut -c1-16)
        clean=0
        [ -z "$(git -C "$root" status --porcelain --untracked-files=no)" ] && clean=1
        if [ "$(cat "$seed/current/lock" 2> /dev/null)" = "$lock" ]; then
            # Same third-party crates: only a clean checkout refreshes, and only
            # once the seed's workspace crates are a quarter of an hour old.
            [ "$clean" = 1 ] || exit 0
            [ -z "$(find "$seed/current/" -maxdepth 1 -name lock -mmin -15)" ] || exit 0
        fi
        mkdir -p "$seed"
        new="$seed/$lock-$$"
        rm -rf "$new"
        mkdir -p "$new"
        for p in "${profiles[@]}"; do
            dir="$root/target/$p"
            [ -d "$dir/deps" ] || continue
            copy "$dir" "$new/$p"
            [ "$clean" = 1 ] && continue
            # Workspace units, test binaries and all, by their unit hash.
            for hash in $(workspace_units "$dir"); do
                find "$new/$p" -maxdepth 2 -name "*-$hash*" -exec rm -rf {} +
            done
        done
        if [ "$clean" = 1 ]; then
            git -C "$root" rev-parse HEAD > "$new/commit"
            echo "$root" > "$new/root"
        fi
        echo "$lock" > "$new/lock"
        # Swap the link in one rename, so a checkout taking the seed at the same
        # moment sees the old one or the new one, never half of either.
        ln -sfn "$(basename "$new")" "$seed/current.$$"
        mv -T "$seed/current.$$" "$seed/current"
        # Old seeds are only links and copies; drop all but the current one.
        find "$seed" -mindepth 1 -maxdepth 1 -type d ! -name "$(basename "$new")" -mmin +10 \
            -exec rm -rf {} +
        ;;
    *)
        sed -n '2,/^set -euo/p' "$0" | grep '^#' | sed 's/^# \{0,1\}//'
        exit 2
        ;;
esac
